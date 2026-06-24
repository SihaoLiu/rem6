use crate::{
    vector_group::{
        read_mask_bit, read_register_group, register_groups_overlap, valid_register_group,
        write_register_group, MAX_VECTOR_GROUP_BYTES,
    },
    RiscvHartState, RiscvVectorMaskIndexInstruction, RiscvVectorMaskMode, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorMaskIndexInstruction,
) -> bool {
    let Some(plan) = MaskIndexPlan::new(
        hart,
        instruction.destination(),
        instruction.mask(),
        instruction.source_mask(),
    ) else {
        return false;
    };
    let selector = instruction
        .mask()
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let mut result = read_register_group(hart, instruction.destination(), plan.group_registers);
    match instruction {
        RiscvVectorMaskIndexInstruction::Iota { vs2, .. } => {
            let source = hart.read_vector(vs2);
            apply_iota(&plan, &mut result, &source, selector.as_ref());
        }
        RiscvVectorMaskIndexInstruction::Id { .. } => {
            apply_id(&plan, &mut result, selector.as_ref());
        }
    }
    write_register_group(
        hart,
        instruction.destination(),
        plan.group_registers,
        &result,
    );
    true
}

impl RiscvVectorMaskIndexInstruction {
    fn destination(self) -> VectorRegister {
        match self {
            Self::Iota { vd, .. } | Self::Id { vd, .. } => vd,
        }
    }

    fn mask(self) -> RiscvVectorMaskMode {
        match self {
            Self::Iota { mask, .. } | Self::Id { mask, .. } => mask,
        }
    }

    fn source_mask(self) -> Option<VectorRegister> {
        match self {
            Self::Iota { vs2, .. } => Some(vs2),
            Self::Id { .. } => None,
        }
    }
}

struct MaskIndexPlan {
    element_bytes: usize,
    group_registers: usize,
    active_elements: usize,
}

impl MaskIndexPlan {
    fn new(
        hart: &RiscvHartState,
        destination: VectorRegister,
        mask: RiscvVectorMaskMode,
        source_mask: Option<VectorRegister>,
    ) -> Option<Self> {
        let config = hart.vector_config();
        let element_bytes = config.element_width_bytes()?;
        let group_registers = config.register_group_registers()?;
        if !valid_register_group(destination, group_registers)
            || (mask.is_masked() && register_group_overlaps_v0(destination, group_registers))
            || source_mask.is_some_and(|source| {
                register_groups_overlap(destination, group_registers, source, 1)
            })
        {
            return None;
        }

        let active_elements = config.vl() as usize;
        let active_bytes = active_elements.checked_mul(element_bytes)?;
        if active_bytes > group_registers * RISCV_VECTOR_REGISTER_BYTES
            || active_elements.div_ceil(8) > RISCV_VECTOR_REGISTER_BYTES
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

fn apply_iota(
    plan: &MaskIndexPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    source: &[u8; RISCV_VECTOR_REGISTER_BYTES],
    selector: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
) {
    let mut count = 0_u64;
    for element_index in 0..plan.active_elements {
        if !selected(selector, element_index) {
            continue;
        }

        write_index_lane(plan, result, element_index, count);
        if read_mask_bit(source, element_index) {
            count += 1;
        }
    }
}

fn apply_id(
    plan: &MaskIndexPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    selector: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
) {
    for element_index in 0..plan.active_elements {
        if selected(selector, element_index) {
            write_index_lane(plan, result, element_index, element_index as u64);
        }
    }
}

fn write_index_lane(
    plan: &MaskIndexPlan,
    result: &mut [u8; MAX_VECTOR_GROUP_BYTES],
    element_index: usize,
    value: u64,
) {
    let offset = element_index * plan.element_bytes;
    result[offset..offset + plan.element_bytes]
        .copy_from_slice(&value.to_le_bytes()[..plan.element_bytes]);
}

fn selected(mask: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>, element_index: usize) -> bool {
    mask.is_none_or(|mask| read_mask_bit(mask, element_index))
}

fn register_group_overlaps_v0(register: VectorRegister, group_registers: usize) -> bool {
    register_groups_overlap(register, group_registers, VectorRegister::from_field(0), 1)
}
