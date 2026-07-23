use rem6_isa_riscv::{
    MemoryWidth, RiscvInstruction, RiscvVectorMaskMode,
    RiscvVectorMemoryInstruction::LoadUnitStride,
};

use crate::{
    o3_runtime::o3_scalar_integer_source_registers,
    riscv_live_retire_window::RiscvCompletedFetchInstruction,
};

use crate::riscv_fetch_ahead::{O3MemoryResultWindowAuthorization, O3MemoryResultWindowRole};

fn exact_translated_result_pair_shape(
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
) -> bool {
    let (head, younger) = (head.decoded(), younger.decoded());
    let (
        RiscvInstruction::Load {
            rd: head_destination,
            width: MemoryWidth::Doubleword,
            ..
        },
        RiscvInstruction::Load {
            rd: younger_destination,
            width: MemoryWidth::Doubleword,
            ..
        },
    ) = (head.instruction(), younger.instruction())
    else {
        return false;
    };
    head.bytes() == 4
        && younger.bytes() == 4
        && !head_destination.is_zero()
        && !younger_destination.is_zero()
        && head_destination != younger_destination
}

pub(in crate::riscv_fetch_ahead) fn result_head_allows_younger_read(
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    younger_authorization: O3MemoryResultWindowAuthorization,
) -> bool {
    let head_translated = head_authorization.is_translated();
    let younger_translated = younger_authorization.is_translated();
    let translated_ranges_are_disjoint = head_authorization
        .virtual_range()
        .zip(younger_authorization.virtual_range())
        .is_some_and(|(head, younger)| !head.overlaps(younger));
    if head_authorization.role() != O3MemoryResultWindowRole::Head
        || younger_authorization.role() != O3MemoryResultWindowRole::YoungerRead
        || ((head_translated || younger_translated)
            && (!head_translated
                || !younger_translated
                || !exact_translated_result_pair_shape(head, younger)
                || !translated_ranges_are_disjoint))
        || head_authorization
            .integer_destination()
            .is_some_and(|destination| {
                let instruction = younger.decoded().instruction();
                let mut sources = o3_scalar_integer_source_registers(&instruction);
                if let RiscvInstruction::VectorMemory(LoadUnitStride { rs1, .. }) = instruction {
                    sources.push(rs1);
                }
                sources.contains(&destination)
            })
    {
        return false;
    }
    let head_writes_v0 = matches!(
        head.decoded().instruction(),
        RiscvInstruction::VectorMemory(LoadUnitStride { vd, .. }) if vd.index() == 0
    );
    let younger_reads_v0 = matches!(
        younger.decoded().instruction(),
        RiscvInstruction::VectorMemory(LoadUnitStride {
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
                && head_authorization
                    .resolved_range()
                    .zip(younger_authorization.resolved_range())
                    .is_some_and(|(head, younger)| !head.overlaps(younger))
        }
        _ => true,
    }
}
