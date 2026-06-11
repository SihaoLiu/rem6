#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState, RiscvSyscallTable};
use support::*;

const RISCV_LINUX_IOCTL: u64 = 29;
const RISCV_LINUX_TCGETS: u64 = 0x5401;
const RISCV_LINUX_FIONREAD: u64 = 0x541b;
const RISCV_LINUX_ENOTTY: u64 = 25;
const RISCV_LINUX_EBADF: u64 = 9;

#[test]
fn linux_table_ioctl_tcgets_reports_standard_fd_is_not_tty() {
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_IOCTL,
            [1, RISCV_LINUX_TCGETS, 0, 0, 0, 0],
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
fn user_ecall_ioctl_tcgets_returns_enotty_before_exit() {
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
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, RISCV_LINUX_IOCTL as i32)),
        (0x8004, addi(10, 0, 1)),
        (0x8008, lui(11, 5)),
        (0x800c, addi(11, 11, 0x401)),
        (0x8010, addi(12, 0, 0)),
        (0x8014, 0x0000_0073),
        (0x8018, addi(5, 10, 0)),
        (0x801c, addi(17, 0, 93)),
        (0x8020, addi(10, 0, 0)),
        (0x8024, 0x0000_0073),
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
            90,
            |cpu| GuestEventId::new(600 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(600), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(
        core.read_register(reg(5)),
        0u64.wrapping_sub(RISCV_LINUX_ENOTTY)
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
