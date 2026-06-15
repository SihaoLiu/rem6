use std::{fs, process::Command};

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_raw_kill_signal_zero() {
    let program = riscv64_program(&[
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        i_type(0, 0, 0x0, 11, 0x13),   // addi a1, x0, 0
        i_type(129, 0, 0x0, 17, 0x13), // addi a7, x0, kill
        0x0000_0073,                   // ecall
        b_type(16, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(59, 0, 0x0, 10, 0x13),  // addi a0, x0, 59
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(60, 0, 0x0, 10, 0x13),  // addi a0, x0, 60
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-kill-zero", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
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
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 59, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_thread_signal_zero() {
    let program = riscv64_program(&[
        i_type(178, 0, 0x0, 17, 0x13), // addi a7, x0, gettid
        0x0000_0073,                   // ecall
        i_type(0, 0, 0x0, 11, 0x13),   // addi a1, x0, 0
        i_type(130, 0, 0x0, 17, 0x13), // addi a7, x0, tkill
        0x0000_0073,                   // ecall
        b_type(60, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        i_type(0, 10, 0x0, 13, 0x13),  // addi a3, a0, 0
        i_type(178, 0, 0x0, 17, 0x13), // addi a7, x0, gettid
        0x0000_0073,                   // ecall
        i_type(0, 10, 0x0, 11, 0x13),  // addi a1, a0, 0
        i_type(0, 13, 0x0, 10, 0x13),  // addi a0, a3, 0
        i_type(0, 0, 0x0, 12, 0x13),   // addi a2, x0, 0
        i_type(131, 0, 0x0, 17, 0x13), // addi a7, x0, tgkill
        0x0000_0073,                   // ecall
        b_type(16, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(61, 0, 0x0, 10, 0x13),  // addi a0, x0, 61
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(62, 0, 0x0, 10, 0x13),  // addi a0, x0, 62
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-thread-signal-zero", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
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
    assert!(stdout.contains("\"stop_code\":61"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 61, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_sigaltstack_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw sigaltstack smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw sigaltstack smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-sigaltstack");
    let source = workspace.join("raw-sigaltstack.c");
    let binary = workspace.join("raw-sigaltstack");
    fs::write(
        &source,
        r#"struct linux_stack {
    void *ss_sp;
    int ss_flags;
    unsigned int padding;
    unsigned long ss_size;
};

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

static void finish(int ok) {
    static const char pass[] = "raw-sigaltstack:0:2:0:0:0:1:0:8192:0:0:2:0\n";
    static const char fail[] = "raw-sigaltstack:fail\n";
    if (ok) {
        linux_syscall3(64, 1, (long)pass, sizeof(pass) - 1);
        linux_syscall2(93, 69, 0);
    } else {
        linux_syscall3(64, 1, (long)fail, sizeof(fail) - 1);
        linux_syscall2(93, 70, 0);
    }
    for (;;) {
    }
}

int main(void) {
    char alt[8192];
    struct linux_stack old0 = {0};
    struct linux_stack set = {alt, 0, 0, sizeof(alt)};
    struct linux_stack old1 = {0};
    struct linux_stack disable = {0, 2, 0, 0};
    struct linux_stack old2 = {0};
    long q0 = linux_syscall2(132, 0, (long)&old0);
    long s0 = linux_syscall2(132, (long)&set, 0);
    long q1 = linux_syscall2(132, 0, (long)&old1);
    long d0 = linux_syscall2(132, (long)&disable, 0);
    long q2 = linux_syscall2(132, 0, (long)&old2);
    int old1_matches = old1.ss_sp == alt;
    finish(q0 == 0 && old0.ss_sp == 0 && old0.ss_flags == 2 && old0.ss_size == 0 &&
           s0 == 0 && q1 == 0 && old1_matches && old1.ss_flags == 0 &&
           old1.ss_size == sizeof(alt) && d0 == 0 && q2 == 0 &&
           old2.ss_flags == 2 && old2.ss_size == 0);
    return 71;
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
        Some(69),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(
        qemu_output.stdout,
        b"raw-sigaltstack:0:2:0:0:0:1:0:8192:0:0:2:0\n"
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
    assert!(stdout.contains("\"stop_code\":69"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-sigaltstack:0:2:0:0:0:1:0:8192:0:0:2:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
