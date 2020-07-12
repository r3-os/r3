use std::{
    env,
    path::{Path, PathBuf},
};
use structopt::StructOpt;
use thiserror::Error;

mod subprocess;
mod targets;
mod utils;

#[tokio::main]
async fn main() {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    if let Err(e) = main_inner().await {
        log::error!("Command failed.\n{}", e);
    }
}

#[derive(Error, Debug)]
pub enum MainError {
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
    #[error("Could not build the test '{0}'.\n\n{1}")]
    BuildTest(String, #[source] subprocess::SubprocessError),
}

/// Test runner for the Arm-M port of Constance
#[derive(StructOpt)]
struct Opt {
    /// Target type
    #[structopt(short = "t", long = "target", parse(try_from_str = try_parse_target),
        possible_values(&TARGET_POSSIBLE_VALUES))]
    target: &'static dyn targets::Target,
    /// If specified, only run tests containing this string in their names
    tests: Vec<String>,
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
    let tests: Vec<_> = constance_test_suite::kernel_tests::TEST_NAMES
        .iter()
        .cloned()
        .filter(|test_name| {
            if opt.tests.is_empty() {
                true
            } else {
                opt.tests
                    .iter()
                    .any(|arg_test_name| test_name.contains(arg_test_name))
            }
        })
        .collect();

    log::info!("Running {} test(s)", tests.len());

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

    for test_name in tests.iter() {
        let full_test_name = format!("kernel_tests::{}", test_name);
        log::info!(" - {}", full_test_name);

        // Build the test driver
        log::debug!("Building the test");
        subprocess::CmdBuilder::new(cargo_cmd)
            .arg("build")
            .arg("--release")
            .arg("--features=test")
            .env(
                "CONSTANCE_PORT_ARM_M_TEST_DRIVER_LINK_SEARCH",
                link_dir.path(),
            )
            .env("CONSTANCE_TEST", &full_test_name)
            .spawn_expecting_success()
            .await
            .map_err(|e| MainError::BuildTest(full_test_name.clone(), e))?;

        // TODO
        log::warn!("TODO: Run the test");
    }

    println!("Hello, world!");

    Ok(())
}
