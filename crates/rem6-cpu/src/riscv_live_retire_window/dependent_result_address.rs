use rem6_isa_riscv::{MemoryWidth, RiscvInstruction};
use rem6_memory::{Address, MemoryRequestId};

use super::*;
use crate::{
    o3_runtime::O3PendingDataAddressRequest,
    riscv_fetch_ahead::{O3MemoryResultWindowAuthorization, O3MemoryResultWindowRole},
};

pub(super) fn stage_dependent_result_address_window(
    state: &mut RiscvCoreState,
    head: &RiscvCpuExecutionEvent,
    issue_tick: u64,
    fetch_events: &[CpuFetchEvent],
) -> bool {
    let Some(head_completed) = completed_fetch_instruction_starting_with(
        &state.executed_fetches,
        fetch_events,
        head.fetch(),
    ) else {
        return false;
    };
    let next_pc = sequential_pc(&head_completed);
    let Some(dependent) = completed_fetch_instruction_at(
        state,
        fetch_events,
        head_completed.last_consumed_request(),
        next_pc,
    ) else {
        return false;
    };
    let Some(authorization) = dependent_authorization(state, dependent.first_consumed_request())
    else {
        return false;
    };
    let Some((producer_register, dependent_rd)) = pending_registers(&dependent, authorization)
    else {
        return false;
    };
    let Some(head_reservation) = state
        .o3_runtime
        .live_data_access_head_reservation(head.fetch().request_id())
    else {
        return false;
    };

    let request = O3PendingDataAddressRequest::new(
        head_completed.last_consumed_request(),
        dependent.fetch().clone(),
        dependent.consumed_requests().to_vec(),
        dependent.decoded(),
        producer_register,
    );
    let suffix = accepted_suffix(
        state,
        fetch_events,
        dependent.last_consumed_request(),
        sequential_pc(&dependent),
        producer_register,
        dependent_rd,
    );
    let expected_staged = suffix.len().saturating_add(1);
    let staged = state.o3_runtime.stage_pending_data_address_window(
        head.fetch().request_id(),
        vec![request],
        suffix
            .iter()
            .map(|instruction| (instruction.pc(), instruction.decoded().instruction())),
    );
    if staged != expected_staged {
        state.o3_runtime.discard_pending_data_address();
        return false;
    }

    let dependent_request = dependent.first_consumed_request();
    let mut scheduled = Vec::with_capacity(expected_staged);
    scheduled.push(dependent);
    scheduled.extend(suffix);
    let schedule_result = schedule_o3_live_speculative_younger_executions(
        state,
        head_reservation,
        &scheduled,
        issue_tick,
    );
    match schedule_result {
        Ok(true) => {
            state
                .memory_result_window_authorizations
                .remove(&dependent_request);
            true
        }
        Ok(false) | Err(_) => {
            state.o3_runtime.discard_pending_data_address();
            false
        }
    }
}

fn dependent_authorization(
    state: &RiscvCoreState,
    request: MemoryRequestId,
) -> Option<O3MemoryResultWindowAuthorization> {
    state
        .memory_result_window_authorizations
        .get(&request)
        .copied()
        .filter(|authorization| {
            authorization.role() == O3MemoryResultWindowRole::YoungerDependentRead
        })
}

fn pending_registers(
    dependent: &RiscvCompletedFetchInstruction,
    authorization: O3MemoryResultWindowAuthorization,
) -> Option<(rem6_isa_riscv::Register, rem6_isa_riscv::Register)> {
    let (producer_register, width, immediate) = authorization.dependent_source()?;
    let RiscvInstruction::Load {
        rd,
        rs1,
        offset,
        width: MemoryWidth::Doubleword,
        ..
    } = dependent.decoded().instruction()
    else {
        return None;
    };
    (width == MemoryWidth::Doubleword
        && offset == immediate
        && rs1 == producer_register
        && authorization.integer_destination() == Some(rd)
        && !rd.is_zero())
    .then_some((producer_register, rd))
}

fn accepted_suffix(
    state: &RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    mut current_request: MemoryRequestId,
    mut pc: Address,
    producer_register: rem6_isa_riscv::Register,
    dependent_rd: rem6_isa_riscv::Register,
) -> Vec<RiscvCompletedFetchInstruction> {
    let Some(mut window) = RiscvScalarIntegerLiveWindow::from_memory_results(
        [producer_register, dependent_rd],
        2,
        state.o3_runtime.scalar_memory_window_limit(),
    ) else {
        return Vec::new();
    };
    let mut suffix = Vec::new();
    for _ in 0..2 {
        let Some(instruction) =
            completed_fetch_instruction_at(state, fetch_events, current_request, pc)
        else {
            break;
        };
        if window.classify_younger(instruction.decoded().instruction())
            == RiscvScalarIntegerYoungerDecision::Reject
        {
            break;
        }
        current_request = instruction.last_consumed_request();
        pc = sequential_pc(&instruction);
        suffix.push(instruction);
    }
    suffix
}

fn sequential_pc(instruction: &RiscvCompletedFetchInstruction) -> Address {
    Address::new(
        instruction
            .pc()
            .get()
            .wrapping_add(u64::from(instruction.decoded().bytes())),
    )
}
