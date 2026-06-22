use std::collections::VecDeque;

use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EAFNOSUPPORT, RISCV_LINUX_EAGAIN,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EMFILE,
    RISCV_LINUX_ENOTSOCK, RISCV_LINUX_ENOTSUP, RISCV_LINUX_EPIPE, RISCV_LINUX_EPROTONOSUPPORT,
    RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDONLY,
    RISCV_LINUX_O_RDWR, RISCV_LINUX_O_WRONLY,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_SOCKETPAIR: u64 = 199;
pub(super) const RISCV_LINUX_SENDTO: u64 = 206;
pub(super) const RISCV_LINUX_RECVFROM: u64 = 207;

const RISCV_LINUX_AF_UNIX: u64 = 1;
const RISCV_LINUX_SOCK_STREAM: u64 = 1;
const RISCV_LINUX_SOCK_TYPE_MASK: u64 = 0xf;
const RISCV_LINUX_SOCKETPAIR_ALLOWED_FLAGS: u64 =
    RISCV_LINUX_SOCK_STREAM | RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;
const RISCV_LINUX_MSG_DONTWAIT: u64 = 0x40;
const RISCV_LINUX_MSG_NOSIGNAL: u64 = 0x4000;
const RISCV_LINUX_SENDTO_ALLOWED_FLAGS: u64 = RISCV_LINUX_MSG_DONTWAIT | RISCV_LINUX_MSG_NOSIGNAL;
const RISCV_LINUX_RECVFROM_ALLOWED_FLAGS: u64 = RISCV_LINUX_MSG_DONTWAIT;
const RISCV_LINUX_DEFAULT_SOCKET_CAPACITY_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct RiscvGuestSocketQueueId(u64);

impl RiscvGuestSocketQueueId {
    const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestSocketEndpoint {
    read_queue: RiscvGuestSocketQueueId,
    write_queue: RiscvGuestSocketQueueId,
}

impl RiscvGuestSocketEndpoint {
    const fn new(
        read_queue: RiscvGuestSocketQueueId,
        write_queue: RiscvGuestSocketQueueId,
    ) -> Self {
        Self {
            read_queue,
            write_queue,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestSocketQueue {
    buffer: VecDeque<u8>,
    capacity: usize,
}

impl RiscvGuestSocketQueue {
    const fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::new(),
            capacity,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestSocketRead {
    NotSocket,
    Bytes(Vec<u8>),
    WouldBlock,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestSocketWrite {
    NotSocket,
    Written(usize),
    WouldBlock,
    Blocked,
    BrokenPipe,
}

impl RiscvSyscallState {
    pub(super) fn guest_socket_read(
        &self,
        fd: GuestFd,
        count: usize,
    ) -> Result<RiscvGuestSocketRead, GuestFdError> {
        self.guest_socket_read_with_nonblocking(fd, count, false)
    }

    fn guest_socket_read_with_nonblocking(
        &self,
        fd: GuestFd,
        count: usize,
        force_nonblocking: bool,
    ) -> Result<RiscvGuestSocketRead, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).copied() else {
            return Ok(RiscvGuestSocketRead::NotSocket);
        };
        let Some(queue) = self.guest_socket_queues.get(&endpoint.read_queue) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        let bytes = queue.buffer.iter().take(count).copied().collect::<Vec<_>>();
        if !bytes.is_empty() || count == 0 {
            return Ok(RiscvGuestSocketRead::Bytes(bytes));
        }
        let write_live = self
            .guest_socket_descriptions
            .values()
            .any(|candidate| candidate.write_queue == endpoint.read_queue);
        if !write_live {
            return Ok(RiscvGuestSocketRead::Bytes(Vec::new()));
        }
        if force_nonblocking || self.guest_fd_nonblocking(fd)? {
            Ok(RiscvGuestSocketRead::WouldBlock)
        } else {
            Ok(RiscvGuestSocketRead::Blocked)
        }
    }

    pub(super) fn consume_guest_socket_read(
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
            .guest_socket_descriptions
            .get(&description)
            .copied()
            .ok_or(GuestFdError::BadFd { fd })?;
        let queue = self
            .guest_socket_queues
            .get_mut(&endpoint.read_queue)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        for _ in 0..count.min(queue.buffer.len()) {
            queue.buffer.pop_front();
        }
        Ok(())
    }

    pub(super) fn guest_socket_write_plan(
        &self,
        fd: GuestFd,
        byte_count: usize,
    ) -> Result<RiscvGuestSocketWrite, GuestFdError> {
        self.guest_socket_write_plan_with_nonblocking(fd, byte_count, false)
    }

    fn guest_socket_write_plan_with_nonblocking(
        &self,
        fd: GuestFd,
        byte_count: usize,
        force_nonblocking: bool,
    ) -> Result<RiscvGuestSocketWrite, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).copied() else {
            return Ok(RiscvGuestSocketWrite::NotSocket);
        };
        let read_live = self
            .guest_socket_descriptions
            .values()
            .any(|candidate| candidate.read_queue == endpoint.write_queue);
        if !read_live {
            return Ok(RiscvGuestSocketWrite::BrokenPipe);
        }
        let queue = self
            .guest_socket_queues
            .get(&endpoint.write_queue)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        let available = queue.capacity.saturating_sub(queue.buffer.len());
        if byte_count == 0 {
            return Ok(RiscvGuestSocketWrite::Written(0));
        }
        if available == 0 {
            return Ok(if force_nonblocking || self.guest_fd_nonblocking(fd)? {
                RiscvGuestSocketWrite::WouldBlock
            } else {
                RiscvGuestSocketWrite::Blocked
            });
        }
        Ok(RiscvGuestSocketWrite::Written(available.min(byte_count)))
    }

    pub(super) fn write_guest_socket_from_fd(
        &mut self,
        fd: GuestFd,
        bytes: &[u8],
    ) -> Result<RiscvGuestSocketWrite, GuestFdError> {
        let write = self.guest_socket_write_plan(fd, bytes.len())?;
        let RiscvGuestSocketWrite::Written(written) = write else {
            return Ok(write);
        };
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let endpoint = self
            .guest_socket_descriptions
            .get(&description)
            .copied()
            .expect("socket write plan found endpoint");
        let queue = self
            .guest_socket_queues
            .get_mut(&endpoint.write_queue)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        queue.buffer.extend(bytes.iter().take(written).copied());
        Ok(RiscvGuestSocketWrite::Written(written))
    }

    pub(super) fn guest_socket_read_ready(
        &self,
        fd: GuestFd,
    ) -> Result<Option<bool>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).copied() else {
            return Ok(None);
        };
        let Some(queue) = self.guest_socket_queues.get(&endpoint.read_queue) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        let write_live = self
            .guest_socket_descriptions
            .values()
            .any(|candidate| candidate.write_queue == endpoint.read_queue);
        Ok(Some(!queue.buffer.is_empty() || !write_live))
    }

    pub(super) fn guest_socket_write_ready(
        &self,
        fd: GuestFd,
    ) -> Result<Option<bool>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).copied() else {
            return Ok(None);
        };
        let read_live = self
            .guest_socket_descriptions
            .values()
            .any(|candidate| candidate.read_queue == endpoint.write_queue);
        if !read_live {
            return Ok(Some(true));
        }
        let Some(queue) = self.guest_socket_queues.get(&endpoint.write_queue) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(queue.capacity > queue.buffer.len()))
    }

    pub(super) fn remove_guest_socket_description(&mut self, description: GuestFileDescriptionId) {
        let Some(endpoint) = self.guest_socket_descriptions.remove(&description) else {
            return;
        };
        self.remove_socket_queue_if_unreferenced(endpoint.read_queue);
        self.remove_socket_queue_if_unreferenced(endpoint.write_queue);
    }

    fn open_guest_socketpair(
        &mut self,
        left_fd: GuestFd,
        right_fd: GuestFd,
        left_description: GuestFileDescriptionId,
        right_description: GuestFileDescriptionId,
        type_flags: u64,
    ) -> Result<(), GuestFdError> {
        let left_to_right = self.next_guest_socket_queue_id()?;
        let right_to_left = self.next_guest_socket_queue_id_excluding(&[left_to_right])?;
        let nonblock = type_flags & RISCV_LINUX_O_NONBLOCK;
        let close_on_exec = type_flags & RISCV_LINUX_O_CLOEXEC != 0;
        let status_flags = GuestFileStatusFlags::new((RISCV_LINUX_O_RDWR | nonblock) as u32);

        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                left_description,
                status_flags,
            ))?;
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                right_description,
                status_flags,
            ))?;
        self.guest_fds.insert(
            left_fd,
            GuestFdEntry::new(left_description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_fds.insert(
            right_fd,
            GuestFdEntry::new(right_description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_socket_queues.insert(
            left_to_right,
            RiscvGuestSocketQueue::new(RISCV_LINUX_DEFAULT_SOCKET_CAPACITY_BYTES),
        );
        self.guest_socket_queues.insert(
            right_to_left,
            RiscvGuestSocketQueue::new(RISCV_LINUX_DEFAULT_SOCKET_CAPACITY_BYTES),
        );
        self.guest_socket_descriptions.insert(
            left_description,
            RiscvGuestSocketEndpoint::new(right_to_left, left_to_right),
        );
        self.guest_socket_descriptions.insert(
            right_description,
            RiscvGuestSocketEndpoint::new(left_to_right, right_to_left),
        );
        Ok(())
    }

    fn guest_fd_nonblocking(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        Ok(self.guest_fds.status_flags(fd)?.bits() & RISCV_LINUX_O_NONBLOCK as u32 != 0)
    }

    fn guest_fd_is_socket(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self.guest_socket_descriptions.contains_key(&description))
    }

    fn remove_socket_queue_if_unreferenced(&mut self, queue: RiscvGuestSocketQueueId) {
        let live = self
            .guest_socket_descriptions
            .values()
            .any(|endpoint| endpoint.read_queue == queue || endpoint.write_queue == queue);
        if !live {
            self.guest_socket_queues.remove(&queue);
        }
    }

    fn next_guest_socket_queue_id(&self) -> Result<RiscvGuestSocketQueueId, GuestFdError> {
        self.next_guest_socket_queue_id_excluding(&[])
    }

    fn next_guest_socket_queue_id_excluding(
        &self,
        excluded: &[RiscvGuestSocketQueueId],
    ) -> Result<RiscvGuestSocketQueueId, GuestFdError> {
        let mut candidate = 0_u64;
        loop {
            let queue = RiscvGuestSocketQueueId::new(candidate);
            if !self.guest_socket_queues.contains_key(&queue) && !excluded.contains(&queue) {
                return Ok(queue);
            }
            candidate = candidate
                .checked_add(1)
                .ok_or(GuestFdError::FdSpaceExhausted)?;
        }
    }

    fn next_socketpair_fds(&self) -> Result<(GuestFd, GuestFd), GuestFdError> {
        let left_fd = self.next_guest_fd_excluding(&[])?;
        let right_fd = self.next_guest_fd_excluding(&[left_fd])?;
        Ok((left_fd, right_fd))
    }

    fn next_socketpair_descriptions(
        &self,
    ) -> Result<(GuestFileDescriptionId, GuestFileDescriptionId), GuestFdError> {
        let left_description = self.next_guest_file_description_excluding(&[])?;
        let right_description = self.next_guest_file_description_excluding(&[left_description])?;
        Ok((left_description, right_description))
    }
}

pub(super) fn syscall_socketpair(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let domain = request.argument(0);
    let type_flags = request.argument(1);
    let protocol = request.argument(2);
    if domain != RISCV_LINUX_AF_UNIX {
        return linux_error(RISCV_LINUX_EAFNOSUPPORT);
    }
    if type_flags & !RISCV_LINUX_SOCKETPAIR_ALLOWED_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if type_flags & RISCV_LINUX_SOCK_TYPE_MASK != RISCV_LINUX_SOCK_STREAM {
        return linux_error(RISCV_LINUX_EPROTONOSUPPORT);
    }
    if protocol != 0 {
        return linux_error(RISCV_LINUX_EPROTONOSUPPORT);
    }

    let (left_fd, right_fd) = match state.next_socketpair_fds() {
        Ok(fds) => fds,
        Err(_) => return linux_error(RISCV_LINUX_EMFILE),
    };
    let (left_description, right_description) = match state.next_socketpair_descriptions() {
        Ok(descriptions) => descriptions,
        Err(_) => return linux_error(RISCV_LINUX_EMFILE),
    };
    let mut fd_bytes = [0_u8; 8];
    fd_bytes[..4].copy_from_slice(&left_fd.get().to_le_bytes());
    fd_bytes[4..].copy_from_slice(&right_fd.get().to_le_bytes());
    if !guest_memory.write(request.argument(3), &fd_bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }

    match state.open_guest_socketpair(
        left_fd,
        right_fd,
        left_description,
        right_description,
        type_flags,
    ) {
        Ok(()) => 0,
        Err(_) => linux_error(RISCV_LINUX_EMFILE),
    }
}

pub(super) fn syscall_sendto(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    match state.guest_fd_is_socket(fd) {
        Ok(true) => {}
        Ok(false) => return Some(linux_error(RISCV_LINUX_ENOTSOCK)),
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }
    let flags = request.argument(3);
    if flags & !RISCV_LINUX_SENDTO_ALLOWED_FLAGS != 0
        || request.argument(4) != 0
        || request.argument(5) != 0
    {
        return Some(linux_error(RISCV_LINUX_ENOTSUP));
    }

    let Ok(byte_count) = usize::try_from(request.argument(2)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let force_nonblocking = flags & RISCV_LINUX_MSG_DONTWAIT != 0;
    let written =
        match state.guest_socket_write_plan_with_nonblocking(fd, byte_count, force_nonblocking) {
            Ok(RiscvGuestSocketWrite::NotSocket) => return Some(linux_error(RISCV_LINUX_ENOTSOCK)),
            Ok(RiscvGuestSocketWrite::Written(written)) => written,
            Ok(write @ RiscvGuestSocketWrite::WouldBlock)
            | Ok(write @ RiscvGuestSocketWrite::Blocked)
            | Ok(write @ RiscvGuestSocketWrite::BrokenPipe) => return socket_write_result(write),
            Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
        };
    if written == 0 {
        return Some(0);
    }
    let Some(bytes) = guest_memory.read(request.argument(1), written) else {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    };
    if bytes.len() != written {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    match state.write_guest_socket_from_fd(fd, &bytes) {
        Ok(write) => socket_write_result(write),
        Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}

pub(super) fn syscall_recvfrom(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    match state.guest_fd_is_socket(fd) {
        Ok(true) => {}
        Ok(false) => return Some(linux_error(RISCV_LINUX_ENOTSOCK)),
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }
    let flags = request.argument(3);
    if flags & !RISCV_LINUX_RECVFROM_ALLOWED_FLAGS != 0
        || request.argument(4) != 0
        || request.argument(5) != 0
    {
        return Some(linux_error(RISCV_LINUX_ENOTSUP));
    }

    let Ok(byte_count) = usize::try_from(request.argument(2)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let force_nonblocking = flags & RISCV_LINUX_MSG_DONTWAIT != 0;
    match state.guest_socket_read_with_nonblocking(fd, byte_count, force_nonblocking) {
        Ok(RiscvGuestSocketRead::Bytes(bytes)) => {
            if bytes.is_empty() {
                return Some(0);
            }
            if !guest_memory.write(request.argument(1), &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_socket_read(fd, bytes.len()).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            Some(bytes.len() as u64)
        }
        Ok(RiscvGuestSocketRead::WouldBlock) => Some(linux_error(RISCV_LINUX_EAGAIN)),
        Ok(RiscvGuestSocketRead::Blocked) => None,
        Ok(RiscvGuestSocketRead::NotSocket) => Some(linux_error(RISCV_LINUX_ENOTSOCK)),
        Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}

pub(super) fn socket_write_result(write: RiscvGuestSocketWrite) -> Option<u64> {
    match write {
        RiscvGuestSocketWrite::Written(written) => Some(written as u64),
        RiscvGuestSocketWrite::WouldBlock => Some(linux_error(RISCV_LINUX_EAGAIN)),
        RiscvGuestSocketWrite::Blocked => None,
        RiscvGuestSocketWrite::BrokenPipe => Some(linux_error(RISCV_LINUX_EPIPE)),
        RiscvGuestSocketWrite::NotSocket => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}
