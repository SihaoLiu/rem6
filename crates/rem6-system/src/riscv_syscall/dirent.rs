use crate::GuestFdError;

use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_GETDENTS64: u64 = 61;
const RISCV_LINUX_DIRENT64_HEADER_BYTES: usize = 19;
const RISCV_LINUX_DIRENT64_ALIGN_BYTES: usize = 8;
const RISCV_LINUX_DT_DIR: u8 = 4;
const RISCV_LINUX_DT_REG: u8 = 8;
const RISCV_LINUX_DT_LNK: u8 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvGuestNodeKind {
    Directory,
    RegularFile,
    Symlink,
}

impl RiscvGuestNodeKind {
    const fn linux_dirent_type(self) -> u8 {
        match self {
            Self::Directory => RISCV_LINUX_DT_DIR,
            Self::RegularFile => RISCV_LINUX_DT_REG,
            Self::Symlink => RISCV_LINUX_DT_LNK,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvGuestDirectoryEntry {
    name: Vec<u8>,
    kind: RiscvGuestNodeKind,
    inode: u64,
}

impl RiscvGuestDirectoryEntry {
    pub(super) fn new(name: Vec<u8>, kind: RiscvGuestNodeKind, inode: u64) -> Self {
        Self { name, kind, inode }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RiscvGuestDirectoryReadError {
    BufferTooSmall,
    InvalidOffset,
    Fd(GuestFdError),
}

pub(super) fn syscall_getdents64(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(byte_count) = usize::try_from(request.argument(2)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if byte_count == 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let bytes = match state.guest_directory_prefix(fd, byte_count) {
        Ok(Some(bytes)) => bytes,
        Ok(None) | Err(RiscvGuestDirectoryReadError::Fd(_)) => {
            return linux_error(RISCV_LINUX_EBADF);
        }
        Err(
            RiscvGuestDirectoryReadError::BufferTooSmall
            | RiscvGuestDirectoryReadError::InvalidOffset,
        ) => {
            return linux_error(RISCV_LINUX_EINVAL);
        }
    };
    if bytes.is_empty() {
        return 0;
    }
    if !guest_memory.write(request.argument(1), &bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    if state
        .advance_guest_directory_offset(fd, bytes.len() as u64)
        .is_err()
    {
        return linux_error(RISCV_LINUX_EBADF);
    }
    bytes.len() as u64
}

pub(super) fn riscv_linux_dirent64_bytes(entries: &[RiscvGuestDirectoryEntry]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in entries {
        let record_len = aligned_dirent64_record_len(entry.name.len());
        let next_offset = u64::try_from(bytes.len() + record_len).unwrap_or(u64::MAX);
        bytes.extend_from_slice(&entry.inode.to_le_bytes());
        bytes.extend_from_slice(&next_offset.to_le_bytes());
        bytes.extend_from_slice(&(record_len as u16).to_le_bytes());
        bytes.push(entry.kind.linux_dirent_type());
        bytes.extend_from_slice(&entry.name);
        bytes.push(0);
        bytes.resize(
            bytes.len() + record_len - RISCV_LINUX_DIRENT64_HEADER_BYTES - entry.name.len() - 1,
            0,
        );
    }
    bytes
}

pub(super) fn guest_directory_child_name(directory: &[u8], path: &[u8]) -> Option<(Vec<u8>, bool)> {
    let directory = normalized_guest_directory_path(directory);
    let path = normalized_guest_directory_member_path(path);
    let relative = if directory.is_empty() {
        path
    } else {
        let rest = path.strip_prefix(directory)?;
        rest.strip_prefix(b"/")?
    };
    if relative.is_empty() {
        return None;
    }

    match relative.iter().position(|byte| *byte == b'/') {
        Some(0) => None,
        Some(separator) => Some((relative[..separator].to_vec(), true)),
        None => Some((relative.to_vec(), false)),
    }
}

pub(super) fn linux_dirent64_record_boundary(contents: &[u8], start: usize) -> bool {
    let mut offset = 0;
    while offset < contents.len() {
        if offset == start {
            return true;
        }
        if offset + 18 > contents.len() {
            return false;
        }
        let record_len =
            u16::from_le_bytes([contents[offset + 16], contents[offset + 17]]) as usize;
        if record_len < 24 || offset + record_len > contents.len() {
            return false;
        }
        offset += record_len;
    }
    offset == start
}

fn aligned_dirent64_record_len(name_bytes: usize) -> usize {
    let raw = RISCV_LINUX_DIRENT64_HEADER_BYTES + name_bytes + 1;
    raw.next_multiple_of(RISCV_LINUX_DIRENT64_ALIGN_BYTES)
}

fn normalized_guest_directory_path(path: &[u8]) -> &[u8] {
    if path == b"." || path == b"/" {
        b""
    } else {
        path.strip_prefix(b"/").unwrap_or(path)
    }
}

fn normalized_guest_directory_member_path(path: &[u8]) -> &[u8] {
    path.strip_prefix(b"/").unwrap_or(path)
}
