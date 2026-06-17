#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    GuestFd, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome,
    RiscvSyscallRequest, RiscvSyscallState, RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_EVENTFD2: u64 = 19;
const RISCV_LINUX_EPOLL_CREATE1: u64 = 20;
const RISCV_LINUX_EPOLL_CTL: u64 = 21;
const RISCV_LINUX_EPOLL_PWAIT: u64 = 22;
const RISCV_LINUX_DUP: u64 = 23;
const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_F_GETFD: u64 = 1;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EEXIST: u64 = 17;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_ENOENT: u64 = 2;
const RISCV_LINUX_O_CLOEXEC: u64 = 0o2_000_000;
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_EPOLL_CTL_ADD: u64 = 1;
const RISCV_LINUX_EPOLL_CTL_DEL: u64 = 2;
const RISCV_LINUX_EPOLL_CTL_MOD: u64 = 3;
const RISCV_LINUX_EPOLLIN: u32 = 0x0001;
const RISCV_LINUX_EPOLLOUT: u32 = 0x0004;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn epoll_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let write_value = 5_u64.to_le_bytes();
    let add_event = epoll_event_bytes(RISCV_LINUX_EPOLLIN, 0x55aa);
    let out_event = epoll_event_bytes(RISCV_LINUX_EPOLLOUT, 0x66bb);
    let zero_events = [0_u8; 24];
    let edge_event = [0_u8; 16];
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x9000, &write_value),
            (0x9010, &add_event),
            (0x9020, &out_event),
            (0x9030, &zero_events),
            (0x9050, &zero_events),
            (0x9ff0, &edge_event),
        ],
    )
}

fn epoll_event_bytes(events: u32, data: u64) -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    bytes[..4].copy_from_slice(&events.to_le_bytes());
    bytes[8..].copy_from_slice(&data.to_le_bytes());
    bytes
}

fn epoll_event_at(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> (u32, u64) {
    let bytes = guest_memory_reader(Arc::clone(store))(address, 16).unwrap();
    let events = u32::from_le_bytes(bytes[..4].try_into().unwrap());
    let data = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    (events, data)
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

fn return_value(outcome: RiscvSyscallOutcome) -> u64 {
    match outcome {
        RiscvSyscallOutcome::Return { value } => value,
        outcome => panic!("unexpected syscall outcome: {outcome:?}"),
    }
}

#[test]
fn linux_table_epoll_create1_validates_flags_and_close_on_exec() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_EPOLL_CREATE1,
            [RISCV_LINUX_O_CLOEXEC, 0, 0, 0, 0, 0],
        )),
        3
    );
    let epfd = GuestFd::new(3).unwrap();
    assert!(state.guest_fds().close_on_exec(epfd).unwrap());
    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_FCNTL,
            [3, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
        )),
        1
    );
    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_EPOLL_CREATE1,
            [RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0, 0],
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
}

#[test]
fn linux_table_epoll_ctl_and_pwait_report_registered_eventfd_readiness() {
    let store = epoll_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let event_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [0, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    let epfd = return_value(handle(
        &mut state,
        RISCV_LINUX_EPOLL_CREATE1,
        [0, 0, 0, 0, 0, 0],
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_ADD, event_fd, 0x9010, 0, 0,],
            Some(&reader),
            None,
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 2, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [event_fd, 0x9000, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        8
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 2, 0, 0, 0],
            None,
            Some(&writer),
        )),
        1
    );
    assert_eq!(
        epoll_event_at(&store, 0x9030),
        (RISCV_LINUX_EPOLLIN, 0x55aa)
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_MOD, event_fd, 0x9020, 0, 0,],
            Some(&reader),
            None,
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 2, 0, 0, 0],
            None,
            Some(&writer),
        )),
        1
    );
    assert_eq!(
        epoll_event_at(&store, 0x9030),
        (RISCV_LINUX_EPOLLOUT, 0x66bb)
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_DEL, event_fd, 0, 0, 0,],
            None,
            None,
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 2, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
}

#[test]
fn linux_table_epoll_interest_survives_dup_close_until_file_description_releases() {
    let store = epoll_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let event_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [0, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    let epfd = return_value(handle(
        &mut state,
        RISCV_LINUX_EPOLL_CREATE1,
        [0, 0, 0, 0, 0, 0],
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_ADD, event_fd, 0x9010, 0, 0,],
            Some(&reader),
            None,
        )),
        0
    );

    let dup_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_DUP,
        [event_fd, 0, 0, 0, 0, 0],
    ));
    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_CLOSE,
            [event_fd, 0, 0, 0, 0, 0],
        )),
        0
    );
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
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        1
    );
    assert_eq!(
        epoll_event_at(&store, 0x9030),
        (RISCV_LINUX_EPOLLIN, 0x55aa)
    );

    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_CLOSE,
            [dup_fd, 0, 0, 0, 0, 0],
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
}

#[test]
fn linux_table_epoll_rejects_bad_operations_without_mutating_registry() {
    let store = epoll_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let event_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [1, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    let other_event_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [1, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    let epfd = return_value(handle(
        &mut state,
        RISCV_LINUX_EPOLL_CREATE1,
        [0, 0, 0, 0, 0, 0],
    ));

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_MOD, event_fd, 0x9010, 0, 0,],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_ENOENT)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [
                epfd,
                RISCV_LINUX_EPOLL_CTL_ADD,
                other_event_fd + 99,
                0x9010,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_ADD, event_fd, 0x7000, 0, 0,],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EFAULT)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_ADD, event_fd, 0x9010, 0, 0,],
            Some(&reader),
            None,
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_ADD, event_fd, 0x9010, 0, 0,],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EEXIST)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_DEL, other_event_fd, 0, 0, 0,],
            None,
            None,
        )),
        linux_error(RISCV_LINUX_ENOENT)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [event_fd, RISCV_LINUX_EPOLL_CTL_ADD, epfd, 0x9010, 0, 0,],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_ADD, epfd, 0x9010, 0, 0,],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        1
    );
}

#[test]
fn linux_table_epoll_pwait_validates_memory_and_blocking_timeout() {
    let store = epoll_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let range_probe_store = Arc::clone(&store);
    let range_writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)))
        .with_write_probe(move |address, bytes| {
            guest_memory_reader(Arc::clone(&range_probe_store))(address, bytes).is_some()
        });
    let mut state = RiscvSyscallState::new(0);
    let event_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [0, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    let epfd = return_value(handle(
        &mut state,
        RISCV_LINUX_EPOLL_CREATE1,
        [0, 0, 0, 0, 0, 0],
    ));

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_CTL,
            [epfd, RISCV_LINUX_EPOLL_CTL_ADD, event_fd, 0x9010, 0, 0,],
            Some(&reader),
            None,
        )),
        0
    );
    assert_eq!(
        handle_with_memory_option(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 1, u64::MAX, 0, 0],
            None,
            Some(&writer),
        ),
        None
    );
    let sentinel_event = [0xa5_u8; 16];
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9030,
        &sentinel_event
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9030, 16).unwrap(),
        sentinel_event
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9ff0, 2, 0, 0, 0],
            None,
            Some(&range_writer),
        )),
        linux_error(RISCV_LINUX_EFAULT)
    );
    assert!(guest_memory_writer(Arc::clone(&store))(
        0x9ff0,
        &sentinel_event
    ));
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x7000, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EFAULT)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [event_fd, 0x9000, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        8
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9ff0, 2, 0, 0, 0],
            None,
            Some(&range_writer),
        )),
        linux_error(RISCV_LINUX_EFAULT)
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9ff0, 16).unwrap(),
        sentinel_event
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x7000, 1, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EFAULT)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 0, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
}

#[test]
fn linux_table_epoll_pwait_accepts_large_positive_maxevents() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true)
        .with_write_probe(|_address, _bytes| true);
    let mut state = RiscvSyscallState::new(0);
    let epfd = return_value(handle(
        &mut state,
        RISCV_LINUX_EPOLL_CREATE1,
        [0, 0, 0, 0, 0, 0],
    ));

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_EPOLL_PWAIT,
            [epfd, 0x9030, 1025, 0, 0, 0],
            None,
            Some(&writer),
        )),
        0
    );
}

#[test]
fn user_ecall_epoll_pwait_observes_eventfd_readiness_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(57);
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
    let add_event = epoll_event_bytes(RISCV_LINUX_EPOLLIN, 0x77);
    let empty_events = [0_u8; 16];
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(10, 0, 7)),
            (0x8004, addi(11, 0, 0)),
            (0x8008, addi(17, 0, RISCV_LINUX_EVENTFD2 as i32)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(8, 10, 0)),
            (0x8014, addi(10, 0, 0)),
            (0x8018, addi(17, 0, RISCV_LINUX_EPOLL_CREATE1 as i32)),
            (0x801c, 0x0000_0073),
            (0x8020, addi(9, 10, 0)),
            (0x8024, addi(10, 9, 0)),
            (0x8028, addi(11, 0, RISCV_LINUX_EPOLL_CTL_ADD as i32)),
            (0x802c, addi(12, 8, 0)),
            (0x8030, lui(13, 9)),
            (0x8034, addi(17, 0, RISCV_LINUX_EPOLL_CTL as i32)),
            (0x8038, 0x0000_0073),
            (0x803c, addi(10, 9, 0)),
            (0x8040, lui(11, 9)),
            (0x8044, addi(11, 11, 0x10)),
            (0x8048, addi(12, 0, 1)),
            (0x804c, addi(13, 0, 0)),
            (0x8050, addi(14, 0, 0)),
            (0x8054, addi(15, 0, 0)),
            (0x8058, addi(17, 0, RISCV_LINUX_EPOLL_PWAIT as i32)),
            (0x805c, 0x0000_0073),
            (0x8060, addi(17, 0, RISCV_LINUX_EXIT as i32)),
            (0x8064, 0x0000_0073),
        ],
        &[(0x9000, &add_event), (0x9010, &empty_events)],
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
            200,
            |cpu| GuestEventId::new(570 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(570), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(8)), 3);
    assert_eq!(core.read_register(reg(9)), 4);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
