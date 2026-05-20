use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionId, Tick};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterruptLineId(u64);

impl InterruptLineId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterruptTargetId(u32);

impl InterruptTargetId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterruptSourceId(u32);

impl InterruptSourceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptRoute {
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
}

impl InterruptRoute {
    pub const fn new(
        line: InterruptLineId,
        target: InterruptTargetId,
        target_partition: PartitionId,
    ) -> Self {
        Self {
            line,
            target,
            target_partition,
        }
    }

    pub const fn line(&self) -> InterruptLineId {
        self.line
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InterruptEventKind {
    Assert,
    Deassert,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptEvent {
    tick: Tick,
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
    source: InterruptSourceId,
    kind: InterruptEventKind,
}

impl InterruptEvent {
    pub const fn new(
        tick: Tick,
        line: InterruptLineId,
        target: InterruptTargetId,
        source: InterruptSourceId,
        kind: InterruptEventKind,
    ) -> Self {
        Self::routed(
            tick,
            line,
            target,
            PartitionId::new(target.get()),
            source,
            kind,
        )
    }

    pub const fn routed(
        tick: Tick,
        line: InterruptLineId,
        target: InterruptTargetId,
        target_partition: PartitionId,
        source: InterruptSourceId,
        kind: InterruptEventKind,
    ) -> Self {
        Self {
            tick,
            line,
            target,
            target_partition,
            source,
            kind,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn line(&self) -> InterruptLineId {
        self.line
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn kind(&self) -> InterruptEventKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingInterrupt {
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
    source: InterruptSourceId,
    asserted_tick: Tick,
}

impl PendingInterrupt {
    pub const fn new(
        line: InterruptLineId,
        target: InterruptTargetId,
        source: InterruptSourceId,
        asserted_tick: Tick,
    ) -> Self {
        Self::routed(
            line,
            target,
            PartitionId::new(target.get()),
            source,
            asserted_tick,
        )
    }

    pub const fn routed(
        line: InterruptLineId,
        target: InterruptTargetId,
        target_partition: PartitionId,
        source: InterruptSourceId,
        asserted_tick: Tick,
    ) -> Self {
        Self {
            line,
            target,
            target_partition,
            source,
            asserted_tick,
        }
    }

    pub const fn line(&self) -> InterruptLineId {
        self.line
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn asserted_tick(&self) -> Tick {
        self.asserted_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterruptSnapshot {
    tick: Tick,
    pending: Vec<PendingInterrupt>,
    history: Vec<InterruptEvent>,
}

impl InterruptSnapshot {
    pub const fn new(
        tick: Tick,
        pending: Vec<PendingInterrupt>,
        history: Vec<InterruptEvent>,
    ) -> Self {
        Self {
            tick,
            pending,
            history,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn pending(&self) -> &[PendingInterrupt] {
        &self.pending
    }

    pub fn history(&self) -> &[InterruptEvent] {
        &self.history
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InterruptController {
    routes: BTreeMap<InterruptLineId, InterruptRoute>,
    pending: BTreeMap<InterruptLineId, PendingInterrupt>,
    history: Vec<InterruptEvent>,
}

impl InterruptController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_line(
        &mut self,
        line: InterruptLineId,
        target: InterruptTargetId,
    ) -> Result<(), InterruptError> {
        self.register_route(InterruptRoute::new(
            line,
            target,
            PartitionId::new(target.get()),
        ))
    }

    pub fn register_route(&mut self, route: InterruptRoute) -> Result<(), InterruptError> {
        if self.routes.contains_key(&route.line()) {
            return Err(InterruptError::DuplicateLine { line: route.line() });
        }

        self.routes.insert(route.line(), route);
        Ok(())
    }

    pub fn route(&self, line: InterruptLineId) -> Option<&InterruptRoute> {
        self.routes.get(&line)
    }

    pub fn assert(
        &mut self,
        line: InterruptLineId,
        source: InterruptSourceId,
        tick: Tick,
    ) -> Result<(), InterruptError> {
        let route = self.route_for(line)?;
        if let Some(pending) = self.pending.get(&line) {
            return Err(InterruptError::AlreadyPending {
                line,
                source: pending.source(),
            });
        }

        self.pending.insert(
            line,
            PendingInterrupt::routed(line, route.target(), route.target_partition(), source, tick),
        );
        self.history.push(InterruptEvent::routed(
            tick,
            line,
            route.target(),
            route.target_partition(),
            source,
            InterruptEventKind::Assert,
        ));
        Ok(())
    }

    pub fn deassert(
        &mut self,
        line: InterruptLineId,
        source: InterruptSourceId,
        tick: Tick,
    ) -> Result<(), InterruptError> {
        let route = self.route_for(line)?;
        let pending = self
            .pending
            .get(&line)
            .ok_or(InterruptError::NotPending { line })?;
        if pending.source() != source {
            return Err(InterruptError::SourceMismatch {
                line,
                expected: pending.source(),
                actual: source,
            });
        }

        self.pending.remove(&line);
        self.history.push(InterruptEvent::routed(
            tick,
            line,
            route.target(),
            route.target_partition(),
            source,
            InterruptEventKind::Deassert,
        ));
        Ok(())
    }

    pub fn pending(&self) -> Vec<PendingInterrupt> {
        let mut pending = self.pending.values().cloned().collect::<Vec<_>>();
        pending.sort_by_key(|entry| (entry.target_partition(), entry.target(), entry.line()));
        pending
    }

    pub fn history(&self) -> &[InterruptEvent] {
        &self.history
    }

    pub fn snapshot(&self, tick: Tick) -> InterruptSnapshot {
        InterruptSnapshot::new(tick, self.pending(), self.history.clone())
    }

    fn route_for(&self, line: InterruptLineId) -> Result<InterruptRoute, InterruptError> {
        self.routes
            .get(&line)
            .copied()
            .ok_or(InterruptError::UnknownLine { line })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterruptError {
    DuplicateLine {
        line: InterruptLineId,
    },
    UnknownLine {
        line: InterruptLineId,
    },
    AlreadyPending {
        line: InterruptLineId,
        source: InterruptSourceId,
    },
    NotPending {
        line: InterruptLineId,
    },
    SourceMismatch {
        line: InterruptLineId,
        expected: InterruptSourceId,
        actual: InterruptSourceId,
    },
}

impl fmt::Display for InterruptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateLine { line } => {
                write!(
                    formatter,
                    "interrupt line {} is already registered",
                    line.get()
                )
            }
            Self::UnknownLine { line } => {
                write!(formatter, "unknown interrupt line {}", line.get())
            }
            Self::AlreadyPending { line, source } => write!(
                formatter,
                "interrupt line {} is already pending from source {}",
                line.get(),
                source.get()
            ),
            Self::NotPending { line } => {
                write!(formatter, "interrupt line {} is not pending", line.get())
            }
            Self::SourceMismatch {
                line,
                expected,
                actual,
            } => write!(
                formatter,
                "interrupt line {} is pending from source {}, not source {}",
                line.get(),
                expected.get(),
                actual.get()
            ),
        }
    }
}

impl Error for InterruptError {}
