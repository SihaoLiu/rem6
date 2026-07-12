use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler, Tick};
use rem6_memory::TranslationPageMap;
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::riscv_cluster_scheduler::drive_parallel_scheduler_turn_until_tick;
use crate::{CpuId, RiscvCluster, RiscvClusterError, RiscvClusterTurn, RiscvCore, RiscvCpuError};

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

impl RiscvCluster {
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
        if scheduler.now() >= tick_limit {
            return Ok(None);
        }

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
