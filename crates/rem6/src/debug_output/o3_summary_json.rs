use rem6_cpu::{O3RuntimeFuLatencyClass, O3RuntimeStats};

fn o3_inst_type_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "int_div",
        _ => class.stat_stem(),
    }
}

fn o3_inst_type_json(stats: O3RuntimeStats) -> String {
    let mut fields = vec![
        format!("\"mem_read\":{}", stats.lsq_loads()),
        format!("\"mem_write\":{}", stats.lsq_stores()),
    ];
    fields.extend(O3RuntimeFuLatencyClass::ALL.into_iter().map(|class| {
        format!(
            "\"{}\":{}",
            o3_inst_type_stem(class),
            stats.fu_latency_class_instructions(class)
        )
    }));
    format!("{{{}}}", fields.join(","))
}

fn o3_branch_mispredicts(stats: O3RuntimeStats) -> u64 {
    stats
        .iew_predicted_taken_incorrect()
        .saturating_add(stats.iew_predicted_not_taken_incorrect())
}

pub(super) fn o3_iq_to_json(stats: O3RuntimeStats) -> String {
    let issued_inst_type = o3_inst_type_json(stats);
    format!(
        "{{\"insts_issued\":{},\"mem_insts_issued\":{},\"branch_insts_issued\":{},\"issued_inst_type\":{issued_inst_type}}}",
        stats.instructions(),
        stats.lsq_loads().saturating_add(stats.lsq_stores()),
        stats.iq_branch_insts_issued(),
    )
}

pub(super) fn o3_iew_to_json(stats: O3RuntimeStats) -> String {
    format!(
        "{{\"dispatched_insts\":{},\"insts_to_commit\":{},\"writeback_count\":{},\"producer_inst\":{},\"consumer_inst\":{},\"predicted_taken_incorrect\":{},\"predicted_not_taken_incorrect\":{},\"branch_mispredicts\":{}}}",
        stats.instructions(),
        stats.rob_commits(),
        stats.instructions(),
        stats.iew_producer_insts(),
        stats.iew_consumer_insts(),
        stats.iew_predicted_taken_incorrect(),
        stats.iew_predicted_not_taken_incorrect(),
        o3_branch_mispredicts(stats),
    )
}

pub(super) fn o3_commit_to_json(stats: O3RuntimeStats) -> String {
    let committed_inst_type = o3_inst_type_json(stats);
    format!(
        "{{\"branch_mispredicts\":{},\"committed_inst_type\":{committed_inst_type}}}",
        o3_branch_mispredicts(stats)
    )
}

pub(super) fn o3_rob_to_json(stats: O3RuntimeStats) -> String {
    format!(
        "{{\"allocations\":{},\"commits\":{},\"max_occupancy\":{}}}",
        stats.rob_allocations(),
        stats.rob_commits(),
        stats.max_rob_occupancy()
    )
}

pub(super) fn o3_rename_to_json(stats: O3RuntimeStats) -> String {
    format!(
        "{{\"writes\":{},\"map_entries\":{}}}",
        stats.rename_writes(),
        stats.rename_map_entries()
    )
}
