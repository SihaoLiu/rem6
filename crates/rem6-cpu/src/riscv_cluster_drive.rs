use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, PartitionedScheduler, Tick};
use rem6_memory::{MemoryRequest, MemoryRequestId};
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
};

use crate::riscv_cluster::RiscvClusterError;
use crate::riscv_cluster_run::RiscvClusterDriveEvent;
use crate::riscv_data_issue::{
    OutstandingDataAccess, PreparedDataIssueCleanup, PreparedDataParallelAccess,
};
use crate::riscv_fetch_ahead::PreparedRiscvFetchAheadSpeculation;
use crate::riscv_in_order_drive::RiscvInOrderDriveStatus;
use crate::{CpuId, OutstandingFetch, RiscvCore, RiscvCoreDriveAction, RiscvCpuError};

#[allow(clippy::large_enum_variant)]
pub(crate) enum PreparedParallelAction {
    Ready(RiscvClusterDriveEvent),
    PipelineCycle {
        cpu: CpuId,
        core: RiscvCore,
        event: PartitionEventId,
    },
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
        cleanup: PreparedDataIssueCleanup,
    },
    LocalDataFailure {
        cpu: CpuId,
        core: RiscvCore,
        issue: OutstandingDataAccess,
        cleanup: PreparedDataIssueCleanup,
    },
    LocalDataForwarding {
        cpu: CpuId,
        core: RiscvCore,
        issue: OutstandingDataAccess,
        cleanup: PreparedDataIssueCleanup,
    },
    LocalBufferedStore {
        cpu: CpuId,
        core: RiscvCore,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
        cleanup: PreparedDataIssueCleanup,
    },
    BufferedStoreData {
        cpu: CpuId,
        core: RiscvCore,
        request_id: MemoryRequestId,
        transaction_index: usize,
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

    fn cancel_pipeline_cycles(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<(), RiscvClusterError> {
        for action in &self.actions {
            if let PreparedParallelAction::PipelineCycle { cpu, core, event } = action {
                core.cancel_scheduled_in_order_pipeline_cycle(scheduler, *event)
                    .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?;
            }
        }
        Ok(())
    }
}

pub(crate) fn finish_prepared_parallel_actions(
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    mut prepared_actions: PreparedParallelActions,
    transaction_cpus: Vec<CpuId>,
    transactions: Vec<ParallelMemoryTransaction>,
) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError> {
    debug_assert_eq!(transaction_cpus.len(), transactions.len());
    let events = if transactions.is_empty() {
        Vec::new()
    } else {
        match transport.submit_parallel_batch(scheduler, transactions) {
            Ok(events) => events,
            Err(error) => {
                prepared_actions.cancel_pipeline_cycles(scheduler)?;
                return Err(RiscvClusterError::Core {
                    cpu: transaction_cpus
                        .first()
                        .copied()
                        .expect("batch submission has at least one CPU"),
                    error: RiscvCpuError::Transport(error),
                });
            }
        }
    };

    let mut actions = Vec::with_capacity(prepared_actions.len());
    let mut pipeline_cycles = Vec::new();
    let mut first_error = None;
    for prepared in prepared_actions.drain() {
        match prepared {
            PreparedParallelAction::Ready(event) => actions.push(event),
            PreparedParallelAction::PipelineCycle { cpu, core, event } => {
                pipeline_cycles.push((cpu, core, event));
                actions.push(RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::PipelineCycleScheduled { event },
                ));
            }
            PreparedParallelAction::Fetch {
                cpu,
                core,
                issue,
                fetch_ahead,
                transaction_index,
            } => match core
                .record_prepared_fetch_issue_with_prepared_fetch_ahead(issue, fetch_ahead)
            {
                Ok(()) => actions.push(RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::FetchIssued {
                        event: events[transaction_index],
                    },
                )),
                Err(error) => {
                    first_error.get_or_insert(RiscvClusterError::Core { cpu, error });
                }
            },
            PreparedParallelAction::Data {
                cpu,
                core,
                issue,
                transaction_index,
                cleanup,
            } => {
                core.record_prepared_data_issue(issue);
                cleanup.disarm();
                actions.push(RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::DataAccessIssued {
                        event: events[transaction_index],
                    },
                ));
            }
            PreparedParallelAction::LocalDataFailure {
                cpu,
                core,
                issue,
                cleanup,
            } => {
                match core.schedule_prepared_store_conditional_failure_parallel(scheduler, issue) {
                    Ok(event) => {
                        cleanup.disarm();
                        actions.push(RiscvClusterDriveEvent::new(
                            cpu,
                            RiscvCoreDriveAction::DataAccessIssued { event },
                        ));
                    }
                    Err(error) => {
                        first_error.get_or_insert(RiscvClusterError::Core { cpu, error });
                    }
                }
            }
            PreparedParallelAction::LocalDataForwarding {
                cpu,
                core,
                issue,
                cleanup,
            } => match core.schedule_forwarded_load_completion_parallel(scheduler, issue) {
                Ok(event) => {
                    cleanup.disarm();
                    actions.push(RiscvClusterDriveEvent::new(
                        cpu,
                        RiscvCoreDriveAction::DataAccessIssued { event },
                    ));
                }
                Err(error) => {
                    first_error.get_or_insert(RiscvClusterError::Core { cpu, error });
                }
            },
            PreparedParallelAction::LocalBufferedStore {
                cpu,
                core,
                issue,
                request,
                predecessor,
                cleanup,
            } => match core.schedule_prepared_buffered_o3_store_parallel(
                scheduler,
                issue,
                request,
                predecessor,
            ) {
                Ok(event) => {
                    cleanup.disarm();
                    actions.push(RiscvClusterDriveEvent::new(
                        cpu,
                        RiscvCoreDriveAction::DataAccessIssued { event },
                    ));
                }
                Err(error) => {
                    first_error.get_or_insert(RiscvClusterError::Core { cpu, error });
                }
            },
            PreparedParallelAction::BufferedStoreData {
                cpu,
                core,
                request_id,
                transaction_index,
            } => {
                core.record_buffered_o3_store_submission(request_id);
                actions.push(RiscvClusterDriveEvent::new(
                    cpu,
                    RiscvCoreDriveAction::DataAccessIssued {
                        event: events[transaction_index],
                    },
                ));
            }
        }
    }

    if let Some(error) = first_error {
        for (cpu, core, event) in pipeline_cycles {
            core.cancel_scheduled_in_order_pipeline_cycle(scheduler, event)
                .map_err(|error| RiscvClusterError::Core { cpu, error })?;
        }
        Err(error)
    } else {
        Ok(actions)
    }
}

pub(crate) fn push_prepared_data_action(
    cpu: CpuId,
    core: &RiscvCore,
    prepared: Option<PreparedDataParallelAccess>,
    prepared_actions: &mut PreparedParallelActions,
    transaction_cpus: &mut Vec<CpuId>,
    transactions: &mut Vec<ParallelMemoryTransaction>,
) -> bool {
    let Some(prepared) = prepared else {
        return false;
    };
    match prepared {
        PreparedDataParallelAccess::Transaction {
            issue,
            transaction,
            cleanup,
        } => {
            let transaction_index = transactions.len();
            transaction_cpus.push(cpu);
            transactions.push(transaction);
            prepared_actions.push(PreparedParallelAction::Data {
                cpu,
                core: core.clone(),
                issue,
                transaction_index,
                cleanup,
            });
        }
        PreparedDataParallelAccess::ConditionalFailed { issue, cleanup } => {
            prepared_actions.push(PreparedParallelAction::LocalDataFailure {
                cpu,
                core: core.clone(),
                issue,
                cleanup,
            });
        }
        PreparedDataParallelAccess::Forwarded { issue, cleanup } => {
            prepared_actions.push(PreparedParallelAction::LocalDataForwarding {
                cpu,
                core: core.clone(),
                issue,
                cleanup,
            });
        }
        PreparedDataParallelAccess::BufferedStore {
            issue,
            request,
            predecessor,
            cleanup,
        } => {
            prepared_actions.push(PreparedParallelAction::LocalBufferedStore {
                cpu,
                core: core.clone(),
                issue,
                request,
                predecessor,
                cleanup,
            });
        }
        PreparedDataParallelAccess::BufferedTransaction {
            request_id,
            transaction,
        } => {
            let transaction_index = transactions.len();
            transaction_cpus.push(cpu);
            transactions.push(transaction);
            prepared_actions.push(PreparedParallelAction::BufferedStoreData {
                cpu,
                core: core.clone(),
                request_id,
                transaction_index,
            });
        }
    }
    true
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
        .drive_next_completed_fetch_parallel_action(scheduler)
        .map_err(|error| RiscvClusterError::Core { cpu, error })?
        .map(|action| RiscvClusterDriveEvent::new(cpu, action)))
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

pub(crate) fn fetch_before_pipeline_is_admitted(core: &RiscvCore) -> bool {
    if inherited_o3_retirement_suppresses_pipeline(core) {
        return false;
    }
    core.detailed_o3_window_prefers_fetch_ahead()
        || (!core.has_pending_o3_scalar_memory_retirement()
            && core.in_order_fetch_admission().allows_fetch())
}

fn inherited_o3_retirement_suppresses_pipeline(core: &RiscvCore) -> bool {
    !core.detailed_o3_window_prefers_fetch_ahead()
        && core.o3_retirement_suppresses_normal_pipeline()
}

pub(crate) fn push_prepared_pipeline_cycle_drive_event(
    cpu: CpuId,
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    prepared_actions: &mut PreparedParallelActions,
) -> Result<bool, RiscvClusterError> {
    if inherited_o3_retirement_suppresses_pipeline(core) {
        return Ok(false);
    }
    if core.detailed_o3_window_prefers_fetch_ahead() {
        return Ok(false);
    }
    if core.has_pending_o3_scalar_memory_retirement() {
        return Ok(true);
    }
    match core
        .schedule_next_completed_fetch_pipeline_cycle_parallel(scheduler)
        .map_err(|error| RiscvClusterError::Core { cpu, error })?
    {
        RiscvInOrderDriveStatus::Scheduled(event) => {
            prepared_actions.push(PreparedParallelAction::PipelineCycle {
                cpu,
                core: core.clone(),
                event,
            });
            Ok(true)
        }
        RiscvInOrderDriveStatus::Pending => Ok(true),
        RiscvInOrderDriveStatus::Ready if core.live_retire_gate_blocks_new_work() => {
            if let Some(event) = completed_fetch_drive_event(cpu, core, scheduler)? {
                prepared_actions.push(PreparedParallelAction::Ready(event));
            }
            Ok(true)
        }
        RiscvInOrderDriveStatus::Unavailable if core.live_retire_gate_blocks_new_work() => Ok(true),
        RiscvInOrderDriveStatus::Unavailable | RiscvInOrderDriveStatus::Ready => Ok(false),
        RiscvInOrderDriveStatus::Reserved { .. } => {
            unreachable!("pipeline reservation is scheduled before returning")
        }
    }
}

pub(crate) fn push_pipeline_cycle_drive_event(
    cpu: CpuId,
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    actions: &mut Vec<RiscvClusterDriveEvent>,
) -> Result<bool, RiscvClusterError> {
    if inherited_o3_retirement_suppresses_pipeline(core) {
        return Ok(false);
    }
    if core.detailed_o3_window_prefers_fetch_ahead() {
        return Ok(false);
    }
    if core.has_pending_o3_scalar_memory_retirement() {
        return Ok(true);
    }
    match core
        .schedule_next_completed_fetch_pipeline_cycle_parallel(scheduler)
        .map_err(|error| RiscvClusterError::Core { cpu, error })?
    {
        RiscvInOrderDriveStatus::Scheduled(event) => {
            actions.push(RiscvClusterDriveEvent::new(
                cpu,
                RiscvCoreDriveAction::PipelineCycleScheduled { event },
            ));
            Ok(true)
        }
        RiscvInOrderDriveStatus::Pending => Ok(true),
        RiscvInOrderDriveStatus::Ready if core.live_retire_gate_blocks_new_work() => {
            if let Some(event) = completed_fetch_drive_event(cpu, core, scheduler)? {
                actions.push(event);
            }
            Ok(true)
        }
        RiscvInOrderDriveStatus::Unavailable if core.live_retire_gate_blocks_new_work() => Ok(true),
        RiscvInOrderDriveStatus::Unavailable | RiscvInOrderDriveStatus::Ready => Ok(false),
        RiscvInOrderDriveStatus::Reserved { .. } => {
            unreachable!("pipeline reservation is scheduled before returning")
        }
    }
}

#[cfg(test)]
#[path = "riscv_cluster_drive_tests.rs"]
mod tests;
