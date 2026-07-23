use super::*;

use crate::o3_runtime::{O3DataAccessWindowPolicy, O3PendingDataAddressRequest};
use rem6_isa_riscv::RiscvInstruction;

const HEAD_PC: u64 = 0x8000;
const FIRST_PC: u64 = 0x8004;
const SECOND_PC: u64 = 0x8008;
const SUFFIX_PC: u64 = 0x800c;
const HEAD_ADDRESS: u64 = 0x8800;
const ISSUE_TICK: u64 = 41;
const SUBMIT_TICK: u64 = 43;
const PENDING_WAKE_TICK: u64 = 47;

struct TwoPendingIssueFixture {
    core: RiscvCore,
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    fetches: [MemoryRequestId; 2],
    sequences: [u64; 2],
    physicals: [u32; 2],
    rob_count: usize,
    lsq_count: usize,
    head_lsq_count: usize,
    addresses: [u64; 2],
}

impl TwoPendingIssueFixture {
    fn siblings(addresses: [u64; 2], materialized: [bool; 2]) -> Self {
        Self::new(addresses, materialized, false, false)
    }

    fn chain(addresses: [u64; 2], materialized: [bool; 2], atomic: bool) -> Self {
        Self::new(addresses, materialized, true, atomic)
    }

    fn new(addresses: [u64; 2], materialized: [bool; 2], chained: bool, atomic: bool) -> Self {
        let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
        scheduler
            .schedule_at(PartitionId::new(0), SUBMIT_TICK, |_| {})
            .unwrap();
        scheduler.run_until_idle();
        let core = RiscvCore::with_data(
            cpu_core(fetch_route, HEAD_PC),
            CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
        );
        core.set_detailed_live_retire_gate_enabled(true);
        core.set_o3_window_depths(4, 4);
        let head = if atomic {
            super::atomic::atomic_memory_event(HEAD_PC, 10, HEAD_ADDRESS)
        } else {
            scalar_load_event_with_base_width(
                HEAD_PC,
                10,
                5,
                2,
                HEAD_ADDRESS,
                MemoryWidth::Doubleword,
                false,
            )
        };
        let first_raw = ld(6, 5, 0);
        let second_base = if chained { 6 } else { 5 };
        let second_offset = if chained {
            0
        } else {
            i32::try_from(addresses[1] - addresses[0]).expect("test offset fits")
        };
        let second_raw = ld(7, second_base, second_offset);
        let raws = [first_raw, second_raw];
        let fetches = [memory_request(11), memory_request(12)];
        let pending = raws.into_iter().enumerate().map(|(index, raw)| {
            O3PendingDataAddressRequest::new(
                memory_request(10 + index as u64),
                fetch_event_with_raw(FIRST_PC + 4 * index as u64, 11 + index as u64, raw),
                vec![fetches[index]],
                decode(raw),
                reg(if index == 0 { 5 } else { second_base }),
            )
        });
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.stage_live_data_access_issue(
            &head,
            memory_request(20),
            31,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
        assert_eq!(
            state.o3_runtime.stage_pending_data_address_window(
                head.fetch().request_id(),
                pending,
                [(Address::new(SUFFIX_PC), decode(addi(8, 7, 1)).instruction())],
                ISSUE_TICK,
            ),
            3
        );
        for index in 0..2 {
            if materialized[index] {
                state
                    .o3_runtime
                    .set_pending_data_address_materialized_for_fetch_for_test(
                        fetches[index],
                        ISSUE_TICK + index as u64,
                        super::dependent_result_address::materialized_load(
                            fetch_event_with_raw(
                                FIRST_PC + 4 * index as u64,
                                11 + index as u64,
                                raws[index],
                            ),
                            decode(raws[index]),
                            FIRST_PC + 4 * index as u64,
                            if index == 0 { 6 } else { 7 },
                            addresses[index],
                        ),
                    );
            }
        }
        state.hart.write(reg(5), addresses[0]);
        if chained {
            state.hart.write(reg(6), addresses[1]);
        }
        state.hart.set_pc(FIRST_PC);
        let snapshot = state.o3_runtime.snapshot();
        let entries = [FIRST_PC, SECOND_PC].map(|pc| {
            snapshot
                .reorder_buffer()
                .iter()
                .find(|entry| entry.pc() == Address::new(pc))
                .expect("pending ROB row")
        });
        let sequences = entries.map(|entry| entry.sequence());
        let physicals = entries.map(|entry| entry.destination().unwrap().get());
        let rob_count = snapshot.reorder_buffer().len();
        let lsq_count = snapshot.load_store_queue().len();
        drop(state);
        Self {
            core,
            scheduler,
            transport,
            fetches,
            sequences,
            physicals,
            rob_count,
            lsq_count,
            head_lsq_count: 1 + usize::from(atomic),
            addresses,
        }
    }

    fn issue(&mut self) -> bool {
        self.core
            .issue_next_data_access(
                &mut self.scheduler,
                &self.transport,
                MemoryTrace::new(),
                |_delivery, _context| TargetOutcome::NoResponse,
            )
            .unwrap()
            .is_some()
    }

    fn make_second_uncacheable(&self) {
        self.core
            .add_pma_uncacheable_range(
                RiscvPmaRange::new(self.addresses[1], self.addresses[1] + 8).unwrap(),
            )
            .unwrap();
    }

    fn assert_first_live_and_no_pending(&self) {
        let state = self.core.state.lock().expect("riscv core lock");
        assert_eq!(state.o3_runtime.pending_data_address_count(), 0);
        assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 2);
        assert!(state
            .o3_runtime
            .live_data_access_issue_identity_for_test(self.fetches[0])
            .is_some());
        assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 2);
        assert_eq!(
            state.o3_runtime.snapshot().load_store_queue().len(),
            self.head_lsq_count + 1
        );
        assert!(state.o3_runtime.pending_data_address_wake_tick().is_none());
    }

    fn schedule_second_pending_address_wake(&mut self) {
        let mut state = self.core.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .set_pending_data_address_resource_blocked_wake_for_test(
                self.sequences[1],
                PENDING_WAKE_TICK,
            );
        drop(state);
        let requested = self.core.requested_o3_writeback_wake_tick(SUBMIT_TICK);
        let event = self
            .scheduler
            .schedule_at(PartitionId::new(0), PENDING_WAKE_TICK, |_| {})
            .unwrap();
        self.core.mark_o3_writeback_wake_scheduled(
            self.scheduler.instance_id(),
            self.scheduler.pending_event_snapshot(event).unwrap(),
        );
        assert_eq!(
            (requested, self.core.owned_o3_writeback_wakes().len()),
            (Some(PENDING_WAKE_TICK), 1)
        );
    }

    fn assert_first_pending_suffix_discarded_without_stale_wake(&self) {
        let mut state = self.core.state.lock().expect("riscv core lock");
        assert_eq!(state.o3_runtime.pending_data_address_count(), 0);
        assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(state.o3_runtime.snapshot().load_store_queue().len(), 1);
        assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 1);
        state.o3_runtime.discard_live_data_access_lifecycle();
        drop(state);
        assert_eq!(
            (
                self.core.owned_o3_writeback_wakes(),
                self.core.data_access_lifecycle_is_quiescent(),
            ),
            (Vec::new(), true)
        );
    }
}

#[test]
fn two_pending_unissued_selector_returns_oldest_materialized_first() {
    let fixture = TwoPendingIssueFixture::siblings([0x9000, 0x9008], [false, true]);
    let state = fixture.core.state.lock().expect("riscv core lock");
    let selected = state.next_unissued_data_access().unwrap().0;
    assert_eq!(selected, fixture.fetches[1]);
}

#[test]
fn two_pending_data_access_execution_looks_up_exact_pending_fetch() {
    let fixture = TwoPendingIssueFixture::siblings([0x9000, 0x9008], [true, true]);
    let mut state = fixture.core.state.lock().expect("riscv core lock");
    let second = state.data_access_execution(fixture.fetches[1]).unwrap();
    assert_eq!(second.fetch_pc(), Address::new(SECOND_PC));
    state
        .data_access_execution_mut(fixture.fetches[1])
        .unwrap()
        .set_data_access_event_kind(RiscvDataAccessEventKind::ConditionalFailed);
    let kinds = fixture.fetches.map(|fetch| {
        state
            .data_access_execution(fetch)
            .unwrap()
            .data_access_event_kind()
    });
    assert_eq!(
        kinds,
        [None, Some(RiscvDataAccessEventKind::ConditionalFailed)]
    );
    let lineage_request = memory_request(99);
    let runtime = &mut state.o3_runtime;
    runtime
        .append_pending_data_address_consumed_request_for_test(fixture.fetches[0], lineage_request);
    assert!(runtime.pending_data_address_owns_fetch(lineage_request));
    let before_discard = runtime.clone();
    assert!(!runtime.discard_pending_data_address_for_fetch(lineage_request));
    assert_eq!(*runtime, before_discard);
    let primary = runtime
        .pending_data_address_execution_for_fetch(fixture.fetches[0])
        .cloned()
        .unwrap();
    let lineage_execution = RiscvCpuExecutionEvent::new(
        fetch_event_with_raw(FIRST_PC, 99, ld(6, 5, 0)),
        primary.instruction(),
        primary.execution().clone(),
    );
    assert_eq!(
        runtime.bind_pending_data_address_issue(
            &lineage_execution,
            memory_request(40),
            Address::new(0x9000),
            SUBMIT_TICK,
        ),
        None
    );
    assert_eq!(runtime.pending_data_address_count(), 2);
}

#[test]
fn two_pending_bind_first_removes_exact_row_and_keeps_second_pending() {
    let mut fixture = TwoPendingIssueFixture::chain([0x9000, 0x9040], [true, false], false);
    let expected_seed = fixture
        .core
        .state
        .lock()
        .unwrap()
        .o3_runtime
        .pending_data_address_wake_seed()
        .unwrap();
    assert!(fixture.issue());
    let state = fixture.core.state.lock().expect("riscv core lock");
    let snapshot = state.o3_runtime.snapshot();
    assert_eq!(state.o3_runtime.pending_data_address_count(), 1);
    assert_eq!(
        (
            snapshot.reorder_buffer().len(),
            snapshot.load_store_queue().len()
        ),
        (fixture.rob_count, fixture.lsq_count)
    );
    assert_eq!(
        state
            .o3_runtime
            .live_data_access_issue_identity_for_test(fixture.fetches[0]),
        Some((
            fixture.sequences[0],
            fixture.rob_count,
            fixture.lsq_count,
            ISSUE_TICK
        ))
    );
    assert_eq!(
        snapshot
            .load_store_queue()
            .iter()
            .find(|entry| entry.sequence() == fixture.sequences[0])
            .and_then(|entry| entry.address()),
        Some(Address::new(fixture.addresses[0]))
    );
    assert!(snapshot
        .load_store_queue()
        .iter()
        .any(|entry| entry.sequence() == fixture.sequences[1] && entry.address().is_none()));
    assert_eq!(
        snapshot
            .reorder_buffer()
            .iter()
            .find(|entry| entry.sequence() == fixture.sequences[0])
            .and_then(|entry| entry.destination())
            .map(|physical| physical.get()),
        Some(fixture.physicals[0])
    );
    let seed = state.o3_runtime.pending_data_address_wake_seed().unwrap();
    assert_eq!(seed, expected_seed);
    assert_eq!(seed.fetch_predecessor_request(), fixture.fetches[0]);
    assert_eq!(seed.younger_pcs(), [SECOND_PC, SUFFIX_PC].map(Address::new));
}

#[test]
fn two_pending_bind_second_preserves_first_live_access() {
    let mut fixture = TwoPendingIssueFixture::siblings([0x9000, 0x9008], [true, true]);
    assert!(fixture.issue());
    let first_identity = fixture
        .core
        .state
        .lock()
        .unwrap()
        .o3_runtime
        .live_data_access_issue_identity_for_test(fixture.fetches[0]);
    assert!(fixture.issue());
    let state = fixture.core.state.lock().expect("riscv core lock");
    let snapshot = state.o3_runtime.snapshot();
    let identities = fixture.fetches.map(|fetch| {
        state
            .o3_runtime
            .live_data_access_issue_identity_for_test(fetch)
    });
    assert_eq!(
        (
            state.o3_runtime.pending_data_address_count(),
            state.o3_runtime.live_data_access_count_for_test(),
            snapshot.reorder_buffer().len(),
            snapshot.load_store_queue().len(),
            identities[0],
            identities[1],
        ),
        (
            0,
            3,
            fixture.rob_count,
            fixture.lsq_count,
            first_identity,
            Some((
                fixture.sequences[1],
                fixture.rob_count,
                fixture.lsq_count,
                ISSUE_TICK + 1,
            )),
        )
    );
    assert_eq!(
        snapshot
            .load_store_queue()
            .iter()
            .find(|entry| entry.sequence() == fixture.sequences[1])
            .and_then(|entry| entry.address()),
        Some(Address::new(fixture.addresses[1]))
    );
    assert_eq!(
        snapshot
            .reorder_buffer()
            .iter()
            .find(|entry| entry.sequence() == fixture.sequences[1])
            .and_then(|entry| entry.destination())
            .map(|physical| physical.get()),
        Some(fixture.physicals[1])
    );
}

#[test]
fn two_pending_first_pre_submit_replay_discards_second_and_suffix() {
    let mut fixture = TwoPendingIssueFixture::siblings([0x9000, 0x9008], [true, false]);
    fixture.schedule_second_pending_address_wake();
    fixture
        .core
        .add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9008).unwrap())
        .unwrap();
    assert!(!fixture.issue());
    fixture.assert_first_pending_suffix_discarded_without_stale_wake();
}

#[test]
fn two_pending_dropped_parallel_prepare_clears_scheduled_pending_wake() {
    let mut fixture = TwoPendingIssueFixture::siblings([0x9000, 0x9008], [true, false]);
    fixture.schedule_second_pending_address_wake();
    let prepared = fixture
        .core
        .prepare_data_parallel_access(
            fixture.scheduler.now(),
            &fixture.transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("first dependent address prepares");
    drop(prepared);
    fixture.assert_first_pending_suffix_discarded_without_stale_wake();
}

#[test]
fn two_pending_second_pre_submit_replay_preserves_first_live_access() {
    let mut fixture = TwoPendingIssueFixture::siblings([0x9000, 0x9008], [true, true]);
    assert!(fixture.issue());
    fixture.make_second_uncacheable();
    assert!(!fixture.issue());
    fixture.assert_first_live_and_no_pending();
}

#[test]
fn two_pending_atomic_chain_second_overlap_uses_root_head_range() {
    let mut fixture = TwoPendingIssueFixture::chain([0x9000, HEAD_ADDRESS], [true, true], true);
    assert!(fixture.issue());
    assert!(!fixture.issue());
    fixture.assert_first_live_and_no_pending();
}

#[test]
fn two_pending_second_pma_and_cross_line_replay_preserve_first_live_access() {
    for (second, uncacheable) in [(0x9040, true), (0x900c, false)] {
        let mut fixture = TwoPendingIssueFixture::siblings([0x9000, second], [true, true]);
        assert!(fixture.issue());
        if uncacheable {
            fixture.make_second_uncacheable();
        }
        assert!(!fixture.issue());
        fixture.assert_first_live_and_no_pending();
    }
}

fn fetch_event_with_raw(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            endpoint("cpu0.ifetch"),
            memory_request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn decode(raw: u32) -> rem6_isa_riscv::RiscvDecodedInstruction {
    RiscvInstruction::decode_with_length(raw).unwrap()
}

fn ld(rd: u8, rs1: u8, offset: i32) -> u32 {
    i_type(offset, rs1, 0b011, rd, 0x03)
}

fn addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0b000, rd, 0x13)
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}
