use super::*;

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
