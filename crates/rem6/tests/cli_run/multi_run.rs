use std::{fs, process::Command};

use crate::support::*;

#[test]
fn rem6_multi_run_executes_run_configs_and_writes_aggregate_artifacts() {
    let workspace = temp_workspace("multi-run-run-configs");
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    fs::write(
        workspace.join("program.elf"),
        riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
    .unwrap();
    fs::write(
        workspace.join("first.toml"),
        r#"[run]
isa = "riscv"
binary = "program.elf"
max_tick = 40
stats_format = "json"
execute = true
memory_system = "direct"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("second.toml"),
        r#"[run]
isa = "riscv"
binary = "program.elf"
max_tick = 40
stats_format = "json"
execute = true
memory_system = "direct"
"#,
    )
    .unwrap();
    let config = workspace.join("multi-run.toml");
    let artifact = workspace.join("multi-run.json");
    let stats = workspace.join("multi-run-stats.json");
    fs::write(
        &config,
        r#"[multi_run]
suite_id = "two-riscv-smoke"
stats_format = "json"
output = "multi-run.json"
stats_output = "multi-run-stats.json"

[[multi_run.runs]]
id = "first"
config = "first.toml"

[[multi_run.runs]]
id = "second"
config = "second.toml"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["multi-run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.output.v1\""));
    assert!(stdout.contains("\"artifact\":\""));
    assert!(stdout.contains("multi-run.json"));
    assert!(stdout.contains("\"stats_artifact\":\""));
    assert!(stdout.contains("multi-run-stats.json"));

    let artifact = fs::read_to_string(artifact).unwrap();
    assert!(artifact.contains("\"schema\":\"rem6.cli.multi-run.v1\""));
    assert!(artifact.contains("\"suite_id\":\"two-riscv-smoke\""));
    assert!(artifact.contains("\"runs\":2"));
    assert!(artifact.contains("\"succeeded\":2"));
    assert!(artifact.contains("\"failed\":0"));
    assert!(artifact.contains("\"total_committed_instructions\":4"));
    assert!(artifact.contains("\"id\":\"first\""));
    assert!(artifact.contains("\"id\":\"second\""));
    assert!(artifact.contains("\"run_schema\":\"rem6.cli.run.v1\""));
    assert!(artifact.contains("\"status\":\"executed_until_trap\""));
    assert!(artifact.contains("\"committed_instructions\":2"));
    assert!(artifact.contains("\"stats\":["));

    let stats = fs::read_to_string(stats).unwrap();
    assert_stat(&stats, "sim.multi_run.runs", "Count", 2, "constant");
    assert_stat(&stats, "sim.multi_run.succeeded", "Count", 2, "monotonic");
    assert_stat(&stats, "sim.multi_run.failed", "Count", 0, "monotonic");
    assert_stat(
        &stats,
        "sim.multi_run.instructions.committed",
        "Count",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_multi_run_orchestrates_run_and_gups_configs() {
    let workspace = temp_workspace("multi-run-run-and-gups");
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    fs::write(
        workspace.join("program.elf"),
        riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
    .unwrap();
    fs::write(
        workspace.join("cpu.toml"),
        r#"[run]
isa = "riscv"
binary = "program.elf"
max_tick = 40
stats_format = "json"
execute = true
memory_system = "direct"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("traffic.toml"),
        r#"[gups]
memory_start = 4096
memory_size = 8
updates = 2
max_tick = 40
stats_format = "json"
rng_state = 0
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "mixed-smoke"
stats_format = "json"

[[multi_run.runs]]
id = "cpu"
command = "run"
config = "cpu.toml"

[[multi_run.runs]]
id = "traffic"
command = "gups"
config = "traffic.toml"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "multi-run",
            "--config",
            workspace.join("multi-run.toml").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.multi-run.v1\""));
    assert!(stdout.contains("\"suite_id\":\"mixed-smoke\""));
    assert!(stdout.contains("\"runs\":2"));
    assert!(stdout.contains("\"total_final_tick\":17"));
    assert!(stdout.contains("\"total_committed_instructions\":2"));
    assert!(stdout.contains("\"total_scheduled_requests\":4"));
    assert!(stdout.contains("\"id\":\"cpu\""));
    assert!(stdout.contains("\"command\":\"run\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.run.v1\""));
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"final_tick\":5"));
    assert!(stdout.contains("\"id\":\"traffic\""));
    assert!(stdout.contains("\"command\":\"gups\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.gups.v1\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"final_tick\":12"));
    assert!(stdout.contains("\"scheduled_requests\":4"));
    assert_stat(
        &stdout,
        "sim.multi_run.scheduled_requests",
        "Count",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_multi_run_rejects_duplicate_run_ids() {
    let workspace = temp_workspace("multi-run-duplicate-ids");
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "duplicate"

[[multi_run.runs]]
id = "same"
config = "first.toml"

[[multi_run.runs]]
id = "same"
config = "second.toml"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "multi-run",
            "--config",
            workspace.join("multi-run.toml").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("multi-run ids must be unique: same"));
}
