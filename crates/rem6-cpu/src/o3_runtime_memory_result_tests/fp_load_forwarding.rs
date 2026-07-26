use rem6_isa_riscv::{RiscvFloatRoundingMode, RiscvVectorScalarMoveInstruction};

use super::*;

#[path = "fp_load_forwarding/pairs.rs"]
mod pairs;

fn float_load_event_with_width(
    pc: u64,
    sequence: u64,
    width: MemoryWidth,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::FloatLoad {
        rd: freg(3),
        rs1: reg(10),
        offset: Immediate::new(0),
        width,
    };
    execution_event(
        pc,
        sequence,
        instruction,
        MemoryAccessKind::FloatLoad {
            rd: freg(3),
            address: 0x9000,
            width,
        },
    )
}

fn float_mul_s() -> RiscvInstruction {
    RiscvInstruction::FloatMulS {
        rd: freg(5),
        rs1: freg(3),
        rs2: freg(4),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn float_mul_d() -> RiscvInstruction {
    RiscvInstruction::FloatMulD {
        rd: freg(5),
        rs1: freg(3),
        rs2: freg(4),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn independent_addi() -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(7),
        rs1: reg(0),
        imm: Immediate::new(1),
    }
}

fn stage_result(runtime: &mut O3RuntimeState, event: &RiscvCpuExecutionEvent) -> bool {
    runtime.stage_live_data_access_issue(
        event,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    )
}

#[test]
fn memory_result_runtime_stages_flw_and_fld_consumers_at_typed_dependency_boundary() {
    for (label, width, consumer) in [
        ("flw", MemoryWidth::Word, float_mul_s()),
        ("fld", MemoryWidth::Doubleword, float_mul_d()),
    ] {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let head = float_load_event_with_width(0x8000, 1, width);
        assert!(stage_result(&mut runtime, &head), "{label}");

        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                head.fetch().request_id(),
                [
                    (Address::new(0x8004), consumer),
                    (Address::new(0x8008), independent_addi()),
                ],
            ),
            1,
            "{label}"
        );
    }
}

#[test]
fn memory_depth_one_with_deeper_live_window_stages_fp_consumer() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_window_depths(1, 5));
    let head = float_load_event_with_width(0x8000, 1, MemoryWidth::Word);
    assert!(stage_result(&mut runtime, &head));

    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            head.fetch().request_id(),
            [
                (Address::new(0x8004), float_mul_s()),
                (Address::new(0x8008), independent_addi()),
            ],
        ),
        1
    );
    assert!(
        !runtime.can_stage_memory_result_window(&float_load_event_with_width(
            0x800c,
            2,
            MemoryWidth::Word,
        ))
    );
}

#[test]
fn prefixed_fp_load_consumer_stages_before_response() {
    for (label, width, consumer) in [
        ("flw", MemoryWidth::Word, float_mul_s()),
        ("fld", MemoryWidth::Doubleword, float_mul_d()),
    ] {
        let mut runtime = O3RuntimeState::default();
        assert!(runtime.set_window_depths(1, 3), "{label}");
        let fixed = multiply_instruction(6, 7);
        let fixed_sequence = runtime
            .stage_live_retire_window(Address::new(0x8000), fixed, 0, [])
            .expect("fixed-FU prefix stages");
        assert_eq!(fixed_sequence, 0, "{label}");
        record_fixed_fu_owner(
            &mut runtime,
            fixed_sequence,
            decoded_instruction(fixed),
            0x8000,
            request(1),
            0,
        );

        let load = float_load_event_with_width(0x8004, 2, width);
        assert!(stage_result(&mut runtime, &load), "{label}");
        let load_sequence = runtime.live_data_accesses[0].sequence;
        assert_eq!(load_sequence, fixed_sequence + 1, "{label}");
        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                load.fetch().request_id(),
                [
                    (Address::new(0x8008), consumer),
                    (Address::new(0x800c), independent_addi()),
                ],
            ),
            1,
            "{label}: fixed prefix + load + consumer exactly fill live depth three"
        );

        let live = &runtime.live_data_accesses[0];
        assert_eq!(live.outcome, O3LiveDataAccessOutcome::Resident, "{label}");
        assert_eq!(live.response_tick, None, "{label}");
        assert_eq!(live.memory_result, None, "{label}");
        assert_eq!(
            runtime.writeback_reservation(load_sequence),
            None,
            "{label}"
        );
        assert_eq!(
            runtime
                .snapshot()
                .reorder_buffer()
                .iter()
                .map(|entry| (entry.sequence(), entry.pc()))
                .collect::<Vec<_>>(),
            vec![
                (fixed_sequence, Address::new(0x8000)),
                (load_sequence, Address::new(0x8004)),
                (load_sequence + 1, Address::new(0x8008)),
            ],
            "{label}: fixed < load < consumer and the trailing row is excluded"
        );
        assert!(
            runtime.snapshot().reorder_buffer()[2].is_live_staged(),
            "{label}"
        );
    }
}

#[test]
fn fp_load_vector_destination_remains_unforwardable() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let head = vector_unit_event(0x8000, 1, 0x9000, None);
    assert!(stage_result(&mut runtime, &head));
    let sequence = runtime.live_data_accesses[0].sequence;

    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            head.fetch().request_id(),
            [(
                Address::new(0x8004),
                RiscvInstruction::VectorScalarMove(
                    RiscvVectorScalarMoveInstruction::MoveToScalar {
                        rd: reg(7),
                        vs2: vreg(2),
                    },
                )
            )],
        ),
        0
    );

    let mut completed = head;
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(&completed, request(20), 41, 10, Some(&[0x11; 16]),)
        .unwrap());
    assert!(runtime.writeback_reservation(sequence).is_some());
    assert_eq!(
        runtime.live_issue_source_value(sequence, O3ArchitecturalRegister::vector(vreg(2)),),
        None,
        "vector memory results remain outside scalar typed forwarding"
    );
}

fn decoded_float_mul(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    let RiscvInstruction::FloatMulD { rd, rs1, rs2, .. } = instruction else {
        panic!("expected double-precision multiply")
    };
    let raw = (0x09_u32 << 25)
        | (u32::from(rs2.index()) << 20)
        | (u32::from(rs1.index()) << 15)
        | (u32::from(rd.index()) << 7)
        | 0x53;
    RiscvInstruction::decode_with_length(raw).expect("FMUL.D decodes")
}

fn complete_result(
    runtime: &mut O3RuntimeState,
    event: &RiscvCpuExecutionEvent,
    request_sequence: u64,
    response_tick: u64,
    data: &[u8],
) {
    let mut completed = event.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            request(request_sequence),
            response_tick,
            10,
            Some(data),
        )
        .unwrap());
}

#[test]
fn fp_load_wrong_class_waw_never_satisfies_consumer() {
    for (label, younger_is_fp, expected_producer_index) in [
        ("wrong-class same index", false, 0_usize),
        ("typed WAW", true, 1),
    ] {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let older_fp = float_load_event_with_width(0x8000, 1, MemoryWidth::Doubleword);
        assert!(stage_result(&mut runtime, &older_fp), "{label}");
        let mut result_events = vec![(older_fp, 20, 3.0_f64.to_bits().to_le_bytes().to_vec())];
        if younger_is_fp {
            let younger_fp = float_load_event_with_width(0x8004, 2, MemoryWidth::Doubleword);
            assert!(runtime.stage_live_data_access_issue(
                &younger_fp,
                request(21),
                32,
                O3DataAccessWindowPolicy::MemoryResultWindow,
            ));
            result_events.push((younger_fp, 21, 5.0_f64.to_bits().to_le_bytes().to_vec()));
        } else {
            let same_index_integer = load_event(0x8004, 2, 3);
            assert!(runtime.stage_live_data_access_issue(
                &same_index_integer,
                request(21),
                32,
                O3DataAccessWindowPolicy::MemoryResultWindow,
            ));
            result_events.push((same_index_integer, 21, 7_u32.to_le_bytes().to_vec()));
        }
        let producer_sequences = runtime
            .live_data_accesses
            .iter()
            .map(|live| live.sequence)
            .collect::<Vec<_>>();
        let consumer_pc = 0x8008;
        let consumer = float_mul_d();
        let tail = runtime.live_data_accesses.last().unwrap();
        assert_eq!(
            runtime.stage_live_data_access_younger_window(
                tail.fetch_request,
                [(Address::new(consumer_pc), consumer)],
            ),
            1,
            "{label}"
        );
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(consumer_pc),
            decoded_float_mul(consumer),
            &[request(30)],
            34,
        ));

        for (index, (event, request_sequence, data)) in result_events.iter().enumerate() {
            complete_result(
                &mut runtime,
                event,
                *request_sequence,
                41 + index as u64,
                data,
            );
        }
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(consumer_pc), consumer)
            .expect("typed FP consumer materializes after its producer response");
        assert_eq!(
            candidate.producer_sequences(),
            &[producer_sequences[expected_producer_index]],
            "{label}: only the nearest floating-point destination may satisfy f3"
        );
        if younger_is_fp {
            assert!(
                !candidate
                    .producer_sequences()
                    .contains(&producer_sequences[0]),
                "{label}: the older f3 is hidden by the younger typed WAW"
            );
        } else {
            assert!(
                !candidate
                    .producer_sequences()
                    .contains(&producer_sequences[1]),
                "{label}: integer x3 never aliases floating-point f3"
            );
        }
    }
}
