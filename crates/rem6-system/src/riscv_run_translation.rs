use rem6_cpu::{CpuId, RiscvCluster, RiscvClusterError};
use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler};
use rem6_memory::TranslationPageMap;
use rem6_mmio::MmioBus;
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::{
    pending_trap_cores_from_turn, GuestEventId, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvSystemRunStopReason, SystemError,
};

impl RiscvSystemRunDriver {
    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_parallel_with_data_translation<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        let mut turns = Vec::new();
        let mut scheduled_traps = Vec::new();

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        for _ in 0..max_turns {
            let turn = cluster
                .drive_turn_parallel_with_data_translation(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    page_map,
                    &mut fetch_responder,
                    &mut data_responder,
                )
                .map_err(SystemError::RiscvCluster)?;
            self.record_instruction_stats(&turn)?;
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.trap_port().schedule_pending_core_traps_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::Idle { tick },
                ));
            }

            turns.push(turn);
        }

        Err(SystemError::RiscvCluster(
            RiscvClusterError::TurnLimitExceeded {
                limit: max_turns,
                completed: turns.len(),
            },
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_parallel_with_mmio_and_data_translation<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut event_for: E,
    ) -> Result<RiscvSystemRun, SystemError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        E: FnMut(CpuId) -> GuestEventId,
    {
        let mut turns = Vec::new();
        let mut scheduled_traps = Vec::new();

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        for _ in 0..max_turns {
            let turn = cluster
                .drive_turn_parallel_with_mmio_and_data_translation(
                    scheduler,
                    transport,
                    bus,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    page_map,
                    &mut fetch_responder,
                    &mut data_responder,
                )
                .map_err(SystemError::RiscvCluster)?;
            self.record_instruction_stats(&turn)?;
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.trap_port().schedule_pending_core_traps_parallel(
                    scheduler,
                    trap_cores,
                    &mut event_for,
                )?);
            }

            if let Some(stop) = self.host_stop_request() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::HostStop(stop),
                ));
            }
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::Idle { tick },
                ));
            }

            turns.push(turn);
        }

        Err(SystemError::RiscvCluster(
            RiscvClusterError::TurnLimitExceeded {
                limit: max_turns,
                completed: turns.len(),
            },
        ))
    }
}
