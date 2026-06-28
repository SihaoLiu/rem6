use crate::{
    MemoryAccessKind, MemoryWidth, RiscvHartState, RiscvVectorMemoryInstruction, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

pub(crate) fn memory_access(
    hart: &RiscvHartState,
    instruction: RiscvVectorMemoryInstruction,
) -> Result<Option<MemoryAccessKind>, ()> {
    match instruction {
        RiscvVectorMemoryInstruction::LoadUnitStride { vd, rs1, width } => {
            let byte_len = unit_stride_access_bytes(hart, width).ok_or(())?;
            if byte_len == 0 {
                return Ok(None);
            }

            Ok(Some(MemoryAccessKind::VectorLoadUnitStride {
                vd,
                address: hart.read(rs1),
                width,
                byte_len,
            }))
        }
        RiscvVectorMemoryInstruction::StoreUnitStride { vs3, rs1, width } => {
            let Some(data) = unit_stride_store_data(hart, vs3, width) else {
                return Err(());
            };
            if data.is_empty() {
                return Ok(None);
            }

            Ok(Some(MemoryAccessKind::VectorStoreUnitStride {
                address: hart.read(rs1),
                width,
                data,
            }))
        }
    }
}

fn unit_stride_access_bytes(hart: &RiscvHartState, width: MemoryWidth) -> Option<usize> {
    let config = hart.vector_config();
    if config.vill()
        || config.vtype() & 0x7 != 0
        || config.element_width_bytes()? != width.bytes()
        || config.register_group_registers()? != 1
    {
        return None;
    }

    let byte_len = config.vl() as usize * width.bytes();
    (byte_len <= RISCV_VECTOR_REGISTER_BYTES).then_some(byte_len)
}

fn unit_stride_store_data(
    hart: &RiscvHartState,
    source: VectorRegister,
    width: MemoryWidth,
) -> Option<Vec<u8>> {
    let byte_len = unit_stride_access_bytes(hart, width)?;
    let register = hart.read_vector(source);
    Some(register[..byte_len].to_vec())
}
