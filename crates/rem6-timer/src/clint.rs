use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptLinePort, InterruptSourceId};
use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerContext, Tick};
use rem6_memory::{Address, ByteMask};
use rem6_mmio::{MmioAccess, MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

use crate::TimerError;

pub const CLINT_MSIP_BASE_OFFSET: u64 = 0x0000;
pub const CLINT_MSIP_REGISTER_BYTES: u64 = 4;
pub const CLINT_MSIP_STRIDE: u64 = 4;
pub const CLINT_MTIMECMP_BASE_OFFSET: u64 = 0x4000;
pub const CLINT_MTIMECMP_REGISTER_BYTES: u64 = 8;
pub const CLINT_MTIMECMP_STRIDE: u64 = 8;
pub const CLINT_MTIME_OFFSET: u64 = 0xbff8;
pub const CLINT_MTIME_REGISTER_BYTES: u64 = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClintId(u64);

impl ClintId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClintResetPolicy {
    mtimecmp_reset_value: Option<u64>,
}

impl ClintResetPolicy {
    pub const fn preserve_mtimecmp() -> Self {
        Self {
            mtimecmp_reset_value: None,
        }
    }

    pub const fn reset_mtimecmp_to(value: u64) -> Self {
        Self {
            mtimecmp_reset_value: Some(value),
        }
    }

    pub const fn mtimecmp_reset_value(self) -> Option<u64> {
        self.mtimecmp_reset_value
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ClintTimebase {
    #[default]
    SchedulerTicks,
    RtcDriven,
}

impl ClintTimebase {
    pub const fn scheduler_ticks() -> Self {
        Self::SchedulerTicks
    }

    pub const fn rtc_driven() -> Self {
        Self::RtcDriven
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClintTimeState {
    mtime: u64,
}

impl ClintTimeState {
    const fn new() -> Self {
        Self { mtime: 0 }
    }

    fn mtime(&self) -> u64 {
        self.mtime
    }

    fn tick(&mut self) -> u64 {
        self.mtime += 1;
        self.mtime
    }

    fn reset(&mut self) {
        self.mtime = 0;
    }

    fn set_mtime(&mut self, mtime: u64) {
        self.mtime = mtime;
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

    fn snapshot(&self, hart: u32) -> ClintHartSnapshot {
        self.state
            .lock()
            .expect("CLINT hart state lock")
            .snapshot(hart)
    }

    fn restore(&self, snapshot: &ClintHartSnapshot) {
        *self.state.lock().expect("CLINT hart state lock") =
            ClintHartState::from_snapshot(snapshot);
    }

    fn reset(&self, hart: u32, policy: ClintResetPolicy) -> ClintHartReset {
        let mut state = self.state.lock().expect("CLINT hart state lock");
        let reset = ClintHartReset::new(hart, state.msip, state.timer_asserted);
        state.timer_generation += 1;
        state.msip = 0;
        if let Some(value) = policy.mtimecmp_reset_value() {
            state.mtimecmp = value;
        }
        state.timer_asserted = false;
        reset
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

    fn mark_timer_asserted_if_due(&self, mtime: u64) -> bool {
        let mut state = self.state.lock().expect("CLINT hart state lock");
        if mtime < state.mtimecmp || state.timer_asserted {
            return false;
        }
        state.timer_asserted = true;
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClintHartReset {
    hart: u32,
    software_was_asserted: bool,
    timer_was_asserted: bool,
}

impl ClintHartReset {
    const fn new(hart: u32, msip: u32, timer_was_asserted: bool) -> Self {
        Self {
            hart,
            software_was_asserted: msip != 0,
            timer_was_asserted,
        }
    }

    const fn hart(self) -> u32 {
        self.hart
    }

    const fn software_was_asserted(self) -> bool {
        self.software_was_asserted
    }

    const fn timer_was_asserted(self) -> bool {
        self.timer_was_asserted
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

    const fn from_snapshot(snapshot: &ClintHartSnapshot) -> Self {
        Self {
            msip: snapshot.msip(),
            mtimecmp: snapshot.mtimecmp(),
            timer_generation: snapshot.timer_generation(),
            timer_asserted: snapshot.timer_asserted(),
        }
    }

    const fn snapshot(&self, hart: u32) -> ClintHartSnapshot {
        ClintHartSnapshot::new(
            hart,
            self.msip,
            self.mtimecmp,
            self.timer_generation,
            self.timer_asserted,
        )
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClintHartSnapshot {
    hart: u32,
    msip: u32,
    mtimecmp: u64,
    timer_generation: u64,
    timer_asserted: bool,
}

impl ClintHartSnapshot {
    pub const fn new(
        hart: u32,
        msip: u32,
        mtimecmp: u64,
        timer_generation: u64,
        timer_asserted: bool,
    ) -> Self {
        Self {
            hart,
            msip,
            mtimecmp,
            timer_generation,
            timer_asserted,
        }
    }

    pub const fn hart(&self) -> u32 {
        self.hart
    }

    pub const fn msip(&self) -> u32 {
        self.msip
    }

    pub const fn mtimecmp(&self) -> u64 {
        self.mtimecmp
    }

    pub const fn timer_generation(&self) -> u64 {
        self.timer_generation
    }

    pub const fn timer_asserted(&self) -> bool {
        self.timer_asserted
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClintSnapshot {
    base: Address,
    mtime: u64,
    harts: Vec<ClintHartSnapshot>,
}

impl ClintSnapshot {
    pub fn new(base: Address, harts: Vec<ClintHartSnapshot>) -> Self {
        Self {
            base,
            mtime: 0,
            harts,
        }
    }

    pub fn with_mtime(base: Address, mtime: u64, harts: Vec<ClintHartSnapshot>) -> Self {
        Self { base, mtime, harts }
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn mtime(&self) -> u64 {
        self.mtime
    }

    pub fn harts(&self) -> &[ClintHartSnapshot] {
        &self.harts
    }
}

#[derive(Clone, Debug)]
pub struct RiscvRtcSource {
    clint: ClintMmioDevice,
    clint_partition: PartitionId,
    period: Tick,
}

impl RiscvRtcSource {
    pub fn new(
        clint: ClintMmioDevice,
        clint_partition: PartitionId,
        period: Tick,
    ) -> Result<Self, TimerError> {
        if period == 0 {
            return Err(TimerError::ZeroRtcPeriod);
        }
        if clint.timebase() != ClintTimebase::RtcDriven {
            return Err(TimerError::ClintRtcRequiresRtcTimebase);
        }

        Ok(Self {
            clint,
            clint_partition,
            period,
        })
    }

    pub const fn clint_partition(&self) -> PartitionId {
        self.clint_partition
    }

    pub const fn period(&self) -> Tick {
        self.period
    }

    pub fn schedule_pulses(
        &self,
        context: &mut SchedulerContext<'_>,
        pulses: u64,
    ) -> Result<(), TimerError> {
        if pulses == 0 {
            return Ok(());
        }

        let source = self.clone();
        context
            .schedule_remote_after(self.clint_partition, self.period, move |context| {
                source
                    .deliver_pulse(context, pulses)
                    .expect("validated RISC-V RTC pulse");
            })
            .map(|_| ())
            .map_err(TimerError::Scheduler)
    }

    pub fn schedule_pulses_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        pulses: u64,
    ) -> Result<(), TimerError> {
        if pulses == 0 {
            return Ok(());
        }

        let source = self.clone();
        context
            .schedule_remote_after(self.clint_partition, self.period, move |context| {
                source
                    .deliver_pulse_parallel(context, pulses)
                    .expect("validated RISC-V RTC pulse");
            })
            .map(|_| ())
            .map_err(TimerError::Scheduler)
    }

    fn deliver_pulse(
        &self,
        context: &mut SchedulerContext<'_>,
        remaining: u64,
    ) -> Result<(), TimerError> {
        self.clint.rtc_tick(context)?;
        self.schedule_pulses(context, remaining.saturating_sub(1))
    }

    fn deliver_pulse_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        remaining: u64,
    ) -> Result<(), TimerError> {
        self.clint.rtc_tick_parallel(context)?;
        self.schedule_pulses_parallel(context, remaining.saturating_sub(1))
    }
}

#[derive(Clone, Debug)]
pub struct ClintMmioDevice {
    base: Address,
    harts: Arc<BTreeMap<u32, ClintHartRuntime>>,
    reset_policy: ClintResetPolicy,
    timebase: ClintTimebase,
    time_state: Arc<Mutex<ClintTimeState>>,
}

impl ClintMmioDevice {
    pub fn new<I>(base: Address, harts: I) -> Result<Self, TimerError>
    where
        I: IntoIterator<Item = ClintHartConfig>,
    {
        Self::with_reset_policy(base, harts, ClintResetPolicy::preserve_mtimecmp())
    }

    pub fn with_timebase<I>(
        base: Address,
        harts: I,
        timebase: ClintTimebase,
    ) -> Result<Self, TimerError>
    where
        I: IntoIterator<Item = ClintHartConfig>,
    {
        Self::with_reset_policy_and_timebase(
            base,
            harts,
            ClintResetPolicy::preserve_mtimecmp(),
            timebase,
        )
    }

    pub fn with_reset_policy<I>(
        base: Address,
        harts: I,
        reset_policy: ClintResetPolicy,
    ) -> Result<Self, TimerError>
    where
        I: IntoIterator<Item = ClintHartConfig>,
    {
        Self::with_reset_policy_and_timebase(
            base,
            harts,
            reset_policy,
            ClintTimebase::scheduler_ticks(),
        )
    }

    pub fn with_reset_policy_and_timebase<I>(
        base: Address,
        harts: I,
        reset_policy: ClintResetPolicy,
        timebase: ClintTimebase,
    ) -> Result<Self, TimerError>
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
            reset_policy,
            timebase,
            time_state: Arc::new(Mutex::new(ClintTimeState::new())),
        })
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn hart_count(&self) -> usize {
        self.harts.len()
    }

    pub const fn reset_policy(&self) -> ClintResetPolicy {
        self.reset_policy
    }

    pub const fn timebase(&self) -> ClintTimebase {
        self.timebase
    }

    pub fn snapshot(&self) -> ClintSnapshot {
        ClintSnapshot::with_mtime(
            self.base,
            self.snapshot_mtime(),
            self.harts
                .iter()
                .map(|(hart, runtime)| runtime.snapshot(*hart))
                .collect(),
        )
    }

    pub fn restore(&self, snapshot: &ClintSnapshot) -> Result<(), TimerError> {
        if snapshot.base() != self.base {
            return Err(TimerError::ClintSnapshotBaseMismatch {
                expected: self.base,
                actual: snapshot.base(),
            });
        }
        let expected = self.hart_ids();
        let actual: Vec<u32> = snapshot
            .harts()
            .iter()
            .map(ClintHartSnapshot::hart)
            .collect();
        if actual != expected {
            return Err(TimerError::ClintSnapshotHartMismatch { expected, actual });
        }
        for hart in snapshot.harts() {
            self.harts
                .get(&hart.hart())
                .expect("validated CLINT hart snapshot")
                .restore(hart);
        }
        self.time_state
            .lock()
            .expect("CLINT time state lock")
            .set_mtime(snapshot.mtime());
        Ok(())
    }

    pub fn reset(&self, context: &mut SchedulerContext<'_>) -> Result<(), TimerError> {
        self.reset_time();
        for (hart, runtime) in self.harts.iter() {
            let reset = runtime.reset(*hart, self.reset_policy);
            self.signal_reset_deassertions(context, runtime, reset)?;
        }
        Ok(())
    }

    pub fn reset_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
    ) -> Result<(), TimerError> {
        self.reset_time();
        for (hart, runtime) in self.harts.iter() {
            let reset = runtime.reset(*hart, self.reset_policy);
            self.signal_reset_deassertions_parallel(context, runtime, reset)?;
        }
        Ok(())
    }

    pub fn rtc_tick(&self, context: &mut SchedulerContext<'_>) -> Result<u64, TimerError> {
        let mtime = self.increment_rtc_mtime()?;
        for (hart, runtime) in self.harts.iter() {
            if runtime.mark_timer_asserted_if_due(mtime) {
                runtime
                    .config
                    .timer_interrupt
                    .assert(context, runtime.config.timer_source)
                    .map_err(|error| TimerError::ClintRtcSignal { hart: *hart, error })?;
            }
        }
        Ok(mtime)
    }

    pub fn rtc_tick_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
    ) -> Result<u64, TimerError> {
        let mtime = self.increment_rtc_mtime()?;
        for (hart, runtime) in self.harts.iter() {
            if runtime.mark_timer_asserted_if_due(mtime) {
                runtime
                    .config
                    .timer_interrupt
                    .assert_parallel(context, runtime.config.timer_source)
                    .map_err(|error| TimerError::ClintRtcSignal { hart: *hart, error })?;
            }
        }
        Ok(mtime)
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
                    Some(le64(self.current_mtime(context.now()))),
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
                    Some(le64(self.current_mtime(context.now()))),
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

    fn current_mtime(&self, scheduler_now: Tick) -> u64 {
        match self.timebase {
            ClintTimebase::SchedulerTicks => scheduler_now,
            ClintTimebase::RtcDriven => self
                .time_state
                .lock()
                .expect("CLINT time state lock")
                .mtime(),
        }
    }

    fn snapshot_mtime(&self) -> u64 {
        match self.timebase {
            ClintTimebase::SchedulerTicks => 0,
            ClintTimebase::RtcDriven => self
                .time_state
                .lock()
                .expect("CLINT time state lock")
                .mtime(),
        }
    }

    fn reset_time(&self) {
        if self.timebase == ClintTimebase::RtcDriven {
            self.time_state
                .lock()
                .expect("CLINT time state lock")
                .reset();
        }
    }

    fn increment_rtc_mtime(&self) -> Result<u64, TimerError> {
        if self.timebase != ClintTimebase::RtcDriven {
            return Err(TimerError::ClintRtcRequiresRtcTimebase);
        }

        Ok(self
            .time_state
            .lock()
            .expect("CLINT time state lock")
            .tick())
    }

    fn hart_ids(&self) -> Vec<u32> {
        self.harts.keys().copied().collect()
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

    fn signal_reset_deassertions(
        &self,
        context: &mut SchedulerContext<'_>,
        runtime: &ClintHartRuntime,
        reset: ClintHartReset,
    ) -> Result<(), TimerError> {
        if reset.software_was_asserted() {
            runtime
                .config
                .software_interrupt
                .deassert(context, runtime.config.software_source)
                .map_err(|error| TimerError::ClintResetSignal {
                    hart: reset.hart(),
                    error,
                })?;
        }
        if reset.timer_was_asserted() {
            runtime
                .config
                .timer_interrupt
                .deassert(context, runtime.config.timer_source)
                .map_err(|error| TimerError::ClintResetSignal {
                    hart: reset.hart(),
                    error,
                })?;
        }
        Ok(())
    }

    fn signal_reset_deassertions_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        runtime: &ClintHartRuntime,
        reset: ClintHartReset,
    ) -> Result<(), TimerError> {
        if reset.software_was_asserted() {
            runtime
                .config
                .software_interrupt
                .deassert_parallel(context, runtime.config.software_source)
                .map_err(|error| TimerError::ClintResetSignal {
                    hart: reset.hart(),
                    error,
                })?;
        }
        if reset.timer_was_asserted() {
            runtime
                .config
                .timer_interrupt
                .deassert_parallel(context, runtime.config.timer_source)
                .map_err(|error| TimerError::ClintResetSignal {
                    hart: reset.hart(),
                    error,
                })?;
        }
        Ok(())
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
        let current_mtime = self.current_mtime(context.now());
        if deadline <= current_mtime {
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
        if self.timebase == ClintTimebase::RtcDriven {
            return Ok(());
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
        let current_mtime = self.current_mtime(context.now());
        if deadline <= current_mtime {
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
        if self.timebase == ClintTimebase::RtcDriven {
            return Ok(());
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
