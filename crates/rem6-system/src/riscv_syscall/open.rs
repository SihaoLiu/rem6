use crate::{GuestFdError, GuestFileStatusFlags};

use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvGuestNodeKind, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_EBADF, RISCV_LINUX_EEXIST, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EISDIR, RISCV_LINUX_EMFILE, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT,
    RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDONLY,
    RISCV_LINUX_O_WRONLY, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_OPEN: u64 = 1024;
const RISCV_LINUX_O_DIRECTORY: u64 = 0o200000;
const RISCV_LINUX_O_CREAT: u64 = 0o100;
const RISCV_LINUX_O_TRUNC: u64 = 0o1000;
const RISCV_LINUX_O_RDWR: u64 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvGuestOpenRecord {
    fd: crate::GuestFd,
    dirfd: u64,
    path: Vec<u8>,
    flags: u64,
    mode: u64,
}

impl RiscvGuestOpenRecord {
    pub fn new(fd: crate::GuestFd, dirfd: u64, path: Vec<u8>, flags: u64, mode: u64) -> Self {
        Self {
            fd,
            dirfd,
            path,
            flags,
            mode,
        }
    }

    pub const fn fd(&self) -> crate::GuestFd {
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

pub(super) struct RiscvGuestOpenRequest {
    pub(super) dirfd: u64,
    pub(super) path: Vec<u8>,
    pub(super) flags: u64,
    pub(super) mode: u64,
    pub(super) status_flags: GuestFileStatusFlags,
    pub(super) close_on_exec: bool,
    pub(super) node_kind: RiscvGuestNodeKind,
    pub(super) file_contents: Option<Vec<u8>>,
    pub(super) directory_contents: Option<Vec<u8>>,
}

pub(super) fn syscall_openat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    syscall_open_registered_path(
        request.argument(0),
        request.argument(1),
        request.argument(2),
        request.argument(3),
        state,
        guest_memory,
    )
}

pub(super) fn syscall_open(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    syscall_open_registered_path(
        RISCV_LINUX_AT_FDCWD,
        request.argument(0),
        request.argument(1),
        request.argument(2),
        state,
        guest_memory,
    )
}

fn syscall_open_registered_path(
    dirfd: u64,
    path_address: u64,
    flags: u64,
    mode: u64,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if dirfd != RISCV_LINUX_AT_FDCWD {
        return linux_error(RISCV_LINUX_EBADF);
    }

    if flags
        & !(RISCV_LINUX_O_ACCMODE
            | RISCV_LINUX_O_CLOEXEC
            | RISCV_LINUX_O_NONBLOCK
            | RISCV_LINUX_O_DIRECTORY
            | RISCV_LINUX_O_CREAT
            | RISCV_LINUX_O_TRUNC)
        != 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let access_mode = flags & RISCV_LINUX_O_ACCMODE;
    if !matches!(
        access_mode,
        RISCV_LINUX_O_RDONLY | RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_RDWR
    ) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let writable = access_mode != RISCV_LINUX_O_RDONLY;
    let creates_file = flags & RISCV_LINUX_O_CREAT != 0;
    let truncates_file = flags & RISCV_LINUX_O_TRUNC != 0;

    let path = match read_guest_c_string(guest_memory, path_address, RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    let open_directory = flags & RISCV_LINUX_O_DIRECTORY != 0;
    let (path, node_kind, file_contents, directory_contents) = if open_directory {
        if writable || creates_file || truncates_file {
            return linux_error(RISCV_LINUX_EINVAL);
        }
        let path = match state.guest_directory_path(&path) {
            Ok(path) => path,
            Err(error) => return linux_error(error.linux_error_code()),
        };
        let entries = state
            .guest_directory_entries(&path)
            .expect("resolved guest directory has entries");
        (
            path,
            RiscvGuestNodeKind::Directory,
            None,
            Some(super::riscv_linux_dirent64_bytes(&entries)),
        )
    } else if writable || creates_file || truncates_file {
        let path = match state.resolve_guest_path(&path) {
            Ok(path) => path,
            Err(error) => return linux_error(error.linux_error_code()),
        };
        if state.guest_directory_entries(&path).is_some() {
            return linux_error(RISCV_LINUX_EISDIR);
        }
        if state.guest_link_target(&path).is_some() {
            return linux_error(RISCV_LINUX_EEXIST);
        }
        let path = state.existing_guest_path_key(&path).unwrap_or(path);
        let existing = state.guest_file_contents(&path).map(Vec::from);
        if existing.is_none() && !state.guest_path_registered(&path) && !creates_file {
            return linux_error(RISCV_LINUX_ENOENT);
        }
        let contents = if truncates_file {
            Vec::new()
        } else {
            existing.unwrap_or_default()
        };
        state.replace_guest_file_contents(&path, contents.clone());
        (path, RiscvGuestNodeKind::RegularFile, Some(contents), None)
    } else {
        let path = match state.resolve_existing_guest_regular_path(&path) {
            Ok(Some(path)) => path,
            Ok(None) => return linux_error(RISCV_LINUX_ENOENT),
            Err(error) => return linux_error(error.linux_error_code()),
        };
        (
            path.clone(),
            RiscvGuestNodeKind::RegularFile,
            state.guest_file_contents(&path).map(Vec::from),
            None,
        )
    };
    let status_flags = GuestFileStatusFlags::new((flags & !RISCV_LINUX_O_CLOEXEC) as u32);
    let close_on_exec = flags & RISCV_LINUX_O_CLOEXEC != 0;
    match state.open_guest_path(RiscvGuestOpenRequest {
        dirfd,
        path,
        flags,
        mode,
        status_flags,
        close_on_exec,
        node_kind,
        file_contents,
        directory_contents,
    }) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}
