use super::{
    guest_fd_argument, linux_error, stat::guest_path_inode, RiscvGuestFileIdentity,
    RiscvGuestMemoryReader, RiscvGuestNodeKind, RiscvGuestWriteRecord, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EFBIG,
    RISCV_LINUX_EINVAL, RISCV_LINUX_ESPIPE, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_APPEND,
    RISCV_LINUX_O_RDONLY,
};
use crate::{GuestFd, GuestFdError, GuestFileOffset};
use rem6_kernel::Tick;

const RISCV_GUEST_FILE_DENSE_LIMIT_BYTES: u64 = 64 * 1024 * 1024;

pub(super) const RISCV_LINUX_FTRUNCATE: u64 = 46;
pub(super) const RISCV_LINUX_WRITE: u64 = 64;
pub(super) const RISCV_LINUX_PWRITE64: u64 = 68;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestFileWriteError {
    Fd(GuestFdError),
    FileTooLarge,
}

impl From<GuestFdError> for RiscvGuestFileWriteError {
    fn from(error: GuestFdError) -> Self {
        Self::Fd(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestFileResizeError {
    Fd(GuestFdError),
    FileTooLarge,
    NotRegularWritableFile,
}

impl From<GuestFdError> for RiscvGuestFileResizeError {
    fn from(error: GuestFdError) -> Self {
        Self::Fd(error)
    }
}

impl RiscvSyscallState {
    pub(super) fn replace_guest_file_contents(&mut self, path: &[u8], contents: Vec<u8>) {
        let path = path.to_vec();
        self.guest_paths.insert(path.clone());
        self.guest_file_identities
            .entry(path.clone())
            .or_insert_with(|| RiscvGuestFileIdentity {
                inode: guest_path_inode(&path),
            });
        let identity = self.guest_file_identity(&path);
        self.guest_file_modes
            .entry(identity)
            .or_insert(super::stat::RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS);
        self.synchronize_guest_file_contents(identity, contents);
    }

    fn synchronize_guest_file_contents(
        &mut self,
        identity: RiscvGuestFileIdentity,
        contents: Vec<u8>,
    ) {
        let paths = self
            .guest_paths
            .iter()
            .filter(|path| self.guest_file_identity(path) == identity)
            .cloned()
            .collect::<Vec<_>>();
        for path in paths {
            self.guest_files.insert(path, contents.clone());
        }

        let description_ids = self
            .guest_file_stats
            .iter()
            .filter_map(|(description, stat)| {
                (stat.identity == identity && stat.kind == RiscvGuestNodeKind::RegularFile)
                    .then_some(*description)
            })
            .collect::<Vec<_>>();
        for description in description_ids {
            if let Some(file_contents) = self.guest_file_descriptions.get_mut(&description) {
                *file_contents = contents.clone();
            }
            if let Some(stat) = self.guest_file_stats.get_mut(&description) {
                stat.size = contents.len() as u64;
            }
        }
    }

    pub(super) fn write_guest_file_from_fd(
        &mut self,
        fd: GuestFd,
        bytes: &[u8],
    ) -> Result<bool, RiscvGuestFileWriteError> {
        let byte_count =
            u64::try_from(bytes.len()).map_err(|_| RiscvGuestFileWriteError::FileTooLarge)?;
        if self.guest_file_write_exceeds_dense_limit(fd, byte_count)? {
            return Err(RiscvGuestFileWriteError::FileTooLarge);
        }
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let append = self.guest_fd_appends_to_file(fd)?;
        let offset = if append {
            let Some(contents) = self.guest_file_descriptions.get(&description) else {
                return Ok(false);
            };
            GuestFileOffset::new(contents.len() as u64)
        } else {
            self.guest_fds.file_offset(fd)?
        };
        if append {
            self.guest_fds.set_file_offset(fd, offset)?;
        }
        let Some(contents) = self.guest_file_descriptions.get_mut(&description) else {
            return Ok(false);
        };
        let start = usize::try_from(offset.get()).map_err(|_| GuestFdError::BadFd { fd })?;
        let end = start
            .checked_add(bytes.len())
            .ok_or(GuestFdError::BadFd { fd })?;
        if end > contents.len() {
            contents.resize(end, 0);
        }
        contents[start..end].copy_from_slice(bytes);
        let contents = contents.clone();
        if let Some(stat) = self.guest_file_stats.get(&description).copied() {
            self.synchronize_guest_file_contents(stat.identity, contents);
        } else if let Some(path) = self.guest_file_description_paths.get(&description).cloned() {
            self.guest_files.insert(path, contents);
        }
        Ok(true)
    }

    pub(super) fn write_guest_file_from_fd_at(
        &mut self,
        fd: GuestFd,
        offset: u64,
        bytes: &[u8],
    ) -> Result<bool, RiscvGuestFileWriteError> {
        let byte_count =
            u64::try_from(bytes.len()).map_err(|_| RiscvGuestFileWriteError::FileTooLarge)?;
        if self.guest_file_write_at_exceeds_dense_limit(fd, offset, byte_count)? {
            return Err(RiscvGuestFileWriteError::FileTooLarge);
        }
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(contents) = self.guest_file_descriptions.get_mut(&description) else {
            return Ok(false);
        };
        let start = usize::try_from(offset).map_err(|_| GuestFdError::BadFd { fd })?;
        let end = start
            .checked_add(bytes.len())
            .ok_or(GuestFdError::BadFd { fd })?;
        if end > contents.len() {
            contents.resize(end, 0);
        }
        contents[start..end].copy_from_slice(bytes);
        let contents = contents.clone();
        if let Some(stat) = self.guest_file_stats.get(&description).copied() {
            self.synchronize_guest_file_contents(stat.identity, contents);
        } else if let Some(path) = self.guest_file_description_paths.get(&description).cloned() {
            self.guest_files.insert(path, contents);
        }
        Ok(true)
    }

    pub(super) fn guest_file_append_offset(
        &self,
        fd: GuestFd,
    ) -> Result<Option<u64>, GuestFdError> {
        if !self.guest_fd_appends_to_file(fd)? {
            return Ok(None);
        }
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self
            .guest_file_descriptions
            .get(&description)
            .map(|contents| contents.len() as u64))
    }

    pub(super) fn truncate_guest_file_from_fd(
        &mut self,
        fd: GuestFd,
        length: u64,
    ) -> Result<(), RiscvGuestFileResizeError> {
        let status_flags = self.guest_fds.status_flags(fd)?;
        if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
            return Err(RiscvGuestFileResizeError::NotRegularWritableFile);
        }

        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        let Some(contents) = self.guest_file_descriptions.get_mut(&description) else {
            return Err(RiscvGuestFileResizeError::NotRegularWritableFile);
        };
        if (length as i64) < 0 {
            return Err(RiscvGuestFileResizeError::NotRegularWritableFile);
        }
        if length > RISCV_GUEST_FILE_DENSE_LIMIT_BYTES {
            return Err(RiscvGuestFileResizeError::FileTooLarge);
        }
        let length =
            usize::try_from(length).map_err(|_| RiscvGuestFileResizeError::FileTooLarge)?;
        contents.resize(length, 0);
        let contents = contents.clone();
        if let Some(stat) = self.guest_file_stats.get(&description).copied() {
            self.synchronize_guest_file_contents(stat.identity, contents);
        } else if let Some(path) = self.guest_file_description_paths.get(&description).cloned() {
            self.guest_files.insert(path, contents);
        }
        Ok(())
    }

    pub(super) fn guest_file_write_exceeds_dense_limit(
        &self,
        fd: GuestFd,
        byte_count: u64,
    ) -> Result<bool, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        if !self.guest_file_descriptions.contains_key(&description) {
            return Ok(false);
        }
        let offset = if self.guest_fd_appends_to_file(fd)? {
            let Some(contents) = self.guest_file_descriptions.get(&description) else {
                return Ok(false);
            };
            contents.len() as u64
        } else {
            self.guest_fds.file_offset(fd)?.get()
        };
        self.guest_file_write_at_exceeds_dense_limit(fd, offset, byte_count)
    }

    pub(super) fn guest_file_write_at_exceeds_dense_limit(
        &self,
        fd: GuestFd,
        offset: u64,
        byte_count: u64,
    ) -> Result<bool, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        if !self.guest_file_descriptions.contains_key(&description) {
            return Ok(false);
        }
        let Some(end) = offset.checked_add(byte_count) else {
            return Ok(true);
        };
        Ok(end > RISCV_GUEST_FILE_DENSE_LIMIT_BYTES)
    }

    fn guest_fd_appends_to_file(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        Ok(self.guest_fds.status_flags(fd)?.bits() & RISCV_LINUX_O_APPEND as u32 != 0)
    }
}

pub(super) fn syscall_ftruncate(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.truncate_guest_file_from_fd(fd, request.argument(1)) {
        Ok(()) => 0,
        Err(RiscvGuestFileResizeError::FileTooLarge) => linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileResizeError::NotRegularWritableFile) => linux_error(RISCV_LINUX_EINVAL),
        Err(RiscvGuestFileResizeError::Fd(_)) => linux_error(RISCV_LINUX_EBADF),
    }
}

pub(super) fn syscall_write(
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
    match state.guest_file_write_exceeds_dense_limit(fd, count) {
        Ok(true) => return linux_error(RISCV_LINUX_EFBIG),
        Ok(false) => {}
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }
    let address = request.argument(1);
    let Some(bytes) = guest_memory.read(address, byte_count) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    if bytes.len() != byte_count {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    match state.write_guest_file_from_fd(fd, &bytes) {
        Ok(_) => {}
        Err(RiscvGuestFileWriteError::FileTooLarge) => return linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileWriteError::Fd(_)) => return linux_error(RISCV_LINUX_EBADF),
    }
    if state.guest_fds.advance_file_offset(fd, count).is_err() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    state.push_guest_write(RiscvGuestWriteRecord::new(fd, address, tick, bytes));
    count
}

pub(super) fn syscall_pwrite64(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let offset = request.argument(3);
    if (offset as i64) < 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }
    match state.guest_file_fd_is_seekable(fd) {
        Ok(true) => {}
        Ok(false) => return linux_error(RISCV_LINUX_ESPIPE),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }

    let count = request.argument(2);
    if count == 0 {
        return 0;
    }
    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let offset = match state.guest_file_append_offset(fd) {
        Ok(Some(append_offset)) => append_offset,
        Ok(None) => offset,
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    match state.guest_file_write_at_exceeds_dense_limit(fd, offset, count) {
        Ok(true) => return linux_error(RISCV_LINUX_EFBIG),
        Ok(false) => {}
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }
    let address = request.argument(1);
    let Some(bytes) = guest_memory.read(address, byte_count) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    if bytes.len() != byte_count {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    match state.write_guest_file_from_fd_at(fd, offset, &bytes) {
        Ok(true) => {}
        Ok(false) => return linux_error(RISCV_LINUX_ESPIPE),
        Err(RiscvGuestFileWriteError::FileTooLarge) => return linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileWriteError::Fd(_)) => return linux_error(RISCV_LINUX_EBADF),
    }

    state.push_guest_write(RiscvGuestWriteRecord::new(fd, address, tick, bytes));
    count
}
