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
