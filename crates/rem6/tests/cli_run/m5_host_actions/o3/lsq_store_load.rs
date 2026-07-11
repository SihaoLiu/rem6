use std::collections::BTreeSet;

use super::*;

const OLDER_STORE_PC: &str = "0x80000010";
const YOUNGER_LOAD_PC: &str = "0x80000014";
const DATA_ADDRESS: &str = "0x80000100";
const DISJOINT_LOAD_ADDRESS: &str = "0x80000140";
const DISJOINT_RESULTS: &str = "2a000000630000006400000000000000";
const ALIAS_RESULTS: &str = "2a0000002a0000002b00000000000000";

#[test]
fn rem6_run_o3_detailed_store_then_disjoint_load_overlap_direct() {
    let path = store_load_binary("o3-store-load-direct", 64);
    let json = store_load_json(&path, "direct", 900, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_completed_disjoint_store_load(&json);
}

#[test]
fn rem6_run_o3_detailed_store_then_disjoint_load_overlap_cache_fabric_dram() {
    let path = store_load_binary("o3-store-load-cache-fabric-dram", 64);
    let json = store_load_json(&path, "cache-fabric-dram", 1400, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_completed_disjoint_store_load(&json);
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
            "hierarchy-backed store-load run should expose {pointer}: {json}"
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
fn rem6_run_o3_detailed_store_and_disjoint_load_remain_resident_together() {
    let path = store_load_binary("o3-store-load-resident", 64);
    let completed = store_load_json(&path, "direct", 900, None);
    let older = event_at_pc(&completed, OLDER_STORE_PC);
    let younger = event_at_pc(&completed, YOUNGER_LOAD_PC);
    let younger_issue_tick = event_u64(younger, "issue_tick");
    let first_response_tick = event_u64(older, "lsq_data_response_tick")
        .min(event_u64(younger, "lsq_data_response_tick"));
    let stop_tick = younger_issue_tick
        .saturating_add(first_response_tick.saturating_sub(younger_issue_tick) / 2);
    assert!(younger_issue_tick < stop_tick && stop_tick < first_response_tick);

    let json = store_load_json(&path, "direct", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident store-load run should expose ROB rows: {json}"));
    for pc in [OLDER_STORE_PC, YOUNGER_LOAD_PC] {
        let entry = rob
            .iter()
            .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .unwrap_or_else(|| panic!("resident store-load ROB should include {pc}: {json}"));
        assert_eq!(
            entry.pointer("/ready").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            entry.pointer("/live_staged").and_then(Value::as_bool),
            Some(true)
        );
    }
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident store-load run should expose LSQ rows: {json}"));
    assert_eq!(lsq.len(), 2);
    assert_eq!(
        lsq.iter()
            .map(|entry| {
                (
                    entry.pointer("/kind").and_then(Value::as_str),
                    entry.pointer("/address").and_then(Value::as_str),
                    entry.pointer("/completed").and_then(Value::as_bool),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (Some("store"), Some(DATA_ADDRESS), Some(false)),
            (Some("load"), Some(DISJOINT_LOAD_ADDRESS), Some(false)),
        ]
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_timing_store_load_preserves_architecture_without_o3_window() {
    let path = store_load_binary("o3-store-load-timing", 0);
    let json = store_load_json(&path, "direct", 900, Some("timing"));

    assert_final_architecture(&json, ALIAS_RESULTS, "0x2a", "0x2b");
    assert_eq!(data_memory_request_count(&json), 4);
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
        "timing mode should suppress store-load O3 aliases: {unexpected:?}"
    );
}

#[test]
fn rem6_run_o3_detailed_aliasing_store_load_forwards_without_second_memory_request_direct() {
    let path = store_load_binary("o3-store-load-alias", 0);
    let json = store_load_json(&path, "direct", 900, None);

    assert_forwarded_alias_store_load(&json);
}

#[test]
fn rem6_run_o3_detailed_aliasing_store_load_forwards_without_second_memory_request_cache_fabric_dram(
) {
    let path = store_load_binary("o3-store-load-alias-cache-fabric-dram", 0);
    let json = store_load_json(&path, "cache-fabric-dram", 1400, None);

    assert_forwarded_alias_store_load(&json);
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
            "hierarchy-backed forwarding run should expose {pointer}: {json}"
        );
    }
}

#[test]
fn rem6_run_o3_detailed_store_load_width_mismatch_remains_serialized() {
    let path = store_load_width_mismatch_binary("o3-store-load-width-mismatch");
    let json = store_load_json(&path, "direct", 900, None);

    assert_final_architecture(&json, ALIAS_RESULTS, "0x2a", "0x2b");
    let older = event_at_pc(&json, OLDER_STORE_PC);
    let younger = event_at_pc(&json, YOUNGER_LOAD_PC);
    assert!(
        event_u64(younger, "issue_tick") >= event_u64(older, "lsq_data_response_tick"),
        "width-mismatched load must wait for the store response: older={older}, younger={younger}"
    );
    assert_eq!(
        younger
            .pointer("/store_load_forwarding_suppressed")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        younger
            .pointer("/store_load_forwarding_byte_mismatch")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
        "Count",
        1,
        "monotonic",
    );
    assert_eq!(data_memory_request_count(&json), 4);
}

fn assert_forwarded_alias_store_load(json: &Value) {
    assert_final_architecture(json, ALIAS_RESULTS, "0x2a", "0x2b");
    let older = event_at_pc(&json, OLDER_STORE_PC);
    let younger = event_at_pc(&json, YOUNGER_LOAD_PC);
    assert!(
        event_u64(younger, "issue_tick") < event_u64(older, "lsq_data_response_tick"),
        "forwarded load should issue before the older store response: older={older}, younger={younger}"
    );
    assert!(
        event_u64(younger, "lsq_data_response_tick")
            < event_u64(older, "lsq_data_response_tick"),
        "forwarded load should complete locally before the older store response: older={older}, younger={younger}"
    );
    assert!(
        event_u64(older, "commit_tick") <= event_u64(younger, "commit_tick"),
        "forwarded pair must retire in program order: older={older}, younger={younger}"
    );
    assert_eq!(
        younger
            .pointer("/store_load_forwarding_candidate")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        younger
            .pointer("/store_load_forwarding_match")
            .and_then(Value::as_bool),
        Some(true)
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
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
        "Count",
        1,
        "monotonic",
    );
    assert_eq!(data_memory_request_count(json), 3);
    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("forwarding run should expose Data trace: {json}"));
    assert_eq!(data_trace.len(), 4);
    let observed = data_trace
        .iter()
        .map(|record| {
            (
                record
                    .pointer("/kind")
                    .and_then(Value::as_str)
                    .expect("data trace kind"),
                record
                    .pointer("/address")
                    .and_then(Value::as_str)
                    .expect("data trace address"),
                record
                    .pointer("/size")
                    .and_then(Value::as_u64)
                    .expect("data trace size"),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed,
        BTreeSet::from([
            ("load", DATA_ADDRESS, 4),
            ("store", DATA_ADDRESS, 4),
            ("store", "0x80000104", 4),
            ("store", "0x80000108", 4),
        ])
    );
}

fn assert_completed_disjoint_store_load(json: &Value) {
    assert_final_architecture(json, DISJOINT_RESULTS, "0x63", "0x64");
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

    let older = event_at_pc(json, OLDER_STORE_PC);
    let younger = event_at_pc(json, YOUNGER_LOAD_PC);
    assert_eq!(
        older.pointer("/lsq_operation").and_then(Value::as_str),
        Some("store")
    );
    assert_eq!(
        younger.pointer("/lsq_operation").and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        older.pointer("/lsq_store_address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        younger.pointer("/lsq_load_address").and_then(Value::as_str),
        Some(DISJOINT_LOAD_ADDRESS)
    );
    assert!(
        event_u64(younger, "issue_tick") < event_u64(older, "lsq_data_response_tick"),
        "younger disjoint load should issue before the older store response: older={older}, younger={younger}"
    );
    assert!(
        event_u64(older, "commit_tick") <= event_u64(younger, "commit_tick"),
        "store-load pair must retire in program order: older={older}, younger={younger}"
    );
}

fn assert_final_architecture(json: &Value, results: &str, load: &str, dependent: &str) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(results)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("63000000")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some(load)
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x13")
            .and_then(Value::as_str),
        Some(dependent)
    );
}

fn store_load_json(
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
        "O3,Data,Memory",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--dump-memory",
        "0x80000100:16",
        "--dump-memory",
        "0x80000140:4",
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

fn data_memory_request_count(json: &Value) -> usize {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("run JSON Memory trace")
        .iter()
        .filter(|record| record.pointer("/channel").and_then(Value::as_str) == Some("data"))
        .map(|record| {
            (
                record
                    .pointer("/request_agent")
                    .and_then(Value::as_u64)
                    .expect("memory trace request agent"),
                record
                    .pointer("/request")
                    .and_then(Value::as_u64)
                    .expect("memory trace request sequence"),
            )
        })
        .collect::<BTreeSet<_>>()
        .len()
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

fn store_load_binary(name: &str, load_offset: i32) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x2a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 10, 0b010),
        i_type(load_offset, 10, 0b010, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(4, 12, 10, 0b010),
        s_type(8, 13, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0; 16]);
    words.push(0x63);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn store_load_width_mismatch_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x2a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 10, 0b010),
        i_type(0, 10, 0b100, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(4, 12, 10, 0b010),
        s_type(8, 13, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0; 16]);
    words.push(0x63);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
