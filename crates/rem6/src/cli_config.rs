use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnknownLongFlagMode {
    ConsumeFollowingValue,
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

const GPU_RUN_VALUE_FLAGS: &[&str] = &[
    "--workgroups",
    "--compute-units",
    "--wave-slots-per-compute-unit",
    "--workgroup-cycles",
    "--memory-start",
    "--memory-size",
    "--max-tick",
    "--min-remote-delay",
    "--memory-route-delay",
    "--stats-format",
    "--power-format",
    "--power-output",
    "--nomali-output",
    "--dram-memory-profile",
    "--data-cache-protocol",
    "--data-cache-prefetcher",
    "--fabric-link",
    "--fabric-bandwidth-bytes-per-tick",
    "--fabric-request-virtual-network",
    "--fabric-response-virtual-network",
    "--fabric-credit-depth",
    "--global-load",
    "--global-store",
    "--output",
    "--stats-output",
    "--dump-memory",
];
const GPU_RUN_BOOL_FLAGS: &[&str] = &["--dram-memory"];

const ACCELERATOR_RUN_VALUE_FLAGS: &[&str] = &[
    "--engine",
    "--lanes",
    "--command-delay",
    "--npu-inference",
    "--gpu-kernel",
    "--stats-format",
    "--output",
    "--stats-output",
];

const MULTI_RUN_VALUE_FLAGS: &[&str] = &[
    "--suite-id",
    "--run",
    "--stats-format",
    "--output",
    "--stats-output",
];
const MULTI_RUN_BOOL_FLAGS: &[&str] = &["--continue-on-failure"];

const RESOURCE_ACQUIRE_VALUE_FLAGS: &[&str] = &[
    "--workload-id",
    "--boot-entry",
    "--stats-format",
    "--output",
    "--stats-output",
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

pub(crate) fn gpu_run_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        ConfigPrescanProfile::explicit(GPU_RUN_VALUE_FLAGS, GPU_RUN_BOOL_FLAGS),
    )
}

pub(crate) fn accelerator_run_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        ConfigPrescanProfile::explicit(ACCELERATOR_RUN_VALUE_FLAGS, &[]),
    )
}

pub(crate) fn multi_run_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        ConfigPrescanProfile::explicit(MULTI_RUN_VALUE_FLAGS, MULTI_RUN_BOOL_FLAGS),
    )
}

pub(crate) fn resource_acquire_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        ConfigPrescanProfile::explicit(RESOURCE_ACQUIRE_VALUE_FLAGS, &[]),
    )
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
mod tests;
