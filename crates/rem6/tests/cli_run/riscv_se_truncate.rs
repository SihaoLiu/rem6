use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_truncate_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw truncate smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw truncate smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-truncate");
    let source = workspace.join("raw-truncate.c");
    let binary = workspace.join("raw-truncate");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)
#define O_RDONLY 0
#define O_WRONLY 01
#define O_CREAT 0100
#define O_TRUNC 01000

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

static long linux_syscall3(long number, long arg0, long arg1, long arg2) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    const char *path = "truncate-target.txt";
    const char *payload = "abcdef";
    char buffer[8] = {0};

    long fd = linux_syscall4(56, AT_FDCWD, (long)path, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    long written = fd >= 0 ? linux_syscall3(64, fd, (long)payload, 6) : fd;
    long close_written = fd >= 0 ? linux_syscall1(57, fd) : fd;
    long truncated = linux_syscall2(45, (long)path, 3);
    long missing = linux_syscall2(45, (long)"missing.txt", 1);
    long read_fd = linux_syscall4(56, AT_FDCWD, (long)path, O_RDONLY, 0);
    long read_count = read_fd >= 0 ? linux_syscall3(63, read_fd, (long)buffer, 7) : read_fd;
    long close_read = read_fd >= 0 ? linux_syscall1(57, read_fd) : read_fd;

    printf("raw-truncate:%ld:%ld:%ld:%ld:%ld:%ld:%.*s:%ld\n",
           fd >= 0 ? 0 : fd, written, close_written, truncated, missing,
           read_count, (int)(read_count > 0 ? read_count : 0), buffer, close_read);
    return fd >= 0 && written == 6 && close_written == 0 && truncated == 0 &&
           missing == -2 && read_fd >= 0 && read_count == 3 &&
           buffer[0] == 'a' && buffer[1] == 'b' && buffer[2] == 'c' &&
           close_read == 0 ? 57 : 81;
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
        .current_dir(&workspace)
        .arg(&binary)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(57),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-truncate:0:6:0:0:-2:3:abc:0\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(&workspace)
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
    assert!(stdout.contains("\"stop_code\":57"));
    assert!(stdout.contains("\"riscv_guest_writes\":["));
    assert!(stdout.contains("\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-truncate:0:6:0:0:-2:3:abc:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
