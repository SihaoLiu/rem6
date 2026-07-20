use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnknownLongFlagMode {
    ConsumeFollowing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConfigPrescanProfile {
    bool_flags: &'static [&'static str],
    known_value_flags: &'static [&'static str],
    unknown_long_flag_mode: UnknownLongFlagMode,
}

const RUN_BOOL_FLAGS: &[&str] = &[
    "--execute",
    "--checker-cpu",
    "--dram-memory",
    "--riscv-se",
    "--riscv-sbi",
];

const RUN_VALUE_FLAGS: &[&str] = &[
    "--isa",
    "--binary",
    "--resource-config",
    "--kernel-resource",
    "--max-tick",
    "--min-remote-delay",
    "--memory-route-delay",
    "--host-event-delay",
    "--host-checkpoint",
    "--host-restore-checkpoint",
    "--host-switch-cpu-mode",
    "--start-address",
    "--riscv-boot-a0",
    "--riscv-boot-a1",
    "--riscv-sbi-console-input",
    "--riscv-se-arg",
    "--riscv-se-env",
    "--riscv-se-stdin",
    "--riscv-se-file",
    "--riscv-pc-count-target",
    "--riscv-branch-lookahead",
    "--riscv-o3-scalar-memory-depth",
    "--riscv-o3-issue-width",
    "--riscv-o3-writeback-width",
    "--riscv-branch-predictor",
    "--riscv-in-order-width",
    "--riscv-execution-mode",
    "--riscv-data-translation",
    "--m5-switch-cpu-mode",
    "--max-instructions",
    "--stats-format",
    "--memory-system",
    "--dram-memory-profile",
    "--data-cache-protocol",
    "--data-cache-l2-protocol",
    "--data-cache-l3-protocol",
    "--data-cache-prefetcher",
    "--instruction-cache-protocol",
    "--instruction-cache-l2-protocol",
    "--instruction-cache-l3-protocol",
    "--instruction-cache-prefetcher",
    "--fabric-link",
    "--fabric-bandwidth-bytes-per-tick",
    "--fabric-request-virtual-network",
    "--fabric-response-virtual-network",
    "--fabric-credit-depth",
    "--fabric-router",
    "--fabric-router-input-port",
    "--fabric-router-output-port",
    "--fabric-router-virtual-channel",
    "--fabric-request-router-virtual-channel",
    "--fabric-response-router-virtual-channel",
    "--fabric-router-latency",
    "--fabric-qos-queue-policy",
    "--gdb-listen",
    "--debug-flags",
    "--cores",
    "--parallel-workers",
    "--dump-memory",
    "--load-blob",
    "--readfile",
    "--output",
    "--stats-output",
    "--power-format",
    "--power-output",
];

const GUPS_VALUE_FLAGS: &[&str] = &[
    "--memory-start",
    "--memory-size",
    "--updates",
    "--max-tick",
    "--min-remote-delay",
    "--memory-route-delay",
    "--stats-format",
    "--rng-state",
    "--dump-memory",
    "--output",
    "--stats-output",
];

const TRACE_REPLAY_VALUE_FLAGS: &[&str] = &[
    "--trace",
    "--resource-config",
    "--trace-resource",
    "--route",
    "--memory-start",
    "--memory-size",
    "--max-tick",
    "--min-remote-delay",
    "--memory-route-delay",
    "--tick-frequency",
    "--line-bytes",
    "--agent",
    "--control-partition",
    "--data-cache-protocol",
    "--data-cache-dram-memory-profile",
    "--data-cache-dram-qos-priority-levels",
    "--data-cache-dram-qos-default-priority",
    "--fabric-link",
    "--fabric-bandwidth-bytes-per-tick",
    "--fabric-request-virtual-network",
    "--fabric-response-virtual-network",
    "--fabric-credit-depth",
    "--fabric-router",
    "--fabric-router-input-port",
    "--fabric-router-output-port",
    "--fabric-router-virtual-channel",
    "--fabric-router-latency",
    "--external-adapter-kind",
    "--external-adapter-endpoint",
    "--external-adapter-checkpoint-after-events",
    "--host-checkpoint",
    "--host-restore-checkpoint",
    "--stats-format",
    "--output",
    "--stats-output",
    "--power-format",
    "--power-output",
];

const RUN_PROFILE: ConfigPrescanProfile = ConfigPrescanProfile {
    bool_flags: RUN_BOOL_FLAGS,
    known_value_flags: RUN_VALUE_FLAGS,
    unknown_long_flag_mode: UnknownLongFlagMode::ConsumeFollowing,
};

const GUPS_PROFILE: ConfigPrescanProfile = ConfigPrescanProfile {
    bool_flags: &[],
    known_value_flags: GUPS_VALUE_FLAGS,
    unknown_long_flag_mode: UnknownLongFlagMode::ConsumeFollowing,
};

const TRACE_REPLAY_PROFILE: ConfigPrescanProfile = ConfigPrescanProfile {
    bool_flags: &[],
    known_value_flags: TRACE_REPLAY_VALUE_FLAGS,
    unknown_long_flag_mode: UnknownLongFlagMode::ConsumeFollowing,
};

pub(crate) fn run_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, RUN_PROFILE)
}

pub(crate) fn gups_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, GUPS_PROFILE)
}

pub(crate) fn trace_replay_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, TRACE_REPLAY_PROFILE)
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
                    UnknownLongFlagMode::ConsumeFollowing
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

    const WILDCARD_PROFILE: ConfigPrescanProfile = ConfigPrescanProfile {
        bool_flags: &[],
        known_value_flags: &[],
        unknown_long_flag_mode: UnknownLongFlagMode::ConsumeFollowing,
    };

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

        let temp_dir = unique_temp_dir("read_toml_config_maps_valid_read_and_parse_results");
        fs::create_dir_all(&temp_dir).unwrap();
        let valid_path = temp_dir.join("valid.toml");
        let missing_path = temp_dir.join("missing.toml");
        let invalid_path = temp_dir.join("invalid.toml");
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

        fs::remove_dir_all(temp_dir).unwrap();
    }

    fn strings(values: impl IntoIterator<Item = &'static str>) -> Vec<String> {
        values.into_iter().map(str::to_string).collect()
    }

    fn unique_temp_dir(test_name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "rem6-cli-config-{test_name}-{}-{nanos}",
            std::process::id()
        ))
    }
}
