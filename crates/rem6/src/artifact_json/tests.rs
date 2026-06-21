use crate::CliDataCacheSummary;

use rem6_workload::{
    WorkloadParallelExecutionSummary, WorkloadRouteId, WorkloadTrafficTraceReplaySummary,
};

use super::traffic_trace_summary_json;

#[test]
fn traffic_trace_summary_json_emits_nonzero_cache_and_sideband_counters() {
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

    let parallel_summary = WorkloadParallelExecutionSummary::default();
    let data_cache = CliDataCacheSummary {
        cpu_responses: 11,
        directory_decisions: 13,
        ..CliDataCacheSummary::default()
    };
    let data_cache_dram_summary = WorkloadParallelExecutionSummary::default()
        .with_dram_activity(2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13);

    let json = traffic_trace_summary_json(
        &summary,
        &parallel_summary,
        &data_cache,
        &data_cache_dram_summary,
        7,
    );

    assert!(json.contains("\"trace_invalidate_response_count\":1"));
    assert!(json.contains("\"trace_clean_response_count\":1"));
    assert!(json.contains("\"trace_data_cache_response_count\":3"));
    assert!(json.contains("\"trace_data_cache_maintenance_response_count\":2"));
    assert!(json.contains("\"trace_data_cache_clean_maintenance_response_count\":1"));
    assert!(json.contains("\"trace_data_cache_invalidate_maintenance_response_count\":1"));
    assert!(json.contains("\"trace_error_count\":2"));
    assert!(json.contains("\"trace_error_write_count\":1"));
    assert!(json.contains("\"trace_error_functional_write_count\":1"));
    assert!(json.contains("\"trace_cache_flush_count\":1"));
    assert!(json.contains("\"trace_cache_flush_data_byte_count\":64"));
    assert!(json.contains("\"trace_l1_invalidation_count\":1"));
    assert!(json.contains("\"trace_diagnostic_count\":1"));
    assert!(json.contains("\"data_cache_dram_accesses\":7"));
    assert!(json.contains("\"data_cache_cpu_responses\":11"));
    assert!(json.contains("\"data_cache_directory_decisions\":13"));
    assert!(json.contains("\"active_dram_target_count\":2"));
    assert!(json.contains("\"active_dram_port_count\":3"));
    assert!(json.contains("\"active_dram_bank_count\":4"));
    assert!(json.contains("\"dram_access_count\":5"));
    assert!(json.contains("\"dram_read_count\":6"));
    assert!(json.contains("\"dram_write_count\":7"));
    assert!(json.contains("\"dram_row_hit_count\":8"));
    assert!(json.contains("\"dram_row_miss_count\":9"));
    assert!(json.contains("\"dram_command_count\":10"));
    assert!(json.contains("\"dram_turnaround_count\":11"));
    assert!(json.contains("\"dram_total_ready_latency_cycles\":12"));
    assert!(json.contains("\"dram_max_ready_latency_cycles\":13"));
}

fn route_id(value: &str) -> WorkloadRouteId {
    WorkloadRouteId::new(value).unwrap()
}
