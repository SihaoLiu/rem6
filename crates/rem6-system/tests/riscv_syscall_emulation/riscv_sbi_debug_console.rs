use super::riscv_syscall_emulation_support::*;
use rem6_system::{GuestTrap, GuestTrapKind};

const SBI_BASE_EXTENSION: u64 = 0x10;
const SBI_BASE_GET_SPEC_VERSION: i32 = 0;
const SBI_BASE_PROBE_EXTENSION: i32 = 3;
const SBI_LEGACY_CONSOLE_GETCHAR: i32 = 2;
const SBI_DEBUG_CONSOLE_EXTENSION: u64 = 0x4442_434e;
const SBI_DEBUG_CONSOLE_WRITE: i32 = 0;
const SBI_DEBUG_CONSOLE_READ: i32 = 1;
const SBI_DEBUG_CONSOLE_WRITE_BYTE: i32 = 2;
const SBI_ERR_INVALID_ADDRESS: u64 = (-5_i64) as u64;
const SBI_SPEC_VERSION_2_0: u64 = 2 << 24;

fn lui_addi_parts(value: u64) -> (u32, i32) {
    let hi = ((value + 0x800) >> 12) as u32;
    let lo = (value as i64 - ((u64::from(hi) << 12) as i64)) as i32;
    (hi, lo)
}

#[test]
fn supervisor_sbi_debug_console_write_byte_records_output() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(71);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let (dbcn_hi, dbcn_lo) = lui_addi_parts(SBI_DEBUG_CONSOLE_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, SBI_BASE_EXTENSION as i32)),
        (0x8004, addi(16, 0, SBI_BASE_GET_SPEC_VERSION)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(6, 11, 0)),
        (0x8014, lui(17, dbcn_hi)),
        (0x8018, addi(17, 17, dbcn_lo)),
        (0x801c, addi(16, 0, SBI_DEBUG_CONSOLE_WRITE_BYTE)),
        (0x8020, addi(10, 0, i32::from(b'A'))),
        (0x8024, 0x0000_0073),
        (0x8028, addi(7, 10, 0)),
        (0x802c, addi(8, 11, 0)),
        (0x8030, addi(17, 0, SBI_BASE_EXTENSION as i32)),
        (0x8034, addi(16, 0, SBI_BASE_PROBE_EXTENSION)),
        (0x8038, lui(10, dbcn_hi)),
        (0x803c, addi(10, 10, dbcn_lo)),
        (0x8040, 0x0000_0073),
        (0x8044, addi(9, 11, 0)),
        (0x8048, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware()
        .with_riscv_syscall_emulation();

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
            |cpu| GuestEventId::new(710 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(710), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8048)]
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), SBI_SPEC_VERSION_2_0);
    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.read_register(reg(8)), 0);
    assert_eq!(core.read_register(reg(9)), 0);
    assert_eq!(
        driver.riscv_sbi_firmware().unwrap().debug_console_bytes(),
        b"A"
    );
    assert_eq!(run.riscv_debug_console_bytes(), b"A");
}

#[test]
fn supervisor_sbi_legacy_console_getchar_consumes_debug_console_input() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(76);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, SBI_LEGACY_CONSOLE_GETCHAR)),
        (0x8004, 0x0000_0073),
        (0x8008, addi(5, 10, 0)),
        (0x800c, addi(17, 0, SBI_LEGACY_CONSOLE_GETCHAR)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(6, 10, 0)),
        (0x8018, addi(17, 0, SBI_LEGACY_CONSOLE_GETCHAR)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(7, 10, 0)),
        (0x8024, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware()
        .with_riscv_sbi_debug_console_input(b"Q?".to_vec())
        .with_riscv_syscall_emulation();

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
            |cpu| GuestEventId::new(760 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(760), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8024)]
    );
    assert_eq!(core.read_register(reg(5)), u64::from(b'Q'));
    assert_eq!(core.read_register(reg(6)), u64::from(b'?'));
    assert_eq!(core.read_register(reg(7)), u64::MAX);
    assert!(run.riscv_debug_console_bytes().is_empty());
}

#[test]
fn supervisor_sbi_debug_console_write_records_guest_memory_output() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(72);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let (dbcn_hi, dbcn_lo) = lui_addi_parts(SBI_DEBUG_CONSOLE_EXTENSION);
    let store = loaded_program_store_with_data(
        &[
            (0x8000, lui(17, dbcn_hi)),
            (0x8004, addi(17, 17, dbcn_lo)),
            (0x8008, addi(16, 0, SBI_DEBUG_CONSOLE_WRITE)),
            (0x800c, addi(10, 0, 5)),
            (0x8010, lui(11, 9)),
            (0x8014, addi(12, 0, 0)),
            (0x8018, 0x0000_0073),
            (0x801c, addi(5, 10, 0)),
            (0x8020, addi(6, 11, 0)),
            (0x8024, 0x0010_0073),
        ],
        &[(0x9000, b"hello")],
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
        .with_riscv_sbi_firmware_and_functional_guest_memory_reader(guest_memory_reader(
            Arc::clone(&store),
        ))
        .with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(720 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(720), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8024)]
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 5);
    assert_eq!(
        driver.riscv_sbi_firmware().unwrap().debug_console_bytes(),
        b"hello"
    );
    assert_eq!(run.riscv_debug_console_bytes(), b"hello");
}

#[test]
fn supervisor_sbi_debug_console_write_rejects_unreadable_guest_memory() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(73);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let (dbcn_hi, dbcn_lo) = lui_addi_parts(SBI_DEBUG_CONSOLE_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, lui(17, dbcn_hi)),
        (0x8004, addi(17, 17, dbcn_lo)),
        (0x8008, addi(16, 0, SBI_DEBUG_CONSOLE_WRITE)),
        (0x800c, addi(10, 0, 5)),
        (0x8010, lui(11, 9)),
        (0x8014, addi(12, 0, 0)),
        (0x8018, 0x0000_0073),
        (0x801c, addi(5, 10, 0)),
        (0x8020, addi(6, 11, 0)),
        (0x8024, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware_and_functional_guest_memory_reader(|_, _| None)
        .with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(730 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(730), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core.read_register(reg(5)), SBI_ERR_INVALID_ADDRESS);
    assert_eq!(core.read_register(reg(6)), 0);
    assert!(driver
        .riscv_sbi_firmware()
        .unwrap()
        .debug_console_bytes()
        .is_empty());
    assert!(run.riscv_debug_console_bytes().is_empty());
}

#[test]
fn supervisor_sbi_debug_console_read_writes_guest_memory_and_advertises_dbcn() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(74);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let (dbcn_hi, dbcn_lo) = lui_addi_parts(SBI_DEBUG_CONSOLE_EXTENSION);
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, SBI_BASE_EXTENSION as i32)),
            (0x8004, addi(16, 0, SBI_BASE_PROBE_EXTENSION)),
            (0x8008, lui(10, dbcn_hi)),
            (0x800c, addi(10, 10, dbcn_lo)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(5, 10, 0)),
            (0x8018, addi(6, 11, 0)),
            (0x801c, lui(17, dbcn_hi)),
            (0x8020, addi(17, 17, dbcn_lo)),
            (0x8024, addi(16, 0, SBI_DEBUG_CONSOLE_READ)),
            (0x8028, addi(10, 0, 8)),
            (0x802c, lui(11, 9)),
            (0x8030, addi(12, 0, 0)),
            (0x8034, 0x0000_0073),
            (0x8038, addi(7, 10, 0)),
            (0x803c, addi(8, 11, 0)),
            (0x8040, 0x0010_0073),
        ],
        &[(0x9000, &[0u8; 8])],
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
        .with_riscv_sbi_firmware_and_functional_guest_memory_reader(guest_memory_reader(
            Arc::clone(&store),
        ))
        .with_riscv_sbi_firmware_and_functional_guest_memory_writer(guest_memory_writer(
            Arc::clone(&store),
        ))
        .with_riscv_sbi_debug_console_input(b"xyz".to_vec())
        .with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            100,
            |cpu| GuestEventId::new(740 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(740), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 1);
    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.read_register(reg(8)), 3);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9000, 8).unwrap(),
        {
            let mut bytes = b"xyz".to_vec();
            bytes.extend_from_slice(&[0, 0, 0, 0, 0]);
            bytes
        }
    );
    assert!(run.riscv_debug_console_bytes().is_empty());
}

#[test]
fn supervisor_sbi_debug_console_read_rejects_unwritable_requested_range() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(75);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let (dbcn_hi, dbcn_lo) = lui_addi_parts(SBI_DEBUG_CONSOLE_EXTENSION);
    let store = loaded_program_store_with_data(
        &[
            (0x8000, lui(17, dbcn_hi)),
            (0x8004, addi(17, 17, dbcn_lo)),
            (0x8008, addi(16, 0, SBI_DEBUG_CONSOLE_READ)),
            (0x800c, addi(10, 0, 8)),
            (0x8010, lui(11, 9)),
            (0x8014, addi(12, 0, 0)),
            (0x8018, 0x0000_0073),
            (0x801c, addi(5, 10, 0)),
            (0x8020, addi(6, 11, 0)),
            (0x8024, 0x0010_0073),
        ],
        &[(0x9000, &[0u8; 8])],
    );
    let writer_store = Arc::clone(&store);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware_and_functional_guest_memory_writer(move |address, bytes| {
            if bytes.is_empty() && address == 0x9007 {
                return false;
            }
            guest_memory_writer(Arc::clone(&writer_store))(address, bytes)
        })
        .with_riscv_sbi_debug_console_input(b"xyz".to_vec())
        .with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            80,
            |cpu| GuestEventId::new(750 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(750), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core.read_register(reg(5)), SBI_ERR_INVALID_ADDRESS);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9000, 8).unwrap(),
        vec![0; 8]
    );
    assert!(run.riscv_debug_console_bytes().is_empty());
}
