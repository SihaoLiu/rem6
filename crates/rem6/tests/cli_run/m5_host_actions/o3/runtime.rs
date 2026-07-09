use super::*;

#[path = "runtime/event_summary_dump.rs"]
mod event_summary_dump;

fn event_summary_hex_u64(value: &Value, pointer: &str) -> u64 {
    let hex = value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            panic!("runtime event summary should expose hex lane {pointer}: {value}")
        });
    u64::from_str_radix(hex.strip_prefix("0x").unwrap_or(hex), 16)
        .unwrap_or_else(|error| panic!("invalid hex lane {pointer}={hex}: {error}"))
}

fn event_structural_pressure_key(event: &Value) -> (u64, u64, u64, u64, u64, u64) {
    let rob_occupancy = event
        .get("rob_occupancy")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let lsq_occupancy = event
        .get("lsq_occupancy")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let rename_map_entries = event
        .get("rename_map_entries")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let active_structures = u64::from(rob_occupancy != 0)
        + u64::from(lsq_occupancy != 0)
        + u64::from(rename_map_entries != 0);
    (
        active_structures,
        rob_occupancy
            .saturating_add(lsq_occupancy)
            .saturating_add(rename_map_entries),
        rob_occupancy,
        lsq_occupancy,
        rename_map_entries,
        event.get("sequence").and_then(Value::as_u64).unwrap_or(0),
    )
}

fn assert_event_window_row_matches_event(row: &Value, event: &Value, label: &str) {
    for field in [
        "sequence",
        "tick",
        "issue_tick",
        "writeback_tick",
        "commit_tick",
        "pc",
        "rob_occupancy",
        "rob_commits_at_tick",
        "rob_commit_blocked",
        "lsq_occupancy",
        "rename_map_entries",
        "lsq_data_latency_ticks",
        "fu_latency_cycles",
    ] {
        assert_eq!(
            row.get(field),
            event.get(field),
            "event-window row {label}.{field} should be selected from the raw trace event"
        );
    }
}

fn assert_event_window_timing_stat_rows(
    json: &Value,
    window: &Value,
    pointer_prefix: &str,
    stat_prefix: &str,
) {
    for row in [
        "max_rob_occupancy",
        "max_structural_pressure",
        "max_fu_latency",
        "max_lsq_data_latency",
    ] {
        for lane in ["issue_tick", "writeback_tick", "commit_tick"] {
            let pointer = format!("{pointer_prefix}/{row}/{lane}");
            let expected = window
                .pointer(&pointer)
                .and_then(Value::as_u64)
                .expect("event window timing lane");
            assert_json_stat(
                json,
                &format!("{stat_prefix}.{row}.{lane}"),
                "Tick",
                expected,
                "monotonic",
            );
        }
    }
}

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
    let runtime_window = o3_runtime
        .pointer("/event_window")
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose event window: {o3_runtime}"));
    let debug_summary = json
        .pointer("/debug/o3_trace/0/event_summary")
        .unwrap_or_else(|| panic!("O3 debug trace should expose event summary: {json}"));
    let events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("O3 debug trace should expose raw events: {json}"));
    let max_fu_latency_event = events
        .iter()
        .max_by_key(|event| {
            event
                .get("fu_latency_cycles")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        })
        .expect("O3 trace should include events");
    let max_lsq_data_latency_event = events
        .iter()
        .max_by_key(|event| {
            event
                .get("lsq_data_latency_ticks")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        })
        .expect("O3 trace should include events");
    let max_structural_pressure_event = events
        .iter()
        .max_by_key(|event| event_structural_pressure_key(event))
        .expect("O3 trace should include events");
    assert_event_window_row_matches_event(
        runtime_window
            .pointer("/max_fu_latency")
            .expect("runtime event window max FU latency row"),
        max_fu_latency_event,
        "max_fu_latency",
    );
    assert_event_window_row_matches_event(
        runtime_window
            .pointer("/max_lsq_data_latency")
            .expect("runtime event window max LSQ data latency row"),
        max_lsq_data_latency_event,
        "max_lsq_data_latency",
    );
    assert_event_window_row_matches_event(
        runtime_window
            .pointer("/max_structural_pressure")
            .expect("runtime event window max structural-pressure row"),
        max_structural_pressure_event,
        "max_structural_pressure",
    );
    assert_eq!(
        runtime_window,
        runtime_summary
            .pointer("/event_window")
            .unwrap_or_else(|| panic!("runtime event summary should expose event window: {runtime_summary}")),
        "top-level O3 runtime event window should be the same trace-window state as the event summary"
    );

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
        "/rob_allocations",
        "/rob_commits",
        "/rename_writes",
        "/lsq_loads",
        "/lsq_stores",
        "/lsq_operation_load",
        "/lsq_operation_store",
        "/event_window/records",
        "/event_window/span_ticks",
        "/event_window/max_rob_occupancy/tick",
        "/event_window/max_rob_occupancy/issue_tick",
        "/event_window/max_rob_occupancy/writeback_tick",
        "/event_window/max_rob_occupancy/commit_tick",
        "/event_window/max_lsq_occupancy/sequence",
        "/event_window/max_structural_pressure/rob_occupancy",
        "/event_window/max_structural_pressure/issue_tick",
        "/event_window/max_structural_pressure/writeback_tick",
        "/event_window/max_structural_pressure/commit_tick",
        "/rob/allocations",
        "/rob/commits",
        "/rob/max_occupancy",
        "/rename/writes",
        "/rename/map_entries",
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
        "/fu_latency_instructions",
        "/fu_latency_cycles",
        "/event_window/max_fu_latency/fu_latency_cycles",
        "/event_window/max_fu_latency/issue_tick",
        "/event_window/max_fu_latency/writeback_tick",
        "/event_window/max_fu_latency/commit_tick",
        "/event_window/max_lsq_data_latency/lsq_data_latency_ticks",
        "/event_window/max_lsq_data_latency/issue_tick",
        "/event_window/max_lsq_data_latency/writeback_tick",
        "/event_window/max_lsq_data_latency/commit_tick",
        "/fu_latency_max_cycles",
        "/fu_latency_min_cycles",
        "/fu_latency_avg_cycles",
        "/fu_latency_class/integer_mul/instructions",
        "/fu_latency_class/integer_mul/max_cycles",
        "/fu_latency_class/integer_mul/min_cycles",
        "/fu_latency_class/integer_mul/avg_cycles",
        "/fu_latency_class/integer_div/cycles",
        "/fu_latency_class/integer_div/max_cycles",
        "/fu_latency_class/integer_div/min_cycles",
        "/fu_latency_class/integer_div/avg_cycles",
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

    for pointer in [
        "/event_window/first/pc",
        "/event_window/last/pc",
        "/event_window/max_rob_occupancy/pc",
        "/event_window/max_lsq_occupancy/pc",
        "/event_window/max_rename_map_entries/pc",
        "/event_window/max_structural_pressure/pc",
        "/event_window/max_fu_latency/pc",
        "/event_window/max_lsq_data_latency/pc",
    ] {
        assert_eq!(
            runtime_summary.pointer(pointer),
            debug_summary.pointer(pointer),
            "runtime event-summary lane {pointer} should mirror debug trace event summary"
        );
        let pc = event_summary_hex_u64(runtime_summary, pointer);
        assert!(
            pc > 0,
            "representative runtime event-summary lane {pointer} should be a positive PC: {runtime_summary}"
        );
    }

    for pointer in [
        "/event_window/max_rob_occupancy/rob_commits_at_tick",
        "/event_window/max_rob_occupancy/rob_commit_blocked",
        "/event_window/max_structural_pressure/rob_commits_at_tick",
        "/event_window/max_structural_pressure/rob_commit_blocked",
        "/event_window/max_fu_latency/rob_commits_at_tick",
        "/event_window/max_fu_latency/rob_commit_blocked",
        "/event_window/max_lsq_data_latency/rob_commits_at_tick",
        "/event_window/max_lsq_data_latency/rob_commit_blocked",
    ] {
        assert_eq!(
            runtime_summary.pointer(pointer),
            debug_summary.pointer(pointer),
            "runtime event-summary lane {pointer} should mirror debug trace event summary"
        );
        assert!(
            runtime_summary.pointer(pointer).is_some(),
            "runtime event-summary lane {pointer} should be present: {runtime_summary}"
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
            "/event_window/records",
            "sim.cpu0.o3.event_summary.event_window.records",
            "Count",
        ),
        (
            "/event_window/span_ticks",
            "sim.cpu0.o3.event_summary.event_window.span_ticks",
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
            "/event_window/max_structural_pressure/rename_map_entries",
            "sim.cpu0.o3.event_summary.event_window.max_structural_pressure.rename_map_entries",
            "Count",
        ),
        (
            "/event_window/max_fu_latency/fu_latency_cycles",
            "sim.cpu0.o3.event_summary.event_window.max_fu_latency.fu_latency_cycles",
            "Cycle",
        ),
        (
            "/event_window/max_lsq_data_latency/lsq_data_latency_ticks",
            "sim.cpu0.o3.event_summary.event_window.max_lsq_data_latency.lsq_data_latency_ticks",
            "Tick",
        ),
        (
            "/rob_allocations",
            "sim.cpu0.o3.event_summary.rob_allocations",
            "Count",
        ),
        (
            "/rob_commits",
            "sim.cpu0.o3.event_summary.rob_commits",
            "Count",
        ),
        (
            "/rename_writes",
            "sim.cpu0.o3.event_summary.rename_writes",
            "Count",
        ),
        ("/lsq_loads", "sim.cpu0.o3.event_summary.lsq_loads", "Count"),
        (
            "/lsq_stores",
            "sim.cpu0.o3.event_summary.lsq_stores",
            "Count",
        ),
        (
            "/lsq_operation_load",
            "sim.cpu0.o3.event_summary.lsq_operation_load",
            "Count",
        ),
        (
            "/lsq_operation_store",
            "sim.cpu0.o3.event_summary.lsq_operation_store",
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
            "/rob/max_occupancy",
            "sim.cpu0.o3.event_summary.rob.max_occupancy",
            "Count",
        ),
        (
            "/rename/writes",
            "sim.cpu0.o3.event_summary.rename.writes",
            "Count",
        ),
        (
            "/rename/map_entries",
            "sim.cpu0.o3.event_summary.rename.map_entries",
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
            "/iew/predicted_taken_incorrect",
            "sim.cpu0.o3.event_summary.iew.predicted_taken_incorrect",
            "Count",
        ),
        (
            "/iew/predicted_not_taken_incorrect",
            "sim.cpu0.o3.event_summary.iew.predicted_not_taken_incorrect",
            "Count",
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
            "/fu_latency_instructions",
            "sim.cpu0.o3.event_summary.fu_latency.instructions",
            "Count",
        ),
        (
            "/fu_latency_cycles",
            "sim.cpu0.o3.event_summary.fu_latency.cycles",
            "Cycle",
        ),
        (
            "/fu_latency_max_cycles",
            "sim.cpu0.o3.event_summary.fu_latency.max_cycles",
            "Cycle",
        ),
        (
            "/fu_latency_min_cycles",
            "sim.cpu0.o3.event_summary.fu_latency.min_cycles",
            "Cycle",
        ),
        (
            "/fu_latency_avg_cycles",
            "sim.cpu0.o3.event_summary.fu_latency.avg_cycles",
            "Cycle",
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
        (
            "/fu_latency_class/integer_mul/max_cycles",
            "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.max_cycles",
            "Cycle",
        ),
        (
            "/fu_latency_class/integer_mul/min_cycles",
            "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.min_cycles",
            "Cycle",
        ),
        (
            "/fu_latency_class/integer_mul/avg_cycles",
            "sim.cpu0.o3.event_summary.fu_latency_class.integer_mul.avg_cycles",
            "Cycle",
        ),
        (
            "/fu_latency_class/integer_div/max_cycles",
            "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.max_cycles",
            "Cycle",
        ),
        (
            "/fu_latency_class/integer_div/min_cycles",
            "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.min_cycles",
            "Cycle",
        ),
        (
            "/fu_latency_class/integer_div/avg_cycles",
            "sim.cpu0.o3.event_summary.fu_latency_class.integer_div.avg_cycles",
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
    assert_event_window_timing_stat_rows(
        &json,
        runtime_summary,
        "/event_window",
        "sim.cpu0.o3.event_summary.event_window",
    );

    for (pointer, stat_path) in [
        (
            "/event_window/first/pc",
            "sim.cpu0.o3.event_summary.event_window.first.pc",
        ),
        (
            "/event_window/last/pc",
            "sim.cpu0.o3.event_summary.event_window.last.pc",
        ),
        (
            "/event_window/max_rob_occupancy/pc",
            "sim.cpu0.o3.event_summary.event_window.max_rob_occupancy.pc",
        ),
        (
            "/event_window/max_lsq_occupancy/pc",
            "sim.cpu0.o3.event_summary.event_window.max_lsq_occupancy.pc",
        ),
        (
            "/event_window/max_rename_map_entries/pc",
            "sim.cpu0.o3.event_summary.event_window.max_rename_map_entries.pc",
        ),
        (
            "/event_window/max_structural_pressure/pc",
            "sim.cpu0.o3.event_summary.event_window.max_structural_pressure.pc",
        ),
    ] {
        assert_json_stat(
            &json,
            stat_path,
            "Address",
            event_summary_hex_u64(runtime_summary, pointer),
            "monotonic",
        );
    }

    for (pointer, stat_path, unit) in [
        ("/records", "sim.cpu0.o3.event_window.records", "Count"),
        ("/span_ticks", "sim.cpu0.o3.event_window.span_ticks", "Tick"),
        ("/first/tick", "sim.cpu0.o3.event_window.first.tick", "Tick"),
        (
            "/last/sequence",
            "sim.cpu0.o3.event_window.last.sequence",
            "Count",
        ),
        (
            "/max_rob_occupancy/rob_occupancy",
            "sim.cpu0.o3.event_window.max_rob_occupancy.rob_occupancy",
            "Count",
        ),
        (
            "/max_lsq_occupancy/lsq_occupancy",
            "sim.cpu0.o3.event_window.max_lsq_occupancy.lsq_occupancy",
            "Count",
        ),
        (
            "/max_rename_map_entries/rename_map_entries",
            "sim.cpu0.o3.event_window.max_rename_map_entries.rename_map_entries",
            "Count",
        ),
        (
            "/max_structural_pressure/lsq_occupancy",
            "sim.cpu0.o3.event_window.max_structural_pressure.lsq_occupancy",
            "Count",
        ),
        (
            "/max_fu_latency/fu_latency_cycles",
            "sim.cpu0.o3.event_window.max_fu_latency.fu_latency_cycles",
            "Cycle",
        ),
        (
            "/max_lsq_data_latency/lsq_data_latency_ticks",
            "sim.cpu0.o3.event_window.max_lsq_data_latency.lsq_data_latency_ticks",
            "Tick",
        ),
    ] {
        let expected = runtime_window
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("runtime event window should expose u64 lane {pointer}: {runtime_window}")
            });
        assert_json_stat(&json, stat_path, unit, expected, "monotonic");
    }
    for (pointer, stat_path) in [
        ("/first/pc", "sim.cpu0.o3.event_window.first.pc"),
        ("/last/pc", "sim.cpu0.o3.event_window.last.pc"),
        (
            "/max_rob_occupancy/pc",
            "sim.cpu0.o3.event_window.max_rob_occupancy.pc",
        ),
        (
            "/max_lsq_occupancy/pc",
            "sim.cpu0.o3.event_window.max_lsq_occupancy.pc",
        ),
        (
            "/max_rename_map_entries/pc",
            "sim.cpu0.o3.event_window.max_rename_map_entries.pc",
        ),
        (
            "/max_structural_pressure/pc",
            "sim.cpu0.o3.event_window.max_structural_pressure.pc",
        ),
        (
            "/max_fu_latency/pc",
            "sim.cpu0.o3.event_window.max_fu_latency.pc",
        ),
        (
            "/max_lsq_data_latency/pc",
            "sim.cpu0.o3.event_window.max_lsq_data_latency.pc",
        ),
    ] {
        assert_json_stat(
            &json,
            stat_path,
            "Address",
            event_summary_hex_u64(runtime_window, pointer),
            "monotonic",
        );
    }

    for debug_prefix in [
        "sim.debug.o3_trace.event_window",
        "sim.debug.o3_trace.cpu.cpu0.event_window",
    ] {
        for (pointer, stat_tail, unit) in [
            ("/records", "records", "Count"),
            ("/span_ticks", "span_ticks", "Tick"),
            ("/first/tick", "first.tick", "Tick"),
            ("/last/sequence", "last.sequence", "Count"),
            (
                "/max_rob_occupancy/rob_occupancy",
                "max_rob_occupancy.rob_occupancy",
                "Count",
            ),
            (
                "/max_lsq_occupancy/lsq_occupancy",
                "max_lsq_occupancy.lsq_occupancy",
                "Count",
            ),
            (
                "/max_rename_map_entries/rename_map_entries",
                "max_rename_map_entries.rename_map_entries",
                "Count",
            ),
            (
                "/max_structural_pressure/rob_occupancy",
                "max_structural_pressure.rob_occupancy",
                "Count",
            ),
            (
                "/max_fu_latency/fu_latency_cycles",
                "max_fu_latency.fu_latency_cycles",
                "Cycle",
            ),
            (
                "/max_lsq_data_latency/lsq_data_latency_ticks",
                "max_lsq_data_latency.lsq_data_latency_ticks",
                "Tick",
            ),
        ] {
            let expected = runtime_window
                .pointer(pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!(
                        "runtime event window should expose u64 lane {pointer}: {runtime_window}"
                    )
                });
            assert_json_stat(
                &json,
                &format!("{debug_prefix}.{stat_tail}"),
                unit,
                expected,
                "monotonic",
            );
        }
        assert_event_window_timing_stat_rows(&json, runtime_window, "", debug_prefix);

        for (pointer, stat_tail) in [
            ("/first/pc", "first.pc"),
            ("/last/pc", "last.pc"),
            ("/max_rob_occupancy/pc", "max_rob_occupancy.pc"),
            ("/max_lsq_occupancy/pc", "max_lsq_occupancy.pc"),
            ("/max_rename_map_entries/pc", "max_rename_map_entries.pc"),
            ("/max_structural_pressure/pc", "max_structural_pressure.pc"),
            ("/max_fu_latency/pc", "max_fu_latency.pc"),
            ("/max_lsq_data_latency/pc", "max_lsq_data_latency.pc"),
        ] {
            assert_json_stat(
                &json,
                &format!("{debug_prefix}.{stat_tail}"),
                "Address",
                event_summary_hex_u64(runtime_window, pointer),
                "monotonic",
            );
        }
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
    let runtime_repair = o3_runtime.pointer("/branch_repair").unwrap_or_else(|| {
        panic!("O3 runtime JSON should expose branch repair matrix: {o3_runtime}")
    });
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
        "/branch_event/taken",
        "/branch_event/not_taken",
        "/branch_event/predicted_taken",
        "/branch_event/predicted_not_taken",
        "/branch_event/predicted_targets",
        "/branch_event/predicted_target_matches",
        "/branch_event/predicted_target_mismatches",
        "/branch_event/resolved_targets",
        "/branch_event/mispredictions",
        "/branch_event/link_writes",
        "/branch_event/without_link_writes",
        "/branch_event/taken_kind/direct_conditional",
        "/branch_event/not_taken_kind/direct_unconditional",
        "/branch_event/predicted_taken_kind/direct_conditional",
        "/branch_event/predicted_not_taken_kind/direct_unconditional",
        "/branch_event/predicted_target_kind/direct_conditional",
        "/branch_event/predicted_target_match_kind/direct_conditional",
        "/branch_event/predicted_target_mismatch_kind/direct_conditional",
        "/branch_event/resolved_target_kind/direct_unconditional",
        "/branch_event/misprediction_kind/direct_conditional",
        "/branch_event/link_write_kind/call_indirect",
        "/branch_event/without_link_write_kind/direct_unconditional",
    ] {
        assert_eq!(
            runtime_summary.pointer(pointer),
            debug_summary.pointer(pointer),
            "runtime event-summary branch lane {pointer} should mirror debug trace event summary"
        );
    }

    for pointer in [
        "/branch_event/taken",
        "/branch_event/not_taken",
        "/branch_event/predicted_taken",
        "/branch_event/predicted_not_taken",
        "/branch_event/predicted_targets",
        "/branch_event/predicted_target_mismatches",
        "/branch_event/resolved_targets",
        "/branch_event/mispredictions",
        "/branch_event/without_link_writes",
        "/branch_event/taken_kind/direct_unconditional",
        "/branch_event/not_taken_kind/direct_conditional",
        "/branch_event/predicted_taken_kind/direct_conditional",
        "/branch_event/predicted_not_taken_kind/direct_unconditional",
        "/branch_event/predicted_target_kind/direct_conditional",
        "/branch_event/predicted_target_mismatch_kind/direct_conditional",
        "/branch_event/resolved_target_kind/direct_unconditional",
        "/branch_event/misprediction_kind/direct_conditional",
        "/branch_event/without_link_write_kind/direct_unconditional",
        "/iew/predicted_taken_incorrect",
        "/iew/predicted_not_taken_incorrect",
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
            "/mismatches",
            "sim.cpu0.o3.branch_direction_mismatch.mismatches",
        ),
        (
            "/without_link_writes",
            "sim.cpu0.o3.branch_direction_mismatch.without_link_writes",
        ),
        (
            "/squashed_targets",
            "sim.cpu0.o3.branch_direction_mismatch.squashed_targets",
        ),
        (
            "/kind/direct_unconditional",
            "sim.cpu0.o3.branch_direction_mismatch.kind.direct_unconditional",
        ),
        (
            "/squashed_target_without_link_write_kind/direct_unconditional",
            "sim.cpu0.o3.branch_direction_mismatch.squashed_target_without_link_write_kind.direct_unconditional",
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
            "/targetless_mismatches",
            "sim.cpu0.o3.branch_target_mismatch.targetless_mismatches",
        ),
        (
            "/targetless_mismatch_without_link_writes",
            "sim.cpu0.o3.branch_target_mismatch.targetless_mismatch_without_link_writes",
        ),
        (
            "/targetless_mismatch_squashed_targets",
            "sim.cpu0.o3.branch_target_mismatch.targetless_mismatch_squashed_targets",
        ),
        (
            "/targetless_mismatch_kind/direct_conditional",
            "sim.cpu0.o3.branch_target_mismatch.targetless_mismatch_kind.direct_conditional",
        ),
        (
            "/targetless_mismatch_squashed_target_without_link_write_kind/direct_conditional",
            "sim.cpu0.o3.branch_target_mismatch.targetless_mismatch_squashed_target_without_link_write_kind.direct_conditional",
        ),
        (
            "/wrong_targets",
            "sim.cpu0.o3.branch_target_mismatch.wrong_targets",
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
            "/targetless_mismatches",
            "sim.cpu0.o3.branch_repair.targetless_mismatches",
        ),
        ("/wrong_targets", "sim.cpu0.o3.branch_repair.wrong_targets"),
        (
            "/direction_only_mismatches",
            "sim.cpu0.o3.branch_repair.direction_only_mismatches",
        ),
        (
            "/targetless_mismatch_kind/direct_conditional",
            "sim.cpu0.o3.branch_repair.targetless_mismatch_kind.direct_conditional",
        ),
        (
            "/wrong_target_kind/call_indirect",
            "sim.cpu0.o3.branch_repair.wrong_target_kind.call_indirect",
        ),
        (
            "/direction_only_kind/direct_unconditional",
            "sim.cpu0.o3.branch_repair.direction_only_kind.direct_unconditional",
        ),
    ] {
        let expected = runtime_repair
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("runtime branch repair should expose u64 lane {pointer}: {runtime_repair}")
            });
        assert_json_stat(&json, stat_path, "Count", expected, "monotonic");
    }

    for (pointer, stat_path) in [
        (
            "/branch_event/taken",
            "sim.cpu0.o3.event_summary.branch_event.taken",
        ),
        (
            "/branch_event/not_taken",
            "sim.cpu0.o3.event_summary.branch_event.not_taken",
        ),
        (
            "/branch_event/predicted_taken",
            "sim.cpu0.o3.event_summary.branch_event.predicted_taken",
        ),
        (
            "/branch_event/predicted_not_taken",
            "sim.cpu0.o3.event_summary.branch_event.predicted_not_taken",
        ),
        (
            "/branch_event/predicted_targets",
            "sim.cpu0.o3.event_summary.branch_event.predicted_targets",
        ),
        (
            "/branch_event/predicted_target_matches",
            "sim.cpu0.o3.event_summary.branch_event.predicted_target_matches",
        ),
        (
            "/branch_event/predicted_target_mismatches",
            "sim.cpu0.o3.event_summary.branch_event.predicted_target_mismatches",
        ),
        (
            "/branch_event/resolved_targets",
            "sim.cpu0.o3.event_summary.branch_event.resolved_targets",
        ),
        (
            "/branch_event/mispredictions",
            "sim.cpu0.o3.event_summary.branch_event.mispredictions",
        ),
        (
            "/branch_event/link_writes",
            "sim.cpu0.o3.event_summary.branch_event.link_writes",
        ),
        (
            "/branch_event/without_link_writes",
            "sim.cpu0.o3.event_summary.branch_event.without_link_writes",
        ),
        (
            "/branch_event/taken_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_event.taken_kind.direct_conditional",
        ),
        (
            "/branch_event/not_taken_kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_event.not_taken_kind.direct_unconditional",
        ),
        (
            "/branch_event/predicted_taken_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_event.predicted_taken_kind.direct_conditional",
        ),
        (
            "/branch_event/predicted_not_taken_kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_event.predicted_not_taken_kind.direct_unconditional",
        ),
        (
            "/branch_event/predicted_target_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_event.predicted_target_kind.direct_conditional",
        ),
        (
            "/branch_event/predicted_target_match_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_event.predicted_target_match_kind.direct_conditional",
        ),
        (
            "/branch_event/predicted_target_mismatch_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_event.predicted_target_mismatch_kind.direct_conditional",
        ),
        (
            "/branch_event/resolved_target_kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_event.resolved_target_kind.direct_unconditional",
        ),
        (
            "/branch_event/misprediction_kind/direct_conditional",
            "sim.cpu0.o3.event_summary.branch_event.misprediction_kind.direct_conditional",
        ),
        (
            "/branch_event/link_write_kind/call_indirect",
            "sim.cpu0.o3.event_summary.branch_event.link_write_kind.call_indirect",
        ),
        (
            "/branch_event/without_link_write_kind/direct_unconditional",
            "sim.cpu0.o3.event_summary.branch_event.without_link_write_kind.direct_unconditional",
        ),
        (
            "/iew/predicted_taken_incorrect",
            "sim.cpu0.o3.event_summary.iew.predicted_taken_incorrect",
        ),
        (
            "/iew/predicted_not_taken_incorrect",
            "sim.cpu0.o3.event_summary.iew.predicted_not_taken_incorrect",
        ),
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
        "sim.cpu0.o3.event_summary.span_ticks",
        "sim.cpu0.o3.event_window.records",
        "sim.cpu0.o3.event_window.span_ticks",
        "sim.cpu0.o3.event_window.max_rob_occupancy.tick",
        "sim.cpu0.o3.event_window.max_lsq_occupancy.pc",
        "sim.cpu0.o3.event_window.max_fu_latency.fu_latency_cycles",
        "sim.cpu0.o3.event_window.max_lsq_data_latency.lsq_data_latency_ticks",
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

#[test]
fn rem6_run_o3_runtime_json_exposes_lsq_operation_byte_aliases() {
    let path = detailed_o3_float_vector_lsq_binary(
        "m5-switch-cpu-detailed-o3-runtime-lsq-operation-byte-aliases",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
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
    let operations = json
        .pointer("/cores/0/o3_runtime/lsq/operation")
        .unwrap_or_else(|| panic!("run JSON should include O3 LSQ operations: {json}"));

    for (operation, alias, active_lane, inactive_lane) in [
        ("float_load", "floatLoad", "load_bytes", "store_bytes"),
        ("float_store", "floatStore", "store_bytes", "load_bytes"),
        ("vector_load", "vectorLoad", "load_bytes", "store_bytes"),
        ("vector_store", "vectorStore", "store_bytes", "load_bytes"),
    ] {
        for (lane, alias_lane) in [("load_bytes", "loadBytes"), ("store_bytes", "storeBytes")] {
            let expected = operations
                .pointer(&format!("/{operation}/{lane}"))
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!("runtime LSQ operation byte lane missing for {operation}/{lane}: {operations}")
                });
            assert_json_stat(
                &json,
                &format!("system.cpu.lsq0.operation.{alias}.{alias_lane}"),
                "Byte",
                expected,
                "monotonic",
            );
        }
        assert!(
            operations
                .pointer(&format!("/{operation}/{active_lane}"))
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "active runtime LSQ byte lane should be positive for {operation}: {operations}"
        );
        assert_eq!(
            operations
                .pointer(&format!("/{operation}/{inactive_lane}"))
                .and_then(Value::as_u64),
            Some(0),
            "inactive runtime LSQ byte lane should stay zero for {operation}: {operations}"
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_lsq_operation_store_conditional_failure_aliases() {
    let path = detailed_o3_store_conditional_failure_binary(
        "m5-switch-cpu-detailed-o3-runtime-lsq-store-conditional-failure-aliases",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
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
    let operations = json
        .pointer("/cores/0/o3_runtime/lsq/operation")
        .unwrap_or_else(|| panic!("run JSON should include O3 LSQ operations: {json}"));

    for (operation, alias, expected) in [
        ("store_conditional", "storeConditional", 1),
        ("store", "store", 0),
        ("load", "load", 0),
        ("atomic", "atomic", 0),
    ] {
        assert_eq!(
            operations
                .pointer(&format!("/{operation}/store_conditional_failures"))
                .and_then(Value::as_u64),
            Some(expected),
            "runtime LSQ operation failed-SC lane should match {operation}: {operations}"
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.lsq0.operation.{alias}.storeConditionalFailures"),
            "Count",
            expected,
            "monotonic",
        );
    }
}
