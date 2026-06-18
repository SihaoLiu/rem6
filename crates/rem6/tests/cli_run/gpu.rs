use std::process::Command;

use crate::support::{assert_stat, assert_transport_stats};

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
