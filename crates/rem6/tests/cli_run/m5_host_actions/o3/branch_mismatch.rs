use super::*;

#[test]
fn rem6_run_json_stats_exposes_live_o3_branch_mismatch_without_debug_trace() {
    let path = detailed_o3_branch_repair_text_stats_binary(
        "m5-switch-cpu-o3-live-branch-mismatch-no-debug",
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
    let o3_runtime = json
        .pointer("/cores/0/o3_runtime")
        .unwrap_or_else(|| panic!("run JSON should include core O3 runtime state: {json}"));
    assert!(
        o3_runtime
            .pointer("/branch_direction_mismatch")
            .is_some_and(Value::is_null),
        "non-debug O3 runtime JSON should keep trace-derived direction summary null: {o3_runtime}"
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu0.o3.branch_direction_mismatch.mismatches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat_at_least(
        &json,
        "sim.cpu0.o3.branch_target_mismatch.targetless_mismatches",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_wrong_target_mismatch_snapshot() {
    let path = detailed_o3_indirect_call_wrong_target_dump_reset_stats_binary(
        "m5-switch-cpu-o3-wrong-target-mismatch-dump-reset-stats",
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
            "--debug-flags",
            "O3",
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
    let wrong_target_samples = [
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_targets",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_without_link_writes",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_squashed_targets",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_squashed_target_link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_link_write_kind.call_indirect",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_squashed_target_link_write_kind.call_indirect",
            1,
        ),
    ];
    for (path, value) in wrong_target_samples {
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
    for (path, _) in wrong_target_samples {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_target_mismatch.wrong_targets",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_no_link_wrong_target_mismatch_snapshot() {
    let path = detailed_o3_indirect_jump_wrong_target_dump_reset_stats_binary(
        "m5-switch-cpu-o3-no-link-wrong-target-mismatch-dump-reset-stats",
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
            "--debug-flags",
            "O3",
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
    let no_link_wrong_target_samples = [
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_targets",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_link_writes",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_without_link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_squashed_targets",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_squashed_target_without_link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_kind.indirect_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_without_link_write_kind.indirect_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_target_mismatch.wrong_target_squashed_target_without_link_write_kind.indirect_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.wrong_targets",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.wrong_target_without_link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.wrong_target_kind.indirect_unconditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.wrong_target_squashed_target_without_link_write_kind.indirect_unconditional",
            1,
        ),
        ("system.cpu.iew.branchWrongTargets", 1),
        ("system.cpu.iew.branchWrongTargetWithoutLinkWrites", 1),
        ("system.cpu.iew.branchWrongTargetLinkWrites", 0),
        ("system.cpu.iew.branchWrongTargetSquashedTargets", 1),
        (
            "system.cpu.iew.branchWrongTargetSquashedTargetWithoutLinkWrites",
            1,
        ),
        ("system.cpu.iew.branchWrongTarget_0::IndirectUncond", 1),
        (
            "system.cpu.iew.branchWrongTargetWithoutLinkWrites_0::IndirectUncond",
            1,
        ),
        (
            "system.cpu.iew.branchWrongTargetLinkWrites_0::IndirectUncond",
            0,
        ),
        (
            "system.cpu.iew.branchWrongTargetSquashedTargetWithoutLinkWrites_0::IndirectUncond",
            1,
        ),
    ];
    for (path, value) in no_link_wrong_target_samples {
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
    for (path, _) in no_link_wrong_target_samples {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_target_mismatch.wrong_targets",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.event_summary.branch_target_mismatch.wrong_targets",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_exposes_live_o3_branch_mismatch_gem5_alias_stats() {
    let path = detailed_o3_branch_repair_text_stats_binary(
        "m5-switch-cpu-o3-live-branch-mismatch-gem5-alias-stats",
    );

    let text_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--riscv-branch-lookahead",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        text_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&text_output.stderr)
    );
    let stdout = String::from_utf8(text_output.stdout).unwrap();
    for (path, value) in [
        ("system.cpu.iew.branchDirectionMismatches", 3),
        ("system.cpu.iew.branchDirectionMismatch_0::DirectUncond", 2),
        ("system.cpu.iew.branchTargetlessMismatches", 1),
        ("system.cpu.iew.branchTargetlessMismatch_0::DirectCond", 1),
        ("system.cpu.iew.branchWrongTargets", 0),
        ("system.cpu.iew.branchWrongTarget_0::CallIndirect", 0),
    ] {
        assert_text_count_stat(&stdout, path, value);
        assert_text_stat_occurs_once(&stdout, path);
    }

    let json_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--riscv-branch-lookahead",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        json_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let json: Value = serde_json::from_slice(&json_output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    assert_json_stat(
        &json,
        "system.cpu.iew.branchDirectionMismatches",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.branchDirectionMismatch_0::DirectUncond",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.branchTargetlessMismatches",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.branchTargetlessMismatch_0::DirectCond",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.branchWrongTargets",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.iew.branchWrongTarget_0::CallIndirect",
        "Count",
        0,
        "monotonic",
    );
}
