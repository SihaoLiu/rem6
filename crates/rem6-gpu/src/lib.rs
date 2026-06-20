mod command;
mod error;
mod isa;
mod memory_access;
mod snapshot;
mod summary;
mod topology;
mod trace;

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

pub use command::{
    GpuComputeConfig, GpuDeviceId, GpuDmaId, GpuKernelId, GpuKernelLaunch, GpuWaitForMarker,
    GpuWorkgroupId,
};
pub use error::GpuError;
pub use isa::{GpuIsaInstruction, GpuIsaProgram, GpuScalarRegister, GpuWorkgroupIsaState};
pub use memory_access::{
    GpuCoalescedMemoryAccess, GpuCoalescedMemoryAccessContext, GpuMemoryAccessKind,
};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError,
    Tick, WaitForEdgeKind, WaitForGraph, WaitForNode,
};
use rem6_memory::{Address, ByteMask, MemoryRequest, MemoryRequestId, MemoryResponse};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    ResponseDelivery, TargetOutcome,
};
pub use snapshot::{
    GpuDeviceSnapshot, GpuQueuedIsaProgramSnapshot, GpuQueuedWorkgroupSnapshot, GpuSlotSnapshot,
};
pub use summary::{GpuComputeUnitQueueWaitSummary, ParallelGpuRunSummary};

pub use topology::{GpuCommandPath, GpuTopologyConfig, GpuTopologyDevice};
pub use trace::{GpuTraceEvent, GpuTraceKind, GpuWorkgroupCompletion};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuDmaCopy {
    transfer: GpuDmaId,
    read_route: MemoryRouteId,
    read_request: MemoryRequest,
    write_route: MemoryRouteId,
    write_request: MemoryRequestId,
    destination: Address,
}

impl GpuDmaCopy {
    pub fn new(
        transfer: GpuDmaId,
        read_route: MemoryRouteId,
        read_request: MemoryRequest,
        write_route: MemoryRouteId,
        write_request: MemoryRequestId,
        destination: Address,
    ) -> Result<Self, GpuError> {
        if !read_request.returns_data() {
            return Err(GpuError::DmaReadRequiresData {
                transfer,
                request: read_request.id(),
            });
        }

        Ok(Self {
            transfer,
            read_route,
            read_request,
            write_route,
            write_request,
            destination,
        })
    }

    pub const fn transfer(&self) -> GpuDmaId {
        self.transfer
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

    fn make_write_request(&self, data: Vec<u8>) -> Result<MemoryRequest, GpuError> {
        MemoryRequest::write(
            self.write_request,
            self.destination,
            self.read_request.size(),
            data,
            ByteMask::full(self.read_request.size()).map_err(GpuError::Memory)?,
            self.read_request.line_layout(),
        )
        .map(|request| request.with_ordering(self.read_request.ordering()))
        .map_err(GpuError::Memory)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuPendingDmaWrite {
    copy: GpuDmaCopy,
    data: Vec<u8>,
    read_completed_at: Tick,
}

impl GpuPendingDmaWrite {
    pub fn new(copy: GpuDmaCopy, data: Vec<u8>, read_completed_at: Tick) -> Self {
        Self {
            copy,
            data,
            read_completed_at,
        }
    }

    pub const fn copy(&self) -> &GpuDmaCopy {
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
pub struct GpuDmaCompletion {
    transfer: GpuDmaId,
    read_request: MemoryRequestId,
    write_request: MemoryRequestId,
    read_completed_at: Tick,
    write_completed_at: Tick,
}

impl GpuDmaCompletion {
    pub const fn new(
        transfer: GpuDmaId,
        read_request: MemoryRequestId,
        write_request: MemoryRequestId,
        read_completed_at: Tick,
        write_completed_at: Tick,
    ) -> Self {
        Self {
            transfer,
            read_request,
            write_request,
            read_completed_at,
            write_completed_at,
        }
    }

    pub const fn transfer(&self) -> GpuDmaId {
        self.transfer
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

pub struct GpuPreparedDmaRead {
    issue: GpuDmaIssueRecord,
    transaction: ParallelMemoryTransaction,
}

impl GpuPreparedDmaRead {
    pub fn into_parts(self) -> (GpuDmaIssueRecord, ParallelMemoryTransaction) {
        (self.issue, self.transaction)
    }
}

pub struct GpuPreparedDmaWrite {
    issue: GpuDmaIssueRecord,
    transaction: ParallelMemoryTransaction,
    rollback: GpuDmaWriteRollback,
}

impl GpuPreparedDmaWrite {
    pub fn into_parts(
        self,
    ) -> (
        GpuDmaIssueRecord,
        ParallelMemoryTransaction,
        GpuDmaWriteRollback,
    ) {
        (self.issue, self.transaction, self.rollback)
    }
}

pub struct GpuDmaWriteRollback {
    gpu: GpuDevice,
    pending: GpuPendingDmaWrite,
}

impl GpuDmaWriteRollback {
    pub fn restore(self) {
        self.gpu.push_pending_dma_write(self.pending);
    }
}

pub struct GpuDmaIssueRecord {
    gpu: GpuDevice,
    event: GpuTraceEvent,
}

impl GpuDmaIssueRecord {
    pub fn record(self) {
        self.gpu.record(self.event);
    }
}

#[derive(Clone, Debug)]
pub struct GpuDevice {
    config: GpuComputeConfig,
    state: Arc<Mutex<GpuDeviceState>>,
}

impl GpuDevice {
    pub fn new(config: GpuComputeConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(GpuDeviceState::new(&config))),
            config,
        }
    }

    pub const fn id(&self) -> GpuDeviceId {
        self.config.device()
    }

    pub const fn partition(&self) -> PartitionId {
        self.config.partition()
    }

    pub const fn compute_units(&self) -> u32 {
        self.config.compute_units()
    }

    pub const fn wave_slots_per_compute_unit(&self) -> u32 {
        self.config.wave_slots_per_compute_unit()
    }

    pub fn submit_kernel_from_partition(
        &self,
        scheduler: &mut PartitionedScheduler,
        source: PartitionId,
        submission_latency: Tick,
        launch: GpuKernelLaunch,
    ) -> Result<PartitionEventId, GpuError> {
        let target = self.partition();
        let source_tick = scheduler
            .partition_now(source)
            .map_err(GpuError::Scheduler)?;
        scheduler
            .partition_now(target)
            .map_err(GpuError::Scheduler)?;
        validate_submission_latency(
            source,
            target,
            source_tick,
            scheduler.min_remote_delay(),
            submission_latency,
        )?;

        let gpu = self.clone();
        scheduler
            .schedule_parallel_at(source, source_tick, move |context| {
                gpu.record(GpuTraceEvent::new(
                    context.now(),
                    GpuTraceKind::LaunchSubmitted {
                        kernel: launch.kernel(),
                        source,
                        target,
                    },
                ));
                let target_gpu = gpu.clone();
                context
                    .schedule_remote_after(target, submission_latency, move |context| {
                        target_gpu.accept_launch(context, launch);
                    })
                    .expect("GPU submission latency was validated");
            })
            .map_err(GpuError::Scheduler)
    }

    pub fn trace(&self) -> Vec<GpuTraceEvent> {
        self.state.lock().expect("GPU state lock").trace.clone()
    }

    pub fn completions(&self) -> Vec<GpuWorkgroupCompletion> {
        self.state
            .lock()
            .expect("GPU state lock")
            .completions
            .clone()
    }

    pub fn pending_dma_writes(&self) -> Vec<GpuPendingDmaWrite> {
        self.state
            .lock()
            .expect("GPU state lock")
            .pending_dma_writes
            .clone()
    }

    pub fn dma_completions(&self) -> Vec<GpuDmaCompletion> {
        self.state
            .lock()
            .expect("GPU state lock")
            .dma_completions
            .clone()
    }

    pub fn snapshot(&self) -> GpuDeviceSnapshot {
        self.state.lock().expect("GPU state lock").snapshot()
    }

    pub fn wait_for_graph(&self) -> WaitForGraph {
        self.state
            .lock()
            .expect("GPU state lock")
            .wait_for_graph(self.id())
    }

    pub fn mark_wait_for(&self) -> GpuWaitForMarker {
        self.state.lock().expect("GPU state lock").mark_wait_for()
    }

    pub fn wait_for_graph_since(&self, marker: GpuWaitForMarker) -> WaitForGraph {
        self.state
            .lock()
            .expect("GPU state lock")
            .wait_for_graph_since(self.id(), marker)
    }

    pub fn validate_snapshot(&self, snapshot: &GpuDeviceSnapshot) -> Result<(), GpuError> {
        let expected = self.config.slot_count();
        let actual = snapshot.slot_count();
        if actual != expected {
            return Err(GpuError::SnapshotSlotCountMismatch {
                device: self.id(),
                expected,
                actual,
            });
        }
        let mut seen = snapshot
            .slots()
            .iter()
            .map(|slot| vec![false; slot.queued().len()])
            .collect::<Vec<_>>();

        for program in snapshot.queued_isa_programs() {
            let slot_index = program.slot_index();
            let queue_index = program.queue_index();
            let Some(slot_seen) = seen.get_mut(slot_index) else {
                return Err(GpuError::SnapshotQueuedIsaProgramOutOfRange {
                    device: self.id(),
                    slot_index,
                    queue_index,
                });
            };
            let Some(entry_seen) = slot_seen.get_mut(queue_index) else {
                return Err(GpuError::SnapshotQueuedIsaProgramOutOfRange {
                    device: self.id(),
                    slot_index,
                    queue_index,
                });
            };
            if *entry_seen {
                return Err(GpuError::SnapshotQueuedIsaProgramDuplicate {
                    device: self.id(),
                    slot_index,
                    queue_index,
                });
            }
            *entry_seen = true;
        }

        for (slot_index, slot_seen) in seen.iter().enumerate() {
            for (queue_index, entry_seen) in slot_seen.iter().enumerate() {
                if !entry_seen {
                    return Err(GpuError::SnapshotQueuedIsaProgramMissing {
                        device: self.id(),
                        slot_index,
                        queue_index,
                    });
                }
            }
        }

        Ok(())
    }

    pub fn restore(&self, snapshot: &GpuDeviceSnapshot) -> Result<(), GpuError> {
        self.validate_snapshot(snapshot)?;
        *self.state.lock().expect("GPU state lock") = GpuDeviceState::from_snapshot(snapshot);
        Ok(())
    }

    pub fn run_until_idle_parallel_recorded(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<ParallelGpuRunSummary, GpuError> {
        let before = self.snapshot();
        let before_memory_access_count = snapshot_memory_access_count(&before);
        let mut queue_wait_records = snapshot_queued_workgroups(&before);
        let before_wait_log_count = self.state.lock().expect("GPU state lock").wait_log.len();
        self.schedule_queued_slots_from_scheduler(scheduler)?;
        let scheduler_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(GpuError::Scheduler)?;
        let after = self.snapshot();
        let after_memory_access_count = snapshot_memory_access_count(&after);
        let queue_waits = {
            let state = self.state.lock().expect("GPU state lock");
            queue_wait_records.extend_from_slice(&state.wait_log[before_wait_log_count..]);
            gpu_queue_wait_summary(&queue_wait_records)
        };

        Ok(ParallelGpuRunSummary::new(
            scheduler_run,
            after.trace().len().saturating_sub(before.trace().len()),
            after
                .completions()
                .len()
                .saturating_sub(before.completions().len()),
            after.pending_dma_writes().len(),
            after
                .dma_completions()
                .len()
                .saturating_sub(before.dma_completions().len()),
            after_memory_access_count.saturating_sub(before_memory_access_count),
            after
                .coalesced_memory_accesses()
                .len()
                .saturating_sub(before.coalesced_memory_accesses().len()),
            queue_waits.waited_workgroups,
            queue_waits.wait_ticks,
            queue_waits.max_wait_ticks,
            queue_waits.compute_units,
        ))
    }

    fn schedule_queued_slots_from_scheduler(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<(), GpuError> {
        let slot_indices = {
            let mut state = self.state.lock().expect("GPU state lock");
            state
                .slots
                .iter_mut()
                .enumerate()
                .filter_map(|(index, slot)| {
                    if slot.queued.is_empty() {
                        return None;
                    }
                    slot.pump_scheduled = false;
                    Some(index)
                })
                .collect::<Vec<_>>()
        };

        for slot_index in slot_indices {
            self.schedule_slot_from_scheduler(scheduler, slot_index)?;
        }

        Ok(())
    }

    fn schedule_slot_from_scheduler(
        &self,
        scheduler: &mut PartitionedScheduler,
        slot_index: usize,
    ) -> Result<(), GpuError> {
        let partition = self.partition();
        let now = scheduler
            .partition_now(partition)
            .map_err(GpuError::Scheduler)?;
        let Some(delay) = self.reserve_slot_pump(now, slot_index) else {
            return Ok(());
        };
        let tick = now
            .checked_add(delay)
            .ok_or(GpuError::TickOverflow { now, delay })?;
        let gpu = self.clone();
        scheduler
            .schedule_parallel_at(partition, tick, move |context| {
                gpu.start_next_workgroup(context, slot_index);
            })
            .map_err(GpuError::Scheduler)?;

        Ok(())
    }

    pub fn submit_dma_copy_read<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        copy: GpuDmaCopy,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, GpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let prepared = self.prepare_dma_copy_read(scheduler.now(), copy, trace, responder);
        let (issue, transaction) = prepared.into_parts();
        let event = transport
            .submit_parallel_batch(scheduler, [transaction])
            .map_err(GpuError::Transport)?
            .into_iter()
            .next()
            .expect("one GPU DMA read transaction was submitted");
        issue.record();
        Ok(event)
    }

    pub fn prepare_dma_copy_read<F>(
        &self,
        issued_at: Tick,
        copy: GpuDmaCopy,
        trace: MemoryTrace,
        responder: F,
    ) -> GpuPreparedDmaRead
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let transfer = copy.transfer();
        let request = copy.read_request().id();
        let read_request = copy.read_request().clone();
        let sink_gpu = self.clone();
        let sink_copy = copy.clone();
        let transaction = ParallelMemoryTransaction::new(
            copy.read_route(),
            read_request,
            trace,
            responder,
            move |delivery| sink_gpu.accept_dma_read_response(sink_copy, delivery),
        );
        GpuPreparedDmaRead {
            issue: GpuDmaIssueRecord {
                gpu: self.clone(),
                event: GpuTraceEvent::new(
                    issued_at,
                    GpuTraceKind::DmaReadIssued { transfer, request },
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
    ) -> Result<Option<PartitionEventId>, GpuError>
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
                .expect("one GPU DMA write transaction was submitted"),
            Err(error) => {
                rollback.restore();
                return Err(GpuError::Transport(error));
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
    ) -> Result<Option<GpuPreparedDmaWrite>, GpuError>
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
        let transfer = pending.copy.transfer();
        let request = write_request.id();
        let sink_gpu = self.clone();
        let completion = GpuDmaCompletion::new(
            transfer,
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
            move |delivery| sink_gpu.accept_dma_write_response(completion, delivery),
        );
        Ok(Some(GpuPreparedDmaWrite {
            issue: GpuDmaIssueRecord {
                gpu: self.clone(),
                event: GpuTraceEvent::new(
                    issued_at,
                    GpuTraceKind::DmaWriteIssued { transfer, request },
                ),
            },
            transaction,
            rollback: GpuDmaWriteRollback {
                gpu: self.clone(),
                pending: rollback_pending,
            },
        }))
    }

    fn accept_launch(&self, context: &mut ParallelSchedulerContext<'_>, launch: GpuKernelLaunch) {
        self.record(GpuTraceEvent::new(
            context.now(),
            GpuTraceKind::LaunchAccepted {
                kernel: launch.kernel(),
                workgroups: launch.workgroups(),
            },
        ));

        let touched_slots = self.enqueue_launch(context.now(), &launch);
        for slot_index in touched_slots {
            self.schedule_slot_if_needed(context, slot_index);
        }
    }

    fn enqueue_launch(&self, now: Tick, launch: &GpuKernelLaunch) -> Vec<usize> {
        let mut state = self.state.lock().expect("GPU state lock");
        let mut touched_slots = Vec::new();
        for workgroup in 0..launch.workgroups() {
            let slot_index = state.next_slot_index();
            let compute_unit =
                compute_unit_for_slot(slot_index, self.wave_slots_per_compute_unit());
            let wave_slot = wave_slot_for_slot(slot_index, self.wave_slots_per_compute_unit());
            let queued = {
                let slot = &mut state.slots[slot_index];
                let started_at = now.max(slot.available_at);
                let completed_at = started_at
                    .checked_add(launch.workgroup_latency())
                    .expect("validated GPU workgroup latency fits");
                slot.available_at = completed_at;
                GpuQueuedWorkgroup {
                    kernel: launch.kernel(),
                    workgroup: GpuWorkgroupId::new(workgroup),
                    compute_unit,
                    slot: wave_slot,
                    queued_at: now,
                    started_at,
                    completed_at,
                    isa_program: launch.isa_program().clone(),
                }
            };
            if queued.started_at > queued.queued_at {
                state.wait_log.push(queued.clone());
            }
            state.slots[slot_index].queued.push_back(queued);
            touched_slots.push(slot_index);
        }
        touched_slots.sort_unstable();
        touched_slots.dedup();
        touched_slots
    }

    fn schedule_slot_if_needed(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        slot_index: usize,
    ) {
        let Some(delay) = self.reserve_slot_pump(context.now(), slot_index) else {
            return;
        };
        let gpu = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                gpu.start_next_workgroup(context, slot_index);
            })
            .expect("GPU slot pump tick was reserved");
    }

    fn reserve_slot_pump(&self, now: Tick, slot_index: usize) -> Option<Tick> {
        let mut state = self.state.lock().expect("GPU state lock");
        let slot = &mut state.slots[slot_index];
        if slot.pump_scheduled {
            return None;
        }
        let workgroup = slot.queued.front()?;
        slot.pump_scheduled = true;
        Some(workgroup.started_at.saturating_sub(now))
    }

    fn start_next_workgroup(&self, context: &mut ParallelSchedulerContext<'_>, slot_index: usize) {
        let Some(workgroup) = self.pop_ready_slot_workgroup(slot_index, context.now()) else {
            return;
        };
        self.record(GpuTraceEvent::new(
            context.now(),
            GpuTraceKind::WorkgroupStarted {
                kernel: workgroup.kernel,
                workgroup: workgroup.workgroup,
                compute_unit: workgroup.compute_unit,
                slot: workgroup.slot,
                complete_at: workgroup.completed_at,
            },
        ));

        let delay = workgroup
            .completed_at
            .checked_sub(context.now())
            .expect("GPU workgroup completion is not before start");
        let gpu = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                gpu.complete_workgroup(context, slot_index, workgroup);
            })
            .expect("GPU workgroup completion tick was reserved");
    }

    fn pop_ready_slot_workgroup(&self, slot_index: usize, now: Tick) -> Option<GpuQueuedWorkgroup> {
        let mut state = self.state.lock().expect("GPU state lock");
        let slot = &mut state.slots[slot_index];
        slot.pump_scheduled = false;
        let workgroup = slot.queued.front()?;
        if workgroup.started_at > now {
            return None;
        }
        slot.queued.pop_front()
    }

    fn complete_workgroup(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        slot_index: usize,
        workgroup: GpuQueuedWorkgroup,
    ) {
        let execution = workgroup.isa_program.execute(workgroup.workgroup);
        let completion = GpuWorkgroupCompletion::new(
            workgroup.kernel,
            workgroup.workgroup,
            workgroup.compute_unit,
            workgroup.slot,
            workgroup.started_at,
            context.now(),
        )
        .with_isa_state(execution.isa_state().clone());
        let coalesced_memory_accesses = execution
            .coalesced_memory_accesses()
            .iter()
            .map(|access| {
                let access_context = GpuCoalescedMemoryAccessContext::new(
                    workgroup.kernel,
                    workgroup.workgroup,
                    workgroup.compute_unit,
                    workgroup.slot,
                    context.now(),
                );
                GpuCoalescedMemoryAccess::new(
                    access_context,
                    access.instruction_index(),
                    access.kind(),
                    access.line(),
                    access.access_count(),
                    access.byte_count(),
                )
            })
            .collect::<Vec<_>>();
        let mut state = self.state.lock().expect("GPU state lock");
        state.trace.push(GpuTraceEvent::new(
            context.now(),
            GpuTraceKind::WorkgroupCompleted {
                kernel: workgroup.kernel,
                workgroup: workgroup.workgroup,
                compute_unit: workgroup.compute_unit,
                slot: workgroup.slot,
            },
        ));
        state.completions.push(completion);
        state
            .coalesced_memory_accesses
            .extend(coalesced_memory_accesses);
        drop(state);

        self.schedule_slot_if_needed(context, slot_index);
    }

    fn record(&self, event: GpuTraceEvent) {
        self.state.lock().expect("GPU state lock").trace.push(event);
    }

    fn accept_dma_read_response(&self, copy: GpuDmaCopy, delivery: ResponseDelivery) {
        if let Some(data) = response_data(delivery.response()) {
            let bytes = data.len() as u64;
            let mut state = self.state.lock().expect("GPU state lock");
            state.trace.push(GpuTraceEvent::new(
                delivery.tick(),
                GpuTraceKind::DmaReadCompleted {
                    transfer: copy.transfer(),
                    request: copy.read_request().id(),
                    bytes,
                },
            ));
            state
                .pending_dma_writes
                .push(GpuPendingDmaWrite::new(copy, data, delivery.tick()));
        }
    }

    fn accept_dma_write_response(
        &self,
        mut completion: GpuDmaCompletion,
        delivery: ResponseDelivery,
    ) {
        completion.write_completed_at = delivery.tick();
        let mut state = self.state.lock().expect("GPU state lock");
        state.trace.push(GpuTraceEvent::new(
            delivery.tick(),
            GpuTraceKind::DmaWriteCompleted {
                transfer: completion.transfer(),
                request: completion.write_request(),
            },
        ));
        state.dma_completions.push(completion);
    }

    fn pop_pending_dma_write(&self) -> Option<GpuPendingDmaWrite> {
        let mut state = self.state.lock().expect("GPU state lock");
        if state.pending_dma_writes.is_empty() {
            None
        } else {
            Some(state.pending_dma_writes.remove(0))
        }
    }

    fn push_pending_dma_write(&self, pending: GpuPendingDmaWrite) {
        self.state
            .lock()
            .expect("GPU state lock")
            .pending_dma_writes
            .insert(0, pending);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GpuDeviceState {
    slots: Vec<GpuSlotState>,
    trace: Vec<GpuTraceEvent>,
    completions: Vec<GpuWorkgroupCompletion>,
    pending_dma_writes: Vec<GpuPendingDmaWrite>,
    dma_completions: Vec<GpuDmaCompletion>,
    coalesced_memory_accesses: Vec<GpuCoalescedMemoryAccess>,
    wait_log: Vec<GpuQueuedWorkgroup>,
}

impl GpuDeviceState {
    fn new(config: &GpuComputeConfig) -> Self {
        Self {
            slots: vec![GpuSlotState::new(); config.slot_count()],
            trace: Vec::new(),
            completions: Vec::new(),
            pending_dma_writes: Vec::new(),
            dma_completions: Vec::new(),
            coalesced_memory_accesses: Vec::new(),
            wait_log: Vec::new(),
        }
    }

    fn snapshot(&self) -> GpuDeviceSnapshot {
        let mut queued_isa_programs = Vec::new();
        let slots = self
            .slots
            .iter()
            .enumerate()
            .map(|(slot_index, slot)| {
                for (queue_index, workgroup) in slot.queued.iter().enumerate() {
                    queued_isa_programs.push(GpuQueuedIsaProgramSnapshot::new(
                        slot_index,
                        queue_index,
                        workgroup.isa_program.clone(),
                    ));
                }
                slot.snapshot()
            })
            .collect();

        GpuDeviceSnapshot::new(
            slots,
            self.trace.clone(),
            self.completions.clone(),
            self.pending_dma_writes.clone(),
            self.dma_completions.clone(),
        )
        .with_queued_isa_programs(queued_isa_programs)
        .with_coalesced_memory_accesses(self.coalesced_memory_accesses.clone())
    }

    fn from_snapshot(snapshot: &GpuDeviceSnapshot) -> Self {
        let mut state = Self {
            slots: snapshot
                .slots()
                .iter()
                .map(GpuSlotState::from_snapshot)
                .collect(),
            trace: snapshot.trace().to_vec(),
            completions: snapshot.completions().to_vec(),
            pending_dma_writes: snapshot.pending_dma_writes().to_vec(),
            dma_completions: snapshot.dma_completions().to_vec(),
            coalesced_memory_accesses: snapshot.coalesced_memory_accesses().to_vec(),
            wait_log: Vec::new(),
        };
        for program in snapshot.queued_isa_programs() {
            let Some(slot) = state.slots.get_mut(program.slot_index()) else {
                continue;
            };
            let Some(workgroup) = slot.queued.get_mut(program.queue_index()) else {
                continue;
            };
            workgroup.isa_program = program.isa_program().clone();
        }
        state
    }

    fn mark_wait_for(&self) -> GpuWaitForMarker {
        GpuWaitForMarker::new(self.wait_log.len())
    }

    fn wait_for_graph(&self, device: GpuDeviceId) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        for slot in &self.slots {
            for workgroup in &slot.queued {
                if workgroup.started_at <= workgroup.queued_at {
                    continue;
                }
                record_gpu_wait_interval(&mut graph, device, workgroup, workgroup.queued_at);
            }
        }
        graph
    }

    fn wait_for_graph_since(&self, device: GpuDeviceId, marker: GpuWaitForMarker) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        let Some(records) = self.wait_log.get(marker.offset()..) else {
            return graph;
        };
        for workgroup in records {
            record_gpu_wait_interval(
                &mut graph,
                device,
                workgroup,
                workgroup.started_at.saturating_sub(1),
            );
        }
        graph
    }

    fn next_slot_index(&self) -> usize {
        self.slots
            .iter()
            .enumerate()
            .min_by_key(|(index, slot)| (slot.available_at, *index))
            .map(|(index, _slot)| index)
            .expect("GPU has at least one execution slot")
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct GpuQueueWaitSummary {
    waited_workgroups: u64,
    wait_ticks: Tick,
    max_wait_ticks: Tick,
    compute_units: Vec<GpuComputeUnitQueueWaitSummary>,
}

fn gpu_queue_wait_summary(records: &[GpuQueuedWorkgroup]) -> GpuQueueWaitSummary {
    let mut summary = GpuQueueWaitSummary::default();
    let mut per_compute_unit: BTreeMap<u32, (u64, Tick, Tick)> = BTreeMap::new();
    for record in records {
        let delay = record.started_at.saturating_sub(record.queued_at);
        if delay == 0 {
            continue;
        }
        summary.waited_workgroups += 1;
        summary.wait_ticks += delay;
        summary.max_wait_ticks = summary.max_wait_ticks.max(delay);

        let entry = per_compute_unit
            .entry(record.compute_unit)
            .or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += delay;
        entry.2 = entry.2.max(delay);
    }
    summary.compute_units = per_compute_unit
        .into_iter()
        .map(
            |(compute_unit, (waited_workgroups, wait_ticks, max_wait_ticks))| {
                GpuComputeUnitQueueWaitSummary::new(
                    compute_unit,
                    waited_workgroups,
                    wait_ticks,
                    max_wait_ticks,
                )
            },
        )
        .collect();
    summary
}

fn snapshot_queued_workgroups(snapshot: &GpuDeviceSnapshot) -> Vec<GpuQueuedWorkgroup> {
    snapshot
        .slots()
        .iter()
        .flat_map(|slot| slot.queued().iter().map(GpuQueuedWorkgroup::from_snapshot))
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GpuSlotState {
    available_at: Tick,
    pump_scheduled: bool,
    queued: VecDeque<GpuQueuedWorkgroup>,
}

impl GpuSlotState {
    fn new() -> Self {
        Self {
            available_at: 0,
            pump_scheduled: false,
            queued: VecDeque::new(),
        }
    }

    fn snapshot(&self) -> GpuSlotSnapshot {
        GpuSlotSnapshot::new(
            self.available_at,
            self.pump_scheduled,
            self.queued
                .iter()
                .map(GpuQueuedWorkgroup::snapshot)
                .collect(),
        )
    }

    fn from_snapshot(snapshot: &GpuSlotSnapshot) -> Self {
        Self {
            available_at: snapshot.available_at(),
            pump_scheduled: snapshot.pump_scheduled(),
            queued: snapshot
                .queued()
                .iter()
                .map(GpuQueuedWorkgroup::from_snapshot)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GpuQueuedWorkgroup {
    kernel: GpuKernelId,
    workgroup: GpuWorkgroupId,
    compute_unit: u32,
    slot: u32,
    queued_at: Tick,
    started_at: Tick,
    completed_at: Tick,
    isa_program: GpuIsaProgram,
}

impl GpuQueuedWorkgroup {
    fn snapshot(&self) -> GpuQueuedWorkgroupSnapshot {
        GpuQueuedWorkgroupSnapshot::new(
            self.kernel,
            self.workgroup,
            self.compute_unit,
            self.slot,
            self.queued_at,
            self.started_at,
            self.completed_at,
        )
    }

    fn from_snapshot(snapshot: &GpuQueuedWorkgroupSnapshot) -> Self {
        Self {
            kernel: snapshot.kernel(),
            workgroup: snapshot.workgroup(),
            compute_unit: snapshot.compute_unit(),
            slot: snapshot.slot(),
            queued_at: snapshot.queued_at(),
            started_at: snapshot.started_at(),
            completed_at: snapshot.completed_at(),
            isa_program: GpuIsaProgram::empty(),
        }
    }
}

fn record_gpu_wait_interval(
    graph: &mut WaitForGraph,
    device: GpuDeviceId,
    workgroup: &GpuQueuedWorkgroup,
    last_tick: Tick,
) {
    let source = gpu_workgroup_node(device, workgroup.kernel, workgroup.workgroup);
    let target = gpu_slot_node(device, workgroup.compute_unit, workgroup.slot);
    graph
        .record_wait(
            source.clone(),
            target.clone(),
            WaitForEdgeKind::Queue,
            workgroup.queued_at,
        )
        .expect("GPU wait-for labels are generated from typed ids");
    if last_tick != workgroup.queued_at {
        graph
            .record_wait(source, target, WaitForEdgeKind::Queue, last_tick)
            .expect("GPU wait-for labels are generated from typed ids");
    }
}

fn gpu_workgroup_node(
    device: GpuDeviceId,
    kernel: GpuKernelId,
    workgroup: GpuWorkgroupId,
) -> WaitForNode {
    WaitForNode::transaction(format!(
        "gpu.{}.kernel.{}.wg.{}",
        device.get(),
        kernel.get(),
        workgroup.get()
    ))
    .expect("GPU workgroup wait-for label is generated from numeric ids")
}

fn gpu_slot_node(device: GpuDeviceId, compute_unit: u32, slot: u32) -> WaitForNode {
    WaitForNode::resource(format!(
        "gpu.{}.cu.{}.slot.{}",
        device.get(),
        compute_unit,
        slot
    ))
    .expect("GPU slot wait-for label is generated from numeric ids")
}

fn compute_unit_for_slot(slot_index: usize, slots_per_compute_unit: u32) -> u32 {
    (slot_index / slots_per_compute_unit as usize) as u32
}

fn wave_slot_for_slot(slot_index: usize, slots_per_compute_unit: u32) -> u32 {
    (slot_index % slots_per_compute_unit as usize) as u32
}

fn response_data(response: &MemoryResponse) -> Option<Vec<u8>> {
    response.data().map(<[u8]>::to_vec)
}

fn snapshot_memory_access_count(snapshot: &GpuDeviceSnapshot) -> usize {
    snapshot
        .coalesced_memory_accesses()
        .iter()
        .map(|access| access.access_count() as usize)
        .sum()
}

fn validate_submission_latency(
    source: PartitionId,
    target: PartitionId,
    source_tick: Tick,
    min_remote_delay: Tick,
    delay: Tick,
) -> Result<(), GpuError> {
    if source != target && delay == 0 {
        return Err(GpuError::Scheduler(
            SchedulerError::ZeroDelayRemoteMessage { source, target },
        ));
    }
    let delivery_tick = source_tick
        .checked_add(delay)
        .ok_or(GpuError::TickOverflow {
            now: source_tick,
            delay,
        })?;

    if source != target {
        let minimum_delivery_tick =
            source_tick
                .checked_add(min_remote_delay)
                .ok_or(GpuError::TickOverflow {
                    now: source_tick,
                    delay: min_remote_delay,
                })?;
        if delivery_tick < minimum_delivery_tick {
            return Err(GpuError::Scheduler(
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
