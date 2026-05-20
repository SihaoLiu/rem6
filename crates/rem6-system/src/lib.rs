use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointError, CheckpointManifest, CheckpointRegistry};
use rem6_kernel::{PartitionEventId, PartitionId, SchedulerContext, SchedulerError, Tick};
use rem6_stats::{StatSnapshot, StatsError, StatsRegistry, StatsResetRecord};

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
    RestoreCheckpoint { manifest: CheckpointManifest },
    Stop { code: i32 },
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemActionOutcome {
    InjectedCommand {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        command: String,
    },
    StatsReset(StatsResetRecord),
    StatsSnapshot(StatSnapshot),
    Checkpoint {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        manifest: CheckpointManifest,
    },
    CheckpointRestored {
        tick: Tick,
        event: GuestEventId,
        source: GuestSourceId,
        manifest: CheckpointManifest,
    },
    Stop(StopRequest),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemActionExecutor {
    stats: StatsRegistry,
    checkpoints: CheckpointRegistry,
}

impl SystemActionExecutor {
    pub fn new(stats: StatsRegistry) -> Self {
        Self::with_checkpoint(stats, CheckpointRegistry::new())
    }

    pub fn with_checkpoint(stats: StatsRegistry, checkpoints: CheckpointRegistry) -> Self {
        Self { stats, checkpoints }
    }

    pub const fn stats(&self) -> &StatsRegistry {
        &self.stats
    }

    pub const fn stats_mut(&mut self) -> &mut StatsRegistry {
        &mut self.stats
    }

    pub const fn checkpoints(&self) -> &CheckpointRegistry {
        &self.checkpoints
    }

    pub const fn checkpoints_mut(&mut self) -> &mut CheckpointRegistry {
        &mut self.checkpoints
    }

    pub fn apply(&mut self, record: &HostActionRecord) -> Result<SystemActionOutcome, SystemError> {
        match record.action() {
            HostAction::InjectCommand { command } => Ok(SystemActionOutcome::InjectedCommand {
                tick: record.tick(),
                event: record.event(),
                source: record.source(),
                command: command.clone(),
            }),
            HostAction::ResetStats => Ok(SystemActionOutcome::StatsReset(
                self.stats.reset(record.tick()),
            )),
            HostAction::DumpStats => self
                .stats
                .try_snapshot(record.tick())
                .map(SystemActionOutcome::StatsSnapshot)
                .map_err(SystemError::Stats),
            HostAction::Checkpoint { label } => self
                .checkpoints
                .capture(label.clone(), record.tick())
                .map(|manifest| SystemActionOutcome::Checkpoint {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest,
                })
                .map_err(SystemError::Checkpoint),
            HostAction::RestoreCheckpoint { manifest } => self
                .checkpoints
                .restore(manifest)
                .map(|()| SystemActionOutcome::CheckpointRestored {
                    tick: record.tick(),
                    event: record.event(),
                    source: record.source(),
                    manifest: manifest.clone(),
                })
                .map_err(SystemError::Checkpoint),
            HostAction::Stop { code } => Ok(SystemActionOutcome::Stop(StopRequest::new(
                record.tick(),
                record.event(),
                record.source(),
                *code,
            ))),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemRunController {
    policy: HostEventPolicy,
    deliveries: Vec<GuestEventDelivery>,
    actions: Vec<HostActionRecord>,
    outcomes: Vec<SystemActionOutcome>,
    stop_request: Option<StopRequest>,
}

impl SystemRunController {
    pub const fn new(policy: HostEventPolicy) -> Self {
        Self {
            policy,
            deliveries: Vec::new(),
            actions: Vec::new(),
            outcomes: Vec::new(),
            stop_request: None,
        }
    }

    pub fn handle_delivery(&mut self, delivery: GuestEventDelivery) -> Vec<HostActionRecord> {
        let produced: Vec<_> = self
            .policy
            .actions_for(delivery.event())
            .into_iter()
            .map(|action| {
                HostActionRecord::new(
                    delivery.tick(),
                    delivery.source_partition(),
                    delivery.host_partition(),
                    delivery.event().id(),
                    delivery.event().source(),
                    action,
                )
            })
            .collect();

        for record in &produced {
            self.record_stop_request(record);
        }

        self.deliveries.push(delivery);
        self.actions.extend(produced.iter().cloned());
        produced
    }

    pub fn execute_record(
        &mut self,
        record: HostActionRecord,
        executor: &mut SystemActionExecutor,
    ) -> Result<SystemActionOutcome, SystemError> {
        self.record_stop_request(&record);
        self.actions.push(record.clone());
        let outcome = executor.apply(&record)?;
        self.outcomes.push(outcome.clone());
        Ok(outcome)
    }

    fn record_stop_request(&mut self, record: &HostActionRecord) {
        if self.stop_request.is_none() && matches!(record.action(), HostAction::Stop { .. }) {
            let HostAction::Stop { code } = record.action() else {
                unreachable!("stop record was matched above");
            };
            self.stop_request = Some(StopRequest::new(
                record.tick(),
                record.event(),
                record.source(),
                *code,
            ));
        }
    }

    pub fn execute_delivery(
        &mut self,
        delivery: GuestEventDelivery,
        executor: &mut SystemActionExecutor,
    ) -> Result<Vec<SystemActionOutcome>, SystemError> {
        let records = self.handle_delivery(delivery);
        let outcomes = records
            .iter()
            .map(|record| executor.apply(record))
            .collect::<Result<Vec<_>, _>>()?;
        self.outcomes.extend(outcomes.iter().cloned());
        Ok(outcomes)
    }

    pub fn deliveries(&self) -> &[GuestEventDelivery] {
        &self.deliveries
    }

    pub fn action_records(&self) -> &[HostActionRecord] {
        &self.actions
    }

    pub fn action_outcomes(&self) -> &[SystemActionOutcome] {
        &self.outcomes
    }

    pub const fn stop_request(&self) -> Option<&StopRequest> {
        self.stop_request.as_ref()
    }

    pub const fn is_stopped(&self) -> bool {
        self.stop_request.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemHostController {
    run: SystemRunController,
    executor: SystemActionExecutor,
    action_errors: Vec<SystemError>,
}

impl SystemHostController {
    pub fn new(policy: HostEventPolicy, stats: StatsRegistry) -> Self {
        Self {
            run: SystemRunController::new(policy),
            executor: SystemActionExecutor::new(stats),
            action_errors: Vec::new(),
        }
    }

    pub const fn run(&self) -> &SystemRunController {
        &self.run
    }

    pub const fn run_mut(&mut self) -> &mut SystemRunController {
        &mut self.run
    }

    pub const fn executor(&self) -> &SystemActionExecutor {
        &self.executor
    }

    pub const fn executor_mut(&mut self) -> &mut SystemActionExecutor {
        &mut self.executor
    }

    pub fn handle_delivery(&mut self, delivery: GuestEventDelivery) -> Vec<SystemActionOutcome> {
        match self.run.execute_delivery(delivery, &mut self.executor) {
            Ok(outcomes) => outcomes,
            Err(error) => {
                self.action_errors.push(error);
                Vec::new()
            }
        }
    }

    pub fn action_errors(&self) -> &[SystemError] {
        &self.action_errors
    }
}

#[derive(Clone, Debug)]
pub struct SystemEventPort {
    channel: GuestEventChannel,
    controller: Arc<Mutex<SystemRunController>>,
}

impl SystemEventPort {
    pub fn new(channel: GuestEventChannel, controller: Arc<Mutex<SystemRunController>>) -> Self {
        Self {
            channel,
            controller,
        }
    }

    pub fn with_controller(
        host_partition: PartitionId,
        host_latency: Tick,
        policy: HostEventPolicy,
    ) -> Result<Self, SystemError> {
        Ok(Self::new(
            GuestEventChannel::new(host_partition, host_latency)?,
            Arc::new(Mutex::new(SystemRunController::new(policy))),
        ))
    }

    pub const fn channel(&self) -> GuestEventChannel {
        self.channel
    }

    pub fn controller(&self) -> Arc<Mutex<SystemRunController>> {
        Arc::clone(&self.controller)
    }

    pub fn emit(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
    ) -> Result<PartitionEventId, SystemError> {
        let controller = Arc::clone(&self.controller);
        self.channel.emit(context, event, move |delivery| {
            controller
                .lock()
                .expect("system run controller lock")
                .handle_delivery(delivery);
        })
    }
}

#[derive(Clone, Debug)]
pub struct SystemHostEventPort {
    channel: GuestEventChannel,
    controller: Arc<Mutex<SystemHostController>>,
}

impl SystemHostEventPort {
    pub fn new(channel: GuestEventChannel, controller: Arc<Mutex<SystemHostController>>) -> Self {
        Self {
            channel,
            controller,
        }
    }

    pub fn with_controller(
        host_partition: PartitionId,
        host_latency: Tick,
        controller: Arc<Mutex<SystemHostController>>,
    ) -> Result<Self, SystemError> {
        Ok(Self::new(
            GuestEventChannel::new(host_partition, host_latency)?,
            controller,
        ))
    }

    pub const fn channel(&self) -> GuestEventChannel {
        self.channel
    }

    pub fn controller(&self) -> Arc<Mutex<SystemHostController>> {
        Arc::clone(&self.controller)
    }

    pub fn emit(
        &self,
        context: &mut SchedulerContext<'_>,
        event: GuestEvent,
    ) -> Result<PartitionEventId, SystemError> {
        let controller = Arc::clone(&self.controller);
        self.channel.emit(context, event, move |delivery| {
            controller
                .lock()
                .expect("system host controller lock")
                .handle_delivery(delivery);
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemError {
    ZeroHostLatency,
    Scheduler(SchedulerError),
    Stats(StatsError),
    Checkpoint(CheckpointError),
}

impl fmt::Display for SystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroHostLatency => {
                write!(formatter, "guest event channel latency must be positive")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Stats(error) => write!(formatter, "{error}"),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for SystemError {}
