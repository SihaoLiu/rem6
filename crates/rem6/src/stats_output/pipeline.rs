use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::{Rem6CliError, Rem6CoreSummary};

const IN_ORDER_STAGES: [&str; 5] = ["fetch1", "fetch2", "decode", "execute", "commit"];

pub(super) fn emit_in_order_stage_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    name: &str,
    unit: &'static str,
    reset_policy: StatResetPolicy,
    values: [u64; 5],
) -> Result<(), Rem6CliError> {
    for (stage, value) in IN_ORDER_STAGES.into_iter().zip(values) {
        increment_stat(
            stats,
            &format!("sim.cpu{}.pipeline.in_order.stage.{stage}.{name}", core.cpu),
            unit,
            reset_policy,
            value,
        )?;
    }
    Ok(())
}

pub(super) fn emit_in_order_stall_cause_stage_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    cause: &str,
    name: &str,
    unit: &'static str,
    reset_policy: StatResetPolicy,
    values: [u64; 5],
) -> Result<(), Rem6CliError> {
    for (stage, value) in IN_ORDER_STAGES.into_iter().zip(values) {
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.stall_cause.{cause}.stage.{stage}.{name}",
                core.cpu
            ),
            unit,
            reset_policy,
            value,
        )?;
    }
    Ok(())
}

pub(super) fn emit_in_order_stall_cause_stat(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    cause: &str,
    name: &str,
    unit: &'static str,
    reset_policy: StatResetPolicy,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!(
            "sim.cpu{}.pipeline.in_order.stall_cause.{cause}.{name}",
            core.cpu
        ),
        unit,
        reset_policy,
        value,
    )?;
    Ok(())
}

pub(super) fn emit_in_order_cause_stage_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    family: &str,
    cause: &str,
    name: &str,
    unit: &'static str,
    reset_policy: StatResetPolicy,
    values: [u64; 5],
) -> Result<(), Rem6CliError> {
    for (stage, value) in IN_ORDER_STAGES.into_iter().zip(values) {
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.pipeline.in_order.{family}.{cause}.stage.{stage}.{name}",
                core.cpu,
            ),
            unit,
            reset_policy,
            value,
        )?;
    }
    Ok(())
}

pub(super) fn emit_in_order_cause_stat(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    family: &str,
    cause: &str,
    name: &str,
    unit: &'static str,
    reset_policy: StatResetPolicy,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!(
            "sim.cpu{}.pipeline.in_order.{family}.{cause}.{name}",
            core.cpu
        ),
        unit,
        reset_policy,
        value,
    )?;
    Ok(())
}
