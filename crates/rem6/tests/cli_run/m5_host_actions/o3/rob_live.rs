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
}
