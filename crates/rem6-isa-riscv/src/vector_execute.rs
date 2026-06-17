use crate::{
    vector_group::{
        read_mask_bit, read_register_group, valid_register_group, write_register_group,
        VectorBinaryPlan, MAX_VECTOR_GROUP_BYTES,
    },
    RiscvHartState, RiscvInstruction, RiscvVectorConfig, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn execute_vector_integer_binary(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
) -> bool {
    match instruction {
        RiscvInstruction::VectorAddVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Add)
        }
        RiscvInstruction::VectorAddVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Add)
        }
        RiscvInstruction::VectorAddVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::Add)
        }
        RiscvInstruction::VectorSubVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Sub)
        }
        RiscvInstruction::VectorSubVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Sub)
        }
        RiscvInstruction::VectorMinUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MinUnsigned)
        }
        RiscvInstruction::VectorMinUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MinUnsigned)
        }
        RiscvInstruction::VectorMinSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MinSigned)
        }
        RiscvInstruction::VectorMinSignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MinSigned)
        }
        RiscvInstruction::VectorMaxUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MaxUnsigned)
        }
        RiscvInstruction::VectorMaxUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MaxUnsigned)
        }
        RiscvInstruction::VectorMaxSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MaxSigned)
        }
        RiscvInstruction::VectorMaxSignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MaxSigned)
        }
        RiscvInstruction::VectorMultiplyLowVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MultiplyLow)
        }
        RiscvInstruction::VectorMultiplyLowVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::MultiplyLow)
        }
        RiscvInstruction::VectorMultiplyHighUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MultiplyHighUnsigned)
        }
        RiscvInstruction::VectorMultiplyHighUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                LaneBinaryOp::MultiplyHighUnsigned,
            )
        }
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MultiplyHighSignedUnsigned)
        }
        RiscvInstruction::VectorMultiplyHighSignedUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                LaneBinaryOp::MultiplyHighSignedUnsigned,
            )
        }
        RiscvInstruction::VectorMultiplyHighSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::MultiplyHighSigned)
        }
        RiscvInstruction::VectorMultiplyHighSignedVx { vd, vs2, rs1 } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            LaneBinaryOp::MultiplyHighSigned,
        ),
        RiscvInstruction::VectorDivideUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::DivideUnsigned)
        }
        RiscvInstruction::VectorDivideUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::DivideUnsigned)
        }
        RiscvInstruction::VectorDivideSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::DivideSigned)
        }
        RiscvInstruction::VectorDivideSignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::DivideSigned)
        }
        RiscvInstruction::VectorRemainderUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::RemainderUnsigned)
        }
        RiscvInstruction::VectorRemainderUnsignedVx { vd, vs2, rs1 } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            LaneBinaryOp::RemainderUnsigned,
        ),
        RiscvInstruction::VectorRemainderSignedVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::RemainderSigned)
        }
        RiscvInstruction::VectorRemainderSignedVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::RemainderSigned)
        }
        RiscvInstruction::VectorMergeVvm { vd, vs2, vs1 } => {
            execute_vector_merge_vv(hart, vd, vs2, vs1)
        }
        RiscvInstruction::VectorMergeVxm { vd, vs2, rs1 } => {
            execute_vector_merge_vx(hart, vd, vs2, hart.read(rs1))
        }
        RiscvInstruction::VectorMergeVim { vd, vs2, imm } => {
            execute_vector_merge_vi(hart, vd, vs2, imm)
        }
        RiscvInstruction::VectorCompressVm(vd, vs2, vs1) => {
            crate::vector_compress_execute::execute_vector_compress_vm(hart, vd, vs2, vs1)
        }
        RiscvInstruction::VectorNarrowClipUnsignedWi(vd, vs2, shift) => {
            crate::vector_narrow_clip_execute::execute_vector_narrow_clip_unsigned_wi(
                hart, vd, vs2, shift,
            )
        }
        RiscvInstruction::VectorMoveVv { vd, vs1 } => execute_vector_move_vv(hart, vd, vs1),
        RiscvInstruction::VectorMoveVx { vd, rs1 } => {
            execute_vector_move_vx(hart, vd, hart.read(rs1))
        }
        RiscvInstruction::VectorMoveVi { vd, imm } => execute_vector_move_vi(hart, vd, imm),
        RiscvInstruction::VectorMaskAndMm { vd, vs2, vs1 } => {
            execute_vector_mask_logical_mm(hart, vd, vs2, vs1, MaskLogicalOp::And)
        }
        RiscvInstruction::VectorMaskNandMm { vd, vs2, vs1 } => {
            execute_vector_mask_logical_mm(hart, vd, vs2, vs1, MaskLogicalOp::Nand)
        }
        RiscvInstruction::VectorMaskAndNotMm { vd, vs2, vs1 } => {
            execute_vector_mask_logical_mm(hart, vd, vs2, vs1, MaskLogicalOp::AndNot)
        }
        RiscvInstruction::VectorMaskXorMm { vd, vs2, vs1 } => {
            execute_vector_mask_logical_mm(hart, vd, vs2, vs1, MaskLogicalOp::Xor)
        }
        RiscvInstruction::VectorMaskOrMm { vd, vs2, vs1 } => {
            execute_vector_mask_logical_mm(hart, vd, vs2, vs1, MaskLogicalOp::Or)
        }
        RiscvInstruction::VectorMaskNorMm { vd, vs2, vs1 } => {
            execute_vector_mask_logical_mm(hart, vd, vs2, vs1, MaskLogicalOp::Nor)
        }
        RiscvInstruction::VectorMaskOrNotMm { vd, vs2, vs1 } => {
            execute_vector_mask_logical_mm(hart, vd, vs2, vs1, MaskLogicalOp::OrNot)
        }
        RiscvInstruction::VectorMaskXnorMm { vd, vs2, vs1 } => {
            execute_vector_mask_logical_mm(hart, vd, vs2, vs1, MaskLogicalOp::Xnor)
        }
        RiscvInstruction::VectorMaskEqualVv { vd, vs1, vs2 } => {
            execute_vector_mask_compare_vv(hart, vd, vs1, vs2, MaskCompareOp::Equal)
        }
        RiscvInstruction::VectorMaskEqualVx { vd, vs2, rs1 } => {
            execute_vector_mask_compare_vx(hart, vd, vs2, hart.read(rs1), MaskCompareOp::Equal)
        }
        RiscvInstruction::VectorMaskEqualVi { vd, vs2, imm } => {
            execute_vector_mask_compare_vi(hart, vd, vs2, imm, MaskCompareOp::Equal)
        }
        RiscvInstruction::VectorMaskNotEqualVv { vd, vs1, vs2 } => {
            execute_vector_mask_compare_vv(hart, vd, vs1, vs2, MaskCompareOp::NotEqual)
        }
        RiscvInstruction::VectorMaskNotEqualVx { vd, vs2, rs1 } => {
            execute_vector_mask_compare_vx(hart, vd, vs2, hart.read(rs1), MaskCompareOp::NotEqual)
        }
        RiscvInstruction::VectorMaskNotEqualVi { vd, vs2, imm } => {
            execute_vector_mask_compare_vi(hart, vd, vs2, imm, MaskCompareOp::NotEqual)
        }
        RiscvInstruction::VectorMaskLessUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_mask_compare_vv(hart, vd, vs1, vs2, MaskCompareOp::LessUnsigned)
        }
        RiscvInstruction::VectorMaskLessUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_mask_compare_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                MaskCompareOp::LessUnsigned,
            )
        }
        RiscvInstruction::VectorMaskLessSignedVv { vd, vs1, vs2 } => {
            execute_vector_mask_compare_vv(hart, vd, vs1, vs2, MaskCompareOp::LessSigned)
        }
        RiscvInstruction::VectorMaskLessSignedVx { vd, vs2, rs1 } => {
            execute_vector_mask_compare_vx(hart, vd, vs2, hart.read(rs1), MaskCompareOp::LessSigned)
        }
        RiscvInstruction::VectorMaskLessEqualUnsignedVv { vd, vs1, vs2 } => {
            execute_vector_mask_compare_vv(hart, vd, vs1, vs2, MaskCompareOp::LessEqualUnsigned)
        }
        RiscvInstruction::VectorMaskLessEqualUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_mask_compare_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                MaskCompareOp::LessEqualUnsigned,
            )
        }
        RiscvInstruction::VectorMaskLessEqualUnsignedVi { vd, vs2, imm } => {
            execute_vector_mask_compare_vi(hart, vd, vs2, imm, MaskCompareOp::LessEqualUnsigned)
        }
        RiscvInstruction::VectorMaskLessEqualSignedVv { vd, vs1, vs2 } => {
            execute_vector_mask_compare_vv(hart, vd, vs1, vs2, MaskCompareOp::LessEqualSigned)
        }
        RiscvInstruction::VectorMaskLessEqualSignedVx { vd, vs2, rs1 } => {
            execute_vector_mask_compare_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                MaskCompareOp::LessEqualSigned,
            )
        }
        RiscvInstruction::VectorMaskLessEqualSignedVi { vd, vs2, imm } => {
            execute_vector_mask_compare_vi(hart, vd, vs2, imm, MaskCompareOp::LessEqualSigned)
        }
        RiscvInstruction::VectorMaskGreaterUnsignedVx { vd, vs2, rs1 } => {
            execute_vector_mask_compare_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                MaskCompareOp::GreaterUnsigned,
            )
        }
        RiscvInstruction::VectorMaskGreaterUnsignedVi { vd, vs2, imm } => {
            execute_vector_mask_compare_vi(hart, vd, vs2, imm, MaskCompareOp::GreaterUnsigned)
        }
        RiscvInstruction::VectorMaskGreaterSignedVx { vd, vs2, rs1 } => {
            execute_vector_mask_compare_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                MaskCompareOp::GreaterSigned,
            )
        }
        RiscvInstruction::VectorMaskGreaterSignedVi { vd, vs2, imm } => {
            execute_vector_mask_compare_vi(hart, vd, vs2, imm, MaskCompareOp::GreaterSigned)
        }
        RiscvInstruction::VectorAndVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::And)
        }
        RiscvInstruction::VectorAndVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::And)
        }
        RiscvInstruction::VectorAndVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::And)
        }
        RiscvInstruction::VectorOrVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Or)
        }
        RiscvInstruction::VectorOrVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Or)
        }
        RiscvInstruction::VectorOrVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::Or)
        }
        RiscvInstruction::VectorXorVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Xor)
        }
        RiscvInstruction::VectorXorVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Xor)
        }
        RiscvInstruction::VectorXorVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::Xor)
        }
        RiscvInstruction::VectorShiftLeftLogicalVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::ShiftLeftLogical)
        }
        RiscvInstruction::VectorShiftLeftLogicalVx { vd, vs2, rs1 } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            LaneBinaryOp::ShiftLeftLogical,
        ),
        RiscvInstruction::VectorShiftLeftLogicalVi { vd, vs2, shamt } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            u64::from(shamt),
            LaneBinaryOp::ShiftLeftLogical,
        ),
        RiscvInstruction::VectorShiftRightLogicalVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::ShiftRightLogical)
        }
        RiscvInstruction::VectorShiftRightLogicalVx { vd, vs2, rs1 } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            LaneBinaryOp::ShiftRightLogical,
        ),
        RiscvInstruction::VectorShiftRightLogicalVi { vd, vs2, shamt } => execute_vector_binary_vx(
            hart,
            vd,
            vs2,
            u64::from(shamt),
            LaneBinaryOp::ShiftRightLogical,
        ),
        RiscvInstruction::VectorShiftRightArithmeticVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::ShiftRightArithmetic)
        }
        RiscvInstruction::VectorShiftRightArithmeticVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                LaneBinaryOp::ShiftRightArithmetic,
            )
        }
        RiscvInstruction::VectorShiftRightArithmeticVi { vd, vs2, shamt } => {
            execute_vector_binary_vx(
                hart,
                vd,
                vs2,
                u64::from(shamt),
                LaneBinaryOp::ShiftRightArithmetic,
            )
        }
        _ => false,
    }
}

fn execute_vector_binary_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
    operation: LaneBinaryOp,
) -> bool {
    execute_vector_binary_vx(hart, vd, vs2, imm as i64 as u64, operation)
}

fn execute_vector_binary_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: LaneBinaryOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_vector_lanes(&plan, &mut result, &left, &right, operation);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_vector_binary_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    operation: LaneBinaryOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_scalar_lanes(&plan, &mut result, &left, scalar, operation);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_vector_merge_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
) -> bool {
    execute_vector_merge_vx(hart, vd, vs2, imm as i64 as u64)
}

fn execute_vector_merge_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    if register_group_overlaps_v0(vd, plan.group_registers)
        || register_group_overlaps_v0(vs2, plan.group_registers)
        || register_group_overlaps_v0(vs1, plan.group_registers)
    {
        return false;
    }
    let mask = hart.read_vector(VectorRegister::from_field(0));
    let fallback = read_register_group(hart, vs2, plan.group_registers);
    let selected = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_masked_vector_select_lanes(&plan, &mut result, &fallback, &selected, &mask);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_vector_merge_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if register_group_overlaps_v0(vd, plan.group_registers)
        || register_group_overlaps_v0(vs2, plan.group_registers)
    {
        return false;
    }
    let mask = hart.read_vector(VectorRegister::from_field(0));
    let fallback = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_masked_scalar_select_lanes(&plan, &mut result, &fallback, scalar, &mask);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_vector_move_vi(hart: &mut RiscvHartState, vd: VectorRegister, imm: i8) -> bool {
    execute_vector_move_vx(hart, vd, imm as i64 as u64)
}

fn execute_vector_move_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs1]) else {
        return false;
    };
    let source = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_vector_move_lanes(&plan, &mut result, &source);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_vector_move_vx(hart: &mut RiscvHartState, vd: VectorRegister, scalar: u64) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[]) else {
        return false;
    };
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_scalar_move_lanes(&plan, &mut result, scalar);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_vector_mask_compare_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
    operation: MaskCompareOp,
) -> bool {
    execute_vector_mask_compare_vx(hart, vd, vs2, imm as i64 as u64, operation)
}

fn execute_vector_mask_compare_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: MaskCompareOp,
) -> bool {
    let Some(plan) = VectorMaskPlan::new(hart, &[vs2, vs1]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    let mut mask = hart.read_vector(vd);
    apply_vector_mask_lanes(&plan, &mut mask, &left, &right, operation);
    hart.write_vector(vd, mask);
    true
}

fn execute_vector_mask_compare_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    operation: MaskCompareOp,
) -> bool {
    let Some(plan) = VectorMaskPlan::new(hart, &[vs2]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let mut mask = hart.read_vector(vd);
    apply_scalar_mask_lanes(&plan, &mut mask, &left, scalar, operation);
    hart.write_vector(vd, mask);
    true
}

fn execute_vector_mask_logical_mm(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
    operation: MaskLogicalOp,
) -> bool {
    let Some(plan) = VectorMaskLogicalPlan::new(hart) else {
        return false;
    };
    let left = hart.read_vector(vs2);
    let right = hart.read_vector(vs1);
    let mut result = hart.read_vector(vd);
    apply_mask_logical_bits(&plan, &mut result, &left, &right, operation);
    hart.write_vector(vd, result);
    true
}

#[derive(Clone, Copy)]
enum LaneBinaryOp {
    Add,
    Sub,
    MinUnsigned,
    MinSigned,
    MaxUnsigned,
    MaxSigned,
    MultiplyLow,
    MultiplyHighUnsigned,
    MultiplyHighSignedUnsigned,
    MultiplyHighSigned,
    DivideUnsigned,
    DivideSigned,
    RemainderUnsigned,
    RemainderSigned,
    And,
    Or,
    Xor,
    ShiftLeftLogical,
    ShiftRightLogical,
    ShiftRightArithmetic,
}

impl LaneBinaryOp {
    fn apply_u8(self, left: u8, right: u8) -> u8 {
        let shift = shift_amount(u64::from(right), 8);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i8).min(right as i8) as u8,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i8).max(right as i8) as u8,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u8::BITS) as u8
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i8), u128::from(right), u8::BITS)
                    as u8
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i8), i128::from(right as i8), u8::BITS)
                    as u8
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u8::MAX.into()) as u8
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i8),
                i128::from(right as i8),
                i128::from(i8::MIN),
            ) as u8,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u8
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i8),
                i128::from(right as i8),
                i128::from(i8::MIN),
            ) as u8,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i8) >> shift) as u8,
        }
    }

    fn apply_u16(self, left: u16, right: u16) -> u16 {
        let shift = shift_amount(u64::from(right), 16);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i16).min(right as i16) as u16,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i16).max(right as i16) as u16,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u16::BITS) as u16
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i16), u128::from(right), u16::BITS)
                    as u16
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i16), i128::from(right as i16), u16::BITS)
                    as u16
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u16::MAX.into()) as u16
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i16),
                i128::from(right as i16),
                i128::from(i16::MIN),
            ) as u16,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u16
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i16),
                i128::from(right as i16),
                i128::from(i16::MIN),
            ) as u16,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i16) >> shift) as u16,
        }
    }

    fn apply_u32(self, left: u32, right: u32) -> u32 {
        let shift = shift_amount(u64::from(right), 32);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i32).min(right as i32) as u32,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i32).max(right as i32) as u32,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u32::BITS) as u32
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i32), u128::from(right), u32::BITS)
                    as u32
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i32), i128::from(right as i32), u32::BITS)
                    as u32
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u32::MAX.into()) as u32
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i32),
                i128::from(right as i32),
                i128::from(i32::MIN),
            ) as u32,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u32
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i32),
                i128::from(right as i32),
                i128::from(i32::MIN),
            ) as u32,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i32) >> shift) as u32,
        }
    }

    fn apply_u64(self, left: u64, right: u64) -> u64 {
        let shift = shift_amount(right, 64);
        match self {
            Self::Add => left.wrapping_add(right),
            Self::Sub => left.wrapping_sub(right),
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => (left as i64).min(right as i64) as u64,
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => (left as i64).max(right as i64) as u64,
            Self::MultiplyLow => left.wrapping_mul(right),
            Self::MultiplyHighUnsigned => {
                multiply_high_unsigned(u128::from(left), u128::from(right), u64::BITS) as u64
            }
            Self::MultiplyHighSignedUnsigned => {
                multiply_high_signed_unsigned(i128::from(left as i64), u128::from(right), u64::BITS)
                    as u64
            }
            Self::MultiplyHighSigned => {
                multiply_high_signed(i128::from(left as i64), i128::from(right as i64), u64::BITS)
                    as u64
            }
            Self::DivideUnsigned => {
                divide_unsigned(u128::from(left), u128::from(right), u64::MAX.into()) as u64
            }
            Self::DivideSigned => divide_signed(
                i128::from(left as i64),
                i128::from(right as i64),
                i128::from(i64::MIN),
            ) as u64,
            Self::RemainderUnsigned => {
                remainder_unsigned(u128::from(left), u128::from(right)) as u64
            }
            Self::RemainderSigned => remainder_signed(
                i128::from(left as i64),
                i128::from(right as i64),
                i128::from(i64::MIN),
            ) as u64,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::ShiftLeftLogical => left << shift,
            Self::ShiftRightLogical => left >> shift,
            Self::ShiftRightArithmetic => ((left as i64) >> shift) as u64,
        }
    }
}

#[derive(Clone, Copy)]
enum MaskCompareOp {
    Equal,
    NotEqual,
    LessUnsigned,
    LessSigned,
    LessEqualUnsigned,
    LessEqualSigned,
    GreaterUnsigned,
    GreaterSigned,
}

impl MaskCompareOp {
    fn apply(self, left: &[u8], right: &[u8]) -> bool {
        match self {
            Self::Equal => left == right,
            Self::NotEqual => left != right,
            Self::LessUnsigned => mask_lane_unsigned(left) < mask_lane_unsigned(right),
            Self::LessSigned => mask_lane_signed(left) < mask_lane_signed(right),
            Self::LessEqualUnsigned => mask_lane_unsigned(left) <= mask_lane_unsigned(right),
            Self::LessEqualSigned => mask_lane_signed(left) <= mask_lane_signed(right),
            Self::GreaterUnsigned => mask_lane_unsigned(left) > mask_lane_unsigned(right),
            Self::GreaterSigned => mask_lane_signed(left) > mask_lane_signed(right),
        }
    }
}

#[derive(Clone, Copy)]
enum MaskLogicalOp {
    And,
    Nand,
    AndNot,
    Xor,
    Or,
    Nor,
    OrNot,
    Xnor,
}

impl MaskLogicalOp {
    fn apply(self, left: bool, right: bool) -> bool {
        match self {
            Self::And => left & right,
            Self::Nand => !(left & right),
            Self::AndNot => left & !right,
            Self::Xor => left ^ right,
            Self::Or => left | right,
            Self::Nor => !(left | right),
            Self::OrNot => left | !right,
            Self::Xnor => !(left ^ right),
        }
    }
}

fn mask_lane_unsigned(bytes: &[u8]) -> u64 {
    match bytes.len() {
        1 => u64::from(bytes[0]),
        2 => u64::from(u16::from_le_bytes([bytes[0], bytes[1]])),
        4 => u64::from(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        8 => u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
        _ => unreachable!("validated vector element width"),
    }
}

fn mask_lane_signed(bytes: &[u8]) -> i64 {
    match bytes.len() {
        1 => i64::from(bytes[0] as i8),
        2 => i64::from(i16::from_le_bytes([bytes[0], bytes[1]])),
        4 => i64::from(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        8 => i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]),
        _ => unreachable!("validated vector element width"),
    }
}

fn multiply_high_unsigned(left: u128, right: u128, element_bits: u32) -> u128 {
    (left * right) >> element_bits
}

fn multiply_high_signed(left: i128, right: i128, element_bits: u32) -> u128 {
    ((left * right) >> element_bits) as u128
}

fn multiply_high_signed_unsigned(left: i128, right: u128, element_bits: u32) -> u128 {
    ((left * right as i128) >> element_bits) as u128
}

fn divide_unsigned(left: u128, right: u128, division_by_zero_result: u128) -> u128 {
    if right == 0 {
        division_by_zero_result
    } else {
        left / right
    }
}

fn divide_signed(left: i128, right: i128, min_value: i128) -> u128 {
    if right == 0 {
        u128::MAX
    } else if left == min_value && right == -1 {
        min_value as u128
    } else {
        (left / right) as u128
    }
}

fn remainder_unsigned(left: u128, right: u128) -> u128 {
    if right == 0 {
        left
    } else {
        left % right
    }
}

fn remainder_signed(left: i128, right: i128, min_value: i128) -> u128 {
    if right == 0 {
        left as u128
    } else if left == min_value && right == -1 {
        0
    } else {
        (left % right) as u128
    }
}

fn shift_amount(raw: u64, element_bits: u32) -> u32 {
    (raw & u64::from(element_bits - 1)) as u32
}

struct VectorMaskPlan {
    element_bytes: usize,
    group_registers: usize,
    active_elements: usize,
}

impl VectorMaskPlan {
    fn new(hart: &RiscvHartState, sources: &[VectorRegister]) -> Option<Self> {
        let config = hart.vector_config();
        let element_bytes = config.element_width_bytes()?;
        let group_registers = config.register_group_registers()?;
        if sources
            .iter()
            .any(|source| !valid_register_group(*source, group_registers))
        {
            return None;
        }

        let active_elements = config.vl() as usize;
        let active_bytes = active_elements.checked_mul(element_bytes)?;
        let active_mask_bytes = active_elements.div_ceil(8);
        if active_bytes > group_registers * RISCV_VECTOR_REGISTER_BYTES
            || active_mask_bytes > RISCV_VECTOR_REGISTER_BYTES
        {
            return None;
        }

        Some(Self {
            element_bytes,
            group_registers,
            active_elements,
        })
    }
}

struct VectorMaskLogicalPlan {
    active_elements: usize,
}

impl VectorMaskLogicalPlan {
    fn new(hart: &RiscvHartState) -> Option<Self> {
        let config = hart.vector_config();
        let _ = config.element_width_bytes()?;
        let vlmax = RiscvVectorConfig::vlmax(config.vtype())? as usize;
        let active_elements = config.vl() as usize;
        if active_elements > vlmax || active_elements.div_ceil(8) > RISCV_VECTOR_REGISTER_BYTES {
            return None;
        }

        Some(Self { active_elements })
    }
}

fn apply_vector_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    right: &[u8; MAX_VECTOR_GROUP_BYTES],
    operation: LaneBinaryOp,
) {
    for offset in (0..plan.active_bytes).step_by(plan.element_bytes) {
        apply_lane(
            &mut result[offset..offset + plan.element_bytes],
            &left[offset..offset + plan.element_bytes],
            &right[offset..offset + plan.element_bytes],
            operation,
        );
    }
}

fn apply_scalar_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
    operation: LaneBinaryOp,
) {
    for offset in (0..plan.active_bytes).step_by(plan.element_bytes) {
        apply_lane_scalar(
            &mut result[offset..offset + plan.element_bytes],
            &left[offset..offset + plan.element_bytes],
            scalar,
            operation,
        );
    }
}

fn apply_masked_vector_select_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    fallback: &[u8; MAX_VECTOR_GROUP_BYTES],
    selected: &[u8; MAX_VECTOR_GROUP_BYTES],
    mask: &[u8; RISCV_VECTOR_REGISTER_BYTES],
) {
    for element_index in 0..plan.active_element_count() {
        let offset = element_index * plan.element_bytes;
        let source = if read_mask_bit(mask, element_index) {
            selected
        } else {
            fallback
        };
        result[offset..offset + plan.element_bytes]
            .copy_from_slice(&source[offset..offset + plan.element_bytes]);
    }
}

fn apply_masked_scalar_select_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    fallback: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
    mask: &[u8; RISCV_VECTOR_REGISTER_BYTES],
) {
    let scalar_bytes = scalar.to_le_bytes();
    for element_index in 0..plan.active_element_count() {
        let offset = element_index * plan.element_bytes;
        if read_mask_bit(mask, element_index) {
            result[offset..offset + plan.element_bytes]
                .copy_from_slice(&scalar_bytes[..plan.element_bytes]);
        } else {
            result[offset..offset + plan.element_bytes]
                .copy_from_slice(&fallback[offset..offset + plan.element_bytes]);
        }
    }
}

fn apply_vector_move_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    source: &[u8; MAX_VECTOR_GROUP_BYTES],
) {
    for offset in (0..plan.active_bytes).step_by(plan.element_bytes) {
        result[offset..offset + plan.element_bytes]
            .copy_from_slice(&source[offset..offset + plan.element_bytes]);
    }
}

fn apply_scalar_move_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
) {
    let scalar_bytes = scalar.to_le_bytes();
    for offset in (0..plan.active_bytes).step_by(plan.element_bytes) {
        result[offset..offset + plan.element_bytes]
            .copy_from_slice(&scalar_bytes[..plan.element_bytes]);
    }
}

fn apply_vector_mask_lanes(
    plan: &VectorMaskPlan,
    mask: &mut [u8; RISCV_VECTOR_REGISTER_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    right: &[u8; MAX_VECTOR_GROUP_BYTES],
    operation: MaskCompareOp,
) {
    for element_index in 0..plan.active_elements {
        let offset = element_index * plan.element_bytes;
        let result = operation.apply(
            &left[offset..offset + plan.element_bytes],
            &right[offset..offset + plan.element_bytes],
        );
        write_mask_bit(mask, element_index, result);
    }
}

fn apply_scalar_mask_lanes(
    plan: &VectorMaskPlan,
    mask: &mut [u8; RISCV_VECTOR_REGISTER_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
    operation: MaskCompareOp,
) {
    let scalar_bytes = scalar.to_le_bytes();
    for element_index in 0..plan.active_elements {
        let offset = element_index * plan.element_bytes;
        let result = operation.apply(
            &left[offset..offset + plan.element_bytes],
            &scalar_bytes[..plan.element_bytes],
        );
        write_mask_bit(mask, element_index, result);
    }
}

fn apply_mask_logical_bits(
    plan: &VectorMaskLogicalPlan,
    result: &mut [u8; RISCV_VECTOR_REGISTER_BYTES],
    left: &[u8; RISCV_VECTOR_REGISTER_BYTES],
    right: &[u8; RISCV_VECTOR_REGISTER_BYTES],
    operation: MaskLogicalOp,
) {
    for element_index in 0..plan.active_elements {
        let result_bit = operation.apply(
            read_mask_bit(left, element_index),
            read_mask_bit(right, element_index),
        );
        write_mask_bit(result, element_index, result_bit);
    }
}

fn write_mask_bit(mask: &mut [u8; RISCV_VECTOR_REGISTER_BYTES], element_index: usize, value: bool) {
    let byte_index = element_index / 8;
    let bit = 1_u8 << (element_index % 8);
    if value {
        mask[byte_index] |= bit;
    } else {
        mask[byte_index] &= !bit;
    }
}

fn register_group_overlaps_v0(register: VectorRegister, group_registers: usize) -> bool {
    register.index() == 0 && group_registers > 0
}

fn apply_lane(result: &mut [u8], left: &[u8], right: &[u8], operation: LaneBinaryOp) {
    match result.len() {
        1 => result[0] = operation.apply_u8(left[0], right[0]),
        2 => result.copy_from_slice(
            &operation
                .apply_u16(
                    u16::from_le_bytes([left[0], left[1]]),
                    u16::from_le_bytes([right[0], right[1]]),
                )
                .to_le_bytes(),
        ),
        4 => result.copy_from_slice(
            &operation
                .apply_u32(
                    u32::from_le_bytes([left[0], left[1], left[2], left[3]]),
                    u32::from_le_bytes([right[0], right[1], right[2], right[3]]),
                )
                .to_le_bytes(),
        ),
        8 => result.copy_from_slice(
            &operation
                .apply_u64(
                    u64::from_le_bytes([
                        left[0], left[1], left[2], left[3], left[4], left[5], left[6], left[7],
                    ]),
                    u64::from_le_bytes([
                        right[0], right[1], right[2], right[3], right[4], right[5], right[6],
                        right[7],
                    ]),
                )
                .to_le_bytes(),
        ),
        _ => unreachable!("validated vector element width"),
    }
}

fn apply_lane_scalar(result: &mut [u8], left: &[u8], scalar: u64, operation: LaneBinaryOp) {
    match result.len() {
        1 => result[0] = operation.apply_u8(left[0], scalar as u8),
        2 => result.copy_from_slice(
            &operation
                .apply_u16(u16::from_le_bytes([left[0], left[1]]), scalar as u16)
                .to_le_bytes(),
        ),
        4 => result.copy_from_slice(
            &operation
                .apply_u32(
                    u32::from_le_bytes([left[0], left[1], left[2], left[3]]),
                    scalar as u32,
                )
                .to_le_bytes(),
        ),
        8 => result.copy_from_slice(
            &operation
                .apply_u64(
                    u64::from_le_bytes([
                        left[0], left[1], left[2], left[3], left[4], left[5], left[6], left[7],
                    ]),
                    scalar,
                )
                .to_le_bytes(),
        ),
        _ => unreachable!("validated vector element width"),
    }
}
