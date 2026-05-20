use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionEventId, PartitionId, SchedulerContext, SchedulerError, Tick};

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
    BootMilestone { name: String },
    Command { command: String },
    RoiBegin,
    RoiEnd,
    StatsReset,
    StatsDump,
    Checkpoint { label: String },
    Terminate { code: i32 },
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAction {
    InjectCommand { command: String },
    ResetStats,
    DumpStats,
    Checkpoint { label: String },
    Stop { code: i32 },
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
            GuestEventKind::RoiBegin | GuestEventKind::StatsReset => {
                vec![HostAction::ResetStats]
            }
            GuestEventKind::RoiEnd | GuestEventKind::StatsDump => vec![HostAction::DumpStats],
            GuestEventKind::Checkpoint { label } => vec![HostAction::Checkpoint {
                label: label.clone(),
            }],
            GuestEventKind::Terminate { code } => vec![HostAction::Stop { code: *code }],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemError {
    ZeroHostLatency,
    Scheduler(SchedulerError),
}

impl fmt::Display for SystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroHostLatency => {
                write!(formatter, "guest event channel latency must be positive")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for SystemError {}
