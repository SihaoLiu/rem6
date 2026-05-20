use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, SchedulerContext, SchedulerError, Tick,
};
use rem6_memory::{Address, ByteMask};
use rem6_mmio::{MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

pub const INTERRUPT_MMIO_REGISTER_BYTES: u64 = 8;
pub const INTERRUPT_MMIO_PENDING_OFFSET: u64 = 0x00;
pub const INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET: u64 = 0x08;
pub const INTERRUPT_MMIO_PRIORITY_BASE_OFFSET: u64 = 0x100;
pub const INTERRUPT_MMIO_PRIORITY_STRIDE: u64 = INTERRUPT_MMIO_REGISTER_BYTES;

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

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InterruptPriority(u32);

impl InterruptPriority {
    pub const ZERO: Self = Self(0);
    pub const DEFAULT: Self = Self(1);

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
    Claim,
    Complete,
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

    pub fn assert_parallel<F>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
        handler: F,
    ) -> Result<PartitionEventId, InterruptError>
    where
        F: FnOnce(InterruptDelivery) + Send + 'static,
    {
        self.emit_parallel(context, source, InterruptEventKind::Assert, handler)
    }

    pub fn deassert_parallel<F>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
        handler: F,
    ) -> Result<PartitionEventId, InterruptError>
    where
        F: FnOnce(InterruptDelivery) + Send + 'static,
    {
        self.emit_parallel(context, source, InterruptEventKind::Deassert, handler)
    }

    pub fn emit_parallel<F>(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
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

    pub fn assert_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<PartitionEventId, InterruptError> {
        self.emit_parallel(context, source, InterruptEventKind::Assert)
    }

    pub fn deassert_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<PartitionEventId, InterruptError> {
        self.emit_parallel(context, source, InterruptEventKind::Deassert)
    }

    pub fn emit_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
        kind: InterruptEventKind,
    ) -> Result<PartitionEventId, InterruptError> {
        let controller = Arc::clone(&self.controller);
        let delivery_errors = Arc::clone(&self.delivery_errors);
        self.channel
            .emit_parallel(context, source, kind, move |delivery| {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterruptClaim {
    line: InterruptLineId,
    target: InterruptTargetId,
    target_partition: PartitionId,
    source: InterruptSourceId,
    asserted_tick: Tick,
    claimed_tick: Tick,
}

impl InterruptClaim {
    pub const fn new(
        line: InterruptLineId,
        target: InterruptTargetId,
        target_partition: PartitionId,
        source: InterruptSourceId,
        asserted_tick: Tick,
        claimed_tick: Tick,
    ) -> Self {
        Self {
            line,
            target,
            target_partition,
            source,
            asserted_tick,
            claimed_tick,
        }
    }

    pub const fn line(self) -> InterruptLineId {
        self.line
    }

    pub const fn target(self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(self) -> PartitionId {
        self.target_partition
    }

    pub const fn source(self) -> InterruptSourceId {
        self.source
    }

    pub const fn asserted_tick(self) -> Tick {
        self.asserted_tick
    }

    pub const fn claimed_tick(self) -> Tick {
        self.claimed_tick
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
    priorities: BTreeMap<InterruptLineId, InterruptPriority>,
    pending: BTreeMap<InterruptLineId, PendingInterrupt>,
    claimed: BTreeMap<(InterruptTargetId, PartitionId), InterruptClaim>,
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

        let line = route.line();
        self.routes.insert(line, route);
        self.priorities.insert(line, InterruptPriority::DEFAULT);
        Ok(())
    }

    pub fn route(&self, line: InterruptLineId) -> Option<&InterruptRoute> {
        self.routes.get(&line)
    }

    pub fn priority(&self, line: InterruptLineId) -> Result<InterruptPriority, InterruptError> {
        self.route_for(line)?;
        Ok(self.priority_for(line))
    }

    pub fn set_priority(
        &mut self,
        line: InterruptLineId,
        priority: InterruptPriority,
    ) -> Result<(), InterruptError> {
        self.route_for(line)?;
        self.priorities.insert(line, priority);
        Ok(())
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
            InterruptEventKind::Claim | InterruptEventKind::Complete => {
                Err(InterruptError::NonSignalDelivery {
                    kind: delivery.kind(),
                })
            }
        }
    }

    pub fn pending(&self) -> Vec<PendingInterrupt> {
        let mut pending = self.pending.values().cloned().collect::<Vec<_>>();
        pending.sort_by_key(|entry| (entry.target_partition(), entry.target(), entry.line()));
        pending
    }

    pub fn peek_claimable(
        &self,
        target: InterruptTargetId,
        target_partition: PartitionId,
    ) -> Option<PendingInterrupt> {
        self.pending
            .values()
            .filter(|pending| {
                pending.target() == target && pending.target_partition() == target_partition
            })
            .filter(|pending| self.priority_for(pending.line()).get() > 0)
            .min_by_key(|pending| (Reverse(self.priority_for(pending.line())), pending.line()))
            .cloned()
    }

    pub fn claim(
        &mut self,
        target: InterruptTargetId,
        target_partition: PartitionId,
        tick: Tick,
    ) -> Option<InterruptClaim> {
        let key = (target, target_partition);
        if let Some(claim) = self.claimed.get(&key) {
            return Some(*claim);
        }

        let pending = self.peek_claimable(target, target_partition)?;
        self.pending.remove(&pending.line());
        let claim = InterruptClaim::new(
            pending.line(),
            target,
            target_partition,
            pending.source(),
            pending.asserted_tick(),
            tick,
        );
        self.claimed.insert(key, claim);
        self.history.push(InterruptEvent::routed(
            tick,
            pending.line(),
            target,
            target_partition,
            pending.source(),
            InterruptEventKind::Claim,
        ));
        Some(claim)
    }

    pub fn complete(
        &mut self,
        target: InterruptTargetId,
        target_partition: PartitionId,
        line: InterruptLineId,
        tick: Tick,
    ) -> Result<(), InterruptError> {
        let key = (target, target_partition);
        let claimed = self
            .claimed
            .get(&key)
            .ok_or(InterruptError::NoClaimedInterrupt {
                target,
                target_partition,
            })?;
        if claimed.line() != line {
            return Err(InterruptError::ClaimMismatch {
                target,
                target_partition,
                expected: claimed.line(),
                actual: line,
            });
        }

        let claimed = self.claimed.remove(&key).expect("claimed interrupt exists");
        self.history.push(InterruptEvent::routed(
            tick,
            claimed.line(),
            target,
            target_partition,
            claimed.source(),
            InterruptEventKind::Complete,
        ));
        Ok(())
    }

    pub fn claimed(&self) -> Vec<InterruptClaim> {
        let mut claimed = self.claimed.values().copied().collect::<Vec<_>>();
        claimed.sort_by_key(|entry| (entry.target_partition(), entry.target(), entry.line()));
        claimed
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

    fn priority_for(&self, line: InterruptLineId) -> InterruptPriority {
        self.priorities
            .get(&line)
            .copied()
            .unwrap_or(InterruptPriority::DEFAULT)
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

#[derive(Clone, Debug)]
pub struct InterruptControllerMmioDevice {
    controller: Arc<Mutex<InterruptController>>,
    base: Address,
    target: InterruptTargetId,
    target_partition: PartitionId,
}

impl InterruptControllerMmioDevice {
    pub const fn new(
        controller: Arc<Mutex<InterruptController>>,
        base: Address,
        target: InterruptTargetId,
        target_partition: PartitionId,
    ) -> Self {
        Self {
            controller,
            base,
            target,
            target_partition,
        }
    }

    pub fn controller(&self) -> Arc<Mutex<InterruptController>> {
        Arc::clone(&self.controller)
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_at_tick(context.now(), request)
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_at_tick(context.now(), request)
    }

    fn respond_at_tick(
        &self,
        tick: Tick,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        if let Some(line) = self.priority_line(request, offset)? {
            return self.respond_priority(request, line);
        }

        match (offset, request.operation()) {
            (INTERRUPT_MMIO_PENDING_OFFSET, MmioOperation::Read) => {
                let line = self
                    .controller
                    .lock()
                    .expect("interrupt controller lock")
                    .peek_claimable(self.target, self.target_partition)
                    .map(|pending| pending.line().get())
                    .unwrap_or_default();
                Ok(MmioResponse::completed(request.id(), Some(le64(line))))
            }
            (INTERRUPT_MMIO_PENDING_OFFSET, MmioOperation::Write) => Err(MmioError::AccessDenied {
                request: request.id(),
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            }),
            (INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, MmioOperation::Read) => {
                let line = self
                    .controller
                    .lock()
                    .expect("interrupt controller lock")
                    .claim(self.target, self.target_partition, tick)
                    .map(|claim| claim.line().get())
                    .unwrap_or_default();
                Ok(MmioResponse::completed(request.id(), Some(le64(line))))
            }
            (INTERRUPT_MMIO_CLAIM_COMPLETE_OFFSET, MmioOperation::Write) => {
                let line = self.line_from_write(request)?;
                self.controller
                    .lock()
                    .expect("interrupt controller lock")
                    .complete(self.target, self.target_partition, line, tick)
                    .map_err(|error| MmioError::DeviceError {
                        request: request.id(),
                        message: error.to_string(),
                    })?;
                Ok(MmioResponse::completed(request.id(), None))
            }
            _ => Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            }),
        }
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        if request.size().bytes() != INTERRUPT_MMIO_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: INTERRUPT_MMIO_REGISTER_BYTES,
                actual: request.size().bytes(),
            });
        }
        Ok(())
    }

    fn offset(&self, request: &MmioRequest) -> Result<u64, MmioError> {
        request
            .range()
            .start()
            .get()
            .checked_sub(self.base.get())
            .ok_or(MmioError::UnmappedAddress {
                address: request.range().start(),
            })
    }

    fn priority_line(
        &self,
        request: &MmioRequest,
        offset: u64,
    ) -> Result<Option<InterruptLineId>, MmioError> {
        if offset < INTERRUPT_MMIO_PRIORITY_BASE_OFFSET {
            return Ok(None);
        }

        let window_offset = offset - INTERRUPT_MMIO_PRIORITY_BASE_OFFSET;
        if !window_offset.is_multiple_of(INTERRUPT_MMIO_PRIORITY_STRIDE) {
            return Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            });
        }

        Ok(Some(InterruptLineId::new(
            window_offset / INTERRUPT_MMIO_PRIORITY_STRIDE,
        )))
    }

    fn respond_priority(
        &self,
        request: &MmioRequest,
        line: InterruptLineId,
    ) -> Result<MmioResponse, MmioError> {
        match request.operation() {
            MmioOperation::Read => {
                let priority = self
                    .controller
                    .lock()
                    .expect("interrupt controller lock")
                    .priority(line)
                    .map_err(|error| MmioError::DeviceError {
                        request: request.id(),
                        message: error.to_string(),
                    })?;
                Ok(MmioResponse::completed(
                    request.id(),
                    Some(le64(u64::from(priority.get()))),
                ))
            }
            MmioOperation::Write => {
                let priority = self.priority_from_write(request)?;
                self.controller
                    .lock()
                    .expect("interrupt controller lock")
                    .set_priority(line, priority)
                    .map_err(|error| MmioError::DeviceError {
                        request: request.id(),
                        message: error.to_string(),
                    })?;
                Ok(MmioResponse::completed(request.id(), None))
            }
        }
    }

    fn line_from_write(&self, request: &MmioRequest) -> Result<InterruptLineId, MmioError> {
        Ok(InterruptLineId::new(self.value_from_write(request)?))
    }

    fn priority_from_write(&self, request: &MmioRequest) -> Result<InterruptPriority, MmioError> {
        let value = self.value_from_write(request)?;
        let priority = u32::try_from(value).map_err(|_| MmioError::DeviceError {
            request: request.id(),
            message: format!("interrupt priority {value} does not fit u32"),
        })?;
        Ok(InterruptPriority::new(priority))
    }

    fn value_from_write(&self, request: &MmioRequest) -> Result<u64, MmioError> {
        let data = request.data().ok_or(MmioError::MissingWriteData {
            request: request.id(),
        })?;
        if data.len() as u64 != INTERRUPT_MMIO_REGISTER_BYTES {
            return Err(MmioError::PayloadSizeMismatch {
                request: request.id(),
                expected: INTERRUPT_MMIO_REGISTER_BYTES,
                actual: data.len() as u64,
            });
        }
        let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
            request: request.id(),
        })?;
        validate_interrupt_mmio_mask(request, mask)?;

        let mut bytes = [0; 8];
        for (index, byte) in data.iter().enumerate() {
            if mask.bits()[index] {
                bytes[index] = *byte;
            }
        }
        Ok(u64::from_le_bytes(bytes))
    }
}

impl MmioDevice for InterruptControllerMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        InterruptControllerMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        InterruptControllerMmioDevice::respond_parallel(self, context, request)
    }
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn validate_interrupt_mmio_mask(request: &MmioRequest, mask: &ByteMask) -> Result<(), MmioError> {
    if mask.len() != INTERRUPT_MMIO_REGISTER_BYTES {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected: INTERRUPT_MMIO_REGISTER_BYTES,
            actual: mask.len(),
        });
    }
    Ok(())
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
    NoClaimedInterrupt {
        target: InterruptTargetId,
        target_partition: PartitionId,
    },
    ClaimMismatch {
        target: InterruptTargetId,
        target_partition: PartitionId,
        expected: InterruptLineId,
        actual: InterruptLineId,
    },
    NonSignalDelivery {
        kind: InterruptEventKind,
    },
    Scheduler(SchedulerError),
}

impl fmt::Display for InterruptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSignalLatency => {
                write!(formatter, "interrupt signal latency must be positive")
            }
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
                "interrupt line {} delivery route targets partition {} target {}, \
                 expected partition {} target {}",
                line.get(),
                actual.target_partition().index(),
                actual.target().get(),
                expected.target_partition().index(),
                expected.target().get()
            ),
            Self::NoClaimedInterrupt {
                target,
                target_partition,
            } => write!(
                formatter,
                "target {} partition {} has no claimed interrupt",
                target.get(),
                target_partition.index()
            ),
            Self::ClaimMismatch {
                target,
                target_partition,
                expected,
                actual,
            } => write!(
                formatter,
                "target {} partition {} claimed line {}, not line {}",
                target.get(),
                target_partition.index(),
                expected.get(),
                actual.get()
            ),
            Self::NonSignalDelivery { kind } => {
                write!(formatter, "{kind:?} is not a signal delivery event")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for InterruptError {}
