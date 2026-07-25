use crate::o3_dependency::O3RegisterClass;
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvInstruction, RiscvVectorMaskMode,
    RiscvVectorMaskReductionInstruction, RiscvVectorScalarMoveInstruction, VectorRegister,
};

use super::o3_predicted_scalar_descendant_operands;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3LiveComputeClass {
    ScalarInteger,
    ScalarFloat,
    VectorToScalar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3ArchitecturalRegister {
    register_class: O3RegisterClass,
    architectural: u32,
}

impl O3ArchitecturalRegister {
    pub(crate) const fn integer(register: Register) -> Self {
        Self {
            register_class: O3RegisterClass::Integer,
            architectural: register.index() as u32,
        }
    }

    pub(crate) const fn floating_point(register: FloatRegister) -> Self {
        Self {
            register_class: O3RegisterClass::FloatingPoint,
            architectural: register.index() as u32,
        }
    }

    pub(crate) const fn vector(register: VectorRegister) -> Self {
        Self {
            register_class: O3RegisterClass::Vector,
            architectural: register.index() as u32,
        }
    }

    pub(crate) const fn register_class(self) -> O3RegisterClass {
        self.register_class
    }

    pub(crate) const fn architectural(self) -> u32 {
        self.architectural
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveComputeOperands {
    class: O3LiveComputeClass,
    destination: O3ArchitecturalRegister,
    sources: Vec<O3ArchitecturalRegister>,
}

impl O3LiveComputeOperands {
    fn new<I>(class: O3LiveComputeClass, destination: O3ArchitecturalRegister, sources: I) -> Self
    where
        I: IntoIterator<Item = O3ArchitecturalRegister>,
    {
        let mut deduplicated = Vec::new();
        for source in sources {
            if !deduplicated.contains(&source) {
                deduplicated.push(source);
            }
        }
        Self {
            class,
            destination,
            sources: deduplicated,
        }
    }

    pub(crate) const fn class(&self) -> O3LiveComputeClass {
        self.class
    }

    pub(crate) const fn destination(&self) -> O3ArchitecturalRegister {
        self.destination
    }

    pub(crate) fn sources(&self) -> &[O3ArchitecturalRegister] {
        &self.sources
    }
}

pub(crate) fn o3_live_compute_operands(
    instruction: RiscvInstruction,
) -> Option<O3LiveComputeOperands> {
    if let Some((destination, sources)) = o3_predicted_scalar_descendant_operands(instruction) {
        return Some(O3LiveComputeOperands::new(
            O3LiveComputeClass::ScalarInteger,
            O3ArchitecturalRegister::integer(destination),
            sources.into_iter().map(O3ArchitecturalRegister::integer),
        ));
    }

    match instruction {
        RiscvInstruction::FloatAddS { rd, rs1, rs2, .. }
        | RiscvInstruction::FloatSubS { rd, rs1, rs2, .. }
        | RiscvInstruction::FloatMulS { rd, rs1, rs2, .. }
        | RiscvInstruction::FloatDivS { rd, rs1, rs2, .. } => {
            Some(scalar_float_operands(rd, [rs1, rs2]))
        }
        RiscvInstruction::FloatMultiplyAddS {
            rd, rs1, rs2, rs3, ..
        }
        | RiscvInstruction::FloatMultiplySubtractS {
            rd, rs1, rs2, rs3, ..
        }
        | RiscvInstruction::FloatNegativeMultiplySubtractS {
            rd, rs1, rs2, rs3, ..
        }
        | RiscvInstruction::FloatNegativeMultiplyAddS {
            rd, rs1, rs2, rs3, ..
        } => Some(scalar_float_operands(rd, [rs1, rs2, rs3])),
        RiscvInstruction::FloatSqrtS { rd, rs1, .. } => Some(scalar_float_operands(rd, [rs1])),
        RiscvInstruction::VectorScalarMove(RiscvVectorScalarMoveInstruction::MoveToScalar {
            rd,
            vs2,
        }) => Some(vector_to_scalar_operands(rd, [vs2])),
        RiscvInstruction::VectorMaskReduction(
            RiscvVectorMaskReductionInstruction::PopCount {
                rd,
                vs2,
                mask: RiscvVectorMaskMode::Unmasked,
            }
            | RiscvVectorMaskReductionInstruction::FirstSet {
                rd,
                vs2,
                mask: RiscvVectorMaskMode::Unmasked,
            },
        ) => Some(vector_to_scalar_operands(rd, [vs2])),
        _ => None,
    }
}

fn scalar_float_operands<const N: usize>(
    destination: FloatRegister,
    sources: [FloatRegister; N],
) -> O3LiveComputeOperands {
    O3LiveComputeOperands::new(
        O3LiveComputeClass::ScalarFloat,
        O3ArchitecturalRegister::floating_point(destination),
        sources
            .into_iter()
            .map(O3ArchitecturalRegister::floating_point),
    )
}

fn vector_to_scalar_operands<const N: usize>(
    destination: Register,
    sources: [VectorRegister; N],
) -> O3LiveComputeOperands {
    O3LiveComputeOperands::new(
        O3LiveComputeClass::VectorToScalar,
        O3ArchitecturalRegister::integer(destination),
        sources.into_iter().map(O3ArchitecturalRegister::vector),
    )
}

#[cfg(test)]
#[path = "o3_live_compute_operands_tests.rs"]
mod tests;
