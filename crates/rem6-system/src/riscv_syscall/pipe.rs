use std::collections::VecDeque;

use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EMFILE, RISCV_LINUX_O_CLOEXEC,
    RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_PIPE2: u64 = 59;
pub(super) const RISCV_LINUX_PIPE_PAGE_BYTES: usize = 4096;
pub(super) const RISCV_LINUX_DEFAULT_PIPE_CAPACITY_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct RiscvGuestPipeId(u64);

impl RiscvGuestPipeId {
    const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct RiscvGuestPipeEndpoint {
    pipe: RiscvGuestPipeId,
}

impl RiscvGuestPipeEndpoint {
    const fn new(pipe: RiscvGuestPipeId) -> Self {
        Self { pipe }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestPipe {
    buffer: VecDeque<u8>,
    capacity: usize,
}

impl RiscvGuestPipe {
    const fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            capacity,
        }
    }
}

const RISCV_LINUX_PIPE2_ALLOWED_FLAGS: u64 = RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;
const RISCV_LINUX_PIPE_BUF_BYTES: usize = RISCV_LINUX_PIPE_PAGE_BYTES;
const RISCV_LINUX_PIPE_MAX_CAPACITY_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestPipeWrite {
    NotPipe,
    Written(usize),
    WouldBlock,
    Blocked,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestPipeCapacityError {
    Fd(GuestFdError),
    Busy,
    Permission,
    Invalid,
}

impl From<GuestFdError> for RiscvGuestPipeCapacityError {
    fn from(error: GuestFdError) -> Self {
        Self::Fd(error)
    }
}

impl RiscvSyscallState {
    pub(super) fn guest_pipe_prefix(
        &self,
        fd: GuestFd,
        count: usize,
    ) -> Result<Option<Vec<u8>>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_pipe_read_descriptions.get(&description) else {
            return Ok(None);
        };
        let Some(pipe) = self.guest_pipes.get(&endpoint.pipe) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(pipe.buffer.iter().take(count).copied().collect()))
    }

    pub(super) fn guest_pipe_unread_byte_count(
        &self,
        fd: GuestFd,
    ) -> Result<Option<usize>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let endpoint = self
            .guest_pipe_read_descriptions
            .get(&description)
            .or_else(|| self.guest_pipe_write_descriptions.get(&description));
        let Some(endpoint) = endpoint else {
            return Ok(None);
        };
        let Some(pipe) = self.guest_pipes.get(&endpoint.pipe) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(pipe.buffer.len()))
    }

    pub(super) fn guest_pipe_capacity(&self, fd: GuestFd) -> Result<Option<usize>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let endpoint = self
            .guest_pipe_read_descriptions
            .get(&description)
            .or_else(|| self.guest_pipe_write_descriptions.get(&description));
        let Some(endpoint) = endpoint else {
            return Ok(None);
        };
        let Some(pipe) = self.guest_pipes.get(&endpoint.pipe) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(pipe.capacity))
    }

    pub(super) fn guest_fd_is_pipe(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self.guest_pipe_read_descriptions.contains_key(&description)
            || self
                .guest_pipe_write_descriptions
                .contains_key(&description))
    }

    pub(super) fn guest_pipe_read_ready(&self, fd: GuestFd) -> Result<Option<bool>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_pipe_read_descriptions.get(&description) else {
            return Ok(None);
        };
        let Some(pipe) = self.guest_pipes.get(&endpoint.pipe) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(!pipe.buffer.is_empty()))
    }

    pub(super) fn guest_pipe_write_ready(&self, fd: GuestFd) -> Result<Option<bool>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_pipe_write_descriptions.get(&description) else {
            return Ok(None);
        };
        let Some(pipe) = self.guest_pipes.get(&endpoint.pipe) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(
            pipe.capacity.saturating_sub(pipe.buffer.len()) >= RISCV_LINUX_PIPE_BUF_BYTES,
        ))
    }

    pub(super) fn set_guest_pipe_capacity(
        &mut self,
        fd: GuestFd,
        requested: u64,
    ) -> Result<Option<usize>, RiscvGuestPipeCapacityError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let endpoint = self
            .guest_pipe_read_descriptions
            .get(&description)
            .or_else(|| self.guest_pipe_write_descriptions.get(&description))
            .copied();
        let Some(endpoint) = endpoint else {
            return Ok(None);
        };
        let requested =
            usize::try_from(requested).map_err(|_| RiscvGuestPipeCapacityError::Invalid)?;
        let capacity = rounded_pipe_capacity(requested)?;
        if capacity > RISCV_LINUX_PIPE_MAX_CAPACITY_BYTES {
            return Err(RiscvGuestPipeCapacityError::Permission);
        }
        let Some(pipe) = self.guest_pipes.get_mut(&endpoint.pipe) else {
            return Err(GuestFdError::MissingFileDescription { description }.into());
        };
        if capacity < pipe.buffer.len() {
            return Err(RiscvGuestPipeCapacityError::Busy);
        }
        pipe.capacity = capacity;
        Ok(Some(capacity))
    }

    pub(super) fn consume_guest_pipe_prefix(
        &mut self,
        fd: GuestFd,
        count: usize,
    ) -> Result<(), GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let endpoint = self
            .guest_pipe_read_descriptions
            .get(&description)
            .copied()
            .ok_or(GuestFdError::BadFd { fd })?;
        let pipe = self
            .guest_pipes
            .get_mut(&endpoint.pipe)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        for _ in 0..count.min(pipe.buffer.len()) {
            pipe.buffer.pop_front();
        }
        Ok(())
    }

    pub(super) fn guest_pipe_write_plan(
        &self,
        fd: GuestFd,
        byte_count: usize,
    ) -> Result<RiscvGuestPipeWrite, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self
            .guest_pipe_write_descriptions
            .get(&description)
            .copied()
        else {
            return Ok(RiscvGuestPipeWrite::NotPipe);
        };
        let status_flags = self.guest_fds.status_flags(fd)?;
        let nonblocking = status_flags.bits() & RISCV_LINUX_O_NONBLOCK as u32 != 0;
        let pipe = self
            .guest_pipes
            .get(&endpoint.pipe)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        let available = pipe.capacity.saturating_sub(pipe.buffer.len());
        if available == 0 {
            return Ok(if nonblocking {
                RiscvGuestPipeWrite::WouldBlock
            } else {
                RiscvGuestPipeWrite::Blocked
            });
        }
        if byte_count <= RISCV_LINUX_PIPE_BUF_BYTES && available < byte_count {
            return Ok(if nonblocking {
                RiscvGuestPipeWrite::WouldBlock
            } else {
                RiscvGuestPipeWrite::Blocked
            });
        }
        if !nonblocking && available < byte_count {
            return Ok(RiscvGuestPipeWrite::Blocked);
        }
        Ok(RiscvGuestPipeWrite::Written(available.min(byte_count)))
    }

    pub(super) fn write_guest_pipe_from_fd(
        &mut self,
        fd: GuestFd,
        bytes: &[u8],
    ) -> Result<RiscvGuestPipeWrite, GuestFdError> {
        let write = self.guest_pipe_write_plan(fd, bytes.len())?;
        let RiscvGuestPipeWrite::Written(written) = write else {
            return Ok(write);
        };
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let endpoint = self
            .guest_pipe_write_descriptions
            .get(&description)
            .copied()
            .expect("pipe write plan found write endpoint");
        let pipe = self
            .guest_pipes
            .get_mut(&endpoint.pipe)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        pipe.buffer.extend(bytes.iter().take(written).copied());
        Ok(RiscvGuestPipeWrite::Written(written))
    }

    fn open_guest_pipe(
        &mut self,
        read_fd: GuestFd,
        write_fd: GuestFd,
        read_description: GuestFileDescriptionId,
        write_description: GuestFileDescriptionId,
        flags: u64,
    ) -> Result<(), GuestFdError> {
        let pipe = self.next_guest_pipe_id()?;
        let nonblock = flags & RISCV_LINUX_O_NONBLOCK;
        let close_on_exec = flags & RISCV_LINUX_O_CLOEXEC != 0;

        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                read_description,
                GuestFileStatusFlags::new((RISCV_LINUX_O_RDONLY | nonblock) as u32),
            ))?;
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                write_description,
                GuestFileStatusFlags::new((RISCV_LINUX_O_WRONLY | nonblock) as u32),
            ))?;
        self.guest_fds.insert(
            read_fd,
            GuestFdEntry::new(read_description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_fds.insert(
            write_fd,
            GuestFdEntry::new(write_description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_pipes.insert(
            pipe,
            RiscvGuestPipe::new(RISCV_LINUX_DEFAULT_PIPE_CAPACITY_BYTES),
        );
        self.guest_pipe_read_descriptions
            .insert(read_description, RiscvGuestPipeEndpoint::new(pipe));
        self.guest_pipe_write_descriptions
            .insert(write_description, RiscvGuestPipeEndpoint::new(pipe));
        Ok(())
    }

    pub(super) fn remove_guest_pipe_description(&mut self, description: GuestFileDescriptionId) {
        let endpoint = self
            .guest_pipe_read_descriptions
            .remove(&description)
            .or_else(|| self.guest_pipe_write_descriptions.remove(&description));
        if let Some(endpoint) = endpoint {
            let read_live = self
                .guest_pipe_read_descriptions
                .values()
                .any(|candidate| candidate.pipe == endpoint.pipe);
            let write_live = self
                .guest_pipe_write_descriptions
                .values()
                .any(|candidate| candidate.pipe == endpoint.pipe);
            if !read_live && !write_live {
                self.guest_pipes.remove(&endpoint.pipe);
            }
        }
    }

    fn next_guest_pipe_id(&self) -> Result<RiscvGuestPipeId, GuestFdError> {
        let mut candidate = 0_u64;
        loop {
            let pipe = RiscvGuestPipeId::new(candidate);
            if !self.guest_pipes.contains_key(&pipe) {
                return Ok(pipe);
            }
            candidate = candidate
                .checked_add(1)
                .ok_or(GuestFdError::FdSpaceExhausted)?;
        }
    }

    fn next_pipe_fds(&self) -> Result<(GuestFd, GuestFd), GuestFdError> {
        let read_fd = self.next_guest_fd_excluding(&[])?;
        let write_fd = self.next_guest_fd_excluding(&[read_fd])?;
        Ok((read_fd, write_fd))
    }

    fn next_pipe_descriptions(
        &self,
    ) -> Result<(GuestFileDescriptionId, GuestFileDescriptionId), GuestFdError> {
        let read_description = self.next_guest_file_description_excluding(&[])?;
        let write_description = self.next_guest_file_description_excluding(&[read_description])?;
        Ok((read_description, write_description))
    }
}

fn rounded_pipe_capacity(requested: usize) -> Result<usize, RiscvGuestPipeCapacityError> {
    requested
        .max(RISCV_LINUX_PIPE_PAGE_BYTES)
        .checked_next_power_of_two()
        .ok_or(RiscvGuestPipeCapacityError::Invalid)
}

pub(super) fn syscall_pipe2(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let flags = request.argument(1);
    if flags & !RISCV_LINUX_PIPE2_ALLOWED_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let (read_fd, write_fd) = match state.next_pipe_fds() {
        Ok(fds) => fds,
        Err(_) => return linux_error(RISCV_LINUX_EMFILE),
    };
    let (read_description, write_description) = match state.next_pipe_descriptions() {
        Ok(descriptions) => descriptions,
        Err(_) => return linux_error(RISCV_LINUX_EMFILE),
    };
    let mut fd_bytes = [0_u8; 8];
    fd_bytes[0..4].copy_from_slice(&read_fd.get().to_le_bytes());
    fd_bytes[4..8].copy_from_slice(&write_fd.get().to_le_bytes());
    if !guest_memory.write(request.argument(0), &fd_bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }

    match state.open_guest_pipe(
        read_fd,
        write_fd,
        read_description,
        write_description,
        flags,
    ) {
        Ok(()) => 0,
        Err(_) => linux_error(RISCV_LINUX_EMFILE),
    }
}
