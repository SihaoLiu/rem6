use rem6_cpu::{CpuId, RiscvCluster, RiscvClusterTurn, RiscvCoreDriveAction};
use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler};
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::{
    pending_trap_cores_from_turn, GuestEventId, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvSystemRunStopReason, SystemError,
};

impl RiscvSystemRunDriver {
    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_or_tick_limit_parallel<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        tick_limit: u64,
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
        self.reset_stats_for_run(cluster)?;

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        loop {
            if scheduler.now() >= tick_limit {
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::TickLimit {
                        tick: tick_limit,
                        limit: tick_limit,
                    },
                ));
            }

            let Some(turn) = cluster
                .drive_turn_parallel_until_tick(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    &mut fetch_responder,
                    &mut data_responder,
                    tick_limit,
                )
                .map_err(SystemError::RiscvCluster)?
            else {
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::TickLimit {
                        tick: tick_limit,
                        limit: tick_limit,
                    },
                ));
            };
            self.record_run_stats(cluster, scheduler.now(), &turn)?;
            self.trap_port()
                .schedule_riscv_system_events_from_turn_parallel(
                    scheduler,
                    &turn,
                    &mut event_for,
                )?;
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.schedule_pending_core_events_parallel(
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
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_host_stop_or_instruction_limit_parallel<F, D, FR, DR, E>(
        &self,
        cluster: &RiscvCluster,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        tick_limit: u64,
        max_instructions: u64,
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
        let mut committed_instructions = 0u64;
        self.reset_stats_for_run(cluster)?;

        if let Some(stop) = self.host_stop_request() {
            return Ok(self.run_result(
                cluster,
                turns,
                scheduled_traps,
                RiscvSystemRunStopReason::HostStop(stop),
            ));
        }

        loop {
            if scheduler.now() >= tick_limit {
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::TickLimit {
                        tick: tick_limit,
                        limit: tick_limit,
                    },
                ));
            }

            let remaining_instructions = max_instructions.saturating_sub(committed_instructions);
            let Some(turn) = cluster
                .drive_turn_parallel_with_instruction_budget_until_tick(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    &mut fetch_responder,
                    &mut data_responder,
                    remaining_instructions,
                    tick_limit,
                )
                .map_err(SystemError::RiscvCluster)?
            else {
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::TickLimit {
                        tick: tick_limit,
                        limit: tick_limit,
                    },
                ));
            };
            self.record_run_stats(cluster, scheduler.now(), &turn)?;
            committed_instructions =
                committed_instructions.saturating_add(committed_instruction_count(&turn));
            self.trap_port()
                .schedule_riscv_system_events_from_turn_parallel(
                    scheduler,
                    &turn,
                    &mut event_for,
                )?;
            let trap_cores = pending_trap_cores_from_turn(cluster, &turn)?;
            if !trap_cores.is_empty() {
                scheduled_traps.extend(self.schedule_pending_core_events_parallel(
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
            if committed_instructions >= max_instructions {
                let tick = last_committed_instruction_tick(&turn).unwrap_or(0);
                turns.push(turn);
                return Ok(self.run_result(
                    cluster,
                    turns,
                    scheduled_traps,
                    RiscvSystemRunStopReason::InstructionLimit {
                        tick,
                        limit: max_instructions,
                        committed: committed_instructions,
                    },
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
    }
}

fn committed_instruction_count(turn: &RiscvClusterTurn) -> u64 {
    turn.core_events()
        .iter()
        .filter(|event| match event.action() {
            RiscvCoreDriveAction::InstructionExecuted(execution) => {
                execution.counts_as_retired_instruction()
            }
            RiscvCoreDriveAction::FetchIssued { .. }
            | RiscvCoreDriveAction::DataAccessIssued { .. } => false,
        })
        .count() as u64
}

fn last_committed_instruction_tick(turn: &RiscvClusterTurn) -> Option<u64> {
    turn.core_events()
        .iter()
        .filter_map(|event| match event.action() {
            RiscvCoreDriveAction::InstructionExecuted(execution)
                if execution.counts_as_retired_instruction() =>
            {
                Some(execution.fetch().tick())
            }
            RiscvCoreDriveAction::InstructionExecuted(_) => None,
            RiscvCoreDriveAction::FetchIssued { .. }
            | RiscvCoreDriveAction::DataAccessIssued { .. } => None,
        })
        .max()
}
