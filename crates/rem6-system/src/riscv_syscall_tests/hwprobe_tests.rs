use super::*;

const RISCV_LINUX_RISCV_HWPROBE_FOR_TEST: u64 = 258;
const RISCV_LINUX_RISCV_HWPROBE_PAIR_BYTES_FOR_TEST: usize = 16;
const RISCV_LINUX_RISCV_HWPROBE_KEY_MVENDORID_FOR_TEST: i64 = 0;
const RISCV_LINUX_RISCV_HWPROBE_KEY_MARCHID_FOR_TEST: i64 = 1;
const RISCV_LINUX_RISCV_HWPROBE_KEY_MIMPID_FOR_TEST: i64 = 2;
const RISCV_LINUX_RISCV_HWPROBE_KEY_BASE_BEHAVIOR_FOR_TEST: i64 = 3;
const RISCV_LINUX_RISCV_HWPROBE_KEY_IMA_EXT_0_FOR_TEST: i64 = 4;
const RISCV_LINUX_RISCV_HWPROBE_KEY_CPUPERF_0_FOR_TEST: i64 = 5;
const RISCV_LINUX_RISCV_HWPROBE_BASE_BEHAVIOR_IMA_FOR_TEST: u64 = 1;
const RISCV_LINUX_RISCV_HWPROBE_IMA_FD_FOR_TEST: u64 = 1 << 0;
const RISCV_LINUX_RISCV_HWPROBE_IMA_C_FOR_TEST: u64 = 1 << 1;
const RISCV_LINUX_RISCV_HWPROBE_CPU_PERF_SLOW_FOR_TEST: u64 = 2;
const RISCV_LINUX_EFAULT_FOR_TEST: u64 = 14;
const RISCV_LINUX_EINVAL_FOR_TEST: u64 = 22;

#[test]
fn linux_table_riscv_hwprobe_writes_known_values_and_clears_unknown_keys() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let pairs = hwprobe_pair_bytes(&[
        (RISCV_LINUX_RISCV_HWPROBE_KEY_MVENDORID_FOR_TEST, 0xaaaa),
        (RISCV_LINUX_RISCV_HWPROBE_KEY_MARCHID_FOR_TEST, 0xaaaa),
        (RISCV_LINUX_RISCV_HWPROBE_KEY_MIMPID_FOR_TEST, 0xaaaa),
        (RISCV_LINUX_RISCV_HWPROBE_KEY_BASE_BEHAVIOR_FOR_TEST, 0xbbbb),
        (RISCV_LINUX_RISCV_HWPROBE_KEY_IMA_EXT_0_FOR_TEST, 0xcccc),
        (RISCV_LINUX_RISCV_HWPROBE_KEY_CPUPERF_0_FOR_TEST, 0xcccc),
        (999, 0xdddd),
    ]);
    let guest_memory_reader = memory_reader(vec![(0x9000, pairs)]);
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
                RISCV_LINUX_RISCV_HWPROBE_FOR_TEST,
                [0x9000, 7, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let written = collect_guest_writes(
        &writes.lock().unwrap(),
        0x9000,
        7 * RISCV_LINUX_RISCV_HWPROBE_PAIR_BYTES_FOR_TEST,
    );
    assert_eq!(
        read_hwprobe_pairs(&written),
        vec![
            (RISCV_LINUX_RISCV_HWPROBE_KEY_MVENDORID_FOR_TEST, 0),
            (RISCV_LINUX_RISCV_HWPROBE_KEY_MARCHID_FOR_TEST, 0),
            (RISCV_LINUX_RISCV_HWPROBE_KEY_MIMPID_FOR_TEST, 0),
            (
                RISCV_LINUX_RISCV_HWPROBE_KEY_BASE_BEHAVIOR_FOR_TEST,
                RISCV_LINUX_RISCV_HWPROBE_BASE_BEHAVIOR_IMA_FOR_TEST,
            ),
            (
                RISCV_LINUX_RISCV_HWPROBE_KEY_IMA_EXT_0_FOR_TEST,
                RISCV_LINUX_RISCV_HWPROBE_IMA_FD_FOR_TEST
                    | RISCV_LINUX_RISCV_HWPROBE_IMA_C_FOR_TEST,
            ),
            (
                RISCV_LINUX_RISCV_HWPROBE_KEY_CPUPERF_0_FOR_TEST,
                RISCV_LINUX_RISCV_HWPROBE_CPU_PERF_SLOW_FOR_TEST,
            ),
            (-1, 0),
        ]
    );
}

#[test]
fn linux_table_riscv_hwprobe_accepts_zero_pair_count_without_guest_memory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RISCV_HWPROBE_FOR_TEST,
                [0, 0, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_riscv_hwprobe_rejects_reserved_flags_without_writing() {
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
                RISCV_LINUX_RISCV_HWPROBE_FOR_TEST,
                [0x9000, 1, 0, 0, 2, 0],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL_FOR_TEST)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_riscv_hwprobe_rejects_excessive_pair_count_without_writing() {
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
                RISCV_LINUX_RISCV_HWPROBE_FOR_TEST,
                [0x9000, 1025, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL_FOR_TEST)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_riscv_hwprobe_rejects_cpu_mask_without_online_cpu() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = memory_reader(vec![
        (
            0x9000,
            hwprobe_pair_bytes(&[(RISCV_LINUX_RISCV_HWPROBE_KEY_BASE_BEHAVIOR_FOR_TEST, 0)]),
        ),
        (0xa000, vec![0b0000_0010]),
    ]);
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
                RISCV_LINUX_RISCV_HWPROBE_FOR_TEST,
                [0x9000, 1, 1, 0xa000, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL_FOR_TEST)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_riscv_hwprobe_accepts_cpu_mask_with_online_cpu() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = memory_reader(vec![
        (
            0x9000,
            hwprobe_pair_bytes(&[(RISCV_LINUX_RISCV_HWPROBE_KEY_BASE_BEHAVIOR_FOR_TEST, 0)]),
        ),
        (0xa000, vec![0b0000_0001]),
    ]);
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
                RISCV_LINUX_RISCV_HWPROBE_FOR_TEST,
                [0x9000, 1, 1, 0xa000, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let written = collect_guest_writes(
        &writes.lock().unwrap(),
        0x9000,
        RISCV_LINUX_RISCV_HWPROBE_PAIR_BYTES_FOR_TEST,
    );
    assert_eq!(
        read_hwprobe_pairs(&written),
        vec![(
            RISCV_LINUX_RISCV_HWPROBE_KEY_BASE_BEHAVIOR_FOR_TEST,
            RISCV_LINUX_RISCV_HWPROBE_BASE_BEHAVIOR_IMA_FOR_TEST,
        )]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_riscv_hwprobe_reports_efault_for_guest_memory_failures() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let good_reader = memory_reader(vec![(
        0x9000,
        hwprobe_pair_bytes(&[(RISCV_LINUX_RISCV_HWPROBE_KEY_MVENDORID_FOR_TEST, 0)]),
    )]);
    let faulting_reader = RiscvGuestMemoryReader::new(|_address, _bytes| None);
    let good_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true);
    let faulting_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);

    for (reader, writer) in [
        (None, Some(&good_writer)),
        (Some(&faulting_reader), Some(&good_writer)),
        (Some(&good_reader), None),
        (Some(&good_reader), Some(&faulting_writer)),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_RISCV_HWPROBE_FOR_TEST,
                    [0x9000, 1, 0, 0, 0, 0],
                ),
                &mut state,
                11,
                reader,
                writer,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EFAULT_FOR_TEST)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

fn hwprobe_pair_bytes(pairs: &[(i64, u64)]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(pairs.len() * RISCV_LINUX_RISCV_HWPROBE_PAIR_BYTES_FOR_TEST);
    for (key, value) in pairs {
        bytes.extend_from_slice(&key.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn read_hwprobe_pairs(bytes: &[u8]) -> Vec<(i64, u64)> {
    bytes
        .chunks_exact(RISCV_LINUX_RISCV_HWPROBE_PAIR_BYTES_FOR_TEST)
        .map(|chunk| {
            (
                i64::from_le_bytes(chunk[0..8].try_into().unwrap()),
                read_le_u64(chunk, 8),
            )
        })
        .collect()
}

fn memory_reader(regions: Vec<(u64, Vec<u8>)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        for (base, data) in &regions {
            let Some(offset) = address.checked_sub(*base) else {
                continue;
            };
            let Ok(offset) = usize::try_from(offset) else {
                continue;
            };
            let Some(end) = offset.checked_add(bytes) else {
                continue;
            };
            if end <= data.len() {
                return Some(data[offset..end].to_vec());
            }
        }
        None
    })
}
