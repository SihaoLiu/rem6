use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptLinePort, InterruptSourceId};
use rem6_kernel::{ParallelSchedulerContext, SchedulerContext};
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
    harts: Vec<ClintHartSnapshot>,
}

impl ClintSnapshot {
    pub fn new(base: Address, harts: Vec<ClintHartSnapshot>) -> Self {
        Self { base, harts }
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn harts(&self) -> &[ClintHartSnapshot] {
        &self.harts
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

    pub fn snapshot(&self) -> ClintSnapshot {
        ClintSnapshot::new(
            self.base,
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
        Ok(())
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
