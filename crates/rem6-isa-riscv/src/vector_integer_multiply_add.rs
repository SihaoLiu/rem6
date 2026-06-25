use crate::{
    vector_group::{
        lane_bytes_to_u128, read_mask_bit, read_register_group, write_register_group,
        write_u128_lane, VectorBinaryPlan, MAX_VECTOR_GROUP_BYTES,
    },
    Register, RiscvHartState, RiscvInstruction, RiscvVectorMaskMode, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorIntegerMultiplyAddInstruction {
    MultiplyAddVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    NegativeMultiplySubVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    MultiplyAccumulateVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    NegativeMultiplyAccumulateVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    MultiplyAddVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    NegativeMultiplySubVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    MultiplyAccumulateVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    NegativeMultiplyAccumulateVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
}

#[derive(Clone, Copy)]
enum MulAddMode {
    OverwriteMultiplicand,
    NegativeOverwriteMultiplicand,
    OverwriteAccumulator,
    NegativeOverwriteAccumulator,
}

impl RiscvVectorIntegerMultiplyAddInstruction {
    pub const fn multiply_add_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplyAddVv { vd, vs2, vs1, mask }
    }

    pub const fn negative_multiply_sub_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::NegativeMultiplySubVv { vd, vs2, vs1, mask }
    }

    pub const fn multiply_accumulate_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplyAccumulateVv { vd, vs2, vs1, mask }
    }

    pub const fn negative_multiply_accumulate_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::NegativeMultiplyAccumulateVv { vd, vs2, vs1, mask }
    }

    pub const fn multiply_add_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplyAddVx { vd, vs2, rs1, mask }
    }

    pub const fn negative_multiply_sub_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::NegativeMultiplySubVx { vd, vs2, rs1, mask }
    }

    pub const fn multiply_accumulate_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplyAccumulateVx { vd, vs2, rs1, mask }
    }

    pub const fn negative_multiply_accumulate_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::NegativeMultiplyAccumulateVx { vd, vs2, rs1, mask }
    }
}

pub(crate) fn decode_vv(raw: u32) -> RiscvInstruction {
    let mask = RiscvVectorMaskMode::from_vm_bit((raw & (1 << 25)) != 0);
    let build = match (raw >> 26) & 0x3f {
        0b101001 => RiscvVectorIntegerMultiplyAddInstruction::multiply_add_vv,
        0b101011 => RiscvVectorIntegerMultiplyAddInstruction::negative_multiply_sub_vv,
        0b101101 => RiscvVectorIntegerMultiplyAddInstruction::multiply_accumulate_vv,
        0b101111 => RiscvVectorIntegerMultiplyAddInstruction::negative_multiply_accumulate_vv,
        _ => unreachable!("integer multiply-add vector funct6 is range-checked by decode_vector"),
    };
    RiscvInstruction::VectorIntegerMultiplyAdd(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        vector_register(raw, 15),
        mask,
    ))
}

pub(crate) const fn is_vv_funct6(funct6: u32) -> bool {
    matches!(funct6, 0b101001 | 0b101011 | 0b101101 | 0b101111)
}

pub(crate) fn decode_vx(raw: u32) -> RiscvInstruction {
    let mask = RiscvVectorMaskMode::from_vm_bit((raw & (1 << 25)) != 0);
    let build = match (raw >> 26) & 0x3f {
        0b101001 => RiscvVectorIntegerMultiplyAddInstruction::multiply_add_vx,
        0b101011 => RiscvVectorIntegerMultiplyAddInstruction::negative_multiply_sub_vx,
        0b101101 => RiscvVectorIntegerMultiplyAddInstruction::multiply_accumulate_vx,
        0b101111 => RiscvVectorIntegerMultiplyAddInstruction::negative_multiply_accumulate_vx,
        _ => unreachable!("integer multiply-add scalar funct6 is range-checked by decode_vector"),
    };
    RiscvInstruction::VectorIntegerMultiplyAdd(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        Register::from_field((raw >> 15) & 0x1f),
        mask,
    ))
}

pub(crate) const fn is_vx_funct6(funct6: u32) -> bool {
    is_vv_funct6(funct6)
}

fn vector_register(raw: u32, shift: u32) -> VectorRegister {
    VectorRegister::from_field((raw >> shift) & 0x1f)
}

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorIntegerMultiplyAddInstruction,
) -> bool {
    match instruction {
        RiscvVectorIntegerMultiplyAddInstruction::MultiplyAddVv { vd, vs2, vs1, mask } => {
            execute_vv(hart, vd, vs2, vs1, mask, MulAddMode::OverwriteMultiplicand)
        }
        RiscvVectorIntegerMultiplyAddInstruction::NegativeMultiplySubVv { vd, vs2, vs1, mask } => {
            execute_vv(
                hart,
                vd,
                vs2,
                vs1,
                mask,
                MulAddMode::NegativeOverwriteMultiplicand,
            )
        }
        RiscvVectorIntegerMultiplyAddInstruction::MultiplyAccumulateVv { vd, vs2, vs1, mask } => {
            execute_vv(hart, vd, vs2, vs1, mask, MulAddMode::OverwriteAccumulator)
        }
        RiscvVectorIntegerMultiplyAddInstruction::NegativeMultiplyAccumulateVv {
            vd,
            vs2,
            vs1,
            mask,
        } => execute_vv(
            hart,
            vd,
            vs2,
            vs1,
            mask,
            MulAddMode::NegativeOverwriteAccumulator,
        ),
        RiscvVectorIntegerMultiplyAddInstruction::MultiplyAddVx { vd, vs2, rs1, mask } => {
            execute_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                mask,
                MulAddMode::OverwriteMultiplicand,
            )
        }
        RiscvVectorIntegerMultiplyAddInstruction::NegativeMultiplySubVx { vd, vs2, rs1, mask } => {
            execute_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                mask,
                MulAddMode::NegativeOverwriteMultiplicand,
            )
        }
        RiscvVectorIntegerMultiplyAddInstruction::MultiplyAccumulateVx { vd, vs2, rs1, mask } => {
            execute_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                mask,
                MulAddMode::OverwriteAccumulator,
            )
        }
        RiscvVectorIntegerMultiplyAddInstruction::NegativeMultiplyAccumulateVx {
            vd,
            vs2,
            rs1,
            mask,
        } => execute_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            MulAddMode::NegativeOverwriteAccumulator,
        ),
    }
}

fn execute_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
    mask: RiscvVectorMaskMode,
    mode: MulAddMode,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    if mask.is_masked() && vd.index() == 0 {
        return false;
    }
    let mask = mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let vs2_bytes = read_register_group(hart, vs2, plan.group_registers);
    let vs1_bytes = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_vv_lanes(
        &plan,
        &mut result,
        &vs2_bytes,
        &vs1_bytes,
        mask.as_ref(),
        mode,
    );
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    mask: RiscvVectorMaskMode,
    mode: MulAddMode,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if mask.is_masked() && vd.index() == 0 {
        return false;
    }
    let mask = mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let vs2_bytes = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_vx_lanes(&plan, &mut result, &vs2_bytes, scalar, mask.as_ref(), mode);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn apply_vv_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    vs2: &[u8; MAX_VECTOR_GROUP_BYTES],
    vs1: &[u8; MAX_VECTOR_GROUP_BYTES],
    mask: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
    mode: MulAddMode,
) {
    for element_index in 0..plan.active_element_count() {
        if mask.is_some_and(|mask| !read_mask_bit(mask, element_index)) {
            continue;
        }
        let offset = element_index * plan.element_bytes;
        let range = offset..offset + plan.element_bytes;
        let vd_old = lane_bytes_to_u128(&result[range.clone()]);
        let vs2_lane = lane_bytes_to_u128(&vs2[range.clone()]);
        let vs1_lane = lane_bytes_to_u128(&vs1[range.clone()]);
        let value = mode.apply(vs1_lane, vs2_lane, vd_old, plan.element_bytes);
        write_u128_lane(&mut result[range], value);
    }
}

fn apply_vx_lanes(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    vs2: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
    mask: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
    mode: MulAddMode,
) {
    let rs1 = u128::from(scalar) & element_mask(plan.element_bytes);
    for element_index in 0..plan.active_element_count() {
        if mask.is_some_and(|mask| !read_mask_bit(mask, element_index)) {
            continue;
        }
        let offset = element_index * plan.element_bytes;
        let range = offset..offset + plan.element_bytes;
        let vd_old = lane_bytes_to_u128(&result[range.clone()]);
        let vs2_lane = lane_bytes_to_u128(&vs2[range.clone()]);
        let value = mode.apply(rs1, vs2_lane, vd_old, plan.element_bytes);
        write_u128_lane(&mut result[range], value);
    }
}

impl MulAddMode {
    fn apply(self, vs1_or_rs1: u128, vs2: u128, vd_old: u128, element_bytes: usize) -> u128 {
        let mask = element_mask(element_bytes);
        let (multiplicand, addend, subtract) = match self {
            Self::OverwriteMultiplicand => (vd_old, vs2, false),
            Self::NegativeOverwriteMultiplicand => (vd_old, vs2, true),
            Self::OverwriteAccumulator => (vs2, vd_old, false),
            Self::NegativeOverwriteAccumulator => (vs2, vd_old, true),
        };
        let product = vs1_or_rs1.wrapping_mul(multiplicand) & mask;
        if subtract {
            addend.wrapping_sub(product) & mask
        } else {
            addend.wrapping_add(product) & mask
        }
    }
}

fn element_mask(element_bytes: usize) -> u128 {
    match element_bytes {
        1 => u128::from(u8::MAX),
        2 => u128::from(u16::MAX),
        4 => u128::from(u32::MAX),
        8 => u128::from(u64::MAX),
        _ => unreachable!("validated vector element width"),
    }
}
