#![feature(future_readiness_fns)] // `std::future::ready`
use std::{
    env,
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};
use structopt::StructOpt;
use thiserror::Error;
use tokio::prelude::*;

mod selection;
mod subprocess;
mod targets;
mod utils;

#[tokio::main]
async fn main() {
    env_logger::from_env(
        env_logger::Env::default().default_filter_or("constance_port_arm_m_test_runner=info"),
    )
    .init();

    if let Err(e) = main_inner().await {
        log::error!("Command failed.\n{}", e);
        std::process::exit(1);
    }
}

#[derive(Error, Debug)]
enum MainError {
    #[error("Error while creating a temporary directory: {0}")]
    TempDirError(#[source] std::io::Error),
    #[error("Error while writing {0:?}: {1}")]
    WriteError(PathBuf, #[source] std::io::Error),
    #[error("Error while changing the current directory to {0:?}: {1}")]
    CdError(PathBuf, #[source] std::io::Error),
    #[error("Could not gather the Cargo metadata using `cargo metadata`.\n\n{0}")]
    CargoMetadata(#[source] subprocess::SubprocessError),
    #[error("Could not parse the Cargo metadata.")]
    CargoMetadataParse,
    #[error("{0:?} is not a valid driver path.")]
    BadDriverPath(PathBuf),
    #[error("Could not locate the compiled executable at {0:?}.")]
    ExeNotFound(PathBuf),
    #[error("Could not connect to the target.\n\n{0}")]
    ConnectTarget(#[source] Box<dyn std::error::Error + Send>),
    #[error("Could not build the test '{0}'.\n\n{1}")]
    BuildTest(String, #[source] subprocess::SubprocessError),
    #[error("{0}")]
    Run(
        #[from]
        #[source]
        RunError,
    ),
    #[error("Test failed.")]
    TestFail,
}

/// Test runner for the Arm-M port of Constance
#[derive(StructOpt)]
struct Opt {
    /// Target type
    #[structopt(short = "t", long = "target", parse(try_from_str = try_parse_target),
        possible_values(&TARGET_POSSIBLE_VALUES))]
    target: &'static dyn targets::Target,
    /// If specified, only run tests containing this string in their names
    ///
    /// See the documentation of `TestFilter::from_str` for full syntax.
    #[structopt(parse(try_from_str = std::str::FromStr::from_str))]
    tests: Vec<selection::TestFilter>,
    /// Log level of the test program
    #[structopt(short = "l", long = "log-level",
        possible_values(&LogLevel::variants()), case_insensitive = true,
        default_value = "warn")]
    log_level: LogLevel,
    /// Display build progress and warnings
    #[structopt(short = "v")]
    verbose: bool,
}

lazy_static::lazy_static! {
    static ref TARGET_POSSIBLE_VALUES: Vec<&'static str> =
        targets::TARGETS.iter().map(|x|x.0).collect();
}

fn try_parse_target(arg_target: &str) -> Result<&'static dyn targets::Target, &'static str> {
    targets::TARGETS
        .iter()
        .find(|x| x.0 == arg_target)
        .ok_or("no such target")
        .map(|x| x.1)
}

#[derive(Clone, Copy, arg_enum_proc_macro::ArgEnum)]
enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

async fn main_inner() -> Result<(), Box<dyn std::error::Error>> {
    // Parse arguments
    let opt = Opt::from_args();

    // Hard-coded paths and commands
    let cargo_cmd = "cargo";

    let driver_path = {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        log::debug!("CARGO_MANIFEST_DIR = {}", manifest_dir);
        Path::new(manifest_dir)
            .parent()
            .expect("Couldn't get the parent of `CARGO_MANIFEST_DIR`")
            .join("constance_port_arm_m_test_driver")
    };

    if !driver_path.is_dir() {
        return Err(MainError::BadDriverPath(driver_path).into());
    }

    // Select tests
    let test_filter = if opt.tests.is_empty() {
        selection::TestFilter::Pass
    } else {
        selection::TestFilter::Disjuction(opt.tests.clone())
    };
    let test_runs: Vec<_> = test_filter.all_matching_test_runs().collect();

    log::info!("Performing {} test run(s)", test_runs.len());

    // Connect to the target
    log::debug!("Connecting to the target");
    let mut debug_probe = opt
        .target
        .connect()
        .await
        .map_err(MainError::ConnectTarget)?;

    // Put the linker script in a directory
    let link_dir = tempdir::TempDir::new("constance_port_arm_m_test_runner")
        .map_err(MainError::TempDirError)?;
    {
        let memory_x_path = link_dir.path().join("memory.x");
        log::debug!("Writing '{}'", memory_x_path.display());
        std::fs::write(&memory_x_path, opt.target.memory_layout_script())
            .map_err(|e| MainError::WriteError(memory_x_path, e))?;
    }

    // Move to the driver directory
    log::debug!("cd-ing to '{}'", driver_path.display());
    std::env::set_current_dir(&driver_path)
        .map_err(|e| MainError::CdError(driver_path.clone(), e))?;

    // Find the target directory
    let target_dir = {
        let metadata_json = subprocess::CmdBuilder::new(cargo_cmd)
            .arg("metadata")
            .arg("--format-version=1")
            .spawn_capturing_stdout()
            .await
            .map_err(MainError::CargoMetadata)?;

        #[derive(miniserde::Deserialize)]
        struct MetadataV1 {
            target_directory: String,
        }

        let metadata: MetadataV1 =
            miniserde::json::from_str(&String::from_utf8_lossy(&metadata_json))
                .map_err(|_| MainError::CargoMetadataParse)?;

        PathBuf::from(metadata.target_directory)
    };
    log::debug!("target_dir = '{}'", target_dir.display());

    // Executable path
    let exe_path = target_dir
        .join(opt.target.target_triple())
        .join("release/constance_port_arm_m_test_driver");
    log::debug!("exe_path = '{}'", exe_path.display());

    let mut failed_tests = Vec::new();

    for test_run in test_runs.iter() {
        let full_test_name = test_run.case.to_string();
        log::info!(" - {}", test_run);

        // Delete `exe_path`
        if exe_path.exists() {
            if let Err(e) = std::fs::remove_file(&exe_path) {
                // Failure is non-fatal
                log::warn!("Failed to remove '{}': {}", exe_path.display(), e);
            }
        }

        // Build the test driver
        log::debug!("Building the test");
        let cmd_result = {
            let cmd = subprocess::CmdBuilder::new(cargo_cmd)
                .arg("build")
                .arg("--release")
                .arg("--target")
                .arg(opt.target.target_triple())
                .arg("--features=test")
                .args(
                    opt.target
                        .cargo_features()
                        .iter()
                        .map(|f| format!("--features={}", f)),
                )
                .args(if test_run.cpu_lock_by_basepri {
                    Some("--features=cpu-lock-by-basepri")
                } else {
                    None
                })
                .arg(match opt.log_level {
                    LogLevel::Off => "--features=log/max_level_off",
                    LogLevel::Error => "--features=log/max_level_error",
                    LogLevel::Warn => "--features=log/max_level_warn",
                    LogLevel::Info => "--features=log/max_level_info",
                    LogLevel::Debug => "--features=log/max_level_debug",
                    LogLevel::Trace => "--features=log/max_level_trace",
                })
                .args(if opt.verbose { None } else { Some("-q") })
                .env(
                    "CONSTANCE_PORT_ARM_M_TEST_DRIVER_LINK_SEARCH",
                    link_dir.path(),
                )
                .env("CONSTANCE_TEST", &full_test_name);
            if opt.verbose {
                cmd.spawn_expecting_success().await
            } else {
                // Hide `stderr` unless the command fails
                cmd.spawn_expecting_success_quiet().await
            }
        };

        cmd_result.map_err(|e| MainError::BuildTest(full_test_name.clone(), e))?;

        // Locate the executable
        if !exe_path.is_file() {
            return Err(MainError::ExeNotFound(exe_path).into());
        }

        // Run the executable
        #[derive(Error, Debug)]
        enum TestRunError {
            #[error("Timed out")]
            Timeout,
            #[error("The output is too long")]
            TooLong,
            #[error("'{0}'")]
            General(String),
        }

        log::debug!("Running the test");
        let acquisition_result = debug_probe_program_and_get_output_until(
            &mut *debug_probe,
            &exe_path,
            [b"!- TEST WAS SUCCESSFUL -!", &b"panicked at"[..]].iter(),
        )
        .await;

        // Interpret the result
        let test_result = match acquisition_result {
            Ok(output_bytes) => {
                // Check the output
                let output_str = String::from_utf8_lossy(&output_bytes);
                log::debug!("Output (lossy UTF-8) = {:?}", output_str);

                if output_str.contains("!- TEST WAS SUCCESSFUL -!") {
                    Ok(())
                } else {
                    Err(TestRunError::General(output_str.into_owned()))
                }
            }
            Err(RunError::Timeout) => Err(TestRunError::Timeout),
            Err(RunError::TooLong) => Err(TestRunError::TooLong),
            Err(e @ RunError::Other(_)) => {
                // Fail-fast if the problem is the debug connection, not the
                // test itself
                return Err(e.into());
            }
        };

        match test_result {
            Ok(()) => {
                log::info!("'{}' was successful", full_test_name);
            }
            Err(msg) => {
                log::error!("Test '{}' failed: {}", full_test_name, msg);
                failed_tests.push(full_test_name.clone());
                continue;
            }
        }
    }

    if !failed_tests.is_empty() {
        log::error!("Failed tests:");

        for full_test_name in failed_tests {
            log::error!(" - {}", full_test_name);
        }

        return Err(MainError::TestFail.into());
    }

    Ok(())
}

#[derive(Error, Debug)]
enum RunError {
    #[error("Timeout while reading the output")]
    Timeout,
    #[error("Length limit exceeded while reading the output")]
    TooLong,
    #[error("{0}")]
    Other(
        #[from]
        #[source]
        Box<dyn std::error::Error>,
    ),
}

async fn debug_probe_program_and_get_output_until<P: AsRef<[u8]>>(
    debug_probe: &mut (impl targets::DebugProbe + ?Sized),
    exe: &Path,
    markers: impl IntoIterator<Item = P>,
) -> Result<Vec<u8>, RunError> {
    let mut stream = debug_probe.program_and_get_output(exe).await?;
    log::trace!("debug_probe_program_and_get_output_until: Got a stream");

    let matcher = aho_corasick::AhoCorasickBuilder::new().build(markers);

    let mut output = Vec::new();
    let mut buffer = vec![0u8; 16384];

    loop {
        log::trace!("... calling `read`");
        let read_fut = stream.read(&mut buffer);
        let timeout_fut = tokio::time::delay_for(Duration::from_secs(35));

        let num_bytes = tokio::select! {
            read_result = read_fut => {
                log::trace!("... `read` resolved to {:?}", read_result);
                read_result.unwrap_or(0)
            },
            _ = timeout_fut => {
                log::trace!("... `delay_for` resolved earlier - timeout");
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
    let mut timeout_fut = tokio::time::delay_for(timeout);

    log::trace!("read_to_end_timeout: Got a stream");

    loop {
        log::trace!("... calling `read`");
        let read_fut = stream.read(&mut buffer);

        let num_bytes = tokio::select! {
            read_result = read_fut => {
                log::trace!("... `read` resolved to {:?}", read_result);
                read_result.unwrap_or(0)
            },
            _ = &mut timeout_fut => {
                log::trace!("... `delay_for` resolved earlier - timeout");
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
