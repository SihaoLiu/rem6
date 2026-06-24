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
}

#[derive(Clone, Copy)]
enum WideningIntegerOp {
    AddUnsigned,
    AddSigned,
    SubUnsigned,
    SubSigned,
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
}

pub(crate) fn decode_vv(raw: u32) -> RiscvInstruction {
    let mask = RiscvVectorMaskMode::from_vm_bit((raw & (1 << 25)) != 0);
    let build = match (raw >> 26) & 0x3f {
        0b110000 => RiscvVectorWideningIntegerInstruction::add_unsigned_vv,
        0b110001 => RiscvVectorWideningIntegerInstruction::add_signed_vv,
        0b110010 => RiscvVectorWideningIntegerInstruction::sub_unsigned_vv,
        0b110011 => RiscvVectorWideningIntegerInstruction::sub_signed_vv,
        _ => unreachable!("widening integer vv funct6 is range-checked by decode_vector"),
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
        _ => unreachable!("widening integer vx funct6 is range-checked by decode_vector"),
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
    let Some(plan) = WideningIntegerPlan::new(hart, vd, &[vs2, vs1], mask) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.source_registers);
    let right = read_register_group(hart, vs1, plan.source_registers);
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
    mask: RiscvVectorMaskMode,
    op: WideningIntegerOp,
) -> bool {
    let Some(plan) = WideningIntegerPlan::new(hart, vd, &[vs2], mask) else {
        return false;
    };
    let left = read_register_group(hart, vs2, plan.source_registers);
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
    plan: &WideningIntegerPlan,
    left: &[u8; MAX_VECTOR_GROUP_BYTES],
    right_lane: impl Fn(usize, usize) -> u128,
    op: WideningIntegerOp,
) -> bool {
    let selector = plan
        .mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let mut result = read_register_group(hart, vd, plan.destination_registers);

    for element_index in 0..plan.active_elements {
        if selector
            .as_ref()
            .is_some_and(|mask| !read_mask_bit(mask, element_index))
        {
            continue;
        }
        let source_offset = element_index * plan.source_element_bytes;
        let destination_offset = element_index * plan.destination_element_bytes;
        let left =
            lane_bytes_to_u128(&left[source_offset..source_offset + plan.source_element_bytes]);
        let right = right_lane(element_index, plan.source_element_bytes);
        let value = apply_widening_integer(op, left, right, plan.source_element_bits);
        write_u128_lane(
            &mut result[destination_offset..destination_offset + plan.destination_element_bytes],
            value,
        );
    }

    write_register_group(hart, vd, plan.destination_registers, &result);
    true
}

fn apply_widening_integer(
    op: WideningIntegerOp,
    left: u128,
    right: u128,
    source_bits: usize,
) -> u128 {
    let result_bits = source_bits * 2;
    let mask = unsigned_max(result_bits);
    match op {
        WideningIntegerOp::AddUnsigned => left.wrapping_add(right) & mask,
        WideningIntegerOp::SubUnsigned => left.wrapping_sub(right) & mask,
        WideningIntegerOp::AddSigned => signed_operand_bits(
            sign_extend(left, source_bits) + sign_extend(right, source_bits),
            result_bits,
        ),
        WideningIntegerOp::SubSigned => signed_operand_bits(
            sign_extend(left, source_bits) - sign_extend(right, source_bits),
            result_bits,
        ),
    }
}

struct WideningIntegerPlan {
    source_element_bytes: usize,
    source_element_bits: usize,
    destination_element_bytes: usize,
    source_registers: usize,
    destination_registers: usize,
    active_elements: usize,
    mask: RiscvVectorMaskMode,
}

impl WideningIntegerPlan {
    fn new(
        hart: &RiscvHartState,
        vd: VectorRegister,
        sources: &[VectorRegister],
        mask: RiscvVectorMaskMode,
    ) -> Option<Self> {
        let config = hart.vector_config();
        let source_element_bytes = config.element_width_bytes()?;
        let destination_element_bytes = source_element_bytes.checked_mul(2)?;
        if destination_element_bytes > 16 {
            return None;
        }
        let source_registers = config.register_group_registers()?;
        let destination_registers = widening_destination_registers(config.vtype())?;
        if !valid_register_group(vd, destination_registers)
            || sources.iter().any(|source| {
                !valid_widening_source(
                    config.vtype(),
                    vd,
                    destination_registers,
                    *source,
                    source_registers,
                )
            })
            || (mask.is_masked()
                && register_groups_overlap(
                    vd,
                    destination_registers,
                    VectorRegister::from_field(0),
                    1,
                ))
        {
            return None;
        }

        let active_elements = config.vl() as usize;
        let source_active_bytes = active_elements.checked_mul(source_element_bytes)?;
        let destination_active_bytes = active_elements.checked_mul(destination_element_bytes)?;
        if source_active_bytes > source_registers * RISCV_VECTOR_REGISTER_BYTES
            || destination_active_bytes > destination_registers * RISCV_VECTOR_REGISTER_BYTES
        {
            return None;
        }

        Some(Self {
            source_element_bytes,
            source_element_bits: source_element_bytes * 8,
            destination_element_bytes,
            source_registers,
            destination_registers,
            active_elements,
            mask,
        })
    }
}

fn widening_destination_registers(vtype: u64) -> Option<usize> {
    match vtype & 0x7 {
        0 => Some(2),
        1 => Some(4),
        2 => Some(8),
        3 | 4 => None,
        5..=7 => Some(1),
        _ => unreachable!(),
    }
}

fn valid_widening_source(
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
