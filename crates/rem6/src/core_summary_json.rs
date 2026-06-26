use rem6_cpu::{BranchTargetKind, BranchTargetKindCounts};

use super::Rem6CoreSummary;

fn branch_target_kind_counts_json(counts: BranchTargetKindCounts) -> String {
    let mut fields = BranchTargetKind::ALL
        .into_iter()
        .map(|kind| format!("\"{}\":{}", kind.canonical_stat_name(), counts.value(kind)))
        .collect::<Vec<_>>();
    fields.push(format!("\"total\":{}", counts.total()));
    format!("{{{}}}", fields.join(","))
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
        let committed = branch_target_kind_counts_json(self.branch_predictor_committed);
        let mispredicted = branch_target_kind_counts_json(self.branch_predictor_mispredicted);
        let mispredict_due_to_predictor =
            branch_target_kind_counts_json(self.branch_predictor_mispredict_due_to_predictor);
        format!(
            "{{\"cpu\":{},\"pc\":\"0x{:x}\",\"committed_instructions\":{},\"in_order_pipeline\":{{\"cycles\":{},\"in_flight\":{},\"stage_widths\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_in_flight\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_max_in_flight\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_occupied_cycles\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_resource_blocked\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_ordering_blocked\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"retired\":{},\"advanced\":{},\"flushed\":{},\"resource_blocked\":{},\"ordering_blocked\":{},\"stall_cycles\":{},\"fetch_wait_cycles\":{},\"data_wait_cycles\":{},\"branch_predictions\":{},\"branch_mispredictions\":{},\"conditional_branch_predictions\":{},\"conditional_branch_predicted_taken\":{},\"conditional_branch_mispredictions\":{},\"branch_prediction_flushes\":{},\"redirects\":{},\"branch_speculation_predictions\":{},\"branch_speculation_repairs\":{},\"branch_speculation_removed_youngers\":{},\"branch_speculation_max_pending\":{}}},\"branch_predictor\":{{\"btb\":{{\"lookups\":{},\"hits\":{},\"misses\":{},\"updates\":{},\"evictions\":{},\"mispredictions\":{},\"predicted_taken_misses\":{},\"mispredict_due_to_btb_miss\":{}}},\"lookups\":{},\"committed\":{},\"mispredicted\":{},\"mispredict_due_to_predictor\":{},\"tournament\":{{\"local_predictions\":{},\"global_predictions\":{}}}}},\"data_loads\":{},\"data_stores\":{},\"data_atomics\":{},\"data_load_bytes\":{},\"data_store_bytes\":{},\"data_atomic_bytes\":{}{},\"registers\":{{{}}}}}",
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
            self.in_order_pipeline_stage_ordering_blocked.fetch1,
            self.in_order_pipeline_stage_ordering_blocked.fetch2,
            self.in_order_pipeline_stage_ordering_blocked.decode,
            self.in_order_pipeline_stage_ordering_blocked.execute,
            self.in_order_pipeline_stage_ordering_blocked.commit,
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
            committed,
            mispredicted,
            mispredict_due_to_predictor,
            self.tournament_local_predictions,
            self.tournament_global_predictions,
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
