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
            "sim.riscv.sbi.console.bytes",
            "Byte",
            StatResetPolicy::Constant,
            execution.riscv_sbi_console.byte_count(),
        )?;
        increment_stat(
            stats,
            "sim.riscv.sbi.dbcn.console_bytes",
            "Byte",
            StatResetPolicy::Constant,
            execution.riscv_sbi_console.dbcn_byte_count(),
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
                "sim.riscv.sbi.hsm.starts",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_hsm_events
                    .iter()
                    .filter(|event| event.is_hart_start())
                    .count() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.hsm.stops",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_hsm_events
                    .iter()
                    .filter(|event| event.is_hart_stop())
                    .count() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.hsm.suspends",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_hsm_events
                    .iter()
                    .filter(|event| event.is_hart_suspend())
                    .count() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.hsm.wakes",
                "Count",
                StatResetPolicy::Constant,
                execution.riscv_sbi_hsm_wakes.len() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.hsm.status_queries",
                "Count",
                StatResetPolicy::Constant,
                execution.riscv_sbi_hsm_statuses.len() as u64,
            )?;
            for status_name in [
                "started",
                "stopped",
                "start_pending",
                "stop_pending",
                "suspended",
                "suspend_pending",
                "resume_pending",
                "unknown",
            ] {
                increment_stat(
                    stats,
                    &format!("sim.riscv.sbi.hsm.status.{status_name}"),
                    "Count",
                    StatResetPolicy::Constant,
                    execution
                        .riscv_sbi_hsm_statuses
                        .iter()
                        .filter(|status| status.status_name() == status_name)
                        .count() as u64,
                )?;
            }
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
            increment_stat(
                stats,
                "sim.riscv.sbi.rfence.requests",
                "Count",
                StatResetPolicy::Constant,
                execution.riscv_sbi_rfences.len() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.rfence.targets",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_rfences
                    .iter()
                    .map(|rfence| rfence.target_count())
                    .sum(),
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.rfence.completions",
                "Count",
                StatResetPolicy::Constant,
                execution.riscv_sbi_rfence_completions.len() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.reset.requests",
                "Count",
                StatResetPolicy::Constant,
                execution.riscv_sbi_resets.len() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.reset.shutdowns",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_resets
                    .iter()
                    .filter(|reset| reset.is_shutdown())
                    .count() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.reset.cold_reboots",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_resets
                    .iter()
                    .filter(|reset| reset.is_cold_reboot())
                    .count() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.reset.warm_reboots",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_resets
                    .iter()
                    .filter(|reset| reset.is_warm_reboot())
                    .count() as u64,
            )?;
            increment_stat(
                stats,
                "sim.riscv.sbi.reset.system_failures",
                "Count",
                StatResetPolicy::Constant,
                execution
                    .riscv_sbi_resets
                    .iter()
                    .filter(|reset| reset.is_system_failure())
                    .count() as u64,
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
