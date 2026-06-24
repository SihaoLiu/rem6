use crate::{
    vector_group::{
        read_mask_bit, read_register_group, register_groups_overlap, valid_register_group,
        write_register_group, VectorBinaryPlan, MAX_VECTOR_GROUP_BYTES,
    },
    vector_lane_op::LaneBinaryOp,
    RiscvHartState, RiscvInstruction, RiscvVectorConfig, RiscvVectorExtensionFactor,
    RiscvVectorMaskMode, RiscvVectorWholeMoveInstruction, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn execute_vector_integer_binary(
    hart: &mut RiscvHartState,
    instruction: RiscvInstruction,
) -> bool {
    match instruction {
        RiscvInstruction::VectorAddVv { vd, vs1, vs2, mask } => {
            execute_vector_binary_vv_with_mask(hart, vd, vs1, vs2, mask, LaneBinaryOp::Add)
        }
        RiscvInstruction::VectorAddVx { vd, vs2, rs1, mask } => execute_vector_binary_vx_with_mask(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            LaneBinaryOp::Add,
        ),
        RiscvInstruction::VectorAddVi { vd, vs2, imm, mask } => {
            execute_vector_binary_vi_with_mask(hart, vd, vs2, imm, mask, LaneBinaryOp::Add)
        }
        RiscvInstruction::VectorSubVv { vd, vs1, vs2 } => {
            execute_vector_binary_vv(hart, vd, vs1, vs2, LaneBinaryOp::Sub)
        }
        RiscvInstruction::VectorSubVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::Sub)
        }
        RiscvInstruction::VectorReverseSubVx { vd, vs2, rs1 } => {
            execute_vector_binary_vx(hart, vd, vs2, hart.read(rs1), LaneBinaryOp::ReverseSub)
        }
        RiscvInstruction::VectorReverseSubVi { vd, vs2, imm } => {
            execute_vector_binary_vi(hart, vd, vs2, imm, LaneBinaryOp::ReverseSub)
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
        RiscvInstruction::VectorSlide(instruction) => {
            crate::vector_slide_execute::execute(hart, instruction)
        }
        RiscvInstruction::VectorGather(instruction) => {
            crate::vector_gather_execute::execute(hart, instruction)
        }
        RiscvInstruction::VectorMaskPrefix(instruction) => {
            crate::vector_mask_prefix_execute::execute(hart, instruction)
        }
        RiscvInstruction::VectorMaskIndex(instruction) => {
            crate::vector_mask_index_execute::execute(hart, instruction)
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
        RiscvInstruction::VectorZeroExtend {
            vd,
            vs2,
            factor,
            mask,
        } => execute_vector_extend(hart, vd, vs2, factor, mask, ExtensionSignedness::Unsigned),
        RiscvInstruction::VectorSignExtend {
            vd,
            vs2,
            factor,
            mask,
        } => execute_vector_extend(hart, vd, vs2, factor, mask, ExtensionSignedness::Signed),
        RiscvInstruction::VectorMoveVv { vd, vs1 } => execute_vector_move_vv(hart, vd, vs1),
        RiscvInstruction::VectorMoveVx { vd, rs1 } => {
            execute_vector_move_vx(hart, vd, hart.read(rs1))
        }
        RiscvInstruction::VectorMoveVi { vd, imm } => execute_vector_move_vi(hart, vd, imm),
        RiscvInstruction::VectorWholeMove(instruction) => {
            execute_vector_whole_move(hart, instruction)
        }
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

#[derive(Clone, Copy)]
enum ExtensionSignedness {
    Unsigned,
    Signed,
}

fn execute_vector_extend(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    factor: RiscvVectorExtensionFactor,
    mask: RiscvVectorMaskMode,
    signedness: ExtensionSignedness,
) -> bool {
    let config = hart.vector_config();
    let Some(element_bytes) = config.element_width_bytes() else {
        return false;
    };
    let source_element_bytes = element_bytes / factor.divisor();
    if source_element_bytes == 0 {
        return false;
    }
    let Some(destination_group_registers) = config.register_group_registers() else {
        return false;
    };
    let source_group_registers =
        source_register_group_registers(destination_group_registers, factor);
    if !valid_register_group(vd, destination_group_registers)
        || !valid_register_group(vs2, source_group_registers)
        || (mask.is_masked() && register_group_overlaps_v0(vd, destination_group_registers))
        || !extension_overlap_allowed(
            vd,
            destination_group_registers,
            vs2,
            source_group_registers,
            factor,
        )
    {
        return false;
    }

    let active_elements = config.vl() as usize;
    let Some(destination_active_bytes) = active_elements.checked_mul(element_bytes) else {
        return false;
    };
    let Some(source_active_bytes) = active_elements.checked_mul(source_element_bytes) else {
        return false;
    };
    if destination_active_bytes > destination_group_registers * RISCV_VECTOR_REGISTER_BYTES
        || source_active_bytes > source_group_registers * RISCV_VECTOR_REGISTER_BYTES
    {
        return false;
    }

    let mask = mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let source = read_register_group(hart, vs2, source_group_registers);
    let mut result = read_register_group(hart, vd, destination_group_registers);
    for element in 0..active_elements {
        if !mask
            .as_ref()
            .is_none_or(|mask| read_mask_bit(mask, element))
        {
            continue;
        }
        let source_offset = element * source_element_bytes;
        let destination_offset = element * element_bytes;
        let value = extend_lane(
            &source[source_offset..source_offset + source_element_bytes],
            element_bytes,
            signedness,
        );
        result[destination_offset..destination_offset + element_bytes]
            .copy_from_slice(&value.to_le_bytes()[..element_bytes]);
    }
    write_register_group(hart, vd, destination_group_registers, &result);
    true
}

fn extension_overlap_allowed(
    vd: VectorRegister,
    destination_group_registers: usize,
    vs2: VectorRegister,
    source_group_registers: usize,
    factor: RiscvVectorExtensionFactor,
) -> bool {
    if !register_groups_overlap(vd, destination_group_registers, vs2, source_group_registers) {
        return true;
    }
    if destination_group_registers < factor.divisor() {
        return false;
    }

    vs2.index() as usize
        == vd.index() as usize + destination_group_registers - source_group_registers
}

fn source_register_group_registers(
    destination_group_registers: usize,
    factor: RiscvVectorExtensionFactor,
) -> usize {
    destination_group_registers
        .checked_div(factor.divisor())
        .filter(|registers| *registers != 0)
        .unwrap_or(1)
}

fn extend_lane(bytes: &[u8], element_bytes: usize, signedness: ExtensionSignedness) -> u64 {
    match signedness {
        ExtensionSignedness::Unsigned => mask_lane_unsigned(bytes),
        ExtensionSignedness::Signed => sign_extend_lane(bytes, element_bytes),
    }
}

fn sign_extend_lane(bytes: &[u8], element_bytes: usize) -> u64 {
    let sign_extended = match bytes.len() {
        1 => i64::from(bytes[0] as i8),
        2 => i64::from(i16::from_le_bytes([bytes[0], bytes[1]])),
        4 => i64::from(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        _ => unreachable!("validated vector extension source width"),
    } as u64;
    sign_extended & destination_element_mask(element_bytes)
}

fn destination_element_mask(element_bytes: usize) -> u64 {
    match element_bytes {
        1 => u64::from(u8::MAX),
        2 => u64::from(u16::MAX),
        4 => u64::from(u32::MAX),
        8 => u64::MAX,
        _ => unreachable!("validated vector element width"),
    }
}

fn execute_vector_binary_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
    operation: LaneBinaryOp,
) -> bool {
    execute_vector_binary_vi_with_mask(hart, vd, vs2, imm, RiscvVectorMaskMode::Unmasked, operation)
}

fn execute_vector_binary_vi_with_mask(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
    mask: RiscvVectorMaskMode,
    operation: LaneBinaryOp,
) -> bool {
    execute_vector_binary_vx_with_mask(hart, vd, vs2, imm as i64 as u64, mask, operation)
}

fn execute_vector_binary_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    operation: LaneBinaryOp,
) -> bool {
    execute_vector_binary_vv_with_mask(hart, vd, vs1, vs2, RiscvVectorMaskMode::Unmasked, operation)
}

fn execute_vector_binary_vv_with_mask(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs1: VectorRegister,
    vs2: VectorRegister,
    mask: RiscvVectorMaskMode,
    operation: LaneBinaryOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    if mask.is_masked() && register_group_overlaps_v0(vd, plan.group_registers) {
        return false;
    }
    let mask = mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_vector_lanes_with_mask(&plan, &mut result, &left, &right, mask.as_ref(), operation);
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
    execute_vector_binary_vx_with_mask(
        hart,
        vd,
        vs2,
        scalar,
        RiscvVectorMaskMode::Unmasked,
        operation,
    )
}

fn execute_vector_binary_vx_with_mask(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    mask: RiscvVectorMaskMode,
    operation: LaneBinaryOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if mask.is_masked() && register_group_overlaps_v0(vd, plan.group_registers) {
        return false;
    }
    let mask = mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let left = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_scalar_lanes_with_mask(&plan, &mut result, &left, scalar, mask.as_ref(), operation);
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

fn execute_vector_whole_move(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorWholeMoveInstruction,
) -> bool {
    let register_count = usize::from(instruction.register_count());
    if !matches!(register_count, 1 | 2 | 4 | 8)
        || !valid_register_group(instruction.vd(), register_count)
        || !valid_register_group(instruction.vs2(), register_count)
    {
        return false;
    }

    let source = read_register_group(hart, instruction.vs2(), register_count);
    write_register_group(hart, instruction.vd(), register_count, &source);
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

fn apply_vector_lanes_with_mask(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    right: &[u8; MAX_VECTOR_GROUP_BYTES],
    mask: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
    operation: LaneBinaryOp,
) {
    for element_index in 0..plan.active_element_count() {
        if mask.is_some_and(|mask| !read_mask_bit(mask, element_index)) {
            continue;
        }
        let offset = element_index * plan.element_bytes;
        apply_lane(
            &mut result[offset..offset + plan.element_bytes],
            &left[offset..offset + plan.element_bytes],
            &right[offset..offset + plan.element_bytes],
            operation,
        );
    }
}

fn apply_scalar_lanes_with_mask(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
    mask: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
    operation: LaneBinaryOp,
) {
    for element_index in 0..plan.active_element_count() {
        if mask.is_some_and(|mask| !read_mask_bit(mask, element_index)) {
            continue;
        }
        let offset = element_index * plan.element_bytes;
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
