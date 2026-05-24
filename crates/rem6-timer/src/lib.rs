use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptLinePort, InterruptSourceId};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, SchedulerContext, SchedulerError, Tick,
};
use rem6_memory::{Address, ByteMask};
use rem6_mmio::{MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

pub const TIMER_MMIO_REGISTER_BYTES: u64 = 8;
pub const TIMER_MMIO_TIME_OFFSET: u64 = 0x00;
pub const TIMER_MMIO_DEADLINE_OFFSET: u64 = 0x08;

pub const CLINT_MSIP_BASE_OFFSET: u64 = 0x0000;
pub const CLINT_MSIP_REGISTER_BYTES: u64 = 4;
pub const CLINT_MSIP_STRIDE: u64 = 4;
pub const CLINT_MTIMECMP_BASE_OFFSET: u64 = 0x4000;
pub const CLINT_MTIMECMP_REGISTER_BYTES: u64 = 8;
pub const CLINT_MTIMECMP_STRIDE: u64 = 8;
pub const CLINT_MTIME_OFFSET: u64 = 0xbff8;
pub const CLINT_MTIME_REGISTER_BYTES: u64 = 8;

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

        let generation = {
            let mut state = self.state.lock().expect("timer state lock");
            state.arm(now, deadline)
        };
        let delay = deadline - now;
        let state = Arc::clone(&self.state);
        let interrupt = self.interrupt.clone();
        let source = self.source;

        context
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
            .map_err(TimerError::Scheduler)
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

        let generation = {
            let mut state = self.state.lock().expect("timer state lock");
            state.arm(now, deadline)
        };
        let delay = deadline - now;
        let state = Arc::clone(&self.state);
        let interrupt = self.interrupt.clone();
        let source = self.source;

        context
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
            .map_err(TimerError::Scheduler)
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

#[derive(Clone, Debug)]
pub struct ClintHartConfig {
    hart: u32,
    software_interrupt: InterruptLinePort,
    software_source: InterruptSourceId,
    timer_interrupt: InterruptLinePort,
    timer_source: InterruptSourceId,
}

impl ClintHartConfig {
    pub fn new(
        hart: u32,
        software_interrupt: InterruptLinePort,
        software_source: InterruptSourceId,
        timer_interrupt: InterruptLinePort,
        timer_source: InterruptSourceId,
    ) -> Self {
        Self {
            hart,
            software_interrupt,
            software_source,
            timer_interrupt,
            timer_source,
        }
    }

    pub const fn hart(&self) -> u32 {
        self.hart
    }

    pub const fn software_source(&self) -> InterruptSourceId {
        self.software_source
    }

    pub const fn timer_source(&self) -> InterruptSourceId {
        self.timer_source
    }
}

#[derive(Clone, Debug)]
pub struct ClintMmioDevice {
    base: Address,
    harts: Arc<BTreeMap<u32, ClintHartRuntime>>,
}

impl ClintMmioDevice {
    pub fn new<I>(base: Address, harts: I) -> Result<Self, TimerError>
    where
        I: IntoIterator<Item = ClintHartConfig>,
    {
        let mut runtimes = BTreeMap::new();
        for config in harts {
            let hart = config.hart();
            if runtimes
                .insert(hart, ClintHartRuntime::new(config))
                .is_some()
            {
                return Err(TimerError::DuplicateClintHart { hart });
            }
        }

        Ok(Self {
            base,
            harts: Arc::new(runtimes),
        })
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn hart_count(&self) -> usize {
        self.harts.len()
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let offset = self.offset(request)?;
        if let Some((runtime, register_offset)) =
            self.msip_runtime(offset, request.id(), request.size().bytes())?
        {
            return match request.operation() {
                MmioOperation::Read => Ok(MmioResponse::completed(
                    request.id(),
                    Some(le32(
                        runtime.state.lock().expect("CLINT hart state lock").msip,
                    )),
                )),
                MmioOperation::Write => {
                    self.write_msip(context, request, runtime, register_offset)?;
                    Ok(MmioResponse::completed(request.id(), None))
                }
            };
        }
        if let Some((runtime, register_offset)) =
            self.mtimecmp_runtime(offset, request.id(), request.size().bytes())?
        {
            return match request.operation() {
                MmioOperation::Read => Ok(MmioResponse::completed(
                    request.id(),
                    Some(le64(
                        runtime
                            .state
                            .lock()
                            .expect("CLINT hart state lock")
                            .mtimecmp,
                    )),
                )),
                MmioOperation::Write => {
                    self.write_mtimecmp(context, request, runtime, register_offset)?;
                    Ok(MmioResponse::completed(request.id(), None))
                }
            };
        }
        if self.is_mtime(offset, request.id(), request.size().bytes())? {
            return match request.operation() {
                MmioOperation::Read => Ok(MmioResponse::completed(
                    request.id(),
                    Some(le64(context.now())),
                )),
                MmioOperation::Write => Err(MmioError::AccessDenied {
                    request: request.id(),
                    operation: MmioOperation::Write,
                    access: MmioAccess::ReadOnly,
                }),
            };
        }

        Err(MmioError::UnmappedAddress {
            address: request.range().start(),
        })
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let offset = self.offset(request)?;
        if let Some((runtime, register_offset)) =
            self.msip_runtime(offset, request.id(), request.size().bytes())?
        {
            return match request.operation() {
                MmioOperation::Read => Ok(MmioResponse::completed(
                    request.id(),
                    Some(le32(
                        runtime.state.lock().expect("CLINT hart state lock").msip,
                    )),
                )),
                MmioOperation::Write => {
                    self.write_msip_parallel(context, request, runtime, register_offset)?;
                    Ok(MmioResponse::completed(request.id(), None))
                }
            };
        }
        if let Some((runtime, register_offset)) =
            self.mtimecmp_runtime(offset, request.id(), request.size().bytes())?
        {
            return match request.operation() {
                MmioOperation::Read => Ok(MmioResponse::completed(
                    request.id(),
                    Some(le64(
                        runtime
                            .state
                            .lock()
                            .expect("CLINT hart state lock")
                            .mtimecmp,
                    )),
                )),
                MmioOperation::Write => {
                    self.write_mtimecmp_parallel(context, request, runtime, register_offset)?;
                    Ok(MmioResponse::completed(request.id(), None))
                }
            };
        }
        if self.is_mtime(offset, request.id(), request.size().bytes())? {
            return match request.operation() {
                MmioOperation::Read => Ok(MmioResponse::completed(
                    request.id(),
                    Some(le64(context.now())),
                )),
                MmioOperation::Write => Err(MmioError::AccessDenied {
                    request: request.id(),
                    operation: MmioOperation::Write,
                    access: MmioAccess::ReadOnly,
                }),
            };
        }

        Err(MmioError::UnmappedAddress {
            address: request.range().start(),
        })
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

    fn msip_runtime(
        &self,
        offset: u64,
        request: rem6_mmio::MmioRequestId,
        size: u64,
    ) -> Result<Option<(&ClintHartRuntime, u64)>, MmioError> {
        if !(CLINT_MSIP_BASE_OFFSET..CLINT_MTIMECMP_BASE_OFFSET).contains(&offset) {
            return Ok(None);
        }
        if size != CLINT_MSIP_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request,
                expected: CLINT_MSIP_REGISTER_BYTES,
                actual: size,
            });
        }
        let relative = offset - CLINT_MSIP_BASE_OFFSET;
        if !relative.is_multiple_of(CLINT_MSIP_STRIDE) {
            return Err(MmioError::UnmappedAddress {
                address: Address::new(self.base.get() + offset),
            });
        }
        let hart = (relative / CLINT_MSIP_STRIDE) as u32;
        Ok(self.harts.get(&hart).map(|runtime| (runtime, relative)))
    }

    fn mtimecmp_runtime(
        &self,
        offset: u64,
        request: rem6_mmio::MmioRequestId,
        size: u64,
    ) -> Result<Option<(&ClintHartRuntime, u64)>, MmioError> {
        if !(CLINT_MTIMECMP_BASE_OFFSET..CLINT_MTIME_OFFSET).contains(&offset) {
            return Ok(None);
        }
        if size != CLINT_MTIMECMP_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request,
                expected: CLINT_MTIMECMP_REGISTER_BYTES,
                actual: size,
            });
        }
        let relative = offset - CLINT_MTIMECMP_BASE_OFFSET;
        if !relative.is_multiple_of(CLINT_MTIMECMP_STRIDE) {
            return Err(MmioError::UnmappedAddress {
                address: Address::new(self.base.get() + offset),
            });
        }
        let hart = (relative / CLINT_MTIMECMP_STRIDE) as u32;
        Ok(self.harts.get(&hart).map(|runtime| (runtime, relative)))
    }

    fn is_mtime(
        &self,
        offset: u64,
        request: rem6_mmio::MmioRequestId,
        size: u64,
    ) -> Result<bool, MmioError> {
        if offset != CLINT_MTIME_OFFSET {
            return Ok(false);
        }
        if size != CLINT_MTIME_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request,
                expected: CLINT_MTIME_REGISTER_BYTES,
                actual: size,
            });
        }
        Ok(true)
    }

    fn write_msip(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
        runtime: &ClintHartRuntime,
        register_offset: u64,
    ) -> Result<(), MmioError> {
        let value = clint_u32_write_value(request, runtime.msip(), register_offset)? & 0x1;
        let old = runtime.replace_msip(value);
        if old == value {
            return Ok(());
        }
        let result = if value == 0 {
            runtime
                .config
                .software_interrupt
                .deassert(context, runtime.config.software_source)
        } else {
            runtime
                .config
                .software_interrupt
                .assert(context, runtime.config.software_source)
        };
        result.map(|_| ()).map_err(|error| MmioError::DeviceError {
            request: request.id(),
            message: error.to_string(),
        })
    }

    fn write_msip_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
        runtime: &ClintHartRuntime,
        register_offset: u64,
    ) -> Result<(), MmioError> {
        let value = clint_u32_write_value(request, runtime.msip(), register_offset)? & 0x1;
        let old = runtime.replace_msip(value);
        if old == value {
            return Ok(());
        }
        let result = if value == 0 {
            runtime
                .config
                .software_interrupt
                .deassert_parallel(context, runtime.config.software_source)
        } else {
            runtime
                .config
                .software_interrupt
                .assert_parallel(context, runtime.config.software_source)
        };
        result.map(|_| ()).map_err(|error| MmioError::DeviceError {
            request: request.id(),
            message: error.to_string(),
        })
    }

    fn write_mtimecmp(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
        runtime: &ClintHartRuntime,
        register_offset: u64,
    ) -> Result<(), MmioError> {
        let deadline = clint_u64_write_value(request, runtime.mtimecmp(), register_offset)?;
        let program = runtime.replace_mtimecmp(deadline);
        if deadline <= context.now() {
            runtime.mark_timer_asserted(program.generation());
            return runtime
                .config
                .timer_interrupt
                .assert(context, runtime.config.timer_source)
                .map(|_| ())
                .map_err(|error| MmioError::DeviceError {
                    request: request.id(),
                    message: error.to_string(),
                });
        }

        if program.was_asserted() {
            runtime
                .config
                .timer_interrupt
                .deassert(context, runtime.config.timer_source)
                .map_err(|error| MmioError::DeviceError {
                    request: request.id(),
                    message: error.to_string(),
                })?;
        }
        let delay = deadline - context.now();
        let runtime = runtime.clone();
        let request = request.id();
        context
            .schedule_local_after(delay, move |context| {
                if runtime.mark_timer_asserted(program.generation()) {
                    runtime
                        .config
                        .timer_interrupt
                        .assert(context, runtime.config.timer_source)
                        .expect("validated CLINT timer interrupt");
                }
            })
            .map(|_| ())
            .map_err(|error| MmioError::DeviceError {
                request,
                message: error.to_string(),
            })
    }

    fn write_mtimecmp_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
        runtime: &ClintHartRuntime,
        register_offset: u64,
    ) -> Result<(), MmioError> {
        let deadline = clint_u64_write_value(request, runtime.mtimecmp(), register_offset)?;
        let program = runtime.replace_mtimecmp(deadline);
        if deadline <= context.now() {
            runtime.mark_timer_asserted(program.generation());
            return runtime
                .config
                .timer_interrupt
                .assert_parallel(context, runtime.config.timer_source)
                .map(|_| ())
                .map_err(|error| MmioError::DeviceError {
                    request: request.id(),
                    message: error.to_string(),
                });
        }

        if program.was_asserted() {
            runtime
                .config
                .timer_interrupt
                .deassert_parallel(context, runtime.config.timer_source)
                .map_err(|error| MmioError::DeviceError {
                    request: request.id(),
                    message: error.to_string(),
                })?;
        }
        let delay = deadline - context.now();
        let runtime = runtime.clone();
        let request = request.id();
        context
            .schedule_local_after(delay, move |context| {
                if runtime.mark_timer_asserted(program.generation()) {
                    runtime
                        .config
                        .timer_interrupt
                        .assert_parallel(context, runtime.config.timer_source)
                        .expect("validated CLINT timer interrupt");
                }
            })
            .map(|_| ())
            .map_err(|error| MmioError::DeviceError {
                request,
                message: error.to_string(),
            })
    }
}

impl MmioDevice for ClintMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        ClintMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        ClintMmioDevice::respond_parallel(self, context, request)
    }
}

#[derive(Clone, Debug)]
struct ClintHartRuntime {
    config: ClintHartConfig,
    state: Arc<Mutex<ClintHartState>>,
}

impl ClintHartRuntime {
    fn new(config: ClintHartConfig) -> Self {
        Self {
            config,
            state: Arc::new(Mutex::new(ClintHartState::new())),
        }
    }

    fn msip(&self) -> u32 {
        self.state.lock().expect("CLINT hart state lock").msip
    }

    fn mtimecmp(&self) -> u64 {
        self.state.lock().expect("CLINT hart state lock").mtimecmp
    }

    fn replace_msip(&self, value: u32) -> u32 {
        let mut state = self.state.lock().expect("CLINT hart state lock");
        let old = state.msip;
        state.msip = value;
        old
    }

    fn replace_mtimecmp(&self, value: u64) -> ClintTimerProgram {
        let mut state = self.state.lock().expect("CLINT hart state lock");
        state.timer_generation += 1;
        let was_asserted = state.timer_asserted;
        state.mtimecmp = value;
        state.timer_asserted = false;
        ClintTimerProgram::new(state.timer_generation, was_asserted)
    }

    fn mark_timer_asserted(&self, generation: u64) -> bool {
        let mut state = self.state.lock().expect("CLINT hart state lock");
        if state.timer_generation != generation || state.timer_asserted {
            return false;
        }
        state.timer_asserted = true;
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClintTimerProgram {
    generation: u64,
    was_asserted: bool,
}

impl ClintTimerProgram {
    const fn new(generation: u64, was_asserted: bool) -> Self {
        Self {
            generation,
            was_asserted,
        }
    }

    const fn generation(self) -> u64 {
        self.generation
    }

    const fn was_asserted(self) -> bool {
        self.was_asserted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClintHartState {
    msip: u32,
    mtimecmp: u64,
    timer_generation: u64,
    timer_asserted: bool,
}

impl ClintHartState {
    const fn new() -> Self {
        Self {
            msip: 0,
            mtimecmp: u64::MAX,
            timer_generation: 0,
            timer_asserted: false,
        }
    }
}

fn le64(value: u64) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn le32(value: u32) -> Vec<u8> {
    value.to_le_bytes().to_vec()
}

fn clint_u32_write_value(
    request: &MmioRequest,
    current: u32,
    _register_offset: u64,
) -> Result<u32, MmioError> {
    let data = request.data().ok_or(MmioError::MissingWriteData {
        request: request.id(),
    })?;
    if data.len() as u64 != CLINT_MSIP_REGISTER_BYTES {
        return Err(MmioError::PayloadSizeMismatch {
            request: request.id(),
            expected: CLINT_MSIP_REGISTER_BYTES,
            actual: data.len() as u64,
        });
    }
    let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
        request: request.id(),
    })?;
    validate_clint_mmio_mask(request, mask, CLINT_MSIP_REGISTER_BYTES)?;

    let mut bytes = current.to_le_bytes();
    for (index, byte) in data.iter().enumerate() {
        if mask.bits()[index] {
            bytes[index] = *byte;
        }
    }
    Ok(u32::from_le_bytes(bytes))
}

fn clint_u64_write_value(
    request: &MmioRequest,
    current: u64,
    _register_offset: u64,
) -> Result<u64, MmioError> {
    let data = request.data().ok_or(MmioError::MissingWriteData {
        request: request.id(),
    })?;
    if data.len() as u64 != CLINT_MTIMECMP_REGISTER_BYTES {
        return Err(MmioError::PayloadSizeMismatch {
            request: request.id(),
            expected: CLINT_MTIMECMP_REGISTER_BYTES,
            actual: data.len() as u64,
        });
    }
    let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
        request: request.id(),
    })?;
    validate_clint_mmio_mask(request, mask, CLINT_MTIMECMP_REGISTER_BYTES)?;

    let mut bytes = current.to_le_bytes();
    for (index, byte) in data.iter().enumerate() {
        if mask.bits()[index] {
            bytes[index] = *byte;
        }
    }

    Ok(u64::from_le_bytes(bytes))
}

fn validate_clint_mmio_mask(
    request: &MmioRequest,
    mask: &ByteMask,
    expected: u64,
) -> Result<(), MmioError> {
    if mask.len() != expected {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected,
            actual: mask.len(),
        });
    }
    Ok(())
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

    fn arm(&mut self, programmed_tick: Tick, deadline: Tick) -> u64 {
        self.generation += 1;
        self.next_deadline = Some(deadline);
        self.arms
            .push(TimerArm::new(self.generation, programmed_tick, deadline));
        self.generation
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

impl Error for TimerError {}
