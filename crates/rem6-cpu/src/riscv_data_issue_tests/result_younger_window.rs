use super::*;
use rem6_boot::BootImage;
use rem6_memory::{MemoryTargetId, PartitionedMemoryStore};

fn div_raw(rd: u8, rs1: u8, rs2: u8) -> u32 {
    (1 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (4 << 12)
        | (u32::from(rd) << 7)
        | 0x33
}

fn complete_load_div_witness_fetches(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    address: u64,
) {
    core.write_register(reg(2), address);
    core.write_register(reg(1), 84);
    core.write_register(reg(3), 2);
    let instructions = [
        i_type(0, 2, 0b011, 12, 0x03),
        div_raw(20, 1, 3),
        i_type(1, 12, 0b000, 21, 0x13),
    ];
    for instruction in instructions {
        core.issue_next_fetch(
            scheduler,
            transport,
            MemoryTrace::new(),
            move |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(instruction.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
        scheduler.run_until_idle_conservative();
        if core.pc() == Address::new(0x8000) {
            let executed = core.execute_next_completed_fetch().unwrap().unwrap();
            assert_eq!(executed.fetch_pc(), Address::new(0x8000));
        }
    }
}

fn assert_div_owned_and_witness_waiting(core: &RiscvCore) -> (u64, u64) {
    let state = core.state.lock().expect("riscv core lock");
    let snapshot = state.o3_runtime.snapshot();
    let div_sequence = snapshot
        .reorder_buffer()
        .iter()
        .find(|row| row.pc() == Address::new(0x8004))
        .expect("DIV row stages")
        .sequence();
    let witness_sequence = snapshot
        .reorder_buffer()
        .iter()
        .find(|row| row.pc() == Address::new(0x8008))
        .expect("dependent witness row stages")
        .sequence();
    assert!(state
        .o3_runtime
        .writeback_reservation(div_sequence)
        .is_some());
    assert!(state
        .o3_runtime
        .writeback_reservation(witness_sequence)
        .is_none());
    (div_sequence, witness_sequence)
}

fn assert_younger_window_policy(core: &RiscvCore, expected: O3DataAccessWindowPolicy) {
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.o3_runtime.live_data_access_younger_window_policy(),
        Some(expected)
    );
}

fn test_mmio_bus_with_delays(
    address: u64,
    value: Vec<u8>,
    request_delay: u64,
    response_delay: u64,
) -> MmioBus {
    let mut bank =
        MmioRegisterBank::new(Address::new(address), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        0,
        AccessSize::new(value.len() as u64).unwrap(),
        MmioAccess::ReadOnly,
        value,
    )
    .unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(Address::new(address), AccessSize::new(0x100).unwrap()).unwrap(),
        MmioRoute::new(
            PartitionId::new(0),
            PartitionId::new(1),
            request_delay,
            response_delay,
        )
        .unwrap(),
        Mutex::new(bank),
    )
    .unwrap();
    bus
}

fn fetch_instruction(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    raw: u32,
) {
    core.issue_next_fetch(
        scheduler,
        transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(raw.to_le_bytes().to_vec()))
                    .unwrap(),
            )
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
}

fn cluster_program_store(program: &[(u64, u32)]) -> Arc<Mutex<PartitionedMemoryStore>> {
    let target = MemoryTargetId::new(0);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, line_layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    let mut image = BootImage::new(Address::new(0x8000));
    for (address, raw) in program {
        image = image
            .add_segment(Address::new(*address), raw.to_le_bytes().to_vec())
            .unwrap();
    }
    image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();
    Arc::new(Mutex::new(store))
}

fn cluster_fetch_response(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    delivery: &rem6_transport::RequestDelivery,
) -> TargetOutcome {
    let response = store
        .lock()
        .expect("memory store lock")
        .respond(delivery.request())
        .unwrap()
        .response()
        .cloned()
        .unwrap();
    TargetOutcome::Respond(response)
}

fn schedule_cluster_writeback_wake(core: &RiscvCore, scheduler: &mut PartitionedScheduler) {
    let Some(tick) = core.requested_o3_writeback_wake_tick(scheduler.now()) else {
        return;
    };
    let fired = core.clone();
    let event_id = scheduler
        .schedule_parallel_at(core.partition(), tick, move |context| {
            fired.mark_o3_writeback_wake_fired(context.now());
        })
        .unwrap();
    let event = scheduler
        .pending_event_snapshot(event_id)
        .expect("new O3 writeback wake is pending");
    core.mark_o3_writeback_wake_scheduled(scheduler.instance_id(), event);
}

#[test]
fn cluster_mmio_drive_produces_result_first_div_collision_and_witness_retirement() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_o3_writeback_width(1);
    core.write_register(reg(2), 0xa000);
    core.write_register(reg(1), 84);
    core.write_register(reg(3), 2);
    let load = i_type(0, 2, 0b011, 12, 0x03);
    let div = div_raw(20, 1, 3);
    let witness = i_type(1, 12, 0b000, 21, 0x13);
    let control = 0x0000_006f;
    let store = cluster_program_store(&[
        (0x8000, load),
        (0x8004, div),
        (0x8008, witness),
        (0x800c, control),
    ]);
    let bus = test_mmio_bus_with_delays(0xa000, 42_u64.to_le_bytes().to_vec(), 9, 9);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let mut collision = None;
    let mut collision_verified = false;
    let mut witness_woke = false;
    let mut witness_retired = false;

    for _ in 0..96 {
        let fetch_store = store.clone();
        let actions = cluster
            .drive_ready_cores_parallel_with_mmio(
                &mut scheduler,
                &transport,
                &bus,
                MemoryTrace::new(),
                MemoryTrace::new(),
                move |_cpu| {
                    let fetch_store = fetch_store.clone();
                    move |delivery, _context| cluster_fetch_response(&fetch_store, &delivery)
                },
                |_cpu| |_delivery, _context| panic!("MMIO load must not use memory transport"),
            )
            .unwrap();

        for action in &actions {
            match action.action() {
                crate::RiscvCoreDriveAction::DataAccessIssued { .. } => {
                    assert!(collision.is_none(), "MMIO result issues exactly once");
                    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::MemoryResult);
                    let (div_sequence, _) = assert_div_owned_and_witness_waiting(&core);
                    let state = core.state.lock().expect("riscv core lock");
                    let div_reservation = state
                        .o3_runtime
                        .writeback_reservation(div_sequence)
                        .expect("cluster-staged DIV reservation");
                    assert_eq!(div_reservation.raw_ready_tick(), scheduler.now() + 19);
                    assert_eq!(div_reservation.source_name(), "FixedFu");
                    collision = Some((scheduler.now(), div_sequence));
                }
                crate::RiscvCoreDriveAction::InstructionExecuted(event)
                    if event.fetch_pc() == Address::new(0x8008) =>
                {
                    witness_retired = true;
                }
                _ => {}
            }
        }

        schedule_cluster_writeback_wake(&core, &mut scheduler);
        scheduler.run_until_idle_parallel().unwrap();

        if let Some((issue_tick, div_sequence)) = collision {
            let response_tick = core
                .data_access_events()
                .iter()
                .rev()
                .find(|event| event.tick() > issue_tick)
                .map(|event| event.tick());
            if let Some(response_tick) = response_tick {
                let state = core.state.lock().expect("riscv core lock");
                let reservations = state.o3_runtime.writeback_reservations();
                if let Some(result) = reservations
                    .iter()
                    .copied()
                    .find(|reservation| reservation.source_name() == "MemoryResult")
                {
                    let div = state
                        .o3_runtime
                        .writeback_reservation(div_sequence)
                        .expect("DIV reservation remains live");
                    assert_eq!(result.raw_ready_tick(), response_tick + 1);
                    assert_eq!(result.raw_ready_tick(), div.raw_ready_tick());
                    assert_eq!(result.admitted_tick(), result.raw_ready_tick());
                    assert_eq!(div.admitted_tick(), result.admitted_tick() + 1);
                    assert_eq!(result.slot(), 0);
                    assert_eq!(div.slot(), 0);
                    assert!(!core
                        .core
                        .fetch_events()
                        .iter()
                        .any(|event| event.pc() == Address::new(0x800c)));
                    collision_verified = true;
                }
            }
        }

        while let Some(retired) =
            core.record_ready_o3_data_access_event_with_trace(scheduler.now(), true)
        {
            if retired.fetch_pc() == Address::new(0x8000) {
                let state = core.state.lock().expect("riscv core lock");
                let witness_sequence = state
                    .o3_runtime
                    .snapshot()
                    .reorder_buffer()
                    .iter()
                    .find(|row| row.pc() == Address::new(0x8008))
                    .expect("dependent witness row remains staged")
                    .sequence();
                witness_woke = state
                    .o3_runtime
                    .writeback_reservation(witness_sequence)
                    .is_some();
            }
        }

        if witness_retired && witness_woke && core.o3_live_data_access_lifecycle_is_quiescent() {
            break;
        }
    }

    assert!(collision.is_some(), "cluster must issue the MMIO result");
    assert!(
        collision_verified,
        "cluster must expose the writeback collision"
    );
    assert!(
        witness_woke,
        "result publication must wake the dependent witness"
    );
    assert!(witness_retired, "cluster must retire the dependent witness");
    assert_eq!(core.read_register(reg(12)), 42);
    assert_eq!(core.read_register(reg(20)), 42);
    assert_eq!(core.read_register(reg(21)), 43);
    assert!(core.o3_live_data_access_lifecycle_is_quiescent());
}

#[test]
fn detailed_mmio_real_fetch_ahead_stages_a_real_div_result_collision() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_o3_writeback_width(1);
    core.write_register(reg(2), 0xa000);
    core.write_register(reg(1), 84);
    core.write_register(reg(3), 2);
    let bus = test_mmio_bus_with_delays(0xa000, 42_u64.to_le_bytes().to_vec(), 9, 9);
    let load = i_type(0, 2, 0b011, 12, 0x03);
    let div = div_raw(20, 1, 3);
    let witness = i_type(1, 12, 0b000, 21, 0x13);

    fetch_instruction(&core, &mut scheduler, &transport, load);
    let div_decision = core
        .next_mmio_aware_fetch_ahead_before_retire(&bus)
        .expect("MMIO result head fetches an independent DIV");
    assert_eq!(div_decision.pc(), Address::new(0x8004));
    core.set_fetch_ahead_pc(div_decision.pc());
    fetch_instruction(&core, &mut scheduler, &transport, div);
    let witness_decision = core
        .next_mmio_aware_fetch_ahead_before_retire(&bus)
        .expect("MMIO result head fetches its dependent witness as the terminal row");
    assert_eq!(witness_decision.pc(), Address::new(0x8008));
    core.set_fetch_ahead_pc(witness_decision.pc());
    fetch_instruction(&core, &mut scheduler, &transport, witness);

    let executed = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(executed.fetch_pc(), Address::new(0x8000));
    let issue_tick = scheduler.now();
    core.issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap()
        .expect("MMIO result issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::MemoryResult);
    let (div_sequence, witness_sequence) = assert_div_owned_and_witness_waiting(&core);
    {
        let state = core.state.lock().expect("riscv core lock");
        let div_reservation = state
            .o3_runtime
            .writeback_reservation(div_sequence)
            .expect("real DIV owns a writeback reservation");
        assert_eq!(div_reservation.raw_ready_tick(), issue_tick + 19);
        assert_eq!(div_reservation.source_name(), "FixedFu");
        assert!(div_reservation.decision_counted());
    }

    scheduler.run_until_idle_parallel().unwrap();
    let response_tick = core
        .data_access_events()
        .last()
        .expect("MMIO response event")
        .tick();
    let admitted_tick = {
        let state = core.state.lock().expect("riscv core lock");
        let reservations = state.o3_runtime.writeback_reservations();
        let result = reservations
            .iter()
            .copied()
            .find(|reservation| reservation.source_name() == "MemoryResult")
            .expect("MMIO result owns a writeback reservation");
        let div = state
            .o3_runtime
            .writeback_reservation(div_sequence)
            .expect("DIV reservation remains live");
        assert_eq!(result.raw_ready_tick(), response_tick + 1);
        assert_eq!(result.raw_ready_tick(), div.raw_ready_tick());
        assert_eq!(result.admitted_tick(), result.raw_ready_tick());
        assert_eq!(result.slot(), 0);
        assert_eq!(div.admitted_tick(), result.admitted_tick() + 1);
        assert_eq!(div.slot(), 0);
        assert_eq!(result.sequence(), 0);
        assert_eq!(div.sequence(), div_sequence);
        result.admitted_tick()
    };

    assert!(core
        .record_ready_o3_data_access_event_with_trace(admitted_tick, true)
        .is_some());
    let state = core.state.lock().expect("riscv core lock");
    assert!(state
        .o3_runtime
        .writeback_reservation(witness_sequence)
        .is_some());
}

#[test]
fn direct_mmio_cluster_driver_does_not_fetch_a_second_data_row() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0xa000);
    core.write_register(reg(3), 0xb000);
    let load = i_type(0, 2, 0b011, 12, 0x03);
    let second_load = i_type(0, 3, 0b011, 6, 0x03);
    fetch_instruction(&core, &mut scheduler, &transport, load);
    core.set_fetch_ahead_pc(Address::new(0x8004));
    fetch_instruction(&core, &mut scheduler, &transport, second_load);

    let bus = test_mmio_bus(0xa000, 42_u64.to_le_bytes().to_vec());
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let actions = cluster
        .drive_ready_cores_parallel_with_mmio(
            &mut scheduler,
            &transport,
            &bus,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
            |_cpu| |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap();

    assert!(actions.iter().all(|event| !matches!(
        event.action(),
        crate::RiscvCoreDriveAction::FetchIssued { .. }
    )));
    assert!(!core
        .core
        .fetch_events()
        .iter()
        .any(|event| event.pc() == Address::new(0x8008)));
}

#[test]
fn detailed_mmio_result_opens_div_window_and_wakes_destination_witness_on_admission() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    complete_load_div_witness_fetches(&core, &mut scheduler, &transport, 0xa000);

    let bus = test_mmio_bus(0xa000, 42_u64.to_le_bytes().to_vec());
    core.issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap()
        .expect("detailed scalar MMIO load issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::MemoryResult);
    let (_, witness_sequence) = assert_div_owned_and_witness_waiting(&core);

    scheduler.run_until_idle_parallel().unwrap();
    assert!(core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .is_some());
    let state = core.state.lock().expect("riscv core lock");
    assert!(state
        .o3_runtime
        .writeback_reservation(witness_sequence)
        .is_some());
}

#[test]
fn detailed_cacheable_scalar_load_still_opens_real_div_window() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    complete_load_div_witness_fetches(&core, &mut scheduler, &transport, 0x9000);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("detailed cacheable scalar load issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::ScalarMemoryPrefix);
    assert_div_owned_and_witness_waiting(&core);
}

#[test]
fn detailed_pma_uncacheable_memory_result_opens_div_window_and_blocks_witness() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9008).unwrap())
        .unwrap();
    complete_load_div_witness_fetches(&core, &mut scheduler, &transport, 0x9000);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("detailed PMA-uncacheable scalar load issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::MemoryResult);
    assert_div_owned_and_witness_waiting(&core);
}

#[test]
fn detailed_zero_destination_scalar_load_persists_no_younger_window_policy() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.write_register(reg(2), 0x9000);
    let load_x0 = i_type(0, 2, 0b011, 0, 0x03);
    core.issue_next_fetch(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        move |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(load_x0.to_le_bytes().to_vec()))
                    .unwrap(),
            )
        },
    )
    .unwrap();
    scheduler.run_until_idle_conservative();
    core.execute_next_completed_fetch().unwrap().unwrap();

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("detailed zero-destination scalar load issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::None);
    assert_eq!(core.o3_runtime_snapshot().reorder_buffer().len(), 1);
}
