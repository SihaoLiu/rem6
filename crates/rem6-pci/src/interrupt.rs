use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptLineChannel, InterruptLineId, InterruptLinePort,
    InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, SchedulerContext, Tick,
};

use crate::{PciEndpointConfig, PciError, PciFunctionAddress, PciHostBridge, PciInterruptPin};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PciLegacyInterruptPolicy {
    DeviceModulo,
    PinModulo,
    DevicePinModulo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciLegacyInterruptMapper {
    base_line: InterruptLineId,
    line_count: u32,
    policy: PciLegacyInterruptPolicy,
}

impl PciLegacyInterruptMapper {
    pub const fn new(
        base_line: InterruptLineId,
        line_count: u32,
        policy: PciLegacyInterruptPolicy,
    ) -> Result<Self, PciError> {
        if line_count == 0 {
            return Err(PciError::ZeroLegacyInterruptLines);
        }

        Ok(Self {
            base_line,
            line_count,
            policy,
        })
    }

    pub const fn base_line(self) -> InterruptLineId {
        self.base_line
    }

    pub const fn line_count(self) -> u32 {
        self.line_count
    }

    pub const fn policy(self) -> PciLegacyInterruptPolicy {
        self.policy
    }

    pub fn line(
        self,
        function: PciFunctionAddress,
        pin: PciInterruptPin,
    ) -> Result<InterruptLineId, PciError> {
        let pin_index = legacy_pin_index(function, pin)?;
        let line_count = self.line_count as u64;
        let index = match self.policy {
            PciLegacyInterruptPolicy::DeviceModulo => function.device() as u64 % line_count,
            PciLegacyInterruptPolicy::PinModulo => (pin_index - 1) % line_count,
            PciLegacyInterruptPolicy::DevicePinModulo => {
                (function.device() as u64 + pin_index - 1) % line_count
            }
        };
        self.base_line
            .get()
            .checked_add(index)
            .map(InterruptLineId::new)
            .ok_or(PciError::LegacyInterruptLineOverflow {
                base: self.base_line,
                index,
            })
    }

    pub fn line_for_path(self, path: &PciLegacyInterruptPath) -> Result<InterruptLineId, PciError> {
        self.line(path.root_function(), path.root_pin())
    }

    pub fn route(
        self,
        function: PciFunctionAddress,
        pin: PciInterruptPin,
        target: InterruptTargetId,
        target_partition: PartitionId,
        signal_latency: Tick,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        let line = self.line(function, pin)?;
        PciLegacyInterruptRoute::new(
            function,
            pin,
            InterruptRoute::new(line, target, target_partition),
            signal_latency,
        )
    }

    pub fn route_for_path(
        self,
        path: &PciLegacyInterruptPath,
        target: InterruptTargetId,
        target_partition: PartitionId,
        signal_latency: Tick,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        let line = self.line_for_path(path)?;
        PciLegacyInterruptRoute::new(
            path.endpoint_function(),
            path.endpoint_pin(),
            InterruptRoute::new(line, target, target_partition),
            signal_latency,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciLegacyInterruptRoutingEntry {
    function: PciFunctionAddress,
    pin: PciInterruptPin,
    line: InterruptLineId,
}

impl PciLegacyInterruptRoutingEntry {
    pub fn new(
        function: PciFunctionAddress,
        pin: PciInterruptPin,
        line: InterruptLineId,
    ) -> Result<Self, PciError> {
        legacy_pin_index(function, pin)?;
        Ok(Self {
            function,
            pin,
            line,
        })
    }

    pub const fn function(self) -> PciFunctionAddress {
        self.function
    }

    pub const fn pin(self) -> PciInterruptPin {
        self.pin
    }

    pub const fn line(self) -> InterruptLineId {
        self.line
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciLegacyInterruptRoutingTableSnapshot {
    fallback: PciLegacyInterruptMapper,
    entries: Vec<PciLegacyInterruptRoutingEntry>,
}

impl PciLegacyInterruptRoutingTableSnapshot {
    pub fn new(
        fallback: PciLegacyInterruptMapper,
        entries: Vec<PciLegacyInterruptRoutingEntry>,
    ) -> Result<Self, PciError> {
        let table = PciLegacyInterruptRoutingTable::from_entries(fallback, entries)?;
        Ok(table.snapshot())
    }

    pub const fn fallback(&self) -> PciLegacyInterruptMapper {
        self.fallback
    }

    pub fn entries(&self) -> &[PciLegacyInterruptRoutingEntry] {
        &self.entries
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciLegacyInterruptRoutingTable {
    fallback: PciLegacyInterruptMapper,
    entries: Vec<PciLegacyInterruptRoutingEntry>,
}

impl PciLegacyInterruptRoutingTable {
    pub fn new(fallback: PciLegacyInterruptMapper) -> Self {
        Self {
            fallback,
            entries: Vec::new(),
        }
    }

    pub fn from_entries(
        fallback: PciLegacyInterruptMapper,
        entries: Vec<PciLegacyInterruptRoutingEntry>,
    ) -> Result<Self, PciError> {
        let mut table = Self::new(fallback);
        for entry in entries {
            table.insert_entry(entry)?;
        }
        Ok(table)
    }

    pub fn with_entry(mut self, entry: PciLegacyInterruptRoutingEntry) -> Result<Self, PciError> {
        self.insert_entry(entry)?;
        Ok(self)
    }

    pub fn insert_entry(&mut self, entry: PciLegacyInterruptRoutingEntry) -> Result<(), PciError> {
        if self
            .entries
            .iter()
            .any(|existing| existing.function == entry.function && existing.pin == entry.pin)
        {
            return Err(PciError::DuplicateLegacyInterruptRoutingEntry {
                function: entry.function,
                pin: entry.pin,
            });
        }

        self.entries.push(entry);
        self.sort_entries();
        Ok(())
    }

    pub const fn fallback(&self) -> PciLegacyInterruptMapper {
        self.fallback
    }

    pub fn entries(&self) -> &[PciLegacyInterruptRoutingEntry] {
        &self.entries
    }

    pub fn snapshot(&self) -> PciLegacyInterruptRoutingTableSnapshot {
        PciLegacyInterruptRoutingTableSnapshot {
            fallback: self.fallback,
            entries: self.entries.clone(),
        }
    }

    pub fn restore(&mut self, snapshot: &PciLegacyInterruptRoutingTableSnapshot) {
        self.fallback = snapshot.fallback;
        self.entries = snapshot.entries.clone();
    }

    pub fn line(
        &self,
        function: PciFunctionAddress,
        pin: PciInterruptPin,
    ) -> Result<InterruptLineId, PciError> {
        legacy_pin_index(function, pin)?;
        self.entries
            .iter()
            .find(|entry| entry.function == function && entry.pin == pin)
            .map(|entry| entry.line)
            .map_or_else(|| self.fallback.line(function, pin), Ok)
    }

    pub fn line_for_path(
        &self,
        path: &PciLegacyInterruptPath,
    ) -> Result<InterruptLineId, PciError> {
        self.line(path.root_function(), path.root_pin())
    }

    pub fn route(
        &self,
        function: PciFunctionAddress,
        pin: PciInterruptPin,
        target: InterruptTargetId,
        target_partition: PartitionId,
        signal_latency: Tick,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        let line = self.line(function, pin)?;
        PciLegacyInterruptRoute::new(
            function,
            pin,
            InterruptRoute::new(line, target, target_partition),
            signal_latency,
        )
    }

    pub fn route_for_path(
        &self,
        path: &PciLegacyInterruptPath,
        target: InterruptTargetId,
        target_partition: PartitionId,
        signal_latency: Tick,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        let line = self.line_for_path(path)?;
        PciLegacyInterruptRoute::new(
            path.endpoint_function(),
            path.endpoint_pin(),
            InterruptRoute::new(line, target, target_partition),
            signal_latency,
        )
    }

    fn sort_entries(&mut self) {
        self.entries.sort_by_key(|entry| {
            (
                entry.function,
                legacy_pin_index_unchecked(entry.pin),
                entry.line.get(),
            )
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciLegacyInterruptRouterSnapshot {
    target: InterruptTargetId,
    target_partition: PartitionId,
    signal_latency: Tick,
    routing_table: PciLegacyInterruptRoutingTableSnapshot,
}

impl PciLegacyInterruptRouterSnapshot {
    pub fn new(
        target: InterruptTargetId,
        target_partition: PartitionId,
        signal_latency: Tick,
        routing_table: PciLegacyInterruptRoutingTableSnapshot,
    ) -> Result<Self, PciError> {
        InterruptLineChannel::new(
            InterruptRoute::new(
                routing_table.fallback().base_line(),
                target,
                target_partition,
            ),
            signal_latency,
        )
        .map_err(PciError::Interrupt)?;

        Ok(Self {
            target,
            target_partition,
            signal_latency,
            routing_table,
        })
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub const fn signal_latency(&self) -> Tick {
        self.signal_latency
    }

    pub const fn routing_table(&self) -> &PciLegacyInterruptRoutingTableSnapshot {
        &self.routing_table
    }
}

#[derive(Clone, Debug)]
pub struct PciLegacyInterruptRouter {
    routing_table: PciLegacyInterruptRoutingTable,
    target: InterruptTargetId,
    target_partition: PartitionId,
    signal_latency: Tick,
    controller: Arc<Mutex<InterruptController>>,
}

impl PciLegacyInterruptRouter {
    pub fn new(
        routing_table: PciLegacyInterruptRoutingTable,
        target: InterruptTargetId,
        target_partition: PartitionId,
        signal_latency: Tick,
        controller: Arc<Mutex<InterruptController>>,
    ) -> Result<Self, PciError> {
        InterruptLineChannel::new(
            InterruptRoute::new(
                routing_table.fallback().base_line(),
                target,
                target_partition,
            ),
            signal_latency,
        )
        .map_err(PciError::Interrupt)?;

        Ok(Self {
            routing_table,
            target,
            target_partition,
            signal_latency,
            controller,
        })
    }

    pub const fn routing_table(&self) -> &PciLegacyInterruptRoutingTable {
        &self.routing_table
    }

    pub const fn target(&self) -> InterruptTargetId {
        self.target
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub const fn signal_latency(&self) -> Tick {
        self.signal_latency
    }

    pub fn controller(&self) -> Arc<Mutex<InterruptController>> {
        Arc::clone(&self.controller)
    }

    pub fn insert_entry(&mut self, entry: PciLegacyInterruptRoutingEntry) -> Result<(), PciError> {
        self.routing_table.insert_entry(entry)
    }

    pub fn snapshot(&self) -> PciLegacyInterruptRouterSnapshot {
        PciLegacyInterruptRouterSnapshot {
            target: self.target,
            target_partition: self.target_partition,
            signal_latency: self.signal_latency,
            routing_table: self.routing_table.snapshot(),
        }
    }

    pub fn restore(&mut self, snapshot: &PciLegacyInterruptRouterSnapshot) -> Result<(), PciError> {
        if self.target != snapshot.target
            || self.target_partition != snapshot.target_partition
            || self.signal_latency != snapshot.signal_latency
        {
            return Err(PciError::SnapshotLegacyInterruptRouterMismatch);
        }

        self.routing_table.restore(snapshot.routing_table());
        Ok(())
    }

    pub fn line(
        &self,
        function: PciFunctionAddress,
        pin: PciInterruptPin,
    ) -> Result<InterruptLineId, PciError> {
        self.routing_table.line(function, pin)
    }

    pub fn line_for_path(
        &self,
        path: &PciLegacyInterruptPath,
    ) -> Result<InterruptLineId, PciError> {
        self.routing_table.line_for_path(path)
    }

    pub fn route(
        &self,
        function: PciFunctionAddress,
        pin: PciInterruptPin,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        self.routing_table.route(
            function,
            pin,
            self.target,
            self.target_partition,
            self.signal_latency,
        )
    }

    pub fn route_for_path(
        &self,
        path: &PciLegacyInterruptPath,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        self.routing_table.route_for_path(
            path,
            self.target,
            self.target_partition,
            self.signal_latency,
        )
    }

    pub fn route_for_endpoint(
        &self,
        endpoint: &PciEndpointConfig,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        self.route_for_path(&endpoint.legacy_interrupt_path()?)
    }

    pub fn route_for_host_endpoint(
        &self,
        host: &PciHostBridge,
        function: PciFunctionAddress,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        self.route_for_path(&host.legacy_interrupt_path(function)?)
    }

    pub fn port(
        &self,
        function: PciFunctionAddress,
        pin: PciInterruptPin,
    ) -> Result<PciLegacyInterruptPort, PciError> {
        let route = self.route(function, pin)?;
        self.port_for_route(route)
    }

    pub fn port_for_path(
        &self,
        path: &PciLegacyInterruptPath,
    ) -> Result<PciLegacyInterruptPort, PciError> {
        let route = self.route_for_path(path)?;
        self.port_for_route(route)
    }

    pub fn port_for_endpoint(
        &self,
        endpoint: &PciEndpointConfig,
    ) -> Result<PciLegacyInterruptPort, PciError> {
        let route = self.route_for_endpoint(endpoint)?;
        self.port_for_route(route)
    }

    pub fn port_for_host_endpoint(
        &self,
        host: &PciHostBridge,
        function: PciFunctionAddress,
    ) -> Result<PciLegacyInterruptPort, PciError> {
        let route = self.route_for_host_endpoint(host, function)?;
        self.port_for_route(route)
    }

    pub fn assign_endpoint_interrupt_line(
        &self,
        endpoint: &mut PciEndpointConfig,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        let route = self.route_for_endpoint(endpoint)?;
        endpoint.assign_legacy_interrupt_line(route.line())?;
        Ok(route)
    }

    pub fn assign_host_endpoint_interrupt_line(
        &self,
        host: &mut PciHostBridge,
        function: PciFunctionAddress,
    ) -> Result<PciLegacyInterruptRoute, PciError> {
        let route = self.route_for_host_endpoint(host, function)?;
        host.assign_legacy_interrupt_line(function, route.line())?;
        Ok(route)
    }

    fn port_for_route(
        &self,
        route: PciLegacyInterruptRoute,
    ) -> Result<PciLegacyInterruptPort, PciError> {
        self.register_route(route.interrupt_route())?;
        PciLegacyInterruptPort::new(route, Arc::clone(&self.controller))
    }

    fn register_route(&self, route: InterruptRoute) -> Result<(), PciError> {
        let mut controller = self.controller.lock().unwrap();
        if controller.route(route.line()).copied() == Some(route) {
            return Ok(());
        }

        controller
            .register_route(route)
            .map_err(PciError::Interrupt)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciLegacyInterruptPath {
    endpoint_function: PciFunctionAddress,
    endpoint_pin: PciInterruptPin,
    root_function: PciFunctionAddress,
    root_pin: PciInterruptPin,
    upstream_bridges: Vec<PciFunctionAddress>,
}

impl PciLegacyInterruptPath {
    pub fn new(
        endpoint_function: PciFunctionAddress,
        endpoint_pin: PciInterruptPin,
    ) -> Result<Self, PciError> {
        legacy_pin_index(endpoint_function, endpoint_pin)?;
        Ok(Self {
            endpoint_function,
            endpoint_pin,
            root_function: endpoint_function,
            root_pin: endpoint_pin,
            upstream_bridges: Vec::new(),
        })
    }

    pub fn with_upstream_bridge(mut self, bridge_function: PciFunctionAddress) -> Self {
        self.root_pin = swizzle_pin(self.root_pin, self.root_function.device());
        self.root_function = bridge_function;
        self.upstream_bridges.push(bridge_function);
        self
    }

    pub const fn endpoint_function(&self) -> PciFunctionAddress {
        self.endpoint_function
    }

    pub const fn endpoint_pin(&self) -> PciInterruptPin {
        self.endpoint_pin
    }

    pub const fn root_function(&self) -> PciFunctionAddress {
        self.root_function
    }

    pub const fn root_pin(&self) -> PciInterruptPin {
        self.root_pin
    }

    pub fn upstream_bridges(&self) -> &[PciFunctionAddress] {
        &self.upstream_bridges
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PciLegacyInterruptRoute {
    function: PciFunctionAddress,
    pin: PciInterruptPin,
    interrupt_route: InterruptRoute,
    signal_latency: Tick,
}

impl PciLegacyInterruptRoute {
    pub fn new(
        function: PciFunctionAddress,
        pin: PciInterruptPin,
        interrupt_route: InterruptRoute,
        signal_latency: Tick,
    ) -> Result<Self, PciError> {
        legacy_pin_index(function, pin)?;
        InterruptLineChannel::new(interrupt_route, signal_latency).map_err(PciError::Interrupt)?;

        Ok(Self {
            function,
            pin,
            interrupt_route,
            signal_latency,
        })
    }

    pub const fn function(self) -> PciFunctionAddress {
        self.function
    }

    pub const fn pin(self) -> PciInterruptPin {
        self.pin
    }

    pub const fn interrupt_route(self) -> InterruptRoute {
        self.interrupt_route
    }

    pub const fn line(self) -> InterruptLineId {
        self.interrupt_route.line()
    }

    pub const fn target(self) -> InterruptTargetId {
        self.interrupt_route.target()
    }

    pub const fn target_partition(self) -> PartitionId {
        self.interrupt_route.target_partition()
    }

    pub const fn signal_latency(self) -> Tick {
        self.signal_latency
    }

    fn channel(self) -> Result<InterruptLineChannel, PciError> {
        InterruptLineChannel::new(self.interrupt_route, self.signal_latency)
            .map_err(PciError::Interrupt)
    }
}

#[derive(Clone, Debug)]
pub struct PciLegacyInterruptPort {
    route: PciLegacyInterruptRoute,
    port: InterruptLinePort,
}

impl PciLegacyInterruptPort {
    pub fn new(
        route: PciLegacyInterruptRoute,
        controller: Arc<Mutex<InterruptController>>,
    ) -> Result<Self, PciError> {
        let channel = route.channel()?;
        Ok(Self {
            route,
            port: InterruptLinePort::new(channel, controller),
        })
    }

    pub const fn route(&self) -> PciLegacyInterruptRoute {
        self.route
    }

    pub const fn function(&self) -> PciFunctionAddress {
        self.route.function()
    }

    pub const fn pin(&self) -> PciInterruptPin {
        self.route.pin()
    }

    pub const fn interrupt_route(&self) -> InterruptRoute {
        self.route.interrupt_route()
    }

    pub const fn line(&self) -> InterruptLineId {
        self.route.line()
    }

    pub const fn signal_latency(&self) -> Tick {
        self.route.signal_latency()
    }

    pub fn controller(&self) -> Arc<Mutex<InterruptController>> {
        self.port.controller()
    }

    pub fn delivery_errors(&self) -> Arc<Mutex<Vec<InterruptError>>> {
        self.port.delivery_errors()
    }

    pub fn post(
        &self,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<PartitionEventId, PciError> {
        self.port
            .assert(context, source)
            .map_err(PciError::Interrupt)
    }

    pub fn clear(
        &self,
        context: &mut SchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<PartitionEventId, PciError> {
        self.port
            .deassert(context, source)
            .map_err(PciError::Interrupt)
    }

    pub fn post_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<PartitionEventId, PciError> {
        self.port
            .assert_parallel(context, source)
            .map_err(PciError::Interrupt)
    }

    pub fn clear_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        source: InterruptSourceId,
    ) -> Result<PartitionEventId, PciError> {
        self.port
            .deassert_parallel(context, source)
            .map_err(PciError::Interrupt)
    }
}

fn legacy_pin_index(function: PciFunctionAddress, pin: PciInterruptPin) -> Result<u64, PciError> {
    match pin {
        PciInterruptPin::None => Err(PciError::MissingLegacyInterruptPin { function }),
        PciInterruptPin::IntA => Ok(1),
        PciInterruptPin::IntB => Ok(2),
        PciInterruptPin::IntC => Ok(3),
        PciInterruptPin::IntD => Ok(4),
    }
}

fn swizzle_pin(pin: PciInterruptPin, device: u8) -> PciInterruptPin {
    let index = legacy_pin_index_unchecked(pin);
    pin_from_legacy_index(((index - 1 + u64::from(device)) % 4) + 1)
}

const fn legacy_pin_index_unchecked(pin: PciInterruptPin) -> u64 {
    match pin {
        PciInterruptPin::None => 0,
        PciInterruptPin::IntA => 1,
        PciInterruptPin::IntB => 2,
        PciInterruptPin::IntC => 3,
        PciInterruptPin::IntD => 4,
    }
}

fn pin_from_legacy_index(index: u64) -> PciInterruptPin {
    match index {
        1 => PciInterruptPin::IntA,
        2 => PciInterruptPin::IntB,
        3 => PciInterruptPin::IntC,
        4 => PciInterruptPin::IntD,
        _ => unreachable!("legacy INTx pin index is modulo 1..=4"),
    }
}
