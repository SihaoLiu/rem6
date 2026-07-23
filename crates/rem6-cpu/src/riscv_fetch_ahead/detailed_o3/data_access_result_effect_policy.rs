use rem6_isa_riscv::{RiscvInstruction, RiscvVectorMemoryInstruction};

use crate::{
    o3_runtime::o3_scalar_integer_source_registers,
    riscv_fetch_ahead::{
        O3MemoryResultWindowAuthorization, O3MemoryResultWindowRole, O3MemoryResultWindowRoute,
    },
    riscv_live_retire_window::RiscvCompletedFetchInstruction,
};

use super::{
    data_access_result::data_access_result_authorization,
    data_access_result_translation::translated_younger_result_authorization,
    TranslatedMemoryFetchAhead,
};

pub(in crate::riscv_fetch_ahead) fn data_access_result_younger_authorization(
    state: &crate::RiscvCoreState,
    instruction: &RiscvCompletedFetchInstruction,
    translated: TranslatedMemoryFetchAhead,
) -> Option<O3MemoryResultWindowAuthorization> {
    if state.data_translation.is_some() {
        return translated_younger_result_authorization(state, instruction);
    }
    if translated != TranslatedMemoryFetchAhead::Disabled {
        return None;
    }
    let role = match instruction.decoded().instruction() {
        RiscvInstruction::Load { rd, .. } if !rd.is_zero() => O3MemoryResultWindowRole::YoungerRead,
        RiscvInstruction::FloatLoad { .. }
        | RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadUnitStride { .. }) => {
            O3MemoryResultWindowRole::YoungerRead
        }
        RiscvInstruction::AtomicMemory {
            rd,
            acquire: false,
            release: false,
            ..
        } if !rd.is_zero() => O3MemoryResultWindowRole::YoungerBufferedEffect,
        _ => return None,
    };
    data_access_result_authorization(
        state,
        instruction.first_consumed_request(),
        instruction.decoded().instruction(),
        instruction.decoded().bytes(),
        translated,
        role,
    )
}

pub(in crate::riscv_fetch_ahead) fn result_head_allows_younger_effect(
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    younger_authorization: O3MemoryResultWindowAuthorization,
) -> bool {
    if head_authorization.role() != O3MemoryResultWindowRole::Head
        || !younger_authorization.role().is_buffered_effect()
        || !matches!(
            head.decoded().instruction(),
            RiscvInstruction::Load { .. }
                | RiscvInstruction::FloatLoad { .. }
                | RiscvInstruction::VectorMemory(
                    RiscvVectorMemoryInstruction::LoadUnitStride { .. }
                )
        )
        || head_authorization
            .integer_destination()
            .is_some_and(|destination| {
                o3_scalar_integer_source_registers(&younger.decoded().instruction())
                    .contains(&destination)
            })
    {
        return false;
    }
    let RiscvInstruction::AtomicMemory {
        acquire, release, ..
    } = younger.decoded().instruction()
    else {
        return false;
    };
    !acquire
        && !release
        && head_authorization.route() == O3MemoryResultWindowRoute::Memory
        && younger_authorization.route() == O3MemoryResultWindowRoute::Memory
        && head_authorization
            .resolved_range()
            .zip(younger_authorization.resolved_range())
            .is_some_and(|(head, younger)| !head.overlaps(younger))
}
