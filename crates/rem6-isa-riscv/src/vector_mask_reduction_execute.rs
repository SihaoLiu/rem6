use crate::{
    vector_group::read_mask_bit, write_register, RegisterWrite, RiscvHartState, RiscvVectorConfig,
    RiscvVectorMaskReductionInstruction, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    register_writes: &mut Vec<RegisterWrite>,
    instruction: RiscvVectorMaskReductionInstruction,
) -> bool {
    let Some(plan) = MaskReductionPlan::new(hart) else {
        return false;
    };
    let (rd, vs2, mask, operation) = match instruction {
        RiscvVectorMaskReductionInstruction::PopCount { rd, vs2, mask } => {
            (rd, vs2, mask, MaskReductionOp::PopCount)
        }
        RiscvVectorMaskReductionInstruction::FirstSet { rd, vs2, mask } => {
            (rd, vs2, mask, MaskReductionOp::FirstSet)
        }
    };
    let source = hart.read_vector(vs2);
    let selector = mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let value = operation.apply(&plan, &source, selector.as_ref());
    write_register(hart, register_writes, rd, value);
    true
}

#[derive(Clone, Copy)]
enum MaskReductionOp {
    PopCount,
    FirstSet,
}

impl MaskReductionOp {
    fn apply(
        self,
        plan: &MaskReductionPlan,
        source: &[u8; RISCV_VECTOR_REGISTER_BYTES],
        selector: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
    ) -> u64 {
        match self {
            Self::PopCount => (0..plan.active_elements)
                .filter(|element_index| selected(selector, *element_index))
                .filter(|element_index| read_mask_bit(source, *element_index))
                .count() as u64,
            Self::FirstSet => (0..plan.active_elements)
                .find(|element_index| {
                    selected(selector, *element_index) && read_mask_bit(source, *element_index)
                })
                .map_or(u64::MAX, |element_index| element_index as u64),
        }
    }
}

struct MaskReductionPlan {
    active_elements: usize,
}

impl MaskReductionPlan {
    fn new(hart: &RiscvHartState) -> Option<Self> {
        let config = hart.vector_config();
        let _ = config.element_width_bytes()?;
        let active_elements = config.vl() as usize;
        let vlmax = RiscvVectorConfig::vlmax(config.vtype())? as usize;
        if active_elements > vlmax || active_elements.div_ceil(8) > RISCV_VECTOR_REGISTER_BYTES {
            return None;
        }
        Some(Self { active_elements })
    }
}

fn selected(mask: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>, element_index: usize) -> bool {
    mask.is_none_or(|mask| read_mask_bit(mask, element_index))
}
