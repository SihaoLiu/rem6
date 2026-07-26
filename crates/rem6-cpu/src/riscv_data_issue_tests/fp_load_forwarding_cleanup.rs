use rem6_isa_riscv::{FloatRegister, MemoryWidth};

use super::*;
use crate::o3_runtime::O3ArchitecturalRegister;

const HEAD_PC: u64 = 0x8000;
const YOUNGER_PC: u64 = 0x8004;
const CONSUMER_PC: u64 = 0x8008;
const HEAD_FETCH_SEQUENCE: u64 = 0;
const YOUNGER_FETCH_SEQUENCE: u64 = 1;

#[derive(Clone, Copy, Debug)]
enum TerminalKind {
    Retry,
    Failed,
}

impl TerminalKind {
    const fn event_kind(self) -> RiscvDataAccessEventKind {
        match self {
            Self::Retry => RiscvDataAccessEventKind::Retry,
            Self::Failed => RiscvDataAccessEventKind::Failed,
        }
    }
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn completed_fetch(sequence: u64, pc: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn float_load_raw(base: u8, destination: u8, width: MemoryWidth) -> u32 {
    let funct3 = match width {
        MemoryWidth::Word => 0b010,
        MemoryWidth::Doubleword => 0b011,
        _ => panic!("FP forwarding cleanup only covers FLW and FLD"),
    };
    i_type(0, base, funct3, destination, 0x07)
}

fn float_mul_raw(source: u8, width: MemoryWidth) -> u32 {
    let funct7 = match width {
        MemoryWidth::Word => 0b0001000,
        MemoryWidth::Doubleword => 0b0001001,
        _ => panic!("FP forwarding cleanup only covers FLW and FLD"),
    };
    (funct7 << 25) | (8 << 20) | (u32::from(source) << 15) | (7 << 7) | 0x53
}

fn atomic_raw(base: u8, value: u8, destination: u8) -> u32 {
    (0x01_u32 << 27)
        | (u32::from(value) << 20)
        | (u32::from(base) << 15)
        | (0b011 << 12)
        | (u32::from(destination) << 7)
        | 0x2f
}

fn fp_pair_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    width: MemoryWidth,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, HEAD_PC),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_window_depths(4, 4);
    core.write_register(reg(10), 0x9000);
    core.write_register(reg(11), 0x9040);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch(HEAD_FETCH_SEQUENCE, HEAD_PC, float_load_raw(10, 5, width)),
            completed_fetch(
                YOUNGER_FETCH_SEQUENCE,
                YOUNGER_PC,
                float_load_raw(11, 6, width),
            ),
            completed_fetch(2, CONSUMER_PC, float_mul_raw(6, width)),
        ]);
    core
}

fn buffered_atomic_core(
    fetch_route: MemoryRouteId,
    data_route: MemoryRouteId,
    width: MemoryWidth,
) -> RiscvCore {
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, HEAD_PC),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(10), 0x9000);
    core.write_register(reg(11), 0x9040);
    core.write_register(reg(12), 7);
    core.core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .extend([
            completed_fetch(HEAD_FETCH_SEQUENCE, HEAD_PC, float_load_raw(10, 5, width)),
            completed_fetch(YOUNGER_FETCH_SEQUENCE, YOUNGER_PC, atomic_raw(11, 12, 13)),
        ]);
    core
}

fn issue_head(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    terminal: TerminalKind,
) {
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("FP result head executes");
    match terminal {
        TerminalKind::Retry => core.issue_next_data_access(
            scheduler,
            transport,
            MemoryTrace::new(),
            |delivery, _context| TargetOutcome::RespondAfter {
                delay: 20,
                response: MemoryResponse::retry(delivery.request()),
            },
        ),
        TerminalKind::Failed => core.issue_next_data_access(
            scheduler,
            transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        ),
    }
    .unwrap()
    .expect("FP result head issues");
}

fn issue_younger_without_response(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
) {
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("authorized younger memory result executes");
    core.issue_next_data_access(
        scheduler,
        transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("authorized younger memory result issues");
}

fn terminate_head(
    core: &RiscvCore,
    scheduler: &mut PartitionedScheduler,
    terminal: TerminalKind,
    older_request: MemoryRequestId,
) {
    match terminal {
        TerminalKind::Retry => {
            scheduler.run_until_idle_conservative();
        }
        TerminalKind::Failed => {
            core.record_data_failure(older_request, scheduler.now());
        }
    }
}

fn assert_fp_pair_cleanup_and_fresh_attempt(terminal: TerminalKind, width: MemoryWidth) {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = fp_pair_core(fetch_route, data_route, width);
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    issue_head(&core, &mut scheduler, &transport, terminal);
    issue_younger_without_response(&core, &mut scheduler, &transport);

    let requests = outstanding_data_requests_in_fetch_order(&core);
    assert_eq!(requests.len(), 2);
    let (older_fetch, older_request) = requests[0];
    let (younger_fetch, younger_request) = requests[1];
    assert_eq!(older_fetch, request(HEAD_FETCH_SEQUENCE));
    assert_eq!(younger_fetch, request(YOUNGER_FETCH_SEQUENCE));
    let old_younger_sequence = {
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.outstanding_data.contains_key(&younger_request));
        assert!(state.issued_data_for_fetches.contains(&younger_fetch));
        assert!(state.buffered_o3_effects.is_empty());
        state
            .o3_runtime
            .live_data_access_issue_identity_for_test(younger_fetch)
            .expect("younger FP request owns one live runtime row")
            .0
    };

    terminate_head(&core, &mut scheduler, terminal, older_request);

    {
        let state = core.state.lock().expect("riscv core lock");
        assert!(!state.outstanding_data.contains_key(&younger_request));
        assert!(!state.issued_data_for_fetches.contains(&younger_fetch));
        assert!(state.buffered_o3_effects.is_empty());
        assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 0);
        assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 0);
        assert_eq!(
            state.o3_runtime.live_issue_forwarding_artifacts_for_test(
                old_younger_sequence,
                O3ArchitecturalRegister::floating_point(FloatRegister::new(6).unwrap()),
            ),
            (false, None, false),
        );
    }

    let terminal_event = core
        .record_ready_o3_data_access_event_with_trace(u64::MAX, true)
        .expect("terminal FP head drains before replay");
    assert_eq!(
        terminal_event.data_access_event_kind(),
        Some(terminal.event_kind())
    );
    assert!(core.has_unissued_data_access());
    core.issue_next_data_access(
        &mut scheduler,
        &transport,
        MemoryTrace::new(),
        |_delivery, _context| TargetOutcome::NoResponse,
    )
    .unwrap()
    .expect("fresh same-destination FP attempt issues");

    let fresh = outstanding_data_requests_in_fetch_order(&core);
    assert_eq!(fresh.len(), 1);
    let (fresh_fetch, fresh_request) = fresh[0];
    assert_eq!(fresh_fetch, younger_fetch);
    assert_ne!(fresh_request, younger_request);
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.issued_data_for_fetches.contains(&fresh_fetch));
    let fresh_sequence = state
        .o3_runtime
        .live_data_access_issue_identity_for_test(fresh_fetch)
        .expect("fresh FP request owns one live runtime row")
        .0;
    assert_ne!(fresh_sequence, old_younger_sequence);
    assert_eq!(
        state.o3_runtime.live_issue_forwarding_artifacts_for_test(
            fresh_sequence,
            O3ArchitecturalRegister::floating_point(FloatRegister::new(6).unwrap()),
        ),
        (false, None, false),
    );
}

fn assert_buffered_effect_cleanup(terminal: TerminalKind, width: MemoryWidth) {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = buffered_atomic_core(fetch_route, data_route, width);
    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(CONSUMER_PC)),
    );
    issue_head(&core, &mut scheduler, &transport, terminal);
    issue_younger_without_response(&core, &mut scheduler, &transport);

    let requests = outstanding_data_requests_in_fetch_order(&core);
    assert_eq!(requests.len(), 2);
    let (_, older_request) = requests[0];
    let (younger_fetch, younger_request) = requests[1];
    {
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.outstanding_data.contains_key(&younger_request));
        assert!(state.issued_data_for_fetches.contains(&younger_fetch));
        assert!(state.buffered_o3_effects.contains_key(&younger_request));
    }

    terminate_head(&core, &mut scheduler, terminal, older_request);

    let state = core.state.lock().expect("riscv core lock");
    assert!(!state.outstanding_data.contains_key(&younger_request));
    assert!(!state.issued_data_for_fetches.contains(&younger_fetch));
    assert!(!state.buffered_o3_effects.contains_key(&younger_request));
}

#[test]
fn fp_load_retry_cleans_production_request_maps_before_fresh_flw_attempt() {
    assert_fp_pair_cleanup_and_fresh_attempt(TerminalKind::Retry, MemoryWidth::Word);
    assert_buffered_effect_cleanup(TerminalKind::Retry, MemoryWidth::Word);
}

#[test]
fn fp_load_failure_cleans_production_request_maps_before_fresh_fld_attempt() {
    assert_fp_pair_cleanup_and_fresh_attempt(TerminalKind::Failed, MemoryWidth::Doubleword);
    assert_buffered_effect_cleanup(TerminalKind::Failed, MemoryWidth::Doubleword);
}
