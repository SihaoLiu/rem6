use crate::{
    vector_group::{
        lane_bytes_to_u128, read_mask_bit, read_register_group, register_groups_overlap,
        valid_register_group, write_register_group, write_u128_lane, VectorBinaryPlan,
        MAX_VECTOR_GROUP_BYTES,
    },
    Register, RiscvError, RiscvHartState, RiscvInstruction, RiscvVectorMaskMode, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorIntegerCarryBorrowInstruction {
    AddWithCarryVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    },
    AddWithCarryVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    AddWithCarryVi {
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
    },
    AddCarryOutVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    AddCarryOutVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
    AddCarryOutVi {
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
        mask: RiscvVectorMaskMode,
    },
    SubWithBorrowVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    },
    SubWithBorrowVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    },
    SubBorrowOutVv {
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    },
    SubBorrowOutVx {
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    },
}

#[derive(Clone, Copy)]
enum CarryBorrowOp {
    Add,
    Sub,
}

impl RiscvVectorIntegerCarryBorrowInstruction {
    pub const fn add_with_carry_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::AddWithCarryVv { vd, vs2, vs1 }
    }

    pub const fn add_with_carry_vx(vd: VectorRegister, vs2: VectorRegister, rs1: Register) -> Self {
        Self::AddWithCarryVx { vd, vs2, rs1 }
    }

    pub const fn add_with_carry_vi(vd: VectorRegister, vs2: VectorRegister, imm: i8) -> Self {
        Self::AddWithCarryVi { vd, vs2, imm }
    }

    pub const fn add_carry_out_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddCarryOutVv { vd, vs2, vs1, mask }
    }

    pub const fn add_carry_out_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddCarryOutVx { vd, vs2, rs1, mask }
    }

    pub const fn add_carry_out_vi(
        vd: VectorRegister,
        vs2: VectorRegister,
        imm: i8,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::AddCarryOutVi { vd, vs2, imm, mask }
    }

    pub const fn sub_with_borrow_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
    ) -> Self {
        Self::SubWithBorrowVv { vd, vs2, vs1 }
    }

    pub const fn sub_with_borrow_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
    ) -> Self {
        Self::SubWithBorrowVx { vd, vs2, rs1 }
    }

    pub const fn sub_borrow_out_vv(
        vd: VectorRegister,
        vs2: VectorRegister,
        vs1: VectorRegister,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubBorrowOutVv { vd, vs2, vs1, mask }
    }

    pub const fn sub_borrow_out_vx(
        vd: VectorRegister,
        vs2: VectorRegister,
        rs1: Register,
        mask: RiscvVectorMaskMode,
    ) -> Self {
        Self::SubBorrowOutVx { vd, vs2, rs1, mask }
    }
}

pub(crate) const fn is_vv_funct6(funct6: u32) -> bool {
    matches!(funct6, 0b010000..=0b010011)
}

pub(crate) const fn is_vx_funct6(funct6: u32) -> bool {
    is_vv_funct6(funct6)
}

pub(crate) const fn is_vi_funct6(funct6: u32) -> bool {
    matches!(funct6, 0b010000..=0b010001)
}

pub(crate) fn decode_vv(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let vd = vector_register(raw, 7);
    let vs2 = vector_register(raw, 20);
    let vs1 = vector_register(raw, 15);
    let mask = vector_mask_mode(raw);
    let instruction = match ((raw >> 26) & 0x3f, mask) {
        (0b010000, RiscvVectorMaskMode::Masked) if vd.index() != 0 => {
            RiscvVectorIntegerCarryBorrowInstruction::add_with_carry_vv(vd, vs2, vs1)
        }
        (0b010001, _) => {
            RiscvVectorIntegerCarryBorrowInstruction::add_carry_out_vv(vd, vs2, vs1, mask)
        }
        (0b010010, RiscvVectorMaskMode::Masked) if vd.index() != 0 => {
            RiscvVectorIntegerCarryBorrowInstruction::sub_with_borrow_vv(vd, vs2, vs1)
        }
        (0b010011, _) => {
            RiscvVectorIntegerCarryBorrowInstruction::sub_borrow_out_vv(vd, vs2, vs1, mask)
        }
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };
    Ok(RiscvInstruction::VectorIntegerCarryBorrow(instruction))
}

pub(crate) fn decode_vx(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let vd = vector_register(raw, 7);
    let vs2 = vector_register(raw, 20);
    let rs1 = Register::from_field((raw >> 15) & 0x1f);
    let mask = vector_mask_mode(raw);
    let instruction = match ((raw >> 26) & 0x3f, mask) {
        (0b010000, RiscvVectorMaskMode::Masked) if vd.index() != 0 => {
            RiscvVectorIntegerCarryBorrowInstruction::add_with_carry_vx(vd, vs2, rs1)
        }
        (0b010001, _) => {
            RiscvVectorIntegerCarryBorrowInstruction::add_carry_out_vx(vd, vs2, rs1, mask)
        }
        (0b010010, RiscvVectorMaskMode::Masked) if vd.index() != 0 => {
            RiscvVectorIntegerCarryBorrowInstruction::sub_with_borrow_vx(vd, vs2, rs1)
        }
        (0b010011, _) => {
            RiscvVectorIntegerCarryBorrowInstruction::sub_borrow_out_vx(vd, vs2, rs1, mask)
        }
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };
    Ok(RiscvInstruction::VectorIntegerCarryBorrow(instruction))
}

pub(crate) fn decode_vi(raw: u32) -> Result<RiscvInstruction, RiscvError> {
    let vd = vector_register(raw, 7);
    let vs2 = vector_register(raw, 20);
    let imm = vector_signed_imm5(raw);
    let mask = vector_mask_mode(raw);
    let instruction = match ((raw >> 26) & 0x3f, mask) {
        (0b010000, RiscvVectorMaskMode::Masked) if vd.index() != 0 => {
            RiscvVectorIntegerCarryBorrowInstruction::add_with_carry_vi(vd, vs2, imm)
        }
        (0b010001, _) => {
            RiscvVectorIntegerCarryBorrowInstruction::add_carry_out_vi(vd, vs2, imm, mask)
        }
        _ => return Err(RiscvError::UnknownEncoding { raw }),
    };
    Ok(RiscvInstruction::VectorIntegerCarryBorrow(instruction))
}

fn vector_register(raw: u32, shift: u32) -> VectorRegister {
    VectorRegister::from_field((raw >> shift) & 0x1f)
}

fn vector_mask_mode(raw: u32) -> RiscvVectorMaskMode {
    RiscvVectorMaskMode::from_vm_bit((raw & (1 << 25)) != 0)
}

fn vector_signed_imm5(raw: u32) -> i8 {
    let value = ((raw >> 15) & 0x1f) as i8;
    (value << 3) >> 3
}

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorIntegerCarryBorrowInstruction,
) -> bool {
    match instruction {
        RiscvVectorIntegerCarryBorrowInstruction::AddWithCarryVv { vd, vs2, vs1 } => {
            execute_result_vv(hart, vd, vs2, vs1, CarryBorrowOp::Add)
        }
        RiscvVectorIntegerCarryBorrowInstruction::AddWithCarryVx { vd, vs2, rs1 } => {
            execute_result_vx(hart, vd, vs2, hart.read(rs1), CarryBorrowOp::Add)
        }
        RiscvVectorIntegerCarryBorrowInstruction::AddWithCarryVi { vd, vs2, imm } => {
            execute_result_vx(hart, vd, vs2, imm as i64 as u64, CarryBorrowOp::Add)
        }
        RiscvVectorIntegerCarryBorrowInstruction::AddCarryOutVv { vd, vs2, vs1, mask } => {
            execute_mask_vv(hart, vd, vs2, vs1, mask, CarryBorrowOp::Add)
        }
        RiscvVectorIntegerCarryBorrowInstruction::AddCarryOutVx { vd, vs2, rs1, mask } => {
            execute_mask_vx(hart, vd, vs2, hart.read(rs1), mask, CarryBorrowOp::Add)
        }
        RiscvVectorIntegerCarryBorrowInstruction::AddCarryOutVi { vd, vs2, imm, mask } => {
            execute_mask_vx(hart, vd, vs2, imm as i64 as u64, mask, CarryBorrowOp::Add)
        }
        RiscvVectorIntegerCarryBorrowInstruction::SubWithBorrowVv { vd, vs2, vs1 } => {
            execute_result_vv(hart, vd, vs2, vs1, CarryBorrowOp::Sub)
        }
        RiscvVectorIntegerCarryBorrowInstruction::SubWithBorrowVx { vd, vs2, rs1 } => {
            execute_result_vx(hart, vd, vs2, hart.read(rs1), CarryBorrowOp::Sub)
        }
        RiscvVectorIntegerCarryBorrowInstruction::SubBorrowOutVv { vd, vs2, vs1, mask } => {
            execute_mask_vv(hart, vd, vs2, vs1, mask, CarryBorrowOp::Sub)
        }
        RiscvVectorIntegerCarryBorrowInstruction::SubBorrowOutVx { vd, vs2, rs1, mask } => {
            execute_mask_vx(hart, vd, vs2, hart.read(rs1), mask, CarryBorrowOp::Sub)
        }
    }
}

fn execute_result_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
    operation: CarryBorrowOp,
) -> bool {
    if vd.index() == 0 {
        return false;
    }
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2, vs1]) else {
        return false;
    };
    if sources_overlap_mask(&[vs2, vs1], plan.group_registers) {
        return false;
    }
    let carry = hart.read_vector(VectorRegister::from_field(0));
    let vs2_bytes = read_register_group(hart, vs2, plan.group_registers);
    let vs1_bytes = read_register_group(hart, vs1, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_result_vv(
        &plan,
        &mut result,
        &vs2_bytes,
        &vs1_bytes,
        &carry,
        operation,
    );
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_result_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    operation: CarryBorrowOp,
) -> bool {
    if vd.index() == 0 {
        return false;
    }
    let Some(plan) = VectorBinaryPlan::new(hart, vd, &[vs2]) else {
        return false;
    };
    if sources_overlap_mask(&[vs2], plan.group_registers) {
        return false;
    }
    let carry = hart.read_vector(VectorRegister::from_field(0));
    let vs2_bytes = read_register_group(hart, vs2, plan.group_registers);
    let mut result = read_register_group(hart, vd, plan.group_registers);
    apply_result_vx(&plan, &mut result, &vs2_bytes, scalar, &carry, operation);
    write_register_group(hart, vd, plan.group_registers, &result);
    true
}

fn execute_mask_vv(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    vs1: VectorRegister,
    mask: RiscvVectorMaskMode,
    operation: CarryBorrowOp,
) -> bool {
    let Some(plan) = CarryMaskPlan::new(hart, vd, &[vs2, vs1], mask) else {
        return false;
    };
    let carry = carry_input(hart, mask);
    let vs2_bytes = read_register_group(hart, vs2, plan.group_registers);
    let vs1_bytes = read_register_group(hart, vs1, plan.group_registers);
    let mut result = hart.read_vector(vd);
    apply_mask_vv(
        &plan,
        &mut result,
        &vs2_bytes,
        &vs1_bytes,
        carry.as_ref(),
        operation,
    );
    hart.write_vector(vd, result);
    true
}

fn execute_mask_vx(
    hart: &mut RiscvHartState,
    vd: VectorRegister,
    vs2: VectorRegister,
    scalar: u64,
    mask: RiscvVectorMaskMode,
    operation: CarryBorrowOp,
) -> bool {
    let Some(plan) = CarryMaskPlan::new(hart, vd, &[vs2], mask) else {
        return false;
    };
    let carry = carry_input(hart, mask);
    let vs2_bytes = read_register_group(hart, vs2, plan.group_registers);
    let mut result = hart.read_vector(vd);
    apply_mask_vx(
        &plan,
        &mut result,
        &vs2_bytes,
        scalar,
        carry.as_ref(),
        operation,
    );
    hart.write_vector(vd, result);
    true
}

fn carry_input(
    hart: &RiscvHartState,
    mask: RiscvVectorMaskMode,
) -> Option<[u8; RISCV_VECTOR_REGISTER_BYTES]> {
    mask.is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)))
}

fn apply_result_vv(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    vs2: &[u8; MAX_VECTOR_GROUP_BYTES],
    vs1: &[u8; MAX_VECTOR_GROUP_BYTES],
    carry: &[u8; RISCV_VECTOR_REGISTER_BYTES],
    operation: CarryBorrowOp,
) {
    for element_index in 0..plan.active_element_count() {
        let offset = element_index * plan.element_bytes;
        let range = offset..offset + plan.element_bytes;
        let left = lane_bytes_to_u128(&vs2[range.clone()]);
        let right = lane_bytes_to_u128(&vs1[range.clone()]);
        let carry = u128::from(read_mask_bit(carry, element_index));
        let value = operation.result(left, right, carry, plan.element_bytes);
        write_u128_lane(&mut result[range], value);
    }
}

fn apply_result_vx(
    plan: &VectorBinaryPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    vs2: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
    carry: &[u8; RISCV_VECTOR_REGISTER_BYTES],
    operation: CarryBorrowOp,
) {
    let right = u128::from(scalar) & element_mask(plan.element_bytes);
    for element_index in 0..plan.active_element_count() {
        let offset = element_index * plan.element_bytes;
        let range = offset..offset + plan.element_bytes;
        let left = lane_bytes_to_u128(&vs2[range.clone()]);
        let carry = u128::from(read_mask_bit(carry, element_index));
        let value = operation.result(left, right, carry, plan.element_bytes);
        write_u128_lane(&mut result[range], value);
    }
}

fn apply_mask_vv(
    plan: &CarryMaskPlan,
    result: &mut [u8; RISCV_VECTOR_REGISTER_BYTES],
    vs2: &[u8; MAX_VECTOR_GROUP_BYTES],
    vs1: &[u8; MAX_VECTOR_GROUP_BYTES],
    carry: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
    operation: CarryBorrowOp,
) {
    for element_index in 0..plan.active_elements {
        let offset = element_index * plan.element_bytes;
        let range = offset..offset + plan.element_bytes;
        let left = lane_bytes_to_u128(&vs2[range.clone()]);
        let right = lane_bytes_to_u128(&vs1[range]);
        let carry = carry.map_or(0, |mask| u128::from(read_mask_bit(mask, element_index)));
        write_mask_bit(
            result,
            element_index,
            operation.carry_or_borrow(left, right, carry, plan.element_bytes),
        );
    }
}

fn apply_mask_vx(
    plan: &CarryMaskPlan,
    result: &mut [u8; RISCV_VECTOR_REGISTER_BYTES],
    vs2: &[u8; MAX_VECTOR_GROUP_BYTES],
    scalar: u64,
    carry: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
    operation: CarryBorrowOp,
) {
    let right = u128::from(scalar) & element_mask(plan.element_bytes);
    for element_index in 0..plan.active_elements {
        let offset = element_index * plan.element_bytes;
        let left = lane_bytes_to_u128(&vs2[offset..offset + plan.element_bytes]);
        let carry = carry.map_or(0, |mask| u128::from(read_mask_bit(mask, element_index)));
        write_mask_bit(
            result,
            element_index,
            operation.carry_or_borrow(left, right, carry, plan.element_bytes),
        );
    }
}

impl CarryBorrowOp {
    fn result(self, left: u128, right: u128, carry: u128, element_bytes: usize) -> u128 {
        let mask = element_mask(element_bytes);
        match self {
            Self::Add => left.wrapping_add(right).wrapping_add(carry) & mask,
            Self::Sub => left.wrapping_sub(right).wrapping_sub(carry) & mask,
        }
    }

    fn carry_or_borrow(self, left: u128, right: u128, carry: u128, element_bytes: usize) -> bool {
        let mask = element_mask(element_bytes);
        match self {
            Self::Add => left + right + carry > mask,
            Self::Sub => left < right + carry,
        }
    }
}

struct CarryMaskPlan {
    element_bytes: usize,
    group_registers: usize,
    active_elements: usize,
}

impl CarryMaskPlan {
    fn new(
        hart: &RiscvHartState,
        vd: VectorRegister,
        sources: &[VectorRegister],
        mask: RiscvVectorMaskMode,
    ) -> Option<Self> {
        let config = hart.vector_config();
        let element_bytes = config.element_width_bytes()?;
        let group_registers = config.register_group_registers()?;
        if sources
            .iter()
            .any(|source| !valid_register_group(*source, group_registers))
            || sources
                .iter()
                .any(|source| !mask_destination_overlap_allowed(vd, *source, group_registers))
            || (mask.is_masked() && sources_overlap_mask(sources, group_registers))
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

fn mask_destination_overlap_allowed(
    vd: VectorRegister,
    source: VectorRegister,
    source_registers: usize,
) -> bool {
    !register_groups_overlap(vd, 1, source, source_registers) || vd.index() == source.index()
}

fn sources_overlap_mask(sources: &[VectorRegister], source_registers: usize) -> bool {
    sources.iter().any(|source| {
        register_groups_overlap(*source, source_registers, VectorRegister::from_field(0), 1)
    })
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

fn element_mask(element_bytes: usize) -> u128 {
    match element_bytes {
        1 => u128::from(u8::MAX),
        2 => u128::from(u16::MAX),
        4 => u128::from(u32::MAX),
        8 => u128::from(u64::MAX),
        _ => unreachable!("validated vector element width"),
    }
}
