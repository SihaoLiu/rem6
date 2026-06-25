pub(super) fn gups_response_stats_json(
    stats: &rem6_system::TrafficGupsTransportResponseStats,
) -> String {
    format!(
        "{{\"responses\":{},\"completed\":{},\"retry\":{},\"store_conditional_failed\":{},\"reads\":{},\"writes\":{},\"data_bytes\":{}}}",
        stats.response_count(),
        stats.completed_response_count(),
        stats.retry_response_count(),
        stats.store_conditional_failed_response_count(),
        stats.read_response_count(),
        stats.write_response_count(),
        stats.response_data_byte_count(),
    )
}
