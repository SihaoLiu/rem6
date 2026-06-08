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
