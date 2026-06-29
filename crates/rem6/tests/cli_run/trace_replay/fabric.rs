use std::process::Command;

use crate::support::*;
use serde_json::Value;

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
    assert!(stdout.contains("\"fabric_flit_count\":4"));
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
        "sim.trace_replay.fabric.flits",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.bytes",
        "Byte",
        16,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.resources.activity",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.resources.active",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_fabric_route_uses_virtual_networks_and_credit_depth() {
    let trace = temp_trace(
        "trace-replay-fabric-credit",
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
    assert!(stdout.contains("\"fabric_link\":\"cpu_mem\""));
    assert!(stdout.contains("\"fabric_request_virtual_network\":1"));
    assert!(stdout.contains("\"fabric_response_virtual_network\":2"));
    assert!(stdout.contains("\"fabric_credit_depth\":1"));
    assert!(stdout.contains("\"active_fabric_lane_count\":2"));
    assert!(stdout.contains("\"active_fabric_virtual_network_count\":2"));
    assert!(stdout.contains("\"fabric_transfer_count\":2"));
    assert!(stdout.contains("\"fabric_queue_delay_ticks\":0"));
    assert!(stdout.contains("\"fabric_credit_delay_ticks\":0"));
    assert!(stdout.contains("\"fabric_max_credit_delay_ticks\":0"));
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.active_virtual_networks",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.queue_delay_ticks",
        "Tick",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.credit_delay_ticks",
        "Tick",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.max_credit_delay_ticks",
        "Tick",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_fabric_route_uses_router_stage() {
    let trace = temp_trace(
        "trace-replay-fabric-router",
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
                    command: GEM5_READ_REQ,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(11),
                },
                PacketFields {
                    tick: 40,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
                PacketFields {
                    tick: 50,
                    command: GEM5_READ_RESP,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(11),
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
            "128",
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
            "--fabric-request-virtual-network",
            "1",
            "--fabric-response-virtual-network",
            "1",
            "--fabric-router",
            "router0",
            "--fabric-router-input-port",
            "2",
            "--fabric-router-output-port",
            "3",
            "--fabric-router-virtual-channel",
            "1",
            "--fabric-router-latency",
            "3",
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
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let config_router = artifact
        .get("fabric_router_stage")
        .expect("fabric router stage config");
    assert_eq!(
        config_router.get("router").and_then(Value::as_str),
        Some("router0")
    );
    assert_eq!(
        config_router.get("input_port").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        config_router.get("output_port").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        config_router.get("virtual_channel").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        config_router.get("latency_ticks").and_then(Value::as_u64),
        Some(3)
    );
    let hops = artifact
        .pointer("/summary/fabric_hop_activities")
        .and_then(Value::as_array)
        .expect("fabric hop activity details");
    assert_eq!(hops.len(), 4);
    let router_hops = hops
        .iter()
        .filter_map(|hop| hop.get("router"))
        .collect::<Vec<_>>();
    assert_eq!(router_hops.len(), 4);
    let first_router = router_hops[0];
    assert_eq!(
        first_router.get("router").and_then(Value::as_str),
        Some("router0")
    );
    assert_eq!(
        first_router.get("input_port").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        first_router.get("output_port").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        first_router.get("virtual_channel").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        first_router.get("latency_ticks").and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        router_hops[1]
            .get("queue_delay_ticks")
            .and_then(Value::as_u64),
        Some(2)
    );

    let prefix = "sim.trace_replay.fabric.link.cpu_mem.vn1.hop0";
    assert_stat(
        &stdout,
        &format!("{prefix}.transfers"),
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        &format!("{prefix}.router_latency_ticks"),
        "Tick",
        12,
        "monotonic",
    );
    assert_stat(
        &stdout,
        &format!("{prefix}.router_queue_delay_ticks"),
        "Tick",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        &format!("{prefix}.max_router_queue_delay_ticks"),
        "Tick",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_trace_replay_fabric_route_emits_lane_and_hop_activity_detail() {
    let trace = temp_trace(
        "trace-replay-fabric-activity-detail",
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
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        artifact
            .pointer("/summary/active_fabric_virtual_network_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let lanes = artifact
        .pointer("/summary/fabric_lane_activities")
        .and_then(Value::as_array)
        .expect("fabric lane activity details");
    assert_eq!(lanes.len(), 2);
    assert_fabric_lane_activity(
        lanes,
        ExpectedFabricLaneActivity {
            link: "cpu_mem",
            virtual_network: 1,
            transfer_count: 1,
            byte_count: 8,
            flit_count: 2,
            occupied_ticks: 2,
            backpressure_ticks: 0,
            max_backpressure_ticks: 0,
            queue_delay_ticks: 0,
            max_queue_delay_ticks: 0,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
        },
    );
    assert_fabric_lane_activity(
        lanes,
        ExpectedFabricLaneActivity {
            link: "cpu_mem",
            virtual_network: 2,
            transfer_count: 1,
            byte_count: 8,
            flit_count: 2,
            occupied_ticks: 2,
            backpressure_ticks: 0,
            max_backpressure_ticks: 0,
            queue_delay_ticks: 0,
            max_queue_delay_ticks: 0,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
        },
    );
    assert_fabric_virtual_network_stats(
        &stdout,
        ExpectedFabricVirtualNetworkStats {
            virtual_network: 1,
            active_lanes: 1,
            transfers: 1,
            bytes: 8,
            flits: 2,
            occupied_ticks: 2,
            queue_delay_ticks: 0,
            max_queue_delay_ticks: 0,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
            contended_lanes: 0,
        },
    );
    assert_fabric_virtual_network_stats(
        &stdout,
        ExpectedFabricVirtualNetworkStats {
            virtual_network: 2,
            active_lanes: 1,
            transfers: 1,
            bytes: 8,
            flits: 2,
            occupied_ticks: 2,
            queue_delay_ticks: 0,
            max_queue_delay_ticks: 0,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
            contended_lanes: 0,
        },
    );
    assert_fabric_lane_stats(
        &stdout,
        ExpectedFabricLaneStats {
            link: "cpu_mem",
            virtual_network: 1,
            transfer_count: 1,
            byte_count: 8,
            flit_count: 2,
            occupied_ticks: 2,
            backpressure_ticks: 0,
            max_backpressure_ticks: 0,
            queue_delay_ticks: 0,
            max_queue_delay_ticks: 0,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
        },
    );
    assert_fabric_lane_stats(
        &stdout,
        ExpectedFabricLaneStats {
            link: "cpu_mem",
            virtual_network: 2,
            transfer_count: 1,
            byte_count: 8,
            flit_count: 2,
            occupied_ticks: 2,
            backpressure_ticks: 0,
            max_backpressure_ticks: 0,
            queue_delay_ticks: 0,
            max_queue_delay_ticks: 0,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
        },
    );
    let links = artifact
        .pointer("/summary/fabric_link_activities")
        .and_then(Value::as_array)
        .expect("fabric link activity details");
    assert_eq!(links.len(), 1);
    assert_fabric_link_activity(
        links,
        ExpectedFabricLinkStats {
            link: "cpu_mem",
            active_virtual_networks: 2,
            transfer_count: 2,
            byte_count: 16,
            flit_count: 4,
            occupied_ticks: 4,
            queue_delay_ticks: 0,
            max_queue_delay_ticks: 0,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
            contended_virtual_networks: 0,
        },
    );
    assert_fabric_link_stats(
        &stdout,
        ExpectedFabricLinkStats {
            link: "cpu_mem",
            active_virtual_networks: 2,
            transfer_count: 2,
            byte_count: 16,
            flit_count: 4,
            occupied_ticks: 4,
            queue_delay_ticks: 0,
            max_queue_delay_ticks: 0,
            credit_delay_ticks: 0,
            max_credit_delay_ticks: 0,
            contended_virtual_networks: 0,
        },
    );

    let hops = artifact
        .pointer("/summary/fabric_hop_activities")
        .and_then(Value::as_array)
        .expect("fabric hop activity details");
    assert_eq!(hops.len(), 2);
    for hop in hops {
        assert_eq!(hop.get("link").and_then(Value::as_str), Some("cpu_mem"));
        assert_eq!(hop.get("hop_index").and_then(Value::as_u64), Some(0));
        assert!(matches!(
            hop.get("virtual_network").and_then(Value::as_u64),
            Some(1 | 2)
        ));
        assert_eq!(hop.get("bytes").and_then(Value::as_u64), Some(8));
        assert_eq!(hop.get("flits").and_then(Value::as_u64), Some(2));
        assert_eq!(
            hop.get("credit_delay_ticks").and_then(Value::as_u64),
            Some(0)
        );
        assert!(hop.get("packet").and_then(Value::as_u64).is_some());
        assert!(hop.get("ready_tick").and_then(Value::as_u64).is_some());
        assert!(hop.get("start_tick").and_then(Value::as_u64).is_some());
        assert!(hop.get("occupied_ticks").and_then(Value::as_u64).is_some());
        assert!(hop
            .get("queue_delay_ticks")
            .and_then(Value::as_u64)
            .is_some());
        assert!(hop.get("depart_tick").and_then(Value::as_u64).is_some());
        assert!(hop.get("arrival_tick").and_then(Value::as_u64).is_some());
    }
    for virtual_network in [1, 2] {
        let prefix = format!("sim.trace_replay.fabric.link.cpu_mem.vn{virtual_network}.hop0");
        assert_stat(
            &stdout,
            &format!("{prefix}.transfers"),
            "Count",
            1,
            "monotonic",
        );
        assert_stat(&stdout, &format!("{prefix}.bytes"), "Byte", 8, "monotonic");
        assert_stat(&stdout, &format!("{prefix}.flits"), "Count", 2, "monotonic");
        assert_stat(
            &stdout,
            &format!("{prefix}.occupied_ticks"),
            "Tick",
            2,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("{prefix}.queue_delay_ticks"),
            "Tick",
            0,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("{prefix}.max_queue_delay_ticks"),
            "Tick",
            0,
            "monotonic",
        );
        assert_stat(
            &stdout,
            &format!("{prefix}.credit_delay_ticks"),
            "Tick",
            0,
            "monotonic",
        );
    }
}

#[test]
fn rem6_trace_replay_fabric_route_emits_wait_for_windows() {
    let trace = temp_trace(
        "trace-replay-fabric-wait-for",
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
                    command: GEM5_READ_REQ,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(11),
                },
                PacketFields {
                    tick: 40,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(10),
                },
                PacketFields {
                    tick: 50,
                    command: GEM5_READ_RESP,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(11),
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
            "128",
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
            "--fabric-request-virtual-network",
            "1",
            "--fabric-response-virtual-network",
            "1",
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
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let summary = artifact.pointer("/summary").expect("trace replay summary");

    let wait_for_edge_count = summary
        .get("fabric_wait_for_edge_count")
        .and_then(Value::as_u64)
        .expect("fabric wait-for edge count");
    assert_eq!(wait_for_edge_count, 2);
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.wait_for.edges",
        "Count",
        wait_for_edge_count,
        "monotonic",
    );

    let kind_windows = summary
        .get("fabric_wait_for_edge_kind_windows")
        .and_then(Value::as_array)
        .expect("fabric wait-for edge kind windows");
    assert_wait_for_kind_window(kind_windows, "queue", 1, 1, 1);
    assert_wait_for_kind_window(kind_windows, "credit", 1, 2, 2);
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.wait_for.kind.queue.edges",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.wait_for.kind.queue.first_tick",
        "Tick",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.wait_for.kind.queue.last_tick",
        "Tick",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.wait_for.kind.credit.edges",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.wait_for.kind.credit.first_tick",
        "Tick",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.fabric.wait_for.kind.credit.last_tick",
        "Tick",
        2,
        "monotonic",
    );

    let target_windows = summary
        .get("fabric_wait_for_target_node_windows")
        .and_then(Value::as_array)
        .expect("fabric wait-for target node windows");
    assert_wait_for_node_window(target_windows, "resource:fabric.cpu_mem.vn.1.lane", 1, 1, 1);
    assert_wait_for_node_window(
        target_windows,
        "resource:fabric.cpu_mem.vn.1.credit",
        1,
        2,
        2,
    );

    let blocked_windows = summary
        .get("fabric_wait_for_blocked_node_windows")
        .and_then(Value::as_array)
        .expect("fabric wait-for blocked node windows");
    assert_eq!(blocked_windows.len(), 1);
    let blocked_window = &blocked_windows[0];
    assert!(blocked_window
        .get("node")
        .and_then(Value::as_str)
        .is_some_and(|node| node.starts_with("transaction:fabric.packet.")));
    assert_eq!(
        blocked_window.get("edge_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        blocked_window.get("first_tick").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        blocked_window.get("last_tick").and_then(Value::as_u64),
        Some(2)
    );

    let lane_activities = summary
        .get("fabric_lane_activities")
        .and_then(Value::as_array)
        .expect("fabric lane activity details");
    assert_eq!(lane_activities.len(), 1);
    assert_fabric_lane_activity(
        lane_activities,
        ExpectedFabricLaneActivity {
            link: "cpu_mem",
            virtual_network: 1,
            transfer_count: 4,
            byte_count: 32,
            flit_count: 8,
            occupied_ticks: 8,
            backpressure_ticks: 2,
            max_backpressure_ticks: 2,
            queue_delay_ticks: 2,
            max_queue_delay_ticks: 2,
            credit_delay_ticks: 1,
            max_credit_delay_ticks: 1,
        },
    );
    assert_fabric_lane_stats(
        &stdout,
        ExpectedFabricLaneStats {
            link: "cpu_mem",
            virtual_network: 1,
            transfer_count: 4,
            byte_count: 32,
            flit_count: 8,
            occupied_ticks: 8,
            backpressure_ticks: 2,
            max_backpressure_ticks: 2,
            queue_delay_ticks: 2,
            max_queue_delay_ticks: 2,
            credit_delay_ticks: 1,
            max_credit_delay_ticks: 1,
        },
    );
}

fn assert_fabric_virtual_network_stats(stdout: &str, expected: ExpectedFabricVirtualNetworkStats) {
    let prefix = format!("sim.trace_replay.fabric.vn{}", expected.virtual_network);
    assert_stat(
        stdout,
        &format!("{prefix}.active_lanes"),
        "Count",
        expected.active_lanes,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.transfers"),
        "Count",
        expected.transfers,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.bytes"),
        "Byte",
        expected.bytes,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.flits"),
        "Count",
        expected.flits,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        expected.occupied_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        expected.queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        expected.max_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        expected.credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_credit_delay_ticks"),
        "Tick",
        expected.max_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.contended_lanes"),
        "Count",
        expected.contended_lanes,
        "monotonic",
    );
}

fn assert_wait_for_kind_window(
    windows: &[Value],
    kind: &str,
    edge_count: u64,
    first_tick: u64,
    last_tick: u64,
) {
    let window = windows
        .iter()
        .find(|window| window.get("kind").and_then(Value::as_str) == Some(kind))
        .unwrap_or_else(|| panic!("missing {kind} wait-for kind window"));
    assert_eq!(
        window.get("edge_count").and_then(Value::as_u64),
        Some(edge_count)
    );
    assert_eq!(
        window.get("first_tick").and_then(Value::as_u64),
        Some(first_tick)
    );
    assert_eq!(
        window.get("last_tick").and_then(Value::as_u64),
        Some(last_tick)
    );
}

fn assert_wait_for_node_window(
    windows: &[Value],
    node: &str,
    edge_count: u64,
    first_tick: u64,
    last_tick: u64,
) {
    let window = windows
        .iter()
        .find(|window| window.get("node").and_then(Value::as_str) == Some(node))
        .unwrap_or_else(|| panic!("missing {node} wait-for node window"));
    assert_eq!(
        window.get("edge_count").and_then(Value::as_u64),
        Some(edge_count)
    );
    assert_eq!(
        window.get("first_tick").and_then(Value::as_u64),
        Some(first_tick)
    );
    assert_eq!(
        window.get("last_tick").and_then(Value::as_u64),
        Some(last_tick)
    );
}

fn assert_fabric_lane_stats(stdout: &str, expected: ExpectedFabricLaneStats<'_>) {
    let prefix = format!(
        "sim.trace_replay.fabric.link.{}.vn{}",
        stat_path_segment(expected.link),
        expected.virtual_network
    );
    assert_stat(
        stdout,
        &format!("{prefix}.transfers"),
        "Count",
        expected.transfer_count,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.bytes"),
        "Byte",
        expected.byte_count,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.flits"),
        "Count",
        expected.flit_count,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        expected.occupied_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.backpressure_ticks"),
        "Tick",
        expected.backpressure_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_backpressure_ticks"),
        "Tick",
        expected.max_backpressure_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        expected.queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        expected.max_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        expected.credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_credit_delay_ticks"),
        "Tick",
        expected.max_credit_delay_ticks,
        "monotonic",
    );
}

fn assert_fabric_link_stats(stdout: &str, expected: ExpectedFabricLinkStats<'_>) {
    let prefix = format!(
        "sim.trace_replay.fabric.link.{}",
        stat_path_segment(expected.link)
    );
    assert_stat(
        stdout,
        &format!("{prefix}.active_virtual_networks"),
        "Count",
        expected.active_virtual_networks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.transfers"),
        "Count",
        expected.transfer_count,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.bytes"),
        "Byte",
        expected.byte_count,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.flits"),
        "Count",
        expected.flit_count,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        expected.occupied_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        expected.queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        expected.max_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        expected.credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_credit_delay_ticks"),
        "Tick",
        expected.max_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.contended_virtual_networks"),
        "Count",
        expected.contended_virtual_networks,
        "monotonic",
    );
}

fn assert_fabric_link_activity(links: &[Value], expected: ExpectedFabricLinkStats<'_>) {
    let link = links
        .iter()
        .find(|link| link.get("link").and_then(Value::as_str) == Some(expected.link))
        .expect("fabric link activity entry");
    assert_eq!(
        link.get("active_virtual_networks").and_then(Value::as_u64),
        Some(expected.active_virtual_networks)
    );
    assert_eq!(
        link.get("transfer_count").and_then(Value::as_u64),
        Some(expected.transfer_count)
    );
    assert_eq!(
        link.get("byte_count").and_then(Value::as_u64),
        Some(expected.byte_count)
    );
    assert_eq!(
        link.get("flit_count").and_then(Value::as_u64),
        Some(expected.flit_count)
    );
    assert_eq!(
        link.get("occupied_ticks").and_then(Value::as_u64),
        Some(expected.occupied_ticks)
    );
    assert_eq!(
        link.get("queue_delay_ticks").and_then(Value::as_u64),
        Some(expected.queue_delay_ticks)
    );
    assert_eq!(
        link.get("max_queue_delay_ticks").and_then(Value::as_u64),
        Some(expected.max_queue_delay_ticks)
    );
    assert_eq!(
        link.get("credit_delay_ticks").and_then(Value::as_u64),
        Some(expected.credit_delay_ticks)
    );
    assert_eq!(
        link.get("max_credit_delay_ticks").and_then(Value::as_u64),
        Some(expected.max_credit_delay_ticks)
    );
    assert_eq!(
        link.get("contended_virtual_networks")
            .and_then(Value::as_u64),
        Some(expected.contended_virtual_networks)
    );
    assert!(link.get("first_tick").and_then(Value::as_u64).is_some());
    assert!(link.get("last_tick").and_then(Value::as_u64).is_some());
}

fn assert_fabric_lane_activity(lanes: &[Value], expected: ExpectedFabricLaneActivity<'_>) {
    let lane = lanes
        .iter()
        .find(|lane| {
            lane.get("link").and_then(Value::as_str) == Some(expected.link)
                && lane.get("virtual_network").and_then(Value::as_u64)
                    == Some(expected.virtual_network)
        })
        .expect("fabric lane activity entry");
    assert_eq!(
        lane.get("transfer_count").and_then(Value::as_u64),
        Some(expected.transfer_count)
    );
    assert_eq!(
        lane.get("byte_count").and_then(Value::as_u64),
        Some(expected.byte_count)
    );
    assert_eq!(
        lane.get("flit_count").and_then(Value::as_u64),
        Some(expected.flit_count)
    );
    assert_eq!(
        lane.get("occupied_ticks").and_then(Value::as_u64),
        Some(expected.occupied_ticks)
    );
    assert_eq!(
        lane.get("backpressure_ticks").and_then(Value::as_u64),
        Some(expected.backpressure_ticks)
    );
    assert_eq!(
        lane.get("max_backpressure_ticks").and_then(Value::as_u64),
        Some(expected.max_backpressure_ticks)
    );
    assert_eq!(
        lane.get("queue_delay_ticks").and_then(Value::as_u64),
        Some(expected.queue_delay_ticks)
    );
    assert_eq!(
        lane.get("max_queue_delay_ticks").and_then(Value::as_u64),
        Some(expected.max_queue_delay_ticks)
    );
    assert_eq!(
        lane.get("credit_delay_ticks").and_then(Value::as_u64),
        Some(expected.credit_delay_ticks)
    );
    assert_eq!(
        lane.get("max_credit_delay_ticks").and_then(Value::as_u64),
        Some(expected.max_credit_delay_ticks)
    );
    assert!(lane.get("first_tick").and_then(Value::as_u64).is_some());
    assert!(lane.get("last_tick").and_then(Value::as_u64).is_some());
}

struct ExpectedFabricVirtualNetworkStats {
    virtual_network: u64,
    active_lanes: u64,
    transfers: u64,
    bytes: u64,
    flits: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
    max_credit_delay_ticks: u64,
    contended_lanes: u64,
}

struct ExpectedFabricLaneStats<'a> {
    link: &'a str,
    virtual_network: u64,
    transfer_count: u64,
    byte_count: u64,
    flit_count: u64,
    occupied_ticks: u64,
    backpressure_ticks: u64,
    max_backpressure_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
    max_credit_delay_ticks: u64,
}

struct ExpectedFabricLinkStats<'a> {
    link: &'a str,
    active_virtual_networks: u64,
    transfer_count: u64,
    byte_count: u64,
    flit_count: u64,
    occupied_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
    max_credit_delay_ticks: u64,
    contended_virtual_networks: u64,
}

struct ExpectedFabricLaneActivity<'a> {
    link: &'a str,
    virtual_network: u64,
    transfer_count: u64,
    byte_count: u64,
    flit_count: u64,
    occupied_ticks: u64,
    backpressure_ticks: u64,
    max_backpressure_ticks: u64,
    queue_delay_ticks: u64,
    max_queue_delay_ticks: u64,
    credit_delay_ticks: u64,
    max_credit_delay_ticks: u64,
}

#[test]
fn rem6_trace_replay_rejects_fabric_virtual_network_without_link() {
    let trace = temp_trace(
        "trace-replay-fabric-vn-without-link",
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
            "--fabric-request-virtual-network",
            "1",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing required flag --fabric-link"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
