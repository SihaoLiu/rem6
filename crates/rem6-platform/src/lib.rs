use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptLineChannel, InterruptLineId, InterruptLinePort,
    InterruptRoute, InterruptSourceId, InterruptTargetId, PlicContextRoute, PlicMmioDevice,
};
use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, MemoryError};
use rem6_mmio::{MmioBus, MmioError, MmioRoute};
use rem6_timer::{
    ClintHartConfig, ClintId, ClintMmioDevice, ClintResetPolicy, CpuLocalTimerBank,
    CpuLocalTimerError, CpuLocalTimerInterruptPorts, CpuLocalTimerMmioDevice, Mc146818Rtc,
    Mc146818RtcMmioDevice, Pl031Error, Pl031Rtc, Pl031RtcMmioDevice, ProgrammableTimer,
    RtcDateTime, RtcEncoding, RtcError, Sp804DualTimer, Sp804DualTimerMmioDevice, Sp804Error,
    Sp805Error, Sp805Watchdog, Sp805WatchdogMmioDevice, TimerError, TimerId, TimerMmioDevice,
};
use rem6_topology::{Endpoint, Topology, TopologyError};
use rem6_uart::{Pl011UartMmioDevice, UartId, UartMmioDevice};

mod device_tree;

use self::device_tree::PlatformDeviceTreeInventory;
pub use self::device_tree::{
    PlatformDeviceTree, PlatformDeviceTreeNode, PlatformDeviceTreeProperty,
    PlatformDeviceTreePropertyValue, PlatformRiscvDeviceTreeConfig,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformTimerConfig {
    pub id: TimerId,
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub interrupt_line: InterruptLineId,
    pub interrupt_target: InterruptTargetId,
    pub interrupt_source: InterruptSourceId,
    pub interrupt_latency: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformUartConfig {
    pub id: UartId,
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub interrupt_line: InterruptLineId,
    pub interrupt_target: InterruptTargetId,
    pub interrupt_source: InterruptSourceId,
    pub interrupt_latency: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformPl011UartConfig {
    pub id: UartId,
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub interrupt: Option<PlatformPl011UartInterruptConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformPl011UartInterruptConfig {
    pub line: InterruptLineId,
    pub target: InterruptTargetId,
    pub source: InterruptSourceId,
    pub latency: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformRtcConfig {
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub time: RtcDateTime,
    pub encoding: RtcEncoding,
    pub periodic_interrupt: Option<PlatformRtcPeriodicInterruptConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformRtcPeriodicInterruptConfig {
    pub line: InterruptLineId,
    pub target: InterruptTargetId,
    pub source: InterruptSourceId,
    pub latency: Tick,
    pub interval: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformPl031RtcConfig {
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub initial_time: u32,
    pub ticks_per_second: Tick,
    pub interrupt: Option<PlatformPl031RtcInterruptConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformPl031RtcInterruptConfig {
    pub line: InterruptLineId,
    pub target: InterruptTargetId,
    pub source: InterruptSourceId,
    pub latency: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformSp804TimerConfig {
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub clock0: Tick,
    pub clock1: Tick,
    pub interrupts: Option<[PlatformSp804TimerInterruptConfig; 2]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformSp804TimerInterruptConfig {
    pub line: InterruptLineId,
    pub target: InterruptTargetId,
    pub source: InterruptSourceId,
    pub latency: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformSp805WatchdogConfig {
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub clock_tick: Tick,
    pub interrupt: Option<PlatformSp805WatchdogInterruptConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformSp805WatchdogInterruptConfig {
    pub line: InterruptLineId,
    pub target: InterruptTargetId,
    pub source: InterruptSourceId,
    pub latency: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformCpuLocalTimerConfig {
    pub base: Address,
    pub size: AccessSize,
    pub routes: Vec<MmioRoute>,
    pub clock_tick: Tick,
    pub cpus: Vec<PlatformCpuLocalTimerCpuConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformCpuLocalTimerCpuConfig {
    pub partition: PartitionId,
    pub timer: PlatformCpuLocalTimerInterruptConfig,
    pub watchdog: PlatformCpuLocalTimerInterruptConfig,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformCpuLocalTimerInterruptConfig {
    pub line: InterruptLineId,
    pub target: InterruptTargetId,
    pub source: InterruptSourceId,
    pub latency: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformClintHartConfig {
    pub hart: u32,
    pub target_partition: PartitionId,
    pub interrupt_target: InterruptTargetId,
    pub software_interrupt_line: InterruptLineId,
    pub software_interrupt_source: InterruptSourceId,
    pub timer_interrupt_line: InterruptLineId,
    pub timer_interrupt_source: InterruptSourceId,
    pub interrupt_latency: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformClintConfig {
    pub id: ClintId,
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub reset_policy: ClintResetPolicy,
    pub harts: Vec<PlatformClintHartConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformInterruptControllerContextConfig {
    pub context: u64,
    pub hart: u32,
    pub interrupt: u32,
    pub target: InterruptTargetId,
    pub target_partition: PartitionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformInterruptControllerConfig {
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub target: InterruptTargetId,
    pub source_count: u32,
    pub contexts: Vec<PlatformInterruptControllerContextConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformTopologyRoute {
    source: Endpoint,
    target: Endpoint,
}

impl PlatformTopologyRoute {
    pub fn new(source: Endpoint, target: Endpoint) -> Self {
        Self { source, target }
    }

    pub const fn source(&self) -> &Endpoint {
        &self.source
    }

    pub const fn target(&self) -> &Endpoint {
        &self.target
    }

    pub fn resolve(&self, topology: &Topology) -> Result<MmioRoute, PlatformTopologyError> {
        let source_partition = endpoint_partition(topology, &self.source)?;
        let target_partition = endpoint_partition(topology, &self.target)?;
        let path = topology
            .find_endpoint_path(&self.source, &self.target)
            .ok_or_else(|| PlatformTopologyError::MissingPath {
                source: self.source.clone(),
                target: self.target.clone(),
            })?;

        MmioRoute::new(
            source_partition,
            target_partition,
            path.request_latency(),
            path.response_latency(),
        )
        .map_err(PlatformTopologyError::Mmio)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlatformBuilder {
    partition_count: u32,
    interrupt_controllers: Vec<PlatformInterruptControllerConfig>,
    clints: Vec<PlatformClintConfig>,
    timers: Vec<PlatformTimerConfig>,
    uarts: Vec<PlatformUartConfig>,
    pl011_uarts: Vec<PlatformPl011UartConfig>,
    rtcs: Vec<PlatformRtcConfig>,
    pl031_rtcs: Vec<PlatformPl031RtcConfig>,
    sp804_timers: Vec<PlatformSp804TimerConfig>,
    sp805_watchdogs: Vec<PlatformSp805WatchdogConfig>,
    cpu_local_timers: Vec<PlatformCpuLocalTimerConfig>,
}

impl PlatformBuilder {
    pub const fn new(partition_count: u32) -> Self {
        Self {
            partition_count,
            interrupt_controllers: Vec::new(),
            clints: Vec::new(),
            timers: Vec::new(),
            uarts: Vec::new(),
            pl011_uarts: Vec::new(),
            rtcs: Vec::new(),
            pl031_rtcs: Vec::new(),
            sp804_timers: Vec::new(),
            sp805_watchdogs: Vec::new(),
            cpu_local_timers: Vec::new(),
        }
    }

    pub fn from_topology(topology: &Topology) -> Self {
        Self::new(topology.partition_count())
    }

    pub fn add_interrupt_controller(mut self, config: PlatformInterruptControllerConfig) -> Self {
        self.interrupt_controllers.push(config);
        self
    }

    pub fn add_timer(mut self, config: PlatformTimerConfig) -> Self {
        self.timers.push(config);
        self
    }

    pub fn add_clint(mut self, config: PlatformClintConfig) -> Self {
        self.clints.push(config);
        self
    }

    pub fn add_uart(mut self, config: PlatformUartConfig) -> Self {
        self.uarts.push(config);
        self
    }

    pub fn add_pl011_uart(mut self, config: PlatformPl011UartConfig) -> Self {
        self.pl011_uarts.push(config);
        self
    }

    pub fn add_rtc(mut self, config: PlatformRtcConfig) -> Self {
        self.rtcs.push(config);
        self
    }

    pub fn add_pl031_rtc(mut self, config: PlatformPl031RtcConfig) -> Self {
        self.pl031_rtcs.push(config);
        self
    }

    pub fn add_sp804_timer(mut self, config: PlatformSp804TimerConfig) -> Self {
        self.sp804_timers.push(config);
        self
    }

    pub fn add_sp805_watchdog(mut self, config: PlatformSp805WatchdogConfig) -> Self {
        self.sp805_watchdogs.push(config);
        self
    }

    pub fn add_cpu_local_timer(mut self, config: PlatformCpuLocalTimerConfig) -> Self {
        self.cpu_local_timers.push(config);
        self
    }

    pub fn build(self) -> Result<Platform, PlatformError> {
        if self.partition_count == 0 {
            return Err(PlatformError::NoPartitions);
        }

        let device_tree_inventory = PlatformDeviceTreeInventory::new(
            self.interrupt_controllers.clone(),
            self.clints.clone(),
            self.timers.clone(),
            self.uarts.clone(),
            self.rtcs.clone(),
            self.pl031_rtcs.clone(),
        );
        let controller = Arc::new(Mutex::new(InterruptController::new()));
        let mut bus = MmioBus::new();
        let mut clints = BTreeMap::new();
        let mut plics = BTreeMap::new();
        let mut timers = BTreeMap::new();
        let mut uarts = BTreeMap::new();
        let mut pl011_uarts = BTreeMap::new();
        let mut rtcs = BTreeMap::new();
        let mut pl031_rtcs = BTreeMap::new();
        let mut sp804_timers = BTreeMap::new();
        let mut sp805_watchdogs = BTreeMap::new();
        let mut cpu_local_timers = BTreeMap::new();

        for config in self.interrupt_controllers {
            validate_route(self.partition_count, config.route)?;
            for context in &config.contexts {
                validate_partition(self.partition_count, context.target_partition)?;
            }
            let source_count = device_tree_inventory.max_external_interrupt_source(&config);
            let device = if config.contexts.is_empty() {
                PlicMmioDevice::with_source_count(
                    Arc::clone(&controller),
                    config.base,
                    config.target,
                    config.route.source_partition(),
                    source_count,
                )
            } else {
                PlicMmioDevice::with_contexts_and_source_count(
                    Arc::clone(&controller),
                    config.base,
                    config.contexts.iter().map(|context| {
                        PlicContextRoute::new(
                            context.context,
                            context.target,
                            context.target_partition,
                        )
                    }),
                    source_count,
                )
            };
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            plics.insert(config.base, device);
        }

        for config in self.timers {
            validate_route(self.partition_count, config.route)?;
            let port = register_interrupt(
                &controller,
                config.route.source_partition(),
                config.interrupt_line,
                config.interrupt_target,
                config.interrupt_latency,
            )?;
            let timer = ProgrammableTimer::new(
                config.id,
                config.route.target_partition(),
                config.interrupt_source,
                port,
            );
            let device = TimerMmioDevice::new(timer.clone(), config.base);
            bus.insert_device(region(config.base, config.size)?, config.route, device)
                .map_err(PlatformError::Mmio)?;
            timers.insert(config.id, timer);
        }

        for config in self.clints {
            validate_route(self.partition_count, config.route)?;
            let mut harts = Vec::with_capacity(config.harts.len());
            for hart in config.harts {
                validate_partition(self.partition_count, hart.target_partition)?;
                let software_port = register_interrupt(
                    &controller,
                    hart.target_partition,
                    hart.software_interrupt_line,
                    hart.interrupt_target,
                    hart.interrupt_latency,
                )?;
                let timer_port = register_interrupt(
                    &controller,
                    hart.target_partition,
                    hart.timer_interrupt_line,
                    hart.interrupt_target,
                    hart.interrupt_latency,
                )?;
                harts.push(ClintHartConfig::new(
                    hart.hart,
                    software_port,
                    hart.software_interrupt_source,
                    timer_port,
                    hart.timer_interrupt_source,
                ));
            }
            let device =
                ClintMmioDevice::with_reset_policy(config.base, harts, config.reset_policy)
                    .map_err(PlatformError::Timer)?;
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            clints.insert(config.id, device);
        }

        for config in self.uarts {
            validate_route(self.partition_count, config.route)?;
            let port = register_interrupt(
                &controller,
                config.route.source_partition(),
                config.interrupt_line,
                config.interrupt_target,
                config.interrupt_latency,
            )?;
            let device = UartMmioDevice::with_interrupt(
                config.id,
                config.base,
                config.interrupt_source,
                port,
            );
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            uarts.insert(config.id, device);
        }

        for config in self.pl011_uarts {
            validate_route(self.partition_count, config.route)?;
            let device = if let Some(interrupt) = config.interrupt {
                let port = register_interrupt(
                    &controller,
                    config.route.source_partition(),
                    interrupt.line,
                    interrupt.target,
                    interrupt.latency,
                )?;
                Pl011UartMmioDevice::with_interrupt(config.id, config.base, interrupt.source, port)
            } else {
                Pl011UartMmioDevice::new(config.id, config.base)
            };
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            pl011_uarts.insert(config.base, device);
        }

        for config in self.rtcs {
            validate_route(self.partition_count, config.route)?;
            let rtc = Mc146818Rtc::new(config.time, config.encoding).map_err(PlatformError::Rtc)?;
            let device = if let Some(interrupt) = config.periodic_interrupt {
                let port = register_interrupt(
                    &controller,
                    config.route.source_partition(),
                    interrupt.line,
                    interrupt.target,
                    interrupt.latency,
                )?;
                Mc146818RtcMmioDevice::with_periodic_interrupt(
                    config.base,
                    rtc,
                    config.route.target_partition(),
                    interrupt.source,
                    port,
                    interrupt.interval,
                )
                .map_err(PlatformError::Rtc)?
            } else {
                Mc146818RtcMmioDevice::new(config.base, rtc)
            };
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            rtcs.insert(config.base, device);
        }

        for config in self.pl031_rtcs {
            validate_route(self.partition_count, config.route)?;
            let rtc = Pl031Rtc::new(config.initial_time, config.ticks_per_second)
                .map_err(PlatformError::Pl031)?;
            let device = if let Some(interrupt) = config.interrupt {
                let port = register_interrupt(
                    &controller,
                    config.route.source_partition(),
                    interrupt.line,
                    interrupt.target,
                    interrupt.latency,
                )?;
                Pl031RtcMmioDevice::with_interrupt(
                    config.base,
                    rtc,
                    config.route.target_partition(),
                    interrupt.source,
                    port,
                )
                .map_err(PlatformError::Pl031)?
            } else {
                Pl031RtcMmioDevice::new(config.base, rtc)
            };
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            pl031_rtcs.insert(config.base, device);
        }

        for config in self.sp804_timers {
            validate_route(self.partition_count, config.route)?;
            let timers =
                Sp804DualTimer::new(config.clock0, config.clock1).map_err(PlatformError::Sp804)?;
            let device = if let Some(interrupts) = config.interrupts {
                let [interrupt0, interrupt1] = interrupts;
                let port0 = register_interrupt(
                    &controller,
                    config.route.source_partition(),
                    interrupt0.line,
                    interrupt0.target,
                    interrupt0.latency,
                )?;
                let port1 = register_interrupt(
                    &controller,
                    config.route.source_partition(),
                    interrupt1.line,
                    interrupt1.target,
                    interrupt1.latency,
                )?;
                Sp804DualTimerMmioDevice::with_interrupts(
                    config.base,
                    timers,
                    config.route.target_partition(),
                    [(interrupt0.source, port0), (interrupt1.source, port1)],
                )
                .map_err(PlatformError::Sp804)?
            } else {
                Sp804DualTimerMmioDevice::new(config.base, timers)
            };
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            sp804_timers.insert(config.base, device);
        }

        for config in self.sp805_watchdogs {
            validate_route(self.partition_count, config.route)?;
            let watchdog = Sp805Watchdog::new(config.clock_tick).map_err(PlatformError::Sp805)?;
            let device = if let Some(interrupt) = config.interrupt {
                let port = register_interrupt(
                    &controller,
                    config.route.source_partition(),
                    interrupt.line,
                    interrupt.target,
                    interrupt.latency,
                )?;
                Sp805WatchdogMmioDevice::with_interrupt(
                    config.base,
                    watchdog,
                    config.route.target_partition(),
                    interrupt.source,
                    port,
                )
                .map_err(PlatformError::Sp805)?
            } else {
                Sp805WatchdogMmioDevice::new(config.base, watchdog)
            };
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            sp805_watchdogs.insert(config.base, device);
        }

        for config in self.cpu_local_timers {
            for route in &config.routes {
                validate_route(self.partition_count, *route)?;
            }
            let mut ports = Vec::with_capacity(config.cpus.len());
            for cpu in &config.cpus {
                validate_partition(self.partition_count, cpu.partition)?;
                if !config
                    .routes
                    .iter()
                    .any(|route| route.source_partition() == cpu.partition)
                {
                    return Err(PlatformError::MissingCpuLocalTimerRoute {
                        base: config.base,
                        partition: cpu.partition,
                    });
                }
                let timer_port = register_interrupt(
                    &controller,
                    cpu.partition,
                    cpu.timer.line,
                    cpu.timer.target,
                    cpu.timer.latency,
                )?;
                let watchdog_port = register_interrupt(
                    &controller,
                    cpu.partition,
                    cpu.watchdog.line,
                    cpu.watchdog.target,
                    cpu.watchdog.latency,
                )?;
                ports.push(CpuLocalTimerInterruptPorts::new(
                    cpu.partition,
                    cpu.timer.source,
                    timer_port,
                    cpu.watchdog.source,
                    watchdog_port,
                ));
            }
            let bank = CpuLocalTimerBank::new(config.cpus.len(), config.clock_tick)
                .map_err(PlatformError::CpuLocalTimer)?;
            let device = CpuLocalTimerMmioDevice::with_interrupts(config.base, bank, ports)
                .map_err(PlatformError::CpuLocalTimer)?;
            for route in config.routes {
                bus.insert_device(region(config.base, config.size)?, route, device.clone())
                    .map_err(PlatformError::Mmio)?;
            }
            cpu_local_timers.insert(config.base, device);
        }

        Ok(Platform {
            partition_count: self.partition_count,
            interrupt_controller: controller,
            mmio_bus: bus,
            clints,
            plics,
            timers,
            uarts,
            pl011_uarts,
            rtcs,
            pl031_rtcs,
            sp804_timers,
            sp805_watchdogs,
            cpu_local_timers,
            device_tree_inventory,
        })
    }
}

#[derive(Clone)]
pub struct Platform {
    partition_count: u32,
    interrupt_controller: Arc<Mutex<InterruptController>>,
    mmio_bus: MmioBus,
    clints: BTreeMap<ClintId, ClintMmioDevice>,
    plics: BTreeMap<Address, PlicMmioDevice>,
    timers: BTreeMap<TimerId, ProgrammableTimer>,
    uarts: BTreeMap<UartId, UartMmioDevice>,
    pl011_uarts: BTreeMap<Address, Pl011UartMmioDevice>,
    rtcs: BTreeMap<Address, Mc146818RtcMmioDevice>,
    pl031_rtcs: BTreeMap<Address, Pl031RtcMmioDevice>,
    sp804_timers: BTreeMap<Address, Sp804DualTimerMmioDevice>,
    sp805_watchdogs: BTreeMap<Address, Sp805WatchdogMmioDevice>,
    cpu_local_timers: BTreeMap<Address, CpuLocalTimerMmioDevice>,
    device_tree_inventory: PlatformDeviceTreeInventory,
}

impl Platform {
    pub const fn partition_count(&self) -> u32 {
        self.partition_count
    }

    pub fn interrupt_controller(&self) -> Arc<Mutex<InterruptController>> {
        Arc::clone(&self.interrupt_controller)
    }

    pub const fn mmio_bus(&self) -> &MmioBus {
        &self.mmio_bus
    }

    pub fn clint(&self, id: ClintId) -> Option<&ClintMmioDevice> {
        self.clints.get(&id)
    }

    pub fn clints(&self) -> impl Iterator<Item = (ClintId, &ClintMmioDevice)> {
        self.clints.iter().map(|(id, device)| (*id, device))
    }

    pub fn plic(&self, base: Address) -> Option<&PlicMmioDevice> {
        self.plics.get(&base)
    }

    pub fn plics(&self) -> impl Iterator<Item = (Address, &PlicMmioDevice)> {
        self.plics.iter().map(|(base, device)| (*base, device))
    }

    pub fn timer(&self, id: TimerId) -> Option<&ProgrammableTimer> {
        self.timers.get(&id)
    }

    pub fn timers(&self) -> impl Iterator<Item = (TimerId, &ProgrammableTimer)> {
        self.timers.iter().map(|(id, timer)| (*id, timer))
    }

    pub fn uart(&self, id: UartId) -> Option<&UartMmioDevice> {
        self.uarts.get(&id)
    }

    pub fn uarts(&self) -> impl Iterator<Item = (UartId, &UartMmioDevice)> {
        self.uarts.iter().map(|(id, device)| (*id, device))
    }

    pub fn pl011_uart(&self, base: Address) -> Option<&Pl011UartMmioDevice> {
        self.pl011_uarts.get(&base)
    }

    pub fn pl011_uarts(&self) -> impl Iterator<Item = (Address, &Pl011UartMmioDevice)> {
        self.pl011_uarts
            .iter()
            .map(|(base, device)| (*base, device))
    }

    pub fn rtc(&self, base: Address) -> Option<&Mc146818RtcMmioDevice> {
        self.rtcs.get(&base)
    }

    pub fn rtcs(&self) -> impl Iterator<Item = (Address, &Mc146818RtcMmioDevice)> {
        self.rtcs.iter().map(|(base, device)| (*base, device))
    }

    pub fn pl031_rtc(&self, base: Address) -> Option<&Pl031RtcMmioDevice> {
        self.pl031_rtcs.get(&base)
    }

    pub fn pl031_rtcs(&self) -> impl Iterator<Item = (Address, &Pl031RtcMmioDevice)> {
        self.pl031_rtcs.iter().map(|(base, device)| (*base, device))
    }

    pub fn sp804_timer(&self, base: Address) -> Option<&Sp804DualTimerMmioDevice> {
        self.sp804_timers.get(&base)
    }

    pub fn sp804_timers(&self) -> impl Iterator<Item = (Address, &Sp804DualTimerMmioDevice)> {
        self.sp804_timers
            .iter()
            .map(|(base, device)| (*base, device))
    }

    pub fn sp805_watchdog(&self, base: Address) -> Option<&Sp805WatchdogMmioDevice> {
        self.sp805_watchdogs.get(&base)
    }

    pub fn sp805_watchdogs(&self) -> impl Iterator<Item = (Address, &Sp805WatchdogMmioDevice)> {
        self.sp805_watchdogs
            .iter()
            .map(|(base, device)| (*base, device))
    }

    pub fn cpu_local_timer(&self, base: Address) -> Option<&CpuLocalTimerMmioDevice> {
        self.cpu_local_timers.get(&base)
    }

    pub fn cpu_local_timers(&self) -> impl Iterator<Item = (Address, &CpuLocalTimerMmioDevice)> {
        self.cpu_local_timers
            .iter()
            .map(|(base, device)| (*base, device))
    }

    pub fn riscv_device_tree(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
    ) -> Result<PlatformDeviceTree, PlatformError> {
        self.device_tree_inventory.riscv_device_tree(config)
    }
}

fn register_interrupt(
    controller: &Arc<Mutex<InterruptController>>,
    target_partition: PartitionId,
    line: InterruptLineId,
    target: InterruptTargetId,
    latency: Tick,
) -> Result<InterruptLinePort, PlatformError> {
    let route = InterruptRoute::new(line, target, target_partition);
    controller
        .lock()
        .expect("platform interrupt controller lock")
        .register_route(route)
        .map_err(PlatformError::Interrupt)?;
    let channel = InterruptLineChannel::new(route, latency).map_err(PlatformError::Interrupt)?;
    Ok(InterruptLinePort::new(channel, Arc::clone(controller)))
}

fn region(base: Address, size: AccessSize) -> Result<AddressRange, PlatformError> {
    AddressRange::new(base, size).map_err(PlatformError::Memory)
}

fn validate_route(partition_count: u32, route: MmioRoute) -> Result<(), PlatformError> {
    validate_partition(partition_count, route.source_partition())?;
    validate_partition(partition_count, route.target_partition())
}

fn validate_partition(partitions: u32, partition: PartitionId) -> Result<(), PlatformError> {
    if partition.index() >= partitions {
        return Err(PlatformError::UnknownPartition {
            partition,
            partitions,
        });
    }

    Ok(())
}

fn endpoint_partition(
    topology: &Topology,
    endpoint: &Endpoint,
) -> Result<PartitionId, PlatformTopologyError> {
    let component = topology.component(endpoint.component()).ok_or_else(|| {
        PlatformTopologyError::Topology(TopologyError::UnknownComponent {
            component: endpoint.component().clone(),
        })
    })?;
    component.port_direction(endpoint.port()).ok_or_else(|| {
        PlatformTopologyError::Topology(TopologyError::UnknownPort {
            component: endpoint.component().clone(),
            port: endpoint.port().clone(),
        })
    })?;

    Ok(component.partition())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformTopologyError {
    MissingPath { source: Endpoint, target: Endpoint },
    Topology(TopologyError),
    Mmio(MmioError),
}

impl fmt::Display for PlatformTopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPath { source, target } => write!(
                formatter,
                "topology path from {}.{} to {}.{} is not declared",
                source.component().as_str(),
                source.port().as_str(),
                target.component().as_str(),
                target.port().as_str()
            ),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::Mmio(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PlatformTopologyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Topology(error) => Some(error),
            Self::Mmio(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformError {
    NoPartitions,
    InvalidDeviceTreeConfig {
        field: &'static str,
    },
    DeviceTreeMissingInterruptController {
        device: String,
    },
    DeviceTreeMissingHart {
        device: String,
        hart: u32,
    },
    UnknownPartition {
        partition: PartitionId,
        partitions: u32,
    },
    MissingCpuLocalTimerRoute {
        base: Address,
        partition: PartitionId,
    },
    Memory(MemoryError),
    Mmio(MmioError),
    Interrupt(InterruptError),
    Timer(TimerError),
    Rtc(RtcError),
    Pl031(Pl031Error),
    Sp804(Sp804Error),
    Sp805(Sp805Error),
    CpuLocalTimer(CpuLocalTimerError),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPartitions => write!(formatter, "platform requires at least one partition"),
            Self::InvalidDeviceTreeConfig { field } => {
                write!(formatter, "invalid RISC-V device tree config field {field}")
            }
            Self::DeviceTreeMissingInterruptController { device } => write!(
                formatter,
                "RISC-V device tree node {device} has no interrupt controller"
            ),
            Self::DeviceTreeMissingHart { device, hart } => write!(
                formatter,
                "RISC-V device tree node {device} references missing hart {hart}"
            ),
            Self::UnknownPartition {
                partition,
                partitions,
            } => write!(
                formatter,
                "partition {} is outside platform partition count {partitions}",
                partition.index()
            ),
            Self::MissingCpuLocalTimerRoute { base, partition } => write!(
                formatter,
                "CPU local timer at {:#x} has no MMIO route for partition {}",
                base.get(),
                partition.index()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Mmio(error) => write!(formatter, "{error}"),
            Self::Interrupt(error) => write!(formatter, "{error}"),
            Self::Timer(error) => write!(formatter, "{error}"),
            Self::Rtc(error) => write!(formatter, "{error}"),
            Self::Pl031(error) => write!(formatter, "{error}"),
            Self::Sp804(error) => write!(formatter, "{error}"),
            Self::Sp805(error) => write!(formatter, "{error}"),
            Self::CpuLocalTimer(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PlatformError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Mmio(error) => Some(error),
            Self::Interrupt(error) => Some(error),
            Self::Timer(error) => Some(error),
            Self::Rtc(error) => Some(error),
            Self::Pl031(error) => Some(error),
            Self::Sp804(error) => Some(error),
            Self::Sp805(error) => Some(error),
            Self::CpuLocalTimer(error) => Some(error),
            _ => None,
        }
    }
}
