use super::*;

const MUL_PC: &str = "0x8000000c";
const DEPENDENT_PC: &str = "0x80000010";
const SCALAR_LOAD_PC: &str = "0x80000014";
const SCALAR_DIV_PC: &str = "0x80000018";
const SCALAR_LOAD_DEPENDENT_PC: &str = "0x8000001c";

#[test]
fn rem6_run_o3_writeback_width_one_serializes_direct_fu_dependent_collision() {
    let json = writeback_json(1);
    let multiply = event_at_pc(&json, MUL_PC);
    let dependent = event_at_pc(&json, DEPENDENT_PC);

    assert_eq!(
        event_u64(dependent, "issue_tick"),
        event_u64(multiply, "writeback_tick"),
        "dependent ADDI must issue from the admitted MUL writeback tick: multiply={multiply}, dependent={dependent}"
    );
    assert_eq!(
        event_u64(dependent, "writeback_tick"),
        event_u64(multiply, "writeback_tick") + 1,
        "width-one writeback must serialize the dependent zero-latency ADDI completion: multiply={multiply}, dependent={dependent}"
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x3")
            .and_then(Value::as_str),
        Some("0x2a")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x4")
            .and_then(Value::as_str),
        Some("0x2b")
    );
}

#[test]
fn rem6_run_o3_writeback_width_two_exact_fit_direct_fu_dependent_collision() {
    let json = writeback_json(2);
    let multiply = event_at_pc(&json, MUL_PC);
    let dependent = event_at_pc(&json, DEPENDENT_PC);

    assert_eq!(
        event_u64(dependent, "issue_tick"),
        event_u64(multiply, "writeback_tick"),
        "dependent ADDI must issue from the admitted MUL writeback tick: multiply={multiply}, dependent={dependent}"
    );
    assert_eq!(
        event_u64(dependent, "writeback_tick"),
        event_u64(multiply, "writeback_tick"),
        "width-two writeback should admit the MUL and dependent ADDI in the same cycle: multiply={multiply}, dependent={dependent}"
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x3")
            .and_then(Value::as_str),
        Some("0x2a")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x4")
            .and_then(Value::as_str),
        Some("0x2b")
    );
}

#[test]
fn rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission() {
    let path = scalar_load_admission_binary();
    let mut collision_runs: Vec<_> = [4, 8, 9, 12, 16, 20]
        .into_iter()
        .filter_map(|route_delay| {
            let json = scalar_load_admission_json(&path, 2, route_delay, 600);
            let load = event_at_pc(&json, SCALAR_LOAD_PC);
            let fu = event_at_pc(&json, SCALAR_DIV_PC);
            (event_u64(load, "lsq_data_response_tick") + 1 == event_u64(fu, "issue_tick") + 19)
                .then_some((route_delay, json))
        })
        .collect();
    assert_eq!(
        collision_runs.len(),
        1,
        "route-delay calibration must find exactly one scalar-load/DIV raw-ready collision"
    );
    let (route_delay, calibration) = collision_runs.pop().unwrap();
    assert_eq!(route_delay, 9);
    let calibrated_load = event_at_pc(&calibration, SCALAR_LOAD_PC);
    let calibrated_fu = event_at_pc(&calibration, SCALAR_DIV_PC);
    assert_eq!(
        event_u64(calibrated_load, "lsq_data_response_tick") + 1,
        event_u64(calibrated_fu, "issue_tick") + 19,
        "width-two calibration must align the scalar-load and DIV raw-ready ticks"
    );

    let full = scalar_load_admission_json(&path, 1, route_delay, 600);
    let load = event_at_pc(&full, SCALAR_LOAD_PC);
    let fu = event_at_pc(&full, SCALAR_DIV_PC);
    let dependent = event_at_pc(&full, SCALAR_LOAD_DEPENDENT_PC);
    let load_raw_ready = event_u64(load, "lsq_data_response_tick") + 1;
    let fu_raw_ready = event_u64(fu, "issue_tick") + 19;
    let admitted_tick = event_u64(load, "writeback_tick");
    assert_eq!(load_raw_ready, fu_raw_ready);
    assert_eq!(admitted_tick, load_raw_ready + 1);
    assert_eq!(event_u64(fu, "writeback_tick"), fu_raw_ready);
    assert!(event_u64(dependent, "issue_tick") >= admitted_tick);
    assert_eq!(
        full.pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some("0x2a")
    );

    let before = scalar_load_admission_json(&path, 1, route_delay, admitted_tick - 1);
    assert_eq!(
        before
            .pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        None,
        "the zero-valued architectural load destination must remain absent before admission"
    );
    let load_row = rob_entry_at_pc(&before, SCALAR_LOAD_PC);
    assert_eq!(
        load_row.pointer("/ready").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        load_row.pointer("/live_staged").and_then(Value::as_bool),
        Some(true)
    );

    let at_admission = scalar_load_admission_json(&path, 1, route_delay, admitted_tick);
    assert_eq!(
        at_admission
            .pointer("/cores/0/registers/x12")
            .and_then(Value::as_str),
        Some("0x2a"),
        "the architectural load value must appear exactly at admission"
    );
}

fn writeback_json(writeback_width: usize) -> Value {
    let path = writeback_binary();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "600",
            "--execute",
            "--stats-format",
            "json",
            "--debug-flags",
            "O3,Data,Fetch,Memory,HostAction",
            "--riscv-o3-issue-width",
            "4",
            "--riscv-o3-writeback-width",
            &writeback_width.to_string(),
            "--memory-system",
            "direct",
            "--memory-route-delay",
            "1",
            "--m5-switch-cpu-mode",
            "detailed",
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

fn scalar_load_admission_json(
    path: &std::path::Path,
    writeback_width: usize,
    route_delay: u64,
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
            "--execute",
            "--stats-format",
            "json",
            "--debug-flags",
            "O3,Data,Fetch,Memory,HostAction",
            "--riscv-o3-issue-width",
            "4",
            "--riscv-o3-writeback-width",
            &writeback_width.to_string(),
            "--memory-system",
            "direct",
            "--memory-route-delay",
            &route_delay.to_string(),
            "--m5-switch-cpu-mode",
            "detailed",
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

fn o3_trace_events(json: &Value) -> &[Value] {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("O3 trace should expose events: {json}"))
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    o3_trace_events(json)
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

fn rob_entry_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 ROB should retain live row at {pc}: {json}"))
}

fn writeback_binary() -> std::path::PathBuf {
    let words = [
        i_type(6, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        m5op(M5_SWITCH_CPU),
        r_type(1, 2, 1, 0x0, 3, 0x33),
        i_type(1, 3, 0x0, 4, 0x13),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("m5-switch-cpu-o3-writeback-port", &elf)
}

fn scalar_load_admission_binary() -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(84, 0, 0x0, 1, 0x13),
        i_type(2, 0, 0x0, 2, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        r_type(1, 2, 1, 0b100, 3, 0x33),
        i_type(1, 12, 0x0, 14, 0x13),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary("m5-switch-cpu-o3-writeback-scalar-load-admission", &elf)
}
