use crate::{
    vector_group::{
        lane_bytes_to_u128, read_mask_bit, read_register_group, valid_register_group,
        write_u128_lane,
    },
    RiscvHartState, RiscvInstruction, RiscvVectorMaskMode, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorReductionInstruction {
    Vs {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
        operation: RiscvVectorReductionOperation,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorReductionOperation {
    Sum,
    And,
    Or,
    Xor,
    MinUnsigned,
    MinSigned,
    MaxUnsigned,
    MaxSigned,
}

impl RiscvVectorReductionInstruction {
    pub const fn sum(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::new(vd, vs2, vs1, mask, RiscvVectorReductionOperation::Sum)
    }

    pub const fn and(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::new(vd, vs2, vs1, mask, RiscvVectorReductionOperation::And)
    }

    pub const fn or(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::new(vd, vs2, vs1, mask, RiscvVectorReductionOperation::Or)
    }

    pub const fn xor(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::new(vd, vs2, vs1, mask, RiscvVectorReductionOperation::Xor)
    }

    pub const fn min_unsigned(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::new(
            vd,
            vs2,
            vs1,
            mask,
            RiscvVectorReductionOperation::MinUnsigned,
        )
    }

    pub const fn min_signed(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::new(vd, vs2, vs1, mask, RiscvVectorReductionOperation::MinSigned)
    }

    pub const fn max_unsigned(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::new(
            vd,
            vs2,
            vs1,
            mask,
            RiscvVectorReductionOperation::MaxUnsigned,
        )
    }

    pub const fn max_signed(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::new(vd, vs2, vs1, mask, RiscvVectorReductionOperation::MaxSigned)
    }

    const fn new(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
        operation: RiscvVectorReductionOperation,
    ) -> Self {
        Self::Vs {
            vd,
            vs2,
            vs1,
            mask,
            operation,
        }
    }
}

pub(crate) fn decode(raw: u32) -> RiscvInstruction {
    let operation = match (raw >> 26) & 0x3f {
        0b000000 => RiscvVectorReductionOperation::Sum,
        0b000001 => RiscvVectorReductionOperation::And,
        0b000010 => RiscvVectorReductionOperation::Or,
        0b000011 => RiscvVectorReductionOperation::Xor,
        0b000100 => RiscvVectorReductionOperation::MinUnsigned,
        0b000101 => RiscvVectorReductionOperation::MinSigned,
        0b000110 => RiscvVectorReductionOperation::MaxUnsigned,
        0b000111 => RiscvVectorReductionOperation::MaxSigned,
        _ => unreachable!("reduction funct6 is range-checked by decode_vector"),
    };
    RiscvInstruction::VectorReduction(RiscvVectorReductionInstruction::new(
        vector_register(raw, 7),
        vector_register(raw, 20),
        vector_register(raw, 15),
        RiscvVectorMaskMode::from_vm_bit((raw & (1 << 25)) != 0),
        operation,
    ))
}

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorReductionInstruction,
) -> bool {
    let RiscvVectorReductionInstruction::Vs {
        vd,
        vs2,
        vs1,
        mask,
        operation,
    } = instruction;

    let Some(plan) = ReductionPlan::new(hart, vs2) else {
        return false;
    };
    if plan.active_elements == 0 {
        return true;
    }
    let source = read_register_group(hart, vs2, plan.source_registers);
    let seed = hart.read_vector(vs1);
    let selector = mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let mut accumulator = lane_bytes_to_u128(&seed[..plan.element_bytes]);
    for element_index in 0..plan.active_elements {
        if selector
            .as_ref()
            .is_some_and(|mask| !read_mask_bit(mask, element_index))
        {
            continue;
        }
        let offset = element_index * plan.element_bytes;
        let lane = lane_bytes_to_u128(&source[offset..offset + plan.element_bytes]);
        accumulator = operation.apply(accumulator, lane, plan.element_bits);
    }

    let mut destination = hart.read_vector(vd);
    write_u128_lane(&mut destination[..plan.element_bytes], accumulator);
    hart.write_vector(vd, destination);
    true
}

impl RiscvVectorReductionOperation {
    fn apply(self, left: u128, right: u128, bits: usize) -> u128 {
        let mask = unsigned_max(bits);
        match self {
            Self::Sum => left.wrapping_add(right) & mask,
            Self::And => left & right,
            Self::Or => left | right,
            Self::Xor => left ^ right,
            Self::MinUnsigned => left.min(right),
            Self::MinSigned => {
                if sign_extend(left, bits) <= sign_extend(right, bits) {
                    left
                } else {
                    right
                }
            }
            Self::MaxUnsigned => left.max(right),
            Self::MaxSigned => {
                if sign_extend(left, bits) >= sign_extend(right, bits) {
                    left
                } else {
                    right
                }
            }
        }
    }
}

struct ReductionPlan {
    element_bytes: usize,
    element_bits: usize,
    source_registers: usize,
    active_elements: usize,
}

impl ReductionPlan {
    fn new(hart: &RiscvHartState, vs2: VectorRegister) -> Option<Self> {
        let config = hart.vector_config();
        let element_bytes = config.element_width_bytes()?;
        let source_registers = config.register_group_registers()?;
        if !valid_register_group(vs2, source_registers) {
            return None;
        }
        let active_elements = config.vl() as usize;
        let active_bytes = active_elements.checked_mul(element_bytes)?;
        if active_bytes > source_registers * RISCV_VECTOR_REGISTER_BYTES {
            return None;
        }
        Some(Self {
            element_bytes,
            element_bits: element_bytes * 8,
            source_registers,
            active_elements,
        })
    }
}

fn vector_register(raw: u32, shift: u32) -> VectorRegister {
    VectorRegister::from_field((raw >> shift) & 0x1f)
}

fn unsigned_max(bits: usize) -> u128 {
    (1_u128 << bits) - 1
}

fn sign_extend(value: u128, bits: usize) -> i128 {
    let shift = 128 - bits;
    ((value << shift) as i128) >> shift
}
