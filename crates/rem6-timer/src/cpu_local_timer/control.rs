use rem6_kernel::Tick;

use super::CpuLocalTimerError;

const CONTROL_ENABLE: u32 = 1 << 0;
const CONTROL_AUTO_RELOAD: u32 = 1 << 1;
const CONTROL_INTERRUPT_ENABLE: u32 = 1 << 2;
const CONTROL_PRESCALAR_SHIFT: u32 = 8;
const CONTROL_PRESCALAR_MASK: u32 = 0xff << CONTROL_PRESCALAR_SHIFT;
const WATCHDOG_CONTROL_MODE: u32 = 1 << 3;
pub(in crate::cpu_local_timer) const WATCHDOG_DISABLE_FIRST: u32 = 0x1234_5678;
pub(in crate::cpu_local_timer) const WATCHDOG_DISABLE_SECOND: u32 = 0x8765_4321;
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

    pub(in crate::cpu_local_timer) fn ticks_per_decrement(
        self,
        clock_tick: Tick,
    ) -> Result<Tick, CpuLocalTimerError> {
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

    pub(in crate::cpu_local_timer) fn ticks_per_decrement(
        self,
        clock_tick: Tick,
    ) -> Result<Tick, CpuLocalTimerError> {
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
pub struct CpuLocalTimerZeroOutcome {
    interrupt_asserted: bool,
    reset_asserted: bool,
    next_generation: Option<u64>,
}

impl CpuLocalTimerZeroOutcome {
    pub(in crate::cpu_local_timer) const fn timer(
        interrupt_asserted: bool,
        next_generation: Option<u64>,
    ) -> Self {
        Self {
            interrupt_asserted,
            reset_asserted: false,
            next_generation,
        }
    }

    pub(in crate::cpu_local_timer) const fn watchdog(
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
    pub(in crate::cpu_local_timer) schedule_timer: bool,
    pub(in crate::cpu_local_timer) schedule_watchdog: bool,
    pub(in crate::cpu_local_timer) deassert_timer: bool,
    pub(in crate::cpu_local_timer) deassert_watchdog: bool,
}

impl CpuLocalTimerWriteEffect {
    pub(in crate::cpu_local_timer) const fn none() -> Self {
        Self {
            schedule_timer: false,
            schedule_watchdog: false,
            deassert_timer: false,
            deassert_watchdog: false,
        }
    }
}
