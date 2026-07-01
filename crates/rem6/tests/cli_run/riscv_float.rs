use std::process::Command;

use crate::support::*;

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn fmv_d_x(rs1: u8, rd: u8) -> u32 {
    r_type(0x79, 0, rs1, 0x0, rd, 0x53)
}

fn fmul_d_round_up(rs2: u8, rs1: u8, rd: u8) -> u32 {
    r_type(0x09, rs2, rs1, 0x3, rd, 0x53)
}

fn fadd_d_round_up(rs2: u8, rs1: u8, rd: u8) -> u32 {
    r_type(0x01, rs2, rs1, 0x3, rd, 0x53)
}

fn fsqrt_d_round_up(rs1: u8, rd: u8) -> u32 {
    r_type(0x2d, 0, rs1, 0x3, rd, 0x53)
}

fn fmv_x_d(rs1: u8, rd: u8) -> u32 {
    r_type(0x71, 0, rs1, 0x0, rd, 0x53)
}

#[test]
fn rem6_run_executes_rv64d_fmul_directed_rounding_from_elf() {
    let mut program = riscv64_program(&[
        u_type(0, 10, 0x17),           // auipc a0, 0
        i_type(40, 10, 0x3, 11, 0x03), // ld a1, 40(a0)
        i_type(48, 10, 0x3, 12, 0x03), // ld a2, 48(a0)
        fmv_d_x(11, 1),
        fmv_d_x(12, 2),
        fmul_d_round_up(2, 1, 3),
        fmv_x_d(3, 5),
        csr_read(0x001, 6),
        0x0010_0073, // ebreak
        0x0000_0013, // addi x0, x0, 0
    ]);
    program.extend_from_slice(&0x3ff0_0000_0000_0001_u64.to_le_bytes());
    program.extend_from_slice(&0x3ff0_0000_0000_0001_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("rv64d-fmul-round-up", &elf);

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
            "--memory-system",
            "direct",
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
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x3ff0000000000003\""));
    assert!(stdout.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_executes_rv64d_fadd_directed_rounding_from_elf() {
    let mut program = riscv64_program(&[
        u_type(0, 10, 0x17),           // auipc a0, 0
        i_type(40, 10, 0x3, 11, 0x03), // ld a1, 40(a0)
        i_type(48, 10, 0x3, 12, 0x03), // ld a2, 48(a0)
        fmv_d_x(11, 1),
        fmv_d_x(12, 2),
        fadd_d_round_up(2, 1, 3),
        fmv_x_d(3, 5),
        csr_read(0x001, 6),
        0x0010_0073, // ebreak
        0x0000_0013, // addi x0, x0, 0
    ]);
    program.extend_from_slice(&1.0f64.to_bits().to_le_bytes());
    program.extend_from_slice(&0x3ca0_0000_0000_0000_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("rv64d-fadd-round-up", &elf);

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
            "--memory-system",
            "direct",
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
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x3ff0000000000001\""));
    assert!(stdout.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_executes_rv64d_fmul_directed_overflow_from_elf() {
    let mut program = riscv64_program(&[
        u_type(0, 10, 0x17),           // auipc a0, 0
        i_type(40, 10, 0x3, 11, 0x03), // ld a1, 40(a0)
        i_type(48, 10, 0x3, 12, 0x03), // ld a2, 48(a0)
        fmv_d_x(11, 1),
        fmv_d_x(12, 2),
        fmul_d_round_up(2, 1, 3),
        fmv_x_d(3, 5),
        csr_read(0x001, 6),
        0x0010_0073, // ebreak
        0x0000_0013, // addi x0, x0, 0
    ]);
    program.extend_from_slice(&0x3ff0_0000_0262_5a00_u64.to_le_bytes());
    program.extend_from_slice(&0x7fef_ffff_fb3b_4c00_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("rv64d-fmul-overflow-rup", &elf);

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
            "--memory-system",
            "direct",
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
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x7ff0000000000000\""));
    assert!(stdout.contains("\"x6\":\"0x5\""));
}

#[test]
fn rem6_run_executes_rv64d_fmul_directed_underflow_from_elf() {
    let mut program = riscv64_program(&[
        u_type(0, 10, 0x17),           // auipc a0, 0
        i_type(40, 10, 0x3, 11, 0x03), // ld a1, 40(a0)
        i_type(48, 10, 0x3, 12, 0x03), // ld a2, 48(a0)
        fmv_d_x(11, 1),
        fmv_d_x(12, 2),
        fmul_d_round_up(2, 1, 3),
        fmv_x_d(3, 5),
        csr_read(0x001, 6),
        0x0010_0073, // ebreak
        0x0000_0013, // addi x0, x0, 0
    ]);
    program.extend_from_slice(&f64::MIN_POSITIVE.to_bits().to_le_bytes());
    program.extend_from_slice(&0x3fe0_0000_0000_0001_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("rv64d-fmul-underflow-rup", &elf);

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
            "--memory-system",
            "direct",
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
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x8000000000001\""));
    assert!(stdout.contains("\"x6\":\"0x3\""));
}

#[test]
fn rem6_run_executes_rv64d_fsqrt_directed_rounding_from_elf() {
    let mut program = riscv64_program(&[
        u_type(0, 10, 0x17),           // auipc a0, 0
        i_type(32, 10, 0x3, 11, 0x03), // ld a1, 32(a0)
        fmv_d_x(11, 1),
        fsqrt_d_round_up(1, 2),
        fmv_x_d(2, 5),
        csr_read(0x001, 6),
        0x0010_0073, // ebreak
        0x0000_0013, // addi x0, x0, 0
    ]);
    program.extend_from_slice(&3.0f64.to_bits().to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("rv64d-fsqrt-round-up", &elf);

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
            "--memory-system",
            "direct",
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
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x3ffbb67ae8584cab\""));
    assert!(stdout.contains("\"x6\":\"0x1\""));
}
