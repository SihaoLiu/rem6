use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, O3RuntimeStats};
use rem6_stats::{StatsError, StatsRegistry};

mod cpu;
mod groups;
mod helpers;

use self::cpu::RiscvO3RuntimeCpuStats;

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

#[cfg(test)]
mod tests {
    use rem6_cpu::{CpuCore, CpuFetchConfig, CpuResetState, RiscvCore, RiscvCpuExecutionEvent};
    use rem6_isa_riscv::{Immediate, Register, RiscvExecutionRecord, RiscvInstruction};
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId};
    use rem6_stats::{StatResetPolicy, StatsRegistry};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::helpers::ratio_ppm;
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
