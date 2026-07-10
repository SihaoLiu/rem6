use super::*;

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
            .pointer("/event_window")
            .is_some_and(Value::is_null),
        "non-debug O3 runtime JSON should expose an explicit null event window: {o3_runtime}"
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
        "sim.cpu0.o3.event_summary.event_window.records",
        "sim.cpu0.o3.event_summary.event_window.span_ticks",
        "sim.cpu0.o3.event_summary.event_window.max_rob_occupancy.tick",
        "sim.cpu0.o3.event_summary.event_window.max_lsq_occupancy.pc",
        "sim.cpu0.o3.event_summary.event_window.max_fu_latency.fu_latency_cycles",
        "sim.cpu0.o3.event_summary.event_window.max_lsq_data_latency.lsq_data_latency_ticks",
        "sim.cpu0.o3.event_summary.event_window.max_lsq_data_latency.lsq_ordering.acquire",
        "sim.cpu0.o3.event_summary.span_ticks",
        "sim.cpu0.o3.event_window.records",
        "sim.cpu0.o3.event_window.span_ticks",
        "sim.cpu0.o3.event_window.max_rob_occupancy.tick",
        "sim.cpu0.o3.event_window.max_lsq_occupancy.pc",
        "sim.cpu0.o3.event_window.max_fu_latency.fu_latency_cycles",
        "sim.cpu0.o3.event_window.max_lsq_data_latency.lsq_data_latency_ticks",
        "sim.cpu0.o3.event_window.max_lsq_data_latency.lsq_ordering.acquire",
        "sim.debug.o3_trace.cpu.cpu0.event_window.max_lsq_data_latency.lsq_ordering.acquire",
        "sim.cpu0.o3.event_summary.rob_allocations",
        "sim.cpu0.o3.event_summary.rob_commits",
        "sim.cpu0.o3.event_summary.rename_writes",
        "sim.cpu0.o3.event_summary.lsq_loads",
        "sim.cpu0.o3.event_summary.lsq_stores",
        "sim.cpu0.o3.event_summary.lsq_operation_load",
        "sim.cpu0.o3.event_summary.lsq_operation_store",
        "sim.cpu0.o3.event_summary.rob.allocations",
        "sim.cpu0.o3.event_summary.rob.max_occupancy",
        "sim.cpu0.o3.event_summary.rename.map_entries",
        "sim.cpu0.o3.event_summary.lsq_operation.load",
        "sim.cpu0.o3.event_summary.fu_latency.instructions",
        "sim.cpu0.o3.event_summary.fu_latency.cycles",
        "sim.cpu0.o3.event_summary.fu_latency.max_cycles",
        "sim.cpu0.o3.event_summary.fu_latency.min_cycles",
        "sim.cpu0.o3.event_summary.fu_latency.avg_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.instructions",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.max_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.min_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.avg_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.max_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.min_cycles",
        "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.avg_cycles",
        "sim.cpu0.o3.event_summary.iew.writeback_rate_ppm",
        "sim.cpu0.o3.event_summary.iew.producer_inst",
        "sim.cpu0.o3.event_summary.iew.consumer_inst",
        "sim.cpu0.o3.event_summary.iew.producer_consumer_fanout_ppm",
        "sim.cpu0.o3.event_summary.iew.predicted_taken_incorrect",
        "sim.cpu0.o3.event_summary.iew.predicted_not_taken_incorrect",
        "sim.cpu0.o3.event_summary.iew.dependency.producer",
        "sim.cpu0.o3.event_summary.iew.dependency.consumer",
        "sim.cpu0.o3.event_summary.branch_direction_mismatch.mismatches",
        "sim.cpu0.o3.event_summary.branch_direction_mismatch.kind.direct_unconditional",
        "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatches",
        "sim.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_kind.direct_conditional",
        "sim.cpu0.o3.branch_direction_mismatch.mismatches",
        "sim.cpu0.o3.branch_direction_mismatch.kind.direct_unconditional",
        "sim.cpu0.o3.branch_target_mismatch.targetless_mismatches",
        "sim.cpu0.o3.branch_target_mismatch.targetless_mismatch_kind.direct_conditional",
        "sim.cpu0.o3.event_summary.branch_event.taken",
        "sim.cpu0.o3.event_summary.branch_event.predicted_taken",
        "sim.cpu0.o3.event_summary.branch_event.predicted_target_matches",
        "sim.cpu0.o3.event_summary.branch_event.predicted_target_mismatches",
        "sim.cpu0.o3.event_summary.branch_event.predicted_target_match_kind.direct_conditional",
        "sim.cpu0.o3.event_summary.branch_event.resolved_target_kind.direct_unconditional",
        "sim.cpu0.o3.event_summary.branch_event.misprediction_kind.direct_conditional",
        "sim.cpu0.o3.event_summary.branch_event.link_write_kind.call_indirect",
        "sim.cpu0.o3.event_summary.branch_event.without_link_write_kind.direct_unconditional",
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
    for ordering in ["acquire", "release", "acquire_release"] {
        assert_json_stat(
            &json,
            &format!(
                "sim.debug.o3_trace.event_window.max_lsq_data_latency.lsq_ordering.{ordering}"
            ),
            "Count",
            0,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_m5_dump_reset_stats_suppresses_o3_event_window_without_debug_trace() {
    let path = detailed_o3_branch_dump_reset_stats_binary(
        "m5-switch-cpu-o3-event-window-dump-reset-suppressed",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
            "--stats-format",
            "json",
            "--execute",
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
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    for dump_index in [0, 1] {
        let dump = host_actions
            .pointer(&format!("/stats_dumps/{dump_index}"))
            .unwrap_or_else(|| panic!("missing stats dump action {dump_index}: {host_actions}"));
        assert_stats_dump_sample_at_least(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.instructions",
            "counter",
            "Count",
            1,
            "resettable",
        );
        for path in [
            "sim.host_actions.stats_dump.cpu0.o3.event_window.records",
            "sim.host_actions.stats_dump.cpu0.o3.event_window.span_ticks",
            "sim.host_actions.stats_dump.cpu0.o3.event_window.first.pc",
            "sim.host_actions.stats_dump.cpu0.o3.event_window.max_lsq_data_latency.lsq_data_latency_ticks",
            "sim.host_actions.stats_dump.cpu0.o3.event_window.max_fu_latency.fu_latency_cycles",
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.mispredictions",
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency.instructions",
        ] {
            assert_stats_dump_sample_absent(dump, path);
        }
    }
}

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_event_window_trace_rows() {
    let path =
        detailed_o3_branch_dump_reset_stats_binary("m5-switch-cpu-o3-event-window-dump-reset");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
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
    assert_eq!(
        json.pointer("/parallel/scheduler/worker_limit")
            .and_then(Value::as_u64),
        Some(1),
        "O3 debug trace host-action stats should use deterministic single-worker scheduling"
    );
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-reset stats dump action: {host_actions}"));
    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));

    for (dump, minimum_records) in [(pre_reset_dump, 6), (post_reset_dump, 2)] {
        assert_stats_dump_sample_at_least(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_window.records",
            "counter",
            "Count",
            minimum_records,
            "resettable",
        );
        assert_stats_dump_sample_at_least(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_window.span_ticks",
            "counter",
            "Tick",
            1,
            "resettable",
        );
        assert_stats_dump_sample_at_least(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_window.first.pc",
            "counter",
            "Address",
            0x8000_0000,
            "resettable",
        );
    }

    assert_stats_dump_sample_at_least(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_window.max_lsq_data_latency.pc",
        "counter",
        "Address",
        0x8000_0000,
        "resettable",
    );
    assert_stats_dump_sample_at_least(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_window.max_lsq_data_latency.lsq_data_latency_ticks",
        "counter",
        "Tick",
        1,
        "resettable",
    );
    assert_stats_dump_sample_at_least(
        post_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_window.max_fu_latency.pc",
        "counter",
        "Address",
        0x8000_0000,
        "resettable",
    );
    assert_stats_dump_sample_at_least(
        post_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_window.max_fu_latency.fu_latency_cycles",
        "counter",
        "Cycle",
        1,
        "resettable",
    );

    let pre_reset_records = stats_dump_sample_value(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_window.records",
    );
    let post_reset_records = stats_dump_sample_value(
        post_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_window.records",
    );
    assert!(
        pre_reset_records > post_reset_records,
        "m5_dump_reset_stats should reset O3 event-window trace rows before post-reset work: pre={pre_reset_records}, post={post_reset_records}"
    );
}
