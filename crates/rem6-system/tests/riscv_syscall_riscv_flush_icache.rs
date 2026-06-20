#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState, RiscvSyscallTable};
use support::*;

const RISCV_LINUX_RISCV_FLUSH_ICACHE: u64 = 259;
const RISCV_LINUX_EINVAL: u64 = 22;

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

fn handle_flush_icache(start: u64, end: u64, flags: u64) -> RiscvSyscallOutcome {
    let mut state = RiscvSyscallState::new(0);
    RiscvSyscallTable::new()
        .handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RISCV_FLUSH_ICACHE,
                [start, end, flags, 0, 0, 0],
            ),
            &mut state,
        )
        .expect("riscv_flush_icache must be handled")
}

#[test]
fn linux_table_riscv_flush_icache_accepts_global_and_local_flags() {
    assert_eq!(
        handle_flush_icache(0, 0, 0),
        RiscvSyscallOutcome::Return { value: 0 }
    );
    assert_eq!(
        handle_flush_icache(0x1000, 0x2000, 1),
        RiscvSyscallOutcome::Return { value: 0 }
    );
}

#[test]
fn linux_table_riscv_flush_icache_rejects_reserved_flags() {
    assert_eq!(
        handle_flush_icache(0x1000, 0x2000, 2),
        RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        }
    );
}

#[test]
fn user_ecall_riscv_flush_icache_returns_zero_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(82);
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
        (0x8000, addi(17, 0, RISCV_LINUX_RISCV_FLUSH_ICACHE as i32)),
        (0x8004, lui(10, 1)),
        (0x8008, lui(11, 2)),
        (0x800c, addi(12, 0, 1)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(5, 10, 0)),
        (0x8018, addi(17, 0, 93)),
        (0x801c, addi(10, 5, 0)),
        (0x8020, 0x0000_0073),
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
            |cpu| GuestEventId::new(640 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(640), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
