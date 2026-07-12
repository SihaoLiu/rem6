use super::lsq_store_load::{data_memory_request_count, event_at_pc, event_u64};
use super::*;

const OLDER_STORE_PC: &str = "0x80000014";
const YOUNGER_STORE_PC: &str = "0x80000018";
const LOAD_PC: &str = "0x8000001c";
const DEPENDENT_ALU_PC: &str = "0x80000020";
const DATA_ADDRESS: &str = "0x80000100";
const RESULTS: &str = "63000000630000006400000000000000";

#[test]
fn rem6_run_o3_detailed_store_store_load_forwards_from_youngest_store_direct() {
    let path = store_store_load_binary("o3-store-store-load-direct");
    let json = store_store_load_json(&path, "direct", 1000, None);

    assert_completed_store_store_load(&json);
    assert_eq!(data_memory_request_count(&json), 4);
    assert!(json
        .pointer("/memory_resources/transport/data/activity")
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0));
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64).unwrap_or(0),
            0
        );
    }
}

#[test]
fn rem6_run_o3_detailed_store_store_load_forwards_from_youngest_store_cache_fabric_dram() {
    let path = store_store_load_binary("o3-store-store-load-cache-fabric-dram");
    let json = store_store_load_json(&path, "cache-fabric-dram", 1800, None);

    assert_completed_store_store_load(&json);
    assert_eq!(data_memory_request_count(&json), 4);
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
            "hierarchy-backed store-store-load run should expose {pointer}: {json}"
        );
    }
}

#[test]
fn rem6_run_o3_detailed_store_store_load_rows_are_resident_before_store_responses() {
    let path = store_store_load_binary("o3-store-store-load-resident");
    let completed = store_store_load_json(&path, "direct", 1000, None);
    let older = event_at_pc(&completed, OLDER_STORE_PC);
    let middle = event_at_pc(&completed, YOUNGER_STORE_PC);
    let load = event_at_pc(&completed, LOAD_PC);
    let window_ready_tick =
        event_u64(load, "lsq_data_response_tick").max(event_u64(middle, "issue_tick"));
    let first_store_response =
        event_u64(older, "lsq_data_response_tick").min(event_u64(middle, "lsq_data_response_tick"));
    let stop_tick = window_ready_tick
        .saturating_add(first_store_response.saturating_sub(window_ready_tick) / 2);
    assert!(window_ready_tick < stop_tick && stop_tick < first_store_response);

    let json = store_store_load_json(&path, "direct", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .expect("resident store-store-load ROB rows");
    for pc in [OLDER_STORE_PC, YOUNGER_STORE_PC, LOAD_PC] {
        assert!(
            rob.iter()
                .any(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc)),
            "resident ROB should include {pc}: {json}"
        );
    }
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .expect("resident store-store-load LSQ rows");
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
            (Some("store"), Some(DATA_ADDRESS), Some(false)),
            (Some("load"), Some(DATA_ADDRESS), Some(true)),
        ]
    );
}

#[test]
fn rem6_run_o3_timing_store_store_load_serializes_without_an_o3_window() {
    let path = store_store_load_binary("o3-store-store-load-timing");
    let json = store_store_load_json(&path, "direct", 1000, Some("timing"));

    assert_final_architecture(&json);
    assert_eq!(data_memory_request_count(&json), 5);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
}

fn assert_completed_store_store_load(json: &Value) {
    assert_final_architecture(json);
    let older = event_at_pc(json, OLDER_STORE_PC);
    let middle = event_at_pc(json, YOUNGER_STORE_PC);
    let load = event_at_pc(json, LOAD_PC);
    let dependent = event_at_pc(json, DEPENDENT_ALU_PC);
    let older_store_response = event_u64(older, "lsq_data_response_tick");
    let younger_store_response = event_u64(middle, "lsq_data_response_tick");
    let requests_before_older_response = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("store-store-load run should expose Memory trace")
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record
                    .pointer("/tick")
                    .and_then(Value::as_u64)
                    .is_some_and(|tick| tick < older_store_response)
        })
        .count();

    assert!(
        event_u64(middle, "issue_tick") < older_store_response,
        "younger store should occupy O3 before the leading response: older={older}, middle={middle}"
    );
    assert!(
        event_u64(load, "issue_tick") < older_store_response,
        "load should issue while both stores are resident: older={older}, middle={middle}, load={load}"
    );
    assert!(
        event_u64(load, "lsq_data_response_tick") < older_store_response,
        "forwarded load should complete before the leading store response: older={older}, middle={middle}, load={load}"
    );
    assert_eq!(
        requests_before_older_response, 1,
        "only the leading store may be transport-visible before its response"
    );
    assert!(
        older_store_response < younger_store_response,
        "the buffered younger store should drain only after the leading response: older={older}, middle={middle}"
    );
    assert!(event_u64(older, "commit_tick") <= event_u64(middle, "commit_tick"));
    assert!(event_u64(middle, "commit_tick") <= event_u64(load, "commit_tick"));
    assert!(event_u64(load, "commit_tick") <= event_u64(dependent, "commit_tick"));
    assert_eq!(
        load.pointer("/store_load_forwarding_candidate")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        load.pointer("/store_load_forwarding_match")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(event_u64(load, "store_load_forwarding_bytes"), 4);
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
}

fn assert_final_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(RESULTS)
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some("0x63")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x13")
            .and_then(Value::as_str),
        Some("0x64")
    );
}

fn store_store_load_json(
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
        "--riscv-o3-scalar-memory-depth",
        "3",
        "--dump-memory",
        "0x80000100:16",
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

fn store_store_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x2a, 0, 0x0, 11, 0x13),
        i_type(0x63, 0, 0x0, 14, 0x13),
        s_type(0, 11, 10, 0b010),
        s_type(0, 14, 10, 0b010),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(4, 12, 10, 0b010),
        s_type(8, 13, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0; 16]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
