use std::collections::BTreeMap;

use rem6_fabric::{
    FabricHopActivity, FabricLaneActivity, FabricLinkActivity, FabricVirtualNetworkActivity,
};
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
    emit_fabric_link_stats(stats, prefix, summary.link_activities())?;
    emit_fabric_lane_stats(stats, prefix, summary.lane_activities())?;
    emit_fabric_hop_stats(stats, prefix, summary.hop_activities())
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

pub(super) fn emit_fabric_link_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    activities: &[FabricLinkActivity],
) -> Result<(), Rem6CliError> {
    for activity in activities {
        if activity.is_empty() {
            continue;
        }
        let prefix = format!(
            "{prefix}.link.{}",
            stat_path_segment(activity.link().as_str())
        );
        for (suffix, unit, value) in [
            (
                "active_virtual_networks",
                "Count",
                activity.active_virtual_network_count() as u64,
            ),
            ("transfers", "Count", activity.transfer_count() as u64),
            ("bytes", "Byte", activity.byte_count()),
            ("flits", "Count", activity.flit_count()),
            ("occupied_ticks", "Tick", activity.occupied_ticks()),
            ("queue_delay_ticks", "Tick", activity.queue_delay_ticks()),
            (
                "max_queue_delay_ticks",
                "Tick",
                activity.max_queue_delay_ticks(),
            ),
            ("credit_delay_ticks", "Tick", activity.credit_delay_ticks()),
            (
                "max_credit_delay_ticks",
                "Tick",
                activity.max_credit_delay_ticks(),
            ),
            (
                "contended_virtual_networks",
                "Count",
                activity.contended_virtual_network_count() as u64,
            ),
        ] {
            increment_stat(
                stats,
                &format!("{prefix}.{suffix}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    Ok(())
}

pub(super) fn emit_fabric_lane_stats(
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

#[derive(Clone, Copy, Debug, Default)]
struct FabricHopStatSummary {
    transfers: u64,
    bytes: u64,
    flits: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
}

pub(super) fn emit_fabric_hop_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    activities: &[FabricHopActivity],
) -> Result<(), Rem6CliError> {
    let mut summaries = BTreeMap::<(String, u64, usize), FabricHopStatSummary>::new();
    for activity in activities {
        let summary = summaries
            .entry((
                activity.link().as_str().to_owned(),
                activity.virtual_network().get().into(),
                activity.hop_index(),
            ))
            .or_default();
        summary.transfers = summary.transfers.saturating_add(1);
        summary.bytes = summary.bytes.saturating_add(activity.bytes());
        summary.flits = summary.flits.saturating_add(activity.flits());
        summary.occupied_ticks = summary
            .occupied_ticks
            .saturating_add(activity.occupied_ticks());
        summary.queue_delay_ticks = summary
            .queue_delay_ticks
            .saturating_add(activity.queue_delay_ticks());
        summary.max_queue_delay_ticks = summary
            .max_queue_delay_ticks
            .max(activity.queue_delay_ticks());
        summary.credit_delay_ticks = summary
            .credit_delay_ticks
            .saturating_add(activity.credit_delay_ticks());
    }

    for ((link, virtual_network, hop_index), summary) in summaries {
        let prefix = format!(
            "{prefix}.link.{}.vn{virtual_network}.hop{hop_index}",
            stat_path_segment(&link)
        );
        for (suffix, unit, value) in [
            ("transfers", "Count", summary.transfers),
            ("bytes", "Byte", summary.bytes),
            ("flits", "Count", summary.flits),
            ("occupied_ticks", "Tick", summary.occupied_ticks),
            ("queue_delay_ticks", "Tick", summary.queue_delay_ticks),
            (
                "max_queue_delay_ticks",
                "Tick",
                summary.max_queue_delay_ticks,
            ),
            ("credit_delay_ticks", "Tick", summary.credit_delay_ticks),
        ] {
            increment_stat(
                stats,
                &format!("{prefix}.{suffix}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }

    Ok(())
}
