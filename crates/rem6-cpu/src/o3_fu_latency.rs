use rem6_isa_riscv::{
    RiscvInstruction, RiscvVectorFloatInstruction, RiscvVectorSaturatingInstruction,
    RiscvVectorWideningIntegerInstruction,
};

use crate::o3_runtime_trace::O3RuntimeFuLatencyClass;

pub(crate) const fn o3_fu_latency_class(
    instruction: RiscvInstruction,
) -> Option<O3RuntimeFuLatencyClass> {
    match instruction {
        RiscvInstruction::Mul { .. }
        | RiscvInstruction::Mulh { .. }
        | RiscvInstruction::Mulhsu { .. }
        | RiscvInstruction::Mulhu { .. }
        | RiscvInstruction::Mulw { .. } => Some(O3RuntimeFuLatencyClass::ScalarIntegerMul),
        RiscvInstruction::Div { .. }
        | RiscvInstruction::Divu { .. }
        | RiscvInstruction::Rem { .. }
        | RiscvInstruction::Remu { .. }
        | RiscvInstruction::Divw { .. }
        | RiscvInstruction::Divuw { .. }
        | RiscvInstruction::Remw { .. }
        | RiscvInstruction::Remuw { .. } => Some(O3RuntimeFuLatencyClass::ScalarIntegerDiv),
        RiscvInstruction::FloatMulS { .. } | RiscvInstruction::FloatMulD { .. } => {
            Some(O3RuntimeFuLatencyClass::ScalarFloatMul)
        }
        RiscvInstruction::FloatDivS { .. } | RiscvInstruction::FloatDivD { .. } => {
            Some(O3RuntimeFuLatencyClass::ScalarFloatDiv)
        }
        RiscvInstruction::VectorMultiplyLowVv { .. }
        | RiscvInstruction::VectorMultiplyLowVx { .. }
        | RiscvInstruction::VectorMultiplyHighUnsignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighUnsignedVx { .. }
        | RiscvInstruction::VectorMultiplyHighSignedUnsignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighSignedUnsignedVx { .. }
        | RiscvInstruction::VectorMultiplyHighSignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighSignedVx { .. }
        | RiscvInstruction::VectorSaturating(
            RiscvVectorSaturatingInstruction::MulSignedFractionalVv { .. }
            | RiscvVectorSaturatingInstruction::MulSignedFractionalVx { .. },
        )
        | RiscvInstruction::VectorIntegerMultiplyAdd(_) => {
            Some(O3RuntimeFuLatencyClass::VectorIntegerMul)
        }
        RiscvInstruction::VectorWideningInteger(
            RiscvVectorWideningIntegerInstruction::MultiplyUnsignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyUnsignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplySignedUnsignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplySignedUnsignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplySignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplySignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddUnsignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddUnsignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddSignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddSignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddUnsignedSignedVx { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddSignedUnsignedVv { .. }
            | RiscvVectorWideningIntegerInstruction::MultiplyAddSignedUnsignedVx { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorIntegerMul),
        RiscvInstruction::VectorDivideUnsignedVv { .. }
        | RiscvInstruction::VectorDivideUnsignedVx { .. }
        | RiscvInstruction::VectorDivideSignedVv { .. }
        | RiscvInstruction::VectorDivideSignedVx { .. }
        | RiscvInstruction::VectorRemainderUnsignedVv { .. }
        | RiscvInstruction::VectorRemainderUnsignedVx { .. }
        | RiscvInstruction::VectorRemainderSignedVv { .. }
        | RiscvInstruction::VectorRemainderSignedVx { .. } => {
            Some(O3RuntimeFuLatencyClass::VectorIntegerDiv)
        }
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::MulVv { .. } | RiscvVectorFloatInstruction::MulVf { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorFloatMul),
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::DivVv { .. }
            | RiscvVectorFloatInstruction::DivVf { .. }
            | RiscvVectorFloatInstruction::ReverseDivVf { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorFloatDiv),
        _ => None,
    }
}
