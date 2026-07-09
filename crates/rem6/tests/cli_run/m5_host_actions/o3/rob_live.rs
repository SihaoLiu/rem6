use super::*;

#[test]
fn rem6_run_o3_detailed_mode_exposes_live_rob_overlap() {
    let path = detailed_o3_live_rob_overlap_binary("m5-switch-cpu-o3-live-rob-overlap");

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
            "--debug-flags",
            "O3",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000060:8",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a00000010000000"),
        "O3 live-ROB overlap run should preserve ordered multiply and younger integer results"
    );

    let instructions = json_stat_u64(&json, "sim.cpu0.o3.instructions");
    assert!(instructions >= 8, "expected detailed O3 work: {json}");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rob_allocations",
        "Count",
        instructions,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rob_commits",
        "Count",
        instructions,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.event_summary.rob.commit_blocked_events",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu0.o3.event_summary.rob.max_commits_at_tick",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "system.cpu.rob.maxOccupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu0.o3.fu_integer_mul_latency_cycles",
        "Cycle",
        1,
        "monotonic",
    );

    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime
            .pointer("/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0),
        "ROB should drain after ordered commit: {o3_runtime}"
    );
    assert!(
        o3_runtime
            .pointer("/rob/max_occupancy")
            .and_then(Value::as_u64)
            .is_some_and(|occupancy| occupancy >= 2),
        "O3 runtime JSON should expose live ROB overlap: {o3_runtime}"
    );
    assert_eq!(
        o3_runtime
            .pointer("/event_summary/rob/commit_blocked_events")
            .and_then(Value::as_u64),
        Some(1),
        "O3 runtime event summary should count exactly the resident multiply as commit-blocked: {o3_runtime}"
    );
    assert!(
        o3_runtime
            .pointer("/event_summary/rob/max_commits_at_tick")
            .and_then(Value::as_u64)
            .is_some_and(|commits| commits >= 2),
        "O3 runtime event summary should expose the multi-commit drain at the writeback boundary: {o3_runtime}"
    );
    let max_rob_event = o3_runtime
        .pointer("/event_window/max_rob_occupancy")
        .unwrap_or_else(|| {
            panic!("O3 runtime event window should expose max ROB row: {o3_runtime}")
        });
    assert!(
        max_rob_event
            .pointer("/rob_occupancy")
            .and_then(Value::as_u64)
            .is_some_and(|occupancy| occupancy >= 2),
        "event window should identify the live ROB overlap row: {max_rob_event}"
    );
    assert_eq!(
        max_rob_event.pointer("/pc").and_then(Value::as_str),
        Some("0x80000010"),
        "max ROB occupancy should occur when younger independent integer work overlaps the resident multiply: {max_rob_event}"
    );
    let max_rob_phase_deltas = assert_o3_phase_deltas(max_rob_event);
    let max_fu_latency_event = o3_runtime
        .pointer("/event_window/max_fu_latency")
        .unwrap_or_else(|| {
            panic!("O3 runtime event window should expose max FU-latency row: {o3_runtime}")
        });
    let max_fu_latency_phase_deltas = assert_o3_phase_deltas(max_fu_latency_event);
    assert!(
        max_fu_latency_phase_deltas.0 > 0,
        "max FU-latency row should expose a nonzero issue-to-writeback phase: {max_fu_latency_event}"
    );
    let debug_event_summary = json
        .pointer("/debug/o3_trace/0/event_summary")
        .unwrap_or_else(|| panic!("O3 debug trace should expose event summary JSON: {json}"));
    let debug_event_window = debug_event_summary
        .pointer("/event_window")
        .unwrap_or_else(|| {
            panic!("O3 debug event summary should expose event-window rows: {json}")
        });
    let debug_max_rob_event = debug_event_window
        .pointer("/max_rob_occupancy")
        .unwrap_or_else(|| {
            panic!("O3 debug event summary should expose max ROB row: {debug_event_window}")
        });
    let debug_max_fu_latency_event = debug_event_window
        .pointer("/max_fu_latency")
        .unwrap_or_else(|| {
            panic!("O3 debug event summary should expose max FU-latency row: {debug_event_window}")
        });
    assert_eq!(
        assert_o3_phase_deltas(debug_max_rob_event),
        max_rob_phase_deltas,
        "debug and runtime max ROB rows should expose matching phase deltas: runtime={max_rob_event}, debug={debug_max_rob_event}"
    );
    assert_eq!(
        assert_o3_phase_deltas(debug_max_fu_latency_event),
        max_fu_latency_phase_deltas,
        "debug and runtime max FU-latency rows should expose matching phase deltas: runtime={max_fu_latency_event}, debug={debug_max_fu_latency_event}"
    );
    assert_event_window_phase_stats(&json, "max_rob_occupancy", max_rob_phase_deltas);
    assert_event_window_phase_stats(&json, "max_fu_latency", max_fu_latency_phase_deltas);
    assert_debug_event_window_phase_stats(&json, "max_rob_occupancy", max_rob_phase_deltas);
    assert_debug_event_window_phase_stats(&json, "max_fu_latency", max_fu_latency_phase_deltas);
    let events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("O3 debug trace should expose per-event timing rows: {json}"));
    let multiply = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x8000000c"))
        .unwrap_or_else(|| panic!("missing resident multiply event: {events:?}"));
    let younger_add = events
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some("0x80000010"))
        .unwrap_or_else(|| panic!("missing younger independent add event: {events:?}"));
    let multiply_phase_deltas = assert_o3_phase_deltas(multiply);
    let younger_phase_deltas = assert_o3_phase_deltas(younger_add);
    assert!(
        multiply_phase_deltas.0 > 0,
        "resident multiply event should expose its nonzero issue-to-writeback phase: {multiply}"
    );
    assert_eq!(
        younger_phase_deltas,
        (0, 0, 0),
        "younger independent add should expose a zero-latency event phase tuple: {younger_add}"
    );
    let event_phase_totals = events.iter().fold((0, 0, 0), |accumulator, event| {
        let deltas = assert_o3_phase_deltas(event);
        (
            accumulator.0 + deltas.0,
            accumulator.1 + deltas.1,
            accumulator.2 + deltas.2,
        )
    });
    assert!(
        event_phase_totals.0 > 0,
        "O3 debug events should include nonzero aggregate issue-to-writeback time: {events:?}"
    );
    let runtime_event_summary = o3_runtime
        .pointer("/event_summary")
        .unwrap_or_else(|| panic!("O3 runtime JSON should expose event summary: {o3_runtime}"));
    assert_event_summary_phase_json(runtime_event_summary, event_phase_totals);
    assert_event_summary_phase_json(debug_event_summary, event_phase_totals);
    assert_event_phase_stat_prefix(&json, "sim.cpu0.o3.event_summary", event_phase_totals);
    assert_gem5_iew_phase_alias_stats(&json, event_phase_totals);
    assert_debug_event_phase_stats(&json, event_phase_totals);
    let multiply_issue = json_u64_field(multiply, "/issue_tick");
    let multiply_writeback = json_u64_field(multiply, "/writeback_tick");
    let multiply_commit = json_u64_field(multiply, "/commit_tick");
    let younger_issue = json_u64_field(younger_add, "/issue_tick");
    let multiply_rob_commits = json_u64_field(multiply, "/rob_commits_at_tick");
    let younger_rob_commits = json_u64_field(younger_add, "/rob_commits_at_tick");
    let multiply_commit_blocked = json_bool_field(multiply, "/rob_commit_blocked");
    let younger_commit_blocked = json_bool_field(younger_add, "/rob_commit_blocked");
    assert!(
        multiply_writeback > multiply_issue,
        "multiply should occupy the FU after issue: {multiply}"
    );
    assert!(
        younger_issue <= multiply_writeback,
        "younger independent work should be visible no later than the older multiply writeback boundary: multiply={multiply}, younger={younger_add}"
    );
    assert!(
        younger_add
            .pointer("/rob_occupancy")
            .and_then(Value::as_u64)
            .is_some_and(|occupancy| occupancy >= 2),
        "younger event should carry live ROB overlap at the writeback boundary: {younger_add}"
    );
    assert!(
        multiply_commit >= multiply_writeback,
        "multiply commit timing should not precede writeback: {multiply}"
    );
    assert_eq!(
        multiply_rob_commits, 0,
        "resident multiply should not commit while its FU latency is outstanding: {multiply}"
    );
    assert!(
        multiply_commit_blocked,
        "resident multiply should block the ROB head until writeback: {multiply}"
    );
    assert!(
        younger_rob_commits >= 2,
        "younger event should drain the older multiply and itself at the writeback boundary: {younger_add}"
    );
    assert!(
        !younger_commit_blocked,
        "ROB should no longer be commit-blocked when the younger event drains the resident multiply: {younger_add}"
    );
}

#[test]
fn rem6_run_o3_detailed_mode_delivers_live_rob_dump_before_exit() {
    let path = detailed_o3_live_rob_dump_stats_binary("m5-switch-cpu-o3-live-rob-dump-before-exit");

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
            "--debug-flags",
            "O3",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000040:8",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a00000010000000"),
        "O3 live-ROB dump fixture should preserve ordered multiply and younger integer stores"
    );

    let events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("O3 debug trace should expose per-event timing rows: {json}"));
    let system_event_count = events
        .iter()
        .filter(|event| event.pointer("/system_event").and_then(Value::as_bool) == Some(true))
        .count();
    assert!(
        system_event_count >= 2,
        "fixture should execute dump and exit system events in detailed mode: {events:?}"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "O3 detailed dump should be delivered before the later m5_exit stop: {host_actions}"
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1),
        "fixture should still stop through the later m5_exit: {host_actions}"
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing O3 live ROB stats dump action: {host_actions}"));
    let event_summary_records = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.rob.commits",
        "counter",
        "Count",
        event_summary_records,
        "resettable",
    );
    assert_stats_dump_sample_at_least(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.rob.max_occupancy",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.rob.commit_blocked_events",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample_at_least(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.rob.max_commits_at_tick",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_dump_gem5_iew_phase_alias_stats(dump);
}

fn json_u64_field(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 field {pointer}: {json}"))
}

fn assert_o3_phase_deltas(json: &Value) -> (u64, u64, u64) {
    let issue_tick = json_u64_field(json, "/issue_tick");
    let writeback_tick = json_u64_field(json, "/writeback_tick");
    let commit_tick = json_u64_field(json, "/commit_tick");
    let issue_to_writeback_ticks = json_u64_field(json, "/issue_to_writeback_ticks");
    let writeback_to_commit_ticks = json_u64_field(json, "/writeback_to_commit_ticks");
    let issue_to_commit_ticks = json_u64_field(json, "/issue_to_commit_ticks");
    assert_eq!(
        issue_to_writeback_ticks,
        writeback_tick.saturating_sub(issue_tick),
        "issue-to-writeback phase should match O3 timing ticks: {json}"
    );
    assert_eq!(
        writeback_to_commit_ticks,
        commit_tick.saturating_sub(writeback_tick),
        "writeback-to-commit phase should match O3 timing ticks: {json}"
    );
    assert_eq!(
        issue_to_commit_ticks,
        commit_tick.saturating_sub(issue_tick),
        "issue-to-commit phase should match O3 timing ticks: {json}"
    );
    (
        issue_to_writeback_ticks,
        writeback_to_commit_ticks,
        issue_to_commit_ticks,
    )
}

fn assert_event_window_phase_stats(json: &Value, row: &str, expected: (u64, u64, u64)) {
    let prefix = format!("sim.cpu0.o3.event_summary.event_window.{row}");
    assert_event_window_phase_stat_prefix(json, &prefix, expected);
}

fn assert_debug_event_window_phase_stats(json: &Value, row: &str, expected: (u64, u64, u64)) {
    for prefix in [
        format!("sim.debug.o3_trace.event_window.{row}"),
        format!("sim.debug.o3_trace.cpu.cpu0.event_window.{row}"),
    ] {
        assert_event_window_phase_stat_prefix(json, &prefix, expected);
    }
}

fn assert_debug_event_phase_stats(json: &Value, expected: (u64, u64, u64)) {
    for prefix in [
        "sim.debug.o3_trace.event",
        "sim.debug.o3_trace.cpu.cpu0.event",
    ] {
        assert_event_phase_stat_prefix(json, prefix, expected);
    }
}

fn assert_event_summary_phase_json(json: &Value, expected: (u64, u64, u64)) {
    assert_eq!(
        json_u64_field(json, "/issue_to_writeback_ticks"),
        expected.0,
        "event summary JSON should aggregate raw issue-to-writeback phases: {json}"
    );
    assert_eq!(
        json_u64_field(json, "/writeback_to_commit_ticks"),
        expected.1,
        "event summary JSON should aggregate raw writeback-to-commit phases: {json}"
    );
    assert_eq!(
        json_u64_field(json, "/issue_to_commit_ticks"),
        expected.2,
        "event summary JSON should aggregate raw issue-to-commit phases: {json}"
    );
}

fn assert_event_phase_stat_prefix(json: &Value, prefix: &str, expected: (u64, u64, u64)) {
    assert_json_stat(
        json,
        &format!("{prefix}.issue_to_writeback_ticks"),
        "Tick",
        expected.0,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{prefix}.writeback_to_commit_ticks"),
        "Tick",
        expected.1,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{prefix}.issue_to_commit_ticks"),
        "Tick",
        expected.2,
        "monotonic",
    );
}

fn assert_gem5_iew_phase_alias_stats(json: &Value, expected: (u64, u64, u64)) {
    assert_json_stat(
        json,
        "system.cpu.iew.issueToWritebackTicks",
        "Tick",
        expected.0,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.iew.writebackToCommitTicks",
        "Tick",
        expected.1,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.iew.issueToCommitTicks",
        "Tick",
        expected.2,
        "monotonic",
    );
}

fn assert_dump_gem5_iew_phase_alias_stats(dump: &Value) {
    let mut phase_totals = [0_u64; 3];
    for (index, source, alias) in [
        (
            0,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.issue_to_writeback_ticks",
            "system.cpu.iew.issueToWritebackTicks",
        ),
        (
            1,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.writeback_to_commit_ticks",
            "system.cpu.iew.writebackToCommitTicks",
        ),
        (
            2,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.issue_to_commit_ticks",
            "system.cpu.iew.issueToCommitTicks",
        ),
    ] {
        let value = stats_dump_sample_value(dump, source);
        phase_totals[index] = value;
        assert_stats_dump_sample(dump, alias, "counter", "Tick", value, "resettable");
    }
    assert!(
        phase_totals[0] > 0,
        "live-ROB dump fixture should expose nonzero issue-to-writeback timing: {dump}"
    );
    assert_eq!(
        phase_totals[2],
        phase_totals[0].saturating_add(phase_totals[1]),
        "dumped O3 phase totals should preserve issue-to-commit identity: {dump}"
    );
}

fn assert_event_window_phase_stat_prefix(json: &Value, prefix: &str, expected: (u64, u64, u64)) {
    assert_json_stat(
        json,
        &format!("{prefix}.issue_to_writeback_ticks"),
        "Tick",
        expected.0,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{prefix}.writeback_to_commit_ticks"),
        "Tick",
        expected.1,
        "monotonic",
    );
    assert_json_stat(
        json,
        &format!("{prefix}.issue_to_commit_ticks"),
        "Tick",
        expected.2,
        "monotonic",
    );
}

fn json_bool_field(json: &Value, pointer: &str) -> bool {
    json.pointer(pointer)
        .and_then(Value::as_bool)
        .unwrap_or_else(|| panic!("missing bool field {pointer}: {json}"))
}
