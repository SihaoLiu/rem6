use super::*;

#[test]
fn rem6_run_m5_dump_reset_stats_scopes_o3_event_summary_trace_rows() {
    let path =
        detailed_o3_branch_dump_reset_stats_binary("m5-switch-cpu-o3-event-summary-dump-reset");

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
    let post_reset_dump = host_actions
        .pointer("/stats_dumps/1")
        .unwrap_or_else(|| panic!("missing post-reset stats dump action: {host_actions}"));

    for (dump, minimum_records) in [(pre_reset_dump, 6), (post_reset_dump, 2)] {
        assert_stats_dump_sample_at_least(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
            "counter",
            "Count",
            minimum_records,
            "resettable",
        );
        assert_stats_dump_sample_at_least(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.span_ticks",
            "counter",
            "Tick",
            1,
            "resettable",
        );
        assert_stats_dump_sample_at_least(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.rob.allocations",
            "counter",
            "Count",
            minimum_records,
            "resettable",
        );
        assert_stats_dump_sample_at_least(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.rename.writes",
            "counter",
            "Count",
            1,
            "resettable",
        );
    }

    assert_stats_dump_sample_at_least(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.mispredictions",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample_at_least(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.direction_only_mismatches",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample_at_least(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.branch_mispredicts",
        "counter",
        "Count",
        1,
        "resettable",
    );
    assert_stats_dump_sample_at_least(
        post_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency.instructions",
        "counter",
        "Count",
        1,
        "resettable",
    );

    let pre_reset_records = stats_dump_sample_value(
        pre_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
    );
    let post_reset_records = stats_dump_sample_value(
        post_reset_dump,
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
    );
    assert!(
        pre_reset_records > post_reset_records,
        "m5_dump_reset_stats should reset O3 event-summary trace rows before post-reset work: pre={pre_reset_records}, post={post_reset_records}"
    );
}
