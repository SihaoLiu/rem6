#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_EVENTFD2: u64 = 19;
const RISCV_LINUX_DUP: u64 = 23;
const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_F_GETFD: u64 = 1;
const RISCV_LINUX_F_GETFL: u64 = 3;
const RISCV_LINUX_F_SETFL: u64 = 4;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_READV: u64 = 65;
const RISCV_LINUX_WRITEV: u64 = 66;
const RISCV_LINUX_PPOLL: u64 = 73;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EAGAIN: u64 = 11;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_O_RDWR: u64 = 2;
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_O_CLOEXEC: u64 = 0o2_000_000;
const RISCV_LINUX_EFD_SEMAPHORE: u64 = 1;
const RISCV_LINUX_POLLIN: i16 = 0x0001;
const RISCV_LINUX_POLLOUT: i16 = 0x0004;
const RISCV_LINUX_POLLRDNORM: i16 = 0x0040;
const RISCV_LINUX_POLLWRNORM: i16 = 0x0100;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn eventfd_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let zeroes = [0; 8];
    let zeroes16 = [0; 16];
    let write_value = 5_u64.to_le_bytes();
    let invalid_value = u64::MAX.to_le_bytes();
    let pollfd = pollfd_bytes(3, RISCV_LINUX_POLLIN | RISCV_LINUX_POLLOUT);
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x9000, &write_value),
            (0x9010, &invalid_value),
            (0x9020, &zeroes),
            (0x9030, &pollfd),
            (0x9040, &zeroes),
            (0x9050, &zeroes),
            (0x9060, &zeroes),
            (0x9070, &zeroes),
            (0x9200, &zeroes16),
            (0x9210, &zeroes16),
            (0x9220, &zeroes16),
            (0x9230, &zeroes16),
        ],
    )
}

fn pollfd_bytes(fd: i32, events: i16) -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    bytes[..4].copy_from_slice(&fd.to_le_bytes());
    bytes[4..6].copy_from_slice(&events.to_le_bytes());
    bytes
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
    handle_with_memory_option(state, number, arguments, reader, writer)
        .expect("syscall must be handled")
}

fn handle_with_memory_option(
    state: &mut RiscvSyscallState,
    number: u64,
    arguments: [u64; 6],
    reader: Option<&RiscvGuestMemoryReader>,
    writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, number, arguments),
        state,
        7,
        reader,
        writer,
    )
}

fn write_iovec(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64, base: u64, len: u64) {
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&base.to_le_bytes());
    bytes[8..].copy_from_slice(&len.to_le_bytes());
    assert!(guest_memory_writer(Arc::clone(store))(address, &bytes));
}

fn return_value(outcome: RiscvSyscallOutcome) -> u64 {
    match outcome {
        RiscvSyscallOutcome::Return { value } => value,
        outcome => panic!("unexpected syscall outcome: {outcome:?}"),
    }
}

#[test]
fn linux_table_eventfd2_counts_through_read_write_poll_and_close() {
    let store = eventfd_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [
            7,
            RISCV_LINUX_O_NONBLOCK | RISCV_LINUX_O_CLOEXEC,
            0,
            0,
            0,
            0,
        ],
    ));
    assert_eq!(fd, 3);
    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_FCNTL,
            [fd, RISCV_LINUX_F_GETFD, 0, 0, 0, 0]
        )),
        1
    );
    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_FCNTL,
            [fd, RISCV_LINUX_F_GETFL, 0, 0, 0, 0]
        )),
        RISCV_LINUX_O_RDWR | RISCV_LINUX_O_NONBLOCK
    );

    let requested =
        RISCV_LINUX_POLLIN | RISCV_LINUX_POLLOUT | RISCV_LINUX_POLLRDNORM | RISCV_LINUX_POLLWRNORM;
    let pollfd = pollfd_bytes(fd as i32, requested);
    assert!(guest_memory_writer(Arc::clone(&store))(0x9030, &pollfd));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9030, 1, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(pollfd_revents(&store, 0x9030), requested);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        )),
        8
    );
    assert_eq!(memory_u64(&store, 0x9020), 7);

    assert!(guest_memory_writer(Arc::clone(&store))(0x9030, &pollfd));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9030, 1, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(
        pollfd_revents(&store, 0x9030),
        RISCV_LINUX_POLLOUT | RISCV_LINUX_POLLWRNORM
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd, 0x9000, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        8
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        )),
        8
    );
    assert_eq!(memory_u64(&store, 0x9020), 5);
    assert!(state.guest_writes().is_empty());

    assert_eq!(
        return_value(handle(&mut state, RISCV_LINUX_CLOSE, [fd, 0, 0, 0, 0, 0])),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
}

#[test]
fn linux_table_eventfd2_semaphore_reads_one_ticket_at_a_time() {
    let store = eventfd_store();
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [
            2,
            RISCV_LINUX_EFD_SEMAPHORE | RISCV_LINUX_O_NONBLOCK,
            0,
            0,
            0,
            0,
        ],
    ));

    for expected in [1_u64, 1] {
        assert_eq!(
            return_value(handle_with_memory(
                &mut state,
                RISCV_LINUX_READ,
                [fd, 0x9020, 8, 0, 0, 0],
                None,
                Some(&writer),
            )),
            8
        );
        assert_eq!(memory_u64(&store, 0x9020), expected);
    }
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EAGAIN)
    );
}

#[test]
fn linux_table_eventfd2_rejects_invalid_flags_widths_and_forbidden_write_value() {
    let store = eventfd_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_EVENTFD2,
            [0, 1 << 17, 0, 0, 0, 0]
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_FCNTL,
            [3, RISCV_LINUX_F_GETFD, 0, 0, 0, 0]
        )),
        linux_error(RISCV_LINUX_EBADF)
    );

    let fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [0, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 4, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd, 0x9000, 4, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd, 0x9010, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
}

#[test]
fn linux_table_eventfd2_blocking_read_waits_until_nonblock_is_set() {
    let store = eventfd_store();
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let fd = return_value(handle(&mut state, RISCV_LINUX_EVENTFD2, [0, 0, 0, 0, 0, 0]));
    assert_eq!(
        handle_with_memory_option(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        ),
        None
    );

    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_FCNTL,
            [fd, RISCV_LINUX_F_SETFL, RISCV_LINUX_O_NONBLOCK, 0, 0, 0],
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EAGAIN)
    );
}

#[test]
fn linux_table_eventfd2_uses_32_bit_flags_and_shared_file_description_counter() {
    let store = eventfd_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [3, 1_u64 << 32, 0, 0, 0, 0],
    ));
    let dup_fd = return_value(handle(&mut state, RISCV_LINUX_DUP, [fd, 0, 0, 0, 0, 0]));

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [dup_fd, 0x9000, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        8
    );
    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_CLOSE,
            [dup_fd, 0, 0, 0, 0, 0]
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        )),
        8
    );
    assert_eq!(memory_u64(&store, 0x9020), 8);
}

#[test]
fn linux_table_eventfd2_vector_io_splits_counter_bytes_across_iovecs() {
    let store = eventfd_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [0x0102_0304, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    write_iovec(&store, 0x9200, 0x9040, 3);
    write_iovec(&store, 0x9210, 0x9050, 5);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READV,
            [fd, 0x9200, 2, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        8
    );
    let first = guest_memory_reader(Arc::clone(&store))(0x9040, 3).unwrap();
    let second = guest_memory_reader(Arc::clone(&store))(0x9050, 5).unwrap();
    assert_eq!([first, second].concat(), 0x0102_0304_u64.to_le_bytes());

    let value = 11_u64.to_le_bytes();
    assert!(guest_memory_writer(Arc::clone(&store))(0x9060, &value[..2]));
    assert!(guest_memory_writer(Arc::clone(&store))(0x9070, &value[2..]));
    write_iovec(&store, 0x9220, 0x9060, 2);
    write_iovec(&store, 0x9230, 0x9070, 6);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITEV,
            [fd, 0x9220, 2, 0, 0, 0],
            Some(&reader),
            None,
        )),
        8
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        )),
        8
    );
    assert_eq!(memory_u64(&store, 0x9020), 11);
}

#[test]
fn linux_table_eventfd2_max_counter_controls_poll_and_overflow_write() {
    let store = eventfd_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let max_counter = u64::MAX - 1;
    let one = 1_u64.to_le_bytes();

    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9040,
        &max_counter.to_le_bytes()
    ));
    assert!(guest_memory_writer(Arc::clone(&store))(0x9050, &one));
    let fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [0, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd, 0x9040, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        8
    );

    let requested =
        RISCV_LINUX_POLLIN | RISCV_LINUX_POLLOUT | RISCV_LINUX_POLLRDNORM | RISCV_LINUX_POLLWRNORM;
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9030,
        &pollfd_bytes(fd as i32, requested)
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9030, 1, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(
        pollfd_revents(&store, 0x9030),
        RISCV_LINUX_POLLIN | RISCV_LINUX_POLLRDNORM
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd, 0x9050, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EAGAIN)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9020, 8, 0, 0, 0],
            None,
            Some(&writer),
        )),
        8
    );
    assert_eq!(memory_u64(&store, 0x9020), max_counter);

    let blocking_fd = return_value(handle(&mut state, RISCV_LINUX_EVENTFD2, [0, 0, 0, 0, 0, 0]));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [blocking_fd, 0x9040, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        8
    );
    assert_eq!(
        handle_with_memory_option(
            &mut state,
            RISCV_LINUX_WRITE,
            [blocking_fd, 0x9050, 8, 0, 0, 0],
            Some(&reader),
            None,
        ),
        None
    );
}

#[test]
fn user_ecall_eventfd2_read_reaches_exit_path() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(56);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.data"),
                PartitionId::new(0),
                endpoint("l1d"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_data_core(
        0,
        0,
        7,
        "cpu0.ifetch",
        fetch_route,
        "cpu0.data",
        data_route,
        0x8000,
    );
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(10, 0, 9)),
            (0x8004, addi(11, 0, 0)),
            (0x8008, addi(17, 0, RISCV_LINUX_EVENTFD2 as i32)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(8, 10, 0)),
            (0x8014, addi(10, 8, 0)),
            (0x8018, lui(11, 9)),
            (0x801c, addi(12, 0, 8)),
            (0x8020, addi(17, 0, RISCV_LINUX_READ as i32)),
            (0x8024, 0x0000_0073),
            (0x8028, lui(11, 9)),
            (0x802c, ld(10, 11, 0)),
            (0x8030, addi(17, 0, RISCV_LINUX_EXIT as i32)),
            (0x8034, 0x0000_0073),
        ],
        &[(0x9000, &[0; 8])],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_and_guest_memory_io(
            guest_memory_reader(Arc::clone(&store)),
            guest_memory_writer(Arc::clone(&store)),
        );

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            160,
            |cpu| GuestEventId::new(560 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(560), source, 9);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(8)), 3);
    assert_eq!(core.read_register(reg(10)), 9);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
