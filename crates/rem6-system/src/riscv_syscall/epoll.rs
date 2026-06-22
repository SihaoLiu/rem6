use std::collections::BTreeMap;

use super::time::read_timespec64;
use super::{
    guest_fd_argument, linux_error, poll::ready_events_for_guest_fd, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EBADF, RISCV_LINUX_EEXIST, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_ENOENT, RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_RDWR,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_EPOLL_CREATE1: u64 = 20;
pub(super) const RISCV_LINUX_EPOLL_CTL: u64 = 21;
pub(super) const RISCV_LINUX_EPOLL_PWAIT: u64 = 22;
pub(super) const RISCV_LINUX_EPOLL_PWAIT2: u64 = 441;

const RISCV_LINUX_EPOLL_EVENT_BYTES: usize = 16;
const RISCV_LINUX_SIGSET_BYTES: u64 = 8;
const RISCV_LINUX_EPOLL_CTL_ADD: u64 = 1;
const RISCV_LINUX_EPOLL_CTL_DEL: u64 = 2;
const RISCV_LINUX_EPOLL_CTL_MOD: u64 = 3;
const RISCV_LINUX_EPOLL_MAX_EVENTS: u64 = i32::MAX as u64 / RISCV_LINUX_EPOLL_EVENT_BYTES as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestEpollEvent {
    events: u32,
    data: u64,
}

impl RiscvGuestEpollEvent {
    const fn new(events: u32, data: u64) -> Self {
        Self { events, data }
    }

    const fn events(self) -> u32 {
        self.events
    }

    const fn data(self) -> u64 {
        self.data
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvGuestEpoll {
    entries: BTreeMap<GuestFileDescriptionId, RiscvGuestEpollEvent>,
}

impl RiscvGuestEpoll {
    fn insert(
        &mut self,
        description: GuestFileDescriptionId,
        event: RiscvGuestEpollEvent,
    ) -> Result<(), RiscvGuestEpollError> {
        if self.entries.contains_key(&description) {
            return Err(RiscvGuestEpollError::AlreadyRegistered);
        }
        self.entries.insert(description, event);
        Ok(())
    }

    fn replace(
        &mut self,
        description: GuestFileDescriptionId,
        event: RiscvGuestEpollEvent,
    ) -> Result<(), RiscvGuestEpollError> {
        let Some(entry) = self.entries.get_mut(&description) else {
            return Err(RiscvGuestEpollError::MissingRegistration);
        };
        *entry = event;
        Ok(())
    }

    fn remove(&mut self, description: GuestFileDescriptionId) -> Result<(), RiscvGuestEpollError> {
        self.entries
            .remove(&description)
            .map(|_| ())
            .ok_or(RiscvGuestEpollError::MissingRegistration)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestEpollError {
    AlreadyRegistered,
    MissingRegistration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestEpollTimeout {
    Block,
    Expire,
}

impl RiscvGuestEpollTimeout {
    const fn blocks_when_unready(self) -> bool {
        matches!(self, Self::Block)
    }
}

impl RiscvSyscallState {
    fn open_guest_epoll(&mut self, flags: u64) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_guest_fd_excluding(&[])?;
        let description = self.next_open_description()?;
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(RISCV_LINUX_O_RDWR as u32),
            ))?;
        self.guest_fds.insert(
            fd,
            GuestFdEntry::new(description).with_close_on_exec(flags & RISCV_LINUX_O_CLOEXEC != 0),
        )?;
        self.guest_epolls
            .insert(description, RiscvGuestEpoll::default());
        Ok(fd)
    }

    fn guest_epoll_description_for_fd(
        &self,
        fd: GuestFd,
    ) -> Result<Option<GuestFileDescriptionId>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self
            .guest_epolls
            .contains_key(&description)
            .then_some(description))
    }

    fn guest_epoll_mut(
        &mut self,
        description: GuestFileDescriptionId,
    ) -> Result<&mut RiscvGuestEpoll, GuestFdError> {
        self.guest_epolls
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })
    }

    fn guest_epoll(
        &self,
        description: GuestFileDescriptionId,
    ) -> Result<&RiscvGuestEpoll, GuestFdError> {
        self.guest_epolls
            .get(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })
    }

    pub(super) fn remove_guest_epoll_description(&mut self, description: GuestFileDescriptionId) {
        self.guest_epolls.remove(&description);
    }

    pub(super) fn remove_guest_epoll_target_description(
        &mut self,
        description: GuestFileDescriptionId,
    ) {
        for epoll in self.guest_epolls.values_mut() {
            epoll.entries.remove(&description);
        }
    }

    fn live_fd_for_description(&self, description: GuestFileDescriptionId) -> Option<GuestFd> {
        self.guest_fds
            .snapshot()
            .entries()
            .iter()
            .find(|entry| entry.entry().description() == description)
            .map(|entry| entry.fd())
    }
}

pub(super) fn syscall_epoll_create1(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let flags = request.argument(0);
    if flags & !RISCV_LINUX_O_CLOEXEC != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.open_guest_epoll(flags) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_) => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn syscall_epoll_ctl(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let Some(epfd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let Some(target_fd) = guest_fd_argument(request.argument(2)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let epoll_description = match state.guest_epoll_description_for_fd(epfd) {
        Ok(Some(description)) => description,
        Ok(None) => return Some(linux_error(RISCV_LINUX_EINVAL)),
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    };
    if state.guest_fds.entry(target_fd).is_none() {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }
    if epfd == target_fd {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    let target_description = state
        .guest_fds
        .entry(target_fd)
        .expect("target fd was checked above")
        .description();

    let operation = request.argument(1);
    match operation {
        RISCV_LINUX_EPOLL_CTL_ADD | RISCV_LINUX_EPOLL_CTL_MOD => {
            let event = match read_epoll_event(guest_memory?, request.argument(3)) {
                Ok(event) => event,
                Err(errno) => return Some(linux_error(errno)),
            };
            Some(
                match match operation {
                    RISCV_LINUX_EPOLL_CTL_ADD => state
                        .guest_epoll_mut(epoll_description)
                        .map(|epoll| epoll.insert(target_description, event)),
                    RISCV_LINUX_EPOLL_CTL_MOD => state
                        .guest_epoll_mut(epoll_description)
                        .map(|epoll| epoll.replace(target_description, event)),
                    _ => unreachable!(),
                } {
                    Ok(Ok(())) => 0,
                    Ok(Err(RiscvGuestEpollError::AlreadyRegistered)) => {
                        linux_error(RISCV_LINUX_EEXIST)
                    }
                    Ok(Err(RiscvGuestEpollError::MissingRegistration)) => {
                        linux_error(RISCV_LINUX_ENOENT)
                    }
                    Err(_) => linux_error(RISCV_LINUX_EBADF),
                },
            )
        }
        RISCV_LINUX_EPOLL_CTL_DEL => Some(
            match state
                .guest_epoll_mut(epoll_description)
                .map(|epoll| epoll.remove(target_description))
            {
                Ok(Ok(())) => 0,
                Ok(Err(RiscvGuestEpollError::MissingRegistration)) => {
                    linux_error(RISCV_LINUX_ENOENT)
                }
                Ok(Err(RiscvGuestEpollError::AlreadyRegistered)) => unreachable!(),
                Err(_) => linux_error(RISCV_LINUX_EBADF),
            },
        ),
        _ => Some(linux_error(RISCV_LINUX_EINVAL)),
    }
}

pub(super) fn syscall_epoll_pwait(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    if let Err(errno) = validate_epoll_sigmask(
        request.argument(4),
        request.argument(5),
        guest_memory_reader,
    )? {
        return Some(epoll_return(linux_error(errno)));
    }
    let timeout = if epoll_timeout_may_block(request.argument(3)) {
        RiscvGuestEpollTimeout::Block
    } else {
        RiscvGuestEpollTimeout::Expire
    };
    syscall_epoll_wait(request, state, timeout, guest_memory)
}

pub(super) fn syscall_epoll_pwait2(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    let timeout = match epoll_pwait2_timeout(request.argument(3), guest_memory_reader)? {
        Ok(timeout) => timeout,
        Err(errno) => return Some(epoll_return(linux_error(errno))),
    };
    if let Err(errno) = validate_epoll_sigmask(
        request.argument(4),
        request.argument(5),
        guest_memory_reader,
    )? {
        return Some(epoll_return(linux_error(errno)));
    }
    syscall_epoll_wait(request, state, timeout, guest_memory_writer)
}

fn syscall_epoll_wait(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    timeout: RiscvGuestEpollTimeout,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    let Some(epfd) = guest_fd_argument(request.argument(0)) else {
        return Some(epoll_return(linux_error(RISCV_LINUX_EBADF)));
    };
    let max_events = request.argument(2);
    if max_events == 0 || max_events > RISCV_LINUX_EPOLL_MAX_EVENTS {
        return Some(epoll_return(linux_error(RISCV_LINUX_EINVAL)));
    }
    let epoll_description = match state.guest_epoll_description_for_fd(epfd) {
        Ok(Some(description)) => description,
        Ok(None) => return Some(epoll_return(linux_error(RISCV_LINUX_EINVAL))),
        Err(_) => return Some(epoll_return(linux_error(RISCV_LINUX_EBADF))),
    };
    let ready = match ready_epoll_events(state, epoll_description, max_events) {
        Ok(events) => events,
        Err(_) => return Some(epoll_return(linux_error(RISCV_LINUX_EBADF))),
    };
    if ready.is_empty() {
        return if timeout.blocks_when_unready() {
            Some(RiscvSyscallOutcome::Blocked)
        } else {
            Some(epoll_return(0))
        };
    }

    let output_bytes = ready
        .len()
        .checked_mul(RISCV_LINUX_EPOLL_EVENT_BYTES)
        .expect("ready event count is bounded by max_events");
    let guest_memory = guest_memory?;
    if !guest_memory.can_write(request.argument(1), output_bytes) {
        return Some(epoll_return(linux_error(RISCV_LINUX_EFAULT)));
    }
    for (index, event) in ready.iter().enumerate() {
        let Some(address) = request
            .argument(1)
            .checked_add((index * RISCV_LINUX_EPOLL_EVENT_BYTES) as u64)
        else {
            return Some(epoll_return(linux_error(RISCV_LINUX_EFAULT)));
        };
        if !guest_memory.write(address, &epoll_event_bytes(*event)) {
            return Some(epoll_return(linux_error(RISCV_LINUX_EFAULT)));
        }
    }
    Some(epoll_return(ready.len() as u64))
}

fn epoll_return(value: u64) -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return { value }
}

fn epoll_pwait2_timeout(
    timeout_address: u64,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<Result<RiscvGuestEpollTimeout, u64>> {
    if timeout_address == 0 {
        return Some(Ok(RiscvGuestEpollTimeout::Block));
    }
    let Some(timespec) = read_timespec64(guest_memory?, timeout_address) else {
        return Some(Err(RISCV_LINUX_EFAULT));
    };
    if !timespec.is_valid() {
        return Some(Err(RISCV_LINUX_EINVAL));
    }
    Some(Ok(RiscvGuestEpollTimeout::Expire))
}

fn validate_epoll_sigmask(
    sigmask_address: u64,
    sigset_bytes: u64,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<Result<(), u64>> {
    if sigmask_address == 0 {
        return Some(Ok(()));
    }
    if sigset_bytes != RISCV_LINUX_SIGSET_BYTES {
        return Some(Err(RISCV_LINUX_EINVAL));
    }
    match guest_memory?.read(sigmask_address, RISCV_LINUX_SIGSET_BYTES as usize) {
        Some(bytes) if bytes.len() == RISCV_LINUX_SIGSET_BYTES as usize => Some(Ok(())),
        _ => Some(Err(RISCV_LINUX_EFAULT)),
    }
}

fn read_epoll_event(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
) -> Result<RiscvGuestEpollEvent, u64> {
    let Some(bytes) = guest_memory.read(address, RISCV_LINUX_EPOLL_EVENT_BYTES) else {
        return Err(RISCV_LINUX_EFAULT);
    };
    if bytes.len() != RISCV_LINUX_EPOLL_EVENT_BYTES {
        return Err(RISCV_LINUX_EFAULT);
    }
    let mut raw_events = [0_u8; 4];
    raw_events.copy_from_slice(&bytes[..4]);
    let mut raw_data = [0_u8; 8];
    raw_data.copy_from_slice(&bytes[8..16]);
    Ok(RiscvGuestEpollEvent::new(
        u32::from_le_bytes(raw_events),
        u64::from_le_bytes(raw_data),
    ))
}

fn epoll_event_bytes(event: RiscvGuestEpollEvent) -> [u8; RISCV_LINUX_EPOLL_EVENT_BYTES] {
    let mut bytes = [0_u8; RISCV_LINUX_EPOLL_EVENT_BYTES];
    bytes[..4].copy_from_slice(&event.events().to_le_bytes());
    bytes[8..].copy_from_slice(&event.data().to_le_bytes());
    bytes
}

fn ready_epoll_events(
    state: &RiscvSyscallState,
    epoll_description: GuestFileDescriptionId,
    max_events: u64,
) -> Result<Vec<RiscvGuestEpollEvent>, GuestFdError> {
    let epoll = state.guest_epoll(epoll_description)?;
    let mut ready = Vec::new();
    for (description, event) in &epoll.entries {
        let Some(fd) = state.live_fd_for_description(*description) else {
            continue;
        };
        let ready_events = ready_events_for_guest_fd(state, fd, event.events())?;
        if ready_events != 0 {
            ready.push(RiscvGuestEpollEvent::new(ready_events, event.data()));
        }
        if ready.len() == max_events as usize {
            break;
        }
    }
    Ok(ready)
}

fn epoll_timeout_may_block(timeout: u64) -> bool {
    (timeout as u32 as i32) < 0
}
