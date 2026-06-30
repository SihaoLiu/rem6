use std::{fs, process::Command};

use crate::support::*;

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
fn rem6_run_riscv_se_runs_raw_priority_group_and_user_scopes() {
    let mut words = Vec::new();
    let mut branch_indices = Vec::new();
    let mut expect_return = |words: &mut Vec<u32>, expected: i32| {
        words.push(i_type(expected, 0, 0x0, 5, 0x13)); // addi t0, x0, expected
        branch_indices.push(words.len());
        words.push(0); // patched to bne a0, t0, fail
    };

    words.push(i_type(1, 0, 0x0, 10, 0x13)); // addi a0, x0, PRIO_PGRP
    words.push(i_type(0, 0, 0x0, 11, 0x13)); // addi a1, x0, current group
    words.push(i_type(141, 0, 0x0, 17, 0x13)); // addi a7, x0, getpriority
    words.push(0x0000_0073); // ecall
    expect_return(&mut words, 20);

    words.push(i_type(2, 0, 0x0, 10, 0x13)); // addi a0, x0, PRIO_USER
    words.push(i_type(0, 0, 0x0, 11, 0x13)); // addi a1, x0, current user
    words.push(i_type(141, 0, 0x0, 17, 0x13)); // addi a7, x0, getpriority
    words.push(0x0000_0073); // ecall
    expect_return(&mut words, 20);

    words.push(i_type(1, 0, 0x0, 10, 0x13)); // addi a0, x0, PRIO_PGRP
    words.push(i_type(0, 0, 0x0, 11, 0x13)); // addi a1, x0, current group
    words.push(i_type(9, 0, 0x0, 12, 0x13)); // addi a2, x0, nice
    words.push(i_type(140, 0, 0x0, 17, 0x13)); // addi a7, x0, setpriority
    words.push(0x0000_0073); // ecall
    expect_return(&mut words, 0);

    words.push(i_type(0, 0, 0x0, 10, 0x13)); // addi a0, x0, PRIO_PROCESS
    words.push(i_type(0, 0, 0x0, 11, 0x13)); // addi a1, x0, current process
    words.push(i_type(141, 0, 0x0, 17, 0x13)); // addi a7, x0, getpriority
    words.push(0x0000_0073); // ecall
    expect_return(&mut words, 11);

    words.push(i_type(2, 0, 0x0, 10, 0x13)); // addi a0, x0, PRIO_USER
    words.push(i_type(0, 0, 0x0, 11, 0x13)); // addi a1, x0, current user
    words.push(i_type(12, 0, 0x0, 12, 0x13)); // addi a2, x0, nice
    words.push(i_type(140, 0, 0x0, 17, 0x13)); // addi a7, x0, setpriority
    words.push(0x0000_0073); // ecall
    expect_return(&mut words, 0);

    words.push(i_type(0, 0, 0x0, 10, 0x13)); // addi a0, x0, PRIO_PROCESS
    words.push(i_type(0, 0, 0x0, 11, 0x13)); // addi a1, x0, current process
    words.push(i_type(141, 0, 0x0, 17, 0x13)); // addi a7, x0, getpriority
    words.push(0x0000_0073); // ecall
    expect_return(&mut words, 8);

    words.push(i_type(1, 0, 0x0, 10, 0x13)); // addi a0, x0, PRIO_PGRP
    words.push(i_type(999, 0, 0x0, 11, 0x13)); // addi a1, x0, missing group
    words.push(i_type(141, 0, 0x0, 17, 0x13)); // addi a7, x0, getpriority
    words.push(0x0000_0073); // ecall
    expect_return(&mut words, -3);

    words.push(i_type(2, 0, 0x0, 10, 0x13)); // addi a0, x0, PRIO_USER
    words.push(i_type(999, 0, 0x0, 11, 0x13)); // addi a1, x0, missing user
    words.push(i_type(140, 0, 0x0, 17, 0x13)); // addi a7, x0, setpriority
    words.push(0x0000_0073); // ecall
    expect_return(&mut words, -3);

    words.push(i_type(94, 0, 0x0, 10, 0x13)); // addi a0, x0, pass
    words.push(i_type(93, 0, 0x0, 17, 0x13)); // addi a7, x0, exit
    words.push(0x0000_0073); // ecall

    let fail_index = words.len();
    words.push(i_type(95, 0, 0x0, 10, 0x13)); // addi a0, x0, fail
    words.push(i_type(93, 0, 0x0, 17, 0x13)); // addi a7, x0, exit
    words.push(0x0000_0073); // ecall

    let fail_pc = i32::try_from(fail_index * 4).unwrap();
    for branch_index in branch_indices {
        let branch_pc = i32::try_from(branch_index * 4).unwrap();
        words[branch_index] = b_type(fail_pc - branch_pc, 5, 10, 0x1);
    }

    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-priority-group-user", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "900",
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
    assert!(stdout.contains("\"stop_code\":94"), "stdout: {stdout}");
    assert!(stdout.contains("\"riscv_guest_writes\":[]"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 94, "constant");
}
