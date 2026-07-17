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
    writes_vector_state: bool,
}

impl RiscvFuLatency {
    const fn pipeline(cycles: u64, o3_class: Option<O3RuntimeFuLatencyClass>) -> Self {
        Self {
            cycles,
            owner: RiscvFuLatencyOwner::Pipeline,
            o3_class,
            writes_vector_state: false,
        }
    }

    const fn vector_pipeline(cycles: u64, o3_class: Option<O3RuntimeFuLatencyClass>) -> Self {
        Self {
            cycles,
            owner: RiscvFuLatencyOwner::Pipeline,
            o3_class,
            writes_vector_state: true,
        }
    }

    const fn data_completion(cycles: u64) -> Self {
        Self {
            cycles,
            owner: RiscvFuLatencyOwner::DataCompletion,
            o3_class: None,
            writes_vector_state: false,
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

    pub(crate) const fn writes_vector_state(self) -> bool {
        self.writes_vector_state
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
        ) => Some(RiscvFuLatency::vector_pipeline(
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
        | RiscvInstruction::VectorRemainderSignedVx { .. } => {
            Some(RiscvFuLatency::vector_pipeline(
                SCALAR_INTEGER_DIV_CYCLES,
                Some(O3RuntimeFuLatencyClass::VectorIntegerDiv),
            ))
        }
        RiscvInstruction::VectorFixedPointShift(..) | RiscvInstruction::VectorNarrow(..) => Some(
            RiscvFuLatency::vector_pipeline(VECTOR_INTEGER_SHIFT_CYCLES, None),
        ),
        RiscvInstruction::VectorReduction(..) => Some(RiscvFuLatency::vector_pipeline(
            SCALAR_INTEGER_MUL_CYCLES,
            None,
        )),
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

pub(crate) const fn riscv_pipeline_fu_writes_vector_state(instruction: RiscvInstruction) -> bool {
    matches!(
        riscv_fu_latency(instruction),
        Some(latency)
            if matches!(latency.owner(), RiscvFuLatencyOwner::Pipeline)
                && latency.writes_vector_state()
    )
}

const fn vector_float_fu_latency(instruction: RiscvVectorFloatInstruction) -> RiscvFuLatency {
    match instruction {
        RiscvVectorFloatInstruction::AddVv { .. }
        | RiscvVectorFloatInstruction::AddVf { .. }
        | RiscvVectorFloatInstruction::SubVv { .. }
        | RiscvVectorFloatInstruction::SubVf { .. }
        | RiscvVectorFloatInstruction::ReverseSubVf { .. } => RiscvFuLatency::vector_pipeline(
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
        | RiscvVectorFloatInstruction::MaskLessEqualVf { .. } => RiscvFuLatency::vector_pipeline(
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
        | RiscvVectorFloatInstruction::MoveSv { .. } => RiscvFuLatency::vector_pipeline(
            SCALAR_FLOAT_CONVERT_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatMisc),
        ),
        RiscvVectorFloatInstruction::SignInjectVv { .. }
        | RiscvVectorFloatInstruction::SignInjectVf { .. }
        | RiscvVectorFloatInstruction::SignInjectNegVv { .. }
        | RiscvVectorFloatInstruction::SignInjectNegVf { .. }
        | RiscvVectorFloatInstruction::SignInjectXorVv { .. }
        | RiscvVectorFloatInstruction::SignInjectXorVf { .. }
        | RiscvVectorFloatInstruction::ClassV { .. } => RiscvFuLatency::vector_pipeline(
            SCALAR_FLOAT_MISC_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatMisc),
        ),
        RiscvVectorFloatInstruction::MulVv { .. } | RiscvVectorFloatInstruction::MulVf { .. } => {
            RiscvFuLatency::vector_pipeline(
                SCALAR_FLOAT_MUL_CYCLES,
                Some(O3RuntimeFuLatencyClass::VectorFloatMul),
            )
        }
        RiscvVectorFloatInstruction::MulAddVv { .. }
        | RiscvVectorFloatInstruction::MulAddVf { .. } => RiscvFuLatency::vector_pipeline(
            SCALAR_FLOAT_FMA_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatFma),
        ),
        RiscvVectorFloatInstruction::DivVv { .. }
        | RiscvVectorFloatInstruction::DivVf { .. }
        | RiscvVectorFloatInstruction::ReverseDivVf { .. } => RiscvFuLatency::vector_pipeline(
            SCALAR_FLOAT_DIV_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatDiv),
        ),
        RiscvVectorFloatInstruction::SqrtV { .. } => RiscvFuLatency::vector_pipeline(
            SCALAR_FLOAT_SQRT_CYCLES,
            Some(O3RuntimeFuLatencyClass::VectorFloatSqrt),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_pipeline_latency_families_report_vector_state_writes() {
        for (label, raw) in [
            ("integer multiply", vector_mvv_type(0b100101, 2, 1, 3)),
            ("integer multiply-add", vector_mvv_type(0b101001, 2, 1, 3)),
            ("saturating multiply", vector_vv_type(0b100111, 2, 1, 3)),
            ("widening multiply", vector_mvv_type(0b111000, 2, 1, 4)),
            ("integer divide", vector_mvv_type(0b100000, 2, 1, 3)),
            ("fixed-point shift", vector_vv_type(0b101010, 2, 1, 3)),
            ("narrow shift", vector_vv_type(0b101100, 6, 5, 3)),
            ("reduction", vector_mvv_type(0b000000, 2, 1, 3)),
            ("widening reduction", vector_vv_type(0b110000, 2, 1, 3)),
            ("float add", vector_float_vv_type(0b000000, 2, 1, 3)),
            ("float compare", vector_float_vv_type(0b000100, 2, 1, 3)),
            ("float misc", vector_float_vv_type(0b001000, 2, 1, 3)),
            ("float multiply", vector_float_vv_type(0b100100, 2, 1, 3)),
            (
                "float multiply-add",
                vector_float_vv_type(0b101100, 2, 1, 3),
            ),
            ("float divide", vector_float_vv_type(0b100000, 2, 1, 3)),
            (
                "float square-root",
                vector_float_type(0b010011, 0b001, 1, 0x00, 3),
            ),
        ] {
            let instruction = RiscvInstruction::decode(raw).unwrap();
            assert!(
                riscv_pipeline_fu_writes_vector_state(instruction),
                "{label}: {instruction:?}"
            );
        }
    }

    #[test]
    fn nonvector_or_data_completion_latency_does_not_report_vector_state_write() {
        let scalar_div = RiscvInstruction::decode(r_type(1, 2, 1, 0b100, 3, 0x33)).unwrap();
        let vector_load = RiscvInstruction::decode(0x0205_7087).unwrap();

        assert!(!riscv_pipeline_fu_writes_vector_state(scalar_div));
        assert!(!riscv_pipeline_fu_writes_vector_state(vector_load));
    }

    fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
        (funct7 << 25)
            | (u32::from(rs2) << 20)
            | (u32::from(rs1) << 15)
            | (funct3 << 12)
            | (u32::from(rd) << 7)
            | opcode
    }

    fn vector_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
        (funct6 << 26)
            | (1 << 25)
            | (u32::from(vs2) << 20)
            | (u32::from(vs1) << 15)
            | (u32::from(vd) << 7)
            | 0x57
    }

    fn vector_mvv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
        vector_vv_type(funct6, vs2, vs1, vd) | (0b010 << 12)
    }

    fn vector_float_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
        vector_float_type(funct6, 0b001, vs2, vs1, vd)
    }

    fn vector_float_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
        (funct6 << 26)
            | (1 << 25)
            | (u32::from(vs2) << 20)
            | (u32::from(rs1) << 15)
            | (funct3 << 12)
            | (u32::from(vd) << 7)
            | 0x57
    }
}
