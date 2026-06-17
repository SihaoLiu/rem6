use super::{
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_IOV_MAX: u64 = 1024;

const RISCV_LINUX_IOV_BYTES: usize = 16;

#[derive(Clone, Copy)]
pub(super) struct RiscvIovec {
    pub(super) address: u64,
    pub(super) len: u64,
}

pub(super) fn read_iovecs(
    guest_memory: &RiscvGuestMemoryReader,
    iov_base: u64,
    iov_count: u64,
) -> Result<(Vec<RiscvIovec>, u64), u64> {
    let mut iovecs = Vec::with_capacity(iov_count as usize);
    let mut total = 0_u64;
    for index in 0..iov_count {
        let Some(iov_address) = iov_base.checked_add(index * RISCV_LINUX_IOV_BYTES as u64) else {
            return Err(RISCV_LINUX_EFAULT);
        };
        let Some(iov) = read_guest_exact(guest_memory, iov_address, RISCV_LINUX_IOV_BYTES) else {
            return Err(RISCV_LINUX_EFAULT);
        };
        let data_address = le_u64(&iov, 0);
        let data_len = le_u64(&iov, 8);
        total = total.checked_add(data_len).ok_or(RISCV_LINUX_EINVAL)?;
        iovecs.push(RiscvIovec {
            address: data_address,
            len: data_len,
        });
    }
    Ok((iovecs, total))
}

pub(super) fn read_iovec_bytes(
    guest_memory: &RiscvGuestMemoryReader,
    iovecs: &[RiscvIovec],
) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    for iovec in iovecs {
        let iov_len = usize::try_from(iovec.len).ok()?;
        if iov_len == 0 {
            continue;
        }
        let mut data = read_guest_exact(guest_memory, iovec.address, iov_len)?;
        bytes.append(&mut data);
    }
    Some(bytes)
}

pub(super) fn read_iovec_prefix(
    guest_memory: &RiscvGuestMemoryReader,
    iovecs: &[RiscvIovec],
    len: usize,
) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(len);
    for iovec in iovecs {
        if bytes.len() == len {
            break;
        }
        let iov_len = usize::try_from(iovec.len).ok()?;
        if iov_len == 0 {
            continue;
        }
        let chunk_len = iov_len.min(len - bytes.len());
        let mut chunk = read_guest_exact(guest_memory, iovec.address, chunk_len)?;
        bytes.append(&mut chunk);
    }
    (bytes.len() == len).then_some(bytes)
}

pub(super) fn write_iovecs(
    guest_memory: &RiscvGuestMemoryWriter,
    iovecs: &[RiscvIovec],
    bytes: &[u8],
) -> bool {
    let mut offset = 0usize;
    for iovec in iovecs {
        if offset == bytes.len() {
            return true;
        }
        let Ok(iov_len) = usize::try_from(iovec.len) else {
            return false;
        };
        if iov_len == 0 {
            continue;
        }
        let chunk_len = iov_len.min(bytes.len() - offset);
        if !guest_memory.write(iovec.address, &bytes[offset..offset + chunk_len]) {
            return false;
        }
        offset += chunk_len;
    }
    offset == bytes.len()
}

fn read_guest_exact(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
    len: usize,
) -> Option<Vec<u8>> {
    if len == 0 {
        return Some(Vec::new());
    }
    guest_memory
        .read(address, len)
        .filter(|bytes| bytes.len() == len)
}

fn le_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut raw = [0; 8];
    raw.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(raw)
}
