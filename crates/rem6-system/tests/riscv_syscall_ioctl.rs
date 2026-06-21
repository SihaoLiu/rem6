#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_IOCTL: u64 = 29;
const RISCV_LINUX_PIPE2: u64 = 59;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_TCGETS: u64 = 0x5401;
const RISCV_LINUX_FIONREAD: u64 = 0x541b;
const RISCV_LINUX_ENOTTY: u64 = 25;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EFAULT: u64 = 14;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn expected_linux_termios_bytes() -> [u8; 36] {
    let mut bytes = [0; 36];
    bytes[0..4].copy_from_slice(&0x0000_0500_u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&0x0000_0005_u32.to_le_bytes());
    bytes[8..12].copy_from_slice(&0x0000_00bf_u32.to_le_bytes());
    bytes[12..16].copy_from_slice(&0x0000_8a3b_u32.to_le_bytes());
    bytes[17] = 0x03;
    bytes[18] = 0x1c;
    bytes[19] = 0x7f;
    bytes[20] = 0x15;
    bytes[21] = 0x04;
    bytes[23] = 0x01;
    bytes[25] = 0x11;
    bytes[26] = 0x13;
    bytes[27] = 0x1a;
    bytes[29] = 0x12;
    bytes[30] = 0x0f;
    bytes[31] = 0x17;
    bytes[32] = 0x16;
    bytes
}

fn read_guest_bytes(
    store: Arc<Mutex<PartitionedMemoryStore>>,
    address: u64,
    bytes: usize,
) -> Option<Vec<u8>> {
    let reader = guest_memory_reader(store);
    let mut data = Vec::with_capacity(bytes);
    for offset in 0..bytes {
        data.extend(reader(address + offset as u64, 1)?);
    }
    Some(data)
}

fn pipe_with_bytes() -> (
    Arc<Mutex<PartitionedMemoryStore>>,
    RiscvSyscallState,
    u64,
    u64,
) {
    let fd_array = [0_u8; 8];
    let count_area = [0_u8; 4];
    let store = loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &fd_array),
            (0x9000, b"ready"),
            (0x9100, &count_area),
        ],
    );
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = guest_memory_reader(Arc::clone(&store))(0x8800, 8).unwrap();
    let read_fd = i32::from_le_bytes(fds[..4].try_into().unwrap()) as u64;
    let write_fd = i32::from_le_bytes(fds[4..].try_into().unwrap()) as u64;

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [write_fd, 0x9000, 5, 0, 0, 0]),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );

    (store, state, read_fd, write_fd)
}

#[test]
fn linux_table_ioctl_tcgets_reports_standard_fd_termios() {
    let store = loaded_program_store_with_data(&[(0x8000, 0)], &[(0x9000, &[0xff; 36])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_IOCTL,
            [1, RISCV_LINUX_TCGETS, 0x9000, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    assert_eq!(
        read_guest_bytes(Arc::clone(&store), 0x9000, 36),
        Some(expected_linux_termios_bytes().to_vec())
    );
}

#[test]
fn linux_table_ioctl_fionread_reports_standard_fd_is_not_tty() {
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_IOCTL,
            [0, RISCV_LINUX_FIONREAD, 0x9000, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        None,
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(RISCV_LINUX_ENOTTY),
        })
    );
}

#[test]
fn linux_table_ioctl_fionread_reports_pipe_buffer_bytes() {
    let (store, mut state, read_fd, _write_fd) = pipe_with_bytes();
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_IOCTL,
                [read_fd, RISCV_LINUX_FIONREAD, 0x9100, 0, 0, 0],
            ),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 4),
        Some(5_i32.to_le_bytes().to_vec())
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_ioctl_fionread_reports_pipe_write_end_buffer_bytes() {
    let (store, mut state, _read_fd, write_fd) = pipe_with_bytes();
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_IOCTL,
                [write_fd, RISCV_LINUX_FIONREAD, 0x9100, 0, 0, 0],
            ),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 4),
        Some(5_i32.to_le_bytes().to_vec())
    );
}

#[test]
fn linux_table_ioctl_fionread_faulting_guest_count_pointer_returns_efault() {
    let (_store, mut state, read_fd, _write_fd) = pipe_with_bytes();
    let faulting_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_IOCTL,
                [read_fd, RISCV_LINUX_FIONREAD, 0x9100, 0, 0, 0],
            ),
            &mut state,
            2,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_ioctl_unknown_request_on_valid_fd_returns_enotty() {
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_IOCTL, [2, 0xdead, 0, 0, 0, 0]),
        &mut state,
        0,
        None,
        None,
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(RISCV_LINUX_ENOTTY),
        })
    );
}

#[test]
fn linux_table_ioctl_invalid_fd_returns_ebadf() {
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_IOCTL,
            [99, RISCV_LINUX_TCGETS, 0, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        None,
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(RISCV_LINUX_EBADF),
        })
    );
}

#[test]
fn user_ecall_ioctl_tcgets_writes_termios_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(78);
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, RISCV_LINUX_IOCTL as i32)),
            (0x8004, addi(10, 0, 1)),
            (0x8008, lui(11, 5)),
            (0x800c, addi(11, 11, 0x401)),
            (0x8010, lui(12, 9)),
            (0x8014, 0x0000_0073),
            (0x8018, addi(5, 10, 0)),
            (0x801c, addi(17, 0, 93)),
            (0x8020, addi(10, 0, 0)),
            (0x8024, 0x0000_0073),
        ],
        &[(0x9000, &[0xff; 36])],
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
        .with_riscv_syscall_emulation()
        .with_riscv_syscall_emulation_and_guest_memory_writer(guest_memory_writer(Arc::clone(
            &store,
        )));

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            90,
            |cpu| GuestEventId::new(600 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(600), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(
        read_guest_bytes(Arc::clone(&store), 0x9000, 36),
        Some(expected_linux_termios_bytes().to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
