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
