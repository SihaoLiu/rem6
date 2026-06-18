use std::{fs, process::Command};

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_raw_process_group_session_syscalls() {
    let program = riscv64_program(&[
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        i_type(0, 10, 0x0, 13, 0x13),  // addi a3, a0, 0
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(155, 0, 0x0, 17, 0x13), // addi a7, x0, getpgid
        0x0000_0073,                   // ecall
        b_type(100, 13, 10, 0x1),      // bne a0, a3, fail
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),   // addi a1, x0, 0
        i_type(154, 0, 0x0, 17, 0x13), // addi a7, x0, setpgid
        0x0000_0073,                   // ecall
        i_type(-1, 0, 0x0, 14, 0x13),  // addi a4, x0, -1
        b_type(80, 14, 10, 0x1),       // bne a0, a4, fail
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(155, 0, 0x0, 17, 0x13), // addi a7, x0, getpgid
        0x0000_0073,                   // ecall
        b_type(60, 13, 10, 0x1),       // bne a0, a3, fail
        i_type(157, 0, 0x0, 17, 0x13), // addi a7, x0, setsid
        0x0000_0073,                   // ecall
        i_type(-1, 0, 0x0, 14, 0x13),  // addi a4, x0, -1
        b_type(48, 14, 10, 0x1),       // bne a0, a4, fail
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(155, 0, 0x0, 17, 0x13), // addi a7, x0, getpgid
        0x0000_0073,                   // ecall
        b_type(32, 13, 10, 0x1),       // bne a0, a3, fail
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(156, 0, 0x0, 17, 0x13), // addi a7, x0, getsid
        0x0000_0073,                   // ecall
        b_type(16, 13, 10, 0x1),       // bne a0, a3, fail
        i_type(63, 0, 0x0, 10, 0x13),  // addi a0, x0, 63
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(64, 0, 0x0, 10, 0x13),  // addi a0, x0, 64
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-process-group-session", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "340",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":63"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 63, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_resuid_resgid_syscalls() {
    let program = riscv64_program(&[
        i_type(-64, 2, 0x0, 2, 0x13),  // addi sp, sp, -64
        i_type(0, 2, 0x0, 10, 0x13),   // addi a0, sp, 0
        i_type(4, 2, 0x0, 11, 0x13),   // addi a1, sp, 4
        i_type(8, 2, 0x0, 12, 0x13),   // addi a2, sp, 8
        i_type(148, 0, 0x0, 17, 0x13), // addi a7, x0, getresuid
        0x0000_0073,                   // ecall
        b_type(92, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(100, 0, 0x0, 5, 0x13),  // addi x5, x0, 100
        i_type(0, 2, 0x2, 6, 0x03),    // lw x6, 0(sp)
        b_type(80, 5, 6, 0x1),         // bne x6, x5, fail
        i_type(4, 2, 0x2, 7, 0x03),    // lw x7, 4(sp)
        b_type(72, 5, 7, 0x1),         // bne x7, x5, fail
        i_type(8, 2, 0x2, 28, 0x03),   // lw x28, 8(sp)
        b_type(64, 5, 28, 0x1),        // bne x28, x5, fail
        i_type(16, 2, 0x0, 10, 0x13),  // addi a0, sp, 16
        i_type(20, 2, 0x0, 11, 0x13),  // addi a1, sp, 20
        i_type(24, 2, 0x0, 12, 0x13),  // addi a2, sp, 24
        i_type(150, 0, 0x0, 17, 0x13), // addi a7, x0, getresgid
        0x0000_0073,                   // ecall
        b_type(40, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(16, 2, 0x2, 6, 0x03),   // lw x6, 16(sp)
        b_type(32, 5, 6, 0x1),         // bne x6, x5, fail
        i_type(20, 2, 0x2, 7, 0x03),   // lw x7, 20(sp)
        b_type(24, 5, 7, 0x1),         // bne x7, x5, fail
        i_type(24, 2, 0x2, 28, 0x03),  // lw x28, 24(sp)
        b_type(16, 5, 28, 0x1),        // bne x28, x5, fail
        i_type(69, 0, 0x0, 10, 0x13),  // addi a0, x0, 69
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(70, 0, 0x0, 10, 0x13),  // addi a0, x0, 70
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-resuid-resgid", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":69"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 69, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_setres_identity_syscalls() {
    let program = riscv64_program(&[
        i_type(-64, 2, 0x0, 2, 0x13),  // addi sp, sp, -64
        i_type(100, 0, 0x0, 5, 0x13),  // addi x5, x0, 100
        i_type(-1, 0, 0x0, 29, 0x13),  // addi x29, x0, -1
        i_type(0, 29, 0x0, 10, 0x13),  // addi a0, x29, 0
        i_type(0, 5, 0x0, 11, 0x13),   // addi a1, x5, 0
        i_type(0, 29, 0x0, 12, 0x13),  // addi a2, x29, 0
        i_type(147, 0, 0x0, 17, 0x13), // addi a7, x0, setresuid
        0x0000_0073,                   // ecall
        b_type(160, 0, 10, 0x1),       // bne a0, x0, fail
        i_type(0, 29, 0x0, 10, 0x13),  // addi a0, x29, 0
        i_type(0, 5, 0x0, 11, 0x13),   // addi a1, x5, 0
        i_type(0, 29, 0x0, 12, 0x13),  // addi a2, x29, 0
        i_type(149, 0, 0x0, 17, 0x13), // addi a7, x0, setresgid
        0x0000_0073,                   // ecall
        b_type(136, 0, 10, 0x1),       // bne a0, x0, fail
        i_type(101, 0, 0x0, 10, 0x13), // addi a0, x0, 101
        i_type(0, 29, 0x0, 11, 0x13),  // addi a1, x29, 0
        i_type(0, 29, 0x0, 12, 0x13),  // addi a2, x29, 0
        i_type(147, 0, 0x0, 17, 0x13), // addi a7, x0, setresuid
        0x0000_0073,                   // ecall
        b_type(112, 29, 10, 0x1),      // bne a0, x29, fail
        i_type(0, 2, 0x0, 10, 0x13),   // addi a0, sp, 0
        i_type(4, 2, 0x0, 11, 0x13),   // addi a1, sp, 4
        i_type(8, 2, 0x0, 12, 0x13),   // addi a2, sp, 8
        i_type(148, 0, 0x0, 17, 0x13), // addi a7, x0, getresuid
        0x0000_0073,                   // ecall
        b_type(88, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(0, 2, 0x2, 6, 0x03),    // lw x6, 0(sp)
        b_type(80, 5, 6, 0x1),         // bne x6, x5, fail
        i_type(4, 2, 0x2, 7, 0x03),    // lw x7, 4(sp)
        b_type(72, 5, 7, 0x1),         // bne x7, x5, fail
        i_type(8, 2, 0x2, 28, 0x03),   // lw x28, 8(sp)
        b_type(64, 5, 28, 0x1),        // bne x28, x5, fail
        i_type(16, 2, 0x0, 10, 0x13),  // addi a0, sp, 16
        i_type(20, 2, 0x0, 11, 0x13),  // addi a1, sp, 20
        i_type(24, 2, 0x0, 12, 0x13),  // addi a2, sp, 24
        i_type(150, 0, 0x0, 17, 0x13), // addi a7, x0, getresgid
        0x0000_0073,                   // ecall
        b_type(40, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(16, 2, 0x2, 6, 0x03),   // lw x6, 16(sp)
        b_type(32, 5, 6, 0x1),         // bne x6, x5, fail
        i_type(20, 2, 0x2, 7, 0x03),   // lw x7, 20(sp)
        b_type(24, 5, 7, 0x1),         // bne x7, x5, fail
        i_type(24, 2, 0x2, 28, 0x03),  // lw x28, 24(sp)
        b_type(16, 5, 28, 0x1),        // bne x28, x5, fail
        i_type(71, 0, 0x0, 10, 0x13),  // addi a0, x0, 71
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(72, 0, 0x0, 10, 0x13),  // addi a0, x0, 72
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-setres-identity", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "520",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":71"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 71, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_setre_identity_syscalls_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw setre identity smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw setre identity smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-setre-identity");
    let source = workspace.join("raw-setre-identity.c");
    let binary = workspace.join("raw-setre-identity");
    fs::write(
        &source,
        r#"#include <stdio.h>

static long linux_syscall0(long number) {
    register long a7 asm("a7") = number;
    register long a0 asm("a0");
    asm volatile ("ecall" : "=r"(a0) : "r"(a7) : "memory");
    return a0;
}

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    long uid = linux_syscall0(174);
    long gid = linux_syscall0(176);
    long setreuid_same = linux_syscall2(145, -1, uid);
    long setregid_same = linux_syscall2(143, -1, gid);
    long euid = linux_syscall0(175);
    long egid = linux_syscall0(177);
    int ok = uid >= 0 && gid >= 0 &&
             setreuid_same == 0 && setregid_same == 0 &&
             euid == uid && egid == gid;
    printf("raw-setre-identity:%ld:%ld:%d\n",
           setreuid_same, setregid_same, ok);
    return ok ? 74 : 75;
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "gcc stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(74),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-setre-identity:0:0:1\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "220000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":74"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-setre-identity:0:0:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 74, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_group_list_syscalls() {
    let program = riscv64_program(&[
        i_type(-16, 2, 0x0, 2, 0x13),  // addi sp, sp, -16
        i_type(77, 0, 0x0, 5, 0x13),   // addi x5, x0, 77
        s_type(0, 5, 2, 0x2),          // sw x5, 0(sp)
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),   // addi a1, x0, 0
        i_type(158, 0, 0x0, 17, 0x13), // addi a7, x0, getgroups
        0x0000_0073,                   // ecall
        b_type(68, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(1, 0, 0x0, 10, 0x13),   // addi a0, x0, 1
        i_type(0, 2, 0x0, 11, 0x13),   // addi a1, sp, 0
        i_type(158, 0, 0x0, 17, 0x13), // addi a7, x0, getgroups
        0x0000_0073,                   // ecall
        b_type(48, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(0, 2, 0x2, 6, 0x03),    // lw x6, 0(sp)
        b_type(40, 5, 6, 0x1),         // bne x6, x5, fail
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(0, 0, 0x0, 11, 0x13),   // addi a1, x0, 0
        i_type(159, 0, 0x0, 17, 0x13), // addi a7, x0, setgroups
        0x0000_0073,                   // ecall
        i_type(-1, 0, 0x0, 29, 0x13),  // addi x29, x0, -1
        b_type(16, 29, 10, 0x1),       // bne a0, x29, fail
        i_type(73, 0, 0x0, 10, 0x13),  // addi a0, x0, 73
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(74, 0, 0x0, 10, 0x13),  // addi a0, x0, 74
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-group-list", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "340",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":73"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 73, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_waitid_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw waitid smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw waitid smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-waitid");
    let source = workspace.join("raw-waitid.S");
    let binary = workspace.join("raw-waitid");
    fs::write(
        &source,
        r#".section .bss
.balign 8
info:
    .space 128

.section .text
.global _start
_start:
    la t0, info
    li t1, 0xa5a5a5a5a5a5a5a5
    li t2, 16
fill_info:
    sd t1, 0(t0)
    addi t0, t0, 8
    addi t2, t2, -1
    bnez t2, fill_info

    li a0, 0
    li a1, 0
    la a2, info
    li a3, 5
    li a4, 0
    li a7, 95
    ecall
    li t0, -10
    bne a0, t0, fail

    la t1, info
    li t0, -1515870811
    lw t2, 0(t1)
    bne t2, t0, fail
    lw t2, 4(t1)
    bne t2, t0, fail
    lw t2, 8(t1)
    bne t2, t0, fail

    li a0, 0
    li a1, 0
    la a2, info
    li a3, 0
    li a4, 0
    li a7, 95
    ecall
    li t0, -22
    bne a0, t0, fail

    li a0, 0
    li a1, 0
    li a2, 0
    li a3, 5
    li a4, 0
    li a7, 95
    ecall
    li t0, -10
    bne a0, t0, fail

    li a0, 99
    li a1, 0
    la a2, info
    li a3, 5
    li a4, 0
    li a7, 95
    ecall
    li t0, -22
    bne a0, t0, fail

    li a0, 0
    li a1, 0
    la a2, info
    li a3, 5
    li a4, 1
    li a7, 95
    ecall
    li t0, -10
    bne a0, t0, fail

    li a0, 81
    li a7, 93
    ecall

fail:
    li a0, 82
    li a7, 93
    ecall
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-nostdlib",
            "-nostartfiles",
            "-O1",
            "-static",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "gcc stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(81),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert!(qemu_output.stdout.is_empty());

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "1200",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":81"));
    assert!(stdout.contains("\"riscv_guest_writes\":[]"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 81, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_priority_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw priority smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw priority smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-priority");
    let source = workspace.join("raw-priority.c");
    let binary = workspace.join("raw-priority");
    fs::write(
        &source,
        r#"#include <stdio.h>

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall3(long number, long arg0, long arg1, long arg2) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    long set_high = linux_syscall3(140, 0, 0, 40);
    long after_high = linux_syscall2(141, 0, 0);
    long bad_which_get = linux_syscall2(141, 3, 0);
    long bad_which_set = linux_syscall3(140, 3, 0, 0);

    printf("raw-priority:%ld:%ld:%ld:%ld\n",
           set_high, after_high, bad_which_get, bad_which_set);
    return set_high == 0 &&
           after_high == 1 &&
           bad_which_get == -22 &&
           bad_which_set == -22 ? 77 : 78;
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "gcc stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(77),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-priority:0:1:-22:-22\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":77"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-priority:0:1:-22:-22\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 77, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_sched_setters_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw scheduler setter smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw scheduler setter smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-scheduler-setter");
    let source = workspace.join("raw-scheduler-setter.c");
    let binary = workspace.join("raw-scheduler-setter");
    fs::write(
        &source,
        r#"#include <stdio.h>

struct sched_param {
    int sched_priority;
};

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall3(long number, long arg0, long arg1, long arg2) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    struct sched_param zero = {0};
    struct sched_param one = {1};
    long set_batch = linux_syscall3(119, 0, 3, (long)&zero);
    long get_batch = linux_syscall2(120, 0, 0);
    long set_param = linux_syscall2(118, 0, (long)&zero);
    long bad_getparam = linux_syscall2(121, 999, (long)&zero);
    long bad_priority = linux_syscall3(119, 0, 0, (long)&one);
    long bad_policy = linux_syscall3(119, 0, 4, (long)&zero);

    printf("raw-scheduler-setter:%ld:%ld:%ld:%ld:%ld:%ld\n",
           set_batch, get_batch, set_param, bad_getparam, bad_priority, bad_policy);
    return set_batch == 0 &&
           get_batch == 3 &&
           set_param == 0 &&
           bad_getparam == -3 &&
           bad_priority == -22 &&
           bad_policy == -22 ? 78 : 79;
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "gcc stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(78),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(
        qemu_output.stdout,
        b"raw-scheduler-setter:0:3:0:-3:-22:-22\n"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":78"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-scheduler-setter:0:3:0:-3:-22:-22\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 78, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_futex_wake_op_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw futex wake-op smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw futex wake-op smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-futex-wake-op");
    let source = workspace.join("raw-futex-wake-op.c");
    let binary = workspace.join("raw-futex-wake-op");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define FUTEX_WAKE_OP 5
#define FUTEX_OP_ADD 1
#define FUTEX_OP_CMP_EQ 0
#define FUTEX_OP(op, oparg, cmp, cmparg) \
    (((op) << 28) | ((cmp) << 24) | ((oparg) << 12) | (cmparg))

static long linux_syscall6(long number,
                           long arg0,
                           long arg1,
                           long arg2,
                           long arg3,
                           long arg4,
                           long arg5) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a5 asm("a5") = arg5;
    register long a7 asm("a7") = number;
    asm volatile("ecall"
                 : "+r"(a0)
                 : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(a7)
                 : "memory");
    return a0;
}

int main(void) {
    int source = 1;
    int target = 7;
    long result = linux_syscall6(98,
                                 (long)&source,
                                 FUTEX_WAKE_OP,
                                 0,
                                 0,
                                 (long)&target,
                                 FUTEX_OP(FUTEX_OP_ADD, 3, FUTEX_OP_CMP_EQ, 7));

    printf("raw-futex-wake-op:%ld:%d:%d\n", result, source, target);
    return result == 0 && source == 1 && target == 10 ? 79 : 80;
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "gcc stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(79),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-futex-wake-op:0:1:10\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":79"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-futex-wake-op:0:1:10\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 79, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_set_identity_syscalls() {
    let program = riscv64_program(&[
        i_type(-64, 2, 0x0, 2, 0x13),  // addi sp, sp, -64
        i_type(100, 0, 0x0, 5, 0x13),  // addi x5, x0, 100
        i_type(-1, 0, 0x0, 29, 0x13),  // addi x29, x0, -1
        i_type(0, 5, 0x0, 10, 0x13),   // addi a0, x5, 0
        i_type(146, 0, 0x0, 17, 0x13), // addi a7, x0, setuid
        0x0000_0073,                   // ecall
        b_type(160, 0, 10, 0x1),       // bne a0, x0, fail
        i_type(0, 5, 0x0, 10, 0x13),   // addi a0, x5, 0
        i_type(144, 0, 0x0, 17, 0x13), // addi a7, x0, setgid
        0x0000_0073,                   // ecall
        b_type(144, 0, 10, 0x1),       // bne a0, x0, fail
        i_type(101, 0, 0x0, 10, 0x13), // addi a0, x0, 101
        i_type(146, 0, 0x0, 17, 0x13), // addi a7, x0, setuid
        0x0000_0073,                   // ecall
        b_type(128, 29, 10, 0x1),      // bne a0, x29, fail
        i_type(101, 0, 0x0, 10, 0x13), // addi a0, x0, 101
        i_type(144, 0, 0x0, 17, 0x13), // addi a7, x0, setgid
        0x0000_0073,                   // ecall
        b_type(112, 29, 10, 0x1),      // bne a0, x29, fail
        i_type(0, 2, 0x0, 10, 0x13),   // addi a0, sp, 0
        i_type(4, 2, 0x0, 11, 0x13),   // addi a1, sp, 4
        i_type(8, 2, 0x0, 12, 0x13),   // addi a2, sp, 8
        i_type(148, 0, 0x0, 17, 0x13), // addi a7, x0, getresuid
        0x0000_0073,                   // ecall
        b_type(88, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(0, 2, 0x2, 6, 0x03),    // lw x6, 0(sp)
        b_type(80, 5, 6, 0x1),         // bne x6, x5, fail
        i_type(4, 2, 0x2, 7, 0x03),    // lw x7, 4(sp)
        b_type(72, 5, 7, 0x1),         // bne x7, x5, fail
        i_type(8, 2, 0x2, 28, 0x03),   // lw x28, 8(sp)
        b_type(64, 5, 28, 0x1),        // bne x28, x5, fail
        i_type(16, 2, 0x0, 10, 0x13),  // addi a0, sp, 16
        i_type(20, 2, 0x0, 11, 0x13),  // addi a1, sp, 20
        i_type(24, 2, 0x0, 12, 0x13),  // addi a2, sp, 24
        i_type(150, 0, 0x0, 17, 0x13), // addi a7, x0, getresgid
        0x0000_0073,                   // ecall
        b_type(40, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(16, 2, 0x2, 6, 0x03),   // lw x6, 16(sp)
        b_type(32, 5, 6, 0x1),         // bne x6, x5, fail
        i_type(20, 2, 0x2, 7, 0x03),   // lw x7, 20(sp)
        b_type(24, 5, 7, 0x1),         // bne x7, x5, fail
        i_type(24, 2, 0x2, 28, 0x03),  // lw x28, 24(sp)
        b_type(16, 5, 28, 0x1),        // bne x28, x5, fail
        i_type(75, 0, 0x0, 10, 0x13),  // addi a0, x0, 75
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(76, 0, 0x0, 10, 0x13),  // addi a0, x0, 76
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-set-identity", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "560",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":75"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 75, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_personality_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw personality smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw personality smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-personality");
    let source = workspace.join("raw-personality.c");
    let binary = workspace.join("raw-personality");
    fs::write(
        &source,
        r#"#include <stdio.h>

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

int main(void) {
    long query0 = linux_syscall1(92, 0xffffffffL);
    long set = linux_syscall1(92, 0x0040000L);
    long query1 = linux_syscall1(92, 0xffffffffL);
    long clear = linux_syscall1(92, 0L);
    long query2 = linux_syscall1(92, 0xffffffffL);
    printf("raw-personality:%ld:%ld:%ld:%ld:%ld\n",
           query0, set, query1, clear, query2);
    return query0 == 0 && set == 0 && query1 == 0x0040000L &&
           clear == 0x0040000L && query2 == 0 ? 67 : 68;
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "gcc stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(67),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-personality:0:0:262144:262144:0\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":67"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-personality:0:0:262144:262144:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
