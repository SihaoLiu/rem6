use crate::{GuestFdError, GuestFileStatusFlags};

use super::permissions::apply_file_creation_mask;
use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvGuestNodeKind, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_EBADF, RISCV_LINUX_EEXIST, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EISDIR, RISCV_LINUX_EMFILE, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT,
    RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_APPEND, RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_NONBLOCK,
    RISCV_LINUX_O_RDONLY, RISCV_LINUX_O_WRONLY, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_OPEN: u64 = 1024;
const RISCV_LINUX_ELOOP: u64 = 40;
const RISCV_LINUX_O_NOCTTY: u64 = 0o400;
const RISCV_LINUX_O_DSYNC: u64 = 0o10000;
const RISCV_LINUX_O_DIRECTORY: u64 = 0o200000;
const RISCV_LINUX_O_NOFOLLOW: u64 = 0o400000;
const RISCV_LINUX_O_SYNC: u64 = 0o4010000;
const RISCV_LINUX_O_SYNC_INTERNAL: u64 = RISCV_LINUX_O_SYNC & !RISCV_LINUX_O_DSYNC;
const RISCV_LINUX_O_CREAT: u64 = 0o100;
const RISCV_LINUX_O_EXCL: u64 = 0o200;
const RISCV_LINUX_O_TRUNC: u64 = 0o1000;
const RISCV_LINUX_O_RDWR: u64 = 2;
const RISCV_NEWLIB_O_APPEND: u64 = 0x0008;
const RISCV_NEWLIB_O_CREAT: u64 = 0x0200;
const RISCV_NEWLIB_O_TRUNC: u64 = 0x0400;
const RISCV_NEWLIB_O_EXCL: u64 = 0x0800;
const RISCV_NEWLIB_O_SYNC: u64 = 0x2000;
const RISCV_NEWLIB_O_NOCTTY: u64 = 0x8000;
const RISCV_NEWLIB_O_NONBLOCK: u64 = 0x4000;
const RISCV_NEWLIB_O_CLOEXEC: u64 = 0x40000;
const RISCV_NEWLIB_O_NOFOLLOW: u64 = 0x100000;
const RISCV_NEWLIB_O_DIRECTORY: u64 = 0x200000;

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
    let flags = request.argument(1);
    if legacy_open_unknown_flags(flags) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    syscall_open_registered_path(
        RISCV_LINUX_AT_FDCWD,
        request.argument(0),
        normalize_newlib_legacy_open_flags(flags),
        request.argument(2),
        state,
        guest_memory,
    )
}

fn normalize_newlib_legacy_open_flags(flags: u64) -> u64 {
    // RISC-V newlib/libgloss emits syscall 1024 with newlib fcntl flag values.
    // Linux ABI callers keep Linux flag values on the openat path.
    let mut normalized = flags & RISCV_LINUX_O_ACCMODE;
    if flags & RISCV_NEWLIB_O_APPEND != 0 {
        normalized |= RISCV_LINUX_O_APPEND;
    }
    if flags & RISCV_NEWLIB_O_CREAT != 0 {
        normalized |= RISCV_LINUX_O_CREAT;
    }
    if flags & RISCV_NEWLIB_O_TRUNC != 0 {
        normalized |= RISCV_LINUX_O_TRUNC;
    }
    if flags & RISCV_NEWLIB_O_EXCL != 0 {
        normalized |= RISCV_LINUX_O_EXCL;
    }
    if flags & RISCV_NEWLIB_O_SYNC != 0 {
        normalized |= RISCV_LINUX_O_DSYNC;
    }
    if flags & RISCV_NEWLIB_O_NONBLOCK != 0 {
        normalized |= RISCV_LINUX_O_NONBLOCK;
    }
    if flags & RISCV_NEWLIB_O_NOCTTY != 0 {
        normalized |= RISCV_LINUX_O_NOCTTY;
    }
    if flags & RISCV_NEWLIB_O_CLOEXEC != 0 {
        normalized |= RISCV_LINUX_O_CLOEXEC;
    }
    if flags & RISCV_NEWLIB_O_NOFOLLOW != 0 {
        normalized |= RISCV_LINUX_O_NOFOLLOW;
    }
    if flags & RISCV_NEWLIB_O_DIRECTORY != 0 {
        normalized |= RISCV_LINUX_O_DIRECTORY;
    }
    normalized
}

fn legacy_open_unknown_flags(flags: u64) -> u64 {
    flags
        & !(RISCV_LINUX_O_ACCMODE
            | RISCV_NEWLIB_O_APPEND
            | RISCV_NEWLIB_O_CREAT
            | RISCV_NEWLIB_O_TRUNC
            | RISCV_NEWLIB_O_EXCL
            | RISCV_NEWLIB_O_SYNC
            | RISCV_NEWLIB_O_NONBLOCK
            | RISCV_NEWLIB_O_NOCTTY
            | RISCV_NEWLIB_O_CLOEXEC
            | RISCV_NEWLIB_O_NOFOLLOW
            | RISCV_NEWLIB_O_DIRECTORY)
}

fn normalize_linux_open_flags(flags: u64) -> u64 {
    let mut normalized = flags;
    if flags & RISCV_LINUX_O_SYNC_INTERNAL != 0 {
        normalized |= RISCV_LINUX_O_DSYNC;
    }
    normalized
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
            | RISCV_LINUX_O_APPEND
            | RISCV_LINUX_O_NONBLOCK
            | RISCV_LINUX_O_NOCTTY
            | RISCV_LINUX_O_DSYNC
            | RISCV_LINUX_O_SYNC
            | RISCV_LINUX_O_DIRECTORY
            | RISCV_LINUX_O_NOFOLLOW
            | RISCV_LINUX_O_CREAT
            | RISCV_LINUX_O_EXCL
            | RISCV_LINUX_O_TRUNC)
        != 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let flags = normalize_linux_open_flags(flags);
    let access_mode = flags & RISCV_LINUX_O_ACCMODE;
    if !matches!(
        access_mode,
        RISCV_LINUX_O_RDONLY | RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_RDWR
    ) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let writable = access_mode != RISCV_LINUX_O_RDONLY;
    let creates_file = flags & RISCV_LINUX_O_CREAT != 0;
    let exclusive_create = flags & RISCV_LINUX_O_EXCL != 0;
    let truncates_file = flags & RISCV_LINUX_O_TRUNC != 0;
    if exclusive_create && !creates_file {
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
    if flags & RISCV_LINUX_O_NOFOLLOW != 0 && state.guest_link_target(&path).is_some() {
        return linux_error(RISCV_LINUX_ELOOP);
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
        let path_exists = state.guest_path_registered(&path);
        let existing = state.guest_file_contents(&path).map(Vec::from);
        if exclusive_create && path_exists {
            return linux_error(RISCV_LINUX_EEXIST);
        }
        if existing.is_none() && !path_exists && !creates_file {
            return linux_error(RISCV_LINUX_ENOENT);
        }
        let contents = if truncates_file {
            Vec::new()
        } else {
            existing.unwrap_or_default()
        };
        let created_new_file = creates_file && !path_exists;
        state.replace_guest_file_contents(&path, contents.clone());
        if created_new_file {
            state.set_guest_file_permissions(&path, apply_file_creation_mask(mode, state));
        }
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
    let status_flags = GuestFileStatusFlags::new(
        (flags & !(RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NOCTTY | RISCV_LINUX_O_NOFOLLOW)) as u32,
    );
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
