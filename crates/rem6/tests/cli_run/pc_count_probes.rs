use std::process::Command;

use crate::support::*;

fn pc_count_program_elf() -> Vec<u8> {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(2, 5, 0x0, 6, 0x13), // addi x6, x5, 2
        0x0000_0073,                // ecall
    ]);
    riscv64_elf(0x8000_0000, 0x8000_0000, &program)
}

fn assert_pc_count_probe_output(stdout: &str) {
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"committed_instructions\":3"));
    assert!(stdout.contains(
        "\"instruction_probes\":{\"event_count\":6,\"retired_instruction_events\":3,\"tracked_instructions\":3,\"pc_sample_events\":3,\"pc_target_counters\":1,\"pc_target_armed\":false,\"pc_current_pair\":{\"pc\":\"0x80000000\",\"count\":1},\"pc_target_counts\":[{\"pc\":\"0x80000000\",\"count\":1}],\"pc_pending_targets\":[]}"
    ));
    assert_stat(
        stdout,
        "sim.instructions.probes.events",
        "Count",
        6,
        "monotonic",
    );
    assert_stat(
        stdout,
        "sim.instructions.probes.pc_sample_events",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        stdout,
        "sim.instructions.probes.pc_target_counters",
        "Count",
        1,
        "constant",
    );
}

#[test]
fn rem6_run_emits_riscv_pc_count_probe_stats() {
    let elf = pc_count_program_elf();
    let path = temp_binary("pc-count-probes", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--riscv-pc-count-target",
            "0x80000000:1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_pc_count_probe_output(&stdout);
}

#[test]
fn rem6_run_configures_riscv_pc_count_probe_stats_from_toml() {
    let elf = pc_count_program_elf();
    let binary = temp_binary("pc-count-probes-config-bin", &elf);
    let config = temp_config(
        "pc-count-probes-config",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 80\nstats_format = \"json\"\nexecute = true\ncores = 1\nriscv_pc_count_targets = [\"0x80000000:1\"]\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_pc_count_probe_output(&stdout);
}

#[test]
fn rem6_run_rejects_riscv_pc_count_target_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("pc-count-target-without-execute", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--riscv-pc-count-target",
            "0x80000000:1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-pc-count-target requires --execute"));
}

#[test]
fn rem6_run_rejects_invalid_riscv_pc_count_target() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("invalid-pc-count-target", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-pc-count-target",
            "0x80000000:0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr
        .contains("invalid RISC-V PC count target 0x80000000:0; expected <pc>:<positive-count>"));
}

#[test]
fn rem6_run_rejects_riscv_pc_count_target_for_non_riscv_isa() {
    let elf = x86_64_elf(0x4000_0000, 0x4000_0000, &[0x90]);
    let path = temp_binary("pc-count-target-non-riscv", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "x86",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-pc-count-target",
            "0x40000000:1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-pc-count-target requires --isa riscv"));
}
