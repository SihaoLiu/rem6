use std::collections::BTreeMap;

use rem6_fabric::{
    FabricHopActivity, FabricLaneActivity, FabricLinkActivity, FabricVirtualNetworkActivity,
};
use rem6_stats::{StatResetPolicy, StatsRegistry};

use crate::{Rem6CliError, Rem6RunFabricRouterActivity, Rem6RunFabricSummary};

use super::wait_for::emit_kernel_wait_for_edge_kind_window_stats;
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
    emit_fabric_qos_stats(stats, prefix, summary)?;
    emit_fabric_virtual_network_stats(stats, prefix, summary.virtual_network_activities())?;
    emit_fabric_link_stats(stats, prefix, summary.link_activities())?;
    emit_fabric_lane_stats(stats, prefix, summary.lane_activities())?;
    emit_fabric_hop_stats(stats, prefix, summary.hop_activities())?;
    emit_fabric_router_stats(stats, prefix, summary.router_activities())?;
    emit_kernel_wait_for_edge_kind_window_stats(
        stats,
        &format!("{prefix}.wait_for"),
        summary.wait_for_edge_count(),
        summary.wait_for_edge_kind_windows(),
    )
}

fn emit_fabric_qos_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6RunFabricSummary,
) -> Result<(), Rem6CliError> {
    let prefix = format!("{prefix}.qos");
    for (suffix, value) in [
        ("grants", summary.qos_grant_activities().len() as u64),
        ("candidates", summary.qos_candidate_count()),
        ("suppressed", summary.qos_suppressed_count()),
        ("batches", summary.qos_batch_count()),
        ("max_candidates", summary.qos_max_candidate_count()),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            "Count",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }

    let mut requestor_grants = BTreeMap::<u32, u64>::new();
    let mut priority_grants = BTreeMap::<u8, u64>::new();
    for activity in summary.qos_grant_activities() {
        requestor_grants
            .entry(activity.grant().requestor().get())
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
        priority_grants
            .entry(activity.grant().priority().get())
            .and_modify(|count| *count = count.saturating_add(1))
            .or_insert(1);
    }
    for (requestor, grants) in requestor_grants {
        increment_stat(
            stats,
            &format!("{prefix}.requestor{requestor}.grants"),
            "Count",
            StatResetPolicy::Monotonic,
            grants,
        )?;
    }
    for (priority, grants) in priority_grants {
        increment_stat(
            stats,
            &format!("{prefix}.priority{priority}.grants"),
            "Count",
            StatResetPolicy::Monotonic,
            grants,
        )?;
    }
    Ok(())
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
    router_latency_ticks: u64,
    router_queue_delay_ticks: u64,
    max_router_queue_delay_ticks: u64,
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
        if let Some(router) = activity.router() {
            summary.router_latency_ticks = summary
                .router_latency_ticks
                .saturating_add(router.latency_ticks());
            summary.router_queue_delay_ticks = summary
                .router_queue_delay_ticks
                .saturating_add(router.queue_delay_ticks());
            summary.max_router_queue_delay_ticks = summary
                .max_router_queue_delay_ticks
                .max(router.queue_delay_ticks());
        }
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
            ("router_latency_ticks", "Tick", summary.router_latency_ticks),
            (
                "router_queue_delay_ticks",
                "Tick",
                summary.router_queue_delay_ticks,
            ),
            (
                "max_router_queue_delay_ticks",
                "Tick",
                summary.max_router_queue_delay_ticks,
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

pub(super) fn emit_fabric_router_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    activities: &[Rem6RunFabricRouterActivity],
) -> Result<(), Rem6CliError> {
    for activity in activities {
        if activity.transfer_count() == 0 {
            continue;
        }
        let prefix = format!(
            "{prefix}.router.{}.in{}.out{}.vc{}",
            stat_path_segment(activity.router()),
            activity.input_port(),
            activity.output_port(),
            activity.virtual_channel(),
        );
        for (suffix, unit, value) in [
            ("transfers", "Count", activity.transfer_count()),
            ("bytes", "Byte", activity.byte_count()),
            ("flits", "Count", activity.flit_count()),
            ("latency_ticks", "Tick", activity.latency_ticks()),
            ("queue_delay_ticks", "Tick", activity.queue_delay_ticks()),
            (
                "max_queue_delay_ticks",
                "Tick",
                activity.max_queue_delay_ticks(),
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
