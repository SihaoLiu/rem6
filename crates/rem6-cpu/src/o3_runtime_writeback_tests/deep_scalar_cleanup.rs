use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvHartState};

use super::*;

fn i_type(imm: i64, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32 & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn decoded_addi(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    let RiscvInstruction::Addi { rd, rs1, imm } = instruction else {
        unreachable!()
    };
    RiscvInstruction::decode_with_length(i_type(imm.value(), rs1.index(), 0, rd.index(), 0x13))
        .unwrap()
}

fn deep_runtime() -> (
    O3RuntimeState,
    RiscvCpuExecutionEvent,
    Vec<O3RenameMapEntry>,
) {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_window_depths(1, 8));
    assert!(runtime.set_issue_width(4));
    assert!(runtime.set_writeback_width(4));
    let committed_rename_map = runtime.snapshot.rename_map().to_vec();
    let load = scalar_load_event(0x8000, 10, 12, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &load,
        memory_request(20),
        31,
        O3DataAccessWindowPolicy::UntranslatedScalarMemoryPrefix,
    ));
    let younger = (0..7)
        .map(|index| {
            (
                Address::new(0x8004 + index * 4),
                addi(13 + index as u8, 0, index as i64 + 1),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            younger.iter().copied(),
        ),
        7
    );
    let requests = younger
        .iter()
        .copied()
        .enumerate()
        .map(|(index, (pc, instruction))| {
            let consumed = vec![memory_request(100 + index as u64)];
            assert!(runtime.bind_live_staged_fetch_identity(pc, instruction, &consumed));
            O3LiveIssueRequest::new(pc, consumed, decoded_addi(instruction))
        })
        .collect::<Vec<_>>();
    let head = runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .unwrap();
    runtime
        .schedule_live_speculative_issues(&RiscvHartState::new(0x8000), head, 31, &requests)
        .unwrap();
    assert_eq!(runtime.live_speculative_executions.len(), 7);
    assert!(!runtime.writeback_reservations().is_empty());
    (runtime, load, committed_rename_map)
}

fn assert_outcome_cleanup(kind: RiscvDataAccessEventKind) {
    let (mut runtime, load, committed_rename_map) = deep_runtime();
    let mut outcome = load;
    outcome.set_data_access_event_kind(kind);
    assert!(runtime
        .complete_live_data_access_response(&outcome, memory_request(20), 40, 9, None,)
        .unwrap());
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.live_data_access_younger_sequences.is_empty());
    assert!(runtime.live_control_lineages.is_empty());
    assert!(runtime.live_staged_fetch_identities.is_empty());
    assert!(runtime.writeback_reservations().is_empty());
    assert!(runtime.snapshot.reorder_buffer().is_empty());
    assert!(runtime.snapshot.load_store_queue().is_empty());
    assert_eq!(runtime.live_data_accesses.len(), 1);
    let expected_outcome = match kind {
        RiscvDataAccessEventKind::Retry => O3LiveDataAccessOutcome::Retried,
        RiscvDataAccessEventKind::Failed => O3LiveDataAccessOutcome::Failed,
        _ => unreachable!(),
    };
    assert_eq!(runtime.live_data_accesses[0].outcome, expected_outcome);
    assert!(runtime.snapshot.committed_rename_map.is_none());
    assert_eq!(
        runtime.snapshot.rename_map(),
        committed_rename_map.as_slice()
    );
}

#[test]
fn retry_cleanup_discards_deep_scalar_suffix() {
    assert_outcome_cleanup(RiscvDataAccessEventKind::Retry);
}

#[test]
fn failure_cleanup_discards_deep_scalar_suffix() {
    assert_outcome_cleanup(RiscvDataAccessEventKind::Failed);
}

#[test]
fn redirect_cleanup_discards_deep_scalar_suffix() {
    let (mut runtime, _, committed_rename_map) = deep_runtime();
    runtime.discard_live_staged_instructions_at(31);
    assert!(runtime.snapshot.reorder_buffer().is_empty());
    assert!(runtime.snapshot.load_store_queue().is_empty());
    assert_eq!(
        runtime.snapshot.rename_map(),
        committed_rename_map.as_slice()
    );
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.live_data_access_younger_sequences.is_empty());
    assert!(runtime.live_control_lineages.is_empty());
    assert!(runtime.live_staged_fetch_identities.is_empty());
    assert!(runtime.writeback_reservations().is_empty());
}
