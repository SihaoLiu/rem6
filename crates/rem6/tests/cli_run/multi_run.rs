use std::{fs, path::Path, process::Command};

use serde_json::Value;

use crate::support::*;

const M5_EXIT: u32 = 0x21;
const M5_CHECKPOINT: u32 = 0x43;

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
fn rem6_multi_run_orchestrates_run_gups_and_trace_replay_configs() {
    let workspace = temp_workspace("multi-run-run-gups-trace");
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
        workspace.join("packet.pb"),
        packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
            ],
        ),
    )
    .unwrap();
    fs::write(
        workspace.join("packet.toml"),
        r#"[trace_replay]
trace = "packet.pb"
route = "cpu0.fetch"
memory_start = 4096
memory_size = 4096
max_tick = 64
tick_frequency = 1000
line_bytes = 64
agent = 7
control_partition = 2
stats_format = "json"
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

[[multi_run.runs]]
id = "packet"
command = "trace-replay"
config = "packet.toml"
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
    assert!(stdout.contains("\"runs\":3"));
    assert!(stdout.contains("\"total_final_tick\":21"));
    assert!(stdout.contains("\"total_committed_instructions\":2"));
    assert!(stdout.contains("\"total_scheduled_requests\":5"));
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
    assert!(stdout.contains("\"id\":\"packet\""));
    assert!(stdout.contains("\"command\":\"trace-replay\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.trace_replay.v1\""));
    assert!(stdout.contains("\"final_tick\":4"));
    assert!(stdout.contains("\"scheduled_requests\":1"));
    assert_stat(
        &stdout,
        "sim.multi_run.scheduled_requests",
        "Count",
        5,
        "monotonic",
    );
}

#[test]
fn rem6_multi_run_reports_run_child_checkpoint_actions() {
    let workspace = temp_workspace("multi-run-run-child-checkpoint-actions");
    let program = riscv64_program(&[m5op(M5_CHECKPOINT), m5op(M5_EXIT)]);
    fs::write(
        workspace.join("program.elf"),
        riscv64_elf(0x8000_0000, 0x8000_0000, &program),
    )
    .unwrap();
    fs::write(
        workspace.join("checkpoint.toml"),
        r#"[run]
isa = "riscv"
binary = "program.elf"
max_tick = 80
stats_format = "json"
execute = true
memory_system = "direct"
output = "artifacts/checkpoint-run.json"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "checkpoint-suite"
stats_format = "json"

[[multi_run.runs]]
id = "checkpoint"
command = "run"
config = "checkpoint.toml"
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
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.get("total_checkpoints").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.get("total_checkpoint_restores")
            .and_then(Value::as_u64),
        Some(0)
    );
    let summary = multi_run_summary_by_id(&json, "checkpoint");
    assert_eq!(
        summary.get("checkpoint_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        summary
            .get("checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_stat(
        &stdout,
        "sim.multi_run.checkpoints",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.multi_run.checkpoint_restores",
        "Count",
        0,
        "monotonic",
    );

    let child_artifact = fs::read_to_string(workspace.join("artifacts/checkpoint-run.json"))
        .unwrap_or_else(|error| panic!("missing child artifact: {error}"));
    let child_json: Value = serde_json::from_str(&child_artifact).unwrap();
    assert_eq!(
        child_json
            .pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn rem6_multi_run_reports_trace_replay_child_checkpoint_actions() {
    let workspace = temp_workspace("multi-run-trace-child-checkpoint-actions");
    fs::write(
        workspace.join("packet.pb"),
        packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
            ],
        ),
    )
    .unwrap();
    fs::write(
        workspace.join("trace.toml"),
        r#"[trace_replay]
trace = "packet.pb"
route = "cpu0.trace"
memory_start = 4096
memory_size = 4096
max_tick = 64
tick_frequency = 1000
line_bytes = 64
agent = 7
control_partition = 2
host_checkpoints = ["1:trace-cp"]
host_checkpoint_restores = ["2:trace-cp"]
stats_format = "json"
output = "artifacts/trace-run.json"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "trace-checkpoint-suite"
stats_format = "json"

[[multi_run.runs]]
id = "trace"
command = "trace-replay"
config = "trace.toml"
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
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.get("total_checkpoints").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        json.get("total_checkpoint_restores")
            .and_then(Value::as_u64),
        Some(1)
    );
    let summary = multi_run_summary_by_id(&json, "trace");
    assert_eq!(
        summary.get("checkpoint_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        summary
            .get("checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_stat(
        &stdout,
        "sim.multi_run.checkpoints",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.multi_run.checkpoint_restores",
        "Count",
        1,
        "monotonic",
    );

    let child_artifact = fs::read_to_string(workspace.join("artifacts/trace-run.json"))
        .unwrap_or_else(|error| panic!("missing trace child artifact: {error}"));
    let child_json: Value = serde_json::from_str(&child_artifact).unwrap();
    assert_eq!(
        child_json
            .pointer("/simulation/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        child_json
            .pointer("/simulation/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_nonzero_json_u64(&child_json, "/simulation/checkpoint_component_count");
    assert_nonzero_json_u64(&child_json, "/simulation/checkpoint_chunk_count");
    assert_nonzero_json_u64(&child_json, "/simulation/checkpoint_payload_bytes");
    assert_nonzero_json_u64(
        &child_json,
        "/simulation/checkpoint_restored_component_count",
    );
    assert_nonzero_json_u64(&child_json, "/simulation/checkpoint_restored_chunk_count");
    assert_nonzero_json_u64(&child_json, "/simulation/checkpoint_restored_payload_bytes");
}

#[test]
fn rem6_multi_run_preserves_child_output_artifacts() {
    let workspace = temp_workspace("multi-run-child-output-artifacts");
    fs::write(
        workspace.join("traffic.toml"),
        r#"[gups]
memory_start = 4096
memory_size = 8
updates = 2
max_tick = 40
stats_format = "json"
rng_state = 0
output = "artifacts/gups.json"
stats_output = "artifacts/gups-stats.json"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "child-output-smoke"
stats_format = "json"

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
    assert!(stdout.contains("\"suite_id\":\"child-output-smoke\""));
    assert!(stdout.contains("\"runs\":1"));
    assert!(stdout.contains("\"succeeded\":1"));
    assert!(stdout.contains(&format!(
        "\"artifact\":\"{}\"",
        workspace.join("artifacts/gups.json").display()
    )));
    assert!(stdout.contains(&format!(
        "\"stats_artifact\":\"{}\"",
        workspace.join("artifacts/gups-stats.json").display()
    )));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_summary_extra_artifacts(multi_run_summary_by_id(&json, "traffic"), &[]);

    let child_artifact = fs::read_to_string(workspace.join("artifacts/gups.json")).unwrap();
    let child_stats = fs::read_to_string(workspace.join("artifacts/gups-stats.json")).unwrap();
    assert!(child_artifact.contains("\"schema\":\"rem6.cli.gups.v1\""));
    assert!(child_artifact.contains("\"scheduled_requests\":4"));
    assert_stat(
        &child_stats,
        "sim.gups.scheduled_requests",
        "Count",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_multi_run_orchestrates_gpu_run_config() {
    let workspace = temp_workspace("multi-run-gpu-run");
    fs::write(
        workspace.join("gpu.toml"),
        r#"[gpu_run]
workgroups = 2
compute_units = 2
memory_start = 4096
memory_size = 64
max_tick = 80
stats_format = "json"
dram_memory = true
data_cache_protocol = "msi"
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "gpu-smoke"
stats_format = "json"

[[multi_run.runs]]
id = "gpu"
command = "gpu-run"
config = "gpu.toml"
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
    assert!(stdout.contains("\"suite_id\":\"gpu-smoke\""));
    assert!(stdout.contains("\"runs\":1"));
    assert!(stdout.contains("\"succeeded\":1"));
    assert!(stdout.contains("\"id\":\"gpu\""));
    assert!(stdout.contains("\"command\":\"gpu-run\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"executed\":true"));
    assert!(stdout.contains("\"committed_instructions\":0"));
    assert!(stdout.contains("\"scheduled_requests\":2"));
    assert_stat(
        &stdout,
        "sim.multi_run.scheduled_requests",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_multi_run_reports_gpu_child_extra_artifacts() {
    let workspace = temp_workspace("multi-run-gpu-extra-artifacts");
    fs::write(
        workspace.join("gpu.toml"),
        r#"[gpu_run]
workgroups = 2
compute_units = 2
memory_start = 14336
memory_size = 64
max_tick = 80
stats_format = "json"
dram_memory = true
data_cache_protocol = "msi"
power_format = "dsent-csv"
power_output = "artifacts/gpu-power.csv"
nomali_output = "artifacts/gpu-nomali.json"
global_loads = ["0x3800:4:4:4"]
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "gpu-extra-artifact-smoke"
stats_format = "json"

[[multi_run.runs]]
id = "gpu"
command = "gpu-run"
config = "gpu.toml"
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
    let power_path = workspace.join("artifacts/gpu-power.csv");
    let nomali_path = workspace.join("artifacts/gpu-nomali.json");
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.get("suite_id").and_then(Value::as_str),
        Some("gpu-extra-artifact-smoke")
    );
    let gpu_summary = multi_run_summary_by_id(&json, "gpu");
    assert_eq!(
        gpu_summary.get("command").and_then(Value::as_str),
        Some("gpu-run")
    );
    assert_summary_extra_artifacts(
        gpu_summary,
        &[
            ("power_artifact", power_path.as_path()),
            ("nomali_artifact", nomali_path.as_path()),
        ],
    );

    let power = fs::read_to_string(power_path).unwrap();
    let nomali = fs::read_to_string(nomali_path).unwrap();
    assert!(power.contains("gpu.compute_unit0"));
    assert!(power.contains("gpu.data_cache"));
    assert!(nomali.contains("\"schema\":\"rem6.nomali.gpu-adapter.v1\""));
    assert!(nomali.contains("\"workgroup_completions\":2"));
}

#[test]
fn rem6_multi_run_reports_run_and_trace_child_power_artifacts() {
    let workspace = temp_workspace("multi-run-run-trace-power-artifacts");
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
        workspace.join("packet.pb"),
        packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
            ],
        ),
    )
    .unwrap();
    fs::write(
        workspace.join("run.toml"),
        r#"[run]
isa = "riscv"
binary = "program.elf"
max_tick = 40
stats_format = "json"
execute = true
memory_system = "direct"
power_output = "artifacts/run-power.xml"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("trace.toml"),
        r#"[trace_replay]
trace = "packet.pb"
route = "cpu0.power"
memory_start = 4096
memory_size = 4096
max_tick = 64
tick_frequency = 1000
line_bytes = 64
agent = 7
control_partition = 2
stats_format = "json"
power_format = "dsent-csv"
power_output = "artifacts/trace-power.csv"
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "run-trace-power-artifact-smoke"
stats_format = "json"

[[multi_run.runs]]
id = "cpu"
command = "run"
config = "run.toml"

[[multi_run.runs]]
id = "packet"
command = "trace-replay"
config = "trace.toml"
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
    let run_power_path = workspace.join("artifacts/run-power.xml");
    let trace_power_path = workspace.join("artifacts/trace-power.csv");
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.get("suite_id").and_then(Value::as_str),
        Some("run-trace-power-artifact-smoke")
    );
    assert_summary_extra_artifacts(
        multi_run_summary_by_id(&json, "cpu"),
        &[("power_artifact", run_power_path.as_path())],
    );
    assert_summary_extra_artifacts(
        multi_run_summary_by_id(&json, "packet"),
        &[("power_artifact", trace_power_path.as_path())],
    );

    let run_power = fs::read_to_string(run_power_path).unwrap();
    let trace_power = fs::read_to_string(trace_power_path).unwrap();
    assert!(run_power.contains("<component id=\"cpu0.core\""));
    assert!(trace_power.starts_with("record_type,tick,target,state,temperature_c"));
    assert!(trace_power.contains("total,4,__total__,All"));
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
}

fn multi_run_summary_by_id<'a>(json: &'a Value, id: &str) -> &'a Value {
    json.get("run_summaries")
        .and_then(Value::as_array)
        .and_then(|summaries| {
            summaries
                .iter()
                .find(|summary| summary.get("id").and_then(Value::as_str) == Some(id))
        })
        .unwrap_or_else(|| panic!("missing multi-run summary {id}"))
}

fn assert_summary_extra_artifacts(summary: &Value, expected: &[(&str, &Path)]) {
    let artifacts = summary
        .get("extra_artifacts")
        .and_then(Value::as_array)
        .expect("missing extra_artifacts array");
    assert_eq!(artifacts.len(), expected.len());
    for (artifact, (expected_name, expected_path)) in artifacts.iter().zip(expected) {
        assert_eq!(
            artifact.get("name").and_then(Value::as_str),
            Some(*expected_name)
        );
        let expected_path = expected_path.display().to_string();
        assert_eq!(
            artifact.get("artifact").and_then(Value::as_str),
            Some(expected_path.as_str())
        );
    }
}

fn assert_nonzero_json_u64(json: &Value, path: &str) {
    let value = json
        .pointer(path)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing nonzero JSON value {path}: {json}"));
    assert!(value > 0, "expected {path} to be nonzero in {json}");
}

#[test]
fn rem6_multi_run_run_flag_accepts_command_qualified_entries() {
    let workspace = temp_workspace("multi-run-flag-command-qualified");
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
        workspace.join("packet.pb"),
        packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
            ],
        ),
    )
    .unwrap();
    fs::write(
        workspace.join("packet.toml"),
        r#"[trace_replay]
trace = "packet.pb"
route = "cpu0.fetch"
memory_start = 4096
memory_size = 4096
max_tick = 64
tick_frequency = 1000
line_bytes = 64
agent = 7
control_partition = 2
stats_format = "json"
"#,
    )
    .unwrap();

    let gups_run = format!("traffic:gups:{}", workspace.join("traffic.toml").display());
    let trace_run = format!(
        "packet:trace-replay:{}",
        workspace.join("packet.toml").display()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "multi-run",
            "--suite-id",
            "flag-mixed-smoke",
            "--stats-format",
            "json",
            "--run",
            &gups_run,
            "--run",
            &trace_run,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"suite_id\":\"flag-mixed-smoke\""));
    assert!(stdout.contains("\"runs\":2"));
    assert!(stdout.contains("\"total_final_tick\":16"));
    assert!(stdout.contains("\"total_scheduled_requests\":5"));
    assert!(stdout.contains("\"id\":\"traffic\""));
    assert!(stdout.contains("\"command\":\"gups\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.gups.v1\""));
    assert!(stdout.contains("\"id\":\"packet\""));
    assert!(stdout.contains("\"command\":\"trace-replay\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.trace_replay.v1\""));
}

#[test]
fn rem6_multi_run_run_flag_accepts_gpu_run_command_qualified_entry() {
    let workspace = temp_workspace("multi-run-flag-gpu-run");
    fs::write(
        workspace.join("gpu.toml"),
        r#"[gpu_run]
workgroups = 2
compute_units = 2
memory_start = 4096
memory_size = 64
max_tick = 80
stats_format = "json"
dram_memory = true
data_cache_protocol = "msi"
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let gpu_run = format!("gpu:gpu-run:{}", workspace.join("gpu.toml").display());
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "multi-run",
            "--suite-id",
            "flag-gpu-smoke",
            "--stats-format",
            "json",
            "--run",
            &gpu_run,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"suite_id\":\"flag-gpu-smoke\""));
    assert!(stdout.contains("\"runs\":1"));
    assert!(stdout.contains("\"total_scheduled_requests\":2"));
    assert!(stdout.contains("\"id\":\"gpu\""));
    assert!(stdout.contains("\"command\":\"gpu-run\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(stdout.contains("\"scheduled_requests\":2"));
}

#[test]
fn rem6_multi_run_orchestrates_accelerator_run_config() {
    let workspace = temp_workspace("multi-run-accelerator-run");
    fs::write(
        workspace.join("accelerator.toml"),
        r#"[accelerator_run]
engine = 7
lanes = 2
command_delay = 1
stats_format = "json"
output = "artifacts/accelerator.json"
stats_output = "artifacts/accelerator-stats.json"
npu_inferences = ["10:4:3"]
gpu_kernels = ["11:2:5"]
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("multi-run.toml"),
        r#"[multi_run]
suite_id = "accelerator-suite"
stats_format = "json"

[[multi_run.runs]]
id = "accelerator"
command = "accelerator-run"
config = "accelerator.toml"
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
    assert!(stdout.contains("\"suite_id\":\"accelerator-suite\""));
    assert!(stdout.contains("\"runs\":1"));
    assert!(stdout.contains("\"succeeded\":1"));
    assert!(stdout.contains("\"total_scheduled_requests\":2"));
    assert!(stdout.contains("\"id\":\"accelerator\""));
    assert!(stdout.contains("\"command\":\"accelerator-run\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.accelerator_run.v1\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"scheduled_requests\":2"));
    assert!(stdout.contains("artifacts/accelerator.json"));
    assert!(stdout.contains("artifacts/accelerator-stats.json"));
    assert_stat(
        &stdout,
        "sim.multi_run.scheduled_requests",
        "Count",
        2,
        "monotonic",
    );

    let child_artifact = fs::read_to_string(workspace.join("artifacts/accelerator.json")).unwrap();
    assert!(child_artifact.contains("\"schema\":\"rem6.cli.accelerator_run.v1\""));
    assert!(child_artifact.contains("\"command_count\":2"));
    assert!(child_artifact.contains("\"completion_count\":2"));

    let child_stats =
        fs::read_to_string(workspace.join("artifacts/accelerator-stats.json")).unwrap();
    assert_stat(
        &child_stats,
        "sim.accelerator_run.commands",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_multi_run_continue_on_failure_records_failed_child_and_runs_remaining_entries() {
    let workspace = temp_workspace("multi-run-continue-on-failure");
    fs::write(
        workspace.join("bad-gups.toml"),
        r#"[gups]
memory_size = 8
updates = 2
max_tick = 40
stats_format = "json"
rng_state = 0
"#,
    )
    .unwrap();
    fs::write(
        workspace.join("good-gups.toml"),
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
suite_id = "continue-on-failure"
stats_format = "json"
continue_on_failure = true

[[multi_run.runs]]
id = "bad"
command = "gups"
config = "bad-gups.toml"

[[multi_run.runs]]
id = "good"
command = "gups"
config = "good-gups.toml"
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
    assert!(stdout.contains("\"suite_id\":\"continue-on-failure\""));
    assert!(stdout.contains("\"runs\":2"));
    assert!(stdout.contains("\"succeeded\":1"));
    assert!(stdout.contains("\"failed\":1"));
    assert!(stdout.contains("\"id\":\"bad\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.error.v1\""));
    assert!(stdout.contains("\"status\":\"failed\""));
    assert!(stdout.contains("\"executed\":false"));
    assert!(stdout.contains("\"error\":\"missing required flag --memory-start\""));
    assert!(stdout.contains("\"id\":\"good\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.gups.v1\""));
    assert!(stdout.contains("\"scheduled_requests\":4"));
    assert!(stdout.contains("\"total_scheduled_requests\":4"));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_summary_extra_artifacts(multi_run_summary_by_id(&json, "bad"), &[]);
    assert_summary_extra_artifacts(multi_run_summary_by_id(&json, "good"), &[]);
    assert_stat(&stdout, "sim.multi_run.succeeded", "Count", 1, "monotonic");
    assert_stat(&stdout, "sim.multi_run.failed", "Count", 1, "monotonic");
    assert_stat(
        &stdout,
        "sim.multi_run.scheduled_requests",
        "Count",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_multi_run_run_flag_accepts_continue_on_failure() {
    let workspace = temp_workspace("multi-run-flag-continue-on-failure");
    fs::write(
        workspace.join("bad-gups.toml"),
        r#"[gups]
memory_size = 8
updates = 2
max_tick = 40
stats_format = "json"
rng_state = 0
"#,
    )
    .unwrap();

    let bad_run = format!("bad:gups:{}", workspace.join("bad-gups.toml").display());
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "multi-run",
            "--suite-id",
            "flag-continue-on-failure",
            "--stats-format",
            "json",
            "--continue-on-failure",
            "--run",
            &bad_run,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"suite_id\":\"flag-continue-on-failure\""));
    assert!(stdout.contains("\"runs\":1"));
    assert!(stdout.contains("\"succeeded\":0"));
    assert!(stdout.contains("\"failed\":1"));
    assert!(stdout.contains("\"id\":\"bad\""));
    assert!(stdout.contains("\"child_schema\":\"rem6.cli.error.v1\""));
    assert!(stdout.contains("\"error\":\"missing required flag --memory-start\""));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_summary_extra_artifacts(multi_run_summary_by_id(&json, "bad"), &[]);
    assert_stat(&stdout, "sim.multi_run.failed", "Count", 1, "monotonic");
}

#[test]
fn rem6_multi_run_fails_fast_by_default_on_child_error() {
    let workspace = temp_workspace("multi-run-fail-fast-default");
    fs::write(
        workspace.join("bad-gups.toml"),
        r#"[gups]
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
suite_id = "fail-fast-default"
stats_format = "json"

[[multi_run.runs]]
id = "bad"
command = "gups"
config = "bad-gups.toml"
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
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing required flag --memory-start"));
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
