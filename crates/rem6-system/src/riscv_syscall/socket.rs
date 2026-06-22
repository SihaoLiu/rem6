use std::collections::VecDeque;

use super::{
    guest_fd_argument,
    iovec::{read_iovec_prefix, read_iovecs, write_iovecs, RISCV_LINUX_IOV_MAX},
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EADDRINUSE, RISCV_LINUX_EAFNOSUPPORT, RISCV_LINUX_EAGAIN,
    RISCV_LINUX_EBADF, RISCV_LINUX_ECONNREFUSED, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_ENOPROTOOPT, RISCV_LINUX_ENOTCONN, RISCV_LINUX_ENOTSOCK,
    RISCV_LINUX_ENOTSUP, RISCV_LINUX_EPIPE, RISCV_LINUX_EPROTONOSUPPORT, RISCV_LINUX_O_ACCMODE,
    RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_RDWR,
    RISCV_LINUX_O_WRONLY,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_SOCKET: u64 = 198;
pub(super) const RISCV_LINUX_SOCKETPAIR: u64 = 199;
pub(super) const RISCV_LINUX_BIND: u64 = 200;
pub(super) const RISCV_LINUX_LISTEN: u64 = 201;
pub(super) const RISCV_LINUX_ACCEPT: u64 = 202;
pub(super) const RISCV_LINUX_CONNECT: u64 = 203;
pub(super) const RISCV_LINUX_GETSOCKNAME: u64 = 204;
pub(super) const RISCV_LINUX_GETPEERNAME: u64 = 205;
pub(super) const RISCV_LINUX_SENDTO: u64 = 206;
pub(super) const RISCV_LINUX_RECVFROM: u64 = 207;
pub(super) const RISCV_LINUX_SETSOCKOPT: u64 = 208;
pub(super) const RISCV_LINUX_GETSOCKOPT: u64 = 209;
pub(super) const RISCV_LINUX_SHUTDOWN: u64 = 210;
pub(super) const RISCV_LINUX_SENDMSG: u64 = 211;
pub(super) const RISCV_LINUX_RECVMSG: u64 = 212;
pub(super) const RISCV_LINUX_ACCEPT4: u64 = 242;

const RISCV_LINUX_AF_UNIX: u64 = 1;
const RISCV_LINUX_SOCK_STREAM: u64 = 1;
const RISCV_LINUX_SOL_SOCKET: u64 = 1;
const RISCV_LINUX_SO_REUSEADDR: u64 = 2;
const RISCV_LINUX_SO_TYPE: u64 = 3;
const RISCV_LINUX_SO_ERROR: u64 = 4;
const RISCV_LINUX_INT_OPTION_BYTES: usize = 4;
const RISCV_LINUX_SOCK_TYPE_MASK: u64 = 0xf;
const RISCV_LINUX_SOCKET_ALLOWED_FLAGS: u64 =
    RISCV_LINUX_SOCK_STREAM | RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;
const RISCV_LINUX_SOCKETPAIR_ALLOWED_FLAGS: u64 =
    RISCV_LINUX_SOCK_STREAM | RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;
const RISCV_LINUX_MSG_DONTWAIT: u64 = 0x40;
const RISCV_LINUX_MSG_NOSIGNAL: u64 = 0x4000;
const RISCV_LINUX_SENDTO_ALLOWED_FLAGS: u64 = RISCV_LINUX_MSG_DONTWAIT | RISCV_LINUX_MSG_NOSIGNAL;
const RISCV_LINUX_RECVFROM_ALLOWED_FLAGS: u64 = RISCV_LINUX_MSG_DONTWAIT;
const RISCV_LINUX_SENDMSG_ALLOWED_FLAGS: u64 = RISCV_LINUX_SENDTO_ALLOWED_FLAGS;
const RISCV_LINUX_RECVMSG_ALLOWED_FLAGS: u64 = RISCV_LINUX_RECVFROM_ALLOWED_FLAGS;
const RISCV_LINUX_DEFAULT_SOCKET_CAPACITY_BYTES: usize = 64 * 1024;
const RISCV_LINUX_MSGHDR_BYTES: usize = 56;
const RISCV_LINUX_MSGHDR_NAME_OFFSET: usize = 0;
const RISCV_LINUX_MSGHDR_NAMELEN_OFFSET: usize = 8;
const RISCV_LINUX_MSGHDR_IOV_OFFSET: usize = 16;
const RISCV_LINUX_MSGHDR_IOVLEN_OFFSET: usize = 24;
const RISCV_LINUX_MSGHDR_CONTROL_OFFSET: usize = 32;
const RISCV_LINUX_MSGHDR_CONTROLLEN_OFFSET: usize = 40;
const RISCV_LINUX_MSGHDR_FLAGS_OFFSET: usize = 48;
const RISCV_LINUX_SOCKADDR_UN_MIN_BYTES: usize = 3;
const RISCV_LINUX_SOCKADDR_UN_BYTES: usize = 110;
const RISCV_LINUX_SHUT_RD: u64 = 0;
const RISCV_LINUX_SHUT_WR: u64 = 1;
const RISCV_LINUX_SHUT_RDWR: u64 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvLinuxMsgHdr {
    name: u64,
    namelen: u32,
    iov: u64,
    iovlen: u64,
    control: u64,
    controllen: u64,
}

impl RiscvLinuxMsgHdr {
    fn read(guest_memory: &RiscvGuestMemoryReader, address: u64) -> Result<Self, u64> {
        let bytes = read_guest_exact(guest_memory, address, RISCV_LINUX_MSGHDR_BYTES)
            .ok_or(RISCV_LINUX_EFAULT)?;
        let msg = Self {
            name: read_u64(&bytes, RISCV_LINUX_MSGHDR_NAME_OFFSET),
            namelen: read_u32(&bytes, RISCV_LINUX_MSGHDR_NAMELEN_OFFSET),
            iov: read_u64(&bytes, RISCV_LINUX_MSGHDR_IOV_OFFSET),
            iovlen: read_u64(&bytes, RISCV_LINUX_MSGHDR_IOVLEN_OFFSET),
            control: read_u64(&bytes, RISCV_LINUX_MSGHDR_CONTROL_OFFSET),
            controllen: read_u64(&bytes, RISCV_LINUX_MSGHDR_CONTROLLEN_OFFSET),
        };
        if msg.iovlen > RISCV_LINUX_IOV_MAX {
            return Err(RISCV_LINUX_EINVAL);
        }
        if msg.name != 0 || msg.namelen != 0 || msg.control != 0 || msg.controllen != 0 {
            return Err(RISCV_LINUX_ENOTSUP);
        }
        Ok(msg)
    }
}

fn read_guest_exact(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
    len: usize,
) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(len);
    for offset in 0..len {
        let address = address.checked_add(offset as u64)?;
        let byte = guest_memory.read(address, 1)?;
        if byte.len() != 1 {
            return None;
        }
        bytes.push(byte[0]);
    }
    Some(bytes)
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    let mut raw = [0; 4];
    raw.copy_from_slice(&bytes[offset..offset + 4]);
    u32::from_le_bytes(raw)
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut raw = [0; 8];
    raw.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(raw)
}

fn read_sockaddr_un_name(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
    len: u64,
) -> Result<Vec<u8>, u64> {
    let Ok(len) = usize::try_from(len) else {
        return Err(RISCV_LINUX_EINVAL);
    };
    if len < RISCV_LINUX_SOCKADDR_UN_MIN_BYTES {
        return Err(RISCV_LINUX_EINVAL);
    }
    if len > RISCV_LINUX_SOCKADDR_UN_BYTES {
        return Err(RISCV_LINUX_EINVAL);
    }
    let bytes = read_guest_exact(guest_memory, address, len).ok_or(RISCV_LINUX_EFAULT)?;
    let family = u16::from_le_bytes([bytes[0], bytes[1]]);
    if u64::from(family) != RISCV_LINUX_AF_UNIX {
        return Err(RISCV_LINUX_EAFNOSUPPORT);
    }
    Ok(bytes[2..].to_vec())
}

fn read_guest_u32(guest_memory: &RiscvGuestMemoryReader, address: u64) -> Result<u32, u64> {
    let bytes = read_guest_exact(guest_memory, address, 4).ok_or(RISCV_LINUX_EFAULT)?;
    Ok(read_u32(&bytes, 0))
}

fn sockaddr_un_bytes(name: Option<&[u8]>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(2 + name.map_or(0, |name| name.len()));
    bytes.extend_from_slice(&(RISCV_LINUX_AF_UNIX as u16).to_le_bytes());
    if let Some(name) = name {
        bytes.extend_from_slice(name);
    }
    bytes
}

fn write_sockaddr_un(
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
    address: u64,
    len_address: u64,
    name: Option<&[u8]>,
) -> Result<(), u64> {
    let sockaddr = sockaddr_un_bytes(name);
    let requested_len = read_guest_u32(guest_memory_reader, len_address)?;
    if requested_len > i32::MAX as u32 {
        return Err(RISCV_LINUX_EINVAL);
    }
    let requested_len = requested_len as usize;
    let write_len = requested_len.min(sockaddr.len());
    if write_len != 0 && !guest_memory_writer.write(address, &sockaddr[..write_len]) {
        return Err(RISCV_LINUX_EFAULT);
    }
    if !guest_memory_writer.write(len_address, &(sockaddr.len() as u32).to_le_bytes()) {
        return Err(RISCV_LINUX_EFAULT);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct RiscvGuestSocketQueueId(u64);

impl RiscvGuestSocketQueueId {
    const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestSocketEndpoint {
    read_queue: Option<RiscvGuestSocketQueueId>,
    write_queue: Option<RiscvGuestSocketQueueId>,
    local_name: Option<Vec<u8>>,
    peer_name: Option<Vec<u8>>,
    read_shutdown: bool,
    write_shutdown: bool,
    reuse_addr: bool,
}

impl RiscvGuestSocketEndpoint {
    fn new(read_queue: RiscvGuestSocketQueueId, write_queue: RiscvGuestSocketQueueId) -> Self {
        Self {
            read_queue: Some(read_queue),
            write_queue: Some(write_queue),
            local_name: None,
            peer_name: None,
            read_shutdown: false,
            write_shutdown: false,
            reuse_addr: false,
        }
    }

    const fn unconnected() -> Self {
        Self {
            read_queue: None,
            write_queue: None,
            local_name: None,
            peer_name: None,
            read_shutdown: false,
            write_shutdown: false,
            reuse_addr: false,
        }
    }

    const fn is_connected(&self) -> bool {
        self.read_queue.is_some() && self.write_queue.is_some()
    }

    fn with_reuse_addr(mut self, reuse_addr: bool) -> Self {
        self.reuse_addr = reuse_addr;
        self
    }

    fn with_local_name(mut self, local_name: Option<Vec<u8>>) -> Self {
        self.local_name = local_name;
        self
    }

    fn with_peer_name(mut self, peer_name: Option<Vec<u8>>) -> Self {
        self.peer_name = peer_name;
        self
    }

    fn local_name(&self) -> Option<&[u8]> {
        self.local_name.as_deref()
    }

    fn peer_name(&self) -> Option<&[u8]> {
        self.peer_name.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestSocketQueue {
    buffer: VecDeque<u8>,
    capacity: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestSocketListener {
    backlog: usize,
    pending: VecDeque<RiscvGuestSocketEndpoint>,
}

impl RiscvGuestSocketListener {
    fn new(backlog: usize) -> Self {
        Self {
            backlog: backlog.max(1),
            pending: VecDeque::new(),
        }
    }

    fn set_backlog(&mut self, backlog: usize) {
        self.backlog = backlog.max(1);
    }
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
    NotConnected,
    Bytes(Vec<u8>),
    WouldBlock,
    Blocked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestSocketWrite {
    NotSocket,
    NotConnected,
    Written(usize),
    WouldBlock,
    Blocked,
    BrokenPipe,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestSocketConnect {
    Connected,
    Blocked,
    Errno(u64),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvPendingAcceptedGuestSocket {
    fd: GuestFd,
    description: GuestFileDescriptionId,
    endpoint: RiscvGuestSocketEndpoint,
}

impl RiscvPendingAcceptedGuestSocket {
    fn peer_name(&self) -> Option<&[u8]> {
        self.endpoint.peer_name()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestSocketShutdown {
    Read,
    Write,
    ReadWrite,
}

impl RiscvGuestSocketShutdown {
    const fn from_linux_how(how: u64) -> Option<Self> {
        match how {
            RISCV_LINUX_SHUT_RD => Some(Self::Read),
            RISCV_LINUX_SHUT_WR => Some(Self::Write),
            RISCV_LINUX_SHUT_RDWR => Some(Self::ReadWrite),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestSocketOption {
    ReuseAddr,
    Type,
    Error,
}

impl RiscvGuestSocketOption {
    const fn from_getsockopt(level: u64, optname: u64) -> Result<Self, u64> {
        if level != RISCV_LINUX_SOL_SOCKET {
            return Err(RISCV_LINUX_ENOTSUP);
        }
        match optname {
            RISCV_LINUX_SO_REUSEADDR => Ok(Self::ReuseAddr),
            RISCV_LINUX_SO_TYPE => Ok(Self::Type),
            RISCV_LINUX_SO_ERROR => Ok(Self::Error),
            _ => Err(RISCV_LINUX_ENOPROTOOPT),
        }
    }

    const fn from_setsockopt(level: u64, optname: u64) -> Result<Self, u64> {
        if level != RISCV_LINUX_SOL_SOCKET {
            return Err(RISCV_LINUX_ENOTSUP);
        }
        match optname {
            RISCV_LINUX_SO_REUSEADDR => Ok(Self::ReuseAddr),
            _ => Err(RISCV_LINUX_ENOPROTOOPT),
        }
    }
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
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).cloned() else {
            return Ok(RiscvGuestSocketRead::NotSocket);
        };
        if endpoint.read_shutdown {
            return Ok(RiscvGuestSocketRead::Bytes(Vec::new()));
        }
        let Some(read_queue) = endpoint.read_queue else {
            return Ok(RiscvGuestSocketRead::NotConnected);
        };
        let Some(queue) = self.guest_socket_queues.get(&read_queue) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        let bytes = queue.buffer.iter().take(count).copied().collect::<Vec<_>>();
        if !bytes.is_empty() || count == 0 {
            return Ok(RiscvGuestSocketRead::Bytes(bytes));
        }
        let write_live = self.guest_socket_write_queue_live(read_queue);
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
            .cloned()
            .ok_or(GuestFdError::BadFd { fd })?;
        let read_queue = endpoint
            .read_queue
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        let queue = self
            .guest_socket_queues
            .get_mut(&read_queue)
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
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).cloned() else {
            return Ok(RiscvGuestSocketWrite::NotSocket);
        };
        if endpoint.write_shutdown {
            return Ok(RiscvGuestSocketWrite::BrokenPipe);
        }
        let Some(write_queue) = endpoint.write_queue else {
            return Ok(RiscvGuestSocketWrite::NotConnected);
        };
        let read_live = self.guest_socket_read_queue_live(write_queue);
        if !read_live {
            return Ok(RiscvGuestSocketWrite::BrokenPipe);
        }
        let queue = self
            .guest_socket_queues
            .get(&write_queue)
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
            .cloned()
            .expect("socket write plan found endpoint");
        let write_queue = endpoint
            .write_queue
            .expect("socket write plan found connected endpoint");
        let queue = self
            .guest_socket_queues
            .get_mut(&write_queue)
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
        if let Some(listener) = self.guest_socket_listeners.get(&description) {
            return Ok(Some(!listener.pending.is_empty()));
        }
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).cloned() else {
            return Ok(None);
        };
        if endpoint.read_shutdown {
            return Ok(Some(true));
        }
        let Some(read_queue) = endpoint.read_queue else {
            return Ok(Some(false));
        };
        let Some(queue) = self.guest_socket_queues.get(&read_queue) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        let write_live = self.guest_socket_write_queue_live(read_queue);
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
        if self.guest_socket_listeners.contains_key(&description) {
            return Ok(Some(false));
        }
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).cloned() else {
            return Ok(None);
        };
        if endpoint.write_shutdown {
            return Ok(Some(true));
        }
        let Some(write_queue) = endpoint.write_queue else {
            return Ok(Some(true));
        };
        let read_live = self.guest_socket_read_queue_live(write_queue);
        if !read_live {
            return Ok(Some(true));
        }
        let Some(queue) = self.guest_socket_queues.get(&write_queue) else {
            return Err(GuestFdError::MissingFileDescription { description });
        };
        Ok(Some(queue.capacity > queue.buffer.len()))
    }

    pub(super) fn guest_socket_hangup_ready(
        &self,
        fd: GuestFd,
    ) -> Result<Option<bool>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        if self.guest_socket_listeners.contains_key(&description) {
            return Ok(Some(false));
        }
        let Some(endpoint) = self.guest_socket_descriptions.get(&description).cloned() else {
            return Ok(None);
        };
        Ok(Some(!endpoint.is_connected()))
    }

    fn shutdown_guest_socket(
        &mut self,
        fd: GuestFd,
        how: RiscvGuestSocketShutdown,
    ) -> Result<bool, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_socket_descriptions.get_mut(&description) else {
            return Ok(false);
        };
        match how {
            RiscvGuestSocketShutdown::Read => endpoint.read_shutdown = true,
            RiscvGuestSocketShutdown::Write => endpoint.write_shutdown = true,
            RiscvGuestSocketShutdown::ReadWrite => {
                endpoint.read_shutdown = true;
                endpoint.write_shutdown = true;
            }
        }
        Ok(true)
    }

    fn guest_socket_endpoint(
        &self,
        fd: GuestFd,
    ) -> Result<Option<RiscvGuestSocketEndpoint>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self.guest_socket_descriptions.get(&description).cloned())
    }

    fn guest_socket_description_for_fd(
        &self,
        fd: GuestFd,
    ) -> Result<Option<GuestFileDescriptionId>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self
            .guest_socket_descriptions
            .contains_key(&description)
            .then_some(description))
    }

    fn set_guest_socket_reuse_addr(
        &mut self,
        fd: GuestFd,
        enabled: bool,
    ) -> Result<bool, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(endpoint) = self.guest_socket_descriptions.get_mut(&description) else {
            return Ok(false);
        };
        endpoint.reuse_addr = enabled;
        Ok(true)
    }

    pub(super) fn remove_guest_socket_description(&mut self, description: GuestFileDescriptionId) {
        let Some(endpoint) = self.guest_socket_descriptions.remove(&description) else {
            return;
        };
        if let Some(name) = self.guest_socket_bound_names.remove(&description) {
            self.guest_socket_bindings.remove(&name);
        }
        let listener = self.guest_socket_listeners.remove(&description);
        if let Some(read_queue) = endpoint.read_queue {
            self.remove_socket_queue_if_unreferenced(read_queue);
        }
        if let Some(write_queue) = endpoint.write_queue {
            self.remove_socket_queue_if_unreferenced(write_queue);
        }
        if let Some(listener) = listener {
            for pending in listener.pending {
                if let Some(read_queue) = pending.read_queue {
                    self.remove_socket_queue_if_unreferenced(read_queue);
                }
                if let Some(write_queue) = pending.write_queue {
                    self.remove_socket_queue_if_unreferenced(write_queue);
                }
            }
        }
    }

    fn bind_guest_socket(
        &mut self,
        fd: GuestFd,
        name: Vec<u8>,
    ) -> Result<Result<(), u64>, GuestFdError> {
        let Some(description) = self.guest_socket_description_for_fd(fd)? else {
            return Ok(Err(RISCV_LINUX_ENOTSOCK));
        };
        if self
            .guest_socket_descriptions
            .get(&description)
            .is_some_and(RiscvGuestSocketEndpoint::is_connected)
            || self.guest_socket_listeners.contains_key(&description)
            || self.guest_socket_bound_names.contains_key(&description)
        {
            return Ok(Err(RISCV_LINUX_EINVAL));
        }
        if self.guest_socket_bindings.contains_key(&name) {
            return Ok(Err(RISCV_LINUX_EADDRINUSE));
        }
        self.guest_socket_bindings.insert(name.clone(), description);
        self.guest_socket_bound_names
            .insert(description, name.clone());
        if let Some(endpoint) = self.guest_socket_descriptions.get_mut(&description) {
            endpoint.local_name = Some(name);
        }
        Ok(Ok(()))
    }

    fn listen_guest_socket(
        &mut self,
        fd: GuestFd,
        backlog: u64,
    ) -> Result<Result<(), u64>, GuestFdError> {
        let Some(description) = self.guest_socket_description_for_fd(fd)? else {
            return Ok(Err(RISCV_LINUX_ENOTSOCK));
        };
        if !self.guest_socket_bound_names.contains_key(&description)
            || self
                .guest_socket_descriptions
                .get(&description)
                .is_some_and(RiscvGuestSocketEndpoint::is_connected)
        {
            return Ok(Err(RISCV_LINUX_EINVAL));
        }
        let backlog = usize::try_from(backlog.min(i32::MAX as u64)).unwrap_or(usize::MAX);
        if let Some(listener) = self.guest_socket_listeners.get_mut(&description) {
            listener.set_backlog(backlog);
        } else {
            self.guest_socket_listeners
                .insert(description, RiscvGuestSocketListener::new(backlog));
        }
        Ok(Ok(()))
    }

    fn connect_guest_socket(
        &mut self,
        fd: GuestFd,
        name: &[u8],
    ) -> Result<RiscvGuestSocketConnect, GuestFdError> {
        let Some(client_description) = self.guest_socket_description_for_fd(fd)? else {
            return Ok(RiscvGuestSocketConnect::Errno(RISCV_LINUX_ENOTSOCK));
        };
        let Some(client_endpoint) = self
            .guest_socket_descriptions
            .get(&client_description)
            .cloned()
        else {
            return Ok(RiscvGuestSocketConnect::Errno(RISCV_LINUX_ENOTSOCK));
        };
        if client_endpoint.is_connected()
            || self
                .guest_socket_listeners
                .contains_key(&client_description)
        {
            return Ok(RiscvGuestSocketConnect::Errno(RISCV_LINUX_EINVAL));
        }
        let Some(server_description) = self.guest_socket_bindings.get(name).copied() else {
            return Ok(RiscvGuestSocketConnect::Errno(RISCV_LINUX_ECONNREFUSED));
        };
        let Some(listener) = self.guest_socket_listeners.get(&server_description) else {
            return Ok(RiscvGuestSocketConnect::Errno(RISCV_LINUX_ECONNREFUSED));
        };
        if listener.pending.len() >= listener.backlog {
            return if self.guest_fd_nonblocking(fd)? {
                Ok(RiscvGuestSocketConnect::Errno(RISCV_LINUX_EAGAIN))
            } else {
                Ok(RiscvGuestSocketConnect::Blocked)
            };
        }

        let client_to_server = self.next_guest_socket_queue_id()?;
        let server_to_client = self.next_guest_socket_queue_id_excluding(&[client_to_server])?;
        self.guest_socket_queues.insert(
            client_to_server,
            RiscvGuestSocketQueue::new(RISCV_LINUX_DEFAULT_SOCKET_CAPACITY_BYTES),
        );
        self.guest_socket_queues.insert(
            server_to_client,
            RiscvGuestSocketQueue::new(RISCV_LINUX_DEFAULT_SOCKET_CAPACITY_BYTES),
        );
        let client_local_name = client_endpoint.local_name.clone();
        let server_name = name.to_vec();
        self.guest_socket_descriptions.insert(
            client_description,
            RiscvGuestSocketEndpoint::new(server_to_client, client_to_server)
                .with_reuse_addr(client_endpoint.reuse_addr)
                .with_local_name(client_local_name.clone())
                .with_peer_name(Some(server_name.clone())),
        );
        let accepted = RiscvGuestSocketEndpoint::new(client_to_server, server_to_client)
            .with_local_name(Some(server_name))
            .with_peer_name(client_local_name);
        let Some(listener) = self.guest_socket_listeners.get_mut(&server_description) else {
            return Ok(RiscvGuestSocketConnect::Errno(RISCV_LINUX_ECONNREFUSED));
        };
        listener.pending.push_back(accepted);
        Ok(RiscvGuestSocketConnect::Connected)
    }

    fn prepare_guest_socket_accept(
        &mut self,
        fd: GuestFd,
        flags: u64,
    ) -> Result<Result<Option<RiscvPendingAcceptedGuestSocket>, u64>, GuestFdError> {
        if flags & !(RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK) != 0 {
            return Ok(Err(RISCV_LINUX_EINVAL));
        }
        let Some(listener_description) = self.guest_socket_description_for_fd(fd)? else {
            return Ok(Err(RISCV_LINUX_ENOTSOCK));
        };
        let Some(listener) = self.guest_socket_listeners.get(&listener_description) else {
            return Ok(Err(RISCV_LINUX_EINVAL));
        };
        if listener.pending.is_empty() {
            if self.guest_fd_nonblocking(fd)? {
                return Ok(Err(RISCV_LINUX_EAGAIN));
            }
            return Ok(Ok(None));
        }
        let accepted_fd = self.next_guest_fd_excluding(&[])?;
        let accepted_description = self.next_guest_file_description_excluding(&[])?;
        let endpoint = self
            .guest_socket_listeners
            .get_mut(&listener_description)
            .ok_or(GuestFdError::MissingFileDescription {
                description: listener_description,
            })?
            .pending
            .pop_front()
            .ok_or(GuestFdError::MissingFileDescription {
                description: listener_description,
            })?;
        Ok(Ok(Some(RiscvPendingAcceptedGuestSocket {
            fd: accepted_fd,
            description: accepted_description,
            endpoint,
        })))
    }

    fn install_accepted_guest_socket(
        &mut self,
        accepted: RiscvPendingAcceptedGuestSocket,
        flags: u64,
    ) -> Result<GuestFd, GuestFdError> {
        let status_flags = GuestFileStatusFlags::new(
            (RISCV_LINUX_O_RDWR | (flags & RISCV_LINUX_O_NONBLOCK)) as u32,
        );
        let accepted_fd = accepted.fd;
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                accepted.description,
                status_flags,
            ))?;
        self.guest_fds.insert(
            accepted.fd,
            GuestFdEntry::new(accepted.description)
                .with_close_on_exec(flags & RISCV_LINUX_O_CLOEXEC != 0),
        )?;
        self.guest_socket_descriptions
            .insert(accepted.description, accepted.endpoint);
        Ok(accepted_fd)
    }

    fn open_guest_socket(&mut self, type_flags: u64) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_guest_fd_excluding(&[])?;
        let description = self.next_guest_file_description_excluding(&[])?;
        let nonblock = type_flags & RISCV_LINUX_O_NONBLOCK;
        let close_on_exec = type_flags & RISCV_LINUX_O_CLOEXEC != 0;
        let status_flags = GuestFileStatusFlags::new((RISCV_LINUX_O_RDWR | nonblock) as u32);

        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                status_flags,
            ))?;
        self.guest_fds.insert(
            fd,
            GuestFdEntry::new(description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_socket_descriptions
            .insert(description, RiscvGuestSocketEndpoint::unconnected());
        Ok(fd)
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

    fn guest_socket_read_queue_live(&self, queue: RiscvGuestSocketQueueId) -> bool {
        self.guest_socket_descriptions
            .values()
            .any(|endpoint| endpoint.read_queue == Some(queue) && !endpoint.read_shutdown)
            || self.guest_socket_listeners.values().any(|listener| {
                listener
                    .pending
                    .iter()
                    .any(|endpoint| endpoint.read_queue == Some(queue) && !endpoint.read_shutdown)
            })
    }

    fn guest_socket_write_queue_live(&self, queue: RiscvGuestSocketQueueId) -> bool {
        self.guest_socket_descriptions
            .values()
            .any(|endpoint| endpoint.write_queue == Some(queue) && !endpoint.write_shutdown)
            || self.guest_socket_listeners.values().any(|listener| {
                listener
                    .pending
                    .iter()
                    .any(|endpoint| endpoint.write_queue == Some(queue) && !endpoint.write_shutdown)
            })
    }

    fn remove_socket_queue_if_unreferenced(&mut self, queue: RiscvGuestSocketQueueId) {
        let live = self.guest_socket_descriptions.values().any(|endpoint| {
            endpoint.read_queue == Some(queue) || endpoint.write_queue == Some(queue)
        }) || self.guest_socket_listeners.values().any(|listener| {
            listener.pending.iter().any(|endpoint| {
                endpoint.read_queue == Some(queue) || endpoint.write_queue == Some(queue)
            })
        });
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

pub(super) fn syscall_socket(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let domain = request.argument(0);
    let type_flags = request.argument(1);
    let protocol = request.argument(2);
    if domain != RISCV_LINUX_AF_UNIX {
        return linux_error(RISCV_LINUX_EAFNOSUPPORT);
    }
    if type_flags & !RISCV_LINUX_SOCKET_ALLOWED_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if type_flags & RISCV_LINUX_SOCK_TYPE_MASK != RISCV_LINUX_SOCK_STREAM {
        return linux_error(RISCV_LINUX_EPROTONOSUPPORT);
    }
    if protocol != 0 {
        return linux_error(RISCV_LINUX_EPROTONOSUPPORT);
    }

    match state.open_guest_socket(type_flags) {
        Ok(fd) => u64::from(fd.get()),
        Err(_) => linux_error(RISCV_LINUX_EMFILE),
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

pub(super) fn syscall_socket_bind(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let name = match read_sockaddr_un_name(guest_memory, request.argument(1), request.argument(2)) {
        Ok(name) => name,
        Err(errno) => return linux_error(errno),
    };
    match state.bind_guest_socket(fd, name) {
        Ok(Ok(())) => 0,
        Ok(Err(errno)) => linux_error(errno),
        Err(_) => linux_error(RISCV_LINUX_EBADF),
    }
}

pub(super) fn syscall_socket_listen(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.listen_guest_socket(fd, request.argument(1)) {
        Ok(Ok(())) => 0,
        Ok(Err(errno)) => linux_error(errno),
        Err(_) => linux_error(RISCV_LINUX_EBADF),
    }
}

pub(super) fn syscall_socket_connect(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let name = match read_sockaddr_un_name(guest_memory, request.argument(1), request.argument(2)) {
        Ok(name) => name,
        Err(errno) => return Some(linux_error(errno)),
    };
    match state.connect_guest_socket(fd, &name) {
        Ok(RiscvGuestSocketConnect::Connected) => Some(0),
        Ok(RiscvGuestSocketConnect::Blocked) => None,
        Ok(RiscvGuestSocketConnect::Errno(errno)) => Some(linux_error(errno)),
        Err(GuestFdError::FdSpaceExhausted) => Some(linux_error(RISCV_LINUX_EMFILE)),
        Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}

pub(super) fn syscall_socket_accept(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    flags: u64,
    guest_memory: Option<(&RiscvGuestMemoryReader, &RiscvGuestMemoryWriter)>,
) -> Option<u64> {
    enum AcceptAddressWrite {
        Ignore,
        Fault,
        Write { address: u64, len_address: u64 },
    }

    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let address_write = match (request.argument(1), request.argument(2)) {
        (0, _) => AcceptAddressWrite::Ignore,
        (_, 0) => AcceptAddressWrite::Fault,
        (address, len_address) => AcceptAddressWrite::Write {
            address,
            len_address,
        },
    };
    if matches!(address_write, AcceptAddressWrite::Write { .. }) && guest_memory.is_none() {
        return None;
    }
    match state.prepare_guest_socket_accept(fd, flags) {
        Ok(Ok(Some(pending))) => {
            match address_write {
                AcceptAddressWrite::Ignore => {}
                AcceptAddressWrite::Fault => return Some(linux_error(RISCV_LINUX_EFAULT)),
                AcceptAddressWrite::Write {
                    address,
                    len_address,
                } => {
                    let (guest_memory_reader, guest_memory_writer) =
                        guest_memory.expect("peer address write requires guest memory");
                    if let Err(errno) = write_sockaddr_un(
                        guest_memory_reader,
                        guest_memory_writer,
                        address,
                        len_address,
                        pending.peer_name(),
                    ) {
                        return Some(linux_error(errno));
                    }
                }
            }
            match state.install_accepted_guest_socket(pending, flags) {
                Ok(accepted_fd) => Some(u64::from(accepted_fd.get())),
                Err(GuestFdError::FdSpaceExhausted) => Some(linux_error(RISCV_LINUX_EMFILE)),
                Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
            }
        }
        Ok(Ok(None)) => None,
        Ok(Err(errno)) => Some(linux_error(errno)),
        Err(GuestFdError::FdSpaceExhausted) => Some(linux_error(RISCV_LINUX_EMFILE)),
        Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}

pub(super) fn syscall_getsockname(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    syscall_unix_socket_name(
        request,
        state,
        guest_memory_reader,
        guest_memory_writer,
        false,
    )
}

pub(super) fn syscall_getpeername(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    syscall_unix_socket_name(
        request,
        state,
        guest_memory_reader,
        guest_memory_writer,
        true,
    )
}

fn syscall_unix_socket_name(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
    require_connected_peer: bool,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let endpoint = match state.guest_socket_endpoint(fd) {
        Ok(Some(endpoint)) => endpoint,
        Ok(None) => return linux_error(RISCV_LINUX_ENOTSOCK),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    if require_connected_peer && !endpoint.is_connected() {
        return linux_error(RISCV_LINUX_ENOTCONN);
    }

    let name = if require_connected_peer {
        endpoint.peer_name()
    } else {
        endpoint.local_name()
    };
    match write_sockaddr_un(
        guest_memory_reader,
        guest_memory_writer,
        request.argument(1),
        request.argument(2),
        name,
    ) {
        Ok(()) => 0,
        Err(errno) => linux_error(errno),
    }
}

pub(super) fn syscall_shutdown(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.guest_fd_is_socket(fd) {
        Ok(true) => {}
        Ok(false) => return linux_error(RISCV_LINUX_ENOTSOCK),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }
    let Some(how) = RiscvGuestSocketShutdown::from_linux_how(request.argument(1)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    match state.shutdown_guest_socket(fd, how) {
        Ok(true) => 0,
        Ok(false) | Err(_) => linux_error(RISCV_LINUX_EBADF),
    }
}

pub(super) fn syscall_setsockopt(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.guest_socket_endpoint(fd) {
        Ok(Some(_)) => {}
        Ok(None) => return linux_error(RISCV_LINUX_ENOTSOCK),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }
    let option =
        match RiscvGuestSocketOption::from_setsockopt(request.argument(1), request.argument(2)) {
            Ok(option) => option,
            Err(errno) => return linux_error(errno),
        };
    let Ok(optlen) = usize::try_from(request.argument(4)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if optlen < RISCV_LINUX_INT_OPTION_BYTES {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(bytes) = guest_memory.read(request.argument(3), RISCV_LINUX_INT_OPTION_BYTES) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    let Ok(value_bytes) = <[u8; RISCV_LINUX_INT_OPTION_BYTES]>::try_from(bytes.as_slice()) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    match option {
        RiscvGuestSocketOption::ReuseAddr => {
            let enabled = i32::from_le_bytes(value_bytes) != 0;
            match state.set_guest_socket_reuse_addr(fd, enabled) {
                Ok(true) => 0,
                Ok(false) => linux_error(RISCV_LINUX_ENOTSOCK),
                Err(_) => linux_error(RISCV_LINUX_EBADF),
            }
        }
        RiscvGuestSocketOption::Type | RiscvGuestSocketOption::Error => {
            linux_error(RISCV_LINUX_ENOPROTOOPT)
        }
    }
}

pub(super) fn syscall_getsockopt(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let endpoint = match state.guest_socket_endpoint(fd) {
        Ok(Some(endpoint)) => endpoint,
        Ok(None) => return linux_error(RISCV_LINUX_ENOTSOCK),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    let Some(optlen_bytes) = guest_memory_reader.read(request.argument(4), 4) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    let Ok(optlen_bytes) = <[u8; 4]>::try_from(optlen_bytes.as_slice()) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    let requested_len = i32::from_le_bytes(optlen_bytes);
    if requested_len < 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let option =
        match RiscvGuestSocketOption::from_getsockopt(request.argument(1), request.argument(2)) {
            Ok(option) => option,
            Err(errno) => return linux_error(errno),
        };
    let value = match option {
        RiscvGuestSocketOption::ReuseAddr => {
            if endpoint.reuse_addr {
                1
            } else {
                0
            }
        }
        RiscvGuestSocketOption::Type => RISCV_LINUX_SOCK_STREAM as i32,
        RiscvGuestSocketOption::Error => 0,
    };
    let value_bytes = value.to_le_bytes();
    let returned_len = (requested_len as u32).min(RISCV_LINUX_INT_OPTION_BYTES as u32);
    let write_len = returned_len as usize;
    if write_len > 0 && !guest_memory_writer.write(request.argument(3), &value_bytes[..write_len]) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    if !guest_memory_writer.write(request.argument(4), &returned_len.to_le_bytes()) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    0
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
            | Ok(write @ RiscvGuestSocketWrite::BrokenPipe)
            | Ok(write @ RiscvGuestSocketWrite::NotConnected) => return socket_write_result(write),
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
        Ok(RiscvGuestSocketRead::NotConnected) => Some(linux_error(RISCV_LINUX_EINVAL)),
        Ok(RiscvGuestSocketRead::NotSocket) => Some(linux_error(RISCV_LINUX_ENOTSOCK)),
        Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}

pub(super) fn syscall_sendmsg(
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
    let flags = request.argument(2);
    if flags & !RISCV_LINUX_SENDMSG_ALLOWED_FLAGS != 0 {
        return Some(linux_error(RISCV_LINUX_ENOTSUP));
    }

    let msg = match RiscvLinuxMsgHdr::read(guest_memory, request.argument(1)) {
        Ok(msg) => msg,
        Err(errno) => return Some(linux_error(errno)),
    };
    let (iovecs, total) = match read_iovecs(guest_memory, msg.iov, msg.iovlen) {
        Ok(iovecs) => iovecs,
        Err(errno) => return Some(linux_error(errno)),
    };
    let Ok(byte_count) = usize::try_from(total) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let force_nonblocking = flags & RISCV_LINUX_MSG_DONTWAIT != 0;
    let written =
        match state.guest_socket_write_plan_with_nonblocking(fd, byte_count, force_nonblocking) {
            Ok(RiscvGuestSocketWrite::NotSocket) => return Some(linux_error(RISCV_LINUX_ENOTSOCK)),
            Ok(RiscvGuestSocketWrite::Written(written)) => written,
            Ok(write @ RiscvGuestSocketWrite::WouldBlock)
            | Ok(write @ RiscvGuestSocketWrite::Blocked)
            | Ok(write @ RiscvGuestSocketWrite::BrokenPipe)
            | Ok(write @ RiscvGuestSocketWrite::NotConnected) => return socket_write_result(write),
            Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
        };
    if written == 0 {
        return Some(0);
    }
    let Some(bytes) = read_iovec_prefix(guest_memory, &iovecs, written) else {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    };
    match state.write_guest_socket_from_fd(fd, &bytes) {
        Ok(write) => socket_write_result(write),
        Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}

pub(super) fn syscall_recvmsg(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
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
    let flags = request.argument(2);
    if flags & !RISCV_LINUX_RECVMSG_ALLOWED_FLAGS != 0 {
        return Some(linux_error(RISCV_LINUX_ENOTSUP));
    }

    let msg = match RiscvLinuxMsgHdr::read(guest_memory_reader, request.argument(1)) {
        Ok(msg) => msg,
        Err(errno) => return Some(linux_error(errno)),
    };
    let (iovecs, total) = match read_iovecs(guest_memory_reader, msg.iov, msg.iovlen) {
        Ok(iovecs) => iovecs,
        Err(errno) => return Some(linux_error(errno)),
    };
    let Ok(byte_count) = usize::try_from(total) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let force_nonblocking = flags & RISCV_LINUX_MSG_DONTWAIT != 0;
    match state.guest_socket_read_with_nonblocking(fd, byte_count, force_nonblocking) {
        Ok(RiscvGuestSocketRead::Bytes(bytes)) => {
            if !bytes.is_empty() && !write_iovecs(guest_memory_writer, &iovecs, &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            let Some(flags_address) = request
                .argument(1)
                .checked_add(RISCV_LINUX_MSGHDR_FLAGS_OFFSET as u64)
            else {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            };
            if !guest_memory_writer.write(flags_address, &0_i32.to_le_bytes()) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_socket_read(fd, bytes.len()).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            Some(bytes.len() as u64)
        }
        Ok(RiscvGuestSocketRead::WouldBlock) => Some(linux_error(RISCV_LINUX_EAGAIN)),
        Ok(RiscvGuestSocketRead::Blocked) => None,
        Ok(RiscvGuestSocketRead::NotConnected) => Some(linux_error(RISCV_LINUX_EINVAL)),
        Ok(RiscvGuestSocketRead::NotSocket) => Some(linux_error(RISCV_LINUX_ENOTSOCK)),
        Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}

pub(super) fn socket_write_result(write: RiscvGuestSocketWrite) -> Option<u64> {
    match write {
        RiscvGuestSocketWrite::Written(written) => Some(written as u64),
        RiscvGuestSocketWrite::WouldBlock => Some(linux_error(RISCV_LINUX_EAGAIN)),
        RiscvGuestSocketWrite::Blocked => None,
        RiscvGuestSocketWrite::NotConnected => Some(linux_error(RISCV_LINUX_ENOTCONN)),
        RiscvGuestSocketWrite::BrokenPipe => Some(linux_error(RISCV_LINUX_EPIPE)),
        RiscvGuestSocketWrite::NotSocket => Some(linux_error(RISCV_LINUX_EBADF)),
    }
}
