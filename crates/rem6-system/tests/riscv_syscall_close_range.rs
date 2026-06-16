#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState, RiscvSyscallTable};
use support::*;

const RISCV_LINUX_DUP: u64 = 23;
const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_CLOSE_RANGE: u64 = 436;
const RISCV_LINUX_F_DUPFD: u64 = 0;
const RISCV_LINUX_F_GETFD: u64 = 1;
const RISCV_LINUX_CLOSE_RANGE_CLOEXEC: u64 = 4;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EINVAL: u64 = 22;

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

fn handle_syscall(
    state: &mut RiscvSyscallState,
    pc: u64,
    number: u64,
    arguments: [u64; 6],
) -> RiscvSyscallOutcome {
    RiscvSyscallTable::new()
        .handle(RiscvSyscallRequest::new(pc, number, arguments), state)
        .unwrap_or_else(|| panic!("syscall {number} at {pc:#x} was not handled"))
}

fn duplicate_stdout_from(state: &mut RiscvSyscallState, pc: u64, minimum_fd: u64) -> u64 {
    match handle_syscall(
        state,
        pc,
        RISCV_LINUX_FCNTL,
        [1, RISCV_LINUX_F_DUPFD, minimum_fd, 0, 0, 0],
    ) {
        RiscvSyscallOutcome::Return { value } => value,
        outcome => panic!("unexpected duplicate outcome: {outcome:?}"),
    }
}

fn get_fd_flags(state: &mut RiscvSyscallState, pc: u64, fd: u64) -> u64 {
    match handle_syscall(
        state,
        pc,
        RISCV_LINUX_FCNTL,
        [fd, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
    ) {
        RiscvSyscallOutcome::Return { value } => value,
        outcome => panic!("unexpected getfd outcome: {outcome:?}"),
    }
}

#[test]
fn user_ecall_close_range_closes_duplicate_fd_before_exit() {
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
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(10, 0, 1)),
        (0x8004, addi(17, 0, RISCV_LINUX_DUP as i32)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(6, 10, 0)),
        (0x8010, addi(11, 6, 0)),
        (0x8014, addi(12, 0, 0)),
        (0x8018, addi(17, 0, RISCV_LINUX_CLOSE_RANGE as i32)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(7, 10, 0)),
        (0x8024, addi(10, 6, 0)),
        (0x8028, addi(11, 0, 1)),
        (0x802c, addi(17, 0, RISCV_LINUX_FCNTL as i32)),
        (0x8030, 0x0000_0073),
        (0x8034, addi(5, 10, 0)),
        (0x8038, addi(17, 0, RISCV_LINUX_EXIT as i32)),
        (0x803c, addi(10, 0, 0)),
        (0x8040, 0x0000_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation();

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
            |cpu| GuestEventId::new(440 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(440), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(6)), 3);
    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.read_register(reg(5)), linux_error(RISCV_LINUX_EBADF));
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn linux_table_close_range_cloexec_marks_range_without_closing() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(duplicate_stdout_from(&mut state, 0x8000, 7), 7);
    assert_eq!(
        handle_syscall(
            &mut state,
            0x8004,
            RISCV_LINUX_CLOSE_RANGE,
            [7, 7, RISCV_LINUX_CLOSE_RANGE_CLOEXEC, 0, 0, 0],
        ),
        RiscvSyscallOutcome::Return { value: 0 }
    );
    assert_eq!(get_fd_flags(&mut state, 0x8008, 7), 1);
}

#[test]
fn linux_table_close_range_closes_through_unsigned_max_bound() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(duplicate_stdout_from(&mut state, 0x8000, 7), 7);
    assert_eq!(duplicate_stdout_from(&mut state, 0x8004, 8), 8);
    assert_eq!(
        handle_syscall(
            &mut state,
            0x8008,
            RISCV_LINUX_CLOSE_RANGE,
            [7, u64::from(u32::MAX), 0, 0, 0, 0],
        ),
        RiscvSyscallOutcome::Return { value: 0 }
    );
    for (pc, fd) in [(0x800c, 7), (0x8010, 8)] {
        assert_eq!(
            get_fd_flags(&mut state, pc, fd),
            linux_error(RISCV_LINUX_EBADF)
        );
    }
    assert_eq!(get_fd_flags(&mut state, 0x8014, 1), 0);
}

#[test]
fn linux_table_close_range_rejects_invalid_arguments_without_mutating_fds() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_syscall(
            &mut state,
            0x8000,
            RISCV_LINUX_CLOSE_RANGE,
            [1, 1, RISCV_LINUX_CLOSE_RANGE_CLOEXEC << 1, 0, 0, 0,],
        ),
        RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        }
    );
    assert_eq!(
        handle_syscall(
            &mut state,
            0x8004,
            RISCV_LINUX_CLOSE_RANGE,
            [2, 1, 0, 0, 0, 0],
        ),
        RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        }
    );
    assert_eq!(get_fd_flags(&mut state, 0x8008, 1), 0);
}
