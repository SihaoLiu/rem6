use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler, SchedulerContext, Tick};
use rem6_memory::{AccessSize, Address, AgentId, TranslationPageMap};
use rem6_mmio::MmioBus;
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    TargetOutcome,
};

use crate::riscv_cluster_drive::{
    completed_fetch_drive_event, push_completed_fetch_drive_event,
    push_prepared_completed_fetch_drive_event, push_prepared_parallel_fetch_action,
    PreparedParallelAction,
};
pub use crate::riscv_cluster_error::RiscvClusterError;
pub use crate::riscv_cluster_htm::{RiscvClusterHtmAbortOutcome, RiscvClusterHtmBeginOutcome};
use crate::riscv_cluster_run::{
    RiscvClusterDriveEvent, RiscvClusterRun, RiscvClusterStopReason, RiscvClusterTurn,
};
use crate::riscv_cluster_scheduler::{
    drive_parallel_scheduler_turn, drive_parallel_scheduler_turn_until_tick,
};
use crate::riscv_data_issue::PreparedDataParallelAccess;
use crate::riscv_reservation::RiscvReservationTracker;
use crate::{
    CpuId, HtmFailureCause, RiscvCore, RiscvCoreDriveAction, RiscvCpuError,
    RiscvStoreConditionalFailureDiagnostic,
};

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

    pub fn flush_data_translation_tlbs_for_data_route(
        &self,
        route: MemoryRouteId,
    ) -> Option<usize> {
        let mut flushed_entry_count = None;
        for core in self
            .cores
            .values()
            .filter(|core| core.data_route() == Some(route))
        {
            if let Some(core_flushed_entry_count) = core.flush_data_translation_tlb() {
                *flushed_entry_count.get_or_insert(0) += core_flushed_entry_count;
            }
        }
        flushed_entry_count
    }

    pub fn abort_htm_transaction_for_data_route(
        &self,
        route: MemoryRouteId,
        cause: HtmFailureCause,
    ) -> RiscvClusterHtmAbortOutcome {
        let Some((cpu, core)) = self
            .cores
            .iter()
            .find(|(_, core)| core.data_route() == Some(route))
        else {
            return RiscvClusterHtmAbortOutcome::NoMatchingDataRoute { route };
        };
        let cpu = *cpu;
        let Some(active) = core.htm_transaction_snapshot().active().cloned() else {
            return RiscvClusterHtmAbortOutcome::NoActiveTransaction { cpu, route };
        };
        match core.abort_htm_transaction(active.uid(), cause) {
            Ok(abort) => RiscvClusterHtmAbortOutcome::Aborted { cpu, route, abort },
            Err(error) => RiscvClusterHtmAbortOutcome::Failed { cpu, route, error },
        }
    }

    pub fn begin_htm_transaction_for_data_route(
        &self,
        route: MemoryRouteId,
    ) -> RiscvClusterHtmBeginOutcome {
        let Some((cpu, core)) = self
            .cores
            .iter()
            .find(|(_, core)| core.data_route() == Some(route))
        else {
            return RiscvClusterHtmBeginOutcome::NoMatchingDataRoute { route };
        };
        let cpu = *cpu;
        match core.begin_htm_transaction() {
            Ok(begin) => RiscvClusterHtmBeginOutcome::Begun { cpu, route, begin },
            Err(error) => RiscvClusterHtmBeginOutcome::Failed { cpu, route, error },
        }
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

    pub fn store_conditional_failure_diagnostics(
        &self,
    ) -> Vec<RiscvStoreConditionalFailureDiagnostic> {
        self.cores
            .values()
            .flat_map(RiscvCore::store_conditional_failure_diagnostics)
            .collect()
    }

    fn run_result(
        &self,
        turns: Vec<RiscvClusterTurn>,
        stop_reason: RiscvClusterStopReason,
    ) -> RiscvClusterRun {
        RiscvClusterRun::with_store_conditional_failure_diagnostics(
            turns,
            stop_reason,
            self.store_conditional_failure_diagnostics(),
        )
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
            if !core.is_hart_started() {
                continue;
            }
            if core.has_pending_data_access()
                || core.has_unissued_data_access()
                || core.has_pending_trap()
            {
                continue;
            }
            if core.has_pending_fetch() {
                if core.can_retire_completed_fetch_while_fetch_pending()
                    && push_prepared_completed_fetch_drive_event(*cpu, core, &mut prepared_actions)?
                {
                    continue;
                }
                continue;
            }

            if let Some(decision) = core.next_fetch_ahead_before_retire() {
                core.set_fetch_ahead_pc(decision.pc());
                push_prepared_parallel_fetch_action(
                    *cpu,
                    core,
                    scheduler.now(),
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                    &mut prepared_actions,
                    &mut transaction_cpus,
                    &mut transactions,
                    Some(decision),
                )?;
                continue;
            }

            if push_prepared_completed_fetch_drive_event(*cpu, core, &mut prepared_actions)? {
                continue;
            }

            push_prepared_parallel_fetch_action(
                *cpu,
                core,
                scheduler.now(),
                transport,
                fetch_trace.clone(),
                fetch_responder(*cpu),
                &mut prepared_actions,
                &mut transaction_cpus,
                &mut transactions,
                None,
            )?;
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
            if !core.is_hart_started() {
                continue;
            }
            if core.has_pending_data_access() || core.has_pending_trap() {
                continue;
            }
            if core.has_pending_fetch() {
                if core.can_retire_completed_fetch_while_fetch_pending()
                    && push_prepared_completed_fetch_drive_event(*cpu, core, &mut prepared_actions)?
                {
                    continue;
                }
                continue;
            }

            if let Some(decision) = core.next_fetch_ahead_before_retire() {
                core.set_fetch_ahead_pc(decision.pc());
                push_prepared_parallel_fetch_action(
                    *cpu,
                    core,
                    scheduler.now(),
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                    &mut prepared_actions,
                    &mut transaction_cpus,
                    &mut transactions,
                    Some(decision),
                )?;
                continue;
            }

            if push_prepared_completed_fetch_drive_event(*cpu, core, &mut prepared_actions)? {
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

            push_prepared_parallel_fetch_action(
                *cpu,
                core,
                scheduler.now(),
                transport,
                fetch_trace.clone(),
                fetch_responder(*cpu),
                &mut prepared_actions,
                &mut transaction_cpus,
                &mut transactions,
                None,
            )?;
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
    pub fn drive_ready_cores_parallel_with_instruction_budget<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        instruction_budget: u64,
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
        let mut committed_instructions = 0u64;
        let data_only = instruction_budget == 0;
        for (cpu, core) in &self.cores {
            if !core.is_hart_started() {
                continue;
            }
            if core.has_pending_data_access() || core.has_pending_trap() {
                continue;
            }
            if core.has_pending_fetch() {
                if !data_only && core.can_retire_completed_fetch_while_fetch_pending() {
                    if let Some(event) = completed_fetch_drive_event(*cpu, core)? {
                        if committed_instructions >= instruction_budget {
                            break;
                        }
                        prepared_actions.push(PreparedParallelAction::Ready(event));
                        committed_instructions += 1;
                        if committed_instructions >= instruction_budget {
                            break;
                        }
                    }
                }
                continue;
            }

            if !data_only {
                if let Some(decision) = core.next_fetch_ahead_before_retire() {
                    core.set_fetch_ahead_pc(decision.pc());
                    push_prepared_parallel_fetch_action(
                        *cpu,
                        core,
                        scheduler.now(),
                        transport,
                        fetch_trace.clone(),
                        fetch_responder(*cpu),
                        &mut prepared_actions,
                        &mut transaction_cpus,
                        &mut transactions,
                        Some(decision),
                    )?;
                    continue;
                }
            }

            if !data_only {
                if let Some(event) = completed_fetch_drive_event(*cpu, core)? {
                    if committed_instructions >= instruction_budget {
                        break;
                    }
                    prepared_actions.push(PreparedParallelAction::Ready(event));
                    committed_instructions += 1;
                    if committed_instructions >= instruction_budget {
                        break;
                    }
                    continue;
                }
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

            if data_only {
                continue;
            }

            push_prepared_parallel_fetch_action(
                *cpu,
                core,
                scheduler.now(),
                transport,
                fetch_trace.clone(),
                fetch_responder(*cpu),
                &mut prepared_actions,
                &mut transaction_cpus,
                &mut transactions,
                None,
            )?;
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
            if !core.is_hart_started() {
                continue;
            }
            if let Some(event) = core.take_pending_trap_event() {
                prepared_actions.push(PreparedParallelAction::Ready(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                )));
                continue;
            }
            if core.has_outstanding_data_request() || core.has_pending_trap() {
                continue;
            }
            if core.has_pending_fetch() {
                if !core.has_pending_data_access()
                    && core.can_retire_completed_fetch_while_fetch_pending()
                    && push_prepared_completed_fetch_drive_event(*cpu, core, &mut prepared_actions)?
                {
                    continue;
                }
                continue;
            }

            if let Some(decision) = core.next_fetch_ahead_before_retire() {
                core.set_fetch_ahead_pc(decision.pc());
                push_prepared_parallel_fetch_action(
                    *cpu,
                    core,
                    scheduler.now(),
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                    &mut prepared_actions,
                    &mut transaction_cpus,
                    &mut transactions,
                    Some(decision),
                )?;
                continue;
            }

            if push_prepared_completed_fetch_drive_event(*cpu, core, &mut prepared_actions)? {
                continue;
            }

            let has_data_work = core.has_unissued_data_access() || core.has_pending_data_access();
            if has_data_work {
                let prepared = core
                    .prepare_translated_data_parallel_access(
                        scheduler.now(),
                        transport,
                        data_trace.clone(),
                        page_map,
                        data_responder(*cpu),
                    )
                    .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?;
                if let Some(prepared) = prepared {
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
                } else if let Some(event) = core.take_pending_trap_event() {
                    prepared_actions.push(PreparedParallelAction::Ready(
                        RiscvClusterDriveEvent::new(
                            *cpu,
                            RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                        ),
                    ));
                }
                continue;
            }

            push_prepared_parallel_fetch_action(
                *cpu,
                core,
                scheduler.now(),
                transport,
                fetch_trace.clone(),
                fetch_responder(*cpu),
                &mut prepared_actions,
                &mut transaction_cpus,
                &mut transactions,
                None,
            )?;
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
            if !core.is_hart_started() {
                continue;
            }
            if let Some(event) = core.take_pending_trap_event() {
                prepared_actions.push(PreparedParallelAction::Ready(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                )));
                continue;
            }
            if core.has_outstanding_data_request() || core.has_pending_trap() {
                continue;
            }
            if core.has_pending_fetch() {
                if !core.has_pending_data_access()
                    && core.can_retire_completed_fetch_while_fetch_pending()
                    && push_prepared_completed_fetch_drive_event(*cpu, core, &mut prepared_actions)?
                {
                    continue;
                }
                continue;
            }

            if let Some(decision) = core.next_fetch_ahead_before_retire() {
                core.set_fetch_ahead_pc(decision.pc());
                push_prepared_parallel_fetch_action(
                    *cpu,
                    core,
                    scheduler.now(),
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                    &mut prepared_actions,
                    &mut transaction_cpus,
                    &mut transactions,
                    Some(decision),
                )?;
                continue;
            }

            if push_prepared_completed_fetch_drive_event(*cpu, core, &mut prepared_actions)? {
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
                if let Some(event) = core.take_pending_trap_event() {
                    prepared_actions.push(PreparedParallelAction::Ready(
                        RiscvClusterDriveEvent::new(
                            *cpu,
                            RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                        ),
                    ));
                    continue;
                }

                let prepared = core
                    .prepare_translated_data_parallel_access(
                        scheduler.now(),
                        transport,
                        data_trace.clone(),
                        page_map,
                        data_responder(*cpu),
                    )
                    .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?;
                if let Some(prepared) = prepared {
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
                } else if let Some(event) = core.take_pending_trap_event() {
                    prepared_actions.push(PreparedParallelAction::Ready(
                        RiscvClusterDriveEvent::new(
                            *cpu,
                            RiscvCoreDriveAction::InstructionExecuted(Box::new(event)),
                        ),
                    ));
                }
                continue;
            }

            push_prepared_parallel_fetch_action(
                *cpu,
                core,
                scheduler.now(),
                transport,
                fetch_trace.clone(),
                fetch_responder(*cpu),
                &mut prepared_actions,
                &mut transaction_cpus,
                &mut transactions,
                None,
            )?;
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
                    fetch_ahead,
                    transaction_index,
                } => {
                    core.record_prepared_fetch_issue(issue);
                    if let Some(decision) = fetch_ahead {
                        core.record_fetch_ahead_speculation(&decision);
                    }
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
            if !core.is_hart_started() {
                continue;
            }
            if core.has_pending_data_access() || core.has_pending_trap() {
                continue;
            }
            if core.has_pending_fetch() {
                if core.can_retire_completed_fetch_while_fetch_pending()
                    && push_completed_fetch_drive_event(*cpu, core, &mut actions)?
                {
                    continue;
                }
                continue;
            }

            if let Some(decision) = core.next_fetch_ahead_before_retire() {
                core.set_fetch_ahead_pc(decision.pc());
                let event = core
                    .issue_next_fetch_parallel(
                        scheduler,
                        transport,
                        fetch_trace.clone(),
                        fetch_responder(*cpu),
                    )
                    .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?;
                core.record_fetch_ahead_speculation(&decision);
                actions.push(RiscvClusterDriveEvent::new(
                    *cpu,
                    RiscvCoreDriveAction::FetchIssued { event },
                ));
                continue;
            }

            if push_completed_fetch_drive_event(*cpu, core, &mut actions)? {
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
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?;
            actions.push(RiscvClusterDriveEvent::new(
                *cpu,
                RiscvCoreDriveAction::FetchIssued { event },
            ));
        }

        Ok(actions)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_ready_cores_parallel_with_mmio_and_instruction_budget<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        mut fetch_responder: F,
        mut data_responder: D,
        instruction_budget: u64,
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
        let mut committed_instructions = 0u64;
        let data_only = instruction_budget == 0;
        for (cpu, core) in &self.cores {
            if !core.is_hart_started() {
                continue;
            }
            if core.has_pending_data_access() || core.has_pending_trap() {
                continue;
            }
            if core.has_pending_fetch() {
                if !data_only && core.can_retire_completed_fetch_while_fetch_pending() {
                    if let Some(event) = completed_fetch_drive_event(*cpu, core)? {
                        if committed_instructions >= instruction_budget {
                            break;
                        }
                        actions.push(event);
                        committed_instructions += 1;
                        if committed_instructions >= instruction_budget {
                            break;
                        }
                    }
                }
                continue;
            }

            if !data_only {
                if let Some(decision) = core.next_fetch_ahead_before_retire() {
                    core.set_fetch_ahead_pc(decision.pc());
                    let event = core
                        .issue_next_fetch_parallel(
                            scheduler,
                            transport,
                            fetch_trace.clone(),
                            fetch_responder(*cpu),
                        )
                        .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?;
                    core.record_fetch_ahead_speculation(&decision);
                    actions.push(RiscvClusterDriveEvent::new(
                        *cpu,
                        RiscvCoreDriveAction::FetchIssued { event },
                    ));
                    continue;
                }

                if let Some(event) = completed_fetch_drive_event(*cpu, core)? {
                    if committed_instructions >= instruction_budget {
                        break;
                    }
                    actions.push(event);
                    committed_instructions += 1;
                    if committed_instructions >= instruction_budget {
                        break;
                    }
                    continue;
                }
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

            if data_only {
                continue;
            }

            let event = core
                .issue_next_fetch_parallel(
                    scheduler,
                    transport,
                    fetch_trace.clone(),
                    fetch_responder(*cpu),
                )
                .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?;
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
    pub fn drive_turn_parallel_until_tick<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
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

        let core_events = self.drive_ready_cores_parallel(
            scheduler,
            transport,
            fetch_trace,
            data_trace,
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

    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn_parallel_with_instruction_budget<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
        instruction_budget: u64,
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
        let core_events = self.drive_ready_cores_parallel_with_instruction_budget(
            scheduler,
            transport,
            fetch_trace,
            data_trace,
            fetch_responder,
            data_responder,
            instruction_budget,
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
    pub fn drive_turn_parallel_with_instruction_budget_until_tick<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
        instruction_budget: u64,
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

        let core_events = self.drive_ready_cores_parallel_with_instruction_budget(
            scheduler,
            transport,
            fetch_trace,
            data_trace,
            fetch_responder,
            data_responder,
            instruction_budget,
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
    pub fn drive_turn_parallel_with_mmio_until_tick<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
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

    #[allow(clippy::too_many_arguments)]
    pub fn drive_turn_parallel_with_mmio_and_instruction_budget_until_tick<F, D, FR, DR>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        bus: &MmioBus,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
        instruction_budget: u64,
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

        let core_events = self.drive_ready_cores_parallel_with_mmio_and_instruction_budget(
            scheduler,
            transport,
            bus,
            fetch_trace,
            data_trace,
            fetch_responder,
            data_responder,
            instruction_budget,
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
                return Ok(self.run_result(turns, RiscvClusterStopReason::Idle { tick }));
            }
            if stop(&turn) {
                turns.push(turn);
                return Ok(self.run_result(turns, RiscvClusterStopReason::StopCondition));
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
                return Ok(self.run_result(turns, RiscvClusterStopReason::Idle { tick }));
            }
            if stop(&turn) {
                turns.push(turn);
                return Ok(self.run_result(turns, RiscvClusterStopReason::StopCondition));
            }
            turns.push(turn);
        }

        Err(RiscvClusterError::TurnLimitExceeded {
            limit: max_turns,
            completed: turns.len(),
        })
    }
}
