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

    for dump in [pre_reset_dump, post_reset_dump] {
        let event_summary_records = stats_dump_sample_value(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
        );
        let event_summary_span_ticks = stats_dump_sample_value(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.span_ticks",
        );
        let event_summary_producers = stats_dump_sample_value(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.iew.producer_inst",
        );
        let event_summary_consumers = stats_dump_sample_value(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.iew.consumer_inst",
        );
        for (path, value) in [
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.dispatched_insts",
                event_summary_records,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.insts_to_commit",
                stats_dump_sample_value(
                    dump,
                    "sim.host_actions.stats_dump.cpu0.o3.rob_commits",
                ),
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.writeback_count",
                event_summary_records,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.producer_inst",
                event_summary_producers,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.consumer_inst",
                event_summary_consumers,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.dependency.producer",
                event_summary_producers,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.dependency.consumer",
                event_summary_consumers,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.predicted_taken_incorrect",
                stats_dump_sample_value(
                    dump,
                    "sim.host_actions.stats_dump.cpu0.o3.iew.predicted_taken_incorrect",
                ),
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.predicted_not_taken_incorrect",
                stats_dump_sample_value(
                    dump,
                    "sim.host_actions.stats_dump.cpu0.o3.iew.predicted_not_taken_incorrect",
                ),
            ),
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
        }
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.writeback_rate_ppm",
            "counter",
            "Ppm",
            ratio_ppm(event_summary_records, event_summary_span_ticks),
            "resettable",
        );
        assert_stats_dump_sample(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.producer_consumer_fanout_ppm",
            "counter",
            "Ppm",
            ratio_ppm(event_summary_producers, event_summary_consumers),
            "resettable",
        );
    }

    for dump in [pre_reset_dump, post_reset_dump] {
        let event_summary_records = stats_dump_sample_value(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
        );
        let event_summary_lsq_loads = stats_dump_sample_value(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.lsq_operation.load",
        );
        let event_summary_lsq_stores = stats_dump_sample_value(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.lsq_operation.store",
        );
        let event_summary_branch_mispredicts = stats_dump_sample_value(
            dump,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.iew.branch_mispredicts",
        );
        for (path, value) in [
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iq.insts_issued",
                event_summary_records,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iq.mem_insts_issued",
                event_summary_lsq_loads.saturating_add(event_summary_lsq_stores),
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iq.branch_insts_issued",
                stats_dump_sample_value(
                    dump,
                    "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.branches",
                ),
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iq.issued_inst_type.mem_read",
                event_summary_lsq_loads,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iq.issued_inst_type.mem_write",
                event_summary_lsq_stores,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.commit.branch_mispredicts",
                event_summary_branch_mispredicts,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.commit.committed_inst_type.mem_read",
                event_summary_lsq_loads,
            ),
            (
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.commit.committed_inst_type.mem_write",
                event_summary_lsq_stores,
            ),
        ] {
            assert_stats_dump_sample(dump, path, "counter", "Count", value, "resettable");
        }
        for (inst_type, class) in [
            ("int_mul", "integer_mul"),
            ("int_div", "integer_div"),
            ("float_misc", "float_misc"),
            ("vector_float_misc", "vector_float_misc"),
        ] {
            let instructions = stats_dump_sample_value(
                dump,
                &format!(
                    "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.{class}.instructions"
                ),
            );
            for prefix in [
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.iq.issued_inst_type",
                "sim.host_actions.stats_dump.cpu0.o3.event_summary.commit.committed_inst_type",
            ] {
                assert_stats_dump_sample(
                    dump,
                    &format!("{prefix}.{inst_type}"),
                    "counter",
                    "Count",
                    instructions,
                    "resettable",
                );
            }
        }
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
    for (path, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.rob.commit_blocked_events",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.rob.max_commits_at_tick",
            2,
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
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.branches",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.taken",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.not_taken",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_taken",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_not_taken",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_targets",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_target_mismatches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.resolved_targets",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.squashes",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.squashed_targets",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.squashed_targets_without_link_writes",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.not_taken_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_taken_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_not_taken_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_target_mismatch_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.misprediction_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.misprediction_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.squash_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.squashed_target_without_link_write_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.targetless_mismatches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.wrong_targets",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.targetless_mismatch_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.direction_only_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.mismatches",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.without_link_writes",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.squashed_targets",
            3,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.squashed_target_without_link_write_kind.direct_unconditional",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatches",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_without_link_writes",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_squashed_targets",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_squashed_target_without_link_write_kind.direct_conditional",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.wrong_targets",
            0,
        ),
    ] {
        assert_stats_dump_sample(pre_reset_dump, path, "counter", "Count", value, "resettable");
    }
    for (path, unit, value) in [
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency.instructions",
            "Count",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency.cycles",
            "Cycle",
            21,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency.max_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency.min_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency.avg_cycles",
            "Cycle",
            10,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_mul.instructions",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_mul.cycles",
            "Cycle",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_mul.max_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_mul.min_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_mul.avg_cycles",
            "Cycle",
            2,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_div.instructions",
            "Count",
            1,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_div.cycles",
            "Cycle",
            19,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_div.max_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_div.min_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.integer_div.avg_cycles",
            "Cycle",
            19,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.float_misc.instructions",
            "Count",
            0,
        ),
        (
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.fu_latency_class.vector_float_misc.cycles",
            "Cycle",
            0,
        ),
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", unit, value, "resettable");
    }
    for path in [
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.branches",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_targets",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.predicted_target_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.squashes",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.misprediction_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_event.misprediction_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.targetless_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.wrong_targets",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.targetless_mismatch_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_repair.direction_only_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.without_link_writes",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.squashed_targets",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_direction_mismatch.squashed_target_without_link_write_kind.direct_unconditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatches",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_without_link_writes",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_squashed_targets",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.targetless_mismatch_squashed_target_without_link_write_kind.direct_conditional",
        "sim.host_actions.stats_dump.cpu0.o3.event_summary.branch_target_mismatch.wrong_targets",
    ] {
        assert_stats_dump_sample(post_reset_dump, path, "counter", "Count", 0, "resettable");
    }

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
