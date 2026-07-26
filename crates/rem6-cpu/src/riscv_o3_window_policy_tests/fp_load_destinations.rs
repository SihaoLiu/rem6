use rem6_isa_riscv::{
    FloatRegister, Immediate, Register, RiscvFloatRoundingMode, RiscvInstruction,
    RiscvVectorScalarMoveInstruction, VectorRegister,
};

use super::*;

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn r(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn v(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn float_mul_s(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatMulS {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn float_mul_d(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::FloatMulD {
        rd: f(rd),
        rs1: f(rs1),
        rs2: f(rs2),
        rounding_mode: RiscvFloatRoundingMode::RoundNearestEven,
    }
}

fn addi(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: r(rd),
        rs1: r(rs1),
        imm: Immediate::new(1),
    }
}

fn memory_result_window(destination: O3ArchitecturalRegister) -> RiscvScalarIntegerLiveWindow {
    RiscvScalarIntegerLiveWindow::new(vec![destination], 1, 4, 4, false)
}

#[test]
fn memory_result_window_blocks_matching_flw_and_fld_sources() {
    for (label, consumer) in [("flw", float_mul_s(5, 3, 4)), ("fld", float_mul_d(5, 3, 4))] {
        let mut window = memory_result_window(O3ArchitecturalRegister::floating_point(f(3)));

        assert_eq!(
            window.classify_younger(consumer),
            RiscvScalarIntegerYoungerDecision::AdmitStop,
            "{label}"
        );
    }
}

#[test]
fn memory_result_window_keeps_integer_fp_and_vector_destinations_class_distinct() {
    let mut integer = memory_result_window(O3ArchitecturalRegister::integer(r(4)));
    assert_eq!(
        integer.classify_younger(float_mul_s(5, 4, 3)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );

    let mut floating_point = memory_result_window(O3ArchitecturalRegister::floating_point(f(4)));
    assert_eq!(
        floating_point.classify_younger(addi(5, 4)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );

    let mut vector = memory_result_window(O3ArchitecturalRegister::vector(v(2)));
    assert_eq!(
        vector.classify_younger(RiscvInstruction::VectorScalarMove(
            RiscvVectorScalarMoveInstruction::MoveToScalar {
                rd: r(5),
                vs2: v(2),
            },
        )),
        RiscvScalarIntegerYoungerDecision::Reject
    );

    let mut floating_point_zero =
        memory_result_window(O3ArchitecturalRegister::floating_point(f(0)));
    assert_eq!(
        floating_point_zero.classify_younger(float_mul_s(5, 0, 3)),
        RiscvScalarIntegerYoungerDecision::AdmitStop
    );
}

#[test]
fn memory_result_window_from_class_index_validates_supported_classes_and_bounds() {
    for register_class in [
        O3RegisterClass::Integer,
        O3RegisterClass::FloatingPoint,
        O3RegisterClass::Vector,
    ] {
        for architectural in [0, 31] {
            let register = O3ArchitecturalRegister::from_class_index(register_class, architectural)
                .expect("supported architectural register");
            assert_eq!(register.register_class(), register_class);
            assert_eq!(register.architectural(), architectural);
        }
        for architectural in [32, u32::MAX] {
            assert_eq!(
                O3ArchitecturalRegister::from_class_index(register_class, architectural),
                None
            );
        }
    }

    for register_class in [O3RegisterClass::ConditionCode, O3RegisterClass::Misc] {
        for architectural in [0, 31, 32, u32::MAX] {
            assert_eq!(
                O3ArchitecturalRegister::from_class_index(register_class, architectural),
                None
            );
        }
    }
}
