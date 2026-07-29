use std::collections::{BTreeMap, BTreeSet};

use rem6_memory::MemoryRequestId;

use super::{invalid, RiscvO3LiveCheckpointError};

pub(super) struct LiveFetchProjection {
    pub(super) events: Vec<super::RiscvO3LiveCheckpointEvent>,
    pub(super) executed_fetch_requests: Vec<MemoryRequestId>,
    pub(super) issued_fetch_requests: Vec<MemoryRequestId>,
    pub(super) projected_hart: Option<rem6_isa_riscv::RiscvHartState>,
}

pub(super) fn select_live_completed_fetches<'a>(
    cpu_events: &'a [crate::CpuFetchEvent],
    executed_fetches: &BTreeSet<MemoryRequestId>,
    expected_requests: &[MemoryRequestId],
    pending_suffix_after: Option<MemoryRequestId>,
) -> Result<Vec<&'a crate::CpuFetchEvent>, RiscvO3LiveCheckpointError> {
    let expected = expected_requests.iter().copied().collect::<BTreeSet<_>>();
    if expected.len() != expected_requests.len() {
        return Err(invalid("live issue rows repeat a fetch request"));
    }

    let mut issued = BTreeSet::new();
    let mut completed = BTreeMap::new();
    for event in cpu_events {
        let request = event.request_id();
        let permitted_pending_suffix = pending_suffix_after.is_some_and(|pending| {
            request.agent() == pending.agent() && request.sequence() > pending.sequence()
        });
        if !expected.contains(&request)
            && !executed_fetches.contains(&request)
            && !permitted_pending_suffix
        {
            return Err(invalid(
                "operational fetch stream contains an unexecuted fetch",
            ));
        }
        match event.kind() {
            crate::CpuFetchEventKind::Issued => {
                if !issued.insert(request) {
                    return Err(invalid("operational fetch stream repeats an issued fetch"));
                }
            }
            crate::CpuFetchEventKind::Completed => {
                if !matches!(event.data().map(<[u8]>::len), Some(2 | 4))
                    || event.data().map(|bytes| bytes.len() as u64) != Some(event.size().bytes())
                    || completed.insert(request, event).is_some()
                {
                    return Err(invalid(
                        "operational fetch stream contains an invalid completed instruction",
                    ));
                }
            }
            crate::CpuFetchEventKind::Retry | crate::CpuFetchEventKind::Failed => {
                return Err(invalid(
                    "operational fetch stream contains an unsuccessful fetch",
                ));
            }
        }
    }
    if issued
        .iter()
        .any(|request| !completed.contains_key(request))
    {
        return Err(invalid("operational fetch stream retains a pending fetch"));
    }

    expected_requests
        .iter()
        .map(|request| {
            completed
                .get(request)
                .copied()
                .ok_or(invalid("live issue row has no completed fetch"))
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn project_live_fetches(
    cpu_events: &[crate::CpuFetchEvent],
    state: &crate::RiscvCoreState,
    owner_rows: &[(u64, MemoryRequestId)],
    expected_requests: &[MemoryRequestId],
    completed_result: Option<&super::RiscvO3LiveCheckpointCompletedFpLoad>,
    projected_pending_terminal_fetch: Option<MemoryRequestId>,
    pending_address: Option<&super::RiscvO3LiveCheckpointPendingDataAddress>,
    wake_partition: rem6_kernel::PartitionId,
) -> Result<LiveFetchProjection, RiscvO3LiveCheckpointError> {
    let completed_fetches = select_live_completed_fetches(
        cpu_events,
        &state.executed_fetches,
        expected_requests,
        pending_address.map(|pending| pending.fetch.request_id()),
    )?;
    if completed_fetches
        .iter()
        .any(|event| event.partition() != wake_partition)
    {
        return Err(invalid("scheduled wake does not match queue service"));
    }

    if let Some(pending) = pending_address {
        let expected_owner = [(pending.sequence, pending.fetch.request_id())];
        let [completed_fetch] = completed_fetches.as_slice() else {
            return Err(invalid(
                "pending store does not own exactly one completed fetch",
            ));
        };
        let raw: [u8; 4] = completed_fetch
            .data()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(invalid(
                "pending store fetch is not an uncompressed instruction",
            ))?;
        let decoded = rem6_isa_riscv::RiscvInstruction::decode_with_length(u32::from_le_bytes(raw))
            .map_err(|_| invalid("pending store instruction does not decode"))?;
        let pending_pc = pending.fetch.pc().get();
        let next_pc = pending_pc
            .checked_add(4)
            .ok_or(invalid("pending store PC overflows"))?;
        if owner_rows != expected_owner
            || expected_requests != [pending.fetch.request_id()]
            || **completed_fetch != pending.fetch
            || decoded.bytes() != 4
            || !matches!(
                decoded.instruction(),
                rem6_isa_riscv::RiscvInstruction::Store {
                    rs1,
                    rs2,
                    width: rem6_isa_riscv::MemoryWidth::Doubleword,
                    ..
                } if rs1 == pending.producer_register && !rs2.is_zero() && rs1 != rs2
            )
            || !state
                .o3_runtime
                .checkpoint_live_instruction_matches(pending.sequence, decoded.instruction())
            || state
                .events
                .iter()
                .any(|event| event.fetch().request_id() == pending.fetch.request_id())
            || state.executed_fetches.contains(&pending.fetch.request_id())
            || state
                .issued_data_for_fetches
                .contains(&pending.fetch.request_id())
            || !matches!(state.hart.pc(), pc if pc == pending_pc || pc == next_pc)
        {
            return Err(invalid("pending store fetch ownership is inconsistent"));
        }
        return Ok(LiveFetchProjection {
            events: Vec::new(),
            executed_fetch_requests: Vec::new(),
            issued_fetch_requests: Vec::new(),
            projected_hart: None,
        });
    }

    let mut events = Vec::with_capacity(expected_requests.len());
    let mut replay_hart = state.hart.clone();
    for ((sequence, request), completed_fetch) in owner_rows.iter().zip(completed_fetches) {
        let raw_bytes = completed_fetch
            .data()
            .ok_or(invalid("live issue fetch has no instruction bytes"))?;
        let mut padded = [0_u8; 4];
        padded[..raw_bytes.len()].copy_from_slice(raw_bytes);
        let decoded =
            rem6_isa_riscv::RiscvInstruction::decode_with_length(u32::from_le_bytes(padded))
                .map_err(|_| invalid("live issue instruction does not decode"))?;
        if usize::from(decoded.bytes()) != raw_bytes.len()
            || !state
                .o3_runtime
                .checkpoint_live_instruction_matches(*sequence, decoded.instruction())
        {
            return Err(invalid("live issue instruction disagrees with O3 owner"));
        }
        let source_events = state
            .events
            .iter()
            .filter(|event| event.fetch().request_id() == *request)
            .collect::<Vec<_>>();
        let event = if state.executed_fetches.contains(request) {
            let [source_event] = source_events.as_slice() else {
                return Err(invalid(
                    "executed live issue row lacks one canonical execution event",
                ));
            };
            if source_event.fetch() != completed_fetch
                || source_event.instruction() != decoded.instruction()
            {
                return Err(invalid("live issue source event disagrees with fetch"));
            }
            (*source_event).clone()
        } else if projected_pending_terminal_fetch == Some(*request) {
            if !source_events.is_empty() {
                return Err(invalid(
                    "pending terminal live issue row retains an execution history event",
                ));
            }
            state
                .pending_terminal_memory_result
                .as_ref()
                .expect("matched pending terminal result exists")
                .execution()
                .clone()
        } else {
            if !source_events.is_empty() {
                return Err(invalid(
                    "pending live issue row retains an execution history event",
                ));
            }
            replay_hart.set_pc(completed_fetch.pc().get());
            let execution = replay_hart
                .execute_decoded(decoded)
                .map_err(|_| invalid("live issue instruction does not replay"))?;
            crate::RiscvCpuExecutionEvent::new(
                completed_fetch.clone(),
                decoded.instruction(),
                execution,
            )
        };
        let projected = super::project_event(&event);
        let completed_load = completed_result.filter(|result| result.fetch_request == *request);
        let transient_matches = if let Some(result) = completed_load {
            projected.memory_access
                == Some(rem6_isa_riscv::MemoryAccessKind::FloatLoad {
                    rd: result.destination,
                    address: result.physical_address.get(),
                    width: result.width,
                })
                && projected.data_access_event_kind
                    == Some(crate::RiscvDataAccessEventKind::Completed)
        } else {
            projected.memory_access.is_none() && projected.data_access_event_kind.is_none()
        };
        if !transient_matches || projected.rebuild().as_ref() != Ok(&event) {
            return Err(invalid(
                "execution event contains unsupported transient state",
            ));
        }
        for write in &projected.register_writes {
            replay_hart.write(write.register(), write.value());
        }
        for write in &projected.float_register_writes {
            replay_hart.write_float(write.register(), write.value());
        }
        if let Some(result) = completed_load {
            let writeback = projected
                .memory_access
                .as_ref()
                .expect("validated completed FP memory access")
                .read_response_writeback(&result.response_bytes)
                .map_err(|_| invalid("completed FP response bytes do not match the load"))?
                .ok_or(invalid("completed FP load has no writeback"))?;
            if writeback.target()
                != rem6_isa_riscv::MemoryResponseWritebackTarget::Float(result.destination)
            {
                return Err(invalid("completed FP response has the wrong typed target"));
            }
            replay_hart.write_float(result.destination, writeback.value());
        }
        replay_hart.set_pc(projected.next_pc);
        events.push(projected);
    }
    let executed_fetch_requests = expected_requests
        .iter()
        .filter(|request| {
            state.executed_fetches.contains(request)
                || projected_pending_terminal_fetch == Some(**request)
        })
        .copied()
        .collect::<Vec<_>>();
    let issued_fetch_requests = expected_requests
        .iter()
        .filter(|request| state.issued_data_for_fetches.contains(request))
        .copied()
        .collect::<Vec<_>>();
    if let Some(result) = completed_result {
        if executed_fetch_requests != [result.fetch_request]
            || issued_fetch_requests != [result.fetch_request]
        {
            return Err(invalid(
                "completed FP result does not retain exact execution/data issue membership",
            ));
        }
    } else if !issued_fetch_requests.is_empty() {
        return Err(invalid("live replay carries unsupported data issue state"));
    }
    let projected_hart = projected_pending_terminal_fetch.map(|request| {
        let pending = state
            .pending_terminal_memory_result
            .as_ref()
            .filter(|pending| pending.owns_fetch(request))
            .expect("matched pending terminal result exists");
        let mut hart = state.hart.clone();
        hart.set_pc(pending.execution().execution().next_pc());
        hart
    });
    Ok(LiveFetchProjection {
        events,
        executed_fetch_requests,
        issued_fetch_requests,
        projected_hart,
    })
}
