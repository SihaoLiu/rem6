use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::{
    GuestFd, GuestFdCloseRecord, GuestFdDup2Record, GuestFdEntry, GuestFdError, GuestFdTable,
    GuestFileDescription, GuestFileDescriptionId, GuestFutexTable, GuestWaitQueue,
};
#[cfg(test)]
use rem6_boot::BootImage;
use rem6_kernel::Tick;

mod advisory;
mod brk;
mod capability;
mod clock;
mod constants;
mod copy_file_range;
mod cpu_locality;
mod cwd;
mod directory;
mod dirent;
mod emulation;
mod epoll;
mod eventfd;
mod exit;
mod fcntl;
mod fd;
mod file_read;
mod file_write;
mod flock;
mod futex;
mod guest_memory;
mod guest_write;
mod hwprobe;
mod identity;
mod image_layout;
mod ioctl;
mod iovec;
mod limits;
mod link;
mod links;
mod memfd;
mod mkdir;
mod mmap;
mod open;
mod permissions;
mod pipe;
mod poll;
mod positioned;
mod process;
mod procfs;
mod random;
mod readv;
mod rename;
mod request;
mod robust;
mod run_events;
mod scheduler;
mod seek;
mod sendfile;
mod signal;
mod sleep;
mod startup;
mod stat;
mod sync;
mod sysinfo;
mod table;
mod thread;
mod time;
mod unknown;
mod unlink;
mod util;
mod utsname;
mod wait4;
mod writev;
mod xattr;

use advisory::{syscall_fadvise64, RISCV_LINUX_FADVISE64};
use brk::syscall_brk;
use capability::{syscall_capget, syscall_capset, RISCV_LINUX_CAPGET, RISCV_LINUX_CAPSET};
use clock::{
    syscall_clock, syscall_getitimer, syscall_setitimer, RiscvLinuxItimerval,
    RISCV_LINUX_CLOCK_GETRES, RISCV_LINUX_CLOCK_GETTIME, RISCV_LINUX_GETITIMER,
    RISCV_LINUX_GETTIMEOFDAY, RISCV_LINUX_SETITIMER, RISCV_LINUX_TIMES,
};
use constants::*;
use copy_file_range::{syscall_copy_file_range, RISCV_LINUX_COPY_FILE_RANGE};
use cpu_locality::{syscall_getcpu, RISCV_LINUX_GETCPU};
use cwd::{
    syscall_chdir, syscall_fchdir, syscall_getcwd, RiscvGuestPathResolutionError,
    RISCV_LINUX_CHDIR, RISCV_LINUX_FCHDIR,
};
use directory::{RiscvGuestMkdirError, RiscvGuestRmdirError};
use dirent::{
    guest_directory_child_name, linux_dirent64_record_boundary, riscv_linux_dirent64_bytes,
    syscall_getdents64, RiscvGuestDirectoryEntry, RiscvGuestDirectoryReadError, RiscvGuestNodeKind,
    RISCV_LINUX_GETDENTS64,
};
pub use emulation::RiscvSyscallEmulation;
use epoll::{
    syscall_epoll_create1, syscall_epoll_ctl, syscall_epoll_pwait, RiscvGuestEpoll,
    RISCV_LINUX_EPOLL_CREATE1, RISCV_LINUX_EPOLL_CTL, RISCV_LINUX_EPOLL_PWAIT,
};
use eventfd::{syscall_eventfd2, RiscvGuestEventFd, RISCV_LINUX_EVENTFD2};
use exit::{syscall_exit, RISCV_LINUX_EXIT, RISCV_LINUX_EXIT_GROUP};
use fcntl::{syscall_fcntl, RISCV_LINUX_FCNTL};
#[cfg(test)]
use fcntl::{
    RISCV_LINUX_FD_CLOEXEC, RISCV_LINUX_F_GETFD, RISCV_LINUX_F_GETFL, RISCV_LINUX_F_SETFD,
    RISCV_LINUX_F_SETFL,
};
use fd::{
    linux_standard_guest_fds, syscall_close, syscall_close_range, syscall_dup, syscall_dup3,
    RISCV_LINUX_CLOSE, RISCV_LINUX_CLOSE_RANGE, RISCV_LINUX_DUP, RISCV_LINUX_DUP3,
};
use file_read::{syscall_pread64, syscall_read, RISCV_LINUX_PREAD64, RISCV_LINUX_READ};
use file_write::{
    syscall_fallocate, syscall_ftruncate, syscall_pwrite64, syscall_truncate, syscall_write,
    RISCV_LINUX_FALLOCATE, RISCV_LINUX_FTRUNCATE, RISCV_LINUX_PWRITE64, RISCV_LINUX_TRUNCATE,
    RISCV_LINUX_WRITE,
};
use flock::{syscall_flock, RISCV_LINUX_FLOCK};
use futex::syscall_futex;
pub use guest_memory::{
    RiscvGuestMemoryMapRequest, RiscvGuestMemoryMapResult, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter,
};
pub use guest_write::RiscvGuestWriteRecord;
use hwprobe::{syscall_riscv_hwprobe, RISCV_LINUX_RISCV_HWPROBE};
pub(crate) use identity::RiscvSyscallIdentity;
use identity::{
    syscall_getgroups, syscall_identity, syscall_res_identity, syscall_set_identity,
    syscall_setgroups, syscall_setre_identity, syscall_setres_identity, RISCV_LINUX_GETEGID,
    RISCV_LINUX_GETEUID, RISCV_LINUX_GETGID, RISCV_LINUX_GETGROUPS, RISCV_LINUX_GETPID,
    RISCV_LINUX_GETPPID, RISCV_LINUX_GETRESGID, RISCV_LINUX_GETRESUID, RISCV_LINUX_GETTID,
    RISCV_LINUX_GETUID, RISCV_LINUX_SETGID, RISCV_LINUX_SETGROUPS, RISCV_LINUX_SETREGID,
    RISCV_LINUX_SETRESGID, RISCV_LINUX_SETRESUID, RISCV_LINUX_SETREUID, RISCV_LINUX_SETUID,
};
use image_layout::riscv_program_break_for_boot_image;
pub use image_layout::RiscvSyscallImageLayoutError;
use ioctl::{syscall_ioctl, RISCV_LINUX_IOCTL};
pub use limits::RISCV_LINUX_STACK_LIMIT_BYTES;
use limits::{
    syscall_getrlimit, syscall_prlimit64, syscall_setrlimit, RISCV_LINUX_GETRLIMIT,
    RISCV_LINUX_PRLIMIT64,
};
use link::{
    syscall_link_operation, syscall_symlinkat, RiscvGuestLinkError, RiscvGuestSymlinkError,
    RISCV_LINUX_LINK, RISCV_LINUX_LINKAT, RISCV_LINUX_SYMLINKAT,
};
use links::syscall_readlinkat;
use memfd::{syscall_memfd_create, RISCV_LINUX_MEMFD_CREATE};
use mkdir::{syscall_mkdir, RISCV_LINUX_MKDIRAT, RISCV_NEWLIB_LEGACY_MKDIR};
pub use mmap::RiscvMmapRegion;
use mmap::{
    syscall_madvise, syscall_mbind, syscall_memory_lock_range, syscall_mincore, syscall_mlockall,
    syscall_mmap, syscall_mprotect, syscall_mremap, syscall_msync, syscall_munlockall,
    syscall_munmap, RISCV64_LINUX_MMAP_BASE, RISCV_LINUX_MADVISE, RISCV_LINUX_MBIND,
    RISCV_LINUX_MINCORE, RISCV_LINUX_MLOCK, RISCV_LINUX_MMAP, RISCV_LINUX_MPROTECT,
    RISCV_LINUX_MREMAP, RISCV_LINUX_MSYNC, RISCV_LINUX_MUNLOCK, RISCV_LINUX_MUNMAP,
    RISCV_PAGE_BYTES,
};
#[cfg(test)]
use mmap::{RISCV_LINUX_MAP_FIXED, RISCV_LINUX_MAP_PRIVATE};
pub use open::RiscvGuestOpenRecord;
use open::{
    syscall_open, syscall_openat, syscall_openat2, RiscvGuestOpenRequest, RISCV_LINUX_OPEN,
    RISCV_LINUX_OPENAT2,
};
use permissions::{
    syscall_chmod, syscall_fchmod, syscall_fchmodat, syscall_fchmodat2, syscall_fchown,
    syscall_fchownat, syscall_umask, RISCV_LINUX_FCHMOD, RISCV_LINUX_FCHMODAT,
    RISCV_LINUX_FCHMODAT2, RISCV_LINUX_FCHOWN, RISCV_LINUX_FCHOWNAT, RISCV_LINUX_UMASK,
    RISCV_NEWLIB_LEGACY_CHMOD,
};
use pipe::{
    syscall_pipe2, RiscvGuestPipe, RiscvGuestPipeEndpoint, RiscvGuestPipeId, RISCV_LINUX_PIPE2,
};
use poll::{syscall_ppoll, syscall_pselect6, RISCV_LINUX_PPOLL, RISCV_LINUX_PSELECT6};
use process::{
    syscall_execve_error_path, syscall_getpgid, syscall_getsid, syscall_personality, syscall_prctl,
    syscall_setpgid, syscall_setsid, RISCV_LINUX_EXECVE, RISCV_LINUX_GETPGID, RISCV_LINUX_GETSID,
    RISCV_LINUX_PERSONALITY, RISCV_LINUX_PRCTL, RISCV_LINUX_SETPGID, RISCV_LINUX_SETSID,
};
use random::{invalid_getrandom_flags, syscall_getrandom, RISCV_LINUX_GETRANDOM};
use readv::{syscall_preadv, syscall_readv, RISCV_LINUX_PREADV, RISCV_LINUX_READV};
use rename::{syscall_renameat, syscall_renameat2, RISCV_LINUX_RENAMEAT, RISCV_LINUX_RENAMEAT2};
pub use request::RiscvSyscallRequest;
use robust::{syscall_get_robust_list, syscall_set_robust_list, RiscvRobustList};
use seek::{syscall_lseek, RISCV_LINUX_LSEEK};
use sendfile::{syscall_sendfile, RISCV_LINUX_SENDFILE};
use signal::{
    syscall_kill, syscall_rt_sigaction, syscall_rt_sigpending, syscall_rt_sigprocmask,
    syscall_rt_sigqueueinfo, syscall_rt_sigsuspend, syscall_rt_sigtimedwait, syscall_sigaltstack,
    syscall_tgkill, syscall_tkill, RiscvSignalAction, RiscvSignalAltStack, RISCV_LINUX_SIGALTSTACK,
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
    guest_path_inode, syscall_access, syscall_faccessat, syscall_faccessat2, syscall_fstat,
    syscall_fstatfs, syscall_lstat, syscall_newfstatat, syscall_stat, syscall_statfs,
    syscall_statx, syscall_utimensat, RiscvGuestStat, RISCV_LINUX_ACCESS,
    RISCV_LINUX_DEFAULT_DIRECTORY_PERMISSIONS, RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS,
    RISCV_LINUX_FACCESSAT, RISCV_LINUX_FACCESSAT2, RISCV_LINUX_FSTATFS, RISCV_LINUX_LSTAT,
    RISCV_LINUX_STATFS, RISCV_LINUX_STATX, RISCV_LINUX_UTIMENSAT,
};
use sync::{
    syscall_fd_sync, syscall_sync, syscall_sync_file_range, RISCV_LINUX_FDATASYNC,
    RISCV_LINUX_FSYNC, RISCV_LINUX_SYNC, RISCV_LINUX_SYNCFS, RISCV_LINUX_SYNC_FILE_RANGE,
};
use sysinfo::{syscall_sysinfo, RISCV_LINUX_SYSINFO};
pub use table::RiscvSyscallTable;
pub use unknown::RiscvUnknownSyscallRecord;
use unlink::{syscall_unlink_operation, RISCV_LINUX_UNLINK, RISCV_LINUX_UNLINKAT};
use util::{guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError};
use utsname::write_riscv_linux_utsname;
use wait4::{
    syscall_getrusage, syscall_process_group_id, syscall_wait4, syscall_waitid,
    RISCV_LINUX_GETRUSAGE, RISCV_LINUX_WAIT4, RISCV_LINUX_WAITID,
};
use writev::{syscall_pwritev, syscall_writev, RISCV_LINUX_PWRITEV, RISCV_LINUX_WRITEV};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSyscallOutcome {
    Blocked,
    Exit { code: i32 },
    Return { value: u64 },
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

const RISCV_GUEST_ALLOCATED_INODE_BASE: u64 = 1 << 63;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSyscallState {
    identity: RiscvSyscallIdentity,
    current_directory: Vec<u8>,
    child_clear_tid: Option<u64>,
    robust_list: RiscvRobustList,
    personality: u32,
    process_nice: i32,
    sched_policy: i32,
    guest_fds: GuestFdTable,
    guest_futexes: GuestFutexTable,
    guest_wait: GuestWaitQueue,
    session_id: u64,
    process_name: [u8; 16],
    no_new_privs: bool,
    pdeath_signal: u32,
    interval_timers: [RiscvLinuxItimerval; 3],
    next_guest_inode: u64,
    guest_paths: BTreeSet<Vec<u8>>,
    guest_directories: BTreeSet<Vec<u8>>,
    guest_directory_identities: BTreeMap<Vec<u8>, RiscvGuestFileIdentity>,
    guest_directory_modes: BTreeMap<Vec<u8>, u32>,
    guest_files: BTreeMap<Vec<u8>, Vec<u8>>,
    guest_links: BTreeMap<Vec<u8>, Vec<u8>>,
    guest_file_identities: BTreeMap<Vec<u8>, RiscvGuestFileIdentity>,
    guest_file_modes: BTreeMap<RiscvGuestFileIdentity, u32>,
    guest_xattrs: BTreeMap<RiscvGuestFileIdentity, BTreeMap<Vec<u8>, Vec<u8>>>,
    guest_pipes: BTreeMap<RiscvGuestPipeId, RiscvGuestPipe>,
    guest_pipe_read_descriptions: BTreeMap<GuestFileDescriptionId, RiscvGuestPipeEndpoint>,
    guest_pipe_write_descriptions: BTreeMap<GuestFileDescriptionId, RiscvGuestPipeEndpoint>,
    guest_eventfds: BTreeMap<GuestFileDescriptionId, RiscvGuestEventFd>,
    guest_epolls: BTreeMap<GuestFileDescriptionId, RiscvGuestEpoll>,
    guest_opens: Vec<RiscvGuestOpenRecord>,
    stdin_fds: BTreeSet<GuestFd>,
    guest_file_descriptions: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_file_description_paths: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_directory_descriptions: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_directory_paths: BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    guest_file_stats: BTreeMap<GuestFileDescriptionId, RiscvOpenGuestFileStat>,
    guest_file_seals: BTreeMap<GuestFileDescriptionId, u32>,
    guest_writes: Vec<RiscvGuestWriteRecord>,
    unknown_syscalls: Vec<RiscvUnknownSyscallRecord>,
    file_creation_mask: u32,
    signal_mask: u64,
    pending_signal_mask: u64,
    signal_actions: BTreeMap<u64, RiscvSignalAction>,
    signal_alt_stack: RiscvSignalAltStack,
    resource_limits: limits::RiscvResourceLimits,
    membarrier_registrations: u64,
    rseq_registration: Option<thread::RiscvSyscallRseqRegistration>,
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
            personality: 0,
            process_nice: 0,
            sched_policy: scheduler::RISCV_LINUX_DEFAULT_SCHED_POLICY,
            guest_fds: linux_standard_guest_fds(),
            guest_futexes: GuestFutexTable::new(),
            guest_wait: GuestWaitQueue::new(current_process_group),
            session_id: u64::from(current_process_group.get()),
            process_name: *b"rem6\0\0\0\0\0\0\0\0\0\0\0\0",
            no_new_privs: false,
            pdeath_signal: 0,
            interval_timers: Self::initial_interval_timers(),
            next_guest_inode: RISCV_GUEST_ALLOCATED_INODE_BASE,
            guest_paths: BTreeSet::new(),
            guest_directories: BTreeSet::new(),
            guest_directory_identities: BTreeMap::new(),
            guest_directory_modes: BTreeMap::new(),
            guest_files: BTreeMap::new(),
            guest_links: BTreeMap::new(),
            guest_file_identities: BTreeMap::new(),
            guest_file_modes: BTreeMap::new(),
            guest_xattrs: BTreeMap::new(),
            guest_pipes: BTreeMap::new(),
            guest_pipe_read_descriptions: BTreeMap::new(),
            guest_pipe_write_descriptions: BTreeMap::new(),
            guest_eventfds: BTreeMap::new(),
            guest_epolls: BTreeMap::new(),
            guest_opens: Vec::new(),
            stdin_fds,
            guest_file_descriptions: BTreeMap::new(),
            guest_file_description_paths: BTreeMap::new(),
            guest_directory_descriptions: BTreeMap::new(),
            guest_directory_paths: BTreeMap::new(),
            guest_file_stats: BTreeMap::new(),
            guest_file_seals: BTreeMap::new(),
            guest_writes: Vec::new(),
            unknown_syscalls: Vec::new(),
            file_creation_mask: 0,
            signal_mask: 0,
            pending_signal_mask: 0,
            signal_actions: BTreeMap::new(),
            signal_alt_stack: RiscvSignalAltStack::disabled(),
            resource_limits: limits::RiscvResourceLimits::linux_single_process(),
            membarrier_registrations: 0,
            rseq_registration: None,
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
        let identity = self.ensure_guest_file_identity(&path);
        self.guest_file_modes
            .entry(identity)
            .or_insert(RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS);
    }

    pub fn register_guest_file(&mut self, path: impl AsRef<[u8]>, contents: impl AsRef<[u8]>) {
        let path = path.as_ref().to_vec();
        self.guest_paths.insert(path.clone());
        self.guest_files
            .insert(path.clone(), contents.as_ref().to_vec());
        let identity = self.ensure_guest_file_identity(&path);
        self.guest_file_modes
            .entry(identity)
            .or_insert(RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS);
    }

    pub fn register_guest_symlink(&mut self, path: impl AsRef<[u8]>, target: impl AsRef<[u8]>) {
        let path = path.as_ref().to_vec();
        self.guest_links
            .insert(path.clone(), target.as_ref().to_vec());
        self.ensure_guest_file_identity(&path);
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

    fn allocate_guest_file_identity(&mut self) -> RiscvGuestFileIdentity {
        let identity = RiscvGuestFileIdentity {
            inode: self.next_guest_inode,
        };
        self.next_guest_inode = self
            .next_guest_inode
            .checked_add(1)
            .expect("guest inode allocator exhausted");
        identity
    }

    fn ensure_guest_file_identity(&mut self, path: &[u8]) -> RiscvGuestFileIdentity {
        if let Some(identity) = self.guest_file_identities.get(path).copied() {
            return identity;
        }
        let identity = self.allocate_guest_file_identity();
        self.guest_file_identities.insert(path.to_vec(), identity);
        identity
    }

    fn ensure_guest_directory_identity(&mut self, path: &[u8]) -> RiscvGuestFileIdentity {
        if let Some(identity) = self.guest_directory_identities.get(path).copied() {
            return identity;
        }
        let identity = self.allocate_guest_file_identity();
        self.guest_directory_identities
            .insert(path.to_vec(), identity);
        identity
    }

    fn guest_link_target(&self, path: &[u8]) -> Option<&[u8]> {
        self.guest_link_target_result(path).ok().flatten()
    }

    fn guest_link_target_result(
        &self,
        path: &[u8],
    ) -> Result<Option<&[u8]>, RiscvGuestPathResolutionError> {
        let path = self.resolve_guest_path_following_intermediate_symlinks(path)?;
        let Some(path) = self.existing_guest_path_key(&path) else {
            return Ok(None);
        };
        Ok(self.guest_links.get(&path).map(Vec::as_slice))
    }

    fn guest_file_identity(&self, path: &[u8]) -> RiscvGuestFileIdentity {
        self.guest_file_identities
            .get(path)
            .copied()
            .unwrap_or_else(|| RiscvGuestFileIdentity {
                inode: guest_path_inode(path),
            })
    }

    fn guest_directory_identity(&self, path: &[u8]) -> RiscvGuestFileIdentity {
        self.guest_directory_identities
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
            self.drop_guest_xattrs_if_unlinked(identity);
        }
        removed_path || removed_file || removed_link
    }

    fn link_guest_path(
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

    fn symlink_guest_path(
        &mut self,
        link_path: &[u8],
        target: &[u8],
    ) -> Result<(), RiscvGuestSymlinkError> {
        if self.existing_guest_path_key(link_path).is_some()
            || self.guest_directory_entries(link_path).is_some()
        {
            return Err(RiscvGuestSymlinkError::DestinationExists);
        }
        let link_path = link_path.to_vec();
        self.guest_links.insert(link_path.clone(), target.to_vec());
        self.ensure_guest_file_identity(&link_path);
        Ok(())
    }

    #[cfg(test)]
    fn guest_path_stat(&self, path: &[u8]) -> Option<RiscvGuestStat> {
        self.guest_path_stat_result(path).ok().flatten()
    }

    #[cfg(test)]
    fn guest_path_stat_result(
        &self,
        path: &[u8],
    ) -> Result<Option<RiscvGuestStat>, RiscvGuestPathResolutionError> {
        self.guest_path_or_link_stat_result(path, false)
    }

    fn guest_path_or_link_stat_result(
        &self,
        path: &[u8],
        nofollow: bool,
    ) -> Result<Option<RiscvGuestStat>, RiscvGuestPathResolutionError> {
        if nofollow {
            let path = self.resolve_guest_path_following_intermediate_symlinks(path)?;
            if let Some(stat) = self.guest_link_stat_for_resolved_path(&path) {
                return Ok(Some(stat));
            }
            return Ok(self.guest_path_stat_for_resolved_path(&path));
        }

        let path = self.resolve_guest_path_following_symlinks(path)?;
        Ok(self.guest_path_stat_for_resolved_path(&path))
    }

    fn guest_path_stat_for_resolved_path(&self, path: &[u8]) -> Option<RiscvGuestStat> {
        let path = self
            .existing_guest_path_key(path)
            .unwrap_or_else(|| path.to_vec());
        if self.guest_path_registered(&path) {
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
        if self.guest_directory_entries(&path).is_some() {
            let identity = self.guest_directory_identity(&path);
            return Some(RiscvGuestStat::directory(
                self.identity(),
                identity.inode,
                self.guest_directory_permissions(&path),
            ));
        }
        None
    }

    fn guest_symlink_target_path(&self, link_path: &[u8], target: &[u8]) -> Vec<u8> {
        let directory = link_path
            .iter()
            .rposition(|byte| *byte == b'/')
            .map(|index| &link_path[..index])
            .unwrap_or(b"");
        self.canonical_guest_path_from_directory(directory, target)
    }

    #[cfg(test)]
    fn guest_link_stat(&self, path: &[u8]) -> Option<RiscvGuestStat> {
        let path = self.resolve_existing_guest_path(path).ok().flatten()?;
        self.guest_link_stat_for_resolved_path(&path)
    }

    fn guest_link_stat_for_resolved_path(&self, path: &[u8]) -> Option<RiscvGuestStat> {
        let path = self.existing_guest_path_key(path)?;
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
        let identity = if node_kind == RiscvGuestNodeKind::Directory {
            self.guest_directory_identity(&path)
        } else {
            self.guest_file_identity(&path)
        };
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
            self.release_guest_description_sources(description.id());
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
                self.release_guest_description_sources(description.id());
            }
        }
    }

    fn release_guest_description_sources(&mut self, description: GuestFileDescriptionId) {
        let closed_guest_file_identity = self
            .guest_file_stats
            .remove(&description)
            .map(|stat| stat.identity);
        self.guest_file_seals.remove(&description);
        self.guest_file_descriptions.remove(&description);
        self.guest_file_description_paths.remove(&description);
        self.guest_directory_descriptions.remove(&description);
        self.guest_directory_paths.remove(&description);
        self.remove_guest_pipe_description(description);
        self.remove_guest_eventfd_description(description);
        self.remove_guest_epoll_target_description(description);
        self.remove_guest_epoll_description(description);
        if let Some(identity) = closed_guest_file_identity {
            self.drop_guest_xattrs_if_unlinked(identity);
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
        self.next_guest_fd_excluding(&[])
    }

    fn next_guest_fd_excluding(&self, reserved: &[GuestFd]) -> Result<GuestFd, GuestFdError> {
        let snapshot = self.guest_fds.snapshot();
        let mut candidate = 0_i32;
        loop {
            let fd = GuestFd::new(candidate)?;
            if snapshot.entries().iter().all(|entry| entry.fd() != fd) && !reserved.contains(&fd) {
                return Ok(fd);
            }
            candidate = candidate
                .checked_add(1)
                .ok_or(GuestFdError::FdSpaceExhausted)?;
        }
    }

    fn next_open_description(&self) -> Result<GuestFileDescriptionId, GuestFdError> {
        self.next_guest_file_description_excluding(&[])
    }

    fn next_guest_file_description_excluding(
        &self,
        reserved: &[GuestFileDescriptionId],
    ) -> Result<GuestFileDescriptionId, GuestFdError> {
        let mut candidate = 0_u64;
        loop {
            let description = GuestFileDescriptionId::new(candidate);
            if self.guest_fds.description(description).is_none() && !reserved.contains(&description)
            {
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

#[cfg(test)]
#[path = "riscv_syscall_tests.rs"]
mod tests;
