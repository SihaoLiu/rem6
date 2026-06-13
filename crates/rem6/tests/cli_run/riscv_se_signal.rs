use std::process::Command;

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
