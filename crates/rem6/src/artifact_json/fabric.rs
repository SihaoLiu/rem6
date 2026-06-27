use super::json_escape;

use rem6_kernel::WaitForEdgeKind;
use rem6_workload::WorkloadParallelExecutionSummary;

pub(super) fn fabric_link_activities_json(summary: &WorkloadParallelExecutionSummary) -> String {
    summary
        .fabric_link_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"active_virtual_networks\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"contended_virtual_networks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.active_virtual_network_count(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.flit_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.max_credit_delay_ticks(),
                activity.contended_virtual_network_count(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn fabric_lane_activities_json(summary: &WorkloadParallelExecutionSummary) -> String {
    summary
        .fabric_lane_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"link\":\"{}\",\"virtual_network\":{},\"transfer_count\":{},\"byte_count\":{},\"flit_count\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"max_queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"max_credit_delay_ticks\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                activity.transfer_count(),
                activity.byte_count(),
                activity.flit_count(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.max_queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.max_credit_delay_ticks(),
                activity.first_tick(),
                activity.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn fabric_hop_activities_json(summary: &WorkloadParallelExecutionSummary) -> String {
    summary
        .fabric_hop_activities()
        .iter()
        .map(|activity| {
            format!(
                "{{\"packet\":{},\"hop_index\":{},\"link\":\"{}\",\"virtual_network\":{},\"bytes\":{},\"flits\":{},\"ready_tick\":{},\"start_tick\":{},\"occupied_ticks\":{},\"queue_delay_ticks\":{},\"credit_delay_ticks\":{},\"depart_tick\":{},\"arrival_tick\":{}}}",
                activity.packet().get(),
                activity.hop_index(),
                json_escape(activity.link().as_str()),
                activity.virtual_network().get(),
                activity.bytes(),
                activity.flits(),
                activity.ready_tick(),
                activity.start_tick(),
                activity.occupied_ticks(),
                activity.queue_delay_ticks(),
                activity.credit_delay_ticks(),
                activity.depart_tick(),
                activity.arrival_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn fabric_wait_for_json_fields(
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<String> {
    vec![
        format!(
            "\"fabric_wait_for_edge_count\":{}",
            summary.fabric_wait_for_edge_count()
        ),
        format!(
            "\"fabric_wait_for_edge_kind_windows\":[{}]",
            fabric_wait_for_edge_kind_windows_json(summary)
        ),
        format!(
            "\"fabric_wait_for_blocked_node_windows\":[{}]",
            fabric_wait_for_blocked_node_windows_json(summary)
        ),
        format!(
            "\"fabric_wait_for_target_node_windows\":[{}]",
            fabric_wait_for_target_node_windows_json(summary)
        ),
    ]
}

fn fabric_wait_for_edge_kind_windows_json(summary: &WorkloadParallelExecutionSummary) -> String {
    summary
        .fabric_wait_for_edge_kind_windows()
        .iter()
        .map(|window| {
            format!(
                "{{\"kind\":\"{}\",\"edge_count\":{},\"first_tick\":{},\"last_tick\":{}}}",
                wait_for_edge_kind_json(window.kind()),
                window.edge_count(),
                window.first_tick(),
                window.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn fabric_wait_for_blocked_node_windows_json(summary: &WorkloadParallelExecutionSummary) -> String {
    summary
        .fabric_wait_for_blocked_node_windows()
        .iter()
        .map(|window| {
            format!(
                "{{\"node\":\"{}\",\"edge_count\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(&window.node().to_string()),
                window.edge_count(),
                window.first_tick(),
                window.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn fabric_wait_for_target_node_windows_json(summary: &WorkloadParallelExecutionSummary) -> String {
    summary
        .fabric_wait_for_target_node_windows()
        .iter()
        .map(|window| {
            format!(
                "{{\"node\":\"{}\",\"edge_count\":{},\"first_tick\":{},\"last_tick\":{}}}",
                json_escape(&window.node().to_string()),
                window.edge_count(),
                window.first_tick(),
                window.last_tick(),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn wait_for_edge_kind_json(kind: WaitForEdgeKind) -> &'static str {
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
