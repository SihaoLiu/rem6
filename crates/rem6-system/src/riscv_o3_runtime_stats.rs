use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, O3RuntimeSnapshot, O3RuntimeStats, O3RuntimeTraceRecord};
use rem6_stats::{StatsError, StatsRegistry};

mod cpu;
mod event_summary;
mod event_window;
mod groups;
mod helpers;

use self::cpu::RiscvO3RuntimeCpuStats;
use self::event_summary::RiscvO3RuntimeEventSummarySnapshot;
use self::event_window::RiscvO3RuntimeEventWindowSnapshot;

#[derive(Clone, Debug)]
pub struct RiscvO3RuntimeStats {
    cpus: BTreeSet<CpuId>,
    stats: BTreeMap<CpuId, RiscvO3RuntimeCpuStats>,
    active_cpus: Arc<Mutex<BTreeSet<CpuId>>>,
    previous: Arc<Mutex<BTreeMap<CpuId, O3RuntimeStats>>>,
    cycle_baselines: Arc<Mutex<BTreeMap<CpuId, u64>>>,
    trace_offsets: Arc<Mutex<BTreeMap<CpuId, usize>>>,
    event_windows: Arc<Mutex<BTreeMap<CpuId, RiscvO3RuntimeEventWindowSnapshot>>>,
    event_summaries: Arc<Mutex<BTreeMap<CpuId, RiscvO3RuntimeEventSummarySnapshot>>>,
    host_reset_applied: Arc<Mutex<bool>>,
}

impl RiscvO3RuntimeStats {
    pub fn register_for_cpus<I>(
        registry: &mut StatsRegistry,
        cpus: I,
        trace_enabled: bool,
    ) -> Result<Self, StatsError>
    where
        I: IntoIterator<Item = CpuId>,
    {
        let cpus = cpus.into_iter().collect::<BTreeSet<_>>();
        let single_cpu_run = cpus.len() == 1;
        let stats = cpus
            .iter()
            .map(|cpu| {
                RiscvO3RuntimeCpuStats::register(registry, *cpu, single_cpu_run, trace_enabled)
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
            trace_offsets: Arc::new(Mutex::new(
                cpus.iter().copied().map(|cpu| (cpu, 0)).collect(),
            )),
            event_windows: Arc::new(Mutex::new(
                cpus.iter()
                    .copied()
                    .map(|cpu| (cpu, RiscvO3RuntimeEventWindowSnapshot::default()))
                    .collect(),
            )),
            event_summaries: Arc::new(Mutex::new(
                cpus.iter()
                    .copied()
                    .map(|cpu| (cpu, RiscvO3RuntimeEventSummarySnapshot::default()))
                    .collect(),
            )),
            host_reset_applied: Arc::new(Mutex::new(false)),
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
        self.reset_trace_offsets();
        self.reset_event_window_snapshots();
        self.reset_event_summary_snapshots();
        self.clear_host_reset_applied();
    }

    pub fn record_cpu_snapshot(
        &self,
        registry: &mut StatsRegistry,
        cpu: CpuId,
        snapshot: O3RuntimeStats,
        runtime_snapshot: &O3RuntimeSnapshot,
        trace_records: &[O3RuntimeTraceRecord],
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
            runtime_snapshot,
            resettable_pipeline_cycles,
        )?;
        let event_window_snapshot = self.observe_event_window_records(cpu, trace_records);
        stats.set_event_window_snapshot(registry, event_window_snapshot)?;
        let event_summary_snapshot = self.observe_event_summary_records(cpu, trace_records);
        stats.set_event_summary_snapshot(registry, &event_summary_snapshot)?;
        *previous_snapshot = snapshot;
        self.sync_active_cpu(cpu, snapshot);
        Ok(())
    }

    pub fn sync_cpu_snapshot(
        &self,
        registry: &mut StatsRegistry,
        cpu: CpuId,
        snapshot: O3RuntimeStats,
        runtime_snapshot: &O3RuntimeSnapshot,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        let Some(stats) = self.stats.get(&cpu) else {
            return Ok(());
        };
        let resettable_pipeline_cycles =
            self.resettable_pipeline_cycles(cpu, in_order_pipeline_cycles);
        self.reset_trace_offset(cpu);
        self.reset_event_window_snapshot(cpu);
        self.reset_event_summary_snapshot(cpu);
        stats.set_snapshot(
            registry,
            snapshot,
            runtime_snapshot,
            resettable_pipeline_cycles,
        )?;
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

    pub(crate) fn trace_record_offset(&self, cpu: CpuId) -> usize {
        self.trace_offsets
            .lock()
            .expect("O3 runtime stats lock")
            .get(&cpu)
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn set_trace_record_offset(&self, cpu: CpuId, offset: usize) {
        if self.cpus.contains(&cpu) {
            self.trace_offsets
                .lock()
                .expect("O3 runtime stats lock")
                .insert(cpu, offset);
        }
    }

    pub(crate) fn mark_host_reset_applied(&self) {
        *self
            .host_reset_applied
            .lock()
            .expect("O3 runtime stats lock") = true;
    }

    pub(crate) fn take_host_reset_applied(&self) -> bool {
        std::mem::take(
            &mut *self
                .host_reset_applied
                .lock()
                .expect("O3 runtime stats lock"),
        )
    }

    fn clear_host_reset_applied(&self) {
        *self
            .host_reset_applied
            .lock()
            .expect("O3 runtime stats lock") = false;
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

    fn observe_event_window_records(
        &self,
        cpu: CpuId,
        trace_records: &[O3RuntimeTraceRecord],
    ) -> RiscvO3RuntimeEventWindowSnapshot {
        let mut event_windows = self.event_windows.lock().expect("O3 runtime stats lock");
        let snapshot = event_windows.entry(cpu).or_default();
        for record in trace_records {
            snapshot.observe(record);
        }
        *snapshot
    }

    fn observe_event_summary_records(
        &self,
        cpu: CpuId,
        trace_records: &[O3RuntimeTraceRecord],
    ) -> RiscvO3RuntimeEventSummarySnapshot {
        let mut event_summaries = self.event_summaries.lock().expect("O3 runtime stats lock");
        let snapshot = event_summaries.entry(cpu).or_default();
        for record in trace_records {
            snapshot.observe(record);
        }
        snapshot.clone()
    }

    fn reset_event_window_snapshots(&self) {
        let mut event_windows = self.event_windows.lock().expect("O3 runtime stats lock");
        event_windows.clear();
        event_windows.extend(
            self.cpus
                .iter()
                .copied()
                .map(|cpu| (cpu, RiscvO3RuntimeEventWindowSnapshot::default())),
        );
    }

    fn reset_event_window_snapshot(&self, cpu: CpuId) {
        if self.cpus.contains(&cpu) {
            self.event_windows
                .lock()
                .expect("O3 runtime stats lock")
                .insert(cpu, RiscvO3RuntimeEventWindowSnapshot::default());
        }
    }

    fn reset_event_summary_snapshots(&self) {
        let mut event_summaries = self.event_summaries.lock().expect("O3 runtime stats lock");
        event_summaries.clear();
        event_summaries.extend(
            self.cpus
                .iter()
                .copied()
                .map(|cpu| (cpu, RiscvO3RuntimeEventSummarySnapshot::default())),
        );
    }

    fn reset_event_summary_snapshot(&self, cpu: CpuId) {
        if self.cpus.contains(&cpu) {
            self.event_summaries
                .lock()
                .expect("O3 runtime stats lock")
                .insert(cpu, RiscvO3RuntimeEventSummarySnapshot::default());
        }
    }

    fn reset_trace_offsets(&self) {
        let mut trace_offsets = self.trace_offsets.lock().expect("O3 runtime stats lock");
        trace_offsets.clear();
        trace_offsets.extend(self.cpus.iter().copied().map(|cpu| (cpu, 0)));
    }

    fn reset_trace_offset(&self, cpu: CpuId) {
        if self.cpus.contains(&cpu) {
            self.trace_offsets
                .lock()
                .expect("O3 runtime stats lock")
                .insert(cpu, 0);
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
            cycle_baselines: Arc::new(Mutex::new(BTreeMap::new())),
            trace_offsets: Arc::new(Mutex::new(BTreeMap::new())),
            event_windows: Arc::new(Mutex::new(BTreeMap::new())),
            event_summaries: Arc::new(Mutex::new(BTreeMap::new())),
            host_reset_applied: Arc::new(Mutex::new(false)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
    use rem6_cpu::{
        CpuCore, CpuDataConfig, CpuFetchConfig, CpuResetState, RiscvCluster,
        RiscvClusterDriveEvent, RiscvClusterTurn, RiscvCore, RiscvCoreDriveAction,
        RiscvCpuExecutionEvent, RiscvO3LiveDataHandoffTarget,
    };
    use rem6_isa_riscv::{Immediate, Register, RiscvExecutionRecord, RiscvInstruction};
    use rem6_kernel::{
        ParallelSchedulerContext, PartitionId, PartitionedScheduler, SchedulerContext,
    };
    use rem6_memory::{
        AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryRequestId,
        MemoryResponse,
    };
    use rem6_mmio::{
        MmioAccess, MmioBus, MmioDevice, MmioError, MmioRegisterBank, MmioRequest, MmioResponse,
        MmioRoute,
    };
    use rem6_stats::{StatResetPolicy, StatsRegistry};
    use rem6_transport::{
        MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TargetOutcome,
        TransportEndpointId,
    };

    use super::helpers::ratio_ppm;
    use super::*;
    use crate::{
        riscv_execution_mode_target_for_cpu, ExecutionMode, GuestEventId, GuestSourceId,
        HostEventPolicy, RiscvCoreCheckpointError, RiscvCoreCheckpointPort, RiscvInstructionStats,
        RiscvSystemRunDriver, RiscvTrapEventPort, SystemHostController, SystemHostEventPort,
        RISCV_O3_LIVE_DATA_HANDOFF_CHUNK,
    };

    struct CountingMmioDevice {
        responses: Arc<AtomicUsize>,
        bank: Mutex<MmioRegisterBank>,
    }

    impl MmioDevice for CountingMmioDevice {
        fn respond(
            &self,
            _context: &mut SchedulerContext<'_>,
            request: &MmioRequest,
        ) -> Result<MmioResponse, MmioError> {
            self.responses.fetch_add(1, Ordering::SeqCst);
            self.bank
                .lock()
                .expect("counted MMIO register bank lock")
                .respond(request)
        }

        fn respond_parallel(
            &self,
            _context: &mut ParallelSchedulerContext<'_>,
            request: &MmioRequest,
        ) -> Result<MmioResponse, MmioError> {
            self.responses.fetch_add(1, Ordering::SeqCst);
            self.bank
                .lock()
                .expect("counted MMIO register bank lock")
                .respond(request)
        }
    }

    #[test]
    fn schedule_riscv_system_events_from_turn_schedules_o3_writeback_wake() {
        let (driver, core, cluster, mut scheduler, turn, wake_tick) =
            completed_load_waiting_for_writeback();
        assert_eq!(core.read_register(Register::new(12).unwrap()), 0);
        assert!(!core.o3_runtime_snapshot().reorder_buffer()[0].is_ready());
        let events = driver
            .schedule_riscv_system_events_from_turn(&cluster, &mut scheduler, &turn, |_| {
                GuestEventId::new(1)
            })
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            scheduler.pending_event_snapshot(events[0]).unwrap().tick(),
            wake_tick
        );
        assert!(driver
            .schedule_riscv_system_events_from_turn(&cluster, &mut scheduler, &turn, |_| {
                GuestEventId::new(2)
            })
            .unwrap()
            .is_empty());

        let wake_turn = RiscvClusterTurn::scheduler(scheduler.run_until_idle());
        assert_eq!(scheduler.now(), wake_tick);

        driver
            .record_run_stats(&cluster, scheduler.now(), &wake_turn)
            .unwrap();
        assert_eq!(core.read_register(Register::new(12).unwrap()), 42);
        assert!(core.o3_runtime_snapshot().reorder_buffer().is_empty());
    }

    #[test]
    fn schedule_riscv_system_events_from_turn_parallel_schedules_o3_writeback_wake() {
        let (driver, core, cluster, mut scheduler, turn, wake_tick) =
            completed_load_waiting_for_writeback();
        assert_eq!(core.read_register(Register::new(12).unwrap()), 0);
        assert!(!core.o3_runtime_snapshot().reorder_buffer()[0].is_ready());
        let events = driver
            .schedule_riscv_system_events_from_turn_parallel(
                &cluster,
                &mut scheduler,
                &turn,
                |_| GuestEventId::new(1),
            )
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(
            scheduler.pending_event_snapshot(events[0]).unwrap().tick(),
            wake_tick
        );
        assert!(driver
            .schedule_riscv_system_events_from_turn_parallel(
                &cluster,
                &mut scheduler,
                &turn,
                |_| GuestEventId::new(2),
            )
            .unwrap()
            .is_empty());

        let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
        let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
        assert!(recorded
            .dispatches()
            .iter()
            .any(|dispatch| dispatch.tick() == wake_tick));
        let wake_turn = RiscvClusterTurn::parallel_scheduler(plan, recorded);
        assert!(scheduler.now() >= wake_tick);

        driver
            .record_run_stats(&cluster, scheduler.now(), &wake_turn)
            .unwrap();
        assert_eq!(core.read_register(Register::new(12).unwrap()), 42);
        assert!(core.o3_runtime_snapshot().reorder_buffer().is_empty());
    }

    #[test]
    fn record_run_stats_does_not_publish_scalar_load_before_admitted_tick() {
        let (_driver, core, _cluster, _scheduler, _turn, _wake_tick) =
            completed_load_waiting_for_writeback();
        assert!(!core.o3_runtime_snapshot().reorder_buffer()[0].is_ready());
        assert_eq!(core.read_register(Register::new(12).unwrap()), 0);
    }

    #[test]
    fn scalar_memory_execution_turn_defers_o3_retirement() {
        let cpu = CpuId::new(0);
        let (core, cluster, mut scheduler, transport) = scalar_memory_core(cpu);
        let driver = detailed_o3_driver_with_stats(cpu);

        core.issue_next_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(0x0000_2603_u32.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
        scheduler.run_until_idle();
        let execution = core.execute_next_completed_fetch().unwrap().unwrap();
        let turn = RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
            cpu,
            RiscvCoreDriveAction::InstructionExecuted(Box::new(execution)),
        )]);

        driver.record_o3_runtime_stats(&cluster, &turn).unwrap();

        assert_eq!(core.o3_runtime_stats().instructions(), 0);
        assert_eq!(core.o3_runtime_stats().lsq_loads(), 0);
        assert!(core.o3_runtime_snapshot().load_store_queue().is_empty());
        assert!(!core.o3_scalar_memory_lifecycle_is_quiescent());

        let component = CheckpointComponentId::new("cpu0").unwrap();
        let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
        let mut registry = CheckpointRegistry::new();
        port.register(&mut registry).unwrap();
        assert_eq!(
            port.capture_into(&mut registry),
            Err(CheckpointError::ComponentNotQuiescent {
                component: component.clone(),
            })
        );
        assert_eq!(registry.chunk(&component, "pc"), None);

        core.set_detailed_live_retire_gate_enabled(false);
        assert!(!core.o3_scalar_memory_lifecycle_is_quiescent());
    }

    #[test]
    fn scalar_memory_response_turn_records_o3_and_global_retirement_once_without_trace() {
        let cpu = CpuId::new(0);
        let (core, cluster, mut scheduler, transport) = scalar_memory_core(cpu);
        let driver = detailed_o3_driver_with_retirement_stats(cpu);

        core.issue_next_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(0x0000_2603_u32.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
        scheduler.run_until_idle();
        let execution = core.execute_next_completed_fetch().unwrap().unwrap();
        let execution_retirement = driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(execution)),
                )]),
            )
            .unwrap();
        assert_eq!(execution_retirement.count(), 0);
        assert_eq!(retired_instruction_count(&driver), 0);

        let response_event = core
            .issue_next_data_access(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                |delivery, _context| {
                    TargetOutcome::Respond(
                        MemoryResponse::completed(delivery.request(), Some(vec![0x2a, 0, 0, 0]))
                            .unwrap(),
                    )
                },
            )
            .unwrap()
            .unwrap();
        let issue_retirement = driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::DataAccessIssued {
                        event: response_event,
                    },
                )]),
            )
            .unwrap();
        assert_eq!(issue_retirement.count(), 0);
        assert_eq!(retired_instruction_count(&driver), 0);
        assert_eq!(core.o3_runtime_stats().instructions(), 0);
        assert_eq!(core.o3_runtime_snapshot().reorder_buffer().len(), 1);
        assert_eq!(core.o3_runtime_snapshot().load_store_queue().len(), 1);
        assert!(!core.o3_scalar_memory_lifecycle_is_quiescent());
        assert_eq!(
            driver_stat_value(
                &driver,
                scheduler.now(),
                "sim.host_actions.stats_dump.cpu0.o3.max_lsq_occupancy",
            ),
            1
        );
        assert_eq!(
            driver_stat_value(
                &driver,
                scheduler.now(),
                "sim.host_actions.stats_dump.cpu0.o3.snapshot.lsq.count",
            ),
            1
        );

        let response_summary = scheduler.run_until_idle();
        let response_retirement = driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::scheduler(response_summary),
            )
            .unwrap();
        assert_eq!(response_retirement.count(), 1);
        assert_eq!(response_retirement.last_tick(), Some(scheduler.now()));
        assert_eq!(retired_instruction_count(&driver), 1);

        assert_eq!(core.o3_runtime_stats().instructions(), 1);
        assert_eq!(core.o3_runtime_stats().lsq_loads(), 1);
        assert_eq!(core.o3_runtime_stats().max_lsq_occupancy(), 1);
        assert!(core.o3_runtime_snapshot().reorder_buffer().is_empty());
        assert!(core.o3_runtime_snapshot().load_store_queue().is_empty());
        assert!(core.o3_runtime_trace_records().is_empty());
        assert!(core.o3_scalar_memory_lifecycle_is_quiescent());
        assert_eq!(
            driver_stat_value(
                &driver,
                scheduler.now(),
                "sim.host_actions.stats_dump.cpu0.o3.instructions",
            ),
            1
        );
        assert_eq!(
            driver_stat_value(
                &driver,
                scheduler.now(),
                "sim.host_actions.stats_dump.cpu0.o3.snapshot.lsq.count",
            ),
            0
        );

        let repeated_retirement = driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::idle(scheduler.now()),
            )
            .unwrap();
        assert_eq!(repeated_retirement.count(), 0);
        assert_eq!(retired_instruction_count(&driver), 1);
        assert_eq!(core.o3_runtime_stats().instructions(), 1);
    }

    #[test]
    fn pending_scalar_memory_retires_after_detailed_mode_turns_off() {
        let cpu = CpuId::new(0);
        let (core, cluster, mut scheduler, transport) = scalar_memory_core(cpu);
        let driver = detailed_o3_driver_with_retirement_stats(cpu);

        core.issue_next_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(0x0000_2603_u32.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
        scheduler.run_until_idle();
        let execution = core.execute_next_completed_fetch().unwrap().unwrap();
        let execution_retirement = driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(execution)),
                )]),
            )
            .unwrap();
        assert_eq!(execution_retirement.count(), 0);

        {
            let controller = driver.trap_port().controller();
            controller
                .lock()
                .expect("system host controller lock")
                .executor_mut()
                .set_execution_mode(
                    riscv_execution_mode_target_for_cpu(cpu),
                    ExecutionMode::Timing,
                );
        }
        core.set_detailed_live_retire_gate_enabled(false);
        assert!(core.has_pending_o3_scalar_memory_retirement());

        let response_event = core
            .issue_next_data_access(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                |delivery, _context| {
                    TargetOutcome::Respond(
                        MemoryResponse::completed(delivery.request(), Some(vec![0x2a, 0, 0, 0]))
                            .unwrap(),
                    )
                },
            )
            .unwrap()
            .unwrap();
        driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::DataAccessIssued {
                        event: response_event,
                    },
                )]),
            )
            .unwrap();

        let response_summary = scheduler.run_until_idle();
        let response_retirement = driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::scheduler(response_summary),
            )
            .unwrap();

        assert_eq!(response_retirement.count(), 1);
        assert_eq!(retired_instruction_count(&driver), 1);
        assert_eq!(core.read_register(Register::new(12).unwrap()), 0x2a);
        assert_eq!(core.o3_runtime_stats().instructions(), 1);
        assert!(core.o3_scalar_memory_lifecycle_is_quiescent());
    }

    #[test]
    fn scalar_memory_response_retirement_respects_instruction_headroom() {
        let cpu = CpuId::new(0);
        let (core, cluster, mut scheduler, transport) = scalar_memory_core(cpu);
        let driver = detailed_o3_driver_with_retirement_stats(cpu);
        core.write_register(Register::new(2).unwrap(), 0x9000);

        issue_fetch_instruction(
            &core,
            &mut scheduler,
            &transport,
            load_word_instruction(0, 2, 12),
        );
        let older = core.execute_next_completed_fetch().unwrap().unwrap();
        driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(older)),
                )]),
            )
            .unwrap();

        issue_fetch_instruction(
            &core,
            &mut scheduler,
            &transport,
            load_word_instruction(64, 2, 13),
        );
        let older_response = core
            .issue_next_data_access(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                |delivery, _context| TargetOutcome::RespondAfter {
                    delay: 20,
                    response: MemoryResponse::completed(
                        delivery.request(),
                        Some(vec![0x2a, 0, 0, 0]),
                    )
                    .unwrap(),
                },
            )
            .unwrap()
            .unwrap();
        driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::DataAccessIssued {
                        event: older_response,
                    },
                )]),
            )
            .unwrap();

        let younger = core.execute_next_completed_fetch().unwrap().unwrap();
        driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(younger)),
                )]),
            )
            .unwrap();
        let younger_response = core
            .issue_next_data_access(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                |delivery, _context| {
                    TargetOutcome::Respond(
                        MemoryResponse::completed(delivery.request(), Some(vec![0x63, 0, 0, 0]))
                            .unwrap(),
                    )
                },
            )
            .unwrap()
            .unwrap();
        driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::DataAccessIssued {
                        event: younger_response,
                    },
                )]),
            )
            .unwrap();

        let responses = scheduler.run_until_idle();
        let first = driver
            .record_run_stats_with_retirement_budget(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::scheduler(responses),
                1,
            )
            .unwrap();
        assert_eq!(first.count(), 1);
        assert_eq!(retired_instruction_count(&driver), 1);
        assert_eq!(core.pending_o3_scalar_memory_retirement_count(), 1);
        assert_eq!(core.read_register(Register::new(12).unwrap()), 0x2a);
        assert_eq!(core.read_register(Register::new(13).unwrap()), 0);

        let second = driver
            .record_run_stats_with_retirement_budget(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::idle(scheduler.now()),
                1,
            )
            .unwrap();
        assert_eq!(second.count(), 1);
        assert_eq!(retired_instruction_count(&driver), 2);
        assert_eq!(core.pending_o3_scalar_memory_retirement_count(), 0);
        assert_eq!(core.read_register(Register::new(13).unwrap()), 0x63);
    }

    #[test]
    fn scalar_memory_mmio_issue_remains_resident_until_response() {
        let cpu = CpuId::new(0);
        let (core, cluster, mut scheduler, transport) = scalar_memory_core(cpu);
        let driver = detailed_o3_driver(cpu);

        core.issue_next_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(0x0000_2603_u32.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
        scheduler.run_until_idle();
        let execution = core.execute_next_completed_fetch().unwrap().unwrap();
        driver
            .record_o3_runtime_stats(
                &cluster,
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(execution)),
                )]),
            )
            .unwrap();

        let mut bank =
            MmioRegisterBank::new(Address::new(0), AccessSize::new(0x100).unwrap()).unwrap();
        bank.insert_register(
            0,
            AccessSize::new(4).unwrap(),
            MmioAccess::ReadOnly,
            vec![0x2a, 0, 0, 0],
        )
        .unwrap();
        let mut bus = MmioBus::new();
        let device_responses = Arc::new(AtomicUsize::new(0));
        bus.insert_device(
            AddressRange::new(Address::new(0), AccessSize::new(0x100).unwrap()).unwrap(),
            MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
            CountingMmioDevice {
                responses: Arc::clone(&device_responses),
                bank: Mutex::new(bank),
            },
        )
        .unwrap();
        let response_event = core
            .issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
            .unwrap()
            .unwrap();

        driver
            .record_o3_runtime_stats(
                &cluster,
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::DataAccessIssued {
                        event: response_event,
                    },
                )]),
            )
            .unwrap();

        assert_eq!(core.o3_runtime_stats().instructions(), 0);
        assert_eq!(core.o3_runtime_stats().lsq_loads(), 0);
        assert_eq!(core.o3_runtime_stats().max_lsq_occupancy(), 1);
        assert_eq!(core.o3_runtime_snapshot().reorder_buffer().len(), 1);
        assert_eq!(core.o3_runtime_snapshot().load_store_queue().len(), 1);
        assert!(!core.o3_scalar_memory_lifecycle_is_quiescent());
        let handoff = core
            .capture_o3_live_data_handoff()
            .expect("resident MMIO load should have typed live handoff authority");
        assert_eq!(handoff.entries().len(), 1);
        assert_eq!(handoff.younger_rows(), 0);
        assert_eq!(
            handoff.entries()[0].target(),
            RiscvO3LiveDataHandoffTarget::Mmio {
                route: MmioRoute::new(PartitionId::new(0), PartitionId::new(1), 2, 2).unwrap(),
            }
        );
        let component = CheckpointComponentId::new("cpu0").unwrap();
        let port = RiscvCoreCheckpointPort::new(component.clone(), core.clone());
        let mut registry = CheckpointRegistry::new();
        port.register(&mut registry).unwrap();
        registry
            .write_chunk(
                &component,
                RISCV_O3_LIVE_DATA_HANDOFF_CHUNK,
                handoff.encode(),
            )
            .unwrap();
        assert_eq!(
            port.restore_from(&registry),
            Err(RiscvCoreCheckpointError::LiveDataHandoffNotRestorable { component })
        );
        assert_eq!(device_responses.load(Ordering::SeqCst), 0);

        {
            let controller = driver.trap_port().controller();
            controller
                .lock()
                .expect("system host controller lock")
                .executor_mut()
                .set_execution_mode(
                    riscv_execution_mode_target_for_cpu(cpu),
                    ExecutionMode::Timing,
                );
        }
        core.set_detailed_live_retire_gate_enabled(false);

        scheduler.run_until_idle_parallel().unwrap();
        assert_eq!(device_responses.load(Ordering::SeqCst), 1);
        driver
            .record_o3_runtime_stats(&cluster, &RiscvClusterTurn::idle(scheduler.now()))
            .unwrap();
        assert_eq!(core.o3_runtime_stats().instructions(), 1);
        assert_eq!(core.o3_runtime_stats().lsq_loads(), 1);
        assert!(core.o3_runtime_snapshot().reorder_buffer().is_empty());
        assert!(core.o3_runtime_snapshot().load_store_queue().is_empty());
        assert!(core.o3_scalar_memory_lifecycle_is_quiescent());
    }

    #[test]
    fn reset_snapshots_clears_active_o3_dump_cpu_filter() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu], false).unwrap();
        let core = active_o3_core(cpu);
        let runtime_snapshot = core.o3_runtime_snapshot();

        o3_stats
            .record_cpu_snapshot(
                &mut registry,
                cpu,
                core.o3_runtime_stats(),
                &runtime_snapshot,
                &[],
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
                &runtime_snapshot,
                &[],
                core.in_order_pipeline_snapshot().cycle(),
            )
            .unwrap();
        assert_eq!(o3_stats.active_cpu_indices(), vec![0]);
    }

    #[test]
    fn sync_cpu_snapshot_clears_inactive_o3_dump_cpu_filter() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu], false).unwrap();
        let core = active_o3_core(cpu);
        let runtime_snapshot = core.o3_runtime_snapshot();

        o3_stats
            .record_cpu_snapshot(
                &mut registry,
                cpu,
                core.o3_runtime_stats(),
                &runtime_snapshot,
                &[],
                core.in_order_pipeline_snapshot().cycle(),
            )
            .unwrap();
        assert_eq!(o3_stats.active_cpu_indices(), vec![0]);

        let empty_snapshot = RiscvCore::default_o3_runtime_checkpoint_payload()
            .snapshot()
            .clone();
        o3_stats
            .sync_cpu_snapshot(
                &mut registry,
                cpu,
                O3RuntimeStats::default(),
                &empty_snapshot,
                0,
            )
            .unwrap();

        assert!(
            o3_stats.active_cpu_indices().is_empty(),
            "restoring an inactive O3 snapshot must remove stale dump-filter membership"
        );
    }

    #[test]
    fn sync_cpu_snapshot_clears_o3_event_summary_dump_rows() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu], true).unwrap();
        let core = active_o3_trace_core(cpu);
        let runtime_snapshot = core.o3_runtime_snapshot();
        let trace_records = core.o3_runtime_trace_records();
        assert!(
            !trace_records.is_empty(),
            "active O3 fixture should emit trace records"
        );

        o3_stats
            .record_cpu_snapshot(
                &mut registry,
                cpu,
                core.o3_runtime_stats(),
                &runtime_snapshot,
                &trace_records,
                core.in_order_pipeline_snapshot().cycle(),
            )
            .unwrap();
        let sample = stat_sample(
            &registry,
            1,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
        );
        assert_eq!(sample.reset_policy(), StatResetPolicy::Resettable);
        assert!(sample.value() > 0);

        let empty_snapshot = RiscvCore::default_o3_runtime_checkpoint_payload()
            .snapshot()
            .clone();
        o3_stats
            .sync_cpu_snapshot(
                &mut registry,
                cpu,
                O3RuntimeStats::default(),
                &empty_snapshot,
                0,
            )
            .unwrap();

        let sample = stat_sample(
            &registry,
            2,
            "sim.host_actions.stats_dump.cpu0.o3.event_summary.records",
        );
        assert_eq!(
            sample.value(),
            0,
            "syncing an inactive O3 snapshot must clear stale resettable event-summary rows"
        );
    }

    #[test]
    fn reset_snapshots_rebases_o3_writeback_rate_cycles() {
        let cpu = CpuId::new(0);
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu], false).unwrap();
        let core = active_o3_core(cpu);
        let runtime_snapshot = core.o3_runtime_snapshot();

        o3_stats.reset_snapshots([(cpu, 100)]);
        o3_stats
            .record_cpu_snapshot(
                &mut registry,
                cpu,
                core.o3_runtime_stats(),
                &runtime_snapshot,
                &[],
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
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu], false).unwrap();
        let core = active_o3_core(cpu);
        let runtime_snapshot = core.o3_runtime_snapshot();

        o3_stats.reset_snapshots([(cpu, 100)]);
        o3_stats
            .sync_cpu_snapshot(
                &mut registry,
                cpu,
                core.o3_runtime_stats(),
                &runtime_snapshot,
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
        active_o3_core_with_trace(cpu, false)
    }

    fn active_o3_trace_core(cpu: CpuId) -> RiscvCore {
        active_o3_core_with_trace(cpu, true)
    }

    fn active_o3_core_with_trace(cpu: CpuId, trace_enabled: bool) -> RiscvCore {
        let core = inactive_o3_core(cpu);
        core.record_o3_retired_instruction_with_trace(&addi_event(cpu), trace_enabled);
        core
    }

    fn inactive_o3_core(cpu: CpuId) -> RiscvCore {
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
        RiscvCore::new(CpuCore::new(reset, fetch).unwrap())
    }

    fn scalar_memory_core(
        cpu: CpuId,
    ) -> (
        RiscvCore,
        RiscvCluster,
        PartitionedScheduler,
        MemoryTransport,
    ) {
        let scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
        let mut transport = MemoryTransport::new();
        let fetch_route = transport
            .add_route(
                MemoryRoute::new(
                    TransportEndpointId::new("cpu0.ifetch").unwrap(),
                    PartitionId::new(0),
                    TransportEndpointId::new("memory.ifetch").unwrap(),
                    PartitionId::new(1),
                    2,
                    3,
                )
                .unwrap(),
            )
            .unwrap();
        let data_route = transport
            .add_route(
                MemoryRoute::new(
                    TransportEndpointId::new("cpu0.dmem").unwrap(),
                    PartitionId::new(0),
                    TransportEndpointId::new("memory.dmem").unwrap(),
                    PartitionId::new(1),
                    2,
                    3,
                )
                .unwrap(),
            )
            .unwrap();
        let reset = CpuResetState::new(
            cpu,
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(0x8000_0000),
        );
        let fetch = CpuFetchConfig::new(
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            fetch_route,
            CacheLineLayout::new(16).unwrap(),
            AccessSize::new(4).unwrap(),
        );
        let core = RiscvCore::with_data(
            CpuCore::new(reset, fetch).unwrap(),
            CpuDataConfig::new(
                TransportEndpointId::new("cpu0.dmem").unwrap(),
                data_route,
                CacheLineLayout::new(16).unwrap(),
            ),
        );
        core.set_detailed_live_retire_gate_enabled(true);
        let cluster = RiscvCluster::new([core.clone()]).unwrap();
        (core, cluster, scheduler, transport)
    }

    fn completed_load_waiting_for_writeback() -> (
        RiscvSystemRunDriver,
        RiscvCore,
        RiscvCluster,
        PartitionedScheduler,
        RiscvClusterTurn,
        u64,
    ) {
        let cpu = CpuId::new(0);
        let (core, cluster, mut scheduler, transport) = scalar_memory_core(cpu);
        let driver = detailed_o3_driver_with_stats(cpu);
        core.write_register(Register::new(2).unwrap(), 0x9000);
        issue_fetch_instruction(
            &core,
            &mut scheduler,
            &transport,
            load_word_instruction(0, 2, 12),
        );
        let execution = core.execute_next_completed_fetch().unwrap().unwrap();
        driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(execution)),
                )]),
            )
            .unwrap();
        let issued = core
            .issue_next_data_access(
                &mut scheduler,
                &transport,
                MemoryTrace::new(),
                |delivery, _context| {
                    TargetOutcome::Respond(
                        MemoryResponse::completed(delivery.request(), Some(vec![0x2a, 0, 0, 0]))
                            .unwrap(),
                    )
                },
            )
            .unwrap()
            .unwrap();
        driver
            .record_run_stats(
                &cluster,
                scheduler.now(),
                &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::DataAccessIssued { event: issued },
                )]),
            )
            .unwrap();
        let turn = RiscvClusterTurn::scheduler(scheduler.run_until_idle());
        driver
            .record_run_stats(&cluster, scheduler.now(), &turn)
            .unwrap();
        let wake_tick = core
            .requested_o3_writeback_wake_tick(scheduler.now())
            .unwrap();
        (driver, core, cluster, scheduler, turn, wake_tick)
    }

    fn issue_fetch_instruction(
        core: &RiscvCore,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        instruction: u32,
    ) {
        core.issue_next_fetch(
            scheduler,
            transport,
            MemoryTrace::new(),
            move |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(instruction.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
        scheduler.run_until_idle();
    }

    fn load_word_instruction(imm: i32, rs1: u8, rd: u8) -> u32 {
        (((imm as u32) & 0x0fff) << 20)
            | (u32::from(rs1) << 15)
            | (0b010 << 12)
            | (u32::from(rd) << 7)
            | 0x03
    }

    fn detailed_o3_driver(cpu: CpuId) -> RiscvSystemRunDriver {
        detailed_o3_driver_with_registry(cpu, StatsRegistry::new())
    }

    fn detailed_o3_driver_with_stats(cpu: CpuId) -> RiscvSystemRunDriver {
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu], false).unwrap();
        detailed_o3_driver_with_registry(cpu, registry).with_o3_runtime_stats(o3_stats)
    }

    fn detailed_o3_driver_with_retirement_stats(cpu: CpuId) -> RiscvSystemRunDriver {
        let mut registry = StatsRegistry::new();
        let o3_stats = RiscvO3RuntimeStats::register_for_cpus(&mut registry, [cpu], false).unwrap();
        let driver = detailed_o3_driver_with_registry(cpu, registry);
        RiscvSystemRunDriver::with_instruction_stats(
            driver.trap_port().clone(),
            RiscvInstructionStats::for_cpus([cpu]),
        )
        .with_o3_runtime_stats(o3_stats)
    }

    fn retired_instruction_count(driver: &RiscvSystemRunDriver) -> u64 {
        driver
            .instruction_stats()
            .expect("test driver instruction stats")
            .retired_instruction_probe_snapshot()
            .tracker()
            .counter()
    }

    fn detailed_o3_driver_with_registry(
        cpu: CpuId,
        registry: StatsRegistry,
    ) -> RiscvSystemRunDriver {
        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            registry,
        )));
        controller
            .lock()
            .unwrap()
            .executor_mut()
            .set_execution_mode(
                riscv_execution_mode_target_for_cpu(cpu),
                ExecutionMode::Detailed,
            );
        let host_port =
            SystemHostEventPort::with_controller(PartitionId::new(1), 2, controller).unwrap();
        RiscvSystemRunDriver::new(RiscvTrapEventPort::new(host_port, GuestSourceId::new(1)))
    }

    fn driver_stat_value(driver: &RiscvSystemRunDriver, tick: u64, path: &str) -> u64 {
        let controller = driver.trap_port().controller();
        let controller = controller.lock().expect("system host controller lock");
        stat_sample(controller.executor().stats(), tick, path).value()
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
