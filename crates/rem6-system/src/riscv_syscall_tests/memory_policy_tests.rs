use super::*;

const RISCV_LINUX_MBIND_FOR_TEST: u64 = 235;
const RISCV_LINUX_GET_MEMPOLICY_FOR_TEST: u64 = 236;
const RISCV_LINUX_EPERM_FOR_TEST: u64 = 1;
const RISCV_LINUX_EFAULT_FOR_TEST: u64 = 14;
const RISCV_LINUX_MPOL_DEFAULT_FOR_TEST: u64 = 0;
const RISCV_LINUX_MPOL_PREFERRED_FOR_TEST: u64 = 1;
const RISCV_LINUX_MPOL_BIND_FOR_TEST: u64 = 2;
const RISCV_LINUX_MPOL_INTERLEAVE_FOR_TEST: u64 = 3;
const RISCV_LINUX_MPOL_LOCAL_FOR_TEST: u64 = 4;
const RISCV_LINUX_MPOL_F_NUMA_BALANCING_FOR_TEST: u64 = 1 << 13;
const RISCV_LINUX_MPOL_F_STATIC_NODES_FOR_TEST: u64 = 1 << 15;
const RISCV_LINUX_MPOL_F_RELATIVE_NODES_FOR_TEST: u64 = 1 << 14;
const RISCV_LINUX_MPOL_MF_MOVE_ALL_FOR_TEST: u64 = 1 << 2;
const RISCV_LINUX_PAGE_BITS_FOR_TEST: u64 = RISCV_PAGE_BYTES * 8;

#[test]
fn linux_table_get_mempolicy_reports_default_policy_and_empty_nodemask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });
    let faulting_writer = RiscvGuestMemoryWriter::new(|_, _| false);
    let partial_writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let partial_writes_for_writer = std::sync::Arc::clone(&partial_writes);
    let partial_faulting_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        let mut writes = partial_writes_for_writer.lock().unwrap();
        writes.push((address, bytes.to_vec()));
        writes.len() == 1
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x7ff0,
                RISCV_LINUX_GET_MEMPOLICY_FOR_TEST,
                [0x9000, 0x9010, 64, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[
            (0x9000, 0_i32.to_le_bytes().to_vec()),
            (0x9010, 0_u64.to_le_bytes().to_vec()),
        ]
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x7ff4,
                RISCV_LINUX_GET_MEMPOLICY_FOR_TEST,
                [0x9020, 0, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(0x9020, 0_i32.to_le_bytes().to_vec())]
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x7ff8,
                RISCV_LINUX_GET_MEMPOLICY_FOR_TEST,
                [0x9030, 0, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x7ffc,
                RISCV_LINUX_GET_MEMPOLICY_FOR_TEST,
                [0x9040, 0x9050, 64, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&partial_faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    assert_eq!(
        partial_writes.lock().unwrap().as_slice(),
        &[
            (0x9040, 0_i32.to_le_bytes().to_vec()),
            (0x9050, 0_u64.to_le_bytes().to_vec()),
        ]
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_GET_MEMPOLICY_FOR_TEST,
                [0, 0x9040, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GET_MEMPOLICY_FOR_TEST,
                [0x9060, 0x9050, RISCV_LINUX_PAGE_BITS_FOR_TEST + 1, 0, 0, 0,],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(writes.lock().unwrap().as_slice(), &[]);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GET_MEMPOLICY_FOR_TEST,
                [0, 0, 0, 0, 1, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_mbind_accepts_single_node_policy_on_mapped_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0x8000);
    let guest_memory = nodemask_reader(0x9000, 1);
    let large_guest_memory = zero_extended_nodemask_reader(0xa000, 1);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_BRK, [0xa000, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0xa000 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    0x8000,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_DEFAULT_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8006,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    0x8000,
                    RISCV_PAGE_BYTES * 3,
                    RISCV_LINUX_MPOL_DEFAULT_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    for args in [
        [
            0x8000,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_BIND_FOR_TEST,
            0x9000,
            2,
            0,
        ],
        [
            0x8000,
            RISCV_PAGE_BYTES,
            (1_u64 << 32) | RISCV_LINUX_MPOL_BIND_FOR_TEST,
            0x9000,
            2,
            1_u64 << 32,
        ],
    ] {
        assert_eq!(
            table.handle_with_guest_memory_at_tick(
                RiscvSyscallRequest::new(0x8008, RISCV_LINUX_MBIND_FOR_TEST, args),
                &mut state,
                0,
                Some(&guest_memory),
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    0x8000,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_PREFERRED_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    0x8000,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_BIND_FOR_TEST,
                    0xa000,
                    RISCV_LINUX_PAGE_BITS_FOR_TEST + 1,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&large_guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_mbind_rejects_invalid_range_mode_flags_and_nodemask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let node_zero = nodemask_reader(0x9000, 1);
    let node_one = nodemask_reader(0x9008, 2);
    let faulting = RiscvGuestMemoryReader::new(|_, _| None);
    let short_reader = RiscvGuestMemoryReader::new(|_, _| Some(vec![1]));

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

    for args in [
        [
            RISCV64_LINUX_MMAP_BASE + 1,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_DEFAULT_FOR_TEST,
            0,
            0,
            0,
        ],
        [u64::MAX, 1, RISCV_LINUX_MPOL_DEFAULT_FOR_TEST, 0, 0, 0],
        [RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES, 99, 0, 0, 0],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_BIND_FOR_TEST,
            0,
            0,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_BIND_FOR_TEST,
            0x9000,
            1,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_INTERLEAVE_FOR_TEST,
            0,
            0,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_LOCAL_FOR_TEST,
            0x9000,
            2,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_PREFERRED_FOR_TEST,
            0x9000,
            0,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_DEFAULT_FOR_TEST,
            0x9000,
            2,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_BIND_FOR_TEST | RISCV_LINUX_MPOL_F_NUMA_BALANCING_FOR_TEST,
            0x9000,
            2,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_BIND_FOR_TEST
                | RISCV_LINUX_MPOL_F_STATIC_NODES_FOR_TEST
                | RISCV_LINUX_MPOL_F_RELATIVE_NODES_FOR_TEST,
            0x9000,
            2,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_DEFAULT_FOR_TEST,
            0,
            0,
            0x8,
        ],
    ] {
        assert_eq!(
            table.handle_with_guest_memory_at_tick(
                RiscvSyscallRequest::new(0x8004, RISCV_LINUX_MBIND_FOR_TEST, args),
                &mut state,
                0,
                Some(&node_zero),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_BIND_FOR_TEST,
                    0x9008,
                    3,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&node_one),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_BIND_FOR_TEST,
                    0x9010,
                    2,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&faulting),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x800e,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_BIND_FOR_TEST,
                    0x9020,
                    2,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&short_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x800f,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_BIND_FOR_TEST,
                    0x9030,
                    2,
                    0x8,
                ],
            ),
            &mut state,
            0,
            Some(&faulting),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_BIND_FOR_TEST,
                    0x9038,
                    RISCV_LINUX_PAGE_BITS_FOR_TEST + 2,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&node_zero),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_MBIND_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MPOL_DEFAULT_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    for args in [
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_BIND_FOR_TEST,
            0,
            0,
            RISCV_LINUX_MPOL_MF_MOVE_ALL_FOR_TEST,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MPOL_DEFAULT_FOR_TEST,
            0x9000,
            2,
            RISCV_LINUX_MPOL_MF_MOVE_ALL_FOR_TEST,
        ],
    ] {
        assert_eq!(
            table.handle_with_guest_memory_at_tick(
                RiscvSyscallRequest::new(0x8018, RISCV_LINUX_MBIND_FOR_TEST, args),
                &mut state,
                0,
                Some(&node_zero),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EPERM_FOR_TEST)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

fn nodemask_reader(base: u64, mask: u64) -> RiscvGuestMemoryReader {
    let bytes = mask.to_le_bytes();
    RiscvGuestMemoryReader::new(move |address, count| {
        let offset = usize::try_from(address.checked_sub(base)?).ok()?;
        let end = offset.checked_add(count)?;
        (end <= bytes.len()).then(|| bytes[offset..end].to_vec())
    })
}

fn zero_extended_nodemask_reader(base: u64, mask: u64) -> RiscvGuestMemoryReader {
    let mask_bytes = mask.to_le_bytes();
    RiscvGuestMemoryReader::new(move |address, count| {
        let offset = usize::try_from(address.checked_sub(base)?).ok()?;
        let end = offset.checked_add(count)?;
        let mut bytes = vec![0; end];
        bytes[..mask_bytes.len()].copy_from_slice(&mask_bytes);
        Some(bytes[offset..end].to_vec())
    })
}
