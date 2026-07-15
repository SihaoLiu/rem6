use std::path::PathBuf;

use crate::Rem6CliError;

pub(super) fn run_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        &[
            "--execute",
            "--checker-cpu",
            "--dram-memory",
            "--riscv-se",
            "--riscv-sbi",
        ],
    )
}

pub(super) fn gups_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, &[])
}

pub(super) fn trace_replay_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, &[])
}

fn config_path_from_args(
    args: &[String],
    bool_flags: &[&str],
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
            flag if bool_flags.contains(&flag) => {
                index += 1;
            }
            flag if flag.starts_with("--") => {
                index += if args.get(index + 1).is_some() { 2 } else { 1 };
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(path)
}
