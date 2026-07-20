use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnknownLongFlagMode {
    ConsumeFollowingValue,
    #[allow(dead_code)]
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConfigPrescanProfile {
    bool_flags: &'static [&'static str],
    known_value_flags: &'static [&'static str],
    unknown_long_flag_mode: UnknownLongFlagMode,
}

impl ConfigPrescanProfile {
    const fn wildcard(bool_flags: &'static [&'static str]) -> Self {
        Self {
            bool_flags,
            known_value_flags: &[],
            unknown_long_flag_mode: UnknownLongFlagMode::ConsumeFollowingValue,
        }
    }

    #[allow(dead_code)]
    const fn explicit(
        known_value_flags: &'static [&'static str],
        bool_flags: &'static [&'static str],
    ) -> Self {
        Self {
            bool_flags,
            known_value_flags,
            unknown_long_flag_mode: UnknownLongFlagMode::Ignore,
        }
    }
}

const RUN_BOOL_FLAGS: &[&str] = &[
    "--execute",
    "--checker-cpu",
    "--dram-memory",
    "--riscv-se",
    "--riscv-sbi",
];

pub(crate) fn run_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, ConfigPrescanProfile::wildcard(RUN_BOOL_FLAGS))
}

pub(crate) fn gups_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, ConfigPrescanProfile::wildcard(&[]))
}

pub(crate) fn trace_replay_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, ConfigPrescanProfile::wildcard(&[]))
}

fn config_path_from_args(
    args: &[String],
    profile: ConfigPrescanProfile,
) -> Result<Option<PathBuf>, Rem6CliError> {
    let mut path = None;
    let mut index = 0;
    while let Some(token) = args.get(index) {
        match token.as_str() {
            "--config" => {
                let value =
                    args.get(index + 1)
                        .cloned()
                        .ok_or_else(|| Rem6CliError::MissingFlagValue {
                            flag: token.clone(),
                        })?;
                path = Some(PathBuf::from(value));
                index += 2;
            }
            flag if profile.bool_flags.contains(&flag) => {
                index += 1;
            }
            flag if profile.known_value_flags.contains(&flag) => {
                index += flag_with_optional_value_width(args, index);
            }
            flag if flag.starts_with("--")
                && matches!(
                    profile.unknown_long_flag_mode,
                    UnknownLongFlagMode::ConsumeFollowingValue
                ) =>
            {
                index += flag_with_optional_value_width(args, index);
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(path)
}

fn flag_with_optional_value_width(args: &[String], index: usize) -> usize {
    if args.get(index + 1).is_some() {
        2
    } else {
        1
    }
}

pub(crate) fn read_toml_config<T: DeserializeOwned>(path: &Path) -> Result<T, Rem6CliError> {
    let text = std::fs::read_to_string(path).map_err(|error| Rem6CliError::ReadConfig {
        path: path.to_path_buf(),
        error: error.to_string(),
    })?;
    toml::from_str::<T>(&text).map_err(|error| Rem6CliError::ParseConfig {
        path: path.to_path_buf(),
        error: error.to_string(),
    })
}

pub(crate) fn required_value(flag: &str, value: Option<String>) -> Result<String, Rem6CliError> {
    value.ok_or_else(|| Rem6CliError::MissingFlagValue {
        flag: flag.to_string(),
    })
}

pub(crate) fn resolve_config_path(config_dir: Option<&Path>, path: &Path) -> PathBuf {
    if path.is_relative() {
        config_dir
            .map(|dir| dir.join(path))
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
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
    fn run_bool_flag_does_not_consume_real_config_selector() {
        let path =
            run_file_config_from_args(&strings(["--execute", "--config", "run.toml"])).unwrap();

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
}
