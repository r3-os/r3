#![feature(exhaustive_patterns)]
#![feature(generic_arg_infer)]
#![feature(must_not_suspend)] // `must_not_suspend` lint
#![feature(lint_reasons)]
#![feature(decl_macro)] // `macro`
#![feature(pin_macro)] // `core::pin::pin!`
#![warn(must_not_suspend)]
use anyhow::Context;
use clap::Parser;
use std::{env, path::Path};
use thiserror::Error;

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
        log::error!("Command failed.\n{:?}", e);
        std::process::exit(1);
    }
}

// TODO: Top-level error enum is useless; replace it with `anyhow`
#[derive(Error, Debug)]
enum MainError {
    #[error("Could not initialize the test driver interface.")]
    TestDriver(#[source] driverinterface::TestDriverNewError),
    #[error("Could not connect to the target.")]
    ConnectTarget(#[source] anyhow::Error),
    #[error("Could not build the test '{0}'.")]
    BuildTest(String, #[source] driverinterface::TestDriverRunError),
    #[error("Could not run the test '{0}'.")]
    RunTest(String, #[source] driverinterface::TestDriverRunError),
    #[error("Test failed.")]
    TestFail,
    #[error("The target architecture '{0}' is invalid or unsupported.")]
    BadTarget(targets::Arch),
}

/// Test runner for the Arm-M port of R3
#[derive(Parser)]
struct Opt {
    /// Target chip/board
    #[clap(short = 't', long = "target", parse(try_from_str = try_parse_target),
        possible_values(&*TARGET_POSSIBLE_VALUES))]
    target: &'static dyn targets::Target,
    /// Override target architecture
    ///
    /// See the documentation of `Arch::from_str` for full syntax.
    #[clap(short = 'a', long = "arch", parse(try_from_str = std::str::FromStr::from_str))]
    target_arch: Option<targets::Arch>,
    /// Print the list of supported targets and their architecture strings
    #[clap(long = "help-targets")]
    help_targets: bool,
    /// Use a stripped-down build of the standard library
    ///
    /// This option lowers the output binary size by building the `core`
    /// library with `panic_immediate_abort` feature at cost of disabling panic
    /// reporting.
    #[clap(long = "small-rt")]
    small_rt: bool,
    /// Extra command-line flags to pass to `rustc`
    #[clap(long = "rustflags")]
    additional_rustflags: Option<String>,
    /// If specified, only run tests containing this string in their names
    ///
    /// See the documentation of `TestFilter::from_str` for full syntax.
    #[clap(parse(try_from_str = std::str::FromStr::from_str))]
    tests: Vec<selection::TestFilter>,
    /// Select benchmark tests
    #[clap(short = 'b', long = "bench")]
    bench: bool,
    /// Log level of the test program
    #[clap(
        short = 'l',
        long = "log-level",
        possible_values(driverinterface::LogLevel::variants()),
        ignore_case = true,
        default_value = "info"
    )]
    log_level: driverinterface::LogLevel,
    /// Display build progress and warnings
    #[clap(short = 'v')]
    verbose: bool,
    /// Keep going until N tests fail (0 means infinity)
    #[clap(short = 'k', long = "keep-going", default_value = "5")]
    keep_going: usize,
    /// Don't execute the test driver nor attempt to connect to a target
    #[clap(long = "norun")]
    norun: bool,
    /// Execute the specified command with `{}` replaced with the current
    /// test executable path and terminated by `;`
    #[clap(
        long = "exec",
        multiple_values = true,
        value_terminator = ";",
        allow_hyphen_values = true
    )]
    exec: Vec<String>,
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

async fn main_inner() -> anyhow::Result<()> {
    // Parse arguments
    let opt = Opt::parse();

    // If `--help-targets` is specified, print all targets and exit,
    if opt.help_targets {
        println!("Supported targets:");
        for (name, target) in targets::TARGETS {
            println!("  {name:30}{}", target.target_arch());
        }
        return Ok(());
    }

    // Find where the test drivers are located in this workspace, assuming
    // `r3_test_runner` is running on the same environment as where it
    // was built.
    let driver_base_path = {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        log::debug!("CARGO_MANIFEST_DIR = {}", manifest_dir);
        Path::new(manifest_dir)
            .parent()
            .expect("Couldn't get the parent of `CARGO_MANIFEST_DIR`")
    };

    let target_arch = opt.target_arch.unwrap_or_else(|| opt.target.target_arch());
    log::debug!("target_arch = {}", target_arch);

    let target_arch_opt = target_arch
        .build_opt()
        .ok_or(MainError::BadTarget(target_arch))?;
    log::debug!("target_arch_opt = {:?}", target_arch_opt);

    // Initialize the test driver interface
    let test_driver = driverinterface::TestDriver::new(
        driver_base_path,
        opt.target,
        &target_arch,
        target_arch_opt,
        opt.additional_rustflags.unwrap_or_default(),
    )
    .await
    .map_err(MainError::TestDriver)?;

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
                .map_err(MainError::ConnectTarget)?,
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
        log::info!(" - {}", test_run);

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
            .map_err(|e| MainError::BuildTest(full_test_name.clone(), e))?;

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
                    .map_err(|e| MainError::RunTest(full_test_name, e))?;
            }
        }

        match test_result {
            Ok(()) => {
                log::info!("Test run '{}' was successful", test_run);
            }
            Err(msg) => {
                // Test did run, but the result was failure.
                log::error!("Test run '{}' failed: {}", test_run, msg);
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
            log::error!(" - {}", test_run_name);
        }

        if !tests_skipped_to_fail_fast.is_empty() {
            log::warn!("Skipped tests:");
            for test_run_name in tests_skipped_to_fail_fast {
                log::warn!(" - {}", test_run_name);
            }
        }

        return Err(MainError::TestFail.into());
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
