use super::*;

#[test]
fn rem6_run_m5_dump_stats_exposes_multicore_o3_direct_call_ftq_aliases_by_active_hart() {
    let path = multicore_hart1_detailed_o3_direct_call_dump_stats_binary(
        "m5-switch-cpu-o3-multicore-direct-call-ftq-dump-stats",
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
        Some("1c000080000000000000000000000000"),
        "hart 1 direct-call O3 run should skip the fallthrough write and store the JAL link"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "multicore direct-call fixture should deliver one m5_dump_stats action: {host_actions}"
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
        .unwrap_or_else(|| panic!("missing CPU1 direct-call O3 stats dump: {host_actions}"));
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu1.o3.branch_event.branches", 1),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.mispredictions",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.link_write_kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.misprediction_kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squash_kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_link_write_kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_without_link_write_kind.call_direct",
            0,
        ),
        ("system.cpu1.ftq.squashes_0::CallDirect", 1),
        ("system.cpu1.ftq.squashes_0::total", 1),
        ("system.cpu1.ftq.squashedTargets_0::CallDirect", 1),
        ("system.cpu1.ftq.squashedTargets_0::total", 1),
        (
            "system.cpu1.ftq.squashedTargetsWithLinkWrites_0::CallDirect",
            1,
        ),
        (
            "system.cpu1.ftq.squashedTargetsWithLinkWrites_0::total",
            1,
        ),
        (
            "system.cpu1.ftq.squashedTargetsWithoutLinkWrites_0::CallDirect",
            0,
        ),
        (
            "system.cpu1.ftq.squashedTargetsWithoutLinkWrites_0::total",
            0,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }

    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.call_direct",
        "system.cpu.ftq.squashes_0::CallDirect",
        "system.cpu0.ftq.squashes_0::CallDirect",
        "system.cpu.ftq.squashes_0::total",
        "system.cpu0.ftq.squashes_0::total",
        "system.cpu.ftq.squashedTargets_0::CallDirect",
        "system.cpu0.ftq.squashedTargets_0::CallDirect",
        "system.cpu.ftq.squashedTargets_0::total",
        "system.cpu0.ftq.squashedTargets_0::total",
        "system.cpu.ftq.squashedTargetsWithLinkWrites_0::CallDirect",
        "system.cpu0.ftq.squashedTargetsWithLinkWrites_0::CallDirect",
        "system.cpu.ftq.squashedTargetsWithLinkWrites_0::total",
        "system.cpu0.ftq.squashedTargetsWithLinkWrites_0::total",
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::CallDirect",
        "system.cpu0.ftq.squashedTargetsWithoutLinkWrites_0::CallDirect",
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::total",
        "system.cpu0.ftq.squashedTargetsWithoutLinkWrites_0::total",
    ] {
        assert_stats_dump_sample_absent(dump, path);
        assert_json_stat_absent(&json, path);
    }
    assert_json_stat(
        &json,
        "sim.cpu1.o3.branch_event.kind.call_direct",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.ftq.squashes_0::CallDirect",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.branch_event.kind.call_direct");
}

#[test]
fn rem6_run_m5_dump_stats_exposes_multicore_o3_return_ftq_aliases_by_active_hart() {
    let path = multicore_hart1_detailed_o3_return_branch_dump_stats_binary(
        "m5-switch-cpu-o3-multicore-return-ftq-dump-stats",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "380",
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
        Some("28000080000000000000000000000000"),
        "hart 1 return O3 run should skip the fallthrough write and store the return target"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "multicore return fixture should deliver one m5_dump_stats action: {host_actions}"
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
        .unwrap_or_else(|| panic!("missing CPU1 return O3 stats dump: {host_actions}"));
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu1.o3.branch_event.branches", 1),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.kind.return",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.taken_kind.return",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.without_link_write_kind.return",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.link_write_kind.return",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squash_kind.return",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_kind.return",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_link_write_kind.return",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_event.squashed_target_without_link_write_kind.return",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu1.o3.branch_repair_direction_only_kind.return",
            1,
        ),
        ("system.cpu1.ftq.squashes_0::Return", 1),
        ("system.cpu1.ftq.squashes_0::total", 1),
        ("system.cpu1.ftq.squashedTargets_0::Return", 1),
        ("system.cpu1.ftq.squashedTargets_0::total", 1),
        ("system.cpu1.ftq.squashedTargetsWithLinkWrites_0::Return", 0),
        ("system.cpu1.ftq.squashedTargetsWithLinkWrites_0::total", 0),
        (
            "system.cpu1.ftq.squashedTargetsWithoutLinkWrites_0::Return",
            1,
        ),
        (
            "system.cpu1.ftq.squashedTargetsWithoutLinkWrites_0::total",
            1,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }

    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.return",
        "system.cpu.ftq.squashes_0::Return",
        "system.cpu0.ftq.squashes_0::Return",
        "system.cpu.ftq.squashes_0::total",
        "system.cpu0.ftq.squashes_0::total",
        "system.cpu.ftq.squashedTargets_0::Return",
        "system.cpu0.ftq.squashedTargets_0::Return",
        "system.cpu.ftq.squashedTargets_0::total",
        "system.cpu0.ftq.squashedTargets_0::total",
        "system.cpu.ftq.squashedTargetsWithLinkWrites_0::Return",
        "system.cpu0.ftq.squashedTargetsWithLinkWrites_0::Return",
        "system.cpu.ftq.squashedTargetsWithLinkWrites_0::total",
        "system.cpu0.ftq.squashedTargetsWithLinkWrites_0::total",
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::Return",
        "system.cpu0.ftq.squashedTargetsWithoutLinkWrites_0::Return",
        "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::total",
        "system.cpu0.ftq.squashedTargetsWithoutLinkWrites_0::total",
    ] {
        assert_stats_dump_sample_absent(dump, path);
        assert_json_stat_absent(&json, path);
    }
    assert_json_stat(
        &json,
        "sim.cpu1.o3.branch_event.kind.return",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu1.ftq.squashes_0::Return",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_absent(&json, "sim.cpu0.o3.branch_event.kind.return");
}
