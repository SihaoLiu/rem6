use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler, SchedulerContext};
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::riscv_cluster::{RiscvCluster, RiscvClusterError};
use crate::riscv_cluster_run::{RiscvClusterRun, RiscvClusterStopReason, RiscvClusterTurn};
use crate::CpuId;

fn run_result(
    cluster: &RiscvCluster,
    turns: Vec<RiscvClusterTurn>,
    stop_reason: RiscvClusterStopReason,
) -> RiscvClusterRun {
    RiscvClusterRun::with_store_conditional_failure_diagnostics(
        turns,
        stop_reason,
        cluster.store_conditional_failure_diagnostics(),
    )
}

impl RiscvCluster {
    #[allow(clippy::too_many_arguments)]
    pub fn drive_until<F, D, FR, DR, S>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut stop: S,
    ) -> Result<RiscvClusterRun, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        DR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        S: FnMut(&RiscvClusterTurn) -> bool,
    {
        let mut turns = Vec::new();
        for _ in 0..max_turns {
            let turn = self.drive_turn(
                scheduler,
                transport,
                fetch_trace.clone(),
                data_trace.clone(),
                &mut fetch_responder,
                &mut data_responder,
            )?;
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(run_result(
                    self,
                    turns,
                    RiscvClusterStopReason::Idle { tick },
                ));
            }
            if stop(&turn) {
                turns.push(turn);
                return Ok(run_result(
                    self,
                    turns,
                    RiscvClusterStopReason::StopCondition,
                ));
            }
            turns.push(turn);
        }

        Err(RiscvClusterError::TurnLimitExceeded {
            limit: max_turns,
            completed: turns.len(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_until_parallel<F, D, FR, DR, S>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        max_turns: usize,
        mut stop: S,
    ) -> Result<RiscvClusterRun, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        DR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        S: FnMut(&RiscvClusterTurn) -> bool,
    {
        let mut turns = Vec::new();
        for _ in 0..max_turns {
            let turn = self.drive_turn_parallel(
                scheduler,
                transport,
                fetch_trace.clone(),
                data_trace.clone(),
                &mut fetch_responder,
                &mut data_responder,
            )?;
            if let Some(tick) = turn.idle_tick() {
                turns.push(turn);
                return Ok(run_result(
                    self,
                    turns,
                    RiscvClusterStopReason::Idle { tick },
                ));
            }
            if stop(&turn) {
                turns.push(turn);
                return Ok(run_result(
                    self,
                    turns,
                    RiscvClusterStopReason::StopCondition,
                ));
            }
            turns.push(turn);
        }

        Err(RiscvClusterError::TurnLimitExceeded {
            limit: max_turns,
            completed: turns.len(),
        })
    }
}
