#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RiscvSyscallTable, RiscvSystemRunStopReason,
};
use support::*;

const RISCV_LINUX_EVENTFD2: u64 = 19;
const RISCV_LINUX_DUP3: u64 = 24;
const RISCV_LINUX_PSELECT6: u64 = 72;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn pselect_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let read_eventfd = fdset_bytes(&[3], 8);
    let read_high_fd = fdset_bytes(&[1024], 136);
    let read_stdin = fdset_bytes(&[0], 8);
    let read_invalid = fdset_bytes(&[99], 16);
    let except_eventfd = fdset_bytes(&[3], 8);
    let zero_timeout = [0_u8; 16];
    let sigset = [0xa5_u8; 8];
    let invalid_sigmask = sigmask_pair(0x9060, 4);
    let invalid_null_sigmask = sigmask_pair(0, 4);
    let valid_sigmask = sigmask_pair(0x9060, 8);
    let finite_timeout = finite_timespec(0, 1);
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x9000, &read_eventfd),
            (0x9010, &except_eventfd),
            (0x9020, &read_stdin),
            (0x9030, &zero_timeout),
            (0x9040, &read_invalid),
            (0x9050, &invalid_sigmask),
            (0x9060, &sigset),
            (0x9070, &valid_sigmask),
            (0x9080, &read_high_fd),
            (0x9200, &finite_timeout),
            (0x9210, &invalid_null_sigmask),
        ],
    )
}

fn fdset_bytes(fds: &[u64], bytes: usize) -> Vec<u8> {
    let mut fdset = vec![0_u8; bytes];
    for fd in fds {
        let byte = usize::try_from(fd / 8).unwrap();
        let bit = (fd % 8) as u8;
        fdset[byte] |= 1 << bit;
    }
    fdset
}

fn sigmask_pair(sigset_address: u64, sigset_bytes: u64) -> Vec<u8> {
    let mut pair = Vec::with_capacity(16);
    pair.extend_from_slice(&sigset_address.to_le_bytes());
    pair.extend_from_slice(&sigset_bytes.to_le_bytes());
    pair
}

fn finite_timespec(seconds: i64, nanoseconds: i64) -> Vec<u8> {
    let mut timespec = Vec::with_capacity(16);
    timespec.extend_from_slice(&seconds.to_le_bytes());
    timespec.extend_from_slice(&nanoseconds.to_le_bytes());
    timespec
}

fn fdset_word(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> u64 {
    let bytes = guest_memory_reader(Arc::clone(store))(address, 8).unwrap();
    u64::from_le_bytes(bytes.try_into().unwrap())
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
fn linux_table_pselect6_reports_eventfd_readiness_and_clears_except_set() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let event_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [7, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    assert_eq!(event_fd, 3);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [4, 0x9000, 0, 0x9010, 0x9030, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(fdset_word(&store, 0x9000), 1 << event_fd);
    assert_eq!(fdset_word(&store, 0x9010), 0);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_accepts_fd_above_fd_setsize() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let event_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [7, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    assert_eq!(event_fd, 3);
    assert_eq!(
        return_value(handle(
            &mut state,
            RISCV_LINUX_DUP3,
            [event_fd, 1024, 0, 0, 0, 0],
        )),
        1024
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [1025, 0x9080, 0, 0, 0x9030, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(fdset_word(&store, 0x9100), 1);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_zero_timeout_clears_unready_stdin() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [1, 0x9020, 0, 0, 0x9030, 0],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    assert_eq!(fdset_word(&store, 0x9020), 0);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_positive_timeout_returns_ready_fd_without_waiting() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let event_fd = return_value(handle(
        &mut state,
        RISCV_LINUX_EVENTFD2,
        [7, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
    ));
    assert_eq!(event_fd, 3);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [4, 0x9000, 0, 0, 0x9200, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(fdset_word(&store, 0x9000), 1 << event_fd);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_positive_timeout_blocks_when_no_fd_is_ready() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_with_memory_option(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [1, 0x9020, 0, 0, 0x9200, 0],
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(fdset_word(&store, 0x9020), 1);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_all_null_fdsets_do_not_require_guest_writer() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [4, 0, 0, 0, 0x9030, 0],
            Some(&reader),
            None,
        )),
        0
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_reads_timeout_before_rejecting_negative_nfds() {
    let faulting_reader = RiscvGuestMemoryReader::new(|_, _| None);
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [u64::MAX, 0, 0, 0, 0xdead, 0],
            Some(&faulting_reader),
            None,
        )),
        linux_error(RISCV_LINUX_EFAULT)
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_missing_reader_does_not_forge_guest_fault() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_with_memory_option(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [u64::MAX, 0, 0, 0, 0xdead, 0],
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_validates_sigmask_pair() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [4, 0, 0, 0, 0x9030, 0x9050],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [4, 0, 0, 0, 0x9030, 0x9210],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [4, 0, 0, 0, 0x9030, 0x9070],
            Some(&reader),
            None,
        )),
        0
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_null_timeout_blocks_when_no_fd_is_ready() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_with_memory_option(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [1, 0x9020, 0, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(fdset_word(&store, 0x9020), 1);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pselect6_rejects_bad_fd_and_faulting_output_without_partial_write() {
    let store = pselect_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [100, 0x9040, 0, 0, 0x9030, 0],
            Some(&reader),
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
    assert_eq!(fdset_word(&store, 0x9048), 1_u64 << (99 % 64));

    let faulting_writer =
        RiscvGuestMemoryWriter::new(|_address, _bytes| false).with_write_probe(|_, _| false);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [100, 0x9040, 0, 0, 0x9030, 0],
            Some(&reader),
            Some(&faulting_writer),
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
    assert_eq!(fdset_word(&store, 0x9048), 1_u64 << (99 % 64));

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PSELECT6,
            [1, 0x9020, 0, 0, 0x9030, 0],
            Some(&reader),
            Some(&faulting_writer),
        )),
        linux_error(RISCV_LINUX_EFAULT)
    );
    assert_eq!(fdset_word(&store, 0x9020), 1);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn user_ecall_pselect6_observes_eventfd_readiness_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(58);
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
    let fdset = fdset_bytes(&[3], 8);
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(10, 0, 7)),
            (0x8004, addi(11, 0, 0)),
            (0x8008, addi(17, 0, RISCV_LINUX_EVENTFD2 as i32)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(10, 0, 4)),
            (0x8014, lui(11, 9)),
            (0x8018, addi(12, 0, 0)),
            (0x801c, addi(13, 0, 0)),
            (0x8020, addi(14, 0, 0)),
            (0x8024, addi(15, 0, 0)),
            (0x8028, addi(17, 0, RISCV_LINUX_PSELECT6 as i32)),
            (0x802c, 0x0000_0073),
            (0x8030, addi(17, 0, RISCV_LINUX_EXIT as i32)),
            (0x8034, 0x0000_0073),
        ],
        &[(0x9000, &fdset)],
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
            120,
            |cpu| GuestEventId::new(580 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(580), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(10)), 1);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_pselect6_blocking_no_ready_stalls_without_host_trap() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(59);
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
    let fdset = fdset_bytes(&[0], 8);
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(10, 0, 1)),
            (0x8004, lui(11, 9)),
            (0x8008, addi(12, 0, 0)),
            (0x800c, addi(13, 0, 0)),
            (0x8010, addi(14, 0, 0)),
            (0x8014, addi(15, 0, 0)),
            (0x8018, addi(17, 0, RISCV_LINUX_PSELECT6 as i32)),
            (0x801c, 0x0000_0073),
            (0x8020, addi(17, 0, RISCV_LINUX_EXIT as i32)),
            (0x8024, 0x0000_0073),
        ],
        &[(0x9000, &fdset)],
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
            120,
            |cpu| GuestEventId::new(590 + u64::from(cpu.get())),
        )
        .unwrap();

    assert!(matches!(
        run.stop_reason(),
        RiscvSystemRunStopReason::Idle { .. }
    ));
    assert!(run.host_stop().is_none());
    assert!(run.scheduled_traps().is_empty());
    assert!(core.has_pending_trap());
    assert_eq!(fdset_word(&store, 0x9000), 1);
}
