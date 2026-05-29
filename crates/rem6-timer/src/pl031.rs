use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{InterruptError, InterruptLinePort, InterruptSourceId};
use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerContext, Tick};
use rem6_memory::Address;
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

pub const PL031_DATA_OFFSET: u64 = 0x00;
pub const PL031_MATCH_OFFSET: u64 = 0x04;
pub const PL031_LOAD_OFFSET: u64 = 0x08;
pub const PL031_CONTROL_OFFSET: u64 = 0x0c;
pub const PL031_INT_MASK_OFFSET: u64 = 0x10;
pub const PL031_RAW_ISR_OFFSET: u64 = 0x14;
pub const PL031_MASKED_ISR_OFFSET: u64 = 0x18;
pub const PL031_INT_CLEAR_OFFSET: u64 = 0x1c;
pub const PL031_REGISTER_BYTES: u64 = 4;
pub const PL031_MMIO_SIZE_BYTES: u64 = 0x1000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pl031Snapshot {
    time_value: u32,
    last_written_tick: Tick,
    load_value: u32,
    match_value: u32,
    raw_interrupt: bool,
    interrupt_mask: bool,
    pending_interrupt: bool,
    ticks_per_second: Tick,
    generation: u64,
}

impl Pl031Snapshot {
    pub const fn time_value(&self) -> u32 {
        self.time_value
    }

    pub const fn last_written_tick(&self) -> Tick {
        self.last_written_tick
    }

    pub const fn load_value(&self) -> u32 {
        self.load_value
    }

    pub const fn match_value(&self) -> u32 {
        self.match_value
    }

    pub const fn raw_interrupt(&self) -> bool {
        self.raw_interrupt
    }

    pub const fn interrupt_mask(&self) -> bool {
        self.interrupt_mask
    }

    pub const fn pending_interrupt(&self) -> bool {
        self.pending_interrupt
    }

    pub const fn ticks_per_second(&self) -> Tick {
        self.ticks_per_second
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pl031Rtc {
    time_value: u32,
    last_written_tick: Tick,
    load_value: u32,
    match_value: u32,
    raw_interrupt: bool,
    interrupt_mask: bool,
    pending_interrupt: bool,
    ticks_per_second: Tick,
    generation: u64,
}

impl Pl031Rtc {
    pub fn new(initial_time: u32, ticks_per_second: Tick) -> Result<Self, Pl031Error> {
        if ticks_per_second == 0 {
            return Err(Pl031Error::InvalidTicksPerSecond { ticks_per_second });
        }
        Ok(Self {
            time_value: initial_time,
            last_written_tick: 0,
            load_value: initial_time,
            match_value: 0,
            raw_interrupt: false,
            interrupt_mask: false,
            pending_interrupt: false,
            ticks_per_second,
            generation: 0,
        })
    }

    pub fn read_data(&self, tick: Tick) -> Result<u32, Pl031Error> {
        self.current_value(tick)
    }

    pub fn read_register(&self, offset: u64, tick: Tick) -> Result<u32, Pl031Error> {
        match offset {
            PL031_DATA_OFFSET => self.read_data(tick),
            PL031_MATCH_OFFSET => Ok(self.match_value),
            PL031_LOAD_OFFSET => Ok(self.load_value),
            PL031_CONTROL_OFFSET => Ok(1),
            PL031_INT_MASK_OFFSET => Ok(u32::from(self.interrupt_mask)),
            PL031_RAW_ISR_OFFSET => Ok(u32::from(self.raw_interrupt)),
            PL031_MASKED_ISR_OFFSET => Ok(u32::from(self.pending_interrupt)),
            _ => Err(Pl031Error::UnknownRegister { offset }),
        }
    }

    pub fn write_register(
        &mut self,
        offset: u64,
        value: u32,
        tick: Tick,
    ) -> Result<(), Pl031Error> {
        match offset {
            PL031_DATA_OFFSET | PL031_CONTROL_OFFSET => Ok(()),
            PL031_MATCH_OFFSET => {
                self.match_value = value;
                self.generation = self.next_generation()?;
                Ok(())
            }
            PL031_LOAD_OFFSET => {
                self.time_value = value;
                self.load_value = value;
                self.last_written_tick = tick;
                self.generation = self.next_generation()?;
                Ok(())
            }
            PL031_INT_MASK_OFFSET => {
                self.interrupt_mask = value != 0;
                self.pending_interrupt = self.raw_interrupt && self.interrupt_mask;
                Ok(())
            }
            PL031_INT_CLEAR_OFFSET => {
                if value != 0 {
                    self.raw_interrupt = false;
                    self.pending_interrupt = false;
                }
                Ok(())
            }
            _ => Err(Pl031Error::UnknownRegister { offset }),
        }
    }

    pub fn next_match_tick(&self, now: Tick) -> Result<Tick, Pl031Error> {
        let current = self.current_value(now)?;
        let seconds_until = self.match_value.wrapping_sub(current);
        now.checked_add(
            self.ticks_per_second
                .checked_mul(u64::from(seconds_until))
                .ok_or(Pl031Error::MatchTickOverflow {
                    now,
                    ticks_per_second: self.ticks_per_second,
                    seconds_until,
                })?,
        )
        .ok_or(Pl031Error::MatchTickOverflow {
            now,
            ticks_per_second: self.ticks_per_second,
            seconds_until,
        })
    }

    pub fn record_match(&mut self, _tick: Tick) -> Result<bool, Pl031Error> {
        self.raw_interrupt = true;
        let old_pending = self.pending_interrupt;
        self.pending_interrupt = self.raw_interrupt && self.interrupt_mask;
        Ok(self.pending_interrupt && !old_pending)
    }

    pub fn snapshot(&self) -> Pl031Snapshot {
        Pl031Snapshot {
            time_value: self.time_value,
            last_written_tick: self.last_written_tick,
            load_value: self.load_value,
            match_value: self.match_value,
            raw_interrupt: self.raw_interrupt,
            interrupt_mask: self.interrupt_mask,
            pending_interrupt: self.pending_interrupt,
            ticks_per_second: self.ticks_per_second,
            generation: self.generation,
        }
    }

    pub fn restore(&mut self, snapshot: &Pl031Snapshot) -> Result<(), Pl031Error> {
        if snapshot.ticks_per_second == 0 {
            return Err(Pl031Error::InvalidTicksPerSecond {
                ticks_per_second: snapshot.ticks_per_second,
            });
        }
        if snapshot.pending_interrupt && !(snapshot.raw_interrupt && snapshot.interrupt_mask) {
            return Err(Pl031Error::InvalidPendingInterrupt);
        }
        self.time_value = snapshot.time_value;
        self.last_written_tick = snapshot.last_written_tick;
        self.load_value = snapshot.load_value;
        self.match_value = snapshot.match_value;
        self.raw_interrupt = snapshot.raw_interrupt;
        self.interrupt_mask = snapshot.interrupt_mask;
        self.pending_interrupt = snapshot.pending_interrupt;
        self.ticks_per_second = snapshot.ticks_per_second;
        self.generation = snapshot.generation;
        Ok(())
    }

    fn current_value(&self, tick: Tick) -> Result<u32, Pl031Error> {
        if tick < self.last_written_tick {
            return Err(Pl031Error::TimeWentBack {
                tick,
                last_written_tick: self.last_written_tick,
            });
        }
        let elapsed_seconds = (tick - self.last_written_tick) / self.ticks_per_second;
        Ok(self.time_value.wrapping_add(elapsed_seconds as u32))
    }

    fn next_generation(&self) -> Result<u64, Pl031Error> {
        self.generation
            .checked_add(1)
            .ok_or(Pl031Error::GenerationOverflow)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pl031RtcMmioSnapshot {
    rtc: Pl031Snapshot,
}

impl Pl031RtcMmioSnapshot {
    pub const fn new(rtc: Pl031Snapshot) -> Self {
        Self { rtc }
    }

    pub const fn rtc(&self) -> &Pl031Snapshot {
        &self.rtc
    }
}

#[derive(Clone, Debug)]
pub struct Pl031RtcMmioDevice {
    base: Address,
    interrupt: Option<Pl031Interrupt>,
    state: Arc<Mutex<Pl031Rtc>>,
}

impl Pl031RtcMmioDevice {
    pub fn new(base: Address, rtc: Pl031Rtc) -> Self {
        Self {
            base,
            interrupt: None,
            state: Arc::new(Mutex::new(rtc)),
        }
    }

    pub fn with_interrupt(
        base: Address,
        rtc: Pl031Rtc,
        _partition: PartitionId,
        source: InterruptSourceId,
        port: InterruptLinePort,
    ) -> Result<Self, Pl031Error> {
        port.validate_route().map_err(Pl031Error::Interrupt)?;
        Ok(Self {
            base,
            interrupt: Some(Pl031Interrupt { source, port }),
            state: Arc::new(Mutex::new(rtc)),
        })
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn range_size_bytes(&self) -> u64 {
        PL031_MMIO_SIZE_BYTES
    }

    pub fn snapshot(&self) -> Pl031RtcMmioSnapshot {
        Pl031RtcMmioSnapshot::new(self.state.lock().expect("PL031 RTC state lock").snapshot())
    }

    pub fn restore(&self, snapshot: &Pl031RtcMmioSnapshot) -> Result<(), Pl031Error> {
        self.state
            .lock()
            .expect("PL031 RTC state lock")
            .restore(snapshot.rtc())
    }

    pub fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let (response, schedule_match) = self.respond_request(request, context.now())?;
        if schedule_match {
            self.schedule_match(context)
                .map_err(|error| mmio_error(request.id(), error))?;
        }
        Ok(response)
    }

    pub fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        let (response, schedule_match) = self.respond_request(request, context.now())?;
        if schedule_match {
            self.schedule_match_parallel(context)
                .map_err(|error| mmio_error(request.id(), error))?;
        }
        Ok(response)
    }

    fn respond_request(
        &self,
        request: &MmioRequest,
        tick: Tick,
    ) -> Result<(MmioResponse, bool), MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        let mut state = self.state.lock().expect("PL031 RTC state lock");
        match request.operation() {
            MmioOperation::Read => {
                let value = state
                    .read_register(offset, tick)
                    .map_err(|error| mmio_error(request.id(), error))?;
                Ok((
                    MmioResponse::completed(request.id(), Some(value.to_le_bytes().to_vec())),
                    false,
                ))
            }
            MmioOperation::Write => {
                let value = mmio_u32(request)?;
                state
                    .write_register(offset, value, tick)
                    .map_err(|error| mmio_error(request.id(), error))?;
                let schedule_match = matches!(offset, PL031_MATCH_OFFSET)
                    || (offset == PL031_LOAD_OFFSET
                        && state.match_value
                            >= state
                                .current_value(tick)
                                .map_err(|error| mmio_error(request.id(), error))?);
                Ok((MmioResponse::completed(request.id(), None), schedule_match))
            }
        }
    }

    fn schedule_match(&self, context: &mut SchedulerContext<'_>) -> Result<(), Pl031Error> {
        let (tick, generation) = {
            let state = self.state.lock().expect("PL031 RTC state lock");
            (state.next_match_tick(context.now())?, state.generation)
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(Pl031Error::TimeWentBack {
                tick,
                last_written_tick: context.now(),
            })?;
        let rtc = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                rtc.fire_match(context, generation);
            })
            .map(|_| ())
            .map_err(Pl031Error::Scheduler)
    }

    fn schedule_match_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
    ) -> Result<(), Pl031Error> {
        let (tick, generation) = {
            let state = self.state.lock().expect("PL031 RTC state lock");
            (state.next_match_tick(context.now())?, state.generation)
        };
        let delay = tick
            .checked_sub(context.now())
            .ok_or(Pl031Error::TimeWentBack {
                tick,
                last_written_tick: context.now(),
            })?;
        let rtc = self.clone();
        context
            .schedule_local_after(delay, move |context| {
                rtc.fire_match_parallel(context, generation);
            })
            .map(|_| ())
            .map_err(Pl031Error::Scheduler)
    }

    fn fire_match(&self, context: &mut SchedulerContext<'_>, generation: u64) {
        let Some(interrupt) = self.record_match_if_current(context.now(), generation) else {
            return;
        };
        if let Err(_error) = interrupt.port.assert(context, interrupt.source) {
            return;
        }
        let _ = interrupt.port.deassert(context, interrupt.source);
    }

    fn fire_match_parallel(&self, context: &mut ParallelSchedulerContext<'_>, generation: u64) {
        let Some(interrupt) = self.record_match_if_current(context.now(), generation) else {
            return;
        };
        if let Err(_error) = interrupt.port.assert_parallel(context, interrupt.source) {
            return;
        }
        let _ = interrupt.port.deassert_parallel(context, interrupt.source);
    }

    fn record_match_if_current(&self, tick: Tick, generation: u64) -> Option<Pl031Interrupt> {
        let mut state = self.state.lock().expect("PL031 RTC state lock");
        if state.generation != generation {
            return None;
        }
        let should_signal = match state.record_match(tick) {
            Ok(should_signal) => should_signal,
            Err(_) => return None,
        };
        drop(state);
        if should_signal {
            self.interrupt.clone()
        } else {
            None
        }
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        if request.size().bytes() != PL031_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: PL031_REGISTER_BYTES,
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

impl MmioDevice for Pl031RtcMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Pl031RtcMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Pl031RtcMmioDevice::respond_parallel(self, context, request)
    }
}

#[derive(Clone, Debug)]
struct Pl031Interrupt {
    source: InterruptSourceId,
    port: InterruptLinePort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pl031Error {
    InvalidTicksPerSecond {
        ticks_per_second: Tick,
    },
    UnknownRegister {
        offset: u64,
    },
    TimeWentBack {
        tick: Tick,
        last_written_tick: Tick,
    },
    MatchTickOverflow {
        now: Tick,
        ticks_per_second: Tick,
        seconds_until: u32,
    },
    GenerationOverflow,
    InvalidPendingInterrupt,
    Interrupt(InterruptError),
    Scheduler(rem6_kernel::SchedulerError),
}

impl fmt::Display for Pl031Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTicksPerSecond { ticks_per_second } => {
                write!(
                    formatter,
                    "PL031 ticks per second must be positive, got {ticks_per_second}"
                )
            }
            Self::UnknownRegister { offset } => {
                write!(formatter, "unknown PL031 RTC register offset {offset:#x}")
            }
            Self::TimeWentBack {
                tick,
                last_written_tick,
            } => write!(
                formatter,
                "PL031 tick {tick} is earlier than last written tick {last_written_tick}"
            ),
            Self::MatchTickOverflow {
                now,
                ticks_per_second,
                seconds_until,
            } => write!(
                formatter,
                "PL031 match tick overflows from {now} with {seconds_until} seconds at {ticks_per_second} ticks per second"
            ),
            Self::GenerationOverflow => write!(formatter, "PL031 match generation overflowed"),
            Self::InvalidPendingInterrupt => {
                write!(formatter, "PL031 pending interrupt snapshot is inconsistent")
            }
            Self::Interrupt(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for Pl031Error {
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
            expected: PL031_REGISTER_BYTES,
            actual: data.len() as u64,
        })?;
    Ok(u32::from_le_bytes(bytes))
}

fn mmio_error(request: rem6_mmio::MmioRequestId, error: Pl031Error) -> MmioError {
    MmioError::DeviceError {
        request,
        message: error.to_string(),
    }
}
