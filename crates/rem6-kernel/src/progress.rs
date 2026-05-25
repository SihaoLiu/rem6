use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{Tick, WaitForNode};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LivelockTransitionKind {
    SchedulerEpoch,
    ProtocolRetry,
    QueueRotation,
    MessageRetry,
    ResourceArbitration,
}

impl LivelockTransitionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SchedulerEpoch => "scheduler-epoch",
            Self::ProtocolRetry => "protocol-retry",
            Self::QueueRotation => "queue-rotation",
            Self::MessageRetry => "message-retry",
            Self::ResourceArbitration => "resource-arbitration",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LivelockTransitionKindCount {
    kind: LivelockTransitionKind,
    count: u64,
}

impl LivelockTransitionKindCount {
    const fn new(kind: LivelockTransitionKind, count: u64) -> Self {
        Self { kind, count }
    }

    pub const fn kind(&self) -> LivelockTransitionKind {
        self.kind
    }

    pub const fn count(&self) -> u64 {
        self.count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LivelockDiagnostic {
    subject: WaitForNode,
    threshold: u64,
    transition_count: u64,
    transition_kinds: Vec<LivelockTransitionKind>,
    transition_kind_counts: Vec<LivelockTransitionKindCount>,
    first_transition_tick: Tick,
    last_transition_tick: Tick,
    last_useful_tick: Option<Tick>,
}

impl LivelockDiagnostic {
    fn from_window(window: &ProgressWindow) -> Option<Self> {
        if window.transition_count < window.threshold {
            return None;
        }
        Some(Self {
            subject: window.subject.clone(),
            threshold: window.threshold,
            transition_count: window.transition_count,
            transition_kinds: window.transition_kinds.keys().copied().collect(),
            transition_kind_counts: transition_kind_counts(&window.transition_kinds),
            first_transition_tick: window.first_transition_tick?,
            last_transition_tick: window.last_transition_tick?,
            last_useful_tick: window.last_useful_tick,
        })
    }

    pub const fn subject(&self) -> &WaitForNode {
        &self.subject
    }

    pub const fn threshold(&self) -> u64 {
        self.threshold
    }

    pub const fn transition_count(&self) -> u64 {
        self.transition_count
    }

    pub fn transition_kinds(&self) -> &[LivelockTransitionKind] {
        &self.transition_kinds
    }

    pub fn transition_kind_counts(&self) -> &[LivelockTransitionKindCount] {
        &self.transition_kind_counts
    }

    pub fn transition_count_by_kind(&self, kind: LivelockTransitionKind) -> u64 {
        self.transition_kind_counts
            .iter()
            .find(|count| count.kind() == kind)
            .map(LivelockTransitionKindCount::count)
            .unwrap_or(0)
    }

    pub const fn first_transition_tick(&self) -> Tick {
        self.first_transition_tick
    }

    pub const fn last_transition_tick(&self) -> Tick {
        self.last_transition_tick
    }

    pub const fn last_useful_tick(&self) -> Option<Tick> {
        self.last_useful_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressWindowSnapshot {
    subject: WaitForNode,
    transition_count: u64,
    transition_kind_counts: Vec<LivelockTransitionKindCount>,
    first_transition_tick: Option<Tick>,
    last_transition_tick: Option<Tick>,
    last_useful_tick: Option<Tick>,
}

impl ProgressWindowSnapshot {
    fn from_window(window: &ProgressWindow) -> Self {
        Self {
            subject: window.subject.clone(),
            transition_count: window.transition_count,
            transition_kind_counts: transition_kind_counts(&window.transition_kinds),
            first_transition_tick: window.first_transition_tick,
            last_transition_tick: window.last_transition_tick,
            last_useful_tick: window.last_useful_tick,
        }
    }

    pub const fn subject(&self) -> &WaitForNode {
        &self.subject
    }

    pub const fn transition_count(&self) -> u64 {
        self.transition_count
    }

    pub fn transition_kind_counts(&self) -> &[LivelockTransitionKindCount] {
        &self.transition_kind_counts
    }

    pub const fn first_transition_tick(&self) -> Option<Tick> {
        self.first_transition_tick
    }

    pub const fn last_transition_tick(&self) -> Option<Tick> {
        self.last_transition_tick
    }

    pub const fn last_useful_tick(&self) -> Option<Tick> {
        self.last_useful_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressMonitorSnapshot {
    threshold: u64,
    windows: Vec<ProgressWindowSnapshot>,
    diagnostics: Vec<LivelockDiagnostic>,
}

impl ProgressMonitorSnapshot {
    fn from_monitor(monitor: &ProgressMonitor) -> Self {
        Self {
            threshold: monitor.threshold,
            windows: monitor
                .windows
                .values()
                .map(ProgressWindowSnapshot::from_window)
                .collect(),
            diagnostics: monitor.diagnostics(),
        }
    }

    pub const fn threshold(&self) -> u64 {
        self.threshold
    }

    pub fn windows(&self) -> &[ProgressWindowSnapshot] {
        &self.windows
    }

    pub fn diagnostics(&self) -> &[LivelockDiagnostic] {
        &self.diagnostics
    }

    pub fn has_livelock(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn transition_count(&self, subject: &WaitForNode) -> Option<u64> {
        self.windows
            .iter()
            .find(|window| window.subject() == subject)
            .map(ProgressWindowSnapshot::transition_count)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProgressMonitorError {
    ZeroTransitionThreshold,
}

impl fmt::Display for ProgressMonitorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroTransitionThreshold => {
                write!(formatter, "livelock transition threshold must be positive")
            }
        }
    }
}

impl Error for ProgressMonitorError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgressMonitor {
    threshold: u64,
    windows: BTreeMap<WaitForNode, ProgressWindow>,
}

impl ProgressMonitor {
    pub fn with_transition_threshold(threshold: u64) -> Result<Self, ProgressMonitorError> {
        if threshold == 0 {
            return Err(ProgressMonitorError::ZeroTransitionThreshold);
        }
        Ok(Self {
            threshold,
            windows: BTreeMap::new(),
        })
    }

    pub fn record_transition(
        &mut self,
        subject: WaitForNode,
        kind: LivelockTransitionKind,
        tick: Tick,
    ) -> Result<(), ProgressMonitorError> {
        self.windows
            .entry(subject.clone())
            .or_insert_with(|| ProgressWindow::new(subject, self.threshold))
            .record_transition(kind, tick);
        Ok(())
    }

    pub fn record_useful_work(&mut self, subject: &WaitForNode, tick: Tick) -> bool {
        let Some(window) = self.windows.get_mut(subject) else {
            return false;
        };
        window.record_useful_work(tick);
        true
    }

    pub fn diagnostic(&self, subject: &WaitForNode) -> Option<LivelockDiagnostic> {
        self.windows
            .get(subject)
            .and_then(LivelockDiagnostic::from_window)
    }

    pub fn diagnostics(&self) -> Vec<LivelockDiagnostic> {
        self.windows
            .values()
            .filter_map(LivelockDiagnostic::from_window)
            .collect()
    }

    pub fn snapshot(&self) -> ProgressMonitorSnapshot {
        ProgressMonitorSnapshot::from_monitor(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProgressWindow {
    subject: WaitForNode,
    threshold: u64,
    transition_count: u64,
    transition_kinds: BTreeMap<LivelockTransitionKind, u64>,
    first_transition_tick: Option<Tick>,
    last_transition_tick: Option<Tick>,
    last_useful_tick: Option<Tick>,
}

impl ProgressWindow {
    fn new(subject: WaitForNode, threshold: u64) -> Self {
        Self {
            subject,
            threshold,
            transition_count: 0,
            transition_kinds: BTreeMap::new(),
            first_transition_tick: None,
            last_transition_tick: None,
            last_useful_tick: None,
        }
    }

    fn record_transition(&mut self, kind: LivelockTransitionKind, tick: Tick) {
        self.transition_count += 1;
        *self.transition_kinds.entry(kind).or_insert(0) += 1;
        self.first_transition_tick = Some(
            self.first_transition_tick
                .map_or(tick, |first| first.min(tick)),
        );
        self.last_transition_tick = Some(
            self.last_transition_tick
                .map_or(tick, |last| last.max(tick)),
        );
    }

    fn record_useful_work(&mut self, tick: Tick) {
        self.transition_count = 0;
        self.transition_kinds.clear();
        self.first_transition_tick = None;
        self.last_transition_tick = None;
        self.last_useful_tick = Some(tick);
    }
}

fn transition_kind_counts(
    counts: &BTreeMap<LivelockTransitionKind, u64>,
) -> Vec<LivelockTransitionKindCount> {
    counts
        .iter()
        .map(|(kind, count)| LivelockTransitionKindCount::new(*kind, *count))
        .collect()
}
