use super::*;

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
fn rem6_run_m5_dump_stats_exposes_o3_direct_call_ftq_aliases() {
    let path =
        detailed_o3_direct_call_dump_stats_binary("m5-switch-cpu-o3-direct-call-ftq-dump-stats");

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
        Some("10000080000000000000000000000000"),
        "the direct-call O3 run should skip the fallthrough write and store the JAL link register"
    );

    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");
    assert_eq!(
        host_actions
            .pointer("/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
        "direct-call fixture should deliver m5_dump_stats before stop: {host_actions}"
    );
    let dump = host_actions
        .pointer("/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing direct-call O3 stats dump action: {host_actions}"));
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.branch_event.branches", 1),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.mispredictions",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.link_write_kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.misprediction_kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squash_kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_link_write_kind.call_direct",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.squashed_target_without_link_write_kind.call_direct",
            0,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }

    for (path, value) in [
        ("system.cpu.ftq.squashes_0::CallDirect", 1),
        ("system.cpu.ftq.squashes_0::total", 1),
        ("system.cpu.ftq.squashedTargets_0::CallDirect", 1),
        ("system.cpu.ftq.squashedTargets_0::total", 1),
        (
            "system.cpu.ftq.squashedTargetsWithLinkWrites_0::CallDirect",
            1,
        ),
        ("system.cpu.ftq.squashedTargetsWithLinkWrites_0::total", 1),
        (
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::CallDirect",
            0,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::total",
            0,
        ),
    ] {
        assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.kind.call_direct",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.ftq.squashes_0::CallDirect",
        "Count",
        1,
        "monotonic",
    );
}

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
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.branch_event.squash_kind.direct_conditional",
        1,
    );
    assert_text_count_stat(
        &stdout,
        "sim.cpu0.o3.branch_event.squash_kind.direct_unconditional",
        2,
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
    for (path, value) in [
        ("system.cpu.ftq.squashes_0::CallIndirect", 0),
        ("system.cpu.ftq.squashes_0::DirectCond", 1),
        ("system.cpu.ftq.squashes_0::DirectUncond", 2),
        ("system.cpu.ftq.squashes_0::total", branch_mispredicts),
        ("system.cpu.ftq.squashedTargets_0::DirectCond", 1),
        ("system.cpu.ftq.squashedTargets_0::DirectUncond", 2),
        (
            "system.cpu.ftq.squashedTargets_0::total",
            branch_mispredicts,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithLinkWrites_0::DirectCond",
            0,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithLinkWrites_0::DirectUncond",
            0,
        ),
        ("system.cpu.ftq.squashedTargetsWithLinkWrites_0::total", 0),
        (
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::DirectCond",
            1,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::DirectUncond",
            2,
        ),
        (
            "system.cpu.ftq.squashedTargetsWithoutLinkWrites_0::total",
            branch_mispredicts,
        ),
    ] {
        assert_text_count_stat(&stdout, path, value);
        assert_text_stat_occurs_once(&stdout, path);
    }
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
