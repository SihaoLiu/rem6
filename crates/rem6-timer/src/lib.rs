use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptLinePort, InterruptSourceId};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, SchedulerContext, SchedulerError, Tick,
};
use rem6_memory::{Address, ByteMask};
use rem6_mmio::{MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

mod clint;
mod cpu_local_timer;
mod pl031;
mod rtc;
mod sp804;
mod sp805;

pub use self::clint::{
    ClintHartConfig, ClintHartSnapshot, ClintId, ClintMmioDevice, ClintResetPolicy, ClintSnapshot,
    ClintTimebase, RiscvRtcSource, CLINT_MSIP_BASE_OFFSET, CLINT_MSIP_REGISTER_BYTES,
    CLINT_MSIP_STRIDE, CLINT_MTIMECMP_BASE_OFFSET, CLINT_MTIMECMP_REGISTER_BYTES,
    CLINT_MTIMECMP_STRIDE, CLINT_MTIME_OFFSET, CLINT_MTIME_REGISTER_BYTES,
};
pub use self::cpu_local_timer::{
    CpuLocalTimerBank, CpuLocalTimerBankSnapshot, CpuLocalTimerControl,
    CpuLocalTimerCounterSnapshot, CpuLocalTimerCounterSnapshotFields, CpuLocalTimerCpu,
    CpuLocalTimerCpuSnapshot, CpuLocalTimerError, CpuLocalTimerInterruptPorts,
    CpuLocalTimerMmioDevice, CpuLocalTimerWriteEffect, CpuLocalTimerZeroOutcome,
    CpuLocalWatchdogControl, CpuLocalWatchdogSnapshot, CpuLocalWatchdogSnapshotFields,
    CPU_LOCAL_TIMER_CONTROL_OFFSET, CPU_LOCAL_TIMER_COUNTER_OFFSET,
    CPU_LOCAL_TIMER_INT_STATUS_OFFSET, CPU_LOCAL_TIMER_LOAD_OFFSET,
    CPU_LOCAL_TIMER_MMIO_SIZE_BYTES, CPU_LOCAL_TIMER_REGISTER_BYTES,
    CPU_LOCAL_WATCHDOG_CONTROL_OFFSET, CPU_LOCAL_WATCHDOG_COUNTER_OFFSET,
    CPU_LOCAL_WATCHDOG_DISABLE_OFFSET, CPU_LOCAL_WATCHDOG_INT_STATUS_OFFSET,
    CPU_LOCAL_WATCHDOG_LOAD_OFFSET, CPU_LOCAL_WATCHDOG_RESET_STATUS_OFFSET,
};
pub use self::pl031::{
    Pl031Error, Pl031Rtc, Pl031RtcMmioDevice, Pl031RtcMmioSnapshot, Pl031Snapshot,
    Pl031SnapshotFields, PL031_CONTROL_OFFSET, PL031_DATA_OFFSET, PL031_INT_CLEAR_OFFSET,
    PL031_INT_MASK_OFFSET, PL031_LOAD_OFFSET, PL031_MASKED_ISR_OFFSET, PL031_MATCH_OFFSET,
    PL031_MMIO_SIZE_BYTES, PL031_PRIMECELL_ID, PL031_RAW_ISR_OFFSET, PL031_REGISTER_BYTES,
};
pub use self::rtc::{
    Mc146818Rtc, Mc146818RtcMmioDevice, Mc146818RtcMmioSnapshot, RtcDateTime, RtcEncoding,
    RtcError, RtcInterruptError, RtcInterruptErrorKind, RtcInterruptFlags, RtcSnapshot,
    RTC_CMOS_REGISTER_COUNT, RTC_DAY_OF_MONTH_REGISTER, RTC_DAY_OF_WEEK_REGISTER,
    RTC_HOURS_ALARM_REGISTER, RTC_HOURS_REGISTER, RTC_MINUTES_ALARM_REGISTER, RTC_MINUTES_REGISTER,
    RTC_MMIO_ADDRESS_OFFSET, RTC_MMIO_DATA_OFFSET, RTC_MMIO_REGISTER_BYTES, RTC_MONTH_REGISTER,
    RTC_SECONDS_ALARM_REGISTER, RTC_SECONDS_REGISTER, RTC_STATUS_A_REGISTER, RTC_STATUS_B_REGISTER,
    RTC_STATUS_C_AF, RTC_STATUS_C_IRQF, RTC_STATUS_C_PF, RTC_STATUS_C_REGISTER, RTC_STATUS_C_UF,
    RTC_STATUS_D_REGISTER, RTC_YEARS_REGISTER,
};
pub use self::sp804::{
    Sp804DualTimer, Sp804DualTimerMmioDevice, Sp804DualTimerMmioSnapshot, Sp804DualTimerSnapshot,
    Sp804Error, Sp804Timer, Sp804TimerControl, Sp804TimerSnapshot, Sp804TimerSnapshotFields,
    SP804_BGLOAD_OFFSET, SP804_CONTROL_OFFSET, SP804_CURRENT_OFFSET, SP804_INT_CLEAR_OFFSET,
    SP804_LOAD_OFFSET, SP804_MASKED_ISR_OFFSET, SP804_MMIO_SIZE_BYTES, SP804_PRIMECELL_ID,
    SP804_RAW_ISR_OFFSET, SP804_REGISTER_BYTES, SP804_TIMER_COUNT, SP804_TIMER_WINDOW_BYTES,
};
pub use self::sp805::{
    Sp805Error, Sp805TimeoutOutcome, Sp805Watchdog, Sp805WatchdogMmioDevice,
    Sp805WatchdogMmioSnapshot, Sp805WatchdogSnapshot, Sp805WatchdogSnapshotFields,
    SP805_CONTROL_OFFSET, SP805_INT_CLEAR_OFFSET, SP805_ITCR_OFFSET, SP805_ITOP_OFFSET,
    SP805_LOAD_OFFSET, SP805_LOCK_MAGIC, SP805_LOCK_OFFSET, SP805_MASKED_ISR_OFFSET,
    SP805_MMIO_SIZE_BYTES, SP805_PRIMECELL_ID, SP805_RAW_ISR_OFFSET, SP805_REGISTER_BYTES,
    SP805_VALUE_OFFSET,
};
pub use rem6_amba::{
    ArmPrimecellId, AMBA_CELL_ID0_OFFSET, AMBA_CELL_ID1_OFFSET, AMBA_CELL_ID2_OFFSET,
    AMBA_CELL_ID3_OFFSET, AMBA_PERIPHERAL_ID0_OFFSET, AMBA_PERIPHERAL_ID1_OFFSET,
    AMBA_PERIPHERAL_ID2_OFFSET, AMBA_PERIPHERAL_ID3_OFFSET,
};

pub const TIMER_MMIO_REGISTER_BYTES: u64 = 8;
pub const TIMER_MMIO_TIME_OFFSET: u64 = 0x00;
pub const TIMER_MMIO_DEADLINE_OFFSET: u64 = 0x08;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimerId(u64);

impl TimerId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerArm {
    generation: u64,
    programmed_tick: Tick,
    deadline: Tick,
}

impl TimerArm {
    pub const fn new(generation: u64, programmed_tick: Tick, deadline: Tick) -> Self {
        Self {
            generation,
            programmed_tick,
            deadline,
        }
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn programmed_tick(self) -> Tick {
        self.programmed_tick
    }

    pub const fn deadline(self) -> Tick {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimerExpiry {
    generation: u64,
    deadline: Tick,
}

impl TimerExpiry {
    pub const fn new(generation: u64, deadline: Tick) -> Self {
        Self {
            generation,
            deadline,
        }
    }

    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub const fn deadline(self) -> Tick {
        self.deadline
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerSignalError {
    generation: u64,
    tick: Tick,
    error: InterruptError,
}

impl TimerSignalError {
    pub const fn new(generation: u64, tick: Tick, error: InterruptError) -> Self {
        Self {
            generation,
            tick,
            error,
        }
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn error(&self) -> &InterruptError {
        &self.error
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimerSnapshot {
    id: TimerId,
    partition: PartitionId,
    source: InterruptSourceId,
    next_deadline: Option<Tick>,
    arms: Vec<TimerArm>,
    expiries: Vec<TimerExpiry>,
    signal_errors: Vec<TimerSignalError>,
}

impl TimerSnapshot {
    pub fn new(
        id: TimerId,
        partition: PartitionId,
        source: InterruptSourceId,
        next_deadline: Option<Tick>,
        arms: Vec<TimerArm>,
        expiries: Vec<TimerExpiry>,
        signal_errors: Vec<TimerSignalError>,
    ) -> Self {
        Self {
            id,
            partition,
            source,
            next_deadline,
            arms,
            expiries,
            signal_errors,
        }
    }

    pub const fn id(&self) -> TimerId {
        self.id
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn next_deadline(&self) -> Option<Tick> {
        self.next_deadline
    }

    pub fn arms(&self) -> &[TimerArm] {
        &self.arms
    }

    pub fn expiries(&self) -> &[TimerExpiry] {
        &self.expiries
    }

    pub fn signal_errors(&self) -> &[TimerSignalError] {
        &self.signal_errors
    }
}

#[derive(Clone, Debug)]
pub struct ProgrammableTimer {
    id: TimerId,
    partition: PartitionId,
    source: InterruptSourceId,
    interrupt: InterruptLinePort,
    state: Arc<Mutex<TimerState>>,
}

impl ProgrammableTimer {
    pub fn new(
        id: TimerId,
        partition: PartitionId,
        source: InterruptSourceId,
        interrupt: InterruptLinePort,
    ) -> Self {
        Self {
            id,
            partition,
            source,
            interrupt,
            state: Arc::new(Mutex::new(TimerState::new())),
        }
    }

    pub const fn id(&self) -> TimerId {
        self.id
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn source(&self) -> InterruptSourceId {
        self.source
    }

    pub const fn interrupt(&self) -> &InterruptLinePort {
        &self.interrupt
    }

    pub fn arm_at(
        &self,
        context: &mut SchedulerContext<'_>,
        deadline: Tick,
    ) -> Result<PartitionEventId, TimerError> {
        let now = context.now();
        if deadline < now {
            return Err(TimerError::DeadlineInPast { now, deadline });
        }
        self.interrupt
            .validate_route()
            .map_err(TimerError::Interrupt)?;

        let delay = deadline - now;
        let state = Arc::clone(&self.state);
        let interrupt = self.interrupt.clone();
        let source = self.source;
        let mut timer_state = self.state.lock().expect("timer state lock");
        let generation = timer_state.next_generation();

        let event_id = context
            .schedule_remote_after(self.partition, delay, move |context| {
                let should_fire = state
                    .lock()
                    .expect("timer state lock")
                    .expire(generation, context.now());
                if should_fire {
                    if let Err(error) = interrupt.assert(context, source) {
                        state.lock().expect("timer state lock").record_signal_error(
                            generation,
                            context.now(),
                            error,
                        );
                    }
                }
            })
            .map_err(TimerError::Scheduler)?;
        timer_state.arm_generation(generation, now, deadline);
        Ok(event_id)
    }

    pub fn arm_at_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        deadline: Tick,
    ) -> Result<PartitionEventId, TimerError> {
        let now = context.now();
        if deadline < now {
            return Err(TimerError::DeadlineInPast { now, deadline });
        }
        self.interrupt
            .validate_route()
            .map_err(TimerError::Interrupt)?;

        let delay = deadline - now;
        let state = Arc::clone(&self.state);
        let interrupt = self.interrupt.clone();
        let source = self.source;
        let mut timer_state = self.state.lock().expect("timer state lock");
        let generation = timer_state.next_generation();

        let event_id = context
            .schedule_remote_after(self.partition, delay, move |context| {
                let should_fire = state
                    .lock()
                    .expect("timer state lock")
                    .expire(generation, context.now());
                if should_fire {
                    if let Err(error) = interrupt.assert_parallel(context, source) {
                        state.lock().expect("timer state lock").record_signal_error(
                            generation,
                            context.now(),
                            error,
                        );
                    }
                }
            })
            .map_err(TimerError::Scheduler)?;
        timer_state.arm_generation(generation, now, deadline);
        Ok(event_id)
    }

    pub fn snapshot(&self) -> TimerSnapshot {
        let state = self.state.lock().expect("timer state lock");
        TimerSnapshot::new(
            self.id,
            self.partition,
            self.source,
            state.next_deadline,
            state.arms.clone(),
            state.expiries.clone(),
            state.signal_errors.clone(),
        )
    }

    pub fn restore(&self, snapshot: &TimerSnapshot) -> Result<(), TimerError> {
        self.validate_snapshot_identity(snapshot)?;
        *self.state.lock().expect("timer state lock") = TimerState::from_snapshot(snapshot);
        Ok(())
    }

    fn validate_snapshot_identity(&self, snapshot: &TimerSnapshot) -> Result<(), TimerError> {
        if self.id == snapshot.id()
            && self.partition == snapshot.partition()
            && self.source == snapshot.source()
        {
            return Ok(());
        }

        Err(TimerError::SnapshotIdentityMismatch {
            expected_id: self.id,
            actual_id: snapshot.id(),
            expected_partition: self.partition,
            actual_partition: snapshot.partition(),
            expected_source: self.source,
            actual_source: snapshot.source(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct TimerMmioDevice {
    timer: ProgrammableTimer,
    base: Address,
}

impl TimerMmioDevice {
    pub const fn new(timer: ProgrammableTimer, base: Address) -> Self {
        Self { timer, base }
    }

    pub const fn timer(&self) -> &ProgrammableTimer {
        &self.timer
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        match (offset, request.operation()) {
            (TIMER_MMIO_TIME_OFFSET, MmioOperation::Read) => Ok(MmioResponse::completed(
                request.id(),
                Some(le64(context.now())),
            )),
            (TIMER_MMIO_TIME_OFFSET, MmioOperation::Write) => Err(MmioError::AccessDenied {
                request: request.id(),
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            }),
            (TIMER_MMIO_DEADLINE_OFFSET, MmioOperation::Read) => {
                let deadline = self.timer.snapshot().next_deadline().unwrap_or_default();
                Ok(MmioResponse::completed(request.id(), Some(le64(deadline))))
            }
            (TIMER_MMIO_DEADLINE_OFFSET, MmioOperation::Write) => {
                let deadline = self.deadline_from_write(request)?;
                self.timer
                    .arm_at(context, deadline)
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

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        match (offset, request.operation()) {
            (TIMER_MMIO_TIME_OFFSET, MmioOperation::Read) => Ok(MmioResponse::completed(
                request.id(),
                Some(le64(context.now())),
            )),
            (TIMER_MMIO_TIME_OFFSET, MmioOperation::Write) => Err(MmioError::AccessDenied {
                request: request.id(),
                operation: MmioOperation::Write,
                access: MmioAccess::ReadOnly,
            }),
            (TIMER_MMIO_DEADLINE_OFFSET, MmioOperation::Read) => {
                let deadline = self.timer.snapshot().next_deadline().unwrap_or_default();
                Ok(MmioResponse::completed(request.id(), Some(le64(deadline))))
            }
            (TIMER_MMIO_DEADLINE_OFFSET, MmioOperation::Write) => {
                let deadline = self.deadline_from_write(request)?;
                self.timer
                    .arm_at_parallel(context, deadline)
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
        if request.size().bytes() != TIMER_MMIO_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: TIMER_MMIO_REGISTER_BYTES,
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

    fn deadline_from_write(&self, request: &MmioRequest) -> Result<Tick, MmioError> {
        let data = request.data().ok_or(MmioError::MissingWriteData {
            request: request.id(),
        })?;
        if data.len() as u64 != TIMER_MMIO_REGISTER_BYTES {
            return Err(MmioError::PayloadSizeMismatch {
                request: request.id(),
                expected: TIMER_MMIO_REGISTER_BYTES,
                actual: data.len() as u64,
            });
        }

        let mut bytes = le64(self.timer.snapshot().next_deadline().unwrap_or_default());
        let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
            request: request.id(),
        })?;
        validate_timer_mmio_mask(request, mask)?;
        for (index, byte) in data.iter().enumerate() {
            if mask.bits()[index] {
                bytes[index] = *byte;
            }
        }

        let mut deadline = [0; 8];
        deadline.copy_from_slice(&bytes);
        Ok(Tick::from_le_bytes(deadline))
    }
}

impl MmioDevice for TimerMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        TimerMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        TimerMmioDevice::respond_parallel(self, context, request)
    }
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn validate_timer_mmio_mask(request: &MmioRequest, mask: &ByteMask) -> Result<(), MmioError> {
    if mask.len() != TIMER_MMIO_REGISTER_BYTES {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected: TIMER_MMIO_REGISTER_BYTES,
            actual: mask.len(),
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TimerState {
    generation: u64,
    next_deadline: Option<Tick>,
    arms: Vec<TimerArm>,
    expiries: Vec<TimerExpiry>,
    signal_errors: Vec<TimerSignalError>,
}

impl TimerState {
    const fn new() -> Self {
        Self {
            generation: 0,
            next_deadline: None,
            arms: Vec::new(),
            expiries: Vec::new(),
            signal_errors: Vec::new(),
        }
    }

    fn from_snapshot(snapshot: &TimerSnapshot) -> Self {
        Self {
            generation: snapshot_generation(snapshot),
            next_deadline: snapshot.next_deadline(),
            arms: snapshot.arms().to_vec(),
            expiries: snapshot.expiries().to_vec(),
            signal_errors: snapshot.signal_errors().to_vec(),
        }
    }

    fn next_generation(&self) -> u64 {
        self.generation + 1
    }

    fn arm_generation(&mut self, generation: u64, programmed_tick: Tick, deadline: Tick) {
        self.generation = generation;
        self.next_deadline = Some(deadline);
        self.arms
            .push(TimerArm::new(self.generation, programmed_tick, deadline));
    }

    fn expire(&mut self, generation: u64, tick: Tick) -> bool {
        if self.generation != generation || self.next_deadline != Some(tick) {
            return false;
        }

        self.next_deadline = None;
        self.expiries.push(TimerExpiry::new(generation, tick));
        true
    }

    fn record_signal_error(&mut self, generation: u64, tick: Tick, error: InterruptError) {
        self.signal_errors
            .push(TimerSignalError::new(generation, tick, error));
    }
}

fn snapshot_generation(snapshot: &TimerSnapshot) -> u64 {
    let arm_generation = snapshot
        .arms()
        .iter()
        .map(|arm| arm.generation())
        .max()
        .unwrap_or_default();
    let expiry_generation = snapshot
        .expiries()
        .iter()
        .map(|expiry| expiry.generation())
        .max()
        .unwrap_or_default();
    let signal_generation = snapshot
        .signal_errors()
        .iter()
        .map(TimerSignalError::generation)
        .max()
        .unwrap_or_default();

    arm_generation.max(expiry_generation).max(signal_generation)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimerError {
    DuplicateClintHart {
        hart: u32,
    },
    ZeroRtcPeriod,
    ClintRtcRequiresRtcTimebase,
    ClintSnapshotBaseMismatch {
        expected: Address,
        actual: Address,
    },
    ClintSnapshotHartMismatch {
        expected: Vec<u32>,
        actual: Vec<u32>,
    },
    ClintInterruptRoute {
        hart: u32,
        error: InterruptError,
    },
    ClintResetSignal {
        hart: u32,
        error: InterruptError,
    },
    ClintRtcSignal {
        hart: u32,
        error: InterruptError,
    },
    DeadlineInPast {
        now: Tick,
        deadline: Tick,
    },
    SnapshotIdentityMismatch {
        expected_id: TimerId,
        actual_id: TimerId,
        expected_partition: PartitionId,
        actual_partition: PartitionId,
        expected_source: InterruptSourceId,
        actual_source: InterruptSourceId,
    },
    Scheduler(SchedulerError),
    Interrupt(InterruptError),
}

impl fmt::Display for TimerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateClintHart { hart } => {
                write!(formatter, "duplicate CLINT hart {hart}")
            }
            Self::ZeroRtcPeriod => write!(formatter, "RISC-V RTC period must be positive"),
            Self::ClintRtcRequiresRtcTimebase => {
                write!(formatter, "RISC-V RTC pulses require an RTC-driven CLINT timebase")
            }
            Self::ClintSnapshotBaseMismatch { expected, actual } => write!(
                formatter,
                "CLINT snapshot base mismatch: expected {:#x}, got {:#x}",
                expected.get(),
                actual.get()
            ),
            Self::ClintSnapshotHartMismatch { expected, actual } => write!(
                formatter,
                "CLINT snapshot hart mismatch: expected {expected:?}, got {actual:?}"
            ),
            Self::ClintInterruptRoute { hart, error } => {
                write!(
                    formatter,
                    "CLINT hart {hart} interrupt route validation failed: {error}"
                )
            }
            Self::ClintResetSignal { hart, error } => {
                write!(formatter, "CLINT hart {hart} reset signal failed: {error}")
            }
            Self::ClintRtcSignal { hart, error } => {
                write!(formatter, "CLINT hart {hart} RTC signal failed: {error}")
            }
            Self::DeadlineInPast { now, deadline } => {
                write!(
                    formatter,
                    "cannot arm timer for deadline {deadline}; current tick is {now}"
                )
            }
            Self::SnapshotIdentityMismatch {
                expected_id,
                actual_id,
                expected_partition,
                actual_partition,
                expected_source,
                actual_source,
            } => write!(
                formatter,
                "timer snapshot identity mismatch: expected timer {} partition {} source {}, got timer {} partition {} source {}",
                expected_id.get(),
                expected_partition.index(),
                expected_source.get(),
                actual_id.get(),
                actual_partition.index(),
                actual_source.get()
            ),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Interrupt(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TimerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ClintInterruptRoute { error, .. }
            | Self::ClintResetSignal { error, .. }
            | Self::ClintRtcSignal { error, .. } => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Interrupt(error) => Some(error),
            _ => None,
        }
    }
}
