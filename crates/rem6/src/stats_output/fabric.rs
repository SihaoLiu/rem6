use rem6_fabric::{FabricLaneActivity, FabricVirtualNetworkActivity};
use rem6_stats::{StatResetPolicy, StatsRegistry};

use crate::{Rem6CliError, Rem6RunFabricSummary};

use super::{increment_stat, stat_path_segment};

pub(super) fn emit_run_fabric_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6RunFabricSummary,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.active_lanes"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.active_lanes(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.active_virtual_networks"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.active_virtual_networks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.transfers"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.transfers(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.bytes"),
        "Byte",
        StatResetPolicy::Monotonic,
        summary.bytes(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.flits"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.flits(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.occupied_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.queue_delay_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.max_queue_delay_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.credit_delay_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.max_credit_delay_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.max_credit_delay_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.contended_lanes"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.contended_lanes(),
    )?;
    emit_fabric_virtual_network_stats(stats, prefix, summary.virtual_network_activities())?;
    emit_fabric_lane_stats(stats, prefix, summary.lane_activities())
}

pub(super) fn emit_fabric_virtual_network_stats<I>(
    stats: &mut StatsRegistry,
    prefix: &str,
    activities: I,
) -> Result<(), Rem6CliError>
where
    I: IntoIterator<Item = FabricVirtualNetworkActivity>,
{
    for activity in activities {
        if activity.is_empty() {
            continue;
        }
        let prefix = format!("{prefix}.vn{}", activity.virtual_network().get());
        increment_stat(
            stats,
            &format!("{prefix}.active_lanes"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.active_lane_count() as u64,
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.transfers"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.transfer_count() as u64,
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.bytes"),
            "Byte",
            StatResetPolicy::Monotonic,
            activity.byte_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.flits"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.flit_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.occupied_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.occupied_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.queue_delay_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.queue_delay_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.max_queue_delay_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.max_queue_delay_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.credit_delay_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.credit_delay_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.max_credit_delay_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.max_credit_delay_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.contended_lanes"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.contended_lane_count() as u64,
        )?;
    }
    Ok(())
}

fn emit_fabric_lane_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    activities: &[FabricLaneActivity],
) -> Result<(), Rem6CliError> {
    for activity in activities {
        let prefix = format!(
            "{prefix}.link.{}.vn{}",
            stat_path_segment(activity.link().as_str()),
            activity.virtual_network().get()
        );
        increment_stat(
            stats,
            &format!("{prefix}.transfers"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.transfer_count() as u64,
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.bytes"),
            "Byte",
            StatResetPolicy::Monotonic,
            activity.byte_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.flits"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.flit_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.occupied_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.occupied_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.queue_delay_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.queue_delay_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.max_queue_delay_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.max_queue_delay_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.credit_delay_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.credit_delay_ticks(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.max_credit_delay_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.max_credit_delay_ticks(),
        )?;
    }
    Ok(())
}
