use serde_json::Value;

use super::*;

const M5_EXIT: u32 = 0x21;
const M5_FAIL: u32 = 0x22;
const M5_SWITCH_CPU: u32 = 0x52;

#[test]
fn rem6_run_accepts_riscv_o3_memory_issue_width_cli_min_and_max() {
    for (issue_width, memory_width) in [(1, 1), (4, 4)] {
        let path = detailed_o3_memory_issue_width_binary(&format!(
            "riscv-o3-memory-issue-width-cli-{memory_width}"
        ));
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run",
                "--isa",
                "riscv",
                "--binary",
                path.to_str().unwrap(),
                "--max-tick",
                "120",
                "--stats-format",
                "json",
                "--execute",
                "--riscv-execution-mode",
                "detailed",
                "--riscv-o3-issue-width",
                &issue_width.to_string(),
                "--riscv-o3-memory-issue-width",
                &memory_width.to_string(),
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{memory_width} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_o3_width_json(&output.stdout, issue_width, memory_width);
    }
}

#[test]
fn rem6_run_accepts_riscv_o3_memory_issue_width_from_config() {
    let issue_width = 4;
    let memory_width = 4;
    let path = detailed_o3_memory_issue_width_binary("riscv-o3-memory-issue-width-config");
    let config = temp_output("riscv-o3-memory-issue-width-config.toml");
    std::fs::write(
        &config,
        format!(
            "[run]\nriscv_o3_issue_width = {issue_width}\nriscv_o3_memory_issue_width = {memory_width}\n"
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
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
    assert_o3_width_json(&output.stdout, issue_width, memory_width);
}

#[test]
fn rem6_run_cli_o3_memory_issue_width_overrides_config() {
    let issue_width = 4;
    let memory_width = 2;
    let path = detailed_o3_memory_issue_width_binary("riscv-o3-memory-issue-width-override");
    let config = temp_output("riscv-o3-memory-issue-width-override.toml");
    std::fs::write(
        &config,
        "[run]\nriscv_o3_issue_width = 1\nriscv_o3_memory_issue_width = 1\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-execution-mode",
            "detailed",
            "--riscv-o3-issue-width",
            &issue_width.to_string(),
            "--riscv-o3-memory-issue-width",
            &memory_width.to_string(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_o3_width_json(&output.stdout, issue_width, memory_width);
}

#[test]
fn rem6_run_rejects_invalid_riscv_o3_memory_issue_width_values() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-o3-memory-issue-width-invalid", &elf);

    for value in ["0", "5", "wide"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run",
                "--isa",
                "riscv",
                "--binary",
                path.to_str().unwrap(),
                "--max-tick",
                "40",
                "--execute",
                "--stats-format",
                "json",
                "--riscv-o3-memory-issue-width",
                value,
            ])
            .output()
            .unwrap();

        assert!(!output.status.success(), "{value}");
        assert!(output.stdout.is_empty(), "{value}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains(&format!("invalid RISC-V O3 memory issue width {value}")),
            "{value}: {stderr}"
        );
    }
}

#[test]
fn rem6_run_rejects_memory_issue_width_above_total_issue_width() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-o3-memory-issue-width-above-total", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-format",
            "json",
            "--riscv-o3-issue-width",
            "2",
            "--riscv-o3-memory-issue-width",
            "4",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr.trim(),
        "RISC-V O3 memory issue width 4 exceeds total issue width 2"
    );
}

#[test]
fn rem6_run_validates_o3_memory_issue_width_execution_and_riscv_requirements() {
    let riscv = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let x86 = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);

    for (name, isa, elf, execute, expected) in [
        (
            "riscv-o3-memory-issue-width-without-execute",
            "riscv",
            riscv.as_slice(),
            false,
            "--riscv-o3-memory-issue-width requires --execute",
        ),
        (
            "riscv-o3-memory-issue-width-without-riscv",
            "x86",
            x86.as_slice(),
            true,
            "--riscv-o3-memory-issue-width requires --isa riscv",
        ),
    ] {
        let path = temp_binary(name, elf);
        let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
        command.args([
            "run",
            "--isa",
            isa,
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--riscv-o3-memory-issue-width",
            "1",
        ]);
        if execute {
            command.arg("--execute");
        }

        let output = command.output().unwrap();

        assert!(!output.status.success(), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected), "{name}: {stderr}");
    }
}

#[test]
fn rem6_run_config_scan_treats_o3_memory_issue_width_as_value_taking() {
    let bogus_config = temp_output("riscv-o3-memory-issue-width-prescan-bogus-config");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--riscv-o3-memory-issue-width",
            "--config",
            bogus_config.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid RISC-V O3 memory issue width --config"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains(&format!("failed to read config {}", bogus_config.display())));
}

fn assert_o3_width_json(stdout: &[u8], issue_width: usize, memory_width: usize) {
    let json: Value = serde_json::from_slice(stdout).unwrap();
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/issue/configured_width")
            .and_then(Value::as_u64),
        Some(issue_width as u64)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/issue/configured_memory_width")
            .and_then(Value::as_u64),
        Some(memory_width as u64)
    );
}

fn detailed_o3_memory_issue_width_binary(name: &str) -> std::path::PathBuf {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(42, 0, 0x0, 1, 0x13),   // addi x1, x0, 42
        i_type(7, 0, 0x0, 2, 0x13),    // addi x2, x0, 7
        r_type(1, 2, 1, 0x4, 3, 0x33), // div x3, x1, x2
        i_type(77, 0, 0x0, 13, 0x13),  // addi x13, x0, 77
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
}
