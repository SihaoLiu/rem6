use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, RiscvExecutionRecord, RiscvFloatRoundingMode,
    RiscvFloatStatus,
};

use super::*;

const RESPONSE_TICK: u64 = 41;

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn boxed_single(value: f32) -> u64 {
    0xffff_ffff_0000_0000 | u64::from(value.to_bits())
}

fn float_load_event(rd: u8) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::FloatLoad {
        rd: f(rd),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
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
                width: MemoryWidth::Word,
            }),
        ),
    )
}

fn float_add_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatAddS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
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
        RiscvInstruction::FloatAddS { rd, rs1, rs2, .. } => {
            fp_raw(0x00, rs2.index(), rs1.index(), rd.index())
        }
        RiscvInstruction::FloatMulS { rd, rs1, rs2, .. } => {
            fp_raw(0x08, rs2.index(), rs1.index(), rd.index())
        }
        _ => panic!("unsupported FP-load service instruction: {instruction:?}"),
    };
    RiscvInstruction::decode_with_length(raw).unwrap()
}

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25) | (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (u32::from(rd) << 7) | 0x53
}

fn stage_fp_load_window(
    younger: &[(u64, RiscvInstruction, u64)],
) -> (O3RuntimeState, RiscvCpuExecutionEvent, u64, Vec<u64>) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(runtime.set_issue_width(4));
    let load = float_load_event(4);
    assert!(runtime.stage_live_data_access_issue(
        &load,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    let load_sequence = runtime.live_data_accesses[0].sequence;
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            younger
                .iter()
                .map(|(pc, instruction, _)| (Address::new(*pc), *instruction)),
        ),
        younger.len(),
    );
    let mut sequences = Vec::with_capacity(younger.len());
    for (pc, instruction, request_sequence) in younger {
        let sequence = runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.pc() == Address::new(*pc))
            .expect("FP service row staged")
            .sequence();
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(*pc),
            fp_decoded(*instruction),
            &[request(*request_sequence)],
            RESPONSE_TICK,
        ));
        sequences.push(sequence);
    }
    (runtime, load, load_sequence, sequences)
}

fn complete_load(runtime: &mut O3RuntimeState, load: &RiscvCpuExecutionEvent) -> u64 {
    let sequence = runtime.live_data_accesses[0].sequence;
    let mut completed = load.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            request(20),
            RESPONSE_TICK,
            10,
            Some(&2.0f32.to_bits().to_le_bytes()),
        )
        .unwrap());
    runtime
        .writeback_reservation(sequence)
        .expect("FP load writeback reservation")
        .admitted_tick()
}

#[test]
fn fp_load_service_uses_clone_without_mutating_canonical_hart() {
    let consumer = float_mul_s(5, 4, 3);
    let (mut runtime, load, _, sequences) = stage_fp_load_window(&[(BRANCH_PC, consumer, 21)]);
    let consumer_sequence = sequences[0];
    let admitted_tick = complete_load(&mut runtime, &load);
    let mut hart = RiscvHartState::new(BRANCH_PC);
    hart.write_float(f(3), boxed_single(3.0));
    hart.write_float(f(4), boxed_single(99.0));
    hart.write_float(f(5), boxed_single(77.0));
    hart.set_float_status(RiscvFloatStatus::new(0).with_frm(3));
    let canonical_values = [
        hart.read_float(f(3)),
        hart.read_float(f(4)),
        hart.read_float(f(5)),
    ];
    let canonical_status = hart.float_status();

    let blocked = runtime
        .service_live_issue_queue_at(&hart, RESPONSE_TICK)
        .unwrap();
    assert_eq!(blocked.issued_rows(), 0);
    assert_eq!(blocked.next_service_tick(), Some(admitted_tick));
    let issued = runtime
        .service_live_issue_queue_at(&hart, admitted_tick)
        .unwrap();
    assert_eq!(issued.issued_rows(), 1);
    let execution = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == consumer_sequence)
        .expect("FP load consumer execution");
    assert_eq!(
        execution.execution.float_register_writes(),
        &[FloatRegisterWrite::new(f(5), boxed_single(6.0))],
    );
    assert_eq!(
        [
            hart.read_float(f(3)),
            hart.read_float(f(4)),
            hart.read_float(f(5))
        ],
        canonical_values,
    );
    assert_eq!(hart.float_status(), canonical_status);
}

#[test]
fn fp_load_service_handles_nearest_fp_waw_and_two_producer_fan_in() {
    let nearest = float_add_s(4, 1, 2);
    let peer = float_add_s(6, 2, 3);
    let consumer = float_mul_s(5, 4, 6);
    let younger = [
        (BRANCH_PC, nearest, 21),
        (SECOND_PC, peer, 22),
        (THIRD_PC, consumer, 23),
    ];
    let (mut runtime, load, load_sequence, sequences) = stage_fp_load_window(&younger);
    let [nearest_sequence, peer_sequence, consumer_sequence] = sequences.as_slice() else {
        unreachable!()
    };
    let queue =
        match O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences())
            .unwrap()
        {
            O3LiveIssueQueueCapture::Ready(queue) => queue,
            O3LiveIssueQueueCapture::ReplayPending(sequence) => {
                panic!("unexpected FP replay boundary {sequence}")
            }
        };
    let producers = queue
        .entry(*consumer_sequence)
        .expect("fan-in consumer queue row")
        .scheduling()
        .data_producers()
        .iter()
        .map(|producer| producer.sequence())
        .collect::<Vec<_>>();
    assert_eq!(producers, [*nearest_sequence, *peer_sequence]);
    assert!(!producers.contains(&load_sequence));

    complete_load(&mut runtime, &load);
    let mut hart = RiscvHartState::new(BRANCH_PC);
    hart.write_float(f(1), boxed_single(1.0));
    hart.write_float(f(2), boxed_single(2.0));
    hart.write_float(f(3), boxed_single(3.0));
    hart.write_float(f(4), boxed_single(99.0));
    let canonical_f4 = hart.read_float(f(4));

    let mut service_tick = RESPONSE_TICK;
    for _ in 0..4 {
        let outcome = runtime
            .service_live_issue_queue_at(&hart, service_tick)
            .unwrap();
        if runtime.live_issue.resident_sequences().is_empty() {
            break;
        }
        service_tick = outcome
            .next_service_tick()
            .expect("resident FP row advertises its next service tick");
    }
    assert!(runtime.live_issue.resident_sequences().is_empty());
    let consumer_execution = runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == *consumer_sequence)
        .expect("two-producer FP consumer issued");
    assert_eq!(
        consumer_execution.producer_sequences,
        [*nearest_sequence, *peer_sequence],
    );
    assert_eq!(
        consumer_execution.execution.float_register_writes(),
        &[FloatRegisterWrite::new(f(5), boxed_single(15.0))],
    );
    assert_eq!(hart.read_float(f(4)), canonical_f4);
}
