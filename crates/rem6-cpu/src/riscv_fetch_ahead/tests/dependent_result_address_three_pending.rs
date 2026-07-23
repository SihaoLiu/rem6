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

fn reg(raw: u8) -> Register {
    Register::new(raw).unwrap()
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

fn completed_result(
    rows: impl IntoIterator<Item = (u64, u64, u32)>,
) -> (RiscvCore, Vec<RiscvCompletedFetchInstruction>) {
    let core = core_with_completed_fetches(
        rows.into_iter()
            .map(|(sequence, pc, raw)| (sequence, pc, bytes(raw))),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0x9000);
    let fetch_state = core.core.state.lock().expect("cpu core lock");
    let instructions = fetch_state
        .events
        .iter()
        .map(|event| completed_instruction(&fetch_state.events, event.request_id()))
        .collect();
    drop(fetch_state);
    (core, instructions)
}

fn authorizer_for(
    core: &RiscvCore,
    head: &RiscvCompletedFetchInstruction,
) -> detailed_o3::DependentResultAddressAuthorizer {
    let state = core.state.lock().expect("riscv core lock");
    let head_authorization = detailed_o3::data_access_result_fetch_ahead_authorization(
        &state,
        head.first_consumed_request(),
        head.decoded().instruction(),
        head.decoded().bytes(),
        detailed_o3::TranslatedMemoryFetchAhead::Disabled,
    )
    .expect("resolved head authorization");
    let row_limit = state.o3_runtime.scalar_memory_window_limit();
    detailed_o3::DependentResultAddressAuthorizer::from_head(
        &state,
        head,
        head_authorization,
        row_limit,
    )
    .expect("dependent result address authorizer")
}

fn assert_authorizes(rows: [u32; 3], sources: [u8; 3]) {
    let (core, instructions) = completed_result([
        (0, 0x8000, ld(5, 2, 0)),
        (1, 0x8004, rows[0]),
        (2, 0x8008, rows[1]),
        (3, 0x800c, rows[2]),
    ]);
    let mut authorizer = authorizer_for(&core, &instructions[0]);

    for (instruction, source) in instructions[1..].iter().zip(sources) {
        let authorization = authorizer
            .try_authorize_next(instruction)
            .expect("dependent row authorizes");
        assert_eq!(
            authorization.dependent_source(),
            Some((reg(source), MemoryWidth::Doubleword, Immediate::new(8)))
        );
    }

    assert_eq!(authorizer.dependent_rows(), 3);
    assert_eq!(
        authorizer.result_destinations(),
        &[reg(5), reg(6), reg(7), reg(8)]
    );
}

#[test]
fn dependent_address_three_pending_authorizes_siblings_at_depth_four() {
    assert_authorizes([ld(6, 5, 8), ld(7, 5, 8), ld(8, 5, 8)], [5, 5, 5]);
}

#[test]
fn dependent_address_three_pending_authorizes_full_chain_at_depth_four() {
    assert_authorizes([ld(6, 5, 8), ld(7, 6, 8), ld(8, 7, 8)], [5, 6, 7]);
}

#[test]
fn dependent_address_three_pending_authorizes_mixed_fanout_at_depth_four() {
    assert_authorizes([ld(6, 5, 8), ld(7, 5, 8), ld(8, 7, 8)], [5, 5, 7]);
}

#[test]
fn dependent_address_three_pending_rejects_fourth_and_nonadjacent_graphs() {
    let (core, instructions) = completed_result([
        (0, 0x8000, ld(5, 2, 0)),
        (1, 0x8004, ld(6, 5, 8)),
        (2, 0x8008, ld(7, 5, 8)),
        (3, 0x800c, ld(8, 6, 8)),
    ]);
    let mut authorizer = authorizer_for(&core, &instructions[0]);

    assert!(authorizer.try_authorize_next(&instructions[1]).is_some());
    assert!(authorizer.try_authorize_next(&instructions[2]).is_some());
    assert_eq!(authorizer.try_authorize_next(&instructions[3]), None);
    assert_eq!(authorizer.dependent_rows(), 2);
    assert_eq!(authorizer.result_destinations(), &[reg(5), reg(6), reg(7)]);

    let (core, instructions) = completed_result([
        (0, 0x8000, ld(5, 2, 0)),
        (1, 0x8004, ld(6, 5, 8)),
        (2, 0x8008, ld(7, 5, 8)),
        (3, 0x800c, ld(8, 5, 8)),
        (4, 0x8010, ld(9, 5, 8)),
    ]);
    let mut authorizer = authorizer_for(&core, &instructions[0]);
    assert!(authorizer.try_authorize_next(&instructions[1]).is_some());
    assert!(authorizer.try_authorize_next(&instructions[2]).is_some());
    assert!(authorizer.try_authorize_next(&instructions[3]).is_some());
    assert_eq!(authorizer.try_authorize_next(&instructions[4]), None);
    assert_eq!(authorizer.dependent_rows(), 3);
    assert_eq!(
        authorizer.result_destinations(),
        &[reg(5), reg(6), reg(7), reg(8)]
    );
}

#[test]
fn dependent_address_three_pending_window_records_three_split_fetch_authorizations() {
    let third = bytes(ld(8, 7, 8));
    let core = core_with_completed_fetches([
        (0, 0x8000, bytes(ld(5, 2, 0))),
        (1, 0x8004, bytes(ld(6, 5, 8))),
        (2, 0x8008, bytes(ld(7, 5, 8))),
        (3, 0x800c, third[..2].to_vec()),
        (4, 0x800e, third[2..].to_vec()),
        (5, 0x8010, bytes(add(9, 6, 8))),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0x9000);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 4);
    for request_id in [request(1), request(2), request(3)] {
        assert_eq!(
            state
                .memory_result_window_authorizations
                .get(&request_id)
                .copied()
                .map(O3MemoryResultWindowAuthorization::role),
            Some(O3MemoryResultWindowRole::YoungerDependentRead)
        );
    }
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(4)));
}

#[test]
fn dependent_address_three_pending_rejects_late_memory_after_scalar_start() {
    let core = core_with_completed_fetches([
        (0, 0x8000, bytes(ld(5, 2, 0))),
        (1, 0x8004, bytes(ld(6, 5, 8))),
        (2, 0x8008, bytes(add(9, 5, 6))),
        (3, 0x800c, bytes(ld(7, 6, 8))),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.write_register(reg(2), 0x9000);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.memory_result_window_authorizations.len(), 2);
    assert!(!state
        .memory_result_window_authorizations
        .contains_key(&request(3)));
}
