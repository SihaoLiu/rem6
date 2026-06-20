use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_getrusage_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw getrusage smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw getrusage smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-getrusage");
    let source = workspace.join("raw-getrusage.c");
    let binary = workspace.join("raw-getrusage");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <string.h>

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

int main(void) {
    unsigned char usage[160];
    memset(usage, 0xff, sizeof usage);
    long status = linux_syscall2(165, 0, (long)usage);
    if (status < 0) {
        printf("raw-getrusage:fail:%ld\n", status);
        return 72;
    }

    int rusage_changed = 0;
    int tail_unchanged = 1;
    for (unsigned int i = 0; i < 144; ++i) {
        rusage_changed |= usage[i] != 0xff;
    }
    for (unsigned int i = 144; i < sizeof usage; ++i) {
        tail_unchanged &= usage[i] == 0xff;
    }

    printf("raw-getrusage:ok:%d:%d\n", rusage_changed, tail_unchanged);
    return rusage_changed && tail_unchanged ? 40 : 73;
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
        Some(40),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-getrusage:ok:1:1\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "250000",
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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":40"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-getrusage:ok:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_readahead_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw readahead smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw readahead smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-readahead");
    let source = workspace.join("raw-readahead.c");
    let binary = workspace.join("raw-readahead");
    let input = workspace.join("readahead-input.txt");
    fs::write(&input, b"readahead input\n").unwrap();
    fs::write(
        &source,
        r#"#include <stdio.h>

#define AT_FDCWD (-100L)
#define O_RDONLY 0L

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
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
    long fd = linux_syscall4(56, AT_FDCWD, (long)"readahead-input.txt", O_RDONLY, 0);
    long file = fd >= 0 ? linux_syscall4(213, fd, 0, 16, 0) : fd;
    long zero = fd >= 0 ? linux_syscall4(213, fd, 0, 0, 0) : fd;
    long closed = fd >= 0 ? linux_syscall1(57, fd) : fd;
    long bad = linux_syscall4(213, 99, 0, 16, 0);

    printf("raw-readahead:%ld:%ld:%ld:%ld:%ld\n", fd >= 0, file, zero, closed, bad);
    return fd >= 0 && file == 0 && zero == 0 && closed == 0 && bad == -9 ? 42 : 83;
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
        Some(42),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-readahead:1:0:0:0:-9\n");

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
            &format!("readahead-input.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":42"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-readahead:1:0:0:0:-9\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_prlimit64_set_through_cli() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw prlimit64 smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-prlimit64");
    let source = workspace.join("raw-prlimit64.c");
    let binary = workspace.join("raw-prlimit64");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define RLIMIT_STACK 3

struct rlimit64 {
    unsigned long cur;
    unsigned long max;
};

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
    struct rlimit64 old_limit = {0, 0};
    struct rlimit64 requested = {0, 0};
    struct rlimit64 after = {0, 0};

    long initial = linux_syscall4(261, 0, RLIMIT_STACK, 0, (long)&old_limit);
    unsigned long new_current = old_limit.cur;
    if (new_current == ~0UL || new_current > 16UL * 1024UL * 1024UL) {
        new_current = 8UL * 1024UL * 1024UL;
    } else if (new_current >= 8192UL) {
        new_current /= 2;
    }

    requested.cur = new_current;
    requested.max = old_limit.max;
    long set = initial == 0
        ? linux_syscall4(261, 0, RLIMIT_STACK, (long)&requested, 0)
        : -999;
    long query = set == 0
        ? linux_syscall4(261, 0, RLIMIT_STACK, 0, (long)&after)
        : -999;

    int ok = initial == 0
        && set == 0
        && query == 0
        && after.cur == new_current
        && after.max == old_limit.max;
    printf("raw-prlimit64:%s:%d:%d:%d:%d\n",
           ok ? "ok" : "fail",
           initial == 0,
           set == 0,
           query == 0,
           after.cur == new_current);
    return ok ? 41 : 74;
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
            "250000",
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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":41"));
    assert!(stdout.contains("\"text\":\"raw-prlimit64:ok:1:1:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_prlimit64_nofile_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw prlimit64 nofile smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw prlimit64 nofile smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-prlimit64-nofile");
    let source = workspace.join("raw-prlimit64-nofile.c");
    let binary = workspace.join("raw-prlimit64-nofile");
    fs::write(
        &source,
        r#"#include <stdio.h>

#define RLIMIT_NOFILE 7

struct rlimit64 {
    unsigned long cur;
    unsigned long max;
};

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
    struct rlimit64 old_limit = {0, 0};
    struct rlimit64 requested = {0, 0};
    struct rlimit64 after = {0, 0};
    int pipe_fds[2] = {-1, -1};

    long initial = linux_syscall4(261, 0, RLIMIT_NOFILE, 0, (long)&old_limit);
    unsigned long new_current = old_limit.max >= 3 ? 3 : old_limit.cur;
    requested.cur = new_current;
    requested.max = old_limit.max;
    long set = initial == 0
        ? linux_syscall4(261, 0, RLIMIT_NOFILE, (long)&requested, 0)
        : -999;
    long query = set == 0
        ? linux_syscall4(261, 0, RLIMIT_NOFILE, 0, (long)&after)
        : -999;
    long pipe_status = query == 0 && after.cur == 3
        ? linux_syscall4(59, (long)pipe_fds, 0, 0, 0)
        : -999;

    int ok = initial == 0
        && set == 0
        && query == 0
        && old_limit.cur > 0
        && old_limit.cur <= old_limit.max
        && after.cur == new_current
        && after.max == old_limit.max
        && pipe_status == -24
        && pipe_fds[0] == -1
        && pipe_fds[1] == -1;
    printf("raw-prlimit64-nofile:%s:%d:%d:%d:%d:%d\n",
           ok ? "ok" : "fail",
           initial == 0,
           set == 0,
           query == 0,
           after.cur == new_current,
           pipe_status == -24);
    return ok ? 43 : 76;
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
        Some(43),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-prlimit64-nofile:ok:1:1:1:1:1\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "250000",
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
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":43"));
    assert!(stdout.contains("\"text\":\"raw-prlimit64-nofile:ok:1:1:1:1:1\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
