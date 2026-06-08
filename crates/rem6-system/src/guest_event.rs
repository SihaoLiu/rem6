use rem6_checkpoint::CheckpointManifest;
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, SchedulerContext, Tick,
};

use crate::SystemError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestEventId(u64);

impl GuestEventId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GuestSourceId(u32);

impl GuestSourceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestEvent {
    id: GuestEventId,
    source: GuestSourceId,
    kind: GuestEventKind,
}

impl GuestEvent {
    pub const fn new(id: GuestEventId, source: GuestSourceId, kind: GuestEventKind) -> Self {
        Self { id, source, kind }
    }

    pub const fn id(&self) -> GuestEventId {
        self.id
    }

    pub const fn source(&self) -> GuestSourceId {
        self.source
    }

    pub const fn kind(&self) -> &GuestEventKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GuestEventKind {
    BootMilestone {
        name: String,
    },
    Command {
        command: String,
    },
    GuestHostCall {
        selector: u64,
        arguments: Vec<u64>,
        payload: Vec<u8>,
    },
    WorkBegin {
        work_id: u64,
        thread_id: u64,
    },
    WorkEnd {
        work_id: u64,
        thread_id: u64,
    },
    RoiBegin,
    RoiEnd,
    StatsReset,
    StatsDump,
    ExecutionModeSwitch {
        target: ExecutionModeTarget,
        mode: ExecutionMode,
    },
    Checkpoint {
        label: String,
    },
    RestoreCheckpoint {
        label: String,
    },
    Trap {
        trap: GuestTrap,
    },
    Terminate {
        code: i32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestHostCallResponse {
    status: i32,
    return_values: Vec<u64>,
    payload: Vec<u8>,
}

impl GuestHostCallResponse {
    pub fn new(status: i32, return_values: Vec<u64>, payload: Vec<u8>) -> Self {
        Self {
            status,
            return_values,
            payload,
        }
    }

    pub fn ok(return_values: Vec<u64>, payload: Vec<u8>) -> Self {
        Self::new(0, return_values, payload)
    }

    pub fn unhandled() -> Self {
        Self::new(-1, Vec::new(), Vec::new())
    }

    pub const fn status(&self) -> i32 {
        self.status
    }

    pub fn return_values(&self) -> &[u64] {
        &self.return_values
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExecutionMode {
    Functional,
    Timing,
    Detailed,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExecutionModeTarget(String);

impl ExecutionModeTarget {
    pub fn new(target: impl Into<String>) -> Self {
        Self(target.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GuestTrapKind {
    EnvironmentCall,
    Breakpoint,
    IllegalInstruction,
    Interrupt { code: u64 },
}

impl GuestTrapKind {
    pub const fn default_stop_code(self) -> i32 {
        match self {
            Self::EnvironmentCall => 0,
            Self::Breakpoint => 1,
            Self::IllegalInstruction => 2,
            Self::Interrupt { .. } => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestTrap {
    kind: GuestTrapKind,
    pc: u64,
}

impl GuestTrap {
    pub const fn new(kind: GuestTrapKind, pc: u64) -> Self {
        Self { kind, pc }
    }

    pub const fn kind(self) -> GuestTrapKind {
        self.kind
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuestEventDelivery {
    tick: Tick,
    source_partition: PartitionId,
    host_partition: PartitionId,
    event: GuestEvent,
}

impl GuestEventDelivery {
    pub const fn new(
        tick: Tick,
        source_partition: PartitionId,
        host_partition: PartitionId,
        event: GuestEvent,
    ) -> Self {
        Self {
            tick,
            source_partition,
            host_partition,
            event,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source_partition(&self) -> PartitionId {
        self.source_partition
    }

    pub const fn host_partition(&self) -> PartitionId {
        self.host_partition
    }

    pub const fn event(&self) -> &GuestEvent {
        &self.event
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuestEventChannel {
    host_partition: PartitionId,
    host_latency: Tick,
}

impl GuestEventChannel {
    pub const fn new(host_partition: PartitionId, host_latency: Tick) -> Result<Self, SystemError> {
        if host_latency == 0 {
            return Err(SystemError::ZeroHostLatency);
        }

        Ok(Self {
            host_partition,
            host_latency,
        })
    }

    pub const fn host_partition(self) -> PartitionId {
        self.host_partition
    }

    pub const fn host_latency(self) -> Tick {
        self.host_latency
    }

    pub fn emit<F>(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
        handler: F,
    ) -> Result<PartitionEventId, SystemError>
    where
        F: FnOnce(GuestEventDelivery) + Send + 'static,
    {
        let source_partition = context.partition();
        let host_partition = self.host_partition;
        context
            .schedule_remote_after(self.host_partition, self.host_latency, move |context| {
                handler(GuestEventDelivery::new(
                    context.now(),
                    source_partition,
                    host_partition,
                    event,
                ));
            })
            .map_err(SystemError::Scheduler)
    }

    pub fn emit_parallel<F>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        event: GuestEvent,
        handler: F,
    ) -> Result<PartitionEventId, SystemError>
    where
        F: FnOnce(GuestEventDelivery) + Send + 'static,
    {
        let source_partition = context.partition();
        let host_partition = self.host_partition;
        context
            .schedule_remote_after(self.host_partition, self.host_latency, move |context| {
                handler(GuestEventDelivery::new(
                    context.now(),
                    source_partition,
                    host_partition,
                    event,
                ));
            })
            .map_err(SystemError::Scheduler)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAction {
    InjectCommand {
        command: String,
    },
    RecordGuestHostCall {
        selector: u64,
        arguments: Vec<u64>,
        payload: Vec<u8>,
    },
    RecordRoiBegin {
        work_id: u64,
        thread_id: u64,
    },
    RecordRoiEnd {
        work_id: u64,
        thread_id: u64,
    },
    ResetStats,
    DumpStats,
    SwitchExecutionMode {
        target: ExecutionModeTarget,
        mode: ExecutionMode,
    },
    Checkpoint {
        label: String,
    },
    RestoreCheckpointByLabel {
        label: String,
    },
    RestoreCheckpoint {
        manifest: CheckpointManifest,
    },
    Stop {
        code: i32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostActionRecord {
    tick: Tick,
    source_partition: PartitionId,
    host_partition: PartitionId,
    event: GuestEventId,
    source: GuestSourceId,
    action: HostAction,
}

impl HostActionRecord {
    pub const fn new(
        tick: Tick,
        source_partition: PartitionId,
        host_partition: PartitionId,
        event: GuestEventId,
        source: GuestSourceId,
        action: HostAction,
    ) -> Self {
        Self {
            tick,
            source_partition,
            host_partition,
            event,
            source,
            action,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source_partition(&self) -> PartitionId {
        self.source_partition
    }

    pub const fn host_partition(&self) -> PartitionId {
        self.host_partition
    }

    pub const fn event(&self) -> GuestEventId {
        self.event
    }

    pub const fn source(&self) -> GuestSourceId {
        self.source
    }

    pub const fn action(&self) -> &HostAction {
        &self.action
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostEventPolicy;

impl HostEventPolicy {
    pub fn actions_for(&self, event: &GuestEvent) -> Vec<HostAction> {
        match event.kind() {
            GuestEventKind::BootMilestone { .. } => Vec::new(),
            GuestEventKind::Command { command } => vec![HostAction::InjectCommand {
                command: command.clone(),
            }],
            GuestEventKind::GuestHostCall {
                selector,
                arguments,
                payload,
            } => vec![HostAction::RecordGuestHostCall {
                selector: *selector,
                arguments: arguments.clone(),
                payload: payload.clone(),
            }],
            GuestEventKind::WorkBegin { work_id, thread_id } => vec![
                HostAction::RecordRoiBegin {
                    work_id: *work_id,
                    thread_id: *thread_id,
                },
                HostAction::ResetStats,
            ],
            GuestEventKind::WorkEnd { work_id, thread_id } => vec![
                HostAction::RecordRoiEnd {
                    work_id: *work_id,
                    thread_id: *thread_id,
                },
                HostAction::DumpStats,
            ],
            GuestEventKind::RoiBegin | GuestEventKind::StatsReset => {
                vec![HostAction::ResetStats]
            }
            GuestEventKind::RoiEnd | GuestEventKind::StatsDump => vec![HostAction::DumpStats],
            GuestEventKind::ExecutionModeSwitch { target, mode } => {
                vec![HostAction::SwitchExecutionMode {
                    target: target.clone(),
                    mode: *mode,
                }]
            }
            GuestEventKind::Checkpoint { label } => vec![HostAction::Checkpoint {
                label: label.clone(),
            }],
            GuestEventKind::RestoreCheckpoint { label } => {
                vec![HostAction::RestoreCheckpointByLabel {
                    label: label.clone(),
                }]
            }
            GuestEventKind::Trap { trap } => vec![HostAction::Stop {
                code: trap.kind().default_stop_code(),
            }],
            GuestEventKind::Terminate { code } => vec![HostAction::Stop { code: *code }],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StopRequest {
    tick: Tick,
    event: GuestEventId,
    source: GuestSourceId,
    code: i32,
}

impl StopRequest {
    pub const fn new(tick: Tick, event: GuestEventId, source: GuestSourceId, code: i32) -> Self {
        Self {
            tick,
            event,
            source,
            code,
        }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn event(self) -> GuestEventId {
        self.event
    }

    pub const fn source(self) -> GuestSourceId {
        self.source
    }

    pub const fn code(self) -> i32 {
        self.code
    }
}
