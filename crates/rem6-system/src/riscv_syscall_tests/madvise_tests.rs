use super::*;

const RISCV_LINUX_ENOMEM_FOR_TEST: u64 = 12;
const RISCV_LINUX_MADVISE_FOR_TEST: u64 = 233;
const RISCV_LINUX_MPROTECT_FOR_TEST: u64 = 226;
const RISCV_LINUX_MREMAP_FOR_TEST: u64 = 216;
const RISCV_LINUX_MUNMAP_FOR_TEST: u64 = 215;
const RISCV_LINUX_MADV_NORMAL_FOR_TEST: u64 = 0;
const RISCV_LINUX_MADV_RANDOM_FOR_TEST: u64 = 1;
const RISCV_LINUX_MADV_SEQUENTIAL_FOR_TEST: u64 = 2;
const RISCV_LINUX_MADV_WILLNEED_FOR_TEST: u64 = 3;
const RISCV_LINUX_MADV_DONTNEED_FOR_TEST: u64 = 4;
const RISCV_LINUX_MADV_FREE_FOR_TEST: u64 = 8;
const RISCV_LINUX_MADV_REMOVE_FOR_TEST: u64 = 9;
const RISCV_LINUX_MADV_DONTFORK_FOR_TEST: u64 = 10;
const RISCV_LINUX_MADV_DOFORK_FOR_TEST: u64 = 11;
const RISCV_LINUX_MADV_MERGEABLE_FOR_TEST: u64 = 12;
const RISCV_LINUX_MADV_UNMERGEABLE_FOR_TEST: u64 = 13;
const RISCV_LINUX_MADV_HUGEPAGE_FOR_TEST: u64 = 14;
const RISCV_LINUX_MADV_NOHUGEPAGE_FOR_TEST: u64 = 15;
const RISCV_LINUX_MADV_DONTDUMP_FOR_TEST: u64 = 16;
const RISCV_LINUX_MADV_DODUMP_FOR_TEST: u64 = 17;
const RISCV_LINUX_MADV_WIPEONFORK_FOR_TEST: u64 = 18;
const RISCV_LINUX_MADV_KEEPONFORK_FOR_TEST: u64 = 19;
const RISCV_LINUX_MADV_COLD_FOR_TEST: u64 = 20;
const RISCV_LINUX_MADV_PAGEOUT_FOR_TEST: u64 = 21;
const RISCV_LINUX_MADV_POPULATE_READ_FOR_TEST: u64 = 22;
const RISCV_LINUX_MADV_POPULATE_WRITE_FOR_TEST: u64 = 23;
const RISCV_LINUX_MADV_DONTNEED_LOCKED_FOR_TEST: u64 = 24;
const RISCV_LINUX_MADV_COLLAPSE_FOR_TEST: u64 = 25;
const RISCV_LINUX_MADV_GUARD_INSTALL_FOR_TEST: u64 = 102;
const RISCV_LINUX_MADV_GUARD_REMOVE_FOR_TEST: u64 = 103;
const RISCV_LINUX_MADV_SUPPORTED_FOR_TEST: [u64; 25] = [
    RISCV_LINUX_MADV_NORMAL_FOR_TEST,
    RISCV_LINUX_MADV_RANDOM_FOR_TEST,
    RISCV_LINUX_MADV_SEQUENTIAL_FOR_TEST,
    RISCV_LINUX_MADV_WILLNEED_FOR_TEST,
    RISCV_LINUX_MADV_DONTNEED_FOR_TEST,
    RISCV_LINUX_MADV_FREE_FOR_TEST,
    RISCV_LINUX_MADV_REMOVE_FOR_TEST,
    RISCV_LINUX_MADV_DONTFORK_FOR_TEST,
    RISCV_LINUX_MADV_DOFORK_FOR_TEST,
    RISCV_LINUX_MADV_MERGEABLE_FOR_TEST,
    RISCV_LINUX_MADV_UNMERGEABLE_FOR_TEST,
    RISCV_LINUX_MADV_HUGEPAGE_FOR_TEST,
    RISCV_LINUX_MADV_NOHUGEPAGE_FOR_TEST,
    RISCV_LINUX_MADV_DONTDUMP_FOR_TEST,
    RISCV_LINUX_MADV_DODUMP_FOR_TEST,
    RISCV_LINUX_MADV_WIPEONFORK_FOR_TEST,
    RISCV_LINUX_MADV_KEEPONFORK_FOR_TEST,
    RISCV_LINUX_MADV_COLD_FOR_TEST,
    RISCV_LINUX_MADV_PAGEOUT_FOR_TEST,
    RISCV_LINUX_MADV_POPULATE_READ_FOR_TEST,
    RISCV_LINUX_MADV_POPULATE_WRITE_FOR_TEST,
    RISCV_LINUX_MADV_DONTNEED_LOCKED_FOR_TEST,
    RISCV_LINUX_MADV_COLLAPSE_FOR_TEST,
    RISCV_LINUX_MADV_GUARD_INSTALL_FOR_TEST,
    RISCV_LINUX_MADV_GUARD_REMOVE_FOR_TEST,
];
const RISCV_LINUX_MADV_UNSUPPORTED_FOR_TEST: [u64; 6] = [5, 7, 26, 99, 104, 999];

#[test]
fn linux_table_madvise_accepts_supported_advice_for_tracked_mappings() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 2 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );

    for advice in RISCV_LINUX_MADV_SUPPORTED_FOR_TEST {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    RISCV_LINUX_MADVISE_FOR_TEST,
                    [
                        RISCV64_LINUX_MMAP_BASE,
                        RISCV_PAGE_BYTES + 11,
                        advice,
                        0,
                        0,
                        0
                    ]
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_madvise_dontneed_zeroes_tracked_mmap_pages() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 2 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES + 64,
                    RISCV_LINUX_MADV_DONTNEED_FOR_TEST,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 2);
    let zeroed = collect_guest_writes(
        &writes,
        RISCV64_LINUX_MMAP_BASE,
        (2 * RISCV_PAGE_BYTES) as usize,
    );
    assert!(zeroed.iter().all(|byte| *byte == 0));
}

#[test]
fn linux_table_madvise_dontneed_restores_file_backed_pages_after_fd_close() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"file-backed");
    let path = b"/input.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, RISCV_LINUX_MAP_PRIVATE, 3, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MADV_DONTNEED_FOR_TEST,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    let restored =
        collect_guest_writes(&writes, RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES as usize);
    assert_eq!(&restored[..11], b"file-backed");
    assert!(restored[11..].iter().all(|byte| *byte == 0));
}

#[test]
fn linux_table_madvise_dontneed_restores_file_backed_tail_after_mremap_growth() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(
        b"/input.txt",
        page_labeled_file_contents(&[b"first-page", b"second-page"]),
    );
    let path = b"/input.txt\0".to_vec();
    let guest_memory_reader = path_reader(path, 0x9000);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_writer(&writes);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, RISCV_LINUX_MAP_PRIVATE, 3, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MREMAP_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    2 * RISCV_PAGE_BYTES,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    let writes_after_mremap = writes.lock().unwrap().clone();
    let expanded = collect_guest_writes(
        &writes_after_mremap,
        RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
        RISCV_PAGE_BYTES as usize,
    );
    assert_eq!(&expanded[..11], b"second-page");
    assert!(expanded[11..].iter().all(|byte| *byte == 0));
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MADV_DONTNEED_FOR_TEST,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let writes = writes.lock().unwrap();
    let restored = collect_guest_writes(
        &writes,
        RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
        RISCV_PAGE_BYTES as usize,
    );
    assert_eq!(&restored[..11], b"second-page");
    assert!(restored[11..].iter().all(|byte| *byte == 0));
}

#[test]
fn linux_table_madvise_dontneed_restores_file_backed_tail_after_fragment_mremap_growth() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(
        b"/input.txt",
        page_labeled_file_contents(&[b"first-page", b"second-page", b"third-page"]),
    );
    let path = b"/input.txt\0".to_vec();
    let guest_memory_reader = path_reader(path, 0x9000);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_writer(&writes);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [0, 2 * RISCV_PAGE_BYTES, 3, RISCV_LINUX_MAP_PRIVATE, 3, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MUNMAP_FOR_TEST,
                [RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES, 0, 0, 0, 0]
            ),
            &mut state
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MREMAP_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    2 * RISCV_PAGE_BYTES,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES
        })
    );
    let writes_after_mremap = writes.lock().unwrap().clone();
    let expanded = collect_guest_writes(
        &writes_after_mremap,
        RISCV64_LINUX_MMAP_BASE + 2 * RISCV_PAGE_BYTES,
        RISCV_PAGE_BYTES as usize,
    );
    assert_eq!(&expanded[..10], b"third-page");
    assert!(expanded[10..].iter().all(|byte| *byte == 0));
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE + 2 * RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MADV_DONTNEED_FOR_TEST,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let writes = writes.lock().unwrap();
    let restored = collect_guest_writes(
        &writes,
        RISCV64_LINUX_MMAP_BASE + 2 * RISCV_PAGE_BYTES,
        RISCV_PAGE_BYTES as usize,
    );
    assert_eq!(&restored[..10], b"third-page");
    assert!(restored[10..].iter().all(|byte| *byte == 0));
}

#[test]
fn linux_table_madvise_dontneed_restores_file_backed_page_after_munmap_fragment() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(
        b"/input.txt",
        page_labeled_file_contents(&[b"first-page", b"second-page"]),
    );
    let path = b"/input.txt\0".to_vec();
    let guest_memory_reader = path_reader(path, 0x9000);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_writer(&writes);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [0, 2 * RISCV_PAGE_BYTES, 3, RISCV_LINUX_MAP_PRIVATE, 3, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MUNMAP_FOR_TEST,
                [RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES, 0, 0, 0, 0]
            ),
            &mut state
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MADV_DONTNEED_FOR_TEST,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let writes = writes.lock().unwrap();
    let restored = collect_guest_writes(
        &writes,
        RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
        RISCV_PAGE_BYTES as usize,
    );
    assert_eq!(&restored[..11], b"second-page");
    assert!(restored[11..].iter().all(|byte| *byte == 0));
}

#[test]
fn linux_table_madvise_dontneed_restores_file_backed_page_after_mprotect_fragment() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(
        b"/input.txt",
        page_labeled_file_contents(&[b"first-page", b"middle-page", b"third-page"]),
    );
    let path = b"/input.txt\0".to_vec();
    let guest_memory_reader = path_reader(path, 0x9000);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_writer(&writes);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [0, 3 * RISCV_PAGE_BYTES, 3, RISCV_LINUX_MAP_PRIVATE, 3, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MPROTECT_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    1,
                    0,
                    0,
                    0
                ]
            ),
            &mut state
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MADV_DONTNEED_FOR_TEST,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let writes = writes.lock().unwrap();
    let restored = collect_guest_writes(
        &writes,
        RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
        RISCV_PAGE_BYTES as usize,
    );
    assert_eq!(&restored[..11], b"middle-page");
    assert!(restored[11..].iter().all(|byte| *byte == 0));
}

#[test]
fn linux_table_madvise_accepts_zero_length_supported_advice_probe() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [0, 0, RISCV_LINUX_MADV_WILLNEED_FOR_TEST, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

fn page_labeled_file_contents(labels: &[&[u8]]) -> Vec<u8> {
    let mut contents = vec![0; labels.len() * RISCV_PAGE_BYTES as usize];
    for (page, label) in labels.iter().enumerate() {
        let start = page * RISCV_PAGE_BYTES as usize;
        contents[start..start + label.len()].copy_from_slice(label);
    }
    contents
}

fn path_reader(path: Vec<u8>, base: u64) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < base {
            return None;
        }
        path.get((address - base) as usize)
            .copied()
            .map(|byte| vec![byte])
    })
}

fn recording_writer(
    writes: &std::sync::Arc<std::sync::Mutex<Vec<(u64, Vec<u8>)>>>,
) -> RiscvGuestMemoryWriter {
    let writes_for_writer = std::sync::Arc::clone(writes);
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    })
}

#[test]
fn linux_table_madvise_rejects_unmapped_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MADV_NORMAL_FOR_TEST,
                    0,
                    0,
                    0
                ]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOMEM_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_madvise_rejects_invalid_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE + 1,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MADV_NORMAL_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );

    for advice in RISCV_LINUX_MADV_UNSUPPORTED_FOR_TEST {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_MADVISE_FOR_TEST,
                    [0, 0, advice, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
}

#[test]
fn linux_table_madvise_reports_einval_for_overflowing_address_range() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MADVISE_FOR_TEST,
                [
                    u64::MAX - (RISCV_PAGE_BYTES - 1),
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MADV_NORMAL_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}
