use rem6_cpu::{O3RuntimeFuLatencyClass, O3RuntimeStats};

fn o3_commit_inst_type_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "int_div",
        _ => class.stat_stem(),
    }
}

fn o3_commit_inst_type_json(stats: O3RuntimeStats) -> String {
    let mut fields = vec![
        format!("\"mem_read\":{}", stats.lsq_loads()),
        format!("\"mem_write\":{}", stats.lsq_stores()),
    ];
    fields.extend(O3RuntimeFuLatencyClass::ALL.into_iter().map(|class| {
        format!(
            "\"{}\":{}",
            o3_commit_inst_type_stem(class),
            stats.fu_latency_class_instructions(class)
        )
    }));
    format!("{{{}}}", fields.join(","))
}

pub(super) fn o3_commit_to_json(stats: O3RuntimeStats) -> String {
    let committed_inst_type = o3_commit_inst_type_json(stats);
    let branch_mispredicts = stats
        .iew_predicted_taken_incorrect()
        .saturating_add(stats.iew_predicted_not_taken_incorrect());
    format!(
        "{{\"branch_mispredicts\":{},\"committed_inst_type\":{committed_inst_type}}}",
        branch_mispredicts
    )
}
