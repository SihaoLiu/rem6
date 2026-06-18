use super::*;

type GuestWriteRecords = std::sync::Arc<std::sync::Mutex<Vec<(u64, Vec<u8>)>>>;

const RISCV_LINUX_CAPGET_FOR_TEST: u64 = 90;
const RISCV_LINUX_CAPSET_FOR_TEST: u64 = 91;
const RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST: u32 = 0x2008_0522;
const RISCV_LINUX_CAP_HEADER_BYTES_FOR_TEST: usize = 8;
const RISCV_LINUX_CAP_DATA_BYTES_FOR_TEST: usize = 24;
const RISCV_LINUX_EFAULT_FOR_TEST: u64 = 14;
const RISCV_LINUX_EINVAL_FOR_TEST: u64 = 22;
const RISCV_LINUX_EPERM_FOR_TEST: u64 = 1;
const RISCV_LINUX_ESRCH_FOR_TEST: u64 = 3;

#[test]
fn linux_table_capget_writes_zero_current_process_capabilities() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = capability_reader(vec![(
        0x9000,
        capability_header_bytes(RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST, 0),
    )]);
    let writes = recorded_writer();
    let guest_memory_writer = writer_for_records(&writes);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_CAPGET_FOR_TEST,
                [0x9000, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(0x9010, vec![0; RISCV_LINUX_CAP_DATA_BYTES_FOR_TEST])]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_capget_reports_linux_errors_and_version_probe() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CAPGET_FOR_TEST, [0, 0x9010, 0, 0, 0, 0],),
            &mut state,
            11,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );

    let bad_version_reader =
        capability_reader(vec![(0x9100, capability_header_bytes(0x1234_5678, 0))]);
    let writes = recorded_writer();
    let guest_memory_writer = writer_for_records(&writes);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_CAPGET_FOR_TEST,
                [0x9100, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&bad_version_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL_FOR_TEST)
        })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(
            0x9100,
            RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST
                .to_le_bytes()
                .to_vec()
        )]
    );

    let missing_pid_reader = capability_reader(vec![(
        0x9200,
        capability_header_bytes(RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST, 999_999),
    )]);
    let no_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("capget for a missing pid must not write capability data")
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_CAPGET_FOR_TEST,
                [0x9200, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&missing_pid_reader),
            Some(&no_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );

    let faulting_reader = RiscvGuestMemoryReader::new(|_address, _bytes| None);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_CAPGET_FOR_TEST,
                [0x9300, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&faulting_reader),
            Some(&no_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );

    let good_reader = capability_reader(vec![(
        0x9400,
        capability_header_bytes(RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST, 0),
    )]);
    let faulting_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_CAPGET_FOR_TEST,
                [0x9400, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&good_reader),
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_capget_accepts_null_data_for_current_process() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = capability_reader(vec![(
        0x9000,
        capability_header_bytes(RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST, 0),
    )]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CAPGET_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            11,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_capset_accepts_zero_capabilities_for_current_process() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = capability_reader(vec![
        (
            0x9000,
            capability_header_bytes(RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST, 0),
        ),
        (0x9010, vec![0; RISCV_LINUX_CAP_DATA_BYTES_FOR_TEST]),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_CAPSET_FOR_TEST,
                [0x9000, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_capset_reports_linux_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CAPSET_FOR_TEST, [0, 0x9010, 0, 0, 0, 0],),
            &mut state,
            11,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );

    let guest_memory_reader = capability_reader(vec![(
        0x9100,
        capability_header_bytes(RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST, 0),
    )]);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_CAPSET_FOR_TEST, [0x9100, 0, 0, 0, 0, 0],),
            &mut state,
            11,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );

    let nonzero_reader = capability_reader(vec![
        (
            0x9200,
            capability_header_bytes(RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST, 0),
        ),
        (0x9210, vec![1; RISCV_LINUX_CAP_DATA_BYTES_FOR_TEST]),
    ]);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_CAPSET_FOR_TEST,
                [0x9200, 0x9210, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&nonzero_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM_FOR_TEST)
        })
    );

    let bad_version_reader =
        capability_reader(vec![(0x9300, capability_header_bytes(0x1234_5678, 0))]);
    let writes = recorded_writer();
    let guest_memory_writer = writer_for_records(&writes);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_CAPSET_FOR_TEST,
                [0x9300, 0x9310, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&bad_version_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL_FOR_TEST)
        })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(
            0x9300,
            RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST
                .to_le_bytes()
                .to_vec()
        )]
    );

    let missing_pid_reader = capability_reader(vec![(
        0x9380,
        capability_header_bytes(RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST, 999_999),
    )]);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800e,
                RISCV_LINUX_CAPSET_FOR_TEST,
                [0x9380, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&missing_pid_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM_FOR_TEST)
        })
    );

    let faulting_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        if address == 0x9400 && bytes == RISCV_LINUX_CAP_HEADER_BYTES_FOR_TEST {
            Some(capability_header_bytes(
                RISCV_LINUX_CAPABILITY_VERSION_3_FOR_TEST,
                0,
            ))
        } else {
            None
        }
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_CAPSET_FOR_TEST,
                [0x9400, 0x9410, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&faulting_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn capability_header_bytes(version: u32, pid: i32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(RISCV_LINUX_CAP_HEADER_BYTES_FOR_TEST);
    bytes.extend_from_slice(&version.to_le_bytes());
    bytes.extend_from_slice(&pid.to_le_bytes());
    bytes
}

fn capability_reader(regions: Vec<(u64, Vec<u8>)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, count| {
        regions.iter().find_map(|(base, bytes)| {
            let offset = usize::try_from(address.checked_sub(*base)?).ok()?;
            let end = offset.checked_add(count)?;
            bytes.get(offset..end).map(|chunk| chunk.to_vec())
        })
    })
}

fn recorded_writer() -> GuestWriteRecords {
    std::sync::Arc::new(std::sync::Mutex::new(Vec::new()))
}

fn writer_for_records(writes: &GuestWriteRecords) -> RiscvGuestMemoryWriter {
    let writes_for_writer = std::sync::Arc::clone(writes);
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    })
}
