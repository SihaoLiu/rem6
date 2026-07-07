use rem6_cpu::{
    BranchTargetKind, BranchTargetKindCounts, BranchTargetProvider, BranchTargetProviderCounts,
    O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
};

use crate::{pipeline_stats::Rem6InOrderPipelineStageSummary, Rem6CoreSummary};

fn branch_target_kind_counts_json(counts: BranchTargetKindCounts) -> String {
    branch_target_kind_counts_json_with_total(counts, counts.total())
}

fn branch_target_kind_counts_json_with_total(counts: BranchTargetKindCounts, total: u64) -> String {
    let mut fields = BranchTargetKind::ALL
        .into_iter()
        .map(|kind| format!("\"{}\":{}", kind.canonical_stat_name(), counts.value(kind)))
        .collect::<Vec<_>>();
    fields.push(format!("\"total\":{total}"));
    format!("{{{}}}", fields.join(","))
}

fn branch_target_provider_counts_json(counts: BranchTargetProviderCounts) -> String {
    let mut fields = BranchTargetProvider::ALL
        .into_iter()
        .map(|provider| {
            format!(
                "\"{}\":{}",
                provider.canonical_stat_name(),
                counts.value(provider)
            )
        })
        .collect::<Vec<_>>();
    fields.push(format!("\"total\":{}", counts.total()));
    format!("{{{}}}", fields.join(","))
}

fn branch_predictor_counter_json(
    lookups: u64,
    history_updates: u64,
    updates: u64,
    squashes: u64,
) -> String {
    format!(
        "{{\"lookups\":{lookups},\"history_updates\":{history_updates},\"updates\":{updates},\"squashes\":{squashes}}}"
    )
}

fn in_order_pipeline_stage_summary_json(summary: Rem6InOrderPipelineStageSummary) -> String {
    format!(
        "{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}}",
        summary.fetch1, summary.fetch2, summary.decode, summary.execute, summary.commit
    )
}

fn in_order_pipeline_stall_cause_stage_summary_json(
    resource_blocked: Rem6InOrderPipelineStageSummary,
    resource_blocked_cycles: Rem6InOrderPipelineStageSummary,
    ordering_blocked: Rem6InOrderPipelineStageSummary,
    ordering_blocked_cycles: Rem6InOrderPipelineStageSummary,
) -> String {
    format!(
        "{{\"stage_resource_blocked\":{},\"stage_resource_blocked_cycles\":{},\"stage_ordering_blocked\":{},\"stage_ordering_blocked_cycles\":{}}}",
        in_order_pipeline_stage_summary_json(resource_blocked),
        in_order_pipeline_stage_summary_json(resource_blocked_cycles),
        in_order_pipeline_stage_summary_json(ordering_blocked),
        in_order_pipeline_stage_summary_json(ordering_blocked_cycles)
    )
}

fn in_order_pipeline_flush_redirect_cause_stage_summary_json(
    flushed: Rem6InOrderPipelineStageSummary,
    flushed_cycles: Rem6InOrderPipelineStageSummary,
) -> String {
    format!(
        "{{\"stage_flushed\":{},\"stage_flushed_cycles\":{}}}",
        in_order_pipeline_stage_summary_json(flushed),
        in_order_pipeline_stage_summary_json(flushed_cycles)
    )
}

fn o3_runtime_fu_latency_class_json(summary: &Rem6CoreSummary) -> String {
    O3RuntimeFuLatencyClass::ALL
        .into_iter()
        .flat_map(|class| {
            [
                format!(
                    "\"fu_{}_instructions\":{}",
                    class.stat_stem(),
                    summary.o3_runtime.fu_latency_class_instructions(class)
                ),
                format!(
                    "\"fu_{}_latency_cycles\":{}",
                    class.stat_stem(),
                    summary.o3_runtime.fu_latency_class_cycles(class)
                ),
            ]
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn o3_runtime_inst_type_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "int_div",
        _ => class.stat_stem(),
    }
}

fn o3_runtime_inst_type_json(summary: &Rem6CoreSummary) -> String {
    let mut fields = vec![
        format!("\"mem_read\":{}", summary.o3_runtime.lsq_loads()),
        format!("\"mem_write\":{}", summary.o3_runtime.lsq_stores()),
    ];
    fields.extend(O3RuntimeFuLatencyClass::ALL.into_iter().map(|class| {
        format!(
            "\"{}\":{}",
            o3_runtime_inst_type_stem(class),
            summary.o3_runtime.fu_latency_class_instructions(class)
        )
    }));
    format!("{{{}}}", fields.join(","))
}

fn o3_runtime_branch_mispredicts(summary: &Rem6CoreSummary) -> u64 {
    summary
        .o3_runtime
        .iew_predicted_taken_incorrect()
        .saturating_add(summary.o3_runtime.iew_predicted_not_taken_incorrect())
}

fn o3_runtime_iq_json(summary: &Rem6CoreSummary) -> String {
    let issued_inst_type = o3_runtime_inst_type_json(summary);
    format!(
        "{{\"insts_issued\":{},\"mem_insts_issued\":{},\"branch_insts_issued\":{},\"issued_inst_type\":{issued_inst_type}}}",
        summary.o3_runtime.instructions(),
        summary
            .o3_runtime
            .lsq_loads()
            .saturating_add(summary.o3_runtime.lsq_stores()),
        summary.o3_runtime.iq_branch_insts_issued(),
    )
}

fn o3_runtime_iew_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "{{\"dispatched_insts\":{},\"insts_to_commit\":{},\"writeback_count\":{},\"producer_inst\":{},\"consumer_inst\":{},\"predicted_taken_incorrect\":{},\"predicted_not_taken_incorrect\":{},\"branch_mispredicts\":{}}}",
        summary.o3_runtime.instructions(),
        summary.o3_runtime.rob_commits(),
        summary.o3_runtime.instructions(),
        summary.o3_runtime.iew_producer_insts(),
        summary.o3_runtime.iew_consumer_insts(),
        summary.o3_runtime.iew_predicted_taken_incorrect(),
        summary.o3_runtime.iew_predicted_not_taken_incorrect(),
        o3_runtime_branch_mispredicts(summary),
    )
}

fn o3_runtime_commit_json(summary: &Rem6CoreSummary) -> String {
    let committed_inst_type = o3_runtime_inst_type_json(summary);
    format!(
        "{{\"branch_mispredicts\":{},\"committed_inst_type\":{committed_inst_type}}}",
        o3_runtime_branch_mispredicts(summary)
    )
}

fn o3_runtime_rob_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "{{\"allocations\":{},\"commits\":{},\"max_occupancy\":{}}}",
        summary.o3_runtime.rob_allocations(),
        summary.o3_runtime.rob_commits(),
        summary.o3_runtime.max_rob_occupancy()
    )
}

fn o3_runtime_rename_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "{{\"writes\":{},\"map_entries\":{}}}",
        summary.o3_runtime.rename_writes(),
        summary.o3_runtime.rename_map_entries()
    )
}

fn o3_runtime_latency_json(
    samples: u64,
    ticks: u64,
    max_ticks: u64,
    min_ticks: u64,
    avg_ticks: u64,
) -> String {
    format!(
        "{{\"samples\":{samples},\"ticks\":{ticks},\"max_ticks\":{max_ticks},\"min_ticks\":{min_ticks},\"avg_ticks\":{avg_ticks}}}"
    )
}

fn o3_runtime_lsq_operation_matrix_json(summary: &Rem6CoreSummary) -> String {
    let fields = O3RuntimeLsqOperation::TRACKED
        .into_iter()
        .map(|operation| {
            let latency = o3_runtime_latency_json(
                summary.o3_runtime.lsq_operation_latency_samples(operation),
                summary.o3_runtime.lsq_operation_latency_ticks(operation),
                summary
                    .o3_runtime
                    .lsq_operation_latency_max_ticks(operation),
                summary
                    .o3_runtime
                    .lsq_operation_latency_min_ticks(operation),
                summary
                    .o3_runtime
                    .lsq_operation_latency_avg_ticks(operation),
            );
            format!(
                "\"{}\":{{\"count\":{},\"forwarding_candidates\":{},\"forwarding_matches\":{},\"forwarding_suppressed\":{},\"latency\":{latency}}}",
                operation.as_str(),
                summary.o3_runtime.lsq_operation_count(operation),
                summary
                    .o3_runtime
                    .lsq_operation_forwarding_candidates(operation),
                summary.o3_runtime.lsq_operation_forwarding_matches(operation),
                summary
                    .o3_runtime
                    .lsq_operation_forwarding_suppressed(operation),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn o3_runtime_lsq_ordering_matrix_json(summary: &Rem6CoreSummary) -> String {
    let fields = O3RuntimeLsqOrdering::TRACKED
        .into_iter()
        .map(|ordering| {
            format!(
                "\"{}\":{}",
                ordering.as_str(),
                summary.o3_runtime.lsq_ordering_count(ordering)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn o3_runtime_lsq_json(summary: &Rem6CoreSummary) -> String {
    let data_latency = o3_runtime_latency_json(
        summary.o3_runtime.lsq_data_latency_samples(),
        summary.o3_runtime.lsq_data_latency_ticks(),
        summary.o3_runtime.lsq_data_latency_max_ticks(),
        summary.o3_runtime.lsq_data_latency_min_ticks(),
        summary.o3_runtime.lsq_data_latency_avg_ticks(),
    );
    let operation = o3_runtime_lsq_operation_matrix_json(summary);
    let ordering = o3_runtime_lsq_ordering_matrix_json(summary);
    format!(
        "{{\"loads\":{},\"stores\":{},\"load_bytes\":{},\"store_bytes\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"store_load_forwarding_suppressed\":{},\"store_conditional_failures\":{},\"max_occupancy\":{},\"data_latency\":{data_latency},\"operation\":{operation},\"ordering\":{ordering}}}",
        summary.o3_runtime.lsq_loads(),
        summary.o3_runtime.lsq_stores(),
        summary.o3_runtime.lsq_load_bytes(),
        summary.o3_runtime.lsq_store_bytes(),
        summary.o3_runtime.lsq_store_to_load_forwarding_candidates(),
        summary.o3_runtime.lsq_store_to_load_forwarding_matches(),
        summary.o3_runtime.lsq_store_to_load_forwarding_suppressed(),
        summary.o3_runtime.lsq_store_conditional_failures(),
        summary.o3_runtime.max_lsq_occupancy(),
    )
}

fn o3_runtime_lsq_operation_json(summary: &Rem6CoreSummary) -> String {
    O3RuntimeLsqOperation::TRACKED
        .into_iter()
        .flat_map(|operation| {
            let name = operation.as_str();
            [
                format!(
                    "\"lsq_operation_{name}\":{}",
                    summary.o3_runtime.lsq_operation_count(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_forwarding_candidates\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_forwarding_candidates(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_forwarding_matches\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_forwarding_matches(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_forwarding_suppressed\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_forwarding_suppressed(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_ticks\":{}",
                    summary.o3_runtime.lsq_operation_latency_ticks(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_samples\":{}",
                    summary.o3_runtime.lsq_operation_latency_samples(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_max_ticks\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_latency_max_ticks(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_min_ticks\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_latency_min_ticks(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_avg_ticks\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_latency_avg_ticks(operation)
                ),
            ]
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn o3_runtime_lsq_data_latency_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "\"lsq_data_latency_samples\":{},\"lsq_data_latency_ticks\":{},\"lsq_data_latency_max_ticks\":{},\"lsq_data_latency_min_ticks\":{},\"lsq_data_latency_avg_ticks\":{}",
        summary.o3_runtime.lsq_data_latency_samples(),
        summary.o3_runtime.lsq_data_latency_ticks(),
        summary.o3_runtime.lsq_data_latency_max_ticks(),
        summary.o3_runtime.lsq_data_latency_min_ticks(),
        summary.o3_runtime.lsq_data_latency_avg_ticks()
    )
}

fn o3_runtime_lsq_ordering_json(summary: &Rem6CoreSummary) -> String {
    O3RuntimeLsqOrdering::TRACKED
        .into_iter()
        .map(|ordering| {
            format!(
                "\"lsq_ordering_{}\":{}",
                ordering.as_str(),
                summary.o3_runtime.lsq_ordering_count(ordering)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn o3_runtime_branch_repair_kind_json<F>(count: F) -> String
where
    F: Fn(BranchTargetKind) -> u64,
{
    let fields = BranchTargetKind::ALL
        .into_iter()
        .map(|kind| format!("\"{}\":{}", kind.canonical_stat_name(), count(kind)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn o3_runtime_branch_event_json(summary: &Rem6CoreSummary) -> String {
    let kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_kind(branch_kind)
    });
    let taken_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_taken_kind(branch_kind)
    });
    let resolved_target_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_resolved_target_kind(branch_kind)
    });
    let link_write_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_link_write_kind(branch_kind)
    });
    let squash_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_squash_kind(branch_kind)
    });
    let squashed_target_without_link_write_kind =
        o3_runtime_branch_repair_kind_json(|branch_kind| {
            summary
                .o3_runtime
                .branch_event_squashed_target_without_link_write_kind(branch_kind)
        });
    format!(
        "{{\"kind\":{kind},\"taken_kind\":{taken_kind},\"resolved_target_kind\":{resolved_target_kind},\"link_write_kind\":{link_write_kind},\"squash_kind\":{squash_kind},\"squashed_target_without_link_write_kind\":{squashed_target_without_link_write_kind}}}"
    )
}

fn o3_runtime_branch_repair_json(summary: &Rem6CoreSummary) -> String {
    let targetless_mismatch_kind = o3_runtime_branch_repair_kind_json(|kind| {
        summary
            .o3_runtime
            .branch_repair_targetless_mismatch_kind(kind)
    });
    let wrong_target_kind = o3_runtime_branch_repair_kind_json(|kind| {
        summary.o3_runtime.branch_repair_wrong_target_kind(kind)
    });
    let direction_only_kind = o3_runtime_branch_repair_kind_json(|kind| {
        summary.o3_runtime.branch_repair_direction_only_kind(kind)
    });
    format!(
        "{{\"targetless_mismatches\":{},\"wrong_targets\":{},\"direction_only_mismatches\":{},\"targetless_mismatch_kind\":{},\"wrong_target_kind\":{},\"direction_only_kind\":{}}}",
        summary.o3_runtime.branch_repair_targetless_mismatches(),
        summary.o3_runtime.branch_repair_wrong_targets(),
        summary.o3_runtime.branch_repair_direction_only_mismatches(),
        targetless_mismatch_kind,
        wrong_target_kind,
        direction_only_kind,
    )
}

impl Rem6CoreSummary {
    pub(crate) fn to_json(&self) -> String {
        let registers = self
            .registers
            .iter()
            .map(|(register, value)| format!("\"x{}\":\"0x{:x}\"", register, value))
            .collect::<Vec<_>>()
            .join(",");
        let checker = self
            .checker
            .as_ref()
            .map(|checker| {
                format!(
                    ",\"checker\":{{\"checked_instructions\":{},\"mismatches\":{}}}",
                    checker.checked_instructions, checker.mismatches
                )
            })
            .unwrap_or_default();
        let o3_runtime = if self.o3_runtime.has_activity() {
            let fu_latency_classes = o3_runtime_fu_latency_class_json(self);
            let lsq_data_latency = o3_runtime_lsq_data_latency_json(self);
            let lsq_operations = o3_runtime_lsq_operation_json(self);
            let lsq_orderings = o3_runtime_lsq_ordering_json(self);
            let branch_event = o3_runtime_branch_event_json(self);
            let branch_repair = o3_runtime_branch_repair_json(self);
            let iq = o3_runtime_iq_json(self);
            let iew = o3_runtime_iew_json(self);
            let commit = o3_runtime_commit_json(self);
            let rob = o3_runtime_rob_json(self);
            let rename = o3_runtime_rename_json(self);
            let lsq = o3_runtime_lsq_json(self);
            format!(
                ",\"o3_runtime\":{{\"instructions\":{},\"rob_allocations\":{},\"rob_commits\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"lsq_load_bytes\":{},\"lsq_store_bytes\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"store_load_forwarding_suppressed\":{},\"rob\":{},\"rename\":{},\"lsq\":{},\"iq\":{},\"iew\":{},\"commit\":{},\"branch_event\":{},\"branch_repair\":{},\"iew_predicted_taken_incorrect\":{},\"iew_predicted_not_taken_incorrect\":{},\"iew_producer_insts\":{},\"iew_consumer_insts\":{},\"iq_branch_insts_issued\":{},\"fu_latency_instructions\":{},\"fu_latency_cycles\":{},{},{},{},{},\"lsq_store_conditional_failures\":{},\"max_rob_occupancy\":{},\"max_lsq_occupancy\":{},\"rename_map_entries\":{}}}",
                self.o3_runtime.instructions(),
                self.o3_runtime.rob_allocations(),
                self.o3_runtime.rob_commits(),
                self.o3_runtime.rename_writes(),
                self.o3_runtime.lsq_loads(),
                self.o3_runtime.lsq_stores(),
                self.o3_runtime.lsq_load_bytes(),
                self.o3_runtime.lsq_store_bytes(),
                self.o3_runtime.lsq_store_to_load_forwarding_candidates(),
                self.o3_runtime.lsq_store_to_load_forwarding_matches(),
                self.o3_runtime.lsq_store_to_load_forwarding_suppressed(),
                rob,
                rename,
                lsq,
                iq,
                iew,
                commit,
                branch_event,
                branch_repair,
                self.o3_runtime.iew_predicted_taken_incorrect(),
                self.o3_runtime.iew_predicted_not_taken_incorrect(),
                self.o3_runtime.iew_producer_insts(),
                self.o3_runtime.iew_consumer_insts(),
                self.o3_runtime.iq_branch_insts_issued(),
                self.o3_runtime.fu_latency_instructions(),
                self.o3_runtime.fu_latency_cycles(),
                fu_latency_classes,
                lsq_data_latency,
                lsq_operations,
                lsq_orderings,
                self.o3_runtime.lsq_store_conditional_failures(),
                self.o3_runtime.max_rob_occupancy(),
                self.o3_runtime.max_lsq_occupancy(),
                self.o3_runtime.rename_map_entries(),
            )
        } else {
            String::new()
        };
        let btb_mispredict_due_to_btb_miss =
            branch_target_kind_counts_json(self.branch_target_buffer_mispredict_due_to_btb_miss);
        let lookups = branch_target_kind_counts_json(self.branch_predictor_lookups);
        let squashes = branch_target_kind_counts_json_with_total(
            self.branch_predictor_squashes,
            self.branch_predictor_squashes_total,
        );
        let target_provider =
            branch_target_provider_counts_json(self.branch_predictor_target_provider);
        let committed = branch_target_kind_counts_json(self.branch_predictor_committed);
        let mispredicted = branch_target_kind_counts_json(self.branch_predictor_mispredicted);
        let corrected = branch_target_kind_counts_json(self.branch_predictor_corrected);
        let target_wrong = branch_target_kind_counts_json(self.branch_predictor_target_wrong);
        let mispredict_due_to_predictor =
            branch_target_kind_counts_json(self.branch_predictor_mispredict_due_to_predictor);
        let ras = format!(
            "{{\"pushes\":{},\"pops\":{},\"squashes\":{},\"used\":{},\"correct\":{},\"incorrect\":{}}}",
            self.branch_predictor_ras.pushes(),
            self.branch_predictor_ras.pops(),
            self.branch_predictor_ras.squashes(),
            self.branch_predictor_ras.used(),
            self.branch_predictor_ras.correct(),
            self.branch_predictor_ras.incorrect()
        );
        let gshare = branch_predictor_counter_json(
            self.branch_predictor_gshare.lookups,
            self.branch_predictor_gshare.history_updates,
            self.branch_predictor_gshare.updates,
            self.branch_predictor_gshare.squashes,
        );
        let bimode = branch_predictor_counter_json(
            self.branch_predictor_bimode.lookups,
            self.branch_predictor_bimode.history_updates,
            self.branch_predictor_bimode.updates,
            self.branch_predictor_bimode.squashes,
        );
        let tage_sc_l = format!(
            "{{\"lookups\":{},\"history_updates\":{},\"updates\":{},\"repairs\":{},\"selected_rollbacks\":{}}}",
            self.branch_predictor_tage_sc_l.lookups,
            self.branch_predictor_tage_sc_l.history_updates,
            self.branch_predictor_tage_sc_l.updates,
            self.branch_predictor_tage_sc_l.repairs,
            self.branch_predictor_tage_sc_l.selected_rollbacks
        );
        let multiperspective_perceptron = format!(
            "{{\"lookups\":{},\"updates\":{},\"selected_rollbacks\":{}}}",
            self.branch_predictor_multiperspective_perceptron.lookups,
            self.branch_predictor_multiperspective_perceptron.updates,
            self.branch_predictor_multiperspective_perceptron
                .selected_rollbacks
        );
        let stage_advanced =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_advanced);
        let stage_advanced_cycles =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_advanced_cycles);
        let stage_retired =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_retired);
        let stage_retired_cycles =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_retired_cycles);
        let stage_resource_blocked_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_resource_blocked_cycles,
        );
        let stall_cause_fetch_wait = in_order_pipeline_stall_cause_stage_summary_json(
            self.in_order_pipeline_fetch_wait_stage_resource_blocked,
            self.in_order_pipeline_fetch_wait_stage_resource_blocked_cycles,
            self.in_order_pipeline_fetch_wait_stage_ordering_blocked,
            self.in_order_pipeline_fetch_wait_stage_ordering_blocked_cycles,
        );
        let stall_cause_data_wait = in_order_pipeline_stall_cause_stage_summary_json(
            self.in_order_pipeline_data_wait_stage_resource_blocked,
            self.in_order_pipeline_data_wait_stage_resource_blocked_cycles,
            self.in_order_pipeline_data_wait_stage_ordering_blocked,
            self.in_order_pipeline_data_wait_stage_ordering_blocked_cycles,
        );
        let stall_cause_execute_wait = in_order_pipeline_stall_cause_stage_summary_json(
            self.in_order_pipeline_execute_wait_stage_resource_blocked,
            self.in_order_pipeline_execute_wait_stage_resource_blocked_cycles,
            self.in_order_pipeline_execute_wait_stage_ordering_blocked,
            self.in_order_pipeline_execute_wait_stage_ordering_blocked_cycles,
        );
        let stage_ordering_blocked_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_ordering_blocked_cycles,
        );
        let stage_flushed_cycles =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_flushed_cycles);
        let stage_branch_prediction_flushed = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_branch_prediction_flushed,
        );
        let stage_branch_prediction_flushed_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_branch_prediction_flushed_cycles,
        );
        let cause_branch_prediction = in_order_pipeline_flush_redirect_cause_stage_summary_json(
            self.in_order_pipeline_stage_branch_prediction_flushed,
            self.in_order_pipeline_stage_branch_prediction_flushed_cycles,
        );
        let stage_trap_redirect_flushed = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_trap_redirect_flushed,
        );
        let stage_trap_redirect_flushed_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_trap_redirect_flushed_cycles,
        );
        let cause_trap_redirect = in_order_pipeline_flush_redirect_cause_stage_summary_json(
            self.in_order_pipeline_stage_trap_redirect_flushed,
            self.in_order_pipeline_stage_trap_redirect_flushed_cycles,
        );
        let stage_interrupt_redirect_flushed = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_interrupt_redirect_flushed,
        );
        let stage_interrupt_redirect_flushed_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_interrupt_redirect_flushed_cycles,
        );
        let cause_interrupt_redirect = in_order_pipeline_flush_redirect_cause_stage_summary_json(
            self.in_order_pipeline_stage_interrupt_redirect_flushed,
            self.in_order_pipeline_stage_interrupt_redirect_flushed_cycles,
        );
        format!(
            "{{\"cpu\":{},\"pc\":\"0x{:x}\",\"committed_instructions\":{},\"in_order_pipeline\":{{\"cycles\":{},\"in_flight\":{},\"stage_widths\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_in_flight\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_max_in_flight\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_occupied_cycles\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_advanced\":{},\"stage_advanced_cycles\":{},\"stage_retired\":{},\"stage_retired_cycles\":{},\"stage_resource_blocked\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_resource_blocked_cycles\":{},\"stall_cause\":{{\"fetch_wait\":{},\"data_wait\":{},\"execute_wait\":{}}},\"stage_ordering_blocked\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_ordering_blocked_cycles\":{},\"stage_flushed\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_flushed_cycles\":{},\"stage_branch_prediction_flushed\":{},\"stage_branch_prediction_flushed_cycles\":{},\"stage_trap_redirect_flushed\":{},\"stage_trap_redirect_flushed_cycles\":{},\"stage_interrupt_redirect_flushed\":{},\"stage_interrupt_redirect_flushed_cycles\":{},\"flush_cause\":{{\"branch_prediction\":{},\"trap_redirect\":{},\"interrupt_redirect\":{}}},\"redirect_cause\":{{\"branch_prediction\":{},\"trap_redirect\":{},\"interrupt_redirect\":{}}},\"retired\":{},\"advanced\":{},\"flushed\":{},\"flush_cycles\":{},\"resource_blocked\":{},\"ordering_blocked\":{},\"stall_cycles\":{},\"fetch_wait_cycles\":{},\"data_wait_cycles\":{},\"execute_wait_cycles\":{},\"branch_predictions\":{},\"branch_mispredictions\":{},\"conditional_branch_predictions\":{},\"conditional_branch_predicted_taken\":{},\"conditional_branch_mispredictions\":{},\"branch_prediction_flushes\":{},\"branch_prediction_flush_cycles\":{},\"redirects\":{},\"branch_prediction_redirects\":{},\"interrupt_redirects\":{},\"interrupt_redirect_flushes\":{},\"interrupt_redirect_flush_cycles\":{},\"trap_redirects\":{},\"trap_redirect_flushes\":{},\"trap_redirect_flush_cycles\":{},\"branch_speculation_predictions\":{},\"branch_speculation_repairs\":{},\"branch_speculation_removed_youngers\":{},\"branch_speculation_max_pending\":{}}},\"branch_predictor\":{{\"btb\":{{\"lookups\":{},\"hits\":{},\"misses\":{},\"updates\":{},\"evictions\":{},\"mispredictions\":{},\"predicted_taken_misses\":{},\"mispredict_due_to_btb_miss\":{}}},\"lookups\":{},\"squashes\":{},\"target_provider\":{},\"indirect_hits\":{},\"indirect_mispredicted\":{},\"ras\":{},\"committed\":{},\"mispredicted\":{},\"corrected\":{},\"target_wrong\":{},\"mispredict_due_to_predictor\":{},\"gshare\":{},\"bimode\":{},\"tournament\":{{\"lookups\":{},\"history_updates\":{},\"updates\":{},\"squashes\":{},\"local_predictions\":{},\"global_predictions\":{}}},\"tage_sc_l\":{},\"multiperspective_perceptron\":{}}},\"data_loads\":{},\"data_stores\":{},\"data_atomics\":{},\"data_load_bytes\":{},\"data_store_bytes\":{},\"data_atomic_bytes\":{}{}{},\"registers\":{{{}}}}}",
            self.cpu,
            self.pc,
            self.committed_instructions,
            self.in_order_pipeline_cycles,
            self.in_order_pipeline_in_flight,
            self.in_order_pipeline_stage_widths.fetch1,
            self.in_order_pipeline_stage_widths.fetch2,
            self.in_order_pipeline_stage_widths.decode,
            self.in_order_pipeline_stage_widths.execute,
            self.in_order_pipeline_stage_widths.commit,
            self.in_order_pipeline_stage_in_flight.fetch1,
            self.in_order_pipeline_stage_in_flight.fetch2,
            self.in_order_pipeline_stage_in_flight.decode,
            self.in_order_pipeline_stage_in_flight.execute,
            self.in_order_pipeline_stage_in_flight.commit,
            self.in_order_pipeline_stage_max_in_flight.fetch1,
            self.in_order_pipeline_stage_max_in_flight.fetch2,
            self.in_order_pipeline_stage_max_in_flight.decode,
            self.in_order_pipeline_stage_max_in_flight.execute,
            self.in_order_pipeline_stage_max_in_flight.commit,
            self.in_order_pipeline_stage_occupied_cycles.fetch1,
            self.in_order_pipeline_stage_occupied_cycles.fetch2,
            self.in_order_pipeline_stage_occupied_cycles.decode,
            self.in_order_pipeline_stage_occupied_cycles.execute,
            self.in_order_pipeline_stage_occupied_cycles.commit,
            stage_advanced,
            stage_advanced_cycles,
            stage_retired,
            stage_retired_cycles,
            self.in_order_pipeline_stage_resource_blocked.fetch1,
            self.in_order_pipeline_stage_resource_blocked.fetch2,
            self.in_order_pipeline_stage_resource_blocked.decode,
            self.in_order_pipeline_stage_resource_blocked.execute,
            self.in_order_pipeline_stage_resource_blocked.commit,
            stage_resource_blocked_cycles,
            stall_cause_fetch_wait,
            stall_cause_data_wait,
            stall_cause_execute_wait,
            self.in_order_pipeline_stage_ordering_blocked.fetch1,
            self.in_order_pipeline_stage_ordering_blocked.fetch2,
            self.in_order_pipeline_stage_ordering_blocked.decode,
            self.in_order_pipeline_stage_ordering_blocked.execute,
            self.in_order_pipeline_stage_ordering_blocked.commit,
            stage_ordering_blocked_cycles,
            self.in_order_pipeline_stage_flushed.fetch1,
            self.in_order_pipeline_stage_flushed.fetch2,
            self.in_order_pipeline_stage_flushed.decode,
            self.in_order_pipeline_stage_flushed.execute,
            self.in_order_pipeline_stage_flushed.commit,
            stage_flushed_cycles,
            stage_branch_prediction_flushed,
            stage_branch_prediction_flushed_cycles,
            stage_trap_redirect_flushed,
            stage_trap_redirect_flushed_cycles,
            stage_interrupt_redirect_flushed,
            stage_interrupt_redirect_flushed_cycles,
            &cause_branch_prediction,
            &cause_trap_redirect,
            &cause_interrupt_redirect,
            &cause_branch_prediction,
            &cause_trap_redirect,
            &cause_interrupt_redirect,
            self.in_order_pipeline_retired,
            self.in_order_pipeline_advanced,
            self.in_order_pipeline_flushed,
            self.in_order_pipeline_flush_cycles,
            self.in_order_pipeline_resource_blocked,
            self.in_order_pipeline_ordering_blocked,
            self.in_order_pipeline_stall_cycles,
            self.in_order_pipeline_fetch_wait_cycles,
            self.in_order_pipeline_data_wait_cycles,
            self.in_order_pipeline_execute_wait_cycles,
            self.in_order_pipeline_branch_predictions,
            self.in_order_pipeline_branch_mispredictions,
            self.in_order_pipeline_conditional_branch_predictions,
            self.in_order_pipeline_conditional_branch_predicted_taken,
            self.in_order_pipeline_conditional_branch_mispredictions,
            self.in_order_pipeline_branch_prediction_flushes,
            self.in_order_pipeline_branch_prediction_flush_cycles,
            self.in_order_pipeline_redirects,
            self.in_order_pipeline_branch_prediction_redirects,
            self.in_order_pipeline_interrupt_redirects,
            self.in_order_pipeline_interrupt_redirect_flushes,
            self.in_order_pipeline_interrupt_redirect_flush_cycles,
            self.in_order_pipeline_trap_redirects,
            self.in_order_pipeline_trap_redirect_flushes,
            self.in_order_pipeline_trap_redirect_flush_cycles,
            self.in_order_pipeline_branch_speculation_predictions,
            self.in_order_pipeline_branch_speculation_repairs,
            self.in_order_pipeline_branch_speculation_removed_youngers,
            self.in_order_pipeline_branch_speculation_max_pending,
            self.branch_target_buffer_lookups,
            self.branch_target_buffer_hits,
            self.branch_target_buffer_misses,
            self.branch_target_buffer_updates,
            self.branch_target_buffer_evictions,
            self.branch_target_buffer_mispredictions,
            self.branch_target_buffer_predicted_taken_misses,
            btb_mispredict_due_to_btb_miss,
            lookups,
            squashes,
            target_provider,
            self.branch_predictor_indirect_hits,
            self.branch_predictor_indirect_mispredicted,
            ras,
            committed,
            mispredicted,
            corrected,
            target_wrong,
            mispredict_due_to_predictor,
            gshare,
            bimode,
            self.branch_predictor_tournament.lookups,
            self.branch_predictor_tournament.history_updates,
            self.branch_predictor_tournament.updates,
            self.branch_predictor_tournament.squashes,
            self.tournament_local_predictions,
            self.tournament_global_predictions,
            tage_sc_l,
            multiperspective_perceptron,
            self.data_loads,
            self.data_stores,
            self.data_atomics,
            self.data_load_bytes,
            self.data_store_bytes,
            self.data_atomic_bytes,
            o3_runtime,
            checker,
            registers
        )
    }
}
