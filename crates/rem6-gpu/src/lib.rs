use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{
    ConservativeRunSummary, ParallelEpochBatchRecord, ParallelRunProfile, ParallelSchedulerContext,
    PartitionEventId, PartitionId, PartitionedScheduler, RecordedConservativeRunSummary,
    RecordedRunSummary, SchedulerDispatchRecord, SchedulerError, Tick,
};
use rem6_memory::{Address, ByteMask, MemoryError, MemoryRequest, MemoryRequestId, MemoryResponse};
use rem6_topology::{Endpoint, TopologyError};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    ResponseDelivery, TargetOutcome, TopologyRouteError, TransportError,
};

mod topology;

pub use topology::{GpuCommandPath, GpuTopologyConfig, GpuTopologyDevice};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuDeviceId(u32);

impl GpuDeviceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuKernelId(u64);

impl GpuKernelId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuWorkgroupId(u32);

impl GpuWorkgroupId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuDmaId(u64);

impl GpuDmaId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuComputeConfig {
    device: GpuDeviceId,
    partition: PartitionId,
    compute_units: u32,
    wave_slots_per_compute_unit: u32,
}

impl GpuComputeConfig {
    pub fn new(
        device: GpuDeviceId,
        partition: PartitionId,
        compute_units: u32,
        wave_slots_per_compute_unit: u32,
    ) -> Result<Self, GpuError> {
        if compute_units == 0 {
            return Err(GpuError::ZeroComputeUnits { device });
        }
        if wave_slots_per_compute_unit == 0 {
            return Err(GpuError::ZeroWaveSlots { device });
        }

        Ok(Self {
            device,
            partition,
            compute_units,
            wave_slots_per_compute_unit,
        })
    }

    pub const fn device(&self) -> GpuDeviceId {
        self.device
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn compute_units(&self) -> u32 {
        self.compute_units
    }

    pub const fn wave_slots_per_compute_unit(&self) -> u32 {
        self.wave_slots_per_compute_unit
    }

    fn slot_count(&self) -> usize {
        (self.compute_units as usize)
            .checked_mul(self.wave_slots_per_compute_unit as usize)
            .expect("GPU slot count fits usize")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuKernelLaunch {
    kernel: GpuKernelId,
    workgroups: u32,
    workgroup_latency: Tick,
}

impl GpuKernelLaunch {
    pub fn new(
        kernel: GpuKernelId,
        workgroups: u32,
        workgroup_latency: Tick,
    ) -> Result<Self, GpuError> {
        if workgroups == 0 {
            return Err(GpuError::ZeroWorkgroups { kernel });
        }
        if workgroup_latency == 0 {
            return Err(GpuError::ZeroWorkgroupLatency { kernel });
        }

        Ok(Self {
            kernel,
            workgroups,
            workgroup_latency,
        })
    }

    pub const fn kernel(&self) -> GpuKernelId {
        self.kernel
    }

    pub const fn workgroups(&self) -> u32 {
        self.workgroups
    }

    pub const fn workgroup_latency(&self) -> Tick {
        self.workgroup_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuError {
    ZeroComputeUnits {
        device: GpuDeviceId,
    },
    ZeroWaveSlots {
        device: GpuDeviceId,
    },
    ZeroWorkgroups {
        kernel: GpuKernelId,
    },
    ZeroWorkgroupLatency {
        kernel: GpuKernelId,
    },
    DmaReadRequiresData {
        transfer: GpuDmaId,
        request: MemoryRequestId,
    },
    CommandTargetPartitionMismatch {
        endpoint: Endpoint,
        expected: PartitionId,
        actual: PartitionId,
    },
    MemorySourcePartitionMismatch {
        endpoint: Endpoint,
        expected: PartitionId,
        actual: PartitionId,
    },
    TickOverflow {
        now: Tick,
        delay: Tick,
    },
    Scheduler(SchedulerError),
    Memory(MemoryError),
    Topology(TopologyError),
    TopologyRoute(TopologyRouteError),
    Transport(TransportError),
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroComputeUnits { device } => {
                write!(
                    formatter,
                    "GPU device {} needs at least one compute unit",
                    device.get()
                )
            }
            Self::ZeroWaveSlots { device } => write!(
                formatter,
                "GPU device {} needs at least one wave slot per compute unit",
                device.get()
            ),
            Self::ZeroWorkgroups { kernel } => write!(
                formatter,
                "GPU kernel {} needs at least one workgroup",
                kernel.get()
            ),
            Self::ZeroWorkgroupLatency { kernel } => write!(
                formatter,
                "GPU kernel {} needs positive workgroup latency",
                kernel.get()
            ),
            Self::DmaReadRequiresData { transfer, request } => write!(
                formatter,
                "GPU DMA transfer {} read request {} from agent {} must return data",
                transfer.get(),
                request.sequence(),
                request.agent().get(),
            ),
            Self::CommandTargetPartitionMismatch {
                endpoint,
                expected,
                actual,
            } => write!(
                formatter,
                "command endpoint {}.{} is on partition {} but GPU partition is {}",
                endpoint.component().as_str(),
                endpoint.port().as_str(),
                actual.index(),
                expected.index()
            ),
            Self::MemorySourcePartitionMismatch {
                endpoint,
                expected,
                actual,
            } => write!(
                formatter,
                "memory endpoint {}.{} is on partition {} but GPU partition is {}",
                endpoint.component().as_str(),
                endpoint.port().as_str(),
                actual.index(),
                expected.index()
            ),
            Self::TickOverflow { now, delay } => {
                write!(formatter, "tick {now} overflows when adding delay {delay}")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::TopologyRoute(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for GpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Topology(error) => Some(error),
            Self::TopologyRoute(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuTraceEvent {
    tick: Tick,
    kind: GpuTraceKind,
}

impl GpuTraceEvent {
    pub const fn new(tick: Tick, kind: GpuTraceKind) -> Self {
        Self { tick, kind }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn kind(&self) -> &GpuTraceKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuTraceKind {
    LaunchSubmitted {
        kernel: GpuKernelId,
        source: PartitionId,
        target: PartitionId,
    },
    LaunchAccepted {
        kernel: GpuKernelId,
        workgroups: u32,
    },
    WorkgroupStarted {
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
        complete_at: Tick,
    },
    WorkgroupCompleted {
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
    },
    DmaReadIssued {
        transfer: GpuDmaId,
        request: MemoryRequestId,
    },
    DmaReadCompleted {
        transfer: GpuDmaId,
        request: MemoryRequestId,
        bytes: u64,
    },
    DmaWriteIssued {
        transfer: GpuDmaId,
        request: MemoryRequestId,
    },
    DmaWriteCompleted {
        transfer: GpuDmaId,
        request: MemoryRequestId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuWorkgroupCompletion {
    kernel: GpuKernelId,
    workgroup: GpuWorkgroupId,
    compute_unit: u32,
    slot: u32,
    started_at: Tick,
    completed_at: Tick,
}

impl GpuWorkgroupCompletion {
    pub const fn new(
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
        started_at: Tick,
        completed_at: Tick,
    ) -> Self {
        Self {
            kernel,
            workgroup,
            compute_unit,
            slot,
            started_at,
            completed_at,
        }
    }

    pub const fn kernel(&self) -> GpuKernelId {
        self.kernel
    }

    pub const fn workgroup(&self) -> GpuWorkgroupId {
        self.workgroup
    }

    pub const fn compute_unit(&self) -> u32 {
        self.compute_unit
    }

    pub const fn slot(&self) -> u32 {
        self.slot
    }

    pub const fn started_at(&self) -> Tick {
        self.started_at
    }

    pub const fn completed_at(&self) -> Tick {
        self.completed_at
    }
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuDeviceSnapshot {
    slots: Vec<GpuSlotSnapshot>,
    trace: Vec<GpuTraceEvent>,
    completions: Vec<GpuWorkgroupCompletion>,
    pending_dma_writes: Vec<GpuPendingDmaWrite>,
    dma_completions: Vec<GpuDmaCompletion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelGpuRunSummary {
    scheduler_run: RecordedConservativeRunSummary,
    trace_event_count: usize,
    workgroup_completion_count: usize,
    pending_dma_write_count: usize,
    dma_completion_count: usize,
}

impl ParallelGpuRunSummary {
    pub const fn new(
        scheduler_run: RecordedConservativeRunSummary,
        trace_event_count: usize,
        workgroup_completion_count: usize,
        pending_dma_write_count: usize,
        dma_completion_count: usize,
    ) -> Self {
        Self {
            scheduler_run,
            trace_event_count,
            workgroup_completion_count,
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

    pub const fn workgroup_completion_count(&self) -> usize {
        self.workgroup_completion_count
    }

    pub const fn pending_dma_write_count(&self) -> usize {
        self.pending_dma_write_count
    }

    pub const fn dma_completion_count(&self) -> usize {
        self.dma_completion_count
    }

    pub const fn device_activity_count(&self) -> usize {
        self.trace_event_count
            + self.workgroup_completion_count
            + self.pending_dma_write_count
            + self.dma_completion_count
    }

    pub const fn has_device_activity(&self) -> bool {
        self.device_activity_count() != 0
    }

    pub const fn has_compute_activity(&self) -> bool {
        self.workgroup_completion_count != 0
    }

    pub const fn has_dma_activity(&self) -> bool {
        self.pending_dma_write_count != 0 || self.dma_completion_count != 0
    }
}

impl GpuDeviceSnapshot {
    pub fn new(
        slots: Vec<GpuSlotSnapshot>,
        trace: Vec<GpuTraceEvent>,
        completions: Vec<GpuWorkgroupCompletion>,
        pending_dma_writes: Vec<GpuPendingDmaWrite>,
        dma_completions: Vec<GpuDmaCompletion>,
    ) -> Self {
        Self {
            slots,
            trace,
            completions,
            pending_dma_writes,
            dma_completions,
        }
    }

    pub fn slots(&self) -> &[GpuSlotSnapshot] {
        &self.slots
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn has_queued_workgroups(&self) -> bool {
        self.slots.iter().any(|slot| !slot.queued().is_empty())
    }

    pub fn trace(&self) -> &[GpuTraceEvent] {
        &self.trace
    }

    pub fn completions(&self) -> &[GpuWorkgroupCompletion] {
        &self.completions
    }

    pub fn pending_dma_writes(&self) -> &[GpuPendingDmaWrite] {
        &self.pending_dma_writes
    }

    pub fn has_pending_dma_writes(&self) -> bool {
        !self.pending_dma_writes.is_empty()
    }

    pub fn dma_completions(&self) -> &[GpuDmaCompletion] {
        &self.dma_completions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuSlotSnapshot {
    available_at: Tick,
    pump_scheduled: bool,
    queued: Vec<GpuQueuedWorkgroupSnapshot>,
}

impl GpuSlotSnapshot {
    pub fn new(
        available_at: Tick,
        pump_scheduled: bool,
        queued: Vec<GpuQueuedWorkgroupSnapshot>,
    ) -> Self {
        Self {
            available_at,
            pump_scheduled,
            queued,
        }
    }

    pub const fn available_at(&self) -> Tick {
        self.available_at
    }

    pub const fn pump_scheduled(&self) -> bool {
        self.pump_scheduled
    }

    pub fn queued(&self) -> &[GpuQueuedWorkgroupSnapshot] {
        &self.queued
    }

    pub fn is_idle(&self) -> bool {
        !self.pump_scheduled && self.queued.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuQueuedWorkgroupSnapshot {
    kernel: GpuKernelId,
    workgroup: GpuWorkgroupId,
    compute_unit: u32,
    slot: u32,
    started_at: Tick,
    completed_at: Tick,
}

impl GpuQueuedWorkgroupSnapshot {
    pub const fn new(
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
        started_at: Tick,
        completed_at: Tick,
    ) -> Self {
        Self {
            kernel,
            workgroup,
            compute_unit,
            slot,
            started_at,
            completed_at,
        }
    }

    pub const fn kernel(self) -> GpuKernelId {
        self.kernel
    }

    pub const fn workgroup(self) -> GpuWorkgroupId {
        self.workgroup
    }

    pub const fn compute_unit(self) -> u32 {
        self.compute_unit
    }

    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn started_at(self) -> Tick {
        self.started_at
    }

    pub const fn completed_at(self) -> Tick {
        self.completed_at
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
        scheduler
            .partition_now(source)
            .map_err(GpuError::Scheduler)?;
        scheduler
            .partition_now(target)
            .map_err(GpuError::Scheduler)?;
        validate_submission_latency(scheduler, source, target, submission_latency)?;

        let source_tick = scheduler.now();
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

    pub fn restore(&self, snapshot: &GpuDeviceSnapshot) {
        *self.state.lock().expect("GPU state lock") = GpuDeviceState::from_snapshot(snapshot);
    }

    pub fn run_until_idle_parallel_recorded(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<ParallelGpuRunSummary, GpuError> {
        let before = self.snapshot();
        let scheduler_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(GpuError::Scheduler)?;
        let after = self.snapshot();

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
        ))
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
            let slot = &mut state.slots[slot_index];
            let started_at = now.max(slot.available_at);
            let completed_at = started_at
                .checked_add(launch.workgroup_latency())
                .expect("validated GPU workgroup latency fits");
            slot.available_at = completed_at;
            slot.queued.push_back(GpuQueuedWorkgroup {
                kernel: launch.kernel(),
                workgroup: GpuWorkgroupId::new(workgroup),
                compute_unit: compute_unit_for_slot(slot_index, self.wave_slots_per_compute_unit()),
                slot: wave_slot_for_slot(slot_index, self.wave_slots_per_compute_unit()),
                started_at,
                completed_at,
            });
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
        let Some(workgroup) = self.pop_slot_workgroup(slot_index) else {
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

    fn pop_slot_workgroup(&self, slot_index: usize) -> Option<GpuQueuedWorkgroup> {
        let mut state = self.state.lock().expect("GPU state lock");
        let slot = &mut state.slots[slot_index];
        slot.pump_scheduled = false;
        slot.queued.pop_front()
    }

    fn complete_workgroup(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        slot_index: usize,
        workgroup: GpuQueuedWorkgroup,
    ) {
        let completion = GpuWorkgroupCompletion::new(
            workgroup.kernel,
            workgroup.workgroup,
            workgroup.compute_unit,
            workgroup.slot,
            workgroup.started_at,
            context.now(),
        );
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
}

impl GpuDeviceState {
    fn new(config: &GpuComputeConfig) -> Self {
        Self {
            slots: vec![GpuSlotState::new(); config.slot_count()],
            trace: Vec::new(),
            completions: Vec::new(),
            pending_dma_writes: Vec::new(),
            dma_completions: Vec::new(),
        }
    }

    fn snapshot(&self) -> GpuDeviceSnapshot {
        GpuDeviceSnapshot::new(
            self.slots.iter().map(GpuSlotState::snapshot).collect(),
            self.trace.clone(),
            self.completions.clone(),
            self.pending_dma_writes.clone(),
            self.dma_completions.clone(),
        )
    }

    fn from_snapshot(snapshot: &GpuDeviceSnapshot) -> Self {
        Self {
            slots: snapshot
                .slots
                .iter()
                .map(GpuSlotState::from_snapshot)
                .collect(),
            trace: snapshot.trace.clone(),
            completions: snapshot.completions.clone(),
            pending_dma_writes: snapshot.pending_dma_writes.clone(),
            dma_completions: snapshot.dma_completions.clone(),
        }
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
            available_at: snapshot.available_at,
            pump_scheduled: snapshot.pump_scheduled,
            queued: snapshot
                .queued
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
    started_at: Tick,
    completed_at: Tick,
}

impl GpuQueuedWorkgroup {
    fn snapshot(&self) -> GpuQueuedWorkgroupSnapshot {
        GpuQueuedWorkgroupSnapshot::new(
            self.kernel,
            self.workgroup,
            self.compute_unit,
            self.slot,
            self.started_at,
            self.completed_at,
        )
    }

    fn from_snapshot(snapshot: &GpuQueuedWorkgroupSnapshot) -> Self {
        Self {
            kernel: snapshot.kernel,
            workgroup: snapshot.workgroup,
            compute_unit: snapshot.compute_unit,
            slot: snapshot.slot,
            started_at: snapshot.started_at,
            completed_at: snapshot.completed_at,
        }
    }
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

fn validate_submission_latency(
    scheduler: &PartitionedScheduler,
    source: PartitionId,
    target: PartitionId,
    delay: Tick,
) -> Result<(), GpuError> {
    if source != target && delay == 0 {
        return Err(GpuError::Scheduler(
            SchedulerError::ZeroDelayRemoteMessage { source, target },
        ));
    }
    if source != target && delay < scheduler.min_remote_delay() {
        return Err(GpuError::Scheduler(
            SchedulerError::RemoteDelayBelowLookahead {
                source,
                target,
                delay,
                minimum: scheduler.min_remote_delay(),
            },
        ));
    }
    scheduler
        .now()
        .checked_add(delay)
        .ok_or(GpuError::TickOverflow {
            now: scheduler.now(),
            delay,
        })?;

    Ok(())
}
