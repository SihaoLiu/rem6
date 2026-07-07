use super::*;

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
}

#[test]
fn rem6_run_m5_dump_stats_filters_multicore_o3_structural_aliases_by_active_hart() {
    let path = multicore_hart1_detailed_o3_dump_stats_binary(
        "m5-switch-cpu-hart1-detailed-o3-dump-structural-aliases",
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

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_branch_repair_snapshot() {
    let path = detailed_o3_branch_dump_reset_stats_binary(
        "m5-switch-cpu-o3-branch-repair-dump-reset-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "340",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--dump-memory",
            "0x80000080:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("0b000000000000000000000000000000")
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
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-reset stats dump action: {host_actions}"));
    assert_eq!(
        pre_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0),
        "dump-reset should snapshot the branch-repair epoch before resetting: {pre_reset_dump}"
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_targetless_mismatches",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_direction_only_mismatches",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_wrong_targets",
        "counter",
        "Count",
        0,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_targetless_mismatch_kind.direct_conditional",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_direction_only_kind.direct_unconditional",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.predicted_taken_incorrect",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.predicted_not_taken_incorrect",
        "counter",
        "Count",
        2,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.iew.branch_mispredicts",
        "counter",
        "Count",
        3,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.commit.branch_mispredicts",
        "counter",
        "Count",
        3,
        "resettable",
    );
    assert_stats_dump_sample(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.iq.branch_insts_issued",
        "counter",
        "Count",
        3,
        "resettable",
    );
    for (path, value) in [
        ("system.cpu.iew.branchRepair.targetlessMismatch", 1),
        ("system.cpu.iew.branchRepair.directionOnly", 2),
        ("system.cpu.iew.branchRepair.wrongTarget", 0),
        ("system.cpu.iew.branchRepair.total", 3),
        ("system.cpu.iew.branchRepair_0::TargetlessMismatch", 1),
        ("system.cpu.iew.branchRepair_0::DirectionOnly", 2),
        ("system.cpu.iew.branchRepair_0::WrongTarget", 0),
        ("system.cpu.iew.branchRepair_0::total", 3),
        ("system.cpu.iew.predictedTakenIncorrect", 1),
        ("system.cpu.iew.predictedNotTakenIncorrect", 2),
        ("system.cpu.iew.branchMispredicts", 3),
        ("system.cpu.commit.branchMispredicts", 3),
        ("system.cpu.iq.branchInstsIssued", 3),
    ] {
        assert_stats_dump_sample(
            pre_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }

    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));
    assert_eq!(
        post_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1),
        "post-reset dump should belong to the reset epoch: {post_reset_dump}"
    );
    assert!(
        post_reset_dump
            .pointer("/reset_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick > 0),
        "post-reset dump should record the reset tick: {post_reset_dump}"
    );
    assert_stats_dump_sample(
        post_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_instructions",
        "counter",
        "Count",
        1,
        "resettable",
    );
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_targetless_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_direction_only_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_wrong_targets",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_targetless_mismatch_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_repair_direction_only_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.iew.predicted_taken_incorrect",
        "sim.host_actions.stats_dump.cpu0.o3.iew.predicted_not_taken_incorrect",
        "sim.host_actions.stats_dump.cpu0.o3.iew.branch_mispredicts",
        "sim.host_actions.stats_dump.cpu0.o3.commit.branch_mispredicts",
        "sim.host_actions.stats_dump.cpu0.o3.iq.branch_insts_issued",
        "system.cpu.iew.branchRepair.targetlessMismatch",
        "system.cpu.iew.branchRepair.directionOnly",
        "system.cpu.iew.branchRepair.wrongTarget",
        "system.cpu.iew.branchRepair.total",
        "system.cpu.iew.branchRepair_0::TargetlessMismatch",
        "system.cpu.iew.branchRepair_0::DirectionOnly",
        "system.cpu.iew.branchRepair_0::WrongTarget",
        "system.cpu.iew.branchRepair_0::total",
        "system.cpu.iew.predictedTakenIncorrect",
        "system.cpu.iew.predictedNotTakenIncorrect",
        "system.cpu.iew.branchMispredicts",
        "system.cpu.commit.branchMispredicts",
        "system.cpu.iq.branchInstsIssued",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_repair_targetless_mismatches",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_repair_direction_only_mismatches",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_repair_wrong_targets",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.predicted_taken_incorrect",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.predicted_not_taken_incorrect",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.branch_insts_issued",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_branch_event_snapshot() {
    let path = detailed_o3_branch_dump_reset_stats_binary(
        "m5-switch-cpu-o3-branch-event-dump-reset-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "340",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--dump-memory",
            "0x80000080:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("0b000000000000000000000000000000")
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
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-reset stats dump action: {host_actions}"));
    assert_eq!(
        pre_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0),
        "dump-reset should snapshot the branch-event epoch before resetting: {pre_reset_dump}"
    );
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.branch_event.branches", 3),
        ("sim.host_actions.stats_dump.cpu0.o3.branch_event.taken", 2),
        ("sim.host_actions.stats_dump.cpu0.o3.branch_event.not_taken", 1),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_taken",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_not_taken",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_targets",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_matches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_mismatches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.resolved_targets",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.mispredictions",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashes",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_targets",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_targets_without_link_writes",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.not_taken_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_taken_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_not_taken_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_mismatch_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squash_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squash_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_without_link_write_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_without_link_write_kind.direct_unconditional",
            2,
        ),
    ] {
        assert_stats_dump_sample(
            pre_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }

    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));
    assert_eq!(
        post_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1),
        "post-reset dump should belong to the reset epoch: {post_reset_dump}"
    );
    assert!(
        post_reset_dump
            .pointer("/reset_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick > 0),
        "post-reset dump should record the reset tick: {post_reset_dump}"
    );
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.branches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.taken",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.not_taken",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_taken",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_not_taken",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_targets",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_matches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.resolved_targets",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.mispredictions",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashes",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_targets",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_targets_without_link_writes",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.not_taken_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_taken_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_not_taken_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_mismatch_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squash_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squash_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_without_link_write_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_without_link_write_kind.direct_unconditional",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.branches",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.predicted_targets",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.squashes",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_branch_event_link_kind_snapshot() {
    let path = detailed_o3_indirect_call_wrong_target_dump_reset_stats_binary(
        "m5-switch-cpu-o3-branch-event-link-kind-dump-reset-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
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
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-reset stats dump action: {host_actions}"));
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.branch_event.branches", 2),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.mispredictions",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.direct_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.link_write_kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.call_indirect",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.direct_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.direct_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.direct_unconditional",
            1,
        ),
    ] {
        assert_stats_dump_sample(pre_reset_dump, path, "counter", "Count", value, "resettable");
    }

    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.branches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.link_writes",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_writes",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.mispredictions",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.link_write_kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.without_link_write_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_kind.direct_unconditional",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.mispredictions",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.without_link_write_kind.call_indirect",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.without_link_write_kind.direct_unconditional",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_lsq_matrix_snapshot() {
    let path = detailed_o3_lsq_matrix_dump_reset_stats_binary(
        "m5-switch-cpu-o3-lsq-matrix-dump-reset-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000080:16",
            "--dump-memory",
            "0x80000090:16",
            "--dump-memory",
            "0x800000a0:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("04000000000000000900000000000000")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("00000000000000000300000000000000")
    );
    assert_eq!(
        json.pointer("/memory/2/hex").and_then(Value::as_str),
        Some("88776655443322110100000000000000")
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
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-reset stats dump action: {host_actions}"));
    assert_eq!(
        pre_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0),
        "dump-reset should snapshot the LSQ epoch before resetting: {pre_reset_dump}"
    );
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load", 1),
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store", 3),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.acquire",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.release",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.acquire_release",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_conditional_failures",
            0,
        ),
    ] {
        assert_stats_dump_sample(
            pre_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (path, value) in [
        ("system.cpu.lsq0.operation.load", 1),
        ("system.cpu.lsq0.operation.store", 3),
        ("system.cpu.lsq0.operation.loadReserved", 1),
        ("system.cpu.lsq0.operation.storeConditional", 1),
        ("system.cpu.lsq0.operation.atomic", 1),
        ("system.cpu.lsq0.operation.total", 7),
        ("system.cpu.lsq0.ordering.acquire", 1),
        ("system.cpu.lsq0.ordering.release", 1),
        ("system.cpu.lsq0.ordering.acquireRelease", 1),
        ("system.cpu.lsq0.ordering.total", 3),
    ] {
        assert_stats_dump_sample(
            pre_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (path, unit, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_samples",
            "Count",
            7,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_ticks",
            "Tick",
            14,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_max_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_min_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_avg_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_latency_samples",
            "Count",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_latency_ticks",
            "Tick",
            6,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic_latency_ticks",
            "Tick",
            2,
        ),
    ] {
        assert_stats_dump_sample(pre_reset_dump, path, "counter", unit, value, "resettable");
    }

    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));
    assert_eq!(
        post_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1),
        "post-reset dump should belong to the reset epoch: {post_reset_dump}"
    );
    assert!(
        post_reset_dump
            .pointer("/reset_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick > 0),
        "post-reset dump should record the reset tick: {post_reset_dump}"
    );
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load", 0),
        ("sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store", 1),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_reserved",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.atomic",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.acquire",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.release",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_ordering.acquire_release",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_conditional_failures",
            1,
        ),
    ] {
        assert_stats_dump_sample(
            post_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (path, value) in [
        ("system.cpu.lsq0.operation.load", 0),
        ("system.cpu.lsq0.operation.store", 1),
        ("system.cpu.lsq0.operation.storeConditional", 1),
        ("system.cpu.lsq0.operation.total", 2),
        ("system.cpu.lsq0.ordering.acquireRelease", 0),
        ("system.cpu.lsq0.ordering.total", 0),
    ] {
        assert_stats_dump_sample(
            post_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }
    for (path, unit, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_samples",
            "Count",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_max_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_min_ticks",
            "Tick",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_data_latency_avg_ticks",
            "Tick",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_latency_samples",
            "Count",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_latency_ticks",
            "Tick",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_latency_ticks",
            "Tick",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional_latency_samples",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_conditional_latency_ticks",
            "Tick",
            0,
        ),
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", unit, value, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_operation.store_conditional",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_conditional_failures",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_lsq_forwarding_snapshot() {
    let path = detailed_o3_lsq_forwarding_dump_reset_stats_binary(
        "m5-switch-cpu-o3-lsq-forwarding-dump-reset-stats",
    );

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
            "--dump-memory",
            "0x80000080:8",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("5a00000033000000")
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
    assert_eq!(
        host_actions
            .pointer("/stats_reset_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let pre_reset_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing pre-reset stats dump action: {host_actions}"));
    assert_eq!(
        pre_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(0)
    );
    for (path, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_candidates",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_matches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_suppressed",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_candidates",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_matches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_suppressed",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_address_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_byte_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_candidates",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_matches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_suppressed",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_address_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_byte_mismatches",
            0,
        ),
        ("system.cpu.lsq0.storeLoadForwardingSuppressed", 0),
        ("system.cpu.lsq0.storeLoadForwardingAddressMismatches", 0),
        ("system.cpu.lsq0.storeLoadForwardingByteMismatches", 0),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
            1,
        ),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
            1,
        ),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingSuppressed",
            0,
        ),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingAddressMismatches",
            0,
        ),
        (
            "system.cpu.lsq0.operation.load.storeLoadForwardingByteMismatches",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingCandidates",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingMatches",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingSuppressed",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingAddressMismatches",
            0,
        ),
        (
            "system.cpu.lsq0.operation.store.storeLoadForwardingByteMismatches",
            0,
        ),
    ] {
        assert_stats_dump_sample(
            pre_reset_dump,
            path,
            "counter",
            "Count",
            value,
            "resettable",
        );
    }

    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));
    assert_eq!(
        post_reset_dump.pointer("/epoch").and_then(Value::as_u64),
        Some(1)
    );
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_matches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
        "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_suppressed",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_suppressed",
        "system.cpu.lsq0.storeLoadForwardingSuppressed",
        "system.cpu.lsq0.operation.load.storeLoadForwardingSuppressed",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 2, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_address_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load_forwarding_byte_mismatches",
        "system.cpu.lsq0.storeLoadForwardingAddressMismatches",
        "system.cpu.lsq0.storeLoadForwardingByteMismatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingAddressMismatches",
        "system.cpu.lsq0.operation.load.storeLoadForwardingByteMismatches",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 1, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_suppressed",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_address_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store_forwarding_byte_mismatches",
        "system.cpu.lsq0.operation.store.storeLoadForwardingSuppressed",
        "system.cpu.lsq0.operation.store.storeLoadForwardingAddressMismatches",
        "system.cpu.lsq0.operation.store.storeLoadForwardingByteMismatches",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }
}

#[test]
fn rem6_run_records_o3_lsq_store_load_matches_after_detailed_switch() {
    let path =
        detailed_o3_lsq_store_load_match_binary("m5-switch-cpu-detailed-o3-lsq-store-load-match");

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
    assert_json_stat(&json, "sim.cpu0.o3.lsq_loads", "Count", 1, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.lsq_stores", "Count", 1, "monotonic");
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );
    for path in [
        "sim.cpu0.o3.lsq_operation.store_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.store_forwarding_matches",
        "sim.cpu0.o3.lsq_operation.store_conditional_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.store_conditional_forwarding_matches",
        "sim.cpu0.o3.lsq_operation.atomic_forwarding_candidates",
        "sim.cpu0.o3.lsq_operation.atomic_forwarding_matches",
    ] {
        assert_json_stat(&json, path, "Count", 0, "monotonic");
    }
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime
            .pointer("/lsq_operation_load_forwarding_candidates")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_operation_load_forwarding_matches")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_operation_store_forwarding_candidates")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        o3_runtime
            .pointer("/lsq_operation_store_forwarding_matches")
            .and_then(Value::as_u64),
        Some(0)
    );
}

#[test]
fn rem6_run_records_o3_fu_latency_stats_after_detailed_switch() {
    let path = detailed_o3_fu_latency_binary("m5-switch-cpu-detailed-o3-fu-latency-stats");

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
        "sim.cpu0.o3.fu_integer_mul_latency_cycles",
        "Cycle",
        2,
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
        "sim.cpu0.o3.iq.issued_inst_type.int_mul",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.int_div",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.IntDiv",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::IntDiv",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.int_mul",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::IntMult",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.int_div",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.IntDiv",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::IntDiv",
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
}

#[test]
fn rem6_run_records_o3_float_misc_fu_latency_stats_after_detailed_switch() {
    let path =
        detailed_o3_float_misc_fu_latency_binary("m5-switch-cpu-detailed-o3-float-misc-stats");

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
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_float_misc_latency_cycles",
        "Cycle",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_vector_float_misc_latency_cycles",
        "Cycle",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.FloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::FloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.issued_inst_type.vector_float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType.SimdFloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.issuedInstType_0::SimdFloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.FloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::FloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.vector_float_misc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType.SimdFloatMisc",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.committedInstType_0::SimdFloatMisc",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_runtime_json_exposes_extended_float_fu_latency_classes() {
    let path = detailed_o3_float_extended_fu_latency_binary(
        "m5-switch-cpu-detailed-o3-float-extended-runtime-json",
    );

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
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime
            .pointer("/fu_latency_instructions")
            .and_then(Value::as_u64),
        Some(6)
    );

    for class in [
        "float_add",
        "float_fma",
        "float_sqrt",
        "vector_float_add",
        "vector_float_fma",
        "vector_float_sqrt",
    ] {
        let instruction_path = format!("/fu_{class}_instructions");
        let latency_path = format!("/fu_{class}_latency_cycles");
        let stat_instruction_path = format!("sim.cpu0.o3.fu_{class}_instructions");
        let stat_latency_path = format!("sim.cpu0.o3.fu_{class}_latency_cycles");
        let runtime_instructions = o3_runtime
            .pointer(&instruction_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {instruction_path}: {o3_runtime}")
            });
        let runtime_latency_cycles = o3_runtime
            .pointer(&latency_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {latency_path}: {o3_runtime}")
            });
        assert_eq!(
            runtime_instructions, 1,
            "structured O3 runtime JSON should count {instruction_path}: {o3_runtime}"
        );
        assert!(
            runtime_latency_cycles > 0,
            "structured O3 runtime JSON should count positive {latency_path}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_instruction_path),
            runtime_instructions,
            "stat registry should match structured runtime {instruction_path}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_latency_path),
            runtime_latency_cycles,
            "stat registry should match structured runtime {latency_path}"
        );
    }
    for (source_class, alias_class) in [
        ("float_add", "FloatAdd"),
        ("float_fma", "FloatMultAcc"),
        ("float_sqrt", "FloatSqrt"),
        ("vector_float_add", "SimdFloatAdd"),
        ("vector_float_fma", "SimdFloatMultAcc"),
        ("vector_float_sqrt", "SimdFloatSqrt"),
    ] {
        let value = json_stat_u64(
            &json,
            &format!("sim.cpu0.o3.iq.issued_inst_type.{source_class}"),
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.iq.issuedInstType.{alias_class}"),
            "Count",
            value,
            "monotonic",
        );
        let value = json_stat_u64(
            &json,
            &format!("sim.cpu0.o3.commit.committed_inst_type.{source_class}"),
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.commit.committedInstType.{alias_class}"),
            "Count",
            value,
            "monotonic",
        );
    }
    for class in [
        "float_mul",
        "float_div",
        "vector_float_mul",
        "vector_float_div",
    ] {
        let instruction_path = format!("/fu_{class}_instructions");
        let latency_path = format!("/fu_{class}_latency_cycles");
        assert_eq!(
            o3_runtime
                .pointer(&instruction_path)
                .and_then(Value::as_u64),
            Some(0),
            "structured O3 runtime JSON should expose inactive {instruction_path}: {o3_runtime}"
        );
        assert_eq!(
            o3_runtime.pointer(&latency_path).and_then(Value::as_u64),
            Some(0),
            "structured O3 runtime JSON should expose inactive {latency_path}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, &format!("sim.cpu0.o3.fu_{class}_instructions")),
            0,
            "stat registry should expose inactive {instruction_path}"
        );
        assert_eq!(
            json_stat_value(&json, &format!("sim.cpu0.o3.fu_{class}_latency_cycles")),
            0,
            "stat registry should expose inactive {latency_path}"
        );
    }
    for (source_class, alias_class) in [
        ("float_compare", "FloatCmp"),
        ("float_mul", "FloatMult"),
        ("float_div", "FloatDiv"),
        ("vector_float_compare", "SimdFloatCmp"),
        ("vector_float_mul", "SimdFloatMult"),
        ("vector_float_div", "SimdFloatDiv"),
    ] {
        assert_json_stat(
            &json,
            &format!("system.cpu.iq.issuedInstType.{alias_class}"),
            "Count",
            json_stat_u64(
                &json,
                &format!("sim.cpu0.o3.iq.issued_inst_type.{source_class}"),
            ),
            "monotonic",
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.commit.committedInstType.{alias_class}"),
            "Count",
            json_stat_u64(
                &json,
                &format!("sim.cpu0.o3.commit.committed_inst_type.{source_class}"),
            ),
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_vector_integer_fu_latency_classes() {
    let path = detailed_o3_vector_integer_fu_latency_binary(
        "m5-switch-cpu-detailed-o3-vector-integer-runtime-json",
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
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert_eq!(
        o3_runtime
            .pointer("/fu_latency_instructions")
            .and_then(Value::as_u64),
        Some(2)
    );

    for class in ["vector_integer_mul", "vector_integer_div"] {
        let instruction_path = format!("/fu_{class}_instructions");
        let latency_path = format!("/fu_{class}_latency_cycles");
        let stat_instruction_path = format!("sim.cpu0.o3.fu_{class}_instructions");
        let stat_latency_path = format!("sim.cpu0.o3.fu_{class}_latency_cycles");
        let runtime_instructions = o3_runtime
            .pointer(&instruction_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {instruction_path}: {o3_runtime}")
            });
        let runtime_latency_cycles = o3_runtime
            .pointer(&latency_path)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {latency_path}: {o3_runtime}")
            });
        assert_eq!(
            runtime_instructions, 1,
            "structured O3 runtime JSON should count {instruction_path}: {o3_runtime}"
        );
        assert!(
            runtime_latency_cycles > 0,
            "structured O3 runtime JSON should count positive {latency_path}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_instruction_path),
            runtime_instructions,
            "stat registry should match structured runtime {instruction_path}"
        );
        assert_eq!(
            json_stat_value(&json, &stat_latency_path),
            runtime_latency_cycles,
            "stat registry should match structured runtime {latency_path}"
        );
    }

    for (source_class, alias_class) in [
        ("vector_integer_mul", "SimdMult"),
        ("vector_integer_div", "SimdDiv"),
    ] {
        let expected_instructions = json_stat_value(
            &json,
            &format!("sim.cpu0.o3.fu_{source_class}_instructions"),
        );
        let value = json_stat_u64(
            &json,
            &format!("sim.cpu0.o3.iq.issued_inst_type.{source_class}"),
        );
        assert_eq!(
            value, expected_instructions,
            "IQ issued op-class count should match FU class runtime count for {source_class}"
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.iq.issuedInstType.{alias_class}"),
            "Count",
            value,
            "monotonic",
        );
        let value = json_stat_u64(
            &json,
            &format!("sim.cpu0.o3.commit.committed_inst_type.{source_class}"),
        );
        assert_eq!(
            value, expected_instructions,
            "commit op-class count should match FU class runtime count for {source_class}"
        );
        assert_json_stat(
            &json,
            &format!("system.cpu.commit.committedInstType.{alias_class}"),
            "Count",
            value,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_iq_iew_commit_matrices() {
    let path =
        detailed_o3_iq_iew_commit_matrix_binary("m5-switch-cpu-detailed-o3-iq-iew-commit-json");

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
            "--dump-memory",
            "0x800000a0:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("78563412785634120000000000000000")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (pointer, stat_path) in [
        (
            "/iq/issued_inst_type/mem_read",
            "sim.cpu0.o3.iq.issued_inst_type.mem_read",
        ),
        (
            "/iq/issued_inst_type/mem_write",
            "sim.cpu0.o3.iq.issued_inst_type.mem_write",
        ),
        (
            "/iq/issued_inst_type/int_mul",
            "sim.cpu0.o3.iq.issued_inst_type.int_mul",
        ),
        (
            "/iq/issued_inst_type/int_div",
            "sim.cpu0.o3.iq.issued_inst_type.int_div",
        ),
        (
            "/iq/issued_inst_type/float_misc",
            "sim.cpu0.o3.iq.issued_inst_type.float_misc",
        ),
        (
            "/iq/issued_inst_type/vector_float_misc",
            "sim.cpu0.o3.iq.issued_inst_type.vector_float_misc",
        ),
        (
            "/commit/committed_inst_type/mem_read",
            "sim.cpu0.o3.commit.committed_inst_type.mem_read",
        ),
        (
            "/commit/committed_inst_type/mem_write",
            "sim.cpu0.o3.commit.committed_inst_type.mem_write",
        ),
        (
            "/commit/committed_inst_type/int_mul",
            "sim.cpu0.o3.commit.committed_inst_type.int_mul",
        ),
        (
            "/commit/committed_inst_type/int_div",
            "sim.cpu0.o3.commit.committed_inst_type.int_div",
        ),
        (
            "/commit/committed_inst_type/float_misc",
            "sim.cpu0.o3.commit.committed_inst_type.float_misc",
        ),
        (
            "/commit/committed_inst_type/vector_float_misc",
            "sim.cpu0.o3.commit.committed_inst_type.vector_float_misc",
        ),
    ] {
        let structured = o3_runtime
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
            });
        let stat_value = json_stat_value(&json, stat_path);
        assert_eq!(
            structured, stat_value,
            "structured O3 runtime {pointer} should match stat path {stat_path}"
        );
        assert!(
            structured > 0,
            "representative O3 runtime matrix lane {pointer} should be positive: {o3_runtime}"
        );
    }

    for (pointer, stat_path) in [
        ("/iq/insts_issued", "sim.cpu0.o3.iq.insts_issued"),
        ("/iq/mem_insts_issued", "sim.cpu0.o3.iq.mem_insts_issued"),
        (
            "/iq/branch_insts_issued",
            "sim.cpu0.o3.iq.branch_insts_issued",
        ),
        ("/iew/dispatched_insts", "sim.cpu0.o3.iew.dispatched_insts"),
        ("/iew/insts_to_commit", "sim.cpu0.o3.iew.insts_to_commit"),
        ("/iew/writeback_count", "sim.cpu0.o3.iew.writeback_count"),
        ("/iew/producer_inst", "sim.cpu0.o3.iew.producer_inst"),
        ("/iew/consumer_inst", "sim.cpu0.o3.iew.consumer_inst"),
        (
            "/iew/predicted_taken_incorrect",
            "sim.cpu0.o3.iew.predicted_taken_incorrect",
        ),
        (
            "/iew/predicted_not_taken_incorrect",
            "sim.cpu0.o3.iew.predicted_not_taken_incorrect",
        ),
        (
            "/iew/branch_mispredicts",
            "sim.cpu0.o3.iew.branch_mispredicts",
        ),
        (
            "/commit/branch_mispredicts",
            "sim.cpu0.o3.commit.branch_mispredicts",
        ),
    ] {
        let structured = o3_runtime
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
            });
        assert_eq!(
            structured,
            json_stat_value(&json, stat_path),
            "structured O3 runtime {pointer} should match stat path {stat_path}"
        );
    }
}

#[test]
fn rem6_run_o3_runtime_json_exposes_ordered_atomic_lsq_matrix() {
    let path = detailed_o3_ordered_atomic_lsq_binary(
        "m5-switch-cpu-detailed-o3-ordered-atomic-lsq-runtime-json",
    );

    let direct_json = ordered_atomic_lsq_runtime_json(&path, Some("direct"), "220");
    let cache_json = ordered_atomic_lsq_runtime_json(&path, None, "320");

    assert_eq!(
        direct_json
            .pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_eq!(
        cache_json
            .pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    let direct_latency = assert_ordered_atomic_lsq_runtime_json(&direct_json);
    let cache_latency = assert_ordered_atomic_lsq_runtime_json(&cache_json);
    assert!(
        cache_latency >= direct_latency,
        "cache-backed LSQ latency should include at least the direct latency: direct={direct_latency}, cache={cache_latency}"
    );
}

#[test]
fn rem6_run_o3_runtime_json_exposes_nested_rob_lsq_matrices() {
    let path = detailed_o3_ordered_atomic_lsq_binary(
        "m5-switch-cpu-detailed-o3-nested-rob-lsq-runtime-json",
    );
    let json = ordered_atomic_lsq_runtime_json(&path, Some("direct"), "220");
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (pointer, stat_path) in [
        ("/rob/allocations", "sim.cpu0.o3.rob_allocations"),
        ("/rob/commits", "sim.cpu0.o3.rob_commits"),
        ("/rob/max_occupancy", "sim.cpu0.o3.max_rob_occupancy"),
        ("/rename/writes", "sim.cpu0.o3.rename_writes"),
        ("/rename/map_entries", "sim.cpu0.o3.rename_map_entries"),
        ("/lsq/loads", "sim.cpu0.o3.lsq_loads"),
        ("/lsq/stores", "sim.cpu0.o3.lsq_stores"),
        (
            "/lsq/data_latency/samples",
            "sim.cpu0.o3.lsq_data_latency_samples",
        ),
        (
            "/lsq/data_latency/ticks",
            "sim.cpu0.o3.lsq_data_latency_ticks",
        ),
        (
            "/lsq/data_latency/max_ticks",
            "sim.cpu0.o3.lsq_data_latency_max_ticks",
        ),
        (
            "/lsq/data_latency/min_ticks",
            "sim.cpu0.o3.lsq_data_latency_min_ticks",
        ),
        (
            "/lsq/data_latency/avg_ticks",
            "sim.cpu0.o3.lsq_data_latency_avg_ticks",
        ),
        ("/lsq/max_occupancy", "sim.cpu0.o3.max_lsq_occupancy"),
    ] {
        let structured = o3_runtime
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
            });
        assert_eq!(
            structured,
            json_stat_value(&json, stat_path),
            "nested O3 runtime {pointer} should match stat path {stat_path}"
        );
        assert!(
            structured > 0,
            "representative O3 runtime nested lane {pointer} should be positive: {o3_runtime}"
        );
    }

    for (pointer, stat_path, value) in [
        ("/lsq/load_bytes", "sim.cpu0.o3.lsq_load_bytes", 24),
        ("/lsq/store_bytes", "sim.cpu0.o3.lsq_store_bytes", 40),
        (
            "/lsq/store_conditional_failures",
            "sim.cpu0.o3.lsq_store_conditional_failures",
            0,
        ),
        (
            "/lsq/operation/load/count",
            "sim.cpu0.o3.lsq_operation.load",
            1,
        ),
        (
            "/lsq/operation/store/count",
            "sim.cpu0.o3.lsq_operation.store",
            3,
        ),
        (
            "/lsq/operation/load_reserved/count",
            "sim.cpu0.o3.lsq_operation.load_reserved",
            1,
        ),
        (
            "/lsq/operation/store_conditional/count",
            "sim.cpu0.o3.lsq_operation.store_conditional",
            1,
        ),
        (
            "/lsq/operation/atomic/count",
            "sim.cpu0.o3.lsq_operation.atomic",
            1,
        ),
        (
            "/lsq/operation/vector_load/count",
            "sim.cpu0.o3.lsq_operation.vector_load",
            0,
        ),
        (
            "/lsq/operation/vector_store/count",
            "sim.cpu0.o3.lsq_operation.vector_store",
            0,
        ),
        (
            "/lsq/ordering/acquire",
            "sim.cpu0.o3.lsq_ordering.acquire",
            1,
        ),
        (
            "/lsq/ordering/release",
            "sim.cpu0.o3.lsq_ordering.release",
            1,
        ),
        (
            "/lsq/ordering/acquire_release",
            "sim.cpu0.o3.lsq_ordering.acquire_release",
            1,
        ),
    ] {
        assert_eq!(
            o3_runtime.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {pointer}: {o3_runtime}"
        );
        assert_eq!(
            json_stat_value(&json, stat_path),
            value,
            "stat path {stat_path} should match nested O3 runtime expectation"
        );
    }

    for operation in [
        "load",
        "store",
        "load_reserved",
        "store_conditional",
        "atomic",
    ] {
        for (metric, stat_suffix) in [
            ("samples", "latency_samples"),
            ("ticks", "latency_ticks"),
            ("max_ticks", "latency_max_ticks"),
            ("min_ticks", "latency_min_ticks"),
            ("avg_ticks", "latency_avg_ticks"),
        ] {
            let pointer = format!("/lsq/operation/{operation}/latency/{metric}");
            let stat_path = format!("sim.cpu0.o3.lsq_operation.{operation}_{stat_suffix}");
            let structured = o3_runtime
                .pointer(&pointer)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| {
                    panic!("structured O3 runtime JSON should expose {pointer}: {o3_runtime}")
                });
            assert_eq!(
                structured,
                json_stat_value(&json, &stat_path),
                "nested O3 runtime {pointer} should match stat path {stat_path}"
            );
            assert!(
                structured > 0,
                "active LSQ operation latency metric {pointer} should be positive: {o3_runtime}"
            );
        }
    }
    for operation in ["float_load", "float_store", "vector_load", "vector_store"] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/lsq/operation/{operation}/latency/ticks"))
                .and_then(Value::as_u64),
            Some(0),
            "inactive LSQ operation latency should stay zero for {operation}: {o3_runtime}"
        );
    }

    let forwarding_path =
        detailed_o3_lsq_store_load_match_binary("m5-switch-cpu-o3-nested-lsq-forwarding-json");
    let forwarding_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            forwarding_path.to_str().unwrap(),
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
        forwarding_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&forwarding_output.stderr)
    );
    let forwarding_json: Value = serde_json::from_slice(&forwarding_output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    let forwarding_o3 = forwarding_json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| {
            panic!("run JSON should include core O3 runtime state: {forwarding_json}")
        });
    for (pointer, stat_path) in [
        (
            "/lsq/store_load_forwarding_candidates",
            "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        ),
        (
            "/lsq/store_load_forwarding_matches",
            "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        ),
        (
            "/lsq/operation/load/forwarding_candidates",
            "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        ),
        (
            "/lsq/operation/load/forwarding_matches",
            "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
        ),
    ] {
        assert_eq!(
            forwarding_o3.pointer(pointer).and_then(Value::as_u64),
            Some(1),
            "nested O3 forwarding JSON should expose {pointer}: {forwarding_o3}"
        );
        assert_json_stat(&forwarding_json, stat_path, "Count", 1, "monotonic");
    }
    for pointer in [
        "/lsq/operation/store/forwarding_candidates",
        "/lsq/operation/store/forwarding_matches",
        "/lsq/operation/atomic/forwarding_candidates",
        "/lsq/operation/atomic/forwarding_matches",
    ] {
        assert_eq!(
            forwarding_o3.pointer(pointer).and_then(Value::as_u64),
            Some(0),
            "inactive O3 forwarding lane should stay zero at {pointer}: {forwarding_o3}"
        );
    }
}

fn ordered_atomic_lsq_runtime_json(
    path: &Path,
    memory_system: Option<&str>,
    max_tick: &str,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        max_tick,
        "--stats-format",
        "json",
        "--execute",
        "--dump-memory",
        "0x80000080:16",
        "--dump-memory",
        "0x80000090:16",
    ]);
    if let Some(memory_system) = memory_system {
        command.args(["--memory-system", memory_system]);
    }
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn assert_ordered_atomic_lsq_runtime_json(json: &Value) -> u64 {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("04000000000000000900000000000000")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("00000000000000000300000000000000")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_load", 1),
        ("lsq_operation_store", 3),
        ("lsq_operation_load_reserved", 1),
        ("lsq_operation_store_conditional", 1),
        ("lsq_operation_atomic", 1),
        ("lsq_operation_float_load", 0),
        ("lsq_operation_float_store", 0),
        ("lsq_operation_vector_load", 0),
        ("lsq_operation_vector_store", 0),
        ("lsq_ordering_acquire", 1),
        ("lsq_ordering_release", 1),
        ("lsq_ordering_acquire_release", 1),
        ("lsq_store_conditional_failures", 0),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {field}: {o3_runtime}"
        );
        let stat_path = field
            .strip_prefix("lsq_operation_")
            .map(|operation| format!("sim.cpu0.o3.lsq_operation.{operation}"))
            .or_else(|| {
                field
                    .strip_prefix("lsq_ordering_")
                    .map(|ordering| format!("sim.cpu0.o3.lsq_ordering.{ordering}"))
            })
            .unwrap_or_else(|| format!("sim.cpu0.o3.{field}"));
        assert_eq!(
            json_stat_value(&json, &stat_path),
            value,
            "stat registry should match structured runtime {field}"
        );
        assert_o3_lsq_count_alias(json, field, value);
    }
    assert_o3_lsq_count_alias_totals(json, 7, 3);

    let mut aggregate_latency_samples = 0;
    let mut aggregate_latency_ticks = 0;
    let mut aggregate_latency_max_ticks = 0;
    let mut aggregate_latency_min_ticks = u64::MAX;
    for (operation, alias_operation) in [
        ("load", "load"),
        ("store", "store"),
        ("load_reserved", "loadReserved"),
        ("store_conditional", "storeConditional"),
        ("atomic", "atomic"),
    ] {
        let samples_field = format!("lsq_operation_{operation}_latency_samples");
        let total_field = format!("lsq_operation_{operation}_latency_ticks");
        let max_field = format!("lsq_operation_{operation}_latency_max_ticks");
        let min_field = format!("lsq_operation_{operation}_latency_min_ticks");
        let avg_field = format!("lsq_operation_{operation}_latency_avg_ticks");
        let samples = o3_runtime
            .pointer(&format!("/{samples_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {samples_field}: {o3_runtime}")
            });
        let total = o3_runtime
            .pointer(&format!("/{total_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {total_field}: {o3_runtime}")
            });
        let max = o3_runtime
            .pointer(&format!("/{max_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {max_field}: {o3_runtime}")
            });
        let min = o3_runtime
            .pointer(&format!("/{min_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {min_field}: {o3_runtime}")
            });
        let avg = o3_runtime
            .pointer(&format!("/{avg_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {avg_field}: {o3_runtime}")
            });
        assert!(
            samples > 0,
            "{samples_field} should be positive: {o3_runtime}"
        );
        assert!(total > 0, "{total_field} should be positive: {o3_runtime}");
        assert!(
            max >= min && min > 0,
            "invalid latency bounds for {operation}: {o3_runtime}"
        );
        assert!(
            avg >= min && avg <= max,
            "average latency should stay within bounds for {operation}: {o3_runtime}"
        );
        assert_eq!(
            avg,
            total / samples,
            "average latency should use the structured sample count for {operation}: {o3_runtime}"
        );
        aggregate_latency_samples += samples;
        aggregate_latency_ticks += total;
        aggregate_latency_max_ticks = aggregate_latency_max_ticks.max(max);
        aggregate_latency_min_ticks = aggregate_latency_min_ticks.min(min);
        for (suffix, value) in [
            ("latency_samples", samples),
            ("latency_ticks", total),
            ("latency_max_ticks", max),
            ("latency_min_ticks", min),
            ("latency_avg_ticks", avg),
        ] {
            assert_eq!(
                json_stat_value(
                    &json,
                    &format!("sim.cpu0.o3.lsq_operation.{operation}_{suffix}")
                ),
                value,
                "stat registry should match structured runtime {operation}_{suffix}"
            );
        }
        for (suffix, unit, value) in [
            ("samples", "Count", samples),
            ("totalLatency", "Tick", total),
            ("maxLatency", "Tick", max),
            ("minLatency", "Tick", min),
            ("avgLatency", "Tick", avg),
        ] {
            assert_json_stat(
                json,
                &format!("system.cpu.lsq0.dataResponse.{alias_operation}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
    let aggregate_latency_avg_ticks = aggregate_latency_ticks / aggregate_latency_samples;
    for (field, alias, unit, value) in [
        (
            "lsq_data_latency_samples",
            "samples",
            "Count",
            aggregate_latency_samples,
        ),
        (
            "lsq_data_latency_ticks",
            "totalLatency",
            "Tick",
            aggregate_latency_ticks,
        ),
        (
            "lsq_data_latency_max_ticks",
            "maxLatency",
            "Tick",
            aggregate_latency_max_ticks,
        ),
        (
            "lsq_data_latency_min_ticks",
            "minLatency",
            "Tick",
            aggregate_latency_min_ticks,
        ),
        (
            "lsq_data_latency_avg_ticks",
            "avgLatency",
            "Tick",
            aggregate_latency_avg_ticks,
        ),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose aggregate {field}: {o3_runtime}"
        );
        assert_json_stat(
            &json,
            &format!("sim.cpu0.o3.{field}"),
            unit,
            value,
            "monotonic",
        );
        assert_json_stat(
            json,
            &format!("system.cpu.lsq0.dataResponse.{alias}"),
            unit,
            value,
            "monotonic",
        );
    }
    for (operation, alias_operation) in [
        ("float_load", "floatLoad"),
        ("float_store", "floatStore"),
        ("vector_load", "vectorLoad"),
        ("vector_store", "vectorStore"),
    ] {
        for (field, alias, unit) in [
            ("latency_samples", "samples", "Count"),
            ("latency_ticks", "totalLatency", "Tick"),
            ("latency_max_ticks", "maxLatency", "Tick"),
            ("latency_min_ticks", "minLatency", "Tick"),
            ("latency_avg_ticks", "avgLatency", "Tick"),
        ] {
            let runtime_field = format!("lsq_operation_{operation}_{field}");
            assert_eq!(
                o3_runtime
                    .pointer(&format!("/{runtime_field}"))
                    .and_then(Value::as_u64),
                Some(0),
                "structured O3 runtime JSON should expose inactive {runtime_field}: {o3_runtime}"
            );
            assert_eq!(
                json_stat_value(
                    &json,
                    &format!("sim.cpu0.o3.lsq_operation.{operation}_{field}")
                ),
                0,
                "stat registry should expose inactive {operation}_{field}"
            );
            assert_json_stat(
                json,
                &format!("system.cpu.lsq0.dataResponse.{alias_operation}.{alias}"),
                unit,
                0,
                "monotonic",
            );
        }
    }
    [
        "load",
        "store",
        "load_reserved",
        "store_conditional",
        "atomic",
    ]
    .into_iter()
    .map(|operation| {
        o3_runtime
            .pointer(&format!("/lsq_operation_{operation}_latency_ticks"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing latency total for {operation}: {o3_runtime}"))
    })
    .sum()
}

#[test]
fn rem6_run_o3_runtime_json_counts_store_conditional_failures() {
    let path = detailed_o3_store_conditional_failure_binary(
        "m5-switch-cpu-detailed-o3-store-conditional-failure-runtime-json",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000040:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("88776655443322110100000000000000")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_store", 1),
        ("lsq_operation_store_conditional", 1),
        ("lsq_ordering_acquire", 0),
        ("lsq_ordering_release", 0),
        ("lsq_ordering_acquire_release", 0),
        ("lsq_store_conditional_failures", 1),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {field}: {o3_runtime}"
        );
        let stat_path = field
            .strip_prefix("lsq_operation_")
            .map(|operation| format!("sim.cpu0.o3.lsq_operation.{operation}"))
            .or_else(|| {
                field
                    .strip_prefix("lsq_ordering_")
                    .map(|ordering| format!("sim.cpu0.o3.lsq_ordering.{ordering}"))
            })
            .unwrap_or_else(|| format!("sim.cpu0.o3.{field}"));
        assert_eq!(
            json_stat_value(&json, &stat_path),
            value,
            "stat registry should match structured runtime {field}"
        );
        assert_o3_lsq_count_alias(&json, field, value);
    }
    assert_o3_lsq_count_alias_totals(&json, 2, 0);
}

#[test]
fn rem6_run_does_not_record_o3_store_conditional_failure_after_functional_run() {
    let path = functional_store_conditional_failure_binary(
        "functional-store-conditional-failure-omits-o3-runtime-json",
    );

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
            "--dump-memory",
            "0x80000040:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("88776655443322110100000000000000")
    );
    assert!(
        json.pointer("/cores/0/o3_runtime").is_none(),
        "functional failed SC run should not emit O3 runtime state: {json}"
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_conditional_failures");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_operation.store_conditional");
}

#[test]
fn rem6_run_o3_runtime_json_exposes_float_vector_lsq_matrix() {
    let path = detailed_o3_float_vector_lsq_binary(
        "m5-switch-cpu-detailed-o3-float-vector-lsq-runtime-json",
    );

    let direct_json = float_vector_lsq_runtime_json(&path, Some("direct"), "220");
    let cache_json = float_vector_lsq_runtime_json(&path, None, "320");

    assert_eq!(
        direct_json
            .pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_eq!(
        cache_json
            .pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    let direct_latency = assert_float_vector_lsq_runtime_json(&direct_json);
    let cache_latency = assert_float_vector_lsq_runtime_json(&cache_json);
    assert!(
        cache_latency >= direct_latency,
        "cache-backed float/vector LSQ latency should include at least the direct latency: direct={direct_latency}, cache={cache_latency}"
    );
}

fn float_vector_lsq_runtime_json(
    path: &Path,
    memory_system: Option<&str>,
    max_tick: &str,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        max_tick,
        "--stats-format",
        "json",
        "--execute",
        "--dump-memory",
        "0x80000080:16",
        "--dump-memory",
        "0x80000090:16",
    ]);
    if let Some(memory_system) = memory_system {
        command.args(["--memory-system", memory_system]);
    }
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn assert_float_vector_lsq_runtime_json(json: &Value) -> u64 {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("000000000000f03f000000000000f03f")
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some("44332211887766554433221188776655")
    );
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

    for (field, value) in [
        ("lsq_operation_load", 0),
        ("lsq_operation_store", 0),
        ("lsq_operation_load_reserved", 0),
        ("lsq_operation_store_conditional", 0),
        ("lsq_operation_atomic", 0),
        ("lsq_operation_float_load", 1),
        ("lsq_operation_float_store", 1),
        ("lsq_operation_vector_load", 1),
        ("lsq_operation_vector_store", 1),
        ("lsq_ordering_acquire", 0),
        ("lsq_ordering_release", 0),
        ("lsq_ordering_acquire_release", 0),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose {field}: {o3_runtime}"
        );
        let stat_path = field
            .strip_prefix("lsq_operation_")
            .map(|operation| format!("sim.cpu0.o3.lsq_operation.{operation}"))
            .or_else(|| {
                field
                    .strip_prefix("lsq_ordering_")
                    .map(|ordering| format!("sim.cpu0.o3.lsq_ordering.{ordering}"))
            })
            .unwrap_or_else(|| format!("sim.cpu0.o3.{field}"));
        assert_eq!(
            json_stat_value(&json, &stat_path),
            value,
            "stat registry should match structured runtime {field}"
        );
        assert_o3_lsq_count_alias(&json, field, value);
    }
    assert_o3_lsq_count_alias_totals(&json, 4, 0);

    let mut aggregate_latency_samples = 0;
    let mut aggregate_latency_ticks = 0;
    let mut aggregate_latency_max_ticks = 0;
    let mut aggregate_latency_min_ticks = u64::MAX;
    for (operation, alias_operation) in [
        ("float_load", "floatLoad"),
        ("float_store", "floatStore"),
        ("vector_load", "vectorLoad"),
        ("vector_store", "vectorStore"),
    ] {
        let samples_field = format!("lsq_operation_{operation}_latency_samples");
        let total_field = format!("lsq_operation_{operation}_latency_ticks");
        let max_field = format!("lsq_operation_{operation}_latency_max_ticks");
        let min_field = format!("lsq_operation_{operation}_latency_min_ticks");
        let avg_field = format!("lsq_operation_{operation}_latency_avg_ticks");
        let samples = o3_runtime
            .pointer(&format!("/{samples_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {samples_field}: {o3_runtime}")
            });
        let total = o3_runtime
            .pointer(&format!("/{total_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {total_field}: {o3_runtime}")
            });
        let max = o3_runtime
            .pointer(&format!("/{max_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {max_field}: {o3_runtime}")
            });
        let min = o3_runtime
            .pointer(&format!("/{min_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {min_field}: {o3_runtime}")
            });
        let avg = o3_runtime
            .pointer(&format!("/{avg_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("structured O3 runtime JSON should expose {avg_field}: {o3_runtime}")
            });
        assert!(
            samples > 0,
            "{samples_field} should be positive: {o3_runtime}"
        );
        assert!(total > 0, "{total_field} should be positive: {o3_runtime}");
        assert!(
            max >= min && min > 0,
            "invalid latency bounds for {operation}: {o3_runtime}"
        );
        assert!(
            avg >= min && avg <= max,
            "average latency should stay within bounds for {operation}: {o3_runtime}"
        );
        assert_eq!(
            avg,
            total / samples,
            "average latency should use the structured sample count for {operation}: {o3_runtime}"
        );
        aggregate_latency_samples += samples;
        aggregate_latency_ticks += total;
        aggregate_latency_max_ticks = aggregate_latency_max_ticks.max(max);
        aggregate_latency_min_ticks = aggregate_latency_min_ticks.min(min);
        for (suffix, value) in [
            ("latency_samples", samples),
            ("latency_ticks", total),
            ("latency_max_ticks", max),
            ("latency_min_ticks", min),
            ("latency_avg_ticks", avg),
        ] {
            assert_eq!(
                json_stat_value(
                    &json,
                    &format!("sim.cpu0.o3.lsq_operation.{operation}_{suffix}")
                ),
                value,
                "stat registry should match structured runtime {operation}_{suffix}"
            );
        }
        for (suffix, unit, value) in [
            ("samples", "Count", samples),
            ("totalLatency", "Tick", total),
            ("maxLatency", "Tick", max),
            ("minLatency", "Tick", min),
            ("avgLatency", "Tick", avg),
        ] {
            assert_json_stat(
                json,
                &format!("system.cpu.lsq0.dataResponse.{alias_operation}.{suffix}"),
                unit,
                value,
                "monotonic",
            );
        }
    }
    let aggregate_latency_avg_ticks = aggregate_latency_ticks / aggregate_latency_samples;
    for (field, alias, unit, value) in [
        (
            "lsq_data_latency_samples",
            "samples",
            "Count",
            aggregate_latency_samples,
        ),
        (
            "lsq_data_latency_ticks",
            "totalLatency",
            "Tick",
            aggregate_latency_ticks,
        ),
        (
            "lsq_data_latency_max_ticks",
            "maxLatency",
            "Tick",
            aggregate_latency_max_ticks,
        ),
        (
            "lsq_data_latency_min_ticks",
            "minLatency",
            "Tick",
            aggregate_latency_min_ticks,
        ),
        (
            "lsq_data_latency_avg_ticks",
            "avgLatency",
            "Tick",
            aggregate_latency_avg_ticks,
        ),
    ] {
        assert_eq!(
            o3_runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose aggregate {field}: {o3_runtime}"
        );
        assert_json_stat(
            &json,
            &format!("sim.cpu0.o3.{field}"),
            unit,
            value,
            "monotonic",
        );
        assert_json_stat(
            json,
            &format!("system.cpu.lsq0.dataResponse.{alias}"),
            unit,
            value,
            "monotonic",
        );
    }
    for (operation, alias_operation) in [
        ("load", "load"),
        ("store", "store"),
        ("load_reserved", "loadReserved"),
        ("store_conditional", "storeConditional"),
        ("atomic", "atomic"),
    ] {
        for (field, alias, unit) in [
            ("latency_samples", "samples", "Count"),
            ("latency_ticks", "totalLatency", "Tick"),
            ("latency_max_ticks", "maxLatency", "Tick"),
            ("latency_min_ticks", "minLatency", "Tick"),
            ("latency_avg_ticks", "avgLatency", "Tick"),
        ] {
            let runtime_field = format!("lsq_operation_{operation}_{field}");
            assert_eq!(
                o3_runtime
                    .pointer(&format!("/{runtime_field}"))
                    .and_then(Value::as_u64),
                Some(0),
                "structured O3 runtime JSON should expose inactive {runtime_field}: {o3_runtime}"
            );
            assert_eq!(
                json_stat_value(
                    &json,
                    &format!("sim.cpu0.o3.lsq_operation.{operation}_{field}")
                ),
                0,
                "stat registry should expose inactive {operation}_{field}"
            );
            assert_json_stat(
                json,
                &format!("system.cpu.lsq0.dataResponse.{alias_operation}.{alias}"),
                unit,
                0,
                "monotonic",
            );
        }
    }
    ["float_load", "float_store", "vector_load", "vector_store"]
        .into_iter()
        .map(|operation| {
            o3_runtime
                .pointer(&format!("/lsq_operation_{operation}_latency_ticks"))
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("missing latency total for {operation}: {o3_runtime}"))
        })
        .sum()
}

#[test]
fn rem6_run_checkpoints_o3_runtime_state_after_detailed_execution() {
    let path = detailed_o3_checkpoint_state_binary("m5-switch-cpu-detailed-o3-checkpoint-state");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let baseline = checkpoint_chunk_checksum(host_actions, 0, "cpu0", "o3-runtime-state");
    let after_detailed = checkpoint_chunk_checksum(host_actions, 1, "cpu0", "o3-runtime-state");
    assert_ne!(
        after_detailed, baseline,
        "detailed O3 runtime checkpoint chunk should change after retired rename and LSQ work"
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.rename_map_entries",
        "Count",
        3,
        "monotonic",
    );
}

#[test]
fn rem6_run_restores_scheduled_o3_checkpoint_and_replays_detailed_work() {
    let path = detailed_o3_scheduled_restore_binary("m5-switch-cpu-detailed-o3-scheduled-restore");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-checkpoint",
            "8:o3-baseline",
            "--host-checkpoint",
            "50:o3-mutated",
            "--host-restore-checkpoint",
            "70:o3-baseline",
            "--host-checkpoint",
            "113:o3-replayed",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let restored_checkpoint = host_actions
        .pointer("/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing restored checkpoint detail: {host_actions}"));
    assert_eq!(
        restored_checkpoint
            .pointer("/label")
            .and_then(Value::as_str),
        Some("o3-baseline"),
        "restored checkpoint detail should identify the restored manifest: {restored_checkpoint}"
    );
    assert_eq!(
        restored_checkpoint
            .pointer("/execution_mode_authority_present")
            .and_then(Value::as_bool),
        Some(true),
        "restored detailed checkpoint should report decoded execution-mode authority: {restored_checkpoint}"
    );
    assert_eq!(
        restored_checkpoint
            .pointer("/execution_mode_authority_cleared")
            .and_then(Value::as_bool),
        Some(false),
        "restored detailed checkpoint should not report absent-authority rollback: {restored_checkpoint}"
    );
    assert_eq!(
        restored_checkpoint
            .pointer("/execution_mode_authority_decode_error")
            .and_then(Value::as_bool),
        Some(false),
        "restored detailed checkpoint should decode execution-mode authority cleanly: {restored_checkpoint}"
    );
    let restored_execution_modes = restored_checkpoint
        .pointer("/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| {
            panic!(
                "restored detailed checkpoint should expose decoded modes: {restored_checkpoint}"
            )
        });
    assert_eq!(
        restored_execution_modes.len(),
        1,
        "restored detailed checkpoint should decode one target authority: {restored_execution_modes:?}"
    );
    assert_eq!(
        restored_execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        restored_execution_modes[0]
            .pointer("/mode")
            .and_then(Value::as_str),
        Some("detailed")
    );
    let restored_execution_mode_component = restored_checkpoint
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str)
                    == Some("host.execution_modes")
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "restored detailed checkpoint should expose host.execution_modes: {restored_checkpoint}"
            )
        });
    assert_eq!(
        restored_execution_mode_component
            .pointer("/chunk_count")
            .and_then(Value::as_u64),
        Some(1),
        "restored execution-mode component should contain the modes chunk: {restored_execution_mode_component}"
    );
    assert!(
        restored_execution_mode_component
            .pointer("/chunks")
            .and_then(Value::as_array)
            .is_some_and(|chunks| chunks
                .iter()
                .any(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("modes"))),
        "restored execution-mode component should expose the modes chunk: {restored_execution_mode_component}"
    );
    assert_checkpoint(host_actions, 0, "o3-baseline", 9, 9);
    assert_checkpoint(host_actions, 1, "o3-mutated", 51, 51);
    assert_checkpoint(host_actions, 2, "o3-replayed", 114, 114);
    let restored_components = host_actions
        .pointer("/checkpoint_restored_component_count")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint components");
    let restored_chunks = host_actions
        .pointer("/checkpoint_restored_chunk_count")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint chunks");
    let restored_payload_bytes = host_actions
        .pointer("/checkpoint_restored_payload_bytes")
        .and_then(Value::as_u64)
        .expect("scheduled restore should report restored checkpoint payload bytes");
    assert_eq!(
        restored_components,
        host_actions
            .pointer("/checkpoints/0/component_count")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest component count should match the restored baseline checkpoint"
    );
    assert_eq!(
        restored_chunks,
        host_actions
            .pointer("/checkpoints/0/chunk_count")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest chunk count should match the restored baseline checkpoint"
    );
    assert_eq!(
        restored_payload_bytes,
        host_actions
            .pointer("/checkpoints/0/payload_bytes")
            .and_then(Value::as_u64)
            .unwrap(),
        "restored manifest payload bytes should match the restored baseline checkpoint"
    );

    let baseline = checkpoint_chunk_checksum(host_actions, 0, "cpu0", "o3-runtime-state");
    let mutated = checkpoint_chunk_checksum(host_actions, 1, "cpu0", "o3-runtime-state");
    let replayed = checkpoint_chunk_checksum(host_actions, 2, "cpu0", "o3-runtime-state");
    assert_ne!(
        mutated, baseline,
        "detailed O3 runtime state should change after ROB/LSQ/rename/FU work"
    );
    assert_eq!(
        replayed, mutated,
        "restoring the earlier O3 checkpoint should replay deterministic detailed work"
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoints",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restores",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_components",
        "Count",
        restored_components,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_chunks",
        "Count",
        restored_chunks,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restored_payload_bytes",
        "Byte",
        restored_payload_bytes,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.manifests",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.cleared_manifests",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.host_actions.checkpoint_restore.execution_mode_authority.targets",
        "Count",
        1,
        "monotonic",
    );
    for (mode, expected) in [("functional", 0), ("timing", 0), ("detailed", 1)] {
        assert_json_stat(
            &json,
            &format!("sim.host_actions.checkpoint_restore.execution_mode_authority.mode.{mode}"),
            "Count",
            expected,
            "monotonic",
        );
    }
    for (mode, expected) in [("functional", 0), ("timing", 0), ("detailed", 1)] {
        assert_json_stat(
            &json,
            &format!(
                "sim.host_actions.checkpoint_restore.execution_mode_authority.target.cpu0.mode.{mode}"
            ),
            "Count",
            expected,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_m5_dump_stats_resets_o3_snapshot_after_scheduled_restore() {
    let path = detailed_o3_restore_dump_stats_binary("m5-switch-cpu-o3-restore-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "500",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));

    for (path, unit) in [
        ("sim.host_actions.stats_dump.cpu0.o3.instructions", "Count"),
        (
            "sim.host_actions.stats_dump.cpu0.o3.rob_allocations",
            "Count",
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.lsq_store_bytes",
            "Byte",
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.rename_map_entries",
            "Count",
        ),
    ] {
        assert_stats_dump_sample(
            restored_dump,
            path,
            "counter",
            unit,
            stats_dump_sample_value(first_dump, path),
            "resettable",
        );
    }
    assert!(
        json_stat_value(&json, "sim.cpu0.o3.instructions")
            > stats_dump_sample_value(
                first_dump,
                "sim.host_actions.stats_dump.cpu0.o3.instructions"
            )
    );
    assert_json_stat(&json, "sim.cpu0.o3.lsq_store_bytes", "Byte", 4, "monotonic");
}

#[test]
fn rem6_run_m5_dump_stats_restores_o3_fu_class_snapshot_after_scheduled_restore() {
    let path = detailed_o3_restore_fu_dump_stats_binary("m5-switch-cpu-o3-restore-fu-dump-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "1000",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "150:gem5-m5-checkpoint",
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
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let first_dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing first stats dump: {host_actions}"));
    let restored_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing restored stats dump: {host_actions}"));
    let restore_tick = host_actions
        .pointer("/checkpoint_restores/0/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing checkpoint restore tick: {host_actions}"));
    let first_dump_tick = first_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing first dump tick: {first_dump}"));
    let restored_dump_tick = restored_dump
        .pointer("/tick")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing restored dump tick: {restored_dump}"));
    assert!(
        first_dump_tick < restore_tick && restore_tick < restored_dump_tick,
        "expected first dump before restore before restored dump, first={first_dump_tick}, restore={restore_tick}, restored={restored_dump_tick}"
    );
    for dump in [first_dump, restored_dump] {
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
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_mul_latency_cycles",
            "counter",
            "Cycle",
            2,
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_integer_div_instructions",
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
            "sim.host_actions.stats_dump.cpu0.o3.fu_float_misc_latency_cycles",
            "counter",
            "Cycle",
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
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.fu_vector_float_misc_latency_cycles",
            "counter",
            "Cycle",
            0,
            "resettable",
        );
    }
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_instructions",
        "Count",
        6,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.fu_latency_cycles",
        "Cycle",
        27,
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
fn rem6_run_reports_scheduled_restore_missing_checkpoint_label() {
    let path = scheduled_host_restore_missing_label_binary("scheduled-restore-missing-label");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-restore-checkpoint",
            "8:missing-label",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "run should fail when a scheduled restore references a missing checkpoint label"
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("checkpoint manifest missing-label is not available"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_text_stats_alias_o3_runtime_stats_after_detailed_switch() {
    let path = detailed_o3_runtime_stats_binary("m5-switch-cpu-detailed-o3-runtime-text-stats");

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
            "text",
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
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_text_count_stat(&stdout, "sim.cpu0.o3.instructions", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rob_allocations", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rob_commits", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.rename_writes", 4);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_loads", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_stores", 1);
    assert_text_byte_stat(&stdout, "sim.cpu0.o3.lsq_load_bytes", 4);
    assert_text_byte_stat(&stdout, "sim.cpu0.o3.lsq_store_bytes", 4);
    assert_text_count_stat(&stdout, "system.cpu.rename.renamedInsts", 6);
    assert_text_count_stat(&stdout, "system.cpu.rename.renamedOperands", 4);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispatchedInsts", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispLoadInsts", 1);
    assert_text_count_stat(&stdout, "system.cpu.iew.dispStoreInsts", 1);
    assert_text_count_stat(&stdout, "system.cpu.iew.instsToCommit::total", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.writebackCount::total", 6);
    assert_text_count_stat(&stdout, "system.cpu.iew.producerInst::total", 3);
    assert_text_count_stat(&stdout, "system.cpu.iew.consumerInst::total", 4);
    let writeback_count = text_stat_u64(&stdout, "system.cpu.iew.writebackCount::total");
    let cycles = text_stat_u64(&stdout, "system.cpu.numCycles");
    assert_text_ratio_stat(
        &stdout,
        "system.cpu.iew.wbRate",
        &fixed_ratio_text(writeback_count, cycles),
        "(Count/Cycle)",
    );
    assert_text_stat_occurs_once(&stdout, "system.cpu.iew.wbRate");
    let producer_insts = text_stat_u64(&stdout, "system.cpu.iew.producerInst::total");
    let consumer_insts = text_stat_u64(&stdout, "system.cpu.iew.consumerInst::total");
    assert_text_ratio_stat(
        &stdout,
        "system.cpu.iew.wbFanout",
        &fixed_ratio_text(producer_insts, consumer_insts),
        "(Count/Count)",
    );
    assert_text_stat_occurs_once(&stdout, "system.cpu.iew.wbFanout");
    assert_text_count_stat(&stdout, "system.cpu.lsq0.addedLoadsAndStores", 2);
    assert_text_count_stat(&stdout, "system.cpu.lsq0.maxOccupancy", 1);
    assert_text_byte_stat(&stdout, "system.cpu.lsq0.loadBytes", 4);
    assert_text_byte_stat(&stdout, "system.cpu.lsq0.storeBytes", 4);
    let forwarding_matches =
        text_stat_u64(&stdout, "sim.cpu0.o3.lsq_store_to_load_forwarding_matches");
    assert_text_count_stat(&stdout, "system.cpu.lsq0.forwLoads", forwarding_matches);
    assert_text_count_stat(&stdout, "system.cpu.iq.instsIssued", 6);
    assert_text_count_stat(&stdout, "system.cpu.iq.memInstsIssued", 2);
    assert_text_stat_occurs_once(&stdout, "system.cpu.lsq0.forwLoads");
    assert_text_stat_occurs_once(&stdout, "system.cpu.iq.instsIssued");
    assert_text_stat_occurs_once(&stdout, "system.cpu.iq.memInstsIssued");
    assert_text_stat_occurs_once(&stdout, "system.cpu.iq.branchInstsIssued");
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::MemRead", 1);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::MemWrite", 1);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.commit.committed_inst_type.mem_read",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.commit.committed_inst_type.mem_write",
        1,
    );
    assert_text_count_stat(&stdout, "system.cpu.commit.committedInstType_0::MemRead", 1);
    assert_text_count_stat(
        &stdout,
        "system.cpu.commit.committedInstType_0::MemWrite",
        1,
    );
    assert_text_count_stat(&stdout, "system.cpu.rob.writes", 6);
    assert_text_count_stat(&stdout, "system.cpu.rob.reads", 6);
    assert_text_count_stat(&stdout, "system.cpu.rob.maxOccupancy", 1);
}

#[test]
fn rem6_run_text_stats_alias_o3_branch_mispredicts_after_detailed_switch() {
    let path = detailed_o3_branch_repair_text_stats_binary(
        "m5-switch-cpu-detailed-o3-branch-mispredict-text-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
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
    let predicted_taken_incorrect = 1;
    let predicted_not_taken_incorrect = 2;
    let branch_mispredicts = predicted_taken_incorrect + predicted_not_taken_incorrect;

    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.branch_repair_targetless_mismatches",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.branch_repair_direction_only_mismatches",
        2,
    );
    assert_text_count_stat(&stdout, "sim.cpu0.o3.branch_repair_wrong_targets", 0);
    for (path, value) in [
        ("system.cpu.iew.branchRepair_0::TargetlessMismatch", 1),
        ("system.cpu.iew.branchRepair_0::DirectionOnly", 2),
        ("system.cpu.iew.branchRepair_0::WrongTarget", 0),
        ("system.cpu.iew.branchRepair_0::total", branch_mispredicts),
    ] {
        assert_text_count_stat(&stdout, path, value);
        assert_text_stat_occurs_once(&stdout, path);
    }
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.iew.predicted_taken_incorrect",
        predicted_taken_incorrect,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.iew.predicted_not_taken_incorrect",
        predicted_not_taken_incorrect,
    );
    assert_text_count_stat(&stdout, "sim.cpu0.o3.iq.branch_insts_issued", 3);
    assert_text_count_stat(&stdout, "system.cpu.iq.branchInstsIssued", 3);
    assert_text_count_stat(
        &stdout,
        "system.cpu.iew.predictedTakenIncorrect",
        predicted_taken_incorrect,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.iew.predictedNotTakenIncorrect",
        predicted_not_taken_incorrect,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.iew.branchMispredicts",
        branch_mispredicts,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.commit.branchMispredicts",
        branch_mispredicts,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.branch_event.squashes",
        branch_mispredicts,
    );
    assert_text_count_stat(&stdout, "system.cpu.ftq.squashes", branch_mispredicts);
    assert_text_count_stat(
        &stdout,
        "system.cpu.ftq.squashedTargets",
        branch_mispredicts,
    );
    assert_text_count_stat(&stdout, "system.cpu.ftq.squashedTargetsWithLinkWrites", 0);
    assert_text_count_stat(
        &stdout,
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites",
        branch_mispredicts,
    );
    assert_text_stat_occurs_once(&stdout, "system.cpu.iew.predictedTakenIncorrect");
    assert_text_stat_occurs_once(&stdout, "system.cpu.iew.predictedNotTakenIncorrect");
    assert_text_stat_occurs_once(&stdout, "system.cpu.iew.branchMispredicts");
    assert_text_stat_occurs_once(&stdout, "system.cpu.commit.branchMispredicts");
    assert_text_stat_occurs_once(&stdout, "system.cpu.ftq.squashes");
    assert_text_stat_occurs_once(&stdout, "system.cpu.ftq.squashedTargets");
    assert_text_stat_occurs_once(&stdout, "system.cpu.ftq.squashedTargetsWithLinkWrites");
    assert_text_stat_occurs_once(&stdout, "system.cpu.ftq.squashedTargetsWithoutLinkWrites");
}

#[test]
fn rem6_run_json_stats_alias_o3_branch_mispredicts_after_detailed_switch() {
    let path = detailed_o3_branch_repair_text_stats_binary(
        "m5-switch-cpu-detailed-o3-branch-mispredict-json-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
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
    let predicted_taken_incorrect = 1;
    let predicted_not_taken_incorrect = 2;
    let branch_mispredicts = predicted_taken_incorrect + predicted_not_taken_incorrect;
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/iew_predicted_taken_incorrect")
            .and_then(Value::as_u64),
        Some(predicted_taken_incorrect),
        "structured O3 runtime JSON should expose predicted-taken split: {json}"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/iew_predicted_not_taken_incorrect")
            .and_then(Value::as_u64),
        Some(predicted_not_taken_incorrect),
        "structured O3 runtime JSON should expose predicted-not-taken split: {json}"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/iq_branch_insts_issued")
            .and_then(Value::as_u64),
        Some(3),
        "structured O3 runtime JSON should expose branch-issued count: {json}"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/iq/branch_insts_issued")
            .and_then(Value::as_u64),
        Some(3),
        "nested O3 IQ JSON should expose positive branch-issued count: {json}"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/iew/predicted_taken_incorrect")
            .and_then(Value::as_u64),
        Some(predicted_taken_incorrect),
        "nested O3 IEW JSON should expose positive predicted-taken split: {json}"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/iew/predicted_not_taken_incorrect")
            .and_then(Value::as_u64),
        Some(predicted_not_taken_incorrect),
        "nested O3 IEW JSON should expose positive predicted-not-taken split: {json}"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/iew/branch_mispredicts")
            .and_then(Value::as_u64),
        Some(branch_mispredicts),
        "nested O3 IEW JSON should expose positive branch mispredicts: {json}"
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/commit/branch_mispredicts")
            .and_then(Value::as_u64),
        Some(branch_mispredicts),
        "nested O3 commit JSON should expose positive branch mispredicts: {json}"
    );

    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_repair_targetless_mismatches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_repair_direction_only_mismatches",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_repair_wrong_targets",
        "Count",
        0,
        "monotonic",
    );
    for (path, value) in [
        ("system.cpu.iew.branchRepair.targetlessMismatch", 1),
        ("system.cpu.iew.branchRepair_0::TargetlessMismatch", 1),
        ("system.cpu.iew.branchRepair.directionOnly", 2),
        ("system.cpu.iew.branchRepair_0::DirectionOnly", 2),
        ("system.cpu.iew.branchRepair.wrongTarget", 0),
        ("system.cpu.iew.branchRepair_0::WrongTarget", 0),
        ("system.cpu.iew.branchRepair.total", branch_mispredicts),
        ("system.cpu.iew.branchRepair_0::total", branch_mispredicts),
    ] {
        assert_json_stat(&json, path, "Count", value, "monotonic");
    }
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.predicted_taken_incorrect",
        "Count",
        predicted_taken_incorrect,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.predicted_not_taken_incorrect",
        "Count",
        predicted_not_taken_incorrect,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iew.branch_mispredicts",
        "Count",
        branch_mispredicts,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.commit.branch_mispredicts",
        "Count",
        branch_mispredicts,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.iq.branch_insts_issued",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iq.branchInstsIssued",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.predictedTakenIncorrect",
        "Count",
        predicted_taken_incorrect,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.predictedNotTakenIncorrect",
        "Count",
        predicted_not_taken_incorrect,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.branchMispredicts",
        "Count",
        branch_mispredicts,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.commit.branchMispredicts",
        "Count",
        branch_mispredicts,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.squashes",
        "Count",
        branch_mispredicts,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.ftq.squashes",
        "Count",
        branch_mispredicts,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.ftq.squashedTargets",
        "Count",
        branch_mispredicts,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.ftq.squashedTargetsWithLinkWrites",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites",
        "Count",
        branch_mispredicts,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_runtime_json_exposes_return_branch_event_matrix() {
    let path = detailed_o3_return_branch_summary_binary("m5-switch-cpu-o3-return-branch-summary");

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
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
            "--dump-memory",
            "0x80000040:16",
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
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("1c000080000000000000000000000000")
    );

    for (pointer, value) in [
        ("/cores/0/o3_runtime/branch_event/branches", 1),
        ("/cores/0/o3_runtime/branch_event/taken", 1),
        ("/cores/0/o3_runtime/branch_event/not_taken", 0),
        ("/cores/0/o3_runtime/branch_event/resolved_targets", 1),
        ("/cores/0/o3_runtime/branch_event/kind/return", 1),
        ("/cores/0/o3_runtime/branch_event/taken_kind/return", 1),
        (
            "/cores/0/o3_runtime/branch_event/resolved_target_kind/return",
            1,
        ),
        ("/cores/0/o3_runtime/branch_event/link_writes", 0),
        ("/cores/0/o3_runtime/branch_event/without_link_writes", 1),
        ("/cores/0/o3_runtime/branch_event/link_write_kind/return", 0),
        ("/cores/0/o3_runtime/branch_event/squashes", 1),
        ("/cores/0/o3_runtime/branch_event/squashed_targets", 1),
        (
            "/cores/0/o3_runtime/branch_event/squashed_targets_with_link_writes",
            0,
        ),
        (
            "/cores/0/o3_runtime/branch_event/squashed_targets_without_link_writes",
            1,
        ),
        ("/cores/0/o3_runtime/branch_event/squash_kind/return", 1),
        (
            "/cores/0/o3_runtime/branch_event/squashed_target_link_write_kind/return",
            0,
        ),
        (
            "/cores/0/o3_runtime/branch_event/squashed_target_without_link_write_kind/return",
            1,
        ),
        (
            "/cores/0/o3_runtime/branch_repair/direction_only_kind/return",
            1,
        ),
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(value),
            "structured O3 runtime JSON should expose return branch event bucket {pointer}: {json}"
        );
    }

    for (path, value) in [
        ("sim.cpu0.o3.branch_event.branches", 1),
        ("sim.cpu0.o3.branch_event.taken", 1),
        ("sim.cpu0.o3.branch_event.not_taken", 0),
        ("sim.cpu0.o3.branch_event.resolved_targets", 1),
        ("sim.cpu0.o3.branch_event.kind.return", 1),
        ("sim.cpu0.o3.branch_event.taken_kind.return", 1),
        ("sim.cpu0.o3.branch_event.resolved_target_kind.return", 1),
        ("sim.cpu0.o3.branch_event.link_writes", 0),
        ("sim.cpu0.o3.branch_event.without_link_writes", 1),
        ("sim.cpu0.o3.branch_event.link_write_kind.return", 0),
        ("sim.cpu0.o3.branch_event.squashes", 1),
        ("sim.cpu0.o3.branch_event.squashed_targets", 1),
        (
            "sim.cpu0.o3.branch_event.squashed_targets_with_link_writes",
            0,
        ),
        (
            "sim.cpu0.o3.branch_event.squashed_targets_without_link_writes",
            1,
        ),
        ("sim.cpu0.o3.branch_event.squash_kind.return", 1),
        (
            "sim.cpu0.o3.branch_event.squashed_target_link_write_kind.return",
            0,
        ),
        (
            "sim.cpu0.o3.branch_event.squashed_target_without_link_write_kind.return",
            1,
        ),
        ("sim.cpu0.o3.branch_repair_direction_only_kind.return", 1),
        ("sim.cpu0.o3.iew.predicted_not_taken_incorrect", 1),
        ("sim.cpu0.o3.iew.branch_mispredicts", 1),
    ] {
        assert_json_stat(&json, path, "Count", value, "monotonic");
    }
}

#[test]
fn rem6_run_text_stats_alias_o3_lsq_store_load_matches_after_detailed_switch() {
    let path = detailed_o3_lsq_store_load_match_binary(
        "m5-switch-cpu-detailed-o3-lsq-store-load-text-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_loads", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.lsq_stores", 1);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        1,
    );
    assert_text_count_stat(&stdout, "system.cpu.lsq0.storeLoadForwardingCandidates", 1);
    assert_text_count_stat(&stdout, "system.cpu.lsq0.storeLoadForwardingMatches", 1);
    assert_text_count_stat(&stdout, "system.cpu.lsq0.forwLoads", 1);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_operation.load_forwarding_candidates",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_operation.load_forwarding_matches",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_operation.store_forwarding_candidates",
        0,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.lsq_operation.store_forwarding_matches",
        0,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.lsq0.operation.load.storeLoadForwardingCandidates",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.lsq0.operation.load.storeLoadForwardingMatches",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.lsq0.operation.store.storeLoadForwardingCandidates",
        0,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.lsq0.operation.store.storeLoadForwardingMatches",
        0,
    );
}

#[test]
fn rem6_run_o3_runtime_json_exposes_store_load_forwarding_suppression() {
    for (path, address_mismatches, byte_mismatches) in [
        (
            detailed_o3_lsq_store_load_mismatch_binary(
                "m5-switch-cpu-detailed-o3-lsq-store-load-address-suppression-json",
            ),
            1,
            0,
        ),
        (
            detailed_o3_lsq_store_load_byte_mismatch_binary(
                "m5-switch-cpu-detailed-o3-lsq-store-load-byte-suppression-json",
            ),
            0,
            1,
        ),
        (
            detailed_o3_lsq_store_load_address_and_byte_mismatch_binary(
                "m5-switch-cpu-detailed-o3-lsq-store-load-address-byte-suppression-json",
            ),
            1,
            0,
        ),
    ] {
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
        let o3_runtime = json
            .pointer("/cores/0/o3_runtime")
            .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));

        for (pointer, stat_path, value) in [
            (
                "/store_load_forwarding_candidates",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
                0,
            ),
            (
                "/store_load_forwarding_matches",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
                0,
            ),
            (
                "/store_load_forwarding_suppressed",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_suppressed",
                1,
            ),
            (
                "/store_load_forwarding_address_mismatches",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_address_mismatches",
                address_mismatches,
            ),
            (
                "/store_load_forwarding_byte_mismatches",
                "sim.cpu0.o3.lsq_store_to_load_forwarding_byte_mismatches",
                byte_mismatches,
            ),
            (
                "/lsq/store_load_forwarding_suppressed",
                "system.cpu.lsq0.storeLoadForwardingSuppressed",
                1,
            ),
            (
                "/lsq/store_load_forwarding_address_mismatches",
                "system.cpu.lsq0.storeLoadForwardingAddressMismatches",
                address_mismatches,
            ),
            (
                "/lsq/store_load_forwarding_byte_mismatches",
                "system.cpu.lsq0.storeLoadForwardingByteMismatches",
                byte_mismatches,
            ),
            (
                "/lsq/operation/load/forwarding_suppressed",
                "system.cpu.lsq0.operation.load.storeLoadForwardingSuppressed",
                1,
            ),
            (
                "/lsq/operation/load/forwarding_address_mismatches",
                "system.cpu.lsq0.operation.load.storeLoadForwardingAddressMismatches",
                address_mismatches,
            ),
            (
                "/lsq/operation/load/forwarding_byte_mismatches",
                "system.cpu.lsq0.operation.load.storeLoadForwardingByteMismatches",
                byte_mismatches,
            ),
        ] {
            assert_eq!(
                o3_runtime.pointer(pointer).and_then(Value::as_u64),
                Some(value),
                "structured O3 runtime JSON should expose {pointer}: {o3_runtime}"
            );
            assert_json_stat(&json, stat_path, "Count", value, "monotonic");
        }
        for pointer in [
            "/lsq/operation/store/forwarding_suppressed",
            "/lsq/operation/store/forwarding_address_mismatches",
            "/lsq/operation/store/forwarding_byte_mismatches",
            "/lsq/operation/atomic/forwarding_suppressed",
            "/lsq/operation/atomic/forwarding_address_mismatches",
            "/lsq/operation/atomic/forwarding_byte_mismatches",
        ] {
            assert_eq!(
                o3_runtime.pointer(pointer).and_then(Value::as_u64),
                Some(0),
                "inactive O3 forwarding-suppression lane should stay zero at {pointer}: {o3_runtime}"
            );
        }
    }
}

#[test]
fn rem6_run_text_stats_alias_o3_fu_latency_after_detailed_switch() {
    let path = detailed_o3_fu_latency_binary("m5-switch-cpu-detailed-o3-fu-latency-text-stats");

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
            "text",
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
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_latency_instructions", 2);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_latency_cycles", 21);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_integer_mul_instructions", 1);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_integer_mul_latency_cycles", 2);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_integer_div_instructions", 1);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_integer_div_latency_cycles", 19);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::IntMult", 1);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::IntDiv", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.commit.committed_inst_type.int_mul", 1);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.commit.committed_inst_type.int_div", 1);
    assert_text_count_stat(&stdout, "system.cpu.commit.committedInstType_0::IntMult", 1);
    assert_text_count_stat(&stdout, "system.cpu.commit.committedInstType_0::IntDiv", 1);
}

#[test]
fn rem6_run_text_stats_alias_o3_float_misc_fu_latency_after_detailed_switch() {
    let path =
        detailed_o3_float_misc_fu_latency_binary("m5-switch-cpu-detailed-o3-float-misc-text-stats");

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
            "text",
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
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_latency_instructions", 4);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_latency_cycles", 6);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_float_misc_instructions", 2);
    assert_text_cycle_stat(&stdout, "sim.cpu0.o3.fu_float_misc_latency_cycles", 3);
    assert_text_count_stat(&stdout, "sim.cpu0.o3.fu_vector_float_misc_instructions", 2);
    assert_text_cycle_stat(
        &stdout,
        "sim.cpu0.o3.fu_vector_float_misc_latency_cycles",
        3,
    );
    assert_text_count_stat(&stdout, "sim.cpu0.o3.iq.issued_inst_type.float_misc", 2);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.iq.issued_inst_type.vector_float_misc",
        2,
    );
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::FloatMisc", 2);
    assert_text_count_stat(&stdout, "system.cpu.iq.issuedInstType_0::SimdFloatMisc", 2);
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.commit.committed_inst_type.float_misc",
        2,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.commit.committed_inst_type.vector_float_misc",
        2,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.commit.committedInstType_0::FloatMisc",
        2,
    );
    assert_text_count_stat(
        &stdout,
        "system.cpu.commit.committedInstType_0::SimdFloatMisc",
        2,
    );
}

#[test]
fn rem6_run_does_not_record_o3_runtime_stats_after_timing_switch() {
    let path = timing_switch_o3_stats_binary("m5-switch-cpu-timing-no-o3-runtime-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
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
    assert_json_stat_absent(&json, "sim.cpu0.o3.instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.rob_allocations");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_load_bytes");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_bytes");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_store_to_load_forwarding_matches");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_data_latency_samples");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_data_latency_ticks");
    assert_json_stat_absent(&json, "sim.cpu0.o3.lsq_operation.load_latency_ticks");
    assert_json_stat_absent(
        &json,
        "sim.cpu0.o3.lsq_operation.store_conditional_latency_ticks",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_mul_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_mul_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_div_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_integer_div_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_vector_integer_mul_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_vector_integer_mul_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_vector_integer_div_instructions");
    assert_json_stat_absent(&json, "sim.cpu0.o3.fu_vector_integer_div_latency_cycles");
    assert_json_stat_absent(&json, "sim.cpu0.o3.max_rob_occupancy");
    assert_json_stat_absent(&json, "sim.cpu0.o3.max_lsq_occupancy");
    assert_json_stat_absent(&json, "sim.cpu0.o3.rename_map_entries");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.insts_issued");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.mem_insts_issued");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.branch_insts_issued");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.mem_read");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.mem_write");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.int_mul");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.int_div");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.vector_integer_mul");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iq.issued_inst_type.vector_integer_div");
    assert_json_stat_absent(&json, "sim.cpu0.o3.commit.committed_inst_type.mem_read");
    assert_json_stat_absent(&json, "sim.cpu0.o3.commit.committed_inst_type.mem_write");
    assert_json_stat_absent(&json, "sim.cpu0.o3.commit.committed_inst_type.int_mul");
    assert_json_stat_absent(&json, "sim.cpu0.o3.commit.committed_inst_type.int_div");
    assert_json_stat_absent(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.vector_integer_mul",
    );
    assert_json_stat_absent(
        &json,
        "sim.cpu0.o3.commit.committed_inst_type.vector_integer_div",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.dispatched_insts");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.insts_to_commit");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.writeback_count");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.producer_inst");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.consumer_inst");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.predicted_taken_incorrect");
    assert_json_stat_absent(&json, "sim.cpu0.o3.iew.predicted_not_taken_incorrect");
    assert_json_stat_absent(&json, "system.cpu.iew.dispatchedInsts");
    assert_json_stat_absent(&json, "system.cpu.iew.instsToCommit.total");
    assert_json_stat_absent(&json, "system.cpu.iew.instsToCommit::total");
    assert_json_stat_absent(&json, "system.cpu.iew.writebackCount.total");
    assert_json_stat_absent(&json, "system.cpu.iew.writebackCount::total");
    assert_json_stat_absent(&json, "system.cpu.iew.producerInst.total");
    assert_json_stat_absent(&json, "system.cpu.iew.producerInst::total");
    assert_json_stat_absent(&json, "system.cpu.iew.consumerInst.total");
    assert_json_stat_absent(&json, "system.cpu.iew.consumerInst::total");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.MemRead");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.MemWrite");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.IntMult");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.IntDiv");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatAdd");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatCmp");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatMisc");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatMult");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatMultAcc");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatDiv");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.FloatSqrt");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdMult");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdDiv");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatAdd");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatCmp");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatMisc");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatMult");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatMultAcc");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatDiv");
    assert_json_stat_absent(&json, "system.cpu.iq.issuedInstType.SimdFloatSqrt");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.MemRead");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.MemWrite");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.IntMult");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.IntDiv");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatAdd");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatCmp");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatMisc");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatMult");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatMultAcc");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatDiv");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.FloatSqrt");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdMult");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdDiv");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatAdd");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatCmp");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatMisc");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatMult");
    assert_json_stat_absent(
        &json,
        "system.cpu.commit.committedInstType.SimdFloatMultAcc",
    );
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatDiv");
    assert_json_stat_absent(&json, "system.cpu.commit.committedInstType.SimdFloatSqrt");
    assert_json_stat_absent(&json, "system.cpu.iew.predictedTakenIncorrect");
    assert_json_stat_absent(&json, "system.cpu.iew.predictedNotTakenIncorrect");
    assert_json_stat_absent(&json, "system.cpu.lsq0.dataResponse.samples");
    assert_json_stat_absent(&json, "system.cpu.lsq0.dataResponse.totalLatency");
    assert_json_stat_absent(&json, "system.cpu.lsq0.dataResponse.load.totalLatency");
    assert_json_stat_absent(
        &json,
        "system.cpu.lsq0.dataResponse.storeConditional.totalLatency",
    );
    assert_json_stat_absent(&json, "system.cpu.lsq0.operation.load");
    assert_json_stat_absent(&json, "system.cpu.lsq0.operation.total");
    assert_json_stat_absent(&json, "system.cpu.lsq0.ordering.acquire");
    assert_json_stat_absent(&json, "system.cpu.lsq0.ordering.total");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair.targetlessMismatch");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair.directionOnly");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair.wrongTarget");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair.total");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair_0::TargetlessMismatch");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair_0::DirectionOnly");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair_0::WrongTarget");
    assert_json_stat_absent(&json, "system.cpu.iew.branchRepair_0::total");
    assert_json_stat_absent(&json, "system.cpu.iew.branchMispredicts");
    assert_json_stat_absent(&json, "system.cpu.commit.branchMispredicts");
    assert_json_stat_absent(&json, "system.cpu.rob.maxOccupancy");
    assert_json_stat_absent(&json, "system.cpu.lsq0.maxOccupancy");
    assert!(
        json.pointer("/cores/0/o3_runtime").is_none(),
        "timing-mode run should not emit inactive O3 runtime state: {json}"
    );
}

#[test]
fn rem6_run_text_stats_omit_o3_runtime_aliases_after_timing_switch() {
    let path = timing_switch_o3_stats_binary("m5-switch-cpu-timing-no-o3-runtime-text-stats");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "text",
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    for path in [
        "sim.cpu0.o3.instructions",
        "sim.cpu0.o3.rob_allocations",
        "sim.cpu0.o3.rob_commits",
        "sim.cpu0.o3.rename_writes",
        "sim.cpu0.o3.lsq_loads",
        "sim.cpu0.o3.lsq_stores",
        "sim.cpu0.o3.lsq_load_bytes",
        "sim.cpu0.o3.lsq_store_bytes",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_candidates",
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "sim.cpu0.o3.lsq_data_latency_samples",
        "sim.cpu0.o3.lsq_data_latency_ticks",
        "sim.cpu0.o3.lsq_operation.load_latency_ticks",
        "sim.cpu0.o3.lsq_operation.store_conditional_latency_ticks",
        "sim.cpu0.o3.fu_latency_instructions",
        "sim.cpu0.o3.fu_latency_cycles",
        "sim.cpu0.o3.fu_integer_mul_instructions",
        "sim.cpu0.o3.fu_integer_mul_latency_cycles",
        "sim.cpu0.o3.fu_integer_div_instructions",
        "sim.cpu0.o3.fu_integer_div_latency_cycles",
        "sim.cpu0.o3.max_rob_occupancy",
        "sim.cpu0.o3.max_lsq_occupancy",
        "sim.cpu0.o3.rename_map_entries",
        "sim.cpu0.o3.iq.insts_issued",
        "sim.cpu0.o3.iq.mem_insts_issued",
        "sim.cpu0.o3.iq.branch_insts_issued",
        "sim.cpu0.o3.iq.issued_inst_type.mem_read",
        "sim.cpu0.o3.iq.issued_inst_type.mem_write",
        "sim.cpu0.o3.iq.issued_inst_type.int_mul",
        "sim.cpu0.o3.iq.issued_inst_type.int_div",
        "sim.cpu0.o3.commit.committed_inst_type.mem_read",
        "sim.cpu0.o3.commit.committed_inst_type.mem_write",
        "sim.cpu0.o3.commit.committed_inst_type.int_mul",
        "sim.cpu0.o3.commit.committed_inst_type.int_div",
        "system.cpu.rename.renamedInsts",
        "system.cpu.rename.renamedOperands",
        "system.cpu.iew.dispatchedInsts",
        "system.cpu.iew.dispLoadInsts",
        "system.cpu.iew.dispStoreInsts",
        "system.cpu.iew.instsToCommit::total",
        "sim.cpu0.o3.iew.writeback_count",
        "sim.cpu0.o3.iew.producer_inst",
        "sim.cpu0.o3.iew.consumer_inst",
        "system.cpu.iew.writebackCount::total",
        "system.cpu.iew.producerInst::total",
        "system.cpu.iew.consumerInst::total",
        "system.cpu.iew.wbRate",
        "system.cpu.iew.wbFanout",
        "sim.cpu0.o3.iew.predicted_taken_incorrect",
        "sim.cpu0.o3.iew.predicted_not_taken_incorrect",
        "system.cpu.iew.predictedTakenIncorrect",
        "system.cpu.iew.predictedNotTakenIncorrect",
        "system.cpu.lsq0.dataResponse.samples",
        "system.cpu.lsq0.dataResponse.totalLatency",
        "system.cpu.lsq0.dataResponse.load.totalLatency",
        "system.cpu.lsq0.dataResponse.storeConditional.totalLatency",
        "system.cpu.lsq0.operation.load",
        "system.cpu.lsq0.operation.total",
        "system.cpu.lsq0.ordering.acquire",
        "system.cpu.lsq0.ordering.total",
        "system.cpu.iew.branchRepair.targetlessMismatch",
        "system.cpu.iew.branchRepair.directionOnly",
        "system.cpu.iew.branchRepair.wrongTarget",
        "system.cpu.iew.branchRepair.total",
        "system.cpu.iew.branchRepair_0::TargetlessMismatch",
        "system.cpu.iew.branchRepair_0::DirectionOnly",
        "system.cpu.iew.branchRepair_0::WrongTarget",
        "system.cpu.iew.branchRepair_0::total",
        "system.cpu.iew.branchMispredicts",
        "system.cpu.commit.branchMispredicts",
        "system.cpu.commit.committedInstType_0::MemRead",
        "system.cpu.commit.committedInstType_0::MemWrite",
        "system.cpu.commit.committedInstType_0::IntMult",
        "system.cpu.commit.committedInstType_0::IntDiv",
        "system.cpu.lsq0.addedLoadsAndStores",
        "system.cpu.lsq0.loadBytes",
        "system.cpu.lsq0.storeBytes",
        "system.cpu.lsq0.storeLoadForwardingCandidates",
        "system.cpu.lsq0.storeLoadForwardingMatches",
        "system.cpu.lsq0.forwLoads",
        "system.cpu.lsq0.maxOccupancy",
        "system.cpu.iq.instsIssued",
        "system.cpu.iq.memInstsIssued",
        "system.cpu.iq.branchInstsIssued",
        "system.cpu.iq.issuedInstType_0::MemRead",
        "system.cpu.iq.issuedInstType_0::MemWrite",
        "system.cpu.iq.issuedInstType_0::IntMult",
        "system.cpu.iq.issuedInstType_0::IntDiv",
        "system.cpu.rob.writes",
        "system.cpu.rob.reads",
        "system.cpu.rob.maxOccupancy",
    ] {
        assert_text_stat_absent(&stdout, path);
    }
}
