use std::process::Command;

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_checker_cpu_covers_initial_timing_and_detailed_modes() {
    for mode in ["timing", "detailed"] {
        let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &checker_cpu_mode_program());
        let path = temp_binary(&format!("checker-cpu-initial-{mode}"), &elf);

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
                "--checker-cpu",
                "--riscv-execution-mode",
                mode,
                "--dump-memory",
                "0x80000020:4",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "mode {mode} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: Value = serde_json::from_str(&stdout).unwrap();

        assert_eq!(
            json.pointer("/simulation/status").and_then(Value::as_str),
            Some("executed_until_trap"),
            "mode {mode} should run through the trap: {json}"
        );
        assert_eq!(
            json.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("0c000000"),
            "mode {mode} should publish the architectural store witness: {json}"
        );
        assert_eq!(
            json.pointer("/cores/0/registers/x5")
                .and_then(Value::as_str),
            Some("0x7"),
            "mode {mode} should publish the x5 register witness: {json}"
        );
        assert_eq!(
            json.pointer("/cores/0/registers/x6")
                .and_then(Value::as_str),
            Some("0xc"),
            "mode {mode} should publish the x6 register witness: {json}"
        );

        assert_execution_mode_authority(&json, mode);
        assert_eq!(
            json.pointer("/host_actions/execution_mode_switch_count")
                .and_then(Value::as_u64),
            Some(0),
            "mode {mode} should be an initial-mode row, not a switch row: {json}"
        );
        assert_eq!(
            json_stat_value(
                &json,
                &format!("sim.host_actions.execution_mode_authority.mode.{mode}")
            ),
            1,
            "mode {mode} should publish mode authority stats"
        );
        assert_eq!(
            json_stat_value(
                &json,
                &format!("sim.host_actions.execution_mode_authority.target.cpu0.mode.{mode}")
            ),
            1,
            "mode {mode} should publish CPU-scoped mode authority stats"
        );

        let checker = json
            .pointer("/cores/0/checker")
            .unwrap_or_else(|| panic!("missing checker summary for initial mode {mode}: {json}"));
        assert_eq!(
            checker.pointer("/execution_mode").and_then(Value::as_str),
            Some(mode),
            "mode {mode} should publish checker execution-mode authority in the core JSON: {checker}"
        );
        assert_eq!(
            checker
                .pointer("/checked_instructions")
                .and_then(Value::as_u64),
            Some(6),
            "mode {mode} should check all retired instructions: {checker}"
        );
        assert_eq!(
            checker.pointer("/mismatches").and_then(Value::as_u64),
            Some(0),
            "mode {mode} should preserve zero checker mismatches: {checker}"
        );
        assert_eq!(
            json_stat_value(&json, "sim.cpu0.checker.checked_instructions"),
            6,
            "mode {mode} should mirror final checker progress into stats"
        );
        assert_eq!(
            json_stat_value(&json, "sim.cpu0.checker.mismatches"),
            0,
            "mode {mode} should mirror zero checker mismatches into stats"
        );
        for candidate in ["functional", "timing", "detailed"] {
            let expected = if candidate == mode { 6 } else { 0 };
            assert_eq!(
                json_stat_value(
                    &json,
                    &format!("sim.cpu0.checker.execution_mode.{candidate}.checked_instructions")
                ),
                expected,
                "mode {mode} should publish checker progress in the {candidate} lane"
            );
            assert_eq!(
                json_stat_value(
                    &json,
                    &format!("sim.cpu0.checker.execution_mode.{candidate}.mismatches")
                ),
                0,
                "mode {mode} should publish checker mismatches in the {candidate} lane"
            );
        }
    }
}

#[test]
fn rem6_run_rejects_checker_cpu_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("checker-cpu-without-execute", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--checker-cpu",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--checker-cpu requires --execute"));
}

#[test]
fn rem6_run_rejects_checker_cpu_without_riscv_isa() {
    let elf = x86_64_elf(0x4000_0000, 0x4000_0000, &[0x90]);
    let path = temp_binary("checker-cpu-without-riscv", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "x86",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--checker-cpu",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--checker-cpu requires --isa riscv"));
}

fn checker_cpu_mode_program() -> Vec<u8> {
    let mut program = riscv64_program(&[
        u_type(0, 10, 0x17),           // auipc x10, 0
        i_type(32, 10, 0x0, 10, 0x13), // addi x10, x10, data offset
        i_type(7, 0, 0x0, 5, 0x13),    // addi x5, x0, 7
        i_type(5, 5, 0x0, 6, 0x13),    // addi x6, x5, 5
        s_type(0, 6, 10, 0x2),         // sw x6, 0(x10)
        0x0000_0073,                   // ecall
        0x0000_0013,                   // data alignment padding
        0x0000_0013,                   // data alignment padding
    ]);
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
