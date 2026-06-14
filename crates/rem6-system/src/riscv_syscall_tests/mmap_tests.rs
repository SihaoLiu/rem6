use super::*;

const RISCV_LINUX_ENOMEM_FOR_TEST: u64 = 12;
const RISCV_LINUX_MREMAP_FOR_TEST: u64 = 216;
const RISCV_LINUX_MREMAP_MAYMOVE_FOR_TEST: u64 = 1;
const RISCV_LINUX_MREMAP_FIXED_FOR_TEST: u64 = 2;
const RISCV_LINUX_MREMAP_DONTUNMAP_FOR_TEST: u64 = 4;

#[test]
fn linux_table_allocates_anonymous_mmap_regions() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_MMAP, [0, 64, 3, 34, u64::MAX, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            3,
            34,
            u64::MAX,
            0,
        )]
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 1, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES
        })
    );
    assert_eq!(
        state.mmap_next(),
        RISCV64_LINUX_MMAP_BASE + (2 * RISCV_PAGE_BYTES)
    );
}

#[test]
fn linux_table_zeroes_anonymous_mmap_backing_with_guest_writer() {
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
                [0, RISCV_PAGE_BYTES + 17, 3, 34, u64::MAX, 0,]
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

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].0, RISCV64_LINUX_MMAP_BASE);
    assert_eq!(writes[1].0, RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES);
    assert_eq!(writes[0].1.len(), RISCV_PAGE_BYTES as usize);
    assert_eq!(writes[1].1.len(), RISCV_PAGE_BYTES as usize);
    assert!(writes[0].1.iter().all(|byte| *byte == 0));
    assert!(writes[1].1.iter().all(|byte| *byte == 0));
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            2 * RISCV_PAGE_BYTES,
            3,
            34,
            u64::MAX,
            0,
        )]
    );
}

#[test]
fn linux_table_maps_registered_guest_file_contents_by_open_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"hello");
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
                [0, RISCV_PAGE_BYTES + 3, 3, RISCV_LINUX_MAP_PRIVATE, 3, 0,],
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

    let writes = writes.lock().unwrap();
    let mapped = collect_guest_writes(
        &writes,
        RISCV64_LINUX_MMAP_BASE,
        (2 * RISCV_PAGE_BYTES) as usize,
    );
    assert_eq!(&mapped[..5], b"hello");
    assert!(mapped[5..].iter().all(|byte| *byte == 0));
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            2 * RISCV_PAGE_BYTES,
            3,
            RISCV_LINUX_MAP_PRIVATE,
            3,
            0,
        )]
    );
}

#[test]
fn linux_table_maps_registered_guest_file_from_page_offset_without_advancing_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let mut contents = vec![b'A'; RISCV_PAGE_BYTES as usize];
    contents.extend_from_slice(b"tail");
    state.register_guest_file(b"/input.txt", &contents);
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
                [
                    0,
                    RISCV_PAGE_BYTES,
                    3,
                    RISCV_LINUX_MAP_PRIVATE,
                    3,
                    RISCV_PAGE_BYTES,
                ],
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
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [3, 0x9100, 1, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );

    let writes = writes.lock().unwrap();
    let mmap_writes = writes
        .iter()
        .filter(|(address, _bytes)| *address >= RISCV64_LINUX_MMAP_BASE)
        .cloned()
        .collect::<Vec<_>>();
    let mapped = collect_guest_writes(
        &mmap_writes,
        RISCV64_LINUX_MMAP_BASE,
        RISCV_PAGE_BYTES as usize,
    );
    assert_eq!(&mapped[..4], b"tail");
    assert!(mapped[4..].iter().all(|byte| *byte == 0));
    assert_eq!(&writes.last().unwrap().1, b"A");
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            3,
            RISCV_LINUX_MAP_PRIVATE,
            3,
            RISCV_PAGE_BYTES,
        )]
    );
}

#[test]
fn linux_table_maps_registered_guest_file_after_dup_and_close_original_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"dup-data");
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
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_DUP, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, RISCV_LINUX_MAP_PRIVATE, 4, 0],
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

    let writes = writes.lock().unwrap();
    let mapped = collect_guest_writes(&writes, RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES as usize);
    assert_eq!(&mapped[..8], b"dup-data");
    assert!(mapped[8..].iter().all(|byte| *byte == 0));
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            3,
            RISCV_LINUX_MAP_PRIVATE,
            4,
            0,
        )]
    );
}

#[test]
fn linux_table_fixed_mmap_replaces_anonymous_backing_with_registered_guest_file_contents() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"fixed");
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
    })
    .with_region_map_handler(move |request| {
        if request.replace_existing() {
            RiscvGuestMemoryMapResult::Overlap
        } else {
            RiscvGuestMemoryMapResult::Mapped
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0],
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
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
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
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MMAP,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    3,
                    RISCV_LINUX_MAP_PRIVATE | RISCV_LINUX_MAP_FIXED,
                    3,
                    0,
                ],
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

    let writes = writes.lock().unwrap();
    let mapped = collect_guest_writes(&writes, RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES as usize);
    assert_eq!(&mapped[..5], b"fixed");
    assert!(mapped[5..].iter().all(|byte| *byte == 0));
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            3,
            RISCV_LINUX_MAP_PRIVATE | RISCV_LINUX_MAP_FIXED,
            3,
            0,
        )]
    );
}

#[test]
fn linux_table_rejects_file_backed_mmap_without_registered_guest_file_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, RISCV_LINUX_MAP_PRIVATE, 1, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, RISCV_LINUX_MAP_PRIVATE, 42, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.mmap_regions().is_empty());
}

#[test]
fn linux_table_reports_efault_when_mmap_backing_install_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let initial_mmap_next = state.mmap_next();
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0,]
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.mmap_regions().is_empty());
    assert_eq!(state.mmap_next(), initial_mmap_next);
}

#[test]
fn linux_table_retries_nonfixed_mmap_hint_when_backing_overlaps() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let hint = 0x8000_0000;
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.len()));
        true
    })
    .with_region_map_handler(move |request| {
        if request.address() == hint {
            RiscvGuestMemoryMapResult::Overlap
        } else {
            RiscvGuestMemoryMapResult::Mapped
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [hint, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0,]
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
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            3,
            34,
            u64::MAX,
            0,
        )]
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES as usize)]
    );
}

#[test]
fn linux_table_fixed_mmap_allows_backing_overlap_for_replacement() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true)
        .with_region_map_handler(move |request| {
            if request.replace_existing() {
                RiscvGuestMemoryMapResult::Overlap
            } else {
                RiscvGuestMemoryMapResult::Mapped
            }
        });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0,]
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
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    1,
                    34 | RISCV_LINUX_MAP_FIXED,
                    u64::MAX,
                    0,
                ]
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
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            1,
            34 | RISCV_LINUX_MAP_FIXED,
            u64::MAX,
            0,
        )]
    );
}

#[test]
fn linux_table_rejects_invalid_mmap_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_MMAP, [0, 0, 3, 34, u64::MAX, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MMAP, [1, 64, 3, 34, u64::MAX, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.mmap_regions().is_empty());
}

#[test]
fn linux_table_fixed_mmap_preserves_non_overlapping_fragments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let fixed_start = RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES;

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 3 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [
                    fixed_start,
                    RISCV_PAGE_BYTES,
                    1,
                    34 | RISCV_LINUX_MAP_FIXED,
                    u64::MAX,
                    0,
                ]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: fixed_start })
    );
    assert_eq!(
        state.mmap_regions(),
        &[
            RiscvMmapRegion::new(
                RISCV64_LINUX_MMAP_BASE,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                0,
            ),
            RiscvMmapRegion::new(
                fixed_start,
                RISCV_PAGE_BYTES,
                1,
                34 | RISCV_LINUX_MAP_FIXED,
                u64::MAX,
                0,
            ),
            RiscvMmapRegion::new(
                fixed_start + RISCV_PAGE_BYTES,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                2 * RISCV_PAGE_BYTES,
            ),
        ]
    );
}

#[test]
fn linux_table_munmap_removes_mapped_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let unmap_start = RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES;

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 3 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MUNMAP,
                [unmap_start, RISCV_PAGE_BYTES, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        state.mmap_regions(),
        &[
            RiscvMmapRegion::new(
                RISCV64_LINUX_MMAP_BASE,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                0,
            ),
            RiscvMmapRegion::new(
                unmap_start + RISCV_PAGE_BYTES,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                2 * RISCV_PAGE_BYTES,
            ),
        ]
    );
}

#[test]
fn linux_table_mremap_shrinks_mapped_region_in_place() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 3 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MREMAP_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    3 * RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            3,
            34,
            u64::MAX,
            0,
        )]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_mremap_expands_mapped_region_in_place_and_zeroes_tail() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let map_requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let map_requests_for_writer = std::sync::Arc::clone(&map_requests);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    })
    .with_region_map_handler(move |request| {
        map_requests_for_writer.lock().unwrap().push(request);
        RiscvGuestMemoryMapResult::Mapped
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
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
    map_requests.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MREMAP_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    2 * RISCV_PAGE_BYTES,
                    RISCV_LINUX_MREMAP_MAYMOVE_FOR_TEST,
                    0,
                    0,
                ],
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
    assert_eq!(
        state.mmap_regions(),
        &[RiscvMmapRegion::new(
            RISCV64_LINUX_MMAP_BASE,
            2 * RISCV_PAGE_BYTES,
            3,
            34,
            u64::MAX,
            0,
        )]
    );
    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES);
    assert_eq!(writes[0].1.len(), RISCV_PAGE_BYTES as usize);
    assert!(writes[0].1.iter().all(|byte| *byte == 0));
    assert_eq!(
        map_requests.lock().unwrap().as_slice(),
        &[RiscvGuestMemoryMapRequest::new(
            RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
            RISCV_PAGE_BYTES,
            false,
        )]
    );
}

#[test]
fn linux_table_mremap_rejects_blocked_in_place_growth_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [
                    RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    1,
                    34 | RISCV_LINUX_MAP_FIXED,
                    u64::MAX,
                    0,
                ]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES
        })
    );
    let mapped_regions = state.mmap_regions().to_vec();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MREMAP_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    2 * RISCV_PAGE_BYTES,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOMEM_FOR_TEST)
        })
    );
    assert_eq!(state.mmap_regions(), mapped_regions.as_slice());
}

#[test]
fn linux_table_mremap_rejects_invalid_arguments_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    let mapped_regions = state.mmap_regions().to_vec();

    for arguments in [
        [
            RISCV64_LINUX_MMAP_BASE + 1,
            RISCV_PAGE_BYTES,
            RISCV_PAGE_BYTES,
            0,
            0,
            0,
        ],
        [RISCV64_LINUX_MMAP_BASE, 0, RISCV_PAGE_BYTES, 0, 0, 0],
        [RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES, 0, 0, 0, 0],
        [RISCV64_LINUX_MMAP_BASE, u64::MAX, RISCV_PAGE_BYTES, 0, 0, 0],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MREMAP_FIXED_FOR_TEST,
            RISCV64_LINUX_MMAP_BASE + 4 * RISCV_PAGE_BYTES,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MREMAP_DONTUNMAP_FOR_TEST,
            0,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_PAGE_BYTES,
            1 << 8,
            0,
            0,
        ],
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MREMAP_FOR_TEST, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert_eq!(state.mmap_regions(), mapped_regions.as_slice());
    }
}

#[test]
fn linux_table_mprotect_splits_regions_and_updates_protection() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let protect_start = RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES;

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 3 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MPROTECT,
                [protect_start, RISCV_PAGE_BYTES, 1, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        state.mmap_regions(),
        &[
            RiscvMmapRegion::new(
                RISCV64_LINUX_MMAP_BASE,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                0,
            ),
            RiscvMmapRegion::new(
                protect_start,
                RISCV_PAGE_BYTES,
                1,
                34,
                u64::MAX,
                RISCV_PAGE_BYTES,
            ),
            RiscvMmapRegion::new(
                protect_start + RISCV_PAGE_BYTES,
                RISCV_PAGE_BYTES,
                3,
                34,
                u64::MAX,
                2 * RISCV_PAGE_BYTES,
            ),
        ]
    );
}

#[test]
fn linux_table_mprotect_rejects_holes_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    let mapped_regions = state.mmap_regions().to_vec();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MPROTECT,
                [RISCV64_LINUX_MMAP_BASE, 2 * RISCV_PAGE_BYTES, 1, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOMEM_FOR_TEST)
        })
    );
    assert_eq!(state.mmap_regions(), mapped_regions.as_slice());
}

#[test]
fn linux_table_mprotect_rejects_invalid_arguments_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    let mapped_regions = state.mmap_regions().to_vec();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MPROTECT,
                [RISCV64_LINUX_MMAP_BASE, 0, 1, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.mmap_regions(), mapped_regions.as_slice());

    for arguments in [
        [RISCV64_LINUX_MMAP_BASE + 1, RISCV_PAGE_BYTES, 1, 0, 0, 0],
        [RISCV64_LINUX_MMAP_BASE, u64::MAX, 1, 0, 0, 0],
        [
            u64::MAX - (RISCV_PAGE_BYTES - 1),
            RISCV_PAGE_BYTES,
            1,
            0,
            0,
            0,
        ],
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MPROTECT, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert_eq!(state.mmap_regions(), mapped_regions.as_slice());
    }
}

#[test]
fn linux_table_rejects_invalid_munmap_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    let mapped_regions = state.mmap_regions().to_vec();

    for arguments in [
        [RISCV64_LINUX_MMAP_BASE + 1, RISCV_PAGE_BYTES, 0, 0, 0, 0],
        [RISCV64_LINUX_MMAP_BASE, 0, 0, 0, 0, 0],
        [RISCV64_LINUX_MMAP_BASE, u64::MAX, 0, 0, 0, 0],
        [
            u64::MAX - (RISCV_PAGE_BYTES - 1),
            RISCV_PAGE_BYTES,
            0,
            0,
            0,
            0,
        ],
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MUNMAP, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert_eq!(state.mmap_regions(), mapped_regions.as_slice());
    }
}

#[test]
fn linux_table_rejects_overflowing_fixed_mmap() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [
                    u64::MAX - (RISCV_PAGE_BYTES - 1),
                    RISCV_PAGE_BYTES,
                    3,
                    34 | RISCV_LINUX_MAP_FIXED,
                    u64::MAX,
                    0,
                ]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.mmap_regions().is_empty());
}
