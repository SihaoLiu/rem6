use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_renameat_with_qemu_probe() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw renameat smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw renameat smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-renameat");
    let source = workspace.join("raw-renameat.c");
    let binary = workspace.join("raw-renameat");
    let input = workspace.join("guest.txt");
    fs::write(&input, b"file-backed input\n").unwrap();
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall5(long number, long arg0, long arg1, long arg2, long arg3, long arg4) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    long renamed = linux_syscall5(38, AT_FDCWD, (long)"guest.txt", AT_FDCWD, (long)"renamed.txt", 0x7fff0000L);
    long old_after = linux_syscall4(48, AT_FDCWD, (long)"guest.txt", 0, 0);
    long new_after = linux_syscall4(48, AT_FDCWD, (long)"renamed.txt", 0, 0);

    printf("raw-renameat:%ld:%ld:%ld\n", renamed, old_after, new_after);
    return renamed == 0 && old_after == -2 && new_after == 0 ? 44 : 76;
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

    let qemu_output = Command::new(&qemu)
        .arg(&binary)
        .current_dir(&workspace)
        .output()
        .unwrap();
    if qemu_output.status.code() == Some(44) {
        assert_eq!(qemu_output.stdout, b"raw-renameat:0:-2:0\n");
    } else {
        assert_eq!(
            qemu_output.status.code(),
            Some(76),
            "qemu stdout: {}; qemu stderr: {}",
            String::from_utf8_lossy(&qemu_output.stdout),
            String::from_utf8_lossy(&qemu_output.stderr),
        );
        assert_eq!(qemu_output.stdout, b"raw-renameat:-38:0:-2\n");
        eprintln!("qemu-riscv64 reports ENOSYS for raw renameat; checking rem6 SE coverage");
    }
    fs::write(&input, b"file-backed input\n").unwrap();

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
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":44"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-renameat:0:-2:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_renameat2_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw renameat2 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw renameat2 smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-renameat2");
    let source = workspace.join("raw-renameat2.c");
    let binary = workspace.join("raw-renameat2");
    let input = workspace.join("guest.txt");
    fs::write(&input, b"file-backed input\n").unwrap();
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall5(long number, long arg0, long arg1, long arg2, long arg3, long arg4) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    long renamed = linux_syscall5(276, AT_FDCWD, (long)"guest.txt", AT_FDCWD, (long)"renamed.txt", 0);
    long old_after = linux_syscall4(48, AT_FDCWD, (long)"guest.txt", 0, 0);
    long new_after = linux_syscall4(48, AT_FDCWD, (long)"renamed.txt", 0, 0);

    printf("raw-renameat2:%ld:%ld:%ld\n", renamed, old_after, new_after);
    return renamed == 0 && old_after == -2 && new_after == 0 ? 43 : 75;
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

    let qemu_output = Command::new(&qemu)
        .arg(&binary)
        .current_dir(&workspace)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(43),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr),
    );
    assert_eq!(qemu_output.stdout, b"raw-renameat2:0:-2:0\n");
    fs::write(&input, b"file-backed input\n").unwrap();

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
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":43"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-renameat2:0:-2:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
