use std::{fs, process::Command};

use crate::support::{assert_stat, find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_positional_vector_io_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE positional vector I/O smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE positional vector I/O smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-pvec");
    let qemu_workspace = workspace.join("qemu");
    fs::create_dir(&qemu_workspace).unwrap();
    let source = workspace.join("pvec.c");
    let binary = workspace.join("pvec");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

struct iovec {
    void *iov_base;
    unsigned long iov_len;
};

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a7) : "memory");
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

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall5(long number, long arg0, long arg1, long arg2, long arg3, long arg4) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    char first[6] = {0};
    char second[6] = {0};
    struct iovec write_iov[2] = {{"XY", 2}, {"Z9", 2}};
    struct iovec read_iov[2] = {{first, 5}, {second, 5}};

    long fd = linux_syscall4(56, -100, (long)"vec.txt", 02 | 0100 | 01000, 0600);
    long seed = fd < 0 ? fd : linux_syscall3(64, fd, (long)"abcdefghij", 10);
    long wrote = fd < 0 ? fd : linux_syscall5(70, fd, (long)write_iov, 2, 2, 0);
    long pos_after_write = fd < 0 ? fd : linux_syscall3(62, fd, 0, 1);
    long read_count = fd < 0 ? fd : linux_syscall5(69, fd, (long)read_iov, 2, 0, 0);
    long pos_after_read = fd < 0 ? fd : linux_syscall3(62, fd, 0, 1);
    long close_status = fd < 0 ? -1 : linux_syscall1(57, fd);

    printf("pvec:%ld:%ld:%ld:%ld:%ld:%ld:%s:%s\n",
           fd >= 0 ? 0 : fd, seed, wrote, pos_after_write,
           read_count, pos_after_read, first, second);
    return fd >= 0 &&
           seed == 10 &&
           wrote == 4 &&
           pos_after_write == 10 &&
           read_count == 10 &&
           pos_after_read == 10 &&
           close_status == 0 &&
           strcmp(first, "abXYZ") == 0 &&
           strcmp(second, "9ghij") == 0 ? 73 : 74;
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
        .current_dir(&qemu_workspace)
        .arg(&binary)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(73),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"pvec:0:10:4:10:10:10:abXYZ:9ghij\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "500000",
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
    assert!(stdout.contains("\"text\":\"pvec:0:10:4:10:10:10:abXYZ:9ghij\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 73, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_positional_vector_io_v2() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE positional vector I/O v2 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-pvec2");
    let source = workspace.join("pvec2.c");
    let binary = workspace.join("pvec2");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

struct iovec {
    void *iov_base;
    unsigned long iov_len;
};

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a7) : "memory");
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

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall6(long number, long arg0, long arg1, long arg2, long arg3, long arg4, long arg5) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a5 asm("a5") = arg5;
    register long a7 asm("a7") = number;
    asm volatile("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a4), "r"(a5), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    char first[6] = {0};
    char second[6] = {0};
    struct iovec write_iov[2] = {{"XY", 2}, {"Z9", 2}};
    struct iovec read_iov[2] = {{first, 5}, {second, 5}};

    long fd = linux_syscall4(56, -100, (long)"vec2.txt", 02 | 0100 | 01000, 0600);
    long seed = fd < 0 ? fd : linux_syscall3(64, fd, (long)"abcdefghij", 10);
    long wrote = fd < 0 ? fd : linux_syscall6(287, fd, (long)write_iov, 2, 2, 0, 0);
    long pos_after_write = fd < 0 ? fd : linux_syscall3(62, fd, 0, 1);
    long read_count = fd < 0 ? fd : linux_syscall6(286, fd, (long)read_iov, 2, 0, 0, 0);
    long pos_after_read = fd < 0 ? fd : linux_syscall3(62, fd, 0, 1);
    long close_status = fd < 0 ? -1 : linux_syscall1(57, fd);

    printf("pvec2:%ld:%ld:%ld:%ld:%ld:%ld:%s:%s\n",
           fd >= 0 ? 0 : fd, seed, wrote, pos_after_write,
           read_count, pos_after_read, first, second);
    return fd >= 0 &&
           seed == 10 &&
           wrote == 4 &&
           pos_after_write == 10 &&
           read_count == 10 &&
           pos_after_read == 10 &&
           close_status == 0 &&
           strcmp(first, "abXYZ") == 0 &&
           strcmp(second, "9ghij") == 0 ? 83 : 84;
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

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "500000",
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
    assert!(stdout.contains("\"stop_code\":83"));
    assert!(stdout.contains("\"text\":\"pvec2:0:10:4:10:10:10:abXYZ:9ghij\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 83, "constant");
}
