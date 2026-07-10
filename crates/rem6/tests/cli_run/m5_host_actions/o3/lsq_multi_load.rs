use super::*;

const OLDER_LOAD_PC: &str = "0x8000000c";
const YOUNGER_LOAD_PC: &str = "0x80000010";
const DATA_ADDRESS: &str = "0x80000080";
const YOUNGER_DATA_ADDRESS: &str = "0x800000c0";
const EXPECTED_RESULTS_LOWER: &str = "2a000000000000002a00000063000000";
const EXPECTED_RESULTS_UPPER: &str = "2b00000065000000";

#[test]
fn rem6_run_o3_detailed_two_scalar_loads_overlap_before_first_response_direct() {
    let path = two_scalar_load_binary("o3-two-scalar-loads-direct");
    let json = two_scalar_load_json(&path, "direct", 900, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_completed_two_scalar_loads(&json);
}

#[test]
fn rem6_run_o3_detailed_two_scalar_loads_overlap_through_cache_fabric_dram() {
    let path = two_scalar_load_binary("o3-two-scalar-loads-cache-fabric-dram");
    let json = two_scalar_load_json(&path, "cache-fabric-dram", 1400, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_completed_two_scalar_loads(&json);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed two-load run should expose {pointer}: {json}"
        );
    }
    for path in [
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(&json, path, "Count", 1, "monotonic");
    }
}

#[test]
fn rem6_run_o3_detailed_two_scalar_loads_remain_resident_at_tick_limit() {
    let path = two_scalar_load_binary("o3-two-scalar-loads-resident");
    let completed = two_scalar_load_json(&path, "direct", 900, None);
    let older = event_at_pc(&completed, OLDER_LOAD_PC);
    let younger = event_at_pc(&completed, YOUNGER_LOAD_PC);
    let younger_issue_tick = event_u64(younger, "issue_tick");
    let older_response_tick = event_u64(older, "lsq_data_response_tick");
    let stop_tick = younger_issue_tick
        .saturating_add(older_response_tick.saturating_sub(younger_issue_tick) / 2);
    assert!(younger_issue_tick < stop_tick && stop_tick < older_response_tick);

    let json = two_scalar_load_json(&path, "direct", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        json.pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(stop_tick)
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident two-load run should expose ROB rows: {json}"));
    let resident_loads = [OLDER_LOAD_PC, YOUNGER_LOAD_PC]
        .into_iter()
        .map(|pc| {
            rob.iter()
                .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
                .unwrap_or_else(|| panic!("resident two-load ROB should include {pc}: {json}"))
        })
        .collect::<Vec<_>>();
    assert!(resident_loads.iter().all(|entry| {
        entry.pointer("/ready").and_then(Value::as_bool) == Some(false)
            && entry.pointer("/live_staged").and_then(Value::as_bool) == Some(true)
    }));
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident two-load run should expose LSQ rows: {json}"));
    assert_eq!(lsq.len(), 2);
    assert_eq!(
        lsq.iter()
            .filter_map(|entry| entry.pointer("/address").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec![DATA_ADDRESS, YOUNGER_DATA_ADDRESS]
    );
    assert!(lsq.iter().all(|entry| {
        entry.pointer("/kind").and_then(Value::as_str) == Some("load")
            && entry.pointer("/bytes").and_then(Value::as_u64) == Some(4)
            && entry.pointer("/completed").and_then(Value::as_bool) == Some(false)
    }));
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_timing_two_scalar_loads_preserve_architecture_without_o3_window() {
    let path = two_scalar_load_binary("o3-two-scalar-loads-timing");
    let json = two_scalar_load_json(&path, "direct", 900, Some("timing"));

    assert_final_architecture(&json);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("run JSON stats array")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "timing mode should suppress two-load O3 aliases: {unexpected:?}"
    );
}

fn assert_completed_two_scalar_loads(json: &Value) {
    assert_final_architecture(json);
    assert_json_stat_at_least(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        2,
        "monotonic",
    );

    let older = event_at_pc(json, OLDER_LOAD_PC);
    let younger = event_at_pc(json, YOUNGER_LOAD_PC);
    assert_eq!(
        older.pointer("/lsq_operation").and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        younger.pointer("/lsq_operation").and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        older.pointer("/lsq_load_address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        younger.pointer("/lsq_load_address").and_then(Value::as_str),
        Some(YOUNGER_DATA_ADDRESS)
    );
    assert!(
        event_u64(younger, "issue_tick") < event_u64(older, "lsq_data_response_tick"),
        "younger load should issue before the older response: older={older}, younger={younger}"
    );
    assert!(
        event_u64(older, "commit_tick") <= event_u64(younger, "commit_tick"),
        "two scalar loads must retire in program order: older={older}, younger={younger}"
    );
}

fn assert_final_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(EXPECTED_RESULTS_LOWER)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some(EXPECTED_RESULTS_UPPER)
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x63"),
        ("x14", "0x2b"),
        ("x15", "0x65"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should preserve two-load semantics: {json}"
        );
    }
}

fn two_scalar_load_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    switch_mode: Option<&str>,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
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
        "--debug-flags",
        "O3",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--dump-memory",
        "0x80000080:16",
        "--dump-memory",
        "0x80000090:8",
    ]);
    if let Some(switch_mode) = switch_mode {
        command.args(["--m5-switch-cpu-mode", switch_mode]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

fn two_scalar_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(64, 10, 0b010, 13, 0x03),
        i_type(1, 12, 0x0, 14, 0x13),
        i_type(2, 13, 0x0, 15, 0x13),
        s_type(8, 12, 10, 0b010),
        s_type(12, 13, 10, 0b010),
        s_type(16, 14, 10, 0b010),
        s_type(20, 15, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x2a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x63]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
