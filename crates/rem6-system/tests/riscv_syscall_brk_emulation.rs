#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod riscv_syscall_emulation_support;

use riscv_syscall_emulation_support::*;

fn strict_guest_memory_mapper(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl Fn(u64, u64) -> bool + Send + Sync + 'static {
    move |address, bytes| {
        let line_layout = CacheLineLayout::new(16).unwrap();
        let Ok(size) = AccessSize::new(bytes) else {
            return false;
        };
        let mut store = store.lock().unwrap();
        if !matches!(
            store.map_region(MemoryTargetId::new(0), Address::new(address), size),
            Ok(())
        ) {
            return false;
        }
        let Some(end) = address.checked_add(bytes) else {
            return false;
        };
        let zero_line = vec![0; line_layout.bytes() as usize];
        let mut line = line_layout.line_address(Address::new(address));
        while line.get() < end {
            if store
                .insert_line(MemoryTargetId::new(0), line, zero_line.clone())
                .is_err()
            {
                return false;
            }
            let Some(next) = line.get().checked_add(line_layout.bytes()) else {
                return false;
            };
            line = Address::new(next);
        }
        true
    }
}

fn sb(rs2: u8, rs1: u8, imm: i32) -> u32 {
    let imm = imm as u32;
    (((imm >> 5) & 0x7f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | ((imm & 0x1f) << 7)
        | 0x23
}

#[test]
fn user_ecall_brk_returns_to_guest_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(42);
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
        (0x8000, addi(17, 0, 214)),
        (0x8004, addi(10, 0, 64)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(17, 0, 214)),
        (0x8014, addi(10, 0, 0)),
        (0x8018, 0x0000_0073),
        (0x801c, addi(6, 10, 0)),
        (0x8020, addi(17, 0, 93)),
        (0x8024, addi(10, 6, 0)),
        (0x8028, 0x0000_0073),
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
            80,
            |cpu| GuestEventId::new(180 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(180),
        source,
        64,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 64);
    assert_eq!(core.read_register(reg(6)), 64);
    assert_eq!(core.read_register(reg(10)), 64);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn user_ecall_brk_uses_boot_image_program_break_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(150);
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
    let image = boot_image_with_data(
        &[
            (0x8000, addi(17, 0, 214)),
            (0x8004, addi(10, 0, 0)),
            (0x8008, 0x0000_0073),
            (0x800c, addi(5, 10, 0)),
            (0x8010, addi(17, 0, 93)),
            (0x8014, addi(10, 0, 0)),
            (0x8018, 0x0000_0073),
        ],
        &[(0x9002, &[0xaa, 0xbb, 0xcc])],
    );
    let store = loaded_boot_image_store(&image);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver =
        RiscvSystemRunDriver::new(trap_port).with_riscv_syscall_emulation_for_boot_image(&image);

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
            |cpu| GuestEventId::new(230 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(230), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0xa000);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .program_break(),
        0xa000
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

fn check_getcwd_then_brk_with_boot_image_and_guest_writer(seed_before_writer: bool) {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(if seed_before_writer { 151 } else { 152 });
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
    let image = boot_image_with_data(
        &[
            (0x8000, lui(10, 0x9)),
            (0x8004, addi(11, 0, 8)),
            (0x8008, addi(17, 0, 17)),
            (0x800c, 0x0000_0073),
            (0x8010, addi(6, 10, 0)),
            (0x8014, addi(17, 0, 214)),
            (0x8018, addi(10, 0, 0)),
            (0x801c, 0x0000_0073),
            (0x8020, addi(5, 10, 0)),
            (0x8024, addi(17, 0, 93)),
            (0x8028, addi(10, 0, 0)),
            (0x802c, 0x0000_0073),
        ],
        &[(0x9002, &[0xaa, 0xbb, 0xcc])],
    );
    let store = loaded_boot_image_store(&image);
    let read_guest = guest_memory_reader(Arc::clone(&store));
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = if seed_before_writer {
        RiscvSystemRunDriver::new(trap_port)
            .with_riscv_syscall_emulation_for_boot_image(&image)
            .with_riscv_syscall_emulation_and_guest_memory_writer(guest_memory_writer(Arc::clone(
                &store,
            )))
    } else {
        RiscvSystemRunDriver::new(trap_port)
            .with_riscv_syscall_emulation_and_guest_memory_writer(guest_memory_writer(Arc::clone(
                &store,
            )))
            .with_riscv_syscall_emulation_for_boot_image(&image)
    };

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
            |cpu| GuestEventId::new(310 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(310), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(6)), 1);
    assert_eq!(core.read_register(reg(5)), 0xa000);
    assert_eq!(read_guest(0x9000, 4).unwrap(), b"/\0\0\0");
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .program_break(),
        0xa000
    );
}

#[test]
fn user_ecall_brk_preserves_boot_image_break_when_guest_writer_is_added() {
    check_getcwd_then_brk_with_boot_image_and_guest_writer(true);
}

#[test]
fn user_ecall_getcwd_preserves_guest_writer_when_boot_image_break_is_added() {
    check_getcwd_then_brk_with_boot_image_and_guest_writer(false);
}

#[test]
fn user_ecall_brk_installs_zeroed_heap_backing_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(153);
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
    let image = boot_image_with_data(
        &[
            (0x8000, lui(10, 0xa)),
            (0x8004, addi(10, 10, 16)),
            (0x8008, addi(17, 0, 214)),
            (0x800c, 0x0000_0073),
            (0x8010, lui(6, 0xa)),
            (0x8014, addi(5, 0, 37)),
            (0x8018, sb(5, 6, 8)),
            (0x801c, lb(10, 6, 8)),
            (0x8020, addi(17, 0, 93)),
            (0x8024, 0x0000_0073),
        ],
        &[(0x9002, &[0xaa, 0xbb, 0xcc])],
    );
    let store = loaded_boot_image_store(&image);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_for_boot_image(&image)
        .with_riscv_syscall_emulation_and_mapped_guest_memory_writer(
            guest_memory_writer(Arc::clone(&store)),
            guest_memory_mapper(Arc::clone(&store)),
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
            160,
            |cpu| GuestEventId::new(330 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(330),
        source,
        37,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(10)), 37);
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0xa008, 1),
        Some(vec![37])
    );
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .program_break(),
        0xa010
    );
}

#[test]
fn user_ecall_brk_regrows_previously_backed_heap_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(154);
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
    let image = boot_image_with_data(
        &[
            (0x8000, lui(10, 0xc)),
            (0x8004, addi(10, 10, 16)),
            (0x8008, addi(17, 0, 214)),
            (0x800c, 0x0000_0073),
            (0x8010, lui(10, 0xb)),
            (0x8014, addi(17, 0, 214)),
            (0x8018, 0x0000_0073),
            (0x801c, lui(10, 0xc)),
            (0x8020, addi(10, 10, 16)),
            (0x8024, addi(17, 0, 214)),
            (0x8028, 0x0000_0073),
            (0x802c, addi(5, 10, 0)),
            (0x8030, addi(17, 0, 93)),
            (0x8034, addi(10, 0, 0)),
            (0x8038, 0x0000_0073),
        ],
        &[(0x9002, &[0xaa, 0xbb, 0xcc])],
    );
    let store = loaded_boot_image_store(&image);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_for_boot_image(&image)
        .with_riscv_syscall_emulation_and_mapped_guest_memory_writer(
            guest_memory_writer(Arc::clone(&store)),
            strict_guest_memory_mapper(Arc::clone(&store)),
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
            180,
            |cpu| GuestEventId::new(340 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(340), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0xc010);
    assert_eq!(
        driver
            .riscv_syscall_emulation()
            .unwrap()
            .state()
            .program_break(),
        0xc010
    );
}
