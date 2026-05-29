use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptLinePort, InterruptSourceId};
use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerContext, Tick};
use rem6_memory::Address;
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

pub const SP804_LOAD_OFFSET: u64 = 0x00;
pub const SP804_CURRENT_OFFSET: u64 = 0x04;
pub const SP804_CONTROL_OFFSET: u64 = 0x08;
pub const SP804_INT_CLEAR_OFFSET: u64 = 0x0c;
pub const SP804_RAW_ISR_OFFSET: u64 = 0x10;
pub const SP804_MASKED_ISR_OFFSET: u64 = 0x14;
pub const SP804_BGLOAD_OFFSET: u64 = 0x18;
pub const SP804_REGISTER_BYTES: u64 = 4;
pub const SP804_TIMER_WINDOW_BYTES: u64 = 0x20;
pub const SP804_TIMER_COUNT: usize = 2;
pub const SP804_MMIO_SIZE_BYTES: u64 = 0x1000;

const CONTROL_ONE_SHOT: u32 = 1 << 0;
const CONTROL_TIMER_SIZE_32: u32 = 1 << 1;
const CONTROL_PRESCALE_SHIFT: u32 = 2;
const CONTROL_PRESCALE_MASK: u32 = 0b11 << CONTROL_PRESCALE_SHIFT;
const CONTROL_INTERRUPT_ENABLE: u32 = 1 << 5;
const CONTROL_PERIODIC: u32 = 1 << 6;
const CONTROL_ENABLE: u32 = 1 << 7;
const DEFAULT_LOAD_VALUE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sp804TimerControl {
    bits: u32,
}

impl Sp804TimerControl {
    pub const fn new(bits: u32) -> Self {
        Self { bits }
    }

    pub const fn bits(self) -> u32 {
        self.bits
    }

    pub const fn one_shot(self) -> bool {
        self.bits & CONTROL_ONE_SHOT != 0
    }

    pub const fn is_32_bit(self) -> bool {
        self.bits & CONTROL_TIMER_SIZE_32 != 0
    }

    pub const fn prescale(self) -> u32 {
        (self.bits & CONTROL_PRESCALE_MASK) >> CONTROL_PRESCALE_SHIFT
    }

    pub const fn interrupt_enabled(self) -> bool {
        self.bits & CONTROL_INTERRUPT_ENABLE != 0
    }

    pub const fn periodic(self) -> bool {
        self.bits & CONTROL_PERIODIC != 0
    }

    pub const fn enabled(self) -> bool {
        self.bits & CONTROL_ENABLE != 0
    }

    pub const fn with_one_shot(mut self, one_shot: bool) -> Self {
        if one_shot {
            self.bits |= CONTROL_ONE_SHOT;
        } else {
            self.bits &= !CONTROL_ONE_SHOT;
        }
        self
    }

    pub const fn with_32_bit(mut self, is_32_bit: bool) -> Self {
        if is_32_bit {
            self.bits |= CONTROL_TIMER_SIZE_32;
        } else {
            self.bits &= !CONTROL_TIMER_SIZE_32;
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

    pub const fn with_periodic(mut self, periodic: bool) -> Self {
        if periodic {
            self.bits |= CONTROL_PERIODIC;
        } else {
            self.bits &= !CONTROL_PERIODIC;
        }
        self
    }

    pub const fn with_enabled(mut self, enabled: bool) -> Self {
        if enabled {
            self.bits |= CONTROL_ENABLE;
        } else {
            self.bits &= !CONTROL_ENABLE;
        }
        self
    }

    pub const fn with_prescale(self, prescale: u32) -> Result<Self, Sp804Error> {
        if prescale > 2 {
            return Err(Sp804Error::InvalidPrescale { prescale });
        }
        let bits = (self.bits & !CONTROL_PRESCALE_MASK) | (prescale << CONTROL_PRESCALE_SHIFT);
        Ok(Self { bits })
    }

    fn ticks_per_decrement(self, clock_tick: Tick) -> Result<Tick, Sp804Error> {
        let shift = 4 * self.prescale();
        clock_tick
            .checked_shl(shift)
            .ok_or(Sp804Error::DeadlineOverflow)
    }
}

impl Default for Sp804TimerControl {
    fn default() -> Self {
        Self {
            bits: CONTROL_INTERRUPT_ENABLE,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp804TimerSnapshot {
    load_value: u32,
    background_load_value: u32,
    base_value: u32,
    last_updated_tick: Tick,
    control: Sp804TimerControl,
    raw_interrupt: bool,
    pending_interrupt: bool,
    clock_tick: Tick,
    generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Sp804TimerSnapshotFields {
    pub load_value: u32,
    pub background_load_value: u32,
    pub base_value: u32,
    pub last_updated_tick: Tick,
    pub control: Sp804TimerControl,
    pub raw_interrupt: bool,
    pub pending_interrupt: bool,
    pub clock_tick: Tick,
    pub generation: u64,
}

impl Sp804TimerSnapshot {
    pub const fn from_fields(fields: Sp804TimerSnapshotFields) -> Self {
        Self {
            load_value: fields.load_value,
            background_load_value: fields.background_load_value,
            base_value: fields.base_value,
            last_updated_tick: fields.last_updated_tick,
            control: fields.control,
            raw_interrupt: fields.raw_interrupt,
            pending_interrupt: fields.pending_interrupt,
            clock_tick: fields.clock_tick,
            generation: fields.generation,
        }
    }

    pub const fn load_value(&self) -> u32 {
        self.load_value
    }

    pub const fn background_load_value(&self) -> u32 {
        self.background_load_value
    }

    pub const fn base_value(&self) -> u32 {
        self.base_value
    }

    pub const fn last_updated_tick(&self) -> Tick {
        self.last_updated_tick
    }

    pub const fn control(&self) -> Sp804TimerControl {
        self.control
    }

    pub const fn raw_interrupt(&self) -> bool {
        self.raw_interrupt
    }

    pub const fn pending_interrupt(&self) -> bool {
        self.pending_interrupt
    }

    pub const fn clock_tick(&self) -> Tick {
        self.clock_tick
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp804Timer {
    load_value: u32,
    background_load_value: u32,
    base_value: u32,
    last_updated_tick: Tick,
    control: Sp804TimerControl,
    raw_interrupt: bool,
    pending_interrupt: bool,
    clock_tick: Tick,
    generation: u64,
}

impl Sp804Timer {
    pub fn new(clock_tick: Tick) -> Result<Self, Sp804Error> {
        if clock_tick == 0 {
            return Err(Sp804Error::InvalidClockTick { clock_tick });
        }
        Ok(Self {
            load_value: DEFAULT_LOAD_VALUE,
            background_load_value: DEFAULT_LOAD_VALUE,
            base_value: DEFAULT_LOAD_VALUE,
            last_updated_tick: 0,
            control: Sp804TimerControl::default(),
            raw_interrupt: false,
            pending_interrupt: false,
            clock_tick,
            generation: 0,
        })
    }

    pub fn read_register(&self, offset: u64, tick: Tick) -> Result<u32, Sp804Error> {
        match offset {
            SP804_LOAD_OFFSET => Ok(self.load_value),
            SP804_CURRENT_OFFSET => self.current_value(tick),
            SP804_CONTROL_OFFSET => Ok(self.control.bits()),
            SP804_RAW_ISR_OFFSET => Ok(u32::from(self.raw_interrupt)),
            SP804_MASKED_ISR_OFFSET => Ok(u32::from(self.pending_interrupt)),
            SP804_BGLOAD_OFFSET => Ok(self.background_load_value),
            _ => Err(Sp804Error::UnknownRegister { offset }),
        }
    }

    pub fn write_register(
        &mut self,
        offset: u64,
        value: u32,
        tick: Tick,
    ) -> Result<(), Sp804Error> {
        match offset {
            SP804_LOAD_OFFSET => {
                self.load_value = value;
                self.background_load_value = value;
                self.restart_counter(value, tick)
            }
            SP804_CURRENT_OFFSET => Ok(()),
            SP804_CONTROL_OFFSET => self.write_control(value, tick),
            SP804_INT_CLEAR_OFFSET => {
                if value != 0 {
                    self.raw_interrupt = false;
                    self.pending_interrupt = false;
                }
                Ok(())
            }
            SP804_BGLOAD_OFFSET => {
                self.background_load_value = value;
                Ok(())
            }
            _ => Err(Sp804Error::UnknownRegister { offset }),
        }
    }

    pub fn current_value(&self, tick: Tick) -> Result<u32, Sp804Error> {
        if tick < self.last_updated_tick {
            return Err(Sp804Error::TimeWentBack {
                tick,
                last_updated_tick: self.last_updated_tick,
            });
        }
        if !self.control.enabled() {
            return Ok(self.mask_counter(self.base_value));
        }
        let ticks_per_decrement = self.control.ticks_per_decrement(self.clock_tick)?;
        let elapsed_decrements = if ticks_per_decrement == 0 {
            0
        } else {
            (tick - self.last_updated_tick) / ticks_per_decrement
        };
        let base = u64::from(self.mask_counter(self.base_value));
        if elapsed_decrements >= base {
            Ok(0)
        } else {
            Ok((base - elapsed_decrements) as u32)
        }
    }

    pub fn next_zero_tick(&self, now: Tick) -> Result<Option<Tick>, Sp804Error> {
        if !self.control.enabled() {
            return Ok(None);
        }
        let current = self.current_value(now)?;
        if current == 0 {
            return Ok(Some(now));
        }
        let delay = self
            .control
            .ticks_per_decrement(self.clock_tick)?
            .checked_mul(u64::from(current))
            .ok_or(Sp804Error::DeadlineOverflow)?;
        now.checked_add(delay)
            .map(Some)
            .ok_or(Sp804Error::DeadlineOverflow)
    }

    pub fn record_zero(&mut self, tick: Tick) -> Result<(bool, Option<u64>), Sp804Error> {
        if !self.control.enabled() {
            return Ok((false, None));
        }
        self.base_value = 0;
        self.last_updated_tick = tick;
        self.raw_interrupt = true;
        let old_pending = self.pending_interrupt;
        self.pending_interrupt = self.raw_interrupt && self.control.interrupt_enabled();
        let should_signal = self.pending_interrupt && !old_pending;

        if self.control.one_shot() {
            self.control = self.control.with_enabled(false);
            self.generation = self.next_generation()?;
            return Ok((should_signal, None));
        }

        let reload = if self.control.periodic() {
            self.background_load_value
        } else if self.control.is_32_bit() {
            u32::MAX
        } else {
            u16::MAX.into()
        };
        self.base_value = reload;
        self.last_updated_tick = tick;
        self.generation = self.next_generation()?;
        Ok((should_signal, Some(self.generation)))
    }

    pub fn snapshot(&self) -> Sp804TimerSnapshot {
        Sp804TimerSnapshot {
            load_value: self.load_value,
            background_load_value: self.background_load_value,
            base_value: self.base_value,
            last_updated_tick: self.last_updated_tick,
            control: self.control,
            raw_interrupt: self.raw_interrupt,
            pending_interrupt: self.pending_interrupt,
            clock_tick: self.clock_tick,
            generation: self.generation,
        }
    }

    pub fn restore(&mut self, snapshot: &Sp804TimerSnapshot) -> Result<(), Sp804Error> {
        if snapshot.clock_tick == 0 {
            return Err(Sp804Error::InvalidClockTick {
                clock_tick: snapshot.clock_tick,
            });
        }
        if snapshot.pending_interrupt
            && !(snapshot.raw_interrupt && snapshot.control.interrupt_enabled())
        {
            return Err(Sp804Error::InvalidPendingInterrupt);
        }
        self.load_value = snapshot.load_value;
        self.background_load_value = snapshot.background_load_value;
        self.base_value = snapshot.base_value;
        self.last_updated_tick = snapshot.last_updated_tick;
        self.control = snapshot.control;
        self.raw_interrupt = snapshot.raw_interrupt;
        self.pending_interrupt = snapshot.pending_interrupt;
        self.clock_tick = snapshot.clock_tick;
        self.generation = snapshot.generation;
        Ok(())
    }

    fn write_control(&mut self, value: u32, tick: Tick) -> Result<(), Sp804Error> {
        let old_enabled = self.control.enabled();
        let current = self.current_value(tick)?;
        let new_control = Sp804TimerControl::new(value);
        if new_control.prescale() > 2 {
            return Err(Sp804Error::InvalidPrescale {
                prescale: new_control.prescale(),
            });
        }
        self.control = new_control;
        self.pending_interrupt = self.raw_interrupt && self.control.interrupt_enabled();
        if !old_enabled && self.control.enabled() {
            self.restart_counter(self.load_value, tick)
        } else {
            self.base_value = current;
            self.last_updated_tick = tick;
            self.generation = self.next_generation()?;
            Ok(())
        }
    }

    fn restart_counter(&mut self, value: u32, tick: Tick) -> Result<(), Sp804Error> {
        self.base_value = self.mask_counter(value);
        self.last_updated_tick = tick;
        self.generation = self.next_generation()?;
        Ok(())
    }

    fn mask_counter(&self, value: u32) -> u32 {
        if self.control.is_32_bit() {
            value
        } else {
            value & u32::from(u16::MAX)
        }
    }

    fn next_generation(&self) -> Result<u64, Sp804Error> {
        self.generation
            .checked_add(1)
            .ok_or(Sp804Error::GenerationOverflow)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp804DualTimerSnapshot {
    timers: [Sp804TimerSnapshot; SP804_TIMER_COUNT],
}

impl Sp804DualTimerSnapshot {
    pub const fn new(timers: [Sp804TimerSnapshot; SP804_TIMER_COUNT]) -> Self {
        Self { timers }
    }

    pub fn timer(&self, index: usize) -> Result<&Sp804TimerSnapshot, Sp804Error> {
        self.timers
            .get(index)
            .ok_or(Sp804Error::UnknownTimer { index })
    }

    pub fn timers(&self) -> &[Sp804TimerSnapshot; SP804_TIMER_COUNT] {
        &self.timers
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp804DualTimer {
    timers: [Sp804Timer; SP804_TIMER_COUNT],
}

impl Sp804DualTimer {
    pub fn new(clock0: Tick, clock1: Tick) -> Result<Self, Sp804Error> {
        Ok(Self {
            timers: [Sp804Timer::new(clock0)?, Sp804Timer::new(clock1)?],
        })
    }

    pub fn timer(&self, index: usize) -> Result<&Sp804Timer, Sp804Error> {
        self.timers
            .get(index)
            .ok_or(Sp804Error::UnknownTimer { index })
    }

    pub fn timer_mut(&mut self, index: usize) -> Result<&mut Sp804Timer, Sp804Error> {
        self.timers
            .get_mut(index)
            .ok_or(Sp804Error::UnknownTimer { index })
    }

    pub fn snapshot(&self) -> Sp804DualTimerSnapshot {
        Sp804DualTimerSnapshot::new([self.timers[0].snapshot(), self.timers[1].snapshot()])
    }

    pub fn restore(&mut self, snapshot: &Sp804DualTimerSnapshot) -> Result<(), Sp804Error> {
        self.timers[0].restore(&snapshot.timers[0])?;
        self.timers[1].restore(&snapshot.timers[1])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp804DualTimerMmioSnapshot {
    timers: Sp804DualTimerSnapshot,
}

impl Sp804DualTimerMmioSnapshot {
    pub const fn new(timers: Sp804DualTimerSnapshot) -> Self {
        Self { timers }
    }

    pub const fn timers(&self) -> &Sp804DualTimerSnapshot {
        &self.timers
    }

    pub fn timer(&self, index: usize) -> Result<&Sp804TimerSnapshot, Sp804Error> {
        self.timers.timer(index)
    }
}

#[derive(Clone, Debug)]
pub struct Sp804DualTimerMmioDevice {
    base: Address,
    interrupts: [Option<Sp804Interrupt>; SP804_TIMER_COUNT],
    state: Arc<Mutex<Sp804DualTimer>>,
}

impl Sp804DualTimerMmioDevice {
    pub fn new(base: Address, timers: Sp804DualTimer) -> Self {
        Self {
            base,
            interrupts: [None, None],
            state: Arc::new(Mutex::new(timers)),
        }
    }

    pub fn with_interrupts(
        base: Address,
        timers: Sp804DualTimer,
        _partition: PartitionId,
        interrupts: [(InterruptSourceId, InterruptLinePort); SP804_TIMER_COUNT],
    ) -> Result<Self, Sp804Error> {
        let [(source0, port0), (source1, port1)] = interrupts;
        port0.validate_route().map_err(Sp804Error::Interrupt)?;
        port1.validate_route().map_err(Sp804Error::Interrupt)?;
        Ok(Self {
            base,
            interrupts: [
                Some(Sp804Interrupt {
                    source: source0,
                    port: port0,
                }),
                Some(Sp804Interrupt {
                    source: source1,
                    port: port1,
                }),
            ],
            state: Arc::new(Mutex::new(timers)),
        })
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn range_size_bytes(&self) -> u64 {
        SP804_MMIO_SIZE_BYTES
    }

    pub fn snapshot(&self) -> Sp804DualTimerMmioSnapshot {
        Sp804DualTimerMmioSnapshot::new(
            self.state
                .lock()
                .expect("SP804 dual timer state lock")
                .snapshot(),
        )
    }

    pub fn restore(&self, snapshot: &Sp804DualTimerMmioSnapshot) -> Result<(), Sp804Error> {
        self.state
            .lock()
            .expect("SP804 dual timer state lock")
            .restore(snapshot.timers())
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let (response, schedule_timer) = self.respond_request(request, context.now())?;
        if let Some(timer_index) = schedule_timer {
            self.schedule_zero(context, timer_index)
                .map_err(|error| mmio_error(request.id(), error))?;
        }
        Ok(response)
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let (response, schedule_timer) = self.respond_request(request, context.now())?;
        if let Some(timer_index) = schedule_timer {
            self.schedule_zero_parallel(context, timer_index)
                .map_err(|error| mmio_error(request.id(), error))?;
        }
        Ok(response)
    }

    fn respond_request(
        &self,
        request: &MmioRequest,
        tick: Tick,
    ) -> Result<(MmioResponse, Option<usize>), MmioError> {
        self.validate_size(request)?;
        let (timer_index, offset) = self.decode_offset(request)?;
        let mut state = self.state.lock().expect("SP804 dual timer state lock");
        match request.operation() {
            MmioOperation::Read => {
                let value = state
                    .timer(timer_index)
                    .and_then(|timer| timer.read_register(offset, tick))
                    .map_err(|error| mmio_error(request.id(), error))?;
                Ok((
                    MmioResponse::completed(request.id(), Some(value.to_le_bytes().to_vec())),
                    None,
                ))
            }
            MmioOperation::Write => {
                let value = mmio_u32(request)?;
                state
                    .timer_mut(timer_index)
                    .and_then(|timer| timer.write_register(offset, value, tick))
                    .map_err(|error| mmio_error(request.id(), error))?;
                let schedule_timer = match offset {
                    SP804_LOAD_OFFSET | SP804_CONTROL_OFFSET => Some(timer_index),
                    _ => None,
                };
                Ok((MmioResponse::completed(request.id(), None), schedule_timer))
            }
        }
    }

    fn schedule_zero(
        &self,
        context: &mut SchedulerContext<'_>,
        timer_index: usize,
    ) -> Result<(), Sp804Error> {
        let (tick, generation) = {
            let state = self.state.lock().expect("SP804 dual timer state lock");
            let timer = state.timer(timer_index)?;
            let Some(tick) = timer.next_zero_tick(context.now())? else {
                return Ok(());
            };
            (tick, timer.snapshot().generation())
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(Sp804Error::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let timer_device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                timer_device.fire_zero(context, timer_index, generation);
            })
            .map(|_| ())
            .map_err(Sp804Error::Scheduler)
    }

    fn schedule_zero_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        timer_index: usize,
    ) -> Result<(), Sp804Error> {
        let (tick, generation) = {
            let state = self.state.lock().expect("SP804 dual timer state lock");
            let timer = state.timer(timer_index)?;
            let Some(tick) = timer.next_zero_tick(context.now())? else {
                return Ok(());
            };
            (tick, timer.snapshot().generation())
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(Sp804Error::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let timer_device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                timer_device.fire_zero_parallel(context, timer_index, generation);
            })
            .map(|_| ())
            .map_err(Sp804Error::Scheduler)
    }

    fn fire_zero(&self, context: &mut SchedulerContext<'_>, timer_index: usize, generation: u64) {
        let (interrupt, next_generation) =
            match self.record_zero_if_current(context.now(), timer_index, generation) {
                Some(result) => result,
                None => return,
            };
        if let Some(next_generation) = next_generation {
            let _ = self.schedule_zero_generation(context, timer_index, next_generation);
        }
        if let Some(interrupt) = interrupt {
            if interrupt.port.assert(context, interrupt.source).is_ok() {
                let _ = interrupt.port.deassert(context, interrupt.source);
            }
        }
    }

    fn fire_zero_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        timer_index: usize,
        generation: u64,
    ) {
        let (interrupt, next_generation) =
            match self.record_zero_if_current(context.now(), timer_index, generation) {
                Some(result) => result,
                None => return,
            };
        if let Some(next_generation) = next_generation {
            let _ = self.schedule_zero_generation_parallel(context, timer_index, next_generation);
        }
        if let Some(interrupt) = interrupt {
            if interrupt
                .port
                .assert_parallel(context, interrupt.source)
                .is_ok()
            {
                let _ = interrupt.port.deassert_parallel(context, interrupt.source);
            }
        }
    }

    fn schedule_zero_generation(
        &self,
        context: &mut SchedulerContext<'_>,
        timer_index: usize,
        generation: u64,
    ) -> Result<(), Sp804Error> {
        let tick = {
            let state = self.state.lock().expect("SP804 dual timer state lock");
            let timer = state.timer(timer_index)?;
            if timer.snapshot().generation() != generation {
                return Ok(());
            }
            let Some(tick) = timer.next_zero_tick(context.now())? else {
                return Ok(());
            };
            tick
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(Sp804Error::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let timer_device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                timer_device.fire_zero(context, timer_index, generation);
            })
            .map(|_| ())
            .map_err(Sp804Error::Scheduler)
    }

    fn schedule_zero_generation_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        timer_index: usize,
        generation: u64,
    ) -> Result<(), Sp804Error> {
        let tick = {
            let state = self.state.lock().expect("SP804 dual timer state lock");
            let timer = state.timer(timer_index)?;
            if timer.snapshot().generation() != generation {
                return Ok(());
            }
            let Some(tick) = timer.next_zero_tick(context.now())? else {
                return Ok(());
            };
            tick
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(Sp804Error::TimeWentBack {
                tick,
                last_updated_tick: context.now(),
            })?;
        let timer_device = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                timer_device.fire_zero_parallel(context, timer_index, generation);
            })
            .map(|_| ())
            .map_err(Sp804Error::Scheduler)
    }

    fn record_zero_if_current(
        &self,
        tick: Tick,
        timer_index: usize,
        generation: u64,
    ) -> Option<(Option<Sp804Interrupt>, Option<u64>)> {
        let mut state = self.state.lock().expect("SP804 dual timer state lock");
        let timer = state.timer_mut(timer_index).ok()?;
        if timer.snapshot().generation() != generation {
            return None;
        }
        let (should_signal, next_generation) = timer.record_zero(tick).ok()?;
        drop(state);
        let interrupt = if should_signal {
            self.interrupts[timer_index].clone()
        } else {
            None
        };
        Some((interrupt, next_generation))
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        if request.size().bytes() != SP804_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: SP804_REGISTER_BYTES,
                actual: request.size().bytes(),
            });
        }
        Ok(())
    }

    fn decode_offset(&self, request: &MmioRequest) -> Result<(usize, u64), MmioError> {
        let offset = request
            .range()
            .start()
            .get()
            .checked_sub(self.base.get())
            .ok_or(MmioError::UnmappedAddress {
                address: request.range().start(),
            })?;
        let timer_index = (offset / SP804_TIMER_WINDOW_BYTES) as usize;
        let timer_offset = offset % SP804_TIMER_WINDOW_BYTES;
        if timer_index >= SP804_TIMER_COUNT {
            return Err(mmio_error(
                request.id(),
                Sp804Error::UnknownRegister { offset },
            ));
        }
        Ok((timer_index, timer_offset))
    }
}

impl MmioDevice for Sp804DualTimerMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Sp804DualTimerMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Sp804DualTimerMmioDevice::respond_parallel(self, context, request)
    }
}

#[derive(Clone, Debug)]
struct Sp804Interrupt {
    source: InterruptSourceId,
    port: InterruptLinePort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Sp804Error {
    InvalidClockTick { clock_tick: Tick },
    InvalidPrescale { prescale: u32 },
    UnknownRegister { offset: u64 },
    UnknownTimer { index: usize },
    TimeWentBack { tick: Tick, last_updated_tick: Tick },
    DeadlineOverflow,
    GenerationOverflow,
    InvalidPendingInterrupt,
    Interrupt(InterruptError),
    Scheduler(rem6_kernel::SchedulerError),
}

impl fmt::Display for Sp804Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidClockTick { clock_tick } => {
                write!(
                    formatter,
                    "SP804 clock tick must be positive, got {clock_tick}"
                )
            }
            Self::InvalidPrescale { prescale } => {
                write!(
                    formatter,
                    "SP804 prescale must be 0, 1, or 2, got {prescale}"
                )
            }
            Self::UnknownRegister { offset } => {
                write!(formatter, "unknown SP804 timer register offset {offset:#x}")
            }
            Self::UnknownTimer { index } => {
                write!(formatter, "unknown SP804 timer index {index}")
            }
            Self::TimeWentBack {
                tick,
                last_updated_tick,
            } => write!(
                formatter,
                "SP804 tick {tick} is earlier than last updated tick {last_updated_tick}"
            ),
            Self::DeadlineOverflow => write!(formatter, "SP804 deadline overflowed"),
            Self::GenerationOverflow => write!(formatter, "SP804 generation overflowed"),
            Self::InvalidPendingInterrupt => {
                write!(
                    formatter,
                    "SP804 pending interrupt snapshot is inconsistent"
                )
            }
            Self::Interrupt(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for Sp804Error {
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
            expected: SP804_REGISTER_BYTES,
            actual: data.len() as u64,
        })?;
    Ok(u32::from_le_bytes(bytes))
}

fn mmio_error(request: rem6_mmio::MmioRequestId, error: Sp804Error) -> MmioError {
    MmioError::DeviceError {
        request,
        message: error.to_string(),
    }
}
