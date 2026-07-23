use super::*;
use crate::o3_runtime::O3DataAccessWindowPolicy;
use crate::riscv_translation::TranslatedDataAccess;
use rem6_memory::AddressRange;

fn translated_result_pair_core(
    head: u32,
    younger: impl IntoIterator<Item = (u64, u64, Vec<u8>)>,
) -> RiscvCore {
    let core = core_with_completed_fetches(
        [(0, 0x8000, head.to_le_bytes().to_vec())]
            .into_iter()
            .chain(younger),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    for (register, value) in [(2, 0x4000), (3, 0x5000), (4, 0x6000), (11, 0x5000)] {
        core.write_register(Register::new(register).unwrap(), value);
    }
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    install_cached_data_translation(
        &core,
        0x4000,
        0x9000,
        TranslationPagePermissions::read_write(),
        TranslationAccessKind::Load,
    );
    core
}

fn ld(rd: u8, rs1: u8) -> u32 {
    i_type(0, rs1, 0b011, rd, 0x03)
}

fn lw(rd: u8, rs1: u8) -> u32 {
    i_type(0, rs1, 0b010, rd, 0x03)
}

fn load_reserved(rd: u8, rs1: u8) -> u32 {
    (0x02_u32 << 27) | (u32::from(rs1) << 15) | (0b011 << 12) | (u32::from(rd) << 7) | 0x2f
}

fn unordered_atomic(rd: u8, rs1: u8, rs2: u8) -> u32 {
    (0x01_u32 << 27)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b011 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn vector_ld(vd: u8, rs1: u8) -> u32 {
    (1_u32 << 25) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(vd) << 7) | 0x07
}

fn scalar_ld_execution_event(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: Register::new(rd).unwrap(),
        rs1: Register::new(rs1).unwrap(),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: Register::new(rd).unwrap(),
        address,
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        CpuFetchEvent::completed(
            CpuFetchRecord::new(
                4,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            ld(rd, rs1).to_le_bytes().to_vec(),
        ),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn translated_result_pair_is_authorized(
    head: u32,
    younger: Vec<u8>,
    configure_vector: bool,
) -> bool {
    let core = translated_result_pair_core(head, [(1, 0x8004, younger)]);
    if configure_vector {
        core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));
    }

    let _ = core.next_cached_translated_memory_fetch_ahead_before_retire();

    let authorized = !core
        .state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .is_empty();
    authorized
}

#[test]
fn translated_result_pair_authorizes_two_virtual_rows_without_physical_targets() {
    let head = ld(11, 2);
    let younger = ld(12, 3);
    let core = translated_result_pair_core(head, [(1, 0x8004, younger.to_le_bytes().to_vec())]);

    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    let head_authorization = state
        .memory_result_window_authorizations
        .get(&request(0))
        .copied()
        .expect("translated head authorization");
    let younger_authorization = state
        .memory_result_window_authorizations
        .get(&request(1))
        .copied()
        .expect("translated younger authorization");

    assert_eq!(head_authorization.role(), O3MemoryResultWindowRole::Head);
    assert_eq!(
        younger_authorization.role(),
        O3MemoryResultWindowRole::YoungerRead
    );
    assert!(head_authorization.is_translated());
    assert!(younger_authorization.is_translated());
    assert_eq!(
        head_authorization.virtual_range(),
        AddressRange::new(Address::new(0x4000), AccessSize::new(8).unwrap()).ok()
    );
    assert_eq!(
        younger_authorization.virtual_range(),
        AddressRange::new(Address::new(0x5000), AccessSize::new(8).unwrap()).ok()
    );
    assert!(!head_authorization.matches_bound_target(
        O3MemoryResultWindowRoute::Memory,
        Address::new(0x9000),
        AccessSize::new(8).unwrap(),
    ));
    assert!(!younger_authorization.matches_bound_target(
        O3MemoryResultWindowRoute::Memory,
        Address::new(0xa000),
        AccessSize::new(8).unwrap(),
    ));
}

#[test]
fn ready_translated_result_pair_fetches_missing_fourth_row_before_head_issue() {
    let head = ld(11, 2);
    let younger = ld(12, 3);
    let div = r_type(0x01, 2, 1, 0b100, 3, 0x33);
    let core = translated_result_pair_core(
        head,
        [
            (1, 0x8004, younger.to_le_bytes().to_vec()),
            (2, 0x8008, div.to_le_bytes().to_vec()),
        ],
    );
    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );

    let execution = core
        .execute_next_completed_fetch()
        .unwrap()
        .expect("translated head executes");
    let fetch_request = execution.fetch().request_id();
    core.state
        .lock()
        .expect("riscv core lock")
        .ready_translated_data
        .insert(
            fetch_request,
            TranslatedDataAccess {
                request_id: request(99),
                fetch_request,
                access: MemoryAccessKind::Load {
                    rd: Register::new(11).unwrap(),
                    address: 0x4000,
                    width: MemoryWidth::Doubleword,
                    signed: false,
                },
                virtual_address: Address::new(0x4000),
                size: AccessSize::new(8).unwrap(),
                physical_address: Address::new(0x9000),
                request_byte_offset: 0,
            },
        );
    assert_eq!(
        core.next_ready_translated_memory_fetch_ahead_before_issue(fetch_request)
            .map(|decision| decision.pc()),
        Some(Address::new(0x800c))
    );
}

#[test]
fn retained_translated_result_pair_defers_mmio_until_mixed_pair_support() {
    let core =
        translated_result_pair_core(ld(11, 2), [(1, 0x8004, ld(12, 3).to_le_bytes().to_vec())]);
    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    core.execute_next_completed_fetch()
        .unwrap()
        .expect("translated head executes");

    let fetch_events = core.core.fetch_events();
    let state = core.state.lock().expect("riscv core lock");
    assert!(detailed_o3::retained_data_access_result_window_candidate(
        &state,
        &fetch_events,
        detailed_o3::TranslatedMemoryFetchAhead::CachedMemory,
    )
    .is_some());
    assert!(detailed_o3::retained_data_access_result_window_candidate(
        &state,
        &fetch_events,
        detailed_o3::TranslatedMemoryFetchAhead::Mmio,
    )
    .is_none());
}

#[test]
fn translated_result_pair_requires_exact_scalar_ld_heads_and_distinct_destinations() {
    let younger = ld(12, 3).to_le_bytes().to_vec();
    let mut admitted = [
        ("floating-point head", i_type(0, 2, 0b011, 1, 0x07), false),
        ("vector head", vector_ld(1, 2), true),
        ("load-reserved head", load_reserved(11, 2), false),
        ("word-load head", lw(11, 2), false),
        ("zero-destination head", ld(0, 2), false),
    ]
    .into_iter()
    .filter_map(|(label, head, configure_vector)| {
        translated_result_pair_is_authorized(head, younger.clone(), configure_vector)
            .then_some(label)
    })
    .collect::<Vec<_>>();
    if translated_result_pair_is_authorized(ld(11, 2), ld(11, 3).to_le_bytes().to_vec(), false) {
        admitted.push("duplicate destination");
    }
    assert!(
        admitted.is_empty(),
        "admitted translated pairs: {admitted:?}"
    );
}

#[test]
fn translated_result_pair_rejects_unsupported_younger_results() {
    let admitted = [
        ("zero destination", ld(0, 3).to_le_bytes().to_vec(), false),
        ("word load", lw(12, 3).to_le_bytes().to_vec(), false),
        (
            "store",
            s_type(0, 4, 3, 0b011).to_le_bytes().to_vec(),
            false,
        ),
        (
            "floating-point load",
            i_type(0, 3, 0b011, 1, 0x07).to_le_bytes().to_vec(),
            false,
        ),
        ("vector load", vector_ld(1, 3).to_le_bytes().to_vec(), true),
        (
            "atomic",
            unordered_atomic(12, 3, 4).to_le_bytes().to_vec(),
            false,
        ),
        ("compressed load", 0x6004_u16.to_le_bytes().to_vec(), false),
    ]
    .into_iter()
    .filter_map(|(label, younger, configure_vector)| {
        translated_result_pair_is_authorized(ld(11, 2), younger, configure_vector).then_some(label)
    })
    .collect::<Vec<_>>();
    assert!(
        admitted.is_empty(),
        "admitted translated pairs: {admitted:?}"
    );
}

#[test]
fn translated_result_pair_overlap_requires_exact_authorized_virtual_span() {
    let head = ld(11, 2);
    let younger = ld(12, 3);
    let core = translated_result_pair_core(head, [(1, 0x8004, younger.to_le_bytes().to_vec())]);
    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    let head_execution = scalar_ld_execution_event(0x8000, 0, 11, 2, 0x4000);
    let exact = RiscvInstruction::Load {
        rd: Register::new(12).unwrap(),
        rs1: Register::new(3).unwrap(),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    let mut state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.stage_live_data_access_issue(
        &head_execution,
        request(99),
        1,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    assert!(state.can_overlap_detailed_memory_result_instruction(request(1), exact));

    state.hart.write(Register::new(3).unwrap(), 0x5008);
    assert!(!state.can_overlap_detailed_memory_result_instruction(request(1), exact));

    state.hart.write(Register::new(3).unwrap(), 0x5000);
    let wrong_size = RiscvInstruction::Load {
        rd: Register::new(12).unwrap(),
        rs1: Register::new(3).unwrap(),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    assert!(!state.can_overlap_detailed_memory_result_instruction(request(1), wrong_size));
}

#[test]
fn translated_result_pair_rejects_overlapping_virtual_ranges() {
    let core =
        translated_result_pair_core(ld(11, 2), [(1, 0x8004, ld(12, 3).to_le_bytes().to_vec())]);
    core.write_register(Register::new(3).unwrap(), 0x4000);

    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire(),
        None
    );
    assert!(core
        .state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .is_empty());
}

#[test]
fn translated_result_pair_at_depth_two_retains_two_authorizations_without_next_fetch() {
    let head = ld(11, 2);
    let younger = ld(12, 3);
    let core = translated_result_pair_core(head, [(1, 0x8004, younger.to_le_bytes().to_vec())]);
    core.set_o3_scalar_memory_depth(2);

    assert_eq!(
        core.next_cached_translated_memory_fetch_ahead_before_retire(),
        None
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(0))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::Head)
    );
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerRead)
    );
}

#[test]
fn translated_result_pair_binds_each_physical_range_and_target_once() {
    let virtual_range =
        AddressRange::new(Address::new(0x5000), AccessSize::new(8).unwrap()).unwrap();
    let mut authorization = O3MemoryResultWindowAuthorization::translated_unbound(
        Some(Register::new(12).unwrap()),
        virtual_range,
        O3MemoryResultWindowRole::YoungerRead,
    );

    assert!(authorization.bind_translated(
        virtual_range.start(),
        Address::new(0x8000_2000),
        virtual_range.size(),
    ));
    assert!(authorization.bind_translated(
        virtual_range.start(),
        Address::new(0x8000_2000),
        virtual_range.size(),
    ));
    assert!(authorization.bind_target(O3MemoryResultWindowRoute::Memory));
    assert!(authorization.bind_target(O3MemoryResultWindowRoute::Memory));
    assert!(authorization.matches_bound_target(
        O3MemoryResultWindowRoute::Memory,
        Address::new(0x8000_2000),
        virtual_range.size(),
    ));
    assert!(!authorization.bind_target(O3MemoryResultWindowRoute::Mmio));
}

#[test]
fn translated_result_pair_rejects_wrong_virtual_span_rebind_and_target_change() {
    let virtual_range =
        AddressRange::new(Address::new(0x5000), AccessSize::new(8).unwrap()).unwrap();
    let mut authorization = O3MemoryResultWindowAuthorization::translated_unbound(
        Some(Register::new(12).unwrap()),
        virtual_range,
        O3MemoryResultWindowRole::YoungerRead,
    );

    assert!(!authorization.bind_translated(
        Address::new(0x5008),
        Address::new(0x8000_2000),
        virtual_range.size(),
    ));
    assert!(!authorization.bind_translated(
        virtual_range.start(),
        Address::new(0x8000_2000),
        AccessSize::new(4).unwrap(),
    ));
    assert!(authorization.bind_translated(
        virtual_range.start(),
        Address::new(0x8000_2000),
        virtual_range.size(),
    ));
    assert!(!authorization.bind_translated(
        virtual_range.start(),
        Address::new(0x8000_3000),
        virtual_range.size(),
    ));
    assert!(authorization.bind_target(O3MemoryResultWindowRoute::Memory));
    assert!(!authorization.bind_target(O3MemoryResultWindowRoute::Mmio));
    assert!(authorization.matches_bound_target(
        O3MemoryResultWindowRoute::Memory,
        Address::new(0x8000_2000),
        virtual_range.size(),
    ));
    assert!(!authorization.matches_bound_target(
        O3MemoryResultWindowRoute::Memory,
        Address::new(0x8000_3000),
        virtual_range.size(),
    ));
}

#[test]
fn translated_result_pair_rejects_dependent_second_address_and_third_result() {
    let dependent =
        translated_result_pair_core(ld(11, 2), [(1, 0x8004, ld(12, 11).to_le_bytes().to_vec())]);
    assert_eq!(
        dependent.next_cached_translated_memory_fetch_ahead_before_retire(),
        None
    );
    assert!(dependent
        .state
        .lock()
        .expect("riscv core lock")
        .memory_result_window_authorizations
        .is_empty());

    let head = ld(11, 2);
    let second = ld(12, 3);
    let third = ld(13, 4);
    let three_results = translated_result_pair_core(
        head,
        [
            (1, 0x8004, second.to_le_bytes().to_vec()),
            (2, 0x8008, third.to_le_bytes().to_vec()),
        ],
    );
    assert_eq!(
        three_results.next_cached_translated_memory_fetch_ahead_before_retire(),
        None
    );
    let state = three_results.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(0))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::Head)
    );
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerRead)
    );
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(2)));
}
