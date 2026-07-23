use super::*;

use super::dependent_result_address::materialized_load;
use crate::o3_runtime::{O3DataAccessWindowPolicy, O3PendingDataAddressRequest};
use rem6_isa_riscv::RiscvInstruction;

const HEAD_PC: u64 = 0x8000;
const FIRST_PC: u64 = 0x8004;
const SECOND_PC: u64 = 0x8008;
const THIRD_PC: u64 = 0x800c;
const HEAD_ADDRESS: u64 = 0x8800;
const ISSUE_TICK: u64 = 41;
const SUBMIT_TICK: u64 = 43;

struct ThreePendingIssueFixture {
    core: RiscvCore,
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    fetches: [MemoryRequestId; 3],
    sequences: [u64; 3],
    rob_count: usize,
    lsq_count: usize,
    head_lsq_count: usize,
    addresses: [u64; 3],
}

impl ThreePendingIssueFixture {
    fn new(addresses: [u64; 3], materialized: [bool; 3], atomic: bool) -> Self {
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
        let offsets = addresses.map(|address| {
            i32::try_from(address as i64 - addresses[0] as i64).expect("test offset fits")
        });
        let raws = [
            ld(6, 5, offsets[0]),
            ld(7, 5, offsets[1]),
            ld(8, 5, offsets[2]),
        ];
        let fetches = [memory_request(11), memory_request(12), memory_request(13)];
        let pending = raws.into_iter().enumerate().map(|(index, raw)| {
            O3PendingDataAddressRequest::new(
                memory_request(10 + index as u64),
                fetch_event_with_raw(FIRST_PC + 4 * index as u64, 11 + index as u64, raw),
                vec![fetches[index]],
                decode(raw),
                reg(5),
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
                [],
            ),
            3
        );
        for index in 0..3 {
            if materialized[index] {
                state
                    .o3_runtime
                    .set_pending_data_address_materialized_for_fetch_for_test(
                        fetches[index],
                        ISSUE_TICK + index as u64,
                        materialized_load(
                            fetch_event_with_raw(
                                FIRST_PC + 4 * index as u64,
                                11 + index as u64,
                                raws[index],
                            ),
                            decode(raws[index]),
                            FIRST_PC + 4 * index as u64,
                            6 + index as u8,
                            addresses[index],
                        ),
                    );
            }
        }
        state.hart.write(reg(5), addresses[0]);
        state.hart.set_pc(FIRST_PC);
        let snapshot = state.o3_runtime.snapshot();
        let sequences = [FIRST_PC, SECOND_PC, THIRD_PC].map(|pc| {
            snapshot
                .reorder_buffer()
                .iter()
                .find(|entry| entry.pc() == Address::new(pc))
                .expect("pending ROB row")
                .sequence()
        });
        let rob_count = snapshot.reorder_buffer().len();
        let lsq_count = snapshot.load_store_queue().len();
        drop(state);
        Self {
            core,
            scheduler,
            transport,
            fetches,
            sequences,
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

    fn assert_live_count_and_pending(&self, live: usize, pending: usize) {
        let state = self.core.state.lock().expect("riscv core lock");
        assert_eq!(state.o3_runtime.live_data_access_count_for_test(), live);
        assert_eq!(state.o3_runtime.pending_data_address_count(), pending);
    }

    fn outstanding_requests(&self) -> Vec<(MemoryRequestId, MemoryRequestId)> {
        outstanding_data_requests_in_fetch_order(&self.core)
    }

    fn assert_replay_survivors(
        &self,
        live_fetches: usize,
        expected_outstanding: Vec<(MemoryRequestId, MemoryRequestId)>,
    ) {
        let state = self.core.state.lock().expect("riscv core lock");
        let runtime = &state.o3_runtime;
        let snapshot = runtime.snapshot();
        let mut outstanding = state
            .outstanding_data
            .values()
            .map(|access| (access.fetch_request, access.request))
            .collect::<Vec<_>>();
        outstanding.sort_unstable_by_key(|(fetch, _)| fetch.sequence());
        assert_eq!(state.outstanding_data.len(), live_fetches);
        assert_eq!(outstanding, expected_outstanding);
        assert_eq!(runtime.pending_data_address_count(), 0);
        assert_eq!(runtime.live_data_access_count_for_test(), 1 + live_fetches);
        assert_eq!(snapshot.reorder_buffer().len(), 1 + live_fetches);
        assert_eq!(
            snapshot.load_store_queue().len(),
            self.head_lsq_count + live_fetches
        );
        assert_eq!(runtime.pending_data_address_wake_tick(), None);
        for index in 0..3 {
            let identity = runtime.live_data_access_issue_identity_for_test(self.fetches[index]);
            let lsq_address = snapshot
                .load_store_queue()
                .iter()
                .find(|entry| entry.sequence() == self.sequences[index])
                .and_then(|entry| entry.address());
            if index < live_fetches {
                assert_eq!(
                    identity,
                    Some((
                        self.sequences[index],
                        self.rob_count,
                        self.lsq_count,
                        ISSUE_TICK + index as u64,
                    ))
                );
                assert_eq!(lsq_address, Some(Address::new(self.addresses[index])));
            } else {
                assert_eq!(identity, None, "fetch {index} live identity");
                assert_eq!(lsq_address, None, "fetch {index} LSQ address");
            }
            assert!(!runtime.pending_data_address_owns_fetch(self.fetches[index]));
        }
    }
}

#[test]
fn three_pending_unissued_selector_returns_oldest_materialized_first() {
    for (materialized, expected) in [
        ([false, false, true], 2),
        ([false, true, true], 1),
        ([true, true, true], 0),
    ] {
        let fixture = ThreePendingIssueFixture::new([0x9000, 0x9008, 0x9010], materialized, false);
        let state = fixture.core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.next_unissued_data_access().unwrap().0,
            fixture.fetches[expected]
        );
    }
}

#[test]
fn three_pending_bind_removes_exact_rows_in_sequence() {
    let mut fixture =
        ThreePendingIssueFixture::new([0x9000, 0x9008, 0x9010], [true, true, true], false);

    for index in 0..3 {
        assert!(fixture.issue());
        fixture.assert_live_count_and_pending(2 + index, 2 - index);
    }

    let state = fixture.core.state.lock().expect("riscv core lock");
    assert!(fixture.fetches.iter().all(|fetch| {
        state
            .o3_runtime
            .live_data_access_issue_identity_for_test(*fetch)
            .is_some()
    }));
    assert!(state
        .o3_runtime
        .snapshot()
        .load_store_queue()
        .iter()
        .filter(|entry| fixture.sequences.contains(&entry.sequence()))
        .all(|entry| entry.address().is_some()));
}

#[test]
fn three_pending_first_replay_discards_complete_suffix() {
    let mut fixture =
        ThreePendingIssueFixture::new([0x9000, 0x9008, 0x9010], [true, false, false], false);
    fixture
        .core
        .add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9008).unwrap())
        .unwrap();

    assert!(!fixture.issue());

    fixture.assert_replay_survivors(0, Vec::new());
}

#[test]
fn three_pending_middle_replay_preserves_first_live_access() {
    let mut fixture =
        ThreePendingIssueFixture::new([0x9000, 0x9008, 0x9010], [true, true, false], false);
    assert!(fixture.issue());
    let expected = fixture.outstanding_requests();
    fixture
        .core
        .add_pma_uncacheable_range(RiscvPmaRange::new(0x9008, 0x9010).unwrap())
        .unwrap();

    assert!(!fixture.issue());

    fixture.assert_replay_survivors(1, expected);
}

#[test]
fn three_pending_last_replay_preserves_two_older_live_accesses() {
    let mut fixture =
        ThreePendingIssueFixture::new([0x9000, 0x9008, 0x9010], [true, true, true], false);
    assert!(fixture.issue());
    assert!(fixture.issue());
    let expected = fixture.outstanding_requests();
    fixture
        .core
        .add_pma_uncacheable_range(RiscvPmaRange::new(0x9010, 0x9018).unwrap())
        .unwrap();

    assert!(!fixture.issue());

    fixture.assert_replay_survivors(2, expected);
}

#[test]
fn three_pending_atomic_root_range_applies_to_every_descendant() {
    for replay_index in 0..3 {
        let mut addresses = [0x9000, 0x9010, 0x9020];
        addresses[replay_index] = HEAD_ADDRESS;
        let mut fixture = ThreePendingIssueFixture::new(addresses, [true, true, true], true);
        for index in 0..replay_index {
            assert!(fixture.issue(), "older issue {index}");
        }

        assert!(!fixture.issue(), "descendant {replay_index} overlaps root");
        fixture.assert_live_count_and_pending(1 + replay_index, 0);
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

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}
