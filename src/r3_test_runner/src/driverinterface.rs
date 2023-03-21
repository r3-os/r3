//! Interface to a test driver
use std::{
    fmt::Write,
    future::Future,
    path::{Path, PathBuf},
    pin::{pin, Pin},
    time::Duration,
};
use tokio::io::AsyncReadExt;

use crate::{selection, subprocess, targets};

/// Interface to a test driver, encompassing the identity of a test driver crate
/// as well as a reference to its build output directory.
pub(crate) struct TestDriver {
    rustflags: String,
    exe_path: PathBuf,
    target: &'static dyn targets::Target,
    target_arch_opt: targets::BuildOpt,
    link_dir: tempdir::TempDir,
    meta: Meta,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum TestDriverNewError {
    #[error("Error while changing the current directory to {0:?}.")]
    CdError(PathBuf, #[source] std::io::Error),
    #[error("Could not gather the Cargo metadata using `cargo metadata`.")]
    CargoMetadata(#[source] subprocess::SubprocessError),
    #[error("Could not parse the Cargo metadata.")]
    CargoMetadataParse(#[source] serde_json::Error),
    #[error("Could not read the test driver metadata.")]
    DriverMetadata(#[source] std::io::Error),
    #[error("Could not parse the test driver metadata.")]
    DriverMetadataParse(#[source] toml::de::Error),
    #[error("{0:?} is not a valid driver path.")]
    BadDriverPath(PathBuf),
    #[error("Error while creating a temporary directory.")]
    TempDirError(#[source] std::io::Error),
    #[error("Error while writing {0:?}.")]
    WriteError(PathBuf, #[source] std::io::Error),
}

/// The additional parameters used while building the executable image of a test
/// driver.
pub(crate) struct BuildOpt {
    pub verbose: bool,
    pub log_level: LogLevel,
    pub small_rt: bool,
}

#[derive(Clone, Copy, clap::ValueEnum)]
pub(crate) enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum TestDriverRunError {
    #[error("Could not locate the compiled executable at {0:?}.")]
    ExeNotFound(PathBuf),
    #[error("The build command failed.")]
    BuildTest(#[source] subprocess::SubprocessError),
    #[error("Could not run the test '{0}'.")]
    Run(String, #[source] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum TestRunError {
    #[error("Timed out")]
    Timeout,
    #[error("The output is too long")]
    TooLong,
    #[error("'{0}'")]
    General(String),
}

/// Driver metadata (`TestDriver.toml`).
#[derive(Default, Debug, serde::Deserialize)]
struct Meta {
    tests: MetaTests,
}

/// A section of driver metadata describing the additional tests defined by a
/// test driver.
#[derive(Default, Debug, serde::Deserialize)]
struct MetaTests {
    /// The list of custom kernel tests defined by the test driver.
    kernel_tests: Vec<String>,
}

const CARGO_CMD: &str = "cargo";

impl TestDriver {
    pub(crate) fn new(
        driver_base_path: &Path,
        target: &'static dyn targets::Target,
        target_arch: &targets::Arch,
        target_arch_opt: targets::BuildOpt,
        additional_rustflags: String,
    ) -> impl Future<Output = Result<Self, TestDriverNewError>> {
        // Choose the right test driver for the given target architecture
        let crate_name = match target_arch {
            targets::Arch::Armv7A => "r3_port_arm_test_driver",
            targets::Arch::ArmM { .. } => "r3_port_arm_m_test_driver",
            targets::Arch::Riscv { .. } => "r3_port_riscv_test_driver",
        };

        // Locate the test driver's crate
        let crate_path = driver_base_path.join(crate_name);
        log::debug!("driver.crate_name = {crate_name:?}");
        log::debug!("driver.crate_path = {crate_path:?}");

        async move {
            if !crate_path.is_dir() {
                return Err(TestDriverNewError::BadDriverPath(crate_path));
            }

            Self::new_inner(
                crate_path,
                target,
                crate_name,
                target_arch_opt,
                additional_rustflags,
            )
            .await
        }
    }

    async fn new_inner(
        crate_path: PathBuf,
        target: &'static dyn targets::Target,
        crate_name: &'static str,
        target_arch_opt: targets::BuildOpt,
        additional_rustflags: String,
    ) -> Result<Self, TestDriverNewError> {
        // Move to the driver directory
        log::debug!("cd-ing to '{}'", crate_path.display());
        std::env::set_current_dir(&crate_path)
            .map_err(|e| TestDriverNewError::CdError(crate_path.clone(), e))?;

        // Load the driver metadata
        let meta_path = crate_path.join("TestDriver.toml");
        log::debug!("Loading driver metadata from '{}'", meta_path.display());
        let meta: Meta = match tokio::fs::read_to_string(meta_path).await {
            Ok(data) => {
                toml::de::from_str(&data).map_err(TestDriverNewError::DriverMetadataParse)?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::debug!("`TestDriver.toml` wasn't found; using the default metadata");
                Default::default()
            }
            Err(e) => {
                return Err(TestDriverNewError::DriverMetadata(e));
            }
        };
        log::trace!("driver.meta = {meta:?}");

        // Find the target directory
        let target_dir = {
            let metadata_json = subprocess::CmdBuilder::new(CARGO_CMD)
                .arg("metadata")
                .arg("--format-version=1")
                .spawn_capturing_stdout()
                .await
                .map_err(TestDriverNewError::CargoMetadata)?;

            #[derive(serde::Deserialize)]
            struct MetadataV1 {
                target_directory: String,
            }

            let crate_metadata: MetadataV1 =
                serde_json::from_str(&String::from_utf8_lossy(&metadata_json))
                    .map_err(TestDriverNewError::CargoMetadataParse)?;

            PathBuf::from(crate_metadata.target_directory)
        };
        log::debug!("target_dir = '{}'", target_dir.display());

        // Executable path
        let exe_path = target_dir
            .join(target_arch_opt.target_triple)
            .join("release")
            .join(crate_name);
        log::debug!("exe_path = '{}'", exe_path.display());

        // Put generated linker scripts in a directory
        let linker_scripts = target.linker_scripts();
        log::debug!("linker_scripts = {linker_scripts:?}");
        let link_dir =
            tempdir::TempDir::new("r3_test_runner").map_err(TestDriverNewError::TempDirError)?;

        for (name, contents) in linker_scripts.generated_files {
            let generated_file_path = link_dir.path().join(name);
            log::debug!("Writing '{}'", generated_file_path.display());
            std::fs::write(&generated_file_path, contents)
                .map_err(|e| TestDriverNewError::WriteError(generated_file_path, e))?;
        }

        // Derive `RUSTFLAGS`.
        let mut rustflags = std::env::var("RUSTFLAGS").unwrap_or_default();
        if !rustflags.is_empty() {
            // An unintentionally set `RUSTFLAGS` may cause a hard-to-understand
            // build/test failure or compromise the validity of the tests, so
            // warn the user about this behavior
            log::info!(
                "RUSTFLAGS appears to be non-empty; it will be combined with \
                the default flags for the current test target"
            );
        }

        write!(rustflags, " {additional_rustflags}").unwrap();

        for name in linker_scripts.inputs.iter() {
            write!(rustflags, " -C link-arg=-T{name}").unwrap();
        }

        let target_features = &target_arch_opt.target_features;
        log::debug!("target_features = {target_features:?}");
        if !target_features.is_empty() {
            write!(rustflags, " -C target-feature={target_features}").unwrap();
        };

        log::debug!("rustflags = {rustflags:?}");

        Ok(Self {
            rustflags,
            exe_path,
            target_arch_opt,
            link_dir,
            target,
            meta,
        })
    }

    /// Get the list of kernel tests defined by the test driver.
    pub(crate) fn driver_kernel_tests(&self) -> Vec<String> {
        self.meta.tests.kernel_tests.clone()
    }

    /// Run the test driver using the specified debug probe interface. The test
    /// driver must be compiled first by [`Self::compile`].
    pub(crate) async fn run(
        &self,
        test_run: &selection::TestRun<'_>,
        debug_probe: &mut (impl targets::DebugProbe + ?Sized),
    ) -> Result<Result<(), TestRunError>, TestDriverRunError> {
        log::debug!("Running the test");
        let acquisition_result = debug_probe_program_and_get_output_until(
            debug_probe,
            &self.exe_path,
            [b"!- TEST WAS SUCCESSFUL -!", &b"panicked at"[..]].iter(),
        )
        .await;

        // Interpret the result
        let test_result = match acquisition_result {
            Ok(output_bytes) => {
                // Check the output
                let output_str = String::from_utf8_lossy(&output_bytes);
                log::debug!("Output (lossy UTF-8) = {output_str:?}");

                if output_str.contains("!- TEST WAS SUCCESSFUL -!") {
                    Ok(())
                } else {
                    Err(TestRunError::General(output_str.into_owned()))
                }
            }
            Err(RunError::Timeout) => Err(TestRunError::Timeout),
            Err(RunError::TooLong) => Err(TestRunError::TooLong),
            Err(RunError::Other(e)) => {
                // Fail-fast if the problem is the debug connection, not the
                // test itself
                return Err(TestDriverRunError::Run(test_run.to_string(), e));
            }
        };

        Ok(test_result)
    }

    /// Get the path of the executable file produced by [`Self::compile`].
    pub(crate) fn exe_path(&self) -> PathBuf {
        self.exe_path.clone()
    }

    /// Compile an executable of the test driver.
    pub(crate) async fn compile(
        &self,
        test_run: &selection::TestRun<'_>,
        BuildOpt {
            verbose,
            log_level,
            small_rt,
        }: BuildOpt,
    ) -> Result<(), TestDriverRunError> {
        let Self {
            exe_path,
            target_arch_opt,
            rustflags,
            link_dir,
            target,
            ..
        } = self;

        let full_test_name = test_run.case.to_string();

        // Delete `exe_path`
        if exe_path.exists() {
            if let Err(e) = std::fs::remove_file(exe_path) {
                // Failure is non-fatal
                log::warn!("Failed to remove '{}': {e}", exe_path.display());
            }
        }

        let build_std = (!target_arch_opt.target_features.is_empty() && {
            log::debug!("Specifying `-Zbuild-std=core` because of a custom target feature set");
            true
        }) | (small_rt && {
            log::debug!("Specifying `-Zbuild-std=core` because `BuildOpt::small_rt` is enabled");
            true
        });

        // Build the test driver
        log::debug!("Building the test");
        let cmd_result = {
            let cmd = subprocess::CmdBuilder::new(CARGO_CMD)
                .arg("build")
                .arg("--release")
                .arg("--target")
                .arg(target_arch_opt.target_triple)
                .arg(match test_run.case {
                    selection::TestCase::KernelTest(_)
                    | selection::TestCase::DriverKernelTest(_) => "--features=kernel_tests",
                    selection::TestCase::KernelBenchmark(_) => "--features=kernel_benchmarks",
                })
                .args(
                    target
                        .cargo_features()
                        .iter()
                        .map(|f| format!("--features={f}")),
                )
                .args(
                    test_run
                        .cpu_lock_by_basepri
                        .then_some("--features=cpu-lock-by-basepri"),
                )
                .arg(match log_level {
                    LogLevel::Off => "--features=log/max_level_off",
                    LogLevel::Error => "--features=log/max_level_error",
                    LogLevel::Warn => "--features=log/max_level_warn",
                    LogLevel::Info => "--features=log/max_level_info",
                    LogLevel::Debug => "--features=log/max_level_debug",
                    LogLevel::Trace => "--features=log/max_level_trace",
                })
                .args((!verbose).then_some("-q"))
                .args(build_std.then_some("-Zbuild-std=core"))
                .args(small_rt.then_some("-Zbuild-std-features=panic_immediate_abort"))
                .env("R3_TEST_DRIVER_LINK_SEARCH", link_dir.path())
                // Tell `r3_test_suite/build.rs` which test to run
                .env(
                    "R3_TEST",
                    match test_run.case {
                        selection::TestCase::KernelTest(_)
                        | selection::TestCase::KernelBenchmark(_) => &full_test_name,
                        selection::TestCase::DriverKernelTest(_) => "",
                    },
                )
                // Tell `r3_*_test_driver/build.rs` which test to run
                .env(
                    "R3_DRIVER_TEST",
                    match test_run.case {
                        selection::TestCase::KernelTest(_)
                        | selection::TestCase::KernelBenchmark(_) => "",
                        selection::TestCase::DriverKernelTest(_) => &full_test_name,
                    },
                )
                .env("RUSTFLAGS", rustflags);
            if verbose {
                cmd.spawn_expecting_success().await
            } else {
                // Hide `stderr` unless the command fails
                cmd.spawn_expecting_success_quiet().await
            }
        };

        cmd_result.map_err(TestDriverRunError::BuildTest)?;

        // Locate the executable
        if !exe_path.is_file() {
            return Err(TestDriverRunError::ExeNotFound(exe_path.clone()));
        }

        Ok(())
    }
}

enum RunError {
    Timeout,
    TooLong,
    Other(anyhow::Error),
}

async fn debug_probe_program_and_get_output_until<P: AsRef<[u8]>>(
    debug_probe: &mut (impl targets::DebugProbe + ?Sized),
    exe: &Path,
    markers: impl IntoIterator<Item = P>,
) -> Result<Vec<u8>, RunError> {
    let mut stream = debug_probe
        .program_and_get_output(exe)
        .await
        .map_err(RunError::Other)?;
    log::trace!("debug_probe_program_and_get_output_until: Got a stream");

    let matcher = aho_corasick::AhoCorasickBuilder::new().build(markers);

    let mut output = Vec::new();
    let mut buffer = vec![0u8; 16384];

    loop {
        log::trace!("... calling `read`");
        let read_fut = stream.read(&mut buffer);
        let timeout_fut = tokio::time::sleep(Duration::from_secs(35));

        let num_bytes = tokio::select! {
            read_result = read_fut => {
                log::trace!("... `read` resolved to {read_result:?}");
                read_result.unwrap_or(0)
            },
            _ = timeout_fut => {
                log::trace!("... `sleep` resolved earlier - timeout");
                log::trace!("... The output so far: {:?}", String::from_utf8_lossy(&output));
                return Err(RunError::Timeout);
            },
        };

        if num_bytes == 0 {
            break;
        }

        output.extend_from_slice(&buffer[0..num_bytes]);

        // Check for markers
        let check_len = (num_bytes + matcher.max_pattern_len() - 1).min(output.len());
        if output.len() >= check_len {
            let i = output.len() - check_len;
            if let Some(m) = matcher.find(&output[i..]) {
                log::trace!(
                    "... Found the marker at position {:?}",
                    i + m.start()..i + m.end()
                );

                // Read the remaining output, which might include error details
                log::trace!("... Reading the remaining output");
                output.extend_from_slice(
                    &read_to_end_timeout(stream.as_mut(), Duration::from_millis(300))
                        .await
                        .map_err(|e| RunError::Other(e.into()))?,
                );
                break;
            }
        }

        if output.len() > 1024 * 1024 {
            return Err(RunError::TooLong);
        }
    }

    Ok(output)
}

async fn read_to_end_timeout(
    mut stream: Pin<&mut (impl tokio::io::AsyncRead + ?Sized)>,
    timeout: Duration,
) -> tokio::io::Result<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = vec![0u8; 16384];
    let mut timeout_fut = pin!(tokio::time::sleep(timeout));

    log::trace!("read_to_end_timeout: Got a stream");

    loop {
        log::trace!("... calling `read`");
        let read_fut = stream.read(&mut buffer);

        let num_bytes = tokio::select! {
            read_result = read_fut => {
                log::trace!("... `read` resolved to {read_result:?}");
                read_result.unwrap_or(0)
            },
            _ = &mut timeout_fut => {
                log::trace!("... `sleep` resolved earlier - timeout");
                break;
            },
        };

        if num_bytes == 0 {
            break;
        }

        output.extend_from_slice(&buffer[0..num_bytes]);
    }

    Ok(output)
}
