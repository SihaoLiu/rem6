use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryMapResult, RiscvGuestMemoryWriter,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT,
    RISCV_LINUX_EINVAL, RISCV_LINUX_ENOMEM,
};

pub(super) const RISCV_PAGE_BYTES: u64 = 4096;
pub(super) const RISCV64_LINUX_MMAP_BASE: u64 = 0x4000_0000_0000_0000;

pub(super) const RISCV_LINUX_MUNMAP: u64 = 215;
pub(super) const RISCV_LINUX_MREMAP: u64 = 216;
pub(super) const RISCV_LINUX_MMAP: u64 = 222;
pub(super) const RISCV_LINUX_MPROTECT: u64 = 226;
pub(super) const RISCV_LINUX_MSYNC: u64 = 227;
pub(super) const RISCV_LINUX_MLOCK: u64 = 228;
pub(super) const RISCV_LINUX_MUNLOCK: u64 = 229;
pub(super) const RISCV_LINUX_MINCORE: u64 = 232;
pub(super) const RISCV_LINUX_MADVISE: u64 = 233;
pub(super) const RISCV_LINUX_MAP_SHARED: u64 = 0x01;
pub(super) const RISCV_LINUX_MAP_PRIVATE: u64 = 0x02;
pub(super) const RISCV_LINUX_MAP_FIXED: u64 = 0x10;
pub(super) const RISCV_LINUX_MAP_ANONYMOUS: u64 = 0x20;
const RISCV_LINUX_MREMAP_MAYMOVE: u64 = 1;
const RISCV_LINUX_MREMAP_FIXED: u64 = 2;
const RISCV_LINUX_MREMAP_DONTUNMAP: u64 = 4;
const RISCV_LINUX_MREMAP_SUPPORTED_FLAGS: u64 = RISCV_LINUX_MREMAP_MAYMOVE;
const RISCV_LINUX_MS_ASYNC: u64 = 1;
const RISCV_LINUX_MS_INVALIDATE: u64 = 2;
const RISCV_LINUX_MS_SYNC: u64 = 4;
const RISCV_LINUX_MS_VALID_FLAGS: u64 =
    RISCV_LINUX_MS_ASYNC | RISCV_LINUX_MS_INVALIDATE | RISCV_LINUX_MS_SYNC;
const RISCV_LINUX_MADV_NORMAL: u64 = 0;
const RISCV_LINUX_MADV_RANDOM: u64 = 1;
const RISCV_LINUX_MADV_SEQUENTIAL: u64 = 2;
const RISCV_LINUX_MADV_WILLNEED: u64 = 3;
const RISCV_LINUX_MADV_DONTNEED: u64 = 4;
const RISCV_LINUX_MADV_FREE: u64 = 8;
const RISCV_LINUX_MADV_REMOVE: u64 = 9;
const RISCV_LINUX_MADV_DONTFORK: u64 = 10;
const RISCV_LINUX_MADV_DOFORK: u64 = 11;
const RISCV_LINUX_MADV_MERGEABLE: u64 = 12;
const RISCV_LINUX_MADV_UNMERGEABLE: u64 = 13;
const RISCV_LINUX_MADV_HUGEPAGE: u64 = 14;
const RISCV_LINUX_MADV_NOHUGEPAGE: u64 = 15;
const RISCV_LINUX_MADV_DONTDUMP: u64 = 16;
const RISCV_LINUX_MADV_DODUMP: u64 = 17;
const RISCV_LINUX_MADV_WIPEONFORK: u64 = 18;
const RISCV_LINUX_MADV_KEEPONFORK: u64 = 19;
const RISCV_LINUX_MADV_COLD: u64 = 20;
const RISCV_LINUX_MADV_PAGEOUT: u64 = 21;
const RISCV_LINUX_MADV_POPULATE_READ: u64 = 22;
const RISCV_LINUX_MADV_POPULATE_WRITE: u64 = 23;
const RISCV_LINUX_MADV_DONTNEED_LOCKED: u64 = 24;
const RISCV_LINUX_MADV_COLLAPSE: u64 = 25;
const RISCV_LINUX_MADV_GUARD_INSTALL: u64 = 102;
const RISCV_LINUX_MADV_GUARD_REMOVE: u64 = 103;

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

    pub(super) fn push_fragments_after_mprotect(
        self,
        start: u64,
        length: u64,
        protection: u64,
        output: &mut Vec<Self>,
    ) {
        let Some(end) = start.checked_add(length) else {
            output.push(self);
            return;
        };
        let Some(region_end) = self.start.checked_add(self.length) else {
            output.push(self);
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

        let protect_start = self.start.max(start);
        let protect_end = region_end.min(end);
        let protect_delta = protect_start - self.start;
        output.push(Self::new(
            protect_start,
            protect_end - protect_start,
            protection,
            self.flags,
            self.fd,
            self.offset.saturating_add(protect_delta),
        ));

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
    let backing = match mmap_backing(flags, fd, offset, length, state) {
        Some(backing) => backing,
        None => return linux_error(RISCV_LINUX_EBADF),
    };

    let fixed = flags & RISCV_LINUX_MAP_FIXED != 0;
    let mapped_start = if fixed {
        match install_mmap_backing(start, length, guest_memory_writer, true, &backing) {
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
            match install_mmap_backing(start, length, guest_memory_writer, false, &backing) {
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
            match install_mmap_backing(candidate, length, guest_memory_writer, false, &backing) {
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

fn mmap_backing(
    flags: u64,
    fd: u64,
    offset: u64,
    length: u64,
    state: &RiscvSyscallState,
) -> Option<MmapBacking> {
    if flags & RISCV_LINUX_MAP_ANONYMOUS != 0 {
        return Some(MmapBacking::Anonymous);
    }

    let fd = guest_fd_argument(fd)?;
    let byte_count = usize::try_from(length).unwrap_or(usize::MAX);
    let contents = state.guest_file_slice_at(fd, offset, byte_count).ok()??;
    Some(MmapBacking::File { contents })
}

enum MmapBacking {
    Anonymous,
    File { contents: Vec<u8> },
}

fn install_mmap_backing(
    start: u64,
    length: u64,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
    replace_existing: bool,
    backing: &MmapBacking,
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

    match backing {
        MmapBacking::Anonymous => write_zeroed_backing(start, length, guest_memory_writer),
        MmapBacking::File { contents } => {
            write_file_backing(start, length, contents, guest_memory_writer)
        }
    }
}

fn write_file_backing(
    start: u64,
    length: u64,
    contents: &[u8],
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> RiscvGuestMemoryMapResult {
    let bytes = contents
        .len()
        .min(usize::try_from(length).unwrap_or(usize::MAX));
    if bytes > 0 && !guest_memory_writer.write(start, &contents[..bytes]) {
        return RiscvGuestMemoryMapResult::Failed;
    }
    let Some(file_end) = start.checked_add(bytes as u64) else {
        return RiscvGuestMemoryMapResult::Failed;
    };
    write_zeroed_backing(
        file_end,
        length.saturating_sub(bytes as u64),
        guest_memory_writer,
    )
}

fn write_zeroed_backing(
    start: u64,
    length: u64,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> RiscvGuestMemoryMapResult {
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

pub(super) fn syscall_mremap(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let start = request.argument(0);
    let old_size = request.argument(1);
    let new_size = request.argument(2);
    let flags = request.argument(3);

    if !start.is_multiple_of(RISCV_PAGE_BYTES)
        || old_size == 0
        || new_size == 0
        || flags & !RISCV_LINUX_MREMAP_SUPPORTED_FLAGS != 0
        || flags & (RISCV_LINUX_MREMAP_FIXED | RISCV_LINUX_MREMAP_DONTUNMAP) != 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let Some(old_length) = align_to_page(old_size) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let Some(new_length) = align_to_page(new_size) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if start.checked_add(old_length).is_none() || start.checked_add(new_length).is_none() {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let Some(region_index) = exact_mmap_region_index(state, start, old_length) else {
        return linux_error(RISCV_LINUX_ENOMEM);
    };
    if new_length == old_length {
        return start;
    }
    if new_length < old_length {
        state.unmap_mmap_range(start + new_length, old_length - new_length);
        return start;
    }

    let extra_start = start + old_length;
    let extra_length = new_length - old_length;
    if !state.is_mmap_range_available(extra_start, extra_length) {
        return linux_error(RISCV_LINUX_ENOMEM);
    }
    match install_mmap_backing(
        extra_start,
        extra_length,
        guest_memory_writer,
        false,
        &MmapBacking::Anonymous,
    ) {
        RiscvGuestMemoryMapResult::Mapped => {}
        RiscvGuestMemoryMapResult::Overlap => return linux_error(RISCV_LINUX_ENOMEM),
        RiscvGuestMemoryMapResult::Failed => return linux_error(RISCV_LINUX_EFAULT),
    }

    let region = state.mmap_regions[region_index];
    state.mmap_regions[region_index] = RiscvMmapRegion::new(
        region.start(),
        new_length,
        region.protection(),
        region.flags(),
        region.fd(),
        region.offset(),
    );
    start
}

pub(super) fn syscall_mprotect(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let start = request.argument(0);
    let requested_length = request.argument(1);
    let protection = request.argument(2);

    if !start.is_multiple_of(RISCV_PAGE_BYTES) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if requested_length == 0 {
        return 0;
    }
    let Some(length) = align_to_page(requested_length) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if start.checked_add(length).is_none() {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if !mmap_range_is_mapped(state, start, length) {
        return linux_error(RISCV_LINUX_ENOMEM);
    }

    let mut regions = Vec::with_capacity(state.mmap_regions.len() + 2);
    for region in state.mmap_regions.drain(..) {
        region.push_fragments_after_mprotect(start, length, protection, &mut regions);
    }
    state.mmap_regions = regions;
    0
}

pub(super) fn syscall_msync(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    let start = request.argument(0);
    let requested_length = request.argument(1);
    let flags = request.argument(2);

    if !msync_flags_are_valid(flags) || !start.is_multiple_of(RISCV_PAGE_BYTES) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(length) = align_to_page(requested_length) else {
        return linux_error(RISCV_LINUX_ENOMEM);
    };
    if length == 0 {
        return 0;
    }
    if start.checked_add(length).is_none() {
        return linux_error(RISCV_LINUX_ENOMEM);
    }
    if !mmap_range_is_mapped(state, start, length) {
        return linux_error(RISCV_LINUX_ENOMEM);
    }
    0
}

pub(super) fn syscall_memory_lock_range(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
) -> u64 {
    let start = request.argument(0);
    let requested_length = request.argument(1);
    if requested_length == 0 {
        return 0;
    }
    let Some(raw_end) = start.checked_add(requested_length) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let aligned_start = start & !(RISCV_PAGE_BYTES - 1);
    let Some(aligned_end) = align_to_page(raw_end) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let Some(length) = aligned_end.checked_sub(aligned_start) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if !mmap_range_is_mapped(state, aligned_start, length)
        && !brk_backed_range_is_mapped(state, aligned_start, length)
    {
        return linux_error(RISCV_LINUX_ENOMEM);
    }
    0
}

pub(super) fn syscall_madvise(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    let start = request.argument(0);
    let requested_length = request.argument(1);
    let advice = request.argument(2);

    if !madvise_is_known_advice(advice) || !start.is_multiple_of(RISCV_PAGE_BYTES) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(length) = align_to_page(requested_length) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if start.checked_add(length).is_none() {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if length == 0 {
        return 0;
    }
    if !mmap_range_is_mapped(state, start, length) {
        return linux_error(RISCV_LINUX_ENOMEM);
    }
    0
}

pub(super) fn syscall_mincore(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let start = request.argument(0);
    let requested_length = request.argument(1);
    let vector = request.argument(2);

    if !start.is_multiple_of(RISCV_PAGE_BYTES) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(length) = align_to_page(requested_length) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if start.checked_add(length).is_none() {
        return linux_error(RISCV_LINUX_ENOMEM);
    }
    if length == 0 {
        return 0;
    }

    let page_count = length / RISCV_PAGE_BYTES;
    let Some(vector_end) = vector.checked_add(page_count) else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    let Some(guest_memory_writer) = guest_memory_writer else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    if !mmap_range_is_mapped(state, start, length) {
        return linux_error(RISCV_LINUX_ENOMEM);
    }

    let present_page = [1u8; RISCV_PAGE_BYTES as usize];
    let mut cursor = vector;
    while cursor < vector_end {
        let remaining = vector_end - cursor;
        let bytes = remaining.min(RISCV_PAGE_BYTES) as usize;
        if !guest_memory_writer.write(cursor, &present_page[..bytes]) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
        cursor += bytes as u64;
    }
    0
}

fn msync_flags_are_valid(flags: u64) -> bool {
    flags & !RISCV_LINUX_MS_VALID_FLAGS == 0
        && flags & (RISCV_LINUX_MS_ASYNC | RISCV_LINUX_MS_SYNC)
            != (RISCV_LINUX_MS_ASYNC | RISCV_LINUX_MS_SYNC)
}

fn madvise_is_known_advice(advice: u64) -> bool {
    matches!(
        advice,
        RISCV_LINUX_MADV_NORMAL
            | RISCV_LINUX_MADV_RANDOM
            | RISCV_LINUX_MADV_SEQUENTIAL
            | RISCV_LINUX_MADV_WILLNEED
            | RISCV_LINUX_MADV_DONTNEED
            | RISCV_LINUX_MADV_FREE
            | RISCV_LINUX_MADV_REMOVE
            | RISCV_LINUX_MADV_DONTFORK
            | RISCV_LINUX_MADV_DOFORK
            | RISCV_LINUX_MADV_MERGEABLE
            | RISCV_LINUX_MADV_UNMERGEABLE
            | RISCV_LINUX_MADV_HUGEPAGE
            | RISCV_LINUX_MADV_NOHUGEPAGE
            | RISCV_LINUX_MADV_DONTDUMP
            | RISCV_LINUX_MADV_DODUMP
            | RISCV_LINUX_MADV_WIPEONFORK
            | RISCV_LINUX_MADV_KEEPONFORK
            | RISCV_LINUX_MADV_COLD
            | RISCV_LINUX_MADV_PAGEOUT
            | RISCV_LINUX_MADV_POPULATE_READ
            | RISCV_LINUX_MADV_POPULATE_WRITE
            | RISCV_LINUX_MADV_DONTNEED_LOCKED
            | RISCV_LINUX_MADV_COLLAPSE
            | RISCV_LINUX_MADV_GUARD_INSTALL
            | RISCV_LINUX_MADV_GUARD_REMOVE
    )
}

fn exact_mmap_region_index(state: &RiscvSyscallState, start: u64, length: u64) -> Option<usize> {
    state
        .mmap_regions
        .iter()
        .position(|region| region.start() == start && region.length() == length)
}

fn mmap_range_is_mapped(state: &RiscvSyscallState, start: u64, length: u64) -> bool {
    let Some(end) = start.checked_add(length) else {
        return false;
    };
    let mut cursor = start;
    for region in &state.mmap_regions {
        let Some(region_end) = region.start().checked_add(region.length()) else {
            return false;
        };
        if region_end <= cursor {
            continue;
        }
        if region.start() > cursor {
            return false;
        }
        cursor = region_end.min(end);
        if cursor == end {
            return true;
        }
    }
    false
}

fn brk_backed_range_is_mapped(state: &RiscvSyscallState, start: u64, length: u64) -> bool {
    let Some(heap_start) = align_to_page(state.initial_program_break()) else {
        return false;
    };
    let Some(end) = start.checked_add(length) else {
        return false;
    };
    start >= heap_start && end <= state.program_break_backing_end()
}

fn align_to_page(value: u64) -> Option<u64> {
    value
        .checked_add(RISCV_PAGE_BYTES - 1)
        .map(|rounded| rounded & !(RISCV_PAGE_BYTES - 1))
}
