use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler, Tick};
use rem6_memory::TranslationPageMap;
use rem6_mmio::MmioBus;
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
};

use crate::riscv_cluster_drive::{push_prepared_parallel_fetch_action, PreparedParallelActions};
use crate::riscv_cluster_scheduler::{
    drive_parallel_scheduler_turn, drive_parallel_scheduler_turn_until_tick,
};
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

pub(crate) fn can_retire_mmio_fetch_pending(
    cpu: CpuId,
    core: &RiscvCore,
    bus: &MmioBus,
) -> Result<bool, RiscvClusterError> {
    core.can_retire_completed_fetch_while_mmio_aware_fetch_pending(bus)
        .map_err(|error| RiscvClusterError::Core { cpu, error })
}

pub(crate) fn advance_parallel_data_translation(
    cpu: CpuId,
    core: &RiscvCore,
    scheduler: &PartitionedScheduler,
    page_map: &TranslationPageMap,
) -> Result<bool, RiscvClusterError> {
    if core.ready_translated_memory_fetch_ahead_is_pending() {
        return Ok(true);
    }
    core.advance_next_data_translation(scheduler.now(), page_map)
        .map_err(|error| RiscvClusterError::Core { cpu, error })?;
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_ready_translated_memory_fetch_ahead<F, FR>(
    cpu: CpuId,
    core: &RiscvCore,
    scheduler: &PartitionedScheduler,
    transport: &MemoryTransport,
    fetch_trace: MemoryTrace,
    fetch_responder: &mut F,
    prepared_actions: &mut PreparedParallelActions,
    transaction_cpus: &mut Vec<CpuId>,
    transactions: &mut Vec<ParallelMemoryTransaction>,
) -> Result<bool, RiscvClusterError>
where
    F: FnMut(CpuId) -> FR,
    FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
        + Send
        + 'static,
{
    let Some(fetch_request) = core
        .ready_translated_scalar_load_window_fetch_request(scheduler.now(), transport)
        .map_err(|error| RiscvClusterError::Core { cpu, error })?
    else {
        return Ok(false);
    };
    let Some(decision) = core.next_ready_translated_memory_fetch_ahead_before_issue(fetch_request)
    else {
        return Ok(false);
    };
    let fetch_ahead = core
        .prepare_fetch_ahead_speculation(&decision)
        .map_err(|error| RiscvClusterError::Core { cpu, error })?;
    core.set_fetch_ahead_pc(decision.pc());
    push_prepared_parallel_fetch_action(
        cpu,
        core,
        scheduler.now(),
        transport,
        fetch_trace,
        fetch_responder(cpu),
        prepared_actions,
        transaction_cpus,
        transactions,
        fetch_ahead,
    )?;
    Ok(true)
}

impl RiscvCluster {
    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn_parallel_with_data_translation<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<RiscvClusterTurn, RiscvClusterError>
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
        let core_events = self.drive_ready_cores_parallel_with_data_translation(
            scheduler,
            transport,
            fetch_trace,
            data_trace,
            page_map,
            fetch_responder,
            data_responder,
        )?;
        if !core_events.is_empty() {
            return Ok(RiscvClusterTurn::core(core_events));
        }
        if scheduler.is_idle() {
            return Ok(RiscvClusterTurn::idle(scheduler.now()));
        }
        let turn = drive_parallel_scheduler_turn(scheduler)?;
        self.check_pending_callback_errors()?;
        self.reconcile_reservation_invalidations();
        Ok(turn)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn_parallel_with_mmio_and_data_translation<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<RiscvClusterTurn, RiscvClusterError>
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
        let core_events = self.drive_ready_cores_parallel_with_mmio_and_data_translation(
            scheduler,
            transport,
            bus,
            fetch_trace,
            data_trace,
            page_map,
            fetch_responder,
            data_responder,
        )?;
        if !core_events.is_empty() {
            return Ok(RiscvClusterTurn::core(core_events));
        }
        if scheduler.is_idle() {
            return Ok(RiscvClusterTurn::idle(scheduler.now()));
        }
        let turn = drive_parallel_scheduler_turn(scheduler)?;
        self.check_pending_callback_errors()?;
        self.reconcile_reservation_invalidations();
        Ok(turn)
    }

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
        self.check_pending_callback_errors()?;
        self.reconcile_reservation_invalidations();
        Ok(Some(turn))
    }
}
