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
        RiscvInstruction::FloatAddS { .. }
        | RiscvInstruction::FloatAddD { .. }
        | RiscvInstruction::FloatSubS { .. }
        | RiscvInstruction::FloatSubD { .. } => Some(O3RuntimeFuLatencyClass::ScalarFloatAdd),
        RiscvInstruction::FloatMinS { .. }
        | RiscvInstruction::FloatMinD { .. }
        | RiscvInstruction::FloatMaxS { .. }
        | RiscvInstruction::FloatMaxD { .. }
        | RiscvInstruction::FloatLessOrEqualS { .. }
        | RiscvInstruction::FloatLessOrEqualD { .. }
        | RiscvInstruction::FloatLessThanS { .. }
        | RiscvInstruction::FloatLessThanD { .. }
        | RiscvInstruction::FloatEqualS { .. }
        | RiscvInstruction::FloatEqualD { .. } => Some(O3RuntimeFuLatencyClass::ScalarFloatCompare),
        RiscvInstruction::FloatMoveXFromS { .. }
        | RiscvInstruction::FloatMoveXFromD { .. }
        | RiscvInstruction::FloatMoveSFromX { .. }
        | RiscvInstruction::FloatMoveDFromX { .. }
        | RiscvInstruction::FloatConvertSFromW { .. }
        | RiscvInstruction::FloatConvertSFromWu { .. }
        | RiscvInstruction::FloatConvertSFromL { .. }
        | RiscvInstruction::FloatConvertSFromLu { .. }
        | RiscvInstruction::FloatConvertWFromS { .. }
        | RiscvInstruction::FloatConvertWuFromS { .. }
        | RiscvInstruction::FloatConvertLFromS { .. }
        | RiscvInstruction::FloatConvertLuFromS { .. }
        | RiscvInstruction::FloatConvertSFromD { .. }
        | RiscvInstruction::FloatConvertDFromS { .. }
        | RiscvInstruction::FloatConvertDFromW { .. }
        | RiscvInstruction::FloatConvertDFromWu { .. }
        | RiscvInstruction::FloatConvertDFromL { .. }
        | RiscvInstruction::FloatConvertDFromLu { .. }
        | RiscvInstruction::FloatConvertWFromD { .. }
        | RiscvInstruction::FloatConvertWuFromD { .. }
        | RiscvInstruction::FloatConvertLFromD { .. }
        | RiscvInstruction::FloatConvertLuFromD { .. }
        | RiscvInstruction::FloatSignInjectS { .. }
        | RiscvInstruction::FloatSignInjectD { .. }
        | RiscvInstruction::FloatSignInjectNegS { .. }
        | RiscvInstruction::FloatSignInjectNegD { .. }
        | RiscvInstruction::FloatSignInjectXorS { .. }
        | RiscvInstruction::FloatSignInjectXorD { .. }
        | RiscvInstruction::FloatClassS { .. }
        | RiscvInstruction::FloatClassD { .. } => Some(O3RuntimeFuLatencyClass::ScalarFloatMisc),
        RiscvInstruction::FloatMulS { .. } | RiscvInstruction::FloatMulD { .. } => {
            Some(O3RuntimeFuLatencyClass::ScalarFloatMul)
        }
        RiscvInstruction::FloatMultiplyAddS { .. }
        | RiscvInstruction::FloatMultiplyAddD { .. }
        | RiscvInstruction::FloatMultiplySubtractS { .. }
        | RiscvInstruction::FloatMultiplySubtractD { .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractS { .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractD { .. }
        | RiscvInstruction::FloatNegativeMultiplyAddS { .. }
        | RiscvInstruction::FloatNegativeMultiplyAddD { .. } => {
            Some(O3RuntimeFuLatencyClass::ScalarFloatFma)
        }
        RiscvInstruction::FloatDivS { .. } | RiscvInstruction::FloatDivD { .. } => {
            Some(O3RuntimeFuLatencyClass::ScalarFloatDiv)
        }
        RiscvInstruction::FloatSqrtS { .. } | RiscvInstruction::FloatSqrtD { .. } => {
            Some(O3RuntimeFuLatencyClass::ScalarFloatSqrt)
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
            RiscvVectorFloatInstruction::AddVv { .. }
            | RiscvVectorFloatInstruction::AddVf { .. }
            | RiscvVectorFloatInstruction::SubVv { .. }
            | RiscvVectorFloatInstruction::SubVf { .. }
            | RiscvVectorFloatInstruction::ReverseSubVf { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorFloatAdd),
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::MinVv { .. }
            | RiscvVectorFloatInstruction::MinVf { .. }
            | RiscvVectorFloatInstruction::MaxVv { .. }
            | RiscvVectorFloatInstruction::MaxVf { .. }
            | RiscvVectorFloatInstruction::MaskEqualVv { .. }
            | RiscvVectorFloatInstruction::MaskEqualVf { .. }
            | RiscvVectorFloatInstruction::MaskNotEqualVv { .. }
            | RiscvVectorFloatInstruction::MaskNotEqualVf { .. }
            | RiscvVectorFloatInstruction::MaskLessThanVv { .. }
            | RiscvVectorFloatInstruction::MaskLessThanVf { .. }
            | RiscvVectorFloatInstruction::MaskLessEqualVv { .. }
            | RiscvVectorFloatInstruction::MaskLessEqualVf { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorFloatCompare),
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::ConvertFloatFromUnsignedIntV { .. }
            | RiscvVectorFloatInstruction::ConvertFloatFromSignedIntV { .. }
            | RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatV { .. }
            | RiscvVectorFloatInstruction::ConvertSignedIntFromFloatV { .. }
            | RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatTowardZeroV { .. }
            | RiscvVectorFloatInstruction::ConvertSignedIntFromFloatTowardZeroV { .. }
            | RiscvVectorFloatInstruction::MergeVf { .. }
            | RiscvVectorFloatInstruction::MoveVf { .. }
            | RiscvVectorFloatInstruction::MoveFv { .. }
            | RiscvVectorFloatInstruction::MoveSv { .. }
            | RiscvVectorFloatInstruction::SignInjectVv { .. }
            | RiscvVectorFloatInstruction::SignInjectVf { .. }
            | RiscvVectorFloatInstruction::SignInjectNegVv { .. }
            | RiscvVectorFloatInstruction::SignInjectNegVf { .. }
            | RiscvVectorFloatInstruction::SignInjectXorVv { .. }
            | RiscvVectorFloatInstruction::SignInjectXorVf { .. }
            | RiscvVectorFloatInstruction::ClassV { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorFloatMisc),
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::MulVv { .. } | RiscvVectorFloatInstruction::MulVf { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorFloatMul),
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::MulAddVv { .. }
            | RiscvVectorFloatInstruction::MulAddVf { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorFloatFma),
        RiscvInstruction::VectorFloat(
            RiscvVectorFloatInstruction::DivVv { .. }
            | RiscvVectorFloatInstruction::DivVf { .. }
            | RiscvVectorFloatInstruction::ReverseDivVf { .. },
        ) => Some(O3RuntimeFuLatencyClass::VectorFloatDiv),
        RiscvInstruction::VectorFloat(RiscvVectorFloatInstruction::SqrtV { .. }) => {
            Some(O3RuntimeFuLatencyClass::VectorFloatSqrt)
        }
        _ => None,
    }
}
