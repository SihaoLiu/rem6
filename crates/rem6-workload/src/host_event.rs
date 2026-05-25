use rem6_kernel::Tick;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadExecutionMode {
    Functional,
    Timing,
    Detailed,
}

impl WorkloadExecutionMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Functional => "functional",
            Self::Timing => "timing",
            Self::Detailed => "detailed",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadExecutionModeSwitch {
    tick: Tick,
    target: String,
    mode: WorkloadExecutionMode,
    stats_scope: Option<WorkloadStatsScope>,
}

impl WorkloadExecutionModeSwitch {
    pub fn new(tick: Tick, target: impl Into<String>, mode: WorkloadExecutionMode) -> Self {
        Self {
            tick,
            target: target.into(),
            mode,
            stats_scope: None,
        }
    }

    pub const fn with_stats_scope(mut self, epoch: u64, reset_tick: Tick) -> Self {
        self.stats_scope = Some(WorkloadStatsScope::new(epoch, reset_tick));
        self
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn mode(&self) -> &WorkloadExecutionMode {
        &self.mode
    }

    pub const fn stats_scope(&self) -> Option<&WorkloadStatsScope> {
        self.stats_scope.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadStatsScope {
    epoch: u64,
    reset_tick: Tick,
}

impl WorkloadStatsScope {
    pub const fn new(epoch: u64, reset_tick: Tick) -> Self {
        Self { epoch, reset_tick }
    }

    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    pub const fn reset_tick(&self) -> Tick {
        self.reset_tick
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkloadHostActionSummary {
    total_action_count: usize,
    injected_command_count: usize,
    stats_reset_count: usize,
    stats_dump_count: usize,
    checkpoint_count: usize,
    checkpoint_restore_count: usize,
    execution_mode_switch_count: usize,
    guest_host_call_count: usize,
    stop_count: usize,
}

impl WorkloadHostActionSummary {
    pub fn record_injected_command(&mut self) {
        self.total_action_count += 1;
        self.injected_command_count += 1;
    }

    pub fn record_stats_reset(&mut self) {
        self.total_action_count += 1;
        self.stats_reset_count += 1;
    }

    pub fn record_stats_dump(&mut self) {
        self.total_action_count += 1;
        self.stats_dump_count += 1;
    }

    pub fn record_checkpoint(&mut self) {
        self.total_action_count += 1;
        self.checkpoint_count += 1;
    }

    pub fn record_checkpoint_restore(&mut self) {
        self.total_action_count += 1;
        self.checkpoint_restore_count += 1;
    }

    pub fn record_execution_mode_switch(&mut self) {
        self.total_action_count += 1;
        self.execution_mode_switch_count += 1;
    }

    pub fn record_guest_host_call(&mut self) {
        self.total_action_count += 1;
        self.guest_host_call_count += 1;
    }

    pub fn record_stop(&mut self) {
        self.total_action_count += 1;
        self.stop_count += 1;
    }

    pub const fn total_action_count(&self) -> usize {
        self.total_action_count
    }

    pub const fn injected_command_count(&self) -> usize {
        self.injected_command_count
    }

    pub const fn stats_reset_count(&self) -> usize {
        self.stats_reset_count
    }

    pub const fn stats_dump_count(&self) -> usize {
        self.stats_dump_count
    }

    pub const fn checkpoint_count(&self) -> usize {
        self.checkpoint_count
    }

    pub const fn checkpoint_restore_count(&self) -> usize {
        self.checkpoint_restore_count
    }

    pub const fn execution_mode_switch_count(&self) -> usize {
        self.execution_mode_switch_count
    }

    pub const fn guest_host_call_count(&self) -> usize {
        self.guest_host_call_count
    }

    pub const fn stop_count(&self) -> usize {
        self.stop_count
    }

    pub const fn has_host_actions(&self) -> bool {
        self.total_action_count != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadGuestHostCallResponse {
    status: i32,
    return_values: Vec<u64>,
    payload: Vec<u8>,
}

impl WorkloadGuestHostCallResponse {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostEventIntent {
    RoiBegin {
        label: String,
    },
    RoiEnd {
        label: String,
    },
    StatsReset {
        label: String,
    },
    StatsDump {
        label: String,
    },
    SwitchExecutionMode {
        target: String,
        mode: WorkloadExecutionMode,
    },
    GuestHostCall {
        selector: u64,
        arguments: Vec<u64>,
        payload: Vec<u8>,
        response: Option<WorkloadGuestHostCallResponse>,
    },
    Checkpoint {
        label: String,
    },
    RestoreCheckpoint {
        label: String,
    },
    Stop {
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadHostEvent {
    tick: Tick,
    intent: HostEventIntent,
}

impl WorkloadHostEvent {
    pub const fn new(tick: Tick, intent: HostEventIntent) -> Self {
        Self { tick, intent }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn intent(&self) -> &HostEventIntent {
        &self.intent
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointLineage {
    CreatedByWorkload {
        label: String,
    },
    RestoredFrom {
        label: String,
        manifest_identity: String,
    },
}

impl CheckpointLineage {
    pub fn label(&self) -> &str {
        match self {
            Self::CreatedByWorkload { label } | Self::RestoredFrom { label, .. } => label,
        }
    }

    pub fn manifest_identity(&self) -> Option<&str> {
        match self {
            Self::CreatedByWorkload { .. } => None,
            Self::RestoredFrom {
                manifest_identity, ..
            } => Some(manifest_identity),
        }
    }
}

pub(crate) fn host_event_sort_key(event: &WorkloadHostEvent) -> (Tick, u8, String) {
    let (rank, label) = match event.intent() {
        HostEventIntent::RoiBegin { label } => (0, label.clone()),
        HostEventIntent::RoiEnd { label } => (1, label.clone()),
        HostEventIntent::StatsReset { label } => (2, label.clone()),
        HostEventIntent::StatsDump { label } => (3, label.clone()),
        HostEventIntent::SwitchExecutionMode { target, .. } => (4, target.clone()),
        HostEventIntent::GuestHostCall { selector, .. } => (5, selector.to_string()),
        HostEventIntent::Checkpoint { label } => (6, label.clone()),
        HostEventIntent::RestoreCheckpoint { label } => (7, label.clone()),
        HostEventIntent::Stop { reason } => (8, reason.clone()),
    };
    (event.tick(), rank, label)
}

pub(crate) fn planned_checkpoint_labels(events: &[WorkloadHostEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event.intent() {
            HostEventIntent::Checkpoint { label } => Some(label.clone()),
            _ => None,
        })
        .collect()
}

pub(crate) fn planned_checkpoint_restore_labels(events: &[WorkloadHostEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event.intent() {
            HostEventIntent::RestoreCheckpoint { label } => Some(label.clone()),
            _ => None,
        })
        .collect()
}

pub(crate) fn planned_execution_mode_switches(
    events: &[WorkloadHostEvent],
) -> Vec<WorkloadExecutionModeSwitch> {
    events
        .iter()
        .filter_map(|event| match event.intent() {
            HostEventIntent::SwitchExecutionMode { target, mode } => Some(
                WorkloadExecutionModeSwitch::new(event.tick(), target.clone(), mode.clone()),
            ),
            _ => None,
        })
        .collect()
}

pub(crate) fn execution_mode_switch_matches(
    expected: &WorkloadExecutionModeSwitch,
    actual: &WorkloadExecutionModeSwitch,
) -> bool {
    expected.tick() == actual.tick()
        && expected.target() == actual.target()
        && expected.mode() == actual.mode()
}

pub(crate) fn planned_stop_reason(events: &[WorkloadHostEvent]) -> Option<String> {
    events.iter().find_map(|event| match event.intent() {
        HostEventIntent::Stop { reason } => Some(reason.clone()),
        _ => None,
    })
}
