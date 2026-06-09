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
