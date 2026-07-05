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
}

impl RiscvO3RuntimeStats {
    pub fn register_for_cpus<I>(registry: &mut StatsRegistry, cpus: I) -> Result<Self, StatsError>
    where
        I: IntoIterator<Item = CpuId>,
    {
        let cpus = cpus.into_iter().collect::<BTreeSet<_>>();
        let stats = cpus
            .iter()
            .map(|cpu| RiscvO3RuntimeCpuStats::register(registry, *cpu).map(|stats| (*cpu, stats)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        Ok(Self {
            cpus,
            stats,
            active_cpus: Arc::new(Mutex::new(BTreeSet::new())),
            previous: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn reset_snapshots(&self) {
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
    }

    pub fn record_cpu_snapshot(
        &self,
        registry: &mut StatsRegistry,
        cpu: CpuId,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        let Some(stats) = self.stats.get(&cpu) else {
            return Ok(());
        };
        let mut previous = self.previous.lock().expect("O3 runtime stats lock");
        let previous_snapshot = previous.entry(cpu).or_default();
        stats.increment_delta(registry, *previous_snapshot, snapshot)?;
        *previous_snapshot = snapshot;
        self.sync_active_cpu(cpu, snapshot);
        Ok(())
    }

    pub fn sync_cpu_snapshot(
        &self,
        registry: &mut StatsRegistry,
        cpu: CpuId,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        let Some(stats) = self.stats.get(&cpu) else {
            return Ok(());
        };
        stats.set_snapshot(registry, snapshot)?;
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
}

impl Default for RiscvO3RuntimeStats {
    fn default() -> Self {
        Self {
            cpus: BTreeSet::new(),
            stats: BTreeMap::new(),
            active_cpus: Arc::new(Mutex::new(BTreeSet::new())),
            previous: Arc::new(Mutex::new(BTreeMap::new())),
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
    lsq_operation_counts: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_ordering_counts: [StatId; O3RuntimeLsqOrdering::COUNT],
    lsq_store_conditional_failures: StatId,
    branch_repair_targetless_mismatches: StatId,
    branch_repair_wrong_targets: StatId,
    branch_repair_direction_only_mismatches: StatId,
    branch_repair_kinds: [RiscvO3RuntimeBranchRepairStats; BranchTargetKind::COUNT],
    fu_latency_instructions: StatId,
    fu_latency_cycles: StatId,
    fu_latency_classes: [RiscvO3RuntimeFuLatencyClassStats; O3RuntimeFuLatencyClass::COUNT],
    max_rob_occupancy: StatId,
    max_lsq_occupancy: StatId,
    rename_map_entries: StatId,
}

impl RiscvO3RuntimeCpuStats {
    fn register(registry: &mut StatsRegistry, cpu: CpuId) -> Result<Self, StatsError> {
        let prefix = format!("sim.host_actions.stats_dump.cpu{}.o3", cpu.get());
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
            lsq_operation_counts: register_o3_lsq_operation_counters(registry, &prefix)?,
            lsq_ordering_counts: register_o3_lsq_ordering_counters(registry, &prefix)?,
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
        for operation in O3RuntimeLsqOperation::TRACKED {
            let delta = current
                .lsq_operation_count(operation)
                .saturating_sub(previous.lsq_operation_count(operation));
            if delta != 0 {
                registry.increment(self.lsq_operation_counts[operation.index()], delta)?;
            }
        }
        for ordering in O3RuntimeLsqOrdering::TRACKED {
            let delta = current
                .lsq_ordering_count(ordering)
                .saturating_sub(previous.lsq_ordering_count(ordering));
            if delta != 0 {
                registry.increment(self.lsq_ordering_counts[ordering.index()], delta)?;
            }
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
            (
                self.fu_latency_instructions,
                snapshot.fu_latency_instructions(),
            ),
            (self.fu_latency_cycles, snapshot.fu_latency_cycles()),
            (self.max_rob_occupancy, snapshot.max_rob_occupancy()),
            (self.max_lsq_occupancy, snapshot.max_lsq_occupancy()),
            (self.rename_map_entries, snapshot.rename_map_entries()),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        for operation in O3RuntimeLsqOperation::TRACKED {
            registry.set_resettable_counter(
                self.lsq_operation_counts[operation.index()],
                snapshot.lsq_operation_count(operation),
            )?;
        }
        for ordering in O3RuntimeLsqOrdering::TRACKED {
            registry.set_resettable_counter(
                self.lsq_ordering_counts[ordering.index()],
                snapshot.lsq_ordering_count(ordering),
            )?;
        }
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

#[cfg(test)]
mod tests {
    use rem6_cpu::{CpuCore, CpuFetchConfig, CpuResetState, RiscvCore, RiscvCpuExecutionEvent};
    use rem6_isa_riscv::{Immediate, Register, RiscvExecutionRecord, RiscvInstruction};
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId};
    use rem6_stats::StatsRegistry;
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;

    #[test]
    fn reset_snapshots_clears_active_o3_dump_cpu_filter() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu]).unwrap();
        let core = active_o3_core(cpu);

        o3_stats
            .record_cpu_snapshot(&mut registry, cpu, core.o3_runtime_stats())
            .unwrap();
        assert_eq!(o3_stats.active_cpu_indices(), vec![0]);

        o3_stats.reset_snapshots();

        assert!(
            o3_stats.active_cpu_indices().is_empty(),
            "stats reset must clear active O3 dump CPU filter until new post-reset O3 work"
        );

        o3_stats
            .record_cpu_snapshot(&mut registry, cpu, core.o3_runtime_stats())
            .unwrap();
        assert_eq!(o3_stats.active_cpu_indices(), vec![0]);
    }

    #[test]
    fn sync_cpu_snapshot_clears_inactive_o3_dump_cpu_filter() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu]).unwrap();

        o3_stats
            .record_cpu_snapshot(&mut registry, cpu, active_o3_core(cpu).o3_runtime_stats())
            .unwrap();
        assert_eq!(o3_stats.active_cpu_indices(), vec![0]);

        o3_stats
            .sync_cpu_snapshot(&mut registry, cpu, O3RuntimeStats::default())
            .unwrap();

        assert!(
            o3_stats.active_cpu_indices().is_empty(),
            "restoring an inactive O3 snapshot must remove stale dump-filter membership"
        );
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
