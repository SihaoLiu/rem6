use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptLinePort, InterruptSourceId};
use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerContext, Tick};
use rem6_memory::Address;
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

use crate::ArmPrimecellId;

pub const SP805_LOAD_OFFSET: u64 = 0x000;
pub const SP805_VALUE_OFFSET: u64 = 0x004;
pub const SP805_CONTROL_OFFSET: u64 = 0x008;
pub const SP805_INT_CLEAR_OFFSET: u64 = 0x00c;
pub const SP805_RAW_ISR_OFFSET: u64 = 0x010;
pub const SP805_MASKED_ISR_OFFSET: u64 = 0x014;
pub const SP805_LOCK_OFFSET: u64 = 0xc00;
pub const SP805_ITCR_OFFSET: u64 = 0xf00;
pub const SP805_ITOP_OFFSET: u64 = 0xf04;
pub const SP805_REGISTER_BYTES: u64 = 4;
pub const SP805_MMIO_SIZE_BYTES: u64 = 0x1000;
pub const SP805_LOCK_MAGIC: u32 = 0x1acce551;
pub const SP805_PRIMECELL_ID: ArmPrimecellId = ArmPrimecellId::new(0x0014_1805);

const CONTROL_ENABLE: u32 = 1 << 0;
const CONTROL_RESET_ENABLE: u32 = 1 << 1;
const DEFAULT_TIMEOUT_INTERVAL: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sp805TimeoutOutcome {
    interrupt_asserted: bool,
    reset_asserted: bool,
    next_generation: u64,
}

impl Sp805TimeoutOutcome {
    pub const fn new(interrupt_asserted: bool, reset_asserted: bool, next_generation: u64) -> Self {
        Self {
            interrupt_asserted,
            reset_asserted,
            next_generation,
        }
    }

    pub const fn interrupt_asserted(self) -> bool {
        self.interrupt_asserted
    }

    pub const fn reset_asserted(self) -> bool {
        self.reset_asserted
    }

    pub const fn next_generation(self) -> u64 {
        self.next_generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp805WatchdogSnapshot {
    timeout_interval: u32,
    timeout_start_tick: Option<Tick>,
    persisted_value: u32,
    enabled: bool,
    reset_enabled: bool,
    write_access_enabled: bool,
    integration_test_enabled: bool,
    raw_interrupt: bool,
    clock_tick: Tick,
    generation: u64,
    reset_assertions: Vec<Tick>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp805WatchdogSnapshotFields {
    pub timeout_interval: u32,
    pub timeout_start_tick: Option<Tick>,
    pub persisted_value: u32,
    pub enabled: bool,
    pub reset_enabled: bool,
    pub write_access_enabled: bool,
    pub integration_test_enabled: bool,
    pub raw_interrupt: bool,
    pub clock_tick: Tick,
    pub generation: u64,
    pub reset_assertions: Vec<Tick>,
}

impl Sp805WatchdogSnapshot {
    pub fn from_fields(fields: Sp805WatchdogSnapshotFields) -> Self {
        Self {
            timeout_interval: fields.timeout_interval,
            timeout_start_tick: fields.timeout_start_tick,
            persisted_value: fields.persisted_value,
            enabled: fields.enabled,
            reset_enabled: fields.reset_enabled,
            write_access_enabled: fields.write_access_enabled,
            integration_test_enabled: fields.integration_test_enabled,
            raw_interrupt: fields.raw_interrupt,
            clock_tick: fields.clock_tick,
            generation: fields.generation,
            reset_assertions: fields.reset_assertions,
        }
    }

    pub const fn timeout_interval(&self) -> u32 {
        self.timeout_interval
    }

    pub const fn timeout_start_tick(&self) -> Option<Tick> {
        self.timeout_start_tick
    }

    pub const fn persisted_value(&self) -> u32 {
        self.persisted_value
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub const fn reset_enabled(&self) -> bool {
        self.reset_enabled
    }

    pub const fn write_access_enabled(&self) -> bool {
        self.write_access_enabled
    }

    pub const fn integration_test_enabled(&self) -> bool {
        self.integration_test_enabled
    }

    pub const fn raw_interrupt(&self) -> bool {
        self.raw_interrupt
    }

    pub const fn clock_tick(&self) -> Tick {
        self.clock_tick
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn reset_assertions(&self) -> &[Tick] {
        &self.reset_assertions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp805Watchdog {
    timeout_interval: u32,
    timeout_start_tick: Option<Tick>,
    persisted_value: u32,
    enabled: bool,
    reset_enabled: bool,
    write_access_enabled: bool,
    integration_test_enabled: bool,
    raw_interrupt: bool,
    clock_tick: Tick,
    generation: u64,
    reset_assertions: Vec<Tick>,
}

impl Sp805Watchdog {
    pub fn new(clock_tick: Tick) -> Result<Self, Sp805Error> {
        if clock_tick == 0 {
            return Err(Sp805Error::InvalidClockTick { clock_tick });
        }
        Ok(Self {
            timeout_interval: DEFAULT_TIMEOUT_INTERVAL,
            timeout_start_tick: None,
            persisted_value: DEFAULT_TIMEOUT_INTERVAL,
            enabled: false,
            reset_enabled: false,
            write_access_enabled: true,
            integration_test_enabled: false,
            raw_interrupt: false,
            clock_tick,
            generation: 0,
            reset_assertions: Vec::new(),
        })
    }

    pub fn read_register(&self, offset: u64, tick: Tick) -> Result<u32, Sp805Error> {
        match offset {
            SP805_LOAD_OFFSET => Ok(self.timeout_interval),
            SP805_VALUE_OFFSET => self.current_value(tick),
            SP805_CONTROL_OFFSET => {
                Ok(u32::from(self.enabled) | (u32::from(self.reset_enabled) << 1))
            }
            SP805_INT_CLEAR_OFFSET => Ok(0),
            SP805_RAW_ISR_OFFSET => Ok(u32::from(self.raw_interrupt)),
            SP805_MASKED_ISR_OFFSET => Ok(u32::from(self.raw_interrupt && self.enabled)),
            SP805_LOCK_OFFSET => Ok(u32::from(self.write_access_enabled)),
            SP805_ITCR_OFFSET => Ok(u32::from(self.integration_test_enabled)),
            SP805_ITOP_OFFSET => Ok(0),
            _ => Err(Sp805Error::UnknownRegister { offset }),
        }
    }

    pub fn write_register(
        &mut self,
        offset: u64,
        value: u32,
        tick: Tick,
    ) -> Result<Sp805WriteEffect, Sp805Error> {
        if offset == SP805_LOCK_OFFSET {
            self.write_access_enabled = value == SP805_LOCK_MAGIC;
            return Ok(Sp805WriteEffect::none());
        }
        if matches!(offset, SP805_ITCR_OFFSET | SP805_ITOP_OFFSET) {
            return Err(Sp805Error::IntegrationTestUnsupported);
        }
        if !self.write_access_enabled {
            return Ok(Sp805WriteEffect::none());
        }

        match offset {
            SP805_LOAD_OFFSET => {
                self.timeout_interval = value;
                self.persisted_value = value;
                if self.enabled {
                    self.restart_counter(tick)?;
                    Ok(Sp805WriteEffect::schedule(self.generation))
                } else {
                    Ok(Sp805WriteEffect::none())
                }
            }
            SP805_VALUE_OFFSET | SP805_RAW_ISR_OFFSET | SP805_MASKED_ISR_OFFSET => {
                Err(Sp805Error::ReadOnlyRegister { offset })
            }
            SP805_CONTROL_OFFSET => {
                let was_enabled = self.enabled;
                self.enabled = value & CONTROL_ENABLE != 0;
                self.reset_enabled = value & CONTROL_RESET_ENABLE != 0;
                if !was_enabled && self.enabled {
                    self.restart_counter(tick)?;
                    Ok(Sp805WriteEffect::schedule(self.generation))
                } else if was_enabled && !self.enabled {
                    self.stop_counter(tick)?;
                    Ok(Sp805WriteEffect::none())
                } else {
                    Ok(Sp805WriteEffect::none())
                }
            }
            SP805_INT_CLEAR_OFFSET => {
                let deassert = self.raw_interrupt;
                self.raw_interrupt = false;
                if self.enabled {
                    self.restart_counter(tick)?;
                    Ok(Sp805WriteEffect {
                        schedule_generation: Some(self.generation),
                        interrupt_asserted: false,
                        interrupt_deasserted: deassert,
                    })
                } else {
                    Ok(Sp805WriteEffect {
                        schedule_generation: None,
                        interrupt_asserted: false,
                        interrupt_deasserted: deassert,
                    })
                }
            }
            _ => Err(Sp805Error::UnknownRegister { offset }),
        }
    }

    pub fn current_value(&self, tick: Tick) -> Result<u32, Sp805Error> {
        let Some(start_tick) = self.timeout_start_tick else {
            return Ok(self.persisted_value);
        };
        let elapsed_ticks = tick
            .checked_sub(start_tick)
            .ok_or(Sp805Error::TimeWentBack {
                tick,
                last_updated_tick: start_tick,
            })?;
        let elapsed_cycles = elapsed_ticks / self.clock_tick;
        Ok(self
            .timeout_interval
            .saturating_sub(elapsed_cycles.min(u64::from(u32::MAX)) as u32))
    }

    pub fn next_timeout_tick(&self, tick: Tick) -> Result<Option<Tick>, Sp805Error> {
        if !self.enabled {
            return Ok(None);
        }
        let Some(start_tick) = self.timeout_start_tick else {
            return Ok(None);
        };
        let interval_ticks = self.interval_ticks()?;
        let deadline = start_tick
            .checked_add(interval_ticks)
            .ok_or(Sp805Error::DeadlineOverflow)?;
        if deadline < tick {
            Ok(Some(tick))
        } else {
            Ok(Some(deadline))
        }
    }

    pub fn record_timeout(
        &mut self,
        tick: Tick,
        generation: u64,
    ) -> Result<Option<Sp805TimeoutOutcome>, Sp805Error> {
        if generation != self.generation || !self.enabled {
            return Ok(None);
        }
        if let Some(start_tick) = self.timeout_start_tick {
            if tick < start_tick {
                return Err(Sp805Error::TimeWentBack {
                    tick,
                    last_updated_tick: start_tick,
                });
            }
        }

        let interrupt_asserted = !(self.raw_interrupt && self.enabled);
        let reset_asserted = !interrupt_asserted && self.reset_enabled;
        if reset_asserted {
            self.reset_assertions.push(tick);
        } else if interrupt_asserted {
            self.raw_interrupt = true;
        }
        self.restart_counter(tick)?;
        Ok(Some(Sp805TimeoutOutcome::new(
            interrupt_asserted,
            reset_asserted,
            self.generation,
        )))
    }

    pub fn snapshot(&self) -> Sp805WatchdogSnapshot {
        Sp805WatchdogSnapshot::from_fields(Sp805WatchdogSnapshotFields {
            timeout_interval: self.timeout_interval,
            timeout_start_tick: self.timeout_start_tick,
            persisted_value: self.persisted_value,
            enabled: self.enabled,
            reset_enabled: self.reset_enabled,
            write_access_enabled: self.write_access_enabled,
            integration_test_enabled: self.integration_test_enabled,
            raw_interrupt: self.raw_interrupt,
            clock_tick: self.clock_tick,
            generation: self.generation,
            reset_assertions: self.reset_assertions.clone(),
        })
    }

    pub fn restore(&mut self, snapshot: &Sp805WatchdogSnapshot) -> Result<(), Sp805Error> {
        if snapshot.clock_tick() == 0 {
            return Err(Sp805Error::InvalidClockTick {
                clock_tick: snapshot.clock_tick(),
            });
        }
        self.timeout_interval = snapshot.timeout_interval();
        self.timeout_start_tick = snapshot.timeout_start_tick();
        self.persisted_value = snapshot.persisted_value();
        self.enabled = snapshot.enabled();
        self.reset_enabled = snapshot.reset_enabled();
        self.write_access_enabled = snapshot.write_access_enabled();
        self.integration_test_enabled = snapshot.integration_test_enabled();
        self.raw_interrupt = snapshot.raw_interrupt();
        self.clock_tick = snapshot.clock_tick();
        self.generation = snapshot.generation();
        self.reset_assertions = snapshot.reset_assertions().to_vec();
        Ok(())
    }

    fn restart_counter(&mut self, tick: Tick) -> Result<(), Sp805Error> {
        self.timeout_start_tick = Some(tick);
        self.persisted_value = self.timeout_interval;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(Sp805Error::GenerationOverflow)?;
        Ok(())
    }

    fn stop_counter(&mut self, tick: Tick) -> Result<(), Sp805Error> {
        self.persisted_value = self.current_value(tick)?;
        self.timeout_start_tick = None;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(Sp805Error::GenerationOverflow)?;
        Ok(())
    }

    fn interval_ticks(&self) -> Result<Tick, Sp805Error> {
        let interval_cycles = u64::from(self.timeout_interval).max(1);
        self.clock_tick
            .checked_mul(interval_cycles)
            .ok_or(Sp805Error::DeadlineOverflow)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sp805WriteEffect {
    schedule_generation: Option<u64>,
    interrupt_asserted: bool,
    interrupt_deasserted: bool,
}

impl Sp805WriteEffect {
    pub const fn none() -> Self {
        Self {
            schedule_generation: None,
            interrupt_asserted: false,
            interrupt_deasserted: false,
        }
    }

    pub const fn schedule(generation: u64) -> Self {
        Self {
            schedule_generation: Some(generation),
            interrupt_asserted: false,
            interrupt_deasserted: false,
        }
    }

    pub const fn timeout(outcome: Sp805TimeoutOutcome) -> Self {
        Self {
            schedule_generation: Some(outcome.next_generation()),
            interrupt_asserted: outcome.interrupt_asserted(),
            interrupt_deasserted: false,
        }
    }

    pub const fn schedule_generation(self) -> Option<u64> {
        self.schedule_generation
    }

    pub const fn interrupt_asserted(self) -> bool {
        self.interrupt_asserted
    }

    pub const fn interrupt_deasserted(self) -> bool {
        self.interrupt_deasserted
    }
}

#[derive(Clone, Debug)]
pub struct Sp805WatchdogMmioDevice {
    base: Address,
    interrupt: Option<Sp805Interrupt>,
    state: Arc<Mutex<Sp805Watchdog>>,
}

impl Sp805WatchdogMmioDevice {
    pub fn new(base: Address, watchdog: Sp805Watchdog) -> Self {
        Self {
            base,
            interrupt: None,
            state: Arc::new(Mutex::new(watchdog)),
        }
    }

    pub fn with_interrupt(
        base: Address,
        watchdog: Sp805Watchdog,
        _partition: PartitionId,
        source: InterruptSourceId,
        port: InterruptLinePort,
    ) -> Result<Self, Sp805Error> {
        port.validate_route().map_err(Sp805Error::Interrupt)?;
        Ok(Self {
            base,
            interrupt: Some(Sp805Interrupt { source, port }),
            state: Arc::new(Mutex::new(watchdog)),
        })
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn range_size_bytes(&self) -> u64 {
        SP805_MMIO_SIZE_BYTES
    }

    pub fn snapshot(&self) -> Sp805WatchdogSnapshot {
        self.state
            .lock()
            .expect("SP805 watchdog state lock")
            .snapshot()
    }

    pub fn restore(&self, snapshot: &Sp805WatchdogSnapshot) -> Result<(), Sp805Error> {
        self.state
            .lock()
            .expect("SP805 watchdog state lock")
            .restore(snapshot)
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let (response, effect) = self.respond_request(request, context.now())?;
        self.apply_effect(context, effect)
            .map_err(|error| mmio_error(request.id(), error))?;
        Ok(response)
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let (response, effect) = self.respond_request(request, context.now())?;
        self.apply_effect_parallel(context, effect)
            .map_err(|error| mmio_error(request.id(), error))?;
        Ok(response)
    }

    fn respond_request(
        &self,
        request: &MmioRequest,
        tick: Tick,
    ) -> Result<(MmioResponse, Sp805WriteEffect), MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        if let Some(value) = SP805_PRIMECELL_ID.read_u32(offset) {
            return match request.operation() {
                MmioOperation::Read => Ok((
                    MmioResponse::completed(request.id(), Some(value.to_le_bytes().to_vec())),
                    Sp805WriteEffect::none(),
                )),
                MmioOperation::Write => Ok((
                    MmioResponse::completed(request.id(), None),
                    Sp805WriteEffect::none(),
                )),
            };
        }

        match request.operation() {
            MmioOperation::Read => {
                let value = self
                    .state
                    .lock()
                    .expect("SP805 watchdog state lock")
                    .read_register(offset, tick)
                    .map_err(|error| mmio_error(request.id(), error))?;
                Ok((
                    MmioResponse::completed(request.id(), Some(value.to_le_bytes().to_vec())),
                    Sp805WriteEffect::none(),
                ))
            }
            MmioOperation::Write => {
                let value = mmio_u32(request)?;
                let effect = self
                    .state
                    .lock()
                    .expect("SP805 watchdog state lock")
                    .write_register(offset, value, tick)
                    .map_err(|error| mmio_error(request.id(), error))?;
                Ok((MmioResponse::completed(request.id(), None), effect))
            }
        }
    }

    fn apply_effect(
        &self,
        context: &mut SchedulerContext<'_>,
        effect: Sp805WriteEffect,
    ) -> Result<(), Sp805Error> {
        if effect.interrupt_asserted() {
            self.assert_interrupt(context);
        }
        if effect.interrupt_deasserted() {
            self.deassert_interrupt(context);
        }
        if let Some(generation) = effect.schedule_generation() {
            self.schedule_timeout_generation(context, generation)?;
        }
        Ok(())
    }

    fn apply_effect_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        effect: Sp805WriteEffect,
    ) -> Result<(), Sp805Error> {
        if effect.interrupt_asserted() {
            self.assert_interrupt_parallel(context);
        }
        if effect.interrupt_deasserted() {
            self.deassert_interrupt_parallel(context);
        }
        if let Some(generation) = effect.schedule_generation() {
            self.schedule_timeout_generation_parallel(context, generation)?;
        }
        Ok(())
    }

    fn schedule_timeout_generation(
        &self,
        context: &mut SchedulerContext<'_>,
        generation: u64,
    ) -> Result<(), Sp805Error> {
        let deadline = {
            let state = self.state.lock().expect("SP805 watchdog state lock");
            if state.snapshot().generation() != generation {
                return Ok(());
            }
            let Some(deadline) = state.next_timeout_tick(context.now())? else {
                return Ok(());
            };
            deadline
        };
        let delay = deadline
            .checked_sub(context.now())
            .ok_or(Sp805Error::TimeWentBack {
                tick: deadline,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_timeout(context, generation);
            })
            .map(|_| ())
            .map_err(Sp805Error::Scheduler)
    }

    fn schedule_timeout_generation_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        generation: u64,
    ) -> Result<(), Sp805Error> {
        let deadline = {
            let state = self.state.lock().expect("SP805 watchdog state lock");
            if state.snapshot().generation() != generation {
                return Ok(());
            }
            let Some(deadline) = state.next_timeout_tick(context.now())? else {
                return Ok(());
            };
            deadline
        };
        let delay = deadline
            .checked_sub(context.now())
            .ok_or(Sp805Error::TimeWentBack {
                tick: deadline,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_timeout_parallel(context, generation);
            })
            .map(|_| ())
            .map_err(Sp805Error::Scheduler)
    }

    fn fire_timeout(&self, context: &mut SchedulerContext<'_>, generation: u64) {
        let outcome = match self.record_timeout(context.now(), generation) {
            Some(outcome) => outcome,
            None => return,
        };
        if outcome.interrupt_asserted() {
            self.assert_interrupt(context);
        }
        let _ = self.schedule_timeout_generation(context, outcome.next_generation());
    }

    fn fire_timeout_parallel(&self, context: &mut ParallelSchedulerContext<'_>, generation: u64) {
        let outcome = match self.record_timeout(context.now(), generation) {
            Some(outcome) => outcome,
            None => return,
        };
        if outcome.interrupt_asserted() {
            self.assert_interrupt_parallel(context);
        }
        let _ = self.schedule_timeout_generation_parallel(context, outcome.next_generation());
    }

    fn record_timeout(&self, tick: Tick, generation: u64) -> Option<Sp805TimeoutOutcome> {
        self.state
            .lock()
            .expect("SP805 watchdog state lock")
            .record_timeout(tick, generation)
            .ok()
            .flatten()
    }

    fn assert_interrupt(&self, context: &mut SchedulerContext<'_>) {
        if let Some(interrupt) = &self.interrupt {
            let _ = interrupt.port.assert(context, interrupt.source);
        }
    }

    fn deassert_interrupt(&self, context: &mut SchedulerContext<'_>) {
        if let Some(interrupt) = &self.interrupt {
            let _ = interrupt.port.deassert(context, interrupt.source);
        }
    }

    fn assert_interrupt_parallel(&self, context: &mut ParallelSchedulerContext<'_>) {
        if let Some(interrupt) = &self.interrupt {
            let _ = interrupt.port.assert_parallel(context, interrupt.source);
        }
    }

    fn deassert_interrupt_parallel(&self, context: &mut ParallelSchedulerContext<'_>) {
        if let Some(interrupt) = &self.interrupt {
            let _ = interrupt.port.deassert_parallel(context, interrupt.source);
        }
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        if request.size().bytes() != SP805_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: SP805_REGISTER_BYTES,
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
}

impl MmioDevice for Sp805WatchdogMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Sp805WatchdogMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Sp805WatchdogMmioDevice::respond_parallel(self, context, request)
    }
}

#[derive(Clone, Debug)]
struct Sp805Interrupt {
    source: InterruptSourceId,
    port: InterruptLinePort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Sp805Error {
    InvalidClockTick { clock_tick: Tick },
    UnknownRegister { offset: u64 },
    ReadOnlyRegister { offset: u64 },
    IntegrationTestUnsupported,
    TimeWentBack { tick: Tick, last_updated_tick: Tick },
    DeadlineOverflow,
    GenerationOverflow,
    Interrupt(InterruptError),
    Scheduler(rem6_kernel::SchedulerError),
}

impl fmt::Display for Sp805Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidClockTick { clock_tick } => {
                write!(
                    formatter,
                    "SP805 clock tick must be positive, got {clock_tick}"
                )
            }
            Self::UnknownRegister { offset } => {
                write!(
                    formatter,
                    "unknown SP805 watchdog register offset {offset:#x}"
                )
            }
            Self::ReadOnlyRegister { offset } => {
                write!(
                    formatter,
                    "SP805 watchdog register {offset:#x} is read-only"
                )
            }
            Self::IntegrationTestUnsupported => {
                write!(formatter, "SP805 integration test harness is not supported")
            }
            Self::TimeWentBack {
                tick,
                last_updated_tick,
            } => write!(
                formatter,
                "SP805 tick {tick} is earlier than last updated tick {last_updated_tick}"
            ),
            Self::DeadlineOverflow => write!(formatter, "SP805 deadline overflowed"),
            Self::GenerationOverflow => write!(formatter, "SP805 generation overflowed"),
            Self::Interrupt(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for Sp805Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Interrupt(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}

fn mmio_u32(request: &MmioRequest) -> Result<u32, MmioError> {
    let data = request.data().ok_or(MmioError::MissingWriteData {
        request: request.id(),
    })?;
    let bytes: [u8; 4] = data
        .try_into()
        .map_err(|_| MmioError::PayloadSizeMismatch {
            request: request.id(),
            expected: SP805_REGISTER_BYTES,
            actual: data.len() as u64,
        })?;
    Ok(u32::from_le_bytes(bytes))
}

fn mmio_error(request: rem6_mmio::MmioRequestId, error: Sp805Error) -> MmioError {
    MmioError::DeviceError {
        request,
        message: error.to_string(),
    }
}
