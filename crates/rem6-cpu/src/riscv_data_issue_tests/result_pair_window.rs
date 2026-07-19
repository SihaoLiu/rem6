use super::*;

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn completed_fetch_with_raw(sequence: u64, pc: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            rem6_kernel::PartitionId::new(0),
            rem6_transport::MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            request(sequence),
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

fn result_pair_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(3), 0x9010);
    core.write_register(reg(4), 2);
    let float_head = i_type(0, 2, 0b011, 1, 0x07);
    let second_load = i_type(0, 3, 0b011, 13, 0x03);
    let div = r_type(1, 4, 1, 0b100, 20, 0x33);
    let dependent = i_type(1, 13, 0, 21, 0x13);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(0, 0x8000, float_head),
            completed_fetch_with_raw(1, 0x8004, second_load),
            completed_fetch_with_raw(2, 0x8008, div),
            completed_fetch_with_raw(3, 0x800c, dependent),
        ]);
    core
}

fn atomic_float_pair_core(fetch_route: MemoryRouteId, data_route: MemoryRouteId) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(1), 84);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(3), 7);
    core.write_register(reg(4), 0x9010);
    let atomic = (0x01_u32 << 27) | (3 << 20) | (2 << 15) | (0b011 << 12) | (11 << 7) | 0x2f;
    let second_float = i_type(0, 4, 0b011, 2, 0x07);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch_with_raw(0, 0x8000, atomic),
            completed_fetch_with_raw(1, 0x8004, second_float),
        ]);
    core
}

fn issue_without_response(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.issue_next_data_access(
        scheduler,
        transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("authorized result request issues");
}

#[test]
fn authorized_second_result_executes_and_issues_while_the_head_is_outstanding() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = result_pair_core(fetch_route, data_route);
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.memory_result_window_authorizations.len(), 2);
        assert_eq!(
            state
                .memory_result_window_authorizations
                .get(&request(0))
                .copied()
                .map(|authorization| authorization.role()),
            Some(crate::riscv_fetch_ahead::O3MemoryResultWindowRole::Head)
        );
        assert_eq!(
            state
                .memory_result_window_authorizations
                .get(&request(1))
                .copied()
                .map(|authorization| authorization.role()),
            Some(crate::riscv_fetch_ahead::O3MemoryResultWindowRole::YoungerRead)
        );
    }

    assert_eq!(
        core.execute_next_completed_fetch()
            .unwrap()
            .expect("result head executes")
            .fetch_pc(),
        Address::new(0x8000)
    );
    issue_without_response(&core, &mut scheduler, &transport);
    assert_eq!(
        core.execute_next_completed_fetch()
            .unwrap()
            .expect("authorized second result executes")
            .fetch_pc(),
        Address::new(0x8004)
    );
    issue_without_response(&core, &mut scheduler, &transport);

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.memory_result_window_authorizations.is_empty());
    assert_eq!(
        state.o3_runtime.live_data_access_younger_window_policies(),
        vec![
            O3DataAccessWindowPolicy::MemoryResultWindow,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ]
    );
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 2);
}

#[test]
fn authorized_float_read_issues_while_a_disjoint_atomic_head_is_outstanding() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = atomic_float_pair_core(fetch_route, data_route);
    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .memory_result_window_authorizations
            .len(),
        2
    );

    core.execute_next_completed_fetch()
        .unwrap()
        .expect("atomic head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .live_data_access_younger_window_policies(),
        vec![O3DataAccessWindowPolicy::MemoryResultWindow]
    );
    assert!(!core.pending_data_access_blocks_new_work());
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(|authorization| authorization.role()),
        Some(crate::riscv_fetch_ahead::O3MemoryResultWindowRole::YoungerRead)
    );

    core.execute_next_completed_fetch()
        .unwrap()
        .expect("authorized float read executes");
    issue_without_response(&core, &mut scheduler, &transport);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.o3_runtime.live_data_access_younger_window_policies(),
        vec![
            O3DataAccessWindowPolicy::MemoryResultWindow,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ]
    );
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 3);
}

#[test]
fn disabling_detailed_mode_discards_unissued_younger_result_authority() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = atomic_float_pair_core(fetch_route, data_route);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("atomic head executes");
    issue_without_response(&core, &mut scheduler, &transport);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger result executes before mode disable");
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .memory_result_window_authorizations
            .len(),
        1
    );
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .owns_pending_live_data_access_retirement(request(1)));

    core.set_detailed_live_retire_gate_enabled(false);

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.memory_result_window_authorizations.is_empty());
    assert!(!state.can_extend_detailed_memory_result_window());
    assert!(!state
        .o3_runtime
        .owns_pending_live_data_access_retirement(request(1)));
}

#[test]
fn aborting_pair_head_discards_the_younger_authorization() {
    let (_scheduler, _transport, fetch_route, data_route) = memory_routes();
    let core = atomic_float_pair_core(fetch_route, data_route);
    core.next_fetch_ahead_before_retire();
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("atomic head executes");

    let mut state = core.state.lock().expect("riscv core lock");
    assert!(state.abort_deferred_o3_live_data_access_execution(request(0)));
    assert!(state.memory_result_window_authorizations.is_empty());
}

#[test]
fn failed_pair_head_issue_discards_both_authorizations() {
    let (_scheduler, transport, fetch_route, data_route) = memory_routes();
    let mut rejecting_scheduler = PartitionedScheduler::with_min_remote_delay(2, 3).unwrap();
    let core = result_pair_core(fetch_route, data_route);
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("result head executes");

    let error = core
        .issue_next_data_access(
            &mut rejecting_scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap_err();
    assert!(matches!(error, RiscvCpuError::Transport(_)));
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.memory_result_window_authorizations.is_empty());
    assert!(!state
        .o3_runtime
        .owns_pending_live_data_access_retirement(request(0)));
}

#[test]
fn older_retry_discards_executed_but_unissued_younger_result_authority() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = result_pair_core(fetch_route, data_route);
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("result head executes");
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |delivery, _context| TargetOutcome::Respond(MemoryResponse::retry(delivery.request())),
    )
    .unwrap()
    .expect("result head issues");
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("younger result executes before the retry");
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .owns_pending_live_data_access_retirement(request(1)));

    scheduler.run_until_idle_conservative();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.memory_result_window_authorizations.is_empty());
    assert!(!state
        .o3_runtime
        .owns_pending_live_data_access_retirement(request(1)));
}

#[test]
fn younger_result_authorization_revalidates_range_pma_and_translation() {
    let second = rem6_isa_riscv::RiscvInstruction::Load {
        rd: reg(13),
        rs1: reg(3),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    for stale in ["range", "pma", "translation"] {
        let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
        let core = result_pair_core(fetch_route, data_route);
        assert_eq!(core.next_fetch_ahead_before_retire(), None);
        core.execute_next_completed_fetch()
            .unwrap()
            .expect("result head executes");
        issue_without_response(&core, &mut scheduler, &transport);
        match stale {
            "range" => core.write_register(reg(3), 0xa000),
            "pma" => core
                .add_pma_uncacheable_range(RiscvPmaRange::new(0x9010, 0x9018).unwrap())
                .unwrap(),
            "translation" => {
                core.state.lock().expect("riscv core lock").data_translation = Some(
                    CpuTranslationFrontend::new(TranslationQueueConfig::new(4, 0).unwrap()),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            !core
                .state
                .lock()
                .expect("riscv core lock")
                .can_overlap_detailed_memory_result_instruction(request(1), second),
            "{stale} authorization"
        );
    }
}
