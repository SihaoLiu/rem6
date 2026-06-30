use std::{fs, process::Command};

use crate::support::{assert_stat, find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_sched_setattr_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw sched_setattr smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw sched_setattr smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-sched-setattr");
    let source = workspace.join("raw-sched-setattr.c");
    let binary = workspace.join("raw-sched-setattr");
    fs::write(
        &source,
        r#"#include <stdint.h>
#include <stdio.h>
#include <string.h>

struct sched_attr {
    uint32_t size;
    uint32_t sched_policy;
    uint64_t sched_flags;
    int32_t sched_nice;
    uint32_t sched_priority;
    uint64_t sched_runtime;
    uint64_t sched_deadline;
    uint64_t sched_period;
    uint32_t sched_util_min;
    uint32_t sched_util_max;
};

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

int main(void) {
    struct sched_attr attr;
    memset(&attr, 0, sizeof attr);
    attr.size = sizeof attr;
    attr.sched_policy = 0;
    attr.sched_nice = 5;
    long set_current = linux_syscall3(274, 0, (long)&attr, 0);

    struct sched_attr current;
    memset(&current, 0xa5, sizeof current);
    long get_current = linux_syscall4(275, 0, (long)&current, sizeof current, 0);
    long priority = linux_syscall3(141, 0, 0, 0);

    struct sched_attr legacy;
    memset(&legacy, 0, sizeof legacy);
    legacy.size = 48;
    legacy.sched_policy = 0;
    legacy.sched_nice = 6;
    long set_legacy = linux_syscall3(274, 0, (long)&legacy, 0);

    struct sched_attr short_size = attr;
    short_size.size = 47;
    long short_result = linux_syscall3(274, 0, (long)&short_size, 0);
    struct sched_attr bad_flags_attr = attr;
    bad_flags_attr.sched_flags = UINT64_MAX;
    long bad_flags = linux_syscall3(274, 0, (long)&bad_flags_attr, 0);
    long null_attr = linux_syscall3(274, 0, 0, 0);
    long negative_pid = linux_syscall3(274, -1L, (long)&attr, 0);
    struct sched_attr bad_priority = attr;
    bad_priority.sched_priority = 1;
    long bad_priority_result = linux_syscall3(274, 0, (long)&bad_priority, 0);
    long fault = linux_syscall3(274, 0, 1, 0);

    printf("raw-sched-setattr:%ld:%ld:%ld:%u:%u:%d:%u:%ld:%ld:%ld:%ld:%ld:%ld:%ld\n",
           set_current, get_current, priority, current.size, current.sched_policy,
           current.sched_nice, current.sched_priority, set_legacy, short_result,
           bad_flags, null_attr, negative_pid, bad_priority_result, fault);
    return set_current == 0 &&
           get_current == 0 &&
           priority == 15 &&
           current.size == sizeof current &&
           current.sched_policy == 0 &&
           current.sched_nice == 5 &&
           current.sched_priority == 0 &&
           set_legacy == 0 &&
           short_result == -7 &&
           bad_flags == -22 &&
           null_attr == -22 &&
           negative_pid == -22 &&
           bad_priority_result == -22 &&
           fault == -14 ? 82 : 83;
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

    let expected_stdout = b"raw-sched-setattr:0:0:15:56:0:5:0:0:-7:-22:-22:-22:-22:-14\n";
    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(82),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, expected_stdout);

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
    assert!(stdout.contains("\"stop_code\":82"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout
        .contains("\"text\":\"raw-sched-setattr:0:0:15:56:0:5:0:0:-7:-22:-22:-22:-22:-14\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 82, "constant");
}
