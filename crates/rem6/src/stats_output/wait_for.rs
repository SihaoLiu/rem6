use rem6_kernel::{WaitForEdgeKind, WaitForEdgeKindWindow};
use rem6_stats::{StatResetPolicy, StatsRegistry};
use rem6_workload::WorkloadWaitForEdgeKindWindow;

use crate::Rem6CliError;

use super::increment_stat;

pub(super) fn emit_wait_for_edge_kind_window_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    edge_count: u64,
    windows: &[WorkloadWaitForEdgeKindWindow],
) -> Result<(), Rem6CliError> {
    emit_wait_for_edge_kind_window_stats_from(
        stats,
        prefix,
        edge_count,
        windows.iter().map(|window| {
            (
                window.kind(),
                window.edge_count() as u64,
                window.first_tick(),
                window.last_tick(),
            )
        }),
    )
}

pub(super) fn emit_kernel_wait_for_edge_kind_window_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    edge_count: u64,
    windows: &[WaitForEdgeKindWindow],
) -> Result<(), Rem6CliError> {
    emit_wait_for_edge_kind_window_stats_from(
        stats,
        prefix,
        edge_count,
        windows.iter().map(|window| {
            (
                window.kind(),
                window.edge_count() as u64,
                window.first_tick(),
                window.last_tick(),
            )
        }),
    )
}

fn emit_wait_for_edge_kind_window_stats_from<I>(
    stats: &mut StatsRegistry,
    prefix: &str,
    edge_count: u64,
    windows: I,
) -> Result<(), Rem6CliError>
where
    I: IntoIterator<Item = (WaitForEdgeKind, u64, u64, u64)>,
{
    increment_stat(
        stats,
        &format!("{prefix}.edges"),
        "Count",
        StatResetPolicy::Monotonic,
        edge_count,
    )?;
    for (kind, edge_count, first_tick, last_tick) in windows {
        if edge_count == 0 {
            continue;
        }
        let prefix = format!("{prefix}.kind.{}", wait_for_edge_kind_stat_segment(kind));
        increment_stat(
            stats,
            &format!("{prefix}.edges"),
            "Count",
            StatResetPolicy::Monotonic,
            edge_count,
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.first_tick"),
            "Tick",
            StatResetPolicy::Monotonic,
            first_tick,
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.last_tick"),
            "Tick",
            StatResetPolicy::Monotonic,
            last_tick,
        )?;
    }
    Ok(())
}

fn wait_for_edge_kind_stat_segment(kind: WaitForEdgeKind) -> &'static str {
    match kind {
        WaitForEdgeKind::Resource => "resource",
        WaitForEdgeKind::Message => "message",
        WaitForEdgeKind::Protocol => "protocol",
        WaitForEdgeKind::Queue => "queue",
        WaitForEdgeKind::Credit => "credit",
        WaitForEdgeKind::HostAction => "host_action",
        WaitForEdgeKind::Barrier => "barrier",
    }
}
