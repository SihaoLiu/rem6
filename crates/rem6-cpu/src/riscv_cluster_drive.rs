use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler, Tick};
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
};

use crate::riscv_cluster::RiscvClusterError;
use crate::riscv_cluster_run::RiscvClusterDriveEvent;
use crate::riscv_data_issue::OutstandingDataAccess;
use crate::riscv_fetch_ahead::PreparedRiscvFetchAheadSpeculation;
use crate::{CpuId, OutstandingFetch, RiscvCore, RiscvCoreDriveAction};

pub(crate) enum PreparedParallelAction {
    Ready(RiscvClusterDriveEvent),
    Fetch {
        cpu: CpuId,
        core: RiscvCore,
        issue: OutstandingFetch,
        fetch_ahead: Option<PreparedRiscvFetchAheadSpeculation>,
        transaction_index: usize,
    },
    Data {
        cpu: CpuId,
        core: RiscvCore,
        issue: OutstandingDataAccess,
        transaction_index: usize,
    },
    LocalDataFailure {
        cpu: CpuId,
        core: RiscvCore,
        issue: OutstandingDataAccess,
    },
}

pub(crate) struct PreparedParallelActions {
    actions: Vec<PreparedParallelAction>,
}

impl PreparedParallelActions {
    pub(crate) const fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, action: PreparedParallelAction) {
        self.actions.push(action);
    }

    pub(crate) fn len(&self) -> usize {
        self.actions.len()
    }

    pub(crate) fn drain(&mut self) -> std::vec::Drain<'_, PreparedParallelAction> {
        self.actions.drain(..)
    }
}

impl Drop for PreparedParallelActions {
    fn drop(&mut self) {
        for action in &self.actions {
            match action {
                PreparedParallelAction::Data { core, .. }
                | PreparedParallelAction::LocalDataFailure { core, .. } => {
                    core.clear_deferred_o3_scalar_memory_execution();
                }
                PreparedParallelAction::Ready(_) | PreparedParallelAction::Fetch { .. } => {}
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_prepared_parallel_fetch_action<F>(
    cpu: CpuId,
    core: &RiscvCore,
    tick: Tick,
    transport: &MemoryTransport,
    fetch_trace: MemoryTrace,
    fetch_responder: F,
    prepared_actions: &mut PreparedParallelActions,
    transaction_cpus: &mut Vec<CpuId>,
    transactions: &mut Vec<ParallelMemoryTransaction>,
    fetch_ahead: Option<PreparedRiscvFetchAheadSpeculation>,
) -> Result<(), RiscvClusterError>
where
    F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send + 'static,
{
    let (issue, transaction) = core
        .prepare_fetch_parallel_transaction(tick, transport, fetch_trace, fetch_responder)
        .map_err(|error| RiscvClusterError::Core { cpu, error })?;
    let transaction_index = transactions.len();
    transaction_cpus.push(cpu);
    transactions.push(transaction);
    prepared_actions.push(PreparedParallelAction::Fetch {
        cpu,
        core: core.clone(),
        issue,
        fetch_ahead,
        transaction_index,
    });
    Ok(())
}

pub(crate) fn completed_fetch_drive_event(
    cpu: CpuId,
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
) -> Result<Option<RiscvClusterDriveEvent>, RiscvClusterError> {
    Ok(core
        .execute_next_completed_fetch_parallel(scheduler)
        .map_err(|error| RiscvClusterError::Core { cpu, error })?
        .map(|event| {
            RiscvClusterDriveEvent::new(
                cpu,
                RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
            )
        }))
}

pub(crate) fn push_completed_fetch_drive_event(
    cpu: CpuId,
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    actions: &mut Vec<RiscvClusterDriveEvent>,
) -> Result<bool, RiscvClusterError> {
    if let Some(event) = completed_fetch_drive_event(cpu, core, scheduler)? {
        actions.push(event);
        return Ok(true);
    }
    if core.live_retire_gate_blocks_new_work() {
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn push_prepared_completed_fetch_drive_event(
    cpu: CpuId,
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    prepared_actions: &mut PreparedParallelActions,
) -> Result<bool, RiscvClusterError> {
    if let Some(event) = completed_fetch_drive_event(cpu, core, scheduler)? {
        prepared_actions.push(PreparedParallelAction::Ready(event));
        return Ok(true);
    }
    if core.live_retire_gate_blocks_new_work() {
        return Ok(true);
    }
    Ok(false)
}
