use super::*;
use crate::riscv_live_retire_window::{
    completed_fetch_instruction_starting_with, RiscvCompletedFetchInstruction,
};
use rem6_memory::AddressRange;
fn ld(rd: u8, rs1: u8, offset: i32) -> u32 {
    i_type(offset, rs1, 0b011, rd, 0x03)
}

fn bytes(instruction: u32) -> Vec<u8> {
    instruction.to_le_bytes().to_vec()
}

fn vector_ld(vd: u8, rs1: u8) -> u32 {
    (1_u32 << 25) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(vd) << 7) | 0x07
}

fn atomic_type(operation: u32, acquire: bool, release: bool, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (operation << 27)
        | (u32::from(acquire) << 26)
        | (u32::from(release) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b011 << 12)
        | (u32::from(rd) << 7)
        | 0x2f
}

fn unordered_amo(rd: u8, rs1: u8, rs2: u8) -> u32 {
    atomic_type(0x01, false, false, rs2, rs1, rd)
}

fn lr(rd: u8, rs1: u8) -> u32 {
    atomic_type(0x02, false, false, 0, rs1, rd)
}

fn sc(rd: u8, rs1: u8, rs2: u8) -> u32 {
    atomic_type(0x03, false, false, rs2, rs1, rd)
}

type CompletedResultPair = (
    RiscvCore,
    RiscvCompletedFetchInstruction,
    RiscvCompletedFetchInstruction,
);

fn completed_result_pair(head: u32, younger: u32) -> CompletedResultPair {
    completed_result_with_younger_bytes(head, bytes(younger))
}

fn completed_instruction(
    fetch_events: &[CpuFetchEvent],
    request_id: MemoryRequestId,
) -> RiscvCompletedFetchInstruction {
    let executed_fetches = std::collections::BTreeSet::new();
    let event = fetch_events
        .iter()
        .find(|event| event.request_id() == request_id)
        .expect("fetch event");
    completed_fetch_instruction_starting_with(&executed_fetches, fetch_events, event)
        .expect("decoded instruction")
}

fn completed_result_with_younger_bytes(head: u32, younger: Vec<u8>) -> CompletedResultPair {
    let core = core_with_completed_fetches([(0, 0x8000, bytes(head)), (1, 0x8004, younger)]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    for (register, value) in [(2, 0x9000), (3, 7), (4, 0x9010), (5, 0x9020)] {
        core.write_register(Register::new(register).unwrap(), value);
    }
    core.set_vector_config(rem6_isa_riscv::RiscvVectorConfig::new(2, 0xd8));
    let fetch_state = core.core.state.lock().expect("cpu core lock");
    let head = completed_instruction(&fetch_state.events, request(0));
    let younger = completed_instruction(&fetch_state.events, request(1));
    drop(fetch_state);
    (core, head, younger)
}

fn resolved_head_authorization(
    core: &RiscvCore,
    head: &RiscvCompletedFetchInstruction,
) -> O3MemoryResultWindowAuthorization {
    let state = core.state.lock().expect("riscv core lock");
    let authorization = detailed_o3::data_access_result_fetch_ahead_authorization(
        &state,
        head.first_consumed_request(),
        head.decoded().instruction(),
        head.decoded().bytes(),
        detailed_o3::TranslatedMemoryFetchAhead::Disabled,
    )
    .expect("resolved head authorization");
    assert_eq!(authorization.role(), O3MemoryResultWindowRole::Head);
    authorization
}

fn synthetic_resolved_head_authorization() -> O3MemoryResultWindowAuthorization {
    O3MemoryResultWindowAuthorization::resolved(
        Some(Register::new(5).unwrap()),
        O3MemoryResultWindowRoute::Memory,
        AddressRange::new(Address::new(0x9000), AccessSize::new(8).unwrap()).unwrap(),
        O3MemoryResultWindowRole::Head,
    )
}

fn dependent_authorization(
    core: &RiscvCore,
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
) -> Option<O3MemoryResultWindowAuthorization> {
    let head_authorization = resolved_head_authorization(core, head);
    let state = core.state.lock().expect("riscv core lock");
    let row_limit = state.o3_runtime.scalar_memory_window_limit();
    detailed_o3::dependent_result_address_authorization(
        &state,
        head,
        younger,
        head_authorization,
        row_limit,
    )
}

#[test]
fn dependent_scalar_ld_authorizes_addressless_younger_read() {
    let head_ld = ld(5, 2, 0);
    let (core, head, younger) = completed_result_pair(head_ld, ld(6, 5, 16));

    let authorization =
        dependent_authorization(&core, &head, &younger).expect("dependent address authority");

    assert_eq!(
        authorization.role(),
        O3MemoryResultWindowRole::YoungerDependentRead
    );
    assert_eq!(authorization.route(), O3MemoryResultWindowRoute::Memory);
    assert_eq!(
        authorization.integer_destination(),
        Some(Register::new(6).unwrap())
    );
    assert_eq!(authorization.resolved_range(), None);
    assert_eq!(
        authorization.dependent_source(),
        Some((
            Register::new(5).unwrap(),
            MemoryWidth::Doubleword,
            Immediate::new(16)
        ))
    );
    assert!(!authorization.matches_resolved_range(
        O3MemoryResultWindowRoute::Memory,
        Address::new(0x9000),
        AccessSize::new(8).unwrap(),
    ));

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerDependentRead)
    );
}

#[test]
fn dependent_address_fetch_rejects_non_exact_load_shapes() {
    let head_ld = ld(5, 2, 0);
    let cases = [
        ("younger store", bytes(s_type(0, 6, 5, 0b011))),
        ("younger atomic", bytes(unordered_amo(6, 5, 3))),
        ("younger lr", bytes(lr(6, 5))),
        ("younger sc", bytes(sc(6, 5, 3))),
        ("younger fp load", bytes(i_type(0, 5, 0b011, 1, 0x07))),
        ("younger vector load", bytes(vector_ld(1, 5))),
        ("rd x0", bytes(ld(0, 5, 0))),
        ("non-doubleword ld", bytes(i_type(0, 5, 0b010, 6, 0x03))),
        ("wrong rs1", bytes(ld(6, 4, 0))),
        ("compressed ld", 0x6004_u16.to_le_bytes().to_vec()),
    ];

    for (label, younger) in cases {
        let (core, head, younger) = completed_result_with_younger_bytes(head_ld, younger);
        assert_eq!(
            dependent_authorization(&core, &head, &younger),
            None,
            "{label}"
        );
    }

    let (core, head, younger) = completed_result_pair(head_ld, ld(6, 5, 0));
    let state = core.state.lock().expect("riscv core lock");
    let second_dependent_head = O3MemoryResultWindowAuthorization::dependent(
        Register::new(5).unwrap(),
        Register::new(2).unwrap(),
        MemoryWidth::Doubleword,
        Immediate::new(0),
    );
    assert_eq!(
        detailed_o3::dependent_result_address_authorization(
            &state,
            &head,
            &younger,
            second_dependent_head,
            state.o3_runtime.scalar_memory_window_limit(),
        ),
        None,
        "second dependent result"
    );
}

#[test]
fn dependent_address_authorization_requires_integer_result_head() {
    for (label, head_raw) in [
        ("float head", i_type(0, 2, 0b011, 1, 0x07)),
        ("vector head", vector_ld(1, 2)),
    ] {
        let (core, head, younger) = completed_result_pair(head_raw, ld(6, 5, 0));
        let head_authorization = resolved_head_authorization(&core, &head);
        assert_eq!(head_authorization.integer_destination(), None, "{label}");
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            detailed_o3::dependent_result_address_authorization(
                &state,
                &head,
                &younger,
                head_authorization,
                state.o3_runtime.scalar_memory_window_limit(),
            ),
            None,
            "{label}"
        );
    }

    let (core, head, younger) = completed_result_pair(i_type(0, 2, 0b010, 5, 0x03), ld(6, 5, 0));
    assert_eq!(
        dependent_authorization(&core, &head, &younger),
        None,
        "word load head"
    );
}

#[test]
fn dependent_address_atomic_head_rejects_ordering_and_allows_unordered() {
    let dependent_ld = ld(6, 5, 0);
    let (core, head, younger) = completed_result_pair(unordered_amo(5, 2, 3), dependent_ld);
    assert_eq!(
        dependent_authorization(&core, &head, &younger)
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerDependentRead)
    );

    for (label, head_raw) in [
        ("acquire atomic", atomic_type(0x01, true, false, 3, 2, 5)),
        ("release atomic", atomic_type(0x01, false, true, 3, 2, 5)),
        ("load reserved", lr(5, 2)),
        ("store conditional", sc(5, 2, 3)),
    ] {
        let (core, head, younger) = completed_result_pair(head_raw, dependent_ld);
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            detailed_o3::dependent_result_address_authorization(
                &state,
                &head,
                &younger,
                synthetic_resolved_head_authorization(),
                state.o3_runtime.scalar_memory_window_limit(),
            ),
            None,
            "{label}"
        );
    }
}

#[test]
fn dependent_address_authorization_rejects_translation_and_mmio_heads() {
    let (core, head, younger) = completed_result_pair(ld(5, 2, 0), ld(6, 5, 0));
    let head_authorization = resolved_head_authorization(&core, &head);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.data_translation = Some(CpuTranslationFrontend::with_tlb(
            TranslationQueueConfig::new(4, 0).unwrap(),
            TranslationTlbConfig::new(4).unwrap(),
        ));
    }
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        detailed_o3::dependent_result_address_authorization(
            &state,
            &head,
            &younger,
            head_authorization,
            state.o3_runtime.scalar_memory_window_limit(),
        ),
        None
    );
    drop(state);

    let (mmio_core, mmio_head, mmio_younger) = completed_result_pair(ld(5, 2, 0), ld(6, 5, 0));
    let mmio_state = mmio_core.state.lock().expect("riscv core lock");
    let mmio_head_authorization = detailed_o3::data_access_result_fetch_ahead_authorization(
        &mmio_state,
        mmio_head.first_consumed_request(),
        mmio_head.decoded().instruction(),
        mmio_head.decoded().bytes(),
        detailed_o3::TranslatedMemoryFetchAhead::Mmio,
    )
    .expect("mmio head authorization");
    assert_eq!(
        mmio_head_authorization.route(),
        O3MemoryResultWindowRoute::Mmio
    );
    assert_eq!(
        detailed_o3::dependent_result_address_authorization(
            &mmio_state,
            &mmio_head,
            &mmio_younger,
            mmio_head_authorization,
            mmio_state.o3_runtime.scalar_memory_window_limit(),
        ),
        None
    );
}

#[test]
fn dependent_address_counts_as_second_result_and_blocks_third_result() {
    let core = core_with_completed_fetches([
        (0, 0x8000, ld(5, 2, 0).to_le_bytes().to_vec()),
        (1, 0x8004, ld(6, 5, 0).to_le_bytes().to_vec()),
        (
            2,
            0x8008,
            i_type(0, 4, 0b011, 2, 0x07).to_le_bytes().to_vec(),
        ),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    core.write_register(Register::new(4).unwrap(), 0x9010);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert!(state
        .memory_result_window_authorizations
        .contains_key(&request(0)));
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerDependentRead)
    );
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(2)));
}

#[test]
fn dependent_address_window_remains_four_rows_at_scalar_live_depth_eight() {
    let core = core_with_completed_fetches([
        (0, 0x8000, ld(5, 2, 0).to_le_bytes().to_vec()),
        (1, 0x8004, ld(6, 5, 0).to_le_bytes().to_vec()),
        (2, 0x8008, i_type(1, 0, 0, 7, 0x13).to_le_bytes().to_vec()),
        (3, 0x800c, i_type(2, 0, 0, 8, 0x13).to_le_bytes().to_vec()),
        (4, 0x8010, i_type(3, 0, 0, 9, 0x13).to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_window_depths(4, 8);
    core.write_register(Register::new(2).unwrap(), 0x9000);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.scalar_memory_window_limit(), 4);
    assert_eq!(state.o3_runtime.scalar_live_window_limit(), 8);
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerDependentRead)
    );
}

#[test]
fn retained_unissued_head_preserves_dependent_address_authority() {
    let core = core_with_completed_fetches([
        (0, 0x8000, ld(5, 2, 0).to_le_bytes().to_vec()),
        (1, 0x8004, ld(6, 5, 0).to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    assert_eq!(
        core.execute_next_completed_fetch()
            .unwrap()
            .expect("dependent result head executes")
            .fetch_pc(),
        Address::new(0x8000)
    );

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8008))
    );
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.executed_fetches.contains(&request(0)));
    assert!(!state.issued_data_for_fetches.contains(&request(0)));
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role),
        Some(O3MemoryResultWindowRole::YoungerDependentRead)
    );
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(1))
            .copied()
            .and_then(O3MemoryResultWindowAuthorization::dependent_source),
        Some((
            Register::new(5).unwrap(),
            MemoryWidth::Doubleword,
            Immediate::new(0)
        ))
    );
}
