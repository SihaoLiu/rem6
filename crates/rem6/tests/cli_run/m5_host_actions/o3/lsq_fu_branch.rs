use super::*;

const LOAD_PC: &str = "0x80000014";
const FIRST_ALU_PC: &str = "0x80000018";
const SECOND_ALU_PC: &str = "0x8000001c";
const BRANCH_PC: &str = "0x80000020";
const DATA_HEX_AFTER_BRANCH: &str = "2a0000001000000088776655";

#[test]
fn rem6_run_o3_mixed_load_alu_branch_exposes_terminal_resident_row_direct() {
    let path = mixed_load_alu_branch_binary("o3-mixed-load-alu-branch-terminal-direct");
    let completed = mixed_load_alu_branch_json(&path, 1_500);
    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(DATA_HEX_AFTER_BRANCH)
    );
    let load = event_at_pc(&completed, LOAD_PC);
    let issue_tick = event_u64(load, "issue_tick");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let stop_tick = issue_tick.saturating_add(response_tick.saturating_sub(issue_tick) / 2);
    assert!(issue_tick < stop_tick && stop_tick < response_tick);

    let json = mixed_load_alu_branch_json(&path, stop_tick);

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
    words.extend([42, 0, 0x5566_7788]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

pub(super) fn mixed_load_alu_branch_command(path: &Path, max_tick: u64) -> Command {
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
        "direct",
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        "detailed",
        "--dump-memory",
        "0x80000080:12",
    ]);
    command
}

pub(super) fn mixed_load_alu_branch_json(path: &Path, max_tick: u64) -> Value {
    let output = mixed_load_alu_branch_command(path, max_tick)
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

pub(super) fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

pub(super) fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}
