use crate::{
    vector_group::{
        lane_bytes_to_u128, read_mask_bit, read_register_group, register_groups_overlap,
        valid_register_group, write_register_group, write_u128_lane, MAX_VECTOR_GROUP_BYTES,
    },
    Register, RiscvHartState, RiscvInstruction, RiscvVectorMaskMode, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorWideningIntegerInstruction {
    AddUnsignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    AddSignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    SubUnsignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    SubSignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    MultiplyUnsignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    MultiplySignedUnsignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    MultiplySignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    AddUnsignedWv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    AddSignedWv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    SubUnsignedWv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    SubSignedWv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    AddUnsignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    AddSignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    SubUnsignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    SubSignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    MultiplyUnsignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    MultiplySignedUnsignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    MultiplySignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    AddUnsignedWx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    AddSignedWx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    SubUnsignedWx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    SubSignedWx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
}

#[derive(Clone, Copy)]
enum WideningIntegerOp {
    AddUnsigned,
    AddSigned,
    SubUnsigned,
    SubSigned,
    MultiplyUnsigned,
    MultiplySignedUnsigned,
    MultiplySigned,
}

impl RiscvVectorWideningIntegerInstruction {
    pub const fn add_unsigned_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddUnsignedVv { vd, vs2, vs1, mask }
    }

    pub const fn add_signed_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddSignedVv { vd, vs2, vs1, mask }
    }

    pub const fn sub_unsigned_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubUnsignedVv { vd, vs2, vs1, mask }
    }

    pub const fn sub_signed_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubSignedVv { vd, vs2, vs1, mask }
    }

    pub const fn multiply_unsigned_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplyUnsignedVv { vd, vs2, vs1, mask }
    }

    pub const fn multiply_signed_unsigned_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplySignedUnsignedVv { vd, vs2, vs1, mask }
    }

    pub const fn multiply_signed_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplySignedVv { vd, vs2, vs1, mask }
    }

    pub const fn add_unsigned_wv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddUnsignedWv { vd, vs2, vs1, mask }
    }

    pub const fn add_signed_wv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddSignedWv { vd, vs2, vs1, mask }
    }

    pub const fn sub_unsigned_wv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubUnsignedWv { vd, vs2, vs1, mask }
    }

    pub const fn sub_signed_wv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubSignedWv { vd, vs2, vs1, mask }
    }

    pub const fn add_unsigned_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddUnsignedVx { vd, vs2, rs1, mask }
    }

    pub const fn add_signed_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddSignedVx { vd, vs2, rs1, mask }
    }

    pub const fn sub_unsigned_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubUnsignedVx { vd, vs2, rs1, mask }
    }

    pub const fn sub_signed_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubSignedVx { vd, vs2, rs1, mask }
    }

    pub const fn multiply_unsigned_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplyUnsignedVx { vd, vs2, rs1, mask }
    }

    pub const fn multiply_signed_unsigned_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplySignedUnsignedVx { vd, vs2, rs1, mask }
    }

    pub const fn multiply_signed_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::MultiplySignedVx { vd, vs2, rs1, mask }
    }

    pub const fn add_unsigned_wx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddUnsignedWx { vd, vs2, rs1, mask }
    }

    pub const fn add_signed_wx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddSignedWx { vd, vs2, rs1, mask }
    }

    pub const fn sub_unsigned_wx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubUnsignedWx { vd, vs2, rs1, mask }
    }

    pub const fn sub_signed_wx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubSignedWx { vd, vs2, rs1, mask }
    }
}

pub(crate) fn decode_vv(raw: u32) -> RiscvInstruction {
    let mask = RiscvVectorMaskMode::from_vm_bit((raw & (1 << 25)) != 0);
    let build = match (raw >> 26) & 0x3f {
        0b110000 => RiscvVectorWideningIntegerInstruction::add_unsigned_vv,
        0b110001 => RiscvVectorWideningIntegerInstruction::add_signed_vv,
        0b110010 => RiscvVectorWideningIntegerInstruction::sub_unsigned_vv,
        0b110011 => RiscvVectorWideningIntegerInstruction::sub_signed_vv,
        0b111000 => RiscvVectorWideningIntegerInstruction::multiply_unsigned_vv,
        0b111010 => RiscvVectorWideningIntegerInstruction::multiply_signed_unsigned_vv,
        0b111011 => RiscvVectorWideningIntegerInstruction::multiply_signed_vv,
        0b110100 => RiscvVectorWideningIntegerInstruction::add_unsigned_wv,
        0b110101 => RiscvVectorWideningIntegerInstruction::add_signed_wv,
        0b110110 => RiscvVectorWideningIntegerInstruction::sub_unsigned_wv,
        0b110111 => RiscvVectorWideningIntegerInstruction::sub_signed_wv,
        _ => unreachable!("widening integer vector funct6 is range-checked by decode_vector"),
    };
    RiscvInstruction::VectorWideningInteger(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        vector_register(raw, 15),
        mask,
    ))
}

pub(crate) fn decode_vx(raw: u32) -> RiscvInstruction {
    let mask = RiscvVectorMaskMode::from_vm_bit((raw & (1 << 25)) != 0);
    let build = match (raw >> 26) & 0x3f {
        0b110000 => RiscvVectorWideningIntegerInstruction::add_unsigned_vx,
        0b110001 => RiscvVectorWideningIntegerInstruction::add_signed_vx,
        0b110010 => RiscvVectorWideningIntegerInstruction::sub_unsigned_vx,
        0b110011 => RiscvVectorWideningIntegerInstruction::sub_signed_vx,
        0b111000 => RiscvVectorWideningIntegerInstruction::multiply_unsigned_vx,
        0b111010 => RiscvVectorWideningIntegerInstruction::multiply_signed_unsigned_vx,
        0b111011 => RiscvVectorWideningIntegerInstruction::multiply_signed_vx,
        0b110100 => RiscvVectorWideningIntegerInstruction::add_unsigned_wx,
        0b110101 => RiscvVectorWideningIntegerInstruction::add_signed_wx,
        0b110110 => RiscvVectorWideningIntegerInstruction::sub_unsigned_wx,
        0b110111 => RiscvVectorWideningIntegerInstruction::sub_signed_wx,
        _ => unreachable!("widening integer scalar funct6 is range-checked by decode_vector"),
    };
    RiscvInstruction::VectorWideningInteger(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        Register::from_field((raw >> 15) & 0x1f),
        mask,
    ))
}

fn vector_register(raw: u32, shift: u32) -> VectorRegister {
    VectorRegister::from_field((raw >> shift) & 0x1f)
}

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorWideningIntegerInstruction,
) -> bool {
    match instruction {
        RiscvVectorWideningIntegerInstruction::AddUnsignedVv { vd, vs2, vs1, mask } => {
            execute_vv(hart, vd, vs2, vs1, mask, WideningIntegerOp::AddUnsigned)
        }
        RiscvVectorWideningIntegerInstruction::AddSignedVv { vd, vs2, vs1, mask } => {
            execute_vv(hart, vd, vs2, vs1, mask, WideningIntegerOp::AddSigned)
        }
        RiscvVectorWideningIntegerInstruction::SubUnsignedVv { vd, vs2, vs1, mask } => {
            execute_vv(hart, vd, vs2, vs1, mask, WideningIntegerOp::SubUnsigned)
        }
        RiscvVectorWideningIntegerInstruction::SubSignedVv { vd, vs2, vs1, mask } => {
            execute_vv(hart, vd, vs2, vs1, mask, WideningIntegerOp::SubSigned)
        }
        RiscvVectorWideningIntegerInstruction::MultiplyUnsignedVv { vd, vs2, vs1, mask } => {
            execute_vv(
                hart,
                vd,
                vs2,
                vs1,
                mask,
                WideningIntegerOp::MultiplyUnsigned,
            )
        }
        RiscvVectorWideningIntegerInstruction::MultiplySignedUnsignedVv { vd, vs2, vs1, mask } => {
            execute_vv(
                hart,
                vd,
                vs2,
                vs1,
                mask,
                WideningIntegerOp::MultiplySignedUnsigned,
            )
        }
        RiscvVectorWideningIntegerInstruction::MultiplySignedVv { vd, vs2, vs1, mask } => {
            execute_vv(hart, vd, vs2, vs1, mask, WideningIntegerOp::MultiplySigned)
        }
        RiscvVectorWideningIntegerInstruction::AddUnsignedWv { vd, vs2, vs1, mask } => {
            execute_wv(hart, vd, vs2, vs1, mask, WideningIntegerOp::AddUnsigned)
        }
        RiscvVectorWideningIntegerInstruction::AddSignedWv { vd, vs2, vs1, mask } => {
            execute_wv(hart, vd, vs2, vs1, mask, WideningIntegerOp::AddSigned)
        }
        RiscvVectorWideningIntegerInstruction::SubUnsignedWv { vd, vs2, vs1, mask } => {
            execute_wv(hart, vd, vs2, vs1, mask, WideningIntegerOp::SubUnsigned)
        }
        RiscvVectorWideningIntegerInstruction::SubSignedWv { vd, vs2, vs1, mask } => {
            execute_wv(hart, vd, vs2, vs1, mask, WideningIntegerOp::SubSigned)
        }
        RiscvVectorWideningIntegerInstruction::AddUnsignedVx { vd, vs2, rs1, mask } => execute_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            WideningIntegerOp::AddUnsigned,
        ),
        RiscvVectorWideningIntegerInstruction::AddSignedVx { vd, vs2, rs1, mask } => execute_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            WideningIntegerOp::AddSigned,
        ),
        RiscvVectorWideningIntegerInstruction::SubUnsignedVx { vd, vs2, rs1, mask } => execute_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            WideningIntegerOp::SubUnsigned,
        ),
        RiscvVectorWideningIntegerInstruction::SubSignedVx { vd, vs2, rs1, mask } => execute_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            WideningIntegerOp::SubSigned,
        ),
        RiscvVectorWideningIntegerInstruction::MultiplyUnsignedVx { vd, vs2, rs1, mask } => {
            execute_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                mask,
                WideningIntegerOp::MultiplyUnsigned,
            )
        }
        RiscvVectorWideningIntegerInstruction::MultiplySignedUnsignedVx { vd, vs2, rs1, mask } => {
            execute_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                mask,
                WideningIntegerOp::MultiplySignedUnsigned,
            )
        }
        RiscvVectorWideningIntegerInstruction::MultiplySignedVx { vd, vs2, rs1, mask } => {
            execute_vx(
                hart,
                vd,
                vs2,
                hart.read(rs1),
                mask,
                WideningIntegerOp::MultiplySigned,
            )
        }
        RiscvVectorWideningIntegerInstruction::AddUnsignedWx { vd, vs2, rs1, mask } => execute_wx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            WideningIntegerOp::AddUnsigned,
        ),
        RiscvVectorWideningIntegerInstruction::AddSignedWx { vd, vs2, rs1, mask } => execute_wx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            WideningIntegerOp::AddSigned,
        ),
        RiscvVectorWideningIntegerInstruction::SubUnsignedWx { vd, vs2, rs1, mask } => execute_wx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            WideningIntegerOp::SubUnsigned,
        ),
        RiscvVectorWideningIntegerInstruction::SubSignedWx { vd, vs2, rs1, mask } => execute_wx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            mask,
            WideningIntegerOp::SubSigned,
        ),
    }
}

fn execute_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
    mask: RiscvVectorMaskMode,
    op: WideningIntegerOp,
) -> bool {
    let Some(plan) = WideningIntegerPlan::new(hart, vd, &[], &[vs2, vs1], mask) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.narrow_registers);
    let right = read_register_group(hart, vs1, plan.narrow_registers);
    execute_lanes(
        hart,
        vd,
        &plan,
        &left,
        plan.narrow_element_bytes,
        |element_index| {
            let offset = element_index * plan.narrow_element_bytes;
            lane_bytes_to_u128(&right[offset..offset + plan.narrow_element_bytes])
        },
        op,
    )
}

fn execute_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    mask: RiscvVectorMaskMode,
    op: WideningIntegerOp,
) -> bool {
    let Some(plan) = WideningIntegerPlan::new(hart, vd, &[], &[vs2], mask) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.narrow_registers);
    execute_lanes(
        hart,
        vd,
        &plan,
        &left,
        plan.narrow_element_bytes,
        |_| u128::from(scalar) & unsigned_max(plan.narrow_element_bits),
        op,
    )
}

fn execute_wv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
    mask: RiscvVectorMaskMode,
    op: WideningIntegerOp,
) -> bool {
    let Some(plan) = WideningIntegerPlan::new(hart, vd, &[vs2], &[vs1], mask) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.wide_registers);
    let right = read_register_group(hart, vs1, plan.narrow_registers);
    execute_lanes(
        hart,
        vd,
        &plan,
        &left,
        plan.wide_element_bytes,
        |element_index| {
            let offset = element_index * plan.narrow_element_bytes;
            lane_bytes_to_u128(&right[offset..offset + plan.narrow_element_bytes])
        },
        op,
    )
}

fn execute_wx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    mask: RiscvVectorMaskMode,
    op: WideningIntegerOp,
) -> bool {
    let Some(plan) = WideningIntegerPlan::new(hart, vd, &[vs2], &[], mask) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.wide_registers);
    execute_lanes(
        hart,
        vd,
        &plan,
        &left,
        plan.wide_element_bytes,
        |_| u128::from(scalar) & unsigned_max(plan.narrow_element_bits),
        op,
    )
}

fn execute_lanes(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    plan: &WideningIntegerPlan,
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    left_element_bytes: usize,
    right_lane: impl Fn(usize) -> u128,
    op: WideningIntegerOp,
) -> bool {
    let selector = plan
        .mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let mut result = read_register_group(hart, vd, plan.wide_registers);

    for element_index in 0..plan.active_elements {
        if selector
            .as_ref()
            .is_some_and(|mask| !read_mask_bit(mask, element_index))
        {
            continue;
        }
        let source_offset = element_index * left_element_bytes;
        let destination_offset = element_index * plan.wide_element_bytes;
        let left = lane_bytes_to_u128(&left[source_offset..source_offset + left_element_bytes]);
        let right = right_lane(element_index);
        let value = apply_widening_integer(
            op,
            left,
            left_element_bytes * 8,
            right,
            plan.narrow_element_bits,
            plan.wide_element_bits,
        );
        write_u128_lane(
            &mut result[destination_offset..destination_offset + plan.wide_element_bytes],
            value,
        );
    }

    write_register_group(hart, vd, plan.wide_registers, &result);
    true
}

fn apply_widening_integer(
    op: WideningIntegerOp,
    left: u128,
    left_bits: usize,
    right: u128,
    right_bits: usize,
    result_bits: usize,
) -> u128 {
    let mask = unsigned_max(result_bits);
    let (left_signed, right_signed) = widening_operand_signedness(op);
    let left = extend_operand(left, left_bits, result_bits, left_signed);
    let right = extend_operand(right, right_bits, result_bits, right_signed);
    match op {
        WideningIntegerOp::AddUnsigned | WideningIntegerOp::AddSigned => {
            left.wrapping_add(right) & mask
        }
        WideningIntegerOp::SubUnsigned | WideningIntegerOp::SubSigned => {
            left.wrapping_sub(right) & mask
        }
        WideningIntegerOp::MultiplyUnsigned
        | WideningIntegerOp::MultiplySignedUnsigned
        | WideningIntegerOp::MultiplySigned => left.wrapping_mul(right) & mask,
    }
}

fn widening_operand_signedness(op: WideningIntegerOp) -> (bool, bool) {
    match op {
        WideningIntegerOp::AddSigned
        | WideningIntegerOp::SubSigned
        | WideningIntegerOp::MultiplySigned => (true, true),
        WideningIntegerOp::MultiplySignedUnsigned => (true, false),
        WideningIntegerOp::AddUnsigned
        | WideningIntegerOp::SubUnsigned
        | WideningIntegerOp::MultiplyUnsigned => (false, false),
    }
}

struct WideningIntegerPlan {
    narrow_element_bytes: usize,
    narrow_element_bits: usize,
    wide_element_bytes: usize,
    wide_element_bits: usize,
    narrow_registers: usize,
    wide_registers: usize,
    active_elements: usize,
    mask: RiscvVectorMaskMode,
}

impl WideningIntegerPlan {
    fn new(
        hart: &RiscvHartState,
        vd: VectorRegister,
        wide_sources: &[VectorRegister],
        narrow_sources: &[VectorRegister],
        mask: RiscvVectorMaskMode,
    ) -> Option<Self> {
        let config = hart.vector_config();
        let narrow_element_bytes = config.element_width_bytes()?;
        let wide_element_bytes = narrow_element_bytes.checked_mul(2)?;
        if wide_element_bytes > 16 {
            return None;
        }
        let narrow_registers = config.register_group_registers()?;
        let wide_registers = widening_registers(config.vtype())?;
        if !valid_register_group(vd, wide_registers)
            || wide_sources
                .iter()
                .any(|source| !valid_register_group(*source, wide_registers))
            || wide_sources.iter().any(|wide| {
                narrow_sources.iter().any(|narrow| {
                    register_groups_overlap(*wide, wide_registers, *narrow, narrow_registers)
                })
            })
            || narrow_sources.iter().any(|source| {
                !valid_widening_narrow_source(
                    config.vtype(),
                    vd,
                    wide_registers,
                    *source,
                    narrow_registers,
                )
            })
            || (mask.is_masked()
                && (register_groups_overlap(vd, wide_registers, VectorRegister::from_field(0), 1)
                    || sources_overlap_mask(
                        wide_sources,
                        wide_registers,
                        narrow_sources,
                        narrow_registers,
                    )))
        {
            return None;
        }

        let active_elements = config.vl() as usize;
        let narrow_active_bytes = active_elements.checked_mul(narrow_element_bytes)?;
        let wide_active_bytes = active_elements.checked_mul(wide_element_bytes)?;
        if narrow_active_bytes > narrow_registers * RISCV_VECTOR_REGISTER_BYTES
            || wide_active_bytes > wide_registers * RISCV_VECTOR_REGISTER_BYTES
        {
            return None;
        }

        Some(Self {
            narrow_element_bytes,
            narrow_element_bits: narrow_element_bytes * 8,
            wide_element_bytes,
            wide_element_bits: wide_element_bytes * 8,
            narrow_registers,
            wide_registers,
            active_elements,
            mask,
        })
    }
}

fn widening_registers(vtype: u64) -> Option<usize> {
    match vtype & 0x7 {
        0 => Some(2),
        1 => Some(4),
        2 => Some(8),
        3 | 4 => None,
        5..=7 => Some(1),
        _ => unreachable!(),
    }
}

fn valid_widening_narrow_source(
    vtype: u64,
    vd: VectorRegister,
    destination_registers: usize,
    source: VectorRegister,
    source_registers: usize,
) -> bool {
    if !valid_register_group(source, source_registers) {
        return false;
    }
    if !register_groups_overlap(vd, destination_registers, source, source_registers) {
        return true;
    }
    source_emul_at_least_one(vtype)
        && source.index() as usize == vd.index() as usize + destination_registers - source_registers
}

fn source_emul_at_least_one(vtype: u64) -> bool {
    matches!(vtype & 0x7, 0..=3)
}

fn sources_overlap_mask(
    wide_sources: &[VectorRegister],
    wide_registers: usize,
    narrow_sources: &[VectorRegister],
    narrow_registers: usize,
) -> bool {
    let mask = VectorRegister::from_field(0);
    wide_sources
        .iter()
        .any(|source| register_groups_overlap(*source, wide_registers, mask, 1))
        || narrow_sources
            .iter()
            .any(|source| register_groups_overlap(*source, narrow_registers, mask, 1))
}

fn extend_operand(value: u128, operand_bits: usize, result_bits: usize, signed: bool) -> u128 {
    let result_mask = unsigned_max(result_bits);
    let operand_mask = unsigned_max(operand_bits);
    let value = value & operand_mask;
    if signed && operand_bits < result_bits && (value & (1_u128 << (operand_bits - 1))) != 0 {
        value | (!operand_mask & result_mask)
    } else {
        value
    }
}

fn unsigned_max(bits: usize) -> u128 {
    if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    }
}
