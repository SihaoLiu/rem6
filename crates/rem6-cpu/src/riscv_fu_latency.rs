use rem6_isa_riscv::{
    RiscvInstruction, RiscvVectorFloatInstruction, RiscvVectorMemoryInstruction,
    RiscvVectorSaturatingInstruction, RiscvVectorWideningIntegerInstruction,
};

use crate::o3_runtime_trace::O3RuntimeFuLatencyClass;

const SCALAR_INTEGER_MUL_CYCLES: u64 = 2;
const SCALAR_INTEGER_DIV_CYCLES: u64 = 19;
const VECTOR_INTEGER_SHIFT_CYCLES: u64 = 1;
const VECTOR_LOAD_CYCLES: u64 = 2;
const VECTOR_STORE_CYCLES: u64 = 1;
const SCALAR_FLOAT_ADD_CYCLES: u64 = 1;
const SCALAR_FLOAT_COMPARE_CYCLES: u64 = 1;
const SCALAR_FLOAT_CONVERT_CYCLES: u64 = 1;
const SCALAR_FLOAT_MISC_CYCLES: u64 = 2;
const SCALAR_FLOAT_MUL_CYCLES: u64 = 3;
const SCALAR_FLOAT_FMA_CYCLES: u64 = 4;
const SCALAR_FLOAT_DIV_CYCLES: u64 = 11;
const SCALAR_FLOAT_SQRT_CYCLES: u64 = 23;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvFuLatencyOwner {
    Pipeline,
    DataCompletion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvFuLatency {
    cycles: u64,
    owner: RiscvFuLatencyOwner,
    o3_class: Option<O3RuntimeFuLatencyClass>,
}

impl RiscvFuLatency {
    const fn pipeline(cycles: u64, o3_class: Option<O3RuntimeFuLatencyClass>) -> Self {
        Self {
            cycles,
            owner: RiscvFuLatencyOwner::Pipeline,
            o3_class,
        }
    }

    const fn data_completion(cycles: u64) -> Self {
        Self {
            cycles,
            owner: RiscvFuLatencyOwner::DataCompletion,
            o3_class: None,
        }
    }

    pub(crate) const fn cycles(self) -> u64 {
        self.cycles
    }

    pub(crate) const fn owner(self) -> RiscvFuLatencyOwner {
        self.owner
    }

    pub(crate) const fn o3_class(self) -> Option<O3RuntimeFuLatencyClass> {
        self.o3_class
    }
}

pub(crate) const fn riscv_fu_latency(instruction: RiscvInstruction) -> Option<RiscvFuLatency> {
    match instruction {
        RiscvInstruction::Mul { .. }
        | RiscvInstruction::Mulh { .. }
        | RiscvInstruction::Mulhsu { .. }
        | RiscvInstruction::Mulhu { .. }
        | RiscvInstruction::Mulw { .. } => Some(RiscvFuLatency::pipeline(
            SCALAR_INTEGER_MUL_CYCLES,
            Some(O3RuntimeFuLatencyClass::ScalarIntegerMul),
        )),
        RiscvInstruction::Div { .. }
        | RiscvInstruction::Divu { .. }
        | RiscvInstruction::Rem { .. }
        | RiscvInstruction::Remu { .. }
        | RiscvInstruction::Divw { .. }
        | RiscvInstruction::Divuw { .. }
        | RiscvInstruction::Remw { .. }
        | RiscvInstruction::Remuw { .. } => Some(RiscvFuLatency::pipeline(
            SCALAR_INTEGER_DIV_CYCLES,
            Some(O3RuntimeFuLatencyClass::ScalarIntegerDiv),
        )),
        RiscvInstruction::VectorMultiplyLowVv { .. }
        | RiscvInstruction::VectorMultiplyLowVx { .. }
        | RiscvInstruction::VectorMultiplyHighUnsignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighUnsignedVx { .. }
        | RiscvInstruction::VectorMultiplyHighSignedUnsignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighSignedUnsignedVx { .. }
        | RiscvInstruction::VectorMultiplyHighSignedVv { .. }
        | RiscvInstruction::VectorMultiplyHighSignedVx { .. }
        | RiscvInstruction::VectorIntegerMultiplyAdd(_)
        | RiscvInstruction::VectorSaturating(
            RiscvVectorSaturatingInstruction::MulSignedFractionalVv { .. }
            | RiscvVectorSaturatingInstruction::MulSignedFractionalVx { .. },
        )
        | RiscvInstruction::VectorWideningInteger(
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
        ) => Some(RiscvFuLatency::pipeline(
            SCALAR_INTEGER_MUL_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorIntegerMul),
        )),
        RiscvInstruction::VectorDivideUnsignedVv { .. }
        | RiscvInstruction::VectorDivideUnsignedVx { .. }
        | RiscvInstruction::VectorDivideSignedVv { .. }
        | RiscvInstruction::VectorDivideSignedVx { .. }
        | RiscvInstruction::VectorRemainderUnsignedVv { .. }
        | RiscvInstruction::VectorRemainderUnsignedVx { .. }
        | RiscvInstruction::VectorRemainderSignedVv { .. }
        | RiscvInstruction::VectorRemainderSignedVx { .. } => Some(RiscvFuLatency::pipeline(
            SCALAR_INTEGER_DIV_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorIntegerDiv),
        )),
        RiscvInstruction::VectorFixedPointShift(..) | RiscvInstruction::VectorNarrow(..) => {
            Some(RiscvFuLatency::pipeline(VECTOR_INTEGER_SHIFT_CYCLES, None))
        }
        RiscvInstruction::VectorReduction(..) => {
            Some(RiscvFuLatency::pipeline(SCALAR_INTEGER_MUL_CYCLES, None))
        }
        RiscvInstruction::VectorMemory(
            RiscvVectorMemoryInstruction::LoadUnitStride { .. }
            | RiscvVectorMemoryInstruction::LoadUnitStrideFaultOnly { .. }
            | RiscvVectorMemoryInstruction::LoadSegmentUnitStride { .. }
            | RiscvVectorMemoryInstruction::LoadStrided { .. }
            | RiscvVectorMemoryInstruction::LoadIndexedUnordered { .. },
        ) => Some(RiscvFuLatency::data_completion(VECTOR_LOAD_CYCLES)),
        RiscvInstruction::VectorMemory(
            RiscvVectorMemoryInstruction::StoreUnitStride { .. }
            | RiscvVectorMemoryInstruction::StoreSegmentUnitStride { .. }
            | RiscvVectorMemoryInstruction::StoreStrided { .. }
            | RiscvVectorMemoryInstruction::StoreIndexedUnordered { .. },
        ) => Some(RiscvFuLatency::data_completion(VECTOR_STORE_CYCLES)),
        RiscvInstruction::FloatAddS { .. }
        | RiscvInstruction::FloatAddD { .. }
        | RiscvInstruction::FloatSubS { .. }
        | RiscvInstruction::FloatSubD { .. } => Some(RiscvFuLatency::pipeline(
            SCALAR_FLOAT_ADD_CYCLES,
            Some(O3RuntimeFuLatencyClass::ScalarFloatAdd),
        )),
        RiscvInstruction::FloatMinS { .. }
        | RiscvInstruction::FloatMinD { .. }
        | RiscvInstruction::FloatMaxS { .. }
        | RiscvInstruction::FloatMaxD { .. }
        | RiscvInstruction::FloatLessOrEqualS { .. }
        | RiscvInstruction::FloatLessOrEqualD { .. }
        | RiscvInstruction::FloatLessThanS { .. }
        | RiscvInstruction::FloatLessThanD { .. }
        | RiscvInstruction::FloatEqualS { .. }
        | RiscvInstruction::FloatEqualD { .. } => Some(RiscvFuLatency::pipeline(
            SCALAR_FLOAT_COMPARE_CYCLES,
            Some(O3RuntimeFuLatencyClass::ScalarFloatCompare),
        )),
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
        | RiscvInstruction::FloatConvertLuFromD { .. } => Some(RiscvFuLatency::pipeline(
            SCALAR_FLOAT_CONVERT_CYCLES,
            Some(O3RuntimeFuLatencyClass::ScalarFloatMisc),
        )),
        RiscvInstruction::FloatSignInjectS { .. }
        | RiscvInstruction::FloatSignInjectD { .. }
        | RiscvInstruction::FloatSignInjectNegS { .. }
        | RiscvInstruction::FloatSignInjectNegD { .. }
        | RiscvInstruction::FloatSignInjectXorS { .. }
        | RiscvInstruction::FloatSignInjectXorD { .. }
        | RiscvInstruction::FloatClassS { .. }
        | RiscvInstruction::FloatClassD { .. } => Some(RiscvFuLatency::pipeline(
            SCALAR_FLOAT_MISC_CYCLES,
            Some(O3RuntimeFuLatencyClass::ScalarFloatMisc),
        )),
        RiscvInstruction::FloatMulS { .. } | RiscvInstruction::FloatMulD { .. } => {
            Some(RiscvFuLatency::pipeline(
                SCALAR_FLOAT_MUL_CYCLES,
                Some(O3RuntimeFuLatencyClass::ScalarFloatMul),
            ))
        }
        RiscvInstruction::FloatMultiplyAddS { .. }
        | RiscvInstruction::FloatMultiplyAddD { .. }
        | RiscvInstruction::FloatMultiplySubtractS { .. }
        | RiscvInstruction::FloatMultiplySubtractD { .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractS { .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractD { .. }
        | RiscvInstruction::FloatNegativeMultiplyAddS { .. }
        | RiscvInstruction::FloatNegativeMultiplyAddD { .. } => Some(RiscvFuLatency::pipeline(
            SCALAR_FLOAT_FMA_CYCLES,
            Some(O3RuntimeFuLatencyClass::ScalarFloatFma),
        )),
        RiscvInstruction::FloatDivS { .. } | RiscvInstruction::FloatDivD { .. } => {
            Some(RiscvFuLatency::pipeline(
                SCALAR_FLOAT_DIV_CYCLES,
                Some(O3RuntimeFuLatencyClass::ScalarFloatDiv),
            ))
        }
        RiscvInstruction::FloatSqrtS { .. } | RiscvInstruction::FloatSqrtD { .. } => {
            Some(RiscvFuLatency::pipeline(
                SCALAR_FLOAT_SQRT_CYCLES,
                Some(O3RuntimeFuLatencyClass::ScalarFloatSqrt),
            ))
        }
        RiscvInstruction::VectorFloat(vector_instruction) => {
            Some(vector_float_fu_latency(vector_instruction))
        }
        _ => None,
    }
}

pub(crate) const fn riscv_execute_wait_cycles(instruction: RiscvInstruction) -> u64 {
    match riscv_fu_latency(instruction) {
        Some(latency) => latency.cycles(),
        None => 0,
    }
}

pub(crate) const fn riscv_pipeline_execute_wait_cycles(instruction: RiscvInstruction) -> u64 {
    match riscv_fu_latency(instruction) {
        Some(latency) if matches!(latency.owner(), RiscvFuLatencyOwner::Pipeline) => {
            latency.cycles()
        }
        _ => 0,
    }
}

pub(crate) const fn riscv_data_completion_execute_wait_cycles(
    instruction: RiscvInstruction,
) -> u64 {
    match riscv_fu_latency(instruction) {
        Some(latency) if matches!(latency.owner(), RiscvFuLatencyOwner::DataCompletion) => {
            latency.cycles()
        }
        _ => 0,
    }
}

pub(crate) const fn riscv_o3_fu_latency_class(
    instruction: RiscvInstruction,
) -> Option<O3RuntimeFuLatencyClass> {
    match riscv_fu_latency(instruction) {
        Some(latency) => latency.o3_class(),
        None => None,
    }
}

const fn vector_float_fu_latency(instruction: RiscvVectorFloatInstruction) -> RiscvFuLatency {
    match instruction {
        RiscvVectorFloatInstruction::AddVv { .. }
        | RiscvVectorFloatInstruction::AddVf { .. }
        | RiscvVectorFloatInstruction::SubVv { .. }
        | RiscvVectorFloatInstruction::SubVf { .. }
        | RiscvVectorFloatInstruction::ReverseSubVf { .. } => RiscvFuLatency::pipeline(
            SCALAR_FLOAT_ADD_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatAdd),
        ),
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
        | RiscvVectorFloatInstruction::MaskLessEqualVf { .. } => RiscvFuLatency::pipeline(
            SCALAR_FLOAT_COMPARE_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatCompare),
        ),
        RiscvVectorFloatInstruction::ConvertFloatFromUnsignedIntV { .. }
        | RiscvVectorFloatInstruction::ConvertFloatFromSignedIntV { .. }
        | RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatV { .. }
        | RiscvVectorFloatInstruction::ConvertSignedIntFromFloatV { .. }
        | RiscvVectorFloatInstruction::ConvertUnsignedIntFromFloatTowardZeroV { .. }
        | RiscvVectorFloatInstruction::ConvertSignedIntFromFloatTowardZeroV { .. }
        | RiscvVectorFloatInstruction::MergeVf { .. }
        | RiscvVectorFloatInstruction::MoveVf { .. }
        | RiscvVectorFloatInstruction::MoveFv { .. }
        | RiscvVectorFloatInstruction::MoveSv { .. } => RiscvFuLatency::pipeline(
            SCALAR_FLOAT_CONVERT_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatMisc),
        ),
        RiscvVectorFloatInstruction::SignInjectVv { .. }
        | RiscvVectorFloatInstruction::SignInjectVf { .. }
        | RiscvVectorFloatInstruction::SignInjectNegVv { .. }
        | RiscvVectorFloatInstruction::SignInjectNegVf { .. }
        | RiscvVectorFloatInstruction::SignInjectXorVv { .. }
        | RiscvVectorFloatInstruction::SignInjectXorVf { .. }
        | RiscvVectorFloatInstruction::ClassV { .. } => RiscvFuLatency::pipeline(
            SCALAR_FLOAT_MISC_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatMisc),
        ),
        RiscvVectorFloatInstruction::MulVv { .. } | RiscvVectorFloatInstruction::MulVf { .. } => {
            RiscvFuLatency::pipeline(
                SCALAR_FLOAT_MUL_CYCLES,
                Some(O3RuntimeFuLatencyClass::VectorFloatMul),
            )
        }
        RiscvVectorFloatInstruction::MulAddVv { .. }
        | RiscvVectorFloatInstruction::MulAddVf { .. } => RiscvFuLatency::pipeline(
            SCALAR_FLOAT_FMA_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatFma),
        ),
        RiscvVectorFloatInstruction::DivVv { .. }
        | RiscvVectorFloatInstruction::DivVf { .. }
        | RiscvVectorFloatInstruction::ReverseDivVf { .. } => RiscvFuLatency::pipeline(
            SCALAR_FLOAT_DIV_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatDiv),
        ),
        RiscvVectorFloatInstruction::SqrtV { .. } => RiscvFuLatency::pipeline(
            SCALAR_FLOAT_SQRT_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatSqrt),
        ),
    }
}
