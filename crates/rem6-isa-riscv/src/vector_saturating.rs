use crate::{
    vector::RiscvVectorFixedRoundingMode,
    vector_fixed_point_shift::round_signed,
    vector_group::{
        lane_bytes_to_u128, read_register_group, write_register_group, write_u128_lane,
        VectorBinaryPlan,
    },
    Register, RiscvHartState, RiscvInstruction, VectorRegister,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorSaturatingInstruction {
    AddUnsignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    },
    AddSignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    },
    SubUnsignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    },
    SubSignedVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    },
    AddUnsignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    AddSignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    SubUnsignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    SubSignedVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    MulSignedFractionalVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    },
    MulSignedFractionalVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    AddUnsignedVi {
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
    },
    AddSignedVi {
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
    },
}

#[derive(Clone, Copy)]
enum SaturatingOp {
    AddUnsigned,
    AddSigned,
    SubUnsigned,
    SubSigned,
    MulSignedFractional,
}

impl RiscvVectorSaturatingInstruction {
    pub const fn add_unsigned_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::AddUnsignedVv { vd, vs2, vs1 }
    }

    pub const fn add_signed_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::AddSignedVv { vd, vs2, vs1 }
    }

    pub const fn sub_unsigned_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::SubUnsignedVv { vd, vs2, vs1 }
    }

    pub const fn sub_signed_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::SubSignedVv { vd, vs2, vs1 }
    }

    pub const fn add_unsigned_vx(vd: VectorRegister, vs2: VectorRegister, rs1: Register) -> Self {
        Self::AddUnsignedVx { vd, vs2, rs1 }
    }

    pub const fn add_signed_vx(vd: VectorRegister, vs2: VectorRegister, rs1: Register) -> Self {
        Self::AddSignedVx { vd, vs2, rs1 }
    }

    pub const fn sub_unsigned_vx(vd: VectorRegister, vs2: VectorRegister, rs1: Register) -> Self {
        Self::SubUnsignedVx { vd, vs2, rs1 }
    }

    pub const fn sub_signed_vx(vd: VectorRegister, vs2: VectorRegister, rs1: Register) -> Self {
        Self::SubSignedVx { vd, vs2, rs1 }
    }

    pub const fn mul_signed_fractional_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::MulSignedFractionalVv { vd, vs2, vs1 }
    }

    pub const fn mul_signed_fractional_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    ) -> Self {
        Self::MulSignedFractionalVx { vd, vs2, rs1 }
    }

    pub const fn add_unsigned_vi(vd: VectorRegister, vs2: VectorRegister, imm: i8) -> Self {
        Self::AddUnsignedVi { vd, vs2, imm }
    }

    pub const fn add_signed_vi(vd: VectorRegister, vs2: VectorRegister, imm: i8) -> Self {
        Self::AddSignedVi { vd, vs2, imm }
    }
}

pub(crate) fn decode_add_unsigned_vv(raw: u32) -> RiscvInstruction {
    decode_vv(raw, RiscvVectorSaturatingInstruction::add_unsigned_vv)
}

pub(crate) fn decode_add_signed_vv(raw: u32) -> RiscvInstruction {
    decode_vv(raw, RiscvVectorSaturatingInstruction::add_signed_vv)
}

pub(crate) fn decode_sub_unsigned_vv(raw: u32) -> RiscvInstruction {
    decode_vv(raw, RiscvVectorSaturatingInstruction::sub_unsigned_vv)
}

pub(crate) fn decode_sub_signed_vv(raw: u32) -> RiscvInstruction {
    decode_vv(raw, RiscvVectorSaturatingInstruction::sub_signed_vv)
}

pub(crate) fn decode_add_unsigned_vx(raw: u32) -> RiscvInstruction {
    decode_vx(raw, RiscvVectorSaturatingInstruction::add_unsigned_vx)
}

pub(crate) fn decode_add_signed_vx(raw: u32) -> RiscvInstruction {
    decode_vx(raw, RiscvVectorSaturatingInstruction::add_signed_vx)
}

pub(crate) fn decode_sub_unsigned_vx(raw: u32) -> RiscvInstruction {
    decode_vx(raw, RiscvVectorSaturatingInstruction::sub_unsigned_vx)
}

pub(crate) fn decode_sub_signed_vx(raw: u32) -> RiscvInstruction {
    decode_vx(raw, RiscvVectorSaturatingInstruction::sub_signed_vx)
}

pub(crate) fn decode_mul_signed_fractional_vv(raw: u32) -> RiscvInstruction {
    decode_vv(
        raw,
        RiscvVectorSaturatingInstruction::mul_signed_fractional_vv,
    )
}

pub(crate) fn decode_mul_signed_fractional_vx(raw: u32) -> RiscvInstruction {
    decode_vx(
        raw,
        RiscvVectorSaturatingInstruction::mul_signed_fractional_vx,
    )
}

pub(crate) fn decode_add_unsigned_vi(raw: u32) -> RiscvInstruction {
    decode_vi(raw, RiscvVectorSaturatingInstruction::add_unsigned_vi)
}

pub(crate) fn decode_add_signed_vi(raw: u32) -> RiscvInstruction {
    decode_vi(raw, RiscvVectorSaturatingInstruction::add_signed_vi)
}

fn decode_vv(
    raw: u32,
    build: fn(VectorRegister, VectorRegister, VectorRegister) -> RiscvVectorSaturatingInstruction,
) -> RiscvInstruction {
    RiscvInstruction::VectorSaturating(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        vector_register(raw, 15),
    ))
}

fn decode_vx(
    raw: u32,
    build: fn(VectorRegister, VectorRegister, Register) -> RiscvVectorSaturatingInstruction,
) -> RiscvInstruction {
    RiscvInstruction::VectorSaturating(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        Register::from_field((raw >> 15) & 0x1f),
    ))
}

fn decode_vi(
    raw: u32,
    build: fn(VectorRegister, VectorRegister, i8) -> RiscvVectorSaturatingInstruction,
) -> RiscvInstruction {
    RiscvInstruction::VectorSaturating(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        vector_signed_imm5(raw),
    ))
}

fn vector_register(raw: u32, shift: u32) -> VectorRegister {
    VectorRegister::from_field((raw >> shift) & 0x1f)
}

fn vector_signed_imm5(raw: u32) -> i8 {
    let value = ((raw >> 15) & 0x1f) as i8;
    (value << 3) >> 3
}

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorSaturatingInstruction,
) -> bool {
    match instruction {
        RiscvVectorSaturatingInstruction::AddUnsignedVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, SaturatingOp::AddUnsigned)
        }
        RiscvVectorSaturatingInstruction::AddSignedVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, SaturatingOp::AddSigned)
        }
        RiscvVectorSaturatingInstruction::SubUnsignedVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, SaturatingOp::SubUnsigned)
        }
        RiscvVectorSaturatingInstruction::SubSignedVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, SaturatingOp::SubSigned)
        }
        RiscvVectorSaturatingInstruction::AddUnsignedVx { vd, vs2, rs1 } => {
            execute_vx(hart, vd, vs2, hart.read(rs1), SaturatingOp::AddUnsigned)
        }
        RiscvVectorSaturatingInstruction::AddSignedVx { vd, vs2, rs1 } => {
            execute_vx(hart, vd, vs2, hart.read(rs1), SaturatingOp::AddSigned)
        }
        RiscvVectorSaturatingInstruction::SubUnsignedVx { vd, vs2, rs1 } => {
            execute_vx(hart, vd, vs2, hart.read(rs1), SaturatingOp::SubUnsigned)
        }
        RiscvVectorSaturatingInstruction::SubSignedVx { vd, vs2, rs1 } => {
            execute_vx(hart, vd, vs2, hart.read(rs1), SaturatingOp::SubSigned)
        }
        RiscvVectorSaturatingInstruction::MulSignedFractionalVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, SaturatingOp::MulSignedFractional)
        }
        RiscvVectorSaturatingInstruction::MulSignedFractionalVx { vd, vs2, rs1 } => execute_vx(
            hart,
            vd,
            vs2,
            hart.read(rs1),
            SaturatingOp::MulSignedFractional,
        ),
        RiscvVectorSaturatingInstruction::AddUnsignedVi { vd, vs2, imm } => {
            execute_vi(hart, vd, vs2, imm, SaturatingOp::AddUnsigned)
        }
        RiscvVectorSaturatingInstruction::AddSignedVi { vd, vs2, imm } => {
            execute_vi(hart, vd, vs2, imm, SaturatingOp::AddSigned)
        }
    }
}

fn execute_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
    op: SaturatingOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    let right = read_register_group(hart, vs1, plan.group_registers);
    execute_lanes(
        hart,
        vd,
        &plan,
        &left,
        |element_index, element_bytes| {
            let offset = element_index * element_bytes;
            lane_bytes_to_u128(&right[offset..offset + element_bytes])
        },
        op,
    )
}

fn execute_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    op: SaturatingOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    execute_lanes(
        hart,
        vd,
        &plan,
        &left,
        |_, element_bytes| scalar_operand_bits(scalar, element_bytes * 8),
        op,
    )
}

fn execute_vi(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    imm: i8,
    op: SaturatingOp,
) -> bool {
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.group_registers);
    execute_lanes(
        hart,
        vd,
        &plan,
        &left,
        |_, element_bytes| signed_operand_bits(i128::from(imm), element_bytes * 8),
        op,
    )
}

fn execute_lanes(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    plan: &VectorBinaryPlan,
    left: &[u8],
    right_lane: impl Fn(usize, usize) -> u128,
    op: SaturatingOp,
) -> bool {
    let mut result = read_register_group(hart, vd, plan.group_registers);
    let mut fixed = hart.vector_fixed_point();
    let element_bytes = plan.element_bytes;
    let element_bits = element_bytes * 8;
    let rounding_mode = fixed.rounding_mode();

    for element_index in 0..plan.active_element_count() {
        let offset = element_index * element_bytes;
        let left = lane_bytes_to_u128(&left[offset..offset + element_bytes]);
        let right = right_lane(element_index, element_bytes);
        let outcome = apply_saturating(op, left, right, element_bits, rounding_mode);
        if outcome.saturated {
            fixed.write_vxsat_bit(true);
        }
        write_u128_lane(&mut result[offset..offset + element_bytes], outcome.value);
    }

    hart.set_vector_fixed_point(fixed);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

#[derive(Clone, Copy)]
struct SaturatingOutcome {
    value: u128,
    saturated: bool,
}

fn apply_saturating(
    op: SaturatingOp,
    left: u128,
    right: u128,
    bits: usize,
    rounding_mode: RiscvVectorFixedRoundingMode,
) -> SaturatingOutcome {
    match op {
        SaturatingOp::AddUnsigned => saturating_add_unsigned(left, right, bits),
        SaturatingOp::AddSigned => saturating_add_signed(left, right, bits),
        SaturatingOp::SubUnsigned => saturating_sub_unsigned(left, right),
        SaturatingOp::SubSigned => saturating_sub_signed(left, right, bits),
        SaturatingOp::MulSignedFractional => {
            saturating_mul_signed_fractional(left, right, bits, rounding_mode)
        }
    }
}

fn saturating_add_unsigned(left: u128, right: u128, bits: usize) -> SaturatingOutcome {
    let max = unsigned_max(bits);
    let sum = left + right;
    if sum > max {
        SaturatingOutcome {
            value: max,
            saturated: true,
        }
    } else {
        SaturatingOutcome {
            value: sum,
            saturated: false,
        }
    }
}

fn saturating_sub_unsigned(left: u128, right: u128) -> SaturatingOutcome {
    if left < right {
        SaturatingOutcome {
            value: 0,
            saturated: true,
        }
    } else {
        SaturatingOutcome {
            value: left - right,
            saturated: false,
        }
    }
}

fn saturating_add_signed(left: u128, right: u128, bits: usize) -> SaturatingOutcome {
    let left = sign_extend(left, bits);
    let right = sign_extend(right, bits);
    saturating_signed_result(left + right, bits)
}

fn saturating_sub_signed(left: u128, right: u128, bits: usize) -> SaturatingOutcome {
    let left = sign_extend(left, bits);
    let right = sign_extend(right, bits);
    saturating_signed_result(left - right, bits)
}

fn saturating_mul_signed_fractional(
    left: u128,
    right: u128,
    bits: usize,
    rounding_mode: RiscvVectorFixedRoundingMode,
) -> SaturatingOutcome {
    let product = sign_extend(left, bits) * sign_extend(right, bits);
    let shift = (bits - 1) as u32;
    let rounded = round_signed(product, shift, rounding_mode)
        .expect("single-width signed vector fractional multiply cannot overflow")
        >> shift;
    saturating_signed_result(rounded, bits)
}

fn saturating_signed_result(value: i128, bits: usize) -> SaturatingOutcome {
    let min = signed_min(bits);
    let max = signed_max(bits);
    if value < min {
        SaturatingOutcome {
            value: signed_operand_bits(min, bits),
            saturated: true,
        }
    } else if value > max {
        SaturatingOutcome {
            value: signed_operand_bits(max, bits),
            saturated: true,
        }
    } else {
        SaturatingOutcome {
            value: signed_operand_bits(value, bits),
            saturated: false,
        }
    }
}

fn sign_extend(value: u128, bits: usize) -> i128 {
    let shift = 128 - bits;
    ((value << shift) as i128) >> shift
}

fn scalar_operand_bits(value: u64, bits: usize) -> u128 {
    u128::from(value) & unsigned_max(bits)
}

fn signed_operand_bits(value: i128, bits: usize) -> u128 {
    (value as u128) & unsigned_max(bits)
}

fn unsigned_max(bits: usize) -> u128 {
    (1_u128 << bits) - 1
}

fn signed_min(bits: usize) -> i128 {
    -(1_i128 << (bits - 1))
}

fn signed_max(bits: usize) -> i128 {
    (1_i128 << (bits - 1)) - 1
}
