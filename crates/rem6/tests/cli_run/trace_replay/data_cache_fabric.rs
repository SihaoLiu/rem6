use std::process::Command;

use crate::support::*;

#[test]
fn rem6_trace_replay_data_cache_protocol_uses_explicit_fabric_route() {
    let trace = temp_trace(
        "trace-replay-data-cache-fabric-route",
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
                    tick: 24,
                    command: GEM5_CLEAN_SHARED_RESP,
                    address: Some(0x1040),
                    size: Some(64),
                    packet_id: Some(30),
                },
                PacketFields {
                    tick: 32,
                    command: GEM5_INVALIDATE_REQ,
                    address: Some(0x1080),
                    size: Some(64),
                    packet_id: Some(31),
                },
                PacketFields {
                    tick: 56,
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
            "96",
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
            "--fabric-link",
            "cpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "4",
            "--fabric-request-virtual-network",
            "1",
            "--fabric-response-virtual-network",
            "2",
            "--fabric-credit-depth",
            "1",
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
    assert!(stdout.contains("\"fabric_link\":\"cpu_mem\""));
    assert!(stdout.contains("\"fabric_request_virtual_network\":1"));
    assert!(stdout.contains("\"fabric_response_virtual_network\":2"));
    assert!(stdout.contains("\"fabric_credit_depth\":1"));
    assert!(stdout.contains("\"trace_data_cache_response_count\":2"));
    assert!(stdout.contains("\"trace_data_cache_maintenance_response_count\":2"));
    assert!(stdout.contains("\"trace_data_cache_clean_maintenance_response_count\":1"));
    assert!(stdout.contains("\"trace_data_cache_invalidate_maintenance_response_count\":1"));
    assert!(stdout.contains("\"active_fabric_lane_count\":2"));
    assert!(stdout.contains("\"active_fabric_virtual_network_count\":2"));
    assert!(stdout.contains("\"fabric_transfer_count\":4"));
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
    assert_stat_greater_than(
        &stdout,
        "sim.trace_replay.data_cache.scheduler.epochs",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.trace_replay.data_cache.scheduler.dispatches",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.trace_replay.data_cache.scheduler.active_partitions",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.active_virtual_networks",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.transfers",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.bytes",
        "Byte",
        130,
        "monotonic",
    );
}
