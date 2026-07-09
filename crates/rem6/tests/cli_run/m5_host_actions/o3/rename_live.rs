use super::*;

#[test]
fn rem6_run_o3_detailed_mode_exposes_live_rename_map_pressure() {
    let path = detailed_o3_live_rename_pressure_binary("m5-switch-cpu-o3-live-rename-pressure");

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
        Some("1000000000000000"),
        "O3 live-rename-pressure run should preserve the last renamed producer result"
    );

    let rename_entries = json_stat_u64(&json, "sim.cpu0.o3.rename_map_entries");
    assert!(
        rename_entries >= 8,
        "expected a live rename map with distinct architectural destinations: {json}"
    );
    assert_json_stat_at_least(&json, "sim.cpu0.o3.rename_writes", "Count", 9, "monotonic");
    assert_json_stat(
        &json,
        "system.cpu.rename.mapEntries",
        "Count",
        rename_entries,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "system.cpu.rename.renamedOperands",
        "Count",
        9,
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
        "ROB should drain after the rename-pressure work commits: {o3_runtime}"
    );
    assert_eq!(
        o3_runtime
            .pointer("/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0),
        "LSQ should drain after the rename-pressure result store commits: {o3_runtime}"
    );
    assert_eq!(
        o3_runtime
            .pointer("/snapshot/rename_map/count")
            .and_then(Value::as_u64),
        Some(rename_entries),
        "rename map should retain final physical mappings after commit: {o3_runtime}"
    );
    assert_eq!(
        o3_runtime
            .pointer("/rename/map_entries")
            .and_then(Value::as_u64),
        Some(rename_entries),
        "O3 runtime JSON should expose live rename-map pressure: {o3_runtime}"
    );
    let max_rename_event = o3_runtime
        .pointer("/event_window/max_rename_map_entries")
        .unwrap_or_else(|| {
            panic!("O3 runtime event window should expose max rename-map row: {o3_runtime}")
        });
    assert_eq!(
        max_rename_event
            .pointer("/rename_map_entries")
            .and_then(Value::as_u64),
        Some(rename_entries),
        "event window should identify the live rename pressure row: {max_rename_event}"
    );
    assert!(
        max_rename_event
            .pointer("/rob_occupancy")
            .and_then(Value::as_u64)
            .is_some_and(|occupancy| occupancy >= 1),
        "rename pressure row should still be tied to real O3 ROB residency: {max_rename_event}"
    );
}

#[test]
fn rem6_run_o3_detailed_mode_dumps_live_rename_pressure_before_exit() {
    let path =
        detailed_o3_live_rename_dump_stats_binary("m5-switch-cpu-o3-live-rename-dump-before-exit");

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
        Some("1000000000000000"),
        "O3 live-rename dump fixture should preserve the renamed producer result"
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
    let dump_sequence = events
        .iter()
        .find(|event| {
            event.pointer("/pc").and_then(Value::as_str) == Some("0x8000002c")
                && event.pointer("/system_event").and_then(Value::as_bool) == Some(true)
        })
        .map(|event| rename_event_u64(event, "/sequence"))
        .unwrap_or_else(|| panic!("missing detailed-mode m5_dump_stats event: {events:?}"));
    let rename_entries_before_dump = events
        .iter()
        .filter(|event| rename_event_u64(event, "/sequence") <= dump_sequence)
        .map(|event| rename_event_u64(event, "/rename_map_entries"));
    let expected_rename_entries = rename_entries_before_dump.clone().max().unwrap_or(0);
    let summed_rename_entries = rename_entries_before_dump.sum::<u64>();
    assert!(
        summed_rename_entries > expected_rename_entries,
        "fixture should distinguish max-live rename-map entries from a summed implementation: {events:?}"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "O3 detailed rename dump should be delivered before m5_exit: {host_actions}"
    );
    assert_eq!(
        host_actions.pointer("/stop_count").and_then(Value::as_u64),
        Some(1),
        "fixture should still stop through the later m5_exit: {host_actions}"
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing O3 live rename stats dump action: {host_actions}"));
    assert_stats_dump_sample_at_least(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.rename.writes",
        "counter",
        "Count",
        9,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.rename.map_entries",
        "counter",
        "Count",
        expected_rename_entries,
        "resettable",
    );
}

fn rename_event_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 field {pointer}: {json}"))
}
