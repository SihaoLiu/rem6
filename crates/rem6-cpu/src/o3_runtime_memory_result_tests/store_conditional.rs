use super::*;
use crate::riscv_data_completion::RiscvDataCompletion;
use rem6_kernel::Tick;

#[test]
fn nonzero_store_conditional_is_one_integer_memory_result() {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 1, 7);

    assert!(runtime.stage_live_data_access_issue_for_test(&event, request(20), 31));

    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 1);
    assert_eq!(snapshot.load_store_queue().len(), 1);
    let rob = snapshot.reorder_buffer()[0];
    let rename = staged_rename_entry(rob).expect("SC status has one staged rename");
    assert_eq!(rename.register_class(), O3RegisterClass::Integer);
    assert_eq!(rename.architectural(), 7);
    assert_eq!(Some(rename.physical()), rob.destination());
}

#[test]
fn zero_destination_store_conditional_publishes_at_response_tick_without_a_reservation() {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 1, 0);
    assert!(runtime.stage_live_data_access_issue_for_test(&event, request(20), 31));
    assert_eq!(runtime.snapshot().reorder_buffer()[0].destination(), None);

    let completed = completed_store_conditional(event, RiscvDataAccessEventKind::Completed);
    let completion = successful_completion(&completed);
    assert!(runtime
        .complete_live_data_access_completion(
            &completed,
            request(20),
            41,
            10,
            expected_completion_identity(&completed),
            Some(completion.clone()),
        )
        .unwrap());

    assert!(runtime.writeback_reservations().is_empty());
    assert_eq!(
        runtime.ready_live_memory_result_completion(),
        Some(completion)
    );
    assert_eq!(
        runtime.ready_live_data_access_completion_timing(),
        Some((completed.fetch().request_id(), 31, 41))
    );
    assert!(runtime.take_ready_live_data_access_event(40).is_none());
    assert_eq!(
        runtime.take_ready_live_data_access_event(41),
        Some(completed.clone())
    );
    assert!(runtime.snapshot().reorder_buffer()[0].is_ready());
    assert_eq!(runtime.snapshot().reorder_buffer()[0].ready_tick(), 41);

    runtime.record_retired_instruction_with_trace(&completed, true);

    assert!(runtime.live_data_accesses.is_empty());
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert!(runtime.writeback_reservations().is_empty());
    assert_eq!(
        runtime
            .trace_records()
            .last()
            .expect("zero-destination SC records retirement trace")
            .admitted_writeback_tick(),
        None
    );
}

#[test]
fn failed_store_conditional_reserves_response_plus_one_writeback() {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 1, 7);
    assert!(runtime.stage_live_data_access_issue_for_test(&event, request(20), 31));
    let sequence = runtime.live_data_accesses[0].sequence;
    let failed = completed_store_conditional(event, RiscvDataAccessEventKind::ConditionalFailed);
    let completion = failed_completion(&failed, 41);

    assert!(runtime
        .complete_live_data_access_completion(
            &failed,
            request(20),
            41,
            10,
            expected_completion_identity(&failed),
            Some(completion.clone()),
        )
        .unwrap());

    assert_eq!(
        completion.data_event_kind(),
        RiscvDataAccessEventKind::ConditionalFailed
    );
    assert_eq!(calendar_rows(&runtime), vec![(sequence, 42, 0)]);
    assert_memory_owner(&runtime, sequence, Some(42), Some(0));
    assert!(runtime.snapshot().load_store_queue()[0].is_completed());
    assert_eq!(
        runtime.ready_live_memory_result_completion(),
        Some(completion)
    );
}

#[test]
fn width_one_serializes_store_conditional_against_older_fu() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    let fixed = fixed_instruction(6);
    let fixed_sequence = runtime
        .stage_live_retire_window(Address::new(0x8000), fixed, 0, [])
        .expect("older fixed-FU row stages first");
    assert_eq!(fixed_sequence, 0);
    let sc = store_conditional_event(0x8004, 2, 7);
    assert!(runtime.stage_live_data_access_issue_for_test(&sc, request(20), 31));
    let sc_sequence = runtime.live_data_accesses[0].sequence;
    assert_eq!(sc_sequence, 1);
    let completed = completed_store_conditional(sc, RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_completion(
            &completed,
            request(20),
            41,
            10,
            expected_completion_identity(&completed),
            Some(successful_completion(&completed)),
        )
        .unwrap());
    let fixed_request = request(30);
    let fixed_execution = record_fixed_fu_owner(
        &mut runtime,
        fixed_sequence,
        decoded_instruction(fixed),
        0x8000,
        fixed_request,
        42,
    );
    let collision = RealOwnerCollision {
        runtime,
        fixed_sequence,
        fixed_request,
        fixed_execution,
    };

    assert_eq!(
        calendar_rows(&collision.runtime),
        vec![(0, 42, 0), (1, 43, 0)]
    );
    assert_fixed_owner(&collision, 42, Some(0));
    assert_memory_owner(&collision.runtime, sc_sequence, Some(43), Some(0));
}

#[test]
fn mismatched_store_conditional_completion_kind_is_rejected() {
    assert_mismatched_completion_is_rejected(
        RiscvDataAccessEventKind::Completed,
        CompletionKind::Failed,
    );
    assert_mismatched_completion_is_rejected(
        RiscvDataAccessEventKind::ConditionalFailed,
        CompletionKind::Successful,
    );
}

#[test]
fn mismatched_store_conditional_completion_identity_is_rejected() {
    let event = store_conditional_event(0x8000, 1, 7);
    let completed = completed_store_conditional(event.clone(), RiscvDataAccessEventKind::Completed);
    let (access, physical_address, size) = completion_parts(&completed);
    let wrong_fetch = RiscvDataCompletion::from_issued_response(
        request(99),
        access.clone(),
        physical_address,
        size,
        0,
        None,
    );
    let foreign = store_conditional_event(0x8004, 2, 6);
    let (foreign_access, foreign_address, foreign_size) = completion_parts(&foreign);
    let wrong_access = RiscvDataCompletion::from_issued_response(
        completed.fetch().request_id(),
        foreign_access,
        foreign_address,
        foreign_size,
        0,
        None,
    );
    let wrong_physical_address = RiscvDataCompletion::from_issued_response(
        completed.fetch().request_id(),
        access.clone(),
        Address::new(physical_address.get() + 8),
        size,
        0,
        None,
    );
    let wrong_size = RiscvDataCompletion::from_issued_response(
        completed.fetch().request_id(),
        access.clone(),
        physical_address,
        AccessSize::new(4).unwrap(),
        0,
        None,
    );
    let wrong_request_byte_offset = RiscvDataCompletion::from_issued_response(
        completed.fetch().request_id(),
        access,
        physical_address,
        size,
        1,
        None,
    );

    for mismatch in [
        wrong_fetch,
        wrong_access,
        wrong_physical_address,
        wrong_size,
        wrong_request_byte_offset,
    ] {
        let mut runtime = O3RuntimeState::default();
        assert!(runtime.stage_live_data_access_issue_for_test(&event, request(20), 31));
        assert!(!runtime
            .complete_live_data_access_completion(
                &completed,
                request(20),
                41,
                10,
                expected_completion_identity(&completed),
                Some(mismatch),
            )
            .unwrap());
        assert_resident_without_result(&runtime);
    }
}

#[test]
fn zero_destination_store_conditional_requires_typed_completion() {
    for event_kind in [
        RiscvDataAccessEventKind::Completed,
        RiscvDataAccessEventKind::ConditionalFailed,
    ] {
        let mut runtime = O3RuntimeState::default();
        let event = store_conditional_event(0x8000, 1, 0);
        assert!(runtime.stage_live_data_access_issue_for_test(&event, request(20), 31));
        let completed = completed_store_conditional(event, event_kind);

        assert!(!runtime
            .complete_live_data_access_completion(
                &completed,
                request(20),
                41,
                10,
                expected_completion_identity(&completed),
                None,
            )
            .unwrap());
        assert_resident_without_result(&runtime);
    }
}

#[test]
fn retry_discards_unpublished_store_conditional_reservation() {
    let mut runtime = O3RuntimeState::default();
    let sc = store_conditional_event(0x8000, 1, 7);
    assert!(runtime.stage_live_data_access_issue_for_test(&sc, request(20), 31));
    let sequence = runtime.live_data_accesses[0].sequence;
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::memory_result(sequence, 42)])
        .unwrap();
    assert!(runtime.writeback_reservation(sequence).is_some());
    let retry = completed_store_conditional(sc, RiscvDataAccessEventKind::Retry);

    assert!(runtime
        .complete_live_data_access_completion(
            &retry,
            request(20),
            41,
            10,
            expected_completion_identity(&retry),
            None,
        )
        .unwrap());

    assert!(runtime.writeback_reservation(sequence).is_none());
    assert!(runtime.snapshot().reorder_buffer().is_empty());
    assert!(runtime.snapshot().load_store_queue().is_empty());
    assert_eq!(
        runtime.live_data_accesses[0].outcome,
        O3LiveDataAccessOutcome::Retried
    );
}

#[derive(Clone, Copy)]
enum CompletionKind {
    Successful,
    Failed,
}

fn assert_mismatched_completion_is_rejected(
    event_kind: RiscvDataAccessEventKind,
    completion_kind: CompletionKind,
) {
    let mut runtime = O3RuntimeState::default();
    let event = store_conditional_event(0x8000, 1, 7);
    assert!(runtime.stage_live_data_access_issue_for_test(&event, request(20), 31));
    let completed = completed_store_conditional(event, event_kind);
    let completion = match completion_kind {
        CompletionKind::Successful => successful_completion(&completed),
        CompletionKind::Failed => failed_completion(&completed, 41),
    };

    assert!(!runtime
        .complete_live_data_access_completion(
            &completed,
            request(20),
            41,
            10,
            expected_completion_identity(&completed),
            Some(completion),
        )
        .unwrap());
    assert_resident_without_result(&runtime);
}

fn assert_resident_without_result(runtime: &O3RuntimeState) {
    assert!(runtime.writeback_reservations().is_empty());
    assert_eq!(
        runtime.live_data_accesses[0].outcome,
        O3LiveDataAccessOutcome::Resident
    );
    assert_eq!(runtime.live_data_accesses[0].response_tick, None);
}

fn store_conditional_event(pc: u64, sequence: u64, rd: u8) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::StoreConditional {
        rd: reg(rd),
        rs1: reg(10),
        rs2: reg(11),
        width: MemoryWidth::Doubleword,
        acquire: false,
        release: false,
    };
    let access = MemoryAccessKind::StoreConditional {
        rd: reg(rd),
        address: 0x9000,
        width: MemoryWidth::Doubleword,
        value: 0x1122_3344_5566_7788,
        acquire: false,
        release: false,
    };
    execution_event(pc, sequence, instruction, access)
}

fn completed_store_conditional(
    mut event: RiscvCpuExecutionEvent,
    kind: RiscvDataAccessEventKind,
) -> RiscvCpuExecutionEvent {
    event.set_data_access_event_kind(kind);
    event
}

fn successful_completion(event: &RiscvCpuExecutionEvent) -> RiscvDataCompletion {
    let (access, physical_address, size) = completion_parts(event);
    RiscvDataCompletion::from_issued_response(
        event.fetch().request_id(),
        access,
        physical_address,
        size,
        0,
        None,
    )
}

fn failed_completion(event: &RiscvCpuExecutionEvent, tick: Tick) -> RiscvDataCompletion {
    let (access, physical_address, size) = completion_parts(event);
    RiscvDataCompletion::store_conditional_failed(
        event.fetch().request_id(),
        access,
        physical_address,
        size,
        0,
        tick,
    )
}

fn completion_parts(event: &RiscvCpuExecutionEvent) -> (MemoryAccessKind, Address, AccessSize) {
    let access = event
        .execution()
        .memory_access()
        .cloned()
        .expect("SC event has a memory access");
    let (physical_address, size) = match &access {
        MemoryAccessKind::StoreConditional { address, width, .. } => (
            Address::new(*address),
            AccessSize::new(width.bytes() as u64).unwrap(),
        ),
        _ => panic!("expected store-conditional access"),
    };
    (access, physical_address, size)
}

fn expected_completion_identity(event: &RiscvCpuExecutionEvent) -> (Address, AccessSize, usize) {
    let (_, physical_address, size) = completion_parts(event);
    (physical_address, size, 0)
}
