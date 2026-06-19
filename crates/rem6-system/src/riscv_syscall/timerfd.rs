use rem6_kernel::Tick;

use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EAGAIN, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EMFILE, RISCV_LINUX_O_CLOEXEC,
    RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDWR,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_TIMERFD_CREATE: u64 = 85;
pub(super) const RISCV_LINUX_TIMERFD_SETTIME: u64 = 86;
pub(super) const RISCV_LINUX_TIMERFD_GETTIME: u64 = 87;

const RISCV_LINUX_CLOCK_REALTIME: u64 = 0;
const RISCV_LINUX_CLOCK_MONOTONIC: u64 = 1;
const RISCV_LINUX_CLOCK_BOOTTIME: u64 = 7;
const RISCV_LINUX_CLOCK_TAI: u64 = 11;
const RISCV_LINUX_TFD_TIMER_ABSTIME: u64 = 1;
const RISCV_LINUX_TFD_TIMER_CANCEL_ON_SET: u64 = 2;
const RISCV_LINUX_TIMERFD_VALID_CREATE_FLAGS: u64 = RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;
const RISCV_LINUX_TIMERFD_VALID_SETTIME_FLAGS: u64 =
    RISCV_LINUX_TFD_TIMER_ABSTIME | RISCV_LINUX_TFD_TIMER_CANCEL_ON_SET;
const RISCV_LINUX_TIMERFD_COUNTER_BYTES: u64 = 8;
const RISCV_LINUX_TIMERFD_SPEC_BYTES: usize = 32;
const RISCV_LINUX_NANOSECONDS_PER_SECOND: u128 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RiscvLinuxTimerFdTimespec {
    seconds: u64,
    nanoseconds: u64,
}

impl RiscvLinuxTimerFdTimespec {
    const fn zero() -> Self {
        Self {
            seconds: 0,
            nanoseconds: 0,
        }
    }

    const fn is_zero(self) -> bool {
        self.seconds == 0 && self.nanoseconds == 0
    }

    const fn total_nanoseconds(self) -> u128 {
        self.seconds as u128 * RISCV_LINUX_NANOSECONDS_PER_SECOND + self.nanoseconds as u128
    }

    fn from_total_nanoseconds(value: u128) -> Self {
        Self {
            seconds: (value / RISCV_LINUX_NANOSECONDS_PER_SECOND) as u64,
            nanoseconds: (value % RISCV_LINUX_NANOSECONDS_PER_SECOND) as u64,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RiscvLinuxTimerFdSpec {
    interval: RiscvLinuxTimerFdTimespec,
    value: RiscvLinuxTimerFdTimespec,
}

impl RiscvLinuxTimerFdSpec {
    const fn disarmed(interval: RiscvLinuxTimerFdTimespec) -> Self {
        Self {
            interval,
            value: RiscvLinuxTimerFdTimespec::zero(),
        }
    }

    fn encode(self) -> [u8; RISCV_LINUX_TIMERFD_SPEC_BYTES] {
        let mut bytes = [0; RISCV_LINUX_TIMERFD_SPEC_BYTES];
        encode_timespec(self.interval, &mut bytes[0..16]);
        encode_timespec(self.value, &mut bytes[16..32]);
        bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestTimerFd {
    interval: RiscvLinuxTimerFdTimespec,
    next_expiration_ns: Option<u128>,
    expirations: u64,
}

impl RiscvGuestTimerFd {
    const fn disarmed() -> Self {
        Self {
            interval: RiscvLinuxTimerFdTimespec::zero(),
            next_expiration_ns: None,
            expirations: 0,
        }
    }

    fn arm(&mut self, spec: RiscvLinuxTimerFdSpec, flags: u64, tick: Tick) {
        self.interval = spec.interval;
        self.expirations = 0;
        self.next_expiration_ns = if spec.value.is_zero() {
            None
        } else if flags & RISCV_LINUX_TFD_TIMER_ABSTIME != 0 {
            Some(spec.value.total_nanoseconds())
        } else {
            Some(u128::from(tick).saturating_add(spec.value.total_nanoseconds()))
        };
    }

    fn refresh(&mut self, tick: Tick) {
        let Some(deadline) = self.next_expiration_ns else {
            return;
        };
        let now = u128::from(tick);
        if now < deadline {
            return;
        }

        let missed = if self.interval.is_zero() {
            self.next_expiration_ns = None;
            1
        } else {
            let interval = self.interval.total_nanoseconds();
            let missed = 1 + ((now - deadline) / interval);
            self.next_expiration_ns = deadline.checked_add(missed.saturating_mul(interval));
            missed
        };
        self.expirations = self
            .expirations
            .saturating_add(u64::try_from(missed).unwrap_or(u64::MAX));
    }

    fn current_spec(self, tick: Tick) -> RiscvLinuxTimerFdSpec {
        let value = self
            .next_expiration_ns
            .map(|deadline| {
                RiscvLinuxTimerFdTimespec::from_total_nanoseconds(
                    deadline.saturating_sub(u128::from(tick)),
                )
            })
            .unwrap_or_else(RiscvLinuxTimerFdTimespec::zero);
        RiscvLinuxTimerFdSpec {
            interval: self.interval,
            value,
        }
    }

    const fn readable(self) -> bool {
        self.expirations > 0
    }

    const fn read_value(self) -> Option<u64> {
        if self.expirations == 0 {
            None
        } else {
            Some(self.expirations)
        }
    }

    fn consume_read(&mut self) {
        self.expirations = 0;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestTimerFdRead {
    Value(u64),
    Blocked,
    WouldBlock,
    InvalidSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestTimerFdReady {
    readable: bool,
}

impl RiscvGuestTimerFdReady {
    pub(super) const fn readable(self) -> bool {
        self.readable
    }
}

impl RiscvSyscallState {
    pub(super) fn refresh_guest_timerfds(&mut self, tick: Tick) {
        for timerfd in self.guest_timerfds.values_mut() {
            timerfd.refresh(tick);
        }
    }

    pub(super) fn guest_timerfd_read(
        &self,
        fd: GuestFd,
        count: u64,
    ) -> Result<Option<RiscvGuestTimerFdRead>, GuestFdError> {
        let Some(timerfd) = self.timerfd_for_fd(fd)? else {
            return Ok(None);
        };
        if count < RISCV_LINUX_TIMERFD_COUNTER_BYTES {
            return Ok(Some(RiscvGuestTimerFdRead::InvalidSize));
        }
        Ok(Some(match timerfd.read_value() {
            Some(value) => RiscvGuestTimerFdRead::Value(value),
            None if self.timerfd_nonblocking(fd)? => RiscvGuestTimerFdRead::WouldBlock,
            None => RiscvGuestTimerFdRead::Blocked,
        }))
    }

    pub(super) fn consume_guest_timerfd_read(&mut self, fd: GuestFd) -> Result<(), GuestFdError> {
        let Some(description) = self.timerfd_description_for_fd(fd)? else {
            return Err(GuestFdError::BadFd { fd });
        };
        let timerfd = self
            .guest_timerfds
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        timerfd.consume_read();
        Ok(())
    }

    pub(super) fn guest_timerfd_ready(
        &self,
        fd: GuestFd,
    ) -> Result<Option<RiscvGuestTimerFdReady>, GuestFdError> {
        let Some(timerfd) = self.timerfd_for_fd(fd)? else {
            return Ok(None);
        };
        Ok(Some(RiscvGuestTimerFdReady {
            readable: timerfd.readable(),
        }))
    }

    pub(super) fn remove_guest_timerfd_description(&mut self, description: GuestFileDescriptionId) {
        self.guest_timerfds.remove(&description);
    }

    fn open_guest_timerfd(&mut self, flags: u64) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_guest_fd_excluding(&[])?;
        let description = self.next_open_description()?;
        let close_on_exec = flags & RISCV_LINUX_O_CLOEXEC != 0;
        let status_flags = RISCV_LINUX_O_RDWR | (flags & RISCV_LINUX_O_NONBLOCK);
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(status_flags as u32),
            ))?;
        self.guest_fds.insert(
            fd,
            GuestFdEntry::new(description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_timerfds
            .insert(description, RiscvGuestTimerFd::disarmed());
        Ok(fd)
    }

    fn timerfd_description_for_fd(
        &self,
        fd: GuestFd,
    ) -> Result<Option<GuestFileDescriptionId>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self
            .guest_timerfds
            .contains_key(&description)
            .then_some(description))
    }

    fn timerfd_for_fd(&self, fd: GuestFd) -> Result<Option<RiscvGuestTimerFd>, GuestFdError> {
        let Some(description) = self.timerfd_description_for_fd(fd)? else {
            return Ok(None);
        };
        self.guest_timerfds
            .get(&description)
            .copied()
            .map(Some)
            .ok_or(GuestFdError::MissingFileDescription { description })
    }

    fn arm_guest_timerfd(
        &mut self,
        fd: GuestFd,
        spec: RiscvLinuxTimerFdSpec,
        flags: u64,
        tick: Tick,
    ) -> Result<bool, GuestFdError> {
        let Some(description) = self.timerfd_description_for_fd(fd)? else {
            return Ok(false);
        };
        let timerfd = self
            .guest_timerfds
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        timerfd.arm(spec, flags, tick);
        Ok(true)
    }

    fn guest_timerfd_current_spec(
        &self,
        fd: GuestFd,
        tick: Tick,
    ) -> Result<Option<RiscvLinuxTimerFdSpec>, GuestFdError> {
        Ok(self
            .timerfd_for_fd(fd)?
            .map(|timerfd| timerfd.current_spec(tick)))
    }

    fn timerfd_nonblocking(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        Ok(self.guest_fds.status_flags(fd)?.bits() & RISCV_LINUX_O_NONBLOCK as u32 != 0)
    }
}

pub(super) fn syscall_timerfd_create(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    if !valid_timerfd_clock_id(request.argument(0)) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let flags = u64::from(request.argument(1) as u32);
    if flags & !RISCV_LINUX_TIMERFD_VALID_CREATE_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.open_guest_timerfd(flags) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn syscall_timerfd_settime(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let old_spec = match state.guest_timerfd_current_spec(fd, tick) {
        Ok(Some(spec)) => spec,
        Ok(None) => return Some(linux_error(RISCV_LINUX_EINVAL)),
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    };
    let flags = u64::from(request.argument(1) as u32);
    if flags & !RISCV_LINUX_TIMERFD_VALID_SETTIME_FLAGS != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    let guest_memory_reader = guest_memory_reader?;
    let new_spec = match read_timerfd_spec(guest_memory_reader, request.argument(2)) {
        Ok(spec) => spec,
        Err(error) => return Some(linux_error(error)),
    };
    if request.argument(3) != 0 {
        let guest_memory_writer = guest_memory_writer?;
        if !guest_memory_writer.write(request.argument(3), &old_spec.encode()) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }
    match state.arm_guest_timerfd(fd, new_spec, flags, tick) {
        Ok(true) => Some(0),
        Ok(false) => Some(linux_error(RISCV_LINUX_EINVAL)),
        Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}

pub(super) fn syscall_timerfd_gettime(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    tick: Tick,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let spec = match state.guest_timerfd_current_spec(fd, tick) {
        Ok(Some(spec)) => spec,
        Ok(None) => return Some(linux_error(RISCV_LINUX_EINVAL)),
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    };
    let guest_memory_writer = guest_memory_writer?;
    if guest_memory_writer.write(request.argument(1), &spec.encode()) {
        Some(0)
    } else {
        Some(linux_error(RISCV_LINUX_EFAULT))
    }
}

pub(super) fn timerfd_read_bytes(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

pub(super) fn timerfd_read_result(read: RiscvGuestTimerFdRead) -> Option<u64> {
    match read {
        RiscvGuestTimerFdRead::Value(_) => Some(RISCV_LINUX_TIMERFD_COUNTER_BYTES),
        RiscvGuestTimerFdRead::Blocked => None,
        RiscvGuestTimerFdRead::WouldBlock => Some(linux_error(RISCV_LINUX_EAGAIN)),
        RiscvGuestTimerFdRead::InvalidSize => Some(linux_error(RISCV_LINUX_EINVAL)),
    }
}

pub(super) fn timerfd_write_result() -> u64 {
    linux_error(RISCV_LINUX_EINVAL)
}

fn valid_timerfd_clock_id(clock_id: u64) -> bool {
    matches!(
        clock_id,
        RISCV_LINUX_CLOCK_REALTIME
            | RISCV_LINUX_CLOCK_MONOTONIC
            | RISCV_LINUX_CLOCK_BOOTTIME
            | RISCV_LINUX_CLOCK_TAI
    )
}

fn read_timerfd_spec(
    guest_memory_reader: &RiscvGuestMemoryReader,
    address: u64,
) -> Result<RiscvLinuxTimerFdSpec, u64> {
    let Some(bytes) = guest_memory_reader.read(address, RISCV_LINUX_TIMERFD_SPEC_BYTES) else {
        return Err(RISCV_LINUX_EFAULT);
    };
    if bytes.len() != RISCV_LINUX_TIMERFD_SPEC_BYTES {
        return Err(RISCV_LINUX_EFAULT);
    }
    let interval = decode_timespec(&bytes[0..16])?;
    let value = decode_timespec(&bytes[16..32])?;
    if value.is_zero() {
        Ok(RiscvLinuxTimerFdSpec::disarmed(interval))
    } else {
        Ok(RiscvLinuxTimerFdSpec { interval, value })
    }
}

fn decode_timespec(bytes: &[u8]) -> Result<RiscvLinuxTimerFdTimespec, u64> {
    let seconds = i64::from_le_bytes(bytes[0..8].try_into().expect("timespec seconds width"));
    let nanoseconds =
        i64::from_le_bytes(bytes[8..16].try_into().expect("timespec nanoseconds width"));
    if seconds < 0 || !(0..1_000_000_000).contains(&nanoseconds) {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(RiscvLinuxTimerFdTimespec {
        seconds: seconds as u64,
        nanoseconds: nanoseconds as u64,
    })
}

fn encode_timespec(timespec: RiscvLinuxTimerFdTimespec, bytes: &mut [u8]) {
    bytes[0..8].copy_from_slice(&timespec.seconds.to_le_bytes());
    bytes[8..16].copy_from_slice(&timespec.nanoseconds.to_le_bytes());
}
