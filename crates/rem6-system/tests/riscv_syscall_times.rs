#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};
use support::*;

#[test]
fn linux_table_times_writes_riscv_tms() {
    let store =
        loaded_program_store_with_data(&[(0x8000, addi(0, 0, 0))], &[(0x9008, &[0xff; 32])]);
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 153, [0x9008, 0, 0, 0, 0, 0]),
        &mut state,
        1_234_567_890,
        None,
        Some(&writer),
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 123 }));
    let bytes = read_guest_bytes(Arc::clone(&store), 0x9008, 32);
    assert_eq!(read_u64(&bytes, 0), 123);
    assert_eq!(read_u64(&bytes, 8), 0);
    assert_eq!(read_u64(&bytes, 16), 0);
    assert_eq!(read_u64(&bytes, 24), 0);
}

#[test]
fn linux_table_times_accepts_null_tms_without_guest_writer() {
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 153, [0, 0, 0, 0, 0, 0]),
        &mut state,
        2_000_000_000,
        None,
        None,
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 200 }));
}

#[test]
fn linux_table_times_reports_guest_write_fault() {
    let writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, 153, [0x9000, 0, 0, 0, 0, 0]),
        &mut state,
        3_000_000_000,
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
fn user_ecall_times_writes_tms_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(79);
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
            (0x8000, addi(17, 0, 153)),
            (0x8004, lui(10, 9)),
            (0x8008, 0x0000_0073),
            (0x800c, addi(5, 10, 0)),
            (0x8010, addi(17, 0, 93)),
            (0x8014, addi(10, 0, 0)),
            (0x8018, 0x0000_0073),
        ],
        &[(0x9000, &[0xff; 32])],
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
            |cpu| GuestEventId::new(590 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(590), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    let elapsed = core.read_register(reg(5));
    let bytes = read_guest_bytes(Arc::clone(&store), 0x9000, 32);
    assert_ne!(bytes, vec![0xff; 32]);
    assert_eq!(read_u64(&bytes, 0), elapsed);
    assert_eq!(read_u64(&bytes, 8), 0);
    assert_eq!(read_u64(&bytes, 16), 0);
    assert_eq!(read_u64(&bytes, 24), 0);
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

fn read_guest_bytes(
    store: Arc<Mutex<PartitionedMemoryStore>>,
    address: u64,
    bytes: usize,
) -> Vec<u8> {
    let reader = guest_memory_reader(store);
    (0..bytes)
        .map(|offset| reader(address + offset as u64, 1).unwrap()[0])
        .collect()
}
