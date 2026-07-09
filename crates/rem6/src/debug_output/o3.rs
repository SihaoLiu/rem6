use rem6_cpu::{
    BranchTargetKind, CpuId, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeStats, O3RuntimeTraceRecord, RiscvCluster,
};

use crate::{
    formatting::json_escape, Rem6HostCheckpointSummary, Rem6HostExecutionModeSummary,
    Rem6HostStatsResetSummary,
};

#[path = "o3_branch_direction_mismatch.rs"]
mod o3_branch_direction_mismatch;
#[path = "o3_branch_repair.rs"]
mod o3_branch_repair;
#[path = "o3_branch_stats.rs"]
mod o3_branch_stats;
#[path = "o3_branch_target_mismatch.rs"]
mod o3_branch_target_mismatch;
#[path = "o3_checkpoint_restore_json.rs"]
mod o3_checkpoint_restore_json;
#[path = "o3_event_iew.rs"]
mod o3_event_iew;
#[path = "o3_event_inst_type_stats.rs"]
mod o3_event_inst_type_stats;
#[path = "o3_event_json.rs"]
mod o3_event_json;
#[path = "o3_event_summary_json.rs"]
mod o3_event_summary_json;
#[path = "o3_execution_mode_stats.rs"]
mod o3_execution_mode_stats;
#[path = "o3_fu_latency_stats.rs"]
mod o3_fu_latency_stats;
#[path = "o3_lsq_json.rs"]
mod o3_lsq_json;
#[path = "o3_summary_json.rs"]
mod o3_summary_json;
#[path = "o3_trace_totals.rs"]
mod o3_trace_totals;

pub(crate) use o3_branch_direction_mismatch::o3_branch_direction_mismatch_to_json;
use o3_branch_direction_mismatch::Rem6O3BranchDirectionMismatchTotals;
use o3_branch_repair::{
    o3_branch_repair_kind, o3_branch_repair_to_json, o3_branch_targetless_mismatch,
    o3_branch_wrong_target, Rem6O3BranchRepairTotals,
};
use o3_branch_stats::{
    o3_branch_event_json, o3_branch_kind_stat_suffix, o3_branch_link_write_kind_stat_suffix,
    o3_branch_misprediction_kind_stat_suffix, o3_branch_not_taken_kind_stat_suffix,
    o3_branch_predicted_not_taken_kind_stat_suffix, o3_branch_predicted_taken_kind_stat_suffix,
    o3_branch_predicted_target_kind_stat_suffix, o3_branch_predicted_target_match_kind_stat_suffix,
    o3_branch_predicted_target_mismatch_kind_stat_suffix,
    o3_branch_resolved_target_kind_stat_suffix, o3_branch_squash_kind_stat_suffix,
    o3_branch_squashed_target_kind_stat_suffix,
    o3_branch_squashed_target_link_write_kind_stat_suffix,
    o3_branch_squashed_target_without_link_write_kind_stat_suffix,
    o3_branch_taken_kind_stat_suffix, o3_branch_targetless_mismatch_kind_stat_suffix,
    o3_branch_targetless_mismatch_squashed_target_kind_stat_suffix,
    o3_branch_targetless_mismatch_squashed_target_without_link_write_kind_stat_suffix,
    o3_branch_targetless_mismatch_without_link_write_kind_stat_suffix,
    o3_branch_wrong_target_kind_stat_suffix, o3_branch_wrong_target_link_write_kind_stat_suffix,
    o3_branch_wrong_target_squashed_target_kind_stat_suffix,
    o3_branch_wrong_target_squashed_target_link_write_kind_stat_suffix,
    o3_branch_wrong_target_squashed_target_without_link_write_kind_stat_suffix,
    o3_branch_wrong_target_without_link_write_kind_stat_suffix, push_o3_branch_kind_count_stats,
};
pub(crate) use o3_branch_target_mismatch::o3_branch_target_mismatch_to_json;
use o3_checkpoint_restore_json::{
    o3_checkpoint_restore_to_json, o3_trace_checkpoint_restore_authority_stats,
    o3_trace_checkpoint_restore_component_stats, Rem6O3CheckpointRestoreAuthorityTotals,
    Rem6O3CheckpointRestoreScope,
};
pub(super) use o3_checkpoint_restore_json::{
    o3_trace_cpu_checkpoint_restore_authority_stats,
    o3_trace_cpu_checkpoint_restore_component_stats,
};
use o3_event_iew::Rem6O3EventIewTotals;
use o3_event_inst_type_stats::{
    o3_event_commit_committed_inst_type_stat_suffix, o3_event_iq_issued_inst_type_stat_suffix,
};
use o3_event_json::o3_event_to_json;
pub(crate) use o3_event_summary_json::o3_event_summary_to_json;
pub(super) use o3_execution_mode_stats::o3_trace_cpu_execution_mode_authority_stats;
pub(crate) use o3_execution_mode_stats::Rem6O3ExecutionModeAuthorityStat;
use o3_execution_mode_stats::{
    o3_trace_execution_mode_authority_stats, o3_trace_execution_mode_authority_to_json,
    Rem6O3ExecutionModeTraceTotals,
};
use o3_fu_latency_stats::{Rem6O3FuLatencyClassTotals, REM6_O3_FU_LATENCY_CLASS_STATS};
use o3_lsq_json::o3_lsq_to_json;
use o3_summary_json::{
    o3_commit_to_json, o3_fu_latency_class_to_json, o3_iew_to_json, o3_iq_to_json,
    o3_rename_to_json, o3_rob_to_json,
};
use o3_trace_totals::Rem6O3TraceTotals;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6O3TraceRecord {
    cpu: u32,
    target: String,
    execution_mode: Option<&'static str>,
    stats_epoch: u64,
    stats_reset_tick: u64,
    checkpoint_restore: Option<Rem6O3CheckpointRestoreScope>,
    stats: O3RuntimeStats,
    events: Vec<O3RuntimeTraceRecord>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6O3TraceStat {
    suffix: &'static str,
    unit: &'static str,
    value: u64,
}

impl Rem6O3TraceRecord {
    fn new(
        cpu: CpuId,
        target: String,
        execution_mode: Option<&'static str>,
        stats_epoch: u64,
        stats_reset_tick: u64,
        checkpoint_restore: Option<Rem6O3CheckpointRestoreScope>,
        stats: O3RuntimeStats,
        events: Vec<O3RuntimeTraceRecord>,
    ) -> Self {
        Self {
            cpu: cpu.get(),
            target,
            execution_mode,
            stats_epoch,
            stats_reset_tick,
            checkpoint_restore,
            stats,
            events,
        }
    }

    pub(super) const fn cpu(&self) -> u32 {
        self.cpu
    }

    pub(super) fn target(&self) -> &str {
        &self.target
    }

    pub(super) fn stats(&self) -> O3RuntimeStats {
        self.stats
    }

    pub(super) const fn stats_epoch(&self) -> u64 {
        self.stats_epoch
    }

    pub(super) const fn stats_reset_tick(&self) -> u64 {
        self.stats_reset_tick
    }

    pub(super) const fn execution_mode(&self) -> Option<&'static str> {
        self.execution_mode
    }

    fn checkpoint_restore(&self) -> Option<&Rem6O3CheckpointRestoreScope> {
        self.checkpoint_restore.as_ref()
    }

    pub(super) fn events(&self) -> &[O3RuntimeTraceRecord] {
        &self.events
    }

    pub(super) fn to_json(&self) -> String {
        let rob = o3_rob_to_json(self.stats);
        let rename = o3_rename_to_json(self.stats);
        let lsq = o3_lsq_to_json(self.stats);
        let iq = o3_iq_to_json(self.stats);
        let iew = o3_iew_to_json(self.stats);
        let commit = o3_commit_to_json(self.stats);
        let fu_latency_class = o3_fu_latency_class_to_json(self.stats);
        let checkpoint_restore = o3_checkpoint_restore_to_json(self.checkpoint_restore.as_ref());
        let branch_event = o3_branch_event_json(self.stats);
        let branch_repair = o3_branch_repair_to_json(self.stats);
        let branch_direction_mismatch = o3_branch_direction_mismatch_to_json(&self.events);
        let branch_target_mismatch = o3_branch_target_mismatch_to_json(&self.events);
        let event_summary = o3_event_summary_to_json(&self.events);
        let events = self
            .events
            .iter()
            .map(o3_event_to_json)
            .collect::<Vec<_>>()
            .join(",");
        let execution_mode = self.execution_mode.map_or_else(
            || "null".to_string(),
            |mode| format!("\"{}\"", json_escape(mode)),
        );
        let execution_mode_authority =
            o3_trace_execution_mode_authority_to_json(&self.target, self.execution_mode);
        let checkpoint_restore_label = self.checkpoint_restore.as_ref().map_or_else(
            || "null".to_string(),
            |restore| format!("\"{}\"", json_escape(&restore.label)),
        );
        let checkpoint_restore_labels = self.checkpoint_restore.as_ref().map_or_else(
            || "[]".to_string(),
            |restore| {
                format!(
                    "[{}]",
                    restore
                        .labels
                        .iter()
                        .map(|label| format!("\"{}\"", json_escape(label)))
                        .collect::<Vec<_>>()
                        .join(",")
                )
            },
        );
        let (
            checkpoint_restore_count,
            checkpoint_restore_tick,
            checkpoint_restore_manifest_tick,
            checkpoint_restore_payload_bytes,
        ) = self
            .checkpoint_restore
            .as_ref()
            .map_or((0, 0, 0, 0), |restore| {
                (
                    restore.count,
                    restore.tick,
                    restore.manifest_tick,
                    restore.payload_bytes,
                )
            });
        format!(
            "{{\"cpu\":{},\"target\":\"{}\",\"execution_mode\":{},\"execution_mode_authority\":{},\"stats_epoch\":{},\"stats_reset_tick\":{},\"checkpoint_restore_count\":{},\"checkpoint_restore_labels\":{},\"checkpoint_restore_label\":{},\"checkpoint_restore_tick\":{},\"checkpoint_restore_manifest_tick\":{},\"checkpoint_restore_payload_bytes\":{},\"checkpoint_restore\":{},\"instructions\":{},\"rob_allocations\":{},\"rob_commits\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"lsq_load_bytes\":{},\"lsq_store_bytes\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"store_load_forwarding_suppressed\":{},\"store_load_forwarding_address_mismatches\":{},\"store_load_forwarding_byte_mismatches\":{},\"fu_latency_instructions\":{},\"fu_latency_cycles\":{},\"fu_integer_mul_instructions\":{},\"fu_integer_mul_latency_cycles\":{},\"fu_integer_div_instructions\":{},\"fu_integer_div_latency_cycles\":{},\"fu_latency_class\":{},\"max_rob_occupancy\":{},\"max_lsq_occupancy\":{},\"rename_map_entries\":{},\"rob\":{},\"rename\":{},\"lsq\":{},\"iq\":{},\"iew\":{},\"commit\":{},\"branch_event\":{},\"branch_repair\":{},\"branch_direction_mismatch\":{},\"branch_target_mismatch\":{},\"event_summary\":{},\"events\":[{}]}}",
            self.cpu,
            json_escape(&self.target),
            execution_mode,
            execution_mode_authority,
            self.stats_epoch,
            self.stats_reset_tick,
            checkpoint_restore_count,
            checkpoint_restore_labels,
            checkpoint_restore_label,
            checkpoint_restore_tick,
            checkpoint_restore_manifest_tick,
            checkpoint_restore_payload_bytes,
            checkpoint_restore,
            self.stats.instructions(),
            self.stats.rob_allocations(),
            self.stats.rob_commits(),
            self.stats.rename_writes(),
            self.stats.lsq_loads(),
            self.stats.lsq_stores(),
            self.stats.lsq_load_bytes(),
            self.stats.lsq_store_bytes(),
            self.stats.lsq_store_to_load_forwarding_candidates(),
            self.stats.lsq_store_to_load_forwarding_matches(),
            self.stats.lsq_store_to_load_forwarding_suppressed(),
            self.stats.lsq_store_to_load_forwarding_address_mismatches(),
            self.stats.lsq_store_to_load_forwarding_byte_mismatches(),
            self.stats.fu_latency_instructions(),
            self.stats.fu_latency_cycles(),
            self.stats.fu_integer_mul_instructions(),
            self.stats.fu_integer_mul_latency_cycles(),
            self.stats.fu_integer_div_instructions(),
            self.stats.fu_integer_div_latency_cycles(),
            fu_latency_class,
            self.stats.max_rob_occupancy(),
            self.stats.max_lsq_occupancy(),
            self.stats.rename_map_entries(),
            rob,
            rename,
            lsq,
            iq,
            iew,
            commit,
            branch_event,
            branch_repair,
            branch_direction_mismatch,
            branch_target_mismatch,
            event_summary,
            events,
        )
    }
}

impl Rem6O3TraceStat {
    pub(crate) const fn suffix(self) -> &'static str {
        self.suffix
    }

    pub(crate) const fn unit(self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(self) -> u64 {
        self.value
    }
}

pub(super) fn o3_trace_records(
    cluster: &RiscvCluster,
    core_count: u32,
    execution_modes: &[Rem6HostExecutionModeSummary],
    latest_stats_reset: Option<&Rem6HostStatsResetSummary>,
    checkpoint_restores: &[Rem6HostCheckpointSummary],
) -> Vec<Rem6O3TraceRecord> {
    let mut records = Vec::new();
    let stats_epoch = latest_stats_reset.map_or(0, |reset| reset.epoch);
    let stats_reset_tick = latest_stats_reset.map_or(0, |reset| reset.tick);
    let checkpoint_restore = Rem6O3CheckpointRestoreScope::from_summaries(checkpoint_restores);
    for cpu_index in 0..core_count {
        let cpu = CpuId::new(cpu_index);
        let Ok(core) = cluster.core(cpu) else {
            continue;
        };
        let stats = core.o3_runtime_stats();
        let events = core.o3_runtime_trace_records();
        if stats.has_activity() || !events.is_empty() {
            let target = format!("cpu{}", cpu.get());
            let execution_mode = execution_modes
                .iter()
                .find(|mode| mode.target == target)
                .map(|mode| mode.mode);
            records.push(Rem6O3TraceRecord::new(
                cpu,
                target,
                execution_mode,
                stats_epoch,
                stats_reset_tick,
                checkpoint_restore.clone(),
                stats,
                events,
            ));
        }
    }
    records.sort_by_key(|record| record.cpu());
    records
}

pub(super) fn o3_trace_stats(records: &[Rem6O3TraceRecord]) -> Vec<Rem6O3TraceStat> {
    let mut totals = Rem6O3TraceTotals::default();
    for record in records {
        totals.add(record);
    }
    totals.stats()
}

pub(super) fn o3_trace_authority_stats(
    records: &[Rem6O3TraceRecord],
    stat_path_segment: impl Fn(&str) -> String,
) -> Vec<Rem6O3ExecutionModeAuthorityStat> {
    let mut stats = o3_trace_execution_mode_authority_stats(records, &stat_path_segment);
    stats.extend(o3_trace_checkpoint_restore_authority_stats(
        records,
        &stat_path_segment,
    ));
    stats.extend(o3_trace_checkpoint_restore_component_stats(
        records,
        stat_path_segment,
    ));
    stats
}

pub(super) fn o3_trace_cpu_stats(records: &[Rem6O3TraceRecord]) -> Vec<(u32, Rem6O3TraceStat)> {
    let mut cpu_totals = std::collections::BTreeMap::<u32, Rem6O3TraceTotals>::new();
    for record in records {
        cpu_totals.entry(record.cpu()).or_default().add(record);
    }
    cpu_totals
        .into_iter()
        .flat_map(|(cpu, totals)| totals.stats().into_iter().map(move |stat| (cpu, stat)))
        .collect()
}
