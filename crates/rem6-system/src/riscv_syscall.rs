use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCore};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode, RiscvTrapKind};
use rem6_kernel::{PartitionedScheduler, Tick};

use crate::{
    GuestEventId, GuestFd, GuestFdEntry, GuestFdTable, GuestFileDescription,
    GuestFileDescriptionId, GuestFileStatusFlags, GuestFutexAddress, GuestFutexTable,
    GuestThreadGroupId, RiscvSystemRunDriver, ScheduledRiscvTrap, SystemError,
};

const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_CLOSE: u64 = 57;
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
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EINVAL: u64 = 22;
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
const RISCV_LINUX_O_RDONLY: u64 = 0;
const RISCV_LINUX_O_WRONLY: u64 = 1;
#[cfg(test)]
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_MAP_SHARED: u64 = 0x01;
const RISCV_LINUX_MAP_PRIVATE: u64 = 0x02;
const RISCV_LINUX_MAP_FIXED: u64 = 0x10;
const RISCV_LINUX_MAP_ANONYMOUS: u64 = 0x20;
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
pub struct RiscvSyscallState {
    identity: RiscvSyscallIdentity,
    child_clear_tid: Option<u64>,
    guest_fds: GuestFdTable,
    guest_futexes: GuestFutexTable,
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
        Self {
            identity,
            child_clear_tid: None,
            guest_fds: linux_standard_guest_fds(),
            guest_futexes: GuestFutexTable::new(),
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
        match request.number() {
            RISCV_LINUX_FCNTL => syscall_fcntl(request, state),
            RISCV_LINUX_CLOSE => Some(RiscvSyscallOutcome::Return {
                value: syscall_close(request.argument(0), state),
            }),
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
}

impl RiscvSyscallEmulation {
    pub fn new(table: RiscvSyscallTable, state: RiscvSyscallState) -> Self {
        Self {
            table,
            state: Arc::new(Mutex::new(state)),
        }
    }

    pub fn linux_user() -> Self {
        Self::new(RiscvSyscallTable::new(), RiscvSyscallState::new(0))
    }

    pub const fn table(&self) -> RiscvSyscallTable {
        self.table
    }

    pub fn state(&self) -> RiscvSyscallState {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .clone()
    }

    pub fn handle_pending_core_trap(
        &self,
        core: &RiscvCore,
        tick: Tick,
    ) -> Option<RiscvSyscallOutcome> {
        let request = RiscvSyscallRequest::from_pending_core_trap(core)?;
        let mut state = self.state.lock().expect("RISC-V syscall state lock");
        self.table.handle_at_tick(request, &mut state, tick)
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
        Ok(_record) => 0,
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
mod tests {
    use super::*;
    use crate::{
        GuestFd, GuestFileStatusFlags, GuestFutexAddress, GuestFutexKey, GuestFutexWaitRequest,
        GuestThreadGroupId, GuestThreadId,
    };
    use rem6_kernel::PartitionId;

    #[test]
    fn linux_table_maps_exit_numbers_to_stop_codes() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT, [17; 6]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Exit { code: 17 })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT_GROUP, [19; 6]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Exit { code: 19 })
        );
    }

    #[test]
    fn linux_table_tracks_program_break() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_BRK, [64, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 64 })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_BRK, [0; 6]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 64 })
        );
        assert_eq!(state.program_break(), 64);
    }

    #[test]
    fn linux_table_returns_process_identity() {
        let table = RiscvSyscallTable::new();
        let mut state =
            RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

        for (number, value) in [
            (RISCV_LINUX_GETPID, 41),
            (RISCV_LINUX_GETTID, 42),
            (RISCV_LINUX_GETPPID, 43),
            (RISCV_LINUX_GETUID, 7),
            (RISCV_LINUX_GETEUID, 8),
            (RISCV_LINUX_GETGID, 9),
            (RISCV_LINUX_GETEGID, 10),
        ] {
            assert_eq!(
                table.handle(RiscvSyscallRequest::new(0x8000, number, [0; 6]), &mut state,),
                Some(RiscvSyscallOutcome::Return { value })
            );
        }
    }

    #[test]
    fn linux_table_uses_gem5_default_process_identity() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        for number in [
            RISCV_LINUX_GETPID,
            RISCV_LINUX_GETTID,
            RISCV_LINUX_GETUID,
            RISCV_LINUX_GETEUID,
            RISCV_LINUX_GETGID,
            RISCV_LINUX_GETEGID,
        ] {
            assert_eq!(
                table.handle(RiscvSyscallRequest::new(0x8000, number, [0; 6]), &mut state,),
                Some(RiscvSyscallOutcome::Return { value: 100 })
            );
        }
    }

    #[test]
    fn linux_table_returns_parent_process_identity() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETPPID, [77, 0, 0, 0, 0, 0],),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }

    #[test]
    fn linux_table_records_child_clear_tid_address() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_SET_TID_ADDRESS,
                    [0x1234, 0, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 100 })
        );
        assert_eq!(state.child_clear_tid(), Some(0x1234));
    }

    #[test]
    fn linux_table_clears_child_clear_tid_address_with_zero() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_SET_TID_ADDRESS,
                    [0x1234, 0, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 100 })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SET_TID_ADDRESS, [0; 6]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 100 })
        );
        assert_eq!(state.child_clear_tid(), None);
    }

    #[test]
    fn linux_table_ignores_gem5_warn_once_startup_syscalls() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        for number in [
            RISCV_LINUX_SET_ROBUST_LIST,
            RISCV_LINUX_GET_ROBUST_LIST,
            RISCV_LINUX_NANOSLEEP,
            RISCV_LINUX_SCHED_YIELD,
            RISCV_LINUX_RT_SIGSUSPEND,
            RISCV_LINUX_RT_SIGACTION,
            RISCV_LINUX_RT_SIGPROCMASK,
            RISCV_LINUX_RT_SIGPENDING,
            RISCV_LINUX_RT_SIGTIMEDWAIT,
            RISCV_LINUX_RT_SIGQUEUEINFO,
            RISCV_LINUX_RT_SIGRETURN,
        ] {
            assert_eq!(
                table.handle(RiscvSyscallRequest::new(0x8000, number, [0; 6]), &mut state,),
                Some(RiscvSyscallOutcome::Return { value: 0 })
            );
        }
    }

    #[test]
    fn linux_table_ignores_gem5_memory_management_advisory_syscalls() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        for number in [
            RISCV_LINUX_MPROTECT,
            RISCV_LINUX_MSYNC,
            RISCV_LINUX_MLOCK,
            RISCV_LINUX_MUNLOCK,
            RISCV_LINUX_MLOCKALL,
            RISCV_LINUX_MUNLOCKALL,
            RISCV_LINUX_MINCORE,
            RISCV_LINUX_MADVISE,
            RISCV_LINUX_MBIND,
        ] {
            assert_eq!(
                table.handle(
                    RiscvSyscallRequest::new(0x8000, number, [0x4000, 4096, 0, 0, 0, 0]),
                    &mut state,
                ),
                Some(RiscvSyscallOutcome::Return { value: 0 })
            );
        }
    }

    #[test]
    fn linux_table_returns_enosys_for_gem5_ignored_rseq() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_RSEQ, [0x4000, 32, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_ENOSYS)
            })
        );
    }

    #[test]
    fn linux_table_handles_fcntl_descriptor_and_status_flags() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);
        let stdout = GuestFd::new(1).unwrap();

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_FCNTL,
                    [1, RISCV_LINUX_F_SETFD, RISCV_LINUX_FD_CLOEXEC, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        assert!(state.guest_fds().close_on_exec(stdout).unwrap());
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    RISCV_LINUX_FCNTL,
                    [1, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV_LINUX_FD_CLOEXEC
            })
        );

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8008,
                    RISCV_LINUX_FCNTL,
                    [1, RISCV_LINUX_F_SETFL, RISCV_LINUX_O_NONBLOCK, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        assert_eq!(
            state.guest_fds().status_flags(stdout).unwrap(),
            GuestFileStatusFlags::new(RISCV_LINUX_O_WRONLY as u32 | RISCV_LINUX_O_NONBLOCK as u32)
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x800c,
                    RISCV_LINUX_FCNTL,
                    [1, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_NONBLOCK
            })
        );
    }

    #[test]
    fn linux_table_closes_guest_fd_and_rejects_reuse() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);
        let stdout = GuestFd::new(1).unwrap();
        let stdout_description = GuestFileDescriptionId::new(1);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CLOSE, [1, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        assert!(state.guest_fds().entry(stdout).is_none());
        assert!(state.guest_fds().description(stdout_description).is_none());
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    RISCV_LINUX_FCNTL,
                    [1, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EBADF)
            })
        );
    }

    #[test]
    fn linux_table_returns_ebadf_for_close_on_unknown_fd() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CLOSE, [99, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EBADF)
            })
        );
    }

    #[test]
    fn linux_table_returns_ebadf_for_fcntl_on_unknown_fd() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_FCNTL,
                    [99, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EBADF)
            })
        );
    }

    #[test]
    fn linux_table_returns_ebadf_for_fcntl_on_out_of_range_fd() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_FCNTL,
                    [(1_u64 << 32) | 1, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EBADF)
            })
        );
    }

    #[test]
    fn linux_table_leaves_unsupported_fcntl_commands_unhandled_before_fd_validation() {
        const RISCV_LINUX_F_DUPFD: u64 = 0;

        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_FCNTL,
                    [u64::MAX, RISCV_LINUX_F_DUPFD, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            None
        );
    }

    #[test]
    fn linux_table_wakes_guest_futex_waiters() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);
        let address = GuestFutexAddress::new(0x180);
        let thread_group = GuestThreadGroupId::new(100);
        let key = GuestFutexKey::new(address, thread_group);

        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(7),
                PartitionId::new(1),
                20,
                3,
                3,
            ))
            .unwrap();
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(8),
                PartitionId::new(2),
                21,
                3,
                3,
            ))
            .unwrap();

        assert_eq!(
            table.handle_at_tick(
                RiscvSyscallRequest::new(0x8000, 98, [address.get(), 1, 1, 0, 0, 0]),
                &mut state,
                40,
            ),
            Some(RiscvSyscallOutcome::Return { value: 1 })
        );
        assert_eq!(
            state.guest_futexes().waiter_threads(address, thread_group),
            vec![GuestThreadId::new(8)]
        );
    }

    #[test]
    fn linux_table_wakes_guest_futex_waiters_by_bitset() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);
        let address = GuestFutexAddress::new(0x184);
        let thread_group = GuestThreadGroupId::new(100);
        let key = GuestFutexKey::new(address, thread_group);

        state
            .guest_futexes_mut()
            .wait(
                GuestFutexWaitRequest::new(
                    key,
                    GuestThreadId::new(9),
                    PartitionId::new(1),
                    22,
                    4,
                    4,
                )
                .with_bitset(0b01),
            )
            .unwrap();
        state
            .guest_futexes_mut()
            .wait(
                GuestFutexWaitRequest::new(
                    key,
                    GuestThreadId::new(10),
                    PartitionId::new(2),
                    23,
                    4,
                    4,
                )
                .with_bitset(0b10),
            )
            .unwrap();

        assert_eq!(
            table.handle_at_tick(
                RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 0, 0, 0, 0b01]),
                &mut state,
                41,
            ),
            Some(RiscvSyscallOutcome::Return { value: 1 })
        );
        assert_eq!(
            state.guest_futexes().waiter_threads(address, thread_group),
            vec![GuestThreadId::new(10)]
        );
    }

    #[test]
    fn linux_table_allocates_anonymous_mmap_regions() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_MMAP, [0, 64, 3, 34, u64::MAX, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV64_LINUX_MMAP_BASE
            })
        );
        assert_eq!(
            state.mmap_regions(),
            &[RiscvMmapRegion::new(
                RISCV64_LINUX_MMAP_BASE,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                0,
            )]
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    RISCV_LINUX_MMAP,
                    [0, RISCV_PAGE_BYTES, 1, 34, u64::MAX, 0]
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES
            })
        );
        assert_eq!(
            state.mmap_next(),
            RISCV64_LINUX_MMAP_BASE + (2 * RISCV_PAGE_BYTES)
        );
    }

    #[test]
    fn linux_table_rejects_invalid_mmap_arguments() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_MMAP, [0, 0, 3, 34, u64::MAX, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MMAP, [1, 64, 3, 34, u64::MAX, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert!(state.mmap_regions().is_empty());
    }

    #[test]
    fn linux_table_fixed_mmap_preserves_non_overlapping_fragments() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);
        let fixed_start = RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES;

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_MMAP,
                    [0, 3 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV64_LINUX_MMAP_BASE
            })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    RISCV_LINUX_MMAP,
                    [
                        fixed_start,
                        RISCV_PAGE_BYTES,
                        1,
                        34 | RISCV_LINUX_MAP_FIXED,
                        u64::MAX,
                        0,
                    ]
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: fixed_start })
        );
        assert_eq!(
            state.mmap_regions(),
            &[
                RiscvMmapRegion::new(
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    3,
                    34,
                    u64::MAX,
                    0,
                ),
                RiscvMmapRegion::new(
                    fixed_start,
                    RISCV_PAGE_BYTES,
                    1,
                    34 | RISCV_LINUX_MAP_FIXED,
                    u64::MAX,
                    0,
                ),
                RiscvMmapRegion::new(
                    fixed_start + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    3,
                    34,
                    u64::MAX,
                    2 * RISCV_PAGE_BYTES,
                ),
            ]
        );
    }

    #[test]
    fn linux_table_munmap_removes_mapped_ranges() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);
        let unmap_start = RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES;

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_MMAP,
                    [0, 3 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV64_LINUX_MMAP_BASE
            })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    RISCV_LINUX_MUNMAP,
                    [unmap_start, RISCV_PAGE_BYTES, 0, 0, 0, 0]
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        assert_eq!(
            state.mmap_regions(),
            &[
                RiscvMmapRegion::new(
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    3,
                    34,
                    u64::MAX,
                    0,
                ),
                RiscvMmapRegion::new(
                    unmap_start + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    3,
                    34,
                    u64::MAX,
                    2 * RISCV_PAGE_BYTES,
                ),
            ]
        );
    }

    #[test]
    fn linux_table_rejects_invalid_munmap_arguments() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_MMAP,
                    [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV64_LINUX_MMAP_BASE
            })
        );
        let mapped_regions = state.mmap_regions().to_vec();

        for arguments in [
            [RISCV64_LINUX_MMAP_BASE + 1, RISCV_PAGE_BYTES, 0, 0, 0, 0],
            [RISCV64_LINUX_MMAP_BASE, 0, 0, 0, 0, 0],
            [RISCV64_LINUX_MMAP_BASE, u64::MAX, 0, 0, 0, 0],
            [
                u64::MAX - (RISCV_PAGE_BYTES - 1),
                RISCV_PAGE_BYTES,
                0,
                0,
                0,
                0,
            ],
        ] {
            assert_eq!(
                table.handle(
                    RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MUNMAP, arguments),
                    &mut state,
                ),
                Some(RiscvSyscallOutcome::Return {
                    value: linux_error(RISCV_LINUX_EINVAL)
                })
            );
            assert_eq!(state.mmap_regions(), mapped_regions.as_slice());
        }
    }

    #[test]
    fn linux_table_rejects_overflowing_fixed_mmap() {
        let table = RiscvSyscallTable::new();
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_MMAP,
                    [
                        u64::MAX - (RISCV_PAGE_BYTES - 1),
                        RISCV_PAGE_BYTES,
                        3,
                        34 | RISCV_LINUX_MAP_FIXED,
                        u64::MAX,
                        0,
                    ]
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert!(state.mmap_regions().is_empty());
    }

    #[test]
    fn linux_table_leaves_unknown_numbers_for_the_trap_path() {
        let mut state = RiscvSyscallState::new(0);

        assert_eq!(
            RiscvSyscallTable::new()
                .handle(RiscvSyscallRequest::new(0x8000, 9999, [0; 6]), &mut state,),
            None
        );
    }
}
