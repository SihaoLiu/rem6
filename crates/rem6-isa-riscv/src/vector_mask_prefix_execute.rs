use crate::{
    vector_group::read_mask_bit, RiscvHartState, RiscvVectorConfig,
    RiscvVectorMaskPrefixInstruction, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn execute(
    hart: &mut RiscvHartState,
    instruction: RiscvVectorMaskPrefixInstruction,
) -> bool {
    let Some(plan) = MaskPrefixPlan::new(hart) else {
        return false;
    };
    let (vd, vs2, mask, operation) = match instruction {
        RiscvVectorMaskPrefixInstruction::BeforeFirst { vd, vs2, mask } => {
            (vd, vs2, mask, MaskPrefixOp::BeforeFirst)
        }
        RiscvVectorMaskPrefixInstruction::OnlyFirst { vd, vs2, mask } => {
            (vd, vs2, mask, MaskPrefixOp::OnlyFirst)
        }
        RiscvVectorMaskPrefixInstruction::IncludingFirst { vd, vs2, mask } => {
            (vd, vs2, mask, MaskPrefixOp::IncludingFirst)
        }
    };
    let source = hart.read_vector(vs2);
    let selector = mask
        .is_masked()
        .then(|| hart.read_vector(VectorRegister::from_field(0)));
    let mut result = hart.read_vector(vd);
    operation.apply(&plan, &mut result, &source, selector.as_ref());
    hart.write_vector(vd, result);
    true
}

#[derive(Clone, Copy)]
enum MaskPrefixOp {
    BeforeFirst,
    OnlyFirst,
    IncludingFirst,
}

impl MaskPrefixOp {
    fn apply(
        self,
        plan: &MaskPrefixPlan,
        result: &mut [u8; RISCV_VECTOR_REGISTER_BYTES],
        source: &[u8; RISCV_VECTOR_REGISTER_BYTES],
        selector: Option<&[u8; RISCV_VECTOR_REGISTER_BYTES]>,
    ) {
        let mut seen_one = false;
        for element_index in 0..plan.active_elements {
            if !selected(selector, element_index) {
                continue;
            }

            let source_bit = read_mask_bit(source, element_index);
            let result_bit = match self {
                Self::BeforeFirst => !seen_one && !source_bit,
                Self::OnlyFirst => !seen_one && source_bit,
                Self::IncludingFirst => !seen_one,
            };
            if source_bit {
                seen_one = true;
            }
            write_mask_bit(result, element_index, result_bit);
        }
    }
}

struct MaskPrefixPlan {
    active_elements: usize,
}

impl MaskPrefixPlan {
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

fn write_mask_bit(mask: &mut [u8; RISCV_VECTOR_REGISTER_BYTES], element_index: usize, value: bool) {
    let byte_index = element_index / 8;
    let bit = 1_u8 << (element_index % 8);
    if value {
        mask[byte_index] |= bit;
    } else {
        mask[byte_index] &= !bit;
    }
}
