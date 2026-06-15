use crate::{GuestFd, GuestFileDescriptionId};

use super::stat::RISCV_LINUX_MODE_BITS;
use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvGuestNodeKind, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_AT_EMPTY_PATH, RISCV_LINUX_AT_FDCWD, RISCV_LINUX_AT_SYMLINK_NOFOLLOW,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_ENOENT, RISCV_LINUX_ENOTDIR, RISCV_LINUX_EPERM, RISCV_LINUX_PATH_MAX,
};

const RISCV_LINUX_MODE_PERMISSION_BITS: u32 = 0o777;
pub(super) const RISCV_LINUX_FCHMOD: u64 = 52;
pub(super) const RISCV_LINUX_FCHMODAT: u64 = 53;
pub(super) const RISCV_LINUX_FCHOWNAT: u64 = 54;
pub(super) const RISCV_LINUX_FCHOWN: u64 = 55;
pub(super) const RISCV_LINUX_UMASK: u64 = 166;
pub(super) const RISCV_NEWLIB_LEGACY_CHMOD: u64 = 1028;
const RISCV_LINUX_CHOWNAT_VALID_FLAGS: u64 =
    RISCV_LINUX_AT_EMPTY_PATH | RISCV_LINUX_AT_SYMLINK_NOFOLLOW;
const RISCV_LINUX_CHOWN_NO_CHANGE: u64 = u64::MAX;
const RISCV_LINUX_CHOWN_TRUNCATED_NO_CHANGE: u64 = u32::MAX as u64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestChmodError {
    Missing,
    BadFd,
}

impl RiscvGuestChmodError {
    const fn linux_error_code(self) -> u64 {
        match self {
            Self::Missing => RISCV_LINUX_ENOENT,
            Self::BadFd => RISCV_LINUX_EBADF,
        }
    }
}

pub(super) fn syscall_chmod(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_chmod_path(request.argument(0), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let path = match resolve_chmod_path(RISCV_LINUX_AT_FDCWD, &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    chmod_path(&path, request.argument(1), state)
}

pub(super) fn syscall_fchmod(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.chmod_guest_fd(fd, chmod_permissions(request.argument(1))) {
        Ok(()) => 0,
        Err(error) => linux_error(error.linux_error_code()),
    }
}

pub(super) fn syscall_fchmodat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_chmod_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let path = match resolve_chmod_path(request.argument(0), &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    chmod_path(&path, request.argument(2), state)
}

pub(super) fn syscall_fchownat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let flags = request.argument(4);
    if flags & !RISCV_LINUX_CHOWNAT_VALID_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let path = match read_chmod_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if path.is_empty() {
        if flags & RISCV_LINUX_AT_EMPTY_PATH == 0 {
            return linux_error(RISCV_LINUX_ENOENT);
        }
        if request.argument(0) == RISCV_LINUX_AT_FDCWD {
            return chown_existing_node(request.argument(2), request.argument(3));
        }
        let Some(fd) = guest_fd_argument(request.argument(0)) else {
            return linux_error(RISCV_LINUX_EBADF);
        };
        return chown_fd(fd, request.argument(2), request.argument(3), state);
    }

    let path = match resolve_chmod_path(request.argument(0), &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    chown_path(
        &path,
        flags & RISCV_LINUX_AT_SYMLINK_NOFOLLOW != 0,
        request.argument(2),
        request.argument(3),
        state,
    )
}

pub(super) fn syscall_fchown(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    chown_fd(fd, request.argument(1), request.argument(2), state)
}

pub(super) fn syscall_umask(mask: u64, state: &mut RiscvSyscallState) -> u64 {
    let next_mask = (mask as u32) & RISCV_LINUX_MODE_PERMISSION_BITS;
    u64::from(state.replace_file_creation_mask(next_mask))
}

pub(super) fn apply_file_creation_mask(mode: u64, state: &RiscvSyscallState) -> u32 {
    (mode as u32) & RISCV_LINUX_MODE_PERMISSION_BITS & !state.file_creation_mask()
}

fn read_chmod_path(
    path_address: u64,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<Vec<u8>, u64> {
    read_guest_c_string(guest_memory, path_address, RISCV_LINUX_PATH_MAX).map_err(|error| {
        linux_error(match error {
            RiscvGuestCStringError::Fault => RISCV_LINUX_EFAULT,
            RiscvGuestCStringError::TooLong => RISCV_LINUX_ENAMETOOLONG,
        })
    })
}

fn resolve_chmod_path(dirfd: u64, path: &[u8], state: &RiscvSyscallState) -> Result<Vec<u8>, u64> {
    if path.is_empty() {
        return Err(linux_error(RISCV_LINUX_ENOENT));
    }
    if dirfd == RISCV_LINUX_AT_FDCWD || path.starts_with(b"/") {
        return state
            .resolve_guest_path(path)
            .map_err(|error| linux_error(error.linux_error_code()));
    }
    let Some(fd) = guest_fd_argument(dirfd) else {
        return Err(linux_error(RISCV_LINUX_EBADF));
    };
    let directory = match state.guest_directory_path_for_fd(fd) {
        Ok(Some(path)) => path,
        Ok(None) => return Err(linux_error(RISCV_LINUX_ENOTDIR)),
        Err(_error) => return Err(linux_error(RISCV_LINUX_EBADF)),
    };
    state
        .resolve_guest_path_from_directory(&directory, path)
        .map_err(|error| linux_error(error.linux_error_code()))
}

fn chmod_path(path: &[u8], mode: u64, state: &mut RiscvSyscallState) -> u64 {
    match state.chmod_guest_path(path, chmod_permissions(mode)) {
        Ok(()) => 0,
        Err(error) => linux_error(error.linux_error_code()),
    }
}

fn chown_path(
    path: &[u8],
    nofollow: bool,
    owner: u64,
    group: u64,
    state: &RiscvSyscallState,
) -> u64 {
    let existing_path = state.existing_guest_path_key(path);
    if let Some(link_path) = existing_path
        .as_deref()
        .filter(|path| state.guest_links.contains_key(*path))
    {
        if nofollow {
            return chown_existing_node(owner, group);
        }
        let Some(target) = state.guest_links.get(link_path) else {
            return linux_error(RISCV_LINUX_ENOENT);
        };
        let target_path = match symlink_target_path(link_path, target, state) {
            Some(path) => path,
            None => return linux_error(RISCV_LINUX_ENOENT),
        };
        return chown_existing_path(&target_path, owner, group, state);
    }

    chown_existing_path(
        existing_path.as_deref().unwrap_or(path),
        owner,
        group,
        state,
    )
}

fn chown_existing_path(path: &[u8], owner: u64, group: u64, state: &RiscvSyscallState) -> u64 {
    let existing_path = state.existing_guest_path_key(path);
    if let Some(path) = existing_path
        .as_deref()
        .filter(|path| state.guest_path_registered(path))
    {
        if state.guest_path_exists(path) {
            return chown_existing_node(owner, group);
        }
    }

    let directory_path = existing_path.as_deref().unwrap_or(path);
    if state.guest_directory_entries(directory_path).is_some() {
        return chown_existing_node(owner, group);
    }
    linux_error(RISCV_LINUX_ENOENT)
}

fn chown_fd(fd: GuestFd, owner: u64, group: u64, state: &RiscvSyscallState) -> u64 {
    match state.guest_fd_stat(fd) {
        Ok(_stat) => chown_existing_node(owner, group),
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}

fn chown_existing_node(owner: u64, group: u64) -> u64 {
    if chown_argument_is_no_change(owner) && chown_argument_is_no_change(group) {
        0
    } else {
        linux_error(RISCV_LINUX_EPERM)
    }
}

const fn chown_argument_is_no_change(argument: u64) -> bool {
    argument == RISCV_LINUX_CHOWN_NO_CHANGE || argument == RISCV_LINUX_CHOWN_TRUNCATED_NO_CHANGE
}

fn symlink_target_path(
    link_path: &[u8],
    target: &[u8],
    state: &RiscvSyscallState,
) -> Option<Vec<u8>> {
    let directory = link_path
        .iter()
        .rposition(|byte| *byte == b'/')
        .map(|index| &link_path[..index])
        .unwrap_or(b"");
    state
        .resolve_guest_path_from_directory(directory, target)
        .ok()
}

fn chmod_permissions(mode: u64) -> u32 {
    (mode as u32) & RISCV_LINUX_MODE_BITS
}

impl RiscvSyscallState {
    fn chmod_guest_path(
        &mut self,
        path: &[u8],
        permissions: u32,
    ) -> Result<(), RiscvGuestChmodError> {
        let existing_path = self.existing_guest_path_key(path);
        if let Some(path) = existing_path
            .as_deref()
            .filter(|path| self.guest_path_registered(path))
        {
            let identity = self.guest_file_identity(path);
            self.update_guest_file_permissions(identity, permissions);
            return Ok(());
        }

        let directory_path = existing_path.as_deref().unwrap_or(path);
        if self.guest_directory_entries(directory_path).is_some() {
            self.update_guest_directory_permissions(directory_path, permissions);
            return Ok(());
        }
        Err(RiscvGuestChmodError::Missing)
    }

    fn chmod_guest_fd(
        &mut self,
        fd: GuestFd,
        permissions: u32,
    ) -> Result<(), RiscvGuestChmodError> {
        let description = self
            .guest_fds
            .description_for_fd(fd)
            .map_err(|_error| RiscvGuestChmodError::BadFd)?
            .id();
        let stat = self
            .guest_file_stats
            .get(&description)
            .copied()
            .ok_or(RiscvGuestChmodError::BadFd)?;
        match stat.kind {
            RiscvGuestNodeKind::Directory => {
                let path = self
                    .guest_directory_paths
                    .get(&description)
                    .cloned()
                    .ok_or(RiscvGuestChmodError::BadFd)?;
                self.update_guest_directory_permissions(&path, permissions);
            }
            RiscvGuestNodeKind::RegularFile | RiscvGuestNodeKind::Symlink => {
                self.update_guest_file_permissions(stat.identity, permissions);
            }
        }
        Ok(())
    }

    fn update_guest_file_permissions(
        &mut self,
        identity: super::RiscvGuestFileIdentity,
        permissions: u32,
    ) {
        let permissions = permissions & RISCV_LINUX_MODE_BITS;
        self.guest_file_modes.insert(identity, permissions);
        for stat in self.guest_file_stats.values_mut() {
            if stat.identity == identity && stat.kind != RiscvGuestNodeKind::Directory {
                stat.permissions = permissions;
            }
        }
    }

    fn update_guest_directory_permissions(&mut self, path: &[u8], permissions: u32) {
        let permissions = permissions & RISCV_LINUX_MODE_BITS;
        self.guest_directory_modes
            .insert(path.to_vec(), permissions);
        let descriptions = matching_directory_descriptions(&self.guest_directory_paths, path);
        for description in descriptions {
            if let Some(stat) = self.guest_file_stats.get_mut(&description) {
                stat.permissions = permissions;
            }
        }
    }
}

fn matching_directory_descriptions(
    paths: &std::collections::BTreeMap<GuestFileDescriptionId, Vec<u8>>,
    path: &[u8],
) -> Vec<GuestFileDescriptionId> {
    paths
        .iter()
        .filter_map(|(description, candidate)| {
            (candidate.as_slice() == path).then_some(*description)
        })
        .collect()
}
