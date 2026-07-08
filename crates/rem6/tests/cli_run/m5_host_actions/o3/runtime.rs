use super::*;

#[test]
fn rem6_run_o3_runtime_json_exposes_trace_event_summary() {
    let path =
        detailed_o3_iq_iew_commit_matrix_binary("m5-switch-cpu-detailed-o3-runtime-event-summary");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--debug-flags",
            "O3",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    let runtime_summary = o3_runtime
        .pointer("/event_summary")
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose event summary: {o3_runtime}"));
    let debug_summary = json
        .pointer("/debug/o3_trace/0/event_summary")
        .unwrap_or_else(|| panic!("O3 debug trace should expose event summary: {json}"));

    assert_eq!(
        runtime_summary.pointer("/records"),
        o3_runtime.pointer("/instructions"),
        "runtime event summary should count the same real O3 events as runtime instructions: {runtime_summary}"
    );
    for pointer in [
        "/records",
        "/span_ticks",
        "/max_rob_occupancy",
        "/max_lsq_occupancy",
        "/max_rename_map_entries",
        "/event_window/max_rob_occupancy/tick",
        "/event_window/max_lsq_occupancy/sequence",
        "/rob/allocations",
        "/rob/commits",
        "/rename/writes",
        "/lsq_operation/load/count",
        "/lsq_operation/store/count",
        "/lsq_data_latency/samples",
        "/lsq_data_latency/ticks",
        "/iq/issued_inst_type/int_mul",
        "/iq/issued_inst_type/int_div",
        "/iew/dispatched_insts",
        "/iew/writeback_count",
        "/iew/writeback_rate_ppm",
        "/iew/producer_inst",
        "/iew/consumer_inst",
        "/iew/producer_consumer_fanout_ppm",
        "/iew/dependency/producer",
        "/iew/dependency/consumer",
        "/commit/committed_inst_type/int_mul",
        "/commit/committed_inst_type/int_div",
        "/fu_latency_class/integer_mul/instructions",
        "/fu_latency_class/integer_div/cycles",
    ] {
        assert_eq!(
            runtime_summary.pointer(pointer),
            debug_summary.pointer(pointer),
            "runtime event-summary lane {pointer} should mirror debug trace event summary"
        );
        assert!(
            runtime_summary
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "representative runtime event-summary lane {pointer} should be positive: {runtime_summary}"
        );
    }

    for (pointer, stat_path, unit) in [
        (
            "/first_tick",
            "sim.cpu0.o3.event_summary.first_tick",
            "Tick",
        ),
        ("/last_tick", "sim.cpu0.o3.event_summary.last_tick", "Tick"),
        ("/records", "sim.cpu0.o3.event_summary.records", "Count"),
        (
            "/span_ticks",
            "sim.cpu0.o3.event_summary.span_ticks",
            "Tick",
        ),
        (
            "/event_window/first/tick",
            "sim.cpu0.o3.event_summary.event_window.first.tick",
            "Tick",
        ),
        (
            "/event_window/last/sequence",
            "sim.cpu0.o3.event_summary.event_window.last.sequence",
            "Count",
        ),
        (
            "/event_window/max_rob_occupancy/tick",
            "sim.cpu0.o3.event_summary.event_window.max_rob_occupancy.tick",
            "Tick",
        ),
        (
            "/event_window/max_lsq_occupancy/lsq_occupancy",
            "sim.cpu0.o3.event_summary.event_window.max_lsq_occupancy.lsq_occupancy",
            "Count",
        ),
        (
            "/event_window/max_rename_map_entries/rename_map_entries",
            "sim.cpu0.o3.event_summary.event_window.max_rename_map_entries.rename_map_entries",
            "Count",
        ),
        (
            "/rob/allocations",
            "sim.cpu0.o3.event_summary.rob.allocations",
            "Count",
        ),
        (
            "/rob/commits",
            "sim.cpu0.o3.event_summary.rob.commits",
            "Count",
        ),
        (
            "/rename/writes",
            "sim.cpu0.o3.event_summary.rename.writes",
            "Count",
        ),
        (
            "/lsq_operation/load/count",
            "sim.cpu0.o3.event_summary.lsq_operation.load",
            "Count",
        ),
        (
            "/lsq_operation/store/count",
            "sim.cpu0.o3.event_summary.lsq_operation.store",
            "Count",
        ),
        (
            "/lsq_data_latency/samples",
            "sim.cpu0.o3.event_summary.lsq_data_latency.samples",
            "Count",
        ),
        (
            "/lsq_data_latency/ticks",
            "sim.cpu0.o3.event_summary.lsq_data_latency.ticks",
            "Tick",
        ),
        (
            "/iq/issued_inst_type/int_mul",
            "sim.cpu0.o3.event_summary.iq.issued_inst_type.int_mul",
            "Count",
        ),
        (
            "/iew/writeback_count",
            "sim.cpu0.o3.event_summary.iew.writeback_count",
            "Count",
        ),
        (
            "/iew/writeback_rate_ppm",
            "sim.cpu0.o3.event_summary.iew.writeback_rate_ppm",
            "Ppm",
        ),
        (
            "/iew/producer_inst",
            "sim.cpu0.o3.event_summary.iew.producer_inst",
            "Count",
        ),
        (
            "/iew/consumer_inst",
            "sim.cpu0.o3.event_summary.iew.consumer_inst",
            "Count",
        ),
        (
            "/iew/producer_consumer_fanout_ppm",
            "sim.cpu0.o3.event_summary.iew.producer_consumer_fanout_ppm",
            "Ppm",
        ),
        (
            "/iew/dependency/producer",
            "sim.cpu0.o3.event_summary.iew.dependency.producer",
            "Count",
        ),
        (
            "/iew/dependency/consumer",
            "sim.cpu0.o3.event_summary.iew.dependency.consumer",
            "Count",
        ),
        (
            "/commit/committed_inst_type/int_div",
            "sim.cpu0.o3.event_summary.commit.committed_inst_type.int_div",
            "Count",
        ),
        (
            "/fu_latency_class/integer_mul/instructions",
            "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.instructions",
            "Count",
        ),
        (
            "/fu_latency_class/integer_div/cycles",
            "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.cycles",
            "Cycle",
        ),
    ] {
        let expected = runtime_summary
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("runtime event summary should expose u64 lane {pointer}: {runtime_summary}")
            });
        assert_json_stat(&json, stat_path, unit, expected, "monotonic");
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_branch_mismatch_trace_partitions() {
    let path = detailed_o3_branch_repair_text_stats_binary(
        "m5-switch-cpu-detailed-o3-runtime-branch-mismatch",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--debug-flags",
            "O3",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    let runtime_direction = o3_runtime
        .pointer("/branch_direction_mismatch")
        .unwrap_or_else(|| {
            panic!("O3 runtime JSON should expose direction mismatch partitions: {o3_runtime}")
        });
    let runtime_target = o3_runtime
        .pointer("/branch_target_mismatch")
        .unwrap_or_else(|| {
            panic!("O3 runtime JSON should expose target mismatch partitions: {o3_runtime}")
        });
    let runtime_summary = o3_runtime
        .pointer("/event_summary")
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose event summary: {o3_runtime}"));
    let debug_direction = json
        .pointer("/debug/o3_trace/0/branch_direction_mismatch")
        .unwrap_or_else(|| {
            panic!("O3 debug trace should expose direction mismatch partitions: {json}")
        });
    let debug_target = json
        .pointer("/debug/o3_trace/0/branch_target_mismatch")
        .unwrap_or_else(|| {
            panic!("O3 debug trace should expose target mismatch partitions: {json}")
        });
    let debug_summary = json
        .pointer("/debug/o3_trace/0/event_summary")
        .unwrap_or_else(|| panic!("O3 debug trace should expose event summary: {json}"));

    for pointer in [
        "/branch_event/squashes",
        "/branch_event/squashed_targets",
        "/branch_event/squashed_target_kind/direct_unconditional",
        "/branch_event/squashed_target_without_link_write_kind/direct_unconditional",
        "/branch_repair/targetless_mismatches",
        "/branch_repair/direction_only_mismatches",
        "/branch_repair/targetless_mismatch_kind/direct_conditional",
        "/branch_repair/direction_only_kind/direct_unconditional",
    ] {
        assert_eq!(
            runtime_summary.pointer(pointer),
            debug_summary.pointer(pointer),
            "runtime event-summary branch lane {pointer} should mirror debug trace event summary"
        );
        assert!(
            runtime_summary
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "representative runtime event-summary branch lane {pointer} should be positive: {runtime_summary}"
        );
    }

    for pointer in [
        "/mismatches",
        "/without_link_writes",
        "/squashed_targets",
        "/kind/direct_unconditional",
        "/squashed_target_without_link_write_kind/direct_unconditional",
    ] {
        assert_eq!(
            runtime_direction.pointer(pointer),
            debug_direction.pointer(pointer),
            "runtime direction-mismatch lane {pointer} should mirror debug trace"
        );
        assert!(
            runtime_direction
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "representative direction-mismatch lane {pointer} should be positive: {runtime_direction}"
        );
    }

    for pointer in [
        "/targetless_mismatches",
        "/targetless_mismatch_without_link_writes",
        "/targetless_mismatch_squashed_targets",
        "/targetless_mismatch_kind/direct_conditional",
        "/targetless_mismatch_squashed_target_without_link_write_kind/direct_conditional",
    ] {
        assert_eq!(
            runtime_target.pointer(pointer),
            debug_target.pointer(pointer),
            "runtime target-mismatch lane {pointer} should mirror debug trace"
        );
        assert!(
            runtime_target
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "representative target-mismatch lane {pointer} should be positive: {runtime_target}"
        );
    }

    for pointer in [
        "/wrong_targets",
        "/wrong_target_squashed_targets",
        "/wrong_target_link_writes",
        "/wrong_target_without_link_writes",
    ] {
        assert_eq!(
            runtime_target.pointer(pointer),
            debug_target.pointer(pointer),
            "runtime wrong-target lane {pointer} should mirror debug trace"
        );
    }

    for (pointer, stat_path) in [
        (
            "/mismatches",
            "sim.cpu0.o3.event_summary.branch_direction_mismatch.mismatches",
        ),
        (
            "/without_link_writes",
            "sim.cpu0.o3.event_summary.branch_direction_mismatch.without_link_writes",
        ),
        (
            "/squashed_targets",
            "sim.cpu0.o3.event_summary.branch_direction_mismatch.squashed_targets",
        ),
        (
            "/kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_direction_mismatch.kind.direct_unconditional",
        ),
        (
            "/squashed_target_without_link_write_kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_direction_mismatch.squashed_target_without_link_write_kind.direct_unconditional",
        ),
    ] {
        let expected = runtime_direction
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("runtime direction mismatch should expose u64 lane {pointer}: {runtime_direction}")
            });
        assert_json_stat(&json, stat_path, "Count", expected, "monotonic");
    }

    for (pointer, stat_path) in [
        (
            "/targetless_mismatches",
            "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatches",
        ),
        (
            "/targetless_mismatch_without_link_writes",
            "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_without_link_writes",
        ),
        (
            "/targetless_mismatch_squashed_targets",
            "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_squashed_targets",
        ),
        (
            "/targetless_mismatch_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_kind.direct_conditional",
        ),
        (
            "/targetless_mismatch_squashed_target_without_link_write_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_squashed_target_without_link_write_kind.direct_conditional",
        ),
        (
            "/wrong_targets",
            "sim.cpu0.o3.event_summary.branch_target_mismatch.wrong_targets",
        ),
    ] {
        let expected = runtime_target
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("runtime target mismatch should expose u64 lane {pointer}: {runtime_target}")
            });
        assert_json_stat(&json, stat_path, "Count", expected, "monotonic");
    }

    for (pointer, stat_path) in [
        (
            "/branch_event/squashes",
            "sim.cpu0.o3.event_summary.branch_event.squashes",
        ),
        (
            "/branch_event/squashed_targets",
            "sim.cpu0.o3.event_summary.branch_event.squashed_targets",
        ),
        (
            "/branch_event/squashed_target_kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_event.squashed_target_kind.direct_unconditional",
        ),
        (
            "/branch_event/squashed_target_without_link_write_kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_event.squashed_target_without_link_write_kind.direct_unconditional",
        ),
        (
            "/branch_repair/targetless_mismatches",
            "sim.cpu0.o3.event_summary.branch_repair.targetless_mismatches",
        ),
        (
            "/branch_repair/direction_only_mismatches",
            "sim.cpu0.o3.event_summary.branch_repair.direction_only_mismatches",
        ),
        (
            "/branch_repair/targetless_mismatch_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_repair.targetless_mismatch_kind.direct_conditional",
        ),
        (
            "/branch_repair/direction_only_kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_repair.direction_only_kind.direct_unconditional",
        ),
    ] {
        let expected = runtime_summary
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("runtime event summary should expose u64 lane {pointer}: {runtime_summary}")
            });
        assert_json_stat(&json, stat_path, "Count", expected, "monotonic");
    }
}

#[test]
fn rem6_run_o3_runtime_json_keeps_trace_event_summary_null_without_debug_trace() {
    let path = detailed_o3_float_extended_fu_latency_binary(
        "m5-switch-cpu-detailed-o3-runtime-event-summary-suppressed",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    assert!(
        o3_runtime
            .pointer("/event_summary")
            .is_some_and(Value::is_null),
        "non-debug O3 runtime JSON should expose an explicit null event summary: {o3_runtime}"
    );
    assert!(
        o3_runtime
            .pointer("/branch_direction_mismatch")
            .is_some_and(Value::is_null),
        "non-debug O3 runtime JSON should expose an explicit null direction mismatch summary: {o3_runtime}"
    );
    assert!(
        o3_runtime
            .pointer("/branch_target_mismatch")
            .is_some_and(Value::is_null),
        "non-debug O3 runtime JSON should expose an explicit null target mismatch summary: {o3_runtime}"
    );
    for path in [
        "sim.cpu0.o3.event_summary.records",
        "sim.cpu0.o3.event_summary.first_tick",
        "sim.cpu0.o3.event_summary.event_window.max_rob_occupancy.tick",
        "sim.cpu0.o3.event_summary.span_ticks",
        "sim.cpu0.o3.event_summary.rob.allocations",
        "sim.cpu0.o3.event_summary.lsq_operation.load",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.instructions",
        "sim.cpu0.o3.event_summary.iew.writeback_rate_ppm",
        "sim.cpu0.o3.event_summary.iew.producer_inst",
        "sim.cpu0.o3.event_summary.iew.consumer_inst",
        "sim.cpu0.o3.event_summary.iew.producer_consumer_fanout_ppm",
        "sim.cpu0.o3.event_summary.iew.dependency.producer",
        "sim.cpu0.o3.event_summary.iew.dependency.consumer",
        "sim.cpu0.o3.event_summary.branch_direction_mismatch.mismatches",
        "sim.cpu0.o3.event_summary.branch_direction_mismatch.kind.direct_unconditional",
        "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatches",
        "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_kind.direct_conditional",
        "sim.cpu0.o3.event_summary.branch_event.squashes",
        "sim.cpu0.o3.event_summary.branch_event.squashed_target_kind.direct_unconditional",
        "sim.cpu0.o3.event_summary.branch_repair.targetless_mismatches",
        "sim.cpu0.o3.event_summary.branch_repair.direction_only_kind.direct_unconditional",
        "sim.cpu0.o3.event_summary.store_load_forwarding_matches",
        "sim.cpu0.o3.event_summary.lsq_operation.load.forwarding_matches",
        "sim.cpu0.o3.event_summary.lsq_operation.load.latency.ticks",
        "sim.cpu0.o3.event_summary.lsq_ordering.acquire_release",
    ] {
        assert_json_stat_absent(&json, path);
    }
}
