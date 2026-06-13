#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_PRLIMIT64: u64 = 261;
const RISCV_LINUX_RLIMIT_DATA: u64 = 2;
const RISCV_LINUX_RLIMIT_STACK: u64 = 3;
const RISCV_LINUX_RLIMIT_NPROC: u64 = 6;
const RISCV_LINUX_STACK_LIMIT_BYTES: u64 = 8 * 1024 * 1024;
const RISCV_LINUX_DATA_LIMIT_BYTES: u64 = 256 * 1024 * 1024;

#[test]
fn linux_table_prlimit64_writes_stack_limit() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_PRLIMIT64,
            [0, RISCV_LINUX_RLIMIT_STACK, 0, 0x9000, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), RISCV_LINUX_STACK_LIMIT_BYTES);
    assert_eq!(read_u64(&bytes, 8), RISCV_LINUX_STACK_LIMIT_BYTES);
}

#[test]
fn linux_table_prlimit64_writes_data_limit() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_PRLIMIT64,
            [0, RISCV_LINUX_RLIMIT_DATA, 0, 0x9000, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), RISCV_LINUX_DATA_LIMIT_BYTES);
    assert_eq!(read_u64(&bytes, 8), RISCV_LINUX_DATA_LIMIT_BYTES);
}

#[test]
fn linux_table_prlimit64_accepts_current_process_id() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9000, &[0xff; 16])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_PRLIMIT64,
            [100, RISCV_LINUX_RLIMIT_STACK, 0, 0x9000, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), RISCV_LINUX_STACK_LIMIT_BYTES);
    assert_eq!(read_u64(&bytes, 8), RISCV_LINUX_STACK_LIMIT_BYTES);
}

#[test]
fn linux_table_prlimit64_rejects_nonzero_pid_without_writing() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("nonzero pid should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_PRLIMIT64,
            [99, RISCV_LINUX_RLIMIT_STACK, 0, 0x9000, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(1),
        })
    );
}

#[test]
fn linux_table_prlimit64_rejects_unsupported_resource_without_writing() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("unsupported resource should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PRLIMIT64, [0, 7, 0, 0x9000, 0, 0]),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(22),
        })
    );
}

#[test]
fn linux_table_prlimit64_rejects_nproc_resource_without_writing() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("unsupported resource should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_PRLIMIT64,
            [0, RISCV_LINUX_RLIMIT_NPROC, 0, 0x9000, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(22),
        })
    );
}

#[test]
fn linux_table_prlimit64_reports_guest_write_fault() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_PRLIMIT64,
            [0, RISCV_LINUX_RLIMIT_STACK, 0, 0x9000, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: 0u64.wrapping_sub(14),
        })
    );
}

#[test]
fn linux_table_prlimit64_allows_null_old_limit() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("null old limit should not write guest memory")
    });
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_PRLIMIT64,
            [0, RISCV_LINUX_RLIMIT_STACK, 0x9100, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
}

#[test]
fn linux_table_prlimit64_allows_null_old_limit_without_guest_writer() {
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_PRLIMIT64,
            [0, RISCV_LINUX_RLIMIT_STACK, 0x9100, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        None,
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 0 }));
}

#[test]
fn user_ecall_prlimit64_writes_stack_limit_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(77);
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
            (0x8000, addi(17, 0, RISCV_LINUX_PRLIMIT64 as i32)),
            (0x8004, addi(10, 0, 0)),
            (0x8008, addi(11, 0, RISCV_LINUX_RLIMIT_STACK as i32)),
            (0x800c, addi(12, 0, 0)),
            (0x8010, lui(13, 9)),
            (0x8014, 0x0000_0073),
            (0x8018, addi(5, 10, 0)),
            (0x801c, addi(17, 0, 93)),
            (0x8020, addi(10, 5, 0)),
            (0x8024, 0x0000_0073),
        ],
        &[(0x9000, &[0xff; 16])],
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
            |cpu| GuestEventId::new(580 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(580), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    let bytes = guest_memory_reader(Arc::clone(&store))(0x9000, 16).unwrap();
    assert_eq!(read_u64(&bytes, 0), RISCV_LINUX_STACK_LIMIT_BYTES);
    assert_eq!(read_u64(&bytes, 8), RISCV_LINUX_STACK_LIMIT_BYTES);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let mut value = [0; 8];
    value.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(value)
}
