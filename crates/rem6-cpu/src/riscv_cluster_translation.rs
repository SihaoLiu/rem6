use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler, Tick};
use rem6_memory::TranslationPageMap;
use rem6_mmio::MmioBus;
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::riscv_cluster_scheduler::drive_parallel_scheduler_turn_until_tick;
use crate::{
    CpuId, RiscvCluster, RiscvClusterDriveEvent, RiscvClusterError, RiscvClusterTurn, RiscvCore,
    RiscvCpuError,
};

pub(crate) fn schedule_pending_data_translation_wake(
    cpu: CpuId,
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
) -> Result<(), RiscvClusterError> {
    let Some(ready_tick) = core.next_data_translation_ready_tick() else {
        return Ok(());
    };
    let next_pending_tick = scheduler
        .next_pending_tick(core.partition())
        .map_err(RiscvClusterError::Scheduler)?;
    if next_pending_tick.is_some_and(|tick| tick <= ready_tick) {
        return Ok(());
    }
    scheduler
        .schedule_parallel_at(core.partition(), ready_tick, |_context| {})
        .map_err(|error| RiscvClusterError::Core {
            cpu,
            error: RiscvCpuError::Scheduler(error),
        })?;
    Ok(())
}

pub(crate) fn can_retire_mmio_aware_translated_fetch_pending(
    cpu: CpuId,
    core: &RiscvCore,
    bus: &MmioBus,
) -> Result<bool, RiscvClusterError> {
    core.can_retire_completed_fetch_while_mmio_aware_cached_translated_memory_fetch_pending(bus)
        .map_err(|error| RiscvClusterError::Core { cpu, error })
}

impl RiscvCluster {
    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn_parallel_with_mmio_and_data_translation_until_tick<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        fetch_responder: F,
        data_responder: D,
        tick_limit: Tick,
    ) -> Result<Option<RiscvClusterTurn>, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        self.drive_turn_parallel_with_data_translation_until_tick_with(
            scheduler,
            tick_limit,
            |scheduler| {
                self.drive_ready_cores_parallel_with_mmio_and_data_translation(
                    scheduler,
                    transport,
                    bus,
                    fetch_trace,
                    data_trace,
                    page_map,
                    fetch_responder,
                    data_responder,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn_parallel_with_data_translation_until_tick<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        fetch_responder: F,
        data_responder: D,
        tick_limit: Tick,
    ) -> Result<Option<RiscvClusterTurn>, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        self.drive_turn_parallel_with_data_translation_until_tick_with(
            scheduler,
            tick_limit,
            |scheduler| {
                self.drive_ready_cores_parallel_with_data_translation(
                    scheduler,
                    transport,
                    fetch_trace,
                    data_trace,
                    page_map,
                    fetch_responder,
                    data_responder,
                )
            },
        )
    }

    fn drive_turn_parallel_with_data_translation_until_tick_with<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        tick_limit: Tick,
        drive_ready_cores: F,
    ) -> Result<Option<RiscvClusterTurn>, RiscvClusterError>
    where
        F: FnOnce(
            &mut PartitionedScheduler,
        ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError>,
    {
        if scheduler.now() >= tick_limit {
            return Ok(None);
        }

        let core_events = drive_ready_cores(scheduler)?;
        if !core_events.is_empty() {
            return Ok(Some(RiscvClusterTurn::core(core_events)));
        }

        if scheduler.is_idle() {
            return Ok(Some(RiscvClusterTurn::idle(scheduler.now())));
        }

        let Some(turn) = drive_parallel_scheduler_turn_until_tick(scheduler, tick_limit)? else {
            return Ok(None);
        };
        self.reconcile_reservation_invalidations();
        Ok(Some(turn))
    }
}
