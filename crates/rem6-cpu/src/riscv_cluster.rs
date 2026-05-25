use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{
    ParallelEpochBatchRecord, ParallelEpochPlan, ParallelPartitionActivity, ParallelRunProfile,
    ParallelSchedulerContext, ParallelWorkerRecord, PartitionFrontier, PartitionId,
    PartitionedScheduler, ReadyPartition, RecordedRunSummary, RunSummary, ScheduledEventKind,
    SchedulerContext, SchedulerDispatchRecord, SchedulerError, Tick,
};
use rem6_memory::{AccessSize, Address, AgentId, TranslationPageMap};
use rem6_mmio::MmioBus;
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
    TransportEndpointId,
};

use crate::riscv_activity::{drive_action_partition, RiscvCoreDriveActivity};
use crate::riscv_reservation::RiscvReservationTracker;
use crate::{
    CpuId, OutstandingDataAccess, OutstandingFetch, PreparedDataParallelAccess, RiscvCore,
    RiscvCoreDriveAction, RiscvCpuError,
};

enum PreparedParallelAction {
    Ready(RiscvClusterDriveEvent),
    Fetch {
        cpu: CpuId,
        core: RiscvCore,
        issue: OutstandingFetch,
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

#[derive(Clone, Debug)]
pub struct RiscvCluster {
    cores: BTreeMap<CpuId, RiscvCore>,
    reservations: Arc<Mutex<RiscvReservationTracker>>,
}

impl RiscvCluster {
    pub fn new<I>(cores: I) -> Result<Self, RiscvClusterError>
    where
        I: IntoIterator<Item = RiscvCore>,
    {
        let mut by_cpu = BTreeMap::new();
        let mut by_agent = BTreeMap::new();
        let mut by_fetch_endpoint = BTreeMap::new();
        let mut by_data_endpoint = BTreeMap::new();

        for core in cores {
            let cpu = core.id();
            if by_cpu.contains_key(&cpu) {
                return Err(RiscvClusterError::DuplicateCpu { cpu });
            }

            let agent = core.agent();
            if let Some(existing) = by_agent.insert(agent, cpu) {
                return Err(RiscvClusterError::DuplicateAgent {
                    agent,
                    existing,
                    duplicate: cpu,
                });
            }

            let fetch_endpoint = core.fetch_endpoint();
            if let Some(existing) = by_fetch_endpoint.insert(fetch_endpoint.clone(), cpu) {
                return Err(RiscvClusterError::DuplicateFetchEndpoint {
                    endpoint: fetch_endpoint,
                    existing,
                    duplicate: cpu,
                });
            }

            if let Some(data_endpoint) = core.data_endpoint() {
                if let Some(existing) = by_data_endpoint.insert(data_endpoint.clone(), cpu) {
                    return Err(RiscvClusterError::DuplicateDataEndpoint {
                        endpoint: data_endpoint,
                        existing,
                        duplicate: cpu,
                    });
                }
            }

            by_cpu.insert(cpu, core);
        }

        Ok(Self {
            cores: by_cpu,
            reservations: Arc::new(Mutex::new(RiscvReservationTracker::default())),
        })
    }

    fn reconcile_reservation_invalidations(&self) {
        self.reservations
            .lock()
            .expect("riscv cluster reservation tracker lock")
            .reconcile(self.cores.iter());
    }

    pub fn core_count(&self) -> usize {
        self.cores.len()
    }

    pub fn core_ids(&self) -> Vec<CpuId> {
        self.cores.keys().copied().collect()
    }

    pub fn core(&self, cpu: CpuId) -> Result<RiscvCore, RiscvClusterError> {
        self.cores
            .get(&cpu)
            .cloned()
            .ok_or(RiscvClusterError::UnknownCpu { cpu })
    }

    pub fn invalidate_load_reservation_for_agent_if_overlaps(
        &self,
        agent: AgentId,
        address: Address,
        size: AccessSize,
    ) -> bool {
        self.cores
            .values()
            .find(|core| core.agent() == agent)
            .and_then(|core| core.invalidate_load_reservation_if_overlaps(address, size))
            .is_some()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_core_next_action<F, D>(
        &self,
        cpu: CpuId,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvClusterError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        D: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        self.reconcile_reservation_invalidations();
        self.core(cpu)?
            .drive_next_action(
                scheduler,
                transport,
                fetch_trace,
                data_trace,
                fetch_responder,
                data_responder,
            )
            .map_err(|error| RiscvClusterError::Core { cpu, error })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_ready_cores<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
    ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        DR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        self.reconcile_reservation_invalidations();
        let mut actions = Vec::new();
        for (cpu, core) in &self.cores {
            if let Some(action) = core
                .drive_next_action(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    data_trace.clone(),
                    fetch_responder(*cpu),
                    data_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                actions.push(RiscvClusterDriveEvent::new(*cpu, action));
            }
        }

        Ok(actions)
    }

    pub fn drive_ready_cores_parallel_fetch<F, FR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        mut fetch_responder: F,
    ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        self.reconcile_reservation_invalidations();
        let mut prepared_actions = Vec::new();
        let mut transaction_cpus = Vec::new();
        let mut transactions = Vec::new();
        for (cpu, core) in &self.cores {
            if core.has_pending_fetch()
                || core.has_pending_data_access()
                || core.has_unissued_data_access()
                || core.has_pending_trap()
            {
                continue;
            }

            if let Some(event) = core
                .execute_next_completed_fetch()
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                prepared_actions.push(PreparedParallelAction::Ready(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                )));
                continue;
            }

            let (issue, transaction) = core
                .prepare_fetch_parallel_transaction(
                    scheduler.now(),
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core {
                    cpu: *cpu,
                    error: RiscvCpuError::Cpu(error),
                })?;
            let transaction_index = transactions.len();
            transaction_cpus.push(*cpu);
            transactions.push(transaction);
            prepared_actions.push(PreparedParallelAction::Fetch {
                cpu: *cpu,
                core: core.clone(),
                issue,
                transaction_index,
            });
        }

        self.finish_prepared_parallel_actions(
            scheduler,
            transport,
            prepared_actions,
            transaction_cpus,
            transactions,
        )
    }

    pub fn drive_ready_cores_parallel<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
    ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError>
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
        self.reconcile_reservation_invalidations();
        let mut prepared_actions = Vec::new();
        let mut transaction_cpus = Vec::new();
        let mut transactions = Vec::new();
        for (cpu, core) in &self.cores {
            if core.has_pending_fetch() || core.has_pending_data_access() || core.has_pending_trap()
            {
                continue;
            }

            if let Some(event) = core
                .execute_next_completed_fetch()
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                prepared_actions.push(PreparedParallelAction::Ready(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                )));
                continue;
            }

            if let Some(prepared) = core
                .prepare_data_parallel_access(
                    scheduler.now(),
                    transport,
                    data_trace.clone(),
                    data_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                match prepared {
                    PreparedDataParallelAccess::Transaction { issue, transaction } => {
                        let transaction_index = transactions.len();
                        transaction_cpus.push(*cpu);
                        transactions.push(transaction);
                        prepared_actions.push(PreparedParallelAction::Data {
                            cpu: *cpu,
                            core: core.clone(),
                            issue,
                            transaction_index,
                        });
                    }
                    PreparedDataParallelAccess::ConditionalFailed { issue } => {
                        prepared_actions.push(PreparedParallelAction::LocalDataFailure {
                            cpu: *cpu,
                            core: core.clone(),
                            issue,
                        });
                    }
                }
                continue;
            }

            let (issue, transaction) = core
                .prepare_fetch_parallel_transaction(
                    scheduler.now(),
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core {
                    cpu: *cpu,
                    error: RiscvCpuError::Cpu(error),
                })?;
            let transaction_index = transactions.len();
            transaction_cpus.push(*cpu);
            transactions.push(transaction);
            prepared_actions.push(PreparedParallelAction::Fetch {
                cpu: *cpu,
                core: core.clone(),
                issue,
                transaction_index,
            });
        }

        self.finish_prepared_parallel_actions(
            scheduler,
            transport,
            prepared_actions,
            transaction_cpus,
            transactions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_ready_cores_parallel_with_data_translation<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        mut fetch_responder: F,
        mut data_responder: D,
    ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError>
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
        self.reconcile_reservation_invalidations();
        let mut prepared_actions = Vec::new();
        let mut transaction_cpus = Vec::new();
        let mut transactions = Vec::new();
        for (cpu, core) in &self.cores {
            if core.has_pending_fetch()
                || core.has_outstanding_data_request()
                || core.has_pending_trap()
            {
                continue;
            }

            if let Some(event) = core
                .execute_next_completed_fetch()
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                prepared_actions.push(PreparedParallelAction::Ready(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                )));
                continue;
            }

            let has_data_work = core.has_unissued_data_access() || core.has_pending_data_access();
            if has_data_work {
                if let Some(prepared) = core
                    .prepare_translated_data_parallel_access(
                        scheduler.now(),
                        transport,
                        data_trace.clone(),
                        page_map,
                        data_responder(*cpu),
                    )
                    .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
                {
                    match prepared {
                        PreparedDataParallelAccess::Transaction { issue, transaction } => {
                            let transaction_index = transactions.len();
                            transaction_cpus.push(*cpu);
                            transactions.push(transaction);
                            prepared_actions.push(PreparedParallelAction::Data {
                                cpu: *cpu,
                                core: core.clone(),
                                issue,
                                transaction_index,
                            });
                        }
                        PreparedDataParallelAccess::ConditionalFailed { issue } => {
                            prepared_actions.push(PreparedParallelAction::LocalDataFailure {
                                cpu: *cpu,
                                core: core.clone(),
                                issue,
                            });
                        }
                    }
                }
                continue;
            }

            let (issue, transaction) = core
                .prepare_fetch_parallel_transaction(
                    scheduler.now(),
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core {
                    cpu: *cpu,
                    error: RiscvCpuError::Cpu(error),
                })?;
            let transaction_index = transactions.len();
            transaction_cpus.push(*cpu);
            transactions.push(transaction);
            prepared_actions.push(PreparedParallelAction::Fetch {
                cpu: *cpu,
                core: core.clone(),
                issue,
                transaction_index,
            });
        }

        self.finish_prepared_parallel_actions(
            scheduler,
            transport,
            prepared_actions,
            transaction_cpus,
            transactions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_ready_cores_parallel_with_mmio_and_data_translation<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        mut fetch_responder: F,
        mut data_responder: D,
    ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError>
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
        self.reconcile_reservation_invalidations();
        let mut prepared_actions = Vec::new();
        let mut transaction_cpus = Vec::new();
        let mut transactions = Vec::new();
        for (cpu, core) in &self.cores {
            if core.has_pending_fetch()
                || core.has_outstanding_data_request()
                || core.has_pending_trap()
            {
                continue;
            }

            if let Some(event) = core
                .execute_next_completed_fetch()
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                prepared_actions.push(PreparedParallelAction::Ready(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                )));
                continue;
            }

            let has_data_work = core.has_unissued_data_access() || core.has_pending_data_access();
            if has_data_work {
                if let Some(event) = core
                    .issue_next_translated_mmio_data_access_parallel(scheduler, bus, page_map)
                    .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
                {
                    prepared_actions.push(PreparedParallelAction::Ready(
                        RiscvClusterDriveEvent::new(
                            *cpu,
                            RiscvCoreDriveAction::DataAccessIssued { event },
                        ),
                    ));
                    continue;
                }

                if let Some(prepared) = core
                    .prepare_translated_data_parallel_access(
                        scheduler.now(),
                        transport,
                        data_trace.clone(),
                        page_map,
                        data_responder(*cpu),
                    )
                    .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
                {
                    match prepared {
                        PreparedDataParallelAccess::Transaction { issue, transaction } => {
                            let transaction_index = transactions.len();
                            transaction_cpus.push(*cpu);
                            transactions.push(transaction);
                            prepared_actions.push(PreparedParallelAction::Data {
                                cpu: *cpu,
                                core: core.clone(),
                                issue,
                                transaction_index,
                            });
                        }
                        PreparedDataParallelAccess::ConditionalFailed { issue } => {
                            prepared_actions.push(PreparedParallelAction::LocalDataFailure {
                                cpu: *cpu,
                                core: core.clone(),
                                issue,
                            });
                        }
                    }
                }
                continue;
            }

            let (issue, transaction) = core
                .prepare_fetch_parallel_transaction(
                    scheduler.now(),
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core {
                    cpu: *cpu,
                    error: RiscvCpuError::Cpu(error),
                })?;
            let transaction_index = transactions.len();
            transaction_cpus.push(*cpu);
            transactions.push(transaction);
            prepared_actions.push(PreparedParallelAction::Fetch {
                cpu: *cpu,
                core: core.clone(),
                issue,
                transaction_index,
            });
        }

        self.finish_prepared_parallel_actions(
            scheduler,
            transport,
            prepared_actions,
            transaction_cpus,
            transactions,
        )
    }

    fn finish_prepared_parallel_actions(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        prepared_actions: Vec<PreparedParallelAction>,
        transaction_cpus: Vec<CpuId>,
        transactions: Vec<ParallelMemoryTransaction>,
    ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError> {
        debug_assert_eq!(transaction_cpus.len(), transactions.len());
        let events = if transactions.is_empty() {
            Vec::new()
        } else {
            transport
                .submit_parallel_batch(scheduler, transactions)
                .map_err(|error| RiscvClusterError::Core {
                    cpu: transaction_cpus
                        .first()
                        .copied()
                        .expect("batch submission has at least one CPU"),
                    error: RiscvCpuError::Transport(error),
                })?
        };

        let mut actions = Vec::with_capacity(prepared_actions.len());
        for prepared in prepared_actions {
            match prepared {
                PreparedParallelAction::Ready(event) => actions.push(event),
                PreparedParallelAction::Fetch {
                    cpu,
                    core,
                    issue,
                    transaction_index,
                } => {
                    core.record_prepared_fetch_issue(issue);
                    actions.push(RiscvClusterDriveEvent::new(
                        cpu,
                        RiscvCoreDriveAction::FetchIssued {
                            event: events[transaction_index],
                        },
                    ));
                }
                PreparedParallelAction::Data {
                    cpu,
                    core,
                    issue,
                    transaction_index,
                } => {
                    core.record_prepared_data_issue(issue);
                    actions.push(RiscvClusterDriveEvent::new(
                        cpu,
                        RiscvCoreDriveAction::DataAccessIssued {
                            event: events[transaction_index],
                        },
                    ));
                }
                PreparedParallelAction::LocalDataFailure { cpu, core, issue } => {
                    let event = core
                        .schedule_prepared_store_conditional_failure_parallel(scheduler, issue)
                        .map_err(|error| RiscvClusterError::Core { cpu, error })?;
                    actions.push(RiscvClusterDriveEvent::new(
                        cpu,
                        RiscvCoreDriveAction::DataAccessIssued { event },
                    ));
                }
            }
        }

        Ok(actions)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_ready_cores_parallel_with_mmio<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
    ) -> Result<Vec<RiscvClusterDriveEvent>, RiscvClusterError>
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
        self.reconcile_reservation_invalidations();
        let mut actions = Vec::new();
        for (cpu, core) in &self.cores {
            if core.has_pending_fetch() || core.has_pending_data_access() || core.has_pending_trap()
            {
                continue;
            }

            if let Some(event) = core
                .execute_next_completed_fetch()
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                actions.push(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                ));
                continue;
            }

            if let Some(event) = core
                .issue_next_mmio_data_access_parallel(scheduler, bus)
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                actions.push(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::DataAccessIssued { event },
                ));
                continue;
            }

            if let Some(event) = core
                .issue_next_data_access_parallel(
                    scheduler,
                    transport,
                    data_trace.clone(),
                    data_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?
            {
                actions.push(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::DataAccessIssued { event },
                ));
                continue;
            }

            let event = core
                .issue_next_fetch_parallel(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core {
                    cpu: *cpu,
                    error: RiscvCpuError::Cpu(error),
                })?;
            actions.push(RiscvClusterDriveEvent::new(
                *cpu,
                RiscvCoreDriveAction::FetchIssued { event },
            ));
        }

        Ok(actions)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<RiscvClusterTurn, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        D: FnMut(CpuId) -> DR,
        FR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        DR: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let core_events = self.drive_ready_cores(
            scheduler,
            transport,
            fetch_trace,
            data_trace,
            fetch_responder,
            data_responder,
        )?;
        if !core_events.is_empty() {
            return Ok(RiscvClusterTurn::core(core_events));
        }

        if scheduler.is_idle() {
            return Ok(RiscvClusterTurn::idle(scheduler.now()));
        }

        let turn = RiscvClusterTurn::scheduler(scheduler.run_next_epoch());
        self.reconcile_reservation_invalidations();
        Ok(turn)
    }

    pub fn drive_turn_parallel_fetch<F, FR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        fetch_responder: F,
    ) -> Result<RiscvClusterTurn, RiscvClusterError>
    where
        F: FnMut(CpuId) -> FR,
        FR: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let core_events = self.drive_ready_cores_parallel_fetch(
            scheduler,
            transport,
            fetch_trace,
            fetch_responder,
        )?;
        if !core_events.is_empty() {
            return Ok(RiscvClusterTurn::core(core_events));
        }

        if scheduler.is_idle() {
            return Ok(RiscvClusterTurn::idle(scheduler.now()));
        }

        let turn = drive_parallel_scheduler_turn(scheduler)?;
        self.reconcile_reservation_invalidations();
        Ok(turn)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn_parallel<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
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
        let core_events = self.drive_ready_cores_parallel(
            scheduler,
            transport,
            fetch_trace,
            data_trace,
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
        self.reconcile_reservation_invalidations();
        Ok(turn)
    }

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
        self.reconcile_reservation_invalidations();
        Ok(turn)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn_parallel_with_mmio<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
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
        let core_events = self.drive_ready_cores_parallel_with_mmio(
            scheduler,
            transport,
            bus,
            fetch_trace,
            data_trace,
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
        self.reconcile_reservation_invalidations();
        Ok(turn)
    }

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
                return Ok(RiscvClusterRun::new(
                    turns,
                    RiscvClusterStopReason::Idle { tick },
                ));
            }
            if stop(&turn) {
                turns.push(turn);
                return Ok(RiscvClusterRun::new(
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
                return Ok(RiscvClusterRun::new(
                    turns,
                    RiscvClusterStopReason::Idle { tick },
                ));
            }
            if stop(&turn) {
                turns.push(turn);
                return Ok(RiscvClusterRun::new(
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterDriveEvent {
    cpu: CpuId,
    action: RiscvCoreDriveAction,
}

impl RiscvClusterDriveEvent {
    pub const fn new(cpu: CpuId, action: RiscvCoreDriveAction) -> Self {
        Self { cpu, action }
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn action(&self) -> &RiscvCoreDriveAction {
        &self.action
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterTurn {
    core_events: Vec<RiscvClusterDriveEvent>,
    scheduler: Option<RunSummary>,
    parallel_scheduler: Option<RiscvClusterSchedulerEpoch>,
    idle_tick: Option<Tick>,
}

impl RiscvClusterTurn {
    pub fn core(core_events: Vec<RiscvClusterDriveEvent>) -> Self {
        Self {
            core_events,
            scheduler: None,
            parallel_scheduler: None,
            idle_tick: None,
        }
    }

    pub const fn scheduler(summary: RunSummary) -> Self {
        Self {
            core_events: Vec::new(),
            scheduler: Some(summary),
            parallel_scheduler: None,
            idle_tick: None,
        }
    }

    pub fn parallel_scheduler(plan: ParallelEpochPlan, recorded: RecordedRunSummary) -> Self {
        Self {
            core_events: Vec::new(),
            scheduler: None,
            parallel_scheduler: Some(RiscvClusterSchedulerEpoch::new(plan, recorded)),
            idle_tick: None,
        }
    }

    pub const fn idle(tick: Tick) -> Self {
        Self {
            core_events: Vec::new(),
            scheduler: None,
            parallel_scheduler: None,
            idle_tick: Some(tick),
        }
    }

    pub fn core_events(&self) -> &[RiscvClusterDriveEvent] {
        &self.core_events
    }

    pub fn cpu_activity(&self, cpu: CpuId) -> Option<RiscvCoreDriveActivity> {
        self.cpu_activities().remove(&cpu)
    }

    pub fn has_cpu_activity(&self, cpu: CpuId) -> bool {
        self.cpu_activity(cpu)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_cpu_count(&self) -> usize {
        self.cpu_activities().len()
    }

    pub fn cpu_activities(&self) -> BTreeMap<CpuId, RiscvCoreDriveActivity> {
        let mut activities = BTreeMap::new();
        for event in &self.core_events {
            activities
                .entry(event.cpu())
                .or_insert_with(RiscvCoreDriveActivity::default)
                .record_action(event.action());
        }
        activities
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<RiscvCoreDriveActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, RiscvCoreDriveActivity> {
        let mut activities = BTreeMap::new();
        for event in &self.core_events {
            activities
                .entry(drive_action_partition(event.action()))
                .or_insert_with(RiscvCoreDriveActivity::default)
                .record_action(event.action());
        }
        activities
    }

    pub const fn scheduler_summary(&self) -> Option<RunSummary> {
        match (self.scheduler, self.parallel_scheduler.as_ref()) {
            (Some(summary), _) => Some(summary),
            (None, Some(epoch)) => Some(epoch.summary()),
            (None, None) => None,
        }
    }

    pub const fn serial_scheduler_summary(&self) -> Option<RunSummary> {
        self.scheduler
    }

    pub const fn parallel_scheduler_epoch(&self) -> Option<&RiscvClusterSchedulerEpoch> {
        self.parallel_scheduler.as_ref()
    }

    pub const fn idle_tick(&self) -> Option<Tick> {
        self.idle_tick
    }

    pub const fn is_idle(&self) -> bool {
        self.idle_tick.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterSchedulerEpoch {
    plan: ParallelEpochPlan,
    summary: RunSummary,
    dispatches: Vec<SchedulerDispatchRecord>,
    batches: Vec<ParallelEpochBatchRecord>,
    profile: ParallelRunProfile,
    partition_activities: BTreeMap<PartitionId, ParallelPartitionActivity>,
}

impl RiscvClusterSchedulerEpoch {
    pub fn new(plan: ParallelEpochPlan, recorded: RecordedRunSummary) -> Self {
        let profile = recorded.profile();
        let partition_activities = recorded.partition_activities();
        Self {
            plan,
            summary: recorded.summary(),
            dispatches: recorded.dispatches().to_vec(),
            batches: recorded.batches().to_vec(),
            profile,
            partition_activities,
        }
    }

    pub const fn plan(&self) -> &ParallelEpochPlan {
        &self.plan
    }

    pub fn horizon(&self) -> Tick {
        self.plan.horizon()
    }

    pub fn ready_partitions(&self) -> &[ReadyPartition] {
        self.plan.ready_partitions()
    }

    pub fn ready_partition_count(&self) -> usize {
        self.plan.ready_partition_count()
    }

    pub fn frontiers(&self) -> &[PartitionFrontier] {
        self.plan.frontiers()
    }

    pub fn frontier(&self, partition: PartitionId) -> Option<PartitionFrontier> {
        self.plan.frontier(partition)
    }

    pub fn serial_blockers(&self) -> &[SchedulerDispatchRecord] {
        self.plan.serial_blockers()
    }

    pub fn serial_blocker_count(&self) -> usize {
        self.plan.serial_blocker_count()
    }

    pub fn first_serial_blocker(&self) -> Option<SchedulerDispatchRecord> {
        self.plan.first_serial_blocker()
    }

    pub fn is_parallel_safe(&self) -> bool {
        self.plan.is_parallel_safe()
    }

    pub const fn summary(&self) -> RunSummary {
        self.summary
    }

    pub const fn turn_summary(&self) -> Option<RunSummary> {
        Some(self.summary)
    }

    pub fn dispatches(&self) -> &[SchedulerDispatchRecord] {
        &self.dispatches
    }

    pub fn batches(&self) -> &[ParallelEpochBatchRecord] {
        &self.batches
    }

    pub const fn profile(&self) -> ParallelRunProfile {
        self.profile
    }

    pub fn dispatch_count(&self) -> usize {
        self.profile.dispatch_count()
    }

    pub fn batch_count(&self) -> usize {
        self.profile.batch_count()
    }

    pub fn empty_epoch_count(&self) -> usize {
        self.profile.empty_epoch_count()
    }

    pub fn is_empty_epoch(&self) -> bool {
        self.profile.empty_epoch_count() != 0
    }

    pub fn max_parallel_workers(&self) -> usize {
        self.profile.max_parallel_workers()
    }

    pub fn total_parallel_workers(&self) -> usize {
        self.profile.total_parallel_workers()
    }

    pub fn has_parallel_work(&self) -> bool {
        self.profile.has_parallel_work()
    }

    pub fn parallel_worker_partitions(&self) -> Vec<PartitionId> {
        self.batches
            .iter()
            .flat_map(ParallelEpochBatchRecord::worker_partitions)
            .collect()
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<ParallelPartitionActivity> {
        self.partition_activities.get(&partition).copied()
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities.len()
    }

    pub fn partition_activities(&self) -> &BTreeMap<PartitionId, ParallelPartitionActivity> {
        &self.partition_activities
    }

    pub fn workers(&self) -> Vec<ParallelWorkerRecord> {
        self.batches
            .iter()
            .flat_map(|batch| batch.workers().iter().copied())
            .collect()
    }

    pub fn dispatches_for_partition(&self, partition: PartitionId) -> Vec<SchedulerDispatchRecord> {
        self.dispatches
            .iter()
            .copied()
            .filter(|record| record.partition() == partition)
            .collect()
    }

    pub fn parallel_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.dispatches
            .iter()
            .copied()
            .filter(|record| record.kind() == ScheduledEventKind::Parallel)
            .collect()
    }

    pub fn serial_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.dispatches
            .iter()
            .copied()
            .filter(|record| record.kind() == ScheduledEventKind::Serial)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterRun {
    turns: Vec<RiscvClusterTurn>,
    stop_reason: RiscvClusterStopReason,
}

impl RiscvClusterRun {
    pub const fn new(turns: Vec<RiscvClusterTurn>, stop_reason: RiscvClusterStopReason) -> Self {
        Self { turns, stop_reason }
    }

    pub fn turns(&self) -> &[RiscvClusterTurn] {
        &self.turns
    }

    pub fn cpu_activity(&self, cpu: CpuId) -> Option<RiscvCoreDriveActivity> {
        self.cpu_activities().remove(&cpu)
    }

    pub fn has_cpu_activity(&self, cpu: CpuId) -> bool {
        self.cpu_activity(cpu)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_cpu_count(&self) -> usize {
        self.cpu_activities().len()
    }

    pub fn cpu_activities(&self) -> BTreeMap<CpuId, RiscvCoreDriveActivity> {
        let mut activities = BTreeMap::new();
        for turn in &self.turns {
            for (cpu, activity) in turn.cpu_activities() {
                activities
                    .entry(cpu)
                    .and_modify(|stored: &mut RiscvCoreDriveActivity| {
                        *stored = stored.merge(activity);
                    })
                    .or_insert(activity);
            }
        }
        activities
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<RiscvCoreDriveActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, RiscvCoreDriveActivity> {
        let mut activities = BTreeMap::new();
        for turn in &self.turns {
            for (partition, activity) in turn.partition_activities() {
                activities
                    .entry(partition)
                    .and_modify(|stored: &mut RiscvCoreDriveActivity| {
                        *stored = stored.merge(activity);
                    })
                    .or_insert(activity);
            }
        }
        activities
    }

    pub const fn stop_reason(&self) -> RiscvClusterStopReason {
        self.stop_reason
    }

    pub fn scheduler_summaries(&self) -> Vec<RunSummary> {
        self.turns
            .iter()
            .filter_map(RiscvClusterTurn::scheduler_summary)
            .collect()
    }

    pub fn parallel_scheduler_epochs(&self) -> Vec<&RiscvClusterSchedulerEpoch> {
        self.turns
            .iter()
            .filter_map(RiscvClusterTurn::parallel_scheduler_epoch)
            .collect()
    }

    pub fn parallel_scheduler_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.dispatches().iter().copied())
            .collect()
    }

    pub fn parallel_scheduler_batches(&self) -> Vec<ParallelEpochBatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.batches().iter().cloned())
            .collect()
    }

    pub fn parallel_scheduler_workers(&self) -> Vec<ParallelWorkerRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(RiscvClusterSchedulerEpoch::workers)
            .collect()
    }

    pub fn parallel_scheduler_worker_partitions(&self) -> Vec<PartitionId> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(RiscvClusterSchedulerEpoch::parallel_worker_partitions)
            .collect()
    }

    pub fn max_parallel_scheduler_workers(&self) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(RiscvClusterSchedulerEpoch::max_parallel_workers)
            .max()
            .unwrap_or(0)
    }

    pub fn parallel_scheduler_profile(&self) -> ParallelRunProfile {
        self.parallel_scheduler_epochs()
            .into_iter()
            .fold(ParallelRunProfile::default(), |profile, epoch| {
                profile.merge(epoch.profile())
            })
    }

    pub fn parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        self.parallel_scheduler_partition_activities()
            .remove(&partition)
    }

    pub fn has_parallel_scheduler_partition_activity(&self, partition: PartitionId) -> bool {
        self.parallel_scheduler_partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_parallel_scheduler_partition_count(&self) -> usize {
        self.parallel_scheduler_partition_activities().len()
    }

    pub fn parallel_scheduler_partition_activities(
        &self,
    ) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        let mut activities = BTreeMap::new();
        for epoch in self.parallel_scheduler_epochs() {
            merge_parallel_partition_activity_maps(&mut activities, epoch.partition_activities());
        }
        activities
    }

    pub fn parallel_scheduler_dispatches_for_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<SchedulerDispatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.dispatches_for_partition(partition))
            .collect()
    }

    pub fn parallel_scheduler_frontiers(&self) -> Vec<PartitionFrontier> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.frontiers().iter().copied())
            .collect()
    }

    pub fn parallel_scheduler_ready_partitions(&self) -> Vec<ReadyPartition> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.ready_partitions().iter().copied())
            .collect()
    }

    pub const fn idle_tick(&self) -> Option<Tick> {
        match self.stop_reason {
            RiscvClusterStopReason::Idle { tick } => Some(tick),
            RiscvClusterStopReason::StopCondition => None,
        }
    }
}

fn drive_parallel_scheduler_turn(
    scheduler: &mut PartitionedScheduler,
) -> Result<RiscvClusterTurn, RiscvClusterError> {
    let Some(plan) = scheduler
        .plan_next_parallel_epoch()
        .map_err(RiscvClusterError::Scheduler)?
    else {
        return Ok(RiscvClusterTurn::idle(scheduler.now()));
    };
    let recorded = scheduler
        .run_next_epoch_parallel_recorded()
        .map_err(RiscvClusterError::Scheduler)?;
    Ok(RiscvClusterTurn::parallel_scheduler(plan, recorded))
}

fn merge_parallel_partition_activity_maps(
    target: &mut BTreeMap<PartitionId, ParallelPartitionActivity>,
    source: &BTreeMap<PartitionId, ParallelPartitionActivity>,
) {
    for (partition, activity) in source {
        target
            .entry(*partition)
            .and_modify(|stored| {
                *stored = ParallelPartitionActivity::with_remote_counts(
                    stored.worker_count() + activity.worker_count(),
                    stored.dispatch_count() + activity.dispatch_count(),
                    stored.remote_send_count() + activity.remote_send_count(),
                    stored.remote_receive_count() + activity.remote_receive_count(),
                    stored
                        .max_pending_events()
                        .max(activity.max_pending_events()),
                );
            })
            .or_insert(*activity);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvClusterStopReason {
    StopCondition,
    Idle { tick: Tick },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvClusterError {
    DuplicateCpu {
        cpu: CpuId,
    },
    DuplicateAgent {
        agent: AgentId,
        existing: CpuId,
        duplicate: CpuId,
    },
    DuplicateFetchEndpoint {
        endpoint: TransportEndpointId,
        existing: CpuId,
        duplicate: CpuId,
    },
    DuplicateDataEndpoint {
        endpoint: TransportEndpointId,
        existing: CpuId,
        duplicate: CpuId,
    },
    UnknownCpu {
        cpu: CpuId,
    },
    Core {
        cpu: CpuId,
        error: RiscvCpuError,
    },
    Scheduler(SchedulerError),
    TurnLimitExceeded {
        limit: usize,
        completed: usize,
    },
}

impl fmt::Display for RiscvClusterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCpu { cpu } => {
                write!(formatter, "CPU {} is already registered", cpu.get())
            }
            Self::DuplicateAgent {
                agent,
                existing,
                duplicate,
            } => write!(
                formatter,
                "agent {} is assigned to CPU {} and CPU {}",
                agent.get(),
                existing.get(),
                duplicate.get()
            ),
            Self::DuplicateFetchEndpoint {
                endpoint,
                existing,
                duplicate,
            } => write!(
                formatter,
                "fetch endpoint {} is assigned to CPU {} and CPU {}",
                endpoint.as_str(),
                existing.get(),
                duplicate.get()
            ),
            Self::DuplicateDataEndpoint {
                endpoint,
                existing,
                duplicate,
            } => write!(
                formatter,
                "data endpoint {} is assigned to CPU {} and CPU {}",
                endpoint.as_str(),
                existing.get(),
                duplicate.get()
            ),
            Self::UnknownCpu { cpu } => write!(formatter, "CPU {} is not registered", cpu.get()),
            Self::Core { cpu, error } => {
                write!(formatter, "CPU {} action failed: {error}", cpu.get())
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::TurnLimitExceeded { limit, completed } => write!(
                formatter,
                "RISC-V cluster run reached turn limit {limit} after {completed} completed turns"
            ),
        }
    }
}

impl Error for RiscvClusterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Core { error, .. } => Some(error),
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}
