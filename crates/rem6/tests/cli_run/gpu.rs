use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rem6::Rem6GpuRunConfig;

use crate::support::{assert_stat, assert_transport_stats};

fn unique_gpu_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rem6-gpu-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[test]
fn rem6_gpu_run_routes_coalesced_global_memory_through_cache_and_dram() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "2",
            "--global-load",
            "0x1000:4:4:4",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "64",
            "--data-cache-protocol",
            "msi",
            "--dram-memory",
            "--max-tick",
            "80",
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
    assert!(stdout.contains("\"schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"workgroups\":2"));
    assert!(stdout.contains("\"workgroup_completions\":2"));
    assert!(stdout.contains("\"coalesced_memory_accesses\":2"));
    assert!(stdout.contains("\"global_memory_requests\":2"));
    assert!(stdout.contains("\"data_cache_protocol\":\"msi\""));
    assert!(stdout.contains("\"data_cache_runs\":2"));
    assert!(stdout.contains("\"data_cache_msi_runs\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":1"));
    assert!(stdout.contains("\"accesses\":1"));
    assert!(stdout.contains("\"reads\":1"));
    assert_transport_stats(&stdout, "sim.gpu_run.transport", 2, 4, 2);
    assert_stat(
        &stdout,
        "sim.gpu_run.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.coalesced_memory_accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.runs",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.dram_accesses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.memory.dram.accesses",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_routes_recorded_store_to_direct_memory_and_dumps_result() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--compute-units",
            "1",
            "--global-store",
            "0x2000:4:4:4",
            "--memory-start",
            "0x2000",
            "--memory-size",
            "16",
            "--dump-memory",
            "0x2000:16",
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
    assert!(stdout.contains("\"schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(stdout.contains("\"data_cache_protocol\":null"));
    assert!(stdout.contains("\"global_memory_requests\":1"));
    assert!(stdout.contains("\"memory_responses\":1"));
    assert!(stdout.contains("\"data_cache_runs\":0"));
    assert!(stdout.contains("\"active_targets\":0"));
    assert!(stdout.contains(
        "\"memory\":[{\"address\":\"0x2000\",\"bytes\":16,\"hex\":\"a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5\"}]"
    ));
    assert_transport_stats(&stdout, "sim.gpu_run.transport", 1, 2, 2);
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.runs",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.memory.dram.accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(&stdout, "sim.gpu_run.memory.dumps", "Count", 1, "constant");
}

#[test]
fn rem6_gpu_run_reports_per_compute_unit_activity() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "5",
            "--compute-units",
            "2",
            "--wave-slots-per-compute-unit",
            "1",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3000:4:4:4",
            "--memory-start",
            "0x3000",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
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
    assert!(stdout.contains("\"workgroup_completions\":5"));
    assert!(stdout.contains(
        "\"compute_unit_activity\":[{\"compute_unit\":0,\"workgroup_completions\":3,\"busy_cycles\":12"
    ));
    assert!(stdout.contains("{\"compute_unit\":1,\"workgroup_completions\":2,\"busy_cycles\":8"));
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.workgroup_completions",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.busy_cycles",
        "Cycle",
        12,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.busy_cycles",
        "Cycle",
        8,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_merges_overlapping_wave_slots_for_compute_unit_busy_cycles() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "1",
            "--wave-slots-per-compute-unit",
            "2",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3400:4:4:4",
            "--memory-start",
            "0x3400",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
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
    assert!(stdout.contains(
        "\"compute_unit_activity\":[{\"compute_unit\":0,\"workgroup_completions\":2,\"busy_cycles\":4,\"first_started_at\":1,\"last_completed_at\":5"
    ));
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.busy_cycles",
        "Cycle",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_accepts_toml_config_for_top_level_execution() {
    let temp_dir = unique_gpu_temp_dir("config");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
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

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

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
    assert!(stdout.contains("\"global_memory_requests\":2"));
    assert!(stdout.contains("\"data_cache_protocol\":\"msi\""));
    assert!(stdout.contains("\"data_cache_msi_runs\":2"));
    assert!(stdout.contains("\"accesses\":1"));
    assert_transport_stats(&stdout, "sim.gpu_run.transport", 2, 4, 2);
    assert_stat(
        &stdout,
        "sim.gpu_run.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_writes_toml_relative_output_files() {
    let temp_dir = unique_gpu_temp_dir("relative-output");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    let artifact_path = temp_dir.join("artifacts/gpu.json");
    let stats_path = temp_dir.join("artifacts/gpu-stats.json");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 4096
memory_size = 64
max_tick = 80
stats_format = "json"
output = "artifacts/gpu.json"
stats_output = "artifacts/gpu-stats.json"
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
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
    let artifact = std::fs::read_to_string(&artifact_path).unwrap();
    let stats = std::fs::read_to_string(&stats_path).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(artifact.contains("\"schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(artifact.contains("\"status\":\"completed\""));
    assert!(artifact.contains("\"workgroups\":1"));
    assert_stat(
        &stats,
        "sim.gpu_run.workgroup_completions",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_merges_toml_stores_with_cli_loads() {
    let temp_dir = unique_gpu_temp_dir("mixed-access");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 8192
memory_size = 64
max_tick = 80
stats_format = "json"
global_stores = ["0x2000:4:4:4"]
memory_dumps = ["0x2000:16"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .args(["--global-load", "0x2010:4:4:4"])
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"global_memory_requests\":2"));
    assert!(stdout.contains("\"memory_responses\":2"));
    assert!(stdout.contains(
        "\"memory\":[{\"address\":\"0x2000\",\"bytes\":16,\"hex\":\"a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5\"}]"
    ));
}

#[test]
fn rem6_gpu_run_rejects_toml_output_stats_output_path_conflict() {
    let temp_dir = unique_gpu_temp_dir("output-conflict");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 4096
memory_size = 64
max_tick = 80
output = "same.json"
stats_output = "same.json"
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--output and --stats-output must use different paths"));
}

#[test]
fn rem6_gpu_run_config_scan_skips_value_that_matches_config_flag() {
    let temp_dir = unique_gpu_temp_dir("config-scan");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 4096
memory_size = 64
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let config = Rem6GpuRunConfig::parse_args(vec![
        "gpu-run".to_string(),
        "--output".to_string(),
        "--config".to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
    ])
    .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert_eq!(config.output().unwrap(), std::path::Path::new("--config"));
    assert_eq!(config.workgroups(), 1);
}

#[test]
fn rem6_gpu_run_rejects_zero_workgroups_from_toml_config() {
    let temp_dir = unique_gpu_temp_dir("bad-config");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 0
memory_start = 4096
memory_size = 64
max_tick = 80
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--workgroups must be positive, got 0"));
}
