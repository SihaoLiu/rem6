use super::lsq_store_load::{data_memory_request_count, event_at_pc, event_u64};
use super::*;

const STORE_PC: &str = "0x80000010";
const FIRST_LOAD_PC: &str = "0x80000014";
const SECOND_LOAD_PC: &str = "0x80000018";
const THIRD_LOAD_PC: &str = "0x8000001c";
const DATA_ADDRESS: &str = "0x80000100";
const FIRST_LOAD_ADDRESS: &str = "0x80000140";
const SECOND_LOAD_ADDRESS: &str = "0x80000180";
const THIRD_LOAD_ADDRESS: &str = "0x800001c0";
const RESULTS_PREFIX: &str = "2a000000630000007700000064000000";
const FOUR_ROW_RESULTS_LOWER: &str = "2a000000630000007700000088000000";
const FOUR_ROW_RESULTS_UPPER: &str = "640000007800000089000000";
const THREE_ROW_DUMPS: [&str; 4] = [
    "0x80000100:16",
    "0x80000110:4",
    "0x80000140:4",
    "0x80000180:4",
];
const FOUR_ROW_DUMPS: [&str; 5] = [
    "0x80000100:16",
    "0x80000110:12",
    "0x80000140:4",
    "0x80000180:4",
    "0x800001c0:4",
];
const LOAD_REGISTERS: [u8; 3] = [12, 13, 16];
const DERIVED_REGISTERS: [u8; 3] = [14, 15, 17];

#[test]
fn rem6_run_o3_detailed_store_and_two_loads_overlap_direct() {
    let path = store_led_binary("o3-store-load-load-direct", &[0x63, 0x77]);
    let json = store_led_json(&path, "direct", 1100, None, 3, &THREE_ROW_DUMPS);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_completed_store_load_load(&json);
    assert_direct_hierarchy_boundary(&json, "store-load-load");
}

#[test]
fn rem6_run_o3_detailed_store_and_three_loads_fill_depth_four_direct() {
    let path = store_led_binary("o3-store-load-load-load-direct", &[0x63, 0x77, 0x88]);
    let json = store_led_json(&path, "direct", 1400, None, 4, &FOUR_ROW_DUMPS);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_final_four_row_architecture(&json);
    assert_eq!(data_memory_request_count(&json), 10);
    assert_json_stat_at_least(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        4,
        "monotonic",
    );

    let events = [
        event_at_pc(&json, STORE_PC),
        event_at_pc(&json, FIRST_LOAD_PC),
        event_at_pc(&json, SECOND_LOAD_PC),
        event_at_pc(&json, THIRD_LOAD_PC),
    ];
    assert_eq!(
        events[3]
            .pointer("/lsq_load_address")
            .and_then(Value::as_str),
        Some(THIRD_LOAD_ADDRESS)
    );
    let first_response_tick = events
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .expect("depth-four store-led window response tick");
    assert!(
        events
            .iter()
            .all(|event| event_u64(event, "issue_tick") < first_response_tick),
        "all four memory operations should issue before the first response: {events:?}"
    );
    assert!(events
        .windows(2)
        .all(|pair| event_u64(pair[0], "commit_tick") <= event_u64(pair[1], "commit_tick")));
    assert_direct_hierarchy_boundary(&json, "store-load-load-load");
}

#[test]
fn rem6_run_o3_detailed_store_and_two_loads_overlap_cache_fabric_dram() {
    let path = store_led_binary("o3-store-load-load-cache-fabric-dram", &[0x63, 0x77]);
    let json = store_led_json(&path, "cache-fabric-dram", 1700, None, 3, &THREE_ROW_DUMPS);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_completed_store_load_load(&json);
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
            "hierarchy-backed store-load-load run should expose {pointer}: {json}"
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
fn rem6_run_o3_detailed_store_and_two_loads_remain_resident_together() {
    let path = store_led_binary("o3-store-load-load-resident", &[0x63, 0x77]);
    let completed = store_led_json(&path, "direct", 1100, None, 3, &THREE_ROW_DUMPS);
    let store = event_at_pc(&completed, STORE_PC);
    let first_load = event_at_pc(&completed, FIRST_LOAD_PC);
    let second_load = event_at_pc(&completed, SECOND_LOAD_PC);
    let second_issue_tick = event_u64(second_load, "issue_tick");
    let first_response_tick = [store, first_load, second_load]
        .into_iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .expect("store-led window response tick");
    let stop_tick =
        second_issue_tick.saturating_add(first_response_tick.saturating_sub(second_issue_tick) / 2);
    assert!(second_issue_tick < stop_tick && stop_tick < first_response_tick);

    let json = store_led_json(&path, "direct", stop_tick, None, 3, &THREE_ROW_DUMPS);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident store-load-load run should expose ROB rows: {json}"));
    for pc in [STORE_PC, FIRST_LOAD_PC, SECOND_LOAD_PC] {
        let entry = rob
            .iter()
            .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .unwrap_or_else(|| panic!("resident store-led ROB should include {pc}: {json}"));
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
        .unwrap_or_else(|| panic!("resident store-load-load run should expose LSQ rows: {json}"));
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
            (Some("load"), Some(FIRST_LOAD_ADDRESS), Some(false)),
            (Some("load"), Some(SECOND_LOAD_ADDRESS), Some(false)),
        ]
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000000000000000000000"),
        "resident store-led window must expose its issued leading store but not younger result stores"
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("00000000"),
        "resident store-led window must not expose its final result store"
    );
    for register in ["x12", "x13", "x14", "x15"] {
        let value = json
            .pointer(&format!("/cores/0/registers/{register}"))
            .and_then(Value::as_str);
        assert!(
            value.is_none_or(|value| value == "0x0"),
            "resident store-led window must defer architectural {register}: {json}"
        );
    }
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        3,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_timing_store_and_two_loads_preserve_architecture_without_o3_window() {
    let path = store_led_binary("o3-store-load-load-timing", &[0x63, 0x77]);
    let json = store_led_json(&path, "direct", 1100, Some("timing"), 3, &THREE_ROW_DUMPS);

    assert_final_architecture(&json);
    assert_eq!(data_memory_request_count(&json), 7);
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
        "timing mode should suppress store-load-load O3 aliases: {unexpected:?}"
    );
}

fn assert_completed_store_load_load(json: &Value) {
    assert_final_architecture(json);
    assert_eq!(data_memory_request_count(json), 7);
    assert_json_stat_at_least(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        3,
        "monotonic",
    );

    let store = event_at_pc(json, STORE_PC);
    let first_load = event_at_pc(json, FIRST_LOAD_PC);
    let second_load = event_at_pc(json, SECOND_LOAD_PC);
    assert_eq!(
        store.pointer("/lsq_operation").and_then(Value::as_str),
        Some("store")
    );
    for load in [first_load, second_load] {
        assert_eq!(
            load.pointer("/lsq_operation").and_then(Value::as_str),
            Some("load")
        );
    }
    assert_eq!(
        store.pointer("/lsq_store_address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        first_load
            .pointer("/lsq_load_address")
            .and_then(Value::as_str),
        Some(FIRST_LOAD_ADDRESS)
    );
    assert_eq!(
        second_load
            .pointer("/lsq_load_address")
            .and_then(Value::as_str),
        Some(SECOND_LOAD_ADDRESS)
    );

    let events = [store, first_load, second_load];
    let first_response_tick = events
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .expect("store-led window response tick");
    assert!(
        events
            .iter()
            .all(|event| event_u64(event, "issue_tick") < first_response_tick),
        "all three memory operations should issue before the first response: {events:?}"
    );
    assert!(
        event_u64(store, "commit_tick") <= event_u64(first_load, "commit_tick")
            && event_u64(first_load, "commit_tick") <= event_u64(second_load, "commit_tick"),
        "store-led mixed window must retire in program order: store={store}, first={first_load}, second={second_load}"
    );
}

fn assert_final_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(RESULTS_PREFIX)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("78000000")
    );
    assert_eq!(
        json.pointer("/memory/2/hex").and_then(Value::as_str),
        Some("63000000")
    );
    assert_eq!(
        json.pointer("/memory/3/hex").and_then(Value::as_str),
        Some("77000000")
    );
    for (register, value) in [
        ("x12", "0x63"),
        ("x13", "0x77"),
        ("x14", "0x64"),
        ("x15", "0x78"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should preserve store-load-load semantics: {json}"
        );
    }
}

fn assert_final_four_row_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(FOUR_ROW_RESULTS_LOWER)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some(FOUR_ROW_RESULTS_UPPER)
    );
    for (index, value) in ["63000000", "77000000", "88000000"].into_iter().enumerate() {
        assert_eq!(
            json.pointer(&format!("/memory/{}/hex", index + 2))
                .and_then(Value::as_str),
            Some(value)
        );
    }
    for (register, value) in [
        ("x12", "0x63"),
        ("x13", "0x77"),
        ("x16", "0x88"),
        ("x14", "0x64"),
        ("x15", "0x78"),
        ("x17", "0x89"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should preserve depth-four store-led semantics: {json}"
        );
    }
}

fn assert_direct_hierarchy_boundary(json: &Value, workload: &str) {
    assert!(
        json.pointer("/memory_resources/transport/data/activity")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0),
        "direct {workload} run should retain transport activity: {json}"
    );
    assert_json_stat_at_least(
        json,
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
            0,
            "direct {workload} run should not exercise {pointer}: {json}"
        );
        assert_json_stat(json, path, "Count", 0, "monotonic");
    }
}

fn store_led_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    switch_mode: Option<&str>,
    depth: usize,
    dumps: &[&str],
) -> Value {
    let max_tick = max_tick.to_string();
    let depth = depth.to_string();
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick,
        "--stats-format",
        "json",
        "--execute",
        "--riscv-o3-scalar-memory-depth",
        &depth,
        "--debug-flags",
        "O3,Data,Memory",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
    ]);
    for dump in dumps {
        command.args(["--dump-memory", dump]);
    }
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

fn store_led_binary(name: &str, source_values: &[u32]) -> std::path::PathBuf {
    assert!(!source_values.is_empty() && source_values.len() <= LOAD_REGISTERS.len());
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x2a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 10, 0b010),
    ]);
    for (index, register) in LOAD_REGISTERS
        .iter()
        .copied()
        .take(source_values.len())
        .enumerate()
    {
        words.push(i_type(
            64 * i32::try_from(index + 1).unwrap(),
            10,
            0b010,
            register,
            0x03,
        ));
    }
    for (load, derived) in LOAD_REGISTERS
        .iter()
        .copied()
        .zip(DERIVED_REGISTERS.iter().copied())
        .take(source_values.len())
    {
        words.push(i_type(1, load, 0x0, derived, 0x13));
    }
    for (index, register) in LOAD_REGISTERS
        .iter()
        .copied()
        .take(source_values.len())
        .enumerate()
    {
        words.push(s_type(
            4 * i32::try_from(index + 1).unwrap(),
            register,
            10,
            0b010,
        ));
    }
    for (index, register) in DERIVED_REGISTERS
        .iter()
        .copied()
        .take(source_values.len())
        .enumerate()
    {
        words.push(s_type(
            4 * i32::try_from(source_values.len() + index + 1).unwrap(),
            register,
            10,
            0b010,
        ));
    }
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    let mut data = vec![0_u32; 16 * source_values.len() + 1];
    for (index, value) in source_values.iter().copied().enumerate() {
        data[16 * (index + 1)] = value;
    }
    words.extend(data);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
