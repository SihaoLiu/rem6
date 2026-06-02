use std::fs;
use std::process::Command;

use crate::support::*;

#[test]
fn rem6_run_loads_riscv_elf_and_emits_json_stats_artifact() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-run", &elf);

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
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.run.v1\""));
    assert!(stdout.contains("\"isa\":\"riscv\""));
    assert!(stdout.contains("\"architecture\":\"riscv64\""));
    assert!(stdout.contains("\"entry\":\"0x80000000\""));
    assert!(stdout.contains("\"start_address\":\"0x80000000\""));
    assert!(stdout.contains("\"riscv_boot\":{\"a0\":\"0x0\",\"a1\":\"0x0\"}"));
    assert!(stdout.contains("\"status\":\"loaded\""));
    assert!(stdout.contains("\"host_event_delay\":1"));
    assert!(stdout.contains("\"parallel\":{\"scheduler\":{"));
    assert!(stdout.contains("\"epochs\":0"));
    assert!(stdout.contains("\"dispatches\":0"));
    assert!(stdout.contains("\"batches\":0"));
    assert!(stdout.contains("\"max_workers\":0"));
    assert!(stdout.contains("\"batch_worker_ticks\":0"));
    assert_stat_id(&stdout, "sim.binary.bytes", 0);
    assert_stat(
        &stdout,
        "sim.binary.bytes",
        "Byte",
        elf.len() as u64,
        "constant",
    );
    assert!(stdout.contains("\"path\":\"sim.elf.load_segments\""));
    assert!(stdout.contains("\"path\":\"sim.max_tick\""));
    assert_stat(
        &stdout,
        "sim.start_address",
        "Address",
        0x8000_0000,
        "constant",
    );
    assert_stat(&stdout, "sim.riscv.boot.a0", "Value", 0, "constant");
    assert_stat(&stdout, "sim.riscv.boot.a1", "Value", 0, "constant");
    assert_stat(&stdout, "sim.host.event_delay", "Tick", 1, "constant");
    assert!(stdout.contains("\"reset_policy\":\"constant\""));
}

#[test]
fn rem6_run_loads_x86_elf_without_riscv_boot_artifact_fields() {
    let elf = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);
    let path = temp_binary("x86-run", &elf);

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
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"isa\":\"x86\""));
    assert!(stdout.contains("\"architecture\":\"x86_64\""));
    assert!(stdout.contains("\"status\":\"loaded\""));
    assert!(!stdout.contains("\"riscv_boot\""));
    assert!(!stdout.contains("sim.riscv.boot.a0"));
    assert!(!stdout.contains("sim.riscv.boot.a1"));
}

#[test]
fn rem6_run_loads_riscv_elf_with_explicit_start_address() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("loaded-start-address", &elf);

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
            "--start-address",
            "0X80000008",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"loaded\""));
    assert!(stdout.contains("\"entry\":\"0x80000000\""));
    assert!(stdout.contains("\"start_address\":\"0x80000008\""));
    assert_stat(
        &stdout,
        "sim.start_address",
        "Address",
        0x8000_0008,
        "constant",
    );
}

#[test]
fn rem6_run_loads_riscv_elf_with_explicit_boot_registers() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("loaded-riscv-boot-registers", &elf);

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
            "--riscv-boot-a0",
            "7",
            "--riscv-boot-a1",
            "0X80002000",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"loaded\""));
    assert!(stdout.contains("\"riscv_boot\":{\"a0\":\"0x7\",\"a1\":\"0x80002000\"}"));
    assert_stat(&stdout, "sim.riscv.boot.a0", "Value", 7, "constant");
    assert_stat(
        &stdout,
        "sim.riscv.boot.a1",
        "Value",
        0x8000_2000,
        "constant",
    );
}

#[test]
fn rem6_run_loads_riscv_elf_with_explicit_blob() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("loaded-blob", &elf);
    let blob_path = temp_binary("loaded-blob-data", &[0xde, 0xad, 0xbe, 0xef]);

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
            "--load-blob",
            &format!("0X80001000:{}", blob_path.display()),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"loaded\""));
    assert!(stdout.contains(&format!(
        "\"load_blobs\":[{{\"address\":\"0x80001000\",\"bytes\":4,\"path\":\"{}\"}}]",
        blob_path.display()
    )));
    assert_stat(&stdout, "sim.load_blobs", "Count", 1, "constant");
    assert_stat(&stdout, "sim.load_blob_bytes", "Byte", 4, "constant");
    assert_stat(
        &stdout,
        "sim.load_blob0.address",
        "Address",
        0x8000_1000,
        "constant",
    );
    assert_stat(&stdout, "sim.load_blob0.bytes", "Byte", 4, "constant");
}

#[test]
fn rem6_run_writes_json_artifact_to_requested_output_path() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("output-sink", &elf);
    let artifact_path = temp_output("output-sink");

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
            "--output",
            artifact_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        format!(
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\"}}\n",
            artifact_path.display()
        )
    );
    let artifact = fs::read_to_string(&artifact_path).unwrap();
    assert!(artifact.contains("\"schema\":\"rem6.cli.run.v1\""));
    assert!(artifact.contains("\"status\":\"loaded\""));
    assert!(artifact.contains("\"path\":\"sim.binary.bytes\""));
}

#[test]
fn rem6_run_writes_stats_array_to_requested_stats_output_path() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("stats-output", &elf);
    let stats_path = temp_output("stats-output");

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
            "--stats-output",
            stats_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.run.v1\""));
    let stats = fs::read_to_string(&stats_path).unwrap();
    assert!(stats.starts_with('['));
    assert!(stats.ends_with("]\n"));
    assert_stat_id(&stats, "sim.binary.bytes", 0);
    assert_stat(
        &stats,
        "sim.binary.bytes",
        "Byte",
        elf.len() as u64,
        "constant",
    );
    assert!(stats.contains("\"path\":\"sim.max_tick\""));
    assert!(!stats.contains("\"schema\":\"rem6.cli.run.v1\""));
}

#[test]
fn rem6_run_emits_text_stats_when_requested() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("text-stats", &elf);

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
            "text",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("---------- Begin Simulation Statistics ----------"));
    assert!(stdout.contains("sim.binary.bytes"));
    assert!(stdout.contains("sim.max_tick"));
    assert!(stdout.contains("unit=Byte"));
    assert!(stdout.contains("reset_policy=constant"));
    assert!(stdout.contains("---------- End Simulation Statistics   ----------"));
    assert!(!stdout.contains("\"schema\":\"rem6.cli.run.v1\""));
    assert!(!stdout.contains("\"path\":\"sim.binary.bytes\""));
}

#[test]
fn rem6_run_writes_text_stats_to_requested_output_paths() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("text-output", &elf);
    let artifact_path = temp_output("text-output-artifact");
    let stats_path = temp_output("text-output-stats");

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
            "text",
            "--output",
            artifact_path.to_str().unwrap(),
            "--stats-output",
            stats_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        format!(
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"text\",\"artifact\":\"{}\",\"stats_artifact\":\"{}\"}}\n",
            artifact_path.display(),
            stats_path.display()
        )
    );
    let artifact = fs::read_to_string(&artifact_path).unwrap();
    assert!(artifact.contains("---------- Begin Simulation Statistics ----------"));
    assert!(artifact.contains("sim.binary.bytes"));
    assert!(!artifact.contains("\"schema\":\"rem6.cli.run.v1\""));
    let stats = fs::read_to_string(&stats_path).unwrap();
    assert!(stats.contains("---------- Begin Simulation Statistics ----------"));
    assert!(stats.contains("sim.max_tick"));
    assert!(!stats.starts_with('['));
}

#[test]
fn rem6_run_reports_both_artifact_paths_when_output_and_stats_output_are_requested() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("dual-output", &elf);
    let artifact_path = temp_output("dual-output-artifact");
    let stats_path = temp_output("dual-output-stats");

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
            "--output",
            artifact_path.to_str().unwrap(),
            "--stats-output",
            stats_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        format!(
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\",\"stats_artifact\":\"{}\"}}\n",
            artifact_path.display(),
            stats_path.display()
        )
    );
    assert!(fs::read_to_string(&artifact_path)
        .unwrap()
        .contains("\"schema\":\"rem6.cli.run.v1\""));
    assert!(fs::read_to_string(&stats_path)
        .unwrap()
        .contains("\"path\":\"sim.binary.bytes\""));
}

#[test]
fn rem6_run_rejects_overlapping_artifact_and_stats_output_paths() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("overlap-output", &elf);
    let output_path = temp_output("overlap-output");

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
            "--output",
            output_path.to_str().unwrap(),
            "--stats-output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--output and --stats-output must use different paths"));
    assert!(!output_path.exists());
}

#[test]
fn rem6_run_rejects_isa_mismatch_before_emitting_stats() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("isa-mismatch", &elf);

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
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("requested ISA x86 does not match ELF architecture riscv64"));
}
