use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::support::{assert_stat, temp_workspace};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("rem6 crate lives under workspace crates directory")
        .to_path_buf()
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

#[test]
fn repository_gups_example_config_runs_without_recompilation() {
    let config = workspace_root().join("examples/gups/basic.toml");
    assert!(config.is_file(), "missing {}", config.display());

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gups", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.gups.v1\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"memory_start\":\"0x1000\""));
    assert!(stdout.contains("\"memory_size\":8"));
    assert!(stdout.contains("\"updates\":2"));
    assert!(stdout.contains("\"final_tick\":12"));
    assert!(stdout.contains("\"address\":\"0x1000\""));
    assert!(stdout.contains("\"hex\":\"0100000000000000\""));
}

#[test]
fn repository_gpu_run_example_config_runs_without_recompilation() {
    let config = workspace_root().join("examples/gpu/basic.toml");
    assert!(config.is_file(), "missing {}", config.display());

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"workgroups\":2"));
    assert!(stdout.contains("\"compute_units\":2"));
    assert!(stdout.contains("\"coalesced_memory_accesses\":6"));
    assert!(stdout.contains("\"global_memory_requests\":6"));
    assert!(stdout.contains("\"global_memory_reads\":1"));
    assert!(stdout.contains("\"global_memory_writes\":2"));
    assert!(stdout.contains("\"data_cache_protocol\":\"msi\""));
    assert!(stdout.contains("\"data_cache_runs\":6"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":2"));
    assert!(stdout.contains("\"address\":\"0x1000\""));
    assert!(stdout.contains("\"hex\":\"a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5\""));
}

#[test]
fn repository_accelerator_run_example_config_runs_without_recompilation() {
    let config = workspace_root().join("examples/accelerator/basic.toml");
    assert!(config.is_file(), "missing {}", config.display());

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
        "sim.accelerator_run.completions",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn repository_multi_run_example_suite_writes_aggregate_and_child_artifacts() {
    let example = workspace_root().join("examples/multi-run");
    assert!(
        example.join("basic.toml").is_file(),
        "missing {}",
        example.join("basic.toml").display()
    );

    let workspace = temp_workspace("repository-multi-run-example");
    copy_dir_recursive(&example, &workspace).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "multi-run",
            "--config",
            workspace.join("basic.toml").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.output.v1\""));
    assert!(stdout.contains("artifacts/multi-run.json"));
    assert!(stdout.contains("artifacts/multi-run-stats.json"));

    let artifact_path = workspace.join("artifacts/multi-run.json");
    let stats_path = workspace.join("artifacts/multi-run-stats.json");
    let artifact = fs::read_to_string(&artifact_path).unwrap();
    let stats = fs::read_to_string(&stats_path).unwrap();
    let json: Value = serde_json::from_str(&artifact).unwrap();
    assert_eq!(
        json.get("schema").and_then(Value::as_str),
        Some("rem6.cli.multi-run.v1")
    );
    assert_eq!(
        json.get("suite_id").and_then(Value::as_str),
        Some("repository-basic-suite")
    );
    assert_eq!(json.get("runs").and_then(Value::as_u64), Some(3));
    assert_eq!(json.get("succeeded").and_then(Value::as_u64), Some(3));
    assert_eq!(json.get("failed").and_then(Value::as_u64), Some(0));
    assert_eq!(
        json.get("total_accelerator_commands")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        json.get("total_accelerator_completions")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert!(artifact.contains("\"id\":\"traffic\""));
    assert!(artifact.contains("\"id\":\"gpu\""));
    assert!(artifact.contains("\"id\":\"accelerator\""));
    assert!(artifact.contains("gpu-power.csv"));
    assert!(artifact.contains("gpu-nomali.json"));

    assert!(workspace.join("artifacts/gups.json").is_file());
    assert!(workspace.join("artifacts/gups-stats.json").is_file());
    assert!(workspace.join("artifacts/gpu.json").is_file());
    assert!(workspace.join("artifacts/gpu-stats.json").is_file());
    assert!(workspace.join("artifacts/gpu-power.csv").is_file());
    assert!(workspace.join("artifacts/gpu-nomali.json").is_file());
    assert!(workspace.join("artifacts/accelerator.json").is_file());
    assert!(workspace.join("artifacts/accelerator-stats.json").is_file());
    assert_stat(&stats, "sim.multi_run.runs", "Count", 3, "constant");
    assert_stat(&stats, "sim.multi_run.succeeded", "Count", 3, "monotonic");
    assert_stat(&stats, "sim.multi_run.failed", "Count", 0, "monotonic");
    assert_stat(
        &stats,
        "sim.multi_run.accelerator.commands",
        "Count",
        2,
        "monotonic",
    );
}
