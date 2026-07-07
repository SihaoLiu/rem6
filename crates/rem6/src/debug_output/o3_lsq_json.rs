use rem6_cpu::{O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeStats};

fn o3_latency_json(
    samples: u64,
    ticks: u64,
    max_ticks: u64,
    min_ticks: u64,
    avg_ticks: u64,
) -> String {
    format!(
        "{{\"samples\":{samples},\"ticks\":{ticks},\"max_ticks\":{max_ticks},\"min_ticks\":{min_ticks},\"avg_ticks\":{avg_ticks}}}"
    )
}

fn o3_lsq_operation_matrix_json(stats: O3RuntimeStats) -> String {
    let fields = O3RuntimeLsqOperation::TRACKED
        .into_iter()
        .map(|operation| {
            let latency = o3_latency_json(
                stats.lsq_operation_latency_samples(operation),
                stats.lsq_operation_latency_ticks(operation),
                stats.lsq_operation_latency_max_ticks(operation),
                stats.lsq_operation_latency_min_ticks(operation),
                stats.lsq_operation_latency_avg_ticks(operation),
            );
            format!(
                "\"{}\":{{\"count\":{},\"forwarding_candidates\":{},\"forwarding_matches\":{},\"forwarding_suppressed\":{},\"forwarding_address_mismatches\":{},\"forwarding_byte_mismatches\":{},\"latency\":{latency}}}",
                operation.as_str(),
                stats.lsq_operation_count(operation),
                stats.lsq_operation_forwarding_candidates(operation),
                stats.lsq_operation_forwarding_matches(operation),
                stats.lsq_operation_forwarding_suppressed(operation),
                stats.lsq_operation_forwarding_address_mismatches(operation),
                stats.lsq_operation_forwarding_byte_mismatches(operation),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn o3_lsq_ordering_matrix_json(stats: O3RuntimeStats) -> String {
    let fields = O3RuntimeLsqOrdering::TRACKED
        .into_iter()
        .map(|ordering| {
            format!(
                "\"{}\":{}",
                ordering.as_str(),
                stats.lsq_ordering_count(ordering)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

pub(super) fn o3_lsq_to_json(stats: O3RuntimeStats) -> String {
    let data_latency = o3_latency_json(
        stats.lsq_data_latency_samples(),
        stats.lsq_data_latency_ticks(),
        stats.lsq_data_latency_max_ticks(),
        stats.lsq_data_latency_min_ticks(),
        stats.lsq_data_latency_avg_ticks(),
    );
    let operation = o3_lsq_operation_matrix_json(stats);
    let ordering = o3_lsq_ordering_matrix_json(stats);
    format!(
        "{{\"loads\":{},\"stores\":{},\"load_bytes\":{},\"store_bytes\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"store_load_forwarding_suppressed\":{},\"store_load_forwarding_address_mismatches\":{},\"store_load_forwarding_byte_mismatches\":{},\"store_conditional_failures\":{},\"max_occupancy\":{},\"data_latency\":{data_latency},\"operation\":{operation},\"ordering\":{ordering}}}",
        stats.lsq_loads(),
        stats.lsq_stores(),
        stats.lsq_load_bytes(),
        stats.lsq_store_bytes(),
        stats.lsq_store_to_load_forwarding_candidates(),
        stats.lsq_store_to_load_forwarding_matches(),
        stats.lsq_store_to_load_forwarding_suppressed(),
        stats.lsq_store_to_load_forwarding_address_mismatches(),
        stats.lsq_store_to_load_forwarding_byte_mismatches(),
        stats.lsq_store_conditional_failures(),
        stats.max_lsq_occupancy(),
    )
}
