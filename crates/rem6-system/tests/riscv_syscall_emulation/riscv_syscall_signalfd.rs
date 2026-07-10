use super::riscv_syscall_emulation_support::*;
use rem6_system::{
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RiscvSyscallTable,
};

const RISCV_LINUX_SIGNALFD4: u64 = 74;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_PPOLL: u64 = 73;
const RISCV_LINUX_KILL: u64 = 129;
const RISCV_LINUX_RT_SIGPROCMASK: u64 = 135;
const RISCV_LINUX_RT_SIGPENDING: u64 = 136;
const RISCV_LINUX_EAGAIN: u64 = 11;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_POLLIN: i16 = 0x0001;
const RISCV_LINUX_SIG_BLOCK: u64 = 0;
const RISCV_LINUX_SIGSET_BYTES: u64 = 8;
const RISCV_LINUX_SIGNALFD_SIGINFO_BYTES: u64 = 128;
const SIGUSR1: u64 = 10;
const SIGUSR2: u64 = 12;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn signalfd_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let sigusr1_mask = signal_mask(SIGUSR1);
    let pollfd = pollfd_bytes(3, RISCV_LINUX_POLLIN);
    let zero_timeout = [0_u8; 16];
    let siginfo = [0_u8; 128];
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x9000, &sigusr1_mask),
            (0x9010, &pollfd),
            (0x9020, &zero_timeout),
            (0x9030, &siginfo),
            (0x9100, &[0_u8; 8]),
            (0x9300, &[0_u8; 256]),
        ],
    )
}

fn signal_mask(signal: u64) -> [u8; 8] {
    (1_u64 << (signal - 1)).to_le_bytes()
}

fn pollfd_bytes(fd: i32, events: i16) -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    bytes[..4].copy_from_slice(&fd.to_le_bytes());
    bytes[4..6].copy_from_slice(&events.to_le_bytes());
    bytes
}

fn memory_u32(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> u32 {
    let bytes = guest_memory_reader(Arc::clone(store))(address, 4).unwrap();
    u32::from_le_bytes(bytes.try_into().unwrap())
}

fn memory_u64(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> u64 {
    let bytes = guest_memory_reader(Arc::clone(store))(address, 8).unwrap();
    u64::from_le_bytes(bytes.try_into().unwrap())
}

fn pollfd_revents(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> i16 {
    let bytes = guest_memory_reader(Arc::clone(store))(address + 6, 2).unwrap();
    i16::from_le_bytes(bytes.try_into().unwrap())
}

fn handle(state: &mut RiscvSyscallState, number: u64, arguments: [u64; 6]) -> RiscvSyscallOutcome {
    RiscvSyscallTable::new()
        .handle(RiscvSyscallRequest::new(0x8000, number, arguments), state)
        .expect("syscall must be handled")
}

fn handle_with_memory(
    state: &mut RiscvSyscallState,
    number: u64,
    arguments: [u64; 6],
    reader: Option<&RiscvGuestMemoryReader>,
    writer: Option<&RiscvGuestMemoryWriter>,
) -> RiscvSyscallOutcome {
    RiscvSyscallTable::new()
        .handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, number, arguments),
            state,
            7,
            reader,
            writer,
        )
        .expect("syscall must be handled")
}

fn return_value(outcome: RiscvSyscallOutcome) -> u64 {
    match outcome {
        RiscvSyscallOutcome::Return { value } => value,
        outcome => panic!("unexpected syscall outcome: {outcome:?}"),
    }
}

#[test]
fn linux_table_signalfd4_consumes_blocked_signal_through_readiness() {
    let store = signalfd_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RT_SIGPROCMASK,
            [
                RISCV_LINUX_SIG_BLOCK,
                0x9000,
                0,
                RISCV_LINUX_SIGSET_BYTES,
                0,
                0,
            ],
            Some(&reader),
            Some(&writer),
        )),
        0
    );

    let fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_SIGNALFD4,
        [
            u64::MAX,
            0x9000,
            RISCV_LINUX_SIGSET_BYTES,
            RISCV_LINUX_O_NONBLOCK,
            0,
            0,
        ],
        Some(&reader),
        None,
    ));
    assert_eq!(fd, 3);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9030, RISCV_LINUX_SIGNALFD_SIGINFO_BYTES, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EAGAIN)
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9010, 1, 0x9020, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    assert_eq!(pollfd_revents(&store, 0x9010), 0);

    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_KILL,
            [100, SIGUSR1, 0, 0, 0, 0],
        )),
        0
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9010, 1, 0x9020, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(
        pollfd_revents(&store, 0x9010) & RISCV_LINUX_POLLIN,
        RISCV_LINUX_POLLIN
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9030, RISCV_LINUX_SIGNALFD_SIGINFO_BYTES, 0, 0, 0],
            None,
            Some(&writer),
        )),
        RISCV_LINUX_SIGNALFD_SIGINFO_BYTES
    );
    assert_eq!(memory_u32(&store, 0x9030), SIGUSR1 as u32);
    assert_eq!(memory_u32(&store, 0x9030 + 12), 100);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RT_SIGPENDING,
            [0x9100, RISCV_LINUX_SIGSET_BYTES, 0, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
    assert_eq!(memory_u64(&store, 0x9100), 0);

    assert_eq!(
        return_value(handle(&mut state, RISCV_LINUX_CLOSE, [fd, 0, 0, 0, 0, 0],)),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9030, RISCV_LINUX_SIGNALFD_SIGINFO_BYTES, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_signalfd4_validates_fd_flags_and_write_direction() {
    let store = signalfd_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let _writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SIGNALFD4,
            [u64::MAX, 0x9000, 4, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SIGNALFD4,
            [u64::MAX, 0x9000, RISCV_LINUX_SIGSET_BYTES, 0x40, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );

    let fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_SIGNALFD4,
        [
            u64::MAX,
            0x9000,
            RISCV_LINUX_SIGSET_BYTES,
            RISCV_LINUX_O_NONBLOCK,
            0,
            0,
        ],
        Some(&reader),
        None,
    ));
    assert_eq!(fd, 3);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SIGNALFD4,
            [0, 0x9000, RISCV_LINUX_SIGSET_BYTES, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SIGNALFD4,
            [99, 0x9000, RISCV_LINUX_SIGSET_BYTES, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd, 0x9030, RISCV_LINUX_SIGNALFD_SIGINFO_BYTES, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_signalfd4_update_preserves_existing_file_status_flags() {
    let store = signalfd_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_SIGNALFD4,
        [u64::MAX, 0x9000, RISCV_LINUX_SIGSET_BYTES, 0, 0, 0],
        Some(&reader),
        None,
    ));
    assert_eq!(fd, 3);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_SIGNALFD4,
            [
                fd,
                0x9000,
                RISCV_LINUX_SIGSET_BYTES,
                RISCV_LINUX_O_NONBLOCK,
                0,
                0
            ],
            Some(&reader),
            None,
        )),
        fd
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_READ,
                [fd, 0x9030, RISCV_LINUX_SIGNALFD_SIGINFO_BYTES, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_signalfd4_read_returns_complete_pending_records_that_fit() {
    let store = signalfd_store();
    let combined_mask = (signal_mask_value(SIGUSR1) | signal_mask_value(SIGUSR2)).to_le_bytes();
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9000,
        &combined_mask
    ));
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RT_SIGPROCMASK,
            [
                RISCV_LINUX_SIG_BLOCK,
                0x9000,
                0,
                RISCV_LINUX_SIGSET_BYTES,
                0,
                0,
            ],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    let fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_SIGNALFD4,
        [
            u64::MAX,
            0x9000,
            RISCV_LINUX_SIGSET_BYTES,
            RISCV_LINUX_O_NONBLOCK,
            0,
            0,
        ],
        Some(&reader),
        None,
    ));
    assert_eq!(fd, 3);

    for signal in [SIGUSR1, SIGUSR2] {
        assert_eq!(
            return_value(handle(
                &mut state,
                RISCV_LINUX_KILL,
                [100, signal, 0, 0, 0, 0],
            )),
            0
        );
    }
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9300, RISCV_LINUX_SIGNALFD_SIGINFO_BYTES * 2, 0, 0, 0],
            None,
            Some(&writer),
        )),
        RISCV_LINUX_SIGNALFD_SIGINFO_BYTES * 2
    );
    assert_eq!(memory_u32(&store, 0x9300), SIGUSR1 as u32);
    assert_eq!(
        memory_u32(&store, 0x9300 + RISCV_LINUX_SIGNALFD_SIGINFO_BYTES),
        SIGUSR2 as u32
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_RT_SIGPENDING,
            [0x9100, RISCV_LINUX_SIGSET_BYTES, 0, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
    assert_eq!(memory_u64(&store, 0x9100), 0);
    assert!(state.unknown_syscalls().is_empty());
}

const fn signal_mask_value(signal: u64) -> u64 {
    1_u64 << (signal - 1)
}
