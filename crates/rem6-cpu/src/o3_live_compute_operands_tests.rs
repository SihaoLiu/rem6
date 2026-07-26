use crate::o3_dependency::O3RegisterClass;
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatRoundingMode, RiscvInstruction, RiscvVectorMaskMode,
    RiscvVectorMaskReductionInstruction, RiscvVectorScalarMoveInstruction, VectorRegister,
};

use super::*;

macro_rules! fp2 {
    ($variant:ident, $rd:expr, $rs1:expr, $rs2:expr) => {
        RiscvInstruction::$variant {
            rd: f($rd),
            rs1: f($rs1),
            rs2: f($rs2),
            rounding_mode: rm(),
        }
    };
}

macro_rules! fp3 {
    ($variant:ident, $rd:expr, $rs1:expr, $rs2:expr, $rs3:expr) => {
        RiscvInstruction::$variant {
            rd: f($rd),
            rs1: f($rs1),
            rs2: f($rs2),
            rs3: f($rs3),
            rounding_mode: rm(),
        }
    };
}

macro_rules! fcmp {
    ($variant:ident) => {
        RiscvInstruction::$variant {
            rd: r(1),
            rs1: f(2),
            rs2: f(3),
        }
    };
}

macro_rules! reject {
    ($($instruction:expr),+ $(,)?) => {
        $(assert_eq!(o3_live_compute_operands($instruction), None);)+
    };
}

fn r(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn v(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}

fn ireg(index: u8) -> O3ArchitecturalRegister {
    O3ArchitecturalRegister::integer(r(index))
}

fn freg(index: u8) -> O3ArchitecturalRegister {
    O3ArchitecturalRegister::floating_point(f(index))
}

fn vreg(index: u8) -> O3ArchitecturalRegister {
    O3ArchitecturalRegister::vector(v(index))
}

fn rm() -> RiscvFloatRoundingMode {
    RiscvFloatRoundingMode::RoundNearestEven
}

fn expect_operands(
    instruction: RiscvInstruction,
    class: O3LiveComputeClass,
    destination: O3ArchitecturalRegister,
    sources: &[O3ArchitecturalRegister],
) {
    let operands: O3LiveComputeOperands = o3_live_compute_operands(instruction).unwrap();
    assert_eq!(operands.class(), class);
    assert_eq!(operands.destination(), destination);
    assert_eq!(operands.sources(), sources);
}

#[test]
fn live_compute_operands_register_wrappers_expose_class_and_architectural_index() {
    for (register, register_class, architectural) in [
        (ireg(5), O3RegisterClass::Integer, 5),
        (freg(6), O3RegisterClass::FloatingPoint, 6),
        (vreg(7), O3RegisterClass::Vector, 7),
    ] {
        assert_eq!(register.register_class(), register_class);
        assert_eq!(register.architectural(), architectural);
    }
}

#[test]
fn integer_typed_identity_converts_only_to_integer_register() {
    let identity = ireg(5);

    assert_eq!(identity.integer_register(), Some(r(5)));
    assert_eq!(identity.float_register(), None);
}

#[test]
fn floating_point_typed_identity_converts_only_to_float_register() {
    let identity = freg(6);

    assert_eq!(identity.float_register(), Some(f(6)));
    assert_eq!(identity.integer_register(), None);
}

#[test]
fn live_compute_operands_classifies_supported_scalar_integer_fp_and_vector_results() {
    let cases = [
        (
            RiscvInstruction::Add {
                rd: r(1),
                rs1: r(2),
                rs2: r(3),
            },
            O3LiveComputeClass::ScalarInteger,
            ireg(1),
            vec![ireg(2), ireg(3)],
        ),
        (
            fp2!(FloatAddS, 4, 5, 6),
            O3LiveComputeClass::ScalarFloat,
            freg(4),
            vec![freg(5), freg(6)],
        ),
        (
            fp2!(FloatSubS, 7, 8, 9),
            O3LiveComputeClass::ScalarFloat,
            freg(7),
            vec![freg(8), freg(9)],
        ),
        (
            fp2!(FloatMulS, 10, 11, 12),
            O3LiveComputeClass::ScalarFloat,
            freg(10),
            vec![freg(11), freg(12)],
        ),
        (
            fp3!(FloatMultiplyAddS, 13, 14, 15, 16),
            O3LiveComputeClass::ScalarFloat,
            freg(13),
            vec![freg(14), freg(15), freg(16)],
        ),
        (
            fp3!(FloatMultiplySubtractS, 17, 18, 19, 20),
            O3LiveComputeClass::ScalarFloat,
            freg(17),
            vec![freg(18), freg(19), freg(20)],
        ),
        (
            fp3!(FloatNegativeMultiplySubtractS, 21, 22, 23, 24),
            O3LiveComputeClass::ScalarFloat,
            freg(21),
            vec![freg(22), freg(23), freg(24)],
        ),
        (
            fp3!(FloatNegativeMultiplyAddS, 25, 26, 27, 28),
            O3LiveComputeClass::ScalarFloat,
            freg(25),
            vec![freg(26), freg(27), freg(28)],
        ),
        (
            fp2!(FloatDivS, 29, 30, 31),
            O3LiveComputeClass::ScalarFloat,
            freg(29),
            vec![freg(30), freg(31)],
        ),
        (
            RiscvInstruction::FloatSqrtS {
                rd: f(2),
                rs1: f(3),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(2),
            vec![freg(3)],
        ),
        (
            RiscvInstruction::VectorScalarMove(RiscvVectorScalarMoveInstruction::MoveToScalar {
                rd: r(8),
                vs2: v(9),
            }),
            O3LiveComputeClass::VectorToScalar,
            ireg(8),
            vec![vreg(9)],
        ),
        (
            RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::PopCount {
                rd: r(10),
                vs2: v(11),
                mask: RiscvVectorMaskMode::Unmasked,
            }),
            O3LiveComputeClass::VectorToScalar,
            ireg(10),
            vec![vreg(11)],
        ),
        (
            RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::FirstSet {
                rd: r(12),
                vs2: v(13),
                mask: RiscvVectorMaskMode::Unmasked,
            }),
            O3LiveComputeClass::VectorToScalar,
            ireg(12),
            vec![vreg(13)],
        ),
    ];

    for (instruction, class, destination, sources) in cases {
        expect_operands(instruction, class, destination, &sources);
    }
}

#[test]
fn live_compute_operands_deduplicates_sources_without_reordering() {
    expect_operands(
        RiscvInstruction::Add {
            rd: r(1),
            rs1: r(2),
            rs2: r(2),
        },
        O3LiveComputeClass::ScalarInteger,
        ireg(1),
        &[ireg(2)],
    );
    expect_operands(
        fp3!(FloatMultiplyAddS, 1, 2, 3, 2),
        O3LiveComputeClass::ScalarFloat,
        freg(1),
        &[freg(2), freg(3)],
    );
}

#[test]
#[rustfmt::skip]
fn live_compute_operands_rejects_unsupported_fp_vector_and_system_families() {
    reject!(
        fp2!(FloatAddD, 1, 2, 3), fp2!(FloatSubD, 1, 2, 3),
        fp2!(FloatMulD, 1, 2, 3), fp2!(FloatDivD, 1, 2, 3),
        fp3!(FloatMultiplyAddD, 1, 2, 3, 4),
        fp3!(FloatMultiplySubtractD, 1, 2, 3, 4),
        fp3!(FloatNegativeMultiplySubtractD, 1, 2, 3, 4),
        fp3!(FloatNegativeMultiplyAddD, 1, 2, 3, 4),
        RiscvInstruction::FloatSqrtD { rd: f(1), rs1: f(2), rounding_mode: rm() },
        fcmp!(FloatLessOrEqualS), fcmp!(FloatLessThanS),
        fcmp!(FloatEqualS), fcmp!(FloatEqualD),
        RiscvInstruction::FloatConvertSFromW { rd: f(1), rs1: r(2), rounding_mode: rm() },
        RiscvInstruction::FloatConvertDFromL { rd: f(1), rs1: r(2), rounding_mode: rm() },
        RiscvInstruction::FloatConvertWFromS { rd: r(1), rs1: f(2), rounding_mode: rm() },
        RiscvInstruction::FloatConvertLuFromD { rd: r(1), rs1: f(2), rounding_mode: rm() },
        RiscvInstruction::FloatConvertSFromD { rd: f(1), rs1: f(2), rounding_mode: rm() },
        RiscvInstruction::FloatConvertDFromS { rd: f(1), rs1: f(2) },
        RiscvInstruction::FloatMoveXFromS { rd: r(1), rs1: f(2) },
        RiscvInstruction::FloatMoveSFromX { rd: f(1), rs1: r(2) },
        RiscvInstruction::FloatMoveXFromD { rd: r(1), rs1: f(2) },
        RiscvInstruction::FloatMoveDFromX { rd: f(1), rs1: r(2) },
        RiscvInstruction::FloatSignInjectS { rd: f(1), rs1: f(2), rs2: f(3) },
        RiscvInstruction::FloatSignInjectNegD { rd: f(1), rs1: f(2), rs2: f(3) },
        RiscvInstruction::FloatSignInjectXorS { rd: f(1), rs1: f(2), rs2: f(3) },
        RiscvInstruction::FloatMinS { rd: f(1), rs1: f(2), rs2: f(3) },
        RiscvInstruction::FloatMaxD { rd: f(1), rs1: f(2), rs2: f(3) },
        RiscvInstruction::FloatClassS { rd: r(1), rs1: f(2) },
        RiscvInstruction::FloatClassD { rd: r(1), rs1: f(2) },
        RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::PopCount {
            rd: r(1),
            vs2: v(2),
            mask: RiscvVectorMaskMode::Masked,
        }),
        RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::FirstSet {
            rd: r(1),
            vs2: v(2),
            mask: RiscvVectorMaskMode::Masked,
        }),
        RiscvInstruction::VectorAddVv {
            vd: v(1),
            vs1: v(2),
            vs2: v(3),
            mask: RiscvVectorMaskMode::Unmasked,
        },
        RiscvInstruction::VectorScalarMove(RiscvVectorScalarMoveInstruction::MoveFromScalar {
            vd: v(1),
            rs1: r(2),
        }),
        RiscvInstruction::Ecall,
        RiscvInstruction::Ebreak,
    );
}
