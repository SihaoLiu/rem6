use rem6_kernel::Tick;

use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EAGAIN, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_ENOTSUP,
};

pub(super) const RISCV_LINUX_TIMER_CREATE: u64 = 107;
pub(super) const RISCV_LINUX_TIMER_GETTIME: u64 = 108;
pub(super) const RISCV_LINUX_TIMER_GETOVERRUN: u64 = 109;
pub(super) const RISCV_LINUX_TIMER_SETTIME: u64 = 110;
pub(super) const RISCV_LINUX_TIMER_DELETE: u64 = 111;

const RISCV_LINUX_CLOCK_REALTIME: u64 = 0;
const RISCV_LINUX_CLOCK_MONOTONIC: u64 = 1;
const RISCV_LINUX_CLOCK_BOOTTIME: u64 = 7;
const RISCV_LINUX_CLOCK_TAI: u64 = 11;
const RISCV_LINUX_TIMER_ABSTIME: u64 = 1;
const RISCV_LINUX_ITIMERSPEC_BYTES: usize = 32;
const RISCV_LINUX_SIGEVENT_BYTES: usize = 64;
const RISCV_LINUX_SIGEVENT_NOTIFY_OFFSET: usize = 12;
const RISCV_LINUX_SIGEV_SIGNAL: u32 = 0;
const RISCV_LINUX_SIGEV_NONE: u32 = 1;
const RISCV_LINUX_SIGEV_THREAD: u32 = 2;
const RISCV_LINUX_SIGEV_THREAD_ID: u32 = 4;
const RISCV_LINUX_NANOSECONDS_PER_SECOND: u128 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RiscvLinuxPosixTimerTimespec {
    seconds: u64,
    nanoseconds: u64,
}

impl RiscvLinuxPosixTimerTimespec {
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
struct RiscvLinuxPosixTimerSpec {
    interval: RiscvLinuxPosixTimerTimespec,
    value: RiscvLinuxPosixTimerTimespec,
}

impl RiscvLinuxPosixTimerSpec {
    const fn disarmed(interval: RiscvLinuxPosixTimerTimespec) -> Self {
        Self {
            interval,
            value: RiscvLinuxPosixTimerTimespec::zero(),
        }
    }

    fn encode(self) -> [u8; RISCV_LINUX_ITIMERSPEC_BYTES] {
        let mut bytes = [0; RISCV_LINUX_ITIMERSPEC_BYTES];
        encode_timespec(self.interval, &mut bytes[0..16]);
        encode_timespec(self.value, &mut bytes[16..32]);
        bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestPosixTimer {
    interval: RiscvLinuxPosixTimerTimespec,
    next_expiration_ns: Option<u128>,
}

impl RiscvGuestPosixTimer {
    const fn disarmed() -> Self {
        Self {
            interval: RiscvLinuxPosixTimerTimespec::zero(),
            next_expiration_ns: None,
        }
    }

    fn arm(&mut self, spec: RiscvLinuxPosixTimerSpec, flags: u64, tick: Tick) {
        self.interval = spec.interval;
        self.next_expiration_ns = if spec.value.is_zero() {
            None
        } else if flags & RISCV_LINUX_TIMER_ABSTIME != 0 {
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

        self.next_expiration_ns = if self.interval.is_zero() {
            None
        } else {
            let interval = self.interval.total_nanoseconds();
            let missed = 1 + ((now - deadline) / interval);
            deadline.checked_add(missed.saturating_mul(interval))
        };
    }

    fn current_spec(self, tick: Tick) -> RiscvLinuxPosixTimerSpec {
        let value = self
            .next_expiration_ns
            .map(|deadline| {
                RiscvLinuxPosixTimerTimespec::from_total_nanoseconds(
                    deadline.saturating_sub(u128::from(tick)),
                )
            })
            .unwrap_or_else(RiscvLinuxPosixTimerTimespec::zero);
        RiscvLinuxPosixTimerSpec {
            interval: self.interval,
            value,
        }
    }
}

impl RiscvSyscallState {
    pub(super) fn refresh_guest_posix_timers(&mut self, tick: Tick) {
        for timer in self.guest_posix_timers.values_mut() {
            timer.refresh(tick);
        }
    }

    fn guest_posix_timer_current_spec(
        &self,
        timer_id: u32,
        tick: Tick,
    ) -> Option<RiscvLinuxPosixTimerSpec> {
        self.guest_posix_timers
            .get(&timer_id)
            .map(|timer| timer.current_spec(tick))
    }

    fn arm_guest_posix_timer(
        &mut self,
        timer_id: u32,
        spec: RiscvLinuxPosixTimerSpec,
        flags: u64,
        tick: Tick,
    ) -> bool {
        let Some(timer) = self.guest_posix_timers.get_mut(&timer_id) else {
            return false;
        };
        timer.arm(spec, flags, tick);
        true
    }

    fn insert_guest_posix_timer(&mut self, timer_id: u32, next_timer_id: u32) {
        self.next_guest_posix_timer_id = next_timer_id;
        self.guest_posix_timers
            .insert(timer_id, RiscvGuestPosixTimer::disarmed());
    }

    fn delete_guest_posix_timer(&mut self, timer_id: u32) -> bool {
        self.guest_posix_timers.remove(&timer_id).is_some()
    }
}

pub(super) fn syscall_timer_create(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    if !valid_posix_timer_clock_id(request.argument(0)) {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if request.argument(2) == 0 {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    match validate_sigevent(request.argument(1), guest_memory_reader) {
        Some(Ok(())) => {}
        Some(Err(error)) => return Some(linux_error(error)),
        None => return None,
    }

    let timer_id = state.next_guest_posix_timer_id;
    let Some(next_timer_id) = timer_id.checked_add(1) else {
        return Some(linux_error(RISCV_LINUX_EAGAIN));
    };
    let guest_memory_writer = guest_memory_writer?;
    if !guest_memory_writer.write(request.argument(2), &timer_id.to_le_bytes()) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    state.insert_guest_posix_timer(timer_id, next_timer_id);
    Some(0)
}

pub(super) fn syscall_timer_settime(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let Some(timer_id) = timer_id_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let Some(old_spec) = state.guest_posix_timer_current_spec(timer_id, tick) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let flags = u64::from(request.argument(1) as u32);
    if flags & !RISCV_LINUX_TIMER_ABSTIME != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    let guest_memory_reader = guest_memory_reader?;
    let new_spec = match read_timer_spec(guest_memory_reader, request.argument(2)) {
        Ok(spec) => spec,
        Err(error) => return Some(linux_error(error)),
    };
    if request.argument(3) != 0 {
        let guest_memory_writer = guest_memory_writer?;
        if !guest_memory_writer.write(request.argument(3), &old_spec.encode()) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }
    if state.arm_guest_posix_timer(timer_id, new_spec, flags, tick) {
        Some(0)
    } else {
        Some(linux_error(RISCV_LINUX_EINVAL))
    }
}

pub(super) fn syscall_timer_gettime(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    tick: Tick,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let Some(timer_id) = timer_id_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let Some(spec) = state.guest_posix_timer_current_spec(timer_id, tick) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let guest_memory_writer = guest_memory_writer?;
    if guest_memory_writer.write(request.argument(1), &spec.encode()) {
        Some(0)
    } else {
        Some(linux_error(RISCV_LINUX_EFAULT))
    }
}

pub(super) fn syscall_timer_getoverrun(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
) -> u64 {
    match timer_id_argument(request.argument(0))
        .and_then(|timer_id| state.guest_posix_timers.get(&timer_id))
    {
        Some(_) => 0,
        None => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn syscall_timer_delete(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    match timer_id_argument(request.argument(0)) {
        Some(timer_id) if state.delete_guest_posix_timer(timer_id) => 0,
        _ => linux_error(RISCV_LINUX_EINVAL),
    }
}

fn validate_sigevent(
    address: u64,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<Result<(), u64>> {
    if address == 0 {
        return Some(Ok(()));
    }
    let guest_memory_reader = guest_memory_reader?;
    let bytes = guest_memory_reader
        .read(address, RISCV_LINUX_SIGEVENT_BYTES)
        .filter(|bytes| bytes.len() == RISCV_LINUX_SIGEVENT_BYTES)
        .ok_or(RISCV_LINUX_EFAULT);
    Some(bytes.and_then(|bytes| {
        let notify = u32::from_le_bytes(
            bytes[RISCV_LINUX_SIGEVENT_NOTIFY_OFFSET..RISCV_LINUX_SIGEVENT_NOTIFY_OFFSET + 4]
                .try_into()
                .unwrap(),
        );
        match notify {
            RISCV_LINUX_SIGEV_NONE => Ok(()),
            RISCV_LINUX_SIGEV_SIGNAL | RISCV_LINUX_SIGEV_THREAD | RISCV_LINUX_SIGEV_THREAD_ID => {
                Err(RISCV_LINUX_ENOTSUP)
            }
            _ => Err(RISCV_LINUX_EINVAL),
        }
    }))
}

fn valid_posix_timer_clock_id(clock_id: u64) -> bool {
    matches!(
        clock_id,
        RISCV_LINUX_CLOCK_REALTIME
            | RISCV_LINUX_CLOCK_MONOTONIC
            | RISCV_LINUX_CLOCK_BOOTTIME
            | RISCV_LINUX_CLOCK_TAI
    )
}

fn timer_id_argument(argument: u64) -> Option<u32> {
    u32::try_from(argument).ok()
}

fn read_timer_spec(
    guest_memory_reader: &RiscvGuestMemoryReader,
    address: u64,
) -> Result<RiscvLinuxPosixTimerSpec, u64> {
    let bytes = guest_memory_reader
        .read(address, RISCV_LINUX_ITIMERSPEC_BYTES)
        .filter(|bytes| bytes.len() == RISCV_LINUX_ITIMERSPEC_BYTES)
        .ok_or(RISCV_LINUX_EFAULT)?;
    let interval = decode_timespec(&bytes[0..16])?;
    let value = decode_timespec(&bytes[16..32])?;
    if value.is_zero() {
        Ok(RiscvLinuxPosixTimerSpec::disarmed(interval))
    } else {
        Ok(RiscvLinuxPosixTimerSpec { interval, value })
    }
}

fn decode_timespec(bytes: &[u8]) -> Result<RiscvLinuxPosixTimerTimespec, u64> {
    let seconds = i64::from_le_bytes(bytes[0..8].try_into().expect("timespec seconds width"));
    let nanoseconds =
        i64::from_le_bytes(bytes[8..16].try_into().expect("timespec nanoseconds width"));
    if seconds < 0 || !(0..1_000_000_000).contains(&nanoseconds) {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(RiscvLinuxPosixTimerTimespec {
        seconds: seconds as u64,
        nanoseconds: nanoseconds as u64,
    })
}

fn encode_timespec(timespec: RiscvLinuxPosixTimerTimespec, bytes: &mut [u8]) {
    bytes[0..8].copy_from_slice(&timespec.seconds.to_le_bytes());
    bytes[8..16].copy_from_slice(&timespec.nanoseconds.to_le_bytes());
}
