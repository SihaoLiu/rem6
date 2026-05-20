use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptError, InterruptLineChannel, InterruptLineId, InterruptLinePort,
    InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, MemoryError};
use rem6_mmio::{MmioBus, MmioError, MmioRoute};
use rem6_timer::{ProgrammableTimer, TimerId, TimerMmioDevice};
use rem6_uart::{UartId, UartMmioDevice};

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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlatformBuilder {
    partition_count: u32,
    timers: Vec<PlatformTimerConfig>,
    uarts: Vec<PlatformUartConfig>,
}

impl PlatformBuilder {
    pub const fn new(partition_count: u32) -> Self {
        Self {
            partition_count,
            timers: Vec::new(),
            uarts: Vec::new(),
        }
    }

    pub fn add_timer(mut self, config: PlatformTimerConfig) -> Self {
        self.timers.push(config);
        self
    }

    pub fn add_uart(mut self, config: PlatformUartConfig) -> Self {
        self.uarts.push(config);
        self
    }

    pub fn build(self) -> Result<Platform, PlatformError> {
        if self.partition_count == 0 {
            return Err(PlatformError::NoPartitions);
        }

        let controller = Arc::new(Mutex::new(InterruptController::new()));
        let mut bus = MmioBus::new();
        let mut timers = BTreeMap::new();
        let mut uarts = BTreeMap::new();

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

        Ok(Platform {
            partition_count: self.partition_count,
            interrupt_controller: controller,
            mmio_bus: bus,
            timers,
            uarts,
        })
    }
}

#[derive(Clone)]
pub struct Platform {
    partition_count: u32,
    interrupt_controller: Arc<Mutex<InterruptController>>,
    mmio_bus: MmioBus,
    timers: BTreeMap<TimerId, ProgrammableTimer>,
    uarts: BTreeMap<UartId, UartMmioDevice>,
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

    pub fn timer(&self, id: TimerId) -> Option<&ProgrammableTimer> {
        self.timers.get(&id)
    }

    pub fn uart(&self, id: UartId) -> Option<&UartMmioDevice> {
        self.uarts.get(&id)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformError {
    NoPartitions,
    UnknownPartition {
        partition: PartitionId,
        partitions: u32,
    },
    Memory(MemoryError),
    Mmio(MmioError),
    Interrupt(InterruptError),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPartitions => write!(formatter, "platform requires at least one partition"),
            Self::UnknownPartition {
                partition,
                partitions,
            } => write!(
                formatter,
                "partition {} is outside platform partition count {partitions}",
                partition.index()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Mmio(error) => write!(formatter, "{error}"),
            Self::Interrupt(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PlatformError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Mmio(error) => Some(error),
            Self::Interrupt(error) => Some(error),
            _ => None,
        }
    }
}
