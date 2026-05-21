use std::collections::VecDeque;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError,
    Tick,
};

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
    ZeroComputeUnits { device: GpuDeviceId },
    ZeroWaveSlots { device: GpuDeviceId },
    ZeroWorkgroups { kernel: GpuKernelId },
    ZeroWorkgroupLatency { kernel: GpuKernelId },
    TickOverflow { now: Tick, delay: Tick },
    Scheduler(SchedulerError),
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
            Self::TickOverflow { now, delay } => {
                write!(formatter, "tick {now} overflows when adding delay {delay}")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for GpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
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

#[derive(Clone)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GpuDeviceState {
    slots: Vec<GpuSlotState>,
    trace: Vec<GpuTraceEvent>,
    completions: Vec<GpuWorkgroupCompletion>,
}

impl GpuDeviceState {
    fn new(config: &GpuComputeConfig) -> Self {
        Self {
            slots: vec![GpuSlotState::new(); config.slot_count()],
            trace: Vec::new(),
            completions: Vec::new(),
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

fn compute_unit_for_slot(slot_index: usize, slots_per_compute_unit: u32) -> u32 {
    (slot_index / slots_per_compute_unit as usize) as u32
}

fn wave_slot_for_slot(slot_index: usize, slots_per_compute_unit: u32) -> u32 {
    (slot_index % slots_per_compute_unit as usize) as u32
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
