use std::process::Command;

use crate::support::*;

#[test]
fn rem6_trace_replay_executes_packet_trace_and_emits_summary_stats() {
    let trace = temp_trace(
        "trace-replay",
        &packet_trace_bytes(
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
                PacketFields {
                    tick: 4,
                    command: GEM5_READ_REQ,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(11),
                },
                PacketFields {
                    tick: 6,
                    command: GEM5_READ_ERROR,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(11),
                },
                PacketFields {
                    tick: 7,
                    command: GEM5_MEM_FENCE_REQ,
                    address: None,
                    size: None,
                    packet_id: Some(12),
                },
                PacketFields {
                    tick: 9,
                    command: GEM5_MEM_FENCE_RESP,
                    address: None,
                    size: None,
                    packet_id: Some(12),
                },
            ],
        ),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "trace-replay",
            "--trace",
            trace.to_str().unwrap(),
            "--route",
            "cpu0.fetch",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "0x1000",
            "--max-tick",
            "64",
            "--tick-frequency",
            "1000",
            "--line-bytes",
            "64",
            "--agent",
            "7",
            "--control-partition",
            "2",
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
    assert!(stdout.contains("\"schema\":\"rem6.cli.trace_replay.v1\""));
    assert!(stdout.contains("\"generator\":\"trace-replay\""));
    assert!(stdout.contains("\"route\":\"cpu0.fetch\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"scheduled_count\":3"));
    assert!(stdout.contains("\"response_delivery_count\":1"));
    assert!(stdout.contains("\"trace_completed_response_count\":1"));
    assert!(stdout.contains("\"trace_read_response_count\":1"));
    assert!(stdout.contains("\"trace_response_data_byte_count\":8"));
    assert!(stdout.contains("\"trace_response_fill_data_byte_count\":8"));
    assert!(stdout.contains("\"memory_failure_count\":1"));
    assert!(stdout.contains("\"memory_failure_read_count\":1"));
    assert!(stdout.contains("\"control_ack_count\":1"));
    assert!(stdout.contains("\"sync_control_ack_count\":1"));
    assert_stat_id(&stdout, "sim.trace_replay.response_data_bytes", 17);
    assert_stat_id(&stdout, "sim.trace_replay.sideband_events", 28);
    assert_stat(
        &stdout,
        "sim.trace_replay.scheduled",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.responses.completed",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.memory_failures.read",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.control_acks.sync",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_fabric_route_emits_activity_stats() {
    let trace = temp_trace(
        "trace-replay-fabric-route",
        &packet_trace_bytes(
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
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "trace-replay",
            "--trace",
            trace.to_str().unwrap(),
            "--route",
            "cpu0.fetch",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "0x1000",
            "--max-tick",
            "64",
            "--tick-frequency",
            "1000",
            "--line-bytes",
            "64",
            "--agent",
            "7",
            "--control-partition",
            "2",
            "--fabric-link",
            "cpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "4",
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
    assert!(stdout.contains("\"active_fabric_lane_count\":1"));
    assert!(stdout.contains("\"fabric_transfer_count\":2"));
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.active_lanes",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.transfers",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.bytes",
        "Byte",
        16,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_loads_toml_config_relative_trace_and_cli_route_override() {
    let workspace = temp_workspace("trace-replay-toml-config");
    let trace_name = format!("trace-{}.pb", std::process::id());
    let trace = workspace.join(&trace_name);
    std::fs::write(
        &trace,
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
    let config = workspace.join("trace-replay.toml");
    std::fs::write(
        &config,
        format!(
            "[trace_replay]\ntrace = \"{}\"\nroute = \"cpu0.config\"\nmemory_start = 4096\nmemory_size = 4096\nmax_tick = 64\ntick_frequency = 1000\nline_bytes = 64\nagent = 7\ncontrol_partition = 2\nfabric_link = \"cpu_mem\"\nfabric_bandwidth_bytes_per_tick = 4\nstats_format = \"json\"\n",
            trace_name
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "trace-replay",
            "--config",
            config.to_str().unwrap(),
            "--route",
            "cpu0.fetch",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.trace_replay.v1\""));
    assert!(stdout.contains("\"route\":\"cpu0.fetch\""));
    assert!(!stdout.contains("\"route\":\"cpu0.config\""));
    assert!(stdout.contains("\"scheduled_count\":1"));
    assert!(stdout.contains("\"trace_read_response_count\":1"));
    assert!(stdout.contains("\"trace_response_data_byte_count\":8"));
    assert!(stdout.contains("\"active_fabric_lane_count\":1"));
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.transfers",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_loads_trace_payload_from_resource_config() {
    let workspace = temp_workspace("trace-replay-resource-config");
    let trace_dir = workspace.join("artifacts");
    std::fs::create_dir(&trace_dir).unwrap();
    let trace_name = "trace.pb";
    std::fs::write(
        trace_dir.join(trace_name),
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
    let resource_config = workspace.join("resource-acquire.toml");
    std::fs::write(
        &resource_config,
        format!(
            "[resource_acquire]\nworkload_id = \"trace-resource-cli\"\nboot_entry = 4096\nstats_format = \"json\"\n\n[[resource_acquire.resources]]\nid = \"trace\"\nkind = \"input\"\ndigest = \"sha256:trace-resource\"\nlocator = \"resources/trace.pb\"\nrequired = true\nacquisition_kind = \"local-file\"\nacquisition_locator = \"catalog://trace\"\nartifact = \"artifacts/{trace_name}\"\nartifact_digest = \"sha256:trace-resource\"\n",
        ),
    )
    .unwrap();
    let config = workspace.join("trace-replay.toml");
    std::fs::write(
        &config,
        "[trace_replay]\nresource_config = \"resource-acquire.toml\"\nroute = \"cpu0.resource\"\nmemory_start = 4096\nmemory_size = 4096\nmax_tick = 64\ntick_frequency = 1000\nline_bytes = 64\nagent = 7\ncontrol_partition = 2\nstats_format = \"json\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["trace-replay", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.trace_replay.v1\""));
    assert!(stdout.contains("\"route\":\"cpu0.resource\""));
    assert!(stdout.contains("\"scheduled_count\":1"));
    assert!(stdout.contains("\"trace_read_response_count\":1"));
    assert!(stdout.contains("\"trace_response_data_byte_count\":8"));
    assert_stat(
        &stdout,
        "sim.trace_replay.scheduled",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_loads_trace_payload_from_suite_resource_config() {
    let workspace = temp_workspace("trace-replay-suite-resource-config");
    let trace_dir = workspace.join("artifacts");
    std::fs::create_dir(&trace_dir).unwrap();
    std::fs::write(
        trace_dir.join("trace.pb"),
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
    std::fs::write(workspace.join("kernel.bin"), [0x13, 0x00, 0x00, 0x00]).unwrap();
    let resource_config = workspace.join("resource-acquire-suite.toml");
    std::fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "trace-suite-cli"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "trace-workload"
boot_entry = 4096

[[resource_acquire.manifests.resources]]
id = "trace"
kind = "input"
digest = "sha256:trace-resource"
locator = "resources/trace.pb"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://trace"
artifact = "artifacts/trace.pb"
artifact_digest = "sha256:trace-resource"

[[resource_acquire.manifests]]
workload_id = "side-workload"
boot_entry = 8192

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:kernel-resource"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.bin"
artifact_digest = "sha256:kernel-resource"
artifact_size = 4
"#,
    )
    .unwrap();
    let config = workspace.join("trace-replay.toml");
    std::fs::write(
        &config,
        "[trace_replay]\nresource_config = \"resource-acquire-suite.toml\"\nroute = \"cpu0.suite-resource\"\nmemory_start = 4096\nmemory_size = 4096\nmax_tick = 64\ntick_frequency = 1000\nline_bytes = 64\nagent = 7\ncontrol_partition = 2\nstats_format = \"json\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["trace-replay", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.trace_replay.v1\""));
    assert!(stdout.contains("\"trace\":\"resource-config:"));
    assert!(stdout.contains("resource-acquire-suite.toml"));
    assert!(stdout.contains("\"route\":\"cpu0.suite-resource\""));
    assert!(stdout.contains("\"scheduled_count\":1"));
    assert!(stdout.contains("\"trace_read_response_count\":1"));
    assert!(stdout.contains("\"trace_response_data_byte_count\":8"));
    assert_stat(
        &stdout,
        "sim.trace_replay.scheduled",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_rejects_ambiguous_suite_trace_resources() {
    let workspace = temp_workspace("trace-replay-suite-resource-config-ambiguous");
    let trace_dir = workspace.join("artifacts");
    std::fs::create_dir(&trace_dir).unwrap();
    for trace_name in ["trace-a.pb", "trace-b.pb"] {
        std::fs::write(
            trace_dir.join(trace_name),
            packet_trace_bytes(
                1_000,
                &[PacketFields {
                    tick: 0,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                }],
            ),
        )
        .unwrap();
    }
    let resource_config = workspace.join("resource-acquire-suite.toml");
    std::fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "ambiguous-trace-suite-cli"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "trace-workload-a"
boot_entry = 4096

[[resource_acquire.manifests.resources]]
id = "trace"
kind = "input"
digest = "sha256:trace-resource-a"
locator = "resources/trace-a.pb"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://trace-a"
artifact = "artifacts/trace-a.pb"
artifact_digest = "sha256:trace-resource-a"

[[resource_acquire.manifests]]
workload_id = "trace-workload-b"
boot_entry = 8192

[[resource_acquire.manifests.resources]]
id = "trace"
kind = "input"
digest = "sha256:trace-resource-b"
locator = "resources/trace-b.pb"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://trace-b"
artifact = "artifacts/trace-b.pb"
artifact_digest = "sha256:trace-resource-b"
"#,
    )
    .unwrap();
    let config = workspace.join("trace-replay.toml");
    std::fs::write(
        &config,
        "[trace_replay]\nresource_config = \"resource-acquire-suite.toml\"\nroute = \"cpu0.suite-resource\"\nmemory_start = 4096\nmemory_size = 4096\nmax_tick = 64\ntick_frequency = 1000\nline_bytes = 64\nagent = 7\ncontrol_partition = 2\nstats_format = \"json\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["trace-replay", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("acquired 2 required trace resources"));
    assert!(stderr.contains("expected exactly one"));
}

#[test]
fn rem6_trace_replay_rejects_remote_uri_resource_config_before_replay() {
    let workspace = temp_workspace("trace-replay-resource-config-remote-uri");
    let resource_config = workspace.join("resource-acquire.toml");
    std::fs::write(
        &resource_config,
        r#"[resource_acquire]
workload_id = "remote-trace"
boot_entry = 4096

[[resource_acquire.resources]]
id = "trace"
kind = "input"
digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111"
locator = "resources/trace.pb"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "http://127.0.0.1:9/trace.pb"
artifact_digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111"
"#,
    )
    .unwrap();
    let config = workspace.join("trace-replay.toml");
    std::fs::write(
        &config,
        "[trace_replay]\nresource_config = \"resource-acquire.toml\"\nroute = \"cpu0.remote-resource\"\nmemory_start = 4096\nmemory_size = 4096\nmax_tick = 64\ntick_frequency = 1000\nline_bytes = 64\nagent = 7\ncontrol_partition = 2\nstats_format = \"json\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["trace-replay", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("runtime resource handoff does not allow remote-uri resources"));
    assert!(stderr.contains("trace"));
    assert!(stderr.contains("rem6 resource-acquire"));
}

#[test]
fn rem6_trace_replay_cli_trace_overrides_toml_resource_config() {
    let workspace = temp_workspace("trace-replay-resource-config-override");
    let resource_trace_dir = workspace.join("artifacts");
    std::fs::create_dir(&resource_trace_dir).unwrap();
    std::fs::write(
        resource_trace_dir.join("resource-trace.pb"),
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
    let direct_trace = workspace.join("direct-trace.pb");
    std::fs::write(
        &direct_trace,
        packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(20),
                },
                PacketFields {
                    tick: 2,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(20),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_READ_REQ,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(21),
                },
                PacketFields {
                    tick: 5,
                    command: GEM5_READ_RESP,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(21),
                },
            ],
        ),
    )
    .unwrap();
    let resource_config = workspace.join("resource-acquire.toml");
    std::fs::write(
        &resource_config,
        "[resource_acquire]\nworkload_id = \"trace-resource-override-cli\"\nboot_entry = 4096\nstats_format = \"json\"\n\n[[resource_acquire.resources]]\nid = \"trace\"\nkind = \"input\"\ndigest = \"sha256:trace-resource\"\nlocator = \"resources/trace.pb\"\nrequired = true\nacquisition_kind = \"local-file\"\nacquisition_locator = \"catalog://trace\"\nartifact = \"artifacts/resource-trace.pb\"\nartifact_digest = \"sha256:trace-resource\"\n",
    )
    .unwrap();
    let config = workspace.join("trace-replay.toml");
    std::fs::write(
        &config,
        "[trace_replay]\nresource_config = \"resource-acquire.toml\"\nroute = \"cpu0.config\"\nmemory_start = 4096\nmemory_size = 4096\nmax_tick = 64\ntick_frequency = 1000\nline_bytes = 64\nagent = 7\ncontrol_partition = 2\nstats_format = \"json\"\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "trace-replay",
            "--config",
            config.to_str().unwrap(),
            "--trace",
            direct_trace.to_str().unwrap(),
            "--route",
            "cpu0.direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"route\":\"cpu0.direct\""));
    assert!(stdout.contains("\"scheduled_count\":2"));
    assert!(stdout.contains("\"trace_read_response_count\":2"));
    assert!(stdout.contains("\"trace_response_data_byte_count\":16"));
}

#[test]
fn rem6_trace_replay_rejects_zero_memory_size_from_toml_config() {
    let workspace = temp_workspace("trace-replay-toml-invalid-memory-size");
    let trace_name = format!("trace-{}.pb", std::process::id());
    let trace = workspace.join(&trace_name);
    std::fs::write(
        &trace,
        packet_trace_bytes(
            1_000,
            &[PacketFields {
                tick: 0,
                command: GEM5_READ_REQ,
                address: Some(0x1008),
                size: Some(8),
                packet_id: Some(10),
            }],
        ),
    )
    .unwrap();
    let config = workspace.join("trace-replay-invalid-memory-size.toml");
    std::fs::write(
        &config,
        format!(
            "[trace_replay]\ntrace = \"{}\"\nroute = \"cpu0.fetch\"\nmemory_start = 4096\nmemory_size = 0\nmax_tick = 64\ntick_frequency = 1000\nline_bytes = 64\nagent = 7\ncontrol_partition = 2\nstats_format = \"json\"\n",
            trace_name
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["trace-replay", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("invalid trace replay memory size 0"));
}

#[test]
fn rem6_trace_replay_uses_max_trace_tick_for_duration() {
    let trace = temp_trace(
        "trace-replay-out-of-order",
        &packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 20,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
                PacketFields {
                    tick: 21,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_WRITE_REQ,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(11),
                },
                PacketFields {
                    tick: 4,
                    command: GEM5_WRITE_ERROR,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(11),
                },
                PacketFields {
                    tick: 5,
                    command: GEM5_WRITE_REQ,
                    address: Some(0x1018),
                    size: Some(8),
                    packet_id: Some(12),
                },
                PacketFields {
                    tick: 6,
                    command: GEM5_WRITE_RESP,
                    address: Some(0x1018),
                    size: Some(8),
                    packet_id: Some(12),
                },
            ],
        ),
    );
    let output = trace_replay_output(&trace, "64");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"scheduled_count\":3"));
    assert!(stdout.contains("\"trace_read_response_count\":1"));
    assert!(stdout.contains("\"trace_write_response_count\":1"));
    assert!(stdout.contains("\"memory_failure_write_count\":1"));
    assert_stat(
        &stdout,
        "sim.trace_replay.responses.reads",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.responses.writes",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.memory_failures.write",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_emits_write_completion_bytes() {
    let trace = temp_trace(
        "trace-replay-write-completion",
        &packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_WRITE_REQ,
                    address: Some(0x1800),
                    size: Some(8),
                    packet_id: Some(31),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_WRITE_RESP,
                    address: Some(0x1800),
                    size: Some(8),
                    packet_id: Some(31),
                },
                PacketFields {
                    tick: 5,
                    command: GEM5_WRITE_COMPLETE_RESP,
                    address: Some(0x1800),
                    size: Some(8),
                    packet_id: Some(31),
                },
            ],
        ),
    );
    let output = trace_replay_output(&trace, "64");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"memory_write_completion_count\":1"));
    assert!(stdout.contains("\"memory_write_completion_byte_count\":8"));
    assert_stat(
        &stdout,
        "sim.trace_replay.memory.write_completion_bytes",
        "Byte",
        8,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_digest_changes_with_trace_payload() {
    let first = temp_trace(
        "trace-replay-digest-a",
        &packet_trace_bytes(
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
                    tick: 1,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
            ],
        ),
    );
    let second = temp_trace(
        "trace-replay-digest-b",
        &packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(11),
                },
                PacketFields {
                    tick: 1,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(11),
                },
            ],
        ),
    );
    let first_output = trace_replay_output(&first, "64");
    let second_output = trace_replay_output(&second, "64");

    assert!(
        first_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&first_output.stderr)
    );
    assert!(
        second_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&second_output.stderr)
    );
    let first_stdout = String::from_utf8(first_output.stdout).unwrap();
    let second_stdout = String::from_utf8(second_output.stdout).unwrap();
    let first_digest = json_string_field(&first_stdout, "trace_digest");
    let second_digest = json_string_field(&second_stdout, "trace_digest");

    assert!(first_digest.starts_with("sha256:"));
    assert!(second_digest.starts_with("sha256:"));
    assert_ne!(first_digest, second_digest);
}

#[test]
fn rem6_trace_replay_rejects_final_tick_after_max_tick() {
    let trace = temp_trace(
        "trace-replay-max-tick",
        &packet_trace_bytes(
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
                    tick: 1,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
            ],
        ),
    );
    let output = trace_replay_output(&trace, "1");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exceeds max tick 1"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rem6_trace_replay_emits_typed_sideband_and_control_stats() {
    let trace = temp_trace(
        "trace-replay-typed-sideband",
        &packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_CLEAN_SHARED_REQ,
                    address: Some(0x1040),
                    size: Some(64),
                    packet_id: Some(20),
                },
                PacketFields {
                    tick: 2,
                    command: GEM5_CLEAN_SHARED_RESP,
                    address: Some(0x1040),
                    size: Some(64),
                    packet_id: Some(20),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_INVALIDATE_REQ,
                    address: Some(0x1080),
                    size: Some(64),
                    packet_id: Some(21),
                },
                PacketFields {
                    tick: 5,
                    command: GEM5_INVALIDATE_RESP,
                    address: Some(0x1080),
                    size: Some(64),
                    packet_id: Some(21),
                },
                PacketFields {
                    tick: 6,
                    command: GEM5_TLBI_EXT_SYNC,
                    address: Some(0),
                    size: Some(64),
                    packet_id: Some(22),
                },
                PacketFields {
                    tick: 7,
                    command: GEM5_FLUSH_REQ,
                    address: Some(0x10c0),
                    size: Some(64),
                    packet_id: Some(23),
                },
                PacketFields {
                    tick: 8,
                    command: GEM5_PRINT_REQ,
                    address: Some(0x1100),
                    size: Some(1),
                    packet_id: Some(24),
                },
                PacketFields {
                    tick: 9,
                    command: GEM5_HTM_ABORT,
                    address: None,
                    size: None,
                    packet_id: Some(25),
                },
                PacketFields {
                    tick: 10,
                    command: GEM5_HTM_REQ,
                    address: None,
                    size: None,
                    packet_id: Some(26),
                },
                PacketFields {
                    tick: 12,
                    command: GEM5_HTM_REQ_RESP,
                    address: None,
                    size: None,
                    packet_id: Some(26),
                },
            ],
        ),
    );
    let output = trace_replay_output(&trace, "64");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"memory_trace_event_count\":6"));
    assert!(stdout.contains("\"trace_data_cache_response_count\":0"));
    assert!(stdout.contains("\"trace_data_cache_maintenance_response_count\":0"));
    assert!(stdout.contains("\"htm_control_ack_count\":1"));
    assert!(stdout.contains("\"sideband_event_count\":4"));
    assert!(stdout.contains("\"tlb_sync_event_count\":1"));
    assert!(stdout.contains("\"trace_tlb_sync_count\":0"));
    assert!(stdout.contains("\"trace_tlb_sync_flushed_entry_count\":0"));
    assert!(stdout.contains("\"cache_flush_event_count\":1"));
    assert!(stdout.contains("\"trace_cache_flush_count\":0"));
    assert!(stdout.contains("\"diagnostic_print_event_count\":1"));
    assert!(stdout.contains("\"trace_diagnostic_count\":0"));
    assert!(stdout.contains("\"htm_abort_event_count\":1"));
    assert!(stdout.contains("\"trace_htm_abort_count\":1"));
    assert_stat(
        &stdout,
        "sim.trace_replay.memory.events",
        "Count",
        6,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.responses.cache",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.control_acks.htm",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.sideband.tlb_sync_events",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.sideband.tlb_sync_flushed_entries",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.sideband.cache_flush_events",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.sideband.diagnostic_print_events",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.sideband.htm_abort",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_data_cache_protocol_drives_executable_policy_stats() {
    let trace = temp_trace(
        "trace-replay-data-cache-policy",
        &packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_CLEAN_SHARED_REQ,
                    address: Some(0x1040),
                    size: Some(64),
                    packet_id: Some(30),
                },
                PacketFields {
                    tick: 2,
                    command: GEM5_CLEAN_SHARED_RESP,
                    address: Some(0x1040),
                    size: Some(64),
                    packet_id: Some(30),
                },
                PacketFields {
                    tick: 3,
                    command: GEM5_INVALIDATE_REQ,
                    address: Some(0x1080),
                    size: Some(64),
                    packet_id: Some(31),
                },
                PacketFields {
                    tick: 5,
                    command: GEM5_INVALIDATE_RESP,
                    address: Some(0x1080),
                    size: Some(64),
                    packet_id: Some(31),
                },
            ],
        ),
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "trace-replay",
            "--trace",
            trace.to_str().unwrap(),
            "--route",
            "cpu0.data",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "0x1000",
            "--max-tick",
            "64",
            "--tick-frequency",
            "1000",
            "--line-bytes",
            "64",
            "--agent",
            "7",
            "--control-partition",
            "2",
            "--data-cache-protocol",
            "msi",
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
    assert!(stdout.contains("\"route\":\"cpu0.data\""));
    assert!(stdout.contains("\"trace_data_cache_response_count\":2"));
    assert!(stdout.contains("\"trace_data_cache_maintenance_response_count\":2"));
    assert!(stdout.contains("\"trace_data_cache_clean_maintenance_response_count\":1"));
    assert!(stdout.contains("\"trace_data_cache_invalidate_maintenance_response_count\":1"));
    assert_stat(
        &stdout,
        "sim.trace_replay.responses.cache",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.responses.cache.maintenance",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.data_cache.runs",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.data_cache.msi.runs",
        "Count",
        2,
        "monotonic",
    );
}

fn trace_replay_output(trace: &std::path::Path, max_tick: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "trace-replay",
            "--trace",
            trace.to_str().unwrap(),
            "--route",
            "cpu0.fetch",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "0x1000",
            "--max-tick",
            max_tick,
            "--tick-frequency",
            "1000",
            "--line-bytes",
            "64",
            "--agent",
            "7",
            "--control-partition",
            "2",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap()
}

fn json_string_field<'a>(json: &'a str, field: &str) -> &'a str {
    let needle = format!("\"{field}\":\"");
    let start = json.find(&needle).unwrap() + needle.len();
    let rest = &json[start..];
    let end = rest.find('"').unwrap();
    &rest[..end]
}
