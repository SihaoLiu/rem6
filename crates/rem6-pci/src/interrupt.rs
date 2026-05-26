use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptLineChannel, InterruptLineId, InterruptLinePort,
    InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, SchedulerContext, Tick,
};

use crate::{PciError, PciFunctionAddress, PciInterruptPin};

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
