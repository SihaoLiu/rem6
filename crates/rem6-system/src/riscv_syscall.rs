use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt,
};

use rem6_boot::BootImage;
use rem6_cpu::{CpuId, RiscvCore};
use rem6_kernel::{PartitionedScheduler, Tick};

use crate::{
    GuestEventId, GuestFd, GuestFdCloseRecord, GuestFdDup2Record, GuestFdEntry, GuestFdError,
    GuestFdTable, GuestFileDescription, GuestFileDescriptionId, GuestFutexTable, GuestWaitQueue,
    RiscvSystemRunDriver, ScheduledRiscvTrap, SystemError,
};

mod brk;
mod clock;
mod cpu_locality;
mod cwd;
mod directory;
mod dirent;
mod emulation;
mod exit;
mod fcntl;
mod fd;
mod file_read;
mod file_write;
mod futex;
mod guest_memory;
mod guest_write;
mod identity;
mod ioctl;
mod limits;
mod link;
mod links;
mod mkdir;
mod mmap;
mod open;
mod permissions;
mod poll;
mod process;
mod random;
mod readv;
mod rename;
mod request;
mod robust;
mod scheduler;
mod seek;
mod signal;
mod sleep;
mod startup;
mod stat;
mod sysinfo;
mod thread;
mod time;
mod unknown;
mod unlink;
mod utsname;
mod wait4;
mod writev;

use brk::syscall_brk;
use clock::{
    syscall_clock, RISCV_LINUX_CLOCK_GETRES, RISCV_LINUX_CLOCK_GETTIME, RISCV_LINUX_GETTIMEOFDAY,
    RISCV_LINUX_TIMES,
};
use cpu_locality::{syscall_getcpu, RISCV_LINUX_GETCPU};
use cwd::{syscall_chdir, syscall_fchdir, syscall_getcwd, RISCV_LINUX_CHDIR, RISCV_LINUX_FCHDIR};
use directory::{RiscvGuestMkdirError, RiscvGuestRmdirError};
use dirent::{
    guest_directory_child_name, linux_dirent64_record_boundary, riscv_linux_dirent64_bytes,
    syscall_getdents64, RiscvGuestDirectoryEntry, RiscvGuestDirectoryReadError, RiscvGuestNodeKind,
    RISCV_LINUX_GETDENTS64,
};
pub use emulation::RiscvSyscallEmulation;
use exit::{syscall_exit_code, RISCV_LINUX_EXIT, RISCV_LINUX_EXIT_GROUP};
use fcntl::{syscall_fcntl, RISCV_LINUX_FCNTL};
#[cfg(test)]
use fcntl::{
    RISCV_LINUX_FD_CLOEXEC, RISCV_LINUX_F_GETFD, RISCV_LINUX_F_GETFL, RISCV_LINUX_F_SETFD,
    RISCV_LINUX_F_SETFL,
};
use fd::{
    linux_standard_guest_fds, syscall_close, syscall_dup, syscall_dup3, RISCV_LINUX_CLOSE,
    RISCV_LINUX_DUP, RISCV_LINUX_DUP3,
};
use file_read::{syscall_pread64, syscall_read, RISCV_LINUX_PREAD64, RISCV_LINUX_READ};
use file_write::{
    syscall_ftruncate, syscall_pwrite64, syscall_write, RISCV_LINUX_FTRUNCATE,
    RISCV_LINUX_PWRITE64, RISCV_LINUX_WRITE,
};
use futex::syscall_futex;
pub use guest_memory::{
    RiscvGuestMemoryMapRequest, RiscvGuestMemoryMapResult, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter,
};
pub use guest_write::RiscvGuestWriteRecord;
pub(crate) use identity::RiscvSyscallIdentity;
use identity::{
    syscall_identity, RISCV_LINUX_GETEGID, RISCV_LINUX_GETEUID, RISCV_LINUX_GETGID,
    RISCV_LINUX_GETPID, RISCV_LINUX_GETPPID, RISCV_LINUX_GETTID, RISCV_LINUX_GETUID,
};
use ioctl::{syscall_ioctl, RISCV_LINUX_IOCTL};
pub use limits::RISCV_LINUX_STACK_LIMIT_BYTES;
use limits::{syscall_getrlimit, syscall_prlimit64, RISCV_LINUX_GETRLIMIT, RISCV_LINUX_PRLIMIT64};
use link::{syscall_link_operation, RISCV_LINUX_LINK, RISCV_LINUX_LINKAT};
use links::syscall_readlinkat;
use mkdir::{syscall_mkdirat, RISCV_LINUX_MKDIRAT};
pub use mmap::RiscvMmapRegion;
use mmap::{
    syscall_madvise, syscall_memory_lock_range, syscall_mincore, syscall_mmap, syscall_mprotect,
    syscall_mremap, syscall_msync, syscall_munmap, RISCV64_LINUX_MMAP_BASE, RISCV_LINUX_MADVISE,
    RISCV_LINUX_MINCORE, RISCV_LINUX_MLOCK, RISCV_LINUX_MMAP, RISCV_LINUX_MPROTECT,
    RISCV_LINUX_MREMAP, RISCV_LINUX_MSYNC, RISCV_LINUX_MUNLOCK, RISCV_LINUX_MUNMAP,
    RISCV_PAGE_BYTES,
};
#[cfg(test)]
use mmap::{RISCV_LINUX_MAP_FIXED, RISCV_LINUX_MAP_PRIVATE};
pub use open::RiscvGuestOpenRecord;
use open::{syscall_open, syscall_openat, RiscvGuestOpenRequest, RISCV_LINUX_OPEN};
use permissions::{syscall_umask, RISCV_LINUX_UMASK};
use poll::{syscall_ppoll, RISCV_LINUX_PPOLL};
use process::{
    syscall_getpgid, syscall_getsid, syscall_setpgid, syscall_setsid, RISCV_LINUX_GETPGID,
    RISCV_LINUX_GETSID, RISCV_LINUX_SETPGID, RISCV_LINUX_SETSID,
};
use random::{invalid_getrandom_flags, syscall_getrandom, RISCV_LINUX_GETRANDOM};
use readv::{syscall_readv, RISCV_LINUX_READV};
use rename::{syscall_renameat2, RISCV_LINUX_RENAMEAT2};
pub use request::RiscvSyscallRequest;
use robust::{syscall_get_robust_list, syscall_set_robust_list, RiscvRobustList};
use seek::{syscall_lseek, RISCV_LINUX_LSEEK};
use signal::{
    syscall_kill, syscall_rt_sigaction, syscall_rt_sigpending, syscall_rt_sigprocmask,
    syscall_rt_sigtimedwait, syscall_tgkill, syscall_tkill, RiscvSignalAction,
};
use sleep::{
    syscall_clock_nanosleep, syscall_nanosleep, RISCV_LINUX_CLOCK_NANOSLEEP, RISCV_LINUX_NANOSLEEP,
};
pub use startup::{
    RiscvSeAuxvEntry, RiscvSeStartupConfig, RiscvSeStartupError, RiscvSeStartupImage,
    RiscvSeStartupStringField, RISCV_LINUX_AT_ENTRY, RISCV_LINUX_AT_NULL, RISCV_LINUX_AT_PAGESZ,
    RISCV_LINUX_AT_PHDR, RISCV_LINUX_AT_PHENT, RISCV_LINUX_AT_PHNUM, RISCV_LINUX_AT_RANDOM,
    RISCV_LINUX_AT_SECURE,
};
use stat::{
    guest_path_inode, syscall_access, syscall_faccessat, syscall_fstat, syscall_lstat,
    syscall_newfstatat, syscall_stat, syscall_statx, RiscvGuestStat, RISCV_LINUX_ACCESS,
    RISCV_LINUX_DEFAULT_DIRECTORY_PERMISSIONS, RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS,
    RISCV_LINUX_FACCESSAT, RISCV_LINUX_LSTAT, RISCV_LINUX_STATX,
};
use sysinfo::{syscall_sysinfo, RISCV_LINUX_SYSINFO};
pub use unknown::RiscvUnknownSyscallRecord;
use unlink::{syscall_unlink_operation, RISCV_LINUX_UNLINK, RISCV_LINUX_UNLINKAT};
use utsname::write_riscv_linux_utsname;
use wait4::{
    syscall_getrusage, syscall_process_group_id, syscall_wait4, RISCV_LINUX_GETRUSAGE,
    RISCV_LINUX_WAIT4,
};
use writev::{syscall_writev, RISCV_LINUX_WRITEV};

const RISCV_LINUX_GETCWD: u64 = 17;
const RISCV_LINUX_OPENAT: u64 = 56;
const RISCV_LINUX_READLINKAT: u64 = 78;
const RISCV_LINUX_NEWFSTATAT: u64 = 79;
const RISCV_LINUX_FSTAT: u64 = 80;
const RISCV_LINUX_SET_ROBUST_LIST: u64 = 99;
const RISCV_LINUX_GET_ROBUST_LIST: u64 = 100;
const RISCV_LINUX_SCHED_YIELD: u64 = 124;
const RISCV_LINUX_KILL: u64 = 129;
const RISCV_LINUX_TKILL: u64 = 130;
const RISCV_LINUX_TGKILL: u64 = 131;
const RISCV_LINUX_RT_SIGSUSPEND: u64 = 133;
const RISCV_LINUX_RT_SIGACTION: u64 = 134;
const RISCV_LINUX_RT_SIGPROCMASK: u64 = 135;
const RISCV_LINUX_RT_SIGPENDING: u64 = 136;
const RISCV_LINUX_RT_SIGTIMEDWAIT: u64 = 137;
const RISCV_LINUX_RT_SIGQUEUEINFO: u64 = 138;
const RISCV_LINUX_RT_SIGRETURN: u64 = 139;
const RISCV_LINUX_SETUID: u64 = 146;
const RISCV_LINUX_UNAME: u64 = 160;
const RISCV_LINUX_SETRLIMIT: u64 = 164;
const RISCV_LINUX_FUTEX: u64 = 98;
const RISCV_LINUX_BRK: u64 = 214;
const RISCV_LINUX_MLOCKALL: u64 = 230;
const RISCV_LINUX_MUNLOCKALL: u64 = 231;
const RISCV_LINUX_MBIND: u64 = 235;
const RISCV_LINUX_STAT: u64 = 1038;
const RISCV_LINUX_EPERM: u64 = 1;
const RISCV_LINUX_ENOENT: u64 = 2;
const RISCV_LINUX_ESRCH: u64 = 3;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EAGAIN: u64 = 11;
const RISCV_LINUX_ENOMEM: u64 = 12;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EEXIST: u64 = 17;
const RISCV_LINUX_ENOTDIR: u64 = 20;
const RISCV_LINUX_EISDIR: u64 = 21;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_EMFILE: u64 = 24;
const RISCV_LINUX_ENOTTY: u64 = 25;
const RISCV_LINUX_EFBIG: u64 = 27;
const RISCV_LINUX_ESPIPE: u64 = 29;
const RISCV_LINUX_ERANGE: u64 = 34;
const RISCV_LINUX_ENAMETOOLONG: u64 = 36;
const RISCV_LINUX_ENOSYS: u64 = 38;
const RISCV_LINUX_ENOTEMPTY: u64 = 39;
const RISCV_LINUX_ENOTSUP: u64 = 95;
const RISCV_LINUX_O_ACCMODE: u64 = 0x3;
const RISCV_LINUX_O_CLOEXEC: u64 = 0o2_000_000;
const RISCV_LINUX_O_RDONLY: u64 = 0;
const RISCV_LINUX_O_WRONLY: u64 = 1;
const RISCV_LINUX_O_APPEND: u64 = 0o2000;
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_AT_FDCWD: u64 = (-100_i64) as u64;
const RISCV_LINUX_AT_EMPTY_PATH: u64 = 0x1000;
const RISCV_LINUX_AT_NO_AUTOMOUNT: u64 = 0x800;
const RISCV_LINUX_AT_SYMLINK_NOFOLLOW: u64 = 0x100;
const RISCV_LINUX_PATH_MAX: usize = 4096;
const RISCV_LINUX_DEFAULT_SE_MEMORY_CAPACITY_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSyscallOutcome {
    Exit { code: i32 },
    Return { value: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestLinkError {
    SourceMissing,
    SourceIsDirectory,
    DestinationExists,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RiscvGuestFileIdentity {
    inode: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvOpenGuestFileStat {
    identity: RiscvGuestFileIdentity,
    size: u64,
    kind: RiscvGuestNodeKind,
    permissions: u32,
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
    session_id: u64,
    guest_paths: BTreeSet<Vec<u8>>,
    guest_directories: BTreeSet<Vec<u8>>,
    guest_directory_modes: BTreeMap<Vec<u8>, u32>,
    guest_files: BTreeMap<Vec<u8>, Vec<u8>>,
    guest_links: BTreeMap<Vec<u8>, Vec<u8>>,
    guest_file_identities: BTreeMap<Vec<u8>, RiscvGuestFileIdentity>,
    guest_file_modes: BTreeMap<RiscvGuestFileIdentity, u32>,
    guest_opens: Vec<RiscvGuestOpenRecord>,
    stdin_fds: BTreeSet<GuestFd>,
    guest_file_descriptions: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_file_description_paths: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_directory_descriptions: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_directory_paths: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_file_stats: BTreeMap<GuestFileDescriptionId, RiscvOpenGuestFileStat>,
    guest_writes: Vec<RiscvGuestWriteRecord>,
    unknown_syscalls: Vec<RiscvUnknownSyscallRecord>,
    file_creation_mask: u32,
    signal_mask: u64,
    signal_actions: BTreeMap<u64, RiscvSignalAction>,
    membarrier_registrations: u64,
    stdin: VecDeque<u8>,
    getrandom_byte_counter: u8,
    initial_program_break: u64,
    program_break: u64,
    program_break_backing_end: u64,
    linux_se_memory_capacity_bytes: u64,
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

    #[cfg(test)]
    fn with_identity_process_group_and_session(
        program_break: u64,
        identity: RiscvSyscallIdentity,
        process_group: crate::GuestProcessGroupId,
        session_id: u64,
    ) -> Self {
        let mut state = Self::with_identity(program_break, identity);
        state.guest_wait = GuestWaitQueue::new(process_group);
        state.session_id = session_id;
        state
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
            session_id: u64::from(current_process_group.get()),
            guest_paths: BTreeSet::new(),
            guest_directories: BTreeSet::new(),
            guest_directory_modes: BTreeMap::new(),
            guest_files: BTreeMap::new(),
            guest_links: BTreeMap::new(),
            guest_file_identities: BTreeMap::new(),
            guest_file_modes: BTreeMap::new(),
            guest_opens: Vec::new(),
            stdin_fds,
            guest_file_descriptions: BTreeMap::new(),
            guest_file_description_paths: BTreeMap::new(),
            guest_directory_descriptions: BTreeMap::new(),
            guest_directory_paths: BTreeMap::new(),
            guest_file_stats: BTreeMap::new(),
            guest_writes: Vec::new(),
            unknown_syscalls: Vec::new(),
            file_creation_mask: 0,
            signal_mask: 0,
            signal_actions: BTreeMap::new(),
            membarrier_registrations: 0,
            stdin: VecDeque::new(),
            getrandom_byte_counter: 0,
            initial_program_break: program_break,
            program_break,
            program_break_backing_end: program_break,
            linux_se_memory_capacity_bytes: RISCV_LINUX_DEFAULT_SE_MEMORY_CAPACITY_BYTES,
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
        let path = path.as_ref().to_vec();
        self.guest_paths.insert(path.clone());
        self.guest_file_identities
            .entry(path.clone())
            .or_insert_with(|| RiscvGuestFileIdentity {
                inode: guest_path_inode(&path),
            });
        let identity = self.guest_file_identity(&path);
        self.guest_file_modes
            .entry(identity)
            .or_insert(RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS);
    }

    pub fn register_guest_file(&mut self, path: impl AsRef<[u8]>, contents: impl AsRef<[u8]>) {
        let path = path.as_ref().to_vec();
        self.guest_paths.insert(path.clone());
        self.guest_files
            .insert(path.clone(), contents.as_ref().to_vec());
        self.guest_file_identities
            .entry(path.clone())
            .or_insert_with(|| RiscvGuestFileIdentity {
                inode: guest_path_inode(&path),
            });
        let identity = self.guest_file_identity(&path);
        self.guest_file_modes
            .entry(identity)
            .or_insert(RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS);
    }

    pub fn register_guest_symlink(&mut self, path: impl AsRef<[u8]>, target: impl AsRef<[u8]>) {
        let path = path.as_ref().to_vec();
        self.guest_links
            .insert(path.clone(), target.as_ref().to_vec());
        self.guest_file_identities
            .entry(path.clone())
            .or_insert_with(|| RiscvGuestFileIdentity {
                inode: guest_path_inode(&path),
            });
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

    pub(super) const fn initial_program_break(&self) -> u64 {
        self.initial_program_break
    }

    pub(super) const fn program_break_backing_end(&self) -> u64 {
        self.program_break_backing_end
    }

    pub const fn with_linux_se_memory_capacity(mut self, bytes: u64) -> Self {
        self.linux_se_memory_capacity_bytes = bytes;
        self
    }

    const fn linux_se_memory_capacity(&self) -> u64 {
        self.linux_se_memory_capacity_bytes
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

    pub(super) const fn file_creation_mask(&self) -> u32 {
        self.file_creation_mask
    }

    pub(super) fn replace_file_creation_mask(&mut self, mask: u32) -> u32 {
        let previous = self.file_creation_mask;
        self.file_creation_mask = mask;
        previous
    }

    pub(super) const fn signal_mask(&self) -> u64 {
        self.signal_mask
    }

    pub(super) fn set_signal_mask(&mut self, value: u64) {
        self.signal_mask = value;
    }

    fn signal_action(&self, signal: u64) -> RiscvSignalAction {
        self.signal_actions
            .get(&signal)
            .copied()
            .unwrap_or_default()
    }

    fn set_signal_action(&mut self, signal: u64, action: RiscvSignalAction) {
        self.signal_actions.insert(signal, action);
    }

    fn guest_path_registered(&self, path: &[u8]) -> bool {
        self.guest_paths.contains(path)
    }

    fn guest_path_exists(&self, path: &[u8]) -> bool {
        self.guest_paths.contains(path)
            || self.guest_directories.contains(path)
            || self.guest_files.contains_key(path)
            || self.guest_links.contains_key(path)
    }

    fn guest_file_contents(&self, path: &[u8]) -> Option<&[u8]> {
        self.guest_files.get(path).map(Vec::as_slice)
    }

    fn guest_link_target(&self, path: &[u8]) -> Option<&[u8]> {
        let path = self.resolve_existing_guest_path(path).ok().flatten()?;
        self.guest_links.get(&path).map(Vec::as_slice)
    }

    fn guest_file_identity(&self, path: &[u8]) -> RiscvGuestFileIdentity {
        self.guest_file_identities
            .get(path)
            .copied()
            .unwrap_or_else(|| RiscvGuestFileIdentity {
                inode: guest_path_inode(path),
            })
    }

    pub(super) fn set_guest_file_permissions(&mut self, path: &[u8], permissions: u32) {
        let identity = self.guest_file_identity(path);
        self.guest_file_modes.insert(identity, permissions & 0o777);
    }

    fn guest_file_permissions(&self, identity: RiscvGuestFileIdentity) -> u32 {
        self.guest_file_modes
            .get(&identity)
            .copied()
            .unwrap_or(RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS)
    }

    fn drop_guest_file_mode_if_unlinked(&mut self, identity: RiscvGuestFileIdentity) {
        if !self
            .guest_file_identities
            .values()
            .any(|candidate| *candidate == identity)
        {
            self.guest_file_modes.remove(&identity);
        }
    }

    fn guest_file_link_count(&self, identity: RiscvGuestFileIdentity) -> u32 {
        self.guest_paths
            .iter()
            .filter(|path| self.guest_file_identity(path) == identity)
            .count()
            .min(u32::MAX as usize) as u32
    }

    fn guest_file_stat(&self, size: u64, identity: RiscvGuestFileIdentity) -> RiscvGuestStat {
        RiscvGuestStat::regular_file(
            size,
            self.identity(),
            identity.inode,
            self.guest_file_link_count(identity),
            self.guest_file_permissions(identity),
        )
    }

    pub(super) fn unlink_guest_path(&mut self, path: &[u8]) -> bool {
        let removed_path = self.guest_paths.remove(path);
        let removed_file = self.guest_files.remove(path).is_some();
        let removed_link = self.guest_links.remove(path).is_some();
        if let Some(identity) = self.guest_file_identities.remove(path) {
            self.drop_guest_file_mode_if_unlinked(identity);
        }
        removed_path || removed_file || removed_link
    }

    pub(super) fn link_guest_path(
        &mut self,
        source: &[u8],
        destination: &[u8],
    ) -> Result<(), RiscvGuestLinkError> {
        let source_link = self.guest_links.get(source).cloned();
        let source_is_path = self.guest_path_registered(source);
        if self.guest_directory_entries(source).is_some() {
            return Err(RiscvGuestLinkError::SourceIsDirectory);
        }
        if !source_is_path && source_link.is_none() {
            return Err(RiscvGuestLinkError::SourceMissing);
        }
        if self.guest_path_exists(destination) {
            return Err(RiscvGuestLinkError::DestinationExists);
        }

        if !source_is_path {
            let identity = self.guest_file_identity(source);
            let target = source_link.expect("source link exists");
            self.guest_links.insert(destination.to_vec(), target);
            self.guest_file_identities
                .insert(destination.to_vec(), identity);
            return Ok(());
        }

        self.guest_paths.insert(destination.to_vec());
        if let Some(contents) = self.guest_files.get(source).cloned() {
            self.guest_files.insert(destination.to_vec(), contents);
        }
        if let Some(target) = self.guest_links.get(source).cloned() {
            self.guest_links.insert(destination.to_vec(), target);
        }
        if let Some(identity) = self.guest_file_identities.get(source).copied() {
            self.guest_file_identities
                .insert(destination.to_vec(), identity);
        }
        Ok(())
    }

    fn guest_path_stat(&self, path: &[u8]) -> Option<RiscvGuestStat> {
        if let Some(path) = self
            .resolve_existing_guest_regular_path(path)
            .ok()
            .flatten()
        {
            let identity = self.guest_file_identity(&path);
            return Some(
                self.guest_file_stat(
                    self.guest_file_contents(&path)
                        .map(|contents| contents.len() as u64)
                        .unwrap_or(0),
                    identity,
                ),
            );
        }
        let path = self.resolve_guest_path(path).ok()?;
        if self.guest_directory_entries(&path).is_some() {
            return Some(RiscvGuestStat::directory(
                self.identity(),
                guest_path_inode(&path),
                self.guest_directory_permissions(&path),
            ));
        }
        None
    }

    fn guest_link_stat(&self, path: &[u8]) -> Option<RiscvGuestStat> {
        let path = self.resolve_existing_guest_path(path).ok().flatten()?;
        let target = self.guest_links.get(&path)?;
        let identity = self.guest_file_identity(&path);
        Some(RiscvGuestStat::symbolic_link(
            target.len() as u64,
            self.identity(),
            identity.inode,
            self.guest_link_count(identity),
        ))
    }

    fn guest_link_count(&self, identity: RiscvGuestFileIdentity) -> u32 {
        self.guest_links
            .keys()
            .filter(|path| self.guest_file_identity(path) == identity)
            .count()
            .min(u32::MAX as usize) as u32
    }

    fn guest_fd_stat(&self, fd: GuestFd) -> Result<RiscvGuestStat, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        if let Some(stat) = self.guest_file_stats.get(&description).copied() {
            return Ok(match stat.kind {
                RiscvGuestNodeKind::Directory => RiscvGuestStat::directory(
                    self.identity(),
                    stat.identity.inode,
                    stat.permissions,
                ),
                RiscvGuestNodeKind::RegularFile | RiscvGuestNodeKind::Symlink => {
                    RiscvGuestStat::regular_file(
                        stat.size,
                        self.identity(),
                        stat.identity.inode,
                        self.guest_file_link_count(stat.identity),
                        stat.permissions,
                    )
                }
            });
        }
        Ok(RiscvGuestStat::character_device(
            self.identity(),
            u64::from(fd.get()).saturating_add(1),
        ))
    }

    pub(super) fn guest_fd_is_directory(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        Ok(self.guest_directory_descriptions.contains_key(&description))
    }

    pub(super) fn guest_directory_description_len(
        &self,
        fd: GuestFd,
    ) -> Result<Option<u64>, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        Ok(self
            .guest_directory_descriptions
            .get(&description)
            .map(|contents| contents.len() as u64))
    }

    fn open_guest_path(&mut self, request: RiscvGuestOpenRequest) -> Result<GuestFd, GuestFdError> {
        let RiscvGuestOpenRequest {
            dirfd,
            path,
            flags,
            mode,
            status_flags,
            close_on_exec,
            node_kind,
            file_contents,
            directory_contents,
        } = request;
        let fd = self.next_open_fd()?;
        let description = self.next_open_description()?;
        let identity = self.guest_file_identity(&path);
        let size = file_contents
            .as_ref()
            .map(|contents| contents.len() as u64)
            .unwrap_or(0);
        let permissions = if node_kind == RiscvGuestNodeKind::Directory {
            self.guest_directory_permissions(&path)
        } else {
            self.guest_file_permissions(identity)
        };
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
            self.guest_file_description_paths
                .insert(description, path.clone());
            self.guest_file_descriptions.insert(description, contents);
        }
        if let Some(contents) = directory_contents {
            self.guest_directory_descriptions
                .insert(description, contents);
            self.guest_directory_paths.insert(description, path.clone());
        }
        self.guest_file_stats.insert(
            description,
            RiscvOpenGuestFileStat {
                identity,
                size,
                kind: node_kind,
                permissions,
            },
        );
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
            self.guest_file_description_paths.remove(&description.id());
            self.guest_directory_descriptions.remove(&description.id());
            self.guest_directory_paths.remove(&description.id());
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
                self.guest_file_description_paths.remove(&description.id());
                self.guest_directory_descriptions.remove(&description.id());
                self.guest_directory_paths.remove(&description.id());
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

    pub(super) fn guest_directory_prefix(
        &self,
        fd: GuestFd,
        count: usize,
    ) -> Result<Option<Vec<u8>>, RiscvGuestDirectoryReadError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })
            .map_err(RiscvGuestDirectoryReadError::Fd)?
            .description();
        let Some(contents) = self.guest_directory_descriptions.get(&description) else {
            return Ok(None);
        };
        let offset = self
            .guest_fds
            .file_offset(fd)
            .map_err(RiscvGuestDirectoryReadError::Fd)?;
        let Ok(start) = usize::try_from(offset.get()) else {
            return Ok(Some(Vec::new()));
        };
        if start >= contents.len() {
            return Ok(Some(Vec::new()));
        }
        if !linux_dirent64_record_boundary(contents, start) {
            return Err(RiscvGuestDirectoryReadError::InvalidOffset);
        }

        let mut end = start;
        while end < contents.len() {
            if end + 18 > contents.len() {
                return Err(RiscvGuestDirectoryReadError::InvalidOffset);
            }
            let record_len = u16::from_le_bytes([contents[end + 16], contents[end + 17]]) as usize;
            if record_len < 24 || end + record_len > contents.len() {
                return Err(RiscvGuestDirectoryReadError::InvalidOffset);
            }
            if end + record_len - start > count {
                break;
            }
            end += record_len;
        }
        if end == start {
            return Err(RiscvGuestDirectoryReadError::BufferTooSmall);
        }
        Ok(Some(contents[start..end].to_vec()))
    }

    pub(super) fn advance_guest_directory_offset(
        &mut self,
        fd: GuestFd,
        count: u64,
    ) -> Result<(), GuestFdError> {
        self.guest_fds.advance_file_offset(fd, count)?;
        Ok(())
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
            RISCV_LINUX_CHDIR => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_chdir(request, state, guest_memory),
                })
            }
            RISCV_LINUX_FCHDIR => Some(RiscvSyscallOutcome::Return {
                value: syscall_fchdir(request, state),
            }),
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
            RISCV_LINUX_FTRUNCATE => Some(RiscvSyscallOutcome::Return {
                value: syscall_ftruncate(request, state),
            }),
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
            RISCV_LINUX_GETDENTS64 => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_getdents64(request, state, guest_memory),
                })
            }
            RISCV_LINUX_MKDIRAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_mkdirat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_LINK | RISCV_LINUX_LINKAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_link_operation(request, state, guest_memory),
                })
            }
            RISCV_LINUX_UNLINK | RISCV_LINUX_UNLINKAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_unlink_operation(request, state, guest_memory),
                })
            }
            RISCV_LINUX_RENAMEAT2 => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_renameat2(request, state, guest_memory),
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
            RISCV_LINUX_PREAD64 => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_pread64(request, state, guest_memory),
                })
            }
            RISCV_LINUX_WRITE => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_write(request, state, tick, guest_memory),
                })
            }
            RISCV_LINUX_PWRITE64 => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_pwrite64(request, state, tick, guest_memory),
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
            RISCV_LINUX_PPOLL => {
                syscall_ppoll(request, state, guest_memory_reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_READLINKAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_readlinkat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_FACCESSAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_faccessat(request, state, guest_memory),
                })
            }
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
            RISCV_LINUX_LSTAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_lstat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_STATX => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_statx(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_ACCESS => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_access(request, state, guest_memory),
                })
            }
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
            thread::RISCV_LINUX_SET_TID_ADDRESS
            | thread::RISCV_LINUX_MEMBARRIER
            | thread::RISCV_LINUX_RSEQ => Some(RiscvSyscallOutcome::Return {
                value: thread::syscall_thread(request, state),
            }),
            RISCV_LINUX_TIMES
            | RISCV_LINUX_GETTIMEOFDAY
            | RISCV_LINUX_CLOCK_GETTIME
            | RISCV_LINUX_CLOCK_GETRES => syscall_clock(request, tick, guest_memory_writer),
            RISCV_LINUX_SETPGID => Some(RiscvSyscallOutcome::Return {
                value: syscall_setpgid(request, state),
            }),
            RISCV_LINUX_GETPGID => Some(RiscvSyscallOutcome::Return {
                value: syscall_getpgid(request, state),
            }),
            RISCV_LINUX_GETSID => Some(RiscvSyscallOutcome::Return {
                value: syscall_getsid(request, state),
            }),
            RISCV_LINUX_SETSID => Some(RiscvSyscallOutcome::Return {
                value: syscall_setsid(state),
            }),
            RISCV_LINUX_UNAME => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: write_riscv_linux_utsname(request.argument(0), guest_memory),
                })
            }
            RISCV_LINUX_SYSINFO => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_sysinfo(
                        request.argument(0),
                        tick,
                        state.linux_se_memory_capacity(),
                        guest_memory,
                    ),
                })
            }
            RISCV_LINUX_FUTEX => syscall_futex(request, state, tick, guest_memory_reader),
            RISCV_LINUX_WAIT4 => Some(RiscvSyscallOutcome::Return {
                value: syscall_wait4(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_GETRUSAGE => Some(RiscvSyscallOutcome::Return {
                value: syscall_getrusage(request, guest_memory_writer),
            }),
            RISCV_LINUX_GETRLIMIT => syscall_getrlimit(request, guest_memory_writer)
                .map(|value| RiscvSyscallOutcome::Return { value }),
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
            RISCV_LINUX_GETCPU => syscall_getcpu(request, guest_memory_writer)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_NANOSLEEP => syscall_nanosleep(request, guest_memory_reader)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_CLOCK_NANOSLEEP => {
                syscall_clock_nanosleep(request, tick, guest_memory_reader)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_UMASK => Some(RiscvSyscallOutcome::Return {
                value: syscall_umask(request.argument(0), state),
            }),
            scheduler::RISCV_LINUX_SCHED_SETAFFINITY => {
                scheduler::syscall_sched_setaffinity(request, state, guest_memory_reader)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_SCHED_GETSCHEDULER => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_sched_getscheduler(request, state),
            }),
            scheduler::RISCV_LINUX_SCHED_GETPARAM => {
                scheduler::syscall_sched_getparam(request, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_SCHED_GETAFFINITY => {
                scheduler::syscall_sched_getaffinity(request, state, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_SCHED_GET_PRIORITY_MAX => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_sched_get_priority_max(request),
            }),
            scheduler::RISCV_LINUX_SCHED_GET_PRIORITY_MIN => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_sched_get_priority_min(request),
            }),
            scheduler::RISCV_LINUX_SCHED_RR_GET_INTERVAL => {
                scheduler::syscall_sched_rr_get_interval(request, state, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_KILL => Some(RiscvSyscallOutcome::Return {
                value: syscall_kill(request, state, tick),
            }),
            RISCV_LINUX_TKILL => Some(RiscvSyscallOutcome::Return {
                value: syscall_tkill(request, state, tick),
            }),
            RISCV_LINUX_TGKILL => Some(RiscvSyscallOutcome::Return {
                value: syscall_tgkill(request, state, tick),
            }),
            RISCV_LINUX_SCHED_YIELD
            | RISCV_LINUX_RT_SIGSUSPEND
            | RISCV_LINUX_RT_SIGQUEUEINFO
            | RISCV_LINUX_RT_SIGRETURN => Some(RiscvSyscallOutcome::Return { value: 0 }),
            RISCV_LINUX_RT_SIGACTION => {
                syscall_rt_sigaction(request, state, guest_memory_reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_RT_SIGPROCMASK => {
                syscall_rt_sigprocmask(request, state, guest_memory_reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_RT_SIGPENDING => syscall_rt_sigpending(request, guest_memory_writer)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_RT_SIGTIMEDWAIT => syscall_rt_sigtimedwait(request, guest_memory_reader)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_MPROTECT => Some(RiscvSyscallOutcome::Return {
                value: syscall_mprotect(request, state),
            }),
            RISCV_LINUX_MSYNC => Some(RiscvSyscallOutcome::Return {
                value: syscall_msync(request, state),
            }),
            RISCV_LINUX_MLOCK | RISCV_LINUX_MUNLOCK => Some(RiscvSyscallOutcome::Return {
                value: syscall_memory_lock_range(request, state),
            }),
            RISCV_LINUX_MLOCKALL
            | RISCV_LINUX_MUNLOCKALL
            | RISCV_LINUX_MBIND
            | RISCV_LINUX_SETUID
            | RISCV_LINUX_SETRLIMIT => Some(RiscvSyscallOutcome::Return { value: 0 }),
            RISCV_LINUX_EXIT | RISCV_LINUX_EXIT_GROUP => Some(RiscvSyscallOutcome::Exit {
                code: syscall_exit_code(request.argument(0)),
            }),
            RISCV_LINUX_GETPID | RISCV_LINUX_GETPPID | RISCV_LINUX_GETTID | RISCV_LINUX_GETUID
            | RISCV_LINUX_GETEUID | RISCV_LINUX_GETGID | RISCV_LINUX_GETEGID => {
                Some(RiscvSyscallOutcome::Return {
                    value: syscall_identity(request.number(), state.identity())
                        .expect("RISC-V Linux identity syscall is handled"),
                })
            }
            RISCV_LINUX_BRK => Some(RiscvSyscallOutcome::Return {
                value: syscall_brk(request.argument(0), state, guest_memory_writer),
            }),
            RISCV_LINUX_MMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_mmap(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_MUNMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_munmap(request.argument(0), request.argument(1), state),
            }),
            RISCV_LINUX_MREMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_mremap(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_MINCORE => Some(RiscvSyscallOutcome::Return {
                value: syscall_mincore(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_MADVISE => Some(RiscvSyscallOutcome::Return {
                value: syscall_madvise(request, state),
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

impl RiscvSystemRunDriver {
    pub(crate) fn schedule_pending_core_events<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        cores: Vec<RiscvCore>,
        event_for: F,
    ) -> Result<Vec<ScheduledRiscvTrap>, SystemError>
    where
        F: FnMut(CpuId) -> GuestEventId,
    {
        if self.riscv_sbi_firmware.is_some() {
            self.trap_port
                .schedule_pending_core_traps_with_riscv_emulation(
                    scheduler,
                    cores,
                    self.riscv_sbi_firmware.as_ref(),
                    self.riscv_syscall_emulation.as_ref(),
                    event_for,
                )
        } else if let Some(syscalls) = self.riscv_syscall_emulation.as_ref() {
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
        if self.riscv_sbi_firmware.is_some() {
            self.trap_port
                .schedule_pending_core_traps_with_riscv_emulation_parallel(
                    scheduler,
                    cores,
                    self.riscv_sbi_firmware.as_ref(),
                    self.riscv_syscall_emulation.as_ref(),
                    event_for,
                )
        } else if let Some(syscalls) = self.riscv_syscall_emulation.as_ref() {
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

pub(super) fn guest_fd_argument(value: u64) -> Option<GuestFd> {
    i32::try_from(value)
        .ok()
        .and_then(|fd| GuestFd::new(fd).ok())
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
