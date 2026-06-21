use std::process::Command;

use crate::support::*;

#[test]
fn rem6_trace_replay_hands_off_packet_requests_to_sst_adapter() {
    assert_external_adapter_handoff("sst", "sst.link0", "cpu0.sst");
}

#[test]
fn rem6_trace_replay_hands_off_packet_requests_to_systemc_and_tlm_adapters() {
    assert_external_adapter_handoff("systemc", "systemc.bridge0", "cpu0.systemc");
    assert_external_adapter_handoff("tlm", "tlm.bridge0", "cpu0.tlm");
}

fn assert_external_adapter_handoff(kind: &str, endpoint: &str, route: &str) {
    let trace = temp_trace(
        &format!("trace-replay-{kind}-adapter"),
        &packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 2,
                    command: GEM5_READ_REQ,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(21),
                },
                PacketFields {
                    tick: 5,
                    command: GEM5_READ_RESP,
                    address: Some(0x1008),
                    size: Some(8),
                    packet_id: Some(21),
                },
                PacketFields {
                    tick: 7,
                    command: GEM5_WRITE_REQ,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(22),
                },
                PacketFields {
                    tick: 9,
                    command: GEM5_WRITE_RESP,
                    address: Some(0x1010),
                    size: Some(8),
                    packet_id: Some(22),
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
            route,
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
            "--external-adapter-kind",
            kind,
            "--external-adapter-endpoint",
            endpoint,
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
    assert!(stdout.contains(&format!("\"external_adapter\":{{\"kind\":\"{kind}\"")));
    assert!(stdout.contains(&format!("\"endpoint\":\"{endpoint}\"")));
    assert!(stdout.contains("\"events\":2"));
    assert!(stdout.contains("\"completed_events\":2"));
    assert!(stdout.contains("\"pending_events\":0"));
    assert!(stdout.contains("\"checkpoint_endpoints\":1"));
    assert!(stdout.contains("\"checkpoint_completed_events\":2"));
    assert!(stdout.contains("\"first_tick\":2"));
    assert!(stdout.contains("\"last_tick\":7"));
    assert!(stdout.contains("\"scheduled_count\":2"));
    assert!(stdout.contains("\"trace_read_response_count\":1"));
    assert!(stdout.contains("\"trace_write_response_count\":1"));
}

#[test]
fn rem6_trace_replay_rejects_external_adapter_endpoint_without_kind() {
    let trace = temp_trace(
        "trace-replay-adapter-endpoint-without-kind",
        &packet_trace_bytes(
            1_000,
            &[PacketFields {
                tick: 0,
                command: GEM5_READ_REQ,
                address: Some(0x1008),
                size: Some(8),
                packet_id: Some(10),
            }],
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "trace-replay",
            "--trace",
            trace.to_str().unwrap(),
            "--route",
            "cpu0.sst",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "0x1000",
            "--max-tick",
            "64",
            "--external-adapter-endpoint",
            "sst.link0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--external-adapter-endpoint requires --external-adapter-kind"));
}

#[test]
fn rem6_trace_replay_rejects_unsupported_external_adapter_kind() {
    let trace = temp_trace(
        "trace-replay-unsupported-adapter",
        &packet_trace_bytes(
            1_000,
            &[PacketFields {
                tick: 0,
                command: GEM5_READ_REQ,
                address: Some(0x1008),
                size: Some(8),
                packet_id: Some(10),
            }],
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "trace-replay",
            "--trace",
            trace.to_str().unwrap(),
            "--route",
            "cpu0.sst",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "0x1000",
            "--max-tick",
            "64",
            "--external-adapter-kind",
            "vpi",
            "--external-adapter-endpoint",
            "vpi.bridge0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unsupported trace replay external adapter kind vpi"));
    assert!(stderr.contains("supported: systemc, tlm, sst"));
}
