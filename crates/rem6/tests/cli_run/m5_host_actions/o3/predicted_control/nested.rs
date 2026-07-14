use super::*;

const LOAD_PC: &str = "0x80000024";
const OUTER_BRANCH_PC: &str = "0x80000028";
const INNER_BRANCH_PC: &str = "0x8000002c";
const DESCENDANT_PC: &str = "0x80000030";
const WRONG_STORE_PC: &str = "0x80000034";
const INNER_TARGET_PC: &str = "0x8000003c";
const OUTER_TARGET_PC: &str = "0x80000040";
const DATA_ADDRESS: &str = "0x800000c0";
const WRONG_STORE_ADDRESS: &str = "0x800000c8";

#[test]
fn rem6_run_o3_nested_controls_commit_direct() {
    let path = nested_control_binary("o3-nested-control-direct", false, false, false);
    let json = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000000000001200000000000000")
    );
    let load = event_at_pc(&json, LOAD_PC);
    let outer = event_at_pc(&json, OUTER_BRANCH_PC);
    let inner = event_at_pc(&json, INNER_BRANCH_PC);
    let descendant = event_at_pc(&json, DESCENDANT_PC);
    event_at_pc(&json, WRONG_STORE_PC);
    event_at_pc(&json, INNER_TARGET_PC);
    event_at_pc(&json, OUTER_TARGET_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");

    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert!(event_u64(inner, "issue_tick") < response_tick);
    assert!(event_u64(descendant, "issue_tick") < response_tick);
    assert!([load, outer, inner, descendant]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
    for branch in [outer, inner] {
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
            Some(false)
        );
        assert_eq!(
            branch
                .pointer("/branch_mispredicted")
                .and_then(Value::as_bool),
            Some(false)
        );
    }
    for (register, value) in [("x12", 42), ("x13", 18), ("x14", 1), ("x15", 2), ("x16", 3)] {
        assert_eq!(register_value(&json, register), value);
    }
    assert!(json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .is_some_and(|records| records.iter().any(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("store")
                && record.pointer("/address").and_then(Value::as_str)
                    == Some(WRONG_STORE_ADDRESS)
        })));
    assert_json_stat(
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
        1,
        "monotonic",
    );
}

fn nested_control_binary(
    name: &str,
    outer_taken: bool,
    inner_taken: bool,
    dependent_inner: bool,
) -> std::path::PathBuf {
    let data_start = 192_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(if outer_taken { 1 } else { 2 }, 0, 0x0, 6, 0x13),
        i_type(3, 0, 0x0, 7, 0x13),
        i_type(if inner_taken { 3 } else { 4 }, 0, 0x0, 8, 0x13),
        i_type(6, 0, 0x0, 9, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        b_type(24, 6, 5, 0b000),
        if dependent_inner {
            b_type(16, 0, 12, 0b000)
        } else {
            b_type(16, 8, 7, 0b000)
        },
        r_type(0x01, 9, 7, 0x0, 13, 0x33),
        s_type(8, 13, 10, 0b010),
        i_type(1, 0, 0x0, 14, 0x13),
        i_type(2, 0, 0x0, 15, 0x13),
        i_type(3, 0, 0x0, 16, 0x13),
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

fn nested_control_command(
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
        "--riscv-branch-lookahead",
        "2",
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

fn run_nested_control_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    extra_args: &[&str],
) -> Value {
    let mut command = nested_control_command(path, memory_system, max_tick, execution_mode);
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} {execution_mode}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid nested-control JSON: {error}"))
}
