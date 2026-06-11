use std::{
    collections::{BTreeSet, VecDeque},
    fmt,
    sync::{Arc, Mutex},
};

use rem6_cpu::{CpuId, RiscvCore};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::{PartitionedScheduler, Tick};

use crate::{
    GuestEventId, GuestFd, GuestFdEntry, GuestFdError, GuestFdTable, GuestFileDescription,
    GuestFileDescriptionId, GuestFileStatusFlags, GuestFutexAddress, GuestFutexTable,
    GuestThreadGroupId, RiscvSystemRunDriver, ScheduledRiscvTrap, SystemError,
};

const RISCV_LINUX_DUP: u64 = 23;
const RISCV_LINUX_DUP3: u64 = 24;
const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_OPENAT: u64 = 56;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_SET_TID_ADDRESS: u64 = 96;
const RISCV_LINUX_SET_ROBUST_LIST: u64 = 99;
const RISCV_LINUX_GET_ROBUST_LIST: u64 = 100;
const RISCV_LINUX_NANOSLEEP: u64 = 101;
const RISCV_LINUX_SCHED_YIELD: u64 = 124;
const RISCV_LINUX_RT_SIGSUSPEND: u64 = 133;
const RISCV_LINUX_RT_SIGACTION: u64 = 134;
const RISCV_LINUX_RT_SIGPROCMASK: u64 = 135;
const RISCV_LINUX_RT_SIGPENDING: u64 = 136;
const RISCV_LINUX_RT_SIGTIMEDWAIT: u64 = 137;
const RISCV_LINUX_RT_SIGQUEUEINFO: u64 = 138;
const RISCV_LINUX_RT_SIGRETURN: u64 = 139;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_EXIT_GROUP: u64 = 94;
const RISCV_LINUX_FUTEX: u64 = 98;
const RISCV_LINUX_GETPID: u64 = 172;
const RISCV_LINUX_GETPPID: u64 = 173;
const RISCV_LINUX_GETUID: u64 = 174;
const RISCV_LINUX_GETEUID: u64 = 175;
const RISCV_LINUX_GETGID: u64 = 176;
const RISCV_LINUX_GETEGID: u64 = 177;
const RISCV_LINUX_GETTID: u64 = 178;
const RISCV_LINUX_BRK: u64 = 214;
const RISCV_LINUX_MUNMAP: u64 = 215;
const RISCV_LINUX_MMAP: u64 = 222;
const RISCV_LINUX_MPROTECT: u64 = 226;
const RISCV_LINUX_MSYNC: u64 = 227;
const RISCV_LINUX_MLOCK: u64 = 228;
const RISCV_LINUX_MUNLOCK: u64 = 229;
const RISCV_LINUX_MLOCKALL: u64 = 230;
const RISCV_LINUX_MUNLOCKALL: u64 = 231;
const RISCV_LINUX_MINCORE: u64 = 232;
const RISCV_LINUX_MADVISE: u64 = 233;
const RISCV_LINUX_MBIND: u64 = 235;
const RISCV_LINUX_RSEQ: u64 = 293;
const RISCV_LINUX_ENOENT: u64 = 2;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_EMFILE: u64 = 24;
const RISCV_LINUX_ENAMETOOLONG: u64 = 36;
const RISCV_LINUX_ENOSYS: u64 = 38;
const RISCV_LINUX_FUTEX_WAKE: u32 = 1;
const RISCV_LINUX_FUTEX_WAKE_BITSET: u32 = 10;
const RISCV_LINUX_FUTEX_PRIVATE_FLAG: u32 = 128;
const RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG: u32 = 256;
const RISCV_LINUX_F_GETFD: u64 = 1;
const RISCV_LINUX_F_SETFD: u64 = 2;
const RISCV_LINUX_F_GETFL: u64 = 3;
const RISCV_LINUX_F_SETFL: u64 = 4;
const RISCV_LINUX_FD_CLOEXEC: u64 = 1;
const RISCV_LINUX_O_ACCMODE: u64 = 0x3;
const RISCV_LINUX_O_CLOEXEC: u64 = 0o2_000_000;
const RISCV_LINUX_O_RDONLY: u64 = 0;
const RISCV_LINUX_O_WRONLY: u64 = 1;
#[cfg(test)]
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_MAP_SHARED: u64 = 0x01;
const RISCV_LINUX_MAP_PRIVATE: u64 = 0x02;
const RISCV_LINUX_MAP_FIXED: u64 = 0x10;
const RISCV_LINUX_MAP_ANONYMOUS: u64 = 0x20;
const RISCV_LINUX_AT_FDCWD: u64 = (-100_i64) as u64;
const RISCV_LINUX_PATH_MAX: usize = 4096;
const RISCV_PAGE_BYTES: u64 = 4096;
const RISCV64_LINUX_MMAP_BASE: u64 = 0x4000_0000_0000_0000;
const RISCV_LINUX_DEFAULT_PROCESS_ID: u64 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvSyscallRequest {
    pc: u64,
    number: u64,
    arguments: [u64; 6],
}

impl RiscvSyscallRequest {
    pub const fn new(pc: u64, number: u64, arguments: [u64; 6]) -> Self {
        Self {
            pc,
            number,
            arguments,
        }
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }

    pub const fn number(self) -> u64 {
        self.number
    }

    pub const fn arguments(self) -> [u64; 6] {
        self.arguments
    }

    pub const fn argument(self, index: usize) -> u64 {
        self.arguments[index]
    }

    pub fn from_pending_core_trap(core: &RiscvCore) -> Option<Self> {
        let trap = core.pending_trap()?;
        if !matches!(trap.kind(), RiscvTrapKind::EnvironmentCall) {
            return None;
        }
        if core.pending_trap_return_privilege_mode()? != RiscvPrivilegeMode::User {
            return None;
        }

        Some(Self::new(
            trap.pc(),
            core.read_register(register(17)),
            [
                core.read_register(register(10)),
                core.read_register(register(11)),
                core.read_register(register(12)),
                core.read_register(register(13)),
                core.read_register(register(14)),
                core.read_register(register(15)),
            ],
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSyscallOutcome {
    Exit { code: i32 },
    Return { value: u64 },
}

type RiscvGuestMemoryReadFn = dyn Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static;

#[derive(Clone)]
pub struct RiscvGuestMemoryReader {
    read: Arc<RiscvGuestMemoryReadFn>,
}

impl RiscvGuestMemoryReader {
    pub fn new<F>(read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        Self {
            read: Arc::new(read),
        }
    }

    fn read(&self, address: u64, bytes: usize) -> Option<Vec<u8>> {
        (self.read)(address, bytes)
    }
}

impl fmt::Debug for RiscvGuestMemoryReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RiscvGuestMemoryReader")
            .finish_non_exhaustive()
    }
}

type RiscvGuestMemoryWriteFn = dyn Fn(u64, &[u8]) -> bool + Send + Sync + 'static;

#[derive(Clone)]
pub struct RiscvGuestMemoryWriter {
    write: Arc<RiscvGuestMemoryWriteFn>,
}

impl RiscvGuestMemoryWriter {
    pub fn new<F>(write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        Self {
            write: Arc::new(write),
        }
    }

    fn write(&self, address: u64, bytes: &[u8]) -> bool {
        (self.write)(address, bytes)
    }
}

impl fmt::Debug for RiscvGuestMemoryWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RiscvGuestMemoryWriter")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvMmapRegion {
    start: u64,
    length: u64,
    protection: u64,
    flags: u64,
    fd: u64,
    offset: u64,
}

impl RiscvMmapRegion {
    pub const fn new(
        start: u64,
        length: u64,
        protection: u64,
        flags: u64,
        fd: u64,
        offset: u64,
    ) -> Self {
        Self {
            start,
            length,
            protection,
            flags,
            fd,
            offset,
        }
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn length(self) -> u64 {
        self.length
    }

    pub const fn protection(self) -> u64 {
        self.protection
    }

    pub const fn flags(self) -> u64 {
        self.flags
    }

    pub const fn fd(self) -> u64 {
        self.fd
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvGuestWriteRecord {
    fd: GuestFd,
    address: u64,
    tick: Tick,
    bytes: Vec<u8>,
}

impl RiscvGuestWriteRecord {
    pub fn new(fd: GuestFd, address: u64, tick: Tick, bytes: Vec<u8>) -> Self {
        Self {
            fd,
            address,
            tick,
            bytes,
        }
    }

    pub const fn fd(&self) -> GuestFd {
        self.fd
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvGuestOpenRecord {
    fd: GuestFd,
    dirfd: u64,
    path: Vec<u8>,
    flags: u64,
    mode: u64,
}

impl RiscvGuestOpenRecord {
    pub fn new(fd: GuestFd, dirfd: u64, path: Vec<u8>, flags: u64, mode: u64) -> Self {
        Self {
            fd,
            dirfd,
            path,
            flags,
            mode,
        }
    }

    pub const fn fd(&self) -> GuestFd {
        self.fd
    }

    pub const fn dirfd(&self) -> u64 {
        self.dirfd
    }

    pub fn path(&self) -> &[u8] {
        &self.path
    }

    pub const fn flags(&self) -> u64 {
        self.flags
    }

    pub const fn mode(&self) -> u64 {
        self.mode
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSyscallState {
    identity: RiscvSyscallIdentity,
    child_clear_tid: Option<u64>,
    guest_fds: GuestFdTable,
    guest_futexes: GuestFutexTable,
    guest_paths: BTreeSet<Vec<u8>>,
    guest_opens: Vec<RiscvGuestOpenRecord>,
    stdin_fds: BTreeSet<GuestFd>,
    guest_writes: Vec<RiscvGuestWriteRecord>,
    stdin: VecDeque<u8>,
    program_break: u64,
    mmap_next: u64,
    mmap_regions: Vec<RiscvMmapRegion>,
}

impl RiscvSyscallState {
    pub fn new(program_break: u64) -> Self {
        Self::with_mmap_base(program_break, RISCV64_LINUX_MMAP_BASE)
    }

    pub fn with_mmap_base(program_break: u64, mmap_next: u64) -> Self {
        Self::with_identity_and_mmap_base(
            program_break,
            RiscvSyscallIdentity::linux_single_process(),
            mmap_next,
        )
    }

    #[cfg(test)]
    fn with_identity(program_break: u64, identity: RiscvSyscallIdentity) -> Self {
        Self::with_identity_and_mmap_base(program_break, identity, RISCV64_LINUX_MMAP_BASE)
    }

    fn with_identity_and_mmap_base(
        program_break: u64,
        identity: RiscvSyscallIdentity,
        mmap_next: u64,
    ) -> Self {
        let mut stdin_fds = BTreeSet::new();
        stdin_fds.insert(GuestFd::new(0).expect("standard stdin fd is non-negative"));
        Self {
            identity,
            child_clear_tid: None,
            guest_fds: linux_standard_guest_fds(),
            guest_futexes: GuestFutexTable::new(),
            guest_paths: BTreeSet::new(),
            guest_opens: Vec::new(),
            stdin_fds,
            guest_writes: Vec::new(),
            stdin: VecDeque::new(),
            program_break,
            mmap_next,
            mmap_regions: Vec::new(),
        }
    }

    const fn identity(&self) -> RiscvSyscallIdentity {
        self.identity
    }

    pub const fn child_clear_tid(&self) -> Option<u64> {
        self.child_clear_tid
    }

    pub const fn guest_fds(&self) -> &GuestFdTable {
        &self.guest_fds
    }

    pub const fn guest_futexes(&self) -> &GuestFutexTable {
        &self.guest_futexes
    }

    pub fn guest_writes(&self) -> &[RiscvGuestWriteRecord] {
        &self.guest_writes
    }

    pub fn guest_opens(&self) -> &[RiscvGuestOpenRecord] {
        &self.guest_opens
    }

    pub fn register_guest_path(&mut self, path: impl AsRef<[u8]>) {
        self.guest_paths.insert(path.as_ref().to_vec());
    }

    pub fn push_stdin_bytes(&mut self, bytes: &[u8]) {
        self.stdin.extend(bytes.iter().copied());
    }

    pub fn stdin_byte_count(&self) -> usize {
        self.stdin.len()
    }

    #[cfg(test)]
    fn guest_futexes_mut(&mut self) -> &mut GuestFutexTable {
        &mut self.guest_futexes
    }

    pub const fn program_break(&self) -> u64 {
        self.program_break
    }

    pub const fn mmap_next(&self) -> u64 {
        self.mmap_next
    }

    pub fn mmap_regions(&self) -> &[RiscvMmapRegion] {
        &self.mmap_regions
    }

    fn set_program_break(&mut self, value: u64) {
        self.program_break = value;
    }

    fn set_child_clear_tid(&mut self, value: u64) {
        self.child_clear_tid = (value != 0).then_some(value);
    }

    fn is_mmap_range_available(&self, start: u64, length: u64) -> bool {
        start.checked_add(length).is_some_and(|_| {
            self.mmap_regions
                .iter()
                .all(|region| !region.overlaps(start, length))
        })
    }

    fn unmap_mmap_range(&mut self, start: u64, length: u64) {
        let mut regions = Vec::with_capacity(self.mmap_regions.len());
        for region in self.mmap_regions.drain(..) {
            region.push_fragments_after_unmap(start, length, &mut regions);
        }
        self.mmap_regions = regions;
    }

    fn extend_mmap(&mut self, length: u64) -> Option<u64> {
        let mut start = self.mmap_next;
        while !self.is_mmap_range_available(start, length) {
            start = start.checked_add(RISCV_PAGE_BYTES)?;
        }
        self.mmap_next = start.checked_add(length)?;
        Some(start)
    }

    fn push_mmap_region(&mut self, region: RiscvMmapRegion) {
        self.mmap_regions.push(region);
        self.mmap_regions.sort_by_key(|region| region.start());
    }

    fn push_guest_write(&mut self, write: RiscvGuestWriteRecord) {
        self.guest_writes.push(write);
    }

    fn guest_path_registered(&self, path: &[u8]) -> bool {
        self.guest_paths.contains(path)
    }

    fn open_guest_path(
        &mut self,
        dirfd: u64,
        path: Vec<u8>,
        flags: u64,
        mode: u64,
        status_flags: GuestFileStatusFlags,
        close_on_exec: bool,
    ) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_open_fd()?;
        let description = self.next_open_description()?;
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                status_flags,
            ))?;
        self.guest_fds.insert(
            fd,
            GuestFdEntry::new(description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_opens
            .push(RiscvGuestOpenRecord::new(fd, dirfd, path, flags, mode));
        Ok(fd)
    }

    fn stdin_readable(&self, fd: GuestFd) -> bool {
        self.stdin_fds.contains(&fd)
    }

    fn close_fd_source(&mut self, fd: GuestFd) {
        self.stdin_fds.remove(&fd);
    }

    fn duplicate_fd_source(&mut self, old_fd: GuestFd, new_fd: GuestFd) {
        if self.stdin_fds.contains(&old_fd) {
            self.stdin_fds.insert(new_fd);
        } else {
            self.stdin_fds.remove(&new_fd);
        }
    }

    fn next_open_fd(&self) -> Result<GuestFd, GuestFdError> {
        let snapshot = self.guest_fds.snapshot();
        let mut candidate = 0_i32;
        loop {
            let fd = GuestFd::new(candidate)?;
            if snapshot.entries().iter().all(|entry| entry.fd() != fd) {
                return Ok(fd);
            }
            candidate = candidate
                .checked_add(1)
                .ok_or(GuestFdError::FdSpaceExhausted)?;
        }
    }

    fn next_open_description(&self) -> Result<GuestFileDescriptionId, GuestFdError> {
        let mut candidate = 0_u64;
        loop {
            let description = GuestFileDescriptionId::new(candidate);
            if self.guest_fds.description(description).is_none() {
                return Ok(description);
            }
            candidate = candidate
                .checked_add(1)
                .ok_or(GuestFdError::FdSpaceExhausted)?;
        }
    }

    fn stdin_prefix(&self, count: usize) -> Vec<u8> {
        self.stdin.iter().take(count).copied().collect()
    }

    fn consume_stdin_prefix(&mut self, count: usize) {
        self.stdin.drain(..count);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvSyscallIdentity {
    thread_group_id: u64,
    thread_id: u64,
    parent_process_id: u64,
    user_id: u64,
    effective_user_id: u64,
    group_id: u64,
    effective_group_id: u64,
}

impl RiscvSyscallIdentity {
    const fn new(
        thread_group_id: u64,
        thread_id: u64,
        parent_process_id: u64,
        user_id: u64,
        effective_user_id: u64,
        group_id: u64,
        effective_group_id: u64,
    ) -> Self {
        Self {
            thread_group_id,
            thread_id,
            parent_process_id,
            user_id,
            effective_user_id,
            group_id,
            effective_group_id,
        }
    }

    const fn linux_single_process() -> Self {
        Self::new(
            RISCV_LINUX_DEFAULT_PROCESS_ID,
            RISCV_LINUX_DEFAULT_PROCESS_ID,
            0,
            RISCV_LINUX_DEFAULT_PROCESS_ID,
            RISCV_LINUX_DEFAULT_PROCESS_ID,
            RISCV_LINUX_DEFAULT_PROCESS_ID,
            RISCV_LINUX_DEFAULT_PROCESS_ID,
        )
    }

    const fn thread_group_id(self) -> u64 {
        self.thread_group_id
    }

    const fn thread_id(self) -> u64 {
        self.thread_id
    }

    const fn parent_process_id(self) -> u64 {
        self.parent_process_id
    }

    const fn user_id(self) -> u64 {
        self.user_id
    }

    const fn effective_user_id(self) -> u64 {
        self.effective_user_id
    }

    const fn group_id(self) -> u64 {
        self.group_id
    }

    const fn effective_group_id(self) -> u64 {
        self.effective_group_id
    }
}

impl RiscvMmapRegion {
    fn overlaps(self, start: u64, length: u64) -> bool {
        let Some(end) = start.checked_add(length) else {
            return true;
        };
        let Some(region_end) = self.start.checked_add(self.length) else {
            return true;
        };
        start < region_end && self.start < end
    }

    fn push_fragments_after_unmap(self, start: u64, length: u64, output: &mut Vec<Self>) {
        let Some(end) = start.checked_add(length) else {
            return;
        };
        let Some(region_end) = self.start.checked_add(self.length) else {
            return;
        };
        if start >= region_end || self.start >= end {
            output.push(self);
            return;
        }
        if self.start < start {
            output.push(Self::new(
                self.start,
                start - self.start,
                self.protection,
                self.flags,
                self.fd,
                self.offset,
            ));
        }
        if end < region_end {
            let delta = end - self.start;
            output.push(Self::new(
                end,
                region_end - end,
                self.protection,
                self.flags,
                self.fd,
                self.offset.saturating_add(delta),
            ));
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvSyscallTable;

impl RiscvSyscallTable {
    pub const fn new() -> Self {
        Self
    }

    pub fn handle(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
    ) -> Option<RiscvSyscallOutcome> {
        self.handle_at_tick(request, state, 0)
    }

    pub fn handle_at_tick(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
        tick: Tick,
    ) -> Option<RiscvSyscallOutcome> {
        self.handle_with_guest_memory_at_tick(request, state, tick, None)
    }

    pub fn handle_with_guest_memory_at_tick(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
        tick: Tick,
        guest_memory: Option<&RiscvGuestMemoryReader>,
    ) -> Option<RiscvSyscallOutcome> {
        self.handle_with_guest_memory_io_at_tick(request, state, tick, guest_memory, None)
    }

    pub fn handle_with_guest_memory_io_at_tick(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
        tick: Tick,
        guest_memory_reader: Option<&RiscvGuestMemoryReader>,
        guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
    ) -> Option<RiscvSyscallOutcome> {
        match request.number() {
            RISCV_LINUX_DUP => Some(RiscvSyscallOutcome::Return {
                value: syscall_dup(request.argument(0), state),
            }),
            RISCV_LINUX_DUP3 => Some(RiscvSyscallOutcome::Return {
                value: syscall_dup3(
                    request.argument(0),
                    request.argument(1),
                    request.argument(2),
                    state,
                ),
            }),
            RISCV_LINUX_FCNTL => syscall_fcntl(request, state),
            RISCV_LINUX_OPENAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_openat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_CLOSE => Some(RiscvSyscallOutcome::Return {
                value: syscall_close(request.argument(0), state),
            }),
            RISCV_LINUX_READ => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_read(request, state, guest_memory),
                })
            }
            RISCV_LINUX_WRITE => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_write(request, state, tick, guest_memory),
                })
            }
            RISCV_LINUX_SET_TID_ADDRESS => Some(RiscvSyscallOutcome::Return {
                value: syscall_set_tid_address(request.argument(0), state),
            }),
            RISCV_LINUX_FUTEX => syscall_futex(request, state, tick),
            RISCV_LINUX_SET_ROBUST_LIST
            | RISCV_LINUX_GET_ROBUST_LIST
            | RISCV_LINUX_NANOSLEEP
            | RISCV_LINUX_SCHED_YIELD
            | RISCV_LINUX_RT_SIGSUSPEND
            | RISCV_LINUX_RT_SIGACTION
            | RISCV_LINUX_RT_SIGPROCMASK
            | RISCV_LINUX_RT_SIGPENDING
            | RISCV_LINUX_RT_SIGTIMEDWAIT
            | RISCV_LINUX_RT_SIGQUEUEINFO
            | RISCV_LINUX_RT_SIGRETURN => Some(RiscvSyscallOutcome::Return { value: 0 }),
            RISCV_LINUX_MPROTECT
            | RISCV_LINUX_MSYNC
            | RISCV_LINUX_MLOCK
            | RISCV_LINUX_MUNLOCK
            | RISCV_LINUX_MLOCKALL
            | RISCV_LINUX_MUNLOCKALL
            | RISCV_LINUX_MINCORE
            | RISCV_LINUX_MADVISE
            | RISCV_LINUX_MBIND => Some(RiscvSyscallOutcome::Return { value: 0 }),
            RISCV_LINUX_RSEQ => Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_ENOSYS),
            }),
            RISCV_LINUX_EXIT | RISCV_LINUX_EXIT_GROUP => Some(RiscvSyscallOutcome::Exit {
                code: syscall_exit_code(request.argument(0)),
            }),
            RISCV_LINUX_GETPID => Some(RiscvSyscallOutcome::Return {
                value: state.identity().thread_group_id(),
            }),
            RISCV_LINUX_GETPPID => Some(RiscvSyscallOutcome::Return {
                value: state.identity().parent_process_id(),
            }),
            RISCV_LINUX_GETTID => Some(RiscvSyscallOutcome::Return {
                value: state.identity().thread_id(),
            }),
            RISCV_LINUX_GETUID => Some(RiscvSyscallOutcome::Return {
                value: state.identity().user_id(),
            }),
            RISCV_LINUX_GETEUID => Some(RiscvSyscallOutcome::Return {
                value: state.identity().effective_user_id(),
            }),
            RISCV_LINUX_GETGID => Some(RiscvSyscallOutcome::Return {
                value: state.identity().group_id(),
            }),
            RISCV_LINUX_GETEGID => Some(RiscvSyscallOutcome::Return {
                value: state.identity().effective_group_id(),
            }),
            RISCV_LINUX_BRK => Some(RiscvSyscallOutcome::Return {
                value: syscall_brk(request.argument(0), state),
            }),
            RISCV_LINUX_MMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_mmap(request, state),
            }),
            RISCV_LINUX_MUNMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_munmap(request.argument(0), request.argument(1), state),
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RiscvSyscallEmulation {
    table: RiscvSyscallTable,
    state: Arc<Mutex<RiscvSyscallState>>,
    guest_memory_reader: Option<RiscvGuestMemoryReader>,
    guest_memory_writer: Option<RiscvGuestMemoryWriter>,
}

impl RiscvSyscallEmulation {
    pub fn new(table: RiscvSyscallTable, state: RiscvSyscallState) -> Self {
        Self {
            table,
            state: Arc::new(Mutex::new(state)),
            guest_memory_reader: None,
            guest_memory_writer: None,
        }
    }

    pub fn linux_user() -> Self {
        Self::new(RiscvSyscallTable::new(), RiscvSyscallState::new(0))
    }

    pub const fn table(&self) -> RiscvSyscallTable {
        self.table
    }

    pub fn with_guest_memory_reader<F>(mut self, read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        self.guest_memory_reader = Some(RiscvGuestMemoryReader::new(read));
        self
    }

    pub fn with_guest_memory_writer<F>(mut self, write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        self.guest_memory_writer = Some(RiscvGuestMemoryWriter::new(write));
        self
    }

    pub fn state(&self) -> RiscvSyscallState {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .clone()
    }

    pub fn push_stdin_bytes(&self, bytes: &[u8]) {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .push_stdin_bytes(bytes);
    }

    pub fn register_guest_path(&self, path: impl AsRef<[u8]>) {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .register_guest_path(path);
    }

    pub fn handle_pending_core_trap(
        &self,
        core: &RiscvCore,
        tick: Tick,
    ) -> Option<RiscvSyscallOutcome> {
        let request = RiscvSyscallRequest::from_pending_core_trap(core)?;
        let mut state = self.state.lock().expect("RISC-V syscall state lock");
        self.table.handle_with_guest_memory_io_at_tick(
            request,
            &mut state,
            tick,
            self.guest_memory_reader.as_ref(),
            self.guest_memory_writer.as_ref(),
        )
    }
}

impl Default for RiscvSyscallEmulation {
    fn default() -> Self {
        Self::linux_user()
    }
}

impl RiscvSystemRunDriver {
    pub fn with_riscv_syscall_emulation(mut self) -> Self {
        self.riscv_syscall_emulation = Some(RiscvSyscallEmulation::linux_user());
        self
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_reader<F>(mut self, read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        self.riscv_syscall_emulation =
            Some(RiscvSyscallEmulation::linux_user().with_guest_memory_reader(read));
        self
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_writer<F>(mut self, write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        self.riscv_syscall_emulation =
            Some(RiscvSyscallEmulation::linux_user().with_guest_memory_writer(write));
        self
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_io<R, W>(
        mut self,
        read: R,
        write: W,
    ) -> Self
    where
        R: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        self.riscv_syscall_emulation = Some(
            RiscvSyscallEmulation::linux_user()
                .with_guest_memory_reader(read)
                .with_guest_memory_writer(write),
        );
        self
    }

    pub const fn riscv_syscall_emulation(&self) -> Option<&RiscvSyscallEmulation> {
        self.riscv_syscall_emulation.as_ref()
    }

    pub(crate) fn schedule_pending_core_events<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: Vec<RiscvCore>,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        if let Some(syscalls) = self.riscv_syscall_emulation.as_ref() {
            self.trap_port
                .schedule_pending_core_traps_with_syscall_emulation(
                    scheduler, cores, syscalls, event_for,
                )
        } else {
            self.trap_port
                .schedule_pending_core_traps(scheduler, cores, event_for)
        }
    }

    pub(crate) fn schedule_pending_core_events_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: Vec<RiscvCore>,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        if let Some(syscalls) = self.riscv_syscall_emulation.as_ref() {
            self.trap_port
                .schedule_pending_core_traps_with_syscall_emulation_parallel(
                    scheduler, cores, syscalls, event_for,
                )
        } else {
            self.trap_port
                .schedule_pending_core_traps_parallel(scheduler, cores, event_for)
        }
    }
}

fn register(index: u8) -> Register {
    Register::new(index).expect("valid RISC-V integer register")
}

fn syscall_exit_code(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}

fn syscall_brk(requested: u64, state: &mut RiscvSyscallState) -> u64 {
    if requested != 0 {
        state.set_program_break(requested);
    }
    state.program_break()
}

fn syscall_set_tid_address(clear_tid_address: u64, state: &mut RiscvSyscallState) -> u64 {
    state.set_child_clear_tid(clear_tid_address);
    state.identity().thread_id()
}

fn linux_standard_guest_fds() -> GuestFdTable {
    let mut table = GuestFdTable::new();
    for (fd, description, flags) in [
        (0, 0, RISCV_LINUX_O_RDONLY),
        (1, 1, RISCV_LINUX_O_WRONLY),
        (2, 2, RISCV_LINUX_O_WRONLY),
    ] {
        let description = GuestFileDescriptionId::new(description);
        table
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(flags as u32),
            ))
            .expect("standard RISC-V Linux file description is unique");
        table
            .insert(
                GuestFd::new(fd).expect("standard RISC-V Linux fd is non-negative"),
                GuestFdEntry::new(description),
            )
            .expect("standard RISC-V Linux fd is unique");
    }
    table
}

fn syscall_close(fd_argument: u64, state: &mut RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.guest_fds.close_descriptor(fd) {
        Ok(_record) => {
            state.close_fd_source(fd);
            0
        }
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}

fn syscall_openat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let dirfd = request.argument(0);
    if dirfd != RISCV_LINUX_AT_FDCWD {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let flags = request.argument(2);
    if flags & !(RISCV_LINUX_O_ACCMODE | RISCV_LINUX_O_CLOEXEC) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if flags & RISCV_LINUX_O_ACCMODE != RISCV_LINUX_O_RDONLY {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let path = match read_guest_c_string(guest_memory, request.argument(1), RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => {
            return linux_error(RISCV_LINUX_ENAMETOOLONG);
        }
    };
    if path.is_empty() || !state.guest_path_registered(&path) {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    let status_flags = GuestFileStatusFlags::new((flags & !RISCV_LINUX_O_CLOEXEC) as u32);
    let close_on_exec = flags & RISCV_LINUX_O_CLOEXEC != 0;
    match state.open_guest_path(
        dirfd,
        path,
        flags,
        request.argument(3),
        status_flags,
        close_on_exec,
    ) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}

fn syscall_read(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if !state.stdin_readable(fd) {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let count = request.argument(2);
    if count == 0 {
        return 0;
    }

    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let bytes = state.stdin_prefix(byte_count);
    if bytes.is_empty() {
        return 0;
    }
    let read_count = bytes.len() as u64;
    let Ok(offset) = state.guest_fds.file_offset(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if offset.get().checked_add(read_count).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    if !guest_memory.write(request.argument(1), &bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    if state.guest_fds.advance_file_offset(fd, read_count).is_err() {
        return linux_error(RISCV_LINUX_EBADF);
    }
    state.consume_stdin_prefix(bytes.len());
    read_count
}

fn syscall_write(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let count = request.argument(2);
    if count == 0 {
        return 0;
    }

    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let address = request.argument(1);
    let Some(bytes) = guest_memory.read(address, byte_count) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    if bytes.len() != byte_count {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    if state.guest_fds.advance_file_offset(fd, count).is_err() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    state.push_guest_write(RiscvGuestWriteRecord::new(fd, address, tick, bytes));
    count
}

fn syscall_dup(old_fd_argument: u64, state: &mut RiscvSyscallState) -> u64 {
    let Some(old_fd) = guest_fd_argument(old_fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.guest_fds.dup(old_fd) {
        Ok(new_fd) => {
            state.duplicate_fd_source(old_fd, new_fd);
            u64::from(new_fd.get())
        }
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}

fn syscall_dup3(
    old_fd_argument: u64,
    new_fd_argument: u64,
    flags: u64,
    state: &mut RiscvSyscallState,
) -> u64 {
    if flags & !RISCV_LINUX_O_CLOEXEC != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(old_fd) = guest_fd_argument(old_fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Some(new_fd) = guest_fd_argument(new_fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if old_fd == new_fd {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.guest_fds.dup2_with_replacement(old_fd, new_fd) {
        Ok(record) => {
            state.duplicate_fd_source(old_fd, record.fd());
            if flags & RISCV_LINUX_O_CLOEXEC != 0
                && state
                    .guest_fds
                    .set_close_on_exec(record.fd(), true)
                    .is_err()
            {
                return linux_error(RISCV_LINUX_EBADF);
            }
            u64::from(record.fd().get())
        }
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}

fn syscall_fcntl(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> Option<RiscvSyscallOutcome> {
    let command = request.argument(1);
    if !matches!(
        command,
        RISCV_LINUX_F_GETFD | RISCV_LINUX_F_SETFD | RISCV_LINUX_F_GETFL | RISCV_LINUX_F_SETFL
    ) {
        return None;
    }

    let fd = match guest_fd_argument(request.argument(0)) {
        Some(fd) => fd,
        None => return Some(guest_fd_error_return()),
    };

    let outcome = match command {
        RISCV_LINUX_F_GETFD => state
            .guest_fds
            .close_on_exec(fd)
            .map(|close| u64::from(close) * RISCV_LINUX_FD_CLOEXEC),
        RISCV_LINUX_F_SETFD => state
            .guest_fds
            .set_close_on_exec(fd, request.argument(2) & RISCV_LINUX_FD_CLOEXEC != 0)
            .map(|()| 0),
        RISCV_LINUX_F_GETFL => state
            .guest_fds
            .status_flags(fd)
            .map(|flags| u64::from(flags.bits())),
        RISCV_LINUX_F_SETFL => {
            let current = match state.guest_fds.status_flags(fd) {
                Ok(flags) => flags,
                Err(_error) => return Some(guest_fd_error_return()),
            };
            let access_mode = current.bits() & RISCV_LINUX_O_ACCMODE as u32;
            let requested = request.argument(2) as u32;
            state
                .guest_fds
                .set_status_flags(
                    fd,
                    GuestFileStatusFlags::new(
                        access_mode | (requested & !(RISCV_LINUX_O_ACCMODE as u32)),
                    ),
                )
                .map(|()| 0)
        }
        _ => return None,
    };

    Some(match outcome {
        Ok(value) => RiscvSyscallOutcome::Return { value },
        Err(_error) => guest_fd_error_return(),
    })
}

fn guest_fd_argument(value: u64) -> Option<GuestFd> {
    i32::try_from(value)
        .ok()
        .and_then(|fd| GuestFd::new(fd).ok())
}

fn guest_fd_error_return() -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return {
        value: linux_error(RISCV_LINUX_EBADF),
    }
}

fn syscall_futex(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
) -> Option<RiscvSyscallOutcome> {
    let op = (request.argument(1) as u32)
        & !(RISCV_LINUX_FUTEX_PRIVATE_FLAG | RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG);
    let address = GuestFutexAddress::new(request.argument(0));
    let thread_group = GuestThreadGroupId::new(state.identity().thread_group_id());
    match op {
        RISCV_LINUX_FUTEX_WAKE => {
            let count = futex_wake_count(request.argument(2));
            let outcome = state
                .guest_futexes
                .wake(address, thread_group, count, tick)
                .expect("guest futex wake cannot fail");
            Some(RiscvSyscallOutcome::Return {
                value: outcome.woken_count() as u64,
            })
        }
        RISCV_LINUX_FUTEX_WAKE_BITSET => {
            let bitset = request.argument(5) as u32;
            let outcome = state
                .guest_futexes
                .wake_bitset(address, thread_group, usize::MAX, bitset, tick)
                .expect("guest futex bitset wake cannot fail");
            Some(RiscvSyscallOutcome::Return {
                value: outcome.woken_count() as u64,
            })
        }
        _ => None,
    }
}

fn futex_wake_count(value: u64) -> usize {
    let count = value as i32;
    if count <= 0 {
        0
    } else {
        count as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestCStringError {
    Fault,
    TooLong,
}

fn read_guest_c_string(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
    limit: usize,
) -> Result<Vec<u8>, RiscvGuestCStringError> {
    let mut bytes = Vec::new();
    for offset in 0..limit {
        let address = address
            .checked_add(offset as u64)
            .ok_or(RiscvGuestCStringError::Fault)?;
        let byte = guest_memory
            .read(address, 1)
            .filter(|bytes| bytes.len() == 1)
            .and_then(|bytes| bytes.first().copied())
            .ok_or(RiscvGuestCStringError::Fault)?;
        if byte == 0 {
            return Ok(bytes);
        }
        bytes.push(byte);
    }
    Err(RiscvGuestCStringError::TooLong)
}

fn syscall_mmap(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let start = request.argument(0);
    let Some(length) = align_to_page(request.argument(1)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let protection = request.argument(2);
    let flags = request.argument(3);
    let fd = request.argument(4);
    let offset = request.argument(5);

    let shared = flags & RISCV_LINUX_MAP_SHARED != 0;
    let private = flags & RISCV_LINUX_MAP_PRIVATE != 0;
    if !start.is_multiple_of(RISCV_PAGE_BYTES)
        || !offset.is_multiple_of(RISCV_PAGE_BYTES)
        || shared == private
        || request.argument(1) == 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if start.checked_add(length).is_none() {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if flags & RISCV_LINUX_MAP_ANONYMOUS == 0 {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let fixed = flags & RISCV_LINUX_MAP_FIXED != 0;
    let mapped_start = if fixed {
        state.unmap_mmap_range(start, length);
        start
    } else if start != 0 && state.is_mmap_range_available(start, length) {
        start
    } else {
        match state.extend_mmap(length) {
            Some(start) => start,
            None => return linux_error(RISCV_LINUX_EINVAL),
        }
    };
    state.push_mmap_region(RiscvMmapRegion::new(
        mapped_start,
        length,
        protection,
        flags,
        fd,
        offset,
    ));
    mapped_start
}

fn syscall_munmap(start: u64, requested_length: u64, state: &mut RiscvSyscallState) -> u64 {
    let Some(length) = align_to_page(requested_length) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if !start.is_multiple_of(RISCV_PAGE_BYTES)
        || requested_length == 0
        || start.checked_add(length).is_none()
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    state.unmap_mmap_range(start, length);
    0
}

fn align_to_page(value: u64) -> Option<u64> {
    value
        .checked_add(RISCV_PAGE_BYTES - 1)
        .map(|rounded| rounded & !(RISCV_PAGE_BYTES - 1))
}

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

#[cfg(test)]
#[path = "riscv_syscall_tests.rs"]
mod tests;
