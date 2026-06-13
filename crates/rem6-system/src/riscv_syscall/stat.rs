use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallIdentity, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_AT_EMPTY_PATH, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_AT_NO_AUTOMOUNT, RISCV_LINUX_AT_SYMLINK_NOFOLLOW, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT,
    RISCV_LINUX_ENOTDIR, RISCV_LINUX_PATH_MAX,
};

const RISCV_LINUX_STAT_BYTES: usize = 128;
const RISCV_LINUX_STATX_BYTES: usize = 256;
const RISCV_LINUX_STAT_BLOCK_BYTES: u64 = 512;
const RISCV_LINUX_STAT_BLOCK_SIZE: u64 = 8192;
const RISCV_LINUX_STATX_BASIC_STATS: u32 = 0x0000_07ff;
const RISCV_LINUX_STATX_RESERVED: u64 = 0x8000_0000;
const RISCV_LINUX_S_IFCHR: u32 = 0o020000;
const RISCV_LINUX_S_IFDIR: u32 = 0o040000;
const RISCV_LINUX_S_IFREG: u32 = 0o100000;
const RISCV_LINUX_S_IFLNK: u32 = 0o120000;
pub(super) const RISCV_LINUX_DEFAULT_REGULAR_FILE_PERMISSIONS: u32 = 0o444;
pub(super) const RISCV_LINUX_DEFAULT_DIRECTORY_PERMISSIONS: u32 = 0o555;
const RISCV_LINUX_CHARACTER_DEVICE_MODE: u32 = RISCV_LINUX_S_IFCHR | 0o666;
const RISCV_LINUX_SYMBOLIC_LINK_MODE: u32 = RISCV_LINUX_S_IFLNK | 0o777;
pub(super) const RISCV_LINUX_FACCESSAT: u64 = 48;
pub(super) const RISCV_LINUX_STATX: u64 = 291;
pub(super) const RISCV_LINUX_ACCESS: u64 = 1033;
pub(super) const RISCV_LINUX_LSTAT: u64 = 1039;
const RISCV_LINUX_EACCES: u64 = 13;
const RISCV_LINUX_X_OK: u64 = 1;
const RISCV_LINUX_W_OK: u64 = 2;
const RISCV_LINUX_R_OK: u64 = 4;
const RISCV_LINUX_ACCESS_VALID_MODE: u64 = RISCV_LINUX_X_OK | RISCV_LINUX_W_OK | RISCV_LINUX_R_OK;
const RISCV_LINUX_AT_STATX_SYNC_TYPE: u64 = 0x6000;
const RISCV_LINUX_STATX_VALID_FLAGS: u64 = RISCV_LINUX_AT_EMPTY_PATH
    | RISCV_LINUX_AT_NO_AUTOMOUNT
    | RISCV_LINUX_AT_SYMLINK_NOFOLLOW
    | RISCV_LINUX_AT_STATX_SYNC_TYPE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestStat {
    device: u64,
    inode: u64,
    mode: u32,
    link_count: u32,
    user_id: u32,
    group_id: u32,
    special_device: u64,
    size: u64,
    block_size: u64,
    blocks: u64,
}

impl RiscvGuestStat {
    pub(super) fn regular_file(
        size: u64,
        identity: RiscvSyscallIdentity,
        inode: u64,
        link_count: u32,
        permissions: u32,
    ) -> Self {
        Self {
            device: 0,
            inode,
            mode: RISCV_LINUX_S_IFREG | (permissions & 0o777),
            link_count,
            user_id: linux_stat_user_id(identity.user_id()),
            group_id: linux_stat_user_id(identity.group_id()),
            special_device: 0,
            size,
            block_size: RISCV_LINUX_STAT_BLOCK_SIZE,
            blocks: size.div_ceil(RISCV_LINUX_STAT_BLOCK_BYTES),
        }
    }

    pub(super) fn character_device(identity: RiscvSyscallIdentity, inode: u64) -> Self {
        Self {
            device: 0x0a,
            inode,
            mode: RISCV_LINUX_CHARACTER_DEVICE_MODE,
            link_count: 1,
            user_id: linux_stat_user_id(identity.user_id()),
            group_id: linux_stat_user_id(identity.group_id()),
            special_device: 0x880d,
            size: 0,
            block_size: RISCV_LINUX_STAT_BLOCK_SIZE,
            blocks: 0,
        }
    }

    pub(super) fn directory(identity: RiscvSyscallIdentity, inode: u64, permissions: u32) -> Self {
        Self {
            device: 0,
            inode,
            mode: RISCV_LINUX_S_IFDIR | (permissions & 0o777),
            link_count: 2,
            user_id: linux_stat_user_id(identity.user_id()),
            group_id: linux_stat_user_id(identity.group_id()),
            special_device: 0,
            size: 0,
            block_size: RISCV_LINUX_STAT_BLOCK_SIZE,
            blocks: 0,
        }
    }

    pub(super) fn symbolic_link(
        size: u64,
        identity: RiscvSyscallIdentity,
        inode: u64,
        link_count: u32,
    ) -> Self {
        Self {
            device: 0,
            inode,
            mode: RISCV_LINUX_SYMBOLIC_LINK_MODE,
            link_count,
            user_id: linux_stat_user_id(identity.user_id()),
            group_id: linux_stat_user_id(identity.group_id()),
            special_device: 0,
            size,
            block_size: RISCV_LINUX_STAT_BLOCK_SIZE,
            blocks: size.div_ceil(RISCV_LINUX_STAT_BLOCK_BYTES),
        }
    }

    pub(super) const fn size(self) -> u64 {
        self.size
    }

    pub(super) const fn is_regular_file(self) -> bool {
        self.mode & RISCV_LINUX_S_IFREG == RISCV_LINUX_S_IFREG
    }

    pub(super) const fn allows_access(self, mode: u64) -> bool {
        (mode & RISCV_LINUX_R_OK == 0 || self.mode & 0o444 != 0)
            && (mode & RISCV_LINUX_W_OK == 0 || self.mode & 0o222 != 0)
            && (mode & RISCV_LINUX_X_OK == 0 || self.mode & 0o111 != 0)
    }
}

pub(super) fn guest_path_inode(path: &[u8]) -> u64 {
    path.iter()
        .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
        })
        .max(1)
}

pub(super) fn write_riscv_linux_stat(
    address: u64,
    stat: RiscvGuestStat,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let bytes = riscv_linux_stat_bytes(stat);
    for (offset, byte) in bytes.iter().enumerate() {
        let Some(byte_address) = address.checked_add(offset as u64) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        if !guest_memory.write(byte_address, std::slice::from_ref(byte)) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    0
}

pub(super) fn syscall_newfstatat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let flags = request.argument(3);
    if flags
        & !(RISCV_LINUX_AT_EMPTY_PATH
            | RISCV_LINUX_AT_NO_AUTOMOUNT
            | RISCV_LINUX_AT_SYMLINK_NOFOLLOW)
        != 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let path = match read_guest_c_string(
        guest_memory_reader,
        request.argument(1),
        RISCV_LINUX_PATH_MAX,
    ) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };

    let stat = if path.is_empty() {
        if flags & RISCV_LINUX_AT_EMPTY_PATH == 0 {
            return linux_error(RISCV_LINUX_ENOENT);
        }
        let Some(fd) = guest_fd_argument(request.argument(0)) else {
            return linux_error(RISCV_LINUX_EBADF);
        };
        match state.guest_fd_stat(fd) {
            Ok(stat) => stat,
            Err(_error) => return linux_error(RISCV_LINUX_EBADF),
        }
    } else {
        if request.argument(0) != RISCV_LINUX_AT_FDCWD {
            return linux_error(RISCV_LINUX_EBADF);
        }
        match state.guest_path_stat(&path) {
            Some(stat) => stat,
            None => return linux_error(RISCV_LINUX_ENOENT),
        }
    };

    write_riscv_linux_stat(request.argument(2), stat, guest_memory_writer)
}

pub(super) fn syscall_stat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let path = match read_guest_c_string(
        guest_memory_reader,
        request.argument(0),
        RISCV_LINUX_PATH_MAX,
    ) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let Some(stat) = state.guest_path_stat(&path) else {
        return linux_error(RISCV_LINUX_ENOENT);
    };

    write_riscv_linux_stat(request.argument(1), stat, guest_memory_writer)
}

pub(super) fn syscall_lstat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let path = match read_guest_c_string(
        guest_memory_reader,
        request.argument(0),
        RISCV_LINUX_PATH_MAX,
    ) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let Some(stat) = state
        .guest_link_stat(&path)
        .or_else(|| state.guest_path_stat(&path))
    else {
        return linux_error(RISCV_LINUX_ENOENT);
    };

    write_riscv_linux_stat(request.argument(1), stat, guest_memory_writer)
}

pub(super) fn syscall_statx(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    let flags = request.argument(2);
    if invalid_statx_flags(flags) || request.argument(3) & RISCV_LINUX_STATX_RESERVED != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let path = match read_guest_c_string(
        guest_memory_reader,
        request.argument(1),
        RISCV_LINUX_PATH_MAX,
    ) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };

    let stat = if path.is_empty() {
        if flags & RISCV_LINUX_AT_EMPTY_PATH == 0 {
            return linux_error(RISCV_LINUX_ENOENT);
        }
        if request.argument(0) == RISCV_LINUX_AT_FDCWD {
            match state.guest_path_stat(b".") {
                Some(stat) => stat,
                None => return linux_error(RISCV_LINUX_ENOENT),
            }
        } else {
            let Some(fd) = guest_fd_argument(request.argument(0)) else {
                return linux_error(RISCV_LINUX_EBADF);
            };
            match state.guest_fd_stat(fd) {
                Ok(stat) => stat,
                Err(_error) => return linux_error(RISCV_LINUX_EBADF),
            }
        }
    } else {
        let path = match resolve_statx_path(request.argument(0), &path, state) {
            Ok(path) => path,
            Err(error) => return error,
        };
        let stat = if flags & RISCV_LINUX_AT_SYMLINK_NOFOLLOW != 0 {
            state
                .guest_link_stat(&path)
                .or_else(|| state.guest_path_stat(&path))
        } else {
            state.guest_path_stat(&path)
        };
        let Some(stat) = stat else {
            return linux_error(RISCV_LINUX_ENOENT);
        };
        stat
    };

    write_riscv_linux_statx(request.argument(4), stat, guest_memory_writer)
}

const fn invalid_statx_flags(flags: u64) -> bool {
    flags & !RISCV_LINUX_STATX_VALID_FLAGS != 0
        || flags & RISCV_LINUX_AT_STATX_SYNC_TYPE == RISCV_LINUX_AT_STATX_SYNC_TYPE
}

fn resolve_statx_path(dirfd: u64, path: &[u8], state: &RiscvSyscallState) -> Result<Vec<u8>, u64> {
    if dirfd == RISCV_LINUX_AT_FDCWD || path.starts_with(b"/") {
        return Ok(path.to_vec());
    }

    let Some(fd) = guest_fd_argument(dirfd) else {
        return Err(linux_error(RISCV_LINUX_EBADF));
    };
    let directory = match state.guest_directory_path_for_fd(fd) {
        Ok(Some(path)) => path,
        Ok(None) => return Err(linux_error(RISCV_LINUX_ENOTDIR)),
        Err(_error) => return Err(linux_error(RISCV_LINUX_EBADF)),
    };
    let resolved = state
        .resolve_guest_path_from_directory(&directory, path)
        .map_err(|error| linux_error(error.linux_error_code()))?;
    if resolved.is_empty() || resolved.starts_with(b"/") {
        Ok(resolved)
    } else {
        let mut absolute = Vec::with_capacity(resolved.len() + 1);
        absolute.push(b'/');
        absolute.extend_from_slice(&resolved);
        Ok(absolute)
    }
}

pub(super) fn syscall_access(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
) -> u64 {
    let mode = request.argument(1);
    let path = match read_access_path(request.argument(0), mode, guest_memory_reader) {
        Ok(path) => path,
        Err(error) => return error,
    };
    syscall_access_registered_path(&path, mode, state)
}

pub(super) fn syscall_faccessat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
) -> u64 {
    let mode = request.argument(2);
    let path = match read_access_path(request.argument(1), mode, guest_memory_reader) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    if request.argument(0) != RISCV_LINUX_AT_FDCWD && !path.starts_with(b"/") {
        return linux_error(RISCV_LINUX_EBADF);
    }
    syscall_access_registered_path(&path, mode, state)
}

fn read_access_path(
    path_address: u64,
    mode: u64,
    guest_memory_reader: &RiscvGuestMemoryReader,
) -> Result<Vec<u8>, u64> {
    if mode & !RISCV_LINUX_ACCESS_VALID_MODE != 0 {
        return Err(linux_error(RISCV_LINUX_EINVAL));
    }

    read_guest_c_string(guest_memory_reader, path_address, RISCV_LINUX_PATH_MAX).map_err(|error| {
        linux_error(match error {
            RiscvGuestCStringError::Fault => RISCV_LINUX_EFAULT,
            RiscvGuestCStringError::TooLong => RISCV_LINUX_ENAMETOOLONG,
        })
    })
}

fn syscall_access_registered_path(path: &[u8], mode: u64, state: &RiscvSyscallState) -> u64 {
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let Some(stat) = state.guest_path_stat(path) else {
        return linux_error(RISCV_LINUX_ENOENT);
    };
    if !stat.allows_access(mode) {
        return linux_error(RISCV_LINUX_EACCES);
    }

    0
}

pub(super) fn syscall_fstat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let stat = match state.guest_fd_stat(fd) {
        Ok(stat) => stat,
        Err(_error) => return linux_error(RISCV_LINUX_EBADF),
    };
    write_riscv_linux_stat(request.argument(1), stat, guest_memory)
}

fn riscv_linux_stat_bytes(stat: RiscvGuestStat) -> [u8; RISCV_LINUX_STAT_BYTES] {
    let mut bytes = [0; RISCV_LINUX_STAT_BYTES];
    write_le_u64(&mut bytes, 0, stat.device);
    write_le_u64(&mut bytes, 8, stat.inode);
    write_le_u32(&mut bytes, 16, stat.mode);
    write_le_u32(&mut bytes, 20, stat.link_count);
    write_le_u32(&mut bytes, 24, stat.user_id);
    write_le_u32(&mut bytes, 28, stat.group_id);
    write_le_u64(&mut bytes, 32, stat.special_device);
    write_le_u64(&mut bytes, 48, stat.size);
    write_le_u64(&mut bytes, 56, stat.block_size);
    write_le_u64(&mut bytes, 64, stat.blocks);
    bytes
}

fn write_riscv_linux_statx(
    address: u64,
    stat: RiscvGuestStat,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let bytes = riscv_linux_statx_bytes(stat);
    for (offset, byte) in bytes.iter().enumerate() {
        let Some(byte_address) = address.checked_add(offset as u64) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        if !guest_memory.write(byte_address, std::slice::from_ref(byte)) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    0
}

fn riscv_linux_statx_bytes(stat: RiscvGuestStat) -> [u8; RISCV_LINUX_STATX_BYTES] {
    let mut bytes = [0; RISCV_LINUX_STATX_BYTES];
    write_le_u32(&mut bytes, 0, RISCV_LINUX_STATX_BASIC_STATS);
    write_le_u32(
        &mut bytes,
        4,
        stat.block_size.min(u64::from(u32::MAX)) as u32,
    );
    write_le_u32(&mut bytes, 16, stat.link_count);
    write_le_u32(&mut bytes, 20, stat.user_id);
    write_le_u32(&mut bytes, 24, stat.group_id);
    write_le_u16(&mut bytes, 28, stat.mode as u16);
    write_le_u64(&mut bytes, 32, stat.inode);
    write_le_u64(&mut bytes, 40, stat.size);
    write_le_u64(&mut bytes, 48, stat.blocks);
    bytes
}

fn linux_stat_user_id(value: u64) -> u32 {
    value.min(u32::MAX as u64) as u32
}

fn write_le_u16(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_le_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_le_u64(output: &mut [u8], offset: usize, value: u64) {
    output[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
