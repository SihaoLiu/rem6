use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("rem6 crate lives under workspace crates directory")
        .to_path_buf()
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
