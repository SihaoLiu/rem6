use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_memfd_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE memfd smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE memfd smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-memfd");
    let source = workspace.join("raw-memfd.c");
    let binary = workspace.join("raw-memfd");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define MFD_CLOEXEC 0x0001
#define SEEK_SET 0

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

int main(void) {
    const char *payload = "abc";
    char buffer[8] = {0};
    long fd = linux_syscall2(279, (long)"scratch", MFD_CLOEXEC);
    long written = fd >= 0 ? linux_syscall3(64, fd, (long)payload, 3) : fd;
    long seek = fd >= 0 ? linux_syscall3(62, fd, 0, SEEK_SET) : fd;
    long read_count = fd >= 0 ? linux_syscall3(63, fd, (long)buffer, 8) : fd;
    long truncate_result = fd >= 0 ? linux_syscall2(46, fd, 5) : fd;
    long close_result = fd >= 0 ? linux_syscall1(57, fd) : fd;

    printf("memfd:%ld:%ld:%ld:%ld:%.*s:%ld:%ld\n",
           fd >= 0 ? 0 : fd, written, seek, read_count,
           (int)(read_count > 0 ? read_count : 0), buffer,
           truncate_result, close_result);
    return fd >= 0 && written == 3 && seek == 0 && read_count == 3 &&
           buffer[0] == 'a' && buffer[1] == 'b' && buffer[2] == 'c' &&
           truncate_result == 0 && close_result == 0 ? 58 : 82;
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
        Some(58),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"memfd:0:3:0:3:abc:0:0\n");

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
    assert!(stdout.contains("\"stop_code\":58"));
    assert!(stdout.contains("\"text\":\"memfd:0:3:0:3:abc:0:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
