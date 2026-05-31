use rem6_kernel::Tick;

use super::{CpuLocalTimerControl, CpuLocalTimerError, CpuLocalWatchdogControl};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerCounterSnapshot {
    pub(super) load_value: u32,
    pub(super) base_value: u32,
    pub(super) last_updated_tick: Tick,
    pub(super) control: CpuLocalTimerControl,
    pub(super) raw_interrupt: bool,
    pub(super) pending_interrupt: bool,
    pub(super) clock_tick: Tick,
    pub(super) generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerCounterSnapshotFields {
    pub load_value: u32,
    pub base_value: u32,
    pub last_updated_tick: Tick,
    pub control: CpuLocalTimerControl,
    pub raw_interrupt: bool,
    pub pending_interrupt: bool,
    pub clock_tick: Tick,
    pub generation: u64,
}

impl CpuLocalTimerCounterSnapshot {
    pub const fn from_fields(fields: CpuLocalTimerCounterSnapshotFields) -> Self {
        Self {
            load_value: fields.load_value,
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

    pub const fn base_value(&self) -> u32 {
        self.base_value
    }

    pub const fn last_updated_tick(&self) -> Tick {
        self.last_updated_tick
    }

    pub const fn control(&self) -> CpuLocalTimerControl {
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
pub struct CpuLocalWatchdogSnapshot {
    pub(super) load_value: u32,
    pub(super) base_value: u32,
    pub(super) last_updated_tick: Tick,
    pub(super) control: CpuLocalWatchdogControl,
    pub(super) raw_interrupt: bool,
    pub(super) pending_interrupt: bool,
    pub(super) raw_reset: bool,
    pub(super) disable_register: u32,
    pub(super) clock_tick: Tick,
    pub(super) generation: u64,
    pub(super) reset_assertions: Vec<Tick>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalWatchdogSnapshotFields {
    pub load_value: u32,
    pub base_value: u32,
    pub last_updated_tick: Tick,
    pub control: CpuLocalWatchdogControl,
    pub raw_interrupt: bool,
    pub pending_interrupt: bool,
    pub raw_reset: bool,
    pub disable_register: u32,
    pub clock_tick: Tick,
    pub generation: u64,
    pub reset_assertions: Vec<Tick>,
}

impl CpuLocalWatchdogSnapshot {
    pub fn from_fields(fields: CpuLocalWatchdogSnapshotFields) -> Self {
        Self {
            load_value: fields.load_value,
            base_value: fields.base_value,
            last_updated_tick: fields.last_updated_tick,
            control: fields.control,
            raw_interrupt: fields.raw_interrupt,
            pending_interrupt: fields.pending_interrupt,
            raw_reset: fields.raw_reset,
            disable_register: fields.disable_register,
            clock_tick: fields.clock_tick,
            generation: fields.generation,
            reset_assertions: fields.reset_assertions,
        }
    }

    pub const fn load_value(&self) -> u32 {
        self.load_value
    }

    pub const fn base_value(&self) -> u32 {
        self.base_value
    }

    pub const fn last_updated_tick(&self) -> Tick {
        self.last_updated_tick
    }

    pub const fn control(&self) -> CpuLocalWatchdogControl {
        self.control
    }

    pub const fn raw_interrupt(&self) -> bool {
        self.raw_interrupt
    }

    pub const fn pending_interrupt(&self) -> bool {
        self.pending_interrupt
    }

    pub const fn raw_reset(&self) -> bool {
        self.raw_reset
    }

    pub const fn disable_register(&self) -> u32 {
        self.disable_register
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
pub struct CpuLocalTimerCpuSnapshot {
    pub(super) timer: CpuLocalTimerCounterSnapshot,
    pub(super) watchdog: CpuLocalWatchdogSnapshot,
}

impl CpuLocalTimerCpuSnapshot {
    pub const fn new(
        timer: CpuLocalTimerCounterSnapshot,
        watchdog: CpuLocalWatchdogSnapshot,
    ) -> Self {
        Self { timer, watchdog }
    }

    pub const fn timer(&self) -> &CpuLocalTimerCounterSnapshot {
        &self.timer
    }

    pub const fn watchdog(&self) -> &CpuLocalWatchdogSnapshot {
        &self.watchdog
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerBankSnapshot {
    pub(super) cpus: Vec<CpuLocalTimerCpuSnapshot>,
}

impl CpuLocalTimerBankSnapshot {
    pub fn new(cpus: Vec<CpuLocalTimerCpuSnapshot>) -> Self {
        Self { cpus }
    }

    pub fn cpus(&self) -> &[CpuLocalTimerCpuSnapshot] {
        &self.cpus
    }

    pub fn cpu(&self, index: usize) -> Result<&CpuLocalTimerCpuSnapshot, CpuLocalTimerError> {
        self.cpus
            .get(index)
            .ok_or(CpuLocalTimerError::UnknownCpu { index })
    }
}
