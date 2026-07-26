use rem6_isa_riscv::{FloatRegister, FloatRegisterWrite, RiscvFloatRoundingMode};

use super::super::super::o3_runtime_issue::O3LiveIssueForwardedValue;
use super::*;

const RESPONSE_TICK: u64 = 41;
const BOXED_TWO: u64 = 0xffff_ffff_4000_0000;

struct FpLoadQueueFixture {
    runtime: O3RuntimeState,
    hart: RiscvHartState,
    load: RiscvCpuExecutionEvent,
    load_sequence: u64,
    consumer: RiscvInstruction,
    consumer_sequence: u64,
}

impl FpLoadQueueFixture {
    fn staged() -> Self {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        assert!(runtime.set_issue_width(1));
        assert!(runtime.set_writeback_width(1));
        let load = float_load_event(4, MemoryWidth::Word);
        assert!(runtime.stage_live_data_access_issue(
            &load,
            request(20),
            31,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
        let load_sequence = runtime.live_data_accesses[0].sequence;
        let consumer = float_mul_s(5, 4, 3);
        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                load.fetch().request_id(),
                [(Address::new(BRANCH_PC), consumer)],
            ),
            1,
        );
        let consumer_sequence = runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.pc() == Address::new(BRANCH_PC))
            .expect("FP load consumer staged")
            .sequence();
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(BRANCH_PC),
            fp_decoded(consumer),
            &[request(21)],
            RESPONSE_TICK,
        ));
        let mut hart = RiscvHartState::new(BRANCH_PC);
        hart.write_float(f(3), 0xffff_ffff_4040_0000);
        Self {
            runtime,
            hart,
            load,
            load_sequence,
            consumer,
            consumer_sequence,
        }
    }

    fn complete(&mut self) -> u64 {
        let mut completed = self.load.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(self
            .runtime
            .complete_live_data_access_response(
                &completed,
                request(20),
                RESPONSE_TICK,
                10,
                Some(&2.0f32.to_bits().to_le_bytes()),
            )
            .unwrap());
        let admitted_tick = self
            .runtime
            .writeback_reservation(self.load_sequence)
            .expect("FP load writeback reservation")
            .admitted_tick();
        assert!(admitted_tick > RESPONSE_TICK);
        admitted_tick
    }

    fn candidate(&self) -> Option<O3LiveSpeculativeIssueCandidate> {
        self.runtime
            .live_speculative_issue_candidate(Address::new(BRANCH_PC), self.consumer)
    }
}

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn float_load_event(rd: u8, width: MemoryWidth) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::FloatLoad {
        rd: f(rd),
        rs1: reg(10),
        offset: Immediate::new(0),
        width,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(LOAD_PC, 10),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            LOAD_PC,
            LOAD_PC + 4,
            Vec::new(),
            Some(MemoryAccessKind::FloatLoad {
                rd: f(rd),
                address: 0x9000,
                width,
            }),
        ),
    )
}

fn float_mul_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatMulS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn fp_decoded(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    let raw = match instruction {
        RiscvInstruction::FloatMulS { rd, rs1, rs2, .. } => {
            fp_raw(0x08, rs2.index(), rs1.index(), rd.index())
        }
        _ => panic!("unsupported FP-load forwarding instruction: {instruction:?}"),
    };
    RiscvInstruction::decode_with_length(raw).unwrap()
}

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25) | (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (u32::from(rd) << 7) | 0x53
}

#[test]
fn fp_load_queue_blocks_before_response_and_before_writeback_admission() {
    let mut fixture = FpLoadQueueFixture::staged();
    assert!(fixture.candidate().is_none());

    let admitted_tick = fixture.complete();
    let candidate = fixture
        .candidate()
        .expect("completed FP load should materialize its consumer");
    assert_eq!(
        candidate.forwarded_values(),
        &[O3LiveIssueForwardedValue::FloatingPoint(
            FloatRegisterWrite::new(f(4), BOXED_TWO),
        )],
    );
    assert_eq!(candidate.producer_sequences(), &[fixture.load_sequence]);
    assert_eq!(candidate.issue_tick(RESPONSE_TICK), admitted_tick);

    let blocked = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, RESPONSE_TICK)
        .unwrap();
    assert_eq!(blocked.issued_rows(), 0);
    assert_eq!(blocked.next_service_tick(), Some(admitted_tick));
    assert!(fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .any(|record| {
            record.sequence() == fixture.consumer_sequence
                && record.service_tick() == RESPONSE_TICK
                && record.action() == O3LiveIssueTraceAction::RetainedDependency
                && record.next_wake_tick() == Some(admitted_tick)
        }));
    assert!(!fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .any(|record| {
            record.sequence() == fixture.consumer_sequence
                && record.service_tick() <= RESPONSE_TICK
                && record.action() == O3LiveIssueTraceAction::Selected
        }));
}

#[test]
fn fp_load_queue_wakes_exactly_at_admitted_memory_result_writeback() {
    let mut fixture = FpLoadQueueFixture::staged();
    let admitted_tick = fixture.complete();
    let blocked = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, RESPONSE_TICK)
        .unwrap();
    assert_eq!(blocked.next_service_tick(), Some(admitted_tick));

    if admitted_tick > RESPONSE_TICK + 1 {
        let early = fixture
            .runtime
            .service_live_issue_queue_at(&fixture.hart, admitted_tick - 1)
            .unwrap();
        assert_eq!(early.issued_rows(), 0);
    }
    let issued = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, admitted_tick)
        .unwrap();
    assert_eq!(issued.issued_rows(), 1);
    let selected = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .filter(|record| {
            record.sequence() == fixture.consumer_sequence
                && record.action() == O3LiveIssueTraceAction::Selected
        })
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].service_tick(), admitted_tick);
    assert_eq!(
        fixture
            .runtime
            .live_speculative_executions
            .iter()
            .find(|row| row.sequence == fixture.consumer_sequence)
            .expect("FP consumer issued")
            .issue_tick,
        admitted_tick,
    );
}

#[test]
fn fp_load_pending_address_fallback_remains_integer_only() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_window_depths(4, 4));
    let load = float_load_event(12, MemoryWidth::Doubleword);
    assert!(runtime.stage_live_data_access_issue(
        &load,
        request(20),
        20,
        O3DataAccessWindowPolicy::MemoryResultWindow,
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
            20,
        ),
        0,
    );
    assert!(runtime.pending_data_address_sequences_for_test().is_empty());
}
