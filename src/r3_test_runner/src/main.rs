#![feature(exhaustive_patterns)]
#![feature(generic_arg_infer)]
#![feature(must_not_suspend)] // `must_not_suspend` lint
#![feature(lint_reasons)]
#![feature(decl_macro)] // `macro`
#![feature(once_cell)]
#![warn(must_not_suspend)]
use anyhow::{bail, Context};
use clap::Parser;
use std::{env, path::Path};

mod driverinterface;
mod selection;
mod subprocess;
mod targets;
mod utils;

// This program isn't particularly heavy-duty, so use the single-threaded
// runtime to keep the compile time and runtime footprint low
#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("r3_test_runner=info"),
    )
    .init();

    if let Err(e) = main_inner().await {
        log::error!("Command failed.\n{e:?}");
        std::process::exit(1);
    }
}

/// R3 test runner
#[derive(Parser)]
struct Opt {
    /// Target chip/board
    #[arg(short = 't', long = "target", value_enum)]
    target: OptTarget,
    /// Override target architecture
    ///
    /// See the documentation of `Arch::from_str` for full syntax.
    #[arg(short = 'a', long = "arch")]
    target_arch: Option<targets::Arch>,
    /// Print the list of supported targets and their architecture strings
    #[arg(long = "help-targets")]
    help_targets: bool,
    /// Use a stripped-down build of the standard library
    ///
    /// This option lowers the output binary size by building the `core`
    /// library with `panic_immediate_abort` feature at cost of disabling panic
    /// reporting.
    #[arg(long = "small-rt")]
    small_rt: bool,
    /// Extra command-line flags to pass to `rustc`
    #[arg(long = "rustflags")]
    additional_rustflags: Option<String>,
    /// If specified, only run tests containing this string in their names
    ///
    /// See the documentation of `TestFilter::from_str` for full syntax.
    tests: Vec<selection::TestFilter>,
    /// Select benchmark tests
    #[arg(short = 'b', long = "bench")]
    bench: bool,
    /// Log level of the test program
    #[arg(
        short = 'l',
        long = "log-level",
        ignore_case = true,
        default_value = "info",
        value_enum
    )]
    log_level: driverinterface::LogLevel,
    /// Display build progress and warnings
    #[arg(short = 'v')]
    verbose: bool,
    /// Keep going until N tests fail (0 means infinity)
    #[arg(short = 'k', long = "keep-going", default_value = "5")]
    keep_going: usize,
    /// Don't execute the test driver nor attempt to connect to a target
    #[arg(long = "norun")]
    norun: bool,
    /// Execute the specified command with `{}` replaced with the current
    /// test executable path and terminated by `;`
    #[arg(
        long = "exec",
        num_args = 1..,
        value_terminator = ";",
        allow_hyphen_values = true
    )]
    exec: Vec<String>,
}

#[derive(Clone)]
struct OptTarget {
    name: &'static str,
    target: &'static dyn targets::Target,
}

impl clap::ValueEnum for OptTarget {
    fn value_variants<'a>() -> &'a [Self] {
        use std::sync::LazyLock;
        static VARIANTS: LazyLock<Vec<OptTarget>> = LazyLock::new(|| {
            targets::TARGETS
                .iter()
                .map(|&(name, target)| OptTarget { name, target })
                .collect()
        });

        &VARIANTS
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(
            clap::builder::PossibleValue::new(self.name)
                .help(self.target.target_arch().to_string()),
        )
    }
}

impl std::ops::Deref for OptTarget {
    type Target = dyn targets::Target;

    fn deref(&self) -> &Self::Target {
        self.target
    }
}

async fn main_inner() -> anyhow::Result<()> {
    // Parse arguments
    let opt = Opt::parse();

    // Find where the test drivers are located in this workspace, assuming
    // `r3_test_runner` is running on the same environment as where it
    // was built.
    let driver_base_path = {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        log::debug!("CARGO_MANIFEST_DIR = {manifest_dir}");
        Path::new(manifest_dir)
            .parent()
            .expect("Couldn't get the parent of `CARGO_MANIFEST_DIR`")
    };

    let target_arch = opt.target_arch.unwrap_or_else(|| opt.target.target_arch());
    log::debug!("target_arch = {target_arch}");

    let target_arch_opt = target_arch.build_opt().with_context(|| {
        format!("The target architecture '{target_arch}' is invalid or unsupported.")
    })?;
    log::debug!("target_arch_opt = {target_arch_opt:?}");

    // Initialize the test driver interface
    let test_driver = driverinterface::TestDriver::new(
        driver_base_path,
        opt.target.target,
        &target_arch,
        target_arch_opt,
        opt.additional_rustflags.unwrap_or_default(),
    )
    .await
    .context("Could not initialize the test driver interface.")?;

    // Select tests
    let test_source = selection::TestSource {
        driver_kernel_tests: test_driver.driver_kernel_tests(),
    };
    let test_filter = if opt.tests.is_empty() {
        selection::TestFilter::Pass
    } else {
        selection::TestFilter::Disjuction(opt.tests.clone())
    };
    let test_filter = selection::TestFilter::Conjunction(vec![
        test_filter,
        selection::TestFilter::IsBenchmark(opt.bench),
    ]);
    let supports_basepri = matches!(
        target_arch,
        // v6-M, v8-M Baseline, and non-M architectures don't support BASEPRI
        targets::Arch::ArmM {
            version: targets::ArmMVersion::Armv7M | targets::ArmMVersion::Armv8MMainline,
            ..
        },
    );
    let test_runs: Vec<_> = test_filter
        .all_matching_test_runs(&test_source)
        .filter(|r| supports_basepri || !r.cpu_lock_by_basepri)
        .collect();

    log::info!("Performing {} test run(s)", test_runs.len());

    // Connect to the target
    let mut debug_probe = if opt.norun {
        log::debug!("Not nonnecting to the target because `--norun` is specified");
        None
    } else {
        log::debug!("Connecting to the target");
        Some(
            opt.target
                .connect()
                .await
                .context("Could not connect to the target.")?,
        )
    };

    let mut failed_tests = Vec::new();
    let mut tests_skipped_to_fail_fast = Vec::new();

    for test_run in test_runs.iter() {
        if opt.keep_going != 0 && failed_tests.len() >= opt.keep_going {
            // Skip all remaining tests if a certain number of tests have failed
            tests_skipped_to_fail_fast.push(test_run.to_string());
            continue;
        }

        let full_test_name = test_run.case.to_string();
        log::info!(" - {test_run}");

        // Build the test driver
        test_driver
            .compile(
                test_run,
                driverinterface::BuildOpt {
                    verbose: opt.verbose,
                    log_level: opt.log_level,
                    small_rt: opt.small_rt,
                },
            )
            .await
            .with_context(|| format!("Could not build the test '{full_test_name}'."))?;

        // Run the specified program
        let mut test_result = if opt.exec.is_empty() {
            Ok(())
        } else {
            let exe_path = test_driver.exe_path();
            let exe_path = exe_path
                .to_str()
                .context("Non-UTF-8 path doesn't work with `--exec`, sorry...")?;

            let exec_result = exec_substituting(&opt.exec, exe_path, opt.verbose, &mut |cmd| {
                cmd.env("R3_TEST", &full_test_name)
            })
            .await;

            match exec_result {
                Ok(()) => Ok(()),
                e @ Err(subprocess::SubprocessError::Spawn { .. }) => {
                    e.context("Failed to spawn the program specified by `--exec`.")?;
                    unreachable!();
                }
                Err(e @ subprocess::SubprocessError::FailStatus { .. }) => {
                    Err(driverinterface::TestRunError::General(e.to_string()))
                }
                Err(subprocess::SubprocessError::WriteInput { .. }) => unreachable!(),
            }
        };

        // Build and run the test driver
        if test_result.is_ok() {
            if let Some(debug_probe) = &mut debug_probe {
                test_result = test_driver
                    .run(test_run, &mut **debug_probe)
                    .await
                    .with_context(|| format!("Could not run the test '{full_test_name}'."))?;
            }
        }

        match test_result {
            Ok(()) => {
                log::info!("Test run '{test_run}' was successful");
            }
            Err(msg) => {
                // Test did run, but the result was failure.
                log::error!("Test run '{test_run}' failed: {msg}");
                failed_tests.push(test_run.to_string());
                continue;
            }
        }
    }

    log::info!(
        "Summary: {} success, {} fail, {} skipped",
        test_runs.len() - failed_tests.len() - tests_skipped_to_fail_fast.len(),
        failed_tests.len(),
        tests_skipped_to_fail_fast.len(),
    );

    if !failed_tests.is_empty() {
        log::error!("Failed tests:");

        for test_run_name in failed_tests {
            log::error!(" - {test_run_name}");
        }

        if !tests_skipped_to_fail_fast.is_empty() {
            log::warn!("Skipped tests:");
            for test_run_name in tests_skipped_to_fail_fast {
                log::warn!(" - {test_run_name}");
            }
        }

        bail!("Test failed.");
    }

    assert!(tests_skipped_to_fail_fast.is_empty());

    Ok(())
}

async fn exec_substituting(
    cmd: &[String],
    param: &str,
    verbose: bool,
    modifier: &mut dyn FnMut(subprocess::CmdBuilder) -> subprocess::CmdBuilder,
) -> Result<(), subprocess::SubprocessError> {
    let mut cmd = cmd.iter();
    let mut cmd_builder = subprocess::CmdBuilder::new(cmd.next().unwrap().as_str());
    for arg in cmd {
        cmd_builder = if arg.contains("{}") {
            cmd_builder.arg(arg.replace("{}", param))
        } else {
            cmd_builder.arg(arg)
        };
    }
    cmd_builder = modifier(cmd_builder);
    if verbose {
        cmd_builder.spawn_expecting_success().await
    } else {
        // Hide `stderr` unless the command fails
        cmd_builder.spawn_expecting_success_quiet().await
    }
}
