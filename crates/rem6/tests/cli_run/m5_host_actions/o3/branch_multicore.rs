use super::*;

#[test]
fn rem6_run_m5_dump_stats_exposes_multicore_o3_indirect_call_ftq_aliases_by_active_hart() {
    let path = multicore_hart1_detailed_o3_indirect_call_wrong_target_dump_stats_binary(
        "m5-switch-cpu-o3-multicore-indirect-call-ftq-dump-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "420",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "2",
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
        Some("30000080000000001800008000000000"),
        "hart 1 indirect-call O3 run should store target and link witnesses"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "multicore indirect-call fixture should deliver one m5_dump_stats action: {host_actions}"
    );
    assert_execution_mode_switch(
        host_actions,
        0,
        "cpu1",
        None,
        "detailed",
        "execution-mode-switch-cpu1",
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing CPU1 indirect-call O3 stats dump: {host_actions}"));

    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu1.o3.branch_event.branches", 2),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.without_link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.mispredictions",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.kind.direct_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.link_write_kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.without_link_write_kind.direct_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.misprediction_kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.misprediction_kind.direct_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_kind.direct_unconditional",
            1,
        ),
        ("system.cpu1.ftq.squashes_0::CallIndirect", 1),
        ("system.cpu1.ftq.squashes_0::DirectUncond", 1),
        ("system.cpu1.ftq.squashes_0::total", 2),
        ("system.cpu1.ftq.squashedTargets_0::CallIndirect", 1),
        ("system.cpu1.ftq.squashedTargets_0::DirectUncond", 1),
        ("system.cpu1.ftq.squashedTargets_0::total", 2),
        (
            "system.cpu1.ftq.squashedTargetsWithLinkWrites_0::CallIndirect",
            1,
        ),
        (
            "system.cpu1.ftq.squashedTargetsWithLinkWrites_0::DirectUncond",
            0,
        ),
        (
            "system.cpu1.ftq.squashedTargetsWithoutLinkWrites_0::CallIndirect",
            0,
        ),
        (
            "system.cpu1.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
            1,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }

    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.call_indirect",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.direct_unconditional",
        "system.cpu.ftq.squashes_0::CallIndirect",
        "system.cpu0.ftq.squashes_0::CallIndirect",
        "system.cpu.ftq.squashes_0::DirectUncond",
        "system.cpu0.ftq.squashes_0::DirectUncond",
        "system.cpu.ftq.squashedTargets_0::CallIndirect",
        "system.cpu0.ftq.squashedTargets_0::CallIndirect",
        "system.cpu.ftq.squashedTargetsWithLinkWrites_0::CallIndirect",
        "system.cpu0.ftq.squashedTargetsWithLinkWrites_0::CallIndirect",
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
        "system.cpu0.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
    ] {
        assert_stats_dump_sample_absent(dump, path);
        assert_json_stat_absent(&json, path);
    }
    assert_json_stat(
        &json,
        "sim.cpu1.o3.branch_event.kind.call_indirect",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu1.o3.branch_event.kind.direct_unconditional",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.ftq.squashes_0::CallIndirect",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.branch_event.kind.call_indirect");
}
