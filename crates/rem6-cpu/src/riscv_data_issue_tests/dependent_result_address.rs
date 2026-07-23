use super::*;

use crate::o3_runtime::{O3DataAccessWindowPolicy, O3PendingDataAddressRequest};
use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvInstruction};

const HEAD_PC: u64 = 0x8000;
const PENDING_PC: u64 = 0x8004;
const FIRST_SUFFIX_PC: u64 = 0x8008;
const SECOND_SUFFIX_PC: u64 = 0x800c;
const HEAD_ADDRESS: u64 = 0x8800;
const ISSUE_TICK: u64 = 41;
const SUBMIT_TICK: u64 = 43;
pub(super) struct PendingIssueFixture {
    pub(super) core: RiscvCore,
    pub(super) scheduler: PartitionedScheduler,
    pub(super) transport: MemoryTransport,
    data_route: MemoryRouteId,
    fetch_request: MemoryRequestId,
    head_lsq_count: usize,
}

impl PendingIssueFixture {
    pub(super) fn load(address: u64) -> Self {
        Self::new(address, false)
    }
    fn atomic(address: u64) -> Self {
        Self::new(address, true)
    }
    fn new(address: u64, atomic_head: bool) -> Self {
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
        let head = if atomic_head {
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
        let raw = ld(6, 5, 0);
        let decoded = decode(raw);
        let fetch = fetch_event_with_raw(PENDING_PC, 11, raw);
        let fetch_request = fetch.request_id();
        let materialized = materialized_load(fetch.clone(), decoded, PENDING_PC, 6, address);
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
                vec![O3PendingDataAddressRequest::new(
                    head.fetch().request_id(),
                    fetch,
                    vec![fetch_request],
                    decoded,
                    reg(5),
                )],
                [
                    (
                        Address::new(FIRST_SUFFIX_PC),
                        decode(addi(7, 5, 8)).instruction()
                    ),
                    (
                        Address::new(SECOND_SUFFIX_PC),
                        decode(add(8, 6, 7)).instruction()
                    ),
                ],
                ISSUE_TICK,
            ),
            3
        );
        state
            .o3_runtime
            .set_pending_data_address_materialized_for_test(ISSUE_TICK, materialized);
        state.hart.write(reg(5), address);
        state.hart.set_pc(PENDING_PC);
        drop(state);
        Self {
            core,
            scheduler,
            transport,
            data_route,
            fetch_request,
            head_lsq_count: 1 + usize::from(atomic_head),
        }
    }

    fn allocation_identity(&self) -> (u64, u32, usize, usize) {
        let state = self.core.state.lock().expect("riscv core lock");
        let sequence = state
            .o3_runtime
            .pending_data_address_sequence_for_test()
            .expect("pending sequence");
        let physical = state
            .o3_runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.sequence() == sequence)
            .and_then(|entry| entry.destination())
            .expect("pending physical destination")
            .get();
        (
            sequence,
            physical,
            state.o3_runtime.snapshot().reorder_buffer().len(),
            state.o3_runtime.snapshot().load_store_queue().len(),
        )
    }

    fn outstanding(&self, forwarded: Option<Vec<u8>>) -> OutstandingDataAccess {
        OutstandingDataAccess {
            tick: ISSUE_TICK,
            partition: PartitionId::new(0),
            target: RiscvDataAccessTarget::Memory {
                route: self.data_route,
                endpoint: endpoint("cpu0.dmem"),
            },
            request_id: memory_request(30),
            fetch_request: self.fetch_request,
            access: MemoryAccessKind::Load {
                rd: reg(6),
                address: 0x9000,
                width: MemoryWidth::Doubleword,
                signed: true,
            },
            size: AccessSize::new(8).unwrap(),
            physical_address: Address::new(0x9000),
            request_byte_offset: 0,
            line_layout: Some(line_layout()),
            forwarded_load_data: forwarded,
            store_load_forwarding_plan: None,
        }
    }

    fn issue_memory(&mut self) -> Option<PartitionEventId> {
        self.core
            .issue_next_data_access(
                &mut self.scheduler,
                &self.transport,
                MemoryTrace::new(),
                |_delivery, _context| TargetOutcome::NoResponse,
            )
            .unwrap()
    }

    fn assert_replayed(&self) {
        let state = self.core.state.lock().expect("riscv core lock");
        assert!(!state.o3_runtime.has_pending_data_address());
        assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(
            state.o3_runtime.snapshot().load_store_queue().len(),
            self.head_lsq_count
        );
        assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 1);
        assert!(state.outstanding_data.is_empty());
        assert!(state
            .o3_runtime
            .snapshot()
            .rename_map()
            .iter()
            .all(|entry| {
                entry.architectural() != 6
                    && entry.architectural() != 7
                    && entry.architectural() != 8
            }));
    }
}

#[test]
fn dependent_result_address_pre_submit_validation_binds_serial_request() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    fixture.core.enable_checker_cpu();
    let calls = Arc::new(AtomicU64::new(0));
    let responder_calls = calls.clone();

    assert!(fixture
        .core
        .issue_next_data_access(
            &mut fixture.scheduler,
            &fixture.transport,
            MemoryTrace::new(),
            move |_delivery, _context| {
                responder_calls.fetch_add(1, Ordering::Relaxed);
                TargetOutcome::NoResponse
            },
        )
        .unwrap()
        .is_some());

    let state = fixture.core.state.lock().expect("riscv core lock");
    assert!(!state.o3_runtime.has_pending_data_address());
    assert_eq!(state.outstanding_data.len(), 1);
    assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 2);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(state.hart.pc(), FIRST_SUFFIX_PC);
    let checker = state.checker.as_ref().expect("checker CPU").snapshot();
    assert_eq!(checker.checked_instructions(), 1);
    assert!(checker.mismatches().is_empty());
}

#[test]
fn dependent_result_address_parallel_transaction_binds_after_submit() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    let prepared = fixture
        .core
        .prepare_data_parallel_access(
            fixture.scheduler.now(),
            &fixture.transport,
            MemoryTrace::new(),
            |_delivery, _context| TargetOutcome::NoResponse,
        )
        .unwrap()
        .expect("pending load prepares");
    assert!(fixture
        .core
        .state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .has_pending_data_address());

    fixture
        .core
        .submit_prepared_data_parallel_access(&mut fixture.scheduler, &fixture.transport, prepared)
        .unwrap()
        .expect("pending load submits");

    let state = fixture.core.state.lock().expect("riscv core lock");
    assert!(!state.o3_runtime.has_pending_data_address());
    assert_eq!(state.outstanding_data.len(), 1);
}

#[test]
fn dependent_result_address_forwarded_load_binds_without_transport() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    let issue = fixture.outstanding(Some(0x2a_u64.to_le_bytes().to_vec()));

    fixture
        .core
        .schedule_forwarded_load_completion(&mut fixture.scheduler, issue)
        .unwrap();

    let state = fixture.core.state.lock().expect("riscv core lock");
    assert!(!state.o3_runtime.has_pending_data_address());
    assert_eq!(state.outstanding_data.len(), 1);
    assert_eq!(state.o3_runtime.live_data_access_count_for_test(), 2);
}

#[test]
fn dependent_result_address_pmp_denial_replays_without_request() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    fixture
        .core
        .set_privilege_mode(rem6_isa_riscv::RiscvPrivilegeMode::User);
    fixture.core.write_pmp_addr(0, 0xa000 >> 2).unwrap();
    fixture
        .core
        .write_pmp_config(
            0,
            rem6_isa_riscv::RiscvPmpConfig::new(rem6_isa_riscv::RiscvPmpAddressMode::Tor),
        )
        .unwrap();

    assert!(fixture.issue_memory().is_none());
    fixture.assert_replayed();
}

#[test]
fn dependent_result_address_pma_uncacheable_replays_without_request() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    fixture
        .core
        .add_pma_uncacheable_range(RiscvPmaRange::new(0x9000, 0x9008).unwrap())
        .unwrap();

    assert!(fixture.issue_memory().is_none());
    fixture.assert_replayed();
}

#[test]
fn dependent_result_address_cross_line_replays_without_request() {
    let mut fixture = PendingIssueFixture::load(0x900c);
    assert!(fixture.issue_memory().is_none());
    fixture.assert_replayed();
}

#[test]
fn dependent_result_address_mmio_route_replays_without_mmio_request() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    let bus = test_mmio_bus(0x9000, vec![0; 8]);

    assert!(fixture
        .core
        .issue_next_mmio_data_access_parallel(&mut fixture.scheduler, &bus)
        .unwrap()
        .is_none());
    fixture.assert_replayed();
}

#[test]
fn dependent_result_address_unknown_memory_route_replays_without_request() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    assert!(fixture
        .core
        .issue_next_data_access(
            &mut fixture.scheduler,
            &MemoryTransport::new(),
            MemoryTrace::new(),
            |_delivery, _context| panic!("unknown route must replay"),
        )
        .unwrap()
        .is_none());
    fixture.assert_replayed();
}

#[test]
fn dependent_result_address_atomic_overlap_replays_without_request() {
    let mut fixture = PendingIssueFixture::atomic(HEAD_ADDRESS);
    assert!(fixture.issue_memory().is_none());
    fixture.assert_replayed();
}

#[test]
fn dependent_result_address_dropped_parallel_prepare_discards_pending_suffix() {
    let fixture = PendingIssueFixture::load(0x9000);
    let calls = Arc::new(AtomicU64::new(0));
    let responder_calls = calls.clone();
    let prepared = fixture
        .core
        .prepare_data_parallel_access(
            fixture.scheduler.now(),
            &fixture.transport,
            MemoryTrace::new(),
            move |_delivery, _context| {
                responder_calls.fetch_add(1, Ordering::Relaxed);
                TargetOutcome::NoResponse
            },
        )
        .unwrap()
        .expect("pending load prepares");

    drop(prepared);

    fixture.assert_replayed();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn dependent_result_address_submit_failure_discards_pending_suffix() {
    let fixture = PendingIssueFixture::load(0x9000);
    let calls = Arc::new(AtomicU64::new(0));
    let responder_calls = calls.clone();
    let prepared = fixture
        .core
        .prepare_data_parallel_access(
            fixture.scheduler.now(),
            &fixture.transport,
            MemoryTrace::new(),
            move |_delivery, _context| {
                responder_calls.fetch_add(1, Ordering::Relaxed);
                TargetOutcome::NoResponse
            },
        )
        .unwrap()
        .expect("pending load prepares");
    let mut undersized_scheduler = PartitionedScheduler::new(1).unwrap();

    assert!(fixture
        .core
        .submit_prepared_data_parallel_access(
            &mut undersized_scheduler,
            &fixture.transport,
            prepared,
        )
        .is_err());
    fixture.assert_replayed();
    assert_eq!(calls.load(Ordering::Relaxed), 0);
}

#[test]
fn pending_address_bind_reuses_sequence_and_resolves_lsq_address() {
    let mut fixture = PendingIssueFixture::load(0x9000);
    let before = fixture.allocation_identity();

    fixture.issue_memory().unwrap();

    let state = fixture.core.state.lock().expect("riscv core lock");
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), before.2);
    assert_eq!(
        state.o3_runtime.snapshot().load_store_queue().len(),
        before.3
    );
    assert_eq!(
        state
            .o3_runtime
            .live_data_access_issue_identity_for_test(fixture.fetch_request),
        Some((before.0, before.2, before.3, ISSUE_TICK))
    );
    assert_eq!(state.data_events.last().unwrap().tick(), SUBMIT_TICK);
    assert_eq!(
        state
            .o3_runtime
            .snapshot()
            .load_store_queue()
            .iter()
            .find(|entry| entry.sequence() == before.0)
            .and_then(|entry| entry.address()),
        Some(Address::new(0x9000))
    );
    assert_eq!(
        state
            .o3_runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.sequence() == before.0)
            .and_then(|entry| entry.destination())
            .map(|physical| physical.get()),
        Some(before.1)
    );
}

#[test]
fn pending_address_bind_publishes_one_execution_event() {
    let mut fixture = PendingIssueFixture::load(0x9000);

    fixture.issue_memory().unwrap();

    let state = fixture.core.state.lock().expect("riscv core lock");
    let published = state
        .events
        .iter()
        .filter(|event| event.fetch().request_id() == fixture.fetch_request)
        .collect::<Vec<_>>();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].fetch_pc(), Address::new(PENDING_PC));
    assert_eq!(
        state.executed_fetches.contains(&fixture.fetch_request),
        true
    );
}

pub(super) fn materialized_load(
    fetch: CpuFetchEvent,
    decoded: RiscvDecodedInstruction,
    pc: u64,
    rd: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let access = MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width: MemoryWidth::Doubleword,
        signed: true,
    };
    RiscvCpuExecutionEvent::new(
        fetch,
        decoded.instruction(),
        RiscvExecutionRecord::new_with_instruction_bytes(
            decoded.instruction(),
            decoded.bytes(),
            pc,
            pc + 4,
            Vec::new(),
            Some(access),
        ),
    )
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

fn decode(raw: u32) -> RiscvDecodedInstruction {
    RiscvInstruction::decode_with_length(raw).unwrap()
}

fn ld(rd: u8, rs1: u8, offset: i32) -> u32 {
    i_type(offset, rs1, 0b011, rd, 0x03)
}

fn addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0b000, rd, 0x13)
}

fn add(rd: u8, rs1: u8, rs2: u8) -> u32 {
    (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (u32::from(rd) << 7) | 0x33
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}
