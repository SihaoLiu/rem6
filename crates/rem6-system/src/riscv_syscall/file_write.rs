use super::{
    eventfd::{eventfd_write_bytes_written, eventfd_write_result},
    guest_fd_argument, linux_error,
    pipe::RiscvGuestPipeWrite,
    read_guest_c_string,
    signalfd::signalfd_write_result,
    socket::{socket_write_result, RiscvGuestSocketWrite},
    timerfd::timerfd_write_result,
    RiscvGuestCStringError, RiscvGuestFileIdentity, RiscvGuestMemoryReader, RiscvGuestNodeKind,
    RiscvGuestWriteRecord, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EAGAIN,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EFBIG, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EISDIR, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT, RISCV_LINUX_EPERM,
    RISCV_LINUX_ESPIPE, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_APPEND, RISCV_LINUX_O_RDONLY,
    RISCV_LINUX_PATH_MAX,
};
use crate::{GuestFd, GuestFdError, GuestFileOffset};
use rem6_kernel::Tick;

const RISCV_GUEST_FILE_DENSE_LIMIT_BYTES: u64 = 64 * 1024 * 1024;

pub(super) const RISCV_LINUX_TRUNCATE: u64 = 45;
pub(super) const RISCV_LINUX_FTRUNCATE: u64 = 46;
pub(super) const RISCV_LINUX_FALLOCATE: u64 = 47;
pub(super) const RISCV_LINUX_WRITE: u64 = 64;
pub(super) const RISCV_LINUX_PWRITE64: u64 = 68;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestFileWriteError {
    Fd(GuestFdError),
    FileTooLarge,
    Permission,
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
    Permission,
}

impl From<GuestFdError> for RiscvGuestFileResizeError {
    fn from(error: GuestFdError) -> Self {
        Self::Fd(error)
    }
}

impl RiscvSyscallState {
    pub(super) fn replace_guest_file_contents(&mut self, path: &[u8], contents: Vec<u8>) {
        let previous = self.guest_file_contents(path).map(Vec::from);
        let path = path.to_vec();
        self.guest_paths.insert(path.clone());
        let identity = self.ensure_guest_file_identity(&path);
        self.guest_file_modes
            .entry(identity)
            .or_insert(super::stat::RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS);
        let contents_changed = previous.as_deref() != Some(contents.as_slice());
        self.synchronize_guest_file_contents(identity, contents);
        if contents_changed {
            self.mark_guest_file_contents_dirty(identity);
        }
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
        if self.guest_fd_write_denied_by_file_seal(fd)? {
            return Err(RiscvGuestFileWriteError::Permission);
        }
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
            self.mark_guest_file_contents_dirty(stat.identity);
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
        if self.guest_fd_write_denied_by_file_seal(fd)? {
            return Err(RiscvGuestFileWriteError::Permission);
        }
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
            self.mark_guest_file_contents_dirty(stat.identity);
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
        if self.guest_fd_resize_denied_by_file_seal(fd, length)? {
            return Err(RiscvGuestFileResizeError::Permission);
        }
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
        let contents_changed = contents.len() != length;
        contents.resize(length, 0);
        let contents = contents.clone();
        if let Some(stat) = self.guest_file_stats.get(&description).copied() {
            self.synchronize_guest_file_contents(stat.identity, contents);
            if contents_changed {
                self.mark_guest_file_contents_dirty(stat.identity);
            }
        } else if let Some(path) = self.guest_file_description_paths.get(&description).cloned() {
            self.guest_files.insert(path, contents);
        }
        Ok(())
    }

    pub(super) fn ensure_guest_file_length_from_fd(
        &mut self,
        fd: GuestFd,
        length: u64,
    ) -> Result<(), RiscvGuestFileResizeError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        if self.guest_fd_resize_denied_by_file_seal(fd, length)? {
            return Err(RiscvGuestFileResizeError::Permission);
        }
        let Some(contents) = self.guest_file_descriptions.get_mut(&description) else {
            return Err(RiscvGuestFileResizeError::NotRegularWritableFile);
        };
        if length > RISCV_GUEST_FILE_DENSE_LIMIT_BYTES {
            return Err(RiscvGuestFileResizeError::FileTooLarge);
        }
        let length =
            usize::try_from(length).map_err(|_| RiscvGuestFileResizeError::FileTooLarge)?;
        if contents.len() < length {
            contents.resize(length, 0);
            let contents = contents.clone();
            if let Some(stat) = self.guest_file_stats.get(&description).copied() {
                self.synchronize_guest_file_contents(stat.identity, contents);
                self.mark_guest_file_contents_dirty(stat.identity);
            } else if let Some(path) = self.guest_file_description_paths.get(&description).cloned()
            {
                self.guest_files.insert(path, contents);
            }
        }
        Ok(())
    }

    pub(super) fn truncate_guest_file_path(
        &mut self,
        path: &[u8],
        length: u64,
    ) -> Result<(), RiscvGuestFileResizeError> {
        if (length as i64) < 0 {
            return Err(RiscvGuestFileResizeError::NotRegularWritableFile);
        }
        if length > RISCV_GUEST_FILE_DENSE_LIMIT_BYTES {
            return Err(RiscvGuestFileResizeError::FileTooLarge);
        }
        let length =
            usize::try_from(length).map_err(|_| RiscvGuestFileResizeError::FileTooLarge)?;
        let identity = self.guest_file_identity(path);
        let mut contents = self
            .guest_file_contents(path)
            .map(Vec::from)
            .unwrap_or_default();
        let contents_changed = contents.len() != length;
        contents.resize(length, 0);
        self.synchronize_guest_file_contents(identity, contents);
        if contents_changed {
            self.mark_guest_file_contents_dirty(identity);
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

pub(super) fn syscall_truncate(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_guest_c_string(guest_memory, request.argument(0), RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    if (request.argument(1) as i64) < 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let path = match state.resolve_guest_path_following_symlinks(&path) {
        Ok(path) => path,
        Err(error) => return linux_error(error.linux_error_code()),
    };
    let path = state.existing_guest_path_key(&path).unwrap_or(path);
    if state.guest_directory_entries(&path).is_some() {
        return linux_error(RISCV_LINUX_EISDIR);
    }
    if !state.guest_path_registered(&path) {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    match state.truncate_guest_file_path(&path, request.argument(1)) {
        Ok(()) => 0,
        Err(RiscvGuestFileResizeError::FileTooLarge) => linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileResizeError::NotRegularWritableFile) => linux_error(RISCV_LINUX_EINVAL),
        Err(RiscvGuestFileResizeError::Permission) => linux_error(RISCV_LINUX_EPERM),
        Err(RiscvGuestFileResizeError::Fd(_)) => linux_error(RISCV_LINUX_EBADF),
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
        Err(RiscvGuestFileResizeError::Permission) => linux_error(RISCV_LINUX_EPERM),
        Err(RiscvGuestFileResizeError::Fd(_)) => linux_error(RISCV_LINUX_EBADF),
    }
}

pub(super) fn syscall_fallocate(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let mode = request.argument(1);
    let offset = request.argument(2);
    let length = request.argument(3);
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if mode != 0 || (offset as i64) < 0 || (length as i64) <= 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(end) = offset.checked_add(length) else {
        return linux_error(RISCV_LINUX_EFBIG);
    };
    match state.ensure_guest_file_length_from_fd(fd, end) {
        Ok(()) => 0,
        Err(RiscvGuestFileResizeError::FileTooLarge) => linux_error(RISCV_LINUX_EFBIG),
        Err(RiscvGuestFileResizeError::NotRegularWritableFile) => linux_error(RISCV_LINUX_EINVAL),
        Err(RiscvGuestFileResizeError::Permission) => linux_error(RISCV_LINUX_EPERM),
        Err(RiscvGuestFileResizeError::Fd(_)) => linux_error(RISCV_LINUX_EBADF),
    }
}

pub(super) fn syscall_write(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryReader,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_RDONLY as u32 {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }

    let count = request.argument(2);
    match state.guest_eventfd_ready(fd) {
        Ok(Some(_ready)) => {
            if count < eventfd_write_bytes_written() {
                return Some(linux_error(RISCV_LINUX_EINVAL));
            }
            let address = request.argument(1);
            let Some(bytes) = guest_memory.read(address, eventfd_write_bytes_written() as usize)
            else {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            };
            if bytes.len() != eventfd_write_bytes_written() as usize {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            return match state.write_guest_eventfd_from_fd(fd, &bytes) {
                Ok(Some(write)) => eventfd_write_result(write),
                Ok(None) | Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
            };
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    match state.guest_timerfd_ready(fd) {
        Ok(Some(_ready)) => return Some(timerfd_write_result()),
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    match state.guest_signalfd_ready(fd) {
        Ok(Some(_ready)) => return Some(signalfd_write_result()),
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    if count == 0 {
        return Some(0);
    }

    let Ok(byte_count) = usize::try_from(count) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    match state.guest_file_write_exceeds_dense_limit(fd, count) {
        Ok(true) => return Some(linux_error(RISCV_LINUX_EFBIG)),
        Ok(false) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    let socket_write = match state.guest_socket_write_plan(fd, byte_count) {
        Ok(RiscvGuestSocketWrite::NotSocket) => None,
        Ok(RiscvGuestSocketWrite::Written(written)) => Some(written),
        Ok(write @ RiscvGuestSocketWrite::WouldBlock)
        | Ok(write @ RiscvGuestSocketWrite::Blocked)
        | Ok(write @ RiscvGuestSocketWrite::BrokenPipe) => return socket_write_result(write),
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    };
    if let Some(written) = socket_write {
        let address = request.argument(1);
        let Some(bytes) = guest_memory.read(address, written) else {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        };
        if bytes.len() != written {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
        return match state.write_guest_socket_from_fd(fd, &bytes) {
            Ok(write) => socket_write_result(write),
            Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
        };
    }
    let pipe_write = match state.guest_pipe_write_plan(fd, byte_count) {
        Ok(RiscvGuestPipeWrite::NotPipe) => None,
        Ok(RiscvGuestPipeWrite::Written(written)) => Some(written),
        Ok(RiscvGuestPipeWrite::WouldBlock) => return Some(linux_error(RISCV_LINUX_EAGAIN)),
        Ok(RiscvGuestPipeWrite::Blocked) => return None,
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    };
    let address = request.argument(1);
    let read_count = pipe_write.unwrap_or(byte_count);
    let Some(bytes) = guest_memory.read(address, read_count) else {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    };
    if bytes.len() != read_count {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    if pipe_write.is_some() {
        return match state.write_guest_pipe_from_fd(fd, &bytes) {
            Ok(RiscvGuestPipeWrite::Written(written)) => Some(written as u64),
            Ok(RiscvGuestPipeWrite::WouldBlock) => Some(linux_error(RISCV_LINUX_EAGAIN)),
            Ok(RiscvGuestPipeWrite::Blocked) => None,
            Ok(RiscvGuestPipeWrite::NotPipe) | Err(_) => Some(linux_error(RISCV_LINUX_EBADF)),
        };
    }
    match state.write_guest_file_from_fd(fd, &bytes) {
        Ok(_) => {}
        Err(RiscvGuestFileWriteError::FileTooLarge) => {
            return Some(linux_error(RISCV_LINUX_EFBIG));
        }
        Err(RiscvGuestFileWriteError::Permission) => {
            return Some(linux_error(RISCV_LINUX_EPERM));
        }
        Err(RiscvGuestFileWriteError::Fd(_)) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    if state.guest_fds.advance_file_offset(fd, count).is_err() {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }

    state.push_guest_write(RiscvGuestWriteRecord::new(fd, address, tick, bytes));
    Some(count)
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
        Err(RiscvGuestFileWriteError::Permission) => return linux_error(RISCV_LINUX_EPERM),
        Err(RiscvGuestFileWriteError::Fd(_)) => return linux_error(RISCV_LINUX_EBADF),
    }

    state.push_guest_write(RiscvGuestWriteRecord::new(fd, address, tick, bytes));
    count
}
