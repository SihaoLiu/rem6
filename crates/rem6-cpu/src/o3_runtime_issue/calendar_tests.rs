use rem6_isa_riscv::{RiscvExecutionRecord, RiscvInstruction};

use crate::o3_pipeline::{O3DependencyScopeId, O3IssueOpClass, O3ScopedReadyInstruction};

use super::super::o3_runtime_issue::calendar::{
    O3LiveIssueCalendar, O3LiveIssueCyclePlan, O3LiveIssueTickDecision, LIVE_ISSUE_QUEUE,
};
use super::*;

#[test]
fn live_issue_calendar_derives_fixed_fu_reservations_without_caller_head() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let instruction = addi(3, 0, 1);
    stage_live_row(&mut runtime, 1, LOAD_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(1, 20, LOAD_PC, instruction));

    let plan = O3LiveIssueCalendar::capture(&runtime)
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
fn live_issue_calendar_keeps_narrow_head_admission_helper() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(1));
    let head = O3LiveIssueHeadReservation::for_instruction(1, 20, addi(3, 0, 1));

    let plan = O3LiveIssueCalendar::capture_with_head_for_admission(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(2, O3IssueOpClass::IntAlu)],
        )
        .unwrap();
    assert_eq!(plan.reserved_width(), 1);
    assert!(plan.issued().is_empty());
    assert_eq!(sequences(plan.resource_blocked()), vec![2]);
}

#[test]
fn live_issue_calendar_rebuild_releases_removed_prior_row() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    stage_live_row(&mut runtime, 2, BRANCH_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(2, 20, BRANCH_PC, mul(3, 1, 2)));

    let blocked = calendar_plan(&runtime, 20, [ready(3, O3IssueOpClass::IntMult)]);
    assert_eq!(blocked.reserved_width(), 1);
    assert_eq!(sequences(blocked.resource_blocked()), vec![3]);

    runtime.live_speculative_executions.clear();
    let released = calendar_plan(&runtime, 20, [ready(3, O3IssueOpClass::IntMult)]);
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
    stage_two_pending_loads(&mut runtime, &head_event);
    runtime.set_pending_data_address_materialized_for_fetch_for_test(
        request(11),
        40,
        calendar_load_event(BRANCH_PC, 11, 6, 5, 0x9100),
    );

    let plan = calendar_plan(
        &runtime,
        40,
        [
            ready(99, O3IssueOpClass::Memory),
            ready(100, O3IssueOpClass::IntAlu),
        ],
    );

    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![100]);
    assert_eq!(sequences(plan.resource_blocked()), vec![99]);
}

#[test]
fn live_issue_calendar_derives_memory_head_from_live_data_access() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    assert!(runtime.set_memory_issue_width(1));
    let head = calendar_load_event(LOAD_PC, 10, 5, 2, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &head,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));

    let plan = O3LiveIssueCalendar::capture(&runtime)
        .plan_scoped_at(
            31,
            std::iter::empty::<O3DependencyScopeId>(),
            [
                ready(2, O3IssueOpClass::Memory),
                ready(3, O3IssueOpClass::IntAlu),
            ],
        )
        .unwrap();
    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![3]);
    assert_eq!(sequences(plan.resource_blocked()), vec![2]);
}

#[test]
fn live_issue_calendar_memory_width_one_blocks_younger_ready_memory_rows() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(4));
    assert!(runtime.set_memory_issue_width(1));

    let plan = calendar_plan(
        &runtime,
        40,
        [
            ready(2, O3IssueOpClass::Memory),
            ready(3, O3IssueOpClass::Memory),
            ready(4, O3IssueOpClass::Memory),
        ],
    );

    assert_eq!(plan.reserved_width(), 0);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![2]);
    assert_eq!(sequences(plan.resource_blocked()), vec![3, 4]);
    assert!(plan.dependency_blocked().is_empty());
}

#[test]
fn live_issue_calendar_memory_width_two_selects_two_oldest_ready_memory_rows() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(4));
    assert!(runtime.set_memory_issue_width(2));

    let plan = calendar_plan(
        &runtime,
        40,
        [
            ready(2, O3IssueOpClass::Memory),
            ready(3, O3IssueOpClass::Memory),
            ready(4, O3IssueOpClass::Memory),
        ],
    );

    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![2, 3]);
    assert_eq!(sequences(plan.resource_blocked()), vec![4]);
    assert!(plan.dependency_blocked().is_empty());
}

#[test]
fn live_issue_calendar_total_width_still_bounds_memory_width_four() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(4));
    assert!(runtime.set_memory_issue_width(4));
    stage_live_row(&mut runtime, 1, LOAD_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(1, 40, LOAD_PC, addi(3, 0, 1)));

    let plan = calendar_plan(
        &runtime,
        40,
        [
            ready(2, O3IssueOpClass::Memory),
            ready(3, O3IssueOpClass::Memory),
            ready(4, O3IssueOpClass::Memory),
            ready(5, O3IssueOpClass::Memory),
        ],
    );

    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![2, 3, 4]);
    assert_eq!(sequences(plan.resource_blocked()), vec![5]);
    assert!(plan.dependency_blocked().is_empty());
}

#[test]
fn live_issue_calendar_rebuild_counts_each_same_tick_memory_reservation() {
    let global_exhausted = same_tick_memory_reservation_plan(2);
    assert_eq!(global_exhausted.reserved_width(), 2);
    assert!(global_exhausted.issued().is_empty());
    assert_eq!(
        sequences(global_exhausted.resource_blocked()),
        vec![99, 100]
    );
    assert!(global_exhausted.dependency_blocked().is_empty());

    let spare_global_width = same_tick_memory_reservation_plan(4);
    assert_eq!(spare_global_width.reserved_width(), 2);
    assert_eq!(
        spare_global_width.issued_sequences().collect::<Vec<_>>(),
        vec![100]
    );
    assert_eq!(sequences(spare_global_width.resource_blocked()), vec![99]);
    assert!(spare_global_width.dependency_blocked().is_empty());
}

#[test]
fn live_issue_calendar_separates_resource_and_dependency_blocks() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    stage_live_row(&mut runtime, 1, LOAD_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(1, 20, LOAD_PC, mul(3, 1, 2)));
    let unresolved = O3DependencyScopeId::new(7);

    let plan = calendar_plan(
        &runtime,
        20,
        [
            ready(2, O3IssueOpClass::IntMult),
            ready(3, O3IssueOpClass::Branch).with_waits_on([unresolved]),
        ],
    );

    assert!(plan.issued().is_empty());
    assert_eq!(sequences(plan.resource_blocked()), vec![2]);
    assert_eq!(sequences(plan.dependency_blocked()), vec![3]);
}

#[test]
fn live_issue_calendar_tick_decision_aggregates_same_tick_attempts() {
    let mut first_runtime = O3RuntimeState::default();
    assert!(first_runtime.set_issue_width(2));
    stage_live_row(&mut first_runtime, 1, LOAD_PC);
    first_runtime
        .live_speculative_executions
        .push(live_execution(1, 20, LOAD_PC, addi(3, 0, 1)));
    let unresolved = O3DependencyScopeId::new(7);
    let first = calendar_plan(
        &first_runtime,
        20,
        [
            ready(2, O3IssueOpClass::Branch),
            ready(3, O3IssueOpClass::IntMult),
            ready(4, O3IssueOpClass::Branch).with_waits_on([unresolved]),
        ],
    );
    assert_eq!(first.resource_blocked().len(), 1);
    assert_eq!(first.dependency_blocked().len(), 1);

    let mut second_runtime = O3RuntimeState::default();
    assert!(second_runtime.set_issue_width(2));
    let second = calendar_plan(&second_runtime, 20, [ready(4, O3IssueOpClass::IntAlu)]);
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

fn sequences(rows: &[O3ScopedReadyInstruction]) -> Vec<u64> {
    rows.iter()
        .map(O3ScopedReadyInstruction::sequence)
        .collect()
}

fn calendar_plan<I>(runtime: &O3RuntimeState, tick: u64, ready: I) -> O3LiveIssueCyclePlan
where
    I: IntoIterator<Item = O3ScopedReadyInstruction>,
{
    O3LiveIssueCalendar::capture(runtime)
        .plan_scoped_at(tick, std::iter::empty::<O3DependencyScopeId>(), ready)
        .unwrap()
}

fn same_tick_memory_reservation_plan(issue_width: usize) -> O3LiveIssueCyclePlan {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(issue_width));
    assert!(runtime.set_memory_issue_width(2));
    assert!(runtime.set_window_depths(4, 4));
    let head_event = calendar_load_event(LOAD_PC, 10, 5, 2, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &head_event,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    stage_two_pending_loads(&mut runtime, &head_event);
    for (fetch, pc, sequence, rd, address) in [
        (request(11), BRANCH_PC, 11, 6, 0x9100),
        (request(12), SECOND_PC, 12, 7, 0x9200),
    ] {
        runtime.set_pending_data_address_materialized_for_fetch_for_test(
            fetch,
            40,
            calendar_load_event(pc, sequence, rd, 5, address),
        );
    }
    calendar_plan(
        &runtime,
        40,
        [
            ready(99, O3IssueOpClass::Memory),
            ready(100, O3IssueOpClass::IntAlu),
        ],
    )
}

fn stage_two_pending_loads(runtime: &mut O3RuntimeState, head_event: &RiscvCpuExecutionEvent) {
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
            0,
        ),
        2
    );
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
