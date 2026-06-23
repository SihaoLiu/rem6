use std::process::Command;

use crate::support::*;

#[test]
fn rem6_trace_replay_data_cache_dram_qos_emits_stats() {
    let trace = temp_trace(
        "trace-replay-data-cache-dram-qos",
        &packet_trace_bytes(
            1_000,
            &[
                PacketFields {
                    tick: 0,
                    command: GEM5_READ_REQ,
                    address: Some(0x1040),
                    size: Some(8),
                    packet_id: Some(40),
                },
                PacketFields {
                    tick: 2,
                    command: GEM5_READ_RESP,
                    address: Some(0x1040),
                    size: Some(8),
                    packet_id: Some(40),
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
            "--data-cache-dram-memory-profile",
            "hbm",
            "--data-cache-dram-qos-priority-levels",
            "2",
            "--data-cache-dram-qos-default-priority",
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
    assert_stat(
        &stdout,
        "sim.trace_replay.data_cache.dram_accesses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.dram.qos.accesses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.dram.qos.bytes",
        "Byte",
        64,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.dram.qos.priority1.accesses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.trace_replay.dram.qos.requestor7.bytes",
        "Byte",
        64,
        "monotonic",
    );
}
