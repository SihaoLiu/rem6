use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::{Rem6CliError, Rem6ExecutionSummary, Rem6RunConfig};

pub(super) fn emit_riscv_run_stats(
    stats: &mut StatsRegistry,
    config: &Rem6RunConfig,
    execution: Option<&Rem6ExecutionSummary>,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        "sim.riscv.boot.a0",
        "Value",
        StatResetPolicy::Constant,
        config.riscv_boot_a0(),
    )?;
    increment_stat(
        stats,
        "sim.riscv.boot.a1",
        "Value",
        StatResetPolicy::Constant,
        config.riscv_boot_a1(),
    )?;
    increment_stat(
        stats,
        "sim.riscv.sbi",
        "Count",
        StatResetPolicy::Constant,
        u64::from(config.riscv_sbi()),
    )?;
    if let Some(execution) = execution {
        increment_stat(
            stats,
            "sim.riscv.sbi.dbcn.console_bytes",
            "Byte",
            StatResetPolicy::Constant,
            execution.riscv_sbi_console.byte_count(),
        )?;
        if config.riscv_sbi() {
            increment_stat(
                stats,
                "sim.riscv.sbi.timer.deadlines",
                "Count",
                StatResetPolicy::Constant,
                execution.riscv_sbi_timers.len() as u64,
            )?;
            if let Some(deadline) = execution
                .riscv_sbi_timers
                .iter()
                .map(|timer| timer.deadline())
                .min()
            {
                increment_stat(
                    stats,
                    "sim.riscv.sbi.timer.next_deadline",
                    "Tick",
                    StatResetPolicy::Constant,
                    deadline,
                )?;
            }
            increment_stat(
                stats,
                "sim.riscv.sbi.ipi.requests",
                "Count",
                StatResetPolicy::Constant,
                execution.riscv_sbi_ipis.len() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.ipi.targets",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_ipis
                    .iter()
                    .map(|ipi| ipi.target_count())
                    .sum(),
            )?;
        }
    }
    increment_stat(
        stats,
        "sim.riscv.se",
        "Count",
        StatResetPolicy::Constant,
        u64::from(config.riscv_se()),
    )
}
