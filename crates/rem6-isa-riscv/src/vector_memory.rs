use crate::{
    vector_group::{
        read_mask_bit, read_register_group, register_groups_overlap, valid_register_group,
        MAX_VECTOR_GROUP_BYTES,
    },
    MemoryAccessKind, MemoryWidth, RiscvHartState, RiscvVectorMaskMode,
    RiscvVectorMemoryInstruction, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn memory_access(
    hart: &RiscvHartState,
    instruction: RiscvVectorMemoryInstruction,
) -> Result<Option<MemoryAccessKind>, ()> {
    match instruction {
        RiscvVectorMemoryInstruction::LoadUnitStride {
            vd,
            rs1,
            width,
            mask,
        } => {
            let plan = unit_stride_access_plan(hart, vd, width).ok_or(())?;
            if masked_unit_stride_unsupported(mask, width, &plan)
                || masked_load_overlaps_v0(mask, vd, plan.group_registers)
            {
                return Err(());
            }
            if plan.byte_len == 0 {
                return Ok(None);
            }
            let byte_mask = active_byte_mask(hart, mask, &plan);
            if byte_mask
                .as_ref()
                .is_some_and(|mask| !mask.iter().any(|active| *active))
            {
                return Ok(None);
            }

            Ok(Some(MemoryAccessKind::VectorLoadUnitStride {
                vd,
                address: hart.read(rs1),
                width,
                byte_len: plan.byte_len,
                byte_mask,
                group_registers: plan.group_registers,
            }))
        }
        RiscvVectorMemoryInstruction::StoreUnitStride {
            vs3,
            rs1,
            width,
            mask,
        } => {
            let plan = unit_stride_access_plan(hart, vs3, width).ok_or(())?;
            if masked_unit_stride_unsupported(mask, width, &plan) {
                return Err(());
            }
            let group = read_register_group(hart, vs3, plan.group_registers);
            let data = group[..plan.byte_len].to_vec();
            if data.is_empty() {
                return Ok(None);
            }
            let byte_mask = active_byte_mask(hart, mask, &plan);
            if byte_mask
                .as_ref()
                .is_some_and(|mask| !mask.iter().any(|active| *active))
            {
                return Ok(None);
            }

            Ok(Some(MemoryAccessKind::VectorStoreUnitStride {
                address: hart.read(rs1),
                width,
                data,
                byte_mask,
                group_registers: plan.group_registers,
            }))
        }
    }
}

struct UnitStrideAccessPlan {
    byte_len: usize,
    element_bytes: usize,
    group_registers: usize,
}

fn unit_stride_access_plan(
    hart: &RiscvHartState,
    register: VectorRegister,
    width: MemoryWidth,
) -> Option<UnitStrideAccessPlan> {
    let config = hart.vector_config();
    let vlmul = config.vtype() & 0x7;
    if config.vill() || !matches!(vlmul, 0..=3) || config.element_width_bytes()? != width.bytes() {
        return None;
    }

    let group_registers = config.register_group_registers()?;
    if !valid_register_group(register, group_registers) {
        return None;
    }

    let element_bytes = width.bytes();
    let byte_len = (config.vl() as usize).checked_mul(element_bytes)?;
    let group_bytes = group_registers.checked_mul(RISCV_VECTOR_REGISTER_BYTES)?;
    (byte_len <= group_bytes && byte_len <= MAX_VECTOR_GROUP_BYTES).then_some(
        UnitStrideAccessPlan {
            byte_len,
            element_bytes,
            group_registers,
        },
    )
}

fn active_byte_mask(
    hart: &RiscvHartState,
    mask: RiscvVectorMaskMode,
    plan: &UnitStrideAccessPlan,
) -> Option<Vec<bool>> {
    if !mask.is_masked() {
        return None;
    }

    let source = hart.read_vector(VectorRegister::new(0).expect("v0 is a valid vector register"));
    let mut byte_mask = vec![false; plan.byte_len];
    for element_index in 0..plan.byte_len / plan.element_bytes {
        if read_mask_bit(&source, element_index) {
            let offset = element_index * plan.element_bytes;
            byte_mask[offset..offset + plan.element_bytes].fill(true);
        }
    }
    Some(byte_mask)
}

fn masked_unit_stride_unsupported(
    mask: RiscvVectorMaskMode,
    width: MemoryWidth,
    plan: &UnitStrideAccessPlan,
) -> bool {
    mask.is_masked()
        && !(plan.group_registers == 1
            || (width == MemoryWidth::Word && plan.group_registers == 2 && plan.byte_len == 32))
}

fn masked_load_overlaps_v0(
    mask: RiscvVectorMaskMode,
    register: VectorRegister,
    group_registers: usize,
) -> bool {
    mask.is_masked()
        && register_groups_overlap(register, group_registers, VectorRegister::from_field(0), 1)
}
