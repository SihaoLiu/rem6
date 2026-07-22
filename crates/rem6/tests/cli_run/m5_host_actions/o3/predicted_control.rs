use super::lsq_fu_branch::{event_at_pc, event_at_pc_if_present, event_u64};
use super::*;

#[path = "predicted_control/coroutine.rs"]
mod coroutine;
#[path = "predicted_control/general_iq.rs"]
mod general_iq;
#[path = "predicted_control/link_kind.rs"]
mod link_kind;
#[path = "predicted_control/link_return.rs"]
mod link_return;
#[path = "predicted_control/mixed_kind.rs"]
mod mixed_kind;
#[path = "predicted_control/nested.rs"]
mod nested;
#[path = "predicted_control/producer_forwarded_jalr.rs"]
mod producer_forwarded_jalr;
#[path = "predicted_control/producer_forwarded_lineage.rs"]
mod producer_forwarded_lineage;
#[path = "predicted_control/producer_forwarded_return.rs"]
mod producer_forwarded_return;
#[path = "predicted_control/producer_forwarded_scalar_return.rs"]
mod producer_forwarded_scalar_return;
#[path = "predicted_control/same_link.rs"]
mod same_link;
#[path = "predicted_control/three_deep.rs"]
mod three_deep;
#[path = "predicted_control/window_support.rs"]
mod window_support;

const LOAD_PC: &str = "0x80000024";
const BRANCH_PC: &str = "0x80000028";
const MUL_PC: &str = "0x8000002c";
const ADD_PC: &str = "0x80000030";
const DATA_ADDRESS: &str = "0x800000c0";
const TAKEN_LOAD_PC: &str = "0x8000002c";
const TAKEN_BRANCH_PC: &str = "0x80000030";
const TAKEN_MUL_PC: &str = "0x8000003c";
const TAKEN_ADD_PC: &str = "0x80000040";

#[test]
fn rem6_run_o3_correctly_predicted_taken_descendants_commit_direct() {
    let path = predicted_taken_control_binary("o3-predicted-taken-control-direct");
    let completed = run_predicted_control_json(&path, "direct", 2_500, "detailed", &[]);

    assert_eq!(register_value(&completed, "x13"), 42);
    assert_eq!(register_value(&completed, "x14"), 45);
    assert_eq!(register_value(&completed, "x16"), 2);
    assert_eq!(register_value(&completed, "x17"), 2);

    let loads = events_at_pc(&completed, TAKEN_LOAD_PC);
    let branches = events_at_pc(&completed, TAKEN_BRANCH_PC);
    let multiplies = events_at_pc(&completed, TAKEN_MUL_PC);
    let adds = events_at_pc(&completed, TAKEN_ADD_PC);
    assert_eq!(loads.len(), 2, "expected two load iterations: {completed}");
    assert_eq!(
        branches.len(),
        2,
        "expected two branch iterations: {completed}"
    );
    assert_eq!(
        multiplies.len(),
        2,
        "expected two multiply iterations: {completed}"
    );
    assert_eq!(adds.len(), 2, "expected two add iterations: {completed}");

    let load = loads[1];
    let branch = branches
        .iter()
        .copied()
        .find(|event| {
            event
                .pointer("/branch_predicted_taken")
                .and_then(Value::as_bool)
                == Some(true)
        })
        .unwrap_or_else(|| panic!("missing trained taken prediction: {completed}"));
    let multiply = multiplies[1];
    let add = adds[1];
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(branch, "issue_tick") < response_tick);
    assert!(event_u64(multiply, "issue_tick") < response_tick);
    assert!(event_u64(add, "issue_tick") < response_tick);
    assert_eq!(
        event_u64(add, "issue_tick"),
        event_u64(multiply, "writeback_tick")
    );
    assert_eq!(
        branch
            .pointer("/branch_resolved_taken")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!([load, branch, multiply, add]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
}

#[test]
fn rem6_run_o3_predicted_descendants_squash_cache_fabric_dram() {
    let path = predicted_control_binary("o3-predicted-control-hierarchy", true, true, false);
    let completed = run_predicted_control_json(&path, "cache-fabric-dram", 2_000, "detailed", &[]);

    assert_eq!(register_value(&completed, "x12"), 0x2a);
    assert_eq!(register_value(&completed, "x13"), 0);
    assert_eq!(register_value(&completed, "x14"), 0);
    assert_eq!(register_value(&completed, "x15"), 0);
    assert_eq!(register_value(&completed, "x16"), 2);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000000000000000000000")
    );

    let load = event_at_pc(&completed, LOAD_PC);
    let branch = event_at_pc(&completed, BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(branch, "issue_tick") < response_tick);
    assert_eq!(
        branch
            .pointer("/branch_predicted_taken")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        branch
            .pointer("/branch_resolved_taken")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch.pointer("/branch_squash").and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&completed, MUL_PC).is_none());
    assert!(event_at_pc_if_present(&completed, ADD_PC).is_none());

    let live_tick = event_u64(branch, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident =
        run_predicted_control_json(&path, "cache-fabric-dram", live_tick, "detailed", &[]);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing resident wrong-path ROB: {resident}"));
    assert_eq!(
        rob.iter()
            .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        [LOAD_PC, BRANCH_PC, MUL_PC, ADD_PC]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .filter_map(|entry| entry.pointer("/architectural").and_then(Value::as_u64))
            .filter(|architectural| matches!(architectural, 12 | 13 | 14))
            .collect::<Vec<_>>(),
        [12, 13, 14]
    );

    let data = completed
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("predicted-control Data trace");
    assert!(data.iter().all(|record| {
        record.pointer("/address").and_then(Value::as_str) != Some("0x800000c8")
    }));
    let memory = completed
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("predicted-control Memory trace");
    assert!(memory.iter().all(|record| {
        record.pointer("/address").and_then(Value::as_str) != Some("0x800000c8")
    }));
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            completed
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed predicted control should expose {pointer}: {completed}"
        );
    }
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.branch_event.squashes",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_load_dependent_branch_suppresses_predicted_descendants() {
    let path = predicted_control_binary("o3-predicted-control-dependent", false, false, true);
    let completed = run_predicted_control_json(&path, "direct", 1_500, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let branch = event_at_pc(&completed, BRANCH_PC);
    let issue_tick = event_u64(load, "issue_tick");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(branch, "issue_tick") >= response_tick);
    assert!(event_u64(event_at_pc(&completed, MUL_PC), "issue_tick") >= response_tick);
    assert!(event_u64(event_at_pc(&completed, ADD_PC), "issue_tick") >= response_tick);

    let live_tick = issue_tick + (response_tick - issue_tick) / 2;
    let resident = run_predicted_control_json(&path, "direct", live_tick, "detailed", &[]);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing dependent-branch ROB: {resident}"));
    assert_eq!(
        rob.iter()
            .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        [LOAD_PC, BRANCH_PC]
    );
    assert_eq!(
        completed
            .pointer("/cores/0/registers/x14")
            .and_then(Value::as_str),
        Some("0x2d")
    );
}

fn predicted_control_binary(
    name: &str,
    branch_taken: bool,
    wrong_path_store: bool,
    dependent_branch: bool,
) -> std::path::PathBuf {
    let data_start = 192_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(if branch_taken { 1 } else { 2 }, 0, 0x0, 6, 0x13),
        i_type(6, 0, 0x0, 7, 0x13),
        i_type(7, 0, 0x0, 8, 0x13),
        i_type(3, 0, 0x0, 9, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        if dependent_branch {
            b_type(20, 0, 12, 0b000)
        } else {
            b_type(20, 6, 5, 0b000)
        },
        r_type(0x01, 8, 7, 0x0, 13, 0x33),
        r_type(0, 9, 13, 0x0, 14, 0x33),
        if wrong_path_store {
            s_type(8, 14, 10, 0b010)
        } else {
            i_type(0, 0, 0x0, 0, 0x13)
        },
        i_type(1, 0, 0x0, 15, 0x13),
        i_type(2, 0, 0x0, 16, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn predicted_taken_control_binary(name: &str) -> std::path::PathBuf {
    let data_start = 192_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(1, 0, 0x0, 6, 0x13),
        i_type(6, 0, 0x0, 7, 0x13),
        i_type(7, 0, 0x0, 8, 0x13),
        i_type(3, 0, 0x0, 9, 0x13),
        i_type(0, 0, 0x0, 17, 0x13),
        i_type(2, 0, 0x0, 18, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        b_type(12, 6, 5, 0b000),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        r_type(0x01, 8, 7, 0x0, 13, 0x33),
        r_type(0, 9, 13, 0x0, 14, 0x33),
        i_type(1, 17, 0x0, 17, 0x13),
        b_type(-28, 18, 17, 0b100),
        i_type(2, 0, 0x0, 16, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn predicted_control_command(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
) -> Command {
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
        "O3,Data,Fetch,Memory,HostAction",
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        execution_mode,
        "--dump-memory",
        &format!("{DATA_ADDRESS}:16"),
    ]);
    command
}

fn run_predicted_control_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    extra_args: &[&str],
) -> Value {
    let mut command = predicted_control_command(path, memory_system, max_tick, execution_mode);
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} {execution_mode}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid predicted-control JSON: {error}"))
}

fn register_value(json: &Value, register: &str) -> u64 {
    json.pointer(&format!("/cores/0/registers/{register}"))
        .and_then(Value::as_str)
        .map(|value| u64::from_str_radix(value.trim_start_matches("0x"), 16).unwrap())
        .unwrap_or(0)
}

fn events_at_pc<'a>(json: &'a Value, pc: &str) -> Vec<&'a Value> {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing O3 events: {json}"))
        .iter()
        .filter(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .collect()
}

#[test]
#[should_panic(expected = "expected exactly one transfer component")]
fn transfer_component_rejects_duplicate_component_entries() {
    let transfer = serde_json::json!({
        "components": [
            { "component": "cpu0", "chunks": [] },
            { "component": "cpu0", "chunks": [] },
        ]
    });

    let _ = transfer_component(&transfer, "cpu0");
}

#[test]
#[should_panic(expected = "expected exactly one transfer chunk")]
fn transfer_o3_runtime_chunk_rejects_duplicate_named_chunks() {
    let transfer = serde_json::json!({
        "components": [{
            "component": "cpu0",
            "chunks": [
                { "name": "o3-runtime-state", "o3_runtime": { "marker": 1 } },
                { "name": "o3-runtime-state", "o3_runtime": { "marker": 2 } },
            ]
        }]
    });

    let _ = transfer_o3_runtime_chunk(&transfer, "cpu0");
}

fn transfer_component<'a>(transfer: &'a Value, component: &str) -> &'a Value {
    transfer_component_with_context(transfer, component, "transfer artifact")
}

fn transfer_component_with_context<'a>(
    transfer: &'a Value,
    component: &str,
    context: &str,
) -> &'a Value {
    let components = transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context}: missing transfer components: {transfer}"));
    exact_one_by_string_field(
        components,
        "/component",
        component,
        "transfer component",
        context,
    )
}

fn transfer_o3_runtime_chunk<'a>(transfer: &'a Value, component: &str) -> &'a Value {
    transfer_o3_runtime_chunk_with_context(transfer, component, "transfer artifact")
}

fn transfer_o3_runtime_chunk_with_context<'a>(
    transfer: &'a Value,
    component: &str,
    context: &str,
) -> &'a Value {
    let component_state = transfer_component_with_context(transfer, component, context);
    let chunks = component_state
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "{context}: missing transfer chunks for component {component}: {component_state}"
            )
        });
    let chunk = exact_one_by_string_field(
        chunks,
        "/name",
        "o3-runtime-state",
        "transfer chunk",
        context,
    );
    chunk
        .pointer("/o3_runtime")
        .unwrap_or_else(|| panic!("{context}: missing decoded O3 runtime chunk: {transfer}"))
}

fn transfer_live_data_handoff_chunk<'a>(transfer: &'a Value, component: &str) -> &'a Value {
    transfer_live_data_handoff_chunk_with_context(transfer, component, "transfer artifact")
}

fn transfer_live_data_handoff_chunk_with_context<'a>(
    transfer: &'a Value,
    component: &str,
    context: &str,
) -> &'a Value {
    let component_state = transfer_component_with_context(transfer, component, context);
    let chunks = component_state
        .pointer("/chunks")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "{context}: missing transfer chunks for component {component}: {component_state}"
            )
        });
    let chunk = exact_one_by_string_field(
        chunks,
        "/name",
        "o3-live-data-handoff",
        "transfer chunk",
        context,
    );
    chunk
        .pointer("/o3_live_data_handoff")
        .unwrap_or_else(|| panic!("{context}: missing decoded live-data handoff chunk: {transfer}"))
}

fn checkpoint_component<'a>(checkpoint: &'a Value, component: &str) -> &'a Value {
    checkpoint_component_with_context(checkpoint, component, "checkpoint artifact")
}

fn checkpoint_component_with_context<'a>(
    checkpoint: &'a Value,
    component: &str,
    context: &str,
) -> &'a Value {
    let components = checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context}: missing checkpoint components: {checkpoint}"));
    exact_one_by_string_field(
        components,
        "/component",
        component,
        "checkpoint component",
        context,
    )
}

fn checkpoint_component_chunks(component: &Value) -> &[Value] {
    checkpoint_component_chunks_with_context(component, "checkpoint artifact")
}

fn checkpoint_component_chunks_with_context<'a>(
    component: &'a Value,
    context: &str,
) -> &'a [Value] {
    component
        .pointer("/chunks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("{context}: missing checkpoint chunks: {component}"))
}

fn checkpoint_component_chunk_with_context<'a>(
    chunks: &'a [Value],
    name: &str,
    context: &str,
) -> &'a Value {
    exact_one_by_string_field(chunks, "/name", name, "checkpoint chunk", context)
}

fn exact_one_by_string_field<'a>(
    values: &'a [Value],
    field: &str,
    expected: &str,
    artifact: &str,
    context: &str,
) -> &'a Value {
    let mut matched = None;
    let mut count = 0;
    for value in values {
        if value.pointer(field).and_then(Value::as_str) == Some(expected) {
            matched = Some(value);
            count += 1;
        }
    }
    match (matched, count) {
        (Some(value), 1) => value,
        _ => panic!(
            "{context}: expected exactly one {artifact} with {field}={expected}, found {count}: {values:?}"
        ),
    }
}
