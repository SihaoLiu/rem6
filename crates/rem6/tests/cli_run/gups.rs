use std::process::Command;

use crate::support::*;

#[test]
fn rem6_gups_executes_controller_transport_and_updates_memory() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gups",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "8",
            "--updates",
            "2",
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--rng-state",
            "0",
            "--dump-memory",
            "0x1000:8",
        ])
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
    assert!(stdout.contains("\"scheduled_requests\":4"));
    assert!(stdout.contains("\"final_tick\":12"));
    assert!(stdout.contains("\"profiles\":[{\"state\":0,\"generator_class\":\"gups\""));
    assert!(stdout.contains("\"memory_profile\":\"gups_table\""));
    assert!(stdout.contains("\"packet_count\":4"));
    assert!(stdout.contains("{\"state\":1,\"generator_class\":\"idle\""));
    assert!(stdout.contains("\"memory_profile\":\"no_memory\""));
    assert!(stdout.contains(
        "\"response_stats\":{\"responses\":4,\"completed\":4,\"retry\":0,\"store_conditional_failed\":0,\"reads\":2,\"writes\":2,\"data_bytes\":16}"
    ));
    assert!(stdout.contains("\"address\":\"0x1000\""));
    assert!(stdout.contains("\"hex\":\"0100000000000000\""));
    assert_transport_stats(&stdout, "sim.gups.transport", 4, 8, 2);
    assert_stat(
        &stdout,
        "sim.gups.scheduled_requests",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gups.responses.completed",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gups.response_data_bytes",
        "Byte",
        16,
        "monotonic",
    );
    assert_stat(&stdout, "sim.gups.traffic_profiles", "Count", 2, "constant");
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.generator_class",
        "Value",
        9,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.memory_profile",
        "Value",
        5,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.packets",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.reads",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.writes",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.bytes_read",
        "Byte",
        16,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.bytes_written",
        "Byte",
        16,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.first_tick",
        "Tick",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gups.traffic_profile0.last_tick",
        "Tick",
        10,
        "monotonic",
    );
}

#[test]
fn rem6_gups_loads_toml_config_and_cli_overrides_updates() {
    let config = temp_config(
        "gups-toml-config",
        "[gups]\nmemory_start = 4096\nmemory_size = 8\nupdates = 100\nmax_tick = 24\nstats_format = \"json\"\nrng_state = 0\nmemory_dumps = [\"0x1000:8\"]\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gups",
            "--config",
            config.to_str().unwrap(),
            "--updates",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.gups.v1\""));
    assert!(stdout.contains("\"memory_start\":\"0x1000\""));
    assert!(stdout.contains("\"updates\":2"));
    assert!(stdout.contains("\"final_tick\":12"));
    assert!(stdout.contains("\"address\":\"0x1000\""));
    assert!(stdout.contains("\"hex\":\"0100000000000000\""));
}

#[test]
fn rem6_gups_rejects_zero_updates_from_toml_config() {
    let config = temp_config(
        "gups-toml-invalid-updates",
        "[gups]\nmemory_start = 4096\nmemory_size = 8\nupdates = 0\nmax_tick = 24\nstats_format = \"json\"\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gups", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid GUPS updates 0"));
}

#[test]
fn rem6_gups_rejects_memory_size_not_multiple_of_element_size() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gups",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "10",
            "--updates",
            "1",
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
    assert!(stderr.contains("traffic GUPS memory size 10 is not a multiple of element size 8"));
}

#[test]
fn rem6_gups_rejects_unaligned_memory_start_before_worker_panic() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gups",
            "--memory-start",
            "0x100c",
            "--memory-size",
            "8",
            "--updates",
            "1",
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
    assert!(stderr.contains("GUPS memory start 0x100c is not aligned to element size 8"));
    assert!(!stderr.contains("panicked"));
    assert!(!stderr.contains("parallel worker"));
}

#[test]
fn rem6_gups_rejects_tick_budget_before_executing_requests() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gups",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "8",
            "--updates",
            "2",
            "--max-tick",
            "11",
            "--stats-format",
            "json",
            "--rng-state",
            "0",
            "--dump-memory",
            "0x1000:8",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("GUPS expected final tick 12 exceeds max tick 11"));
}

#[test]
fn rem6_gups_tick_budget_uses_capped_update_target() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gups",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "8",
            "--updates",
            "100",
            "--max-tick",
            "24",
            "--stats-format",
            "json",
            "--rng-state",
            "0",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"updates\":100"));
    assert!(stdout.contains("\"scheduled_requests\":8"));
    assert!(stdout.contains("\"final_tick\":24"));
}
