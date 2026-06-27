use rem6_kernel::WaitForEdgeKind;
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
    increment_stat(
        stats,
        &format!("{prefix}.edges"),
        "Count",
        StatResetPolicy::Monotonic,
        edge_count,
    )?;
    for window in windows {
        if window.is_empty() {
            continue;
        }
        let prefix = format!(
            "{prefix}.kind.{}",
            wait_for_edge_kind_stat_segment(window.kind())
        );
        increment_stat(
            stats,
            &format!("{prefix}.edges"),
            "Count",
            StatResetPolicy::Monotonic,
            window.edge_count() as u64,
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.first_tick"),
            "Tick",
            StatResetPolicy::Monotonic,
            window.first_tick(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.last_tick"),
            "Tick",
            StatResetPolicy::Monotonic,
            window.last_tick(),
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
