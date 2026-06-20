use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_pidfd_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw pidfd smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw pidfd smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-pidfd");
    let source = workspace.join("raw-pidfd.c");
    let binary = workspace.join("raw-pidfd");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define PIDFD_NONBLOCK 0x800

static long linux_syscall0(long number) {
    register long a0 asm("a0");
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "=r"(a0) : "r"(a7) : "memory");
    return a0;
}

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3),
                  "r"(a7) : "memory");
    return a0;
}

int main(void) {
    long pid = linux_syscall0(172);
    long fd = pid > 0 ? linux_syscall2(434, pid, PIDFD_NONBLOCK) : -99;
    long fd_flags = fd >= 0 ? linux_syscall4(25, fd, 1, 0, 0) : fd;
    long status_flags = fd >= 0 ? linux_syscall4(25, fd, 3, 0, 0) : fd;
    long probe = fd >= 0 ? linux_syscall4(424, fd, 0, 0, 0) : fd;
    long bad_signal = fd >= 0 ? linux_syscall4(424, fd, 65, 0, 0) : fd;
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : fd;
    long after_close = fd >= 0 ? linux_syscall4(424, fd, 0, 0, 0) : fd;
    int ok = pid > 0
        && fd >= 0
        && fd_flags == 1
        && status_flags == 0x8802
        && probe == 0
        && bad_signal == -22
        && close_status == 0
        && after_close == -9;
    printf("pidfd:%ld:%ld:%ld:%ld:%ld:%ld:%ld:%d\n",
           fd >= 0 ? 0 : fd, fd_flags, status_flags, probe, bad_signal,
           close_status, after_close, ok);
    return ok ? 59 : 83;
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
        Some(59),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"pidfd:0:1:34818:0:-22:0:-9:1\n");

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
    assert!(stdout.contains("\"stop_code\":59"));
    assert!(stdout.contains("\"text\":\"pidfd:0:1:34818:0:-22:0:-9:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
