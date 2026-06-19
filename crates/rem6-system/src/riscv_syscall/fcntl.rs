use crate::{GuestFd, GuestFdError, GuestFileStatusFlags};

use super::{
    guest_fd_argument, linux_error, pipe::RiscvGuestPipeCapacityError, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EBADF, RISCV_LINUX_EBUSY, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_EPERM, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_APPEND,
    RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_FCNTL: u64 = 25;
pub(super) const RISCV_LINUX_F_DUPFD: u64 = 0;
pub(super) const RISCV_LINUX_F_GETFD: u64 = 1;
pub(super) const RISCV_LINUX_F_SETFD: u64 = 2;
pub(super) const RISCV_LINUX_F_GETFL: u64 = 3;
pub(super) const RISCV_LINUX_F_SETFL: u64 = 4;
pub(super) const RISCV_LINUX_F_GETLK: u64 = 5;
pub(super) const RISCV_LINUX_F_SETLK: u64 = 6;
pub(super) const RISCV_LINUX_F_SETLKW: u64 = 7;
pub(super) const RISCV_LINUX_F_DUPFD_CLOEXEC: u64 = 1030;
pub(super) const RISCV_LINUX_F_SETPIPE_SZ: u64 = 1031;
pub(super) const RISCV_LINUX_F_GETPIPE_SZ: u64 = 1032;
const RISCV_LINUX_F_ADD_SEALS: u64 = 1033;
const RISCV_LINUX_F_GET_SEALS: u64 = 1034;
pub(super) const RISCV_LINUX_FD_CLOEXEC: u64 = 1;
pub(super) const RISCV_LINUX_F_SEAL_SEAL: u32 = 0x0001;
const RISCV_LINUX_F_SEAL_SHRINK: u32 = 0x0002;
const RISCV_LINUX_F_SEAL_GROW: u32 = 0x0004;
const RISCV_LINUX_F_SEAL_WRITE: u32 = 0x0008;
const RISCV_LINUX_F_SEAL_FUTURE_WRITE: u32 = 0x0010;
const RISCV_LINUX_F_SEAL_SUPPORTED: u32 = RISCV_LINUX_F_SEAL_SEAL
    | RISCV_LINUX_F_SEAL_SHRINK
    | RISCV_LINUX_F_SEAL_GROW
    | RISCV_LINUX_F_SEAL_WRITE
    | RISCV_LINUX_F_SEAL_FUTURE_WRITE;

const RISCV_LINUX_F_RDLCK: u16 = 0;
const RISCV_LINUX_F_WRLCK: u16 = 1;
const RISCV_LINUX_F_UNLCK: u16 = 2;
const RISCV_LINUX_SEEK_SET: u16 = 0;
const RISCV_LINUX_SEEK_CUR: u16 = 1;
const RISCV_LINUX_SEEK_END: u16 = 2;
const RISCV_LINUX_FLOCK_BYTES: usize = 32;
const RISCV_LINUX_FLOCK_TYPE_OFFSET: usize = 0;
const RISCV_LINUX_FLOCK_WHENCE_OFFSET: usize = 2;
const RISCV_LINUX_FLOCK_START_OFFSET: usize = 8;
const RISCV_LINUX_FLOCK_LEN_OFFSET: usize = 16;
const RISCV_LINUX_FLOCK_PID_OFFSET: usize = 24;

pub(super) fn syscall_fcntl(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    let fd = match guest_fd_argument(request.argument(0)) {
        Some(fd) => fd,
        None => return Some(guest_fd_error_return()),
    };
    if state.guest_fds.entry(fd).is_none() {
        return Some(guest_fd_error_return());
    }

    let command = match RiscvFcntlCommand::from_raw(request.argument(1)) {
        Some(command) => command,
        None => {
            return Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL),
            });
        }
    };

    let outcome: RiscvFcntlResult = match command {
        RiscvFcntlCommand::DuplicateFd { close_on_exec } => {
            let Some(minimum_fd) = guest_fd_argument(request.argument(2)) else {
                return Some(RiscvSyscallOutcome::Return {
                    value: linux_error(RISCV_LINUX_EINVAL),
                });
            };
            match state.guest_fds.dup_from_min(fd, minimum_fd) {
                Ok(new_fd) => {
                    state.duplicate_fd_source(fd, new_fd);
                    if close_on_exec && state.guest_fds.set_close_on_exec(new_fd, true).is_err() {
                        return Some(guest_fd_error_return());
                    }
                    Ok(u64::from(new_fd.get()))
                }
                Err(GuestFdError::FdSpaceExhausted) => Err(GuestFdError::FdSpaceExhausted),
                Err(error) => Err(error),
            }
            .map_err(RiscvFcntlError::GuestFd)
        }
        RiscvFcntlCommand::GetFdFlags => state
            .guest_fds
            .close_on_exec(fd)
            .map(|close| u64::from(close) * RISCV_LINUX_FD_CLOEXEC)
            .map_err(RiscvFcntlError::GuestFd),
        RiscvFcntlCommand::SetFdFlags => state
            .guest_fds
            .set_close_on_exec(fd, request.argument(2) & RISCV_LINUX_FD_CLOEXEC != 0)
            .map(|()| 0)
            .map_err(RiscvFcntlError::GuestFd),
        RiscvFcntlCommand::GetStatusFlags => state
            .guest_fds
            .status_flags(fd)
            .map(|flags| u64::from(flags.bits()))
            .map_err(RiscvFcntlError::GuestFd),
        RiscvFcntlCommand::SetStatusFlags => {
            let current = match state.guest_fds.status_flags(fd) {
                Ok(flags) => flags,
                Err(_error) => return Some(guest_fd_error_return()),
            };
            let requested = request.argument(2) as u32;
            let mutable_flags = (RISCV_LINUX_O_APPEND | RISCV_LINUX_O_NONBLOCK) as u32;
            state
                .guest_fds
                .set_status_flags(
                    fd,
                    GuestFileStatusFlags::new(
                        (current.bits() & !mutable_flags) | (requested & mutable_flags),
                    ),
                )
                .map(|()| 0)
                .map_err(RiscvFcntlError::GuestFd)
        }
        RiscvFcntlCommand::GetLock => {
            advisory_lock_request(fd, request.argument(2), state, guest_memory_reader)
                .and_then(validate_get_lock_request)
                .and_then(|lock| {
                    write_no_conflict_lock(lock, request.argument(2), guest_memory_writer)
                })
                .map_err(RiscvFcntlError::Linux)
        }
        RiscvFcntlCommand::SetLock => {
            advisory_lock_request(fd, request.argument(2), state, guest_memory_reader)
                .and_then(validate_set_lock_request)
                .and_then(|lock| validate_set_lock_access(fd, lock, state))
                .map(|_lock| 0)
                .map_err(RiscvFcntlError::Linux)
        }
        RiscvFcntlCommand::GetPipeSize => match state.guest_pipe_capacity(fd) {
            Ok(Some(capacity)) => Ok(capacity as u64),
            Ok(None) => Err(RiscvFcntlError::Linux(RISCV_LINUX_EBADF)),
            Err(error) => Err(RiscvFcntlError::GuestFd(error)),
        },
        RiscvFcntlCommand::GetSeals => get_memfd_seals(fd, state),
        RiscvFcntlCommand::AddSeals => add_memfd_seals(fd, request.argument(2), state),
        RiscvFcntlCommand::SetPipeSize => {
            match state.set_guest_pipe_capacity(fd, request.argument(2)) {
                Ok(Some(capacity)) => Ok(capacity as u64),
                Ok(None) => Err(RiscvFcntlError::Linux(RISCV_LINUX_EBADF)),
                Err(RiscvGuestPipeCapacityError::Fd(error)) => Err(RiscvFcntlError::GuestFd(error)),
                Err(RiscvGuestPipeCapacityError::Busy) => {
                    Err(RiscvFcntlError::Linux(RISCV_LINUX_EBUSY))
                }
                Err(RiscvGuestPipeCapacityError::Permission) => {
                    Err(RiscvFcntlError::Linux(RISCV_LINUX_EPERM))
                }
                Err(RiscvGuestPipeCapacityError::Invalid) => {
                    Err(RiscvFcntlError::Linux(RISCV_LINUX_EINVAL))
                }
            }
        }
    };

    Some(match outcome {
        Ok(value) => RiscvSyscallOutcome::Return { value },
        Err(RiscvFcntlError::GuestFd(GuestFdError::FdSpaceExhausted)) => {
            RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EMFILE),
            }
        }
        Err(RiscvFcntlError::GuestFd(_error)) => guest_fd_error_return(),
        Err(RiscvFcntlError::Linux(error)) => RiscvSyscallOutcome::Return {
            value: linux_error(error),
        },
    })
}

type RiscvFcntlResult = Result<u64, RiscvFcntlError>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum RiscvFcntlError {
    GuestFd(GuestFdError),
    Linux(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvFcntlCommand {
    DuplicateFd { close_on_exec: bool },
    GetFdFlags,
    SetFdFlags,
    GetStatusFlags,
    SetStatusFlags,
    GetLock,
    SetLock,
    GetPipeSize,
    SetPipeSize,
    GetSeals,
    AddSeals,
}

impl RiscvFcntlCommand {
    const fn from_raw(raw: u64) -> Option<Self> {
        match raw {
            RISCV_LINUX_F_DUPFD => Some(Self::DuplicateFd {
                close_on_exec: false,
            }),
            RISCV_LINUX_F_GETFD => Some(Self::GetFdFlags),
            RISCV_LINUX_F_SETFD => Some(Self::SetFdFlags),
            RISCV_LINUX_F_GETFL => Some(Self::GetStatusFlags),
            RISCV_LINUX_F_SETFL => Some(Self::SetStatusFlags),
            RISCV_LINUX_F_GETLK => Some(Self::GetLock),
            RISCV_LINUX_F_SETLK | RISCV_LINUX_F_SETLKW => Some(Self::SetLock),
            RISCV_LINUX_F_DUPFD_CLOEXEC => Some(Self::DuplicateFd {
                close_on_exec: true,
            }),
            RISCV_LINUX_F_SETPIPE_SZ => Some(Self::SetPipeSize),
            RISCV_LINUX_F_GETPIPE_SZ => Some(Self::GetPipeSize),
            RISCV_LINUX_F_ADD_SEALS => Some(Self::AddSeals),
            RISCV_LINUX_F_GET_SEALS => Some(Self::GetSeals),
            _ => None,
        }
    }
}

impl RiscvSyscallState {
    pub(super) fn guest_fd_write_denied_by_file_seal(
        &self,
        fd: GuestFd,
    ) -> Result<bool, GuestFdError> {
        let Some(seals) = self.guest_fd_file_seals(fd)? else {
            return Ok(false);
        };
        Ok(seals & (RISCV_LINUX_F_SEAL_WRITE | RISCV_LINUX_F_SEAL_FUTURE_WRITE) != 0)
    }

    pub(super) fn guest_fd_resize_denied_by_file_seal(
        &self,
        fd: GuestFd,
        length: u64,
    ) -> Result<bool, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        let Some(seals) = self.guest_file_seals.get(&description).copied() else {
            return Ok(false);
        };
        let Some(contents) = self.guest_file_descriptions.get(&description) else {
            return Ok(false);
        };
        let current = contents.len() as u64;
        Ok((length < current && seals & RISCV_LINUX_F_SEAL_SHRINK != 0)
            || (length > current && seals & RISCV_LINUX_F_SEAL_GROW != 0))
    }

    fn guest_fd_file_seals(&self, fd: GuestFd) -> Result<Option<u32>, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        Ok(self.guest_file_seals.get(&description).copied())
    }

    fn set_guest_fd_file_seals(&mut self, fd: GuestFd, seals: u32) -> Result<bool, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        let Some(current) = self.guest_file_seals.get_mut(&description) else {
            return Ok(false);
        };
        *current = seals;
        Ok(true)
    }
}

fn get_memfd_seals(fd: GuestFd, state: &RiscvSyscallState) -> RiscvFcntlResult {
    match state.guest_fd_file_seals(fd) {
        Ok(Some(seals)) => Ok(u64::from(seals)),
        Ok(None) => Err(RiscvFcntlError::Linux(RISCV_LINUX_EINVAL)),
        Err(error) => Err(RiscvFcntlError::GuestFd(error)),
    }
}

fn add_memfd_seals(
    fd: GuestFd,
    seals_argument: u64,
    state: &mut RiscvSyscallState,
) -> RiscvFcntlResult {
    let seals = seals_argument as u32;
    if seals & !RISCV_LINUX_F_SEAL_SUPPORTED != 0 {
        return Err(RiscvFcntlError::Linux(RISCV_LINUX_EINVAL));
    }
    let current = match state.guest_fd_file_seals(fd) {
        Ok(Some(seals)) => seals,
        Ok(None) => return Err(RiscvFcntlError::Linux(RISCV_LINUX_EINVAL)),
        Err(error) => return Err(RiscvFcntlError::GuestFd(error)),
    };
    if seals != 0 && current & RISCV_LINUX_F_SEAL_SEAL != 0 {
        return Err(RiscvFcntlError::Linux(RISCV_LINUX_EPERM));
    }
    match state.set_guest_fd_file_seals(fd, current | seals) {
        Ok(true) => Ok(0),
        Ok(false) => Err(RiscvFcntlError::Linux(RISCV_LINUX_EINVAL)),
        Err(error) => Err(RiscvFcntlError::GuestFd(error)),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvLinuxFlock {
    lock_type: u16,
    whence: u16,
    start: i64,
    len: i64,
    pid: i32,
}

impl RiscvLinuxFlock {
    const fn no_conflict(self) -> Self {
        Self {
            lock_type: RISCV_LINUX_F_UNLCK,
            ..self
        }
    }

    const fn has_valid_whence(self) -> bool {
        matches!(
            self.whence,
            RISCV_LINUX_SEEK_SET | RISCV_LINUX_SEEK_CUR | RISCV_LINUX_SEEK_END
        )
    }

    const fn is_get_lock_request(self) -> bool {
        matches!(self.lock_type, RISCV_LINUX_F_RDLCK | RISCV_LINUX_F_WRLCK)
            && self.has_valid_whence()
    }

    const fn is_set_lock_request(self) -> bool {
        matches!(
            self.lock_type,
            RISCV_LINUX_F_RDLCK | RISCV_LINUX_F_WRLCK | RISCV_LINUX_F_UNLCK
        ) && self.has_valid_whence()
    }
}

fn advisory_lock_request(
    fd: crate::GuestFd,
    address: u64,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Result<RiscvLinuxFlock, u64> {
    if state.guest_fds.entry(fd).is_none() {
        return Err(RISCV_LINUX_EBADF);
    }
    read_linux_flock(address, guest_memory_reader)
}

fn validate_get_lock_request(lock: RiscvLinuxFlock) -> Result<RiscvLinuxFlock, u64> {
    if !lock.is_get_lock_request() {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(lock)
}

fn validate_set_lock_request(lock: RiscvLinuxFlock) -> Result<RiscvLinuxFlock, u64> {
    if !lock.is_set_lock_request() {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(lock)
}

fn validate_set_lock_access(
    fd: crate::GuestFd,
    lock: RiscvLinuxFlock,
    state: &RiscvSyscallState,
) -> Result<RiscvLinuxFlock, u64> {
    if lock.lock_type == RISCV_LINUX_F_UNLCK {
        return Ok(lock);
    }
    let status_flags = state
        .guest_fds
        .status_flags(fd)
        .map_err(|_error| RISCV_LINUX_EBADF)?;
    let access_mode = u64::from(status_flags.bits()) & RISCV_LINUX_O_ACCMODE;
    if (lock.lock_type == RISCV_LINUX_F_WRLCK && access_mode == RISCV_LINUX_O_RDONLY)
        || (lock.lock_type == RISCV_LINUX_F_RDLCK && access_mode == RISCV_LINUX_O_WRONLY)
    {
        return Err(RISCV_LINUX_EBADF);
    }
    Ok(lock)
}

fn read_linux_flock(
    address: u64,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Result<RiscvLinuxFlock, u64> {
    let Some(guest_memory) = guest_memory_reader else {
        return Err(RISCV_LINUX_EFAULT);
    };
    let bytes = read_guest_bytes(guest_memory, address, RISCV_LINUX_FLOCK_BYTES)?;
    Ok(RiscvLinuxFlock {
        lock_type: read_u16(&bytes, RISCV_LINUX_FLOCK_TYPE_OFFSET),
        whence: read_u16(&bytes, RISCV_LINUX_FLOCK_WHENCE_OFFSET),
        start: read_i64(&bytes, RISCV_LINUX_FLOCK_START_OFFSET),
        len: read_i64(&bytes, RISCV_LINUX_FLOCK_LEN_OFFSET),
        pid: read_i32(&bytes, RISCV_LINUX_FLOCK_PID_OFFSET),
    })
}

fn write_no_conflict_lock(
    lock: RiscvLinuxFlock,
    address: u64,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Result<u64, u64> {
    let Some(guest_memory) = guest_memory_writer else {
        return Err(RISCV_LINUX_EFAULT);
    };
    if guest_memory.write(address, &encode_linux_flock(lock.no_conflict())) {
        Ok(0)
    } else {
        Err(RISCV_LINUX_EFAULT)
    }
}

fn encode_linux_flock(lock: RiscvLinuxFlock) -> [u8; RISCV_LINUX_FLOCK_BYTES] {
    let mut bytes = [0; RISCV_LINUX_FLOCK_BYTES];
    bytes[RISCV_LINUX_FLOCK_TYPE_OFFSET..RISCV_LINUX_FLOCK_TYPE_OFFSET + 2]
        .copy_from_slice(&lock.lock_type.to_le_bytes());
    bytes[RISCV_LINUX_FLOCK_WHENCE_OFFSET..RISCV_LINUX_FLOCK_WHENCE_OFFSET + 2]
        .copy_from_slice(&lock.whence.to_le_bytes());
    bytes[RISCV_LINUX_FLOCK_START_OFFSET..RISCV_LINUX_FLOCK_START_OFFSET + 8]
        .copy_from_slice(&lock.start.to_le_bytes());
    bytes[RISCV_LINUX_FLOCK_LEN_OFFSET..RISCV_LINUX_FLOCK_LEN_OFFSET + 8]
        .copy_from_slice(&lock.len.to_le_bytes());
    bytes[RISCV_LINUX_FLOCK_PID_OFFSET..RISCV_LINUX_FLOCK_PID_OFFSET + 4]
        .copy_from_slice(&lock.pid.to_le_bytes());
    bytes
}

fn read_guest_bytes(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
    bytes: usize,
) -> Result<Vec<u8>, u64> {
    let mut output = Vec::with_capacity(bytes);
    for offset in 0..bytes {
        let byte_address = address
            .checked_add(offset as u64)
            .ok_or(RISCV_LINUX_EFAULT)?;
        let byte = guest_memory
            .read(byte_address, 1)
            .ok_or(RISCV_LINUX_EFAULT)?;
        let [byte]: [u8; 1] = byte.try_into().map_err(|_| RISCV_LINUX_EFAULT)?;
        output.push(byte);
    }
    Ok(output)
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_i64(bytes: &[u8], offset: usize) -> i64 {
    i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn guest_fd_error_return() -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return {
        value: linux_error(RISCV_LINUX_EBADF),
    }
}
