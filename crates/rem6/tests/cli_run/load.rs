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
    assert!(stdout.contains("\"riscv_boot\":{\"a0\":\"0x0\",\"a1\":\"0x0\",\"se\":false}"));
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
    assert!(stdout.contains("\"riscv_boot\":{\"a0\":\"0x7\",\"a1\":\"0x80002000\",\"se\":false}"));
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
fn rem6_run_binds_readfile_mmio_payload_from_cli() {
    let program = riscv64_program(&[i_type(0, 10, 0x3, 5, 0x03), 0x0010_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("readfile-mmio", &elf);
    let readfile_path = temp_binary(
        "readfile-mmio-data",
        &[0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--execute",
            "--stats-format",
            "json",
            "--riscv-boot-a0",
            "0x10000000",
            "--readfile",
            &format!("0x10000000:0x100:{}", readfile_path.display()),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_reason\":\"host_trap\""));
    assert!(stdout.contains("\"registers\":{\"x5\":\"0x1122334455667788\",\"x10\":\"0x10000000\"}"));
    assert!(stdout.contains(&format!(
        "\"readfiles\":[{{\"base\":\"0x10000000\",\"size\":256,\"bytes\":8,\"path\":\"{}\"}}]",
        readfile_path.display()
    )));
    assert_stat(&stdout, "sim.readfiles", "Count", 1, "constant");
    assert_stat(
        &stdout,
        "sim.readfile0.base",
        "Address",
        0x1000_0000,
        "constant",
    );
    assert_stat(&stdout, "sim.readfile0.size", "Byte", 0x100, "constant");
    assert_stat(&stdout, "sim.readfile0.bytes", "Byte", 8, "constant");
}

#[test]
fn rem6_run_binds_readfile_mmio_payload_from_toml_config() {
    let program = riscv64_program(&[i_type(0, 10, 0x3, 5, 0x03), 0x0010_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("readfile-mmio-toml");
    let binary = workspace.join("kernel.elf");
    let readfile = workspace.join("boot.readfile");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&readfile, [0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01]).unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"kernel.elf\"\nmax_tick = 80\nexecute = true\nstats_format = \"json\"\nriscv_boot_a0 = 268435456\nreadfiles = [\"0x10000000:0x100:boot.readfile\"]\n",
    )
    .unwrap();

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
    assert!(stdout.contains("\"registers\":{\"x5\":\"0x102030405060708\",\"x10\":\"0x10000000\"}"));
    assert!(stdout.contains(&format!(
        "\"readfiles\":[{{\"base\":\"0x10000000\",\"size\":256,\"bytes\":8,\"path\":\"{}\"}}]",
        readfile.display()
    )));
}

#[test]
fn rem6_run_binds_readfile_mmio_payload_from_resource_config() {
    let program = riscv64_program(&[i_type(0, 10, 0x3, 5, 0x03), 0x0010_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("readfile-mmio-resource-config");
    fs::write(workspace.join("kernel.elf"), elf).unwrap();
    fs::write(
        workspace.join("boot.input"),
        [0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11],
    )
    .unwrap();
    let resource_config = workspace.join("resource-acquire.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
workload_id = "readfile-resource-cli"
boot_entry = 2147483648

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:readfile-resource-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:readfile-resource-kernel"

[[resource_acquire.resources]]
id = "boot-readfile"
kind = "input"
digest = "sha256:readfile-resource-input"
locator = "resources/boot.input"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://boot-readfile"
artifact = "boot.input"
artifact_digest = "sha256:readfile-resource-input"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nmax_tick = 80\nexecute = true\nstats_format = \"json\"\nriscv_boot_a0 = 268435456\nreadfiles = [\"0x10000000:0x100:resource:boot-readfile\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"registers\":{\"x5\":\"0x1112131415161718\",\"x10\":\"0x10000000\"}"));
    assert!(
        stdout.contains(
            "\"readfiles\":[{\"base\":\"0x10000000\",\"size\":256,\"bytes\":8,\"path\":\"resource:boot-readfile\"}]"
        )
    );
    assert_stat(&stdout, "sim.readfiles", "Count", 1, "constant");
    assert_stat(&stdout, "sim.readfile0.bytes", "Byte", 8, "constant");
}

#[test]
fn rem6_run_readfile_mmio_obeys_instruction_limit() {
    let program = riscv64_program(&[i_type(0, 10, 0x3, 5, 0x03), 0x0010_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("readfile-mmio-instruction-limit", &elf);
    let readfile_path = temp_binary(
        "readfile-mmio-instruction-limit-data",
        &[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--execute",
            "--max-instructions",
            "1",
            "--stats-format",
            "json",
            "--riscv-boot-a0",
            "0x10000000",
            "--readfile",
            &format!("0x10000000:0x100:{}", readfile_path.display()),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"stop_reason\":\"instruction_limit\""));
    assert!(stdout.contains("\"committed_instructions\":1"));
    assert!(stdout.contains("\"registers\":{\"x10\":\"0x10000000\"}"));
    assert!(stdout.contains(&format!(
        "\"readfiles\":[{{\"base\":\"0x10000000\",\"size\":256,\"bytes\":8,\"path\":\"{}\"}}]",
        readfile_path.display()
    )));
    assert_stat(&stdout, "sim.readfiles", "Count", 1, "constant");
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
fn rem6_run_writes_power_analysis_output() {
    let elf = riscv64_elf(
        0x8000_0000,
        0x8000_0000,
        &riscv64_program(&[
            0x0070_0293, // addi x5, x0, 7
            0x0000_0073, // ecall
        ]),
    );
    let path = temp_binary("power-output", &elf);
    let power_path = temp_output("power-output");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--power-format",
            "mcpat-xml",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"power_analysis\":{\"format\":\"mcpat-xml\""));
    assert!(stdout.contains(&format!("\"artifact\":\"{}\"", power_path.display())));

    let power = fs::read_to_string(&power_path).unwrap();
    assert!(power.contains("<mcpat_power tick=\""));
    assert!(power.contains("<component id=\"cpu0.core\""));
    assert!(power.contains("dynamic_watts="));
    assert!(power.contains("<totals dynamic_watts="));
}

#[test]
fn rem6_run_reports_power_analysis_path_in_output_envelope() {
    let elf = riscv64_elf(
        0x8000_0000,
        0x8000_0000,
        &riscv64_program(&[
            0x0070_0293, // addi x5, x0, 7
            0x0000_0073, // ecall
        ]),
    );
    let path = temp_binary("power-output-envelope", &elf);
    let artifact_path = temp_output("power-output-envelope-artifact");
    let power_path = temp_output("power-output-envelope");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--output",
            artifact_path.to_str().unwrap(),
            "--power-format",
            "dsent-csv",
            "--power-output",
            power_path.to_str().unwrap(),
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
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\",\"power_artifact\":\"{}\"}}\n",
            artifact_path.display(),
            power_path.display()
        )
    );
    let artifact = fs::read_to_string(&artifact_path).unwrap();
    assert!(artifact.contains("\"power_analysis\":{\"format\":\"dsent-csv\""));
    assert!(artifact.contains(&format!("\"artifact\":\"{}\"", power_path.display())));
    let power = fs::read_to_string(&power_path).unwrap();
    assert!(power.starts_with("record_type,tick,target,state,temperature_c"));
    assert!(power.contains("component,"));
    assert!(power.contains("cpu0.core"));
}

#[test]
fn rem6_run_reports_all_output_artifact_paths_when_power_analysis_is_requested() {
    let elf = riscv64_elf(
        0x8000_0000,
        0x8000_0000,
        &riscv64_program(&[
            0x0070_0293, // addi x5, x0, 7
            0x0000_0073, // ecall
        ]),
    );
    let path = temp_binary("power-all-output-envelope", &elf);
    let artifact_path = temp_output("power-all-output-artifact");
    let stats_path = temp_output("power-all-output-stats");
    let power_path = temp_output("power-all-output");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--output",
            artifact_path.to_str().unwrap(),
            "--stats-output",
            stats_path.to_str().unwrap(),
            "--power-output",
            power_path.to_str().unwrap(),
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
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\",\"stats_artifact\":\"{}\",\"power_artifact\":\"{}\"}}\n",
            artifact_path.display(),
            stats_path.display(),
            power_path.display()
        )
    );
    assert!(fs::read_to_string(&artifact_path)
        .unwrap()
        .contains("\"power_analysis\":{\"format\":\"mcpat-xml\""));
    assert!(fs::read_to_string(&stats_path)
        .unwrap()
        .contains("\"path\":\"sim.instructions.committed\""));
    assert!(fs::read_to_string(&power_path)
        .unwrap()
        .contains("<component id=\"cpu0.core\""));
}

#[test]
fn rem6_run_loads_power_analysis_output_from_toml_config() {
    let elf = riscv64_elf(
        0x8000_0000,
        0x8000_0000,
        &riscv64_program(&[
            0x0070_0293, // addi x5, x0, 7
            0x0000_0073, // ecall
        ]),
    );
    let workspace = temp_workspace("power-output-toml");
    let binary_name = format!("kernel-{}.elf", std::process::id());
    let binary = workspace.join(&binary_name);
    fs::write(&binary, elf).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nexecute = true\npower_format = \"dsent-csv\"\npower_output = \"power.csv\"\n",
            binary_name
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let power_path = workspace.join("power.csv");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"power_analysis\":{\"format\":\"dsent-csv\""));
    assert!(stdout.contains(&format!("\"artifact\":\"{}\"", power_path.display())));
    assert!(fs::read_to_string(power_path)
        .unwrap()
        .contains("cpu0.core"));
}

#[test]
fn rem6_run_config_scan_treats_power_output_value_as_a_value() {
    let elf = riscv64_elf(
        0x8000_0000,
        0x8000_0000,
        &riscv64_program(&[
            0x0070_0293, // addi x5, x0, 7
            0x0000_0073, // ecall
        ]),
    );
    let workspace = temp_workspace("power-output-config-scan");
    let binary = workspace.join("kernel.elf");
    fs::write(&binary, elf).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"kernel.elf\"\nmax_tick = 40\nexecute = true\n",
    )
    .unwrap();
    let power_path = workspace.join("--config");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(&workspace)
        .args([
            "run",
            "--power-output",
            "--config",
            "--config",
            config.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fs::read_to_string(power_path)
        .unwrap()
        .contains("<component id=\"cpu0.core\""));
}

#[test]
fn rem6_run_power_analysis_includes_dram_activity() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),        // sd x6, 8(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    program.extend_from_slice(&[0; 16]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("power-output-dram", &elf);
    let power_path = temp_output("power-output-dram");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--execute",
            "--dram-memory",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let power = fs::read_to_string(&power_path).unwrap();
    assert!(power.contains("<component id=\"cpu0.core\""));
    assert!(power.contains("<component id=\"memory.dram\""));
    assert!(power.contains("total_watts="));
}

#[test]
fn rem6_run_rejects_invalid_power_analysis_format() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("power-output-invalid-format", &elf);
    let power_path = temp_output("power-output-invalid-format");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--power-format",
            "unknown",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported power analysis format unknown"));
    assert!(!power_path.exists());
}

#[test]
fn rem6_run_rejects_invalid_power_analysis_format_from_toml_config() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let workspace = temp_workspace("power-output-invalid-format-toml");
    let binary_name = format!("kernel-{}.elf", std::process::id());
    let binary = workspace.join(&binary_name);
    fs::write(&binary, elf).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nexecute = true\npower_format = \"unknown\"\npower_output = \"power.xml\"\n",
            binary_name
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported power analysis format unknown"));
    assert!(!workspace.join("power.xml").exists());
}

#[test]
fn rem6_run_rejects_overlapping_power_output_paths() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("power-output-overlap", &elf);
    let output_path = temp_output("power-output-overlap");
    let stats_path = temp_output("power-output-overlap-stats");

    let output_conflict = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--output",
            output_path.to_str().unwrap(),
            "--power-output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output_conflict.status.success());
    assert!(output_conflict.stdout.is_empty());
    let stderr = String::from_utf8(output_conflict.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert!(!output_path.exists());

    let stats_conflict = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-output",
            stats_path.to_str().unwrap(),
            "--power-output",
            stats_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!stats_conflict.status.success());
    assert!(stats_conflict.stdout.is_empty());
    let stderr = String::from_utf8(stats_conflict.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert!(!stats_path.exists());
}

#[test]
fn rem6_run_rejects_power_analysis_output_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("power-output-load-only", &elf);
    let power_path = temp_output("power-output-load-only");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--power-output requires --execute"));
    assert!(!power_path.exists());
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
