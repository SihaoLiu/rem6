use super::*;

use rem6_isa_riscv::{AtomicMemoryOp, Immediate, RiscvExecutionRecord};
use rem6_kernel::{PartitionedScheduler, PendingEventSnapshot, SchedulerInstanceId};
use rem6_memory::CacheLineLayout;

use crate::o3_runtime::{O3DataAccessWindowPolicy, O3PendingDataAddressRequest};
use crate::riscv_fetch_ahead::{
    O3MemoryResultWindowAuthorization, O3MemoryResultWindowRole, O3MemoryResultWindowRoute,
};
use crate::{
    CpuCore, CpuFetchConfig, CpuId, CpuResetState, RiscvCore, RiscvCoreCheckpointRestoreInput,
};

#[path = "restore/capture.rs"]
mod capture;
#[path = "restore/materialization.rs"]
mod materialization;
#[path = "restore/scheduling.rs"]
mod scheduling;

const ROOT_FETCH_SEQUENCE: u64 = 0;
const FIRST_LOAD_SEQUENCE: u64 = 1;
const ROOT_PC: u64 = 0x8000;
const FIRST_LOAD_PC: u64 = 0x8004;
const ROOT_ADDRESS: u64 = 0x9000;
const ROOT_VALUE: u64 = 0xa000;
const ROOT_REGISTER: u8 = 5;
const FIRST_DESTINATION_REGISTER: u8 = 6;
const ROOT_ISSUE_TICK: u64 = 31;
const ROOT_RESPONSE_TICK: u64 = 40;
const CAPTURED_TICK: u64 = 41;

struct PendingLoadGraphCheckpointFixture {
    core: RiscvCore,
    scheduler: SchedulerInstanceId,
    wake: PendingEventSnapshot,
    issue_width: usize,
}

impl PendingLoadGraphCheckpointFixture {
    fn new(producers: [u8; 3], issue_width: usize) -> Self {
        Self::new_with_root(
            producers,
            issue_width,
            root_event(),
            Some([1, 2, 3]),
            [1, 2, 3],
        )
    }

    fn new_with_request_gaps(producers: [u8; 3], issue_width: usize) -> Self {
        Self::new_with_root(
            producers,
            issue_width,
            root_event(),
            Some([1, 2, 3]),
            [1, 3, 4],
        )
    }

    fn new_with_atomic_root(producers: [u8; 3], issue_width: usize) -> Self {
        Self::new_with_root(producers, issue_width, atomic_root_event(), None, [1, 2, 3])
    }

    fn new_with_root(
        producers: [u8; 3],
        issue_width: usize,
        root: RiscvCpuExecutionEvent,
        expected_resident_sequences: Option<[u64; 3]>,
        fetch_request_sequences: [u64; 3],
    ) -> Self {
        let core = graph_core(issue_width);
        core.write_register(reg(2), ROOT_ADDRESS);

        let pending = pending_requests(producers, fetch_request_sequences);
        {
            let mut cpu = core.core.state.lock().expect("cpu core lock");
            cpu.events = std::iter::once(root.fetch().clone())
                .chain(pending.iter().map(|request| request.fetch().clone()))
                .collect();
        }
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.events.push(root.clone());
            state.executed_fetches.insert(request(ROOT_FETCH_SEQUENCE));
            state
                .issued_data_for_fetches
                .insert(request(ROOT_FETCH_SEQUENCE));
            assert!(state.o3_runtime.stage_live_data_access_issue(
                &root,
                request(20),
                ROOT_ISSUE_TICK,
                O3DataAccessWindowPolicy::MemoryResultWindow,
            ));
            assert_eq!(
                state.o3_runtime.stage_pending_data_address_window(
                    request(ROOT_FETCH_SEQUENCE),
                    pending,
                    [],
                    0,
                ),
                3
            );
            let hart = state.hart.clone();
            state
                .o3_runtime
                .service_live_issue_scheduler_at(&hart, 0)
                .unwrap();
            assert_eq!(state.o3_runtime.live_issue_service_tick(), None);
            let mut completed = root.clone();
            completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
            assert!(state
                .o3_runtime
                .complete_live_data_access_response(
                    &completed,
                    request(20),
                    ROOT_RESPONSE_TICK,
                    ROOT_RESPONSE_TICK - ROOT_ISSUE_TICK,
                    Some(&ROOT_VALUE.to_le_bytes()),
                )
                .unwrap());
        }

        let published = core
            .record_ready_o3_data_access_event_with_trace(CAPTURED_TICK, false)
            .expect("root publishes through the production data-access path");
        assert_eq!(published.fetch().request_id(), request(ROOT_FETCH_SEQUENCE));
        assert_eq!(core.read_register(reg(ROOT_REGISTER)), ROOT_VALUE);
        core.inner().set_pc(Address::new(next_fetch_pc()));
        core.inner()
            .advance_sequence_past(request(fetch_request_sequences[2]));
        core.state
            .lock()
            .expect("riscv core lock")
            .hart
            .set_pc(next_fetch_pc());
        assert_eq!(
            core.requested_o3_writeback_wake_tick(CAPTURED_TICK),
            Some(CAPTURED_TICK)
        );

        let mut scheduler = PartitionedScheduler::new(3).unwrap();
        let event = scheduler
            .schedule_at(PartitionId::new(2), CAPTURED_TICK, |_| {})
            .unwrap();
        let scheduler_id = scheduler.instance_id();
        let wake = scheduler.pending_event_snapshot(event).unwrap();
        core.mark_o3_writeback_wake_scheduled(scheduler_id, wake);

        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.o3_runtime.pending_data_address_count(), 3);
        assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 0);
        if let Some(expected) = expected_resident_sequences {
            assert_eq!(
                state
                    .o3_runtime
                    .live_issue_resident_sequences_for_checkpoint(),
                expected
            );
        }
        assert_eq!(
            state
                .o3_runtime
                .pending_data_address_selected_issue_ticks_for_test(),
            [None, None, None]
        );
        assert_eq!(
            state
                .o3_runtime
                .pending_data_address_materialized_fetches_for_test(),
            [None, None, None]
        );
        assert!(state.outstanding_data.is_empty());
        drop(state);
        assert_eq!(core.owned_o3_writeback_wakes(), [(scheduler_id, wake)]);

        Self {
            core,
            scheduler: scheduler_id,
            wake,
            issue_width,
        }
    }

    fn capture(&self) -> RiscvO3CheckpointProjection {
        self.core.capture_checkpoint_projection(CAPTURED_TICK)
    }
}

#[test]
fn pending_load_graph_restore_rebuilds_rob_rename_lsq_and_fetches() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    let projection = fixture.capture();
    let live = captured_graph(&projection).clone();
    let destination = graph_core(fixture.issue_width);

    install_graph_projection(&destination, &fixture.core, &projection);

    assert_eq!(destination.read_register(reg(ROOT_REGISTER)), ROOT_VALUE);
    let snapshot = destination.o3_runtime_snapshot();
    assert_restored_snapshot_owns_graph(&snapshot, live.pending_addresses.as_slice());
    assert_eq!(
        destination.inner().fetch_events(),
        live.pending_addresses
            .iter()
            .map(|pending| pending.fetch.clone())
            .collect::<Vec<_>>()
    );
    assert!(destination.execution_events().is_empty());
    let state = destination.state.lock().expect("riscv core lock");
    assert!(state.events.is_empty());
    assert!(state.data_events.is_empty());
    assert_eq!(state.o3_runtime.pending_data_address_count(), 3);
    assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 0);
    assert_eq!(
        state
            .o3_runtime
            .live_issue_resident_sequences_for_checkpoint(),
        [1, 2, 3]
    );
    assert_eq!(
        state
            .o3_runtime
            .pending_data_address_selected_issue_ticks_for_test(),
        [None, None, None]
    );
    assert!(live.issue_rows.iter().all(|row| state
        .o3_runtime
        .live_issue_packet_requests_for_checkpoint_test(row.sequence)
        == [row.fetch_request]));
    for pending in &live.pending_addresses {
        assert!(!state.executed_fetches.contains(&pending.fetch.request_id()));
        assert!(!state
            .issued_data_for_fetches
            .contains(&pending.fetch.request_id()));
    }
}

#[test]
fn pending_load_graph_restore_preserves_sibling_chain_and_mixed_plans() {
    assert_restored_issue_plan([5, 5, 5], 2, [1, 2].as_slice(), [].as_slice());
    assert_restored_issue_plan([5, 6, 7], 4, [1].as_slice(), [2, 3].as_slice());
    assert_restored_issue_plan([5, 5, 7], 2, [1, 2].as_slice(), [3].as_slice());
}

#[test]
fn pending_load_graph_recapture_before_wake_is_identical() {
    let fixture = PendingLoadGraphCheckpointFixture::new([5, 5, 7], 2);
    let first = fixture.capture();
    let destination = graph_core(fixture.issue_width);
    install_graph_projection(&destination, &fixture.core, &first);
    destination.mark_o3_writeback_wake_scheduled(fixture.scheduler, fixture.wake);

    let second = destination.capture_checkpoint_projection(CAPTURED_TICK);

    assert_eq!(captured_graph(&second), captured_graph(&first));
    assert_eq!(second.stable(), first.stable());
}

fn graph_core(issue_width: usize) -> RiscvCore {
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(2),
                AgentId::new(7),
                Address::new(ROOT_PC),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRouteId::new(9),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    core.set_o3_window_depths(4, 4);
    core.set_o3_issue_width(issue_width);
    core.set_o3_memory_issue_width(issue_width);
    core
}

fn pending_requests(
    producers: [u8; 3],
    fetch_request_sequences: [u64; 3],
) -> Vec<O3PendingDataAddressRequest> {
    let mut predecessor = request(ROOT_FETCH_SEQUENCE);
    producers
        .into_iter()
        .enumerate()
        .map(|(index, producer)| {
            let fetch_request_sequence = fetch_request_sequences[index];
            let pc = FIRST_LOAD_PC + 4 * index as u64;
            let destination = FIRST_DESTINATION_REGISTER + index as u8;
            let raw = ld(destination, producer);
            let pending = O3PendingDataAddressRequest::new(
                predecessor,
                completed_fetch(pc, fetch_request_sequence, raw),
                vec![request(fetch_request_sequence)],
                RiscvInstruction::decode_with_length(raw).unwrap(),
                reg(producer),
            );
            predecessor = request(fetch_request_sequence);
            pending
        })
        .collect()
}

fn install_graph_authorizations(core: &RiscvCore, producers: [u8; 3]) {
    let mut state = core.state.lock().expect("riscv core lock");
    let first = O3MemoryResultWindowAuthorization::resolved_for_test(
        Some(reg(FIRST_DESTINATION_REGISTER)),
        O3MemoryResultWindowRoute::Memory,
        AddressRange::new(Address::new(ROOT_VALUE), AccessSize::new(8).unwrap()).unwrap(),
        O3MemoryResultWindowRole::Head,
    );
    state
        .memory_result_window_authorizations
        .insert(request(FIRST_LOAD_SEQUENCE), first);
    for (index, producer) in producers.into_iter().enumerate().skip(1) {
        state.memory_result_window_authorizations.insert(
            request(FIRST_LOAD_SEQUENCE + index as u64),
            O3MemoryResultWindowAuthorization::dependent_for_test(
                Some(reg(FIRST_DESTINATION_REGISTER + index as u8)),
                reg(producer),
                MemoryWidth::Doubleword,
                Immediate::new(0),
            ),
        );
    }
}

fn root_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(ROOT_REGISTER),
        rs1: reg(2),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        completed_fetch(ROOT_PC, ROOT_FETCH_SEQUENCE, ld(ROOT_REGISTER, 2)),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            ROOT_PC,
            FIRST_LOAD_PC,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(ROOT_REGISTER),
                address: ROOT_ADDRESS,
                width: MemoryWidth::Doubleword,
                signed: false,
            }),
        ),
    )
}

fn atomic_root_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::AtomicMemory {
        rd: reg(ROOT_REGISTER),
        rs1: reg(2),
        rs2: reg(3),
        width: MemoryWidth::Doubleword,
        op: AtomicMemoryOp::Swap,
        acquire: false,
        release: false,
    };
    RiscvCpuExecutionEvent::new(
        completed_fetch(ROOT_PC, ROOT_FETCH_SEQUENCE, 0x0831_32af),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            ROOT_PC,
            FIRST_LOAD_PC,
            Vec::new(),
            Some(MemoryAccessKind::AtomicMemory {
                rd: reg(ROOT_REGISTER),
                address: ROOT_ADDRESS,
                width: MemoryWidth::Doubleword,
                op: AtomicMemoryOp::Swap,
                value: 7,
                acquire: false,
                release: false,
            }),
        ),
    )
}

fn completed_fetch(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            31,
            PartitionId::new(2),
            MemoryRouteId::new(9),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn captured_graph(projection: &RiscvO3CheckpointProjection) -> &RiscvO3LiveCheckpointPayload {
    match projection.live_capture() {
        RiscvO3LiveCheckpointCapture::Captured(live) => live,
        other => panic!("expected pending-load graph capture, got {other:?}"),
    }
}

fn install_graph_projection(
    destination: &RiscvCore,
    _source: &RiscvCore,
    projection: &RiscvO3CheckpointProjection,
) {
    let prepared = destination
        .prepare_checkpoint_restore(projection.replay().clone())
        .unwrap();
    destination.install_prepared_checkpoint_restore(prepared);
}

fn checkpoint_input_with_replay_hart(
    projection: &RiscvO3CheckpointProjection,
    source: &RiscvCore,
    stable: O3RuntimeCheckpointPayload,
    live: RiscvO3LiveCheckpointPayload,
) -> RiscvCoreCheckpointRestoreInput {
    RiscvCoreCheckpointRestoreInput::new(
        projection.replay().hart.clone(),
        source.pmp_snapshot(),
        source.hart_run_state(),
        source.in_order_pipeline_snapshot(),
        source.branch_predictor_checkpoint_payload(),
        source.gshare_branch_predictor_checkpoint_payload(),
        source.bimode_branch_predictor_checkpoint_payload(),
        source.tournament_branch_predictor_checkpoint_payload(),
        source.tage_sc_l_branch_predictor_checkpoint_payload(),
        source.multiperspective_perceptron_checkpoint_payload(),
        stable,
        Some(live),
    )
}

fn assert_payload_rows(
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
    producers: [u8; 3],
    producer_sequences: [u64; 3],
    root_ready: [bool; 3],
) {
    assert_eq!(rows.len(), 3);
    for (index, row) in rows.iter().enumerate() {
        let sequence = FIRST_LOAD_SEQUENCE + index as u64;
        let destination = row.destination.expect("pending load destination");
        assert_eq!(row.sequence, sequence);
        assert_eq!(row.fetch.request_id(), request(sequence));
        assert_eq!(
            row.fetch.pc(),
            Address::new(FIRST_LOAD_PC + 4 * index as u64)
        );
        assert_eq!(row.consumed_requests, [request(sequence)]);
        assert_eq!(row.fetch_predecessor_request, request(sequence - 1));
        assert_eq!(row.producer_register, reg(producers[index]));
        assert_eq!(row.producer_sequence, producer_sequences[index]);
        assert_eq!(row.root_sequence, ROOT_FETCH_SEQUENCE);
        assert_eq!(row.root_fetch_request, request(ROOT_FETCH_SEQUENCE));
        assert_eq!(row.root_range, root_range());
        assert_eq!(row.root_atomic, false);
        assert_eq!(row.lsq_kind, O3LoadStoreQueueKind::Load);
        assert_eq!(row.expected_lsq_bytes, 8);
        assert_eq!(
            row.published_producer_ready_tick,
            root_ready[index].then_some(CAPTURED_TICK)
        );
        assert_eq!(
            row.requested_wake_tick,
            root_ready[index].then_some(CAPTURED_TICK)
        );
        assert_eq!(
            (destination.register_class(), destination.architectural()),
            (
                O3RegisterClass::Integer,
                u32::from(FIRST_DESTINATION_REGISTER + index as u8),
            )
        );
    }
}

fn assert_stable_graph_owner_set(
    stable: &O3RuntimeCheckpointPayload,
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
) {
    let snapshot = stable.snapshot();
    assert_graph_rob_rows(snapshot.reorder_buffer(), rows);
    assert_graph_lsq_rows(snapshot.load_store_queue(), rows);
    assert_eq!(snapshot.rename_map().len(), 1);
    assert_eq!(
        (
            snapshot.rename_map()[0].register_class(),
            snapshot.rename_map()[0].architectural(),
        ),
        (O3RegisterClass::Integer, u32::from(ROOT_REGISTER))
    );
}

fn assert_restored_snapshot_owns_graph(
    snapshot: &O3RuntimeSnapshot,
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
) {
    assert_graph_rob_rows(snapshot.reorder_buffer(), rows);
    assert_graph_lsq_rows(snapshot.load_store_queue(), rows);
    let destinations = rows
        .iter()
        .map(|row| row.destination.unwrap())
        .collect::<Vec<_>>();
    assert!(destinations
        .iter()
        .all(|destination| snapshot.rename_map().contains(destination)));
    assert_eq!(
        destinations
            .iter()
            .map(|destination| destination.physical())
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert!(snapshot.rename_map().iter().any(|entry| {
        entry.register_class() == O3RegisterClass::Integer
            && entry.architectural() == u32::from(ROOT_REGISTER)
    }));
}

fn assert_graph_rob_rows(
    owners: &[O3ReorderBufferEntry],
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
) {
    assert_eq!(owners.len(), rows.len());
    for (owner, pending) in owners.iter().zip(rows) {
        let destination = pending.destination.expect("pending load destination");
        assert_eq!(owner.sequence(), pending.sequence);
        assert_eq!(owner.pc(), pending.fetch.pc());
        assert_eq!(owner.destination(), Some(destination.physical()));
        assert_eq!(
            owner.rename_destination(),
            Some((destination.register_class(), destination.architectural()))
        );
        assert!(owner.is_live_staged());
        assert!(!owner.is_ready());
    }
}

fn assert_graph_lsq_rows(
    entries: &[O3LoadStoreQueueEntry],
    rows: &[RiscvO3LiveCheckpointPendingDataAddress],
) {
    assert_eq!(entries.len(), rows.len());
    assert!(entries.iter().zip(rows).all(|(entry, row)| {
        entry.sequence() == row.sequence
            && entry.kind() == O3LoadStoreQueueKind::Load
            && entry.address().is_none()
            && entry.bytes() == 8
            && !entry.is_completed()
    }));
}

fn assert_restored_issue_plan(
    producers: [u8; 3],
    issue_width: usize,
    expected_issued: &[u64],
    expected_dependency_blocked: &[u64],
) {
    let fixture = PendingLoadGraphCheckpointFixture::new(producers, issue_width);
    let projection = fixture.capture();
    let destination = graph_core(issue_width);
    install_graph_projection(&destination, &fixture.core, &projection);
    let state = destination.state.lock().expect("riscv core lock");

    let (issued, dependency_blocked) = state
        .o3_runtime
        .checkpoint_issue_plan_at_for_test(CAPTURED_TICK)
        .unwrap();

    assert_eq!(issued, expected_issued);
    assert_eq!(dependency_blocked, expected_dependency_blocked);
}

fn expected_issue_rows() -> Vec<RiscvO3LiveCheckpointIssueRow> {
    [1, 2, 3]
        .into_iter()
        .map(|sequence| RiscvO3LiveCheckpointIssueRow {
            sequence,
            fetch_request: request(sequence),
        })
        .collect()
}

fn root_range() -> AddressRange {
    AddressRange::new(Address::new(ROOT_ADDRESS), AccessSize::new(8).unwrap()).unwrap()
}

fn next_fetch_pc() -> u64 {
    FIRST_LOAD_PC + 12
}

fn last_load_sequence() -> u64 {
    FIRST_LOAD_SEQUENCE + 2
}

fn ld(rd: u8, rs1: u8) -> u32 {
    i_type(0, rs1, 3, rd, 0x03)
}
