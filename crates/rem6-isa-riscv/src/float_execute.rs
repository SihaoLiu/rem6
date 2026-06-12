use crate::{float, FloatRegisterWrite, RegisterWrite, RiscvHartState, RiscvInstruction};

pub(crate) fn execute_float_register_instruction(
    hart: &mut RiscvHartState,
    writes: &mut Vec<FloatRegisterWrite>,
    instruction: RiscvInstruction,
) {
    match instruction {
        instruction @ (RiscvInstruction::FloatMultiplyAddS { rs1, rs2, rs3, .. }
        | RiscvInstruction::FloatMultiplyAddD { rs1, rs2, rs3, .. }
        | RiscvInstruction::FloatMultiplySubtractS { rs1, rs2, rs3, .. }
        | RiscvInstruction::FloatMultiplySubtractD { rs1, rs2, rs3, .. }
        | RiscvInstruction::FloatNegativeMultiplySubtractS {
            rs1, rs2, rs3, ..
        }
        | RiscvInstruction::FloatNegativeMultiplySubtractD {
            rs1, rs2, rs3, ..
        }
        | RiscvInstruction::FloatNegativeMultiplyAddS { rs1, rs2, rs3, .. }
        | RiscvInstruction::FloatNegativeMultiplyAddD { rs1, rs2, rs3, .. }) => {
            let (rd, value) = float::float_register_write_ternary(
                instruction,
                hart.read_float(rs1),
                hart.read_float(rs2),
                hart.read_float(rs3),
            );
            float::write_float_register(hart, writes, rd, value);
        }
        instruction @ (RiscvInstruction::FloatAddS { rs1, rs2, .. }
        | RiscvInstruction::FloatAddD { rs1, rs2, .. }
        | RiscvInstruction::FloatSubS { rs1, rs2, .. }
        | RiscvInstruction::FloatSubD { rs1, rs2, .. }
        | RiscvInstruction::FloatMulS { rs1, rs2, .. }
        | RiscvInstruction::FloatMulD { rs1, rs2, .. }
        | RiscvInstruction::FloatDivS { rs1, rs2, .. }
        | RiscvInstruction::FloatDivD { rs1, rs2, .. }
        | RiscvInstruction::FloatSignInjectS { rs1, rs2, .. }
        | RiscvInstruction::FloatSignInjectD { rs1, rs2, .. }
        | RiscvInstruction::FloatSignInjectNegS { rs1, rs2, .. }
        | RiscvInstruction::FloatSignInjectNegD { rs1, rs2, .. }
        | RiscvInstruction::FloatSignInjectXorS { rs1, rs2, .. }
        | RiscvInstruction::FloatSignInjectXorD { rs1, rs2, .. }
        | RiscvInstruction::FloatMinS { rs1, rs2, .. }
        | RiscvInstruction::FloatMinD { rs1, rs2, .. }
        | RiscvInstruction::FloatMaxS { rs1, rs2, .. }
        | RiscvInstruction::FloatMaxD { rs1, rs2, .. }) => {
            let lhs = hart.read_float(rs1);
            let rhs = hart.read_float(rs2);
            let flags = float::binary_exception_flags(instruction, lhs, rhs);
            let (rd, value) = float::float_register_write(instruction, lhs, rhs);
            hart.raise_float_exception_flags(flags);
            float::write_float_register(hart, writes, rd, value);
        }
        instruction @ (RiscvInstruction::FloatSqrtS { rs1, .. }
        | RiscvInstruction::FloatSqrtD { rs1, .. }
        | RiscvInstruction::FloatConvertSFromD { rs1, .. }
        | RiscvInstruction::FloatConvertDFromS { rs1, .. }) => {
            let (rd, value) = float::float_register_write(instruction, hart.read_float(rs1), 0);
            float::write_float_register(hart, writes, rd, value);
        }
        instruction @ (RiscvInstruction::FloatMoveSFromX { rs1, .. }
        | RiscvInstruction::FloatMoveDFromX { rs1, .. }
        | RiscvInstruction::FloatConvertSFromW { rs1, .. }
        | RiscvInstruction::FloatConvertSFromWu { rs1, .. }
        | RiscvInstruction::FloatConvertSFromL { rs1, .. }
        | RiscvInstruction::FloatConvertSFromLu { rs1, .. }
        | RiscvInstruction::FloatConvertDFromW { rs1, .. }
        | RiscvInstruction::FloatConvertDFromWu { rs1, .. }
        | RiscvInstruction::FloatConvertDFromL { rs1, .. }
        | RiscvInstruction::FloatConvertDFromLu { rs1, .. }) => {
            let (rd, value) = float::float_register_write_from_integer(instruction, hart.read(rs1));
            float::write_float_register(hart, writes, rd, value);
        }
        _ => unreachable!("non-float-register instruction dispatched to float register executor"),
    }
}

pub(crate) fn execute_float_integer_instruction(
    hart: &mut RiscvHartState,
    writes: &mut Vec<RegisterWrite>,
    instruction: RiscvInstruction,
) -> Result<(), ()> {
    match instruction {
        instruction @ (RiscvInstruction::FloatLessOrEqualS { rs1, rs2, .. }
        | RiscvInstruction::FloatLessOrEqualD { rs1, rs2, .. }
        | RiscvInstruction::FloatLessThanS { rs1, rs2, .. }
        | RiscvInstruction::FloatLessThanD { rs1, rs2, .. }
        | RiscvInstruction::FloatEqualS { rs1, rs2, .. }
        | RiscvInstruction::FloatEqualD { rs1, rs2, .. }) => {
            let (rd, value) = float::integer_register_write_rne(
                instruction,
                hart.read_float(rs1),
                hart.read_float(rs2),
            );
            crate::write_register(hart, writes, rd, value);
        }
        instruction @ (RiscvInstruction::FloatClassS { rs1, .. }
        | RiscvInstruction::FloatClassD { rs1, .. }
        | RiscvInstruction::FloatMoveXFromS { rs1, .. }
        | RiscvInstruction::FloatMoveXFromD { rs1, .. }
        | RiscvInstruction::FloatConvertWFromS { rs1, .. }
        | RiscvInstruction::FloatConvertWuFromS { rs1, .. }
        | RiscvInstruction::FloatConvertLFromS { rs1, .. }
        | RiscvInstruction::FloatConvertLuFromS { rs1, .. }
        | RiscvInstruction::FloatConvertWFromD { rs1, .. }
        | RiscvInstruction::FloatConvertWuFromD { rs1, .. }
        | RiscvInstruction::FloatConvertLFromD { rs1, .. }
        | RiscvInstruction::FloatConvertLuFromD { rs1, .. }) => {
            let Some((rd, value)) = float::integer_register_write(
                instruction,
                hart.read_float(rs1),
                hart.float_status().frm(),
            ) else {
                return Err(());
            };
            crate::write_register(hart, writes, rd, value);
        }
        _ => unreachable!("non-float-integer instruction dispatched to float integer executor"),
    }
    Ok(())
}
