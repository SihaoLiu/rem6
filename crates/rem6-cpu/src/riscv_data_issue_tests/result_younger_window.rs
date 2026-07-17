use super::*;

#[path = "result_younger_window/terminal_ownership.rs"]
mod terminal_ownership;

fn assert_younger_window_policy(core: &RiscvCore, expected: O3DataAccessWindowPolicy) {
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.o3_runtime.live_data_access_younger_window_policy(),
        Some(expected)
    );
}

fn fetch_and_execute(
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
    assert_eq!(
        core.execute_next_completed_fetch()
            .unwrap()
            .unwrap()
            .fetch_pc(),
        Address::new(0x8000)
    );
}

fn completed_fetch_with_raw(sequence: u64, pc: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            MemoryRequestId::new(AgentId::new(7), sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn fp_r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    r_type(funct7, rs2, rs1, funct3, rd, 0x53)
}

fn atomic_type(funct5: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct5 << 27)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn vector_mvv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_unit_stride_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

#[test]
fn older_fixed_fu_head_preissues_one_younger_terminal_result_before_gate_ready() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);

    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    let witness = i_type(1, 5, 0b000, 6, 0x13);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, load),
            completed_fetch_with_raw(3, 0x8008, witness),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    let gate_wakes = core.checkpoint_owned_live_retire_gate_wakes();
    let gate_ready_tick = gate_wakes
        .first()
        .map(|(_, event)| event.tick())
        .expect("older DIV head owns a live-retire gate wake");
    assert!(
        scheduler.now() < gate_ready_tick,
        "test must observe issue before the DIV gate is ready"
    );

    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .is_none());
    scheduler.run_next_epoch();
    assert!(scheduler.now() < gate_ready_tick);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("younger terminal load issues before older DIV writeback");

    let issued = core
        .data_access_events()
        .last()
        .cloned()
        .expect("issued data access event is recorded");
    assert_eq!(
        issued.fetch_request_id(),
        MemoryRequestId::new(AgentId::new(7), 2)
    );
    let snapshot = core.o3_runtime_snapshot();
    assert_eq!(
        snapshot
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        vec![Address::new(0x8000), Address::new(0x8004)]
    );
    assert_eq!(snapshot.load_store_queue().len(), 1);
    assert_eq!(core.pc(), Address::new(0x8000));
    assert_eq!(core.read_register(reg(3)), 0);
    assert!(
        core.execution_events()
            .iter()
            .all(|event| !matches!(event.fetch_pc().get(), 0x8004 | 0x8008)),
        "provisional result and following witness stay out of public execution history"
    );

    scheduler.run_until_idle_conservative();
    assert_eq!(scheduler.now(), gate_ready_tick);
    assert_eq!(
        core.execute_next_completed_fetch_serial(&mut scheduler)
            .unwrap()
            .expect("older DIV retires at its gate")
            .fetch_pc(),
        Address::new(0x8000)
    );
    assert_eq!(core.pc(), Address::new(0x8004));
    assert_eq!(core.read_register(reg(3)), 14);
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(
        core.execution_events()
            .iter()
            .map(RiscvCpuExecutionEvent::fetch_pc)
            .collect::<Vec<_>>(),
        vec![Address::new(0x8000)]
    );
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .is_some());
}

#[test]
fn terminal_result_that_reads_older_fu_destination_is_not_provisioned() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);

    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let dependent_load = i_type(0, 3, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, dependent_load),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    {
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.pending_terminal_memory_result.is_none());
        assert_eq!(
            state
                .o3_runtime
                .snapshot()
                .reorder_buffer()
                .iter()
                .map(|entry| entry.pc())
                .collect::<Vec<_>>(),
            vec![Address::new(0x8000)]
        );
    }

    scheduler.run_next_epoch_until(2).unwrap();
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("dependent terminal load must not issue before the DIV"),
        )
        .unwrap()
        .is_none());
    assert!(core.data_access_events().is_empty());
}

#[test]
fn float_integer_head_blocks_dependent_terminal_load() {
    assert_float_integer_head_blocks_terminal_result(i_type(0, 10, 0b011, 5, 0x03));
}

#[test]
fn float_integer_head_blocks_dependent_terminal_atomic() {
    assert_float_integer_head_blocks_terminal_result(atomic_type(0x00, 11, 10, 0x3, 5));
}

fn assert_float_integer_head_blocks_terminal_result(terminal_result: u32) {
    let (mut scheduler, _transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(10), 0x8800);
    core.write_register(reg(11), 7);
    core.write_float_register(
        rem6_isa_riscv::FloatRegister::new(0).unwrap(),
        (0x9000_u64 as f64).to_bits(),
    );

    let convert_address = fp_r_type(0x61, 2, 0, 0x0, 10);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, convert_address),
            completed_fetch_with_raw(2, 0x8004, terminal_result),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.pending_terminal_memory_result.is_none());
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
}

#[test]
fn vector_head_that_changes_mask_keeps_masked_terminal_load_unprovisioned() {
    let (mut scheduler, _transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));
    core.write_register(reg(10), 0x9000);

    let mut divisor = [0; rem6_isa_riscv::RISCV_VECTOR_REGISTER_BYTES];
    divisor[..8].copy_from_slice(&1_u64.to_le_bytes());
    divisor[8..16].copy_from_slice(&1_u64.to_le_bytes());
    let mut dividend = [0; rem6_isa_riscv::RISCV_VECTOR_REGISTER_BYTES];
    dividend[..8].copy_from_slice(&2_u64.to_le_bytes());
    dividend[8..16].copy_from_slice(&1_u64.to_le_bytes());
    let mut old_mask = [0; rem6_isa_riscv::RISCV_VECTOR_REGISTER_BYTES];
    old_mask[0] = 0b01;
    core.write_vector_register(rem6_isa_riscv::VectorRegister::new(0).unwrap(), old_mask);
    core.write_vector_register(rem6_isa_riscv::VectorRegister::new(1).unwrap(), divisor);
    core.write_vector_register(rem6_isa_riscv::VectorRegister::new(2).unwrap(), dividend);

    let vector_divide = vector_mvv_type(0b100000, 2, 1, 0);
    let masked_load = vector_unit_stride_load_type(false, 0b111, 10, 4);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, vector_divide),
            completed_fetch_with_raw(2, 0x8004, masked_load),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.pending_terminal_memory_result.is_none());
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
}

#[test]
fn faulting_long_latency_head_does_not_authorize_younger_atomic() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(10), 0x9000);
    core.write_register(reg(11), 7);
    let vector_divide = vector_mvv_type(0b100000, 2, 1, 4);
    let amoadd = atomic_type(0x00, 11, 10, 0x3, 5);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, vector_divide),
            completed_fetch_with_raw(2, 0x8004, amoadd),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    scheduler.run_next_epoch();
    let target_calls = Arc::new(AtomicU64::new(0));
    let calls = Arc::clone(&target_calls);
    let issued = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            move |delivery, _context| {
                calls.fetch_add(1, Ordering::Relaxed);
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(9_u64.to_le_bytes().to_vec()),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap();
    scheduler.run_until_idle_conservative();

    assert!(issued.is_none());
    assert_eq!(target_calls.load(Ordering::Relaxed), 0);
    let trapped = core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .expect("faulting vector head retires as a trap");
    assert_eq!(
        trapped.execution().trap().map(|trap| trap.kind()),
        Some(rem6_isa_riscv::RiscvTrapKind::IllegalInstruction)
    );
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.pending_terminal_memory_result.is_none());
    assert!(state.outstanding_data.is_empty());
}

#[test]
fn terminal_result_response_waits_for_older_fu_canonicalization() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);

    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, load),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    let gate_ready_tick = core
        .checkpoint_owned_live_retire_gate_wakes()
        .first()
        .map(|(_, event)| event.tick())
        .expect("older DIV head owns a live-retire gate wake");

    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .is_none());
    scheduler.run_next_epoch_until(3).unwrap();
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(42_u64.to_le_bytes().to_vec()))
                    .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("younger terminal load issues before older DIV writeback");
    scheduler.run_until_idle_conservative();
    assert_eq!(scheduler.now(), gate_ready_tick);
    assert_eq!(core.read_register(reg(5)), 0);
    assert!(core
        .record_ready_o3_data_access_event_with_trace(gate_ready_tick, true)
        .is_none());
    assert!(core
        .execution_events()
        .iter()
        .all(|event| event.fetch_pc() != Address::new(0x8004)));

    assert_eq!(
        core.execute_next_completed_fetch_serial(&mut scheduler)
            .unwrap()
            .expect("older DIV retires at its gate")
            .fetch_pc(),
        Address::new(0x8000)
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(
        core.record_ready_o3_data_access_event_with_trace(gate_ready_tick, true)
            .expect("canonical terminal load publishes after the older DIV")
            .fetch_pc(),
        Address::new(0x8004)
    );
    assert_eq!(core.read_register(reg(5)), 42);
}

#[test]
fn terminal_result_waits_for_its_own_writeback_admission() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_o3_writeback_width(1);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);
    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, load),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    let gate_ready_tick = core
        .checkpoint_owned_live_retire_gate_wakes()
        .first()
        .map(|(_, event)| event.tick())
        .expect("older DIV head owns a live-retire gate wake");
    let issue_tick = gate_ready_tick - 6;
    while scheduler.now() < issue_tick {
        scheduler.run_next_epoch_until(issue_tick).unwrap();
    }
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| {
            TargetOutcome::Respond(
                MemoryResponse::completed(delivery.request(), Some(42_u64.to_le_bytes().to_vec()))
                    .unwrap(),
            )
        },
    )
    .unwrap()
    .expect("younger load issues for a writeback collision");
    scheduler.run_until_idle_conservative();
    assert_eq!(scheduler.now(), gate_ready_tick);
    let result_admitted_tick = core
        .o3_runtime_writeback_reservations()
        .into_iter()
        .map(|reservation| reservation.admitted_tick())
        .max()
        .expect("DIV and load own writeback reservations");
    assert_eq!(result_admitted_tick, gate_ready_tick + 1);

    assert_eq!(
        core.execute_next_completed_fetch_serial(&mut scheduler)
            .unwrap()
            .expect("older DIV retires at its gate")
            .fetch_pc(),
        Address::new(0x8000)
    );
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .is_some());
    assert_eq!(core.pc(), Address::new(0x8004));
    assert!(core
        .execution_events()
        .iter()
        .all(|event| event.fetch_pc() != Address::new(0x8004)));
    scheduler
        .schedule_at(core.partition(), result_admitted_tick, |_| {})
        .unwrap();
    scheduler.run_until_idle_conservative();

    assert_eq!(
        core.execute_next_completed_fetch_serial(&mut scheduler)
            .unwrap()
            .expect("terminal load canonicalizes at its own admission")
            .fetch_pc(),
        Address::new(0x8004)
    );
    assert_eq!(
        core.record_ready_o3_data_access_event_with_trace(result_admitted_tick, true)
            .expect("terminal load publishes at its own admission")
            .fetch_pc(),
        Address::new(0x8004)
    );
    assert_eq!(core.read_register(reg(5)), 42);
}

#[test]
fn interrupt_enabled_head_does_not_provision_younger_terminal_result() {
    let (mut scheduler, _transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);
    core.write_register(reg(11), 7);
    let interrupt_bit = 1_u64 << 1;
    core.set_status(rem6_isa_riscv::RiscvStatusWord::new(0).with_mie(true));
    core.set_machine_interrupt_enable(interrupt_bit);

    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let amoadd = atomic_type(0x00, 11, 10, 0x3, 5);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, amoadd),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .is_none());

    core.set_machine_interrupt_pending(interrupt_bit);
    scheduler.run_until_idle_conservative();
    let interrupted = core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .expect("pending interrupt retires the fixed-FU head");
    assert!(matches!(
        interrupted.execution().trap().map(|trap| trap.kind()),
        Some(rem6_isa_riscv::RiscvTrapKind::Interrupt { code: 1 })
    ));
    assert!(!interrupted.counts_as_retired_instruction());
}

#[test]
fn pending_interrupt_does_not_suppress_already_executed_load_issue() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.write_register(reg(10), 0x9000);
    fetch_and_execute(
        &core,
        &mut scheduler,
        &transport,
        i_type(0, 10, 0b011, 5, 0x03),
    );

    let interrupt_bit = 1_u64 << 1;
    core.set_status(rem6_isa_riscv::RiscvStatusWord::new(0).with_mie(true));
    core.set_machine_interrupt_enable(interrupt_bit);
    core.set_machine_interrupt_pending(interrupt_bit);

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("an older executed load must issue before interrupt delivery");
}

#[test]
fn enabling_interrupt_without_pending_keeps_terminal_issue_wake_live() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);
    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, load),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    core.set_machine_interrupt_enable(1_u64 << 1);
    scheduler.run_next_epoch();
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .as_ref()
        .is_some_and(|pending| pending.issue_ready()));
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("enabled but non-pending interrupt does not strand terminal issue");
}

#[test]
fn terminal_issue_wake_overflow_rolls_back_provisional_owner() {
    let (mut scheduler, _transport, fetch_route, data_route) = memory_routes();
    let near_end = u64::MAX - 1;
    scheduler
        .restore_quiescent(&rem6_kernel::SchedulerSnapshot::new(
            near_end,
            2,
            vec![
                rem6_kernel::PartitionSnapshot::quiescent(PartitionId::new(0), near_end, 0, 0),
                rem6_kernel::PartitionSnapshot::quiescent(PartitionId::new(1), near_end, 0, 0),
            ],
        ))
        .unwrap();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xc8));
    core.write_register(reg(10), 0x9000);

    let vector_shift = vector_vv_type(0b101010, 2, 1, 4);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, vector_shift),
            completed_fetch_with_raw(2, 0x8004, load),
        ]);

    let error = core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap_err();
    assert!(matches!(
        error,
        RiscvCpuError::Scheduler(rem6_kernel::SchedulerError::TickOverflow {
            now,
            delay: 2
        }) if now == near_end
    ));
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.pending_terminal_memory_result.is_none());
    assert!(state.o3_runtime.live_data_access_lifecycle_is_quiescent());
}

#[test]
fn interrupt_after_terminal_provision_discards_pending_result() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);

    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, load),
        ]);

    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .is_some());

    let interrupt_bit = 1_u64 << 1;
    core.set_status(rem6_isa_riscv::RiscvStatusWord::new(0).with_mie(true));
    core.set_machine_interrupt_enable(interrupt_bit);
    core.set_machine_interrupt_pending(interrupt_bit);
    scheduler.run_until_idle_conservative();
    assert!(core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("pending interrupt must suppress terminal issue"),
        )
        .unwrap()
        .is_none());
    let interrupted = core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .expect("pending interrupt discards the provisional result");
    assert!(matches!(
        interrupted.execution().trap().map(|trap| trap.kind()),
        Some(rem6_isa_riscv::RiscvTrapKind::Interrupt { code: 1 })
    ));
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.pending_terminal_memory_result.is_none());
    assert!(state.outstanding_data.is_empty());
    assert!(state.o3_runtime.live_data_access_lifecycle_is_quiescent());
}

#[test]
fn stale_terminal_issue_wake_does_not_rebind_after_redirect() {
    let (mut scheduler, _transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 6);
    core.write_register(reg(10), 0x9000);
    let div = r_type(1, 2, 1, 0x4, 3, 0x33);
    let load = i_type(0, 10, 0b011, 5, 0x03);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(1, 0x8000, div),
            completed_fetch_with_raw(2, 0x8004, load),
        ]);
    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    let old_issue_wake_tick = scheduler.now() + 2;

    core.redirect_pc(Address::new(0x9000));
    let replacement_stage_tick = old_issue_wake_tick - 1;
    scheduler
        .schedule_at(PartitionId::new(0), replacement_stage_tick, |_| {})
        .unwrap();
    scheduler
        .run_next_epoch_until(replacement_stage_tick)
        .unwrap();
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(11, 0x9000, div),
            completed_fetch_with_raw(12, 0x9004, load),
        ]);
    assert!(core
        .execute_next_completed_fetch_serial(&mut scheduler)
        .unwrap()
        .is_none());
    let new_issue_wake_tick = scheduler.now() + 2;
    assert!(!core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .as_ref()
        .expect("new terminal result is provisioned")
        .issue_ready());

    scheduler.run_next_epoch_until(old_issue_wake_tick).unwrap();
    assert_eq!(scheduler.now(), old_issue_wake_tick);
    assert!(!core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .as_ref()
        .expect("new terminal result remains pending")
        .issue_ready());
    scheduler.run_next_epoch_until(new_issue_wake_tick).unwrap();
    assert_eq!(scheduler.now(), new_issue_wake_tick);
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .pending_terminal_memory_result
        .as_ref()
        .expect("new terminal result remains pending")
        .issue_ready());
}

#[test]
fn detailed_mmio_result_is_terminal_and_does_not_fetch_a_younger_div() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0xa000);
    fetch_and_execute(
        &core,
        &mut scheduler,
        &transport,
        i_type(0, 2, 0b011, 12, 0x03),
    );

    let bus = test_mmio_bus(0xa000, 42_u64.to_le_bytes().to_vec());
    assert!(core
        .next_mmio_aware_fetch_ahead_before_retire(&bus)
        .is_none());
    core.issue_next_mmio_data_access_parallel(&mut scheduler, &bus)
        .unwrap()
        .expect("terminal MMIO result issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::None);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
    assert!(state.o3_runtime.writeback_reservations().is_empty());
    drop(state);
    assert!(core
        .core
        .fetch_events()
        .iter()
        .all(|event| event.pc() != Address::new(0x8004)));
}

#[test]
fn detailed_cacheable_scalar_load_keeps_the_only_younger_prefix_policy() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0x9000);
    fetch_and_execute(
        &core,
        &mut scheduler,
        &transport,
        i_type(0, 2, 0b011, 12, 0x03),
    );

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("cacheable scalar load issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::ScalarMemoryPrefix);
}

#[test]
fn detailed_pma_uncacheable_scalar_load_is_terminal() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9008).unwrap())
        .unwrap();
    core.write_register(reg(2), 0x9000);
    fetch_and_execute(
        &core,
        &mut scheduler,
        &transport,
        i_type(0, 2, 0b011, 12, 0x03),
    );

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("PMA-uncacheable scalar load issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::None);
}

#[test]
fn detailed_zero_destination_scalar_load_has_no_younger_prefix_policy() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.write_register(reg(2), 0x9000);
    fetch_and_execute(
        &core,
        &mut scheduler,
        &transport,
        i_type(0, 2, 0b011, 0, 0x03),
    );

    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("zero-destination scalar load issues");
    assert_younger_window_policy(&core, O3DataAccessWindowPolicy::None);
}
