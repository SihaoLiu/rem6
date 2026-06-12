use crate::{GuestFdError, GuestFileStatusFlags};

use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvGuestNodeKind, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EMFILE,
    RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_CLOEXEC,
    RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDONLY, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_OPEN: u64 = 1024;
const RISCV_LINUX_O_DIRECTORY: u64 = 0o200000;

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
            | RISCV_LINUX_O_DIRECTORY)
        != 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if flags & RISCV_LINUX_O_ACCMODE != RISCV_LINUX_O_RDONLY {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let path = match read_guest_c_string(guest_memory, path_address, RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    let open_directory = flags & RISCV_LINUX_O_DIRECTORY != 0;
    let (node_kind, file_contents, directory_contents) = if open_directory {
        let Some(entries) = state.guest_directory_entries(&path) else {
            return linux_error(RISCV_LINUX_ENOENT);
        };
        (
            RiscvGuestNodeKind::Directory,
            None,
            Some(super::riscv_linux_dirent64_bytes(&entries)),
        )
    } else {
        if !state.guest_path_registered(&path) {
            return linux_error(RISCV_LINUX_ENOENT);
        }
        (
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
