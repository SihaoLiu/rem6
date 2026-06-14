use super::*;

const RISCV_LINUX_GETCPU_FOR_TEST: u64 = 168;
const RISCV_LINUX_MEMBARRIER_FOR_TEST: u64 = 283;
const RISCV_LINUX_CPU_ID_FOR_TEST: u32 = 0;
const RISCV_LINUX_NUMA_NODE_FOR_TEST: u32 = 0;
const RISCV_LINUX_MEMBARRIER_CMD_QUERY_FOR_TEST: u64 = 0;
const RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED_FOR_TEST: u64 = 1 << 3;
const RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_FOR_TEST: u64 = 1 << 4;
const RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS_FOR_TEST: u64 = 1 << 9;
const RISCV_LINUX_MEMBARRIER_SUPPORTED_COMMANDS_FOR_TEST: u64 =
    RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED_FOR_TEST
        | RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_FOR_TEST
        | RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS_FOR_TEST;
const RISCV_LINUX_EPERM_FOR_TEST: u64 = 1;
const RISCV_LINUX_EFAULT_FOR_TEST: u64 = 14;
const RISCV_LINUX_EINVAL_FOR_TEST: u64 = 22;

#[test]
fn linux_table_membarrier_reports_supported_single_process_commands() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMBARRIER_FOR_TEST,
                [RISCV_LINUX_MEMBARRIER_CMD_QUERY_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_MEMBARRIER_SUPPORTED_COMMANDS_FOR_TEST
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_membarrier_requires_private_expedited_registration() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMBARRIER_FOR_TEST,
                [
                    RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED_FOR_TEST,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_membarrier_tracks_private_expedited_registration() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMBARRIER_FOR_TEST,
                [
                    RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_FOR_TEST,
                    0,
                    0,
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
                0x8004,
                RISCV_LINUX_MEMBARRIER_FOR_TEST,
                [
                    RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS_FOR_TEST,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED_FOR_TEST
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MEMBARRIER_FOR_TEST,
                [
                    RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED_FOR_TEST,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_membarrier_rejects_invalid_flags_and_commands() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, command, flags) in [
        (0x8000, RISCV_LINUX_MEMBARRIER_CMD_QUERY_FOR_TEST, 1),
        (0x8004, 1 << 30, 0),
        (
            0x8008,
            RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED_FOR_TEST,
            1,
        ),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_MEMBARRIER_FOR_TEST,
                    [command, flags, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL_FOR_TEST)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_getcpu_writes_single_cpu_and_node() {
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
                RISCV_LINUX_GETCPU_FOR_TEST,
                [0x9000, 0x9008, 0, 0, 0, 0],
            ),
            &mut state,
            17,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let writes = writes.lock().unwrap();
    assert_eq!(written_u32_at(&writes, 0x9000), RISCV_LINUX_CPU_ID_FOR_TEST);
    assert_eq!(
        written_u32_at(&writes, 0x9008),
        RISCV_LINUX_NUMA_NODE_FOR_TEST
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_getcpu_allows_null_outputs_without_guest_writer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCPU_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
            18,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_getcpu_writes_only_requested_output() {
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCPU_FOR_TEST, [0, 0x9008, 0, 0, 0, 0],),
            &mut state,
            19,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(
            0x9008,
            RISCV_LINUX_NUMA_NODE_FOR_TEST.to_le_bytes().to_vec()
        )]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_getcpu_reports_guest_write_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, RISCV_LINUX_CPU_ID_FOR_TEST.to_le_bytes());
        false
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCPU_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            20,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_getcpu_without_guest_writer_for_outputs_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCPU_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            21,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn written_u32_at(writes: &[(u64, Vec<u8>)], address: u64) -> u32 {
    writes
        .iter()
        .find_map(|(written_address, bytes)| {
            (*written_address == address)
                .then(|| u32::from_le_bytes(bytes.as_slice().try_into().unwrap()))
        })
        .unwrap_or_else(|| panic!("missing getcpu write at {address:#x}"))
}
