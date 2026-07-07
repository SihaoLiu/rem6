use std::process::Command;

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_records_o3_runtime_stats_from_initial_detailed_mode() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &o3_start_mode_program());
    let path = temp_binary("o3-start-detailed-mode", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-execution-mode",
            "detailed",
            "--dump-memory",
            "0x80000020:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("050000003c000000")
    );
    assert!(stdout.contains("\"x5\":\"0x5\""));
    assert!(stdout.contains("\"x6\":\"0xc\""));
    assert!(stdout.contains("\"x7\":\"0x3c\""));

    assert_eq!(
        json.pointer("/host_actions/execution_mode_switch_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_execution_mode_authority(&json, "detailed");
    assert_eq!(
        json_stat_value(
            &json,
            "sim.host_actions.execution_mode_authority.mode.detailed"
        ),
        1
    );
    assert_eq!(
        json_stat_value(
            &json,
            "sim.host_actions.execution_mode_authority.target.cpu0.mode.detailed",
        ),
        1
    );
    assert_eq!(
        json_stat_value(&json, "sim.host_actions.execution_mode_switches"),
        0
    );

    let instructions = json_stat_value(&json, "sim.cpu0.o3.instructions");
    assert!(instructions > 0, "{stdout}");
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/instructions")
            .and_then(Value::as_u64),
        Some(instructions)
    );
    assert_eq!(
        json_stat_value(&json, "sim.cpu0.o3.rob_allocations"),
        instructions
    );
    assert_eq!(
        json_stat_value(&json, "sim.cpu0.o3.rob_commits"),
        instructions
    );
    assert!(json_stat_value(&json, "sim.cpu0.o3.rename_writes") > 0);
    assert_eq!(json_stat_value(&json, "sim.cpu0.o3.lsq_loads"), 1);
    assert_eq!(json_stat_value(&json, "sim.cpu0.o3.lsq_stores"), 1);
    assert!(json_stat_value(&json, "sim.cpu0.o3.fu_integer_mul_instructions") >= 1);
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/lsq_loads")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/lsq_stores")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/fu_integer_mul_instructions")
            .and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn rem6_run_o3_debug_trace_stats_include_initial_detailed_mode_authority() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &o3_start_mode_program());
    let path = temp_binary("o3-start-detailed-mode-debug-trace", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--debug-flags",
            "O3",
            "--riscv-execution-mode",
            "detailed",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/host_actions/execution_mode_switch_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_execution_mode_authority(&json, "detailed");
    assert_eq!(
        json.pointer("/debug/o3_trace/0/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        json.pointer("/debug/o3_trace/0/execution_mode")
            .and_then(Value::as_str),
        Some("detailed")
    );
    assert_eq!(
        json_stat_value(&json, "sim.debug.o3_trace.execution_mode.functional"),
        0
    );
    assert_eq!(
        json_stat_value(&json, "sim.debug.o3_trace.execution_mode.timing"),
        0
    );
    assert_eq!(
        json_stat_value(&json, "sim.debug.o3_trace.execution_mode.detailed"),
        1
    );
    assert_eq!(
        json_stat_value(
            &json,
            "sim.debug.o3_trace.cpu.cpu0.execution_mode.functional"
        ),
        0
    );
    assert_eq!(
        json_stat_value(&json, "sim.debug.o3_trace.cpu.cpu0.execution_mode.timing"),
        0
    );
    assert_eq!(
        json_stat_value(&json, "sim.debug.o3_trace.cpu.cpu0.execution_mode.detailed"),
        1
    );
}

#[test]
fn rem6_run_initial_timing_mode_executes_without_o3_runtime_records() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &o3_start_mode_program());
    let path = temp_binary("o3-start-timing-mode", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-execution-mode",
            "timing",
            "--dump-memory",
            "0x80000020:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("050000003c000000")
    );
    assert!(stdout.contains("\"x7\":\"0x3c\""));
    assert_eq!(
        json.pointer("/host_actions/execution_mode_switch_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_execution_mode_authority(&json, "timing");
    assert_eq!(
        json_stat_value(
            &json,
            "sim.host_actions.execution_mode_authority.mode.timing"
        ),
        1
    );
    assert_eq!(
        json_stat_value(
            &json,
            "sim.host_actions.execution_mode_authority.target.cpu0.mode.timing",
        ),
        1
    );
    assert_eq!(
        json_stat_value(&json, "sim.host_actions.execution_mode_switches"),
        0
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.instructions");
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
}

fn o3_start_mode_program() -> Vec<u8> {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),               // auipc x2, 0
        i_type(32, 2, 0x0, 2, 0x13),      // addi x2, x2, data offset
        i_type(0, 2, 0x2, 5, 0x03),       // lw x5, 0(x2)
        i_type(7, 5, 0x0, 6, 0x13),       // addi x6, x5, 7
        r_type(0x01, 5, 6, 0x0, 7, 0x33), // mul x7, x6, x5
        s_type(4, 7, 2, 0x2),             // sw x7, 4(x2)
        0x0000_0073,                      // ecall
        0x0000_0013,                      // data alignment padding
    ]);
    program.extend_from_slice(&5u32.to_le_bytes());
    program.extend_from_slice(&0u32.to_le_bytes());
    program
}

fn assert_execution_mode_authority(json: &Value, mode: &str) {
    let modes = json
        .pointer("/host_actions/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution mode authority: {json}"));
    let cpu0 = modes
        .iter()
        .find(|entry| entry.pointer("/target").and_then(Value::as_str) == Some("cpu0"))
        .unwrap_or_else(|| panic!("missing cpu0 execution mode authority: {modes:?}"));
    assert_eq!(cpu0.pointer("/mode").and_then(Value::as_str), Some(mode));
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn json_stat_value(json: &Value, path: &str) -> u64 {
    json.pointer("/stats")
        .and_then(Value::as_array)
        .and_then(|stats| {
            stats
                .iter()
                .find(|sample| sample.pointer("/path").and_then(Value::as_str) == Some(path))
        })
        .and_then(|sample| sample.pointer("/value").and_then(Value::as_u64))
        .unwrap_or_else(|| panic!("missing JSON stat value {path}: {json}"))
}

fn assert_json_stat_absent(json: &Value, path: &str) {
    let stats = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing stats array: {json}"));
    assert!(
        stats
            .iter()
            .all(|sample| sample.pointer("/path").and_then(Value::as_str) != Some(path)),
        "unexpected JSON stat value {path}: {json}"
    );
}
