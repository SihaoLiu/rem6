use super::{
    linux_error, RiscvGuestMemoryMapResult, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_PAGE_BYTES: u64 = 4096;
pub(super) const RISCV64_LINUX_MMAP_BASE: u64 = 0x4000_0000_0000_0000;

pub(super) const RISCV_LINUX_MAP_SHARED: u64 = 0x01;
pub(super) const RISCV_LINUX_MAP_PRIVATE: u64 = 0x02;
pub(super) const RISCV_LINUX_MAP_FIXED: u64 = 0x10;
pub(super) const RISCV_LINUX_MAP_ANONYMOUS: u64 = 0x20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvMmapRegion {
    start: u64,
    length: u64,
    protection: u64,
    flags: u64,
    fd: u64,
    offset: u64,
}

impl RiscvMmapRegion {
    pub const fn new(
        start: u64,
        length: u64,
        protection: u64,
        flags: u64,
        fd: u64,
        offset: u64,
    ) -> Self {
        Self {
            start,
            length,
            protection,
            flags,
            fd,
            offset,
        }
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn length(self) -> u64 {
        self.length
    }

    pub const fn protection(self) -> u64 {
        self.protection
    }

    pub const fn flags(self) -> u64 {
        self.flags
    }

    pub const fn fd(self) -> u64 {
        self.fd
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub(super) fn overlaps(self, start: u64, length: u64) -> bool {
        let Some(end) = start.checked_add(length) else {
            return true;
        };
        let Some(region_end) = self.start.checked_add(self.length) else {
            return true;
        };
        start < region_end && self.start < end
    }

    pub(super) fn push_fragments_after_unmap(
        self,
        start: u64,
        length: u64,
        output: &mut Vec<Self>,
    ) {
        let Some(end) = start.checked_add(length) else {
            return;
        };
        let Some(region_end) = self.start.checked_add(self.length) else {
            return;
        };
        if start >= region_end || self.start >= end {
            output.push(self);
            return;
        }
        if self.start < start {
            output.push(Self::new(
                self.start,
                start - self.start,
                self.protection,
                self.flags,
                self.fd,
                self.offset,
            ));
        }
        if end < region_end {
            let delta = end - self.start;
            output.push(Self::new(
                end,
                region_end - end,
                self.protection,
                self.flags,
                self.fd,
                self.offset.saturating_add(delta),
            ));
        }
    }
}

pub(super) fn syscall_mmap(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let start = request.argument(0);
    let Some(length) = align_to_page(request.argument(1)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let protection = request.argument(2);
    let flags = request.argument(3);
    let fd = request.argument(4);
    let offset = request.argument(5);

    let shared = flags & RISCV_LINUX_MAP_SHARED != 0;
    let private = flags & RISCV_LINUX_MAP_PRIVATE != 0;
    if !start.is_multiple_of(RISCV_PAGE_BYTES)
        || !offset.is_multiple_of(RISCV_PAGE_BYTES)
        || shared == private
        || request.argument(1) == 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if start.checked_add(length).is_none() {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if flags & RISCV_LINUX_MAP_ANONYMOUS == 0 {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let fixed = flags & RISCV_LINUX_MAP_FIXED != 0;
    let mapped_start = if fixed {
        match install_anonymous_mmap_backing(start, length, guest_memory_writer, true) {
            RiscvGuestMemoryMapResult::Mapped => {
                state.unmap_mmap_range(start, length);
                start
            }
            RiscvGuestMemoryMapResult::Overlap | RiscvGuestMemoryMapResult::Failed => {
                return linux_error(RISCV_LINUX_EFAULT);
            }
        }
    } else {
        if start != 0 && state.is_mmap_range_available(start, length) {
            match install_anonymous_mmap_backing(start, length, guest_memory_writer, false) {
                RiscvGuestMemoryMapResult::Mapped => {
                    state.push_mmap_region(RiscvMmapRegion::new(
                        start, length, protection, flags, fd, offset,
                    ));
                    return start;
                }
                RiscvGuestMemoryMapResult::Overlap => {}
                RiscvGuestMemoryMapResult::Failed => return linux_error(RISCV_LINUX_EFAULT),
            }
        }

        let Some(mut candidate) = state.next_mmap_region_start(length) else {
            return linux_error(RISCV_LINUX_EINVAL);
        };
        loop {
            match install_anonymous_mmap_backing(candidate, length, guest_memory_writer, false) {
                RiscvGuestMemoryMapResult::Mapped => {
                    if state.advance_mmap_next(candidate, length).is_none() {
                        return linux_error(RISCV_LINUX_EINVAL);
                    }
                    break candidate;
                }
                RiscvGuestMemoryMapResult::Overlap => {
                    let Some(next) = candidate.checked_add(RISCV_PAGE_BYTES) else {
                        return linux_error(RISCV_LINUX_EINVAL);
                    };
                    let Some(next_candidate) = state.next_mmap_region_start_from(next, length)
                    else {
                        return linux_error(RISCV_LINUX_EINVAL);
                    };
                    candidate = next_candidate;
                }
                RiscvGuestMemoryMapResult::Failed => return linux_error(RISCV_LINUX_EFAULT),
            }
        }
    };
    state.push_mmap_region(RiscvMmapRegion::new(
        mapped_start,
        length,
        protection,
        flags,
        fd,
        offset,
    ));
    mapped_start
}

fn install_anonymous_mmap_backing(
    start: u64,
    length: u64,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
    replace_existing: bool,
) -> RiscvGuestMemoryMapResult {
    let Some(guest_memory_writer) = guest_memory_writer else {
        return RiscvGuestMemoryMapResult::Mapped;
    };
    match guest_memory_writer.map_region(start, length, replace_existing) {
        RiscvGuestMemoryMapResult::Mapped => {}
        RiscvGuestMemoryMapResult::Overlap if replace_existing => {}
        result @ (RiscvGuestMemoryMapResult::Overlap | RiscvGuestMemoryMapResult::Failed) => {
            return result;
        }
    }

    let zero_page = [0; RISCV_PAGE_BYTES as usize];
    let mut cursor = start;
    let mut remaining = length;
    while remaining > 0 {
        let bytes = remaining.min(RISCV_PAGE_BYTES);
        let end = bytes as usize;
        if !guest_memory_writer.write(cursor, &zero_page[..end]) {
            return RiscvGuestMemoryMapResult::Failed;
        }
        let Some(next) = cursor.checked_add(bytes) else {
            return RiscvGuestMemoryMapResult::Failed;
        };
        cursor = next;
        remaining -= bytes;
    }
    RiscvGuestMemoryMapResult::Mapped
}

pub(super) fn syscall_munmap(
    start: u64,
    requested_length: u64,
    state: &mut RiscvSyscallState,
) -> u64 {
    let Some(length) = align_to_page(requested_length) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if !start.is_multiple_of(RISCV_PAGE_BYTES)
        || requested_length == 0
        || start.checked_add(length).is_none()
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    state.unmap_mmap_range(start, length);
    0
}

fn align_to_page(value: u64) -> Option<u64> {
    value
        .checked_add(RISCV_PAGE_BYTES - 1)
        .map(|rounded| rounded & !(RISCV_PAGE_BYTES - 1))
}
