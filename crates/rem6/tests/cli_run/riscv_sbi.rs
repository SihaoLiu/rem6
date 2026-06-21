use std::process::Command;

use crate::support::*;

const SBI_BASE_GET_SPEC_VERSION: i32 = 0;
const SBI_BASE_EXTENSION: i32 = 0x10;
const SBI_SPEC_VERSION_2_0: u64 = 2 << 24;

#[test]
fn rem6_run_riscv_sbi_handles_supervisor_base_ecall() {
    let program = riscv64_program(&[
        i_type(SBI_BASE_EXTENSION, 0, 0x0, 17, 0x13),
        i_type(SBI_BASE_GET_SPEC_VERSION, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0, 11, 0x0, 6, 0x13),
        b_type(12, 0, 10, 0x1),
        i_type(1, 0, 0x0, 5, 0x13),
        0x0010_0073,
        i_type(2, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-sbi-base", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-sbi",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(
        stdout.contains("\"riscv_boot\":{\"a0\":\"0x0\",\"a1\":\"0x0\",\"sbi\":true,\"se\":false}")
    );
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains(&format!("\"x6\":\"0x{:x}\"", SBI_SPEC_VERSION_2_0)));
    assert_stat(&stdout, "sim.riscv.sbi", "Count", 1, "constant");
}

#[test]
fn rem6_run_rejects_riscv_sbi_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-sbi-without-execute", &elf);

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
            "--riscv-sbi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-sbi requires --execute"));
}

#[test]
fn rem6_run_rejects_riscv_sbi_without_riscv_isa() {
    let elf = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);
    let path = temp_binary("riscv-sbi-without-riscv", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "x86",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-format",
            "json",
            "--riscv-sbi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-sbi requires --isa riscv"));
}

#[test]
fn rem6_run_rejects_riscv_sbi_with_riscv_se() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-sbi-with-riscv-se", &elf);

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
            "--riscv-sbi",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-sbi cannot be combined with --riscv-se"));
}

#[test]
fn rem6_run_rejects_riscv_sbi_with_explicit_boot_a0() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-sbi-with-boot-a0", &elf);

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
            "--riscv-sbi",
            "--riscv-boot-a0",
            "7",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-sbi requires --riscv-boot-a0 0"));
}
