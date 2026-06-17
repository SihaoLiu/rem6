use std::collections::VecDeque;

use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvGuestPipeEndpoint, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EMFILE,
    RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_PIPE2: u64 = 59;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct RiscvGuestPipeId(u64);

impl RiscvGuestPipeId {
    const fn new(value: u64) -> Self {
        Self(value)
    }
}

const RISCV_LINUX_PIPE2_ALLOWED_FLAGS: u64 = RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;

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
        let Some(buffer) = self.guest_pipe_buffers.get(&endpoint.pipe) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(buffer.iter().take(count).copied().collect()))
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
        let Some(buffer) = self.guest_pipe_buffers.get(&endpoint.pipe) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(buffer.len()))
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
        let buffer = self
            .guest_pipe_buffers
            .get_mut(&endpoint.pipe)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        for _ in 0..count.min(buffer.len()) {
            buffer.pop_front();
        }
        Ok(())
    }

    pub(super) fn write_guest_pipe_from_fd(
        &mut self,
        fd: GuestFd,
        bytes: &[u8],
    ) -> Result<bool, GuestFdError> {
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
            return Ok(false);
        };
        let buffer = self
            .guest_pipe_buffers
            .get_mut(&endpoint.pipe)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        buffer.extend(bytes.iter().copied());
        Ok(true)
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
        self.guest_pipe_buffers.insert(pipe, VecDeque::new());
        self.guest_pipe_read_descriptions
            .insert(read_description, RiscvGuestPipeEndpoint { pipe });
        self.guest_pipe_write_descriptions
            .insert(write_description, RiscvGuestPipeEndpoint { pipe });
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
                self.guest_pipe_buffers.remove(&endpoint.pipe);
            }
        }
    }

    fn next_guest_pipe_id(&self) -> Result<RiscvGuestPipeId, GuestFdError> {
        let mut candidate = 0_u64;
        loop {
            let pipe = RiscvGuestPipeId::new(candidate);
            if !self.guest_pipe_buffers.contains_key(&pipe) {
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
