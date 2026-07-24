use rem6_isa_riscv::RiscvExecutionRecord;

use super::super::o3_runtime_issue::queue::{O3LiveIssueQueue, O3LiveIssueQueueCapture};
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

    assert!(runtime.bind_live_staged_issue_packet(Address::new(BRANCH_PC), decoded, &requests, 20,));
    assert!(runtime.bind_live_staged_issue_packet(Address::new(BRANCH_PC), decoded, &requests, 20,));

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
        20,
    ));

    assert!(!runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(addi(4, 0, 1)),
        &original_requests,
        20,
    ));
    assert!(!runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        original,
        &[request(12)],
        20,
    ));

    let packet = runtime.live_staged_issue_packet(sequence).unwrap();
    assert_eq!(packet.decoded(), original);
    assert_eq!(packet.consumed_requests(), original_requests);
}

#[test]
fn live_issue_queue_recording_rejects_unbound_packet() {
    assert_live_issue_queue_recording(false, 4, false);
}

#[test]
fn live_issue_queue_recording_rejects_bound_packet_byte_length_mismatch() {
    assert_live_issue_queue_recording(true, 2, false);
}

#[test]
fn live_issue_queue_recording_accepts_exact_bound_packet() {
    assert_live_issue_queue_recording(true, 4, true);
}

#[test]
fn live_issue_head_recording_rejects_unbound_packet() {
    assert_live_issue_head_recording(false, 4, false);
}

#[test]
fn live_issue_head_recording_rejects_bound_packet_byte_length_mismatch() {
    assert_live_issue_head_recording(true, 2, false);
}

#[test]
fn live_issue_head_recording_accepts_exact_bound_packet() {
    assert_live_issue_head_recording(true, 4, true);
}

#[test]
fn live_issue_queue_materialization_is_sequence_ordered_and_requires_bound_packets() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_row(&mut runtime, BRANCH_PC, instructions[0], 11);
    bind_queue_row(&mut runtime, THIRD_PC, instructions[2], 13);

    let first = materialized_queue(&runtime);
    assert_eq!(
        first.sequences().collect::<Vec<_>>(),
        vec![sequences[0], sequences[2]]
    );

    bind_queue_row(&mut runtime, SECOND_PC, instructions[1], 12);
    let second = materialized_queue(&runtime);
    assert_eq!(second.sequences().collect::<Vec<_>>(), sequences);
}

#[test]
fn live_issue_queue_materializes_resident_sequences_without_rob_inventory_scan() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_rows(&mut runtime, instructions);
    assert!(runtime.live_issue.remove_exact_at(
        sequences[1],
        O3LiveIssueTraceAction::Retired,
        Address::new(SECOND_PC),
        O3LiveIssueTraceClass::IntegerMulDiv,
        20,
    ));

    let queue = materialized_queue(&runtime);
    assert_eq!(
        queue.sequences().collect::<Vec<_>>(),
        vec![sequences[0], sequences[2]]
    );
    assert!(runtime.live_staged_issue_packet(sequences[1]).is_some());
}

#[test]
fn live_issue_queue_rejects_stale_ordinary_resident_sequence() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_row_at(&mut runtime, BRANCH_PC, instructions[0], 11, 20);
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequences[0]));

    assert!(matches!(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence })
            if sequence == sequences[0]
    ));
}

#[test]
fn live_issue_queue_returns_exact_pending_replay_boundary() {
    let mut runtime = O3RuntimeState::default();
    let sequence = stage_queue_pending_row(&mut runtime);
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequence));

    assert!(matches!(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()).unwrap(),
        O3LiveIssueQueueCapture::ReplayPending(replay) if replay == sequence
    ));
}

#[test]
fn live_issue_queue_preserves_architectural_sequence_order() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_row_at(&mut runtime, THIRD_PC, instructions[2], 13, 20);
    bind_queue_row_at(&mut runtime, BRANCH_PC, instructions[0], 11, 20);
    bind_queue_row_at(&mut runtime, SECOND_PC, instructions[1], 12, 20);

    let queue = materialized_queue(&runtime);
    assert_eq!(queue.sequences().collect::<Vec<_>>(), sequences);
}

#[test]
fn live_issue_queue_lookup_is_sequence_owned() {
    let (mut runtime, instructions, sequences) = queue_rows();
    bind_queue_rows(&mut runtime, instructions);
    let queue = materialized_queue(&runtime);
    let middle = queue.entry(sequences[1]).expect("middle queue entry");
    assert_eq!(middle.scheduling().pc(), Address::new(SECOND_PC));
    assert_eq!(middle.packet().instruction(), instructions[1]);
    assert_eq!(middle.packet().consumed_requests(), [request(12)]);
    assert!(queue.entry(99).is_none());
}

#[test]
fn live_issue_queue_does_not_enqueue_unsupported_bound_packets() {
    let mut runtime = O3RuntimeState::default();
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 20));
    for (pc, raw, request_sequence) in [
        (BRANCH_PC, 0x0020_81d3, 11),
        (SECOND_PC, 0x0220_81d7, 12),
        (THIRD_PC, 0x0000_0073, 13),
    ] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        assert!(runtime
            .stage_live_instruction(Address::new(pc), decoded.instruction(), 20)
            .is_some());
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            20,
        ));
    }

    assert!(runtime.live_issue.resident_sequences().is_empty());
    let queue = materialized_queue(&runtime);
    assert!(queue.entries().is_empty());
}

#[test]
fn live_issue_queue_rejects_duplicate_sequence_inventory() {
    let (mut runtime, instructions, _) = queue_rows();
    bind_queue_row(&mut runtime, BRANCH_PC, instructions[0], 11);
    let queue = materialized_queue(&runtime);
    let duplicate = queue.entries()[0].clone();
    let duplicate_sequence = duplicate.sequence();

    assert!(matches!(
        O3LiveIssueQueue::from_entries_for_test(vec![duplicate.clone(), duplicate]),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence }) if sequence == duplicate_sequence
    ));
}

#[test]
fn live_issue_queue_excludes_invalidated_descendant_identities() {
    let (mut runtime, instructions, _) = queue_rows();
    bind_queue_rows(&mut runtime, instructions);
    let mut hart = RiscvHartState::new(BRANCH_PC);
    let execution = hart.execute_decoded(decoded(instructions[0])).unwrap();
    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(BRANCH_PC, 99), instructions[0], execution),
        &[request(99)],
        30,
    );

    let queue = materialized_queue(&runtime);
    assert!(queue.entries().is_empty());
}

#[test]
fn live_issue_queue_rejects_materialized_pending_resident_sequence() {
    let mut runtime = O3RuntimeState::default();
    let sequence = stage_queue_pending_row(&mut runtime);
    runtime.set_pending_data_address_materialized_for_test(
        40,
        queue_load_event(BRANCH_PC, 11, 13, 12, 0x9100),
    );

    assert!(matches!(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence: stale })
            if stale == sequence
    ));
}

fn materialized_queue(runtime: &O3RuntimeState) -> O3LiveIssueQueue {
    match O3LiveIssueQueue::materialize(runtime, runtime.live_issue.resident_sequences()).unwrap() {
        O3LiveIssueQueueCapture::Ready(queue) => queue,
        O3LiveIssueQueueCapture::ReplayPending(sequence) => {
            panic!("unexpected pending replay at {sequence}")
        }
    }
}

fn assert_live_issue_queue_recording(bind_packet: bool, instruction_bytes: u8, expected: bool) {
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
    if bind_packet {
        bind_queue_row(&mut runtime, BRANCH_PC, instruction, 11);
    }
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(BRANCH_PC), instruction)
        .unwrap();
    assert_eq!(
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(11)],
                20,
                issue_record(instruction, instruction_bytes),
            )
            .unwrap(),
        expected
    );
}

fn assert_live_issue_head_recording(bind_packet: bool, instruction_bytes: u8, expected: bool) {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    if bind_packet {
        bind_queue_row(&mut runtime, BRANCH_PC, instruction, 11);
    }
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, 20, instruction);
    assert_eq!(
        runtime
            .record_live_issue_head_execution(
                head,
                &[request(11)],
                issue_record(instruction, instruction_bytes),
            )
            .unwrap(),
        expected
    );
}

fn issue_record(instruction: RiscvInstruction, instruction_bytes: u8) -> RiscvExecutionRecord {
    RiscvExecutionRecord::new_with_instruction_bytes(
        instruction,
        instruction_bytes,
        BRANCH_PC,
        BRANCH_PC + u64::from(instruction_bytes),
        vec![RegisterWrite::new(reg(3), 1)],
        None,
    )
}

fn bind_queue_row(
    runtime: &mut O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    request_sequence: u64,
) {
    bind_queue_row_at(runtime, pc, instruction, request_sequence, 20);
}

fn bind_queue_row_at(
    runtime: &mut O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    request_sequence: u64,
    admission_tick: u64,
) {
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(pc),
        decoded(instruction),
        &[request(request_sequence)],
        admission_tick,
    ));
}

fn bind_queue_rows(runtime: &mut O3RuntimeState, instructions: [RiscvInstruction; 3]) {
    for (pc, instruction, request_sequence) in [
        (BRANCH_PC, instructions[0], 11),
        (SECOND_PC, instructions[1], 12),
        (THIRD_PC, instructions[2], 13),
    ] {
        bind_queue_row(runtime, pc, instruction, request_sequence);
    }
}

fn queue_rows() -> (O3RuntimeState, [RiscvInstruction; 3], [u64; 3]) {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let sequences = stage_queue_rows(&mut runtime, instructions);
    (runtime, instructions, sequences)
}

fn stage_queue_rows(runtime: &mut O3RuntimeState, instructions: [RiscvInstruction; 3]) -> [u64; 3] {
    assert!(runtime.set_issue_width(2));
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 20));
    runtime.stage_live_data_access_younger_window(
        load.fetch().request_id(),
        [BRANCH_PC, SECOND_PC, THIRD_PC]
            .into_iter()
            .zip(instructions)
            .map(|(pc, instruction)| (Address::new(pc), instruction)),
    );
    queue_row_sequences(runtime)
}

fn queue_row_sequences(runtime: &O3RuntimeState) -> [u64; 3] {
    let snapshot = runtime.snapshot();
    [BRANCH_PC, SECOND_PC, THIRD_PC].map(|pc| {
        snapshot
            .reorder_buffer()
            .iter()
            .find(|entry| entry.is_live_staged() && entry.pc() == Address::new(pc))
            .expect("queue fixture staged row")
            .sequence()
    })
}

fn stage_queue_pending_row(runtime: &mut O3RuntimeState) -> u64 {
    assert!(runtime.set_window_depths(4, 4));
    let load = queue_load_event(LOAD_PC, 10, 12, 10, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &load,
        request(20),
        20,
        O3DataAccessWindowPolicy::MemoryResultWindow
    ));
    let raw = i_type(0, 12, 0b011, 13, 0x03);
    let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
    let pending = O3PendingDataAddressRequest::new(
        load.fetch().request_id(),
        queue_fetch_event(BRANCH_PC, 11, raw),
        vec![request(11)],
        decoded,
        reg(12),
    );
    assert_eq!(
        runtime.stage_pending_data_address_window(
            load.fetch().request_id(),
            vec![pending],
            std::iter::empty::<(Address, RiscvInstruction)>(),
            0,
        ),
        1,
    );
    runtime.pending_data_address_sequences_for_test()[0]
}

fn queue_load_event(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let raw = i_type(0, rs1, 0b011, rd, 0x03);
    let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
    let mut hart = RiscvHartState::new(pc);
    hart.write(reg(rs1), address);
    RiscvCpuExecutionEvent::new(
        queue_fetch_event(pc, sequence, raw),
        decoded.instruction(),
        hart.execute_decoded(decoded).unwrap(),
    )
}

fn queue_fetch_event(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}
