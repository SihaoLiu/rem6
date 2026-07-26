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
fn memory_result_runtime_keeps_vector_load_consumer_outside_forwardable_lane() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let head = vector_unit_event(0x8000, 1, 0x9000, None);
    assert!(stage_result(&mut runtime, &head));

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
}
