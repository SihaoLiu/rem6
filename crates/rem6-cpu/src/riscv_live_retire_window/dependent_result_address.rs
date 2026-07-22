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
    let Some(head_reservation) = state
        .o3_runtime
        .live_data_access_head_reservation(head.fetch().request_id())
    else {
        return false;
    };
    let mut predecessor = head_completed.last_consumed_request();
    let mut next_pc = sequential_pc(&head_completed);
    let mut requests = Vec::with_capacity(2);
    let mut dependent_requests = Vec::with_capacity(2);
    let mut scheduled = Vec::with_capacity(3);
    let mut result_destinations = Vec::with_capacity(3);
    for _ in 0..2 {
        let Some(dependent) =
            completed_fetch_instruction_at(state, fetch_events, predecessor, next_pc)
        else {
            break;
        };
        let Some(authorization) =
            dependent_authorization(state, dependent.first_consumed_request())
        else {
            break;
        };
        let Some((producer_register, dependent_rd)) = pending_registers(&dependent, authorization)
        else {
            return false;
        };
        if result_destinations.is_empty() {
            result_destinations.push(producer_register);
        }
        result_destinations.push(dependent_rd);
        requests.push(O3PendingDataAddressRequest::new(
            predecessor,
            dependent.fetch().clone(),
            dependent.consumed_requests().to_vec(),
            dependent.decoded(),
            producer_register,
        ));
        dependent_requests.push(dependent.first_consumed_request());
        predecessor = dependent.last_consumed_request();
        next_pc = sequential_pc(&dependent);
        scheduled.push(dependent);
    }
    if requests.is_empty() {
        return false;
    }
    let suffix = accepted_suffix(
        state,
        fetch_events,
        predecessor,
        next_pc,
        &result_destinations,
    );
    let expected_staged = suffix.len().saturating_add(requests.len());
    let staged = state.o3_runtime.stage_pending_data_address_window(
        head.fetch().request_id(),
        requests,
        suffix
            .iter()
            .map(|instruction| (instruction.pc(), instruction.decoded().instruction())),
    );
    if staged != expected_staged {
        state.o3_runtime.discard_pending_data_address();
        return false;
    }

    scheduled.extend(suffix);
    let schedule_result = schedule_o3_live_speculative_younger_executions(
        state,
        head_reservation,
        &scheduled,
        issue_tick,
    );
    match schedule_result {
        Ok(true) => {
            for request in dependent_requests {
                state.memory_result_window_authorizations.remove(&request);
            }
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
    result_destinations: &[rem6_isa_riscv::Register],
) -> Vec<RiscvCompletedFetchInstruction> {
    let Some(mut window) = RiscvScalarIntegerLiveWindow::from_memory_results(
        result_destinations.iter().copied(),
        result_destinations.len(),
        state.o3_runtime.scalar_memory_window_limit(),
    ) else {
        return Vec::new();
    };
    let mut suffix = Vec::new();
    for _ in result_destinations.len()..state.o3_runtime.scalar_memory_window_limit() {
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
