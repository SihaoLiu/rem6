use super::*;

const SIG_IGN: u64 = 1;
const SIGUSR1: u64 = 10;
const SIGCHLD: u64 = 17;

type SignalWriteLog = std::sync::Arc<std::sync::Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_blocked_default_ignored_signal_is_pending_until_unblocked() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_mask_address = 0x2300;
    let first_pending_address = 0x2310;
    let second_pending_address = 0x2320;
    let signal_mask = 1_u64 << (SIGCHLD - 1);
    let guest_memory_reader = signal_mask_reader(signal_mask_address, signal_mask);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_signal_writer(std::sync::Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, signal_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_KILL, [100, SIGCHLD, 0, 0, 0, 0]),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGPENDING,
                [first_pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), first_pending_address),
        signal_mask.to_le_bytes()
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_RT_SIGPROCMASK,
                [1, signal_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_RT_SIGPENDING,
                [second_pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            12,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), second_pending_address),
        0_u64.to_le_bytes()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_blocked_explicit_ignored_signal_is_pending_until_unblocked() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_mask_address = 0x2330;
    let first_pending_address = 0x2340;
    let second_pending_address = 0x2350;
    let signal_mask = 1_u64 << (SIGUSR1 - 1);
    let guest_memory_reader = signal_mask_reader(signal_mask_address, signal_mask);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_signal_writer(std::sync::Arc::clone(&writes));
    install_signal_handler(&table, &mut state, SIGUSR1, SIG_IGN);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, signal_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_KILL, [100, SIGUSR1, 0, 0, 0, 0]),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGPENDING,
                [first_pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), first_pending_address),
        signal_mask.to_le_bytes()
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_RT_SIGPROCMASK,
                [1, signal_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_RT_SIGPENDING,
                [second_pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            12,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), second_pending_address),
        0_u64.to_le_bytes()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sigsuspend_unblocks_pending_ignored_signal_before_blocking() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let block_mask_address = 0x2360;
    let suspend_mask_address = 0x2370;
    let pending_address = 0x2380;
    let signal_mask = 1_u64 << (SIGCHLD - 1);
    let guest_memory_reader = multi_signal_mask_reader(vec![
        (block_mask_address, signal_mask),
        (suspend_mask_address, 0),
    ]);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_signal_writer(std::sync::Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, block_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_KILL, [100, SIGCHLD, 0, 0, 0, 0]),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGSUSPEND,
                [suspend_mask_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_RT_SIGPENDING,
                [pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), pending_address),
        0_u64.to_le_bytes()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sigaction_ignore_clears_pending_signal() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_mask_address = 0x2390;
    let first_pending_address = 0x23a0;
    let second_pending_address = 0x23b0;
    let signal_mask = 1_u64 << (SIGUSR1 - 1);
    let guest_memory_reader = signal_mask_reader(signal_mask_address, signal_mask);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_signal_writer(std::sync::Arc::clone(&writes));
    install_signal_handler(&table, &mut state, SIGUSR1, SIG_IGN);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, signal_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_KILL, [100, SIGUSR1, 0, 0, 0, 0]),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGPENDING,
                [first_pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), first_pending_address),
        signal_mask.to_le_bytes()
    );
    install_signal_handler(&table, &mut state, SIGUSR1, SIG_IGN);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_RT_SIGPENDING,
                [second_pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), second_pending_address),
        0_u64.to_le_bytes()
    );
}

#[test]
fn linux_table_unblock_pending_nonignored_signal_records_unimplemented_delivery() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_mask_address = 0x23c0;
    let signal_mask = 1_u64 << (SIGUSR1 - 1);
    let guest_memory_reader = signal_mask_reader(signal_mask_address, signal_mask);
    install_signal_handler(&table, &mut state, SIGUSR1, SIG_IGN);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, signal_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_KILL, [100, SIGUSR1, 0, 0, 0, 0]),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    install_signal_handler(&table, &mut state, SIGUSR1, 0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGPROCMASK,
                [1, signal_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(state.signal_mask(), 0);
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x800c,
            RISCV_LINUX_RT_SIGPROCMASK,
            [1, signal_mask_address, 0, 8, 0, 0],
            10
        )]
    );
}

#[test]
fn linux_table_blocked_nonignored_signal_is_pending_until_unblocked() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_mask_address = 0x23f0;
    let first_pending_address = 0x2400;
    let oldset_address = 0x2410;
    let signal_mask = 1_u64 << (SIGUSR1 - 1);
    let guest_memory_reader = signal_mask_reader(signal_mask_address, signal_mask);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guest_memory_writer = recording_signal_writer(std::sync::Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, signal_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_KILL, [100, SIGUSR1, 0, 0, 0, 0]),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGPENDING,
                [first_pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), first_pending_address),
        signal_mask.to_le_bytes()
    );
    assert!(state.unknown_syscalls().is_empty());

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_RT_SIGPROCMASK,
                [1, signal_mask_address, oldset_address, 8, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), oldset_address),
        signal_mask.to_le_bytes()
    );
    assert_eq!(state.signal_mask(), 0);
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8010,
            RISCV_LINUX_RT_SIGPROCMASK,
            [1, signal_mask_address, oldset_address, 8, 0, 0],
            11
        )]
    );
}

#[test]
fn linux_table_sigsuspend_pending_nonignored_signal_records_call_tick() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let block_mask_address = 0x23d0;
    let suspend_mask_address = 0x23e0;
    let signal_mask = 1_u64 << (SIGUSR1 - 1);
    let guest_memory_reader = multi_signal_mask_reader(vec![
        (block_mask_address, signal_mask),
        (suspend_mask_address, 0),
    ]);
    install_signal_handler(&table, &mut state, SIGUSR1, SIG_IGN);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, block_mask_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_KILL, [100, SIGUSR1, 0, 0, 0, 0]),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    install_signal_handler(&table, &mut state, SIGUSR1, 0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGSUSPEND,
                [suspend_mask_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            12,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(state.signal_mask(), signal_mask);
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x800c,
            RISCV_LINUX_RT_SIGSUSPEND,
            [suspend_mask_address, 8, 0, 0, 0, 0],
            12
        )]
    );
}

fn install_signal_handler(
    table: &RiscvSyscallTable,
    state: &mut RiscvSyscallState,
    signal: u64,
    handler: u64,
) {
    let action_address = 0x7000;
    let action = sigaction_bytes(handler, 0, 0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == action_address && bytes == 24 {
            Some(action.clone())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGACTION,
                [signal, action_address, 0, 8, 0, 0],
            ),
            state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

fn signal_mask_reader(address: u64, mask: u64) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |read_address, read_bytes| {
        if read_address == address && read_bytes == 8 {
            Some(mask.to_le_bytes().to_vec())
        } else {
            None
        }
    })
}

fn multi_signal_mask_reader(regions: Vec<(u64, u64)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |read_address, read_bytes| {
        regions
            .iter()
            .find(|(address, _)| *address == read_address && read_bytes == 8)
            .map(|(_, mask)| mask.to_le_bytes().to_vec())
    })
}

fn recording_signal_writer(writes: SignalWriteLog) -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes.lock().unwrap().push((address, bytes.to_vec()));
        true
    })
}

fn sigaction_bytes(handler: u64, flags: u64, mask: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&handler.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&mask.to_le_bytes());
    bytes
}

fn written_signal_mask_at(writes: &[(u64, Vec<u8>)], address: u64) -> [u8; 8] {
    let (_, bytes) = writes
        .iter()
        .find(|(write_address, _)| *write_address == address)
        .expect("signal mask write must exist");
    bytes.as_slice().try_into().unwrap()
}
