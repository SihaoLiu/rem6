use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_cpu::{
    BranchTargetKind, CpuId, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeStats,
};
use rem6_stats::{StatId, StatsError, StatsRegistry};

#[derive(Clone, Debug)]
pub struct RiscvO3RuntimeStats {
    cpus: BTreeSet<CpuId>,
    stats: BTreeMap<CpuId, RiscvO3RuntimeCpuStats>,
    active_cpus: Arc<Mutex<BTreeSet<CpuId>>>,
    previous: Arc<Mutex<BTreeMap<CpuId, O3RuntimeStats>>>,
    cycle_baselines: Arc<Mutex<BTreeMap<CpuId, u64>>>,
}

impl RiscvO3RuntimeStats {
    pub fn register_for_cpus<I>(registry: &mut StatsRegistry, cpus: I) -> Result<Self, StatsError>
    where
        I: IntoIterator<Item = CpuId>,
    {
        let cpus = cpus.into_iter().collect::<BTreeSet<_>>();
        let single_cpu_run = cpus.len() == 1;
        let stats = cpus
            .iter()
            .map(|cpu| {
                RiscvO3RuntimeCpuStats::register(registry, *cpu, single_cpu_run)
                    .map(|stats| (*cpu, stats))
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self {
            cpus: cpus.clone(),
            stats,
            active_cpus: Arc::new(Mutex::new(BTreeSet::new())),
            previous: Arc::new(Mutex::new(BTreeMap::new())),
            cycle_baselines: Arc::new(Mutex::new(
                cpus.iter().copied().map(|cpu| (cpu, 0)).collect(),
            )),
        })
    }

    pub fn reset_snapshots<I>(&self, cycle_baselines: I)
    where
        I: IntoIterator<Item = (CpuId, u64)>,
    {
        self.active_cpus
            .lock()
            .expect("O3 runtime stats lock")
            .clear();
        let mut previous = self.previous.lock().expect("O3 runtime stats lock");
        previous.clear();
        previous.extend(
            self.cpus
                .iter()
                .copied()
                .map(|cpu| (cpu, O3RuntimeStats::default())),
        );
        let cycle_baselines = cycle_baselines.into_iter().collect::<BTreeMap<_, _>>();
        let mut stored_cycle_baselines =
            self.cycle_baselines.lock().expect("O3 runtime stats lock");
        stored_cycle_baselines.clear();
        stored_cycle_baselines.extend(
            self.cpus
                .iter()
                .copied()
                .map(|cpu| (cpu, cycle_baselines.get(&cpu).copied().unwrap_or(0))),
        );
    }

    pub fn record_cpu_snapshot(
        &self,
        registry: &mut StatsRegistry,
        cpu: CpuId,
        snapshot: O3RuntimeStats,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        let Some(stats) = self.stats.get(&cpu) else {
            return Ok(());
        };
        let resettable_pipeline_cycles =
            self.resettable_pipeline_cycles(cpu, in_order_pipeline_cycles);
        let mut previous = self.previous.lock().expect("O3 runtime stats lock");
        let previous_snapshot = previous.entry(cpu).or_default();
        stats.increment_delta(
            registry,
            *previous_snapshot,
            snapshot,
            resettable_pipeline_cycles,
        )?;
        *previous_snapshot = snapshot;
        self.sync_active_cpu(cpu, snapshot);
        Ok(())
    }

    pub fn sync_cpu_snapshot(
        &self,
        registry: &mut StatsRegistry,
        cpu: CpuId,
        snapshot: O3RuntimeStats,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        let Some(stats) = self.stats.get(&cpu) else {
            return Ok(());
        };
        let resettable_pipeline_cycles =
            self.resettable_pipeline_cycles(cpu, in_order_pipeline_cycles);
        stats.set_snapshot(registry, snapshot, resettable_pipeline_cycles)?;
        self.previous
            .lock()
            .expect("O3 runtime stats lock")
            .insert(cpu, snapshot);
        self.sync_active_cpu(cpu, snapshot);
        Ok(())
    }

    pub(crate) fn active_cpu_indices(&self) -> Vec<u32> {
        self.active_cpus
            .lock()
            .expect("O3 runtime stats lock")
            .iter()
            .map(|cpu| cpu.get())
            .collect()
    }

    fn sync_active_cpu(&self, cpu: CpuId, snapshot: O3RuntimeStats) {
        let mut active_cpus = self.active_cpus.lock().expect("O3 runtime stats lock");
        if snapshot.has_activity() {
            active_cpus.insert(cpu);
        } else {
            active_cpus.remove(&cpu);
        }
    }

    fn resettable_pipeline_cycles(&self, cpu: CpuId, in_order_pipeline_cycles: u64) -> u64 {
        let mut cycle_baselines = self.cycle_baselines.lock().expect("O3 runtime stats lock");
        let baseline = cycle_baselines.entry(cpu).or_insert(0);
        if in_order_pipeline_cycles < *baseline {
            *baseline = 0;
        }
        in_order_pipeline_cycles.saturating_sub(*baseline)
    }
}

impl Default for RiscvO3RuntimeStats {
    fn default() -> Self {
        Self {
            cpus: BTreeSet::new(),
            stats: BTreeMap::new(),
            active_cpus: Arc::new(Mutex::new(BTreeSet::new())),
            previous: Arc::new(Mutex::new(BTreeMap::new())),
            cycle_baselines: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvO3RuntimeFuLatencyClassStats {
    instructions: StatId,
    latency_cycles: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvO3RuntimeBranchRepairStats {
    targetless_mismatch: StatId,
    wrong_target: StatId,
    direction_only: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvO3RuntimeBranchEventKindStats {
    kind: StatId,
    taken: StatId,
    predicted_taken: StatId,
    predicted_not_taken: StatId,
    predicted_target: StatId,
    predicted_target_match: StatId,
    predicted_target_mismatch: StatId,
    resolved_target: StatId,
    link_write: StatId,
    squash: StatId,
    squashed_target_link_write: StatId,
    squashed_target_without_link_write: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvO3RuntimeLsqLatencyStats {
    samples: StatId,
    ticks: StatId,
    max_ticks: StatId,
    min_ticks: StatId,
    avg_ticks: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvO3RuntimeStructuralAliasStats {
    rob_writes: StatId,
    rob_reads: StatId,
    rob_max_occupancy: StatId,
    rename_renamed_insts: StatId,
    rename_renamed_operands: StatId,
    iew_dispatched_insts: StatId,
    iew_disp_load_insts: StatId,
    iew_disp_store_insts: StatId,
    iew_insts_to_commit_total: StatId,
    iew_writeback_count_total: StatId,
    iew_producer_inst_total: StatId,
    iew_consumer_inst_total: StatId,
    lsq_added_loads_and_stores: StatId,
    lsq_store_load_forwarding_candidates: StatId,
    lsq_store_load_forwarding_matches: StatId,
    lsq_store_load_forwarding_suppressed: StatId,
    lsq_store_load_forwarding_address_mismatches: StatId,
    lsq_store_load_forwarding_byte_mismatches: StatId,
    lsq_forw_loads: StatId,
    lsq_max_occupancy: StatId,
    iq_insts_issued: StatId,
    iq_mem_insts_issued: StatId,
    iq_issued_inst_type_mem_read: StatId,
    iq_issued_inst_type_mem_write: StatId,
    commit_committed_inst_type_mem_read: StatId,
    commit_committed_inst_type_mem_write: StatId,
    lsq_load_bytes: StatId,
    lsq_store_bytes: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvO3RuntimeBranchAliasStats {
    branch_repair_targetless_mismatch: StatId,
    branch_repair_direction_only: StatId,
    branch_repair_wrong_target: StatId,
    branch_repair_total: StatId,
    iew_predicted_taken_incorrect: StatId,
    iew_predicted_not_taken_incorrect: StatId,
    iew_branch_mispredicts: StatId,
    commit_branch_mispredicts: StatId,
    iq_branch_insts_issued: StatId,
}

impl RiscvO3RuntimeBranchAliasStats {
    fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        Ok(Self {
            branch_repair_targetless_mismatch: register_o3_counter(
                registry,
                prefix,
                "iew.branchRepair.targetlessMismatch",
                "Count",
            )?,
            branch_repair_direction_only: register_o3_counter(
                registry,
                prefix,
                "iew.branchRepair.directionOnly",
                "Count",
            )?,
            branch_repair_wrong_target: register_o3_counter(
                registry,
                prefix,
                "iew.branchRepair.wrongTarget",
                "Count",
            )?,
            branch_repair_total: register_o3_counter(
                registry,
                prefix,
                "iew.branchRepair.total",
                "Count",
            )?,
            iew_predicted_taken_incorrect: register_o3_counter(
                registry,
                prefix,
                "iew.predictedTakenIncorrect",
                "Count",
            )?,
            iew_predicted_not_taken_incorrect: register_o3_counter(
                registry,
                prefix,
                "iew.predictedNotTakenIncorrect",
                "Count",
            )?,
            iew_branch_mispredicts: register_o3_counter(
                registry,
                prefix,
                "iew.branchMispredicts",
                "Count",
            )?,
            commit_branch_mispredicts: register_o3_counter(
                registry,
                prefix,
                "commit.branchMispredicts",
                "Count",
            )?,
            iq_branch_insts_issued: register_o3_counter(
                registry,
                prefix,
                "iq.branchInstsIssued",
                "Count",
            )?,
        })
    }

    fn increment_delta(
        self,
        registry: &mut StatsRegistry,
        previous: O3RuntimeStats,
        current: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for ((stat, previous), (_, current)) in self
            .count_values(previous)
            .into_iter()
            .zip(self.count_values(current))
        {
            let delta = current.saturating_sub(previous);
            if delta != 0 {
                registry.increment(stat, delta)?;
            }
        }
        Ok(())
    }

    fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for (stat, value) in self.count_values(snapshot) {
            registry.set_resettable_counter(stat, value)?;
        }
        Ok(())
    }

    fn count_values(self, stats: O3RuntimeStats) -> [(StatId, u64); 9] {
        let branch_mispredicts = o3_branch_mispredicts(stats);
        [
            (
                self.branch_repair_targetless_mismatch,
                stats.branch_repair_targetless_mismatches(),
            ),
            (
                self.branch_repair_direction_only,
                stats.branch_repair_direction_only_mismatches(),
            ),
            (
                self.branch_repair_wrong_target,
                stats.branch_repair_wrong_targets(),
            ),
            (self.branch_repair_total, branch_mispredicts),
            (
                self.iew_predicted_taken_incorrect,
                stats.iew_predicted_taken_incorrect(),
            ),
            (
                self.iew_predicted_not_taken_incorrect,
                stats.iew_predicted_not_taken_incorrect(),
            ),
            (self.iew_branch_mispredicts, branch_mispredicts),
            (self.commit_branch_mispredicts, branch_mispredicts),
            (self.iq_branch_insts_issued, stats.iq_branch_insts_issued()),
        ]
    }
}

impl RiscvO3RuntimeStructuralAliasStats {
    fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        Ok(Self {
            rob_writes: register_o3_counter(registry, prefix, "rob.writes", "Count")?,
            rob_reads: register_o3_counter(registry, prefix, "rob.reads", "Count")?,
            rob_max_occupancy: register_o3_counter(registry, prefix, "rob.maxOccupancy", "Count")?,
            rename_renamed_insts: register_o3_counter(
                registry,
                prefix,
                "rename.renamedInsts",
                "Count",
            )?,
            rename_renamed_operands: register_o3_counter(
                registry,
                prefix,
                "rename.renamedOperands",
                "Count",
            )?,
            iew_dispatched_insts: register_o3_counter(
                registry,
                prefix,
                "iew.dispatchedInsts",
                "Count",
            )?,
            iew_disp_load_insts: register_o3_counter(
                registry,
                prefix,
                "iew.dispLoadInsts",
                "Count",
            )?,
            iew_disp_store_insts: register_o3_counter(
                registry,
                prefix,
                "iew.dispStoreInsts",
                "Count",
            )?,
            iew_insts_to_commit_total: register_o3_counter(
                registry,
                prefix,
                "iew.instsToCommit.total",
                "Count",
            )?,
            iew_writeback_count_total: register_o3_counter(
                registry,
                prefix,
                "iew.writebackCount.total",
                "Count",
            )?,
            iew_producer_inst_total: register_o3_counter(
                registry,
                prefix,
                "iew.producerInst.total",
                "Count",
            )?,
            iew_consumer_inst_total: register_o3_counter(
                registry,
                prefix,
                "iew.consumerInst.total",
                "Count",
            )?,
            lsq_added_loads_and_stores: register_o3_counter(
                registry,
                prefix,
                "lsq0.addedLoadsAndStores",
                "Count",
            )?,
            lsq_store_load_forwarding_candidates: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingCandidates",
                "Count",
            )?,
            lsq_store_load_forwarding_matches: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingMatches",
                "Count",
            )?,
            lsq_store_load_forwarding_suppressed: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingSuppressed",
                "Count",
            )?,
            lsq_store_load_forwarding_address_mismatches: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingAddressMismatches",
                "Count",
            )?,
            lsq_store_load_forwarding_byte_mismatches: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingByteMismatches",
                "Count",
            )?,
            lsq_forw_loads: register_o3_counter(registry, prefix, "lsq0.forwLoads", "Count")?,
            lsq_max_occupancy: register_o3_counter(registry, prefix, "lsq0.maxOccupancy", "Count")?,
            iq_insts_issued: register_o3_counter(registry, prefix, "iq.instsIssued", "Count")?,
            iq_mem_insts_issued: register_o3_counter(
                registry,
                prefix,
                "iq.memInstsIssued",
                "Count",
            )?,
            iq_issued_inst_type_mem_read: register_o3_counter(
                registry,
                prefix,
                "iq.issuedInstType.MemRead",
                "Count",
            )?,
            iq_issued_inst_type_mem_write: register_o3_counter(
                registry,
                prefix,
                "iq.issuedInstType.MemWrite",
                "Count",
            )?,
            commit_committed_inst_type_mem_read: register_o3_counter(
                registry,
                prefix,
                "commit.committedInstType.MemRead",
                "Count",
            )?,
            commit_committed_inst_type_mem_write: register_o3_counter(
                registry,
                prefix,
                "commit.committedInstType.MemWrite",
                "Count",
            )?,
            lsq_load_bytes: register_o3_counter(registry, prefix, "lsq0.loadBytes", "Byte")?,
            lsq_store_bytes: register_o3_counter(registry, prefix, "lsq0.storeBytes", "Byte")?,
        })
    }

    fn increment_delta(
        self,
        registry: &mut StatsRegistry,
        previous: O3RuntimeStats,
        current: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for ((stat, previous), (_, current)) in self
            .count_values(previous)
            .into_iter()
            .zip(self.count_values(current))
        {
            let delta = current.saturating_sub(previous);
            if delta != 0 {
                registry.increment(stat, delta)?;
            }
        }
        for ((stat, previous), (_, current)) in self
            .byte_values(previous)
            .into_iter()
            .zip(self.byte_values(current))
        {
            let delta = current.saturating_sub(previous);
            if delta != 0 {
                registry.increment(stat, delta)?;
            }
        }
        Ok(())
    }

    fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for (stat, value) in self.count_values(snapshot) {
            registry.set_resettable_counter(stat, value)?;
        }
        for (stat, value) in self.byte_values(snapshot) {
            registry.set_resettable_counter(stat, value)?;
        }
        Ok(())
    }

    fn count_values(self, stats: O3RuntimeStats) -> [(StatId, u64); 26] {
        [
            (self.rob_writes, stats.rob_allocations()),
            (self.rob_reads, stats.rob_commits()),
            (self.rob_max_occupancy, stats.max_rob_occupancy()),
            (self.rename_renamed_insts, stats.instructions()),
            (self.rename_renamed_operands, stats.rename_writes()),
            (self.iew_dispatched_insts, stats.instructions()),
            (self.iew_disp_load_insts, stats.lsq_loads()),
            (self.iew_disp_store_insts, stats.lsq_stores()),
            (self.iew_insts_to_commit_total, stats.rob_commits()),
            (self.iew_writeback_count_total, stats.instructions()),
            (self.iew_producer_inst_total, stats.iew_producer_insts()),
            (self.iew_consumer_inst_total, stats.iew_consumer_insts()),
            (
                self.lsq_added_loads_and_stores,
                stats.lsq_loads().saturating_add(stats.lsq_stores()),
            ),
            (
                self.lsq_store_load_forwarding_candidates,
                stats.lsq_store_to_load_forwarding_candidates(),
            ),
            (
                self.lsq_store_load_forwarding_matches,
                stats.lsq_store_to_load_forwarding_matches(),
            ),
            (
                self.lsq_store_load_forwarding_suppressed,
                stats.lsq_store_to_load_forwarding_suppressed(),
            ),
            (
                self.lsq_store_load_forwarding_address_mismatches,
                stats.lsq_store_to_load_forwarding_address_mismatches(),
            ),
            (
                self.lsq_store_load_forwarding_byte_mismatches,
                stats.lsq_store_to_load_forwarding_byte_mismatches(),
            ),
            (
                self.lsq_forw_loads,
                stats.lsq_store_to_load_forwarding_matches(),
            ),
            (self.lsq_max_occupancy, stats.max_lsq_occupancy()),
            (self.iq_insts_issued, stats.instructions()),
            (
                self.iq_mem_insts_issued,
                stats.lsq_loads().saturating_add(stats.lsq_stores()),
            ),
            (self.iq_issued_inst_type_mem_read, stats.lsq_loads()),
            (self.iq_issued_inst_type_mem_write, stats.lsq_stores()),
            (self.commit_committed_inst_type_mem_read, stats.lsq_loads()),
            (
                self.commit_committed_inst_type_mem_write,
                stats.lsq_stores(),
            ),
        ]
    }

    fn byte_values(self, stats: O3RuntimeStats) -> [(StatId, u64); 2] {
        [
            (self.lsq_load_bytes, stats.lsq_load_bytes()),
            (self.lsq_store_bytes, stats.lsq_store_bytes()),
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvO3RuntimeCpuStats {
    instructions: StatId,
    rob_allocations: StatId,
    rob_commits: StatId,
    rename_writes: StatId,
    lsq_loads: StatId,
    lsq_stores: StatId,
    lsq_load_bytes: StatId,
    lsq_store_bytes: StatId,
    lsq_store_to_load_forwarding_candidates: StatId,
    lsq_store_to_load_forwarding_matches: StatId,
    lsq_store_to_load_forwarding_suppressed: StatId,
    lsq_store_to_load_forwarding_address_mismatches: StatId,
    lsq_store_to_load_forwarding_byte_mismatches: StatId,
    structural_aliases: RiscvO3RuntimeStructuralAliasStats,
    lsq_operation_counts: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_alias_counts: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_alias_total: StatId,
    lsq_operation_forwarding_candidates: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_matches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_suppressed: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_address_mismatches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_byte_mismatches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_candidate_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_match_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_suppressed_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_address_mismatch_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_byte_mismatch_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_data_latency: RiscvO3RuntimeLsqLatencyStats,
    lsq_operation_latency: [RiscvO3RuntimeLsqLatencyStats; O3RuntimeLsqOperation::COUNT],
    lsq_ordering_counts: [StatId; O3RuntimeLsqOrdering::COUNT],
    lsq_ordering_alias_counts: [StatId; O3RuntimeLsqOrdering::COUNT],
    lsq_ordering_alias_total: StatId,
    lsq_store_conditional_failures: StatId,
    branch_repair_targetless_mismatches: StatId,
    branch_repair_wrong_targets: StatId,
    branch_repair_direction_only_mismatches: StatId,
    branch_repair_kinds: [RiscvO3RuntimeBranchRepairStats; BranchTargetKind::COUNT],
    branch_event_branches: StatId,
    branch_event_taken: StatId,
    branch_event_not_taken: StatId,
    branch_event_predicted_taken: StatId,
    branch_event_predicted_not_taken: StatId,
    branch_event_predicted_targets: StatId,
    branch_event_predicted_target_matches: StatId,
    branch_event_predicted_target_mismatches: StatId,
    branch_event_resolved_targets: StatId,
    branch_event_link_writes: StatId,
    branch_event_without_link_writes: StatId,
    branch_event_squashes: StatId,
    branch_event_squashed_targets: StatId,
    branch_event_squashed_targets_with_link_writes: StatId,
    branch_event_squashed_targets_without_link_writes: StatId,
    branch_event_kinds: [RiscvO3RuntimeBranchEventKindStats; BranchTargetKind::COUNT],
    branch_aliases: RiscvO3RuntimeBranchAliasStats,
    fu_latency_instructions: StatId,
    fu_latency_cycles: StatId,
    fu_latency_classes: [RiscvO3RuntimeFuLatencyClassStats; O3RuntimeFuLatencyClass::COUNT],
    iq_insts_issued: StatId,
    iq_mem_insts_issued: StatId,
    iq_branch_insts_issued: StatId,
    iq_issued_inst_type_mem_read: StatId,
    iq_issued_inst_type_mem_write: StatId,
    iq_issued_inst_type_fu_classes: [StatId; O3RuntimeFuLatencyClass::COUNT],
    iq_issued_inst_type_fu_aliases: [StatId; O3RuntimeFuLatencyClass::COUNT],
    commit_committed_inst_type_mem_read: StatId,
    commit_committed_inst_type_mem_write: StatId,
    commit_committed_inst_type_fu_classes: [StatId; O3RuntimeFuLatencyClass::COUNT],
    commit_committed_inst_type_fu_aliases: [StatId; O3RuntimeFuLatencyClass::COUNT],
    iew_dispatched_insts: StatId,
    iew_insts_to_commit: StatId,
    iew_writeback_count: StatId,
    iew_producer_inst: StatId,
    iew_consumer_inst: StatId,
    iew_writeback_rate_ppm: StatId,
    iew_producer_consumer_fanout_ppm: StatId,
    iew_predicted_taken_incorrect: StatId,
    iew_predicted_not_taken_incorrect: StatId,
    iew_branch_mispredicts: StatId,
    commit_branch_mispredicts: StatId,
    max_rob_occupancy: StatId,
    max_lsq_occupancy: StatId,
    rename_map_entries: StatId,
}

impl RiscvO3RuntimeCpuStats {
    fn register(
        registry: &mut StatsRegistry,
        cpu: CpuId,
        single_cpu_run: bool,
    ) -> Result<Self, StatsError> {
        let prefix = format!("sim.host_actions.stats_dump.cpu{}.o3", cpu.get());
        let gem5_cpu_alias_prefix = if single_cpu_run {
            "system.cpu".to_string()
        } else {
            format!("system.cpu{}", cpu.get())
        };
        Ok(Self {
            instructions: register_o3_counter(registry, &prefix, "instructions", "Count")?,
            rob_allocations: register_o3_counter(registry, &prefix, "rob_allocations", "Count")?,
            rob_commits: register_o3_counter(registry, &prefix, "rob_commits", "Count")?,
            rename_writes: register_o3_counter(registry, &prefix, "rename_writes", "Count")?,
            lsq_loads: register_o3_counter(registry, &prefix, "lsq_loads", "Count")?,
            lsq_stores: register_o3_counter(registry, &prefix, "lsq_stores", "Count")?,
            lsq_load_bytes: register_o3_counter(registry, &prefix, "lsq_load_bytes", "Byte")?,
            lsq_store_bytes: register_o3_counter(registry, &prefix, "lsq_store_bytes", "Byte")?,
            lsq_store_to_load_forwarding_candidates: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_candidates",
                "Count",
            )?,
            lsq_store_to_load_forwarding_matches: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_matches",
                "Count",
            )?,
            lsq_store_to_load_forwarding_suppressed: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_suppressed",
                "Count",
            )?,
            lsq_store_to_load_forwarding_address_mismatches: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_address_mismatches",
                "Count",
            )?,
            lsq_store_to_load_forwarding_byte_mismatches: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_byte_mismatches",
                "Count",
            )?,
            structural_aliases: RiscvO3RuntimeStructuralAliasStats::register(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            lsq_operation_counts: register_o3_lsq_operation_counters(registry, &prefix)?,
            lsq_operation_alias_counts: register_o3_lsq_operation_alias_counters(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            lsq_operation_alias_total: register_o3_counter(
                registry,
                &gem5_cpu_alias_prefix,
                "lsq0.operation.total",
                "Count",
            )?,
            lsq_operation_forwarding_candidates: register_o3_lsq_operation_forwarding_counters(
                registry,
                &prefix,
                "forwarding_candidates",
            )?,
            lsq_operation_forwarding_matches: register_o3_lsq_operation_forwarding_counters(
                registry,
                &prefix,
                "forwarding_matches",
            )?,
            lsq_operation_forwarding_suppressed: register_o3_lsq_operation_forwarding_counters(
                registry,
                &prefix,
                "forwarding_suppressed",
            )?,
            lsq_operation_forwarding_address_mismatches:
                register_o3_lsq_operation_forwarding_counters(
                    registry,
                    &prefix,
                    "forwarding_address_mismatches",
                )?,
            lsq_operation_forwarding_byte_mismatches:
                register_o3_lsq_operation_forwarding_counters(
                    registry,
                    &prefix,
                    "forwarding_byte_mismatches",
                )?,
            lsq_operation_forwarding_candidate_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingCandidates",
                )?,
            lsq_operation_forwarding_match_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingMatches",
                )?,
            lsq_operation_forwarding_suppressed_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingSuppressed",
                )?,
            lsq_operation_forwarding_address_mismatch_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingAddressMismatches",
                )?,
            lsq_operation_forwarding_byte_mismatch_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingByteMismatches",
                )?,
            lsq_data_latency: register_o3_lsq_latency_counters(
                registry,
                &prefix,
                "lsq_data_latency",
            )?,
            lsq_operation_latency: register_o3_lsq_operation_latency_counters(registry, &prefix)?,
            lsq_ordering_counts: register_o3_lsq_ordering_counters(registry, &prefix)?,
            lsq_ordering_alias_counts: register_o3_lsq_ordering_alias_counters(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            lsq_ordering_alias_total: register_o3_counter(
                registry,
                &gem5_cpu_alias_prefix,
                "lsq0.ordering.total",
                "Count",
            )?,
            lsq_store_conditional_failures: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_conditional_failures",
                "Count",
            )?,
            branch_repair_targetless_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_repair_targetless_mismatches",
                "Count",
            )?,
            branch_repair_wrong_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_repair_wrong_targets",
                "Count",
            )?,
            branch_repair_direction_only_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_repair_direction_only_mismatches",
                "Count",
            )?,
            branch_repair_kinds: register_o3_branch_repair_kind_counters(registry, &prefix)?,
            branch_event_branches: register_o3_counter(
                registry,
                &prefix,
                "branch_event.branches",
                "Count",
            )?,
            branch_event_taken: register_o3_counter(
                registry,
                &prefix,
                "branch_event.taken",
                "Count",
            )?,
            branch_event_not_taken: register_o3_counter(
                registry,
                &prefix,
                "branch_event.not_taken",
                "Count",
            )?,
            branch_event_predicted_taken: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_taken",
                "Count",
            )?,
            branch_event_predicted_not_taken: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_not_taken",
                "Count",
            )?,
            branch_event_predicted_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_targets",
                "Count",
            )?,
            branch_event_predicted_target_matches: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_target_matches",
                "Count",
            )?,
            branch_event_predicted_target_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_target_mismatches",
                "Count",
            )?,
            branch_event_resolved_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_event.resolved_targets",
                "Count",
            )?,
            branch_event_link_writes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.link_writes",
                "Count",
            )?,
            branch_event_without_link_writes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.without_link_writes",
                "Count",
            )?,
            branch_event_squashes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.squashes",
                "Count",
            )?,
            branch_event_squashed_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_event.squashed_targets",
                "Count",
            )?,
            branch_event_squashed_targets_with_link_writes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.squashed_targets_with_link_writes",
                "Count",
            )?,
            branch_event_squashed_targets_without_link_writes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.squashed_targets_without_link_writes",
                "Count",
            )?,
            branch_event_kinds: register_o3_branch_event_kind_counters(registry, &prefix)?,
            branch_aliases: RiscvO3RuntimeBranchAliasStats::register(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            fu_latency_instructions: register_o3_counter(
                registry,
                &prefix,
                "fu_latency_instructions",
                "Count",
            )?,
            fu_latency_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_latency_cycles",
                "Cycle",
            )?,
            fu_latency_classes: register_o3_fu_latency_class_counters(registry, &prefix)?,
            iq_insts_issued: register_o3_counter(registry, &prefix, "iq.insts_issued", "Count")?,
            iq_mem_insts_issued: register_o3_counter(
                registry,
                &prefix,
                "iq.mem_insts_issued",
                "Count",
            )?,
            iq_branch_insts_issued: register_o3_counter(
                registry,
                &prefix,
                "iq.branch_insts_issued",
                "Count",
            )?,
            iq_issued_inst_type_mem_read: register_o3_counter(
                registry,
                &prefix,
                "iq.issued_inst_type.mem_read",
                "Count",
            )?,
            iq_issued_inst_type_mem_write: register_o3_counter(
                registry,
                &prefix,
                "iq.issued_inst_type.mem_write",
                "Count",
            )?,
            iq_issued_inst_type_fu_classes: register_o3_iq_fu_latency_class_counters(
                registry, &prefix,
            )?,
            iq_issued_inst_type_fu_aliases: register_o3_iq_fu_latency_class_alias_counters(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            commit_committed_inst_type_mem_read: register_o3_counter(
                registry,
                &prefix,
                "commit.committed_inst_type.mem_read",
                "Count",
            )?,
            commit_committed_inst_type_mem_write: register_o3_counter(
                registry,
                &prefix,
                "commit.committed_inst_type.mem_write",
                "Count",
            )?,
            commit_committed_inst_type_fu_classes: register_o3_commit_fu_latency_class_counters(
                registry, &prefix,
            )?,
            commit_committed_inst_type_fu_aliases:
                register_o3_commit_fu_latency_class_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                )?,
            iew_dispatched_insts: register_o3_counter(
                registry,
                &prefix,
                "iew.dispatched_insts",
                "Count",
            )?,
            iew_insts_to_commit: register_o3_counter(
                registry,
                &prefix,
                "iew.insts_to_commit",
                "Count",
            )?,
            iew_writeback_count: register_o3_counter(
                registry,
                &prefix,
                "iew.writeback_count",
                "Count",
            )?,
            iew_producer_inst: register_o3_counter(
                registry,
                &prefix,
                "iew.producer_inst",
                "Count",
            )?,
            iew_consumer_inst: register_o3_counter(
                registry,
                &prefix,
                "iew.consumer_inst",
                "Count",
            )?,
            iew_writeback_rate_ppm: register_o3_counter(
                registry,
                &prefix,
                "iew.writeback_rate_ppm",
                "Ppm",
            )?,
            iew_producer_consumer_fanout_ppm: register_o3_counter(
                registry,
                &prefix,
                "iew.producer_consumer_fanout_ppm",
                "Ppm",
            )?,
            iew_predicted_taken_incorrect: register_o3_counter(
                registry,
                &prefix,
                "iew.predicted_taken_incorrect",
                "Count",
            )?,
            iew_predicted_not_taken_incorrect: register_o3_counter(
                registry,
                &prefix,
                "iew.predicted_not_taken_incorrect",
                "Count",
            )?,
            iew_branch_mispredicts: register_o3_counter(
                registry,
                &prefix,
                "iew.branch_mispredicts",
                "Count",
            )?,
            commit_branch_mispredicts: register_o3_counter(
                registry,
                &prefix,
                "commit.branch_mispredicts",
                "Count",
            )?,
            max_rob_occupancy: register_o3_counter(
                registry,
                &prefix,
                "max_rob_occupancy",
                "Count",
            )?,
            max_lsq_occupancy: register_o3_counter(
                registry,
                &prefix,
                "max_lsq_occupancy",
                "Count",
            )?,
            rename_map_entries: register_o3_counter(
                registry,
                &prefix,
                "rename_map_entries",
                "Count",
            )?,
        })
    }

    fn increment_delta(
        self,
        registry: &mut StatsRegistry,
        previous: O3RuntimeStats,
        current: O3RuntimeStats,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        for (stat, previous, current) in [
            (
                self.instructions,
                previous.instructions(),
                current.instructions(),
            ),
            (
                self.rob_allocations,
                previous.rob_allocations(),
                current.rob_allocations(),
            ),
            (
                self.rob_commits,
                previous.rob_commits(),
                current.rob_commits(),
            ),
            (
                self.rename_writes,
                previous.rename_writes(),
                current.rename_writes(),
            ),
            (self.lsq_loads, previous.lsq_loads(), current.lsq_loads()),
            (self.lsq_stores, previous.lsq_stores(), current.lsq_stores()),
            (
                self.lsq_load_bytes,
                previous.lsq_load_bytes(),
                current.lsq_load_bytes(),
            ),
            (
                self.lsq_store_bytes,
                previous.lsq_store_bytes(),
                current.lsq_store_bytes(),
            ),
            (
                self.lsq_store_to_load_forwarding_candidates,
                previous.lsq_store_to_load_forwarding_candidates(),
                current.lsq_store_to_load_forwarding_candidates(),
            ),
            (
                self.lsq_store_to_load_forwarding_matches,
                previous.lsq_store_to_load_forwarding_matches(),
                current.lsq_store_to_load_forwarding_matches(),
            ),
            (
                self.lsq_store_to_load_forwarding_suppressed,
                previous.lsq_store_to_load_forwarding_suppressed(),
                current.lsq_store_to_load_forwarding_suppressed(),
            ),
            (
                self.lsq_store_to_load_forwarding_address_mismatches,
                previous.lsq_store_to_load_forwarding_address_mismatches(),
                current.lsq_store_to_load_forwarding_address_mismatches(),
            ),
            (
                self.lsq_store_to_load_forwarding_byte_mismatches,
                previous.lsq_store_to_load_forwarding_byte_mismatches(),
                current.lsq_store_to_load_forwarding_byte_mismatches(),
            ),
            (
                self.lsq_store_conditional_failures,
                previous.lsq_store_conditional_failures(),
                current.lsq_store_conditional_failures(),
            ),
            (
                self.branch_repair_targetless_mismatches,
                previous.branch_repair_targetless_mismatches(),
                current.branch_repair_targetless_mismatches(),
            ),
            (
                self.branch_repair_wrong_targets,
                previous.branch_repair_wrong_targets(),
                current.branch_repair_wrong_targets(),
            ),
            (
                self.branch_repair_direction_only_mismatches,
                previous.branch_repair_direction_only_mismatches(),
                current.branch_repair_direction_only_mismatches(),
            ),
            (
                self.branch_event_branches,
                previous.branch_events(),
                current.branch_events(),
            ),
            (
                self.branch_event_taken,
                previous.branch_event_taken(),
                current.branch_event_taken(),
            ),
            (
                self.branch_event_not_taken,
                previous.branch_event_not_taken(),
                current.branch_event_not_taken(),
            ),
            (
                self.branch_event_predicted_taken,
                previous.branch_event_predicted_taken(),
                current.branch_event_predicted_taken(),
            ),
            (
                self.branch_event_predicted_not_taken,
                previous.branch_event_predicted_not_taken(),
                current.branch_event_predicted_not_taken(),
            ),
            (
                self.branch_event_predicted_targets,
                previous.branch_event_predicted_targets(),
                current.branch_event_predicted_targets(),
            ),
            (
                self.branch_event_predicted_target_matches,
                previous.branch_event_predicted_target_matches(),
                current.branch_event_predicted_target_matches(),
            ),
            (
                self.branch_event_predicted_target_mismatches,
                previous.branch_event_predicted_target_mismatches(),
                current.branch_event_predicted_target_mismatches(),
            ),
            (
                self.branch_event_resolved_targets,
                previous.branch_event_resolved_targets(),
                current.branch_event_resolved_targets(),
            ),
            (
                self.branch_event_link_writes,
                previous.branch_event_link_writes(),
                current.branch_event_link_writes(),
            ),
            (
                self.branch_event_without_link_writes,
                previous.branch_event_without_link_writes(),
                current.branch_event_without_link_writes(),
            ),
            (
                self.branch_event_squashes,
                previous.branch_event_squashes(),
                current.branch_event_squashes(),
            ),
            (
                self.branch_event_squashed_targets,
                previous.branch_event_squashed_targets(),
                current.branch_event_squashed_targets(),
            ),
            (
                self.branch_event_squashed_targets_with_link_writes,
                previous.branch_event_squashed_targets_with_link_writes(),
                current.branch_event_squashed_targets_with_link_writes(),
            ),
            (
                self.branch_event_squashed_targets_without_link_writes,
                previous.branch_event_squashed_targets_without_link_writes(),
                current.branch_event_squashed_targets_without_link_writes(),
            ),
            (
                self.fu_latency_instructions,
                previous.fu_latency_instructions(),
                current.fu_latency_instructions(),
            ),
            (
                self.fu_latency_cycles,
                previous.fu_latency_cycles(),
                current.fu_latency_cycles(),
            ),
            (
                self.iq_insts_issued,
                previous.instructions(),
                current.instructions(),
            ),
            (
                self.iq_mem_insts_issued,
                previous.lsq_loads().saturating_add(previous.lsq_stores()),
                current.lsq_loads().saturating_add(current.lsq_stores()),
            ),
            (
                self.iq_branch_insts_issued,
                previous.iq_branch_insts_issued(),
                current.iq_branch_insts_issued(),
            ),
            (
                self.iq_issued_inst_type_mem_read,
                previous.lsq_loads(),
                current.lsq_loads(),
            ),
            (
                self.iq_issued_inst_type_mem_write,
                previous.lsq_stores(),
                current.lsq_stores(),
            ),
            (
                self.commit_committed_inst_type_mem_read,
                previous.lsq_loads(),
                current.lsq_loads(),
            ),
            (
                self.commit_committed_inst_type_mem_write,
                previous.lsq_stores(),
                current.lsq_stores(),
            ),
            (
                self.iew_dispatched_insts,
                previous.instructions(),
                current.instructions(),
            ),
            (
                self.iew_insts_to_commit,
                previous.rob_commits(),
                current.rob_commits(),
            ),
            (
                self.iew_writeback_count,
                previous.instructions(),
                current.instructions(),
            ),
            (
                self.iew_producer_inst,
                previous.iew_producer_insts(),
                current.iew_producer_insts(),
            ),
            (
                self.iew_consumer_inst,
                previous.iew_consumer_insts(),
                current.iew_consumer_insts(),
            ),
            (
                self.iew_predicted_taken_incorrect,
                previous.iew_predicted_taken_incorrect(),
                current.iew_predicted_taken_incorrect(),
            ),
            (
                self.iew_predicted_not_taken_incorrect,
                previous.iew_predicted_not_taken_incorrect(),
                current.iew_predicted_not_taken_incorrect(),
            ),
            (
                self.iew_branch_mispredicts,
                o3_branch_mispredicts(previous),
                o3_branch_mispredicts(current),
            ),
            (
                self.commit_branch_mispredicts,
                o3_branch_mispredicts(previous),
                o3_branch_mispredicts(current),
            ),
            (
                self.max_rob_occupancy,
                previous.max_rob_occupancy(),
                current.max_rob_occupancy(),
            ),
            (
                self.max_lsq_occupancy,
                previous.max_lsq_occupancy(),
                current.max_lsq_occupancy(),
            ),
            (
                self.rename_map_entries,
                previous.rename_map_entries(),
                current.rename_map_entries(),
            ),
        ] {
            let delta = current.saturating_sub(previous);
            if delta != 0 {
                registry.increment(stat, delta)?;
            }
        }
        self.structural_aliases
            .increment_delta(registry, previous, current)?;
        self.branch_aliases
            .increment_delta(registry, previous, current)?;
        self.set_iew_rate_snapshots(registry, current, in_order_pipeline_cycles)?;
        for kind in BranchTargetKind::ALL {
            let repair_stats = self.branch_repair_kinds[kind.index()];
            for (stat, previous, current) in [
                (
                    repair_stats.targetless_mismatch,
                    previous.branch_repair_targetless_mismatch_kind(kind),
                    current.branch_repair_targetless_mismatch_kind(kind),
                ),
                (
                    repair_stats.wrong_target,
                    previous.branch_repair_wrong_target_kind(kind),
                    current.branch_repair_wrong_target_kind(kind),
                ),
                (
                    repair_stats.direction_only,
                    previous.branch_repair_direction_only_kind(kind),
                    current.branch_repair_direction_only_kind(kind),
                ),
            ] {
                let delta = current.saturating_sub(previous);
                if delta != 0 {
                    registry.increment(stat, delta)?;
                }
            }
        }
        for kind in BranchTargetKind::ALL {
            let event_stats = self.branch_event_kinds[kind.index()];
            for (stat, previous, current) in [
                (
                    event_stats.kind,
                    previous.branch_event_kind(kind),
                    current.branch_event_kind(kind),
                ),
                (
                    event_stats.taken,
                    previous.branch_event_taken_kind(kind),
                    current.branch_event_taken_kind(kind),
                ),
                (
                    event_stats.predicted_taken,
                    previous.branch_event_predicted_taken_kind(kind),
                    current.branch_event_predicted_taken_kind(kind),
                ),
                (
                    event_stats.predicted_not_taken,
                    previous.branch_event_predicted_not_taken_kind(kind),
                    current.branch_event_predicted_not_taken_kind(kind),
                ),
                (
                    event_stats.predicted_target,
                    previous.branch_event_predicted_target_kind(kind),
                    current.branch_event_predicted_target_kind(kind),
                ),
                (
                    event_stats.predicted_target_match,
                    previous.branch_event_predicted_target_match_kind(kind),
                    current.branch_event_predicted_target_match_kind(kind),
                ),
                (
                    event_stats.predicted_target_mismatch,
                    previous.branch_event_predicted_target_mismatch_kind(kind),
                    current.branch_event_predicted_target_mismatch_kind(kind),
                ),
                (
                    event_stats.resolved_target,
                    previous.branch_event_resolved_target_kind(kind),
                    current.branch_event_resolved_target_kind(kind),
                ),
                (
                    event_stats.link_write,
                    previous.branch_event_link_write_kind(kind),
                    current.branch_event_link_write_kind(kind),
                ),
                (
                    event_stats.squash,
                    previous.branch_event_squash_kind(kind),
                    current.branch_event_squash_kind(kind),
                ),
                (
                    event_stats.squashed_target_link_write,
                    previous.branch_event_squashed_target_link_write_kind(kind),
                    current.branch_event_squashed_target_link_write_kind(kind),
                ),
                (
                    event_stats.squashed_target_without_link_write,
                    previous.branch_event_squashed_target_without_link_write_kind(kind),
                    current.branch_event_squashed_target_without_link_write_kind(kind),
                ),
            ] {
                let delta = current.saturating_sub(previous);
                if delta != 0 {
                    registry.increment(stat, delta)?;
                }
            }
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            let delta = current
                .fu_latency_class_instructions(class)
                .saturating_sub(previous.fu_latency_class_instructions(class));
            if delta != 0 {
                registry.increment(self.iq_issued_inst_type_fu_classes[class.index()], delta)?;
            }
            if delta != 0 {
                registry.increment(self.iq_issued_inst_type_fu_aliases[class.index()], delta)?;
            }
            if delta != 0 {
                registry.increment(
                    self.commit_committed_inst_type_fu_classes[class.index()],
                    delta,
                )?;
            }
            if delta != 0 {
                registry.increment(
                    self.commit_committed_inst_type_fu_aliases[class.index()],
                    delta,
                )?;
            }
        }
        let mut lsq_operation_delta_total = 0_u64;
        for operation in O3RuntimeLsqOperation::TRACKED {
            let delta = current
                .lsq_operation_count(operation)
                .saturating_sub(previous.lsq_operation_count(operation));
            lsq_operation_delta_total = lsq_operation_delta_total.saturating_add(delta);
            if delta != 0 {
                registry.increment(self.lsq_operation_counts[operation.index()], delta)?;
                registry.increment(self.lsq_operation_alias_counts[operation.index()], delta)?;
            }
        }
        if lsq_operation_delta_total != 0 {
            registry.increment(self.lsq_operation_alias_total, lsq_operation_delta_total)?;
        }
        for operation in O3RuntimeLsqOperation::TRACKED {
            let candidate_delta = current
                .lsq_operation_forwarding_candidates(operation)
                .saturating_sub(previous.lsq_operation_forwarding_candidates(operation));
            if candidate_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_candidates[operation.index()],
                    candidate_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_candidate_aliases[operation.index()],
                    candidate_delta,
                )?;
            }
            let match_delta = current
                .lsq_operation_forwarding_matches(operation)
                .saturating_sub(previous.lsq_operation_forwarding_matches(operation));
            if match_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_matches[operation.index()],
                    match_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_match_aliases[operation.index()],
                    match_delta,
                )?;
            }
            let suppressed_delta = current
                .lsq_operation_forwarding_suppressed(operation)
                .saturating_sub(previous.lsq_operation_forwarding_suppressed(operation));
            if suppressed_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_suppressed[operation.index()],
                    suppressed_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_suppressed_aliases[operation.index()],
                    suppressed_delta,
                )?;
            }
            let address_mismatch_delta = current
                .lsq_operation_forwarding_address_mismatches(operation)
                .saturating_sub(previous.lsq_operation_forwarding_address_mismatches(operation));
            if address_mismatch_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_address_mismatches[operation.index()],
                    address_mismatch_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_address_mismatch_aliases[operation.index()],
                    address_mismatch_delta,
                )?;
            }
            let byte_mismatch_delta = current
                .lsq_operation_forwarding_byte_mismatches(operation)
                .saturating_sub(previous.lsq_operation_forwarding_byte_mismatches(operation));
            if byte_mismatch_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_byte_mismatches[operation.index()],
                    byte_mismatch_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_byte_mismatch_aliases[operation.index()],
                    byte_mismatch_delta,
                )?;
            }
        }
        self.set_lsq_latency_snapshot(registry, current)?;
        let mut lsq_ordering_delta_total = 0_u64;
        for ordering in O3RuntimeLsqOrdering::TRACKED {
            let delta = current
                .lsq_ordering_count(ordering)
                .saturating_sub(previous.lsq_ordering_count(ordering));
            lsq_ordering_delta_total = lsq_ordering_delta_total.saturating_add(delta);
            if delta != 0 {
                registry.increment(self.lsq_ordering_counts[ordering.index()], delta)?;
                registry.increment(self.lsq_ordering_alias_counts[ordering.index()], delta)?;
            }
        }
        if lsq_ordering_delta_total != 0 {
            registry.increment(self.lsq_ordering_alias_total, lsq_ordering_delta_total)?;
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            let class_stats = self.fu_latency_classes[class.index()];
            for (stat, previous, current) in [
                (
                    class_stats.instructions,
                    previous.fu_latency_class_instructions(class),
                    current.fu_latency_class_instructions(class),
                ),
                (
                    class_stats.latency_cycles,
                    previous.fu_latency_class_cycles(class),
                    current.fu_latency_class_cycles(class),
                ),
            ] {
                let delta = current.saturating_sub(previous);
                if delta != 0 {
                    registry.increment(stat, delta)?;
                }
            }
        }
        Ok(())
    }

    fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        for (stat, value) in [
            (self.instructions, snapshot.instructions()),
            (self.rob_allocations, snapshot.rob_allocations()),
            (self.rob_commits, snapshot.rob_commits()),
            (self.rename_writes, snapshot.rename_writes()),
            (self.lsq_loads, snapshot.lsq_loads()),
            (self.lsq_stores, snapshot.lsq_stores()),
            (self.lsq_load_bytes, snapshot.lsq_load_bytes()),
            (self.lsq_store_bytes, snapshot.lsq_store_bytes()),
            (
                self.lsq_store_to_load_forwarding_candidates,
                snapshot.lsq_store_to_load_forwarding_candidates(),
            ),
            (
                self.lsq_store_to_load_forwarding_matches,
                snapshot.lsq_store_to_load_forwarding_matches(),
            ),
            (
                self.lsq_store_to_load_forwarding_suppressed,
                snapshot.lsq_store_to_load_forwarding_suppressed(),
            ),
            (
                self.lsq_store_to_load_forwarding_address_mismatches,
                snapshot.lsq_store_to_load_forwarding_address_mismatches(),
            ),
            (
                self.lsq_store_to_load_forwarding_byte_mismatches,
                snapshot.lsq_store_to_load_forwarding_byte_mismatches(),
            ),
            (
                self.lsq_store_conditional_failures,
                snapshot.lsq_store_conditional_failures(),
            ),
            (
                self.branch_repair_targetless_mismatches,
                snapshot.branch_repair_targetless_mismatches(),
            ),
            (
                self.branch_repair_wrong_targets,
                snapshot.branch_repair_wrong_targets(),
            ),
            (
                self.branch_repair_direction_only_mismatches,
                snapshot.branch_repair_direction_only_mismatches(),
            ),
            (self.branch_event_branches, snapshot.branch_events()),
            (self.branch_event_taken, snapshot.branch_event_taken()),
            (
                self.branch_event_not_taken,
                snapshot.branch_event_not_taken(),
            ),
            (
                self.branch_event_predicted_taken,
                snapshot.branch_event_predicted_taken(),
            ),
            (
                self.branch_event_predicted_not_taken,
                snapshot.branch_event_predicted_not_taken(),
            ),
            (
                self.branch_event_predicted_targets,
                snapshot.branch_event_predicted_targets(),
            ),
            (
                self.branch_event_predicted_target_matches,
                snapshot.branch_event_predicted_target_matches(),
            ),
            (
                self.branch_event_predicted_target_mismatches,
                snapshot.branch_event_predicted_target_mismatches(),
            ),
            (
                self.branch_event_resolved_targets,
                snapshot.branch_event_resolved_targets(),
            ),
            (
                self.branch_event_link_writes,
                snapshot.branch_event_link_writes(),
            ),
            (
                self.branch_event_without_link_writes,
                snapshot.branch_event_without_link_writes(),
            ),
            (self.branch_event_squashes, snapshot.branch_event_squashes()),
            (
                self.branch_event_squashed_targets,
                snapshot.branch_event_squashed_targets(),
            ),
            (
                self.branch_event_squashed_targets_with_link_writes,
                snapshot.branch_event_squashed_targets_with_link_writes(),
            ),
            (
                self.branch_event_squashed_targets_without_link_writes,
                snapshot.branch_event_squashed_targets_without_link_writes(),
            ),
            (
                self.fu_latency_instructions,
                snapshot.fu_latency_instructions(),
            ),
            (self.fu_latency_cycles, snapshot.fu_latency_cycles()),
            (self.iq_insts_issued, snapshot.instructions()),
            (
                self.iq_mem_insts_issued,
                snapshot.lsq_loads().saturating_add(snapshot.lsq_stores()),
            ),
            (
                self.iq_branch_insts_issued,
                snapshot.iq_branch_insts_issued(),
            ),
            (self.iq_issued_inst_type_mem_read, snapshot.lsq_loads()),
            (self.iq_issued_inst_type_mem_write, snapshot.lsq_stores()),
            (
                self.commit_committed_inst_type_mem_read,
                snapshot.lsq_loads(),
            ),
            (
                self.commit_committed_inst_type_mem_write,
                snapshot.lsq_stores(),
            ),
            (self.iew_dispatched_insts, snapshot.instructions()),
            (self.iew_insts_to_commit, snapshot.rob_commits()),
            (self.iew_writeback_count, snapshot.instructions()),
            (self.iew_producer_inst, snapshot.iew_producer_insts()),
            (self.iew_consumer_inst, snapshot.iew_consumer_insts()),
            (
                self.iew_predicted_taken_incorrect,
                snapshot.iew_predicted_taken_incorrect(),
            ),
            (
                self.iew_predicted_not_taken_incorrect,
                snapshot.iew_predicted_not_taken_incorrect(),
            ),
            (self.iew_branch_mispredicts, o3_branch_mispredicts(snapshot)),
            (
                self.commit_branch_mispredicts,
                o3_branch_mispredicts(snapshot),
            ),
            (self.max_rob_occupancy, snapshot.max_rob_occupancy()),
            (self.max_lsq_occupancy, snapshot.max_lsq_occupancy()),
            (self.rename_map_entries, snapshot.rename_map_entries()),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        self.structural_aliases.set_snapshot(registry, snapshot)?;
        self.branch_aliases.set_snapshot(registry, snapshot)?;
        self.set_iew_rate_snapshots(registry, snapshot, in_order_pipeline_cycles)?;
        let mut lsq_operation_total = 0_u64;
        for operation in O3RuntimeLsqOperation::TRACKED {
            let value = snapshot.lsq_operation_count(operation);
            lsq_operation_total = lsq_operation_total.saturating_add(value);
            registry.set_resettable_counter(self.lsq_operation_counts[operation.index()], value)?;
            registry.set_resettable_counter(
                self.lsq_operation_alias_counts[operation.index()],
                value,
            )?;
        }
        registry.set_resettable_counter(self.lsq_operation_alias_total, lsq_operation_total)?;
        for operation in O3RuntimeLsqOperation::TRACKED {
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_candidates[operation.index()],
                snapshot.lsq_operation_forwarding_candidates(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_matches[operation.index()],
                snapshot.lsq_operation_forwarding_matches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_suppressed[operation.index()],
                snapshot.lsq_operation_forwarding_suppressed(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_address_mismatches[operation.index()],
                snapshot.lsq_operation_forwarding_address_mismatches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_byte_mismatches[operation.index()],
                snapshot.lsq_operation_forwarding_byte_mismatches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_candidate_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_candidates(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_match_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_matches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_suppressed_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_suppressed(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_address_mismatch_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_address_mismatches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_byte_mismatch_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_byte_mismatches(operation),
            )?;
        }
        self.set_lsq_latency_snapshot(registry, snapshot)?;
        let mut lsq_ordering_total = 0_u64;
        for ordering in O3RuntimeLsqOrdering::TRACKED {
            let value = snapshot.lsq_ordering_count(ordering);
            lsq_ordering_total = lsq_ordering_total.saturating_add(value);
            registry.set_resettable_counter(self.lsq_ordering_counts[ordering.index()], value)?;
            registry
                .set_resettable_counter(self.lsq_ordering_alias_counts[ordering.index()], value)?;
        }
        registry.set_resettable_counter(self.lsq_ordering_alias_total, lsq_ordering_total)?;
        for kind in BranchTargetKind::ALL {
            let repair_stats = self.branch_repair_kinds[kind.index()];
            for (stat, value) in [
                (
                    repair_stats.targetless_mismatch,
                    snapshot.branch_repair_targetless_mismatch_kind(kind),
                ),
                (
                    repair_stats.wrong_target,
                    snapshot.branch_repair_wrong_target_kind(kind),
                ),
                (
                    repair_stats.direction_only,
                    snapshot.branch_repair_direction_only_kind(kind),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        for kind in BranchTargetKind::ALL {
            let event_stats = self.branch_event_kinds[kind.index()];
            for (stat, value) in [
                (event_stats.kind, snapshot.branch_event_kind(kind)),
                (event_stats.taken, snapshot.branch_event_taken_kind(kind)),
                (
                    event_stats.predicted_taken,
                    snapshot.branch_event_predicted_taken_kind(kind),
                ),
                (
                    event_stats.predicted_not_taken,
                    snapshot.branch_event_predicted_not_taken_kind(kind),
                ),
                (
                    event_stats.predicted_target,
                    snapshot.branch_event_predicted_target_kind(kind),
                ),
                (
                    event_stats.predicted_target_match,
                    snapshot.branch_event_predicted_target_match_kind(kind),
                ),
                (
                    event_stats.predicted_target_mismatch,
                    snapshot.branch_event_predicted_target_mismatch_kind(kind),
                ),
                (
                    event_stats.resolved_target,
                    snapshot.branch_event_resolved_target_kind(kind),
                ),
                (
                    event_stats.link_write,
                    snapshot.branch_event_link_write_kind(kind),
                ),
                (event_stats.squash, snapshot.branch_event_squash_kind(kind)),
                (
                    event_stats.squashed_target_link_write,
                    snapshot.branch_event_squashed_target_link_write_kind(kind),
                ),
                (
                    event_stats.squashed_target_without_link_write,
                    snapshot.branch_event_squashed_target_without_link_write_kind(kind),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            let class_stats = self.fu_latency_classes[class.index()];
            for (stat, value) in [
                (
                    class_stats.instructions,
                    snapshot.fu_latency_class_instructions(class),
                ),
                (
                    class_stats.latency_cycles,
                    snapshot.fu_latency_class_cycles(class),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            registry.set_resettable_counter(
                self.iq_issued_inst_type_fu_classes[class.index()],
                snapshot.fu_latency_class_instructions(class),
            )?;
            registry.set_resettable_counter(
                self.iq_issued_inst_type_fu_aliases[class.index()],
                snapshot.fu_latency_class_instructions(class),
            )?;
            registry.set_resettable_counter(
                self.commit_committed_inst_type_fu_classes[class.index()],
                snapshot.fu_latency_class_instructions(class),
            )?;
            registry.set_resettable_counter(
                self.commit_committed_inst_type_fu_aliases[class.index()],
                snapshot.fu_latency_class_instructions(class),
            )?;
        }
        Ok(())
    }

    fn set_iew_rate_snapshots(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        let writeback_rate = ratio_ppm(snapshot.instructions(), in_order_pipeline_cycles);
        let producer_consumer_fanout =
            ratio_ppm(snapshot.iew_producer_insts(), snapshot.iew_consumer_insts());
        registry.set_resettable_counter(self.iew_writeback_rate_ppm, writeback_rate)?;
        registry.set_resettable_counter(
            self.iew_producer_consumer_fanout_ppm,
            producer_consumer_fanout,
        )?;
        Ok(())
    }

    fn set_lsq_latency_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        set_o3_lsq_latency_counters(
            registry,
            self.lsq_data_latency,
            snapshot.lsq_data_latency_samples(),
            snapshot.lsq_data_latency_ticks(),
            snapshot.lsq_data_latency_max_ticks(),
            snapshot.lsq_data_latency_min_ticks(),
            snapshot.lsq_data_latency_avg_ticks(),
        )?;
        for operation in O3RuntimeLsqOperation::TRACKED {
            set_o3_lsq_latency_counters(
                registry,
                self.lsq_operation_latency[operation.index()],
                snapshot.lsq_operation_latency_samples(operation),
                snapshot.lsq_operation_latency_ticks(operation),
                snapshot.lsq_operation_latency_max_ticks(operation),
                snapshot.lsq_operation_latency_min_ticks(operation),
                snapshot.lsq_operation_latency_avg_ticks(operation),
            )?;
        }
        Ok(())
    }
}

fn register_o3_counter(
    registry: &mut StatsRegistry,
    prefix: &str,
    name: &str,
    unit: &str,
) -> Result<StatId, StatsError> {
    registry.register_counter(format!("{prefix}.{name}"), unit)
}

fn o3_branch_mispredicts(stats: O3RuntimeStats) -> u64 {
    stats.branch_repair_mispredicts()
}

fn ratio_ppm(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let ppm = u128::from(numerator).saturating_mul(1_000_000) / u128::from(denominator);
    ppm.min(u128::from(u64::MAX)) as u64
}

fn o3_lsq_operation_alias(operation: O3RuntimeLsqOperation) -> &'static str {
    match operation {
        O3RuntimeLsqOperation::None => "none",
        O3RuntimeLsqOperation::Load => "load",
        O3RuntimeLsqOperation::Store => "store",
        O3RuntimeLsqOperation::LoadReserved => "loadReserved",
        O3RuntimeLsqOperation::StoreConditional => "storeConditional",
        O3RuntimeLsqOperation::Atomic => "atomic",
        O3RuntimeLsqOperation::FloatLoad => "floatLoad",
        O3RuntimeLsqOperation::FloatStore => "floatStore",
        O3RuntimeLsqOperation::VectorLoad => "vectorLoad",
        O3RuntimeLsqOperation::VectorStore => "vectorStore",
    }
}

fn o3_lsq_ordering_alias(ordering: O3RuntimeLsqOrdering) -> &'static str {
    match ordering {
        O3RuntimeLsqOrdering::None => "none",
        O3RuntimeLsqOrdering::Acquire => "acquire",
        O3RuntimeLsqOrdering::Release => "release",
        O3RuntimeLsqOrdering::AcquireRelease => "acquireRelease",
    }
}

fn register_o3_lsq_operation_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq_operation.{}", operation.as_str()),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_lsq_operation_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq0.operation.{}", o3_lsq_operation_alias(operation)),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_lsq_operation_forwarding_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    suffix: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq_operation.{}_{}", operation.as_str(), suffix),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_lsq_operation_forwarding_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    suffix: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!(
                "lsq0.operation.{}.{}",
                o3_lsq_operation_alias(operation),
                suffix
            ),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_lsq_latency_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    stem: &str,
) -> Result<RiscvO3RuntimeLsqLatencyStats, StatsError> {
    Ok(RiscvO3RuntimeLsqLatencyStats {
        samples: register_o3_counter(registry, prefix, &format!("{stem}_samples"), "Count")?,
        ticks: register_o3_counter(registry, prefix, &format!("{stem}_ticks"), "Tick")?,
        max_ticks: register_o3_counter(registry, prefix, &format!("{stem}_max_ticks"), "Tick")?,
        min_ticks: register_o3_counter(registry, prefix, &format!("{stem}_min_ticks"), "Tick")?,
        avg_ticks: register_o3_counter(registry, prefix, &format!("{stem}_avg_ticks"), "Tick")?,
    })
}

fn register_o3_lsq_operation_latency_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeLsqLatencyStats; O3RuntimeLsqOperation::COUNT], StatsError> {
    let empty = RiscvO3RuntimeLsqLatencyStats {
        samples: StatId::new(0),
        ticks: StatId::new(0),
        max_ticks: StatId::new(0),
        min_ticks: StatId::new(0),
        avg_ticks: StatId::new(0),
    };
    let mut stats = [empty; O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_lsq_latency_counters(
            registry,
            prefix,
            &format!("lsq_operation.{}_latency", operation.as_str()),
        )?;
    }
    Ok(stats)
}

fn set_o3_lsq_latency_counters(
    registry: &mut StatsRegistry,
    stats: RiscvO3RuntimeLsqLatencyStats,
    samples: u64,
    ticks: u64,
    max_ticks: u64,
    min_ticks: u64,
    avg_ticks: u64,
) -> Result<(), StatsError> {
    for (stat, value) in [
        (stats.samples, samples),
        (stats.ticks, ticks),
        (stats.max_ticks, max_ticks),
        (stats.min_ticks, min_ticks),
        (stats.avg_ticks, avg_ticks),
    ] {
        registry.set_resettable_counter(stat, value)?;
    }
    Ok(())
}

fn register_o3_lsq_ordering_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeLsqOrdering::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOrdering::COUNT];
    for ordering in O3RuntimeLsqOrdering::TRACKED {
        stats[ordering.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq_ordering.{}", ordering.as_str()),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_lsq_ordering_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeLsqOrdering::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOrdering::COUNT];
    for ordering in O3RuntimeLsqOrdering::TRACKED {
        stats[ordering.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq0.ordering.{}", o3_lsq_ordering_alias(ordering)),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_branch_repair_kind_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeBranchRepairStats; BranchTargetKind::COUNT], StatsError> {
    let mut stats = [RiscvO3RuntimeBranchRepairStats {
        targetless_mismatch: StatId::new(0),
        wrong_target: StatId::new(0),
        direction_only: StatId::new(0),
    }; BranchTargetKind::COUNT];
    for kind in BranchTargetKind::ALL {
        let stat_name = kind.canonical_stat_name();
        stats[kind.index()] = RiscvO3RuntimeBranchRepairStats {
            targetless_mismatch: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair_targetless_mismatch_kind.{stat_name}"),
                "Count",
            )?,
            wrong_target: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair_wrong_target_kind.{stat_name}"),
                "Count",
            )?,
            direction_only: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair_direction_only_kind.{stat_name}"),
                "Count",
            )?,
        };
    }
    Ok(stats)
}

fn register_o3_branch_event_kind_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeBranchEventKindStats; BranchTargetKind::COUNT], StatsError> {
    let mut stats = [RiscvO3RuntimeBranchEventKindStats {
        kind: StatId::new(0),
        taken: StatId::new(0),
        predicted_taken: StatId::new(0),
        predicted_not_taken: StatId::new(0),
        predicted_target: StatId::new(0),
        predicted_target_match: StatId::new(0),
        predicted_target_mismatch: StatId::new(0),
        resolved_target: StatId::new(0),
        link_write: StatId::new(0),
        squash: StatId::new(0),
        squashed_target_link_write: StatId::new(0),
        squashed_target_without_link_write: StatId::new(0),
    }; BranchTargetKind::COUNT];
    for kind in BranchTargetKind::ALL {
        let stat_name = kind.canonical_stat_name();
        stats[kind.index()] = RiscvO3RuntimeBranchEventKindStats {
            kind: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.kind.{stat_name}"),
                "Count",
            )?,
            taken: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.taken_kind.{stat_name}"),
                "Count",
            )?,
            predicted_taken: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_taken_kind.{stat_name}"),
                "Count",
            )?,
            predicted_not_taken: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_not_taken_kind.{stat_name}"),
                "Count",
            )?,
            predicted_target: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_target_kind.{stat_name}"),
                "Count",
            )?,
            predicted_target_match: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_target_match_kind.{stat_name}"),
                "Count",
            )?,
            predicted_target_mismatch: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_target_mismatch_kind.{stat_name}"),
                "Count",
            )?,
            resolved_target: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.resolved_target_kind.{stat_name}"),
                "Count",
            )?,
            link_write: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.link_write_kind.{stat_name}"),
                "Count",
            )?,
            squash: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.squash_kind.{stat_name}"),
                "Count",
            )?,
            squashed_target_link_write: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.squashed_target_link_write_kind.{stat_name}"),
                "Count",
            )?,
            squashed_target_without_link_write: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.squashed_target_without_link_write_kind.{stat_name}"),
                "Count",
            )?,
        };
    }
    Ok(stats)
}

fn register_o3_fu_latency_class_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeFuLatencyClassStats; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [RiscvO3RuntimeFuLatencyClassStats {
        instructions: StatId::new(0),
        latency_cycles: StatId::new(0),
    }; O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        let stat_stem = class.stat_stem();
        stats[class.index()] = RiscvO3RuntimeFuLatencyClassStats {
            instructions: register_o3_counter(
                registry,
                prefix,
                &format!("fu_{stat_stem}_instructions"),
                "Count",
            )?,
            latency_cycles: register_o3_counter(
                registry,
                prefix,
                &format!("fu_{stat_stem}_latency_cycles"),
                "Cycle",
            )?,
        };
    }
    Ok(stats)
}

fn register_o3_iq_fu_latency_class_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        stats[class.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("iq.issued_inst_type.{}", o3_iq_fu_latency_class_stem(class)),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_iq_fu_latency_class_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        stats[class.index()] = register_o3_counter(
            registry,
            prefix,
            &format!(
                "iq.issuedInstType.{}",
                o3_fu_latency_class_inst_type_alias(class)
            ),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_commit_fu_latency_class_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        stats[class.index()] = register_o3_counter(
            registry,
            prefix,
            &format!(
                "commit.committed_inst_type.{}",
                o3_fu_latency_class_inst_type_stem(class)
            ),
            "Count",
        )?;
    }
    Ok(stats)
}

fn register_o3_commit_fu_latency_class_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        stats[class.index()] = register_o3_counter(
            registry,
            prefix,
            &format!(
                "commit.committedInstType.{}",
                o3_fu_latency_class_inst_type_alias(class)
            ),
            "Count",
        )?;
    }
    Ok(stats)
}

fn o3_iq_fu_latency_class_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    o3_fu_latency_class_inst_type_stem(class)
}

fn o3_fu_latency_class_inst_type_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "int_div",
        _ => class.stat_stem(),
    }
}

fn o3_fu_latency_class_inst_type_alias(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "IntMult",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "IntDiv",
        O3RuntimeFuLatencyClass::ScalarFloatAdd => "FloatAdd",
        O3RuntimeFuLatencyClass::ScalarFloatCompare => "FloatCmp",
        O3RuntimeFuLatencyClass::ScalarFloatMisc => "FloatMisc",
        O3RuntimeFuLatencyClass::ScalarFloatMul => "FloatMult",
        O3RuntimeFuLatencyClass::ScalarFloatFma => "FloatMultAcc",
        O3RuntimeFuLatencyClass::ScalarFloatDiv => "FloatDiv",
        O3RuntimeFuLatencyClass::ScalarFloatSqrt => "FloatSqrt",
        O3RuntimeFuLatencyClass::VectorIntegerMul => "SimdMult",
        O3RuntimeFuLatencyClass::VectorIntegerDiv => "SimdDiv",
        O3RuntimeFuLatencyClass::VectorFloatAdd => "SimdFloatAdd",
        O3RuntimeFuLatencyClass::VectorFloatCompare => "SimdFloatCmp",
        O3RuntimeFuLatencyClass::VectorFloatMisc => "SimdFloatMisc",
        O3RuntimeFuLatencyClass::VectorFloatMul => "SimdFloatMult",
        O3RuntimeFuLatencyClass::VectorFloatFma => "SimdFloatMultAcc",
        O3RuntimeFuLatencyClass::VectorFloatDiv => "SimdFloatDiv",
        O3RuntimeFuLatencyClass::VectorFloatSqrt => "SimdFloatSqrt",
    }
}

#[cfg(test)]
mod tests {
    use rem6_cpu::{CpuCore, CpuFetchConfig, CpuResetState, RiscvCore, RiscvCpuExecutionEvent};
    use rem6_isa_riscv::{Immediate, Register, RiscvExecutionRecord, RiscvInstruction};
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId};
    use rem6_stats::{StatResetPolicy, StatsRegistry};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;

    #[test]
    fn reset_snapshots_clears_active_o3_dump_cpu_filter() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu]).unwrap();
        let core = active_o3_core(cpu);

        o3_stats
            .record_cpu_snapshot(
                &mut registry,
                cpu,
                core.o3_runtime_stats(),
                core.in_order_pipeline_snapshot().cycle(),
            )
            .unwrap();
        assert_eq!(o3_stats.active_cpu_indices(), vec![0]);

        o3_stats.reset_snapshots([(cpu, core.in_order_pipeline_snapshot().cycle())]);

        assert!(
            o3_stats.active_cpu_indices().is_empty(),
            "stats reset must clear active O3 dump CPU filter until new post-reset O3 work"
        );

        o3_stats
            .record_cpu_snapshot(
                &mut registry,
                cpu,
                core.o3_runtime_stats(),
                core.in_order_pipeline_snapshot().cycle(),
            )
            .unwrap();
        assert_eq!(o3_stats.active_cpu_indices(), vec![0]);
    }

    #[test]
    fn sync_cpu_snapshot_clears_inactive_o3_dump_cpu_filter() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu]).unwrap();
        let core = active_o3_core(cpu);

        o3_stats
            .record_cpu_snapshot(
                &mut registry,
                cpu,
                core.o3_runtime_stats(),
                core.in_order_pipeline_snapshot().cycle(),
            )
            .unwrap();
        assert_eq!(o3_stats.active_cpu_indices(), vec![0]);

        o3_stats
            .sync_cpu_snapshot(&mut registry, cpu, O3RuntimeStats::default(), 0)
            .unwrap();

        assert!(
            o3_stats.active_cpu_indices().is_empty(),
            "restoring an inactive O3 snapshot must remove stale dump-filter membership"
        );
    }

    #[test]
    fn reset_snapshots_rebases_o3_writeback_rate_cycles() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu]).unwrap();

        o3_stats.reset_snapshots([(cpu, 100)]);
        o3_stats
            .record_cpu_snapshot(
                &mut registry,
                cpu,
                active_o3_core(cpu).o3_runtime_stats(),
                105,
            )
            .unwrap();

        let sample = stat_sample(
            &registry,
            105,
            "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
        );
        assert_eq!(sample.unit(), "Ppm");
        assert_eq!(sample.reset_policy(), StatResetPolicy::Resettable);
        assert_eq!(sample.value(), ratio_ppm(1, 5));
    }

    #[test]
    fn sync_cpu_snapshot_rebases_o3_writeback_rate_after_older_restore_cycle() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu]).unwrap();

        o3_stats.reset_snapshots([(cpu, 100)]);
        o3_stats
            .sync_cpu_snapshot(
                &mut registry,
                cpu,
                active_o3_core(cpu).o3_runtime_stats(),
                50,
            )
            .unwrap();

        let sample = stat_sample(
            &registry,
            50,
            "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
        );
        assert_eq!(sample.value(), ratio_ppm(1, 50));
    }

    fn stat_sample(registry: &StatsRegistry, tick: u64, path: &str) -> rem6_stats::StatSample {
        let snapshot = registry.snapshot(tick);
        snapshot
            .samples()
            .iter()
            .find(|sample| sample.path() == path)
            .cloned()
            .unwrap_or_else(|| panic!("missing stat sample {path}"))
    }

    fn active_o3_core(cpu: CpuId) -> RiscvCore {
        let reset = CpuResetState::new(
            cpu,
            PartitionId::new(cpu.get()),
            AgentId::new(cpu.get() + 1),
            Address::new(0x8000_0000),
        );
        let fetch = CpuFetchConfig::new(
            TransportEndpointId::new(format!("cpu{}.ifetch", cpu.get())).unwrap(),
            MemoryRouteId::new(0),
            CacheLineLayout::new(16).unwrap(),
            AccessSize::new(4).unwrap(),
        );
        let core = RiscvCore::new(CpuCore::new(reset, fetch).unwrap());
        core.record_o3_retired_instruction(&addi_event(cpu));
        core
    }

    fn addi_event(cpu: CpuId) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Addi {
            rd: Register::new(5).unwrap(),
            rs1: Register::new(0).unwrap(),
            imm: Immediate::new(7),
        };
        RiscvCpuExecutionEvent::new(
            rem6_cpu::CpuFetchEvent::completed(
                rem6_cpu::CpuFetchRecord::new(
                    1,
                    PartitionId::new(cpu.get()),
                    MemoryRouteId::new(0),
                    TransportEndpointId::new(format!("cpu{}.ifetch", cpu.get())).unwrap(),
                    MemoryRequestId::new(AgentId::new(cpu.get() + 1), 0),
                    Address::new(0x8000_0000),
                    AccessSize::new(4).unwrap(),
                ),
                0x0070_0293_u32.to_le_bytes().to_vec(),
            ),
            instruction,
            RiscvExecutionRecord::new(instruction, 0x8000_0000, 0x8000_0004, Vec::new(), None),
        )
    }
}
