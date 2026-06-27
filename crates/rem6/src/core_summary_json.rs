use rem6_cpu::{
    BranchTargetKind, BranchTargetKindCounts, BranchTargetProvider, BranchTargetProviderCounts,
};

use super::{Rem6CoreSummary, Rem6InOrderPipelineStageSummary};

fn branch_target_kind_counts_json(counts: BranchTargetKindCounts) -> String {
    let mut fields = BranchTargetKind::ALL
        .into_iter()
        .map(|kind| format!("\"{}\":{}", kind.canonical_stat_name(), counts.value(kind)))
        .collect::<Vec<_>>();
    fields.push(format!("\"total\":{}", counts.total()));
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
        let btb_mispredict_due_to_btb_miss =
            branch_target_kind_counts_json(self.branch_target_buffer_mispredict_due_to_btb_miss);
        let lookups = branch_target_kind_counts_json(self.branch_predictor_lookups);
        let target_provider =
            branch_target_provider_counts_json(self.branch_predictor_target_provider);
        let committed = branch_target_kind_counts_json(self.branch_predictor_committed);
        let mispredicted = branch_target_kind_counts_json(self.branch_predictor_mispredicted);
        let corrected = branch_target_kind_counts_json(self.branch_predictor_corrected);
        let target_wrong = branch_target_kind_counts_json(self.branch_predictor_target_wrong);
        let mispredict_due_to_predictor =
            branch_target_kind_counts_json(self.branch_predictor_mispredict_due_to_predictor);
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
        let stage_resource_blocked_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_resource_blocked_cycles,
        );
        let stage_ordering_blocked_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_ordering_blocked_cycles,
        );
        let stage_branch_prediction_flushed = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_branch_prediction_flushed,
        );
        format!(
            "{{\"cpu\":{},\"pc\":\"0x{:x}\",\"committed_instructions\":{},\"in_order_pipeline\":{{\"cycles\":{},\"in_flight\":{},\"stage_widths\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_in_flight\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_max_in_flight\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_occupied_cycles\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_resource_blocked\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_resource_blocked_cycles\":{},\"stage_ordering_blocked\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_ordering_blocked_cycles\":{},\"stage_flushed\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_branch_prediction_flushed\":{},\"retired\":{},\"advanced\":{},\"flushed\":{},\"resource_blocked\":{},\"ordering_blocked\":{},\"stall_cycles\":{},\"fetch_wait_cycles\":{},\"data_wait_cycles\":{},\"branch_predictions\":{},\"branch_mispredictions\":{},\"conditional_branch_predictions\":{},\"conditional_branch_predicted_taken\":{},\"conditional_branch_mispredictions\":{},\"branch_prediction_flushes\":{},\"redirects\":{},\"branch_speculation_predictions\":{},\"branch_speculation_repairs\":{},\"branch_speculation_removed_youngers\":{},\"branch_speculation_max_pending\":{}}},\"branch_predictor\":{{\"btb\":{{\"lookups\":{},\"hits\":{},\"misses\":{},\"updates\":{},\"evictions\":{},\"mispredictions\":{},\"predicted_taken_misses\":{},\"mispredict_due_to_btb_miss\":{}}},\"lookups\":{},\"target_provider\":{},\"committed\":{},\"mispredicted\":{},\"corrected\":{},\"target_wrong\":{},\"mispredict_due_to_predictor\":{},\"gshare\":{},\"bimode\":{},\"tournament\":{{\"lookups\":{},\"history_updates\":{},\"updates\":{},\"squashes\":{},\"local_predictions\":{},\"global_predictions\":{}}},\"tage_sc_l\":{},\"multiperspective_perceptron\":{}}},\"data_loads\":{},\"data_stores\":{},\"data_atomics\":{},\"data_load_bytes\":{},\"data_store_bytes\":{},\"data_atomic_bytes\":{}{},\"registers\":{{{}}}}}",
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
            self.in_order_pipeline_stage_resource_blocked.fetch1,
            self.in_order_pipeline_stage_resource_blocked.fetch2,
            self.in_order_pipeline_stage_resource_blocked.decode,
            self.in_order_pipeline_stage_resource_blocked.execute,
            self.in_order_pipeline_stage_resource_blocked.commit,
            stage_resource_blocked_cycles,
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
            stage_branch_prediction_flushed,
            self.in_order_pipeline_retired,
            self.in_order_pipeline_advanced,
            self.in_order_pipeline_flushed,
            self.in_order_pipeline_resource_blocked,
            self.in_order_pipeline_ordering_blocked,
            self.in_order_pipeline_stall_cycles,
            self.in_order_pipeline_fetch_wait_cycles,
            self.in_order_pipeline_data_wait_cycles,
            self.in_order_pipeline_branch_predictions,
            self.in_order_pipeline_branch_mispredictions,
            self.in_order_pipeline_conditional_branch_predictions,
            self.in_order_pipeline_conditional_branch_predicted_taken,
            self.in_order_pipeline_conditional_branch_mispredictions,
            self.in_order_pipeline_branch_prediction_flushes,
            self.in_order_pipeline_redirects,
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
            target_provider,
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
            checker,
            registers
        )
    }
}
