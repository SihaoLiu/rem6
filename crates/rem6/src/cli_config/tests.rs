use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use super::*;

const WILDCARD_PROFILE: ConfigPrescanProfile = ConfigPrescanProfile::wildcard(&[]);

#[test]
fn wildcard_unknown_value_flag_suppresses_literal_config_and_last_config_wins() {
    let path = config_path_from_args(
        &strings([
            "--unknown-value",
            "--config",
            "suppressed.toml",
            "--config",
            "first-visible.toml",
            "--config",
            "second-visible.toml",
        ]),
        WILDCARD_PROFILE,
    )
    .unwrap();

    assert_eq!(path, Some(PathBuf::from("second-visible.toml")));
}

#[test]
fn explicit_profile_consumes_known_value_but_not_unknown_long_config_selector() {
    let path = config_path_from_args(
        &strings([
            "--value",
            "--config",
            "--unknown-long",
            "--config",
            "visible.toml",
        ]),
        ConfigPrescanProfile::explicit(&["--value"], &[]),
    )
    .unwrap();

    assert_eq!(path, Some(PathBuf::from("visible.toml")));
}

#[test]
fn explicit_profile_suppresses_known_values_but_not_unknown_flags() {
    let known = strings(["--output", "--config", "--config", "gpu.toml"]);
    assert_eq!(
        gpu_run_file_config_from_args(&known).unwrap(),
        Some(PathBuf::from("gpu.toml"))
    );

    let unknown = strings(["--future-flag", "--config", "gpu.toml"]);
    assert_eq!(
        gpu_run_file_config_from_args(&unknown).unwrap(),
        Some(PathBuf::from("gpu.toml"))
    );
}

#[test]
fn auxiliary_profiles_preserve_value_and_boolean_vocabularies() {
    assert_eq!(
        accelerator_run_file_config_from_args(&strings([
            "--gpu-kernel",
            "--config",
            "--config",
            "accelerator.toml",
        ]))
        .unwrap(),
        Some(PathBuf::from("accelerator.toml"))
    );
    assert_eq!(
        multi_run_file_config_from_args(&strings([
            "--continue-on-failure",
            "--config",
            "multi.toml",
        ]))
        .unwrap(),
        Some(PathBuf::from("multi.toml"))
    );
    assert_eq!(
        resource_acquire_file_config_from_args(&strings([
            "--output",
            "--config",
            "--config",
            "resources.toml",
        ]))
        .unwrap(),
        Some(PathBuf::from("resources.toml"))
    );
}

#[test]
fn auxiliary_profiles_report_missing_visible_config_values() {
    let args = strings(["--config"]);
    for file_config_from_args in [
        gpu_run_file_config_from_args as fn(&[String]) -> Result<Option<PathBuf>, Rem6CliError>,
        accelerator_run_file_config_from_args,
        multi_run_file_config_from_args,
        resource_acquire_file_config_from_args,
    ] {
        assert_eq!(
            file_config_from_args(&args).expect_err("missing --config value should fail"),
            Rem6CliError::MissingFlagValue {
                flag: "--config".to_string()
            }
        );
    }
}

#[test]
fn run_bool_flag_does_not_consume_real_config_selector() {
    let path = run_file_config_from_args(&strings(["--execute", "--config", "run.toml"])).unwrap();

    assert_eq!(path, Some(PathBuf::from("run.toml")));
}

#[test]
fn missing_config_reports_missing_flag_value_with_original_flag() {
    let error = config_path_from_args(&strings(["--config"]), WILDCARD_PROFILE)
        .expect_err("missing --config value should fail");

    assert_eq!(
        error,
        Rem6CliError::MissingFlagValue {
            flag: "--config".to_string()
        }
    );
}

#[test]
fn required_value_returns_value_or_missing_flag_value() {
    assert_eq!(
        required_value("--binary", Some("program.elf".to_string())).unwrap(),
        "program.elf"
    );
    assert_eq!(
        required_value("--binary", None).expect_err("missing value should fail"),
        Rem6CliError::MissingFlagValue {
            flag: "--binary".to_string()
        }
    );
}

#[test]
fn resolve_config_path_handles_relative_base_and_absolute_paths() {
    assert_eq!(
        resolve_config_path(Some(Path::new("configs")), Path::new("run.elf")),
        PathBuf::from("configs/run.elf")
    );
    assert_eq!(
        resolve_config_path(None, Path::new("run.elf")),
        PathBuf::from("run.elf")
    );
    assert_eq!(
        resolve_config_path(Some(Path::new("configs")), Path::new("/tmp/run.elf")),
        PathBuf::from("/tmp/run.elf")
    );
}

#[test]
fn read_toml_config_maps_valid_read_and_parse_results() {
    #[derive(Debug, Deserialize, Eq, PartialEq)]
    struct TestConfig {
        name: String,
        count: u64,
    }

    let temp_dir = TempDirGuard::new("read_toml_config_maps_valid_read_and_parse_results");
    let valid_path = temp_dir.path().join("valid.toml");
    let missing_path = temp_dir.path().join("missing.toml");
    let invalid_path = temp_dir.path().join("invalid.toml");
    fs::write(&valid_path, "name = \"sample\"\ncount = 7\n").unwrap();
    fs::write(&invalid_path, "name = [\n").unwrap();

    assert_eq!(
        read_toml_config::<TestConfig>(&valid_path).unwrap(),
        TestConfig {
            name: "sample".to_string(),
            count: 7
        }
    );

    match read_toml_config::<TestConfig>(&missing_path)
        .expect_err("missing file should map to ReadConfig")
    {
        Rem6CliError::ReadConfig { path, error } => {
            assert_eq!(path, missing_path);
            assert!(!error.is_empty());
        }
        error => panic!("expected ReadConfig, got {error:?}"),
    }

    match read_toml_config::<TestConfig>(&invalid_path)
        .expect_err("invalid TOML should map to ParseConfig")
    {
        Rem6CliError::ParseConfig { path, error } => {
            assert_eq!(path, invalid_path);
            assert!(!error.is_empty());
        }
        error => panic!("expected ParseConfig, got {error:?}"),
    }
}

fn strings(values: impl IntoIterator<Item = &'static str>) -> Vec<String> {
    values.into_iter().map(str::to_string).collect()
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(test_name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rem6-cli-config-{test_name}-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
