use std::collections::{BTreeMap, BTreeSet};

use super::lsq_store_load::{data_memory_request_count, event_at_pc, event_u64};
use super::*;

const OLDER_STORE_PC: &str = "0x80000018";
const DISJOINT_STORE_PC: &str = "0x8000001c";
const OVERLAPPING_STORE_PC: &str = "0x80000020";
const LOAD_PC: &str = "0x80000024";
const DEPENDENT_ALU_PC: &str = "0x80000028";
const DATA_ADDRESS: &str = "0x80000100";
const DISJOINT_ADDRESS: &str = "0x80000118";
const FIRST_RESULTS: &str = "aa00bb0655667788aa00bb0655667788";
const SECOND_RESULTS: &str = "ab00bb06556677885a00000000000000";

#[test]
fn rem6_run_o3_detailed_disjoint_store_prefix_composition_direct() {
    let path = disjoint_store_prefix_binary("o3-disjoint-store-prefix-direct");
    let json = disjoint_store_prefix_json(&path, "direct", 1400, None);

    assert_completed_disjoint_store_prefix(&json);
    assert_eq!(data_memory_request_count(&json), 6);
    assert!(json
        .pointer("/memory_resources/transport/data/activity")
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0));
    assert_json_stat_at_least(
        &json,
        "sim.memory.resources.transport.data.activity",
        "Count",
        1,
        "monotonic",
    );
    for (pointer, path) in [
        (
            "/memory_resources/cache/data/activity",
            "sim.memory.resources.cache.data.activity",
        ),
        (
            "/memory_resources/fabric/activity",
            "sim.memory.resources.fabric.activity",
        ),
        (
            "/memory_resources/dram/activity",
            "sim.memory.resources.dram.activity",
        ),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64).unwrap_or(0),
            0
        );
        assert_json_stat(&json, path, "Count", 0, "monotonic");
    }
}

#[test]
fn rem6_run_o3_detailed_disjoint_store_prefix_composition_cache_fabric_dram() {
    let path = disjoint_store_prefix_binary("o3-disjoint-store-prefix-hierarchy");
    let json = disjoint_store_prefix_json(&path, "cache-fabric-dram", 2800, None);

    assert_completed_disjoint_store_prefix(&json);
    assert_eq!(data_memory_request_count(&json), 6);
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
            "hierarchy-backed disjoint store prefix should expose {pointer}: {json}"
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
fn rem6_run_o3_disjoint_store_prefix_rows_are_resident_before_responses() {
    let path = disjoint_store_prefix_binary("o3-disjoint-store-prefix-resident");
    let completed = disjoint_store_prefix_json(&path, "direct", 1400, None);
    let events = [
        event_at_pc(&completed, OLDER_STORE_PC),
        event_at_pc(&completed, DISJOINT_STORE_PC),
        event_at_pc(&completed, OVERLAPPING_STORE_PC),
        event_at_pc(&completed, LOAD_PC),
    ];
    let window_ready_tick = events
        .iter()
        .map(|event| event_u64(event, "issue_tick"))
        .max()
        .unwrap();
    let first_response_tick = events
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .unwrap();
    let stop_tick =
        window_ready_tick.saturating_add(first_response_tick.saturating_sub(window_ready_tick) / 2);
    assert!(window_ready_tick < stop_tick && stop_tick < first_response_tick);

    let json = disjoint_store_prefix_json(&path, "direct", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .expect("resident disjoint store-prefix ROB rows");
    assert_eq!(
        rob.iter()
            .map(|entry| {
                (
                    entry.pointer("/pc").and_then(Value::as_str),
                    entry.pointer("/ready").and_then(Value::as_bool),
                    entry.pointer("/live_staged").and_then(Value::as_bool),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (Some(OLDER_STORE_PC), Some(false), Some(true)),
            (Some(DISJOINT_STORE_PC), Some(false), Some(true)),
            (Some(OVERLAPPING_STORE_PC), Some(false), Some(true)),
            (Some(LOAD_PC), Some(false), Some(true)),
        ]
    );
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .expect("resident disjoint store-prefix LSQ rows");
    assert_eq!(
        lsq.iter()
            .map(|entry| {
                (
                    entry.pointer("/kind").and_then(Value::as_str),
                    entry.pointer("/address").and_then(Value::as_str),
                    entry.pointer("/bytes").and_then(Value::as_u64),
                    entry.pointer("/completed").and_then(Value::as_bool),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (Some("store"), Some(DATA_ADDRESS), Some(4), Some(false)),
            (Some("store"), Some(DISJOINT_ADDRESS), Some(4), Some(false),),
            (Some("store"), Some("0x80000102"), Some(2), Some(false),),
            (Some("load"), Some(DATA_ADDRESS), Some(8), Some(false)),
        ]
    );
}

#[test]
fn rem6_run_o3_timing_disjoint_store_prefix_uses_transport_without_o3_composition() {
    let path = disjoint_store_prefix_binary("o3-disjoint-store-prefix-timing");
    let json = disjoint_store_prefix_json(&path, "direct", 1400, Some("timing"));

    assert_final_architecture(&json);
    assert_eq!(data_memory_request_count(&json), 6);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
}

fn assert_completed_disjoint_store_prefix(json: &Value) {
    assert_final_architecture(json);
    let older = event_at_pc(json, OLDER_STORE_PC);
    let disjoint = event_at_pc(json, DISJOINT_STORE_PC);
    let overlapping = event_at_pc(json, OVERLAPPING_STORE_PC);
    let load = event_at_pc(json, LOAD_PC);
    let dependent = event_at_pc(json, DEPENDENT_ALU_PC);
    let older_response = event_u64(older, "lsq_data_response_tick");

    for store in [disjoint, overlapping] {
        assert!(
            event_u64(store, "issue_tick") < older_response,
            "every younger store must occupy O3 before the leading response: older={older}, store={store}"
        );
    }
    assert!(
        event_u64(load, "issue_tick") < older_response,
        "the partial load must issue while the disjoint store prefix is resident: older={older}, load={load}"
    );
    let memory_trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("disjoint store-prefix run should expose Memory trace");
    let requests_before_older_response = memory_trace
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record
                    .pointer("/tick")
                    .and_then(Value::as_u64)
                    .is_some_and(|tick| tick < older_response)
        })
        .map(memory_request_identity)
        .collect::<BTreeSet<_>>();
    let all_data_requests = memory_trace
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
        })
        .map(memory_request_identity)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    assert_eq!(all_data_requests.len(), 6);
    assert_eq!(
        requests_before_older_response,
        BTreeSet::from([all_data_requests[0], all_data_requests[3]]),
        "only the leading store and partial load may reach transport before the leading response"
    );
    assert!(event_u64(older, "commit_tick") <= event_u64(disjoint, "commit_tick"));
    assert!(event_u64(disjoint, "commit_tick") <= event_u64(overlapping, "commit_tick"));
    assert!(event_u64(overlapping, "commit_tick") <= event_u64(load, "commit_tick"));
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
    assert_eq!(
        load.pointer("/store_load_forwarding_partial")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(event_u64(load, "store_load_forwarding_bytes"), 4);
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );

    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("disjoint store-prefix run should expose Data trace");
    let mut observed = BTreeMap::new();
    for record in data {
        let key = (
            record.pointer("/kind").and_then(Value::as_str).unwrap(),
            record.pointer("/address").and_then(Value::as_str).unwrap(),
            record.pointer("/size").and_then(Value::as_u64).unwrap(),
        );
        *observed.entry(key).or_insert(0usize) += 1;
    }
    assert_eq!(
        observed,
        BTreeMap::from([
            (("store", DATA_ADDRESS, 4), 1),
            (("store", DISJOINT_ADDRESS, 4), 1),
            (("store", "0x80000102", 2), 1),
            (("load", DATA_ADDRESS, 8), 1),
            (("store", "0x80000108", 8), 1),
            (("store", "0x80000110", 8), 1),
        ])
    );
}

fn memory_request_identity(record: &Value) -> (u64, u64) {
    (
        record
            .pointer("/request_agent")
            .and_then(Value::as_u64)
            .expect("Memory trace request agent"),
        record
            .pointer("/request")
            .and_then(Value::as_u64)
            .expect("Memory trace request sequence"),
    )
}

fn assert_final_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(FIRST_RESULTS)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some(SECOND_RESULTS)
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x14")
            .and_then(Value::as_str),
        Some("0x8877665506bb00aa")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x15")
            .and_then(Value::as_str),
        Some("0x8877665506bb00ab")
    );
}

fn disjoint_store_prefix_json(
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
        "4",
        "--dump-memory",
        "0x80000100:16",
        "--dump-memory",
        "0x80000110:16",
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

fn disjoint_store_prefix_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0xaa, 0, 0x0, 11, 0x13),
        i_type(0x5a, 0, 0x0, 12, 0x13),
        i_type(0x6bb, 0, 0x0, 13, 0x13),
        s_type(0, 11, 10, 0b010),
        s_type(24, 12, 10, 0b010),
        s_type(2, 13, 10, 0b001),
        i_type(0, 10, 0b011, 14, 0x03),
        i_type(1, 14, 0x0, 15, 0x13),
        s_type(8, 14, 10, 0b011),
        s_type(16, 15, 10, 0b011),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x4433_2211, 0x8877_6655, 0, 0, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
