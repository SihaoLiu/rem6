use rem6_isa_riscv::{RiscvExecutionRecord, RiscvInstruction};

use crate::o3_pipeline::{O3DependencyScopeId, O3IssueOpClass, O3ScopedReadyInstruction};

use super::super::o3_runtime_issue::calendar::{
    O3LiveIssueCalendar, O3LiveIssueTickDecision, LIVE_ISSUE_QUEUE,
};
use super::*;

#[test]
fn live_issue_calendar_head_and_recorded_head_consume_one_slot() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let instruction = addi(3, 0, 1);
    let head = O3LiveIssueHeadReservation::for_instruction(1, 20, instruction);
    stage_live_row(&mut runtime, 1, LOAD_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(1, 20, LOAD_PC, instruction));

    let plan = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(2, O3IssueOpClass::Branch)],
        )
        .unwrap();

    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![2]);
    assert!(plan.resource_blocked().is_empty());
}

#[test]
fn live_issue_calendar_rebuild_releases_removed_prior_row() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    stage_live_row(&mut runtime, 2, BRANCH_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(2, 20, BRANCH_PC, mul(3, 1, 2)));
    let head = O3LiveIssueHeadReservation::for_instruction(1, 10, addi(4, 0, 1));

    let blocked = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(3, O3IssueOpClass::IntMult)],
        )
        .unwrap();
    assert_eq!(blocked.reserved_width(), 1);
    assert_eq!(
        blocked
            .resource_blocked()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>(),
        vec![3]
    );

    runtime.snapshot.reorder_buffer.clear();
    let released = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(3, O3IssueOpClass::IntMult)],
        )
        .unwrap();
    assert_eq!(released.reserved_width(), 0);
    assert_eq!(released.issued_sequences().collect::<Vec<_>>(), vec![3]);
}

#[test]
fn live_issue_calendar_selected_pending_tick_reserves_memory() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    assert!(runtime.set_window_depths(4, 4));
    let head_event = calendar_load_event(LOAD_PC, 10, 5, 2, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &head_event,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    let first_pending_raw = load_raw(6, 5, 0);
    let second_pending_raw = load_raw(7, 5, 0);
    let pending = vec![
        O3PendingDataAddressRequest::new(
            request(10),
            calendar_fetch_event(BRANCH_PC, 11, first_pending_raw),
            vec![request(11)],
            RiscvInstruction::decode_with_length(first_pending_raw).unwrap(),
            reg(5),
        ),
        O3PendingDataAddressRequest::new(
            request(11),
            calendar_fetch_event(SECOND_PC, 12, second_pending_raw),
            vec![request(12)],
            RiscvInstruction::decode_with_length(second_pending_raw).unwrap(),
            reg(5),
        ),
    ];
    assert_eq!(
        runtime.stage_pending_data_address_window(
            head_event.fetch().request_id(),
            pending,
            std::iter::empty::<(Address, RiscvInstruction)>(),
        ),
        2
    );
    runtime.set_pending_data_address_materialized_for_fetch_for_test(
        request(11),
        40,
        calendar_load_event(BRANCH_PC, 11, 6, 5, 0x9100),
    );
    runtime.set_pending_data_address_materialized_for_fetch_for_test(
        request(12),
        40,
        calendar_load_event(SECOND_PC, 12, 7, 5, 0x9200),
    );
    let head = runtime
        .live_data_access_head_reservation(head_event.fetch().request_id())
        .unwrap();

    let plan = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            40,
            std::iter::empty::<O3DependencyScopeId>(),
            [
                ready(99, O3IssueOpClass::Memory),
                ready(100, O3IssueOpClass::IntAlu),
            ],
        )
        .unwrap();

    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![100]);
    assert_eq!(
        plan.resource_blocked()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>(),
        vec![99]
    );
}

#[test]
fn live_issue_calendar_separates_resource_and_dependency_blocks() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let head = O3LiveIssueHeadReservation::for_instruction(1, 20, mul(3, 1, 2));
    let unresolved = O3DependencyScopeId::new(7);

    let plan = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [
                ready(2, O3IssueOpClass::IntMult),
                ready(3, O3IssueOpClass::Branch).with_waits_on([unresolved]),
            ],
        )
        .unwrap();

    assert!(plan.issued().is_empty());
    assert_eq!(
        plan.resource_blocked()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(
        plan.dependency_blocked()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>(),
        vec![3]
    );
}

#[test]
fn live_issue_tick_decision_aggregates_same_tick_attempts() {
    let mut first_runtime = O3RuntimeState::default();
    assert!(first_runtime.set_issue_width(2));
    let first_head = O3LiveIssueHeadReservation::for_instruction(1, 20, addi(3, 0, 1));
    let unresolved = O3DependencyScopeId::new(7);
    let first = O3LiveIssueCalendar::capture(&first_runtime, first_head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [
                ready(2, O3IssueOpClass::Branch),
                ready(3, O3IssueOpClass::IntMult),
                ready(4, O3IssueOpClass::Branch).with_waits_on([unresolved]),
            ],
        )
        .unwrap();
    assert_eq!(first.resource_blocked().len(), 1);
    assert_eq!(first.dependency_blocked().len(), 1);

    let mut second_runtime = O3RuntimeState::default();
    assert!(second_runtime.set_issue_width(2));
    let second_head = O3LiveIssueHeadReservation::for_instruction(9, 10, addi(4, 0, 1));
    let second = O3LiveIssueCalendar::capture(&second_runtime, second_head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(4, O3IssueOpClass::IntAlu)],
        )
        .unwrap();
    assert!(second.dependency_blocked().is_empty());

    let mut decision = O3LiveIssueTickDecision::default();
    decision.observe(&first, 1);
    decision.observe(&second, 1);
    let sample = decision.take().unwrap();

    assert_eq!(sample.issued_rows(), 2);
    assert_eq!(sample.resource_blocked_rows(), 0);
    assert_eq!(sample.dependency_blocked_rows(), 0);
    assert_eq!(sample.max_rows_at_tick(), 2);
    assert!(decision.take().is_none());
}

fn ready(sequence: u64, op_class: O3IssueOpClass) -> O3ScopedReadyInstruction {
    O3ScopedReadyInstruction::new(sequence, LIVE_ISSUE_QUEUE, op_class)
}

fn stage_live_row(runtime: &mut O3RuntimeState, sequence: u64, pc: u64) {
    runtime.snapshot.reorder_buffer.push(
        O3ReorderBufferEntry::new(sequence, Address::new(pc), None)
            .with_live_staged_rename_destination(None),
    );
}

fn live_execution(
    sequence: u64,
    issue_tick: u64,
    pc: u64,
    instruction: RiscvInstruction,
) -> O3LiveSpeculativeExecution {
    O3LiveSpeculativeExecution {
        consumed_requests: Vec::new(),
        sequence,
        producer_sequences: Vec::new(),
        issue_tick,
        raw_ready_tick: issue_tick,
        admitted_writeback_tick: issue_tick,
        writeback_slot: None,
        execution: RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), None),
    }
}

fn calendar_load_event(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let raw = load_raw(rd, rs1, 0);
    let instruction = RiscvInstruction::decode_with_length(raw)
        .unwrap()
        .instruction();
    let access = MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        calendar_fetch_event(pc, sequence, raw),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn calendar_fetch_event(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
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

fn load_raw(rd: u8, rs1: u8, offset: i64) -> u32 {
    i_type(offset, rs1, 0b011, rd, 0x03)
}
