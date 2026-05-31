mod command;
mod error;
mod snapshot;
mod topology;
mod trace;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

pub use command::{
    AcceleratorCommand, AcceleratorCommandId, AcceleratorCommandKind, AcceleratorEngineConfig,
    AcceleratorEngineId, AcceleratorWaitForMarker,
};
pub use error::AcceleratorError;
use rem6_kernel::{
    ConservativeRunSummary, ParallelEpochBatchRecord, ParallelPartitionActivity,
    ParallelRunProfile, ParallelSchedulerContext, PartitionEventId, PartitionId,
    PartitionedScheduler, RecordedConservativeRunSummary, RecordedRunSummary,
    SchedulerDispatchRecord, SchedulerError, Tick, WaitForEdgeKind, WaitForGraph, WaitForNode,
};
use rem6_memory::{Address, ByteMask, MemoryRequest, MemoryRequestId, MemoryResponse};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    ResponseDelivery, TargetOutcome,
};
pub use snapshot::{AcceleratorEngineSnapshot, AcceleratorQueuedCommandSnapshot};
pub use topology::{
    AcceleratorCommandPath, AcceleratorCommandSubmissionConfig, AcceleratorTopologyConfig,
    AcceleratorTopologyDevice,
};
pub use trace::{AcceleratorTraceEvent, AcceleratorTraceKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorCompletion {
    command: AcceleratorCommandId,
    kind: AcceleratorCommandKind,
    lane: u32,
    started_at: Tick,
    completed_at: Tick,
}

impl AcceleratorCompletion {
    pub const fn new(
        command: AcceleratorCommandId,
        kind: AcceleratorCommandKind,
        lane: u32,
        started_at: Tick,
        completed_at: Tick,
    ) -> Self {
        Self {
            command,
            kind,
            lane,
            started_at,
            completed_at,
        }
    }

    pub const fn command(&self) -> AcceleratorCommandId {
        self.command
    }

    pub const fn kind(&self) -> &AcceleratorCommandKind {
        &self.kind
    }

    pub const fn lane(&self) -> u32 {
        self.lane
    }

    pub const fn started_at(&self) -> Tick {
        self.started_at
    }

    pub const fn completed_at(&self) -> Tick {
        self.completed_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorDmaCopy {
    command: AcceleratorCommandId,
    read_route: MemoryRouteId,
    read_request: MemoryRequest,
    write_route: MemoryRouteId,
    write_request: MemoryRequestId,
    destination: Address,
}

impl AcceleratorDmaCopy {
    pub fn new(
        command: AcceleratorCommandId,
        read_route: MemoryRouteId,
        read_request: MemoryRequest,
        write_route: MemoryRouteId,
        write_request: MemoryRequestId,
        destination: Address,
    ) -> Result<Self, AcceleratorError> {
        if !read_request.returns_data() {
            return Err(AcceleratorError::DmaReadRequiresData {
                command,
                request: read_request.id(),
            });
        }

        Ok(Self {
            command,
            read_route,
            read_request,
            write_route,
            write_request,
            destination,
        })
    }

    pub const fn command(&self) -> AcceleratorCommandId {
        self.command
    }

    pub const fn read_route(&self) -> MemoryRouteId {
        self.read_route
    }

    pub const fn read_request(&self) -> &MemoryRequest {
        &self.read_request
    }

    pub const fn write_route(&self) -> MemoryRouteId {
        self.write_route
    }

    pub const fn write_request(&self) -> MemoryRequestId {
        self.write_request
    }

    pub const fn destination(&self) -> Address {
        self.destination
    }

    fn make_write_request(&self, data: Vec<u8>) -> Result<MemoryRequest, AcceleratorError> {
        MemoryRequest::write(
            self.write_request,
            self.destination,
            self.read_request.size(),
            data,
            ByteMask::full(self.read_request.size()).map_err(AcceleratorError::Memory)?,
            self.read_request.line_layout(),
        )
        .map(|request| request.with_ordering(self.read_request.ordering()))
        .map_err(AcceleratorError::Memory)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorPendingDmaWrite {
    copy: AcceleratorDmaCopy,
    data: Vec<u8>,
    read_completed_at: Tick,
}

impl AcceleratorPendingDmaWrite {
    pub fn new(copy: AcceleratorDmaCopy, data: Vec<u8>, read_completed_at: Tick) -> Self {
        Self {
            copy,
            data,
            read_completed_at,
        }
    }

    pub const fn copy(&self) -> &AcceleratorDmaCopy {
        &self.copy
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub const fn read_completed_at(&self) -> Tick {
        self.read_completed_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorDmaCompletion {
    command: AcceleratorCommandId,
    read_request: MemoryRequestId,
    write_request: MemoryRequestId,
    read_completed_at: Tick,
    write_completed_at: Tick,
}

impl AcceleratorDmaCompletion {
    pub const fn new(
        command: AcceleratorCommandId,
        read_request: MemoryRequestId,
        write_request: MemoryRequestId,
        read_completed_at: Tick,
        write_completed_at: Tick,
    ) -> Self {
        Self {
            command,
            read_request,
            write_request,
            read_completed_at,
            write_completed_at,
        }
    }

    pub const fn command(&self) -> AcceleratorCommandId {
        self.command
    }

    pub const fn read_request(&self) -> MemoryRequestId {
        self.read_request
    }

    pub const fn write_request(&self) -> MemoryRequestId {
        self.write_request
    }

    pub const fn read_completed_at(&self) -> Tick {
        self.read_completed_at
    }

    pub const fn write_completed_at(&self) -> Tick {
        self.write_completed_at
    }
}

pub struct AcceleratorPreparedDmaRead {
    issue: AcceleratorDmaIssueRecord,
    transaction: ParallelMemoryTransaction,
}

impl AcceleratorPreparedDmaRead {
    pub fn into_parts(self) -> (AcceleratorDmaIssueRecord, ParallelMemoryTransaction) {
        (self.issue, self.transaction)
    }
}

pub struct AcceleratorPreparedDmaWrite {
    issue: AcceleratorDmaIssueRecord,
    transaction: ParallelMemoryTransaction,
    rollback: AcceleratorDmaWriteRollback,
}

impl AcceleratorPreparedDmaWrite {
    pub fn into_parts(
        self,
    ) -> (
        AcceleratorDmaIssueRecord,
        ParallelMemoryTransaction,
        AcceleratorDmaWriteRollback,
    ) {
        (self.issue, self.transaction, self.rollback)
    }
}

pub struct AcceleratorDmaWriteRollback {
    engine: AcceleratorEngine,
    pending: AcceleratorPendingDmaWrite,
}

impl AcceleratorDmaWriteRollback {
    pub fn restore(self) {
        self.engine.push_pending_dma_write(self.pending);
    }
}

pub struct AcceleratorDmaIssueRecord {
    engine: AcceleratorEngine,
    event: AcceleratorTraceEvent,
}

impl AcceleratorDmaIssueRecord {
    pub fn record(self) {
        self.engine.record(self.event);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelAcceleratorRunSummary {
    scheduler_run: RecordedConservativeRunSummary,
    trace_event_count: usize,
    command_completion_count: usize,
    pending_dma_write_count: usize,
    dma_completion_count: usize,
}

impl ParallelAcceleratorRunSummary {
    pub const fn new(
        scheduler_run: RecordedConservativeRunSummary,
        trace_event_count: usize,
        command_completion_count: usize,
        pending_dma_write_count: usize,
        dma_completion_count: usize,
    ) -> Self {
        Self {
            scheduler_run,
            trace_event_count,
            command_completion_count,
            pending_dma_write_count,
            dma_completion_count,
        }
    }

    pub const fn scheduler_run(&self) -> &RecordedConservativeRunSummary {
        &self.scheduler_run
    }

    pub fn scheduler_epochs(&self) -> &[RecordedRunSummary] {
        self.scheduler_run.epochs()
    }

    pub fn summary(&self) -> ConservativeRunSummary {
        self.scheduler_run.summary()
    }

    pub fn profile(&self) -> ParallelRunProfile {
        self.scheduler_run.profile()
    }

    pub fn epoch_count(&self) -> usize {
        self.scheduler_run.epoch_count()
    }

    pub fn empty_epoch_count(&self) -> usize {
        self.scheduler_run.empty_epoch_count()
    }

    pub fn dispatch_count(&self) -> usize {
        self.scheduler_run.dispatch_count()
    }

    pub fn batch_count(&self) -> usize {
        self.scheduler_run.batch_count()
    }

    pub fn max_parallel_workers(&self) -> usize {
        self.scheduler_run.max_parallel_workers()
    }

    pub fn total_parallel_workers(&self) -> usize {
        self.scheduler_run.total_parallel_workers()
    }

    pub fn has_parallel_work(&self) -> bool {
        self.scheduler_run.has_parallel_work()
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<ParallelPartitionActivity> {
        self.scheduler_run.partition_activity(partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.scheduler_run.has_partition_activity(partition)
    }

    pub fn active_partition_count(&self) -> usize {
        self.scheduler_run.active_partition_count()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        self.scheduler_run.partition_activities()
    }

    pub fn dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.scheduler_run.dispatches()
    }

    pub fn batches(&self) -> Vec<ParallelEpochBatchRecord> {
        self.scheduler_run.batches()
    }

    pub fn executed_events(&self) -> usize {
        self.summary().executed_events()
    }

    pub fn final_tick(&self) -> Tick {
        self.summary().final_tick()
    }

    pub const fn trace_event_count(&self) -> usize {
        self.trace_event_count
    }

    pub const fn command_completion_count(&self) -> usize {
        self.command_completion_count
    }

    pub const fn pending_dma_write_count(&self) -> usize {
        self.pending_dma_write_count
    }

    pub const fn dma_completion_count(&self) -> usize {
        self.dma_completion_count
    }

    pub const fn device_activity_count(&self) -> usize {
        self.trace_event_count
            + self.command_completion_count
            + self.pending_dma_write_count
            + self.dma_completion_count
    }

    pub const fn has_device_activity(&self) -> bool {
        self.device_activity_count() != 0
    }

    pub const fn has_command_activity(&self) -> bool {
        self.command_completion_count != 0
    }

    pub const fn has_dma_activity(&self) -> bool {
        self.pending_dma_write_count != 0 || self.dma_completion_count != 0
    }
}

#[derive(Clone)]
pub struct AcceleratorEngine {
    config: AcceleratorEngineConfig,
    state: Arc<Mutex<AcceleratorEngineState>>,
}

impl AcceleratorEngine {
    pub fn new(config: AcceleratorEngineConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(AcceleratorEngineState::new(config.lanes()))),
            config,
        }
    }

    pub const fn id(&self) -> AcceleratorEngineId {
        self.config.id()
    }

    pub const fn partition(&self) -> PartitionId {
        self.config.partition()
    }

    pub const fn lanes(&self) -> u32 {
        self.config.lanes()
    }

    pub fn submit_from_partition(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: PartitionId,
        submission_latency: Tick,
        command: AcceleratorCommand,
    ) -> Result<PartitionEventId, AcceleratorError> {
        let target = self.partition();
        let source_tick = scheduler
            .partition_now(source)
            .map_err(AcceleratorError::Scheduler)?;
        scheduler
            .partition_now(target)
            .map_err(AcceleratorError::Scheduler)?;
        validate_submission_latency(
            source,
            target,
            source_tick,
            scheduler.min_remote_delay(),
            submission_latency,
        )?;

        let engine = self.clone();
        scheduler
            .schedule_parallel_at(source, source_tick, move |context| {
                engine.record(AcceleratorTraceEvent::new(
                    context.now(),
                    AcceleratorTraceKind::Submitted {
                        command: command.id(),
                        source,
                        target,
                    },
                ));
                let target_engine = engine.clone();
                context
                    .schedule_remote_after(target, submission_latency, move |context| {
                        target_engine.accept_on_partition(context, command);
                    })
                    .expect("accelerator submission latency was validated");
            })
            .map_err(AcceleratorError::Scheduler)
    }

    pub fn trace(&self) -> Vec<AcceleratorTraceEvent> {
        self.state
            .lock()
            .expect("accelerator state lock")
            .trace
            .clone()
    }

    pub fn completed(&self) -> Vec<AcceleratorCompletion> {
        self.state
            .lock()
            .expect("accelerator state lock")
            .completed
            .clone()
    }

    pub fn pending_dma_writes(&self) -> Vec<AcceleratorPendingDmaWrite> {
        self.state
            .lock()
            .expect("accelerator state lock")
            .pending_dma_writes
            .clone()
    }

    pub fn dma_completions(&self) -> Vec<AcceleratorDmaCompletion> {
        self.state
            .lock()
            .expect("accelerator state lock")
            .dma_completions
            .clone()
    }

    pub fn snapshot(&self) -> AcceleratorEngineSnapshot {
        self.state
            .lock()
            .expect("accelerator state lock")
            .snapshot()
    }

    pub fn wait_for_graph(&self) -> WaitForGraph {
        self.state
            .lock()
            .expect("accelerator state lock")
            .wait_for_graph(self.id())
    }

    pub fn mark_wait_for(&self) -> AcceleratorWaitForMarker {
        self.state
            .lock()
            .expect("accelerator state lock")
            .mark_wait_for()
    }

    pub fn wait_for_graph_since(&self, marker: AcceleratorWaitForMarker) -> WaitForGraph {
        self.state
            .lock()
            .expect("accelerator state lock")
            .wait_for_graph_since(self.id(), marker)
    }

    pub fn validate_snapshot(
        &self,
        snapshot: &AcceleratorEngineSnapshot,
    ) -> Result<(), AcceleratorError> {
        let expected = self.config.lanes() as usize;
        let actual = snapshot.lane_count();
        if actual != expected {
            return Err(AcceleratorError::SnapshotLaneCountMismatch {
                engine: self.id(),
                expected,
                actual,
            });
        }

        Ok(())
    }

    pub fn restore(&self, snapshot: &AcceleratorEngineSnapshot) -> Result<(), AcceleratorError> {
        self.validate_snapshot(snapshot)?;
        *self.state.lock().expect("accelerator state lock") =
            AcceleratorEngineState::from_snapshot(snapshot);
        Ok(())
    }

    pub fn run_until_idle_parallel_recorded(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<ParallelAcceleratorRunSummary, AcceleratorError> {
        let before = self.snapshot();
        let scheduler_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(AcceleratorError::Scheduler)?;
        let after = self.snapshot();

        Ok(ParallelAcceleratorRunSummary::new(
            scheduler_run,
            after.trace().len().saturating_sub(before.trace().len()),
            after
                .completed()
                .len()
                .saturating_sub(before.completed().len()),
            after.pending_dma_writes().len(),
            after
                .dma_completions()
                .len()
                .saturating_sub(before.dma_completions().len()),
        ))
    }

    pub fn submit_dma_copy_read<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        copy: AcceleratorDmaCopy,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, AcceleratorError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let prepared = self.prepare_dma_copy_read(scheduler.now(), copy, trace, responder);
        let (issue, transaction) = prepared.into_parts();
        let event = transport
            .submit_parallel_batch(scheduler, [transaction])
            .map_err(AcceleratorError::Transport)?
            .into_iter()
            .next()
            .expect("one DMA read transaction was submitted");
        issue.record();
        Ok(event)
    }

    pub fn prepare_dma_copy_read<F>(
        &self,
        issued_at: Tick,
        copy: AcceleratorDmaCopy,
        trace: MemoryTrace,
        responder: F,
    ) -> AcceleratorPreparedDmaRead
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let command = copy.command();
        let request = copy.read_request().id();
        let read_request = copy.read_request().clone();
        let sink_engine = self.clone();
        let sink_copy = copy.clone();
        let transaction = ParallelMemoryTransaction::new(
            copy.read_route(),
            read_request,
            trace,
            responder,
            move |delivery| sink_engine.accept_dma_read_response(sink_copy, delivery),
        );
        AcceleratorPreparedDmaRead {
            issue: AcceleratorDmaIssueRecord {
                engine: self.clone(),
                event: AcceleratorTraceEvent::new(
                    issued_at,
                    AcceleratorTraceKind::DmaReadIssued { command, request },
                ),
            },
            transaction,
        }
    }

    pub fn issue_next_dma_write<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<Option<PartitionEventId>, AcceleratorError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(prepared) = self.prepare_next_dma_write(scheduler.now(), trace, responder)? else {
            return Ok(None);
        };
        let (issue, transaction, rollback) = prepared.into_parts();
        let event = match transport.submit_parallel_batch(scheduler, [transaction]) {
            Ok(events) => events
                .into_iter()
                .next()
                .expect("one DMA write transaction was submitted"),
            Err(error) => {
                rollback.restore();
                return Err(AcceleratorError::Transport(error));
            }
        };
        issue.record();
        Ok(Some(event))
    }

    pub fn prepare_next_dma_write<F>(
        &self,
        issued_at: Tick,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<Option<AcceleratorPreparedDmaWrite>, AcceleratorError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(pending) = self.pop_pending_dma_write() else {
            return Ok(None);
        };
        let rollback_pending = pending.clone();
        let write_request = match pending.copy.make_write_request(pending.data.clone()) {
            Ok(request) => request,
            Err(error) => {
                self.push_pending_dma_write(pending);
                return Err(error);
            }
        };
        let command = pending.copy.command();
        let request = write_request.id();
        let sink_engine = self.clone();
        let completion = AcceleratorDmaCompletion::new(
            command,
            pending.copy.read_request().id(),
            write_request.id(),
            pending.read_completed_at(),
            0,
        );
        let route = pending.copy.write_route();
        let transaction = ParallelMemoryTransaction::new(
            route,
            write_request,
            trace,
            responder,
            move |delivery| sink_engine.accept_dma_write_response(completion, delivery),
        );
        Ok(Some(AcceleratorPreparedDmaWrite {
            issue: AcceleratorDmaIssueRecord {
                engine: self.clone(),
                event: AcceleratorTraceEvent::new(
                    issued_at,
                    AcceleratorTraceKind::DmaWriteIssued { command, request },
                ),
            },
            transaction,
            rollback: AcceleratorDmaWriteRollback {
                engine: self.clone(),
                pending: rollback_pending,
            },
        }))
    }

    fn accept_on_partition(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        command: AcceleratorCommand,
    ) {
        let reservation = self.reserve(context.now(), command.execution_latency());
        if reservation.started_at == context.now() {
            self.start_now(context, command, reservation);
            return;
        }

        self.record_queued_command(command.clone(), context.now(), reservation);
        let engine = self.clone();
        context
            .schedule_local_after(reservation.started_at - context.now(), move |context| {
                engine.start_now(context, command, reservation);
            })
            .expect("accelerator queued start tick was reserved");
    }

    fn start_now(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        command: AcceleratorCommand,
        reservation: AcceleratorReservation,
    ) {
        self.clear_queued_command(command.id(), reservation.lane, reservation.started_at);
        self.record(AcceleratorTraceEvent::new(
            context.now(),
            AcceleratorTraceKind::Started {
                command: command.id(),
                lane: reservation.lane,
                complete_at: reservation.completed_at,
            },
        ));

        let engine = self.clone();
        context
            .schedule_local_after(command.execution_latency(), move |context| {
                engine.complete(context.now(), command, reservation);
            })
            .expect("accelerator command execution latency was validated");
    }

    fn complete(
        &self,
        tick: Tick,
        command: AcceleratorCommand,
        reservation: AcceleratorReservation,
    ) {
        let mut state = self.state.lock().expect("accelerator state lock");
        state.trace.push(AcceleratorTraceEvent::new(
            tick,
            AcceleratorTraceKind::Completed {
                command: command.id(),
                lane: reservation.lane,
            },
        ));
        state.completed.push(AcceleratorCompletion::new(
            command.id(),
            command.kind().clone(),
            reservation.lane,
            reservation.started_at,
            tick,
        ));
    }

    fn reserve(&self, now: Tick, execution_latency: Tick) -> AcceleratorReservation {
        let mut state = self.state.lock().expect("accelerator state lock");
        let (lane, available_at) = state
            .lane_busy_until
            .iter()
            .copied()
            .enumerate()
            .min_by_key(|(lane, tick)| (*tick, *lane))
            .expect("accelerator engine has at least one lane");
        let started_at = now.max(available_at);
        let completed_at = started_at
            .checked_add(execution_latency)
            .expect("accelerator command completion tick fits");
        state.lane_busy_until[lane] = completed_at;

        AcceleratorReservation {
            lane: lane as u32,
            started_at,
            completed_at,
        }
    }

    fn record(&self, event: AcceleratorTraceEvent) {
        self.state
            .lock()
            .expect("accelerator state lock")
            .trace
            .push(event);
    }

    fn record_queued_command(
        &self,
        command: AcceleratorCommand,
        queued_at: Tick,
        reservation: AcceleratorReservation,
    ) {
        let queued = AcceleratorQueuedCommand::new(
            command,
            reservation.lane,
            queued_at,
            reservation.started_at,
            reservation.completed_at,
        );
        let mut state = self.state.lock().expect("accelerator state lock");
        state.wait_log.push(queued.clone());
        state.queued_commands.push(queued);
    }

    fn clear_queued_command(&self, command: AcceleratorCommandId, lane: u32, started_at: Tick) {
        let mut state = self.state.lock().expect("accelerator state lock");
        if let Some(index) = state.queued_commands.iter().position(|queued| {
            queued.command.id() == command && queued.lane == lane && queued.started_at == started_at
        }) {
            state.queued_commands.remove(index);
        }
    }

    fn accept_dma_read_response(&self, copy: AcceleratorDmaCopy, delivery: ResponseDelivery) {
        if let Some(data) = response_data(delivery.response()) {
            let bytes = data.len() as u64;
            let mut state = self.state.lock().expect("accelerator state lock");
            state.trace.push(AcceleratorTraceEvent::new(
                delivery.tick(),
                AcceleratorTraceKind::DmaReadCompleted {
                    command: copy.command(),
                    request: copy.read_request().id(),
                    bytes,
                },
            ));
            state
                .pending_dma_writes
                .push(AcceleratorPendingDmaWrite::new(copy, data, delivery.tick()));
        }
    }

    fn accept_dma_write_response(
        &self,
        mut completion: AcceleratorDmaCompletion,
        delivery: ResponseDelivery,
    ) {
        completion.write_completed_at = delivery.tick();
        let mut state = self.state.lock().expect("accelerator state lock");
        state.trace.push(AcceleratorTraceEvent::new(
            delivery.tick(),
            AcceleratorTraceKind::DmaWriteCompleted {
                command: completion.command(),
                request: completion.write_request(),
            },
        ));
        state.dma_completions.push(completion);
    }

    fn pop_pending_dma_write(&self) -> Option<AcceleratorPendingDmaWrite> {
        let mut state = self.state.lock().expect("accelerator state lock");
        if state.pending_dma_writes.is_empty() {
            None
        } else {
            Some(state.pending_dma_writes.remove(0))
        }
    }

    fn push_pending_dma_write(&self, pending: AcceleratorPendingDmaWrite) {
        self.state
            .lock()
            .expect("accelerator state lock")
            .pending_dma_writes
            .insert(0, pending);
    }
}

impl fmt::Debug for AcceleratorEngine {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AcceleratorEngine")
            .field("id", &self.id())
            .field("partition", &self.partition())
            .field("lanes", &self.lanes())
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AcceleratorReservation {
    lane: u32,
    started_at: Tick,
    completed_at: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AcceleratorEngineState {
    lane_busy_until: Vec<Tick>,
    queued_commands: Vec<AcceleratorQueuedCommand>,
    trace: Vec<AcceleratorTraceEvent>,
    completed: Vec<AcceleratorCompletion>,
    pending_dma_writes: Vec<AcceleratorPendingDmaWrite>,
    dma_completions: Vec<AcceleratorDmaCompletion>,
    wait_log: Vec<AcceleratorQueuedCommand>,
}

impl AcceleratorEngineState {
    fn new(lanes: u32) -> Self {
        Self {
            lane_busy_until: vec![0; lanes as usize],
            queued_commands: Vec::new(),
            trace: Vec::new(),
            completed: Vec::new(),
            pending_dma_writes: Vec::new(),
            dma_completions: Vec::new(),
            wait_log: Vec::new(),
        }
    }

    fn snapshot(&self) -> AcceleratorEngineSnapshot {
        AcceleratorEngineSnapshot::new(
            self.lane_busy_until.clone(),
            self.trace.clone(),
            self.completed.clone(),
            self.pending_dma_writes.clone(),
            self.dma_completions.clone(),
        )
        .with_queued_commands(
            self.queued_commands
                .iter()
                .map(AcceleratorQueuedCommand::snapshot)
                .collect(),
        )
    }

    fn from_snapshot(snapshot: &AcceleratorEngineSnapshot) -> Self {
        Self {
            lane_busy_until: snapshot.lane_busy_until().to_vec(),
            queued_commands: snapshot
                .queued_commands()
                .iter()
                .map(AcceleratorQueuedCommand::from_snapshot)
                .collect(),
            trace: snapshot.trace().to_vec(),
            completed: snapshot.completed().to_vec(),
            pending_dma_writes: snapshot.pending_dma_writes().to_vec(),
            dma_completions: snapshot.dma_completions().to_vec(),
            wait_log: Vec::new(),
        }
    }

    fn mark_wait_for(&self) -> AcceleratorWaitForMarker {
        AcceleratorWaitForMarker::new(self.wait_log.len())
    }

    fn wait_for_graph(&self, engine: AcceleratorEngineId) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        for queued in &self.queued_commands {
            if queued.started_at <= queued.queued_at {
                continue;
            }
            record_accelerator_wait_interval(&mut graph, engine, queued, queued.queued_at);
        }
        graph
    }

    fn wait_for_graph_since(
        &self,
        engine: AcceleratorEngineId,
        marker: AcceleratorWaitForMarker,
    ) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        let Some(records) = self.wait_log.get(marker.offset()..) else {
            return graph;
        };
        for queued in records {
            record_accelerator_wait_interval(
                &mut graph,
                engine,
                queued,
                queued.started_at.saturating_sub(1),
            );
        }
        graph
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AcceleratorQueuedCommand {
    command: AcceleratorCommand,
    lane: u32,
    queued_at: Tick,
    started_at: Tick,
    completed_at: Tick,
}

impl AcceleratorQueuedCommand {
    fn new(
        command: AcceleratorCommand,
        lane: u32,
        queued_at: Tick,
        started_at: Tick,
        completed_at: Tick,
    ) -> Self {
        Self {
            command,
            lane,
            queued_at,
            started_at,
            completed_at,
        }
    }

    fn snapshot(&self) -> AcceleratorQueuedCommandSnapshot {
        AcceleratorQueuedCommandSnapshot::new(
            self.command.clone(),
            self.lane,
            self.queued_at,
            self.started_at,
            self.completed_at,
        )
    }

    fn from_snapshot(snapshot: &AcceleratorQueuedCommandSnapshot) -> Self {
        Self {
            command: snapshot.command().clone(),
            lane: snapshot.lane(),
            queued_at: snapshot.queued_at(),
            started_at: snapshot.started_at(),
            completed_at: snapshot.completed_at(),
        }
    }
}

fn record_accelerator_wait_interval(
    graph: &mut WaitForGraph,
    engine: AcceleratorEngineId,
    queued: &AcceleratorQueuedCommand,
    last_tick: Tick,
) {
    let source = accelerator_command_node(engine, queued.command.id());
    let target = accelerator_lane_node(engine, queued.lane);
    graph
        .record_wait(
            source.clone(),
            target.clone(),
            WaitForEdgeKind::Queue,
            queued.queued_at,
        )
        .expect("accelerator wait-for labels are generated from typed ids");
    if last_tick != queued.queued_at {
        graph
            .record_wait(source, target, WaitForEdgeKind::Queue, last_tick)
            .expect("accelerator wait-for labels are generated from typed ids");
    }
}

fn accelerator_command_node(
    engine: AcceleratorEngineId,
    command: AcceleratorCommandId,
) -> WaitForNode {
    WaitForNode::transaction(format!(
        "accelerator.{}.command.{}",
        engine.get(),
        command.get()
    ))
    .expect("accelerator command wait-for label is generated from numeric ids")
}

fn accelerator_lane_node(engine: AcceleratorEngineId, lane: u32) -> WaitForNode {
    WaitForNode::resource(format!("accelerator.{}.lane.{}", engine.get(), lane))
        .expect("accelerator lane wait-for label is generated from numeric ids")
}

fn response_data(response: &MemoryResponse) -> Option<Vec<u8>> {
    response.data().map(<[u8]>::to_vec)
}

fn validate_submission_latency(
    source: PartitionId,
    target: PartitionId,
    source_tick: Tick,
    min_remote_delay: Tick,
    delay: Tick,
) -> Result<(), AcceleratorError> {
    if source != target && delay == 0 {
        return Err(AcceleratorError::Scheduler(
            SchedulerError::ZeroDelayRemoteMessage { source, target },
        ));
    }
    let delivery_tick = source_tick
        .checked_add(delay)
        .ok_or(AcceleratorError::TickOverflow {
            now: source_tick,
            delay,
        })?;

    if source != target {
        let minimum_delivery_tick =
            source_tick
                .checked_add(min_remote_delay)
                .ok_or(AcceleratorError::TickOverflow {
                    now: source_tick,
                    delay: min_remote_delay,
                })?;
        if delivery_tick < minimum_delivery_tick {
            return Err(AcceleratorError::Scheduler(
                SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                    source,
                    target,
                    source_tick,
                    delivery_tick,
                    minimum_delivery_tick,
                },
            ));
        }
    }

    Ok(())
}
