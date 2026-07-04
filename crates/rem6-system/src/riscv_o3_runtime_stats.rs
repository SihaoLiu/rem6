use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, O3RuntimeStats};
use rem6_stats::{StatId, StatsError, StatsRegistry};

#[derive(Clone, Debug)]
pub struct RiscvO3RuntimeStats {
    cpus: BTreeSet<CpuId>,
    stats: BTreeMap<CpuId, RiscvO3RuntimeCpuStats>,
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
            previous: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub fn reset_snapshots(&self) {
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
        Ok(())
    }
}

impl Default for RiscvO3RuntimeStats {
    fn default() -> Self {
        Self {
            cpus: BTreeSet::new(),
            stats: BTreeMap::new(),
            previous: Arc::new(Mutex::new(BTreeMap::new())),
        }
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
    fu_latency_instructions: StatId,
    fu_latency_cycles: StatId,
    fu_integer_mul_instructions: StatId,
    fu_integer_mul_latency_cycles: StatId,
    fu_integer_div_instructions: StatId,
    fu_integer_div_latency_cycles: StatId,
    max_rob_occupancy: StatId,
    max_lsq_occupancy: StatId,
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
            fu_integer_mul_instructions: register_o3_counter(
                registry,
                &prefix,
                "fu_integer_mul_instructions",
                "Count",
            )?,
            fu_integer_mul_latency_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_integer_mul_latency_cycles",
                "Cycle",
            )?,
            fu_integer_div_instructions: register_o3_counter(
                registry,
                &prefix,
                "fu_integer_div_instructions",
                "Count",
            )?,
            fu_integer_div_latency_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_integer_div_latency_cycles",
                "Cycle",
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
                self.fu_integer_mul_instructions,
                previous.fu_integer_mul_instructions(),
                current.fu_integer_mul_instructions(),
            ),
            (
                self.fu_integer_mul_latency_cycles,
                previous.fu_integer_mul_latency_cycles(),
                current.fu_integer_mul_latency_cycles(),
            ),
            (
                self.fu_integer_div_instructions,
                previous.fu_integer_div_instructions(),
                current.fu_integer_div_instructions(),
            ),
            (
                self.fu_integer_div_latency_cycles,
                previous.fu_integer_div_latency_cycles(),
                current.fu_integer_div_latency_cycles(),
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
        ] {
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
                self.fu_latency_instructions,
                snapshot.fu_latency_instructions(),
            ),
            (self.fu_latency_cycles, snapshot.fu_latency_cycles()),
            (
                self.fu_integer_mul_instructions,
                snapshot.fu_integer_mul_instructions(),
            ),
            (
                self.fu_integer_mul_latency_cycles,
                snapshot.fu_integer_mul_latency_cycles(),
            ),
            (
                self.fu_integer_div_instructions,
                snapshot.fu_integer_div_instructions(),
            ),
            (
                self.fu_integer_div_latency_cycles,
                snapshot.fu_integer_div_latency_cycles(),
            ),
            (self.max_rob_occupancy, snapshot.max_rob_occupancy()),
            (self.max_lsq_occupancy, snapshot.max_lsq_occupancy()),
        ] {
            registry.set_resettable_counter(stat, value)?;
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
