use super::*;
use crate::riscv_live_retire_window::{
    completed_fetch_instruction_starting_with, RiscvCompletedFetchInstruction,
};

fn ld(rd: u8, rs1: u8, offset: i32) -> u32 {
    i_type(offset, rs1, 0b011, rd, 0x03)
}

fn bytes(instruction: u32) -> Vec<u8> {
    instruction.to_le_bytes().to_vec()
}

fn add(rd: u8, rs1: u8, rs2: u8) -> u32 {
    r_type(0, rs2, rs1, 0, rd, 0x33)
}

fn window_core(fetches: Vec<(u64, u64, Vec<u8>)>) -> RiscvCore {
    let core = core_with_completed_fetches(fetches);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0x9000);
    core.write_register(reg(4), 0x9010);
    core
}

type CompletedResultTriple = (
    RiscvCore,
    RiscvCompletedFetchInstruction,
    RiscvCompletedFetchInstruction,
    RiscvCompletedFetchInstruction,
);

type CompletedResultQuad = (
    RiscvCore,
    RiscvCompletedFetchInstruction,
    RiscvCompletedFetchInstruction,
    RiscvCompletedFetchInstruction,
    RiscvCompletedFetchInstruction,
);

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

fn completed_result_triple(head: u32, first: u32, second: u32) -> CompletedResultTriple {
    let core = core_with_completed_fetches([
        (0, 0x8000, bytes(head)),
        (1, 0x8004, bytes(first)),
        (2, 0x8008, bytes(second)),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    let fetch_state = core.core.state.lock().expect("cpu core lock");
    let head = completed_instruction(&fetch_state.events, request(0));
    let first = completed_instruction(&fetch_state.events, request(1));
    let second = completed_instruction(&fetch_state.events, request(2));
    drop(fetch_state);
    (core, head, first, second)
}

fn completed_result_quad(head: u32, first: u32, second: u32, third: u32) -> CompletedResultQuad {
    let core = core_with_completed_fetches([
        (0, 0x8000, bytes(head)),
        (1, 0x8004, bytes(first)),
        (2, 0x8008, bytes(second)),
        (3, 0x800c, bytes(third)),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    let fetch_state = core.core.state.lock().expect("cpu core lock");
    let head = completed_instruction(&fetch_state.events, request(0));
    let first = completed_instruction(&fetch_state.events, request(1));
    let second = completed_instruction(&fetch_state.events, request(2));
    let third = completed_instruction(&fetch_state.events, request(3));
    drop(fetch_state);
    (core, head, first, second, third)
}

fn reg(raw: u8) -> Register {
    Register::new(raw).unwrap()
}

fn resolved_head_authorization(
    core: &RiscvCore,
    head: &RiscvCompletedFetchInstruction,
) -> O3MemoryResultWindowAuthorization {
    let state = core.state.lock().expect("riscv core lock");
    detailed_o3::data_access_result_fetch_ahead_authorization(
        &state,
        head.first_consumed_request(),
        head.decoded().instruction(),
        head.decoded().bytes(),
        detailed_o3::TranslatedMemoryFetchAhead::Disabled,
    )
    .expect("resolved head authorization")
}

fn authorizer_for(
    core: &RiscvCore,
    head: &RiscvCompletedFetchInstruction,
) -> detailed_o3::DependentResultAddressAuthorizer {
    let state = core.state.lock().expect("riscv core lock");
    let row_limit = state.o3_runtime.scalar_memory_window_limit();
    drop(state);
    authorizer_for_with_row_limit(core, head, row_limit)
}

fn authorizer_for_with_row_limit(
    core: &RiscvCore,
    head: &RiscvCompletedFetchInstruction,
    row_limit: usize,
) -> detailed_o3::DependentResultAddressAuthorizer {
    let head_authorization = resolved_head_authorization(core, head);
    let state = core.state.lock().expect("riscv core lock");
    detailed_o3::DependentResultAddressAuthorizer::from_head(
        &state,
        head,
        head_authorization,
        row_limit,
    )
    .expect("dependent result address authorizer")
}

fn assert_dependent_source(
    authorization: O3MemoryResultWindowAuthorization,
    rd: u8,
    rs1: u8,
    offset: i64,
) {
    assert_eq!(
        authorization.integer_destination(),
        Some(reg(rd)),
        "dependent destination"
    );
    assert_eq!(
        authorization.dependent_source(),
        Some((reg(rs1), MemoryWidth::Doubleword, Immediate::new(offset))),
        "dependent source"
    );
    assert_eq!(
        authorization.role(),
        O3MemoryResultWindowRole::YoungerDependentRead
    );
}

#[test]
fn dependent_address_two_pending_authorizes_sibling_loads_before_suffix() {
    let (core, head, first, second) =
        completed_result_triple(ld(5, 2, 0), ld(6, 5, 8), ld(7, 5, 16));
    let mut authorizer = authorizer_for(&core, &head);

    let first_authorization = authorizer
        .try_authorize_next(&first)
        .expect("first dependent sibling");
    let second_authorization = authorizer
        .try_authorize_next(&second)
        .expect("second dependent sibling");

    assert_dependent_source(first_authorization, 6, 5, 8);
    assert_dependent_source(second_authorization, 7, 5, 16);
    assert_eq!(authorizer.dependent_rows(), 2);
    assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6), reg(7)]);
}

#[test]
fn dependent_address_two_pending_authorizes_one_deep_chain_before_suffix() {
    let (core, head, first, second) =
        completed_result_triple(ld(5, 2, 0), ld(6, 5, 8), ld(7, 6, 16));
    let mut authorizer = authorizer_for(&core, &head);

    let first_authorization = authorizer
        .try_authorize_next(&first)
        .expect("first dependent row");
    let second_authorization = authorizer
        .try_authorize_next(&second)
        .expect("one-deep dependent chain");

    assert_dependent_source(first_authorization, 6, 5, 8);
    assert_dependent_source(second_authorization, 7, 6, 16);
    assert_eq!(authorizer.dependent_rows(), 2);
    assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6), reg(7)]);

    let (core, head, first, second) =
        completed_result_triple(ld(5, 2, 0), ld(6, 5, 8), ld(7, 6, 16));
    let mut authorizer = authorizer_for_with_row_limit(&core, &head, 3);
    assert!(authorizer.try_authorize_next(&first).is_some());
    assert_eq!(authorizer.try_authorize_next(&second), None);
    assert_eq!(authorizer.dependent_rows(), 1);
    assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6)]);
}

#[test]
fn dependent_address_two_pending_rejects_third_unresolved_load() {
    let (core, head, first, second, third) =
        completed_result_quad(ld(5, 2, 0), ld(6, 5, 8), ld(7, 6, 16), ld(8, 7, 24));
    let mut authorizer = authorizer_for(&core, &head);

    assert!(authorizer.try_authorize_next(&first).is_some());
    assert!(authorizer.try_authorize_next(&second).is_some());
    assert_eq!(authorizer.try_authorize_next(&third), None);

    assert_eq!(authorizer.dependent_rows(), 2);
    assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6), reg(7)]);
}

#[test]
fn dependent_address_two_pending_rejects_duplicate_self_cycle_and_unrelated_graphs() {
    let (core, head, duplicate_self_cycle, _) =
        completed_result_triple(ld(5, 2, 0), ld(5, 5, 8), ld(6, 5, 16));
    let mut authorizer = authorizer_for(&core, &head);
    assert_eq!(authorizer.try_authorize_next(&duplicate_self_cycle), None);
    assert_eq!(authorizer.dependent_rows(), 0);
    assert_eq!(authorizer.result_destinations(), &[reg(5)]);

    let (core, head, unrelated_first, _) =
        completed_result_triple(ld(5, 2, 0), ld(6, 4, 8), ld(7, 5, 16));
    let mut authorizer = authorizer_for(&core, &head);
    assert_eq!(authorizer.try_authorize_next(&unrelated_first), None);
    assert_eq!(authorizer.dependent_rows(), 0);
    assert_eq!(authorizer.result_destinations(), &[reg(5)]);

    let (core, head, first, duplicate_second) =
        completed_result_triple(ld(5, 2, 0), ld(6, 5, 8), ld(6, 5, 16));
    let mut authorizer = authorizer_for(&core, &head);
    assert!(authorizer.try_authorize_next(&first).is_some());
    assert_eq!(authorizer.try_authorize_next(&duplicate_second), None);
    assert_eq!(authorizer.dependent_rows(), 1);
    assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6)]);

    let (core, head, first, older_overwrite) =
        completed_result_triple(ld(5, 2, 0), ld(6, 5, 8), ld(5, 6, 16));
    let mut authorizer = authorizer_for(&core, &head);
    assert!(authorizer.try_authorize_next(&first).is_some());
    assert_eq!(authorizer.try_authorize_next(&older_overwrite), None);
    assert_eq!(authorizer.dependent_rows(), 1);
    assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6)]);

    let (core, head, first, unrelated_second) =
        completed_result_triple(ld(5, 2, 0), ld(6, 5, 8), ld(7, 4, 16));
    let mut authorizer = authorizer_for(&core, &head);
    assert!(authorizer.try_authorize_next(&first).is_some());
    assert_eq!(authorizer.try_authorize_next(&unrelated_second), None);
    assert_eq!(authorizer.dependent_rows(), 1);
    assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6)]);
}

#[test]
fn dependent_address_two_pending_window_records_both_authorizations() {
    let core = window_core(vec![
        (0, 0x8000, bytes(ld(5, 2, 0))),
        (1, 0x8004, bytes(ld(6, 5, 8))),
        (2, 0x8008, bytes(ld(7, 5, 16))),
        (3, 0x800c, bytes(add(8, 6, 7))),
    ]);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    let roles = [0, 1, 2].map(|sequence| {
        state
            .memory_result_window_authorizations
            .get(&request(sequence))
            .copied()
            .map(O3MemoryResultWindowAuthorization::role)
    });
    assert_eq!(
        roles,
        [
            Some(O3MemoryResultWindowRole::Head),
            Some(O3MemoryResultWindowRole::YoungerDependentRead),
            Some(O3MemoryResultWindowRole::YoungerDependentRead),
        ]
    );
}

#[test]
fn dependent_address_two_pending_split_fetch_uses_previous_last_request() {
    let first = bytes(ld(6, 5, 8));
    let core = window_core(vec![
        (0, 0x8000, bytes(ld(5, 2, 0))),
        (1, 0x8004, first[..2].to_vec()),
        (2, 0x8006, first[2..].to_vec()),
        (3, 0x8008, bytes(ld(7, 6, 16))),
        (4, 0x800c, bytes(add(8, 6, 7))),
    ]);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert!(state
        .memory_result_window_authorizations
        .contains_key(&request(1)));
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(2)));
    assert_eq!(
        state
            .memory_result_window_authorizations
            .get(&request(3))
            .copied()
            .and_then(O3MemoryResultWindowAuthorization::dependent_source),
        Some((reg(6), MemoryWidth::Doubleword, Immediate::new(16)))
    );
}

#[test]
fn dependent_address_two_pending_rejects_late_pending_after_scalar() {
    let core = window_core(vec![
        (0, 0x8000, bytes(ld(5, 2, 0))),
        (1, 0x8004, bytes(ld(6, 5, 8))),
        (2, 0x8008, bytes(add(8, 5, 6))),
        (3, 0x800c, bytes(ld(7, 6, 16))),
    ]);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(3)));
}

#[test]
fn dependent_address_two_pending_rejects_dependent_plus_unrelated_memory_result() {
    let core = window_core(vec![
        (0, 0x8000, bytes(ld(5, 2, 0))),
        (1, 0x8004, bytes(ld(6, 5, 8))),
        (2, 0x8008, bytes(ld(7, 4, 16))),
    ]);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(2)));
}
