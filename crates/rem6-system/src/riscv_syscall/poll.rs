use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_PPOLL: u64 = 73;

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

pub(super) fn syscall_ppoll(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let pollfd_count = request.argument(1);
    if pollfd_count > RISCV_LINUX_POLLFD_MAX {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if pollfd_count == 0 {
        return Some(0);
    }

    Some(syscall_ppoll_with_memory(
        request.argument(0),
        pollfd_count,
        state,
        guest_memory_reader?,
        guest_memory_writer?,
    ))
}

fn syscall_ppoll_with_memory(
    pollfds_address: u64,
    pollfd_count: u64,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let mut entries = Vec::with_capacity(pollfd_count as usize);
    for index in 0..pollfd_count {
        let Some(pollfd_address) =
            pollfds_address.checked_add(index * RISCV_LINUX_POLLFD_BYTES as u64)
        else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        let Some(pollfd) = read_guest_exact(
            guest_memory_reader,
            pollfd_address,
            RISCV_LINUX_POLLFD_BYTES,
        ) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        let revents = pollfd_revents(state, pollfd_fd(&pollfd), pollfd_events(&pollfd));
        entries.push((pollfd_address, revents));
    }

    let mut ready_count = 0_u64;
    for (pollfd_address, revents) in entries {
        let Some(revents_address) = pollfd_address.checked_add(6) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        if !guest_memory_writer.write(revents_address, &revents.to_le_bytes()) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
        if revents != 0 {
            ready_count += 1;
        }
    }
    ready_count
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
