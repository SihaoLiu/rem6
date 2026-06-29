use crate::{
    vector_group::{read_register_group, valid_register_group, MAX_VECTOR_GROUP_BYTES},
    MemoryAccessKind, MemoryWidth, RiscvHartState, RiscvVectorMemoryInstruction, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn memory_access(
    hart: &RiscvHartState,
    instruction: RiscvVectorMemoryInstruction,
) -> Result<Option<MemoryAccessKind>, ()> {
    match instruction {
        RiscvVectorMemoryInstruction::LoadUnitStride { vd, rs1, width } => {
            let plan = unit_stride_access_plan(hart, vd, width).ok_or(())?;
            if plan.byte_len == 0 {
                return Ok(None);
            }

            Ok(Some(MemoryAccessKind::VectorLoadUnitStride {
                vd,
                address: hart.read(rs1),
                width,
                byte_len: plan.byte_len,
                group_registers: plan.group_registers,
            }))
        }
        RiscvVectorMemoryInstruction::StoreUnitStride { vs3, rs1, width } => {
            let plan = unit_stride_access_plan(hart, vs3, width).ok_or(())?;
            let group = read_register_group(hart, vs3, plan.group_registers);
            let data = group[..plan.byte_len].to_vec();
            if data.is_empty() {
                return Ok(None);
            }

            Ok(Some(MemoryAccessKind::VectorStoreUnitStride {
                address: hart.read(rs1),
                width,
                data,
                group_registers: plan.group_registers,
            }))
        }
    }
}

struct UnitStrideAccessPlan {
    byte_len: usize,
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

    let byte_len = (config.vl() as usize).checked_mul(width.bytes())?;
    let group_bytes = group_registers.checked_mul(RISCV_VECTOR_REGISTER_BYTES)?;
    (byte_len <= group_bytes && byte_len <= MAX_VECTOR_GROUP_BYTES).then_some(
        UnitStrideAccessPlan {
            byte_len,
            group_registers,
        },
    )
}
