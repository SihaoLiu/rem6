use crate::{
    vector::RiscvVectorFixedRoundingMode,
    vector_fixed_point_shift::{round_signed, round_unsigned},
    vector_group::{
        lane_bytes_to_u128, read_register_group, write_register_group, write_u128_lane,
        VectorBinaryPlan,
    },
    Register, RiscvHartState, RiscvInstruction, VectorRegister,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorAveragingInstruction {
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
}

#[derive(Clone, Copy)]
enum AveragingOp {
    AddUnsigned,
    AddSigned,
    SubUnsigned,
    SubSigned,
}

impl RiscvVectorAveragingInstruction {
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
}

pub(crate) fn decode_add_unsigned_vv(raw: u32) -> RiscvInstruction {
    decode_vv(raw, RiscvVectorAveragingInstruction::add_unsigned_vv)
}

pub(crate) fn decode_add_signed_vv(raw: u32) -> RiscvInstruction {
    decode_vv(raw, RiscvVectorAveragingInstruction::add_signed_vv)
}

pub(crate) fn decode_sub_unsigned_vv(raw: u32) -> RiscvInstruction {
    decode_vv(raw, RiscvVectorAveragingInstruction::sub_unsigned_vv)
}

pub(crate) fn decode_sub_signed_vv(raw: u32) -> RiscvInstruction {
    decode_vv(raw, RiscvVectorAveragingInstruction::sub_signed_vv)
}

pub(crate) fn decode_add_unsigned_vx(raw: u32) -> RiscvInstruction {
    decode_vx(raw, RiscvVectorAveragingInstruction::add_unsigned_vx)
}

pub(crate) fn decode_add_signed_vx(raw: u32) -> RiscvInstruction {
    decode_vx(raw, RiscvVectorAveragingInstruction::add_signed_vx)
}

pub(crate) fn decode_sub_unsigned_vx(raw: u32) -> RiscvInstruction {
    decode_vx(raw, RiscvVectorAveragingInstruction::sub_unsigned_vx)
}

pub(crate) fn decode_sub_signed_vx(raw: u32) -> RiscvInstruction {
    decode_vx(raw, RiscvVectorAveragingInstruction::sub_signed_vx)
}

fn decode_vv(
    raw: u32,
    build: fn(VectorRegister, VectorRegister, VectorRegister) -> RiscvVectorAveragingInstruction,
) -> RiscvInstruction {
    RiscvInstruction::VectorAveraging(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        vector_register(raw, 15),
    ))
}

fn decode_vx(
    raw: u32,
    build: fn(VectorRegister, VectorRegister, Register) -> RiscvVectorAveragingInstruction,
) -> RiscvInstruction {
    RiscvInstruction::VectorAveraging(build(
        vector_register(raw, 7),
        vector_register(raw, 20),
        Register::from_field((raw >> 15) & 0x1f),
    ))
}

fn vector_register(raw: u32, shift: u32) -> VectorRegister {
    VectorRegister::from_field((raw >> shift) & 0x1f)
}

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorAveragingInstruction,
) -> bool {
    match instruction {
        RiscvVectorAveragingInstruction::AddUnsignedVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, AveragingOp::AddUnsigned)
        }
        RiscvVectorAveragingInstruction::AddSignedVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, AveragingOp::AddSigned)
        }
        RiscvVectorAveragingInstruction::SubUnsignedVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, AveragingOp::SubUnsigned)
        }
        RiscvVectorAveragingInstruction::SubSignedVv { vd, vs2, vs1 } => {
            execute_vv(hart, vd, vs2, vs1, AveragingOp::SubSigned)
        }
        RiscvVectorAveragingInstruction::AddUnsignedVx { vd, vs2, rs1 } => {
            execute_vx(hart, vd, vs2, hart.read(rs1), AveragingOp::AddUnsigned)
        }
        RiscvVectorAveragingInstruction::AddSignedVx { vd, vs2, rs1 } => {
            execute_vx(hart, vd, vs2, hart.read(rs1), AveragingOp::AddSigned)
        }
        RiscvVectorAveragingInstruction::SubUnsignedVx { vd, vs2, rs1 } => {
            execute_vx(hart, vd, vs2, hart.read(rs1), AveragingOp::SubUnsigned)
        }
        RiscvVectorAveragingInstruction::SubSignedVx { vd, vs2, rs1 } => {
            execute_vx(hart, vd, vs2, hart.read(rs1), AveragingOp::SubSigned)
        }
    }
}

fn execute_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
    op: AveragingOp,
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
    op: AveragingOp,
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
        |_, element_bytes| u128::from(scalar) & unsigned_max(element_bytes * 8),
        op,
    )
}

fn execute_lanes(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    plan: &VectorBinaryPlan,
    left: &[u8],
    right_lane: impl Fn(usize, usize) -> u128,
    op: AveragingOp,
) -> bool {
    let mut result = read_register_group(hart, vd, plan.group_registers);
    let rounding_mode = hart.vector_fixed_point().rounding_mode();
    let element_bytes = plan.element_bytes;
    let element_bits = element_bytes * 8;

    for element_index in 0..plan.active_element_count() {
        let offset = element_index * element_bytes;
        let left = lane_bytes_to_u128(&left[offset..offset + element_bytes]);
        let right = right_lane(element_index, element_bytes);
        let value = apply_averaging(op, left, right, element_bits, rounding_mode);
        write_u128_lane(&mut result[offset..offset + element_bytes], value);
    }

    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn apply_averaging(
    op: AveragingOp,
    left: u128,
    right: u128,
    bits: usize,
    rounding_mode: RiscvVectorFixedRoundingMode,
) -> u128 {
    match op {
        AveragingOp::AddUnsigned => average_unsigned(left + right, rounding_mode),
        AveragingOp::AddSigned => average_signed(
            sign_extend(left, bits) + sign_extend(right, bits),
            bits,
            rounding_mode,
        ),
        AveragingOp::SubUnsigned => {
            let wrapped = left.wrapping_sub(right) & unsigned_max(bits);
            average_unsigned(wrapped, rounding_mode) & unsigned_max(bits)
        }
        AveragingOp::SubSigned => average_signed(
            sign_extend(left, bits) - sign_extend(right, bits),
            bits,
            rounding_mode,
        ),
    }
}

fn average_unsigned(value: u128, rounding_mode: RiscvVectorFixedRoundingMode) -> u128 {
    round_unsigned(value, 1, rounding_mode)
        .expect("single-width unsigned vector averaging cannot overflow")
        >> 1
}

fn average_signed(value: i128, bits: usize, rounding_mode: RiscvVectorFixedRoundingMode) -> u128 {
    signed_operand_bits(
        round_signed(value, 1, rounding_mode)
            .expect("single-width signed vector averaging cannot overflow")
            >> 1,
        bits,
    )
}

fn sign_extend(value: u128, bits: usize) -> i128 {
    let shift = 128 - bits;
    ((value << shift) as i128) >> shift
}

fn signed_operand_bits(value: i128, bits: usize) -> u128 {
    (value as u128) & unsigned_max(bits)
}

fn unsigned_max(bits: usize) -> u128 {
    if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    }
}
