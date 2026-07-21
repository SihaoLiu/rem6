use rem6_isa_riscv::{MemoryWidth, RiscvInstruction};

use crate::{
    riscv_fetch_ahead::{
        O3MemoryResultWindowAuthorization, O3MemoryResultWindowRole, O3MemoryResultWindowRoute,
    },
    riscv_live_retire_window::RiscvCompletedFetchInstruction,
    RiscvCoreState,
};

pub(in crate::riscv_fetch_ahead) fn dependent_result_address_authorization(
    state: &RiscvCoreState,
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    row_limit: usize,
) -> Option<O3MemoryResultWindowAuthorization> {
    if !state.live_retire_gate.detailed_policy_enabled()
        || state.data_translation.is_some()
        || row_limit < 2
        || head_authorization.role() != O3MemoryResultWindowRole::Head
        || head_authorization.route() != O3MemoryResultWindowRoute::Memory
        || head_authorization.resolved_range().is_none()
    {
        return None;
    }
    let head_destination = match head.decoded().instruction() {
        RiscvInstruction::Load {
            rd,
            width: MemoryWidth::Doubleword,
            ..
        } if !rd.is_zero() => rd,
        RiscvInstruction::AtomicMemory {
            rd,
            acquire: false,
            release: false,
            ..
        } if !rd.is_zero() => rd,
        _ => return None,
    };
    if head_authorization.integer_destination() != Some(head_destination) {
        return None;
    }
    let RiscvInstruction::Load {
        rd,
        rs1,
        offset,
        width: MemoryWidth::Doubleword,
        ..
    } = younger.decoded().instruction()
    else {
        return None;
    };
    if younger.decoded().bytes() != 4 || rd.is_zero() || rs1 != head_destination {
        return None;
    }
    Some(O3MemoryResultWindowAuthorization::dependent(
        rd,
        rs1,
        MemoryWidth::Doubleword,
        offset,
    ))
}
