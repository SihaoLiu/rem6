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
fn live_issue_queue_capture_is_sequence_ordered_and_requires_bound_packets() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let (head, sequences) = stage_queue_rows(&mut runtime, instructions);
    bind_queue_row(&mut runtime, BRANCH_PC, instructions[0], 11);
    bind_queue_row(&mut runtime, THIRD_PC, instructions[2], 13);

    let first = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert_eq!(
        first.sequences().collect::<Vec<_>>(),
        vec![sequences[0], sequences[2]]
    );

    bind_queue_row(&mut runtime, SECOND_PC, instructions[1], 12);
    let second = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert_eq!(second.sequences().collect::<Vec<_>>(), sequences);
}

#[test]
fn live_issue_queue_lookup_is_sequence_owned() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let (head, sequences) = stage_queue_rows(&mut runtime, instructions);
    for (pc, instruction, sequence) in [
        (BRANCH_PC, instructions[0], 11),
        (SECOND_PC, instructions[1], 12),
        (THIRD_PC, instructions[2], 13),
    ] {
        bind_queue_row(&mut runtime, pc, instruction, sequence);
    }
    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    let middle = queue.entry(sequences[1]).expect("middle queue entry");
    assert_eq!(middle.scheduling().pc(), Address::new(SECOND_PC));
    assert_eq!(middle.packet().instruction(), instructions[1]);
    assert_eq!(middle.packet().consumed_requests(), [request(12)]);
    assert!(queue.entry(99).is_none());
}

#[test]
fn live_issue_queue_excludes_unsupported_bound_packets() {
    let mut runtime = O3RuntimeState::default();
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 20));
    let head = runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .expect("unsupported-row head reservation");

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
        ));
    }

    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert!(queue.entries().is_empty());
}

#[test]
fn live_issue_queue_rejects_duplicate_sequence_inventory() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let (head, _) = stage_queue_rows(&mut runtime, instructions);
    bind_queue_row(&mut runtime, BRANCH_PC, instructions[0], 11);
    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    let duplicate = queue.entries()[0].clone();
    let duplicate_sequence = duplicate.sequence();

    assert!(matches!(
        O3LiveIssueQueue::from_entries_for_test(vec![duplicate.clone(), duplicate]),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence }) if sequence == duplicate_sequence
    ));
}

#[test]
fn live_issue_queue_excludes_invalidated_descendant_identities() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let (head, _) = stage_queue_rows(&mut runtime, instructions);
    for (pc, instruction, sequence) in [
        (BRANCH_PC, instructions[0], 11),
        (SECOND_PC, instructions[1], 12),
        (THIRD_PC, instructions[2], 13),
    ] {
        bind_queue_row(&mut runtime, pc, instruction, sequence);
    }
    let mut hart = RiscvHartState::new(BRANCH_PC);
    let execution = hart.execute_decoded(decoded(instructions[0])).unwrap();
    runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(BRANCH_PC, 99), instructions[0], execution),
        &[request(99)],
        30,
    );

    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert!(queue.entries().is_empty());
}

#[test]
fn live_issue_queue_stale_pending_row_returns_exact_replay_boundary() {
    let mut runtime = O3RuntimeState::default();
    let (head, sequence) = stage_queue_pending_row(&mut runtime);
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequence));

    assert!(matches!(
        O3LiveIssueQueue::capture(&runtime, head).unwrap(),
        O3LiveIssueQueueCapture::ReplayPending(replay) if replay == sequence
    ));
}

#[test]
fn live_issue_queue_excludes_materialized_pending_rows() {
    let mut runtime = O3RuntimeState::default();
    let (head, sequence) = stage_queue_pending_row(&mut runtime);
    runtime.set_pending_data_address_materialized_for_test(
        40,
        queue_load_event(BRANCH_PC, 11, 13, 12, 0x9100),
    );

    let queue = ready_queue(O3LiveIssueQueue::capture(&runtime, head).unwrap());
    assert!(queue.entry(sequence).is_none());
}

fn ready_queue(capture: O3LiveIssueQueueCapture) -> O3LiveIssueQueue {
    match capture {
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
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(pc),
        decoded(instruction),
        &[request(request_sequence)],
    ));
}

fn stage_queue_rows(
    runtime: &mut O3RuntimeState,
    instructions: [RiscvInstruction; 3],
) -> (O3LiveIssueHeadReservation, [u64; 3]) {
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
    let head = runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .expect("queue fixture head reservation");
    (head, queue_row_sequences(runtime))
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

fn stage_queue_pending_row(runtime: &mut O3RuntimeState) -> (O3LiveIssueHeadReservation, u64) {
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
        ),
        1,
    );
    let head = runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .expect("pending queue head reservation");
    let sequence = runtime.pending_data_address_sequences_for_test()[0];
    (head, sequence)
}

fn queue_load_event(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(rd),
        rs1: reg(rs1),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        queue_fetch_event(pc, sequence, i_type(0, rs1, 0b011, rd, 0x03)),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(rd),
                address,
                width: MemoryWidth::Doubleword,
                signed: false,
            }),
        ),
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
