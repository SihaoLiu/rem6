use super::*;
use crate::CpuFetchRecord;
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn completed(sequence: u64, pc: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            0,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        vec![0; 4],
    )
}

#[test]
fn split_fetch_retirement_selects_oldest_suffix_request() {
    let state = RiscvCoreState::new(0x800e, 0);
    let prefix = RiscvPendingFetchPrefix::new(completed(10, 0x800e), [0x13, 0x00]);
    let events = vec![completed(12, 0x8010), completed(11, 0x8010)];

    let suffix = next_completed_fetch_suffix(&state, &events, &prefix).unwrap();

    assert_eq!(suffix.request_id(), request(11));
}

#[test]
fn retire_cycle_waits_for_requested_sequence_when_older_work_is_stale() {
    let mut state = RiscvCoreState::new(0x8000, 0);
    state
        .in_order_pipeline
        .replace_in_flight([
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Commit),
            InOrderPipelineInstruction::new(1, InOrderPipelineStage::Fetch1),
        ])
        .unwrap();

    let record = record_retired_in_order_pipeline_cycle_after_wait_with_cause(
        &mut state,
        1,
        None,
        0,
        InOrderPipelineStallCause::ExecuteWait,
    )
    .unwrap();

    assert!(record
        .plan()
        .advanced()
        .iter()
        .any(|advance| advance.sequence() == 1 && advance.retires()));
    assert!(!record
        .plan()
        .advanced()
        .iter()
        .any(|advance| advance.sequence() == 0 && advance.retires()));
    assert!(!state.in_order_pipeline.contains_sequence(0));
    assert!(state.in_order_pipeline.in_flight().is_empty());
}

#[test]
fn discarded_fetch_sequences_leave_in_order_pipeline_state() {
    let mut state = RiscvCoreState::new(0x8000, 0);
    state
        .in_order_pipeline
        .replace_in_flight([
            InOrderPipelineInstruction::new(1, InOrderPipelineStage::Commit),
            InOrderPipelineInstruction::new(2, InOrderPipelineStage::Fetch2),
            InOrderPipelineInstruction::new(3, InOrderPipelineStage::Fetch1),
        ])
        .unwrap();
    let discarded = [2, 3].into_iter().collect::<BTreeSet<_>>();

    remove_fetch_sequences_from_pipeline(&mut state, &discarded).unwrap();

    assert_eq!(
        state
            .in_order_pipeline
            .in_flight()
            .iter()
            .map(|instruction| (instruction.sequence(), instruction.stage()))
            .collect::<Vec<_>>(),
        vec![(1, InOrderPipelineStage::Commit)]
    );
}

#[test]
fn refetched_stream_discards_orphaned_restored_pipeline_rows() {
    let mut state = RiscvCoreState::new(0x8000, 0);
    state
        .in_order_pipeline
        .replace_in_flight([
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Commit),
            InOrderPipelineInstruction::new(1, InOrderPipelineStage::Fetch1),
        ])
        .unwrap();

    sync_in_order_fetch_state(&mut state, &[completed(2, 0x8000)]).unwrap();

    assert_eq!(
        state.in_order_pipeline.in_flight(),
        &[InOrderPipelineInstruction::new(
            2,
            InOrderPipelineStage::Fetch1
        )]
    );
}

#[test]
fn split_fetch_suffix_keeps_pending_prefix_pipeline_row() {
    let mut state = RiscvCoreState::new(0x800e, 0);
    let prefix = completed(1, 0x800e);
    state.pending_fetch_prefix = Some(RiscvPendingFetchPrefix::new(prefix, [0x13, 0x00]));

    sync_in_order_fetch_state(&mut state, &[completed(2, 0x8010)]).unwrap();

    assert!(state.in_order_pipeline.contains_sequence(1));
}

#[test]
fn stale_fetches_after_retire_discard_duplicate_and_redirect_wrong_path_requests() {
    let state = RiscvCoreState::new(0x8000, 0);
    let events = vec![
        completed(0, 0x8008),
        completed(1, 0x8008),
        completed(2, 0x800e),
        completed(3, 0x8000),
    ];

    let stale = stale_fetch_requests_after_retire(
        &state,
        &events,
        Address::new(0x8008),
        &[request(0)],
        Some(Address::new(0x8000)),
    );

    assert_eq!(stale, vec![request(1), request(2)]);
}

#[test]
fn stale_fetches_after_retire_keep_same_pc_redirect_target_request() {
    let state = RiscvCoreState::new(0x8000, 0);
    let events = vec![completed(0, 0x8000), completed(1, 0x8000)];

    let stale = stale_fetch_requests_after_retire(
        &state,
        &events,
        Address::new(0x8000),
        &[request(0)],
        Some(Address::new(0x8000)),
    );

    assert!(stale.is_empty());
}

#[test]
fn stale_fetches_after_retire_discard_backedge_wrong_path_request() {
    let state = RiscvCoreState::new(0x8000, 0);
    let events = vec![
        completed(0, 0x8010),
        completed(1, 0x8014),
        completed(2, 0x8008),
    ];

    let stale = stale_fetch_requests_after_retire(
        &state,
        &events,
        Address::new(0x8010),
        &[request(0)],
        Some(Address::new(0x8008)),
    );

    assert_eq!(stale, vec![request(1)]);
}
