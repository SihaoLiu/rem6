use crate::GuestFd;

use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
    RISCV_LINUX_ESRCH,
};

pub(super) const RISCV_LINUX_PRLIMIT64: u64 = 261;
pub(super) const RISCV_LINUX_GETRLIMIT: u64 = 163;

const RISCV_LINUX_RLIMIT_DATA: u64 = 2;
const RISCV_LINUX_RLIMIT_STACK: u64 = 3;
const RISCV_LINUX_RLIMIT_CORE: u64 = 4;
const RISCV_LINUX_RLIMIT_NPROC: u64 = 6;
const RISCV_LINUX_RLIMIT_NOFILE: u64 = 7;
const RISCV_LINUX_RLIMIT_AS: u64 = 9;
const RISCV_LINUX_RLIMIT_COUNT: usize = 16;
const RISCV_LINUX_RLIMIT_BYTES: usize = 16;
const RISCV_LINUX_DATA_LIMIT_BYTES: u64 = 256 * 1024 * 1024;
const RISCV_LINUX_SINGLE_PROCESS_COUNT: u64 = 1;
const RISCV_LINUX_OPEN_FILE_SOFT_LIMIT: u64 = 1024;
const RISCV_LINUX_OPEN_FILE_HARD_LIMIT: u64 = 4096;

pub const RISCV_LINUX_STACK_LIMIT_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvResourceLimit {
    current: u64,
    maximum: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiscvResourceLimits {
    limits: [Option<RiscvResourceLimit>; RISCV_LINUX_RLIMIT_COUNT],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvResourceLimitKind {
    Data,
    Stack,
    Core,
    Nproc,
    NoFile,
    AddressSpace,
}

impl RiscvResourceLimitKind {
    const fn index(self) -> usize {
        match self {
            Self::Data => RISCV_LINUX_RLIMIT_DATA as usize,
            Self::Stack => RISCV_LINUX_RLIMIT_STACK as usize,
            Self::Core => RISCV_LINUX_RLIMIT_CORE as usize,
            Self::Nproc => RISCV_LINUX_RLIMIT_NPROC as usize,
            Self::NoFile => RISCV_LINUX_RLIMIT_NOFILE as usize,
            Self::AddressSpace => RISCV_LINUX_RLIMIT_AS as usize,
        }
    }
}

impl RiscvResourceLimit {
    const fn new(current: u64, maximum: u64) -> Self {
        Self { current, maximum }
    }

    const fn current(self) -> u64 {
        self.current
    }

    const fn maximum(self) -> u64 {
        self.maximum
    }
}

impl RiscvResourceLimits {
    pub(super) const fn linux_single_process() -> Self {
        let mut limits = [None; RISCV_LINUX_RLIMIT_COUNT];
        limits[RiscvResourceLimitKind::Data.index()] = Some(RiscvResourceLimit::new(
            RISCV_LINUX_DATA_LIMIT_BYTES,
            RISCV_LINUX_DATA_LIMIT_BYTES,
        ));
        limits[RiscvResourceLimitKind::Stack.index()] = Some(RiscvResourceLimit::new(
            RISCV_LINUX_STACK_LIMIT_BYTES,
            RISCV_LINUX_STACK_LIMIT_BYTES,
        ));
        limits[RiscvResourceLimitKind::Core.index()] =
            Some(RiscvResourceLimit::new(u64::MAX, u64::MAX));
        limits[RiscvResourceLimitKind::Nproc.index()] = Some(RiscvResourceLimit::new(
            RISCV_LINUX_SINGLE_PROCESS_COUNT,
            RISCV_LINUX_SINGLE_PROCESS_COUNT,
        ));
        limits[RiscvResourceLimitKind::NoFile.index()] = Some(RiscvResourceLimit::new(
            RISCV_LINUX_OPEN_FILE_SOFT_LIMIT,
            RISCV_LINUX_OPEN_FILE_HARD_LIMIT,
        ));
        limits[RiscvResourceLimitKind::AddressSpace.index()] =
            Some(RiscvResourceLimit::new(u64::MAX, u64::MAX));
        Self { limits }
    }

    const fn get(&self, kind: RiscvResourceLimitKind) -> RiscvResourceLimit {
        match self.limits[kind.index()] {
            Some(limit) => limit,
            None => RiscvResourceLimit::new(u64::MAX, u64::MAX),
        }
    }

    fn set(&mut self, kind: RiscvResourceLimitKind, limit: RiscvResourceLimit) {
        self.limits[kind.index()] = Some(limit);
    }
}

impl RiscvSyscallState {
    fn resource_limit(&self, kind: RiscvResourceLimitKind) -> RiscvResourceLimit {
        self.resource_limits.get(kind)
    }

    fn set_resource_limit(&mut self, kind: RiscvResourceLimitKind, limit: RiscvResourceLimit) {
        self.resource_limits.set(kind, limit);
    }

    pub(super) fn open_file_soft_limit(&self) -> u64 {
        self.resource_limit(RiscvResourceLimitKind::NoFile)
            .current()
    }

    pub(super) fn guest_fd_is_below_open_file_limit(&self, fd: GuestFd) -> bool {
        u64::from(fd.get()) < self.open_file_soft_limit()
    }

    pub(super) fn has_open_file_capacity(&self, additional_fds: usize) -> bool {
        let Some(open_after) = self.guest_fds().len().checked_add(additional_fds) else {
            return false;
        };
        u64::try_from(open_after).is_ok_and(|open_after| open_after <= self.open_file_soft_limit())
    }
}

pub(super) fn syscall_prlimit64(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let requested = match read_optional_resource_limit(request.argument(2), guest_memory_reader)? {
        Ok(limit) => limit,
        Err(error) => return Some(linux_error(error)),
    };

    if !prlimit_target_is_current_process(request.argument(0), state) {
        return Some(linux_error(RISCV_LINUX_ESRCH));
    }

    let Some(kind) = resource_limit_kind(request.argument(1)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let previous = state.resource_limit(kind);
    if let Some(limit) = requested {
        let limit = match validate_resource_limit(kind, limit, state) {
            Ok(limit) => limit,
            Err(error) => return Some(linux_error(error)),
        };
        state.set_resource_limit(kind, limit);
    }

    let old_limit_address = request.argument(3);
    if old_limit_address != 0 {
        let write_result = write_resource_limit(old_limit_address, previous, guest_memory_writer)?;
        if write_result != 0 {
            return Some(write_result);
        }
    }

    Some(0)
}

fn prlimit_target_is_current_process(pid_argument: u64, state: &RiscvSyscallState) -> bool {
    let pid = pid_argument as u32 as i32;
    if pid < 0 {
        return false;
    }
    pid == 0 || u64::try_from(pid).ok() == Some(state.identity().thread_group_id())
}

pub(super) fn syscall_getrlimit(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let Some(kind) = resource_limit_kind(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };

    write_resource_limit(
        request.argument(1),
        state.resource_limit(kind),
        guest_memory,
    )
}

pub(super) fn syscall_setrlimit(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let limit_address = request.argument(1);
    if limit_address == 0 {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    let requested = match read_resource_limit(limit_address, guest_memory_reader)? {
        Ok(limit) => limit,
        Err(error) => return Some(linux_error(error)),
    };
    let Some(kind) = resource_limit_kind(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let requested = match validate_resource_limit(kind, requested, state) {
        Ok(limit) => limit,
        Err(error) => return Some(linux_error(error)),
    };
    state.set_resource_limit(kind, requested);
    Some(0)
}

fn write_resource_limit(
    address: u64,
    limit: RiscvResourceLimit,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let guest_memory = guest_memory?;
    let bytes = rlimit_bytes(limit);
    for (offset, byte) in bytes.iter().enumerate() {
        let Some(address) = address.checked_add(offset as u64) else {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        };
        if !guest_memory.write(address, &[*byte]) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }

    Some(0)
}

fn read_optional_resource_limit(
    address: u64,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<Result<Option<RiscvResourceLimit>, u64>> {
    if address == 0 {
        return Some(Ok(None));
    }
    match read_resource_limit(address, guest_memory_reader)? {
        Ok(limit) => Some(Ok(Some(limit))),
        Err(error) => Some(Err(error)),
    }
}

fn validate_resource_limit(
    kind: RiscvResourceLimitKind,
    requested: RiscvResourceLimit,
    state: &RiscvSyscallState,
) -> Result<RiscvResourceLimit, u64> {
    if requested.current() > requested.maximum() {
        return Err(RISCV_LINUX_EINVAL);
    }
    if requested.maximum() > state.resource_limit(kind).maximum() {
        return Err(RISCV_LINUX_EPERM);
    }
    Ok(requested)
}

fn read_resource_limit(
    address: u64,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<Result<RiscvResourceLimit, u64>> {
    let guest_memory = guest_memory?;
    let Some(bytes) = guest_memory.read(address, RISCV_LINUX_RLIMIT_BYTES) else {
        return Some(Err(RISCV_LINUX_EFAULT));
    };
    if bytes.len() != RISCV_LINUX_RLIMIT_BYTES {
        return Some(Err(RISCV_LINUX_EFAULT));
    }
    let current = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
    let maximum = u64::from_le_bytes(bytes[8..16].try_into().ok()?);
    Some(Ok(RiscvResourceLimit::new(current, maximum)))
}

fn resource_limit_kind(resource: u64) -> Option<RiscvResourceLimitKind> {
    match resource {
        RISCV_LINUX_RLIMIT_DATA => Some(RiscvResourceLimitKind::Data),
        RISCV_LINUX_RLIMIT_STACK => Some(RiscvResourceLimitKind::Stack),
        RISCV_LINUX_RLIMIT_CORE => Some(RiscvResourceLimitKind::Core),
        RISCV_LINUX_RLIMIT_NPROC => Some(RiscvResourceLimitKind::Nproc),
        RISCV_LINUX_RLIMIT_NOFILE => Some(RiscvResourceLimitKind::NoFile),
        RISCV_LINUX_RLIMIT_AS => Some(RiscvResourceLimitKind::AddressSpace),
        _ => None,
    }
}

fn rlimit_bytes(limit: RiscvResourceLimit) -> [u8; RISCV_LINUX_RLIMIT_BYTES] {
    let mut bytes = [0; RISCV_LINUX_RLIMIT_BYTES];
    bytes[0..8].copy_from_slice(&limit.current().to_le_bytes());
    bytes[8..16].copy_from_slice(&limit.maximum().to_le_bytes());
    bytes
}
