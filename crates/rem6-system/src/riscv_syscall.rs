use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
    sync::{Arc, Mutex},
};

use rem6_boot::BootImage;
use rem6_cpu::{CpuId, RiscvCore};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::{PartitionedScheduler, Tick};

use crate::{
    GuestEventId, GuestFd, GuestFdCloseRecord, GuestFdDup2Record, GuestFdEntry, GuestFdError,
    GuestFdTable, GuestFileDescription, GuestFileDescriptionId, GuestFileStatusFlags,
    GuestFutexTable, GuestWaitQueue, RiscvSystemRunDriver, ScheduledRiscvTrap, SystemError,
};

mod brk;
mod clock;
mod cwd;
mod futex;
mod guest_memory;
mod guest_write;
mod ioctl;
mod limits;
mod links;
mod mmap;
mod open;
mod readv;
mod robust;
mod seek;
mod startup;
mod stat;
mod unknown;
mod unlink;
mod utsname;
mod wait4;
mod writev;

use brk::syscall_brk;
use clock::{syscall_clock_gettime, syscall_gettimeofday};
use cwd::syscall_getcwd;
use futex::syscall_futex;
pub use guest_memory::{
    RiscvGuestMemoryMapRequest, RiscvGuestMemoryMapResult, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter,
};
pub use guest_write::RiscvGuestWriteRecord;
use ioctl::{syscall_ioctl, RISCV_LINUX_IOCTL};
pub use limits::RISCV_LINUX_STACK_LIMIT_BYTES;
use limits::{syscall_prlimit64, RISCV_LINUX_PRLIMIT64};
use links::syscall_readlinkat;
pub use mmap::RiscvMmapRegion;
use mmap::{syscall_mmap, syscall_munmap, RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES};
#[cfg(test)]
use mmap::{RISCV_LINUX_MAP_FIXED, RISCV_LINUX_MAP_PRIVATE};
use open::{syscall_open, syscall_openat, RISCV_LINUX_OPEN};
use readv::{syscall_readv, RISCV_LINUX_READV};
use robust::{syscall_get_robust_list, syscall_set_robust_list, RiscvRobustList};
use seek::{syscall_lseek, RISCV_LINUX_LSEEK};
pub use startup::{
    RiscvSeAuxvEntry, RiscvSeStartupConfig, RiscvSeStartupError, RiscvSeStartupImage,
    RiscvSeStartupStringField, RISCV_LINUX_AT_ENTRY, RISCV_LINUX_AT_NULL, RISCV_LINUX_AT_PAGESZ,
    RISCV_LINUX_AT_PHDR, RISCV_LINUX_AT_PHENT, RISCV_LINUX_AT_PHNUM, RISCV_LINUX_AT_RANDOM,
    RISCV_LINUX_AT_SECURE,
};
use stat::{guest_path_inode, write_riscv_linux_stat, RiscvGuestStat};
pub use unknown::RiscvUnknownSyscallRecord;
use unlink::{syscall_unlink, RISCV_LINUX_UNLINK};
use utsname::write_riscv_linux_utsname;
use wait4::{syscall_process_group_id, syscall_wait4, RISCV_LINUX_WAIT4};
use writev::{syscall_writev, RISCV_LINUX_WRITEV};

const RISCV_LINUX_GETCWD: u64 = 17;
const RISCV_LINUX_DUP: u64 = 23;
const RISCV_LINUX_DUP3: u64 = 24;
const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_OPENAT: u64 = 56;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_READLINKAT: u64 = 78;
const RISCV_LINUX_NEWFSTATAT: u64 = 79;
const RISCV_LINUX_FSTAT: u64 = 80;
const RISCV_LINUX_SET_TID_ADDRESS: u64 = 96;
const RISCV_LINUX_SET_ROBUST_LIST: u64 = 99;
const RISCV_LINUX_GET_ROBUST_LIST: u64 = 100;
const RISCV_LINUX_NANOSLEEP: u64 = 101;
const RISCV_LINUX_CLOCK_GETTIME: u64 = 113;
const RISCV_LINUX_SCHED_YIELD: u64 = 124;
const RISCV_LINUX_RT_SIGSUSPEND: u64 = 133;
const RISCV_LINUX_RT_SIGACTION: u64 = 134;
const RISCV_LINUX_RT_SIGPROCMASK: u64 = 135;
const RISCV_LINUX_RT_SIGPENDING: u64 = 136;
const RISCV_LINUX_RT_SIGTIMEDWAIT: u64 = 137;
const RISCV_LINUX_RT_SIGQUEUEINFO: u64 = 138;
const RISCV_LINUX_RT_SIGRETURN: u64 = 139;
const RISCV_LINUX_UNAME: u64 = 160;
const RISCV_LINUX_GETTIMEOFDAY: u64 = 169;
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
const RISCV_LINUX_GETRANDOM: u64 = 278;
const RISCV_LINUX_RSEQ: u64 = 293;
const RISCV_LINUX_STAT: u64 = 1038;
const RISCV_LINUX_EPERM: u64 = 1;
const RISCV_LINUX_ENOENT: u64 = 2;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_EMFILE: u64 = 24;
const RISCV_LINUX_ENOTTY: u64 = 25;
const RISCV_LINUX_ESPIPE: u64 = 29;
const RISCV_LINUX_ERANGE: u64 = 34;
const RISCV_LINUX_ENAMETOOLONG: u64 = 36;
const RISCV_LINUX_ENOSYS: u64 = 38;
const RISCV_LINUX_F_GETFD: u64 = 1;
const RISCV_LINUX_F_SETFD: u64 = 2;
const RISCV_LINUX_F_GETFL: u64 = 3;
const RISCV_LINUX_F_SETFL: u64 = 4;
const RISCV_LINUX_FD_CLOEXEC: u64 = 1;
const RISCV_LINUX_GRND_NONBLOCK: u64 = 0x0001;
const RISCV_LINUX_GRND_RANDOM: u64 = 0x0002;
const RISCV_LINUX_GRND_INSECURE: u64 = 0x0004;
const RISCV_LINUX_GRND_VALID_FLAGS: u64 =
    RISCV_LINUX_GRND_NONBLOCK | RISCV_LINUX_GRND_RANDOM | RISCV_LINUX_GRND_INSECURE;
const RISCV_LINUX_GETRANDOM_MAX_CHUNK_BYTES: u64 = 256;
const RISCV_LINUX_O_ACCMODE: u64 = 0x3;
const RISCV_LINUX_O_CLOEXEC: u64 = 0o2_000_000;
const RISCV_LINUX_O_RDONLY: u64 = 0;
const RISCV_LINUX_O_WRONLY: u64 = 1;
#[cfg(test)]
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_AT_FDCWD: u64 = (-100_i64) as u64;
const RISCV_LINUX_AT_EMPTY_PATH: u64 = 0x1000;
const RISCV_LINUX_AT_NO_AUTOMOUNT: u64 = 0x800;
const RISCV_LINUX_AT_SYMLINK_NOFOLLOW: u64 = 0x100;
const RISCV_LINUX_PATH_MAX: usize = 4096;
const RISCV_LINUX_DEFAULT_PROCESS_ID: u64 = 100;
const RISCV_LINUX_GETRANDOM_INITIAL_BYTE: u8 = 0x2b;

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

struct RiscvGuestOpenRequest {
    dirfd: u64,
    path: Vec<u8>,
    flags: u64,
    mode: u64,
    status_flags: GuestFileStatusFlags,
    close_on_exec: bool,
    file_contents: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSyscallState {
    identity: RiscvSyscallIdentity,
    current_directory: Vec<u8>,
    child_clear_tid: Option<u64>,
    robust_list: RiscvRobustList,
    guest_fds: GuestFdTable,
    guest_futexes: GuestFutexTable,
    guest_wait: GuestWaitQueue,
    guest_paths: BTreeSet<Vec<u8>>,
    guest_files: BTreeMap<Vec<u8>, Vec<u8>>,
    guest_links: BTreeMap<Vec<u8>, Vec<u8>>,
    guest_opens: Vec<RiscvGuestOpenRecord>,
    stdin_fds: BTreeSet<GuestFd>,
    guest_file_descriptions: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_file_stats: BTreeMap<GuestFileDescriptionId, RiscvGuestStat>,
    guest_writes: Vec<RiscvGuestWriteRecord>,
    unknown_syscalls: Vec<RiscvUnknownSyscallRecord>,
    stdin: VecDeque<u8>,
    getrandom_byte_counter: u8,
    program_break: u64,
    program_break_backing_end: u64,
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
        let current_process_group = syscall_process_group_id(identity);
        Self {
            identity,
            current_directory: b"/".to_vec(),
            child_clear_tid: None,
            robust_list: RiscvRobustList::new(0, 0),
            guest_fds: linux_standard_guest_fds(),
            guest_futexes: GuestFutexTable::new(),
            guest_wait: GuestWaitQueue::new(current_process_group),
            guest_paths: BTreeSet::new(),
            guest_files: BTreeMap::new(),
            guest_links: BTreeMap::new(),
            guest_opens: Vec::new(),
            stdin_fds,
            guest_file_descriptions: BTreeMap::new(),
            guest_file_stats: BTreeMap::new(),
            guest_writes: Vec::new(),
            unknown_syscalls: Vec::new(),
            stdin: VecDeque::new(),
            getrandom_byte_counter: 0,
            program_break,
            program_break_backing_end: program_break,
            mmap_next,
            mmap_regions: Vec::new(),
        }
    }

    const fn identity(&self) -> RiscvSyscallIdentity {
        self.identity
    }

    pub fn current_directory(&self) -> &[u8] {
        &self.current_directory
    }

    pub fn set_current_directory(&mut self, path: impl AsRef<[u8]>) {
        self.current_directory = path.as_ref().to_vec();
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

    pub fn unknown_syscalls(&self) -> &[RiscvUnknownSyscallRecord] {
        &self.unknown_syscalls
    }

    pub fn guest_opens(&self) -> &[RiscvGuestOpenRecord] {
        &self.guest_opens
    }

    pub fn register_guest_path(&mut self, path: impl AsRef<[u8]>) {
        self.guest_paths.insert(path.as_ref().to_vec());
    }

    pub fn register_guest_file(&mut self, path: impl AsRef<[u8]>, contents: impl AsRef<[u8]>) {
        let path = path.as_ref().to_vec();
        self.guest_paths.insert(path.clone());
        self.guest_files.insert(path, contents.as_ref().to_vec());
    }

    pub fn register_guest_symlink(&mut self, path: impl AsRef<[u8]>, target: impl AsRef<[u8]>) {
        self.guest_links
            .insert(path.as_ref().to_vec(), target.as_ref().to_vec());
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

    pub(super) const fn program_break_backing_end(&self) -> u64 {
        self.program_break_backing_end
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

    pub(super) fn set_program_break_backing_end(&mut self, value: u64) {
        self.program_break_backing_end = value;
    }

    fn set_child_clear_tid(&mut self, value: u64) {
        self.child_clear_tid = (value != 0).then_some(value);
    }

    pub(super) fn is_mmap_range_available(&self, start: u64, length: u64) -> bool {
        start.checked_add(length).is_some_and(|_| {
            self.mmap_regions
                .iter()
                .all(|region| !region.overlaps(start, length))
        })
    }

    pub(super) fn unmap_mmap_range(&mut self, start: u64, length: u64) {
        let mut regions = Vec::with_capacity(self.mmap_regions.len());
        for region in self.mmap_regions.drain(..) {
            region.push_fragments_after_unmap(start, length, &mut regions);
        }
        self.mmap_regions = regions;
    }

    pub(super) fn next_mmap_region_start(&self, length: u64) -> Option<u64> {
        self.next_mmap_region_start_from(self.mmap_next, length)
    }

    pub(super) fn next_mmap_region_start_from(&self, mut start: u64, length: u64) -> Option<u64> {
        while !self.is_mmap_range_available(start, length) {
            start = start.checked_add(RISCV_PAGE_BYTES)?;
        }
        Some(start)
    }

    pub(super) fn advance_mmap_next(&mut self, start: u64, length: u64) -> Option<()> {
        self.mmap_next = start.checked_add(length)?;
        Some(())
    }

    pub(super) fn push_mmap_region(&mut self, region: RiscvMmapRegion) {
        self.mmap_regions.push(region);
        self.mmap_regions.sort_by_key(|region| region.start());
    }

    fn push_guest_write(&mut self, write: RiscvGuestWriteRecord) {
        self.guest_writes.push(write);
    }

    fn push_unknown_syscall(&mut self, record: RiscvUnknownSyscallRecord) {
        self.unknown_syscalls.push(record);
    }

    fn guest_path_registered(&self, path: &[u8]) -> bool {
        self.guest_paths.contains(path)
    }

    fn guest_file_contents(&self, path: &[u8]) -> Option<&[u8]> {
        self.guest_files.get(path).map(Vec::as_slice)
    }

    fn guest_link_target(&self, path: &[u8]) -> Option<&[u8]> {
        self.guest_links.get(path).map(Vec::as_slice)
    }

    pub(super) fn unlink_guest_path(&mut self, path: &[u8]) -> bool {
        let removed_path = self.guest_paths.remove(path);
        let removed_file = self.guest_files.remove(path).is_some();
        let removed_link = self.guest_links.remove(path).is_some();
        removed_path || removed_file || removed_link
    }

    fn guest_path_stat(&self, path: &[u8]) -> Option<RiscvGuestStat> {
        if !self.guest_path_registered(path) {
            return None;
        }
        Some(RiscvGuestStat::regular_file(
            self.guest_file_contents(path)
                .map(|contents| contents.len() as u64)
                .unwrap_or(0),
            self.identity(),
            guest_path_inode(path),
        ))
    }

    fn guest_fd_stat(&self, fd: GuestFd) -> Result<RiscvGuestStat, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        Ok(self
            .guest_file_stats
            .get(&description)
            .copied()
            .unwrap_or_else(|| {
                RiscvGuestStat::character_device(
                    self.identity(),
                    u64::from(fd.get()).saturating_add(1),
                )
            }))
    }

    fn open_guest_path(&mut self, request: RiscvGuestOpenRequest) -> Result<GuestFd, GuestFdError> {
        let RiscvGuestOpenRequest {
            dirfd,
            path,
            flags,
            mode,
            status_flags,
            close_on_exec,
            file_contents,
        } = request;
        let fd = self.next_open_fd()?;
        let description = self.next_open_description()?;
        let stat = RiscvGuestStat::regular_file(
            file_contents
                .as_ref()
                .map(|contents| contents.len() as u64)
                .unwrap_or(0),
            self.identity(),
            guest_path_inode(&path),
        );
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                status_flags,
            ))?;
        self.guest_fds.insert(
            fd,
            GuestFdEntry::new(description).with_close_on_exec(close_on_exec),
        )?;
        if let Some(contents) = file_contents {
            self.guest_file_descriptions.insert(description, contents);
        }
        self.guest_file_stats.insert(description, stat);
        self.guest_opens
            .push(RiscvGuestOpenRecord::new(fd, dirfd, path, flags, mode));
        Ok(fd)
    }

    fn stdin_readable(&self, fd: GuestFd) -> bool {
        self.stdin_fds.contains(&fd)
    }

    fn close_fd_sources(&mut self, record: &GuestFdCloseRecord) {
        self.stdin_fds.remove(&record.fd());
        if let Some(description) = record.released_description() {
            self.guest_file_descriptions.remove(&description.id());
            self.guest_file_stats.remove(&description.id());
        }
    }

    fn duplicate_fd_source(&mut self, old_fd: GuestFd, new_fd: GuestFd) {
        if self.stdin_fds.contains(&old_fd) {
            self.stdin_fds.insert(new_fd);
        } else {
            self.stdin_fds.remove(&new_fd);
        }
    }

    fn release_replaced_fd_sources(&mut self, record: &GuestFdDup2Record) {
        if let Some(replaced) = record.replaced() {
            if let Some(description) = replaced.released_description() {
                self.guest_file_descriptions.remove(&description.id());
                self.guest_file_stats.remove(&description.id());
            }
        }
    }

    fn guest_file_prefix(
        &self,
        fd: GuestFd,
        count: usize,
    ) -> Result<Option<Vec<u8>>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(contents) = self.guest_file_descriptions.get(&description) else {
            return Ok(None);
        };
        let offset = self.guest_fds.file_offset(fd)?;
        let Ok(start) = usize::try_from(offset.get()) else {
            return Ok(Some(Vec::new()));
        };
        if start >= contents.len() {
            return Ok(Some(Vec::new()));
        }
        let end = start.saturating_add(count).min(contents.len());
        Ok(Some(contents[start..end].to_vec()))
    }

    pub(super) fn guest_file_slice_at(
        &self,
        fd: GuestFd,
        offset: u64,
        count: usize,
    ) -> Result<Option<Vec<u8>>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(contents) = self.guest_file_descriptions.get(&description) else {
            return Ok(None);
        };
        let Ok(start) = usize::try_from(offset) else {
            return Ok(Some(Vec::new()));
        };
        if start >= contents.len() {
            return Ok(Some(Vec::new()));
        }
        let end = start.saturating_add(count).min(contents.len());
        Ok(Some(contents[start..end].to_vec()))
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

    fn getrandom_bytes(&self, count: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(count);
        let mut counter = self.getrandom_byte_counter;
        for _ in 0..count {
            bytes.push(RISCV_LINUX_GETRANDOM_INITIAL_BYTE ^ counter);
            counter = counter.wrapping_add(1);
        }
        bytes
    }

    fn advance_getrandom_byte_counter(&mut self, count: usize) {
        self.getrandom_byte_counter = self.getrandom_byte_counter.wrapping_add(count as u8);
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
            RISCV_LINUX_GETCWD => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_getcwd(
                        request.argument(0),
                        request.argument(1),
                        state,
                        guest_memory,
                    ),
                })
            }
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
            RISCV_LINUX_IOCTL => Some(RiscvSyscallOutcome::Return {
                value: syscall_ioctl(request, state),
            }),
            RISCV_LINUX_OPENAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_openat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_OPEN => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_open(request, state, guest_memory),
                })
            }
            RISCV_LINUX_UNLINK => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_unlink(request, state, guest_memory),
                })
            }
            RISCV_LINUX_CLOSE => Some(RiscvSyscallOutcome::Return {
                value: syscall_close(request.argument(0), state),
            }),
            RISCV_LINUX_LSEEK => Some(RiscvSyscallOutcome::Return {
                value: syscall_lseek(request, state),
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
            RISCV_LINUX_WRITEV => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_writev(request, state, tick, guest_memory),
                })
            }
            RISCV_LINUX_READV => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_readv(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_READLINKAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_readlinkat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_NEWFSTATAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_newfstatat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_STAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_stat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_FSTAT => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_fstat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_GETRANDOM => {
                let flags = request.argument(2);
                if invalid_getrandom_flags(flags) {
                    Some(RiscvSyscallOutcome::Return {
                        value: linux_error(RISCV_LINUX_EINVAL),
                    })
                } else if request.argument(1) == 0 {
                    Some(RiscvSyscallOutcome::Return { value: 0 })
                } else {
                    guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                        value: syscall_getrandom(request, state, guest_memory),
                    })
                }
            }
            RISCV_LINUX_SET_TID_ADDRESS => Some(RiscvSyscallOutcome::Return {
                value: syscall_set_tid_address(request.argument(0), state),
            }),
            RISCV_LINUX_UNAME => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: write_riscv_linux_utsname(request.argument(0), guest_memory),
                })
            }
            RISCV_LINUX_GETTIMEOFDAY => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_gettimeofday(request.argument(0), tick, guest_memory),
                })
            }
            RISCV_LINUX_FUTEX => syscall_futex(request, state, tick, guest_memory_reader),
            RISCV_LINUX_WAIT4 => Some(RiscvSyscallOutcome::Return {
                value: syscall_wait4(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_CLOCK_GETTIME => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_clock_gettime(
                        request.argument(0),
                        request.argument(1),
                        tick,
                        guest_memory,
                    ),
                })
            }
            RISCV_LINUX_PRLIMIT64 => syscall_prlimit64(request, state, guest_memory_writer)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_SET_ROBUST_LIST => Some(RiscvSyscallOutcome::Return {
                value: syscall_set_robust_list(request, state),
            }),
            RISCV_LINUX_GET_ROBUST_LIST => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_get_robust_list(request, state, guest_memory),
                })
            }
            RISCV_LINUX_NANOSLEEP
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
                value: syscall_brk(request.argument(0), state, guest_memory_writer),
            }),
            RISCV_LINUX_MMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_mmap(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_MUNMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_munmap(request.argument(0), request.argument(1), state),
            }),
            _ => {
                state.push_unknown_syscall(RiscvUnknownSyscallRecord::new(
                    request.pc(),
                    request.number(),
                    request.arguments(),
                    tick,
                ));
                Some(RiscvSyscallOutcome::Return {
                    value: linux_error(RISCV_LINUX_ENOSYS),
                })
            }
        }
    }
}

fn invalid_getrandom_flags(flags: u64) -> bool {
    flags & !RISCV_LINUX_GRND_VALID_FLAGS != 0
        || flags & (RISCV_LINUX_GRND_RANDOM | RISCV_LINUX_GRND_INSECURE)
            == (RISCV_LINUX_GRND_RANDOM | RISCV_LINUX_GRND_INSECURE)
}

#[derive(Clone, Debug)]
pub struct RiscvSyscallEmulation {
    table: RiscvSyscallTable,
    state: Arc<Mutex<RiscvSyscallState>>,
    guest_memory_reader: Option<RiscvGuestMemoryReader>,
    guest_memory_writer: Option<RiscvGuestMemoryWriter>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSyscallImageLayoutError {
    UnrepresentableProgramBreak {
        loaded_segment_end: u64,
        page_bytes: u64,
    },
}

impl fmt::Display for RiscvSyscallImageLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnrepresentableProgramBreak {
                loaded_segment_end,
                page_bytes,
            } => write!(
                formatter,
                "loaded image end {loaded_segment_end:#x} cannot be rounded up to {page_bytes:#x}"
            ),
        }
    }
}

impl std::error::Error for RiscvSyscallImageLayoutError {}

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

    pub fn linux_user_for_boot_image(image: &BootImage) -> Self {
        Self::try_linux_user_for_boot_image(image)
            .expect("RISC-V SE boot image program break is representable")
    }

    pub fn try_linux_user_for_boot_image(
        image: &BootImage,
    ) -> Result<Self, RiscvSyscallImageLayoutError> {
        Ok(Self::new(
            RiscvSyscallTable::new(),
            RiscvSyscallState::new(riscv_program_break_for_boot_image(image)?),
        ))
    }

    fn with_boot_image_program_break(
        self,
        image: &BootImage,
    ) -> Result<Self, RiscvSyscallImageLayoutError> {
        let program_break = riscv_program_break_for_boot_image(image)?;
        {
            let mut state = self.state.lock().expect("RISC-V syscall state lock");
            state.set_program_break(program_break);
            state.set_program_break_backing_end(program_break);
        }
        Ok(self)
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

    pub fn with_mapped_guest_memory_writer<W, M>(mut self, write: W, map_region: M) -> Self
    where
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(u64, u64) -> bool + Send + Sync + 'static,
    {
        self.guest_memory_writer =
            Some(RiscvGuestMemoryWriter::new(write).with_region_mapper(map_region));
        self
    }

    pub fn with_guest_memory_map_handler<W, M>(mut self, write: W, map_region: M) -> Self
    where
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(RiscvGuestMemoryMapRequest) -> RiscvGuestMemoryMapResult + Send + Sync + 'static,
    {
        self.guest_memory_writer =
            Some(RiscvGuestMemoryWriter::new(write).with_region_map_handler(map_region));
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

    pub fn register_guest_file(&self, path: impl AsRef<[u8]>, contents: impl AsRef<[u8]>) {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .register_guest_file(path, contents);
    }

    pub fn register_guest_symlink(&self, path: impl AsRef<[u8]>, target: impl AsRef<[u8]>) {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .register_guest_symlink(path, target);
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

    pub fn with_riscv_syscall_emulation_for_boot_image(self, image: &BootImage) -> Self {
        self.try_with_riscv_syscall_emulation_for_boot_image(image)
            .expect("RISC-V SE boot image program break is representable")
    }

    pub fn try_with_riscv_syscall_emulation_for_boot_image(
        mut self,
        image: &BootImage,
    ) -> Result<Self, RiscvSyscallImageLayoutError> {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_boot_image_program_break(image)?;
        self.riscv_syscall_emulation = Some(emulation);
        Ok(self)
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_reader<F>(mut self, read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_reader(read);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_writer<F>(mut self, write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_writer(write);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_mapped_guest_memory_writer<W, M>(
        mut self,
        write: W,
        map_region: M,
    ) -> Self
    where
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(u64, u64) -> bool + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_mapped_guest_memory_writer(write, map_region);
        self.riscv_syscall_emulation = Some(emulation);
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
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_reader(read)
            .with_guest_memory_writer(write);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_mapped_guest_memory_io<R, W, M>(
        mut self,
        read: R,
        write: W,
        map_region: M,
    ) -> Self
    where
        R: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(u64, u64) -> bool + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_reader(read)
            .with_mapped_guest_memory_writer(write, map_region);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_io_map_handler<R, W, M>(
        mut self,
        read: R,
        write: W,
        map_region: M,
    ) -> Self
    where
        R: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(RiscvGuestMemoryMapRequest) -> RiscvGuestMemoryMapResult + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_reader(read)
            .with_guest_memory_map_handler(write, map_region);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub const fn riscv_syscall_emulation(&self) -> Option<&RiscvSyscallEmulation> {
        self.riscv_syscall_emulation.as_ref()
    }

    fn take_riscv_syscall_emulation_or_linux_user(&mut self) -> RiscvSyscallEmulation {
        self.riscv_syscall_emulation
            .take()
            .unwrap_or_else(RiscvSyscallEmulation::linux_user)
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

fn riscv_program_break_for_boot_image(
    image: &BootImage,
) -> Result<u64, RiscvSyscallImageLayoutError> {
    let end = image.loaded_segment_end().get();
    let mask = RISCV_PAGE_BYTES - 1;
    end.checked_add(mask).map(|value| value & !mask).ok_or(
        RiscvSyscallImageLayoutError::UnrepresentableProgramBreak {
            loaded_segment_end: end,
            page_bytes: RISCV_PAGE_BYTES,
        },
    )
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
        Ok(record) => {
            state.close_fd_sources(&record);
            0
        }
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

    let count = request.argument(2);
    if count == 0 {
        return 0;
    }

    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let read_from_stdin = state.stdin_readable(fd);
    let bytes = if read_from_stdin {
        state.stdin_prefix(byte_count)
    } else {
        match state.guest_file_prefix(fd, byte_count) {
            Ok(Some(bytes)) => bytes,
            Ok(None) | Err(_) => return linux_error(RISCV_LINUX_EBADF),
        }
    };
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
    if read_from_stdin {
        state.consume_stdin_prefix(bytes.len());
    }
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

fn syscall_newfstatat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let flags = request.argument(3);
    if flags
        & !(RISCV_LINUX_AT_EMPTY_PATH
            | RISCV_LINUX_AT_NO_AUTOMOUNT
            | RISCV_LINUX_AT_SYMLINK_NOFOLLOW)
        != 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let path = match read_guest_c_string(
        guest_memory_reader,
        request.argument(1),
        RISCV_LINUX_PATH_MAX,
    ) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };

    let stat = if path.is_empty() {
        if flags & RISCV_LINUX_AT_EMPTY_PATH == 0 {
            return linux_error(RISCV_LINUX_ENOENT);
        }
        let Some(fd) = guest_fd_argument(request.argument(0)) else {
            return linux_error(RISCV_LINUX_EBADF);
        };
        match state.guest_fd_stat(fd) {
            Ok(stat) => stat,
            Err(_error) => return linux_error(RISCV_LINUX_EBADF),
        }
    } else {
        if request.argument(0) != RISCV_LINUX_AT_FDCWD {
            return linux_error(RISCV_LINUX_EBADF);
        }
        match state.guest_path_stat(&path) {
            Some(stat) => stat,
            None => return linux_error(RISCV_LINUX_ENOENT),
        }
    };

    write_riscv_linux_stat(request.argument(2), stat, guest_memory_writer)
}

fn syscall_stat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let path = match read_guest_c_string(
        guest_memory_reader,
        request.argument(0),
        RISCV_LINUX_PATH_MAX,
    ) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    let Some(stat) = state.guest_path_stat(&path) else {
        return linux_error(RISCV_LINUX_ENOENT);
    };

    write_riscv_linux_stat(request.argument(1), stat, guest_memory_writer)
}

fn syscall_fstat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let stat = match state.guest_fd_stat(fd) {
        Ok(stat) => stat,
        Err(_error) => return linux_error(RISCV_LINUX_EBADF),
    };
    write_riscv_linux_stat(request.argument(1), stat, guest_memory)
}

fn syscall_getrandom(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let count = request
        .argument(1)
        .min(RISCV_LINUX_GETRANDOM_MAX_CHUNK_BYTES);
    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let bytes = state.getrandom_bytes(byte_count);
    if !guest_memory.write(request.argument(0), &bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    state.advance_getrandom_byte_counter(byte_count);
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
            state.release_replaced_fd_sources(&record);
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

pub(super) fn guest_fd_argument(value: u64) -> Option<GuestFd> {
    i32::try_from(value)
        .ok()
        .and_then(|fd| GuestFd::new(fd).ok())
}

fn guest_fd_error_return() -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return {
        value: linux_error(RISCV_LINUX_EBADF),
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

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

#[cfg(test)]
#[path = "riscv_syscall_tests.rs"]
mod tests;
