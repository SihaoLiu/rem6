use rem6_cpu::{
    BranchTargetKind, CpuId, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeStats, O3RuntimeTraceRecord, RiscvCluster,
};

use crate::{
    formatting::json_escape, Rem6HostCheckpointSummary, Rem6HostExecutionModeSummary,
    Rem6HostStatsResetSummary,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6O3CheckpointRestoreScope {
    label: String,
    tick: u64,
    manifest_tick: u64,
    payload_bytes: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6O3TraceTotals {
    records: u64,
    stats_epoch: u64,
    stats_reset_tick: u64,
    checkpoint_restores: u64,
    checkpoint_restore_tick: u64,
    checkpoint_restore_payload_bytes: u64,
    instructions: u64,
    rob_allocations: u64,
    rob_commits: u64,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    lsq_load_bytes: u64,
    lsq_store_bytes: u64,
    store_load_forwarding_candidates: u64,
    store_load_forwarding_matches: u64,
    fu_latency_instructions: u64,
    fu_latency_cycles: u64,
    fu_integer_mul_instructions: u64,
    fu_integer_mul_latency_cycles: u64,
    fu_integer_div_instructions: u64,
    fu_integer_div_latency_cycles: u64,
    max_rob_occupancy: u64,
    max_lsq_occupancy: u64,
    rename_map_entries: u64,
    event_records: u64,
    event_first_tick: Option<u64>,
    event_last_tick: Option<u64>,
    event_max_rob_occupancy: u64,
    event_max_lsq_occupancy: u64,
    event_max_rename_map_entries: u64,
    event_rob_allocations: u64,
    event_rob_commits: u64,
    event_rename_writes: u64,
    event_lsq_loads: u64,
    event_lsq_stores: u64,
    event_lsq_operation_load: u64,
    event_lsq_operation_store: u64,
    event_lsq_operation_load_reserved: u64,
    event_lsq_operation_store_conditional: u64,
    event_lsq_operation_atomic: u64,
    event_lsq_operation_float_load: u64,
    event_lsq_operation_float_store: u64,
    event_lsq_operation_vector_load: u64,
    event_lsq_operation_vector_store: u64,
    event_lsq_ordering_acquire: u64,
    event_lsq_ordering_release: u64,
    event_lsq_ordering_acquire_release: u64,
    event_branches: u64,
    event_branch_taken: u64,
    event_branch_not_taken: u64,
    event_branch_predicted_taken: u64,
    event_branch_predicted_target_matches: u64,
    event_branch_predicted_target_mismatches: u64,
    event_branch_targetless_mismatches: u64,
    event_branch_wrong_targets: u64,
    event_branch_mispredictions: u64,
    event_branch_squashes: u64,
    event_branch_link_writes: u64,
    event_branch_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_taken_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_not_taken_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_predicted_taken_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_predicted_target_match_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_predicted_target_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_targetless_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_wrong_target_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_resolved_target_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_link_write_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_misprediction_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_squash_kinds: [u64; BranchTargetKind::COUNT],
    event_lsq_load_bytes: u64,
    event_lsq_store_bytes: u64,
    event_store_load_forwarding_candidates: u64,
    event_store_load_forwarding_matches: u64,
    event_fu_latency_cycles: u64,
    event_fu_integer_mul_instructions: u64,
    event_fu_integer_mul_latency_cycles: u64,
    event_fu_integer_div_instructions: u64,
    event_fu_integer_div_latency_cycles: u64,
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

    pub(super) fn stats(&self) -> O3RuntimeStats {
        self.stats
    }

    pub(super) const fn stats_epoch(&self) -> u64 {
        self.stats_epoch
    }

    pub(super) const fn stats_reset_tick(&self) -> u64 {
        self.stats_reset_tick
    }

    fn checkpoint_restore(&self) -> Option<&Rem6O3CheckpointRestoreScope> {
        self.checkpoint_restore.as_ref()
    }

    pub(super) fn events(&self) -> &[O3RuntimeTraceRecord] {
        &self.events
    }

    pub(super) fn to_json(&self) -> String {
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
        let checkpoint_restore_label = self.checkpoint_restore.as_ref().map_or_else(
            || "null".to_string(),
            |restore| format!("\"{}\"", json_escape(&restore.label)),
        );
        let (
            checkpoint_restore_tick,
            checkpoint_restore_manifest_tick,
            checkpoint_restore_payload_bytes,
        ) = self
            .checkpoint_restore
            .as_ref()
            .map_or((0, 0, 0), |restore| {
                (restore.tick, restore.manifest_tick, restore.payload_bytes)
            });
        format!(
            "{{\"cpu\":{},\"target\":\"{}\",\"execution_mode\":{},\"stats_epoch\":{},\"stats_reset_tick\":{},\"checkpoint_restore_label\":{},\"checkpoint_restore_tick\":{},\"checkpoint_restore_manifest_tick\":{},\"checkpoint_restore_payload_bytes\":{},\"instructions\":{},\"rob_allocations\":{},\"rob_commits\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"lsq_load_bytes\":{},\"lsq_store_bytes\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"fu_latency_instructions\":{},\"fu_latency_cycles\":{},\"fu_integer_mul_instructions\":{},\"fu_integer_mul_latency_cycles\":{},\"fu_integer_div_instructions\":{},\"fu_integer_div_latency_cycles\":{},\"max_rob_occupancy\":{},\"max_lsq_occupancy\":{},\"rename_map_entries\":{},\"events\":[{}]}}",
            self.cpu,
            json_escape(&self.target),
            execution_mode,
            self.stats_epoch,
            self.stats_reset_tick,
            checkpoint_restore_label,
            checkpoint_restore_tick,
            checkpoint_restore_manifest_tick,
            checkpoint_restore_payload_bytes,
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
            self.stats.fu_latency_instructions(),
            self.stats.fu_latency_cycles(),
            self.stats.fu_integer_mul_instructions(),
            self.stats.fu_integer_mul_latency_cycles(),
            self.stats.fu_integer_div_instructions(),
            self.stats.fu_integer_div_latency_cycles(),
            self.stats.max_rob_occupancy(),
            self.stats.max_lsq_occupancy(),
            self.stats.rename_map_entries(),
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
    latest_checkpoint_restore: Option<&Rem6HostCheckpointSummary>,
) -> Vec<Rem6O3TraceRecord> {
    let mut records = Vec::new();
    let stats_epoch = latest_stats_reset.map_or(0, |reset| reset.epoch);
    let stats_reset_tick = latest_stats_reset.map_or(0, |reset| reset.tick);
    let checkpoint_restore = latest_checkpoint_restore.map(Rem6O3CheckpointRestoreScope::from);
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

impl Rem6O3TraceTotals {
    fn add(&mut self, record: &Rem6O3TraceRecord) {
        let stats = record.stats();
        self.records = self.records.saturating_add(1);
        self.stats_epoch = self.stats_epoch.max(record.stats_epoch());
        self.stats_reset_tick = self.stats_reset_tick.max(record.stats_reset_tick());
        if let Some(restore) = record.checkpoint_restore() {
            self.checkpoint_restores = 1;
            self.checkpoint_restore_tick = self.checkpoint_restore_tick.max(restore.tick);
            self.checkpoint_restore_payload_bytes = self
                .checkpoint_restore_payload_bytes
                .max(restore.payload_bytes);
        }
        self.instructions = self.instructions.saturating_add(stats.instructions());
        self.rob_allocations = self.rob_allocations.saturating_add(stats.rob_allocations());
        self.rob_commits = self.rob_commits.saturating_add(stats.rob_commits());
        self.rename_writes = self.rename_writes.saturating_add(stats.rename_writes());
        self.lsq_loads = self.lsq_loads.saturating_add(stats.lsq_loads());
        self.lsq_stores = self.lsq_stores.saturating_add(stats.lsq_stores());
        self.lsq_load_bytes = self.lsq_load_bytes.saturating_add(stats.lsq_load_bytes());
        self.lsq_store_bytes = self.lsq_store_bytes.saturating_add(stats.lsq_store_bytes());
        self.store_load_forwarding_candidates = self
            .store_load_forwarding_candidates
            .saturating_add(stats.lsq_store_to_load_forwarding_candidates());
        self.store_load_forwarding_matches = self
            .store_load_forwarding_matches
            .saturating_add(stats.lsq_store_to_load_forwarding_matches());
        self.fu_latency_instructions = self
            .fu_latency_instructions
            .saturating_add(stats.fu_latency_instructions());
        self.fu_latency_cycles = self
            .fu_latency_cycles
            .saturating_add(stats.fu_latency_cycles());
        self.fu_integer_mul_instructions = self
            .fu_integer_mul_instructions
            .saturating_add(stats.fu_integer_mul_instructions());
        self.fu_integer_mul_latency_cycles = self
            .fu_integer_mul_latency_cycles
            .saturating_add(stats.fu_integer_mul_latency_cycles());
        self.fu_integer_div_instructions = self
            .fu_integer_div_instructions
            .saturating_add(stats.fu_integer_div_instructions());
        self.fu_integer_div_latency_cycles = self
            .fu_integer_div_latency_cycles
            .saturating_add(stats.fu_integer_div_latency_cycles());
        self.max_rob_occupancy = self.max_rob_occupancy.max(stats.max_rob_occupancy());
        self.max_lsq_occupancy = self.max_lsq_occupancy.max(stats.max_lsq_occupancy());
        self.rename_map_entries = self
            .rename_map_entries
            .saturating_add(stats.rename_map_entries());
        for event in record.events() {
            let event_tick = event.tick();
            self.event_records = self.event_records.saturating_add(1);
            self.event_first_tick = Some(
                self.event_first_tick
                    .map_or(event_tick, |tick| tick.min(event_tick)),
            );
            self.event_last_tick = Some(
                self.event_last_tick
                    .map_or(event_tick, |tick| tick.max(event_tick)),
            );
            self.event_max_rob_occupancy = self.event_max_rob_occupancy.max(event.rob_occupancy());
            self.event_max_lsq_occupancy = self.event_max_lsq_occupancy.max(event.lsq_occupancy());
            self.event_max_rename_map_entries = self
                .event_max_rename_map_entries
                .max(event.rename_map_entries());
            self.event_rob_allocations = self
                .event_rob_allocations
                .saturating_add(u64::from(event.rob_allocated()));
            self.event_rob_commits = self
                .event_rob_commits
                .saturating_add(u64::from(event.rob_committed()));
            self.event_rename_writes = self
                .event_rename_writes
                .saturating_add(event.rename_writes());
            self.event_lsq_loads = self.event_lsq_loads.saturating_add(event.lsq_loads());
            self.event_lsq_stores = self.event_lsq_stores.saturating_add(event.lsq_stores());
            self.add_event_lsq_operation(event.lsq_operation());
            self.add_event_lsq_ordering(event.lsq_ordering());
            self.add_event_branch(event);
            self.event_lsq_load_bytes = self
                .event_lsq_load_bytes
                .saturating_add(event.lsq_load_bytes());
            self.event_lsq_store_bytes = self
                .event_lsq_store_bytes
                .saturating_add(event.lsq_store_bytes());
            self.event_store_load_forwarding_candidates = self
                .event_store_load_forwarding_candidates
                .saturating_add(u64::from(event.store_load_forwarding_candidate()));
            self.event_store_load_forwarding_matches = self
                .event_store_load_forwarding_matches
                .saturating_add(u64::from(event.store_load_forwarding_match()));
            self.event_fu_latency_cycles = self
                .event_fu_latency_cycles
                .saturating_add(event.fu_latency_cycles());
            if event.fu_latency_cycles() > 0 {
                match event.fu_latency_class() {
                    Some(O3RuntimeFuLatencyClass::ScalarIntegerMul) => {
                        self.event_fu_integer_mul_instructions =
                            self.event_fu_integer_mul_instructions.saturating_add(1);
                        self.event_fu_integer_mul_latency_cycles = self
                            .event_fu_integer_mul_latency_cycles
                            .saturating_add(event.fu_latency_cycles());
                    }
                    Some(O3RuntimeFuLatencyClass::ScalarIntegerDiv) => {
                        self.event_fu_integer_div_instructions =
                            self.event_fu_integer_div_instructions.saturating_add(1);
                        self.event_fu_integer_div_latency_cycles = self
                            .event_fu_integer_div_latency_cycles
                            .saturating_add(event.fu_latency_cycles());
                    }
                    None => {}
                }
            }
        }
    }

    fn add_event_branch(&mut self, event: &O3RuntimeTraceRecord) {
        if !event.branch_event() {
            return;
        }
        self.event_branches = self.event_branches.saturating_add(1);
        self.event_branch_taken = self
            .event_branch_taken
            .saturating_add(u64::from(event.branch_resolved_taken()));
        self.event_branch_not_taken = self
            .event_branch_not_taken
            .saturating_add(u64::from(!event.branch_resolved_taken()));
        self.event_branch_predicted_taken = self
            .event_branch_predicted_taken
            .saturating_add(u64::from(event.branch_predicted_taken()));
        let predicted_target_matches = event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) == event.branch_resolved_target());
        let predicted_target_mismatches = event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) != event.branch_resolved_target());
        let targetless_mismatch = o3_branch_targetless_mismatch(event);
        let wrong_target = o3_branch_wrong_target(event);
        self.event_branch_predicted_target_matches = self
            .event_branch_predicted_target_matches
            .saturating_add(u64::from(predicted_target_matches));
        self.event_branch_predicted_target_mismatches = self
            .event_branch_predicted_target_mismatches
            .saturating_add(u64::from(predicted_target_mismatches));
        self.event_branch_targetless_mismatches = self
            .event_branch_targetless_mismatches
            .saturating_add(u64::from(targetless_mismatch));
        self.event_branch_wrong_targets = self
            .event_branch_wrong_targets
            .saturating_add(u64::from(wrong_target));
        self.event_branch_mispredictions = self
            .event_branch_mispredictions
            .saturating_add(u64::from(event.branch_mispredicted()));
        self.event_branch_squashes = self
            .event_branch_squashes
            .saturating_add(u64::from(event.branch_squash()));
        self.event_branch_link_writes = self
            .event_branch_link_writes
            .saturating_add(u64::from(event.branch_link_register_write()));
        let index = event.branch_kind().index();
        self.event_branch_kinds[index] = self.event_branch_kinds[index].saturating_add(1);
        if event.branch_resolved_taken() {
            self.event_branch_taken_kinds[index] =
                self.event_branch_taken_kinds[index].saturating_add(1);
        } else {
            self.event_branch_not_taken_kinds[index] =
                self.event_branch_not_taken_kinds[index].saturating_add(1);
        }
        if event.branch_predicted_taken() {
            self.event_branch_predicted_taken_kinds[index] =
                self.event_branch_predicted_taken_kinds[index].saturating_add(1);
        }
        if predicted_target_matches {
            self.event_branch_predicted_target_match_kinds[index] =
                self.event_branch_predicted_target_match_kinds[index].saturating_add(1);
        }
        if predicted_target_mismatches {
            self.event_branch_predicted_target_mismatch_kinds[index] =
                self.event_branch_predicted_target_mismatch_kinds[index].saturating_add(1);
        }
        if targetless_mismatch {
            self.event_branch_targetless_mismatch_kinds[index] =
                self.event_branch_targetless_mismatch_kinds[index].saturating_add(1);
        }
        if wrong_target {
            self.event_branch_wrong_target_kinds[index] =
                self.event_branch_wrong_target_kinds[index].saturating_add(1);
        }
        if event.branch_resolved_target().is_some() {
            self.event_branch_resolved_target_kinds[index] =
                self.event_branch_resolved_target_kinds[index].saturating_add(1);
        }
        if event.branch_link_register_write() {
            self.event_branch_link_write_kinds[index] =
                self.event_branch_link_write_kinds[index].saturating_add(1);
        }
        if event.branch_mispredicted() {
            self.event_branch_misprediction_kinds[index] =
                self.event_branch_misprediction_kinds[index].saturating_add(1);
        }
        if event.branch_squash() {
            self.event_branch_squash_kinds[index] =
                self.event_branch_squash_kinds[index].saturating_add(1);
        }
    }

    fn add_event_lsq_ordering(&mut self, ordering: O3RuntimeLsqOrdering) {
        match ordering {
            O3RuntimeLsqOrdering::None => {}
            O3RuntimeLsqOrdering::Acquire => {
                self.event_lsq_ordering_acquire = self.event_lsq_ordering_acquire.saturating_add(1);
            }
            O3RuntimeLsqOrdering::Release => {
                self.event_lsq_ordering_release = self.event_lsq_ordering_release.saturating_add(1);
            }
            O3RuntimeLsqOrdering::AcquireRelease => {
                self.event_lsq_ordering_acquire_release =
                    self.event_lsq_ordering_acquire_release.saturating_add(1);
            }
        }
    }

    fn add_event_lsq_operation(&mut self, operation: O3RuntimeLsqOperation) {
        match operation {
            O3RuntimeLsqOperation::None => {}
            O3RuntimeLsqOperation::Load => {
                self.event_lsq_operation_load = self.event_lsq_operation_load.saturating_add(1);
            }
            O3RuntimeLsqOperation::Store => {
                self.event_lsq_operation_store = self.event_lsq_operation_store.saturating_add(1);
            }
            O3RuntimeLsqOperation::LoadReserved => {
                self.event_lsq_operation_load_reserved =
                    self.event_lsq_operation_load_reserved.saturating_add(1);
            }
            O3RuntimeLsqOperation::StoreConditional => {
                self.event_lsq_operation_store_conditional =
                    self.event_lsq_operation_store_conditional.saturating_add(1);
            }
            O3RuntimeLsqOperation::Atomic => {
                self.event_lsq_operation_atomic = self.event_lsq_operation_atomic.saturating_add(1);
            }
            O3RuntimeLsqOperation::FloatLoad => {
                self.event_lsq_operation_float_load =
                    self.event_lsq_operation_float_load.saturating_add(1);
            }
            O3RuntimeLsqOperation::FloatStore => {
                self.event_lsq_operation_float_store =
                    self.event_lsq_operation_float_store.saturating_add(1);
            }
            O3RuntimeLsqOperation::VectorLoad => {
                self.event_lsq_operation_vector_load =
                    self.event_lsq_operation_vector_load.saturating_add(1);
            }
            O3RuntimeLsqOperation::VectorStore => {
                self.event_lsq_operation_vector_store =
                    self.event_lsq_operation_vector_store.saturating_add(1);
            }
        }
    }

    fn stats(self) -> Vec<Rem6O3TraceStat> {
        let mut stats = Vec::new();
        for (suffix, value) in [
            ("records", self.records),
            ("stats_epoch", self.stats_epoch),
            ("checkpoint_restores", self.checkpoint_restores),
            ("instructions", self.instructions),
            ("rob_allocations", self.rob_allocations),
            ("rob_commits", self.rob_commits),
            ("rename_writes", self.rename_writes),
            ("lsq_loads", self.lsq_loads),
            ("lsq_stores", self.lsq_stores),
            (
                "store_load_forwarding_candidates",
                self.store_load_forwarding_candidates,
            ),
            (
                "store_load_forwarding_matches",
                self.store_load_forwarding_matches,
            ),
            ("fu_latency_instructions", self.fu_latency_instructions),
            (
                "fu_integer_mul_instructions",
                self.fu_integer_mul_instructions,
            ),
            (
                "fu_integer_div_instructions",
                self.fu_integer_div_instructions,
            ),
            ("max_rob_occupancy", self.max_rob_occupancy),
            ("max_lsq_occupancy", self.max_lsq_occupancy),
            ("rename_map_entries", self.rename_map_entries),
            ("event.records", self.event_records),
            ("event.max_rob_occupancy", self.event_max_rob_occupancy),
            ("event.max_lsq_occupancy", self.event_max_lsq_occupancy),
            (
                "event.max_rename_map_entries",
                self.event_max_rename_map_entries,
            ),
            ("event.rob_allocations", self.event_rob_allocations),
            ("event.rob_commits", self.event_rob_commits),
            ("event.rename_writes", self.event_rename_writes),
            ("event.lsq_loads", self.event_lsq_loads),
            ("event.lsq_stores", self.event_lsq_stores),
            ("event.lsq_operation.load", self.event_lsq_operation_load),
            ("event.lsq_operation.store", self.event_lsq_operation_store),
            (
                "event.lsq_operation.load_reserved",
                self.event_lsq_operation_load_reserved,
            ),
            (
                "event.lsq_operation.store_conditional",
                self.event_lsq_operation_store_conditional,
            ),
            (
                "event.lsq_operation.atomic",
                self.event_lsq_operation_atomic,
            ),
            (
                "event.lsq_operation.float_load",
                self.event_lsq_operation_float_load,
            ),
            (
                "event.lsq_operation.float_store",
                self.event_lsq_operation_float_store,
            ),
            (
                "event.lsq_operation.vector_load",
                self.event_lsq_operation_vector_load,
            ),
            (
                "event.lsq_operation.vector_store",
                self.event_lsq_operation_vector_store,
            ),
            (
                "event.lsq_ordering.acquire",
                self.event_lsq_ordering_acquire,
            ),
            (
                "event.lsq_ordering.release",
                self.event_lsq_ordering_release,
            ),
            (
                "event.lsq_ordering.acquire_release",
                self.event_lsq_ordering_acquire_release,
            ),
            ("event.branches", self.event_branches),
            ("event.branch_taken", self.event_branch_taken),
            ("event.branch_not_taken", self.event_branch_not_taken),
            (
                "event.branch_predicted_taken",
                self.event_branch_predicted_taken,
            ),
            (
                "event.branch_predicted_target_matches",
                self.event_branch_predicted_target_matches,
            ),
            (
                "event.branch_predicted_target_mismatches",
                self.event_branch_predicted_target_mismatches,
            ),
            (
                "event.branch_targetless_mismatches",
                self.event_branch_targetless_mismatches,
            ),
            (
                "event.branch_wrong_targets",
                self.event_branch_wrong_targets,
            ),
            (
                "event.branch_mispredictions",
                self.event_branch_mispredictions,
            ),
            ("event.branch_squashes", self.event_branch_squashes),
            ("event.branch_link_writes", self.event_branch_link_writes),
            (
                "event.store_load_forwarding_candidates",
                self.event_store_load_forwarding_candidates,
            ),
            (
                "event.store_load_forwarding_matches",
                self.event_store_load_forwarding_matches,
            ),
            (
                "event.fu_integer_mul_instructions",
                self.event_fu_integer_mul_instructions,
            ),
            (
                "event.fu_integer_div_instructions",
                self.event_fu_integer_div_instructions,
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value,
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_taken_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_taken_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_not_taken_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_not_taken_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_predicted_taken_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_predicted_taken_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_predicted_target_match_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_predicted_target_match_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_predicted_target_mismatch_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_predicted_target_mismatch_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_targetless_mismatch_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_targetless_mismatch_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_wrong_target_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_wrong_target_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_resolved_target_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_resolved_target_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_link_write_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_link_write_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_misprediction_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_misprediction_kinds[kind.index()],
            });
        }
        for kind in BranchTargetKind::ALL {
            if matches!(kind, BranchTargetKind::NoBranch) {
                continue;
            }
            stats.push(Rem6O3TraceStat {
                suffix: o3_branch_squash_kind_stat_suffix(kind),
                unit: "Count",
                value: self.event_branch_squash_kinds[kind.index()],
            });
        }
        stats.push(Rem6O3TraceStat {
            suffix: "fu_latency_cycles",
            unit: "Cycle",
            value: self.fu_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "stats_reset_tick",
            unit: "Tick",
            value: self.stats_reset_tick,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "checkpoint_restore_tick",
            unit: "Tick",
            value: self.checkpoint_restore_tick,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "checkpoint_restore_payload_bytes",
            unit: "Byte",
            value: self.checkpoint_restore_payload_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "lsq_load_bytes",
            unit: "Byte",
            value: self.lsq_load_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "lsq_store_bytes",
            unit: "Byte",
            value: self.lsq_store_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_load_bytes",
            unit: "Byte",
            value: self.event_lsq_load_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_store_bytes",
            unit: "Byte",
            value: self.event_lsq_store_bytes,
        });
        let first_event_tick = self.event_first_tick.unwrap_or(0);
        let last_event_tick = self.event_last_tick.unwrap_or(0);
        stats.push(Rem6O3TraceStat {
            suffix: "event.first_tick",
            unit: "Tick",
            value: first_event_tick,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.last_tick",
            unit: "Tick",
            value: last_event_tick,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.tick_span",
            unit: "Tick",
            value: last_event_tick.saturating_sub(first_event_tick),
        });
        stats.push(Rem6O3TraceStat {
            suffix: "fu_integer_mul_latency_cycles",
            unit: "Cycle",
            value: self.fu_integer_mul_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "fu_integer_div_latency_cycles",
            unit: "Cycle",
            value: self.fu_integer_div_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.fu_latency_cycles",
            unit: "Cycle",
            value: self.event_fu_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.fu_integer_mul_latency_cycles",
            unit: "Cycle",
            value: self.event_fu_integer_mul_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.fu_integer_div_latency_cycles",
            unit: "Cycle",
            value: self.event_fu_integer_div_latency_cycles,
        });
        stats
    }
}

impl From<&Rem6HostCheckpointSummary> for Rem6O3CheckpointRestoreScope {
    fn from(summary: &Rem6HostCheckpointSummary) -> Self {
        Self {
            label: summary.label.clone(),
            tick: summary.tick,
            manifest_tick: summary.manifest_tick,
            payload_bytes: summary.payload_bytes,
        }
    }
}

fn o3_branch_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_kind.no_branch",
        BranchTargetKind::Return => "event.branch_kind.return",
        BranchTargetKind::CallDirect => "event.branch_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => "event.branch_kind.direct_unconditional",
        BranchTargetKind::IndirectConditional => "event.branch_kind.indirect_conditional",
        BranchTargetKind::IndirectUnconditional => "event.branch_kind.indirect_unconditional",
    }
}

fn o3_branch_taken_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_taken_kind.no_branch",
        BranchTargetKind::Return => "event.branch_taken_kind.return",
        BranchTargetKind::CallDirect => "event.branch_taken_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_taken_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_taken_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => "event.branch_taken_kind.direct_unconditional",
        BranchTargetKind::IndirectConditional => "event.branch_taken_kind.indirect_conditional",
        BranchTargetKind::IndirectUnconditional => "event.branch_taken_kind.indirect_unconditional",
    }
}

fn o3_branch_not_taken_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_not_taken_kind.no_branch",
        BranchTargetKind::Return => "event.branch_not_taken_kind.return",
        BranchTargetKind::CallDirect => "event.branch_not_taken_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_not_taken_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_not_taken_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => "event.branch_not_taken_kind.direct_unconditional",
        BranchTargetKind::IndirectConditional => "event.branch_not_taken_kind.indirect_conditional",
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_not_taken_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_predicted_taken_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_predicted_taken_kind.no_branch",
        BranchTargetKind::Return => "event.branch_predicted_taken_kind.return",
        BranchTargetKind::CallDirect => "event.branch_predicted_taken_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_predicted_taken_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_predicted_taken_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_predicted_taken_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_predicted_taken_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_predicted_taken_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_predicted_target_match_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_predicted_target_match_kind.no_branch",
        BranchTargetKind::Return => "event.branch_predicted_target_match_kind.return",
        BranchTargetKind::CallDirect => "event.branch_predicted_target_match_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_predicted_target_match_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_predicted_target_match_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_predicted_target_match_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_predicted_target_match_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_predicted_target_match_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_predicted_target_mismatch_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_predicted_target_mismatch_kind.no_branch",
        BranchTargetKind::Return => "event.branch_predicted_target_mismatch_kind.return",
        BranchTargetKind::CallDirect => "event.branch_predicted_target_mismatch_kind.call_direct",
        BranchTargetKind::CallIndirect => {
            "event.branch_predicted_target_mismatch_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_predicted_target_mismatch_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_predicted_target_mismatch_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_predicted_target_mismatch_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_predicted_target_mismatch_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_targetless_mismatch_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_targetless_mismatch_kind.no_branch",
        BranchTargetKind::Return => "event.branch_targetless_mismatch_kind.return",
        BranchTargetKind::CallDirect => "event.branch_targetless_mismatch_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_targetless_mismatch_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_targetless_mismatch_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_targetless_mismatch_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_targetless_mismatch_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_targetless_mismatch_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_wrong_target_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_wrong_target_kind.no_branch",
        BranchTargetKind::Return => "event.branch_wrong_target_kind.return",
        BranchTargetKind::CallDirect => "event.branch_wrong_target_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_wrong_target_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_wrong_target_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => {
            "event.branch_wrong_target_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_wrong_target_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_wrong_target_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_resolved_target_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_resolved_target_kind.no_branch",
        BranchTargetKind::Return => "event.branch_resolved_target_kind.return",
        BranchTargetKind::CallDirect => "event.branch_resolved_target_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_resolved_target_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_resolved_target_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_resolved_target_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_resolved_target_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_resolved_target_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_link_write_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_link_write_kind.no_branch",
        BranchTargetKind::Return => "event.branch_link_write_kind.return",
        BranchTargetKind::CallDirect => "event.branch_link_write_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_link_write_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_link_write_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => {
            "event.branch_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_link_write_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_misprediction_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_misprediction_kind.no_branch",
        BranchTargetKind::Return => "event.branch_misprediction_kind.return",
        BranchTargetKind::CallDirect => "event.branch_misprediction_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_misprediction_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_misprediction_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => {
            "event.branch_misprediction_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_misprediction_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_misprediction_kind.indirect_unconditional"
        }
    }
}

fn o3_branch_squash_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_squash_kind.no_branch",
        BranchTargetKind::Return => "event.branch_squash_kind.return",
        BranchTargetKind::CallDirect => "event.branch_squash_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_squash_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_squash_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => "event.branch_squash_kind.direct_unconditional",
        BranchTargetKind::IndirectConditional => "event.branch_squash_kind.indirect_conditional",
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_squash_kind.indirect_unconditional"
        }
    }
}

fn o3_event_to_json(event: &O3RuntimeTraceRecord) -> String {
    let fu_latency_class = event.fu_latency_class().map_or_else(
        || "null".to_string(),
        |class| format!("\"{}\"", class.as_str()),
    );
    let lsq_load_address =
        o3_optional_address_to_json(event.lsq_load_address().map(|address| address.get()));
    let lsq_store_address =
        o3_optional_address_to_json(event.lsq_store_address().map(|address| address.get()));
    let branch_predicted_target =
        o3_optional_address_to_json(event.branch_predicted_target().map(|address| address.get()));
    let branch_resolved_target =
        o3_optional_address_to_json(event.branch_resolved_target().map(|address| address.get()));
    let branch_squashed_target =
        o3_optional_address_to_json(event.branch_squashed_target().map(|address| address.get()));
    let branch_targetless_mismatch = o3_branch_targetless_mismatch(event);
    let branch_wrong_target = o3_branch_wrong_target(event);
    format!(
        "{{\"sequence\":{},\"tick\":{},\"pc\":\"0x{:x}\",\"rob_allocated\":{},\"rob_committed\":{},\"rob_occupancy\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"lsq_occupancy\":{},\"lsq_operation\":\"{}\",\"lsq_ordering\":\"{}\",\"lsq_acquire\":{},\"lsq_release\":{},\"lsq_load_address\":{},\"lsq_store_address\":{},\"lsq_load_bytes\":{},\"lsq_store_bytes\":{},\"rename_map_entries\":{},\"store_load_forwarding_candidate\":{},\"store_load_forwarding_match\":{},\"branch_event\":{},\"branch_kind\":\"{}\",\"branch_predicted_taken\":{},\"branch_resolved_taken\":{},\"branch_mispredicted\":{},\"branch_targetless_mismatch\":{},\"branch_wrong_target\":{},\"branch_link_register_write\":{},\"branch_predicted_target\":{},\"branch_resolved_target\":{},\"branch_squash\":{},\"branch_squashed_target\":{},\"fu_latency_class\":{},\"fu_latency_cycles\":{},\"system_event\":{}}}",
        event.sequence(),
        event.tick(),
        event.pc().get(),
        event.rob_allocated(),
        event.rob_committed(),
        event.rob_occupancy(),
        event.rename_writes(),
        event.lsq_loads(),
        event.lsq_stores(),
        event.lsq_occupancy(),
        event.lsq_operation().as_str(),
        event.lsq_ordering().as_str(),
        event.lsq_ordering().acquire(),
        event.lsq_ordering().release(),
        lsq_load_address,
        lsq_store_address,
        event.lsq_load_bytes(),
        event.lsq_store_bytes(),
        event.rename_map_entries(),
        event.store_load_forwarding_candidate(),
        event.store_load_forwarding_match(),
        event.branch_event(),
        event.branch_kind().canonical_stat_name(),
        event.branch_predicted_taken(),
        event.branch_resolved_taken(),
        event.branch_mispredicted(),
        branch_targetless_mismatch,
        branch_wrong_target,
        event.branch_link_register_write(),
        branch_predicted_target,
        branch_resolved_target,
        event.branch_squash(),
        branch_squashed_target,
        fu_latency_class,
        event.fu_latency_cycles(),
        event.system_event(),
    )
}

fn o3_branch_wrong_target(event: &O3RuntimeTraceRecord) -> bool {
    event
        .branch_predicted_target()
        .zip(event.branch_resolved_target())
        .is_some_and(|(predicted, resolved)| predicted != resolved)
}

fn o3_branch_targetless_mismatch(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_predicted_target().is_some() && event.branch_resolved_target().is_none()
}

fn o3_optional_address_to_json(address: Option<u64>) -> String {
    address.map_or_else(
        || "null".to_string(),
        |address| format!("\"0x{address:x}\""),
    )
}
