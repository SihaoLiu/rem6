use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptLinePort, InterruptSourceId};
use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerContext, Tick};
use rem6_memory::Address;
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

mod error;

use error::mmio_error;
pub use error::CpuLocalTimerError;

pub const CPU_LOCAL_TIMER_LOAD_OFFSET: u64 = 0x00;
pub const CPU_LOCAL_TIMER_COUNTER_OFFSET: u64 = 0x04;
pub const CPU_LOCAL_TIMER_CONTROL_OFFSET: u64 = 0x08;
pub const CPU_LOCAL_TIMER_INT_STATUS_OFFSET: u64 = 0x0c;
pub const CPU_LOCAL_WATCHDOG_LOAD_OFFSET: u64 = 0x20;
pub const CPU_LOCAL_WATCHDOG_COUNTER_OFFSET: u64 = 0x24;
pub const CPU_LOCAL_WATCHDOG_CONTROL_OFFSET: u64 = 0x28;
pub const CPU_LOCAL_WATCHDOG_INT_STATUS_OFFSET: u64 = 0x2c;
pub const CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET: u64 = 0x30;
pub const CPU_LOCAL_WATCHDOG_DISABLE_OFFSET: u64 = 0x34;
pub const CPU_LOCAL_TIMER_REGISTER_BYTES: u64 = 4;
pub const CPU_LOCAL_TIMER_MMIO_SIZE_BYTES: u64 = 0x38;

const CONTROL_ENABLE: u32 = 1 << 0;
const CONTROL_AUTO_RELOAD: u32 = 1 << 1;
const CONTROL_INTERRUPT_ENABLE: u32 = 1 << 2;
const CONTROL_PRESCALAR_SHIFT: u32 = 8;
const CONTROL_PRESCALAR_MASK: u32 = 0xff << CONTROL_PRESCALAR_SHIFT;
const WATCHDOG_CONTROL_MODE: u32 = 1 << 3;
const WATCHDOG_DISABLE_FIRST: u32 = 0x1234_5678;
const WATCHDOG_DISABLE_SECOND: u32 = 0x8765_4321;
pub(crate) const MAX_PRESCALAR_SHIFT_ENTRY: u32 = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerControl {
    bits: u32,
}

impl CpuLocalTimerControl {
    pub const fn new(bits: u32) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    pub const fn enabled(self) -> bool {
        self.bits & CONTROL_ENABLE != 0
    }

    pub const fn auto_reload(self) -> bool {
        self.bits & CONTROL_AUTO_RELOAD != 0
    }

    pub const fn interrupt_enabled(self) -> bool {
        self.bits & CONTROL_INTERRUPT_ENABLE != 0
    }

    pub const fn prescalar(self) -> u32 {
        (self.bits & CONTROL_PRESCALAR_MASK) >> CONTROL_PRESCALAR_SHIFT
    }

    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        if enabled {
            self.bits |= CONTROL_ENABLE;
        } else {
            self.bits &= !CONTROL_ENABLE;
        }
        self
    }

    pub const fn with_auto_reload(mut self, auto_reload: bool) -> Self {
        if auto_reload {
            self.bits |= CONTROL_AUTO_RELOAD;
        } else {
            self.bits &= !CONTROL_AUTO_RELOAD;
        }
        self
    }

    pub const fn with_interrupt_enabled(mut self, interrupt_enabled: bool) -> Self {
        if interrupt_enabled {
            self.bits |= CONTROL_INTERRUPT_ENABLE;
        } else {
            self.bits &= !CONTROL_INTERRUPT_ENABLE;
        }
        self
    }

    pub const fn with_prescalar(mut self, prescalar: u32) -> Result<Self, CpuLocalTimerError> {
        if prescalar > u8::MAX as u32 {
            return Err(CpuLocalTimerError::InvalidPrescalar { prescalar });
        }
        self.bits = (self.bits & !CONTROL_PRESCALAR_MASK) | (prescalar << CONTROL_PRESCALAR_SHIFT);
        Ok(self)
    }

    fn ticks_per_decrement(self, clock_tick: Tick) -> Result<Tick, CpuLocalTimerError> {
        let prescalar = self.prescalar();
        if prescalar > MAX_PRESCALAR_SHIFT_ENTRY {
            return Err(CpuLocalTimerError::InvalidPrescalar { prescalar });
        }
        clock_tick
            .checked_shl(4 * prescalar)
            .ok_or(CpuLocalTimerError::DeadlineOverflow)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuLocalWatchdogControl {
    bits: u32,
}

impl CpuLocalWatchdogControl {
    pub const fn new(bits: u32) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    pub const fn enabled(self) -> bool {
        self.bits & CONTROL_ENABLE != 0
    }

    pub const fn auto_reload(self) -> bool {
        self.bits & CONTROL_AUTO_RELOAD != 0
    }

    pub const fn interrupt_enabled(self) -> bool {
        self.bits & CONTROL_INTERRUPT_ENABLE != 0
    }

    pub const fn watchdog_mode(self) -> bool {
        self.bits & WATCHDOG_CONTROL_MODE != 0
    }

    pub const fn prescalar(self) -> u32 {
        (self.bits & CONTROL_PRESCALAR_MASK) >> CONTROL_PRESCALAR_SHIFT
    }

    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        if enabled {
            self.bits |= CONTROL_ENABLE;
        } else {
            self.bits &= !CONTROL_ENABLE;
        }
        self
    }

    pub const fn with_auto_reload(mut self, auto_reload: bool) -> Self {
        if auto_reload {
            self.bits |= CONTROL_AUTO_RELOAD;
        } else {
            self.bits &= !CONTROL_AUTO_RELOAD;
        }
        self
    }

    pub const fn with_interrupt_enabled(mut self, interrupt_enabled: bool) -> Self {
        if interrupt_enabled {
            self.bits |= CONTROL_INTERRUPT_ENABLE;
        } else {
            self.bits &= !CONTROL_INTERRUPT_ENABLE;
        }
        self
    }

    pub const fn with_watchdog_mode(mut self, watchdog_mode: bool) -> Self {
        if watchdog_mode {
            self.bits |= WATCHDOG_CONTROL_MODE;
        } else {
            self.bits &= !WATCHDOG_CONTROL_MODE;
        }
        self
    }

    pub const fn with_prescalar(mut self, prescalar: u32) -> Result<Self, CpuLocalTimerError> {
        if prescalar > u8::MAX as u32 {
            return Err(CpuLocalTimerError::InvalidPrescalar { prescalar });
        }
        self.bits = (self.bits & !CONTROL_PRESCALAR_MASK) | (prescalar << CONTROL_PRESCALAR_SHIFT);
        Ok(self)
    }

    fn ticks_per_decrement(self, clock_tick: Tick) -> Result<Tick, CpuLocalTimerError> {
        let prescalar = self.prescalar();
        if prescalar > MAX_PRESCALAR_SHIFT_ENTRY {
            return Err(CpuLocalTimerError::InvalidPrescalar { prescalar });
        }
        clock_tick
            .checked_shl(4 * prescalar)
            .ok_or(CpuLocalTimerError::DeadlineOverflow)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerCounterSnapshot {
    load_value: u32,
    base_value: u32,
    last_updated_tick: Tick,
    control: CpuLocalTimerControl,
    raw_interrupt: bool,
    pending_interrupt: bool,
    clock_tick: Tick,
    generation: u64,
}

impl CpuLocalTimerCounterSnapshot {
    pub const fn raw_interrupt(&self) -> bool {
        self.raw_interrupt
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalWatchdogSnapshot {
    load_value: u32,
    base_value: u32,
    last_updated_tick: Tick,
    control: CpuLocalWatchdogControl,
    raw_interrupt: bool,
    pending_interrupt: bool,
    raw_reset: bool,
    disable_register: u32,
    clock_tick: Tick,
    generation: u64,
    reset_assertions: Vec<Tick>,
}

impl CpuLocalWatchdogSnapshot {
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn reset_assertions(&self) -> &[Tick] {
        &self.reset_assertions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerCpuSnapshot {
    timer: CpuLocalTimerCounterSnapshot,
    watchdog: CpuLocalWatchdogSnapshot,
}

impl CpuLocalTimerCpuSnapshot {
    pub const fn timer(&self) -> &CpuLocalTimerCounterSnapshot {
        &self.timer
    }

    pub const fn watchdog(&self) -> &CpuLocalWatchdogSnapshot {
        &self.watchdog
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerBankSnapshot {
    cpus: Vec<CpuLocalTimerCpuSnapshot>,
}

impl CpuLocalTimerBankSnapshot {
    pub fn cpu(&self, index: usize) -> Result<&CpuLocalTimerCpuSnapshot, CpuLocalTimerError> {
        self.cpus
            .get(index)
            .ok_or(CpuLocalTimerError::UnknownCpu { index })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerZeroOutcome {
    interrupt_asserted: bool,
    reset_asserted: bool,
    next_generation: Option<u64>,
}

impl CpuLocalTimerZeroOutcome {
    const fn timer(interrupt_asserted: bool, next_generation: Option<u64>) -> Self {
        Self {
            interrupt_asserted,
            reset_asserted: false,
            next_generation,
        }
    }

    const fn watchdog(
        interrupt_asserted: bool,
        reset_asserted: bool,
        next_generation: Option<u64>,
    ) -> Self {
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

    pub const fn next_generation(self) -> Option<u64> {
        self.next_generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerWriteEffect {
    schedule_timer: bool,
    schedule_watchdog: bool,
    deassert_timer: bool,
    deassert_watchdog: bool,
}

impl CpuLocalTimerWriteEffect {
    const fn none() -> Self {
        Self {
            schedule_timer: false,
            schedule_watchdog: false,
            deassert_timer: false,
            deassert_watchdog: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerCpu {
    timer: LocalTimerCounter,
    watchdog: LocalWatchdogCounter,
}

impl CpuLocalTimerCpu {
    fn new(clock_tick: Tick) -> Result<Self, CpuLocalTimerError> {
        Ok(Self {
            timer: LocalTimerCounter::new(clock_tick)?,
            watchdog: LocalWatchdogCounter::new(clock_tick)?,
        })
    }

    pub fn read_register(&self, offset: u64, tick: Tick) -> Result<u32, CpuLocalTimerError> {
        match offset {
            CPU_LOCAL_TIMER_LOAD_OFFSET
            | CPU_LOCAL_TIMER_COUNTER_OFFSET
            | CPU_LOCAL_TIMER_CONTROL_OFFSET
            | CPU_LOCAL_TIMER_INT_STATUS_OFFSET => self.timer.read_register(offset, tick),
            CPU_LOCAL_WATCHDOG_LOAD_OFFSET
            | CPU_LOCAL_WATCHDOG_COUNTER_OFFSET
            | CPU_LOCAL_WATCHDOG_CONTROL_OFFSET
            | CPU_LOCAL_WATCHDOG_INT_STATUS_OFFSET
            | CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET => self.watchdog.read_register(offset, tick),
            CPU_LOCAL_WATCHDOG_DISABLE_OFFSET => {
                Err(CpuLocalTimerError::WriteOnlyRegister { offset })
            }
            _ => Err(CpuLocalTimerError::UnknownRegister { offset }),
        }
    }

    pub fn write_register(
        &mut self,
        offset: u64,
        value: u32,
        tick: Tick,
    ) -> Result<CpuLocalTimerWriteEffect, CpuLocalTimerError> {
        match offset {
            CPU_LOCAL_TIMER_LOAD_OFFSET
            | CPU_LOCAL_TIMER_COUNTER_OFFSET
            | CPU_LOCAL_TIMER_CONTROL_OFFSET
            | CPU_LOCAL_TIMER_INT_STATUS_OFFSET => self.timer.write_register(offset, value, tick),
            CPU_LOCAL_WATCHDOG_LOAD_OFFSET
            | CPU_LOCAL_WATCHDOG_COUNTER_OFFSET
            | CPU_LOCAL_WATCHDOG_CONTROL_OFFSET
            | CPU_LOCAL_WATCHDOG_INT_STATUS_OFFSET
            | CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET
            | CPU_LOCAL_WATCHDOG_DISABLE_OFFSET => {
                self.watchdog.write_register(offset, value, tick)
            }
            _ => Err(CpuLocalTimerError::UnknownRegister { offset }),
        }
    }

    pub fn next_timer_zero_tick(&self, tick: Tick) -> Result<Option<Tick>, CpuLocalTimerError> {
        self.timer.next_zero_tick(tick)
    }

    pub fn next_watchdog_zero_tick(&self, tick: Tick) -> Result<Option<Tick>, CpuLocalTimerError> {
        self.watchdog.next_zero_tick(tick)
    }

    pub fn record_timer_zero(
        &mut self,
        tick: Tick,
        generation: u64,
    ) -> Result<Option<CpuLocalTimerZeroOutcome>, CpuLocalTimerError> {
        self.timer.record_zero(tick, generation)
    }

    pub fn record_watchdog_zero(
        &mut self,
        tick: Tick,
        generation: u64,
    ) -> Result<Option<CpuLocalTimerZeroOutcome>, CpuLocalTimerError> {
        self.watchdog.record_zero(tick, generation)
    }

    pub fn snapshot(&self) -> CpuLocalTimerCpuSnapshot {
        CpuLocalTimerCpuSnapshot {
            timer: self.timer.snapshot(),
            watchdog: self.watchdog.snapshot(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalTimerCounter {
    load_value: u32,
    base_value: u32,
    last_updated_tick: Tick,
    control: CpuLocalTimerControl,
    raw_interrupt: bool,
    pending_interrupt: bool,
    clock_tick: Tick,
    generation: u64,
}

impl LocalTimerCounter {
    fn new(clock_tick: Tick) -> Result<Self, CpuLocalTimerError> {
        if clock_tick == 0 {
            return Err(CpuLocalTimerError::InvalidClockTick { clock_tick });
        }
        Ok(Self {
            load_value: 0,
            base_value: 0,
            last_updated_tick: 0,
            control: CpuLocalTimerControl::new(0),
            raw_interrupt: false,
            pending_interrupt: false,
            clock_tick,
            generation: 0,
        })
    }

    fn read_register(&self, offset: u64, tick: Tick) -> Result<u32, CpuLocalTimerError> {
        match offset {
            CPU_LOCAL_TIMER_LOAD_OFFSET => Ok(self.load_value),
            CPU_LOCAL_TIMER_COUNTER_OFFSET => self.current_value(tick),
            CPU_LOCAL_TIMER_CONTROL_OFFSET => Ok(self.control.bits()),
            CPU_LOCAL_TIMER_INT_STATUS_OFFSET => Ok(u32::from(self.raw_interrupt)),
            _ => Err(CpuLocalTimerError::UnknownRegister { offset }),
        }
    }

    fn write_register(
        &mut self,
        offset: u64,
        value: u32,
        tick: Tick,
    ) -> Result<CpuLocalTimerWriteEffect, CpuLocalTimerError> {
        match offset {
            CPU_LOCAL_TIMER_LOAD_OFFSET => {
                self.load_value = value;
                self.base_value = value;
                self.last_updated_tick = tick;
                if self.control.enabled() {
                    self.bump_generation()?;
                    Ok(CpuLocalTimerWriteEffect {
                        schedule_timer: true,
                        ..CpuLocalTimerWriteEffect::none()
                    })
                } else {
                    Ok(CpuLocalTimerWriteEffect::none())
                }
            }
            CPU_LOCAL_TIMER_COUNTER_OFFSET => {
                self.base_value = value;
                self.last_updated_tick = tick;
                if self.control.enabled() {
                    self.bump_generation()?;
                    Ok(CpuLocalTimerWriteEffect {
                        schedule_timer: true,
                        ..CpuLocalTimerWriteEffect::none()
                    })
                } else {
                    Ok(CpuLocalTimerWriteEffect::none())
                }
            }
            CPU_LOCAL_TIMER_CONTROL_OFFSET => self.write_control(value, tick),
            CPU_LOCAL_TIMER_INT_STATUS_OFFSET => {
                let deassert_timer = self.pending_interrupt;
                self.raw_interrupt = false;
                self.pending_interrupt = false;
                Ok(CpuLocalTimerWriteEffect {
                    deassert_timer,
                    ..CpuLocalTimerWriteEffect::none()
                })
            }
            _ => Err(CpuLocalTimerError::UnknownRegister { offset }),
        }
    }

    fn write_control(
        &mut self,
        value: u32,
        tick: Tick,
    ) -> Result<CpuLocalTimerWriteEffect, CpuLocalTimerError> {
        let old_enabled = self.control.enabled();
        let control = CpuLocalTimerControl::new(value);
        control.ticks_per_decrement(self.clock_tick)?;
        self.control = control;
        if !old_enabled && self.control.enabled() {
            self.base_value = self.load_value;
            self.last_updated_tick = tick;
            self.bump_generation()?;
            Ok(CpuLocalTimerWriteEffect {
                schedule_timer: true,
                ..CpuLocalTimerWriteEffect::none()
            })
        } else if old_enabled && !self.control.enabled() {
            self.base_value = self.current_value(tick)?;
            self.last_updated_tick = tick;
            self.bump_generation()?;
            Ok(CpuLocalTimerWriteEffect::none())
        } else {
            Ok(CpuLocalTimerWriteEffect::none())
        }
    }

    fn current_value(&self, tick: Tick) -> Result<u32, CpuLocalTimerError> {
        if tick < self.last_updated_tick {
            return Err(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: self.last_updated_tick,
            });
        }
        if !self.control.enabled() {
            return Ok(self.base_value);
        }
        let ticks_per_decrement = self.control.ticks_per_decrement(self.clock_tick)?;
        let elapsed = tick - self.last_updated_tick;
        let decrements = elapsed / ticks_per_decrement;
        if decrements >= u64::from(self.base_value) {
            Ok(0)
        } else {
            Ok(self.base_value - decrements as u32)
        }
    }

    fn next_zero_tick(&self, tick: Tick) -> Result<Option<Tick>, CpuLocalTimerError> {
        if !self.control.enabled() {
            return Ok(None);
        }
        let ticks_per_decrement = self.control.ticks_per_decrement(self.clock_tick)?;
        next_zero_deadline(
            self.last_updated_tick,
            self.base_value,
            tick,
            ticks_per_decrement,
        )
        .map(Some)
    }

    fn record_zero(
        &mut self,
        tick: Tick,
        generation: u64,
    ) -> Result<Option<CpuLocalTimerZeroOutcome>, CpuLocalTimerError> {
        if self.generation != generation || !self.control.enabled() {
            return Ok(None);
        }
        let old_pending = self.pending_interrupt;
        self.raw_interrupt = true;
        if self.control.interrupt_enabled() {
            self.pending_interrupt = true;
        }
        let interrupt_asserted = self.pending_interrupt && !old_pending;
        if self.control.auto_reload() {
            self.base_value = self.load_value;
            self.last_updated_tick = tick;
            self.bump_generation()?;
            Ok(Some(CpuLocalTimerZeroOutcome::timer(
                interrupt_asserted,
                Some(self.generation),
            )))
        } else {
            self.base_value = 0;
            self.last_updated_tick = tick;
            Ok(Some(CpuLocalTimerZeroOutcome::timer(
                interrupt_asserted,
                None,
            )))
        }
    }

    fn snapshot(&self) -> CpuLocalTimerCounterSnapshot {
        CpuLocalTimerCounterSnapshot {
            load_value: self.load_value,
            base_value: self.base_value,
            last_updated_tick: self.last_updated_tick,
            control: self.control,
            raw_interrupt: self.raw_interrupt,
            pending_interrupt: self.pending_interrupt,
            clock_tick: self.clock_tick,
            generation: self.generation,
        }
    }

    fn bump_generation(&mut self) -> Result<(), CpuLocalTimerError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(CpuLocalTimerError::GenerationOverflow)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LocalWatchdogCounter {
    load_value: u32,
    base_value: u32,
    last_updated_tick: Tick,
    control: CpuLocalWatchdogControl,
    raw_interrupt: bool,
    pending_interrupt: bool,
    raw_reset: bool,
    disable_register: u32,
    clock_tick: Tick,
    generation: u64,
    reset_assertions: Vec<Tick>,
}

impl LocalWatchdogCounter {
    fn new(clock_tick: Tick) -> Result<Self, CpuLocalTimerError> {
        if clock_tick == 0 {
            return Err(CpuLocalTimerError::InvalidClockTick { clock_tick });
        }
        Ok(Self {
            load_value: 0,
            base_value: 0,
            last_updated_tick: 0,
            control: CpuLocalWatchdogControl::new(0),
            raw_interrupt: false,
            pending_interrupt: false,
            raw_reset: false,
            disable_register: 0,
            clock_tick,
            generation: 0,
            reset_assertions: Vec::new(),
        })
    }

    fn read_register(&self, offset: u64, tick: Tick) -> Result<u32, CpuLocalTimerError> {
        match offset {
            CPU_LOCAL_WATCHDOG_LOAD_OFFSET => Ok(self.load_value),
            CPU_LOCAL_WATCHDOG_COUNTER_OFFSET => self.current_value(tick),
            CPU_LOCAL_WATCHDOG_CONTROL_OFFSET => Ok(self.control.bits()),
            CPU_LOCAL_WATCHDOG_INT_STATUS_OFFSET => Ok(u32::from(self.raw_interrupt)),
            CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET => Ok(u32::from(self.raw_reset)),
            _ => Err(CpuLocalTimerError::UnknownRegister { offset }),
        }
    }

    fn write_register(
        &mut self,
        offset: u64,
        value: u32,
        tick: Tick,
    ) -> Result<CpuLocalTimerWriteEffect, CpuLocalTimerError> {
        match offset {
            CPU_LOCAL_WATCHDOG_LOAD_OFFSET => {
                self.load_value = value;
                self.base_value = value;
                self.last_updated_tick = tick;
                if self.control.enabled() {
                    self.bump_generation()?;
                    Ok(CpuLocalTimerWriteEffect {
                        schedule_watchdog: true,
                        ..CpuLocalTimerWriteEffect::none()
                    })
                } else {
                    Ok(CpuLocalTimerWriteEffect::none())
                }
            }
            CPU_LOCAL_WATCHDOG_COUNTER_OFFSET => {
                if self.control.watchdog_mode() {
                    return Ok(CpuLocalTimerWriteEffect::none());
                }
                self.base_value = value;
                self.last_updated_tick = tick;
                if self.control.enabled() {
                    self.bump_generation()?;
                    Ok(CpuLocalTimerWriteEffect {
                        schedule_watchdog: true,
                        ..CpuLocalTimerWriteEffect::none()
                    })
                } else {
                    Ok(CpuLocalTimerWriteEffect::none())
                }
            }
            CPU_LOCAL_WATCHDOG_CONTROL_OFFSET => self.write_control(value, tick),
            CPU_LOCAL_WATCHDOG_INT_STATUS_OFFSET => {
                let deassert_watchdog = self.pending_interrupt;
                self.raw_interrupt = false;
                self.pending_interrupt = false;
                Ok(CpuLocalTimerWriteEffect {
                    deassert_watchdog,
                    ..CpuLocalTimerWriteEffect::none()
                })
            }
            CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET => {
                self.raw_reset = false;
                Ok(CpuLocalTimerWriteEffect::none())
            }
            CPU_LOCAL_WATCHDOG_DISABLE_OFFSET => {
                let old_value = self.disable_register;
                self.disable_register = value;
                if old_value == WATCHDOG_DISABLE_FIRST && value == WATCHDOG_DISABLE_SECOND {
                    self.control = self.control.with_watchdog_mode(false);
                }
                Ok(CpuLocalTimerWriteEffect::none())
            }
            _ => Err(CpuLocalTimerError::UnknownRegister { offset }),
        }
    }

    fn write_control(
        &mut self,
        value: u32,
        tick: Tick,
    ) -> Result<CpuLocalTimerWriteEffect, CpuLocalTimerError> {
        let old_enabled = self.control.enabled();
        let old_watchdog_mode = self.control.watchdog_mode();
        let mut control = CpuLocalWatchdogControl::new(value);
        if old_watchdog_mode && !control.watchdog_mode() {
            control = control.with_watchdog_mode(true);
        }
        control.ticks_per_decrement(self.clock_tick)?;
        self.control = control;
        if !old_enabled && self.control.enabled() {
            self.base_value = self.load_value;
            self.last_updated_tick = tick;
            self.bump_generation()?;
            Ok(CpuLocalTimerWriteEffect {
                schedule_watchdog: true,
                ..CpuLocalTimerWriteEffect::none()
            })
        } else if old_enabled && !self.control.enabled() {
            self.base_value = self.current_value(tick)?;
            self.last_updated_tick = tick;
            self.bump_generation()?;
            Ok(CpuLocalTimerWriteEffect::none())
        } else {
            Ok(CpuLocalTimerWriteEffect::none())
        }
    }

    fn current_value(&self, tick: Tick) -> Result<u32, CpuLocalTimerError> {
        if tick < self.last_updated_tick {
            return Err(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: self.last_updated_tick,
            });
        }
        if !self.control.enabled() {
            return Ok(self.base_value);
        }
        let ticks_per_decrement = self.control.ticks_per_decrement(self.clock_tick)?;
        let elapsed = tick - self.last_updated_tick;
        let decrements = elapsed / ticks_per_decrement;
        if decrements >= u64::from(self.base_value) {
            Ok(0)
        } else {
            Ok(self.base_value - decrements as u32)
        }
    }

    fn next_zero_tick(&self, tick: Tick) -> Result<Option<Tick>, CpuLocalTimerError> {
        if !self.control.enabled() {
            return Ok(None);
        }
        let ticks_per_decrement = self.control.ticks_per_decrement(self.clock_tick)?;
        next_zero_deadline(
            self.last_updated_tick,
            self.base_value,
            tick,
            ticks_per_decrement,
        )
        .map(Some)
    }

    fn record_zero(
        &mut self,
        tick: Tick,
        generation: u64,
    ) -> Result<Option<CpuLocalTimerZeroOutcome>, CpuLocalTimerError> {
        if self.generation != generation || !self.control.enabled() {
            return Ok(None);
        }
        self.raw_interrupt = true;
        let old_pending = self.pending_interrupt;
        if self.control.watchdog_mode() {
            self.raw_reset = true;
            self.reset_assertions.push(tick);
            self.base_value = 0;
            self.last_updated_tick = tick;
            Ok(Some(CpuLocalTimerZeroOutcome::watchdog(false, true, None)))
        } else {
            if self.control.interrupt_enabled() {
                self.pending_interrupt = true;
            }
            let interrupt_asserted = self.pending_interrupt && !old_pending;
            if self.control.auto_reload() {
                self.base_value = self.load_value;
                self.last_updated_tick = tick;
                self.bump_generation()?;
                Ok(Some(CpuLocalTimerZeroOutcome::watchdog(
                    interrupt_asserted,
                    false,
                    Some(self.generation),
                )))
            } else {
                self.base_value = 0;
                self.last_updated_tick = tick;
                Ok(Some(CpuLocalTimerZeroOutcome::watchdog(
                    interrupt_asserted,
                    false,
                    None,
                )))
            }
        }
    }

    fn snapshot(&self) -> CpuLocalWatchdogSnapshot {
        CpuLocalWatchdogSnapshot {
            load_value: self.load_value,
            base_value: self.base_value,
            last_updated_tick: self.last_updated_tick,
            control: self.control,
            raw_interrupt: self.raw_interrupt,
            pending_interrupt: self.pending_interrupt,
            raw_reset: self.raw_reset,
            disable_register: self.disable_register,
            clock_tick: self.clock_tick,
            generation: self.generation,
            reset_assertions: self.reset_assertions.clone(),
        }
    }

    fn bump_generation(&mut self) -> Result<(), CpuLocalTimerError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(CpuLocalTimerError::GenerationOverflow)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerBank {
    cpus: Vec<CpuLocalTimerCpu>,
}

impl CpuLocalTimerBank {
    pub fn new(cpu_count: usize, clock_tick: Tick) -> Result<Self, CpuLocalTimerError> {
        if cpu_count == 0 {
            return Err(CpuLocalTimerError::InvalidCpuCount { cpu_count });
        }
        let mut cpus = Vec::with_capacity(cpu_count);
        for _ in 0..cpu_count {
            cpus.push(CpuLocalTimerCpu::new(clock_tick)?);
        }
        Ok(Self { cpus })
    }

    pub fn cpu_count(&self) -> usize {
        self.cpus.len()
    }

    pub fn cpu(&self, index: usize) -> Result<&CpuLocalTimerCpu, CpuLocalTimerError> {
        self.cpus
            .get(index)
            .ok_or(CpuLocalTimerError::UnknownCpu { index })
    }

    pub fn cpu_mut(&mut self, index: usize) -> Result<&mut CpuLocalTimerCpu, CpuLocalTimerError> {
        self.cpus
            .get_mut(index)
            .ok_or(CpuLocalTimerError::UnknownCpu { index })
    }

    pub fn snapshot(&self) -> CpuLocalTimerBankSnapshot {
        CpuLocalTimerBankSnapshot {
            cpus: self.cpus.iter().map(CpuLocalTimerCpu::snapshot).collect(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct CpuLocalTimerInterruptPorts {
    partition: PartitionId,
    timer_source: InterruptSourceId,
    timer_port: InterruptLinePort,
    watchdog_source: InterruptSourceId,
    watchdog_port: InterruptLinePort,
}

impl CpuLocalTimerInterruptPorts {
    pub const fn new(
        partition: PartitionId,
        timer_source: InterruptSourceId,
        timer_port: InterruptLinePort,
        watchdog_source: InterruptSourceId,
        watchdog_port: InterruptLinePort,
    ) -> Self {
        Self {
            partition,
            timer_source,
            timer_port,
            watchdog_source,
            watchdog_port,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CpuLocalTimerMmioDevice {
    base: Address,
    cpu_by_partition: BTreeMap<PartitionId, usize>,
    interrupts: Vec<Option<CpuLocalTimerInterrupts>>,
    state: Arc<Mutex<CpuLocalTimerBank>>,
}

impl CpuLocalTimerMmioDevice {
    pub fn new(
        base: Address,
        bank: CpuLocalTimerBank,
        cpu_partitions: Vec<PartitionId>,
    ) -> Result<Self, CpuLocalTimerError> {
        if cpu_partitions.len() != bank.cpu_count() {
            return Err(CpuLocalTimerError::CpuPartitionCountMismatch {
                cpus: bank.cpu_count(),
                partitions: cpu_partitions.len(),
            });
        }
        let cpu_by_partition = partition_map(cpu_partitions)?;
        Ok(Self {
            base,
            cpu_by_partition,
            interrupts: vec![None; bank.cpu_count()],
            state: Arc::new(Mutex::new(bank)),
        })
    }

    pub fn with_interrupts(
        base: Address,
        bank: CpuLocalTimerBank,
        ports: Vec<CpuLocalTimerInterruptPorts>,
    ) -> Result<Self, CpuLocalTimerError> {
        if ports.len() != bank.cpu_count() {
            return Err(CpuLocalTimerError::CpuPartitionCountMismatch {
                cpus: bank.cpu_count(),
                partitions: ports.len(),
            });
        }
        let cpu_partitions = ports.iter().map(|ports| ports.partition).collect();
        let cpu_by_partition = partition_map(cpu_partitions)?;
        let mut interrupts = Vec::with_capacity(ports.len());
        for ports in ports {
            ports
                .timer_port
                .validate_route()
                .map_err(CpuLocalTimerError::Interrupt)?;
            ports
                .watchdog_port
                .validate_route()
                .map_err(CpuLocalTimerError::Interrupt)?;
            interrupts.push(Some(CpuLocalTimerInterrupts {
                timer: CpuLocalTimerInterrupt {
                    source: ports.timer_source,
                    port: ports.timer_port,
                },
                watchdog: CpuLocalTimerInterrupt {
                    source: ports.watchdog_source,
                    port: ports.watchdog_port,
                },
            }));
        }
        Ok(Self {
            base,
            cpu_by_partition,
            interrupts,
            state: Arc::new(Mutex::new(bank)),
        })
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn range_size_bytes(&self) -> u64 {
        CPU_LOCAL_TIMER_MMIO_SIZE_BYTES
    }

    pub fn snapshot(&self) -> CpuLocalTimerBankSnapshot {
        self.state
            .lock()
            .expect("CPU local timer state lock")
            .snapshot()
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let cpu_index = self.cpu_index_for_partition(context.partition(), request.id())?;
        let (response, effect) = self.respond_request(cpu_index, request, context.now())?;
        self.apply_write_effect(context, cpu_index, effect, request.id())?;
        Ok(response)
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let cpu_index = self.cpu_index_for_partition(context.partition(), request.id())?;
        let (response, effect) = self.respond_request(cpu_index, request, context.now())?;
        self.apply_write_effect_parallel(context, cpu_index, effect, request.id())?;
        Ok(response)
    }

    fn respond_request(
        &self,
        cpu_index: usize,
        request: &MmioRequest,
        tick: Tick,
    ) -> Result<(MmioResponse, CpuLocalTimerWriteEffect), MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        let mut state = self.state.lock().expect("CPU local timer state lock");
        match request.operation() {
            MmioOperation::Read => {
                let value = state
                    .cpu(cpu_index)
                    .and_then(|cpu| cpu.read_register(offset, tick))
                    .map_err(|error| mmio_error(request.id(), error))?;
                Ok((
                    MmioResponse::completed(request.id(), Some(value.to_le_bytes().to_vec())),
                    CpuLocalTimerWriteEffect::none(),
                ))
            }
            MmioOperation::Write => {
                let value = mmio_u32(request)?;
                let effect = state
                    .cpu_mut(cpu_index)
                    .and_then(|cpu| cpu.write_register(offset, value, tick))
                    .map_err(|error| mmio_error(request.id(), error))?;
                Ok((MmioResponse::completed(request.id(), None), effect))
            }
        }
    }

    fn apply_write_effect(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
        effect: CpuLocalTimerWriteEffect,
        request: rem6_mmio::MmioRequestId,
    ) -> Result<(), MmioError> {
        if effect.schedule_timer {
            self.schedule_timer_zero(context, cpu_index)
                .map_err(|error| mmio_error(request, error))?;
        }
        if effect.schedule_watchdog {
            self.schedule_watchdog_zero(context, cpu_index)
                .map_err(|error| mmio_error(request, error))?;
        }
        if effect.deassert_timer {
            self.deassert_timer(context, cpu_index)
                .map_err(|error| mmio_error(request, error))?;
        }
        if effect.deassert_watchdog {
            self.deassert_watchdog(context, cpu_index)
                .map_err(|error| mmio_error(request, error))?;
        }
        Ok(())
    }

    fn apply_write_effect_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
        effect: CpuLocalTimerWriteEffect,
        request: rem6_mmio::MmioRequestId,
    ) -> Result<(), MmioError> {
        if effect.schedule_timer {
            self.schedule_timer_zero_parallel(context, cpu_index)
                .map_err(|error| mmio_error(request, error))?;
        }
        if effect.schedule_watchdog {
            self.schedule_watchdog_zero_parallel(context, cpu_index)
                .map_err(|error| mmio_error(request, error))?;
        }
        if effect.deassert_timer {
            self.deassert_timer_parallel(context, cpu_index)
                .map_err(|error| mmio_error(request, error))?;
        }
        if effect.deassert_watchdog {
            self.deassert_watchdog_parallel(context, cpu_index)
                .map_err(|error| mmio_error(request, error))?;
        }
        Ok(())
    }

    fn schedule_timer_zero(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let (tick, generation) = {
            let state = self.state.lock().expect("CPU local timer state lock");
            let cpu = state.cpu(cpu_index)?;
            let Some(tick) = cpu.next_timer_zero_tick(context.now())? else {
                return Ok(());
            };
            (tick, cpu.snapshot().timer().generation())
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_timer_zero(context, cpu_index, generation);
            })
            .map(|_| ())
            .map_err(CpuLocalTimerError::Scheduler)
    }

    fn schedule_timer_zero_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let (tick, generation) = {
            let state = self.state.lock().expect("CPU local timer state lock");
            let cpu = state.cpu(cpu_index)?;
            let Some(tick) = cpu.next_timer_zero_tick(context.now())? else {
                return Ok(());
            };
            (tick, cpu.snapshot().timer().generation())
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_timer_zero_parallel(context, cpu_index, generation);
            })
            .map(|_| ())
            .map_err(CpuLocalTimerError::Scheduler)
    }

    fn schedule_watchdog_zero(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let (tick, generation) = {
            let state = self.state.lock().expect("CPU local timer state lock");
            let cpu = state.cpu(cpu_index)?;
            let Some(tick) = cpu.next_watchdog_zero_tick(context.now())? else {
                return Ok(());
            };
            (tick, cpu.snapshot().watchdog().generation())
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_watchdog_zero(context, cpu_index, generation);
            })
            .map(|_| ())
            .map_err(CpuLocalTimerError::Scheduler)
    }

    fn schedule_watchdog_zero_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let (tick, generation) = {
            let state = self.state.lock().expect("CPU local timer state lock");
            let cpu = state.cpu(cpu_index)?;
            let Some(tick) = cpu.next_watchdog_zero_tick(context.now())? else {
                return Ok(());
            };
            (tick, cpu.snapshot().watchdog().generation())
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_watchdog_zero_parallel(context, cpu_index, generation);
            })
            .map(|_| ())
            .map_err(CpuLocalTimerError::Scheduler)
    }

    fn fire_timer_zero(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
        generation: u64,
    ) {
        let outcome = match self.record_timer_zero_if_current(context.now(), cpu_index, generation)
        {
            Some(outcome) => outcome,
            None => return,
        };
        if let Some(next_generation) = outcome.next_generation() {
            let _ = self.schedule_timer_zero_generation(context, cpu_index, next_generation);
        }
        if outcome.interrupt_asserted() {
            let _ = self.assert_timer(context, cpu_index);
        }
    }

    fn fire_timer_zero_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
        generation: u64,
    ) {
        let outcome = match self.record_timer_zero_if_current(context.now(), cpu_index, generation)
        {
            Some(outcome) => outcome,
            None => return,
        };
        if let Some(next_generation) = outcome.next_generation() {
            let _ =
                self.schedule_timer_zero_generation_parallel(context, cpu_index, next_generation);
        }
        if outcome.interrupt_asserted() {
            let _ = self.assert_timer_parallel(context, cpu_index);
        }
    }

    fn fire_watchdog_zero(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
        generation: u64,
    ) {
        let outcome =
            match self.record_watchdog_zero_if_current(context.now(), cpu_index, generation) {
                Some(outcome) => outcome,
                None => return,
            };
        if let Some(next_generation) = outcome.next_generation() {
            let _ = self.schedule_watchdog_zero_generation(context, cpu_index, next_generation);
        }
        if outcome.interrupt_asserted() {
            let _ = self.assert_watchdog(context, cpu_index);
        }
    }

    fn fire_watchdog_zero_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
        generation: u64,
    ) {
        let outcome =
            match self.record_watchdog_zero_if_current(context.now(), cpu_index, generation) {
                Some(outcome) => outcome,
                None => return,
            };
        if let Some(next_generation) = outcome.next_generation() {
            let _ = self.schedule_watchdog_zero_generation_parallel(
                context,
                cpu_index,
                next_generation,
            );
        }
        if outcome.interrupt_asserted() {
            let _ = self.assert_watchdog_parallel(context, cpu_index);
        }
    }

    fn schedule_timer_zero_generation(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
        generation: u64,
    ) -> Result<(), CpuLocalTimerError> {
        let tick = {
            let state = self.state.lock().expect("CPU local timer state lock");
            let cpu = state.cpu(cpu_index)?;
            if cpu.snapshot().timer().generation() != generation {
                return Ok(());
            }
            let Some(tick) = cpu.next_timer_zero_tick(context.now())? else {
                return Ok(());
            };
            tick
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_timer_zero(context, cpu_index, generation);
            })
            .map(|_| ())
            .map_err(CpuLocalTimerError::Scheduler)
    }

    fn schedule_timer_zero_generation_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
        generation: u64,
    ) -> Result<(), CpuLocalTimerError> {
        let tick = {
            let state = self.state.lock().expect("CPU local timer state lock");
            let cpu = state.cpu(cpu_index)?;
            if cpu.snapshot().timer().generation() != generation {
                return Ok(());
            }
            let Some(tick) = cpu.next_timer_zero_tick(context.now())? else {
                return Ok(());
            };
            tick
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_timer_zero_parallel(context, cpu_index, generation);
            })
            .map(|_| ())
            .map_err(CpuLocalTimerError::Scheduler)
    }

    fn schedule_watchdog_zero_generation(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
        generation: u64,
    ) -> Result<(), CpuLocalTimerError> {
        let tick = {
            let state = self.state.lock().expect("CPU local timer state lock");
            let cpu = state.cpu(cpu_index)?;
            if cpu.snapshot().watchdog().generation() != generation {
                return Ok(());
            }
            let Some(tick) = cpu.next_watchdog_zero_tick(context.now())? else {
                return Ok(());
            };
            tick
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_watchdog_zero(context, cpu_index, generation);
            })
            .map(|_| ())
            .map_err(CpuLocalTimerError::Scheduler)
    }

    fn schedule_watchdog_zero_generation_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
        generation: u64,
    ) -> Result<(), CpuLocalTimerError> {
        let tick = {
            let state = self.state.lock().expect("CPU local timer state lock");
            let cpu = state.cpu(cpu_index)?;
            if cpu.snapshot().watchdog().generation() != generation {
                return Ok(());
            }
            let Some(tick) = cpu.next_watchdog_zero_tick(context.now())? else {
                return Ok(());
            };
            tick
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(CpuLocalTimerError::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                device.fire_watchdog_zero_parallel(context, cpu_index, generation);
            })
            .map(|_| ())
            .map_err(CpuLocalTimerError::Scheduler)
    }

    fn record_timer_zero_if_current(
        &self,
        tick: Tick,
        cpu_index: usize,
        generation: u64,
    ) -> Option<CpuLocalTimerZeroOutcome> {
        let mut state = self.state.lock().expect("CPU local timer state lock");
        state
            .cpu_mut(cpu_index)
            .ok()?
            .record_timer_zero(tick, generation)
            .ok()?
    }

    fn record_watchdog_zero_if_current(
        &self,
        tick: Tick,
        cpu_index: usize,
        generation: u64,
    ) -> Option<CpuLocalTimerZeroOutcome> {
        let mut state = self.state.lock().expect("CPU local timer state lock");
        state
            .cpu_mut(cpu_index)
            .ok()?
            .record_watchdog_zero(tick, generation)
            .ok()?
    }

    fn assert_timer(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let Some(interrupts) = self.interrupts.get(cpu_index).and_then(Option::as_ref) else {
            return Ok(());
        };
        interrupts
            .timer
            .port
            .assert(context, interrupts.timer.source)
            .map(|_| ())
            .map_err(CpuLocalTimerError::Interrupt)
    }

    fn assert_timer_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let Some(interrupts) = self.interrupts.get(cpu_index).and_then(Option::as_ref) else {
            return Ok(());
        };
        interrupts
            .timer
            .port
            .assert_parallel(context, interrupts.timer.source)
            .map(|_| ())
            .map_err(CpuLocalTimerError::Interrupt)
    }

    fn deassert_timer(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let Some(interrupts) = self.interrupts.get(cpu_index).and_then(Option::as_ref) else {
            return Ok(());
        };
        interrupts
            .timer
            .port
            .deassert(context, interrupts.timer.source)
            .map(|_| ())
            .map_err(CpuLocalTimerError::Interrupt)
    }

    fn deassert_timer_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let Some(interrupts) = self.interrupts.get(cpu_index).and_then(Option::as_ref) else {
            return Ok(());
        };
        interrupts
            .timer
            .port
            .deassert_parallel(context, interrupts.timer.source)
            .map(|_| ())
            .map_err(CpuLocalTimerError::Interrupt)
    }

    fn assert_watchdog(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let Some(interrupts) = self.interrupts.get(cpu_index).and_then(Option::as_ref) else {
            return Ok(());
        };
        interrupts
            .watchdog
            .port
            .assert(context, interrupts.watchdog.source)
            .map(|_| ())
            .map_err(CpuLocalTimerError::Interrupt)
    }

    fn assert_watchdog_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let Some(interrupts) = self.interrupts.get(cpu_index).and_then(Option::as_ref) else {
            return Ok(());
        };
        interrupts
            .watchdog
            .port
            .assert_parallel(context, interrupts.watchdog.source)
            .map(|_| ())
            .map_err(CpuLocalTimerError::Interrupt)
    }

    fn deassert_watchdog(
        &self,
        context: &mut SchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let Some(interrupts) = self.interrupts.get(cpu_index).and_then(Option::as_ref) else {
            return Ok(());
        };
        interrupts
            .watchdog
            .port
            .deassert(context, interrupts.watchdog.source)
            .map(|_| ())
            .map_err(CpuLocalTimerError::Interrupt)
    }

    fn deassert_watchdog_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        cpu_index: usize,
    ) -> Result<(), CpuLocalTimerError> {
        let Some(interrupts) = self.interrupts.get(cpu_index).and_then(Option::as_ref) else {
            return Ok(());
        };
        interrupts
            .watchdog
            .port
            .deassert_parallel(context, interrupts.watchdog.source)
            .map(|_| ())
            .map_err(CpuLocalTimerError::Interrupt)
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        if request.size().bytes() != CPU_LOCAL_TIMER_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: CPU_LOCAL_TIMER_REGISTER_BYTES,
                actual: request.size().bytes(),
            });
        }
        Ok(())
    }

    fn offset(&self, request: &MmioRequest) -> Result<u64, MmioError> {
        let offset = request
            .range()
            .start()
            .get()
            .checked_sub(self.base.get())
            .ok_or(MmioError::UnmappedAddress {
                address: request.range().start(),
            })?;
        if offset >= CPU_LOCAL_TIMER_MMIO_SIZE_BYTES {
            return Err(mmio_error(
                request.id(),
                CpuLocalTimerError::UnknownRegister { offset },
            ));
        }
        Ok(offset)
    }

    fn cpu_index_for_partition(
        &self,
        partition: PartitionId,
        request: rem6_mmio::MmioRequestId,
    ) -> Result<usize, MmioError> {
        self.cpu_by_partition
            .get(&partition)
            .copied()
            .ok_or_else(|| {
                mmio_error(
                    request,
                    CpuLocalTimerError::UnknownCpuPartition { partition },
                )
            })
    }
}

impl MmioDevice for CpuLocalTimerMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        CpuLocalTimerMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        CpuLocalTimerMmioDevice::respond_parallel(self, context, request)
    }
}

#[derive(Clone, Debug)]
struct CpuLocalTimerInterrupts {
    timer: CpuLocalTimerInterrupt,
    watchdog: CpuLocalTimerInterrupt,
}

#[derive(Clone, Debug)]
struct CpuLocalTimerInterrupt {
    source: InterruptSourceId,
    port: InterruptLinePort,
}

fn partition_map(
    cpu_partitions: Vec<PartitionId>,
) -> Result<BTreeMap<PartitionId, usize>, CpuLocalTimerError> {
    let mut by_partition = BTreeMap::new();
    for (index, partition) in cpu_partitions.into_iter().enumerate() {
        if by_partition.insert(partition, index).is_some() {
            return Err(CpuLocalTimerError::DuplicateCpuPartition { partition });
        }
    }
    Ok(by_partition)
}

fn next_zero_deadline(
    last_updated_tick: Tick,
    base_value: u32,
    tick: Tick,
    ticks_per_decrement: Tick,
) -> Result<Tick, CpuLocalTimerError> {
    if tick < last_updated_tick {
        return Err(CpuLocalTimerError::TimeWentBack {
            tick,
            last_updated_tick,
        });
    }
    let decrements = u64::from(base_value).max(1);
    let delay = ticks_per_decrement
        .checked_mul(decrements)
        .ok_or(CpuLocalTimerError::DeadlineOverflow)?;
    let deadline = last_updated_tick
        .checked_add(delay)
        .ok_or(CpuLocalTimerError::DeadlineOverflow)?;
    Ok(deadline.max(tick))
}

fn mmio_u32(request: &MmioRequest) -> Result<u32, MmioError> {
    let data = request.data().ok_or(MmioError::MissingWriteData {
        request: request.id(),
    })?;
    let bytes: [u8; 4] = data
        .try_into()
        .map_err(|_| MmioError::PayloadSizeMismatch {
            request: request.id(),
            expected: CPU_LOCAL_TIMER_REGISTER_BYTES,
            actual: data.len() as u64,
        })?;
    Ok(u32::from_le_bytes(bytes))
}
