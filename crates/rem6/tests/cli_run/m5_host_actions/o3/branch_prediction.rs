use super::*;

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_branch_predicted_target_match_snapshot() {
    let path = detailed_o3_branch_predicted_target_match_dump_reset_stats_binary(
        "m5-switch-cpu-o3-branch-predicted-target-match-dump-reset-stats",
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
    for (path, value) in [
        ("sim.host_actions.stats_dump.cpu0.o3.branch_event.branches", 3),
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
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_mismatches",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_match_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_target_matches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_target_match_kind.direct_conditional",
            1,
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
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.branches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_taken",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_not_taken",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_targets",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_matches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.branch_event.predicted_target_match_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_target_matches",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_target_match_kind.direct_conditional",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }

    assert_json_stat(
        &json,
        "sim.cpu0.o3.branch_event.predicted_target_matches",
        "Count",
        0,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.event_summary.branch_event.predicted_target_matches",
        "Count",
        0,
        "monotonic",
    );
}
