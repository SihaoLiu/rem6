use super::*;

#[test]
fn rem6_run_o3_detailed_div_live_retire_gate_delays_architectural_commit() {
    let path = live_retire_gate_div_witness_binary("m5-switch-cpu-o3-live-retire-div-gate");

    let timing = run_live_retire_gate_case(&path, "direct", "timing", 180, "stopped_by_host");
    let detailed_before_commit = run_live_retire_gate_case(
        &path,
        "direct",
        "detailed",
        timing.final_tick,
        "stopped_at_tick_limit",
    );
    let detailed = run_live_retire_gate_case(&path, "direct", "detailed", 220, "stopped_by_host");

    assert_eq!(
        timing.memory_hex, "01000000",
        "timing run should store the divide witness: {}",
        timing.json
    );
    assert_eq!(
        detailed.memory_hex, timing.memory_hex,
        "detailed run must preserve the architectural witness: {}",
        detailed.json
    );
    assert_eq!(
        timing.stop_code,
        Some(0),
        "timing run should stop through m5_exit: {}",
        timing.json
    );
    assert_eq!(
        detailed.stop_code,
        Some(0),
        "detailed run should stop through m5_exit: {}",
        detailed.json
    );
    assert_eq!(
        detailed_before_commit.memory_hex, "00000000",
        "detailed mode must not store the divide witness by the timing-mode stop tick: {}",
        detailed_before_commit.json
    );
    assert_eq!(
        detailed_before_commit.stop_code, None,
        "detailed mode must not reach m5_exit by the timing-mode stop tick: {}",
        detailed_before_commit.json
    );
    assert!(
        detailed_before_commit.committed_instructions < timing.committed_instructions,
        "detailed mode should retain the DIV before architectural commit at tick {}: timing={} detailed={}",
        timing.final_tick,
        timing.json,
        detailed_before_commit.json
    );
    assert!(
        detailed.final_tick > timing.final_tick,
        "detailed final_tick={} should exceed timing final_tick={} after the cycle-visible DIV gate\ntiming={}\ndetailed={}",
        detailed.final_tick,
        timing.final_tick,
        timing.json,
        detailed.json
    );

    assert_live_retire_window(&detailed_before_commit.json);
    assert_live_retire_gate_stats(&detailed_before_commit.json, 1, 19, 19);
    assert_live_retire_gate_stats(&detailed.json, 1, 19, 19);
    assert_live_retire_gate_stats_absent(&timing.json);
}

#[test]
fn rem6_run_o3_detailed_add_only_does_not_schedule_live_retire_gate() {
    let path = live_retire_gate_add_witness_binary("m5-switch-cpu-o3-live-retire-add-control");
    let detailed = run_live_retire_gate_case(&path, "direct", "detailed", 180, "stopped_by_host");

    assert_eq!(
        detailed.memory_hex, "19000000",
        "detailed add-only control should store the add witness: {}",
        detailed.json
    );
    assert_eq!(
        detailed.stop_code,
        Some(0),
        "detailed add-only control should stop through m5_exit: {}",
        detailed.json
    );
    assert_live_retire_gate_stats_absent(&detailed.json);
}

#[derive(Debug)]
struct LiveRetireGateRun {
    json: Value,
    final_tick: u64,
    stop_code: Option<u64>,
    memory_hex: String,
    committed_instructions: u64,
}

fn run_live_retire_gate_case(
    path: &Path,
    memory_system: &str,
    switch_mode: &str,
    max_tick: u64,
    expected_status: &str,
) -> LiveRetireGateRun {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            &max_tick.to_string(),
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            memory_system,
            "--m5-switch-cpu-mode",
            switch_mode,
            "--dump-memory",
            "0x80000060:4",
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
        Some(expected_status),
        "run should report the expected stop status: {json}"
    );
    let final_tick = live_retire_gate_json_u64_field(&json, "/simulation/final_tick");
    let stop_code = json
        .pointer("/simulation/stop_code")
        .and_then(Value::as_u64);
    let memory_hex = json
        .pointer("/memory/0/hex")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("run JSON should include witness memory dump: {json}"))
        .to_string();
    let committed_instructions =
        live_retire_gate_json_u64_field(&json, "/cores/0/committed_instructions");

    LiveRetireGateRun {
        json,
        final_tick,
        stop_code,
        memory_hex,
        committed_instructions,
    }
}

fn assert_live_retire_gate_stats(
    json: &Value,
    scheduled_waits: u64,
    total_wait_ticks: u64,
    max_wait_ticks: u64,
) {
    assert_json_stat(
        json,
        "sim.cpu0.o3.live_retire_gate.scheduled_waits",
        "Count",
        scheduled_waits,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.live_retire_gate.wait_ticks",
        "Cycle",
        total_wait_ticks,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.live_retire_gate.max_wait_ticks",
        "Cycle",
        max_wait_ticks,
        "monotonic",
    );

    let runtime = json
        .pointer("/cores/0/o3_runtime/live_retire_gate")
        .unwrap_or_else(|| panic!("detailed run should expose live retire-gate JSON: {json}"));
    assert_eq!(
        runtime.pointer("/scheduled_waits").and_then(Value::as_u64),
        Some(scheduled_waits),
        "runtime JSON should expose scheduled wait count: {runtime}"
    );
    assert_eq!(
        runtime.pointer("/wait_ticks").and_then(Value::as_u64),
        Some(total_wait_ticks),
        "runtime JSON should expose total wait ticks: {runtime}"
    );
    assert_eq!(
        runtime.pointer("/max_wait_ticks").and_then(Value::as_u64),
        Some(max_wait_ticks),
        "runtime JSON should expose max wait ticks: {runtime}"
    );
}

fn assert_live_retire_gate_stats_absent(json: &Value) {
    for path in [
        "sim.cpu0.o3.live_retire_gate.scheduled_waits",
        "sim.cpu0.o3.live_retire_gate.wait_ticks",
        "sim.cpu0.o3.live_retire_gate.max_wait_ticks",
    ] {
        assert_json_stat_absent(json, path);
    }
    assert!(
        json.pointer("/cores/0/o3_runtime/live_retire_gate")
            .is_none(),
        "inactive live retire-gate JSON should be absent: {json}"
    );
}

fn assert_live_retire_window(json: &Value) {
    let snapshot = json
        .pointer("/cores/0/o3_runtime/snapshot")
        .unwrap_or_else(|| panic!("detailed run should expose a live O3 snapshot: {json}"));
    let rob = snapshot
        .pointer("/rob")
        .unwrap_or_else(|| panic!("detailed run should expose a live ROB snapshot: {snapshot}"));
    assert_eq!(
        rob.pointer("/count").and_then(Value::as_u64),
        Some(2),
        "the gated DIV and its fetched successor should reside in the ROB before retirement: {rob}"
    );
    let entries = rob
        .pointer("/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("live ROB should expose ordered entries: {rob}"));
    for (entry, pc) in entries.iter().zip(["0x8000000c", "0x80000010"]) {
        assert_eq!(entry.pointer("/pc").and_then(Value::as_str), Some(pc));
        assert_eq!(
            entry.pointer("/ready").and_then(Value::as_bool),
            Some(false),
            "neither staged instruction may become architecturally ready before the gate wakes: {entry}"
        );
        assert_eq!(
            entry.pointer("/live_staged").and_then(Value::as_bool),
            Some(true),
            "pre-retirement ROB residency must be execution-owned rather than reconstructed from retired events: {entry}"
        );
        assert!(
            entry
                .pointer("/destination")
                .and_then(Value::as_u64)
                .is_some(),
            "both scalar instructions should own a physical destination while staged: {entry}"
        );
    }

    let rename_entries = snapshot
        .pointer("/rename_map/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("live snapshot should expose rename mappings: {snapshot}"));
    for architectural in [3, 4] {
        assert!(
            rename_entries.iter().any(|entry| {
                entry.pointer("/register_class").and_then(Value::as_str) == Some("integer")
                    && entry.pointer("/architectural").and_then(Value::as_u64)
                        == Some(architectural)
            }),
            "live rename map should include staged x{architectural}: {rename_entries:?}"
        );
    }
}

fn live_retire_gate_json_u64_field(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 field {pointer}: {json}"))
}
