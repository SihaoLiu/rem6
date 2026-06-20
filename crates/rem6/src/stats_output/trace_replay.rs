use rem6_stats::{StatResetPolicy, StatsRegistry};
use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadParallelExecutionSummary, WorkloadTrafficTraceReplaySummary,
};

use super::{increment_stat, Rem6CliError};

pub(super) fn emit_trace_replay_summary_stats(
    stats: &mut StatsRegistry,
    summary: &WorkloadTrafficTraceReplaySummary,
) -> Result<(), Rem6CliError> {
    emit_trace_count(
        stats,
        "sim.trace_replay.scheduled",
        summary.scheduled_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.delivered",
        summary.response_delivery_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.completed",
        summary.trace_completed_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.retry",
        summary.trace_retry_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.store_conditional_failed",
        summary.trace_store_conditional_failed_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.reads",
        summary.trace_read_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.writes",
        summary.trace_write_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.response_data_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.trace_response_data_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.response_fill_data_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.trace_response_fill_data_byte_count(),
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures",
        summary.memory_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.read",
        summary.memory_failure_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.write",
        summary.memory_failure_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.functional_read",
        summary.memory_failure_functional_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.functional_write",
        summary.memory_failure_functional_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_acks",
        summary.control_ack_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_acks.sync",
        summary.sync_control_ack_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures",
        summary.control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.sync",
        summary.sync_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband_events",
        summary.sideband_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.prefetch",
        summary.trace_prefetch_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.invalidate",
        summary.trace_invalidate_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.clean",
        summary.trace_clean_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.upgrade",
        summary.trace_upgrade_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.llsc",
        summary.trace_llsc_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.locked_rmw",
        summary.trace_locked_rmw_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.writable_intent",
        summary.trace_writable_intent_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory.events",
        summary.memory_trace_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory.write_completions",
        summary.memory_write_completion_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.cache",
        summary.trace_data_cache_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.cache.maintenance",
        summary.trace_data_cache_maintenance_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.cache.clean_maintenance",
        summary.trace_data_cache_clean_maintenance_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.cache.invalidate_maintenance",
        summary.trace_data_cache_invalidate_maintenance_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache",
        summary.trace_data_cache_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.invalid_destination",
        summary.trace_data_cache_invalid_destination_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.bad_address",
        summary.trace_data_cache_bad_address_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.read",
        summary.trace_data_cache_read_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.write",
        summary.trace_data_cache_write_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.functional_read",
        summary.trace_data_cache_functional_read_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.functional_write",
        summary.trace_data_cache_functional_write_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.invalid_destination",
        summary.memory_failure_invalid_destination_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.bad_address",
        summary.memory_failure_bad_address_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors",
        summary.trace_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.invalid_destination",
        summary.trace_error_invalid_destination_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.bad_address",
        summary.trace_error_bad_address_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.read",
        summary.trace_error_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.write",
        summary.trace_error_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.functional_read",
        summary.trace_error_functional_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.functional_write",
        summary.trace_error_functional_write_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.memory.write_completion_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.memory_write_completion_byte_count(),
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.htm.access",
        summary.trace_htm_access_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.htm.begin",
        summary.trace_htm_begin_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_acks.htm",
        summary.htm_control_ack_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.invalid_destination",
        summary.control_failure_invalid_destination_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.bad_address",
        summary.control_failure_bad_address_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.read",
        summary.control_failure_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.write",
        summary.control_failure_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.functional_read",
        summary.control_failure_functional_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.functional_write",
        summary.control_failure_functional_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.tlb",
        summary.tlb_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.cache",
        summary.cache_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.htm",
        summary.htm_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.diagnostic",
        summary.diagnostic_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.failures",
        summary.trace_sideband_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.tlb_sync_events",
        summary.tlb_sync_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.tlb_sync",
        summary.trace_tlb_sync_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.tlb_sync_flushed_entries",
        summary.trace_tlb_sync_flushed_entry_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.cache_flush_events",
        summary.cache_flush_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.cache_flush",
        summary.trace_cache_flush_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.sideband.cache_flush_data_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.trace_cache_flush_data_byte_count(),
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.l1_invalidation",
        summary.trace_l1_invalidation_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.diagnostic_print_events",
        summary.diagnostic_print_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.diagnostic",
        summary.trace_diagnostic_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.htm_abort_events",
        summary.htm_abort_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.htm_abort",
        summary.trace_htm_abort_count() as u64,
    )
}

pub(super) fn emit_trace_replay_data_cache_stats(
    stats: &mut StatsRegistry,
    summary: &WorkloadParallelExecutionSummary,
    data_cache_dram_accesses: usize,
) -> Result<(), Rem6CliError> {
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.runs",
        summary.data_cache_parallel_run_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.attributed_runs",
        summary.attributed_data_cache_parallel_run_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.unattributed_runs",
        summary.unattributed_data_cache_parallel_run_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.scheduler.epochs",
        summary.data_cache_parallel_scheduler_epoch_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.scheduler.empty_epochs",
        summary.data_cache_parallel_scheduler_empty_epoch_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.scheduler.dispatches",
        summary.data_cache_parallel_scheduler_dispatch_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.scheduler.batches",
        summary.data_cache_parallel_scheduler_batch_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.scheduler.active_partitions",
        summary.active_data_cache_parallel_scheduler_partition_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.scheduler.max_workers",
        summary.data_cache_parallel_scheduler_max_workers() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.scheduler.total_workers",
        summary.data_cache_parallel_scheduler_total_workers() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.data_cache.dram_accesses",
        data_cache_dram_accesses as u64,
    )?;
    for protocol in [
        WorkloadDataCacheProtocol::Msi,
        WorkloadDataCacheProtocol::Mesi,
        WorkloadDataCacheProtocol::Moesi,
        WorkloadDataCacheProtocol::Chi,
    ] {
        emit_trace_count(
            stats,
            &format!("sim.trace_replay.data_cache.{}.runs", protocol.as_str()),
            summary.data_cache_parallel_run_count_for_protocol(protocol) as u64,
        )?;
    }
    Ok(())
}

pub(super) fn emit_trace_replay_fabric_stats(
    stats: &mut StatsRegistry,
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), Rem6CliError> {
    emit_trace_count(
        stats,
        "sim.trace_replay.fabric.active_lanes",
        summary.active_fabric_lane_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.fabric.active_virtual_networks",
        summary.active_fabric_virtual_network_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.fabric.transfers",
        summary.fabric_transfer_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.fabric.bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.fabric_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.fabric.occupied_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        summary.fabric_occupied_ticks(),
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.fabric.queue_delay_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        summary.fabric_queue_delay_ticks(),
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.fabric.max_queue_delay_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        summary.fabric_max_queue_delay_ticks(),
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.fabric.contended_lanes",
        summary.contended_fabric_lane_count() as u64,
    )
}

pub(super) fn emit_trace_replay_resource_stats(
    stats: &mut StatsRegistry,
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), Rem6CliError> {
    emit_trace_count(
        stats,
        "sim.trace_replay.resources.activity",
        summary.resource_activity_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.resources.active",
        summary.active_resource_count() as u64,
    )
}

fn emit_trace_count(stats: &mut StatsRegistry, path: &str, value: u64) -> Result<(), Rem6CliError> {
    increment_stat(stats, path, "Count", StatResetPolicy::Monotonic, value)
}

#[cfg(test)]
mod tests {
    use rem6_kernel::WaitForEdgeKind;
    use rem6_stats::StatsRegistry;
    use rem6_workload::{
        WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount,
        WorkloadParallelExecutionSummary, WorkloadRouteId, WorkloadTrafficTraceReplaySummary,
    };

    use super::{
        emit_trace_replay_data_cache_stats, emit_trace_replay_resource_stats,
        emit_trace_replay_summary_stats,
    };
    use crate::stats_output::stats_snapshot_json;

    #[test]
    fn trace_replay_stats_emit_nonzero_cache_and_sideband_counters() {
        let summary = WorkloadTrafficTraceReplaySummary::new(route_id("cpu0.data"), 3)
            .with_trace_invalidate_response_count(1)
            .with_trace_clean_response_count(1)
            .with_trace_data_cache_response_count(3)
            .with_trace_data_cache_maintenance_response_count(2)
            .with_trace_data_cache_clean_maintenance_response_count(1)
            .with_trace_data_cache_invalidate_maintenance_response_count(1)
            .with_trace_error_count(2)
            .with_trace_error_write_count(1)
            .with_trace_error_functional_write_count(1)
            .with_trace_cache_flush_count(1)
            .with_trace_cache_flush_data_byte_count(64)
            .with_trace_l1_invalidation_count(1)
            .with_trace_diagnostic_count(1);
        let mut stats = StatsRegistry::new();

        emit_trace_replay_summary_stats(&mut stats, &summary).unwrap();
        let json = stats_snapshot_json(&stats.snapshot(0));

        assert_stat_value(&json, "sim.trace_replay.responses.cache", "Count", 3);
        assert_stat_value(&json, "sim.trace_replay.responses.invalidate", "Count", 1);
        assert_stat_value(&json, "sim.trace_replay.responses.clean", "Count", 1);
        assert_stat_value(
            &json,
            "sim.trace_replay.responses.cache.maintenance",
            "Count",
            2,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.responses.cache.clean_maintenance",
            "Count",
            1,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.responses.cache.invalidate_maintenance",
            "Count",
            1,
        );
        assert_stat_value(&json, "sim.trace_replay.trace_errors", "Count", 2);
        assert_stat_value(&json, "sim.trace_replay.trace_errors.write", "Count", 1);
        assert_stat_value(
            &json,
            "sim.trace_replay.trace_errors.functional_write",
            "Count",
            1,
        );
        assert_stat_value(&json, "sim.trace_replay.sideband.cache_flush", "Count", 1);
        assert_stat_value(
            &json,
            "sim.trace_replay.sideband.cache_flush_data_bytes",
            "Byte",
            64,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.sideband.l1_invalidation",
            "Count",
            1,
        );
        assert_stat_value(&json, "sim.trace_replay.sideband.diagnostic", "Count", 1);
    }

    #[test]
    fn trace_replay_stats_emit_data_cache_protocol_accounting() {
        let summary = WorkloadParallelExecutionSummary::default()
            .with_data_cache_parallel_counts(5, 7, 6, 4, 3)
            .with_data_cache_parallel_empty_epoch_count(1)
            .with_data_cache_parallel_partitions(2)
            .with_data_cache_parallel_worker_count(8)
            .with_data_cache_run_attribution(4, 1)
            .with_data_cache_protocol_counts([
                WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Msi, 3),
                WorkloadDataCacheProtocolCount::new(WorkloadDataCacheProtocol::Mesi, 1),
            ]);
        let mut stats = StatsRegistry::new();

        emit_trace_replay_data_cache_stats(&mut stats, &summary, 9).unwrap();
        let json = stats_snapshot_json(&stats.snapshot(0));

        assert_stat_value(&json, "sim.trace_replay.data_cache.runs", "Count", 5);
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.attributed_runs",
            "Count",
            4,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.unattributed_runs",
            "Count",
            1,
        );
        assert_stat_value(&json, "sim.trace_replay.data_cache.msi.runs", "Count", 3);
        assert_stat_value(&json, "sim.trace_replay.data_cache.mesi.runs", "Count", 1);
        assert_stat_value(&json, "sim.trace_replay.data_cache.moesi.runs", "Count", 0);
        assert_stat_value(&json, "sim.trace_replay.data_cache.chi.runs", "Count", 0);
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.scheduler.epochs",
            "Count",
            7,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.scheduler.empty_epochs",
            "Count",
            1,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.scheduler.dispatches",
            "Count",
            6,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.scheduler.batches",
            "Count",
            4,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.scheduler.active_partitions",
            "Count",
            2,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.scheduler.max_workers",
            "Count",
            3,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.scheduler.total_workers",
            "Count",
            8,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.data_cache.dram_accesses",
            "Count",
            9,
        );
    }

    #[test]
    fn trace_replay_stats_emit_resource_activity_accounting() {
        let summary = WorkloadParallelExecutionSummary::default()
            .with_resource_wait_for_edge_kind_counts([(WaitForEdgeKind::Resource, 7)], []);
        let mut stats = StatsRegistry::new();

        emit_trace_replay_resource_stats(&mut stats, &summary).unwrap();
        let json = stats_snapshot_json(&stats.snapshot(0));

        assert_stat_value(&json, "sim.trace_replay.resources.activity", "Count", 7);
        assert_stat_value(&json, "sim.trace_replay.resources.active", "Count", 1);
    }

    fn assert_stat_value(json: &str, path: &str, unit: &str, value: u64) {
        let path_field = format!("\"path\":\"{path}\"");
        let path_index = json
            .find(&path_field)
            .unwrap_or_else(|| panic!("missing stat path {path} in {json}"));
        let sample_start = json[..path_index]
            .rfind('{')
            .unwrap_or_else(|| panic!("missing stat object start for {path} in {json}"));
        let sample_end = json[path_index..]
            .find('}')
            .map(|offset| path_index + offset + 1)
            .unwrap_or_else(|| panic!("missing stat object end for {path} in {json}"));
        let sample = &json[sample_start..sample_end];
        assert!(sample.contains(&format!("\"unit\":\"{unit}\"")));
        assert!(sample.contains(&format!("\"value\":{value}")));
    }

    fn route_id(value: &str) -> WorkloadRouteId {
        WorkloadRouteId::new(value).unwrap()
    }
}
