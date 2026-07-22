use rem6_isa_riscv::RiscvExecutionRecord;

use super::*;

#[test]
fn live_issue_queue_packet_binding_is_idempotent_and_exact() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let decoded = decoded(instruction);
    let requests = [request(11)];

    assert!(runtime.bind_live_staged_issue_packet(Address::new(BRANCH_PC), decoded, &requests,));
    assert!(runtime.bind_live_staged_issue_packet(Address::new(BRANCH_PC), decoded, &requests,));

    let packet = runtime
        .live_staged_issue_packet(sequence)
        .expect("bound issue packet");
    assert_eq!(packet.decoded(), decoded);
    assert_eq!(packet.instruction(), instruction);
    assert_eq!(packet.consumed_requests(), requests);
}

#[test]
fn live_issue_queue_packet_rebinding_rejects_any_identity_change() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let original = decoded(instruction);
    let original_requests = [request(11)];
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        original,
        &original_requests,
    ));

    assert!(!runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(addi(4, 0, 1)),
        &original_requests,
    ));
    assert!(!runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        original,
        &[request(12)],
    ));

    let packet = runtime.live_staged_issue_packet(sequence).unwrap();
    assert_eq!(packet.decoded(), original);
    assert_eq!(packet.consumed_requests(), original_requests);
}

#[test]
fn live_issue_queue_recording_rejects_unbound_packet() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    runtime
        .stage_live_retire_window(
            Address::new(LOAD_PC),
            div(9, 1, 2),
            0,
            [(Address::new(BRANCH_PC), instruction)],
        )
        .unwrap();
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(BRANCH_PC), instruction)
        .unwrap();
    let execution = RiscvExecutionRecord::new(
        instruction,
        BRANCH_PC,
        BRANCH_PC + 4,
        vec![RegisterWrite::new(reg(3), 1)],
        None,
    );

    assert!(!runtime
        .record_live_speculative_execution(candidate, &[request(11)], 20, execution)
        .unwrap());
}

#[test]
fn live_issue_queue_recording_rejects_bound_packet_byte_length_mismatch() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    runtime
        .stage_live_retire_window(
            Address::new(LOAD_PC),
            div(9, 1, 2),
            0,
            [(Address::new(BRANCH_PC), instruction)],
        )
        .unwrap();
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(instruction),
        &[request(11)],
    ));
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(BRANCH_PC), instruction)
        .unwrap();
    let execution = RiscvExecutionRecord::new_with_instruction_bytes(
        instruction,
        2,
        BRANCH_PC,
        BRANCH_PC + 2,
        vec![RegisterWrite::new(reg(3), 1)],
        None,
    );

    assert!(!runtime
        .record_live_speculative_execution(candidate, &[request(11)], 20, execution)
        .unwrap());
}

#[test]
fn live_issue_queue_recording_accepts_exact_bound_packet() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    runtime
        .stage_live_retire_window(
            Address::new(LOAD_PC),
            div(9, 1, 2),
            0,
            [(Address::new(BRANCH_PC), instruction)],
        )
        .unwrap();
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(instruction),
        &[request(11)],
    ));
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(BRANCH_PC), instruction)
        .unwrap();
    let execution = RiscvExecutionRecord::new(
        instruction,
        BRANCH_PC,
        BRANCH_PC + 4,
        vec![RegisterWrite::new(reg(3), 1)],
        None,
    );

    assert!(runtime
        .record_live_speculative_execution(candidate, &[request(11)], 20, execution)
        .unwrap());
}

#[test]
fn live_issue_head_recording_rejects_unbound_packet() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, 20, instruction);
    let execution = RiscvExecutionRecord::new(
        instruction,
        BRANCH_PC,
        BRANCH_PC + 4,
        vec![RegisterWrite::new(reg(3), 1)],
        None,
    );

    assert!(!runtime
        .record_live_issue_head_execution(head, &[request(11)], execution)
        .unwrap());
}

#[test]
fn live_issue_head_recording_rejects_bound_packet_byte_length_mismatch() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(instruction),
        &[request(11)],
    ));
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, 20, instruction);
    let execution = RiscvExecutionRecord::new_with_instruction_bytes(
        instruction,
        2,
        BRANCH_PC,
        BRANCH_PC + 2,
        vec![RegisterWrite::new(reg(3), 1)],
        None,
    );

    assert!(!runtime
        .record_live_issue_head_execution(head, &[request(11)], execution)
        .unwrap());
}

#[test]
fn live_issue_head_recording_accepts_exact_bound_packet() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(instruction),
        &[request(11)],
    ));
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, 20, instruction);
    let execution = RiscvExecutionRecord::new(
        instruction,
        BRANCH_PC,
        BRANCH_PC + 4,
        vec![RegisterWrite::new(reg(3), 1)],
        None,
    );

    assert!(runtime
        .record_live_issue_head_execution(head, &[request(11)], execution)
        .unwrap());
}
