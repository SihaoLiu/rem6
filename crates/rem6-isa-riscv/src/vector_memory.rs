use crate::{
    vector_group::{
        read_mask_bit, read_register_group, register_groups_overlap, valid_register_group,
        MAX_VECTOR_GROUP_BYTES,
    },
    MemoryAccessKind, MemoryWidth, RiscvHartState, RiscvVectorMaskMode,
    RiscvVectorMemoryInstruction, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};

const SUPPORTED_STRIDED_E32_M1_SHAPES: &[(usize, usize, usize)] = &[(2, 12, 16), (3, 6, 16)];

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
        RiscvVectorMemoryInstruction::LoadStrided {
            vd,
            rs1,
            rs2,
            width,
            mask,
        } => {
            let plan = strided_access_plan(hart, vd, width, rs2).ok_or(())?;
            if masked_load_overlaps_v0(mask, vd, plan.group_registers) {
                return Err(());
            }
            if plan.element_count == 0 {
                if mask.is_masked() {
                    return Err(());
                }
                return Ok(None);
            }
            let byte_mask = strided_compact_byte_mask(hart, mask, &plan);
            if byte_mask
                .as_ref()
                .is_some_and(|mask| !mask.iter().any(|active| *active))
            {
                return Ok(None);
            }

            Ok(Some(MemoryAccessKind::VectorLoadStrided {
                vd,
                address: hart.read(rs1),
                width,
                stride: plan.stride,
                element_count: plan.element_count,
                span_len: plan.span_len,
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
        RiscvVectorMemoryInstruction::StoreStrided {
            vs3,
            rs1,
            rs2,
            width,
            mask,
        } => {
            let plan = strided_access_plan(hart, vs3, width, rs2).ok_or(())?;
            if plan.element_count == 0 {
                if mask.is_masked() {
                    return Err(());
                }
                return Ok(None);
            }
            let group = read_register_group(hart, vs3, plan.group_registers);
            let compact_byte_mask = strided_compact_byte_mask(hart, mask, &plan);
            if compact_byte_mask
                .as_ref()
                .is_some_and(|mask| !mask.iter().any(|active| *active))
            {
                return Ok(None);
            }
            let (data, byte_mask) =
                strided_store_payload(&group, &plan, compact_byte_mask.as_deref());

            Ok(Some(MemoryAccessKind::VectorStoreStrided {
                address: hart.read(rs1),
                width,
                stride: plan.stride,
                element_count: plan.element_count,
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

struct StridedAccessPlan {
    element_count: usize,
    element_bytes: usize,
    stride: usize,
    span_len: usize,
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

fn strided_access_plan(
    hart: &RiscvHartState,
    register: VectorRegister,
    width: MemoryWidth,
    stride_register: crate::Register,
) -> Option<StridedAccessPlan> {
    if width != MemoryWidth::Word {
        return None;
    }
    let config = hart.vector_config();
    let vlmul = config.vtype() & 0x7;
    let group_registers = config.register_group_registers()?;
    if config.vill()
        || vlmul != 0
        || group_registers != 1
        || config.element_width_bytes()? != width.bytes()
        || !valid_register_group(register, group_registers)
    {
        return None;
    }

    let element_count = config.vl() as usize;
    let element_bytes = width.bytes();
    let stride = usize::try_from(hart.read(stride_register)).ok()?;
    if element_count == 0 {
        return Some(StridedAccessPlan {
            element_count,
            element_bytes,
            stride,
            span_len: 0,
            group_registers,
        });
    }
    if stride < element_bytes {
        return None;
    }
    let span_len = (element_count - 1)
        .checked_mul(stride)?
        .checked_add(element_bytes)?;
    if !supported_strided_e32_m1_shape(element_count, stride, span_len) {
        return None;
    }
    let group_bytes = group_registers.checked_mul(RISCV_VECTOR_REGISTER_BYTES)?;
    (element_count.checked_mul(element_bytes)? <= group_bytes && span_len <= group_bytes).then_some(
        StridedAccessPlan {
            element_count,
            element_bytes,
            stride,
            span_len,
            group_registers,
        },
    )
}

fn supported_strided_e32_m1_shape(element_count: usize, stride: usize, span_len: usize) -> bool {
    SUPPORTED_STRIDED_E32_M1_SHAPES
        .iter()
        .copied()
        .any(|shape| shape == (element_count, stride, span_len))
}

fn strided_compact_byte_mask(
    hart: &RiscvHartState,
    mask: RiscvVectorMaskMode,
    plan: &StridedAccessPlan,
) -> Option<Vec<bool>> {
    if !mask.is_masked() {
        return None;
    }

    let source = hart.read_vector(VectorRegister::new(0).expect("v0 is a valid vector register"));
    let mut byte_mask = vec![false; plan.element_count * plan.element_bytes];
    for element_index in 0..plan.element_count {
        if read_mask_bit(&source, element_index) {
            let offset = element_index * plan.element_bytes;
            byte_mask[offset..offset + plan.element_bytes].fill(true);
        }
    }
    Some(byte_mask)
}

fn strided_store_payload(
    source: &[u8],
    plan: &StridedAccessPlan,
    compact_byte_mask: Option<&[bool]>,
) -> (Vec<u8>, Vec<bool>) {
    let mut data = vec![0; plan.span_len];
    let mut byte_mask = vec![false; plan.span_len];
    for element_index in 0..plan.element_count {
        let source_offset = element_index * plan.element_bytes;
        let memory_offset = element_index * plan.stride;
        let active = compact_byte_mask
            .map(|mask| mask[source_offset])
            .unwrap_or(true);
        if !active {
            continue;
        }
        data[memory_offset..memory_offset + plan.element_bytes]
            .copy_from_slice(&source[source_offset..source_offset + plan.element_bytes]);
        byte_mask[memory_offset..memory_offset + plan.element_bytes].fill(true);
    }
    (data, byte_mask)
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
            || (width == MemoryWidth::Word
                && matches!(plan.group_registers, 2 | 4 | 8)
                && plan.byte_len == plan.group_registers * RISCV_VECTOR_REGISTER_BYTES))
}

fn masked_load_overlaps_v0(
    mask: RiscvVectorMaskMode,
    register: VectorRegister,
    group_registers: usize,
) -> bool {
    mask.is_masked()
        && register_groups_overlap(register, group_registers, VectorRegister::from_field(0), 1)
}
