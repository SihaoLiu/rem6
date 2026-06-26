use rem6_kernel::{ParallelSchedulerContext, Tick};
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn push_prepared_parallel_fetch_action<F>(
    cpu: CpuId,
    core: &RiscvCore,
    tick: Tick,
    transport: &MemoryTransport,
    fetch_trace: MemoryTrace,
    fetch_responder: F,
    prepared_actions: &mut Vec<PreparedParallelAction>,
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
) -> Result<Option<RiscvClusterDriveEvent>, RiscvClusterError> {
    Ok(core
        .execute_next_completed_fetch()
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
    actions: &mut Vec<RiscvClusterDriveEvent>,
) -> Result<bool, RiscvClusterError> {
    if let Some(event) = completed_fetch_drive_event(cpu, core)? {
        actions.push(event);
        return Ok(true);
    }
    Ok(false)
}

pub(crate) fn push_prepared_completed_fetch_drive_event(
    cpu: CpuId,
    core: &RiscvCore,
    prepared_actions: &mut Vec<PreparedParallelAction>,
) -> Result<bool, RiscvClusterError> {
    if let Some(event) = completed_fetch_drive_event(cpu, core)? {
        prepared_actions.push(PreparedParallelAction::Ready(event));
        return Ok(true);
    }
    Ok(false)
}
