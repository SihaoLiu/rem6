use std::path::PathBuf;

use crate::Rem6CliError;

pub(super) fn run_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        &[
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
            "--riscv-branch-predictor",
            "--riscv-in-order-width",
            "--riscv-execution-mode",
            "--m5-switch-cpu-mode",
            "--max-instructions",
            "--stats-format",
            "--memory-system",
            "--dram-memory-profile",
            "--dram-activate-latency",
            "--dram-read-latency",
            "--dram-write-latency",
            "--dram-precharge-latency",
            "--dram-bus-turnaround",
            "--dram-burst-spacing",
            "--dram-same-bank-group-burst-spacing",
            "--dram-command-window-cycles",
            "--dram-command-window-max-commands",
            "--dram-refresh-policy",
            "--dram-refresh-granularity",
            "--dram-low-power-precharge-powerdown-entry-delay",
            "--dram-low-power-self-refresh-entry-delay",
            "--dram-low-power-exit-latency",
            "--dram-low-power-self-refresh-exit-latency",
            "--dram-refresh-interval",
            "--dram-refresh-recovery",
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
            "--gdb-listen",
        ],
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
    config_path_from_args(
        args,
        &[
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
        ],
        &[],
    )
}

pub(super) fn trace_replay_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        &[
            "--trace",
            "--resource-config",
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
        ],
        &[],
    )
}

fn config_path_from_args(
    args: &[String],
    value_flags: &[&str],
    bool_flags: &[&str],
) -> Result<Option<PathBuf>, Rem6CliError> {
    let mut path = None;
    let mut index = 0;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--config" => {
                let value = args
                    .get(index + 1)
                    .cloned()
                    .ok_or_else(|| Rem6CliError::MissingFlagValue { flag: flag.clone() })?;
                path = Some(PathBuf::from(value));
                index += 2;
            }
            flag if bool_flags.contains(&flag) => {
                index += 1;
            }
            flag if value_flags.contains(&flag) => {
                index += 2;
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(path)
}
