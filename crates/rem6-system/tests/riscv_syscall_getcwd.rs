#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod riscv_syscall_emulation_support;

use rem6_system::{
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};
use riscv_syscall_emulation_support::*;

const RISCV_LINUX_GETCWD: u64 = 17;
const RISCV_LINUX_EFAULT_RETURN: u64 = (-14_i64) as u64;
const RISCV_LINUX_ERANGE_RETURN: u64 = (-34_i64) as u64;

fn collect_guest_writes(writes: &[(u64, Vec<u8>)], base: u64, len: usize) -> Vec<u8> {
    let mut bytes = vec![0; len];
    for (address, chunk) in writes {
        let offset = usize::try_from(address.checked_sub(base).unwrap()).unwrap();
        bytes[offset..offset + chunk.len()].copy_from_slice(chunk);
    }
    bytes
}

#[test]
fn linux_table_getcwd_writes_default_target_cwd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writes_for_writer = Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCWD, [0x9000, 8, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );

    assert_eq!(
        collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8),
        b"/\0\0\0\0\0\0\0".to_vec()
    );
}

#[test]
fn linux_table_getcwd_writes_modeled_target_cwd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.set_current_directory(b"/bench/spec");
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writes_for_writer = Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCWD, [0x9000, 16, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 11 })
    );

    assert_eq!(
        collect_guest_writes(&writes.lock().unwrap(), 0x9000, 16),
        b"/bench/spec\0\0\0\0\0".to_vec()
    );
}

#[test]
fn linux_table_getcwd_requires_guest_memory_writer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCWD, [0x9000, 8, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            None,
        ),
        None
    );
}

#[test]
fn linux_table_getcwd_returns_erange_when_buffer_cannot_hold_nul() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCWD, [0x9000, 1, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_ERANGE_RETURN
        })
    );
}

#[test]
fn linux_table_getcwd_returns_efault_when_guest_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETCWD, [0x9000, 8, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_EFAULT_RETURN
        })
    );
}

#[test]
fn linux_table_getcwd_streams_huge_buffer_until_guest_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let calls = Arc::new(Mutex::new(0_u64));
    let calls_for_writer = Arc::clone(&calls);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |_address, _bytes| {
        *calls_for_writer.lock().unwrap() += 1;
        false
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_GETCWD,
                [0x9000, usize::MAX as u64, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_EFAULT_RETURN
        })
    );
    assert_eq!(*calls.lock().unwrap(), 1);
}

#[test]
fn user_ecall_getcwd_writes_cross_line_buffer_and_resumes() {
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
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
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
        "cpu0.dmem",
        data_route,
        0x8000,
    );
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 17)),
            (0x8004, lui(10, 9)),
            (0x8008, addi(10, 10, 8)),
            (0x800c, addi(11, 0, 32)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(5, 10, 0)),
            (0x8018, addi(17, 0, 93)),
            (0x801c, addi(10, 0, 0)),
            (0x8020, 0x0000_0073),
        ],
        &[(0x9008, &[0xaa; 32])],
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
            110,
            |cpu| GuestEventId::new(600 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(600), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 1);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9008, 8),
        Some(b"/\0\0\0\0\0\0\0".to_vec())
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9010, 16),
        Some([0; 16].to_vec())
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9020, 8),
        Some([0; 8].to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_getcwd_writes_target_cwd_and_resumes() {
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
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
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
        "cpu0.dmem",
        data_route,
        0x8000,
    );
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, 17)),
            (0x8004, lui(10, 9)),
            (0x8008, addi(11, 0, 8)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(5, 10, 0)),
            (0x8014, lui(6, 9)),
            (0x8018, lb(7, 6, 0)),
            (0x801c, addi(17, 0, 93)),
            (0x8020, addi(10, 0, 0)),
            (0x8024, 0x0000_0073),
        ],
        &[(0x9000, &[0xaa; 8])],
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
            110,
            |cpu| GuestEventId::new(580 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(580), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 1);
    assert_eq!(core.read_register(reg(7)), u64::from(b'/'));
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9000, 8),
        Some(b"/\0\0\0\0\0\0\0".to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
