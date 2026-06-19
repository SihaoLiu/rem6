use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter,
    RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_RDONLY,
    RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_PPOLL: u64 = 73;
pub(super) const RISCV_LINUX_PSELECT6: u64 = 72;

const RISCV_LINUX_FDSET_WORD_BITS: u64 = 64;
const RISCV_LINUX_FDSET_WORD_BYTES: u64 = 8;
const RISCV_LINUX_TIMESPEC_BYTES: usize = 16;
const RISCV_LINUX_PSELECT6_SIGMASK_BYTES: usize = 16;
const RISCV_LINUX_SIGSET_BYTES: usize = 8;
const RISCV_LINUX_POLLFD_BYTES: usize = 8;
const RISCV_LINUX_POLLFD_MAX: u64 = 1024;
const RISCV_LINUX_POLLIN: i16 = 0x0001;
const RISCV_LINUX_POLLOUT: i16 = 0x0004;
const RISCV_LINUX_POLLNVAL: i16 = 0x0020;
const RISCV_LINUX_POLLRDNORM: i16 = 0x0040;
const RISCV_LINUX_POLLWRNORM: i16 = 0x0100;
pub(super) const RISCV_LINUX_READ_READY_EVENTS: u32 =
    RISCV_LINUX_POLLIN as u32 | RISCV_LINUX_POLLRDNORM as u32;
pub(super) const RISCV_LINUX_WRITE_READY_EVENTS: u32 =
    RISCV_LINUX_POLLOUT as u32 | RISCV_LINUX_POLLWRNORM as u32;

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvSelectFdSet {
    address: u64,
    bits: Vec<u8>,
    ready_bits: Vec<u8>,
}

impl RiscvSelectFdSet {
    fn new(address: u64, bits: Vec<u8>) -> Self {
        let ready_bits = vec![0; bits.len()];
        Self {
            address,
            bits,
            ready_bits,
        }
    }

    fn contains(&self, fd: u64) -> bool {
        fd_set_contains(&self.bits, fd)
    }

    fn set_ready(&mut self, fd: u64) {
        set_fd_bit(&mut self.ready_bits, fd);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PselectTimeout {
    Indefinite,
    Expired,
    Pending,
}

impl PselectTimeout {
    const fn waits_when_unready(self) -> bool {
        matches!(self, Self::Indefinite | Self::Pending)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PselectInput<T> {
    MissingGuestReader,
    Errno(u64),
    Value(T),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PpollTimeout {
    address: Option<u64>,
    state: PselectTimeout,
    original_bytes: [u8; RISCV_LINUX_TIMESPEC_BYTES],
}

impl PpollTimeout {
    const fn indefinite() -> Self {
        Self {
            address: None,
            state: PselectTimeout::Indefinite,
            original_bytes: [0; RISCV_LINUX_TIMESPEC_BYTES],
        }
    }

    fn finite(address: u64, state: PselectTimeout, original_bytes: Vec<u8>) -> Self {
        Self {
            address: Some(address),
            state,
            original_bytes: original_bytes
                .try_into()
                .expect("timespec reader returns fixed-width bytes"),
        }
    }

    const fn blocks_when_unready(&self) -> bool {
        matches!(self.state, PselectTimeout::Indefinite)
    }

    const fn expires_when_unready(&self) -> bool {
        matches!(
            self.state,
            PselectTimeout::Expired | PselectTimeout::Pending
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PpollReadySet {
    entries: Vec<(u64, i16)>,
    ready_count: u64,
}

impl PpollReadySet {
    fn write_revents(&self, guest_memory_writer: &RiscvGuestMemoryWriter) -> Result<(), u64> {
        for (pollfd_address, revents) in &self.entries {
            let Some(revents_address) = pollfd_address.checked_add(6) else {
                return Err(RISCV_LINUX_EFAULT);
            };
            if !guest_memory_writer.write(revents_address, &revents.to_le_bytes()) {
                return Err(RISCV_LINUX_EFAULT);
            }
        }
        Ok(())
    }
}

pub(super) fn syscall_ppoll(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    let pollfd_count = request.argument(1);
    if pollfd_count > RISCV_LINUX_POLLFD_MAX {
        return Some(ppoll_return(linux_error(RISCV_LINUX_EINVAL)));
    }
    let timeout = match ppoll_timeout(request.argument(2), guest_memory_reader) {
        PselectInput::Value(timeout) => timeout,
        PselectInput::Errno(errno) => return Some(ppoll_return(linux_error(errno))),
        PselectInput::MissingGuestReader => return None,
    };
    match validate_ppoll_sigmask(
        request.argument(3),
        request.argument(4),
        guest_memory_reader,
    ) {
        PselectInput::Value(()) => {}
        PselectInput::Errno(errno) => return Some(ppoll_return(linux_error(errno))),
        PselectInput::MissingGuestReader => return None,
    }
    if pollfd_count == 0 {
        return if timeout.blocks_when_unready() {
            Some(RiscvSyscallOutcome::Blocked)
        } else {
            ppoll_return_with_timeout(
                0,
                &timeout,
                guest_memory_writer,
                timeout.expires_when_unready(),
            )
        };
    }

    let ready_set = match ppoll_ready_set(
        request.argument(0),
        pollfd_count,
        state,
        guest_memory_reader?,
    ) {
        Ok(ready_set) => ready_set,
        Err(errno) => return Some(ppoll_return(linux_error(errno))),
    };
    if ready_set.ready_count == 0 && timeout.blocks_when_unready() {
        Some(RiscvSyscallOutcome::Blocked)
    } else {
        let guest_memory_writer = guest_memory_writer?;
        if let Err(errno) = ready_set.write_revents(guest_memory_writer) {
            return Some(ppoll_return(linux_error(errno)));
        }
        ppoll_return_with_timeout(
            ready_set.ready_count,
            &timeout,
            Some(guest_memory_writer),
            ready_set.ready_count == 0 && timeout.expires_when_unready(),
        )
    }
}

fn ppoll_return(value: u64) -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return { value }
}

fn ppoll_return_with_timeout(
    value: u64,
    timeout: &PpollTimeout,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
    timeout_expired: bool,
) -> Option<RiscvSyscallOutcome> {
    if let Some(timeout_address) = timeout.address {
        let guest_memory_writer = guest_memory_writer?;
        let zero_timeout = [0_u8; RISCV_LINUX_TIMESPEC_BYTES];
        let remaining = if timeout_expired {
            &zero_timeout[..]
        } else {
            &timeout.original_bytes[..]
        };
        if !guest_memory_writer.write(timeout_address, remaining) {
            return Some(ppoll_return(linux_error(RISCV_LINUX_EFAULT)));
        }
    }
    Some(ppoll_return(value))
}

fn ppoll_ready_set(
    pollfds_address: u64,
    pollfd_count: u64,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
) -> Result<PpollReadySet, u64> {
    let mut entries = Vec::with_capacity(pollfd_count as usize);
    let mut ready_count = 0_u64;
    for index in 0..pollfd_count {
        let Some(pollfd_address) =
            pollfds_address.checked_add(index * RISCV_LINUX_POLLFD_BYTES as u64)
        else {
            return Err(RISCV_LINUX_EFAULT);
        };
        let Some(pollfd) = read_guest_exact(
            guest_memory_reader,
            pollfd_address,
            RISCV_LINUX_POLLFD_BYTES,
        ) else {
            return Err(RISCV_LINUX_EFAULT);
        };
        let revents = pollfd_revents(state, pollfd_fd(&pollfd), pollfd_events(&pollfd));
        if revents != 0 {
            ready_count += 1;
        }
        entries.push((pollfd_address, revents));
    }

    Ok(PpollReadySet {
        entries,
        ready_count,
    })
}

pub(super) fn syscall_pselect6(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    let timeout = match pselect_timeout(request.argument(4), guest_memory_reader) {
        PselectInput::Value(timeout) => timeout,
        PselectInput::Errno(errno) => return Some(pselect_return(linux_error(errno))),
        PselectInput::MissingGuestReader => return None,
    };
    match validate_pselect_sigmask(request.argument(5), guest_memory_reader) {
        PselectInput::Value(()) => {}
        PselectInput::Errno(errno) => return Some(pselect_return(linux_error(errno))),
        PselectInput::MissingGuestReader => return None,
    }
    let nfds = match pselect_nfds(request.argument(0)) {
        Ok(nfds) => nfds,
        Err(errno) => return Some(pselect_return(linux_error(errno))),
    };
    if nfds == 0 {
        return if timeout.waits_when_unready() {
            Some(RiscvSyscallOutcome::Blocked)
        } else {
            Some(pselect_return(0))
        };
    }
    if request.argument(1) == 0 && request.argument(2) == 0 && request.argument(3) == 0 {
        return if timeout.waits_when_unready() {
            Some(RiscvSyscallOutcome::Blocked)
        } else {
            Some(pselect_return(0))
        };
    }

    let fdset_bytes = match pselect_fdset_bytes(nfds) {
        Some(bytes) => bytes,
        None => return Some(pselect_return(linux_error(RISCV_LINUX_EINVAL))),
    };
    let guest_memory_reader = guest_memory_reader?;
    let guest_memory_writer = guest_memory_writer?;
    let mut readfds =
        match read_select_fd_set(request.argument(1), fdset_bytes, guest_memory_reader) {
            Ok(fdset) => fdset,
            Err(errno) => return Some(pselect_return(linux_error(errno))),
        };
    let mut writefds =
        match read_select_fd_set(request.argument(2), fdset_bytes, guest_memory_reader) {
            Ok(fdset) => fdset,
            Err(errno) => return Some(pselect_return(linux_error(errno))),
        };
    let mut exceptfds =
        match read_select_fd_set(request.argument(3), fdset_bytes, guest_memory_reader) {
            Ok(fdset) => fdset,
            Err(errno) => return Some(pselect_return(linux_error(errno))),
        };

    let ready_count = match mark_ready_select_fds(
        state,
        nfds,
        readfds.as_mut(),
        writefds.as_mut(),
        exceptfds.as_mut(),
    ) {
        Ok(count) => count,
        Err(errno) => return Some(pselect_return(linux_error(errno))),
    };
    if ready_count == 0 && timeout.waits_when_unready() {
        return Some(RiscvSyscallOutcome::Blocked);
    }

    let fdsets = [readfds.as_ref(), writefds.as_ref(), exceptfds.as_ref()];
    if let Err(errno) = validate_select_fd_set_outputs(fdsets, guest_memory_writer) {
        return Some(pselect_return(linux_error(errno)));
    }
    for fdset in fdsets.into_iter().flatten() {
        if !guest_memory_writer.write(fdset.address, &fdset.ready_bits) {
            return Some(pselect_return(linux_error(RISCV_LINUX_EFAULT)));
        }
    }
    Some(pselect_return(ready_count))
}

fn pselect_return(value: u64) -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return { value }
}

fn read_select_fd_set(
    address: u64,
    bytes: usize,
    guest_memory_reader: &RiscvGuestMemoryReader,
) -> Result<Option<RiscvSelectFdSet>, u64> {
    if address == 0 {
        return Ok(None);
    }
    let Some(bits) = read_select_fd_set_bits(guest_memory_reader, address, bytes) else {
        return Err(RISCV_LINUX_EFAULT);
    };
    Ok(Some(RiscvSelectFdSet::new(address, bits)))
}

fn read_select_fd_set_bits(
    guest_memory_reader: &RiscvGuestMemoryReader,
    address: u64,
    bytes: usize,
) -> Option<Vec<u8>> {
    let mut bits = Vec::with_capacity(bytes);
    for offset in (0..bytes).step_by(RISCV_LINUX_FDSET_WORD_BYTES as usize) {
        let word_address = address.checked_add(offset as u64)?;
        let word = read_guest_exact(
            guest_memory_reader,
            word_address,
            RISCV_LINUX_FDSET_WORD_BYTES as usize,
        )?;
        bits.extend_from_slice(&word);
    }
    Some(bits)
}

fn validate_select_fd_set_outputs(
    fdsets: [Option<&RiscvSelectFdSet>; 3],
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> Result<(), u64> {
    for fdset in fdsets.into_iter().flatten() {
        if !guest_memory_writer.can_write(fdset.address, fdset.ready_bits.len()) {
            return Err(RISCV_LINUX_EFAULT);
        }
    }
    Ok(())
}

fn pselect_nfds(raw: u64) -> Result<u64, u64> {
    let nfds = i32::try_from(raw).map_err(|_| RISCV_LINUX_EINVAL)?;
    if nfds < 0 {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(nfds as u64)
}

fn mark_ready_select_fds(
    state: &RiscvSyscallState,
    nfds: u64,
    mut readfds: Option<&mut RiscvSelectFdSet>,
    mut writefds: Option<&mut RiscvSelectFdSet>,
    exceptfds: Option<&mut RiscvSelectFdSet>,
) -> Result<u64, u64> {
    let mut ready_count = 0;
    for raw_fd in 0..nfds {
        let read_requested = readfds.as_ref().is_some_and(|fdset| fdset.contains(raw_fd));
        let write_requested = writefds
            .as_ref()
            .is_some_and(|fdset| fdset.contains(raw_fd));
        let except_requested = exceptfds
            .as_ref()
            .is_some_and(|fdset| fdset.contains(raw_fd));
        if !(read_requested || write_requested || except_requested) {
            continue;
        }

        let Some(fd) = guest_fd_argument(raw_fd) else {
            return Err(RISCV_LINUX_EBADF);
        };
        if read_requested {
            let ready = ready_events_for_guest_fd(state, fd, RISCV_LINUX_READ_READY_EVENTS)
                .map_err(|_| RISCV_LINUX_EBADF)?;
            if ready != 0 {
                readfds
                    .as_mut()
                    .expect("read fd set was requested")
                    .set_ready(raw_fd);
                ready_count += 1;
            }
        }
        if write_requested {
            let ready = ready_events_for_guest_fd(state, fd, RISCV_LINUX_WRITE_READY_EVENTS)
                .map_err(|_| RISCV_LINUX_EBADF)?;
            if ready != 0 {
                writefds
                    .as_mut()
                    .expect("write fd set was requested")
                    .set_ready(raw_fd);
                ready_count += 1;
            }
        }
        if except_requested {
            ready_events_for_guest_fd(state, fd, 0).map_err(|_| RISCV_LINUX_EBADF)?;
        }
    }
    Ok(ready_count)
}

fn pselect_fdset_bytes(nfds: u64) -> Option<usize> {
    let words = nfds.checked_add(RISCV_LINUX_FDSET_WORD_BITS - 1)? / RISCV_LINUX_FDSET_WORD_BITS;
    words
        .checked_mul(RISCV_LINUX_FDSET_WORD_BYTES)
        .and_then(|bytes| usize::try_from(bytes).ok())
}

fn pselect_timeout(
    timeout_address: u64,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> PselectInput<PselectTimeout> {
    if timeout_address == 0 {
        return PselectInput::Value(PselectTimeout::Indefinite);
    }
    let Some(guest_memory) = guest_memory else {
        return PselectInput::MissingGuestReader;
    };
    let Some(timeout) = read_guest_exact(guest_memory, timeout_address, RISCV_LINUX_TIMESPEC_BYTES)
    else {
        return PselectInput::Errno(RISCV_LINUX_EFAULT);
    };
    let seconds = read_i64_le(&timeout[..8]);
    let nanoseconds = read_i64_le(&timeout[8..]);
    if seconds < 0 || !(0..1_000_000_000).contains(&nanoseconds) {
        return PselectInput::Errno(RISCV_LINUX_EINVAL);
    }
    if seconds == 0 && nanoseconds == 0 {
        PselectInput::Value(PselectTimeout::Expired)
    } else {
        PselectInput::Value(PselectTimeout::Pending)
    }
}

fn ppoll_timeout(
    timeout_address: u64,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> PselectInput<PpollTimeout> {
    if timeout_address == 0 {
        return PselectInput::Value(PpollTimeout::indefinite());
    }
    let Some(guest_memory) = guest_memory else {
        return PselectInput::MissingGuestReader;
    };
    let Some(timeout) = read_guest_exact(guest_memory, timeout_address, RISCV_LINUX_TIMESPEC_BYTES)
    else {
        return PselectInput::Errno(RISCV_LINUX_EFAULT);
    };
    let seconds = read_i64_le(&timeout[..8]);
    let nanoseconds = read_i64_le(&timeout[8..]);
    if seconds < 0 || !(0..1_000_000_000).contains(&nanoseconds) {
        return PselectInput::Errno(RISCV_LINUX_EINVAL);
    }
    let state = if seconds == 0 && nanoseconds == 0 {
        PselectTimeout::Expired
    } else {
        PselectTimeout::Pending
    };
    PselectInput::Value(PpollTimeout::finite(timeout_address, state, timeout))
}

fn validate_ppoll_sigmask(
    sigmask_address: u64,
    sigset_bytes: u64,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> PselectInput<()> {
    if sigmask_address == 0 {
        return PselectInput::Value(());
    }
    if sigset_bytes != RISCV_LINUX_SIGSET_BYTES as u64 {
        return PselectInput::Errno(RISCV_LINUX_EINVAL);
    }
    let Some(guest_memory) = guest_memory else {
        return PselectInput::MissingGuestReader;
    };
    if read_guest_exact(guest_memory, sigmask_address, RISCV_LINUX_SIGSET_BYTES).is_none() {
        return PselectInput::Errno(RISCV_LINUX_EFAULT);
    }
    PselectInput::Value(())
}

fn validate_pselect_sigmask(
    sigmask_address: u64,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> PselectInput<()> {
    if sigmask_address == 0 {
        return PselectInput::Value(());
    }
    let Some(guest_memory) = guest_memory else {
        return PselectInput::MissingGuestReader;
    };
    let Some(sigmask) = read_guest_exact(
        guest_memory,
        sigmask_address,
        RISCV_LINUX_PSELECT6_SIGMASK_BYTES,
    ) else {
        return PselectInput::Errno(RISCV_LINUX_EFAULT);
    };
    let sigset_address = read_u64_le(&sigmask[..8]);
    let sigset_bytes = read_u64_le(&sigmask[8..]);
    if sigset_bytes != RISCV_LINUX_SIGSET_BYTES as u64 {
        return PselectInput::Errno(RISCV_LINUX_EINVAL);
    }
    if sigset_address == 0 {
        return PselectInput::Value(());
    }
    if read_guest_exact(guest_memory, sigset_address, RISCV_LINUX_SIGSET_BYTES).is_none() {
        return PselectInput::Errno(RISCV_LINUX_EFAULT);
    }
    PselectInput::Value(())
}

fn fd_set_contains(bits: &[u8], fd: u64) -> bool {
    let Some(byte_index) = usize::try_from(fd / 8).ok() else {
        return false;
    };
    bits.get(byte_index)
        .is_some_and(|byte| byte & (1 << (fd % 8)) != 0)
}

fn set_fd_bit(bits: &mut [u8], fd: u64) {
    let byte_index = usize::try_from(fd / 8).expect("fd set byte index fits usize");
    let bit = (fd % 8) as u8;
    bits[byte_index] |= 1 << bit;
}

fn pollfd_revents(state: &RiscvSyscallState, fd: i32, events: i16) -> i16 {
    let Some(fd) = (fd >= 0).then_some(fd as u64).and_then(guest_fd_argument) else {
        return 0;
    };
    match ready_events_for_guest_fd(state, fd, events as u32) {
        Ok(events) => events as i16,
        Err(_) => RISCV_LINUX_POLLNVAL,
    }
}

pub(super) fn ready_events_for_guest_fd(
    state: &RiscvSyscallState,
    fd: crate::GuestFd,
    events: u32,
) -> Result<u32, crate::GuestFdError> {
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return Err(crate::GuestFdError::BadFd { fd });
    };

    let mut revents = 0_u32;
    let access_mode = u64::from(status_flags.bits()) & RISCV_LINUX_O_ACCMODE;
    match state.guest_eventfd_ready(fd) {
        Ok(Some(ready)) => {
            if ready.readable() {
                revents |= events & RISCV_LINUX_READ_READY_EVENTS;
            }
            if ready.writable() {
                revents |= events & RISCV_LINUX_WRITE_READY_EVENTS;
            }
            return Ok(revents);
        }
        Ok(None) => {}
        Err(error) => return Err(error),
    }
    let mut pipe_endpoint = false;
    match state.guest_pipe_read_ready(fd) {
        Ok(Some(ready)) => {
            pipe_endpoint = true;
            if access_mode != RISCV_LINUX_O_WRONLY && ready {
                revents |= events & RISCV_LINUX_READ_READY_EVENTS;
            }
        }
        Ok(None) => {}
        Err(error) => return Err(error),
    }
    match state.guest_pipe_write_ready(fd) {
        Ok(Some(ready)) => {
            pipe_endpoint = true;
            if access_mode != RISCV_LINUX_O_RDONLY && ready {
                revents |= events & RISCV_LINUX_WRITE_READY_EVENTS;
            }
        }
        Ok(None) => {}
        Err(error) => return Err(error),
    }
    if pipe_endpoint {
        return Ok(revents);
    }
    if access_mode != RISCV_LINUX_O_WRONLY
        && (!state.stdin_readable(fd) || state.stdin_byte_count() > 0)
    {
        revents |= events & RISCV_LINUX_READ_READY_EVENTS;
    }
    if access_mode != RISCV_LINUX_O_RDONLY {
        revents |= events & RISCV_LINUX_WRITE_READY_EVENTS;
    }
    Ok(revents)
}

fn read_guest_exact(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
    len: usize,
) -> Option<Vec<u8>> {
    if len == 0 {
        return Some(Vec::new());
    }
    guest_memory
        .read(address, len)
        .filter(|bytes| bytes.len() == len)
}

fn pollfd_fd(bytes: &[u8]) -> i32 {
    let mut raw = [0; 4];
    raw.copy_from_slice(&bytes[..4]);
    i32::from_le_bytes(raw)
}

fn pollfd_events(bytes: &[u8]) -> i16 {
    let mut raw = [0; 2];
    raw.copy_from_slice(&bytes[4..6]);
    i16::from_le_bytes(raw)
}

fn read_u64_le(bytes: &[u8]) -> u64 {
    let mut raw = [0; 8];
    raw.copy_from_slice(bytes);
    u64::from_le_bytes(raw)
}

fn read_i64_le(bytes: &[u8]) -> i64 {
    let mut raw = [0; 8];
    raw.copy_from_slice(bytes);
    i64::from_le_bytes(raw)
}
