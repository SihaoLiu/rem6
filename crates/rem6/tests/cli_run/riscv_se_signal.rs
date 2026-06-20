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
fn rem6_run_riscv_se_runs_static_raw_ignored_signals() {
    let mut program = riscv64_program(&[
        i_type(10, 0, 0x0, 10, 0x13),    // addi a0, x0, SIGUSR1
        u_type(0, 11, 0x17),             // auipc a1, 0
        i_type(0xfc, 11, 0x0, 11, 0x13), // addi a1, a1, sigaction offset
        i_type(0, 0, 0x0, 12, 0x13),     // addi a2, x0, 0
        i_type(8, 0, 0x0, 13, 0x13),     // addi a3, x0, sigset bytes
        i_type(134, 0, 0x0, 17, 0x13),   // addi a7, x0, rt_sigaction
        0x0000_0073,                     // ecall
        b_type(108, 0, 10, 0x1),         // bne a0, x0, fail
        i_type(172, 0, 0x0, 17, 0x13),   // addi a7, x0, getpid
        0x0000_0073,                     // ecall
        i_type(10, 0, 0x0, 11, 0x13),    // addi a1, x0, SIGUSR1
        i_type(129, 0, 0x0, 17, 0x13),   // addi a7, x0, kill
        0x0000_0073,                     // ecall
        b_type(84, 0, 10, 0x1),          // bne a0, x0, fail
        i_type(178, 0, 0x0, 17, 0x13),   // addi a7, x0, gettid
        0x0000_0073,                     // ecall
        i_type(10, 0, 0x0, 11, 0x13),    // addi a1, x0, SIGUSR1
        i_type(130, 0, 0x0, 17, 0x13),   // addi a7, x0, tkill
        0x0000_0073,                     // ecall
        b_type(60, 0, 10, 0x1),          // bne a0, x0, fail
        i_type(172, 0, 0x0, 17, 0x13),   // addi a7, x0, getpid
        0x0000_0073,                     // ecall
        i_type(0, 10, 0x0, 13, 0x13),    // addi a3, a0, 0
        i_type(178, 0, 0x0, 17, 0x13),   // addi a7, x0, gettid
        0x0000_0073,                     // ecall
        i_type(0, 10, 0x0, 11, 0x13),    // addi a1, a0, 0
        i_type(0, 13, 0x0, 10, 0x13),    // addi a0, a3, 0
        i_type(10, 0, 0x0, 12, 0x13),    // addi a2, x0, SIGUSR1
        i_type(131, 0, 0x0, 17, 0x13),   // addi a7, x0, tgkill
        0x0000_0073,                     // ecall
        b_type(16, 0, 10, 0x1),          // bne a0, x0, fail
        i_type(78, 0, 0x0, 10, 0x13),    // addi a0, x0, 78
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, exit
        0x0000_0073,                     // ecall
        i_type(79, 0, 0x0, 10, 0x13),    // addi a0, x0, 79
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, exit
        0x0000_0073,                     // ecall
    ]);
    program.resize(0x100, 0);
    program.extend_from_slice(&1_u64.to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-ignored-signals", &elf);

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
    assert!(stdout.contains("\"stop_code\":78"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 78, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_default_ignored_signals() {
    let program = riscv64_program(&[
        i_type(172, 0, 0x0, 17, 0x13), // addi a7, x0, getpid
        0x0000_0073,                   // ecall
        i_type(17, 0, 0x0, 11, 0x13),  // addi a1, x0, SIGCHLD
        i_type(129, 0, 0x0, 17, 0x13), // addi a7, x0, kill
        0x0000_0073,                   // ecall
        b_type(84, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(178, 0, 0x0, 17, 0x13), // addi a7, x0, gettid
        0x0000_0073,                   // ecall
        i_type(23, 0, 0x0, 11, 0x13),  // addi a1, x0, SIGURG
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
        i_type(28, 0, 0x0, 12, 0x13),  // addi a2, x0, SIGWINCH
        i_type(131, 0, 0x0, 17, 0x13), // addi a7, x0, tgkill
        0x0000_0073,                   // ecall
        b_type(16, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(80, 0, 0x0, 10, 0x13),  // addi a0, x0, 80
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(81, 0, 0x0, 10, 0x13),  // addi a0, x0, 81
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-default-ignored-signals", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "320",
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
    assert!(stdout.contains("\"stop_code\":80"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 80, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_blocked_default_ignored_signal_pending() {
    let mut program = riscv64_program(&[
        u_type(0, 8, 0x17),             // auipc s0, 0
        i_type(0x100, 8, 0x0, 8, 0x13), // addi s0, s0, signal data offset
        i_type(0, 0, 0x0, 10, 0x13),    // addi a0, x0, SIG_BLOCK
        i_type(0, 8, 0x0, 11, 0x13),    // addi a1, s0, 0
        i_type(0, 0, 0x0, 12, 0x13),    // addi a2, x0, 0
        i_type(8, 0, 0x0, 13, 0x13),    // addi a3, x0, sigset bytes
        i_type(135, 0, 0x0, 17, 0x13),  // addi a7, x0, rt_sigprocmask
        0x0000_0073,                    // ecall
        b_type(128, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(172, 0, 0x0, 17, 0x13),  // addi a7, x0, getpid
        0x0000_0073,                    // ecall
        i_type(17, 0, 0x0, 11, 0x13),   // addi a1, x0, SIGCHLD
        i_type(129, 0, 0x0, 17, 0x13),  // addi a7, x0, kill
        0x0000_0073,                    // ecall
        b_type(104, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(8, 8, 0x0, 10, 0x13),    // addi a0, s0, pending offset
        i_type(8, 0, 0x0, 11, 0x13),    // addi a1, x0, sigset bytes
        i_type(136, 0, 0x0, 17, 0x13),  // addi a7, x0, rt_sigpending
        0x0000_0073,                    // ecall
        b_type(84, 0, 10, 0x1),         // bne a0, x0, fail
        i_type(8, 8, 0x3, 5, 0x03),     // ld t0, 8(s0)
        i_type(0, 8, 0x3, 6, 0x03),     // ld t1, 0(s0)
        b_type(72, 6, 5, 0x1),          // bne t0, t1, fail
        i_type(1, 0, 0x0, 10, 0x13),    // addi a0, x0, SIG_UNBLOCK
        i_type(0, 8, 0x0, 11, 0x13),    // addi a1, s0, 0
        i_type(0, 0, 0x0, 12, 0x13),    // addi a2, x0, 0
        i_type(8, 0, 0x0, 13, 0x13),    // addi a3, x0, sigset bytes
        i_type(135, 0, 0x0, 17, 0x13),  // addi a7, x0, rt_sigprocmask
        0x0000_0073,                    // ecall
        b_type(44, 0, 10, 0x1),         // bne a0, x0, fail
        i_type(8, 8, 0x0, 10, 0x13),    // addi a0, s0, pending offset
        i_type(8, 0, 0x0, 11, 0x13),    // addi a1, x0, sigset bytes
        i_type(136, 0, 0x0, 17, 0x13),  // addi a7, x0, rt_sigpending
        0x0000_0073,                    // ecall
        b_type(24, 0, 10, 0x1),         // bne a0, x0, fail
        i_type(8, 8, 0x3, 5, 0x03),     // ld t0, 8(s0)
        b_type(16, 0, 5, 0x1),          // bne t0, x0, fail
        i_type(82, 0, 0x0, 10, 0x13),   // addi a0, x0, 82
        i_type(93, 0, 0x0, 17, 0x13),   // addi a7, x0, exit
        0x0000_0073,                    // ecall
        i_type(83, 0, 0x0, 10, 0x13),   // addi a0, x0, 83
        i_type(93, 0, 0x0, 17, 0x13),   // addi a7, x0, exit
        0x0000_0073,                    // ecall
    ]);
    program.resize(0x100, 0);
    program.extend_from_slice(&(1_u64 << (17 - 1)).to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-blocked-default-ignored-signal", &elf);

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
    assert!(stdout.contains("\"stop_code\":82"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 82, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_blocked_nonignored_signal_pending() {
    let mut program = riscv64_program(&[
        u_type(0, 8, 0x17),             // auipc s0, 0
        i_type(0x100, 8, 0x0, 8, 0x13), // addi s0, s0, signal data offset
        i_type(0, 0, 0x0, 10, 0x13),    // addi a0, x0, SIG_BLOCK
        i_type(0, 8, 0x0, 11, 0x13),    // addi a1, s0, 0
        i_type(0, 0, 0x0, 12, 0x13),    // addi a2, x0, 0
        i_type(8, 0, 0x0, 13, 0x13),    // addi a3, x0, sigset bytes
        i_type(135, 0, 0x0, 17, 0x13),  // addi a7, x0, rt_sigprocmask
        0x0000_0073,                    // ecall
        b_type(104, 0, 10, 0x1),        // bne a0, x0, fail
        i_type(172, 0, 0x0, 17, 0x13),  // addi a7, x0, getpid
        0x0000_0073,                    // ecall
        i_type(10, 0, 0x0, 11, 0x13),   // addi a1, x0, SIGUSR1
        i_type(129, 0, 0x0, 17, 0x13),  // addi a7, x0, kill
        0x0000_0073,                    // ecall
        b_type(80, 0, 10, 0x1),         // bne a0, x0, fail
        i_type(8, 8, 0x0, 10, 0x13),    // addi a0, s0, pending offset
        i_type(8, 0, 0x0, 11, 0x13),    // addi a1, x0, sigset bytes
        i_type(136, 0, 0x0, 17, 0x13),  // addi a7, x0, rt_sigpending
        0x0000_0073,                    // ecall
        b_type(60, 0, 10, 0x1),         // bne a0, x0, fail
        i_type(8, 8, 0x3, 5, 0x03),     // ld t0, 8(s0)
        i_type(0, 8, 0x3, 6, 0x03),     // ld t1, 0(s0)
        b_type(48, 6, 5, 0x1),          // bne t0, t1, fail
        i_type(1, 0, 0x0, 10, 0x13),    // addi a0, x0, SIG_UNBLOCK
        i_type(0, 8, 0x0, 11, 0x13),    // addi a1, s0, 0
        i_type(0, 0, 0x0, 12, 0x13),    // addi a2, x0, 0
        i_type(8, 0, 0x0, 13, 0x13),    // addi a3, x0, sigset bytes
        i_type(135, 0, 0x0, 17, 0x13),  // addi a7, x0, rt_sigprocmask
        0x0000_0073,                    // ecall
        i_type(-38, 0, 0x0, 5, 0x13),   // addi t0, x0, -ENOSYS
        b_type(16, 5, 10, 0x1),         // bne a0, t0, fail
        i_type(84, 0, 0x0, 10, 0x13),   // addi a0, x0, 84
        i_type(93, 0, 0x0, 17, 0x13),   // addi a7, x0, exit
        0x0000_0073,                    // ecall
        i_type(85, 0, 0x0, 10, 0x13),   // addi a0, x0, 85
        i_type(93, 0, 0x0, 17, 0x13),   // addi a7, x0, exit
        0x0000_0073,                    // ecall
    ]);
    program.resize(0x100, 0);
    program.extend_from_slice(&(1_u64 << (10 - 1)).to_le_bytes());
    program.extend_from_slice(&0_u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-blocked-nonignored-signal", &elf);

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
    assert!(stdout.contains("\"stop_code\":84"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[{\"pc\":\"0x80000070\""));
    assert!(stdout.contains("\"number\":135"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 84, "constant");
    assert_stat(
        &stdout,
        "sim.riscv.unknown_syscalls",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_rt_sigsuspend_invalid_size() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(4, 0, 0x0, 11, 0x13),   // addi a1, x0, 4
        i_type(133, 0, 0x0, 17, 0x13), // addi a7, x0, rt_sigsuspend
        0x0000_0073,                   // ecall
        i_type(-22, 0, 0x0, 5, 0x13),  // addi t0, x0, -EINVAL
        b_type(16, 5, 10, 0x1),        // bne a0, t0, fail
        i_type(72, 0, 0x0, 10, 0x13),  // addi a0, x0, 72
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(73, 0, 0x0, 10, 0x13),  // addi a0, x0, 73
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-rt-sigsuspend-invalid-size", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
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
    assert!(stdout.contains("\"stop_code\":72"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 72, "constant");
}

#[test]
fn rem6_run_riscv_se_records_raw_rt_sigreturn_as_unsupported() {
    let program = riscv64_program(&[
        i_type(139, 0, 0x0, 17, 0x13), // addi a7, x0, rt_sigreturn
        0x0000_0073,                   // ecall
        i_type(-38, 0, 0x0, 5, 0x13),  // addi t0, x0, -ENOSYS
        b_type(16, 5, 10, 0x1),        // bne a0, t0, fail
        i_type(74, 0, 0x0, 10, 0x13),  // addi a0, x0, 74
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
        i_type(75, 0, 0x0, 10, 0x13),  // addi a0, x0, 75
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, exit
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-rt-sigreturn-unsupported", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
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
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[{\"pc\":\"0x80000004\""));
    assert!(stdout.contains("\"number\":139"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 74, "constant");
    assert_stat(
        &stdout,
        "sim.riscv.unknown_syscalls",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_rt_sigqueueinfo_nonzero_signal() {
    let mut program = riscv64_program(&[
        i_type(100, 0, 0x0, 10, 0x13),   // addi a0, x0, current pid
        i_type(10, 0, 0x0, 11, 0x13),    // addi a1, x0, SIGUSR1
        u_type(0, 12, 0x17),             // auipc a2, 0
        i_type(0xf8, 12, 0x0, 12, 0x13), // addi a2, a2, siginfo offset
        i_type(138, 0, 0x0, 17, 0x13),   // addi a7, x0, rt_sigqueueinfo
        0x0000_0073,                     // ecall
        i_type(-38, 0, 0x0, 5, 0x13),    // addi t0, x0, -ENOSYS
        b_type(16, 5, 10, 0x1),          // bne a0, t0, fail
        i_type(76, 0, 0x0, 10, 0x13),    // addi a0, x0, 76
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, exit
        0x0000_0073,                     // ecall
        i_type(77, 0, 0x0, 10, 0x13),    // addi a0, x0, 77
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, exit
        0x0000_0073,                     // ecall
    ]);
    program.resize(0x100, 0);
    let mut siginfo = vec![0; 128];
    siginfo[8..12].copy_from_slice(&(-1_i32).to_le_bytes());
    program.extend_from_slice(&siginfo);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-rt-sigqueueinfo-nonzero", &elf);

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
    assert!(
        stdout.contains("\"status\":\"stopped_by_host\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"stop_code\":76"), "stdout: {stdout}");
    assert!(
        stdout.contains("\"riscv_unknown_syscalls\":[{\"pc\":\"0x80000014\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"number\":138"), "stdout: {stdout}");
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 76, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_raw_rt_sigtimedwait_pending_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static RISC-V SE raw rt_sigtimedwait smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw rt_sigtimedwait smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-rt-sigtimedwait");
    let source = workspace.join("raw-rt-sigtimedwait.c");
    let binary = workspace.join("raw-rt-sigtimedwait");
    fs::write(
        &source,
        r#"struct linux_timespec {
    long tv_sec;
    long tv_nsec;
};

struct linux_siginfo {
    int si_signo;
    int si_errno;
    int si_code;
    char rest[116];
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

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3), "r"(a7) : "memory");
    return a0;
}

static void finish(int ok) {
    static const char pass[] = "raw-rt-sigtimedwait:10:10:0\n";
    static const char fail[] = "raw-rt-sigtimedwait:fail\n";
    if (ok) {
        linux_syscall3(64, 1, (long)pass, sizeof(pass) - 1);
        linux_syscall2(93, 86, 0);
    } else {
        linux_syscall3(64, 1, (long)fail, sizeof(fail) - 1);
        linux_syscall2(93, 87, 0);
    }
    for (;;) {
    }
}

int main(void) {
    const unsigned long sigusr1_mask = 1UL << (10 - 1);
    unsigned long pending_after = 0;
    struct linux_siginfo info = {0};
    struct linux_timespec timeout = {0, 0};
    long block = linux_syscall4(135, 0, (long)&sigusr1_mask, 0, 8);
    long pid = linux_syscall2(172, 0, 0);
    long queued = linux_syscall2(129, pid, 10);
    long waited = linux_syscall4(137, (long)&sigusr1_mask, (long)&info, (long)&timeout, 8);
    long pending = linux_syscall2(136, (long)&pending_after, 8);
    finish(block == 0 && queued == 0 && waited == 10 &&
           info.si_signo == 10 && pending == 0 && pending_after == 0);
    return 87;
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
        Some(86),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-rt-sigtimedwait:10:10:0\n");

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
    assert!(stdout.contains("\"stop_code\":86"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"raw-rt-sigtimedwait:10:10:0\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
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
