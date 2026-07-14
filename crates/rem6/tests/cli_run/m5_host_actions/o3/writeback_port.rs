use super::*;

const MUL_PC: &str = "0x8000000c";
const DEPENDENT_PC: &str = "0x80000010";

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
