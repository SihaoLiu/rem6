use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryMapResult, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENOMEM, RISCV_LINUX_EPERM,
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
pub(super) const RISCV_LINUX_MBIND: u64 = 235;
pub(super) const RISCV_LINUX_GET_MEMPOLICY: u64 = 236;
pub(super) const RISCV_LINUX_MAP_SHARED: u64 = 0x01;
pub(super) const RISCV_LINUX_MAP_PRIVATE: u64 = 0x02;
pub(super) const RISCV_LINUX_MAP_FIXED: u64 = 0x10;
pub(super) const RISCV_LINUX_MAP_ANONYMOUS: u64 = 0x20;
const RISCV_LINUX_MREMAP_MAYMOVE: u64 = 1;
const RISCV_LINUX_MREMAP_FIXED: u64 = 2;
const RISCV_LINUX_MREMAP_DONTUNMAP: u64 = 4;
const RISCV_LINUX_MREMAP_SUPPORTED_FLAGS: u64 = RISCV_LINUX_MREMAP_MAYMOVE;
const RISCV_LINUX_MCL_CURRENT: u64 = 1;
const RISCV_LINUX_MCL_FUTURE: u64 = 2;
const RISCV_LINUX_MCL_ONFAULT: u64 = 4;
const RISCV_LINUX_MCL_SUPPORTED_FLAGS: u64 =
    RISCV_LINUX_MCL_CURRENT | RISCV_LINUX_MCL_FUTURE | RISCV_LINUX_MCL_ONFAULT;
const RISCV_LINUX_MPOL_DEFAULT: u64 = 0;
const RISCV_LINUX_MPOL_PREFERRED: u64 = 1;
const RISCV_LINUX_MPOL_BIND: u64 = 2;
const RISCV_LINUX_MPOL_INTERLEAVE: u64 = 3;
const RISCV_LINUX_MPOL_LOCAL: u64 = 4;
const RISCV_LINUX_MPOL_PREFERRED_MANY: u64 = 5;
const RISCV_LINUX_MPOL_WEIGHTED_INTERLEAVE: u64 = 6;
const RISCV_LINUX_MPOL_MODE_MASK: u64 = 0x7;
const RISCV_LINUX_MPOL_F_NUMA_BALANCING: u64 = 1 << 13;
const RISCV_LINUX_MPOL_F_RELATIVE_NODES: u64 = 1 << 14;
const RISCV_LINUX_MPOL_F_STATIC_NODES: u64 = 1 << 15;
const RISCV_LINUX_MPOL_MODE_FLAGS: u64 = RISCV_LINUX_MPOL_F_NUMA_BALANCING
    | RISCV_LINUX_MPOL_F_RELATIVE_NODES
    | RISCV_LINUX_MPOL_F_STATIC_NODES;
const RISCV_LINUX_MPOL_SUPPORTED_MODE_FLAGS: u64 =
    RISCV_LINUX_MPOL_F_RELATIVE_NODES | RISCV_LINUX_MPOL_F_STATIC_NODES;
const RISCV_LINUX_GET_MEMPOLICY_SUPPORTED_FLAGS: u64 = 0;
const RISCV_LINUX_MPOL_MF_STRICT: u64 = 1;
const RISCV_LINUX_MPOL_MF_MOVE: u64 = 1 << 1;
const RISCV_LINUX_MPOL_MF_MOVE_ALL: u64 = 1 << 2;
const RISCV_LINUX_MPOL_MF_VALID: u64 =
    RISCV_LINUX_MPOL_MF_STRICT | RISCV_LINUX_MPOL_MF_MOVE | RISCV_LINUX_MPOL_MF_MOVE_ALL;
const RISCV_LINUX_MBIND_MAXNODE_BITS: u64 = RISCV_PAGE_BYTES * 8;
const RISCV_LINUX_NODES_PER_ULONG: u64 = 64;
const RISCV_LINUX_ULONG_BYTES: u64 = 8;
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

#[derive(Clone, Debug)]
pub struct RiscvMmapRegion {
    start: u64,
    length: u64,
    protection: u64,
    flags: u64,
    fd: u64,
    offset: u64,
    backing: RiscvMmapRegionBacking,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RiscvMmapRegionBacking {
    Anonymous,
    File { contents: Vec<u8> },
}

impl PartialEq for RiscvMmapRegion {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start
            && self.length == other.length
            && self.protection == other.protection
            && self.flags == other.flags
            && self.fd == other.fd
            && self.offset == other.offset
    }
}

impl Eq for RiscvMmapRegion {}

impl RiscvMmapRegionBacking {
    fn from_mmap_backing(backing: &MmapBacking) -> Self {
        match backing {
            MmapBacking::Anonymous => Self::Anonymous,
            MmapBacking::File { contents } => Self::File {
                contents: contents.clone(),
            },
        }
    }

    fn fragment(&self, delta: u64, _length: u64) -> Self {
        match self {
            Self::Anonymous => Self::Anonymous,
            // Keep the file tail so a later mremap growth can materialize bytes
            // beyond the fragment's current mapped length.
            Self::File { contents } => Self::File {
                contents: file_backing_slice(contents, delta, u64::MAX).to_vec(),
            },
        }
    }

    fn mmap_backing_fragment(&self, delta: u64, length: u64) -> MmapBacking {
        match self {
            Self::Anonymous => MmapBacking::Anonymous,
            Self::File { contents } => MmapBacking::File {
                contents: file_backing_slice(contents, delta, length).to_vec(),
            },
        }
    }

    fn write_madvise_dontneed(
        &self,
        start: u64,
        delta: u64,
        length: u64,
        guest_memory_writer: &RiscvGuestMemoryWriter,
    ) -> RiscvGuestMemoryMapResult {
        match self {
            Self::Anonymous => write_zeroed_backing(start, length, guest_memory_writer),
            Self::File { contents } => {
                let slice_start = usize::try_from(delta).unwrap_or(usize::MAX);
                let contents = if slice_start >= contents.len() {
                    &[]
                } else {
                    let slice_length = usize::try_from(length).unwrap_or(usize::MAX);
                    let slice_end = slice_start.saturating_add(slice_length).min(contents.len());
                    &contents[slice_start..slice_end]
                };
                write_file_backing(start, length, contents, guest_memory_writer)
            }
        }
    }
}

fn file_backing_slice(contents: &[u8], delta: u64, length: u64) -> &[u8] {
    let start = usize::try_from(delta).unwrap_or(usize::MAX);
    if start >= contents.len() {
        return &[];
    }
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    let end = start.saturating_add(length).min(contents.len());
    &contents[start..end]
}

impl RiscvMmapRegion {
    pub fn new(start: u64, length: u64, protection: u64, flags: u64, fd: u64, offset: u64) -> Self {
        Self::new_with_backing(
            start,
            length,
            protection,
            flags,
            fd,
            offset,
            RiscvMmapRegionBacking::Anonymous,
        )
    }

    fn from_mmap_backing(
        start: u64,
        length: u64,
        protection: u64,
        flags: u64,
        fd: u64,
        offset: u64,
        backing: &MmapBacking,
    ) -> Self {
        Self::new_with_backing(
            start,
            length,
            protection,
            flags,
            fd,
            offset,
            RiscvMmapRegionBacking::from_mmap_backing(backing),
        )
    }

    fn new_with_backing(
        start: u64,
        length: u64,
        protection: u64,
        flags: u64,
        fd: u64,
        offset: u64,
        backing: RiscvMmapRegionBacking,
    ) -> Self {
        Self {
            start,
            length,
            protection,
            flags,
            fd,
            offset,
            backing,
        }
    }

    pub const fn start(&self) -> u64 {
        self.start
    }

    pub const fn length(&self) -> u64 {
        self.length
    }

    pub const fn protection(&self) -> u64 {
        self.protection
    }

    pub const fn flags(&self) -> u64 {
        self.flags
    }

    pub const fn fd(&self) -> u64 {
        self.fd
    }

    pub const fn offset(&self) -> u64 {
        self.offset
    }

    pub(super) fn overlaps(&self, start: u64, length: u64) -> bool {
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
            let left_length = start - self.start;
            output.push(
                Self::new(
                    self.start,
                    left_length,
                    self.protection,
                    self.flags,
                    self.fd,
                    self.offset,
                )
                .with_backing(self.backing.fragment(0, left_length)),
            );
        }
        if end < region_end {
            let delta = end - self.start;
            let right_length = region_end - end;
            output.push(
                Self::new(
                    end,
                    right_length,
                    self.protection,
                    self.flags,
                    self.fd,
                    self.offset.saturating_add(delta),
                )
                .with_backing(self.backing.fragment(delta, right_length)),
            );
        }
    }

    fn with_backing(mut self, backing: RiscvMmapRegionBacking) -> Self {
        self.backing = backing;
        self
    }

    fn with_length(mut self, length: u64) -> Self {
        self.length = length;
        self
    }

    fn write_madvise_dontneed(
        &self,
        start: u64,
        length: u64,
        guest_memory_writer: &RiscvGuestMemoryWriter,
    ) -> RiscvGuestMemoryMapResult {
        let delta = start.saturating_sub(self.start);
        self.backing
            .write_madvise_dontneed(start, delta, length, guest_memory_writer)
    }

    fn backing_fragment(&self, delta: u64, length: u64) -> MmapBacking {
        self.backing.mmap_backing_fragment(delta, length)
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
            let left_length = start - self.start;
            output.push(
                Self::new(
                    self.start,
                    left_length,
                    self.protection,
                    self.flags,
                    self.fd,
                    self.offset,
                )
                .with_backing(self.backing.fragment(0, left_length)),
            );
        }

        let protect_start = self.start.max(start);
        let protect_end = region_end.min(end);
        let protect_delta = protect_start - self.start;
        let protect_length = protect_end - protect_start;
        output.push(
            Self::new(
                protect_start,
                protect_length,
                protection,
                self.flags,
                self.fd,
                self.offset.saturating_add(protect_delta),
            )
            .with_backing(self.backing.fragment(protect_delta, protect_length)),
        );

        if end < region_end {
            let delta = end - self.start;
            let right_length = region_end - end;
            output.push(
                Self::new(
                    end,
                    right_length,
                    self.protection,
                    self.flags,
                    self.fd,
                    self.offset.saturating_add(delta),
                )
                .with_backing(self.backing.fragment(delta, right_length)),
            );
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
    let backing = match mmap_backing(flags, fd, offset, state) {
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
                    state.push_mmap_region(RiscvMmapRegion::from_mmap_backing(
                        start, length, protection, flags, fd, offset, &backing,
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
    state.push_mmap_region(RiscvMmapRegion::from_mmap_backing(
        mapped_start,
        length,
        protection,
        flags,
        fd,
        offset,
        &backing,
    ));
    mapped_start
}

fn mmap_backing(
    flags: u64,
    fd: u64,
    offset: u64,
    state: &RiscvSyscallState,
) -> Option<MmapBacking> {
    if flags & RISCV_LINUX_MAP_ANONYMOUS != 0 {
        return Some(MmapBacking::Anonymous);
    }

    let fd = guest_fd_argument(fd)?;
    let contents = state.guest_file_slice_at(fd, offset, usize::MAX).ok()??;
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
    let region = state.mmap_regions[region_index].clone();
    let extra_backing = region.backing_fragment(old_length, extra_length);
    match install_mmap_backing(
        extra_start,
        extra_length,
        guest_memory_writer,
        false,
        &extra_backing,
    ) {
        RiscvGuestMemoryMapResult::Mapped => {}
        RiscvGuestMemoryMapResult::Overlap => return linux_error(RISCV_LINUX_ENOMEM),
        RiscvGuestMemoryMapResult::Failed => return linux_error(RISCV_LINUX_EFAULT),
    }

    state.mmap_regions[region_index] = region.with_length(new_length);
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

pub(super) fn syscall_mlockall(flags: u64) -> u64 {
    if !mlockall_flags_are_valid(flags) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    0
}

pub(super) const fn syscall_munlockall() -> u64 {
    0
}

pub(super) fn syscall_mbind(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> u64 {
    let start = request.argument(0);
    let requested_length = request.argument(1);
    let raw_mode = request.argument(2);
    let nodemask_address = request.argument(3);
    let maxnode = request.argument(4);
    let flags = request.argument(5);

    let Some(mode) = parse_mbind_mode(raw_mode) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let nodemask = match read_mbind_nodemask(nodemask_address, maxnode, guest_memory_reader) {
        Ok(nodemask) => nodemask,
        Err(errno) => return linux_error(errno),
    };
    let Some(flags) = parse_mbind_flags(flags) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let policy = RiscvMbindPolicy { mode, flags };
    if policy.requires_privilege() {
        return linux_error(RISCV_LINUX_EPERM);
    }
    if !mbind_policy_accepts_nodemask(policy.mode, nodemask) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if !start.is_multiple_of(RISCV_PAGE_BYTES) {
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
    let range_is_mapped = if policy.mode == RISCV_LINUX_MPOL_DEFAULT {
        mbind_default_range_has_mapping(state, start, length)
    } else {
        mmap_range_is_mapped(state, start, length)
            || brk_backed_range_is_mapped(state, start, length)
    };
    if !range_is_mapped {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    0
}

pub(super) fn syscall_get_mempolicy(
    request: RiscvSyscallRequest,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let mode_address = request.argument(0);
    let nodemask_address = request.argument(1);
    let maxnode = request.argument(2);
    let flags = request.argument(4);
    if flags & !RISCV_LINUX_GET_MEMPOLICY_SUPPORTED_FLAGS != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if mode_address == 0 && nodemask_address == 0 {
        return Some(0);
    }
    let nodemask_bytes = if nodemask_address == 0 {
        None
    } else {
        if maxnode == 0 {
            return Some(linux_error(RISCV_LINUX_EINVAL));
        }
        if maxnode > RISCV_LINUX_MBIND_MAXNODE_BITS {
            return Some(linux_error(RISCV_LINUX_EINVAL));
        }
        let bytes = match mbind_nodemask_bytes(maxnode) {
            Ok(bytes) => bytes,
            Err(errno) => return Some(linux_error(errno)),
        };
        Some(bytes)
    };

    let guest_memory = guest_memory_writer?;
    if mode_address != 0 {
        let mode = RISCV_LINUX_MPOL_DEFAULT as i32;
        if !guest_memory.write(mode_address, &mode.to_le_bytes()) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }
    if let Some(bytes) = nodemask_bytes {
        if !guest_memory.write(nodemask_address, &vec![0; bytes]) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }
    Some(0)
}

fn mlockall_flags_are_valid(flags: u64) -> bool {
    let flags = u64::from(flags as u32);
    if flags & !RISCV_LINUX_MCL_SUPPORTED_FLAGS != 0 {
        return false;
    }
    let scoped = flags & (RISCV_LINUX_MCL_CURRENT | RISCV_LINUX_MCL_FUTURE);
    scoped != 0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvMbindPolicy {
    mode: u64,
    flags: u64,
}

impl RiscvMbindPolicy {
    const fn requires_privilege(self) -> bool {
        self.flags & RISCV_LINUX_MPOL_MF_MOVE_ALL != 0
    }
}

fn parse_mbind_mode(raw_mode: u64) -> Option<u64> {
    let raw_mode = u64::from(raw_mode as u32);
    let mode_flags = raw_mode & RISCV_LINUX_MPOL_MODE_FLAGS;
    if raw_mode & !(RISCV_LINUX_MPOL_MODE_MASK | RISCV_LINUX_MPOL_MODE_FLAGS) != 0 {
        return None;
    }
    if mode_flags & RISCV_LINUX_MPOL_F_STATIC_NODES != 0
        && mode_flags & RISCV_LINUX_MPOL_F_RELATIVE_NODES != 0
    {
        return None;
    }
    if mode_flags & !RISCV_LINUX_MPOL_SUPPORTED_MODE_FLAGS != 0 {
        return None;
    }
    let mode = raw_mode & RISCV_LINUX_MPOL_MODE_MASK;
    if !mbind_mode_is_known(mode) {
        return None;
    }
    Some(mode)
}

fn parse_mbind_flags(flags: u64) -> Option<u64> {
    let flags = u64::from(flags as u32);
    (flags & !RISCV_LINUX_MPOL_MF_VALID == 0).then_some(flags)
}

const fn mbind_mode_is_known(mode: u64) -> bool {
    matches!(
        mode,
        RISCV_LINUX_MPOL_DEFAULT
            | RISCV_LINUX_MPOL_PREFERRED
            | RISCV_LINUX_MPOL_BIND
            | RISCV_LINUX_MPOL_INTERLEAVE
            | RISCV_LINUX_MPOL_LOCAL
            | RISCV_LINUX_MPOL_PREFERRED_MANY
            | RISCV_LINUX_MPOL_WEIGHTED_INTERLEAVE
    )
}

fn mbind_policy_accepts_nodemask(mode: u64, nodemask: RiscvMbindNodemask) -> bool {
    match mode {
        RISCV_LINUX_MPOL_DEFAULT | RISCV_LINUX_MPOL_LOCAL => nodemask.is_empty(),
        RISCV_LINUX_MPOL_PREFERRED => true,
        RISCV_LINUX_MPOL_BIND
        | RISCV_LINUX_MPOL_INTERLEAVE
        | RISCV_LINUX_MPOL_PREFERRED_MANY
        | RISCV_LINUX_MPOL_WEIGHTED_INTERLEAVE => !nodemask.is_empty(),
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvMbindNodemask {
    node_zero: bool,
}

impl RiscvMbindNodemask {
    const EMPTY: Self = Self { node_zero: false };

    const fn is_empty(self) -> bool {
        !self.node_zero
    }
}

fn read_mbind_nodemask(
    address: u64,
    maxnode: u64,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Result<RiscvMbindNodemask, u64> {
    if address == 0 {
        return Ok(RiscvMbindNodemask::EMPTY);
    }
    let Some(node_bits) = maxnode.checked_sub(1) else {
        return Err(RISCV_LINUX_EINVAL);
    };
    if node_bits > RISCV_LINUX_MBIND_MAXNODE_BITS {
        return Err(RISCV_LINUX_EINVAL);
    }
    if node_bits == 0 {
        return Ok(RiscvMbindNodemask::EMPTY);
    }
    let requested_bytes = mbind_nodemask_bytes(node_bits)?;
    let Some(guest_memory_reader) = guest_memory_reader else {
        return Err(RISCV_LINUX_EFAULT);
    };
    let bytes = guest_memory_reader
        .read(address, requested_bytes)
        .ok_or(RISCV_LINUX_EFAULT)?;
    if bytes.len() != requested_bytes {
        return Err(RISCV_LINUX_EFAULT);
    }
    let node_zero = bytes[0] & 1 != 0;
    if nodemask_has_unsupported_nodes(&bytes, node_bits) {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(RiscvMbindNodemask { node_zero })
}

fn mbind_nodemask_bytes(maxnode: u64) -> Result<usize, u64> {
    let words = maxnode
        .checked_add(RISCV_LINUX_NODES_PER_ULONG - 1)
        .ok_or(RISCV_LINUX_EINVAL)?
        / RISCV_LINUX_NODES_PER_ULONG;
    let bytes = words
        .checked_mul(RISCV_LINUX_ULONG_BYTES)
        .ok_or(RISCV_LINUX_EINVAL)?;
    usize::try_from(bytes).map_err(|_| RISCV_LINUX_EINVAL)
}

fn nodemask_has_unsupported_nodes(bytes: &[u8], maxnode: u64) -> bool {
    for bit in 1..maxnode {
        let byte_index = (bit / 8) as usize;
        let bit_index = (bit % 8) as u8;
        if bytes
            .get(byte_index)
            .is_some_and(|byte| byte & (1_u8 << bit_index) != 0)
        {
            return true;
        }
    }
    false
}

fn mbind_default_range_has_mapping(state: &RiscvSyscallState, start: u64, length: u64) -> bool {
    state
        .mmap_regions
        .iter()
        .any(|region| region.overlaps(start, length))
        || brk_backed_range_overlaps_mapping(state, start, length)
}

pub(super) fn syscall_madvise(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
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
    if advice == RISCV_LINUX_MADV_DONTNEED {
        let Some(guest_memory_writer) = guest_memory_writer else {
            return 0;
        };
        if write_madvise_dontneed_backing(state, start, length, guest_memory_writer)
            != RiscvGuestMemoryMapResult::Mapped
        {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    0
}

fn write_madvise_dontneed_backing(
    state: &RiscvSyscallState,
    start: u64,
    length: u64,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> RiscvGuestMemoryMapResult {
    let Some(end) = start.checked_add(length) else {
        return RiscvGuestMemoryMapResult::Failed;
    };
    let mut cursor = start;
    for region in &state.mmap_regions {
        if cursor >= end {
            return RiscvGuestMemoryMapResult::Mapped;
        }
        let Some(region_end) = region.start().checked_add(region.length()) else {
            return RiscvGuestMemoryMapResult::Failed;
        };
        if region_end <= cursor {
            continue;
        }
        if region.start() > cursor {
            return RiscvGuestMemoryMapResult::Failed;
        }

        let segment_start = cursor.max(region.start());
        let segment_end = end.min(region_end);
        let segment_length = segment_end - segment_start;
        match region.write_madvise_dontneed(segment_start, segment_length, guest_memory_writer) {
            RiscvGuestMemoryMapResult::Mapped => {
                cursor = segment_end;
            }
            result => return result,
        }
    }
    if cursor == end {
        RiscvGuestMemoryMapResult::Mapped
    } else {
        RiscvGuestMemoryMapResult::Failed
    }
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

fn brk_backed_range_overlaps_mapping(state: &RiscvSyscallState, start: u64, length: u64) -> bool {
    let Some(heap_start) = align_to_page(state.initial_program_break()) else {
        return false;
    };
    let Some(end) = start.checked_add(length) else {
        return false;
    };
    start < state.program_break_backing_end() && heap_start < end
}

fn align_to_page(value: u64) -> Option<u64> {
    value
        .checked_add(RISCV_PAGE_BYTES - 1)
        .map(|rounded| rounded & !(RISCV_PAGE_BYTES - 1))
}
