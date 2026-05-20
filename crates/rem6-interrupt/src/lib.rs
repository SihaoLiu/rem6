use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionEventId, PartitionId, SchedulerContext, SchedulerError, Tick};

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
pub struct InterruptDelivery {
    tick: Tick,
    source_partition: PartitionId,
    route: InterruptRoute,
    source: InterruptSourceId,
    kind: InterruptEventKind,
}

impl InterruptDelivery {
    pub const fn new(
        tick: Tick,
        source_partition: PartitionId,
        route: InterruptRoute,
        source: InterruptSourceId,
        kind: InterruptEventKind,
    ) -> Self {
        Self {
            tick,
            source_partition,
            route,
            source,
            kind,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source_partition(&self) -> PartitionId {
        self.source_partition
    }

    pub const fn route(&self) -> InterruptRoute {
        self.route
    }

    pub const fn line(&self) -> InterruptLineId {
        self.route.line()
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.route.target()
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.route.target_partition()
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn kind(&self) -> InterruptEventKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptLineChannel {
    route: InterruptRoute,
    signal_latency: Tick,
}

impl InterruptLineChannel {
    pub const fn new(route: InterruptRoute, signal_latency: Tick) -> Result<Self, InterruptError> {
        if signal_latency == 0 {
            return Err(InterruptError::ZeroSignalLatency);
        }

        Ok(Self {
            route,
            signal_latency,
        })
    }

    pub const fn route(self) -> InterruptRoute {
        self.route
    }

    pub const fn signal_latency(self) -> Tick {
        self.signal_latency
    }

    pub fn assert<F>(
        &self,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
        handler: F,
    ) -> Result<PartitionEventId, InterruptError>
    where
        F: FnOnce(InterruptDelivery) + Send + 'static,
    {
        self.emit(context, source, InterruptEventKind::Assert, handler)
    }

    pub fn deassert<F>(
        &self,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
        handler: F,
    ) -> Result<PartitionEventId, InterruptError>
    where
        F: FnOnce(InterruptDelivery) + Send + 'static,
    {
        self.emit(context, source, InterruptEventKind::Deassert, handler)
    }

    pub fn emit<F>(
        &self,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
        kind: InterruptEventKind,
        handler: F,
    ) -> Result<PartitionEventId, InterruptError>
    where
        F: FnOnce(InterruptDelivery) + Send + 'static,
    {
        let source_partition = context.partition();
        let route = self.route;
        context
            .schedule_remote_after(
                route.target_partition(),
                self.signal_latency,
                move |context| {
                    handler(InterruptDelivery::new(
                        context.now(),
                        source_partition,
                        route,
                        source,
                        kind,
                    ));
                },
            )
            .map_err(InterruptError::Scheduler)
    }
}

#[derive(Clone, Debug)]
pub struct InterruptLinePort {
    channel: InterruptLineChannel,
    controller: Arc<Mutex<InterruptController>>,
    delivery_errors: Arc<Mutex<Vec<InterruptError>>>,
}

impl InterruptLinePort {
    pub fn new(channel: InterruptLineChannel, controller: Arc<Mutex<InterruptController>>) -> Self {
        Self {
            channel,
            controller,
            delivery_errors: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub const fn channel(&self) -> InterruptLineChannel {
        self.channel
    }

    pub fn controller(&self) -> Arc<Mutex<InterruptController>> {
        Arc::clone(&self.controller)
    }

    pub fn delivery_errors(&self) -> Arc<Mutex<Vec<InterruptError>>> {
        Arc::clone(&self.delivery_errors)
    }

    pub fn assert(
        &self,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<PartitionEventId, InterruptError> {
        self.emit(context, source, InterruptEventKind::Assert)
    }

    pub fn deassert(
        &self,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<PartitionEventId, InterruptError> {
        self.emit(context, source, InterruptEventKind::Deassert)
    }

    pub fn emit(
        &self,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
        kind: InterruptEventKind,
    ) -> Result<PartitionEventId, InterruptError> {
        let controller = Arc::clone(&self.controller);
        let delivery_errors = Arc::clone(&self.delivery_errors);
        self.channel.emit(context, source, kind, move |delivery| {
            let result = controller
                .lock()
                .expect("interrupt controller lock")
                .apply_delivery(delivery);
            if let Err(error) = result {
                delivery_errors
                    .lock()
                    .expect("interrupt delivery error lock")
                    .push(error);
            }
        })
    }
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

    pub fn apply_delivery(&mut self, delivery: InterruptDelivery) -> Result<(), InterruptError> {
        self.check_delivery_route(delivery.route())?;
        match delivery.kind() {
            InterruptEventKind::Assert => {
                self.assert(delivery.line(), delivery.source(), delivery.tick())
            }
            InterruptEventKind::Deassert => {
                self.deassert(delivery.line(), delivery.source(), delivery.tick())
            }
        }
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

    fn check_delivery_route(&self, route: InterruptRoute) -> Result<(), InterruptError> {
        let expected = self.route_for(route.line())?;
        if expected != route {
            return Err(InterruptError::RouteMismatch {
                line: route.line(),
                expected,
                actual: route,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterruptError {
    ZeroSignalLatency,
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
    RouteMismatch {
        line: InterruptLineId,
        expected: InterruptRoute,
        actual: InterruptRoute,
    },
    Scheduler(SchedulerError),
}

impl fmt::Display for InterruptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSignalLatency => write!(formatter, "interrupt signal latency must be positive"),
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
            Self::RouteMismatch {
                line,
                expected,
                actual,
            } => write!(
                formatter,
                "interrupt line {} delivery route targets partition {} target {}, expected partition {} target {}",
                line.get(),
                actual.target_partition().index(),
                actual.target().get(),
                expected.target_partition().index(),
                expected.target().get()
            ),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for InterruptError {}
