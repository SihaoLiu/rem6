use super::*;

const FUTEX_WAIT_BITSET: u64 = 9;
const FUTEX_WAKE_OP: u64 = 5;
const FUTEX_OP_ADD: u32 = 1;
const FUTEX_OP_XOR: u32 = 4;
const FUTEX_OP_ARG_SHIFT: u32 = 8;
const FUTEX_OP_CMP_EQ: u32 = 0;
const FUTEX_OP_CMP_GT: u32 = 4;
const FUTEX_CLOCK_REALTIME: u64 = 256;

type FutexWriteLog = std::sync::Arc<std::sync::Mutex<Vec<(u64, Vec<u8>)>>>;

const fn futex_wake_op(op: u32, op_arg: u32, cmp: u32, cmp_arg: u32) -> u64 {
    (((op & 0xf) << 28) | ((cmp & 0xf) << 24) | ((op_arg & 0xfff) << 12) | (cmp_arg & 0xfff)) as u64
}

fn futex_guest_memory(address: GuestFutexAddress, value: i32) -> RiscvGuestMemoryReader {
    futex_guest_memory_for_addresses([address], value)
}

fn futex_guest_memory_for_addresses<const N: usize>(
    addresses: [GuestFutexAddress; N],
    value: i32,
) -> RiscvGuestMemoryReader {
    let addresses = addresses.map(|address| address.get());
    RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if bytes == 4 && addresses.contains(&read_address) {
            Some(value.to_le_bytes().to_vec())
        } else {
            None
        }
    })
}

fn new_write_log() -> FutexWriteLog {
    std::sync::Arc::new(std::sync::Mutex::new(Vec::new()))
}

fn recording_guest_writer(writes: FutexWriteLog) -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes.lock().unwrap().push((address, bytes.to_vec()));
        true
    })
}

fn recorded_i32_writes(writes: &FutexWriteLog) -> Vec<(u64, i32)> {
    writes
        .lock()
        .unwrap()
        .iter()
        .map(|(address, bytes)| {
            let bytes: [u8; 4] = bytes.as_slice().try_into().unwrap();
            (*address, i32::from_le_bytes(bytes))
        })
        .collect()
}

fn install_futex_waiter(
    state: &mut RiscvSyscallState,
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
    thread: u64,
    partition: u32,
    tick: u64,
    value: i32,
) {
    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            GuestFutexKey::new(address, thread_group),
            GuestThreadId::new(thread),
            PartitionId::new(partition),
            tick,
            value,
            value,
        ))
        .unwrap();
}

#[allow(clippy::too_many_arguments)]
fn handle_futex_wake_op(
    table: &RiscvSyscallTable,
    state: &mut RiscvSyscallState,
    source: GuestFutexAddress,
    op: u64,
    wake_count: u64,
    second_wake_count: u64,
    target: GuestFutexAddress,
    encoded_operation: u64,
    tick: u64,
    guest_memory: &RiscvGuestMemoryReader,
    guest_writer: &RiscvGuestMemoryWriter,
) -> Option<RiscvSyscallOutcome> {
    table.handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            98,
            [
                source.get(),
                op,
                wake_count,
                second_wake_count,
                target.get(),
                encoded_operation,
            ],
        ),
        state,
        tick,
        Some(guest_memory),
        Some(guest_writer),
    )
}

fn assert_futex_waiters(
    state: &RiscvSyscallState,
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
    expected_threads: &[u64],
) {
    let expected = expected_threads
        .iter()
        .copied()
        .map(GuestThreadId::new)
        .collect::<Vec<_>>();
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        expected
    );
}

#[test]
fn linux_table_wakes_guest_futex_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x180);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(address, thread_group);

    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(7),
            PartitionId::new(1),
            20,
            3,
            3,
        ))
        .unwrap();
    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(8),
            PartitionId::new(2),
            21,
            3,
            3,
        ))
        .unwrap();

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 1, 1, 0, 0, 0]),
            &mut state,
            40,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        vec![GuestThreadId::new(8)]
    );
}

#[test]
fn linux_table_wakes_guest_futex_waiters_by_bitset() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x184);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(address, thread_group);

    state
        .guest_futexes_mut()
        .wait(
            GuestFutexWaitRequest::new(key, GuestThreadId::new(9), PartitionId::new(1), 22, 4, 4)
                .with_bitset(0b01),
        )
        .unwrap();
    state
        .guest_futexes_mut()
        .wait(
            GuestFutexWaitRequest::new(key, GuestThreadId::new(10), PartitionId::new(2), 23, 4, 4)
                .with_bitset(0b10),
        )
        .unwrap();

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 1, 0, 0, 0b01]),
            &mut state,
            41,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        vec![GuestThreadId::new(10)]
    );
}

#[test]
fn linux_table_futex_wake_bitset_honors_wake_count() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x186);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(address, thread_group);

    for (thread, bitset) in [(11, 0b01), (12, 0b01), (13, 0b10)] {
        state
            .guest_futexes_mut()
            .wait(
                GuestFutexWaitRequest::new(
                    key,
                    GuestThreadId::new(thread),
                    PartitionId::new(1),
                    24,
                    5,
                    5,
                )
                .with_bitset(bitset),
            )
            .unwrap();
    }

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 1, 0, 0, 0b01]),
            &mut state,
            44,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        vec![GuestThreadId::new(12), GuestThreadId::new(13)]
    );
}

#[test]
fn linux_table_futex_requeue_wakes_and_moves_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x187);
    let target = GuestFutexAddress::new(0x288);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);

    for thread in 1..=4 {
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(thread),
                PartitionId::new(thread as u32),
                30 + thread,
                7,
                7,
            ))
            .unwrap();
    }

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 3, 1, 2, target.get(), 0]),
            &mut state,
            45,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        vec![GuestThreadId::new(4)]
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        vec![GuestThreadId::new(2), GuestThreadId::new(3)]
    );
}

#[test]
fn linux_table_futex_wake_op_negative_counts_update_word_without_waking() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x19c);
    let target = GuestFutexAddress::new(0x29c);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = futex_guest_memory(target, 7);
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    install_futex_waiter(&mut state, source, thread_group, 19, 1, 58, 1);
    install_futex_waiter(&mut state, target, thread_group, 20, 2, 59, 7);

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP,
            (-1_i64) as u64,
            (-1_i64) as u64,
            target,
            futex_wake_op(FUTEX_OP_ADD, 3, FUTEX_OP_CMP_EQ, 7),
            51,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(recorded_i32_writes(&writes), vec![(target.get(), 10)]);
    assert_futex_waiters(&state, source, thread_group, &[19]);
    assert_futex_waiters(&state, target, thread_group, &[20]);
}

#[test]
fn linux_table_futex_wake_op_updates_second_word_and_wakes_matching_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x1a0);
    let target = GuestFutexAddress::new(0x2a0);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = futex_guest_memory(target, 7);
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    install_futex_waiter(&mut state, source, thread_group, 21, 1, 60, 1);
    install_futex_waiter(&mut state, target, thread_group, 22, 2, 61, 7);

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP,
            1,
            1,
            target,
            futex_wake_op(FUTEX_OP_ADD, 3, FUTEX_OP_CMP_EQ, 7),
            52,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );
    assert_eq!(recorded_i32_writes(&writes), vec![(target.get(), 10)]);
    assert_futex_waiters(&state, source, thread_group, &[]);
    assert_futex_waiters(&state, target, thread_group, &[]);
}

#[test]
fn linux_table_futex_wake_op_sign_extends_non_shift_operands() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x1a2);
    let target = GuestFutexAddress::new(0x2a2);
    let guest_memory = futex_guest_memory(target, 7);
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP,
            0,
            0,
            target,
            futex_wake_op(FUTEX_OP_ADD, 0xfff, FUTEX_OP_CMP_EQ, 7),
            52,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(recorded_i32_writes(&writes), vec![(target.get(), 6)]);
}

#[test]
fn linux_table_futex_wake_op_shift_operand_uses_low_five_bits() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let first_source = GuestFutexAddress::new(0x1a3);
    let second_source = GuestFutexAddress::new(0x1a7);
    let first_target = GuestFutexAddress::new(0x2a3);
    let second_target = GuestFutexAddress::new(0x2a7);
    let guest_memory = futex_guest_memory_for_addresses([first_target, second_target], 7);
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            first_source,
            FUTEX_WAKE_OP,
            0,
            0,
            first_target,
            futex_wake_op(FUTEX_OP_ADD | FUTEX_OP_ARG_SHIFT, 31, FUTEX_OP_CMP_EQ, 7),
            52,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            second_source,
            FUTEX_WAKE_OP,
            0,
            0,
            second_target,
            futex_wake_op(FUTEX_OP_ADD | FUTEX_OP_ARG_SHIFT, 32, FUTEX_OP_CMP_EQ, 7),
            53,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        recorded_i32_writes(&writes),
        vec![
            (first_target.get(), -2_147_483_641),
            (second_target.get(), 8)
        ]
    );
}

#[test]
fn linux_table_futex_wake_op_keeps_second_waiters_when_compare_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x1a4);
    let target = GuestFutexAddress::new(0x2a4);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = futex_guest_memory(target, 7);
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    install_futex_waiter(&mut state, source, thread_group, 23, 1, 62, 1);
    install_futex_waiter(&mut state, target, thread_group, 24, 2, 63, 7);

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP,
            1,
            1,
            target,
            futex_wake_op(FUTEX_OP_XOR, 4, FUTEX_OP_CMP_GT, 9),
            53,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(recorded_i32_writes(&writes), vec![(target.get(), 3)]);
    assert_futex_waiters(&state, source, thread_group, &[]);
    assert_futex_waiters(&state, target, thread_group, &[24]);
}

#[test]
fn linux_table_futex_wake_op_read_fault_does_not_wake_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x1a6);
    let target = GuestFutexAddress::new(0x2a6);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(|_, _| None);
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    install_futex_waiter(&mut state, source, thread_group, 24, 1, 63, 1);

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP,
            1,
            0,
            target,
            futex_wake_op(FUTEX_OP_ADD, 3, FUTEX_OP_CMP_EQ, 7),
            54,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert_futex_waiters(&state, source, thread_group, &[24]);
}

#[test]
fn linux_table_futex_wake_op_write_fault_does_not_wake_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x1a8);
    let target = GuestFutexAddress::new(0x2a8);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = futex_guest_memory(target, 7);
    let guest_writer = RiscvGuestMemoryWriter::new(|_, _| false);

    install_futex_waiter(&mut state, source, thread_group, 25, 1, 64, 1);

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP,
            1,
            0,
            target,
            futex_wake_op(FUTEX_OP_ADD, 3, FUTEX_OP_CMP_EQ, 7),
            54,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_futex_waiters(&state, source, thread_group, &[25]);
}

#[test]
fn linux_table_futex_wake_op_rejects_unknown_operation_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x1ac);
    let target = GuestFutexAddress::new(0x2ac);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(|_, _| {
        panic!("unknown futex wake-op operation must fail before reading guest memory")
    });
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    install_futex_waiter(&mut state, source, thread_group, 26, 1, 65, 1);

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP,
            1,
            0,
            target,
            futex_wake_op(7, 3, FUTEX_OP_CMP_EQ, 7),
            55,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert_futex_waiters(&state, source, thread_group, &[26]);
}

#[test]
fn linux_table_futex_wake_op_bad_compare_updates_second_word_without_waking() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x1ae);
    let target = GuestFutexAddress::new(0x2ae);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = futex_guest_memory(target, 7);
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    install_futex_waiter(&mut state, source, thread_group, 27, 1, 66, 1);
    install_futex_waiter(&mut state, target, thread_group, 28, 2, 67, 7);

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP,
            1,
            1,
            target,
            futex_wake_op(FUTEX_OP_ADD, 3, 7, 7),
            56,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(recorded_i32_writes(&writes), vec![(target.get(), 10)]);
    assert_futex_waiters(&state, source, thread_group, &[27]);
    assert_futex_waiters(&state, target, thread_group, &[28]);
}

#[test]
fn linux_table_futex_wake_op_rejects_clock_realtime_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x1b0);
    let target = GuestFutexAddress::new(0x2b0);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(|_, _| {
        panic!("invalid futex wake-op clock flag must fail before reading guest memory")
    });
    let writes = new_write_log();
    let guest_writer = recording_guest_writer(std::sync::Arc::clone(&writes));

    install_futex_waiter(&mut state, source, thread_group, 27, 1, 66, 1);

    assert_eq!(
        handle_futex_wake_op(
            &table,
            &mut state,
            source,
            FUTEX_WAKE_OP | FUTEX_CLOCK_REALTIME,
            1,
            0,
            target,
            futex_wake_op(FUTEX_OP_ADD, 3, FUTEX_OP_CMP_EQ, 7),
            56,
            &guest_memory,
            &guest_writer,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert_futex_waiters(&state, source, thread_group, &[27]);
}

#[test]
fn linux_table_futex_cmp_requeue_mismatch_returns_eagain_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18d);
    let target = GuestFutexAddress::new(0x28d);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == source.get() && bytes == 4 {
            Some(4_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    for thread in 5..=6 {
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(thread),
                PartitionId::new(thread as u32),
                50 + thread,
                9,
                9,
            ))
            .unwrap();
    }

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 4, 1, 1, target.get(), 9]),
            &mut state,
            46,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(11)
        })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        vec![GuestThreadId::new(5), GuestThreadId::new(6)]
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        Vec::<GuestThreadId>::new()
    );
}

#[test]
fn linux_table_futex_cmp_requeue_match_wakes_and_moves_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18a);
    let target = GuestFutexAddress::new(0x28a);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == source.get() && bytes == 4 {
            Some(9_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    for thread in 10..=12 {
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(thread),
                PartitionId::new(thread as u32),
                80 + thread,
                9,
                9,
            ))
            .unwrap();
    }

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 4, 1, 2, target.get(), 9]),
            &mut state,
            49,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        Vec::<GuestThreadId>::new()
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        vec![GuestThreadId::new(11), GuestThreadId::new(12)]
    );
}

#[test]
fn linux_table_futex_cmp_requeue_bad_address_returns_efault_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18c);
    let target = GuestFutexAddress::new(0x28c);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);
    let guest_memory = RiscvGuestMemoryReader::new(|_, _| None);

    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(13),
            PartitionId::new(13),
            90,
            15,
            15,
        ))
        .unwrap();

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 4, 0, 1, target.get(), 15]),
            &mut state,
            50,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        vec![GuestThreadId::new(13)]
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        Vec::<GuestThreadId>::new()
    );
}

#[test]
fn linux_table_futex_requeue_rejects_negative_counts_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18e);
    let target = GuestFutexAddress::new(0x28e);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);

    for thread in 7..=8 {
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(thread),
                PartitionId::new(thread as u32),
                60 + thread,
                11,
                11,
            ))
            .unwrap();
    }

    for arguments in [
        [source.get(), 3, (-1_i64) as u64, 1, target.get(), 0],
        [source.get(), 3, 1, (-1_i64) as u64, target.get(), 0],
    ] {
        assert_eq!(
            table.handle_at_tick(
                RiscvSyscallRequest::new(0x8000, 98, arguments),
                &mut state,
                47,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert_eq!(
            state.guest_futexes().waiter_threads(source, thread_group),
            vec![GuestThreadId::new(7), GuestThreadId::new(8)]
        );
        assert_eq!(
            state.guest_futexes().waiter_threads(target, thread_group),
            Vec::<GuestThreadId>::new()
        );
    }
}

#[test]
fn linux_table_futex_requeue_rejects_clock_realtime_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18f);
    let target = GuestFutexAddress::new(0x28f);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);

    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(9),
            PartitionId::new(9),
            70,
            13,
            13,
        ))
        .unwrap();

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 3 | 256, 0, 1, target.get(), 0]),
            &mut state,
            48,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        vec![GuestThreadId::new(9)]
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        Vec::<GuestThreadId>::new()
    );
}

#[test]
fn linux_table_futex_wait_mismatch_returns_eagain_without_queueing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x188);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(1_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, 0, 0, 0]),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(11)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
    assert!(!state.guest_futexes().is_waiting(GuestThreadId::new(100)));
}

#[test]
fn linux_table_futex_wait_match_blocks_and_queues_waiter() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x191);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(2_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, 0, 0, 0]),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 1);
    assert!(state.guest_futexes().is_waiting(GuestThreadId::new(100)));
}

#[test]
fn linux_table_futex_wait_mismatch_still_validates_bad_timeout_pointer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x189);
    let timeout_address = 0x3000;
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(1_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, timeout_address, 0, 0],),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
}

#[test]
fn linux_table_futex_wait_mismatch_still_validates_invalid_timeout() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18a);
    let timeout_address = 0x3010;
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(1_i32.to_le_bytes().to_vec())
        } else if read_address == timeout_address && bytes == 16 {
            let mut timeout = Vec::new();
            timeout.extend_from_slice(&0_i64.to_le_bytes());
            timeout.extend_from_slice(&1_000_000_000_i64.to_le_bytes());
            Some(timeout)
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, timeout_address, 0, 0],),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
}

#[test]
fn linux_table_futex_wait_mismatch_with_zero_timeout_returns_eagain() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18b);
    let timeout_address = 0x3020;
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(1_i32.to_le_bytes().to_vec())
        } else if read_address == timeout_address && bytes == 16 {
            Some(vec![0; 16])
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, timeout_address, 0, 0],),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(11)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
}

#[test]
fn linux_table_futex_wait_bitset_zero_timeout_returns_etimedout_without_queueing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18d);
    let timeout_address = 0x3030;
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(2_i32.to_le_bytes().to_vec())
        } else if read_address == timeout_address && bytes == 16 {
            Some(vec![0; 16])
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                98,
                [
                    address.get(),
                    FUTEX_WAIT_BITSET,
                    2,
                    timeout_address,
                    0,
                    u32::MAX as u64,
                ],
            ),
            &mut state,
            43,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(110)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
}

#[test]
fn linux_table_futex_wait_relative_nonzero_timeout_still_blocks() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18e);
    let timeout_address = 0x3040;
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(2_i32.to_le_bytes().to_vec())
        } else if read_address == timeout_address && bytes == 16 {
            let mut timeout = Vec::new();
            timeout.extend_from_slice(&0_i64.to_le_bytes());
            timeout.extend_from_slice(&1_i64.to_le_bytes());
            Some(timeout)
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, timeout_address, 0, 0],),
            &mut state,
            43,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 1);
}

#[test]
fn linux_table_futex_wait_bitset_elapsed_timeout_returns_etimedout_without_queueing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18f);
    let timeout_address = 0x3050;
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(2_i32.to_le_bytes().to_vec())
        } else if read_address == timeout_address && bytes == 16 {
            let mut timeout = Vec::new();
            timeout.extend_from_slice(&0_i64.to_le_bytes());
            timeout.extend_from_slice(&1_i64.to_le_bytes());
            Some(timeout)
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                98,
                [
                    address.get(),
                    FUTEX_WAIT_BITSET,
                    2,
                    timeout_address,
                    0,
                    u32::MAX as u64,
                ],
            ),
            &mut state,
            43,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(110)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
}

#[test]
fn linux_table_futex_wait_bitset_zero_returns_einval() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18c);
    let guest_memory = RiscvGuestMemoryReader::new(|_, _| panic!("zero bitset must fail first"));

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 9, 2, 0, 0, 0]),
            &mut state,
            43,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_futex_wake_bitset_zero_returns_einval() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x190);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 1, 0, 0, 0]),
            &mut state,
            43,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}
