use crate::o3_dependency::O3RegisterClass;
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvFloatRoundingMode, RiscvInstruction, RiscvVectorMaskMode,
    RiscvVectorMaskReductionInstruction, RiscvVectorScalarMoveInstruction, VectorRegister,
};

use super::*;

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

fn assert_operands(
    name: &str,
    instruction: RiscvInstruction,
    class: O3LiveComputeClass,
    destination: O3ArchitecturalRegister,
    sources: &[O3ArchitecturalRegister],
) {
    let operands: O3LiveComputeOperands = o3_live_compute_operands(instruction)
        .unwrap_or_else(|| panic!("expected live-compute operands for {name}"));
    assert_eq!(operands.class(), class, "{name}: class");
    assert_eq!(operands.destination(), destination, "{name}: destination");
    assert_eq!(operands.sources(), sources, "{name}: sources");
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
fn live_compute_operands_classifies_supported_scalar_integer_fp_and_vector_results() {
    let cases = [
        (
            "ScalarInteger Add",
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
            "FloatAddS",
            RiscvInstruction::FloatAddS {
                rd: f(4),
                rs1: f(5),
                rs2: f(6),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(4),
            vec![freg(5), freg(6)],
        ),
        (
            "FloatSubS",
            RiscvInstruction::FloatSubS {
                rd: f(7),
                rs1: f(8),
                rs2: f(9),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(7),
            vec![freg(8), freg(9)],
        ),
        (
            "FloatMulS",
            RiscvInstruction::FloatMulS {
                rd: f(10),
                rs1: f(11),
                rs2: f(12),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(10),
            vec![freg(11), freg(12)],
        ),
        (
            "FloatMultiplyAddS",
            RiscvInstruction::FloatMultiplyAddS {
                rd: f(13),
                rs1: f(14),
                rs2: f(15),
                rs3: f(16),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(13),
            vec![freg(14), freg(15), freg(16)],
        ),
        (
            "FloatMultiplySubtractS",
            RiscvInstruction::FloatMultiplySubtractS {
                rd: f(17),
                rs1: f(18),
                rs2: f(19),
                rs3: f(20),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(17),
            vec![freg(18), freg(19), freg(20)],
        ),
        (
            "FloatNegativeMultiplySubtractS",
            RiscvInstruction::FloatNegativeMultiplySubtractS {
                rd: f(21),
                rs1: f(22),
                rs2: f(23),
                rs3: f(24),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(21),
            vec![freg(22), freg(23), freg(24)],
        ),
        (
            "FloatNegativeMultiplyAddS",
            RiscvInstruction::FloatNegativeMultiplyAddS {
                rd: f(25),
                rs1: f(26),
                rs2: f(27),
                rs3: f(28),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(25),
            vec![freg(26), freg(27), freg(28)],
        ),
        (
            "FloatDivS",
            RiscvInstruction::FloatDivS {
                rd: f(29),
                rs1: f(30),
                rs2: f(31),
                rounding_mode: rm(),
            },
            O3LiveComputeClass::ScalarFloat,
            freg(29),
            vec![freg(30), freg(31)],
        ),
        (
            "FloatSqrtS",
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
            "VectorScalarMove::MoveToScalar",
            RiscvInstruction::VectorScalarMove(RiscvVectorScalarMoveInstruction::MoveToScalar {
                rd: r(8),
                vs2: v(9),
            }),
            O3LiveComputeClass::VectorToScalar,
            ireg(8),
            vec![vreg(9)],
        ),
        (
            "VectorMaskReduction::PopCount Unmasked",
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
            "VectorMaskReduction::FirstSet Unmasked",
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

    for (name, instruction, class, destination, sources) in cases {
        assert_operands(name, instruction, class, destination, &sources);
    }
}

#[test]
fn live_compute_operands_deduplicates_sources_without_reordering() {
    assert_operands(
        "scalar duplicate",
        RiscvInstruction::Add {
            rd: r(1),
            rs1: r(2),
            rs2: r(2),
        },
        O3LiveComputeClass::ScalarInteger,
        ireg(1),
        &[ireg(2)],
    );
    assert_operands(
        "float duplicate",
        RiscvInstruction::FloatMultiplyAddS {
            rd: f(1),
            rs1: f(2),
            rs2: f(3),
            rs3: f(2),
            rounding_mode: rm(),
        },
        O3LiveComputeClass::ScalarFloat,
        freg(1),
        &[freg(2), freg(3)],
    );
}

#[test]
fn live_compute_operands_rejects_unsupported_fp_vector_and_system_families() {
    let negatives = [
        RiscvInstruction::FloatAddD {
            rd: f(1),
            rs1: f(2),
            rs2: f(3),
            rounding_mode: rm(),
        },
        RiscvInstruction::FloatMultiplyAddD {
            rd: f(1),
            rs1: f(2),
            rs2: f(3),
            rs3: f(4),
            rounding_mode: rm(),
        },
        RiscvInstruction::FloatSqrtD {
            rd: f(1),
            rs1: f(2),
            rounding_mode: rm(),
        },
        RiscvInstruction::FloatLessThanS {
            rd: r(1),
            rs1: f(2),
            rs2: f(3),
        },
        RiscvInstruction::FloatConvertWFromS {
            rd: r(1),
            rs1: f(2),
            rounding_mode: rm(),
        },
        RiscvInstruction::FloatSignInjectS {
            rd: f(1),
            rs1: f(2),
            rs2: f(3),
        },
        RiscvInstruction::FloatClassS {
            rd: r(1),
            rs1: f(2),
        },
        RiscvInstruction::VectorMaskReduction(RiscvVectorMaskReductionInstruction::PopCount {
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
    ];

    for instruction in negatives {
        assert_eq!(o3_live_compute_operands(instruction), None);
    }
}
