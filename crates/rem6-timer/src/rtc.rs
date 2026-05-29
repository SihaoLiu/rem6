use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext};
use rem6_memory::{Address, ByteMask};
use rem6_mmio::{MmioDevice, MmioError, MmioOperation, MmioRequest, MmioResponse};

pub const RTC_SECONDS_REGISTER: u8 = 0x00;
pub const RTC_SECONDS_ALARM_REGISTER: u8 = 0x01;
pub const RTC_MINUTES_REGISTER: u8 = 0x02;
pub const RTC_MINUTES_ALARM_REGISTER: u8 = 0x03;
pub const RTC_HOURS_REGISTER: u8 = 0x04;
pub const RTC_HOURS_ALARM_REGISTER: u8 = 0x05;
pub const RTC_DAY_OF_WEEK_REGISTER: u8 = 0x06;
pub const RTC_DAY_OF_MONTH_REGISTER: u8 = 0x07;
pub const RTC_MONTH_REGISTER: u8 = 0x08;
pub const RTC_YEARS_REGISTER: u8 = 0x09;
pub const RTC_STATUS_A_REGISTER: u8 = 0x0a;
pub const RTC_STATUS_B_REGISTER: u8 = 0x0b;
pub const RTC_STATUS_C_REGISTER: u8 = 0x0c;
pub const RTC_STATUS_D_REGISTER: u8 = 0x0d;
pub const RTC_MMIO_REGISTER_BYTES: u64 = 1;
pub const RTC_MMIO_ADDRESS_OFFSET: u64 = 0x00;
pub const RTC_MMIO_DATA_OFFSET: u64 = 0x01;
pub const RTC_CMOS_REGISTER_COUNT: usize = 128;

const RTC_CLOCK_REGISTER_COUNT: usize = 10;
const RTC_CMOS_REGISTER_MASK: u8 = 0x7f;
const STATUS_A_DEFAULT: u8 = 0x26;
const STATUS_A_UIP: u8 = 0x80;
const STATUS_A_DV_MASK: u8 = 0x70;
const STATUS_A_RS_MASK: u8 = 0x0f;
const STATUS_A_DV_32768HZ: u8 = 0x20;
const STATUS_A_DV_DISABLED0: u8 = 0x60;
const STATUS_A_DV_DISABLED1: u8 = 0x70;
const STATUS_A_RS_1024HZ: u8 = 0x06;

const STATUS_B_SET: u8 = 0x80;
const STATUS_B_PIE: u8 = 0x40;
const STATUS_B_AIE: u8 = 0x20;
const STATUS_B_UIE: u8 = 0x10;
const STATUS_B_DM: u8 = 0x04;
const STATUS_B_24H: u8 = 0x02;
const STATUS_B_DSE: u8 = 0x01;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RtcEncoding {
    Binary,
    Bcd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RtcDateTime {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    weekday: u8,
}

impl RtcDateTime {
    pub fn new(
        year: u16,
        month: u8,
        day: u8,
        hour: u8,
        minute: u8,
        second: u8,
        weekday: u8,
    ) -> Result<Self, RtcError> {
        if !(1900..=2099).contains(&year) {
            return Err(RtcError::InvalidDateTime {
                field: "year",
                value: u32::from(year),
            });
        }
        if !(1..=12).contains(&month) {
            return Err(RtcError::InvalidDateTime {
                field: "month",
                value: u32::from(month),
            });
        }
        if day == 0 || day > days_in_month(year, month) {
            return Err(RtcError::InvalidDateTime {
                field: "day",
                value: u32::from(day),
            });
        }
        if hour > 23 {
            return Err(RtcError::InvalidDateTime {
                field: "hour",
                value: u32::from(hour),
            });
        }
        if minute > 59 {
            return Err(RtcError::InvalidDateTime {
                field: "minute",
                value: u32::from(minute),
            });
        }
        if second > 59 {
            return Err(RtcError::InvalidDateTime {
                field: "second",
                value: u32::from(second),
            });
        }
        if !(1..=7).contains(&weekday) {
            return Err(RtcError::InvalidDateTime {
                field: "weekday",
                value: u32::from(weekday),
            });
        }
        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            weekday,
        })
    }

    pub const fn year(self) -> u16 {
        self.year
    }

    pub const fn month(self) -> u8 {
        self.month
    }

    pub const fn day(self) -> u8 {
        self.day
    }

    pub const fn hour(self) -> u8 {
        self.hour
    }

    pub const fn minute(self) -> u8 {
        self.minute
    }

    pub const fn second(self) -> u8 {
        self.second
    }

    pub const fn weekday(self) -> u8 {
        self.weekday
    }

    fn advance_one_second(self) -> Result<Self, RtcError> {
        let mut next = self;
        next.second += 1;
        if next.second <= 59 {
            return Ok(next);
        }
        next.second = 0;
        next.minute += 1;
        if next.minute <= 59 {
            return Ok(next);
        }
        next.minute = 0;
        next.hour += 1;
        if next.hour <= 23 {
            return Ok(next);
        }
        next.hour = 0;
        next.weekday = if next.weekday == 7 {
            1
        } else {
            next.weekday + 1
        };
        next.day += 1;
        if next.day <= days_in_month(next.year, next.month) {
            return Ok(next);
        }
        next.day = 1;
        next.month += 1;
        if next.month <= 12 {
            return Ok(next);
        }
        next.month = 1;
        next.year = next.year.checked_add(1).ok_or(RtcError::InvalidDateTime {
            field: "year",
            value: u32::MAX,
        })?;
        Self::new(
            next.year,
            next.month,
            next.day,
            next.hour,
            next.minute,
            next.second,
            next.weekday,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RtcSnapshot {
    clock_data: [u8; RTC_CLOCK_REGISTER_COUNT],
    status_a: u8,
    status_b: u8,
}

impl RtcSnapshot {
    pub const fn new(
        clock_data: [u8; RTC_CLOCK_REGISTER_COUNT],
        status_a: u8,
        status_b: u8,
    ) -> Self {
        Self {
            clock_data,
            status_a,
            status_b,
        }
    }

    pub const fn clock_data(&self) -> &[u8; RTC_CLOCK_REGISTER_COUNT] {
        &self.clock_data
    }

    pub const fn status_a(&self) -> u8 {
        self.status_a
    }

    pub const fn status_b(&self) -> u8 {
        self.status_b
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mc146818Rtc {
    clock_data: [u8; RTC_CLOCK_REGISTER_COUNT],
    status_a: u8,
    status_b: u8,
}

impl Mc146818Rtc {
    pub fn new(time: RtcDateTime, encoding: RtcEncoding) -> Result<Self, RtcError> {
        let mut rtc = Self {
            clock_data: [0; RTC_CLOCK_REGISTER_COUNT],
            status_a: STATUS_A_DEFAULT,
            status_b: STATUS_B_PIE | STATUS_B_24H | status_b_encoding_bit(encoding),
        };
        rtc.set_time(time)?;
        Ok(rtc)
    }

    pub fn read_register(&self, register: u8) -> Result<u8, RtcError> {
        match register {
            RTC_SECONDS_REGISTER..=RTC_YEARS_REGISTER => Ok(self.clock_data[usize::from(register)]),
            RTC_STATUS_A_REGISTER => Ok(self.status_a & !STATUS_A_UIP),
            RTC_STATUS_B_REGISTER => Ok(self.status_b),
            RTC_STATUS_C_REGISTER | RTC_STATUS_D_REGISTER => Ok(0),
            _ => Err(RtcError::UnknownRegister { register }),
        }
    }

    pub fn write_register(&mut self, register: u8, value: u8) -> Result<(), RtcError> {
        match register {
            RTC_SECONDS_REGISTER
            | RTC_MINUTES_REGISTER
            | RTC_HOURS_REGISTER
            | RTC_DAY_OF_WEEK_REGISTER
            | RTC_DAY_OF_MONTH_REGISTER
            | RTC_MONTH_REGISTER
            | RTC_YEARS_REGISTER => self.write_calendar_register(register, value),
            RTC_SECONDS_ALARM_REGISTER | RTC_MINUTES_ALARM_REGISTER | RTC_HOURS_ALARM_REGISTER => {
                self.clock_data[usize::from(register)] = value;
                Ok(())
            }
            RTC_STATUS_A_REGISTER => self.write_status_a(value),
            RTC_STATUS_B_REGISTER => self.write_status_b(value),
            RTC_STATUS_C_REGISTER | RTC_STATUS_D_REGISTER => {
                Err(RtcError::ReadOnlyRegister { register })
            }
            _ => Err(RtcError::UnknownRegister { register }),
        }
    }

    pub fn tick_second(&mut self) -> Result<(), RtcError> {
        if status_a_divider_disabled(self.status_a) {
            return Err(RtcError::DividerDisabled);
        }
        if self.status_b & STATUS_B_SET != 0 {
            return Ok(());
        }
        let next = self.date_time()?.advance_one_second()?;
        self.set_time(next)
    }

    pub fn date_time(&self) -> Result<RtcDateTime, RtcError> {
        decode_time(&self.clock_data, rtc_encoding(self.status_b))
    }

    pub fn snapshot(&self) -> RtcSnapshot {
        RtcSnapshot::new(self.clock_data, self.status_a, self.status_b)
    }

    pub fn restore(&mut self, snapshot: &RtcSnapshot) -> Result<(), RtcError> {
        validate_status_a(snapshot.status_a)?;
        validate_status_b(snapshot.status_b)?;
        decode_time(&snapshot.clock_data, rtc_encoding(snapshot.status_b))?;
        self.clock_data = snapshot.clock_data;
        self.status_a = snapshot.status_a;
        self.status_b = snapshot.status_b;
        Ok(())
    }

    fn write_calendar_register(&mut self, register: u8, value: u8) -> Result<(), RtcError> {
        let previous = self.clock_data[usize::from(register)];
        self.clock_data[usize::from(register)] = value;
        if let Err(error) = self.date_time() {
            self.clock_data[usize::from(register)] = previous;
            return Err(error);
        }
        Ok(())
    }

    fn write_status_a(&mut self, value: u8) -> Result<(), RtcError> {
        let next = (value & !STATUS_A_UIP) | (self.status_a & STATUS_A_UIP);
        validate_status_a(next)?;
        self.status_a = next;
        Ok(())
    }

    fn write_status_b(&mut self, value: u8) -> Result<(), RtcError> {
        validate_status_b(value)?;
        let old_encoding = rtc_encoding(self.status_b);
        let new_encoding = rtc_encoding(value);
        if old_encoding == new_encoding {
            self.status_b = value;
            return Ok(());
        }

        let time = self.date_time()?;
        self.status_b = value;
        self.set_time(time)
    }

    fn set_time(&mut self, time: RtcDateTime) -> Result<(), RtcError> {
        RtcDateTime::new(
            time.year(),
            time.month(),
            time.day(),
            time.hour(),
            time.minute(),
            time.second(),
            time.weekday(),
        )?;
        let encoding = rtc_encoding(self.status_b);
        self.clock_data[usize::from(RTC_SECONDS_REGISTER)] =
            encode_two_digit(time.second, encoding);
        self.clock_data[usize::from(RTC_MINUTES_REGISTER)] =
            encode_two_digit(time.minute, encoding);
        self.clock_data[usize::from(RTC_HOURS_REGISTER)] = encode_two_digit(time.hour, encoding);
        self.clock_data[usize::from(RTC_DAY_OF_WEEK_REGISTER)] = time.weekday;
        self.clock_data[usize::from(RTC_DAY_OF_MONTH_REGISTER)] =
            encode_two_digit(time.day, encoding);
        self.clock_data[usize::from(RTC_MONTH_REGISTER)] = encode_two_digit(time.month, encoding);
        self.clock_data[usize::from(RTC_YEARS_REGISTER)] = match encoding {
            RtcEncoding::Binary => {
                u8::try_from(time.year - 1900).map_err(|_| RtcError::InvalidDateTime {
                    field: "year",
                    value: u32::from(time.year),
                })?
            }
            RtcEncoding::Bcd => bcd_encode(((time.year - 1900) % 100) as u8),
        };
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RtcError {
    InvalidDateTime { field: &'static str, value: u32 },
    InvalidBcd { register: u8, value: u8 },
    UnsupportedStatusA { register: u8, value: u8 },
    UnsupportedStatusB { register: u8, value: u8 },
    ReadOnlyRegister { register: u8 },
    UnknownRegister { register: u8 },
    DividerDisabled,
}

impl fmt::Display for RtcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDateTime { field, value } => {
                write!(formatter, "invalid RTC {field} value {value}")
            }
            Self::InvalidBcd { register, value } => write!(
                formatter,
                "invalid RTC BCD value {value:#04x} in register {register:#04x}"
            ),
            Self::UnsupportedStatusA { register, value } => write!(
                formatter,
                "unsupported RTC status A value {value:#04x} for register {register:#04x}"
            ),
            Self::UnsupportedStatusB { register, value } => write!(
                formatter,
                "unsupported RTC status B value {value:#04x} for register {register:#04x}"
            ),
            Self::ReadOnlyRegister { register } => {
                write!(formatter, "RTC register {register:#04x} is read-only")
            }
            Self::UnknownRegister { register } => {
                write!(formatter, "unknown RTC register {register:#04x}")
            }
            Self::DividerDisabled => write!(formatter, "RTC divider is disabled"),
        }
    }
}

impl Error for RtcError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mc146818RtcMmioSnapshot {
    selected_address: u8,
    cmos_data: [u8; RTC_CMOS_REGISTER_COUNT],
    rtc: RtcSnapshot,
}

impl Mc146818RtcMmioSnapshot {
    pub const fn new(
        selected_address: u8,
        cmos_data: [u8; RTC_CMOS_REGISTER_COUNT],
        rtc: RtcSnapshot,
    ) -> Self {
        Self {
            selected_address,
            cmos_data,
            rtc,
        }
    }

    pub const fn selected_address(&self) -> u8 {
        self.selected_address
    }

    pub const fn cmos_data(&self) -> &[u8; RTC_CMOS_REGISTER_COUNT] {
        &self.cmos_data
    }

    pub const fn rtc(&self) -> &RtcSnapshot {
        &self.rtc
    }
}

#[derive(Clone, Debug)]
pub struct Mc146818RtcMmioDevice {
    base: Address,
    state: Arc<Mutex<Mc146818RtcMmioState>>,
}

impl Mc146818RtcMmioDevice {
    pub fn new(base: Address, rtc: Mc146818Rtc) -> Self {
        Self {
            base,
            state: Arc::new(Mutex::new(Mc146818RtcMmioState::new(rtc))),
        }
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub fn snapshot(&self) -> Mc146818RtcMmioSnapshot {
        self.state.lock().expect("RTC MMIO state lock").snapshot()
    }

    pub fn restore(&self, snapshot: &Mc146818RtcMmioSnapshot) -> Result<(), RtcError> {
        let mut state = self.state.lock().expect("RTC MMIO state lock");
        let mut rtc = state.rtc.clone();
        rtc.restore(snapshot.rtc())?;
        state.selected_address = snapshot.selected_address();
        state.cmos_data = *snapshot.cmos_data();
        state.rtc = rtc;
        Ok(())
    }

    pub fn respond(
        &self,
        _context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_request(request)
    }

    pub fn respond_parallel(
        &self,
        _context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        self.respond_request(request)
    }

    fn respond_request(&self, request: &MmioRequest) -> Result<MmioResponse, MmioError> {
        self.validate_size(request)?;
        let offset = self.offset(request)?;
        let mut state = self.state.lock().expect("RTC MMIO state lock");
        match (offset, request.operation()) {
            (RTC_MMIO_ADDRESS_OFFSET, MmioOperation::Read) => Ok(MmioResponse::completed(
                request.id(),
                Some(vec![state.selected_address]),
            )),
            (RTC_MMIO_ADDRESS_OFFSET, MmioOperation::Write) => {
                if let Some(value) = rtc_mmio_write_byte(request)? {
                    state.selected_address = value;
                }
                Ok(MmioResponse::completed(request.id(), None))
            }
            (RTC_MMIO_DATA_OFFSET, MmioOperation::Read) => {
                let value = state.read_data().map_err(|error| MmioError::DeviceError {
                    request: request.id(),
                    message: error.to_string(),
                })?;
                Ok(MmioResponse::completed(request.id(), Some(vec![value])))
            }
            (RTC_MMIO_DATA_OFFSET, MmioOperation::Write) => {
                if let Some(value) = rtc_mmio_write_byte(request)? {
                    state
                        .write_data(value)
                        .map_err(|error| MmioError::DeviceError {
                            request: request.id(),
                            message: error.to_string(),
                        })?;
                }
                Ok(MmioResponse::completed(request.id(), None))
            }
            _ => Err(MmioError::UnmappedAddress {
                address: request.range().start(),
            }),
        }
    }

    fn validate_size(&self, request: &MmioRequest) -> Result<(), MmioError> {
        if request.size().bytes() != RTC_MMIO_REGISTER_BYTES {
            return Err(MmioError::AccessSizeMismatch {
                request: request.id(),
                expected: RTC_MMIO_REGISTER_BYTES,
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

impl MmioDevice for Mc146818RtcMmioDevice {
    fn respond(
        &self,
        context: &mut SchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Mc146818RtcMmioDevice::respond(self, context, request)
    }

    fn respond_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: &MmioRequest,
    ) -> Result<MmioResponse, MmioError> {
        Mc146818RtcMmioDevice::respond_parallel(self, context, request)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Mc146818RtcMmioState {
    selected_address: u8,
    cmos_data: [u8; RTC_CMOS_REGISTER_COUNT],
    rtc: Mc146818Rtc,
}

impl Mc146818RtcMmioState {
    const fn new(rtc: Mc146818Rtc) -> Self {
        Self {
            selected_address: 0,
            cmos_data: [0; RTC_CMOS_REGISTER_COUNT],
            rtc,
        }
    }

    fn snapshot(&self) -> Mc146818RtcMmioSnapshot {
        Mc146818RtcMmioSnapshot::new(self.selected_address, self.cmos_data, self.rtc.snapshot())
    }

    fn selected_register(&self) -> u8 {
        self.selected_address & RTC_CMOS_REGISTER_MASK
    }

    fn read_data(&self) -> Result<u8, RtcError> {
        let register = self.selected_register();
        if is_rtc_register(register) {
            return self.rtc.read_register(register);
        }
        Ok(self.cmos_data[usize::from(register)])
    }

    fn write_data(&mut self, value: u8) -> Result<(), RtcError> {
        let register = self.selected_register();
        if is_rtc_register(register) {
            return self.rtc.write_register(register, value);
        }
        self.cmos_data[usize::from(register)] = value;
        Ok(())
    }
}

fn rtc_mmio_write_byte(request: &MmioRequest) -> Result<Option<u8>, MmioError> {
    let data = request.data().ok_or(MmioError::MissingWriteData {
        request: request.id(),
    })?;
    if data.len() as u64 != RTC_MMIO_REGISTER_BYTES {
        return Err(MmioError::PayloadSizeMismatch {
            request: request.id(),
            expected: RTC_MMIO_REGISTER_BYTES,
            actual: data.len() as u64,
        });
    }
    let mask = request.byte_mask().ok_or(MmioError::MissingByteMask {
        request: request.id(),
    })?;
    validate_rtc_mmio_mask(request, mask)?;
    Ok(mask.bits()[0].then_some(data[0]))
}

fn validate_rtc_mmio_mask(request: &MmioRequest, mask: &ByteMask) -> Result<(), MmioError> {
    if mask.len() != RTC_MMIO_REGISTER_BYTES {
        return Err(MmioError::ByteMaskSizeMismatch {
            request: request.id(),
            expected: RTC_MMIO_REGISTER_BYTES,
            actual: mask.len(),
        });
    }
    Ok(())
}

const fn is_rtc_register(register: u8) -> bool {
    register <= RTC_STATUS_D_REGISTER
}

fn validate_status_a(value: u8) -> Result<(), RtcError> {
    let divider = value & STATUS_A_DV_MASK;
    let rate = value & STATUS_A_RS_MASK;
    let supported_divider = matches!(
        divider,
        STATUS_A_DV_32768HZ | STATUS_A_DV_DISABLED0 | STATUS_A_DV_DISABLED1
    );
    if !supported_divider || rate != STATUS_A_RS_1024HZ {
        return Err(RtcError::UnsupportedStatusA {
            register: RTC_STATUS_A_REGISTER,
            value,
        });
    }
    Ok(())
}

fn validate_status_b(value: u8) -> Result<(), RtcError> {
    if value & (STATUS_B_AIE | STATUS_B_UIE | STATUS_B_DSE) != 0 || value & STATUS_B_24H == 0 {
        return Err(RtcError::UnsupportedStatusB {
            register: RTC_STATUS_B_REGISTER,
            value,
        });
    }
    Ok(())
}

const fn rtc_encoding(status_b: u8) -> RtcEncoding {
    if status_b & STATUS_B_DM == 0 {
        RtcEncoding::Bcd
    } else {
        RtcEncoding::Binary
    }
}

const fn status_b_encoding_bit(encoding: RtcEncoding) -> u8 {
    match encoding {
        RtcEncoding::Binary => STATUS_B_DM,
        RtcEncoding::Bcd => 0,
    }
}

const fn status_a_divider_disabled(status_a: u8) -> bool {
    matches!(
        status_a & STATUS_A_DV_MASK,
        STATUS_A_DV_DISABLED0 | STATUS_A_DV_DISABLED1
    )
}

fn decode_time(
    clock_data: &[u8; RTC_CLOCK_REGISTER_COUNT],
    encoding: RtcEncoding,
) -> Result<RtcDateTime, RtcError> {
    let second = decode_two_digit(RTC_SECONDS_REGISTER, clock_data[0], encoding)?;
    let minute = decode_two_digit(RTC_MINUTES_REGISTER, clock_data[2], encoding)?;
    let hour = decode_two_digit(RTC_HOURS_REGISTER, clock_data[4], encoding)?;
    let weekday = decode_weekday(clock_data[6], encoding)?;
    let day = decode_two_digit(RTC_DAY_OF_MONTH_REGISTER, clock_data[7], encoding)?;
    let month = decode_two_digit(RTC_MONTH_REGISTER, clock_data[8], encoding)?;
    let year = decode_year(clock_data[9], encoding)?;
    RtcDateTime::new(year, month, day, hour, minute, second, weekday)
}

fn decode_year(value: u8, encoding: RtcEncoding) -> Result<u16, RtcError> {
    match encoding {
        RtcEncoding::Binary => Ok(1900 + u16::from(value)),
        RtcEncoding::Bcd => {
            let two_digit = u16::from(bcd_decode(RTC_YEARS_REGISTER, value)?);
            Ok(((two_digit + 50) % 100) + 1950)
        }
    }
}

fn decode_two_digit(register: u8, value: u8, encoding: RtcEncoding) -> Result<u8, RtcError> {
    match encoding {
        RtcEncoding::Binary => Ok(value),
        RtcEncoding::Bcd => bcd_decode(register, value),
    }
}

fn decode_weekday(value: u8, encoding: RtcEncoding) -> Result<u8, RtcError> {
    match encoding {
        RtcEncoding::Binary => Ok(value),
        RtcEncoding::Bcd => bcd_decode(RTC_DAY_OF_WEEK_REGISTER, value),
    }
}

fn encode_two_digit(value: u8, encoding: RtcEncoding) -> u8 {
    match encoding {
        RtcEncoding::Binary => value,
        RtcEncoding::Bcd => bcd_encode(value),
    }
}

fn bcd_encode(value: u8) -> u8 {
    ((value / 10) << 4) | (value % 10)
}

fn bcd_decode(register: u8, value: u8) -> Result<u8, RtcError> {
    let high = value >> 4;
    let low = value & 0x0f;
    if high > 9 || low > 9 {
        return Err(RtcError::InvalidBcd { register, value });
    }
    Ok(high * 10 + low)
}

const fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(4) && !year.is_multiple_of(100) || year.is_multiple_of(400)
}

const fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}
