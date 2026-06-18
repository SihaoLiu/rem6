use std::collections::BTreeMap;

use crate::GuestFd;

use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestFileIdentity, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_E2BIG, RISCV_LINUX_EBADF,
    RISCV_LINUX_EEXIST, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_ENOENT, RISCV_LINUX_ERANGE, RISCV_LINUX_PATH_MAX,
};

const RISCV_LINUX_SETXATTR: u64 = 5;
const RISCV_LINUX_LSETXATTR: u64 = 6;
const RISCV_LINUX_FSETXATTR: u64 = 7;
const RISCV_LINUX_GETXATTR: u64 = 8;
const RISCV_LINUX_LGETXATTR: u64 = 9;
const RISCV_LINUX_FGETXATTR: u64 = 10;
const RISCV_LINUX_LISTXATTR: u64 = 11;
const RISCV_LINUX_LLISTXATTR: u64 = 12;
const RISCV_LINUX_FLISTXATTR: u64 = 13;
const RISCV_LINUX_REMOVEXATTR: u64 = 14;
const RISCV_LINUX_LREMOVEXATTR: u64 = 15;
const RISCV_LINUX_FREMOVEXATTR: u64 = 16;
const RISCV_LINUX_ENODATA: u64 = 61;
const RISCV_LINUX_XATTR_CREATE: u64 = 1;
const RISCV_LINUX_XATTR_REPLACE: u64 = 2;
const RISCV_LINUX_XATTR_NAME_MAX: usize = 255;
const RISCV_LINUX_XATTR_SIZE_MAX: u64 = 64 * 1024;

impl RiscvSyscallState {
    fn xattrs_for_identity_mut(
        &mut self,
        identity: RiscvGuestFileIdentity,
    ) -> &mut BTreeMap<Vec<u8>, Vec<u8>> {
        self.guest_xattrs.entry(identity).or_default()
    }

    fn xattrs_for_identity(
        &self,
        identity: RiscvGuestFileIdentity,
    ) -> Option<&BTreeMap<Vec<u8>, Vec<u8>>> {
        self.guest_xattrs.get(&identity)
    }

    pub(super) fn drop_guest_xattrs_if_unlinked(&mut self, identity: RiscvGuestFileIdentity) {
        if !self.guest_xattr_identity_is_live(identity) {
            self.guest_xattrs.remove(&identity);
        }
    }

    fn guest_xattr_identity_is_live(&self, identity: RiscvGuestFileIdentity) -> bool {
        self.guest_file_identities
            .values()
            .any(|candidate| *candidate == identity)
            || self
                .guest_directory_identities
                .values()
                .any(|candidate| *candidate == identity)
            || self
                .guest_file_stats
                .values()
                .any(|stat| stat.identity == identity)
    }

    fn xattr_identity_for_fd(&self, fd: GuestFd) -> Result<RiscvGuestFileIdentity, u64> {
        let description = self
            .guest_fds
            .description_for_fd(fd)
            .map_err(|_error| RISCV_LINUX_EBADF)?
            .id();
        self.guest_file_stats
            .get(&description)
            .map(|stat| stat.identity)
            .ok_or(RISCV_LINUX_EBADF)
    }

    fn xattr_identity_for_path(
        &self,
        path: &[u8],
        nofollow: bool,
    ) -> Result<RiscvGuestFileIdentity, u64> {
        if path.is_empty() {
            return Err(RISCV_LINUX_ENOENT);
        }
        let resolved = if nofollow {
            self.resolve_guest_path_following_intermediate_symlinks(path)
        } else {
            self.resolve_guest_path_following_symlinks(path)
        }
        .map_err(|error| error.linux_error_code())?;
        let path = self.existing_guest_path_key(&resolved).unwrap_or(resolved);
        if self.guest_path_registered(&path) || self.guest_links.contains_key(&path) {
            return Ok(self.guest_file_identity(&path));
        }
        if self.guest_directories.contains(&path) {
            return Ok(self.guest_directory_identity(&path));
        }
        Err(RISCV_LINUX_ENOENT)
    }
}

pub(super) fn syscall_xattr(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    let outcome = match request.number() {
        RISCV_LINUX_SETXATTR | RISCV_LINUX_LSETXATTR => {
            let reader = guest_memory_reader?;
            syscall_setxattr_path(
                request,
                state,
                reader,
                request.number() == RISCV_LINUX_LSETXATTR,
            )
        }
        RISCV_LINUX_FSETXATTR => {
            let reader = guest_memory_reader?;
            syscall_setxattr_fd(request, state, reader)
        }
        RISCV_LINUX_GETXATTR | RISCV_LINUX_LGETXATTR => {
            let reader = guest_memory_reader?;
            let writer = guest_memory_writer?;
            syscall_getxattr_path(
                request,
                state,
                reader,
                writer,
                request.number() == RISCV_LINUX_LGETXATTR,
            )
        }
        RISCV_LINUX_FGETXATTR => {
            let reader = guest_memory_reader?;
            let writer = guest_memory_writer?;
            syscall_getxattr_fd(request, state, reader, writer)
        }
        RISCV_LINUX_LISTXATTR | RISCV_LINUX_LLISTXATTR => {
            let reader = guest_memory_reader?;
            let writer = guest_memory_writer?;
            syscall_listxattr_path(
                request,
                state,
                reader,
                writer,
                request.number() == RISCV_LINUX_LLISTXATTR,
            )
        }
        RISCV_LINUX_FLISTXATTR => {
            let writer = guest_memory_writer?;
            syscall_listxattr_fd(request, state, writer)
        }
        RISCV_LINUX_REMOVEXATTR | RISCV_LINUX_LREMOVEXATTR => {
            let reader = guest_memory_reader?;
            syscall_removexattr_path(
                request,
                state,
                reader,
                request.number() == RISCV_LINUX_LREMOVEXATTR,
            )
        }
        RISCV_LINUX_FREMOVEXATTR => {
            let reader = guest_memory_reader?;
            syscall_removexattr_fd(request, state, reader)
        }
        _ => unreachable!("RISC-V Linux xattr syscall range is handled by caller"),
    };
    Some(RiscvSyscallOutcome::Return { value: outcome })
}

fn syscall_setxattr_path(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
    nofollow: bool,
) -> u64 {
    let identity = match path_xattr_identity(request.argument(0), state, guest_memory, nofollow) {
        Ok(identity) => identity,
        Err(error) => return linux_error(error),
    };
    syscall_setxattr_identity(
        identity,
        request.argument(1),
        request.argument(2),
        request.argument(3),
        request.argument(4),
        state,
        guest_memory,
    )
}

fn syscall_setxattr_fd(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let identity = match state.xattr_identity_for_fd(fd) {
        Ok(identity) => identity,
        Err(error) => return linux_error(error),
    };
    syscall_setxattr_identity(
        identity,
        request.argument(1),
        request.argument(2),
        request.argument(3),
        request.argument(4),
        state,
        guest_memory,
    )
}

fn syscall_getxattr_path(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
    nofollow: bool,
) -> u64 {
    let identity =
        match path_xattr_identity(request.argument(0), state, guest_memory_reader, nofollow) {
            Ok(identity) => identity,
            Err(error) => return linux_error(error),
        };
    syscall_getxattr_identity(
        identity,
        request.argument(1),
        request.argument(2),
        request.argument(3),
        state,
        guest_memory_reader,
        guest_memory_writer,
    )
}

fn syscall_getxattr_fd(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let identity = match state.xattr_identity_for_fd(fd) {
        Ok(identity) => identity,
        Err(error) => return linux_error(error),
    };
    syscall_getxattr_identity(
        identity,
        request.argument(1),
        request.argument(2),
        request.argument(3),
        state,
        guest_memory_reader,
        guest_memory_writer,
    )
}

fn syscall_listxattr_path(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
    nofollow: bool,
) -> u64 {
    let identity =
        match path_xattr_identity(request.argument(0), state, guest_memory_reader, nofollow) {
            Ok(identity) => identity,
            Err(error) => return linux_error(error),
        };
    syscall_listxattr_identity(
        identity,
        request.argument(1),
        request.argument(2),
        state,
        guest_memory_writer,
    )
}

fn syscall_listxattr_fd(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let identity = match state.xattr_identity_for_fd(fd) {
        Ok(identity) => identity,
        Err(error) => return linux_error(error),
    };
    syscall_listxattr_identity(
        identity,
        request.argument(1),
        request.argument(2),
        state,
        guest_memory_writer,
    )
}

fn syscall_removexattr_path(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
    nofollow: bool,
) -> u64 {
    let identity = match path_xattr_identity(request.argument(0), state, guest_memory, nofollow) {
        Ok(identity) => identity,
        Err(error) => return linux_error(error),
    };
    syscall_removexattr_identity(identity, request.argument(1), state, guest_memory)
}

fn syscall_removexattr_fd(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let identity = match state.xattr_identity_for_fd(fd) {
        Ok(identity) => identity,
        Err(error) => return linux_error(error),
    };
    syscall_removexattr_identity(identity, request.argument(1), state, guest_memory)
}

fn syscall_setxattr_identity(
    identity: RiscvGuestFileIdentity,
    name_address: u64,
    value_address: u64,
    value_bytes: u64,
    flags: u64,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if flags & !(RISCV_LINUX_XATTR_CREATE | RISCV_LINUX_XATTR_REPLACE) != 0
        || flags == (RISCV_LINUX_XATTR_CREATE | RISCV_LINUX_XATTR_REPLACE)
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let name = match read_xattr_name(name_address, guest_memory) {
        Ok(name) => name,
        Err(error) => return linux_error(error),
    };
    if value_bytes > RISCV_LINUX_XATTR_SIZE_MAX {
        return linux_error(RISCV_LINUX_E2BIG);
    }
    let Ok(value_bytes) = usize::try_from(value_bytes) else {
        return linux_error(RISCV_LINUX_E2BIG);
    };
    let Some(value) = guest_memory.read(value_address, value_bytes) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    if value.len() != value_bytes {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    let exists = state
        .xattrs_for_identity(identity)
        .is_some_and(|xattrs| xattrs.contains_key(&name));
    if flags & RISCV_LINUX_XATTR_CREATE != 0 && exists {
        return linux_error(RISCV_LINUX_EEXIST);
    }
    if flags & RISCV_LINUX_XATTR_REPLACE != 0 && !exists {
        return linux_error(RISCV_LINUX_ENODATA);
    }
    state.xattrs_for_identity_mut(identity).insert(name, value);
    0
}

fn syscall_getxattr_identity(
    identity: RiscvGuestFileIdentity,
    name_address: u64,
    value_address: u64,
    value_bytes: u64,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let name = match read_xattr_name(name_address, guest_memory_reader) {
        Ok(name) => name,
        Err(error) => return linux_error(error),
    };
    let Some(value) = state
        .xattrs_for_identity(identity)
        .and_then(|xattrs| xattrs.get(&name))
    else {
        return linux_error(RISCV_LINUX_ENODATA);
    };
    write_xattr_bytes(value_address, value_bytes, value, guest_memory_writer)
}

fn syscall_listxattr_identity(
    identity: RiscvGuestFileIdentity,
    list_address: u64,
    list_bytes: u64,
    state: &RiscvSyscallState,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let mut names = Vec::new();
    if let Some(xattrs) = state.xattrs_for_identity(identity) {
        for name in xattrs.keys() {
            names.extend_from_slice(name);
            names.push(0);
        }
    }
    write_xattr_bytes(list_address, list_bytes, &names, guest_memory_writer)
}

fn syscall_removexattr_identity(
    identity: RiscvGuestFileIdentity,
    name_address: u64,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let name = match read_xattr_name(name_address, guest_memory) {
        Ok(name) => name,
        Err(error) => return linux_error(error),
    };
    let Some(xattrs) = state.guest_xattrs.get_mut(&identity) else {
        return linux_error(RISCV_LINUX_ENODATA);
    };
    if xattrs.remove(&name).is_none() {
        return linux_error(RISCV_LINUX_ENODATA);
    }
    if xattrs.is_empty() {
        state.guest_xattrs.remove(&identity);
    }
    0
}

fn path_xattr_identity(
    path_address: u64,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
    nofollow: bool,
) -> Result<RiscvGuestFileIdentity, u64> {
    let path =
        read_guest_c_string(guest_memory, path_address, RISCV_LINUX_PATH_MAX).map_err(|error| {
            match error {
                RiscvGuestCStringError::Fault => RISCV_LINUX_EFAULT,
                RiscvGuestCStringError::TooLong => RISCV_LINUX_ENAMETOOLONG,
            }
        })?;
    state.xattr_identity_for_path(&path, nofollow)
}

fn read_xattr_name(address: u64, guest_memory: &RiscvGuestMemoryReader) -> Result<Vec<u8>, u64> {
    let name = read_guest_c_string(guest_memory, address, RISCV_LINUX_XATTR_NAME_MAX + 1).map_err(
        |error| match error {
            RiscvGuestCStringError::Fault => RISCV_LINUX_EFAULT,
            RiscvGuestCStringError::TooLong => RISCV_LINUX_ERANGE,
        },
    )?;
    if name.is_empty() {
        return Err(RISCV_LINUX_ERANGE);
    }
    Ok(name)
}

fn write_xattr_bytes(
    address: u64,
    capacity: u64,
    bytes: &[u8],
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    if capacity == 0 {
        return bytes.len() as u64;
    }
    let Ok(capacity) = usize::try_from(capacity) else {
        return linux_error(RISCV_LINUX_ERANGE);
    };
    if capacity < bytes.len() {
        return linux_error(RISCV_LINUX_ERANGE);
    }
    for (offset, byte) in bytes.iter().enumerate() {
        let Some(address) = address.checked_add(offset as u64) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        if !guest_memory.write(address, std::slice::from_ref(byte)) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    bytes.len() as u64
}
