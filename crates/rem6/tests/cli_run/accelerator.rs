use std::{fs, process::Command};

use crate::support::{assert_stat, temp_workspace};

#[test]
fn rem6_accelerator_run_executes_npu_and_gpu_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "accelerator-run",
            "--engine",
            "7",
            "--lanes",
            "2",
            "--command-delay",
            "1",
            "--npu-inference",
            "10:4:3",
            "--gpu-kernel",
            "11:2:5",
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
    assert!(stdout.contains("\"schema\":\"rem6.cli.accelerator_run.v1\""));
    assert!(stdout.contains("\"engine\":7"));
    assert!(stdout.contains("\"lanes\":2"));
    assert!(stdout.contains("\"command_delay\":1"));
    assert!(stdout.contains("\"command_count\":2"));
    assert!(stdout.contains("\"npu_inference_command_count\":1"));
    assert!(stdout.contains("\"gpu_kernel_command_count\":1"));
    assert!(stdout.contains("\"completion_count\":2"));
    assert!(stdout.contains("\"npu_inference_completion_count\":1"));
    assert!(stdout.contains("\"gpu_kernel_completion_count\":1"));
    assert!(stdout.contains("\"trace_event_count\":6"));
    assert_stat(
        &stdout,
        "sim.accelerator_run.commands",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.accelerator_run.commands.npu_inference",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.accelerator_run.completions.gpu_kernel",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.accelerator_run.trace_events",
        "Count",
        6,
        "monotonic",
    );
}

#[test]
fn rem6_accelerator_run_loads_toml_config_and_writes_artifacts() {
    let workspace = temp_workspace("accelerator-run-config");
    let config = workspace.join("accelerator.toml");
    fs::write(
        &config,
        r#"[accelerator_run]
engine = 9
lanes = 2
command_delay = 1
stats_format = "json"
output = "artifacts/accelerator.json"
stats_output = "artifacts/accelerator-stats.json"
npu_inferences = ["20:5:2"]
gpu_kernels = ["21:3:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["accelerator-run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.output.v1\""));
    assert!(stdout.contains("artifacts/accelerator.json"));
    assert!(stdout.contains("artifacts/accelerator-stats.json"));

    let artifact = fs::read_to_string(workspace.join("artifacts/accelerator.json")).unwrap();
    assert!(artifact.contains("\"schema\":\"rem6.cli.accelerator_run.v1\""));
    assert!(artifact.contains("\"engine\":9"));
    assert!(artifact.contains("\"command_count\":2"));
    assert!(artifact.contains("\"completion_count\":2"));

    let stats = fs::read_to_string(workspace.join("artifacts/accelerator-stats.json")).unwrap();
    assert_stat(
        &stats,
        "sim.accelerator_run.commands",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stats,
        "sim.accelerator_run.completions",
        "Count",
        2,
        "monotonic",
    );
}
