use super::*;

#[path = "o3/fixtures.rs"]
mod fixtures;
use fixtures::*;

#[path = "o3/branch.rs"]
mod branch;
#[path = "o3/branch_mismatch.rs"]
mod branch_mismatch;
#[path = "o3/branch_multicore.rs"]
mod branch_multicore;
#[path = "o3/branch_prediction.rs"]
mod branch_prediction;
#[path = "o3/checkpoint.rs"]
mod checkpoint;
#[path = "o3/fu_latency.rs"]
mod fu_latency;
#[path = "o3/fu_live_window.rs"]
mod fu_live_window;
#[path = "o3/live_retire_gate.rs"]
mod live_retire_gate;
#[path = "o3/live_window_depth.rs"]
mod live_window_depth;
#[path = "o3/lsq.rs"]
mod lsq;
#[path = "o3/lsq_fu_branch.rs"]
mod lsq_fu_branch;
#[path = "o3/lsq_fu_window.rs"]
mod lsq_fu_window;
#[path = "o3/lsq_live.rs"]
mod lsq_live;
#[path = "o3/lsq_multi_load.rs"]
mod lsq_multi_load;
#[path = "o3/lsq_overlap.rs"]
mod lsq_overlap;
#[path = "o3/lsq_store_byte_compose.rs"]
mod lsq_store_byte_compose;
#[path = "o3/lsq_store_load.rs"]
mod lsq_store_load;
#[path = "o3/lsq_store_load_load.rs"]
mod lsq_store_load_load;
#[path = "o3/lsq_store_load_partial.rs"]
mod lsq_store_load_partial;
#[path = "o3/lsq_store_prefix.rs"]
mod lsq_store_prefix;
#[path = "o3/lsq_store_store_load.rs"]
mod lsq_store_store_load;
#[path = "o3/predicted_control.rs"]
mod predicted_control;
#[path = "o3/rename_live.rs"]
mod rename_live;
#[path = "o3/restore.rs"]
mod restore;
#[path = "o3/rob_live.rs"]
mod rob_live;
#[path = "o3/runtime.rs"]
mod runtime;
#[path = "o3/runtime_stats.rs"]
mod runtime_stats;
#[path = "o3/scoped_issue.rs"]
mod scoped_issue;
#[path = "o3/switch.rs"]
mod switch;
#[path = "o3/writeback_port.rs"]
mod writeback_port;

#[test]
fn rem6_run_records_o3_runtime_stats_after_detailed_switch() {
    let path = detailed_o3_runtime_stats_binary("m5-switch-cpu-detailed-o3-runtime-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_json_stat(&json, "sim.cpu0.o3.instructions", "Count", 6, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rob_allocations",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(&json, "sim.cpu0.o3.rob_commits", "Count", 6, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.rename_writes", "Count", 4, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_loads", "Count", 1, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_stores", "Count", 1, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.insts_issued",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.mem_insts_issued",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.mem_read",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.MemRead",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::MemRead",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.mem_write",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.MemWrite",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::MemWrite",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.mem_read",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.MemRead",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::MemRead",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.mem_write",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.MemWrite",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::MemWrite",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.dispatched_insts",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.insts_to_commit",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.instsToCommit.total",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.instsToCommit::total",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.writeback_count",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.writebackCount.total",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.writebackCount::total",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.producer_inst",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.producerInst.total",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.producerInst::total",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.consumer_inst",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.consumerInst.total",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.consumerInst::total",
        "Count",
        4,
        "monotonic",
    );
    let writeback_count = json_stat_u64(&json, "sim.cpu0.o3.iew.writeback_count");
    let cycles = json_stat_u64(&json, "system.cpu.numCycles");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.writeback_rate_ppm",
        "Ppm",
        ratio_ppm(writeback_count, cycles),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.wbRate",
        "Ppm",
        ratio_ppm(writeback_count, cycles),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.producer_consumer_fanout_ppm",
        "Ppm",
        ratio_ppm(3, 4),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.wbFanout",
        "Ppm",
        ratio_ppm(3, 4),
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.dispatchedInsts",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.rename.renamedInsts",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.rename.renamedOperands",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.dispLoadInsts",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.dispStoreInsts",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.lsq0.addedLoadsAndStores",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(&json, "system.cpu.lsq0.loadBytes", "Byte", 4, "monotonic");
    assert_json_stat(&json, "system.cpu.lsq0.storeBytes", "Byte", 4, "monotonic");
    let forwarding_matches =
        json_stat_u64(&json, "sim.cpu0.o3.lsq_store_to_load_forwarding_matches");
    assert_json_stat(
        &json,
        "system.cpu.lsq0.forwLoads",
        "Count",
        forwarding_matches,
        "monotonic",
    );
    assert_json_stat(&json, "system.cpu.iq.instsIssued", "Count", 6, "monotonic");
    assert_json_stat(
        &json,
        "system.cpu.iq.memInstsIssued",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(&json, "system.cpu.rob.writes", "Count", 6, "monotonic");
    assert_json_stat(&json, "system.cpu.rob.reads", "Count", 6, "monotonic");
    assert_json_stat(
        &json,
        "system.cpu.rob.maxOccupancy",
        "Count",
        1,
        "monotonic",
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime.pointer("/instructions").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        o3_runtime
            .pointer("/rob_allocations")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        o3_runtime.pointer("/rob_commits").and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        o3_runtime.pointer("/rename_writes").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_load_bytes")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_store_bytes")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        o3_runtime
            .pointer("/max_rob_occupancy")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        o3_runtime
            .pointer("/max_lsq_occupancy")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        o3_runtime
            .pointer("/rename_map_entries")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        o3_runtime
            .pointer("/checkpoint_restore_count")
            .and_then(Value::as_u64),
        Some(0),
        "O3 runtime JSON without a restore should expose a zero restore count"
    );
    for field in [
        "/checkpoint_restore_label",
        "/checkpoint_restore_tick",
        "/checkpoint_restore_manifest_tick",
        "/checkpoint_restore_payload_bytes",
    ] {
        assert!(
            o3_runtime.pointer(field).is_some_and(Value::is_null),
            "O3 runtime JSON without a restore should expose explicit null {field}"
        );
    }
    assert_eq!(
        o3_runtime
            .pointer("/iew_producer_insts")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        o3_runtime
            .pointer("/iew_consumer_insts")
            .and_then(Value::as_u64),
        Some(4)
    );
    let snapshot = o3_runtime.pointer("/snapshot").unwrap_or_else(|| {
        panic!("O3 runtime JSON should expose final snapshot state: {o3_runtime}")
    });
    assert_eq!(
        snapshot.pointer("/rob/count").and_then(Value::as_u64),
        Some(0),
        "final retired O3 runtime snapshot should have a drained ROB: {snapshot}"
    );
    let rob_entries = snapshot
        .pointer("/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("drained ROB snapshot should expose entries array: {snapshot}"));
    assert!(
        rob_entries.is_empty(),
        "drained ROB snapshot should expose no entries: {snapshot}"
    );
    assert_eq!(
        snapshot.pointer("/lsq/count").and_then(Value::as_u64),
        Some(0),
        "final retired O3 runtime snapshot should have a drained LSQ: {snapshot}"
    );
    let lsq_entries = snapshot
        .pointer("/lsq/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("drained LSQ snapshot should expose entries array: {snapshot}"));
    assert!(
        lsq_entries.is_empty(),
        "drained LSQ snapshot should expose no entries: {snapshot}"
    );
    assert_eq!(
        snapshot
            .pointer("/rename_map/count")
            .and_then(Value::as_u64),
        Some(3),
        "final O3 runtime snapshot should retain three live integer rename mappings: {snapshot}"
    );
    let rename_map = snapshot
        .pointer("/rename_map/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("snapshot should expose rename-map entries: {snapshot}"));
    let architectural_registers = rename_map
        .iter()
        .map(|entry| {
            assert_eq!(
                entry.pointer("/register_class").and_then(Value::as_str),
                Some("integer"),
                "detailed O3 run should only create integer mappings: {entry}"
            );
            assert!(
                entry.pointer("/physical").and_then(Value::as_u64).is_some(),
                "rename-map entry should expose physical register: {entry}"
            );
            entry
                .pointer("/architectural")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!("rename-map entry should expose architectural register: {entry}")
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(architectural_registers, [5, 11, 12]);
}

#[test]
fn rem6_run_records_per_core_detailed_o3_mode_switch_authority() {
    let path = multicore_hart1_detailed_o3_binary("m5-switch-cpu-hart1-detailed-o3-authority");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/cores")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/execution_mode_switch_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_execution_mode_switch(
        host_actions,
        0,
        "cpu1",
        None,
        "detailed",
        "execution-mode-switch-cpu1",
    );
    let execution_modes = host_actions
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing final execution-mode authority: {host_actions}"));
    assert_eq!(
        execution_modes.len(),
        1,
        "only hart 1 should own detailed execution-mode authority: {execution_modes:?}"
    );
    assert_eq!(
        execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu1"),
        "final execution-mode authority should target hart 1: {execution_modes:?}"
    );
    assert_eq!(
        execution_modes[0].pointer("/mode").and_then(Value::as_str),
        Some("detailed"),
        "final execution-mode authority should keep hart 1 detailed: {execution_modes:?}"
    );
    let execution_mode_switch = host_actions
        .pointer("/execution_mode_switches/0")
        .unwrap_or_else(|| panic!("missing detailed execution-mode switch: {host_actions}"));
    assert_eq!(
        execution_mode_switch
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu1"),
        "execution-mode switch should target hart 1: {execution_mode_switch}"
    );
    assert_eq!(
        execution_mode_switch
            .pointer("/mode")
            .and_then(Value::as_str),
        Some("detailed"),
        "execution-mode switch should enter detailed mode: {execution_mode_switch}"
    );

    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let cpu0_o3 = checkpoint_chunk_checksum(host_actions, 0, "cpu0", "o3-runtime-state");
    let cpu1_o3 = checkpoint_chunk_checksum(host_actions, 0, "cpu1", "o3-runtime-state");
    assert_ne!(
        cpu1_o3, cpu0_o3,
        "hart 1 detailed O3 checkpoint payload should diverge from functional hart 0"
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.instructions");
    assert_json_stat_at_least(&json, "sim.cpu1.o3.instructions", "Count", 8, "monotonic");
    assert_json_stat_at_least(&json, "sim.cpu1.o3.lsq_load_bytes", "Byte", 4, "monotonic");
    assert_json_stat_at_least(&json, "sim.cpu1.o3.lsq_store_bytes", "Byte", 4, "monotonic");
    let rob_max_occupancy = json_stat_u64(&json, "sim.cpu1.o3.max_rob_occupancy");
    let lsq_max_occupancy = json_stat_u64(&json, "sim.cpu1.o3.max_lsq_occupancy");
    assert!(
        rob_max_occupancy >= 1,
        "hart 1 should record nonzero ROB occupancy: {json}"
    );
    assert!(
        lsq_max_occupancy >= 1,
        "hart 1 should record nonzero LSQ occupancy: {json}"
    );
    assert_json_stat(
        &json,
        "system.cpu1.rob.maxOccupancy",
        "Count",
        rob_max_occupancy,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.lsq0.maxOccupancy",
        "Count",
        lsq_max_occupancy,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu1.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu1.o3.fu_integer_div_instructions",
        "Count",
        1,
        "monotonic",
    );
    let iq_int_mul = json_stat_u64(&json, "sim.cpu1.o3.iq.issued_inst_type.int_mul");
    let iq_int_div = json_stat_u64(&json, "sim.cpu1.o3.iq.issued_inst_type.int_div");
    let commit_int_mul = json_stat_u64(&json, "sim.cpu1.o3.commit.committed_inst_type.int_mul");
    let commit_int_div = json_stat_u64(&json, "sim.cpu1.o3.commit.committed_inst_type.int_div");
    let iq_mem_read = json_stat_u64(&json, "sim.cpu1.o3.iq.issued_inst_type.mem_read");
    let iq_mem_write = json_stat_u64(&json, "sim.cpu1.o3.iq.issued_inst_type.mem_write");
    let commit_mem_read = json_stat_u64(&json, "sim.cpu1.o3.commit.committed_inst_type.mem_read");
    let commit_mem_write = json_stat_u64(&json, "sim.cpu1.o3.commit.committed_inst_type.mem_write");
    for (path, value) in [
        ("system.cpu1.iq.issuedInstType.IntMult", iq_int_mul),
        ("system.cpu1.iq.issuedInstType_0::IntMult", iq_int_mul),
        ("system.cpu1.iq.issuedInstType.IntDiv", iq_int_div),
        ("system.cpu1.iq.issuedInstType_0::IntDiv", iq_int_div),
        ("system.cpu1.iq.issuedInstType.MemRead", iq_mem_read),
        ("system.cpu1.iq.issuedInstType_0::MemRead", iq_mem_read),
        ("system.cpu1.iq.issuedInstType.MemWrite", iq_mem_write),
        ("system.cpu1.iq.issuedInstType_0::MemWrite", iq_mem_write),
        (
            "system.cpu1.commit.committedInstType.IntMult",
            commit_int_mul,
        ),
        (
            "system.cpu1.commit.committedInstType_0::IntMult",
            commit_int_mul,
        ),
        (
            "system.cpu1.commit.committedInstType.IntDiv",
            commit_int_div,
        ),
        (
            "system.cpu1.commit.committedInstType_0::IntDiv",
            commit_int_div,
        ),
        (
            "system.cpu1.commit.committedInstType.MemRead",
            commit_mem_read,
        ),
        (
            "system.cpu1.commit.committedInstType_0::MemRead",
            commit_mem_read,
        ),
        (
            "system.cpu1.commit.committedInstType.MemWrite",
            commit_mem_write,
        ),
        (
            "system.cpu1.commit.committedInstType_0::MemWrite",
            commit_mem_write,
        ),
    ] {
        assert_json_stat(&json, path, "Count", value, "monotonic");
    }
    let insts_to_commit = json_stat_u64(&json, "sim.cpu1.o3.iew.insts_to_commit");
    let writeback_count = json_stat_u64(&json, "sim.cpu1.o3.iew.writeback_count");
    let producer_inst = json_stat_u64(&json, "sim.cpu1.o3.iew.producer_inst");
    let consumer_inst = json_stat_u64(&json, "sim.cpu1.o3.iew.consumer_inst");
    for (path, value) in [
        ("system.cpu1.iew.instsToCommit.total", insts_to_commit),
        ("system.cpu1.iew.instsToCommit::total", insts_to_commit),
        ("system.cpu1.iew.writebackCount.total", writeback_count),
        ("system.cpu1.iew.writebackCount::total", writeback_count),
        ("system.cpu1.iew.producerInst.total", producer_inst),
        ("system.cpu1.iew.producerInst::total", producer_inst),
        ("system.cpu1.iew.consumerInst.total", consumer_inst),
        ("system.cpu1.iew.consumerInst::total", consumer_inst),
    ] {
        assert_json_stat(&json, path, "Count", value, "monotonic");
    }
    let writeback_rate_ppm = json_stat_u64(&json, "sim.cpu1.o3.iew.writeback_rate_ppm");
    let producer_consumer_fanout_ppm =
        json_stat_u64(&json, "sim.cpu1.o3.iew.producer_consumer_fanout_ppm");
    assert_json_stat(
        &json,
        "system.cpu1.iew.wbRate",
        "Ppm",
        writeback_rate_ppm,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.iew.wbFanout",
        "Ppm",
        producer_consumer_fanout_ppm,
        "monotonic",
    );
    for path in [
        "system.cpu0.iew.instsToCommit.total",
        "system.cpu0.iew.instsToCommit::total",
        "system.cpu0.iew.writebackCount.total",
        "system.cpu0.iew.writebackCount::total",
        "system.cpu0.iew.producerInst.total",
        "system.cpu0.iew.producerInst::total",
        "system.cpu0.iew.consumerInst.total",
        "system.cpu0.iew.consumerInst::total",
        "system.cpu0.iew.wbRate",
        "system.cpu0.iew.wbFanout",
        "system.cpu.iew.instsToCommit.total",
        "system.cpu.iew.instsToCommit::total",
        "system.cpu.iew.writebackCount.total",
        "system.cpu.iew.writebackCount::total",
        "system.cpu.iew.producerInst.total",
        "system.cpu.iew.producerInst::total",
        "system.cpu.iew.consumerInst.total",
        "system.cpu.iew.consumerInst::total",
        "system.cpu.iew.wbRate",
        "system.cpu.iew.wbFanout",
        "system.cpu0.rob.maxOccupancy",
        "system.cpu0.lsq0.maxOccupancy",
        "system.cpu.rob.maxOccupancy",
        "system.cpu.lsq0.maxOccupancy",
        "system.cpu0.iq.issuedInstType.MemRead",
        "system.cpu0.iq.issuedInstType.MemWrite",
        "system.cpu0.iq.issuedInstType.IntMult",
        "system.cpu0.iq.issuedInstType.IntDiv",
        "system.cpu0.iq.issuedInstType_0::MemRead",
        "system.cpu0.iq.issuedInstType_0::MemWrite",
        "system.cpu0.iq.issuedInstType_0::IntMult",
        "system.cpu0.iq.issuedInstType_0::IntDiv",
        "system.cpu0.commit.committedInstType.MemRead",
        "system.cpu0.commit.committedInstType.MemWrite",
        "system.cpu0.commit.committedInstType.IntMult",
        "system.cpu0.commit.committedInstType.IntDiv",
        "system.cpu0.commit.committedInstType_0::MemRead",
        "system.cpu0.commit.committedInstType_0::MemWrite",
        "system.cpu0.commit.committedInstType_0::IntMult",
        "system.cpu0.commit.committedInstType_0::IntDiv",
        "system.cpu1.iq.issuedInstType.FloatAdd",
        "system.cpu1.iq.issuedInstType.FloatCmp",
        "system.cpu1.iq.issuedInstType.FloatMisc",
        "system.cpu1.iq.issuedInstType.FloatMult",
        "system.cpu1.iq.issuedInstType.FloatMultAcc",
        "system.cpu1.iq.issuedInstType.FloatDiv",
        "system.cpu1.iq.issuedInstType.FloatSqrt",
        "system.cpu1.iq.issuedInstType.SimdMult",
        "system.cpu1.iq.issuedInstType.SimdDiv",
        "system.cpu1.iq.issuedInstType.SimdFloatAdd",
        "system.cpu1.iq.issuedInstType.SimdFloatCmp",
        "system.cpu1.iq.issuedInstType.SimdFloatMisc",
        "system.cpu1.iq.issuedInstType.SimdFloatMult",
        "system.cpu1.iq.issuedInstType.SimdFloatMultAcc",
        "system.cpu1.iq.issuedInstType.SimdFloatDiv",
        "system.cpu1.iq.issuedInstType.SimdFloatSqrt",
        "system.cpu1.commit.committedInstType.FloatAdd",
        "system.cpu1.commit.committedInstType.FloatCmp",
        "system.cpu1.commit.committedInstType.FloatMisc",
        "system.cpu1.commit.committedInstType.FloatMult",
        "system.cpu1.commit.committedInstType.FloatMultAcc",
        "system.cpu1.commit.committedInstType.FloatDiv",
        "system.cpu1.commit.committedInstType.FloatSqrt",
        "system.cpu1.commit.committedInstType.SimdMult",
        "system.cpu1.commit.committedInstType.SimdDiv",
        "system.cpu1.commit.committedInstType.SimdFloatAdd",
        "system.cpu1.commit.committedInstType.SimdFloatCmp",
        "system.cpu1.commit.committedInstType.SimdFloatMisc",
        "system.cpu1.commit.committedInstType.SimdFloatMult",
        "system.cpu1.commit.committedInstType.SimdFloatMultAcc",
        "system.cpu1.commit.committedInstType.SimdFloatDiv",
        "system.cpu1.commit.committedInstType.SimdFloatSqrt",
        "system.cpu.iq.issuedInstType.MemRead",
        "system.cpu.iq.issuedInstType.MemWrite",
        "system.cpu.iq.issuedInstType.IntMult",
        "system.cpu.iq.issuedInstType.IntDiv",
        "system.cpu.commit.committedInstType.MemRead",
        "system.cpu.commit.committedInstType.MemWrite",
        "system.cpu.commit.committedInstType.IntMult",
        "system.cpu.commit.committedInstType.IntDiv",
        "system.cpu.iq.issuedInstType_0::MemRead",
        "system.cpu.iq.issuedInstType_0::MemWrite",
        "system.cpu.iq.issuedInstType_0::IntMult",
        "system.cpu.iq.issuedInstType_0::IntDiv",
        "system.cpu.commit.committedInstType_0::MemRead",
        "system.cpu.commit.committedInstType_0::MemWrite",
        "system.cpu.commit.committedInstType_0::IntMult",
        "system.cpu.commit.committedInstType_0::IntDiv",
    ] {
        assert_json_stat_absent(&json, path);
    }
    let core0 = json
        .pointer("/cores/0")
        .unwrap_or_else(|| panic!("missing core 0 summary: {json}"));
    let core1_o3 = json
        .pointer("/cores/1/o3_runtime")
        .unwrap_or_else(|| panic!("missing hart 1 O3 runtime summary: {json}"));
    assert!(
        core0.pointer("/o3_runtime").is_none(),
        "functional hart 0 should not emit O3 runtime state: {core0}"
    );
    assert!(
        core1_o3
            .pointer("/instructions")
            .and_then(Value::as_u64)
            .is_some_and(|instructions| instructions >= 8),
        "hart 1 O3 runtime should record detailed instructions: {core1_o3}"
    );
    assert_eq!(
        core1_o3.pointer("/execution_mode").and_then(Value::as_str),
        Some("detailed"),
        "hart 1 O3 runtime summary should expose detailed execution-mode authority: {core1_o3}"
    );
    assert_eq!(
        core1_o3.pointer("/stats_epoch"),
        execution_mode_switch.pointer("/stats_epoch"),
        "hart 1 O3 runtime summary should expose switch stats epoch authority: {core1_o3}"
    );
    assert_eq!(
        core1_o3.pointer("/stats_reset_tick"),
        execution_mode_switch.pointer("/stats_reset_tick"),
        "hart 1 O3 runtime summary should expose switch stats reset tick authority: {core1_o3}"
    );
}

#[test]
fn rem6_run_exposes_multicore_o3_float_misc_op_class_aliases_by_active_hart() {
    let path = multicore_hart1_detailed_o3_float_misc_binary(
        "m5-switch-cpu-hart1-detailed-o3-float-misc-aliases",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    assert_json_stat_absent(&json, "sim.cpu0.o3.instructions");
    assert_json_stat_at_least(&json, "sim.cpu1.o3.instructions", "Count", 4, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );

    let iq_float_misc = json_stat_u64(&json, "sim.cpu1.o3.iq.issued_inst_type.float_misc");
    let iq_vector_float_misc =
        json_stat_u64(&json, "sim.cpu1.o3.iq.issued_inst_type.vector_float_misc");
    let commit_float_misc =
        json_stat_u64(&json, "sim.cpu1.o3.commit.committed_inst_type.float_misc");
    let commit_vector_float_misc = json_stat_u64(
        &json,
        "sim.cpu1.o3.commit.committed_inst_type.vector_float_misc",
    );
    for path in [
        "sim.cpu1.o3.iq.issued_inst_type.float_misc",
        "sim.cpu1.o3.iq.issued_inst_type.vector_float_misc",
        "sim.cpu1.o3.commit.committed_inst_type.float_misc",
        "sim.cpu1.o3.commit.committed_inst_type.vector_float_misc",
    ] {
        assert_json_stat(&json, path, "Count", 2, "monotonic");
    }

    for (path, value) in [
        ("system.cpu1.iq.issuedInstType.FloatMisc", iq_float_misc),
        ("system.cpu1.iq.issuedInstType_0::FloatMisc", iq_float_misc),
        (
            "system.cpu1.iq.issuedInstType.SimdFloatMisc",
            iq_vector_float_misc,
        ),
        (
            "system.cpu1.iq.issuedInstType_0::SimdFloatMisc",
            iq_vector_float_misc,
        ),
        (
            "system.cpu1.commit.committedInstType.FloatMisc",
            commit_float_misc,
        ),
        (
            "system.cpu1.commit.committedInstType_0::FloatMisc",
            commit_float_misc,
        ),
        (
            "system.cpu1.commit.committedInstType.SimdFloatMisc",
            commit_vector_float_misc,
        ),
        (
            "system.cpu1.commit.committedInstType_0::SimdFloatMisc",
            commit_vector_float_misc,
        ),
    ] {
        assert_json_stat(&json, path, "Count", value, "monotonic");
    }

    for path in [
        "system.cpu0.iq.issuedInstType.FloatMisc",
        "system.cpu0.iq.issuedInstType_0::FloatMisc",
        "system.cpu0.iq.issuedInstType.SimdFloatMisc",
        "system.cpu0.iq.issuedInstType_0::SimdFloatMisc",
        "system.cpu0.commit.committedInstType.FloatMisc",
        "system.cpu0.commit.committedInstType_0::FloatMisc",
        "system.cpu0.commit.committedInstType.SimdFloatMisc",
        "system.cpu0.commit.committedInstType_0::SimdFloatMisc",
        "system.cpu.iq.issuedInstType.FloatMisc",
        "system.cpu.iq.issuedInstType_0::FloatMisc",
        "system.cpu.iq.issuedInstType.SimdFloatMisc",
        "system.cpu.iq.issuedInstType_0::SimdFloatMisc",
        "system.cpu.commit.committedInstType.FloatMisc",
        "system.cpu.commit.committedInstType_0::FloatMisc",
        "system.cpu.commit.committedInstType.SimdFloatMisc",
        "system.cpu.commit.committedInstType_0::SimdFloatMisc",
    ] {
        assert_json_stat_absent(&json, path);
    }
}

#[test]
fn rem6_run_m5_dump_stats_filters_multicore_o3_structural_aliases_by_active_hart() {
    let path = multicore_hart_detailed_o3_dump_stats_binary(
        "m5-switch-cpu-hart1-detailed-o3-dump-structural-aliases",
        1,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    for (path, unit, minimum) in [
        ("system.cpu1.rob.writes", "Count", 1),
        ("system.cpu1.rob.maxOccupancy", "Count", 1),
        ("system.cpu1.rename.renamedOperands", "Count", 1),
        ("system.cpu1.iew.writebackCount.total", "Count", 1),
        ("system.cpu1.lsq0.loadBytes", "Byte", 4),
        ("system.cpu1.lsq0.maxOccupancy", "Count", 1),
        ("system.cpu1.iq.issuedInstType.MemRead", "Count", 1),
        ("system.cpu1.commit.committedInstType.MemWrite", "Count", 1),
    ] {
        assert_stats_dump_sample_at_least(dump, path, "counter", unit, minimum, "resettable");
    }
    let writeback_rate_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu1.o3.iew.writeback_rate_ppm",
    );
    let producer_consumer_fanout_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu1.o3.iew.producer_consumer_fanout_ppm",
    );
    for (dotted_path, bucket_path) in [
        (
            "system.cpu1.iew.instsToCommit.total",
            "system.cpu1.iew.instsToCommit::total",
        ),
        (
            "system.cpu1.iew.writebackCount.total",
            "system.cpu1.iew.writebackCount::total",
        ),
        (
            "system.cpu1.iew.producerInst.total",
            "system.cpu1.iew.producerInst::total",
        ),
        (
            "system.cpu1.iew.consumerInst.total",
            "system.cpu1.iew.consumerInst::total",
        ),
    ] {
        let value = stats_dump_sample_value(dump, dotted_path);
        assert_stats_dump_sample(dump, bucket_path, "counter", "Count", value, "resettable");
    }
    assert_stats_dump_sample(
        dump,
        "system.cpu1.iew.wbRate",
        "counter",
        "Ppm",
        writeback_rate_ppm,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu1.iew.wbFanout",
        "counter",
        "Ppm",
        producer_consumer_fanout_ppm,
        "resettable",
    );
    for path in [
        "system.cpu0.rob.writes",
        "system.cpu0.rob.maxOccupancy",
        "system.cpu0.rename.renamedOperands",
        "system.cpu0.iew.writebackCount.total",
        "system.cpu0.iew.instsToCommit::total",
        "system.cpu0.iew.writebackCount::total",
        "system.cpu0.iew.producerInst::total",
        "system.cpu0.iew.consumerInst::total",
        "system.cpu.iew.instsToCommit::total",
        "system.cpu.iew.writebackCount::total",
        "system.cpu.iew.producerInst::total",
        "system.cpu.iew.consumerInst::total",
        "system.cpu0.iew.wbRate",
        "system.cpu0.iew.wbFanout",
        "system.cpu.iew.wbRate",
        "system.cpu.iew.wbFanout",
        "system.cpu0.lsq0.loadBytes",
        "system.cpu0.lsq0.maxOccupancy",
        "system.cpu0.iq.issuedInstType.MemRead",
        "system.cpu0.commit.committedInstType.MemWrite",
    ] {
        assert_stats_dump_sample_absent(dump, path);
    }
}

#[test]
fn rem6_run_m5_dump_stats_keeps_multicore_cpu0_o3_aliases_target_qualified() {
    let path = multicore_hart_detailed_o3_dump_stats_binary(
        "m5-switch-cpu-hart0-detailed-o3-dump-structural-aliases",
        0,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    let dump = json
        .pointer("/host_actions/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {json}"));

    for (dotted_path, bucket_path) in [
        (
            "system.cpu0.iew.instsToCommit.total",
            "system.cpu0.iew.instsToCommit::total",
        ),
        (
            "system.cpu0.iew.writebackCount.total",
            "system.cpu0.iew.writebackCount::total",
        ),
        (
            "system.cpu0.iew.producerInst.total",
            "system.cpu0.iew.producerInst::total",
        ),
        (
            "system.cpu0.iew.consumerInst.total",
            "system.cpu0.iew.consumerInst::total",
        ),
    ] {
        let value = stats_dump_sample_value(dump, dotted_path);
        assert_stats_dump_sample(dump, bucket_path, "counter", "Count", value, "resettable");
    }
    let writeback_rate_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
    );
    let producer_consumer_fanout_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.producer_consumer_fanout_ppm",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu0.iew.wbRate",
        "counter",
        "Ppm",
        writeback_rate_ppm,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu0.iew.wbFanout",
        "counter",
        "Ppm",
        producer_consumer_fanout_ppm,
        "resettable",
    );
    for path in [
        "system.cpu.iew.instsToCommit.total",
        "system.cpu.iew.instsToCommit::total",
        "system.cpu.iew.writebackCount.total",
        "system.cpu.iew.writebackCount::total",
        "system.cpu.iew.producerInst.total",
        "system.cpu.iew.producerInst::total",
        "system.cpu.iew.consumerInst.total",
        "system.cpu.iew.consumerInst::total",
        "system.cpu.iew.wbRate",
        "system.cpu.iew.wbFanout",
        "system.cpu1.iew.wbRate",
        "system.cpu1.iew.wbFanout",
    ] {
        assert_stats_dump_sample_absent(dump, path);
    }
}

#[test]
fn rem6_run_m5_dump_stats_snapshots_detailed_o3_runtime_stats() {
    let path = detailed_o3_dump_stats_binary("m5-switch-cpu-detailed-o3-dump-runtime-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(dump.pointer("/epoch").and_then(Value::as_u64), Some(0));
    assert_eq!(dump.pointer("/reset_tick").and_then(Value::as_u64), Some(0));
    let samples = dump
        .pointer("/samples")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("stats dump should include snapshot samples: {dump}"));
    assert_eq!(
        dump.pointer("/sample_count").and_then(Value::as_u64),
        Some(samples.len() as u64)
    );
    assert_stats_dump_sample_absent(dump, "sim.cpu0.o3.instructions");
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.instructions",
        "counter",
        "Count",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.rob_allocations",
        "counter",
        "Count",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.rob_commits",
        "counter",
        "Count",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.rename_writes",
        "counter",
        "Count",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.lsq_load_bytes",
        "counter",
        "Byte",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_bytes",
        "counter",
        "Byte",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.max_rob_occupancy",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.max_lsq_occupancy",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.rename_map_entries",
        "counter",
        "Count",
        3,
        "resettable",
    );
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.snapshot.rob.count",
        "sim.host_actions.stats_dump.cpu0.o3.snapshot.rob.entries",
        "sim.host_actions.stats_dump.cpu0.o3.snapshot.lsq.count",
        "sim.host_actions.stats_dump.cpu0.o3.snapshot.lsq.entries",
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", 0, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.snapshot.rename_map.count",
        "sim.host_actions.stats_dump.cpu0.o3.snapshot.rename_map.entries",
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", 3, "resettable");
    }
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.iq.insts_issued", 6),
        ("sim.host_actions.stats_dump.cpu0.o3.iq.mem_insts_issued", 2),
        (
            "sim.host_actions.stats_dump.cpu0.o3.iq.issued_inst_type.mem_read",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.iq.issued_inst_type.mem_write",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.mem_read",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.mem_write",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.iew.dispatched_insts",
            6,
        ),
        ("sim.host_actions.stats_dump.cpu0.o3.iew.insts_to_commit", 6),
        ("sim.host_actions.stats_dump.cpu0.o3.iew.writeback_count", 6),
        ("sim.host_actions.stats_dump.cpu0.o3.iew.producer_inst", 3),
        ("sim.host_actions.stats_dump.cpu0.o3.iew.consumer_inst", 4),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }
    for (path, value) in [
        ("system.cpu.rob.writes", 6),
        ("system.cpu.rob.reads", 6),
        ("system.cpu.rename.renamedInsts", 6),
        ("system.cpu.rename.renamedOperands", 4),
        ("system.cpu.iew.dispatchedInsts", 6),
        ("system.cpu.iew.dispLoadInsts", 1),
        ("system.cpu.iew.dispStoreInsts", 1),
        ("system.cpu.iew.instsToCommit.total", 6),
        ("system.cpu.iew.writebackCount.total", 6),
        ("system.cpu.iew.producerInst.total", 3),
        ("system.cpu.iew.consumerInst.total", 4),
        ("system.cpu.lsq0.addedLoadsAndStores", 2),
        ("system.cpu.lsq0.storeLoadForwardingCandidates", 0),
        ("system.cpu.lsq0.storeLoadForwardingMatches", 0),
        ("system.cpu.lsq0.forwLoads", 0),
        ("system.cpu.iq.instsIssued", 6),
        ("system.cpu.iq.memInstsIssued", 2),
        ("system.cpu.iq.issuedInstType.MemRead", 1),
        ("system.cpu.iq.issuedInstType.MemWrite", 1),
        ("system.cpu.commit.committedInstType.MemRead", 1),
        ("system.cpu.commit.committedInstType.MemWrite", 1),
        ("system.cpu.rob.maxOccupancy", 1),
        ("system.cpu.lsq0.maxOccupancy", 1),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }
    for (path, value) in [
        ("system.cpu.lsq0.loadBytes", 4),
        ("system.cpu.lsq0.storeBytes", 4),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Byte", value, "resettable");
    }
    let writeback_rate_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
    );
    let producer_consumer_fanout_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.producer_consumer_fanout_ppm",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu.iew.wbRate",
        "counter",
        "Ppm",
        writeback_rate_ppm,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu.iew.wbFanout",
        "counter",
        "Ppm",
        producer_consumer_fanout_ppm,
        "resettable",
    );
    assert_json_stat(&json, "sim.cpu0.o3.instructions", "Count", 8, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.rename_writes", "Count", 5, "monotonic");
}

#[test]
fn rem6_run_m5_dump_stats_snapshots_o3_float_misc_fu_latency_classes() {
    let path = detailed_o3_float_misc_dump_stats_binary("m5-switch-cpu-o3-float-misc-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "counter",
        "Count",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
        "counter",
        "Cycle",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_latency_cycles",
        "counter",
        "Cycle",
        3,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_latency_cycles",
        "counter",
        "Cycle",
        3,
        "resettable",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_stats_omits_o3_runtime_snapshot_after_timing_switch() {
    let path = timing_switch_o3_dump_stats_binary("m5-switch-cpu-timing-no-o3-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--m5-switch-cpu-mode",
            "timing",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.instructions",
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_latency_cycles",
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_latency_cycles",
        "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.mem_read",
        "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.int_mul",
        "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
        "sim.host_actions.stats_dump.cpu0.o3.iew.producer_consumer_fanout_ppm",
        "system.cpu.rob.writes",
        "system.cpu.rob.reads",
        "system.cpu.rename.renamedInsts",
        "system.cpu.rename.renamedOperands",
        "system.cpu.iew.dispatchedInsts",
        "system.cpu.iew.dispLoadInsts",
        "system.cpu.iew.dispStoreInsts",
        "system.cpu.iew.instsToCommit.total",
        "system.cpu.iew.writebackCount.total",
        "system.cpu.iew.producerInst.total",
        "system.cpu.iew.consumerInst.total",
        "system.cpu.iew.wbRate",
        "system.cpu.iew.wbFanout",
        "system.cpu.lsq0.addedLoadsAndStores",
        "system.cpu.lsq0.loadBytes",
        "system.cpu.lsq0.storeBytes",
        "system.cpu.lsq0.storeLoadForwardingCandidates",
        "system.cpu.lsq0.storeLoadForwardingMatches",
        "system.cpu.lsq0.forwLoads",
        "system.cpu.iq.instsIssued",
        "system.cpu.iq.memInstsIssued",
        "system.cpu.iq.issuedInstType.MemRead",
        "system.cpu.iq.issuedInstType.MemWrite",
        "system.cpu.iq.issuedInstType.IntMult",
        "system.cpu.iq.issuedInstType.IntDiv",
        "system.cpu.iq.issuedInstType.FloatMisc",
        "system.cpu.iq.issuedInstType.SimdFloatMisc",
        "system.cpu.iq.branchInstsIssued",
        "system.cpu.commit.committedInstType.MemRead",
        "system.cpu.commit.committedInstType.MemWrite",
        "system.cpu.commit.committedInstType.IntMult",
        "system.cpu.commit.committedInstType.IntDiv",
        "system.cpu.commit.committedInstType.FloatMisc",
        "system.cpu.commit.committedInstType.SimdFloatMisc",
        "system.cpu.commit.branchMispredicts",
        "system.cpu.iew.branchRepair.targetlessMismatch",
        "system.cpu.iew.branchRepair.directionOnly",
        "system.cpu.iew.branchRepair.wrongTarget",
        "system.cpu.iew.branchRepair.total",
        "system.cpu.iew.predictedTakenIncorrect",
        "system.cpu.iew.predictedNotTakenIncorrect",
        "system.cpu.iew.branchMispredicts",
        "system.cpu.lsq0.operation.load",
        "system.cpu.lsq0.operation.total",
        "system.cpu.lsq0.ordering.acquireRelease",
        "system.cpu.lsq0.ordering.total",
    ] {
        assert_stats_dump_sample_absent(dump, path);
    }
}

#[test]
fn rem6_run_m5_dump_stats_before_detailed_switch_keeps_later_o3_snapshot() {
    let path = pre_dump_then_detailed_o3_dump_stats_binary("m5-pre-dump-before-o3-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump action: {host_actions}"));
    let second_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing second stats dump action: {host_actions}"));
    assert_stats_dump_sample_absent(
        first_dump,
        "sim.host_actions.stats_dump.cpu0.o3.instructions",
    );
    assert_stats_dump_sample_absent(
        first_dump,
        "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.int_mul",
    );
    assert_stats_dump_sample(
        second_dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        second_dump,
        "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.int_mul",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        second_dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_latency_cycles",
        "counter",
        "Cycle",
        19,
        "resettable",
    );
}

#[test]
fn rem6_run_m5_reset_stats_clears_detailed_o3_runtime_stats() {
    let path = detailed_o3_reset_stats_binary("m5-switch-cpu-detailed-o3-reset-runtime-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "140",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/host_actions/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let stats_reset = json
        .pointer("/host_actions/stats_resets/0")
        .unwrap_or_else(|| panic!("missing stats reset action: {json}"));
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("missing post-reset O3 runtime summary: {json}"));
    assert_eq!(
        o3_runtime.pointer("/stats_epoch"),
        stats_reset.pointer("/epoch"),
        "O3 runtime summary should expose the current stats reset epoch: {o3_runtime}"
    );
    assert_eq!(
        o3_runtime.pointer("/stats_reset_tick"),
        stats_reset.pointer("/tick"),
        "O3 runtime summary should expose the current stats reset tick: {o3_runtime}"
    );
    assert_json_stat(&json, "sim.cpu0.o3.instructions", "Count", 2, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rob_allocations",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(&json, "sim.cpu0.o3.rob_commits", "Count", 2, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_loads", "Count", 1, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_stores", "Count", 0, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_load_bytes", "Byte", 4, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_store_bytes", "Byte", 0, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.producer_inst",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.consumer_inst",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_reset_stats_scopes_o3_fu_class_dump_stats() {
    let path = detailed_o3_reset_fu_dump_stats_binary("m5-switch-cpu-o3-reset-fu-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "280",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(
        dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1),
        "post-reset dump should belong to the reset epoch: {dump}"
    );
    assert!(
        dump.pointer("/reset_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick > 0),
        "post-reset dump should record the reset tick: {dump}"
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
        "counter",
        "Cycle",
        21,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_latency_cycles",
        "counter",
        "Cycle",
        19,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
        "counter",
        "Count",
        0,
        "resettable",
    );
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.iq.insts_issued", 5),
        ("sim.host_actions.stats_dump.cpu0.o3.iq.mem_insts_issued", 0),
        (
            "sim.host_actions.stats_dump.cpu0.o3.iq.issued_inst_type.int_mul",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.iq.issued_inst_type.int_div",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.iq.issued_inst_type.float_misc",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.iq.issued_inst_type.vector_float_misc",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.int_mul",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.int_div",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.float_misc",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.vector_float_misc",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.iew.dispatched_insts",
            5,
        ),
        ("sim.host_actions.stats_dump.cpu0.o3.iew.insts_to_commit", 5),
        ("sim.host_actions.stats_dump.cpu0.o3.iew.writeback_count", 5),
        ("sim.host_actions.stats_dump.cpu0.o3.iew.producer_inst", 2),
        ("sim.host_actions.stats_dump.cpu0.o3.iew.consumer_inst", 4),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }
    assert_stats_dump_sample_at_least(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
        "counter",
        "Ppm",
        1,
        "resettable",
    );
    let writeback_rate_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.producer_consumer_fanout_ppm",
        "counter",
        "Ppm",
        ratio_ppm(2, 4),
        "resettable",
    );
    let producer_consumer_fanout_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.producer_consumer_fanout_ppm",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu.iew.wbRate",
        "counter",
        "Ppm",
        writeback_rate_ppm,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu.iew.wbFanout",
        "counter",
        "Ppm",
        producer_consumer_fanout_ppm,
        "resettable",
    );
    for (path, value) in [
        ("system.cpu.iq.issuedInstType.IntMult", 1),
        ("system.cpu.iq.issuedInstType.IntDiv", 1),
        ("system.cpu.iq.issuedInstType.FloatMisc", 0),
        ("system.cpu.iq.issuedInstType.SimdFloatMisc", 0),
        ("system.cpu.commit.committedInstType.IntMult", 1),
        ("system.cpu.commit.committedInstType.IntDiv", 1),
        ("system.cpu.commit.committedInstType.FloatMisc", 0),
        ("system.cpu.commit.committedInstType.SimdFloatMisc", 0),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        21,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_reset_stats_scopes_multicore_o3_fu_class_dump_aliases_by_active_hart() {
    let path = multicore_hart1_detailed_o3_reset_fu_dump_stats_binary(
        "m5-switch-cpu-hart1-detailed-o3-reset-fu-dump-aliases",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "320",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "2",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(
        dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1),
        "post-reset dump should belong to the reset epoch: {dump}"
    );
    assert!(
        dump.pointer("/reset_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick > 0),
        "post-reset dump should record the reset tick: {dump}"
    );

    for (path, unit, value) in [
        (
            "sim.host_actions.stats_dump.cpu1.o3.fu_latency_instructions",
            "Count",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.fu_latency_cycles",
            "Cycle",
            21,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.fu_integer_mul_instructions",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.fu_integer_div_latency_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.fu_float_misc_instructions",
            "Count",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.fu_vector_float_misc_instructions",
            "Count",
            0,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", unit, value, "resettable");
    }

    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu1.o3.iq.insts_issued", 5),
        ("sim.host_actions.stats_dump.cpu1.o3.iq.mem_insts_issued", 0),
        (
            "sim.host_actions.stats_dump.cpu1.o3.iq.issued_inst_type.int_mul",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.iq.issued_inst_type.int_div",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.iq.issued_inst_type.float_misc",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.iq.issued_inst_type.vector_float_misc",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.commit.committed_inst_type.int_mul",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.commit.committed_inst_type.int_div",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.commit.committed_inst_type.float_misc",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.commit.committed_inst_type.vector_float_misc",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.iew.dispatched_insts",
            5,
        ),
        ("sim.host_actions.stats_dump.cpu1.o3.iew.insts_to_commit", 5),
        ("sim.host_actions.stats_dump.cpu1.o3.iew.writeback_count", 5),
        ("sim.host_actions.stats_dump.cpu1.o3.iew.producer_inst", 2),
        ("sim.host_actions.stats_dump.cpu1.o3.iew.consumer_inst", 4),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }
    assert_stats_dump_sample_at_least(
        dump,
        "sim.host_actions.stats_dump.cpu1.o3.iew.writeback_rate_ppm",
        "counter",
        "Ppm",
        1,
        "resettable",
    );
    let writeback_rate_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu1.o3.iew.writeback_rate_ppm",
    );
    let producer_consumer_fanout_ppm = stats_dump_sample_value(
        dump,
        "sim.host_actions.stats_dump.cpu1.o3.iew.producer_consumer_fanout_ppm",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu1.iew.wbRate",
        "counter",
        "Ppm",
        writeback_rate_ppm,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "system.cpu1.iew.wbFanout",
        "counter",
        "Ppm",
        producer_consumer_fanout_ppm,
        "resettable",
    );

    for (path, value) in [
        ("system.cpu1.iq.issuedInstType.IntMult", 1),
        ("system.cpu1.iq.issuedInstType_0::IntMult", 1),
        ("system.cpu1.iq.issuedInstType.IntDiv", 1),
        ("system.cpu1.iq.issuedInstType_0::IntDiv", 1),
        ("system.cpu1.iq.issuedInstType.FloatMisc", 0),
        ("system.cpu1.iq.issuedInstType_0::FloatMisc", 0),
        ("system.cpu1.iq.issuedInstType.SimdFloatMisc", 0),
        ("system.cpu1.iq.issuedInstType_0::SimdFloatMisc", 0),
        ("system.cpu1.commit.committedInstType.IntMult", 1),
        ("system.cpu1.commit.committedInstType_0::IntMult", 1),
        ("system.cpu1.commit.committedInstType.IntDiv", 1),
        ("system.cpu1.commit.committedInstType_0::IntDiv", 1),
        ("system.cpu1.commit.committedInstType.FloatMisc", 0),
        ("system.cpu1.commit.committedInstType_0::FloatMisc", 0),
        ("system.cpu1.commit.committedInstType.SimdFloatMisc", 0),
        ("system.cpu1.commit.committedInstType_0::SimdFloatMisc", 0),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }

    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "sim.host_actions.stats_dump.cpu0.o3.iq.issued_inst_type.int_mul",
        "sim.host_actions.stats_dump.cpu0.o3.commit.committed_inst_type.int_mul",
        "system.cpu0.iq.issuedInstType.IntMult",
        "system.cpu0.iq.issuedInstType_0::IntMult",
        "system.cpu0.commit.committedInstType.IntMult",
        "system.cpu0.commit.committedInstType_0::IntMult",
        "system.cpu.iq.issuedInstType.IntMult",
        "system.cpu.iq.issuedInstType_0::IntMult",
        "system.cpu.commit.committedInstType.IntMult",
        "system.cpu.commit.committedInstType_0::IntMult",
        "system.cpu.iew.wbRate",
        "system.cpu.iew.wbFanout",
    ] {
        assert_stats_dump_sample_absent(dump, path);
    }

    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_latency_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.fu_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.iq.issuedInstType_0::IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_instructions");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType_0::IntMult");
}

#[test]
fn rem6_run_m5_dump_reset_stats_snapshots_then_resets_o3_fu_classes() {
    let path = detailed_o3_dump_reset_fu_stats_binary("m5-switch-cpu-o3-dump-reset-fu-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "320",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing stats dump action: {host_actions}"));
    assert_eq!(
        dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0),
        "dump-reset should snapshot the old epoch before resetting: {dump}"
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_instructions",
        "counter",
        "Count",
        4,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_latency_cycles",
        "counter",
        "Cycle",
        6,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_instructions",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_instructions",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_latency_cycles",
        "counter",
        "Cycle",
        0,
        "resettable",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        21,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_integer_div_latency_cycles",
        "Cycle",
        19,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        0,
        "monotonic",
    );
}
