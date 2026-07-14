use super::*;

const LOAD_PC: &str = "0x80000030";
const BRANCH_PC: &str = "0x80000034";
const SECOND_ROW_PC: &str = "0x80000038";
const THIRD_ROW_PC: &str = "0x8000003c";

const CROSS_RESOURCE_RESULTS: &str = "2a000000070000004d00000012000000";
const SAME_MULTIPLY_RESULTS: &str = "2a000000070000004d00000022000000";
const FU_HEAD_PC: &str = "0x8000000c";
const FU_INDEPENDENT_PC: &str = "0x80000010";
const FU_DEPENDENT_PC: &str = "0x80000014";
const FU_DEPENDENT_RESULTS: &str = "0c000000050000000100000000000000";

#[test]
fn rem6_run_o3_scoped_issue_width_one_serializes_direct_window() {
    let path = scoped_issue_binary("o3-scoped-issue-width-one", ScopedIssueCase::CrossResource);
    let json = scoped_issue_json(&path, "direct", 1, 1_500);

    assert_completed_scoped_issue(
        &json,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
    );
    let load_issue = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(
        event_u64(event_at_pc(&json, BRANCH_PC), "issue_tick"),
        load_issue + 1
    );
    assert_eq!(
        event_u64(event_at_pc(&json, SECOND_ROW_PC), "issue_tick"),
        load_issue + 2
    );
    assert_eq!(
        event_u64(event_at_pc(&json, THIRD_ROW_PC), "issue_tick"),
        load_issue + 3
    );
}

#[test]
fn rem6_run_o3_scoped_issue_width_two_coissues_cross_resource_rows() {
    let path = scoped_issue_binary(
        "o3-scoped-issue-width-two-cross",
        ScopedIssueCase::CrossResource,
    );
    let json = scoped_issue_json(&path, "direct", 2, 1_500);

    assert_completed_scoped_issue(
        &json,
        CROSS_RESOURCE_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x12"),
        ],
    );
    let load_issue = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(
        event_u64(event_at_pc(&json, BRANCH_PC), "issue_tick"),
        load_issue
    );
    assert_eq!(
        event_u64(event_at_pc(&json, SECOND_ROW_PC), "issue_tick"),
        load_issue + 1
    );
    assert_eq!(
        event_u64(event_at_pc(&json, THIRD_ROW_PC), "issue_tick"),
        load_issue + 1
    );
}

#[test]
fn rem6_run_o3_scoped_issue_serializes_same_multiply_resource() {
    let path = scoped_issue_binary(
        "o3-scoped-issue-same-multiply",
        ScopedIssueCase::SameMultiply,
    );
    let json = scoped_issue_json(&path, "cache-fabric-dram", 2, 1_500);

    assert_completed_scoped_issue(
        &json,
        SAME_MULTIPLY_RESULTS,
        [
            ("x12", "0x2a"),
            ("x13", "0x7"),
            ("x14", "0x4d"),
            ("x15", "0x22"),
        ],
    );
    let load_issue = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(
        event_u64(event_at_pc(&json, BRANCH_PC), "issue_tick"),
        load_issue
    );
    assert_eq!(
        event_u64(event_at_pc(&json, SECOND_ROW_PC), "issue_tick"),
        load_issue + 1
    );
    assert_eq!(
        event_u64(event_at_pc(&json, THIRD_ROW_PC), "issue_tick"),
        load_issue + 2
    );
    assert_memory_hierarchy_activity(&json);
}

#[test]
fn rem6_run_o3_scoped_issue_dependency_waits_for_multiply() {
    let path = scoped_issue_fu_head_binary("o3-scoped-issue-dependent-fu-head");
    let json = scoped_issue_fu_json(&path, "direct", 1, 1_500);

    assert_final_witness(
        &json,
        FU_DEPENDENT_RESULTS,
        [("x3", "0xc"), ("x4", "0x5"), ("x5", "0x1")],
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        3,
        "monotonic",
    );
    let multiply = event_at_pc(&json, FU_HEAD_PC);
    let independent = event_at_pc(&json, FU_INDEPENDENT_PC);
    let dependent = event_at_pc(&json, FU_DEPENDENT_PC);
    assert_eq!(
        event_u64(independent, "issue_tick"),
        event_u64(multiply, "issue_tick") + 2,
        "the fetched younger row must not inherit a phantom head reservation: multiply={multiply}, independent={independent}"
    );
    assert!(
        event_u64(dependent, "issue_tick") >= event_u64(multiply, "writeback_tick"),
        "dependent ADDI must wait for IntMult writeback: multiply={multiply}, dependent={dependent}"
    );
    assert!(
        event_u64(independent, "issue_tick") < event_u64(dependent, "issue_tick"),
        "independent branch should issue before the blocked dependent row: independent={independent}, dependent={dependent}"
    );
}

fn assert_completed_scoped_issue(
    json: &Value,
    expected_memory: &str,
    expected_registers: [(&str, &str); 4],
) {
    assert_final_witness(json, expected_memory, expected_registers);
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
}

fn assert_final_witness<const N: usize>(
    json: &Value,
    expected_memory: &str,
    expected_registers: [(&str, &str); N],
) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(expected_memory),
        "final memory witness should match fixture semantics: {json}"
    );
    for (register, value) in expected_registers {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should match fixture semantics: {json}"
        );
    }
}

fn assert_memory_hierarchy_activity(json: &Value) {
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
            "hierarchy-backed scoped issue run should expose {pointer}: {json}"
        );
    }
    for path in [
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(json, path, "Count", 1, "monotonic");
    }
}

fn scoped_issue_json(path: &Path, memory_system: &str, issue_width: usize, max_tick: u64) -> Value {
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
            "--debug-flags",
            "O3,Data,Fetch,Memory,HostAction",
            "--riscv-o3-scalar-memory-depth",
            "4",
            "--riscv-o3-issue-width",
            &issue_width.to_string(),
            "--memory-system",
            memory_system,
            "--memory-route-delay",
            "16",
            "--m5-switch-cpu-mode",
            "detailed",
            "--dump-memory",
            "0x800000a0:16",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn scoped_issue_fu_json(
    path: &Path,
    memory_system: &str,
    issue_width: usize,
    max_tick: u64,
) -> Value {
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
            "--debug-flags",
            "O3,Data,Fetch,Memory,HostAction",
            "--riscv-o3-issue-width",
            &issue_width.to_string(),
            "--memory-system",
            memory_system,
            "--m5-switch-cpu-mode",
            "detailed",
            "--dump-memory",
            "0x800000a0:16",
        ])
        .output()
        .unwrap();
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedIssueCase {
    CrossResource,
    SameMultiply,
}

fn scoped_issue_binary(name: &str, case: ScopedIssueCase) -> std::path::PathBuf {
    let data_start = 160_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(5, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        i_type(11, 0, 0x0, 3, 0x13),
        i_type(17, 0, 0x0, 4, 0x13),
        i_type(2, 0, 0x0, 5, 0x13),
        i_type(1, 0, 0x0, 6, 0x13),
        i_type(2, 0, 0x0, 7, 0x13),
        i_type(7, 0, 0x0, 13, 0x13),
        i_type(18, 0, 0x0, 15, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        b_type(8, 7, 6, 0b000),
    ]);
    match case {
        ScopedIssueCase::CrossResource => {
            words.extend([r_type(1, 3, 2, 0x0, 14, 0x33), i_type(1, 4, 0x0, 15, 0x13)])
        }
        ScopedIssueCase::SameMultiply => words.extend([
            r_type(1, 3, 2, 0x0, 14, 0x33),
            r_type(1, 5, 4, 0x0, 15, 0x33),
        ]),
    }
    words.extend([
        s_type(4, 13, 10, 0b010),
        s_type(8, 14, 10, 0b010),
        s_type(12, 15, 10, 0b010),
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

fn scoped_issue_fu_head_binary(name: &str) -> std::path::PathBuf {
    let data_start = 160_i32;
    let mut words = vec![
        m5op(M5_SWITCH_CPU),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        r_type(1, 2, 1, 0x4, 3, 0x33),
        i_type(5, 0, 0x0, 4, 0x13),
        i_type(-11, 3, 0x0, 5, 0x13),
    ];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 12, 0x17),
        i_type(data_start - auipc_pc, 12, 0x0, 12, 0x13),
        s_type(0, 3, 12, 0b010),
        s_type(4, 4, 12, 0b010),
        s_type(8, 5, 12, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
