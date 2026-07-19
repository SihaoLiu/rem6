use rem6_isa_riscv::{RiscvInstruction, RiscvVectorMaskMode, RiscvVectorMemoryInstruction};

use crate::{
    o3_runtime::o3_scalar_integer_source_registers,
    riscv_live_retire_window::RiscvCompletedFetchInstruction,
};

use crate::riscv_fetch_ahead::O3MemoryResultWindowAuthorization;

pub(in crate::riscv_fetch_ahead) fn result_head_allows_younger_read(
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    younger_authorization: O3MemoryResultWindowAuthorization,
) -> bool {
    if head_authorization
        .integer_destination()
        .is_some_and(|destination| {
            let instruction = younger.decoded().instruction();
            let mut sources = o3_scalar_integer_source_registers(&instruction);
            if let RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadUnitStride {
                rs1,
                ..
            }) = instruction
            {
                sources.push(rs1);
            }
            sources.contains(&destination)
        })
    {
        return false;
    }
    let head_writes_v0 = matches!(
        head.decoded().instruction(),
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadUnitStride {
            vd,
            ..
        }) if vd.index() == 0
    );
    let younger_reads_v0 = matches!(
        younger.decoded().instruction(),
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadUnitStride {
            mask: RiscvVectorMaskMode::Masked,
            ..
        })
    );
    if head_writes_v0 && younger_reads_v0 {
        return false;
    }
    match head.decoded().instruction() {
        RiscvInstruction::AtomicMemory {
            acquire, release, ..
        } => {
            !acquire
                && !release
                && !head_authorization
                    .physical_range()
                    .overlaps(younger_authorization.physical_range())
        }
        _ => true,
    }
}
