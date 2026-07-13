use super::*;

pub(super) const LOAD_PC: &str = "0x80000014";
pub(super) const FIRST_ALU_PC: &str = "0x80000018";
pub(super) const SECOND_ALU_PC: &str = "0x8000001c";
pub(super) const BRANCH_PC: &str = "0x80000020";
pub(super) const WRONG_STORE_PC: &str = "0x80000024";
pub(super) const TARGET_STORE_PC: &str = "0x80000028";
const DATA_ADDRESS: &str = "0x80000080";
const WRONG_STORE_ADDRESS: &str = "0x80000088";
const TARGET_STORE_ADDRESS: &str = "0x80000084";
const FINAL_MEMORY: &str = "2a0000001000000088776655";

#[test]
fn rem6_run_o3_mixed_load_alu_branch_exposes_terminal_resident_row_direct() {
    let path = mixed_load_alu_branch_binary("o3-mixed-load-alu-branch-terminal-direct");
    let completed = run_mixed_branch_json(&path, "direct", 1_500, "detailed", &[]);
    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(FINAL_MEMORY)
    );
    let load = event_at_pc(&completed, LOAD_PC);
    let issue_tick = event_u64(load, "issue_tick");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let stop_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < stop_tick && stop_tick < response_tick);

    let json = run_mixed_branch_json(&path, "direct", stop_tick, "detailed", &[]);

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
        .unwrap_or_else(|| {
            panic!("resident mixed load/ALU/branch run should expose ROB rows: {json}")
        });
    assert_eq!(
        rob.iter()
            .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        [LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC, BRANCH_PC]
    );
    let branch = rob
        .last()
        .unwrap_or_else(|| panic!("resident ROB should include branch row: {json}"));
    assert!(branch.pointer("/destination").is_some_and(Value::is_null));
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn rem6_run_o3_mixed_load_alu_branch_squash_direct() {
    let path = mixed_load_alu_branch_binary("o3-mixed-load-alu-branch-direct");
    let json = run_mixed_branch_json(&path, "direct", 1_500, "detailed", &[]);

    assert_completed_mixed_branch_window(&json);
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
            json.pointer(pointer).and_then(Value::as_u64),
            Some(0),
            "direct mixed branch run should bypass {pointer}: {json}"
        );
    }
}

#[test]
fn rem6_run_o3_mixed_load_alu_branch_squash_cache_fabric_dram() {
    let path = mixed_load_alu_branch_binary("o3-mixed-load-alu-branch-hierarchy");
    let json = run_mixed_branch_json(&path, "cache-fabric-dram", 1_500, "detailed", &[]);

    assert_completed_mixed_branch_window(&json);
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
            "hierarchy-backed mixed branch run should expose {pointer}: {json}"
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
fn rem6_run_timing_suppresses_o3_mixed_load_alu_branch_window() {
    let path = mixed_load_alu_branch_binary("o3-mixed-load-alu-branch-timing");
    let json = run_mixed_branch_json(&path, "direct", 1_500, "timing", &[]);

    assert_final_mixed_branch_architecture(&json);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert_no_o3_stats(&json);
    for path in [
        "sim.debug.o3_trace.records",
        "sim.debug.o3_trace.instructions",
        "sim.debug.o3_trace.max_rob_occupancy",
        "sim.debug.o3_trace.max_lsq_occupancy",
        "sim.debug.o3_trace.execution_mode.timing",
        "sim.debug.o3_trace.execution_mode.detailed",
        "sim.debug.o3_trace.execution_mode_authority.mode.timing",
    ] {
        assert_json_stat(&json, path, "Count", 0, "monotonic");
    }
}

pub(super) fn assert_completed_mixed_branch_window(json: &Value) {
    assert_final_mixed_branch_architecture(json);

    let load = event_at_pc(json, LOAD_PC);
    let first = event_at_pc(json, FIRST_ALU_PC);
    let second = event_at_pc(json, SECOND_ALU_PC);
    let branch = event_at_pc(json, BRANCH_PC);
    assert_eq!(
        branch.pointer("/branch_event").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch.pointer("/branch_kind").and_then(Value::as_str),
        Some("direct_conditional")
    );
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
    assert_eq!(
        branch
            .pointer("/branch_resolved_target")
            .and_then(Value::as_str),
        Some(TARGET_STORE_PC)
    );
    assert_eq!(
        branch
            .pointer("/branch_squashed_target")
            .and_then(Value::as_str),
        Some(WRONG_STORE_PC)
    );

    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(first, "issue_tick") < response_tick);
    assert!(event_u64(second, "issue_tick") < response_tick);
    assert!(event_u64(branch, "issue_tick") >= response_tick);
    let ordered = [load, first, second, branch];
    assert!(ordered
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
    assert!(event_at_pc_if_present(json, WRONG_STORE_PC).is_none());

    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("mixed-window Data trace");
    assert_eq!(data.len(), 2, "mixed-window Data trace: {data:?}");
    assert_eq!(
        data.iter()
            .filter(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("load")
                    && record.pointer("/address").and_then(Value::as_str) == Some(DATA_ADDRESS)
                    && record.pointer("/size").and_then(Value::as_u64) == Some(4)
            })
            .count(),
        1
    );
    assert_eq!(
        data.iter()
            .filter(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("store")
                    && record.pointer("/address").and_then(Value::as_str)
                        == Some(TARGET_STORE_ADDRESS)
                    && record.pointer("/size").and_then(Value::as_u64) == Some(4)
            })
            .count(),
        1
    );
    assert!(data.iter().all(|record| {
        record.pointer("/address").and_then(Value::as_str) != Some(WRONG_STORE_ADDRESS)
    }));
    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("mixed-window Memory trace");
    assert_eq!(
        memory
            .iter()
            .filter(|record| {
                record.pointer("/channel").and_then(Value::as_str) == Some("data")
                    && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
            })
            .count(),
        2,
        "mixed-window Memory trace should only send the load and target-store data requests: {memory:?}"
    );

    assert_json_stat(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.branch_event.squashes",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.branch_event.squash_kind.direct_conditional",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.ftq.squashes_0::DirectCond",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.debug.memory_trace.data.requests",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.debug.memory_trace.channel.data.events.request_sent",
        "Count",
        2,
        "monotonic",
    );
}

fn assert_final_mixed_branch_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(FINAL_MEMORY)
    );
    for (register, value) in [("x12", "0x2a"), ("x13", "0x5"), ("x14", "0x10")] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should preserve mixed branch semantics: {json}"
        );
    }
}

fn assert_no_o3_stats(json: &Value) {
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
        "timing mode should suppress mixed-window O3 aliases: {unexpected:?}"
    );
}

pub(super) fn mixed_load_alu_branch_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(42, 0, 0x0, 11, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 5, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        b_type(8, 11, 12, 0b000),
        s_type(8, 0, 5, 0b010),
        s_type(4, 14, 5, 0b010),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0x5566_7788, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn mixed_branch_command(
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
        &format!("{DATA_ADDRESS}:12"),
    ]);
    command
}

pub(super) fn run_mixed_branch_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    extra_args: &[&str],
) -> Value {
    let mut command = mixed_branch_command(path, memory_system, max_tick, execution_mode);
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} {execution_mode}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid mixed-window JSON: {error}"))
}

pub(super) fn event_at_pc_if_present<'a>(json: &'a Value, pc: &str) -> Option<&'a Value> {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
}

pub(super) fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    event_at_pc_if_present(json, pc).unwrap_or_else(|| panic!("missing O3 event at {pc}: {json}"))
}

pub(super) fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing {field}: {event}"))
}
