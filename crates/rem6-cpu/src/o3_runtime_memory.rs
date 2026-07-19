use super::o3_runtime_writeback::O3LiveWritebackReady;
use super::*;
use crate::riscv_data_completion::RiscvDataCompletion;
use rem6_memory::{AccessSize, Address, AddressRange};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveDataAccess {
    pub(super) fetch_request: MemoryRequestId,
    pub(super) data_request: MemoryRequestId,
    pub(super) execution: RiscvCpuExecutionEvent,
    pub(super) sequence: u64,
    pub(super) lsq_sequence_span: u64,
    pub(super) issue_tick: u64,
    pub(super) issue_rob_occupancy: usize,
    pub(super) issue_lsq_occupancy: usize,
    pub(super) younger_window_policy: O3DataAccessWindowPolicy,
    pub(super) response_tick: Option<u64>,
    pub(super) latency_ticks: Option<u64>,
    pub(super) commit_tick: Option<u64>,
    pub(super) load_data: Option<Vec<u8>>,
    pub(super) memory_result: Option<RiscvDataCompletion>,
    pub(super) forwarding_plan: Option<O3StoreLoadForwardingPlan>,
    pub(super) outcome: O3LiveDataAccessOutcome,
    pub(super) event_taken: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3DataAccessWindowPolicy {
    None,
    ScalarMemoryPrefix,
    MemoryResultWindow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum O3LiveDataAccessOutcome {
    Resident,
    Completed,
    Retried,
    Failed,
}

struct O3LiveDataAccessResponse {
    response_tick: u64,
    latency_ticks: u64,
    load_data: Option<Vec<u8>>,
    expected_memory_result_identity: Option<(Address, AccessSize, usize)>,
    memory_result: Option<RiscvDataCompletion>,
    forwarding_plan: Option<O3StoreLoadForwardingPlan>,
}

pub(crate) fn is_scalar_window_access(access: &MemoryAccessKind) -> bool {
    matches!(
        access,
        MemoryAccessKind::Load { .. } | MemoryAccessKind::Store { .. }
    )
}

pub(crate) fn o3_memory_result_destination(
    access: &MemoryAccessKind,
) -> Option<(O3RegisterClass, u32)> {
    match access {
        MemoryAccessKind::Load { rd, .. }
        | MemoryAccessKind::LoadReserved { rd, .. }
        | MemoryAccessKind::AtomicMemory { rd, .. }
        | MemoryAccessKind::StoreConditional { rd, .. }
            if !rd.is_zero() =>
        {
            Some((O3RegisterClass::Integer, u32::from(rd.index())))
        }
        MemoryAccessKind::FloatLoad { rd, .. } => {
            Some((O3RegisterClass::FloatingPoint, u32::from(rd.index())))
        }
        MemoryAccessKind::VectorLoadUnitStride {
            vd,
            width: MemoryWidth::Doubleword,
            byte_len,
            byte_mask,
            group_registers: 1,
            fault_only_first: false,
            ..
        } if *byte_len > 0
            && *byte_len <= RISCV_VECTOR_REGISTER_BYTES
            && byte_mask
                .as_deref()
                .is_none_or(|mask| mask.iter().copied().any(|active| active)) =>
        {
            Some((O3RegisterClass::Vector, u32::from(vd.index())))
        }
        _ => None,
    }
}

pub(crate) fn o3_memory_result_window_destination(
    access: &MemoryAccessKind,
) -> Option<Option<Register>> {
    match access {
        MemoryAccessKind::Load { rd, .. }
        | MemoryAccessKind::LoadReserved { rd, .. }
        | MemoryAccessKind::AtomicMemory { rd, .. }
            if !rd.is_zero() =>
        {
            o3_memory_result_destination(access)?;
            Some(Some(*rd))
        }
        MemoryAccessKind::FloatLoad { .. } | MemoryAccessKind::VectorLoadUnitStride { .. } => {
            o3_memory_result_destination(access)?;
            Some(None)
        }
        _ => None,
    }
}

pub(crate) fn o3_memory_result_younger_read_destination(
    access: &MemoryAccessKind,
) -> Option<Option<Register>> {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride { .. } => {
            o3_memory_result_window_destination(access)
        }
        _ => None,
    }
}

pub(crate) fn o3_memory_result_younger_buffered_effect_destination(
    access: &MemoryAccessKind,
) -> Option<Option<Register>> {
    match access {
        MemoryAccessKind::AtomicMemory {
            rd,
            acquire: false,
            release: false,
            ..
        } if !rd.is_zero() => o3_memory_result_window_destination(access),
        _ => None,
    }
}

pub(crate) fn o3_memory_result_pure_read_destination(
    access: &MemoryAccessKind,
) -> Option<Option<Register>> {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride { .. } => {
            o3_memory_result_window_destination(access)
        }
        _ => None,
    }
}

pub(crate) fn o3_memory_result_range(access: &MemoryAccessKind) -> Option<AddressRange> {
    let (address, size) = memory_result_address_size(access)?;
    AddressRange::new(address, size).ok()
}

pub(crate) fn is_deferred_o3_data_access(access: Option<&MemoryAccessKind>) -> bool {
    access.is_some_and(|access| {
        matches!(access, MemoryAccessKind::StoreConditional { .. })
            || is_scalar_window_access(access)
            || o3_memory_result_destination(access).is_some()
    })
}

fn live_data_access_younger_window_policy_matches(
    access: &MemoryAccessKind,
    policy: O3DataAccessWindowPolicy,
) -> bool {
    match policy {
        O3DataAccessWindowPolicy::None => true,
        O3DataAccessWindowPolicy::ScalarMemoryPrefix => matches!(
            access,
            MemoryAccessKind::Load { rd, .. } if !rd.is_zero()
        ),
        O3DataAccessWindowPolicy::MemoryResultWindow => {
            o3_memory_result_window_destination(access).is_some()
        }
    }
}

#[cfg(test)]
fn test_memory_result_completion(
    execution: &RiscvCpuExecutionEvent,
    load_data: Option<&[u8]>,
) -> Option<RiscvDataCompletion> {
    let access = execution.execution().memory_access()?.clone();
    o3_memory_result_destination(&access)?;
    let (physical_address, size) = memory_result_address_size(&access)?;
    Some(RiscvDataCompletion::from_issued_response(
        execution.fetch().request_id(),
        access,
        physical_address,
        size,
        0,
        load_data.map(ToOwned::to_owned),
    ))
}

fn memory_result_address_size(access: &MemoryAccessKind) -> Option<(Address, AccessSize)> {
    match access {
        MemoryAccessKind::Load { address, width, .. }
        | MemoryAccessKind::LoadReserved { address, width, .. }
        | MemoryAccessKind::AtomicMemory { address, width, .. }
        | MemoryAccessKind::StoreConditional { address, width, .. }
        | MemoryAccessKind::FloatLoad { address, width, .. } => Some((
            Address::new(*address),
            AccessSize::new(width.bytes() as u64).ok()?,
        )),
        MemoryAccessKind::VectorLoadUnitStride {
            address, byte_len, ..
        } => Some((
            Address::new(*address),
            AccessSize::new(*byte_len as u64).ok()?,
        )),
        _ => None,
    }
}

pub(super) fn is_deferred_o3_data_instruction(instruction: RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::Load { .. }
            | RiscvInstruction::Store { .. }
            | RiscvInstruction::FloatLoad { .. }
            | RiscvInstruction::LoadReserved { .. }
            | RiscvInstruction::StoreConditional { .. }
            | RiscvInstruction::AtomicMemory { .. }
            | RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadUnitStride { .. })
    )
}

pub(super) fn o3_instruction_sequence_span(access: Option<&MemoryAccessKind>) -> u64 {
    if matches!(access, Some(MemoryAccessKind::AtomicMemory { .. })) {
        2
    } else {
        1
    }
}

pub(super) fn is_terminal_o3_data_access_event(execution: &RiscvCpuExecutionEvent) -> bool {
    is_deferred_o3_data_access(execution.execution().memory_access())
        && matches!(
            execution.data_access_event_kind(),
            Some(
                RiscvDataAccessEventKind::Completed
                    | RiscvDataAccessEventKind::ConditionalFailed
                    | RiscvDataAccessEventKind::Retry
                    | RiscvDataAccessEventKind::Failed
            )
        )
}

impl O3RuntimeState {
    #[cfg(test)]
    pub(crate) fn live_data_access_younger_window_policy(
        &self,
    ) -> Option<O3DataAccessWindowPolicy> {
        self.live_data_accesses
            .first()
            .map(|live| live.younger_window_policy)
    }

    #[cfg(test)]
    pub(crate) fn live_data_access_younger_window_policies(&self) -> Vec<O3DataAccessWindowPolicy> {
        self.live_data_accesses
            .iter()
            .map(|live| live.younger_window_policy)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn has_pending_store_forwarding_load_match(&self) -> bool {
        self.store_forwarding_window.pending_load_match.is_some()
    }

    pub(crate) fn live_data_access_lifecycle_is_quiescent(&self) -> bool {
        self.deferred_live_data_access_execution.is_none()
            && self.live_data_accesses.is_empty()
            && self.live_data_access_younger_sequences.is_empty()
    }

    pub(crate) fn has_pending_live_data_access_retirement(&self) -> bool {
        self.pending_live_data_access_retirement_count() > 0
    }

    pub(crate) fn pending_live_data_access_retirement_count(&self) -> usize {
        self.live_data_accesses.len()
            + usize::from(self.deferred_live_data_access_execution.is_some())
    }

    pub(crate) fn owns_pending_live_data_access_retirement(
        &self,
        fetch_request: MemoryRequestId,
    ) -> bool {
        self.deferred_live_data_access_execution == Some(fetch_request)
            || self
                .live_data_accesses
                .iter()
                .any(|live| live.fetch_request == fetch_request)
    }

    pub(crate) fn has_live_data_access(&self) -> bool {
        !self.live_data_accesses.is_empty()
    }

    pub(crate) fn has_live_data_access_window(&self) -> bool {
        !self.live_data_accesses.is_empty() || !self.live_data_access_younger_sequences.is_empty()
    }

    pub(crate) fn has_ready_live_data_access_event(&self) -> bool {
        self.live_data_accesses.first().is_some_and(|live| {
            live.outcome != O3LiveDataAccessOutcome::Resident && !live.event_taken
        })
    }

    pub(crate) fn earliest_unpublished_memory_result_writeback_tick(&self) -> Option<u64> {
        let live = self.live_data_accesses.first()?;
        if live.outcome != O3LiveDataAccessOutcome::Completed || live.event_taken {
            return None;
        }
        self.memory_result_writeback_reservation(live.sequence)
            .map(|reservation| reservation.admitted_tick())
    }

    fn live_data_access_publication_tick(&self, live: &O3LiveDataAccess) -> Option<u64> {
        if live.outcome != O3LiveDataAccessOutcome::Completed {
            return None;
        }
        self.memory_result_writeback_reservation(live.sequence)
            .map(|reservation| reservation.admitted_tick())
            .or(live.response_tick)
    }

    pub(crate) fn ready_live_data_access_event_kind(&self) -> Option<RiscvDataAccessEventKind> {
        let live = self.live_data_accesses.first()?;
        if live.outcome == O3LiveDataAccessOutcome::Resident || live.event_taken {
            return None;
        }
        live.execution.data_access_event_kind()
    }

    pub(crate) fn ready_live_data_access_completion_timing(
        &self,
    ) -> Option<(MemoryRequestId, u64, u64)> {
        let live = self.live_data_accesses.first()?;
        if live.outcome != O3LiveDataAccessOutcome::Completed || live.event_taken {
            return None;
        }
        Some((
            live.fetch_request,
            live.issue_tick,
            self.live_data_access_publication_tick(live)
                .expect("completed live data access has a publication tick"),
        ))
    }

    pub(crate) fn replace_ready_live_data_access_execution(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) -> bool {
        let Some(live) = self.live_data_accesses.first_mut() else {
            return false;
        };
        if live.fetch_request != execution.fetch().request_id()
            || live.outcome != O3LiveDataAccessOutcome::Completed
            || live.event_taken
        {
            return false;
        }
        live.execution = execution.clone();
        true
    }

    pub(crate) fn replace_live_data_access_execution(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) -> bool {
        let Some(live) = self
            .live_data_accesses
            .iter_mut()
            .find(|live| live.fetch_request == execution.fetch().request_id() && !live.event_taken)
        else {
            return false;
        };
        live.execution = execution.clone();
        true
    }

    pub(crate) fn defer_live_data_access_execution(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) -> bool {
        let Some(access) = execution.execution().memory_access() else {
            return false;
        };
        if !is_deferred_o3_data_access(Some(access)) {
            return false;
        }
        let fetch_request = execution.fetch().request_id();
        match self.deferred_live_data_access_execution {
            Some(pending) => pending == fetch_request,
            None => {
                if !self.live_data_accesses.is_empty()
                    && !((is_scalar_window_access(access)
                        && self.can_stage_scalar_memory(execution))
                        || self.can_stage_memory_result_window(execution))
                {
                    return false;
                }
                self.deferred_live_data_access_execution = Some(fetch_request);
                true
            }
        }
    }

    pub(crate) fn defer_live_data_access_if_detailed(
        &mut self,
        detailed: bool,
        execution: &RiscvCpuExecutionEvent,
    ) -> bool {
        !detailed
            || !is_deferred_o3_data_access(execution.execution().memory_access())
            || self.defer_live_data_access_execution(execution)
    }

    pub(crate) fn abort_deferred_live_data_access_execution(
        &mut self,
        fetch_request: MemoryRequestId,
    ) -> bool {
        if self.deferred_live_data_access_execution == Some(fetch_request) {
            self.deferred_live_data_access_execution = None;
            true
        } else {
            false
        }
    }

    pub(crate) const fn deferred_live_data_access_execution(&self) -> Option<MemoryRequestId> {
        self.deferred_live_data_access_execution
    }

    pub(crate) fn stage_live_data_access_issue(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        issue_tick: u64,
        younger_window_policy: O3DataAccessWindowPolicy,
    ) -> bool {
        let Some(access) = execution.execution().memory_access() else {
            return false;
        };
        if !is_deferred_o3_data_access(Some(access))
            || !live_data_access_younger_window_policy_matches(access, younger_window_policy)
        {
            return false;
        }
        let scalar_window = matches!(access, MemoryAccessKind::Store { .. })
            || younger_window_policy == O3DataAccessWindowPolicy::ScalarMemoryPrefix;
        let result_window = younger_window_policy == O3DataAccessWindowPolicy::MemoryResultWindow;
        if scalar_window && !self.has_scalar_memory_window_capacity() {
            return false;
        }
        if !self.live_data_accesses.is_empty()
            && !((scalar_window && self.can_stage_scalar_memory(execution))
                || (result_window && self.can_stage_memory_result_window(execution)))
        {
            return false;
        }
        if self
            .deferred_live_data_access_execution
            .is_some_and(|pending| pending != execution.fetch().request_id())
        {
            return false;
        }
        self.deferred_live_data_access_execution = None;

        let lsq_sequence_span = o3_instruction_sequence_span(Some(access));
        let sequence = self.allocate_sequence_span(lsq_sequence_span);
        let rename_destination = o3_memory_result_destination(access);
        let destination = rename_destination.map(|_| self.allocate_physical_register());
        self.snapshot.reorder_buffer.push(
            O3ReorderBufferEntry::new(
                sequence,
                Address::new(execution.execution().pc()),
                destination,
            )
            .with_live_staged_rename_destination(rename_destination),
        );
        self.live_staged_fetch_identities.insert(
            sequence,
            O3LiveStagedFetchIdentity::new(execution.instruction()),
        );
        self.snapshot
            .load_store_queue
            .extend(o3_lsq_entries(sequence, access));

        let issue_rob_occupancy = self.snapshot.reorder_buffer.len();
        let issue_lsq_occupancy = self.snapshot.load_store_queue.len();
        self.stats.observe_rob_occupancy(issue_rob_occupancy);
        self.stats.observe_lsq_occupancy(issue_lsq_occupancy);
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        self.live_data_accesses.push(O3LiveDataAccess {
            fetch_request: execution.fetch().request_id(),
            data_request,
            execution: execution.clone(),
            sequence,
            lsq_sequence_span,
            issue_tick,
            issue_rob_occupancy,
            issue_lsq_occupancy,
            younger_window_policy,
            response_tick: None,
            latency_ticks: None,
            commit_tick: None,
            load_data: None,
            memory_result: None,
            forwarding_plan: None,
            outcome: O3LiveDataAccessOutcome::Resident,
            event_taken: false,
        });
        true
    }

    #[cfg(test)]
    pub(crate) fn stage_live_data_access_issue_for_test(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        issue_tick: u64,
    ) -> bool {
        let Some(access) = execution.execution().memory_access() else {
            return false;
        };
        let younger_window_policy = match access {
            MemoryAccessKind::Load { rd, .. } if !rd.is_zero() => {
                O3DataAccessWindowPolicy::ScalarMemoryPrefix
            }
            _ => O3DataAccessWindowPolicy::None,
        };
        self.stage_live_data_access_issue(
            execution,
            data_request,
            issue_tick,
            younger_window_policy,
        )
    }

    pub(crate) fn complete_live_data_access_completion(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        response_tick: u64,
        latency_ticks: u64,
        expected_memory_result_identity: (Address, AccessSize, usize),
        completion: Option<RiscvDataCompletion>,
    ) -> Result<bool, O3RuntimeError> {
        let load_data = completion
            .as_ref()
            .and_then(|completion| completion.bytes().map(ToOwned::to_owned));
        self.complete_live_data_access(
            execution,
            data_request,
            O3LiveDataAccessResponse {
                response_tick,
                latency_ticks,
                load_data,
                expected_memory_result_identity: Some(expected_memory_result_identity),
                memory_result: completion,
                forwarding_plan: None,
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn complete_live_data_access_response(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        response_tick: u64,
        latency_ticks: u64,
        load_data: Option<&[u8]>,
    ) -> Result<bool, O3RuntimeError> {
        let completion = test_memory_result_completion(execution, load_data);
        let expected_memory_result_identity = completion.as_ref().map(|completion| {
            (
                completion.physical_address(),
                completion.size(),
                completion.request_byte_offset(),
            )
        });
        self.complete_live_data_access(
            execution,
            data_request,
            O3LiveDataAccessResponse {
                response_tick,
                latency_ticks,
                load_data: load_data.map(ToOwned::to_owned),
                expected_memory_result_identity,
                memory_result: completion,
                forwarding_plan: None,
            },
        )
    }

    pub(crate) fn complete_live_scalar_memory_forwarding_completion(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        response_tick: u64,
        latency_ticks: u64,
        expected_memory_result_identity: (Address, AccessSize, usize),
        completion: RiscvDataCompletion,
        forwarding_plan: O3StoreLoadForwardingPlan,
    ) -> Result<bool, O3RuntimeError> {
        let load_data = completion.bytes().map(ToOwned::to_owned);
        self.complete_live_data_access(
            execution,
            data_request,
            O3LiveDataAccessResponse {
                response_tick,
                latency_ticks,
                load_data,
                expected_memory_result_identity: Some(expected_memory_result_identity),
                memory_result: Some(completion),
                forwarding_plan: Some(forwarding_plan),
            },
        )
    }

    #[cfg(test)]
    pub(crate) fn complete_live_scalar_memory_forwarding(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        response_tick: u64,
        latency_ticks: u64,
        load_data: &[u8],
        forwarding_plan: O3StoreLoadForwardingPlan,
    ) -> Result<bool, O3RuntimeError> {
        let completion = test_memory_result_completion(execution, Some(load_data));
        let expected_memory_result_identity = completion.as_ref().map(|completion| {
            (
                completion.physical_address(),
                completion.size(),
                completion.request_byte_offset(),
            )
        });
        self.complete_live_data_access(
            execution,
            data_request,
            O3LiveDataAccessResponse {
                response_tick,
                latency_ticks,
                load_data: Some(load_data.to_vec()),
                expected_memory_result_identity,
                memory_result: completion,
                forwarding_plan: Some(forwarding_plan),
            },
        )
    }

    fn complete_live_data_access(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        response: O3LiveDataAccessResponse,
    ) -> Result<bool, O3RuntimeError> {
        let O3LiveDataAccessResponse {
            response_tick,
            latency_ticks,
            load_data,
            expected_memory_result_identity,
            memory_result,
            forwarding_plan,
        } = response;
        let Some(index) = self.live_data_accesses.iter().position(|live| {
            live.data_request == data_request
                && live.fetch_request == execution.fetch().request_id()
                && live.outcome == O3LiveDataAccessOutcome::Resident
        }) else {
            return Ok(false);
        };
        let sequence = self.live_data_accesses[index].sequence;
        let lsq_sequence_span = self.live_data_accesses[index].lsq_sequence_span;
        let event_kind = execution.data_access_event_kind();
        let access = execution.execution().memory_access();
        if let Some(completion) = memory_result.as_ref() {
            let Some((physical_address, size, request_byte_offset)) =
                expected_memory_result_identity
            else {
                return Ok(false);
            };
            let Some(access) = access else {
                return Ok(false);
            };
            if !completion.matches_issued_request(
                execution.fetch().request_id(),
                access,
                physical_address,
                size,
                request_byte_offset,
            ) {
                return Ok(false);
            }
        }
        if matches!(
            event_kind,
            Some(RiscvDataAccessEventKind::ConditionalFailed)
        ) && !matches!(access, Some(MemoryAccessKind::StoreConditional { .. }))
        {
            return Ok(false);
        }
        if matches!(
            event_kind,
            Some(RiscvDataAccessEventKind::Completed | RiscvDataAccessEventKind::ConditionalFailed)
        ) && memory_result
            .as_ref()
            .is_some_and(|completion| Some(completion.data_event_kind()) != event_kind)
        {
            return Ok(false);
        }
        if matches!(access, Some(MemoryAccessKind::StoreConditional { .. }))
            && matches!(
                event_kind,
                Some(
                    RiscvDataAccessEventKind::Completed
                        | RiscvDataAccessEventKind::ConditionalFailed
                )
            )
            && memory_result.is_none()
        {
            return Ok(false);
        }

        let outcome = match event_kind {
            Some(
                RiscvDataAccessEventKind::Completed | RiscvDataAccessEventKind::ConditionalFailed,
            ) => {
                let Some(rob_index) = self
                    .snapshot
                    .reorder_buffer
                    .iter()
                    .position(|entry| entry.sequence() == sequence)
                else {
                    return Ok(false);
                };
                let lsq_end = sequence.saturating_add(lsq_sequence_span);
                let lsq_rows = self
                    .snapshot
                    .load_store_queue
                    .iter()
                    .filter(|entry| entry.sequence() >= sequence && entry.sequence() < lsq_end)
                    .count();
                if lsq_rows != usize::try_from(lsq_sequence_span).unwrap_or(usize::MAX) {
                    return Ok(false);
                }
                let memory_result = memory_result.filter(|result| {
                    o3_memory_result_destination(result.access()).is_some()
                        || matches!(result.access(), MemoryAccessKind::StoreConditional { .. })
                });
                let has_result_destination = memory_result
                    .as_ref()
                    .is_some_and(|result| o3_memory_result_destination(result.access()).is_some());
                if has_result_destination
                    && self.snapshot.reorder_buffer[rob_index]
                        .rename_destination()
                        .is_some()
                {
                    let raw_ready_tick = response_tick.checked_add(1).ok_or(
                        O3RuntimeError::WritebackTickOverflow {
                            tick: response_tick,
                        },
                    )?;
                    self.reserve_writeback_completions([O3LiveWritebackReady::memory_result(
                        sequence,
                        raw_ready_tick,
                    )])?
                    .into_iter()
                    .next()
                    .expect("single memory-result reservation returns one row");
                }
                for entry in &mut self.snapshot.load_store_queue {
                    if entry.sequence() >= sequence && entry.sequence() < lsq_end {
                        entry.mark_completed();
                    }
                }
                let live = &mut self.live_data_accesses[index];
                live.memory_result = memory_result;
                O3LiveDataAccessOutcome::Completed
            }
            Some(RiscvDataAccessEventKind::Retry) => O3LiveDataAccessOutcome::Retried,
            Some(RiscvDataAccessEventKind::Failed) => O3LiveDataAccessOutcome::Failed,
            Some(RiscvDataAccessEventKind::Issued) | None => return Ok(false),
        };

        let live = &mut self.live_data_accesses[index];
        live.execution = execution.clone();
        live.response_tick = Some(response_tick);
        live.latency_ticks = Some(latency_ticks);
        live.commit_tick = None;
        live.load_data = load_data;
        live.forwarding_plan = forwarding_plan;
        live.outcome = outcome;
        live.event_taken = false;
        let remove_rows = matches!(
            outcome,
            O3LiveDataAccessOutcome::Retried | O3LiveDataAccessOutcome::Failed
        );
        if remove_rows {
            for stale in self.live_data_accesses.iter().skip(index + 1) {
                self.pending_data_accesses.remove(&stale.fetch_request);
            }
            self.live_data_accesses.truncate(index + 1);
            self.discard_live_data_access_window_rows(sequence);
        }
        Ok(true)
    }

    pub(crate) fn younger_live_scalar_memory_requests(
        &self,
        fetch_request: MemoryRequestId,
        data_request: MemoryRequestId,
    ) -> Vec<(MemoryRequestId, MemoryRequestId)> {
        let Some(index) = self.live_data_accesses.iter().position(|live| {
            live.fetch_request == fetch_request && live.data_request == data_request
        }) else {
            return Vec::new();
        };
        self.live_data_accesses
            .iter()
            .skip(index + 1)
            .map(|live| (live.data_request, live.fetch_request))
            .collect()
    }

    pub(crate) fn take_ready_live_data_access_event(
        &mut self,
        current_tick: u64,
    ) -> Option<RiscvCpuExecutionEvent> {
        let (event, publication_tick) = {
            let live = self.live_data_accesses.first()?;
            if live.outcome == O3LiveDataAccessOutcome::Resident || live.event_taken {
                return None;
            }
            let publication_tick = if live.outcome == O3LiveDataAccessOutcome::Completed {
                Some(
                    self.live_data_access_publication_tick(live)
                        .expect("completed live data access has a publication tick"),
                )
            } else {
                None
            };
            (live.execution.clone(), publication_tick)
        };
        if let Some(publication_tick) = publication_tick {
            if publication_tick > current_tick {
                return None;
            }
            let live = self
                .live_data_accesses
                .first_mut()
                .expect("ready live data access remains resident");
            let rob = self
                .snapshot
                .reorder_buffer
                .iter_mut()
                .find(|entry| entry.sequence() == live.sequence)?;
            rob.mark_ready_at(publication_tick);
            live.commit_tick =
                Some(publication_tick.max(self.last_live_commit_tick.unwrap_or(publication_tick)));
        }
        let live = self
            .live_data_accesses
            .first_mut()
            .expect("ready live data access remains resident");
        live.event_taken = true;
        Some(event)
    }

    pub(crate) fn live_data_access_publication_is_admitted(&self, current_tick: u64) -> bool {
        let Some(live) = self.live_data_accesses.first() else {
            return false;
        };
        live.outcome != O3LiveDataAccessOutcome::Resident
            && !live.event_taken
            && (live.outcome != O3LiveDataAccessOutcome::Completed
                || self
                    .live_data_access_publication_tick(live)
                    .expect("completed live data access has publication tick")
                    <= current_tick)
    }

    pub(super) fn consume_live_data_access_retirement(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) -> Option<(O3LiveDataAccess, Option<u64>)> {
        let live = self.live_data_accesses.first()?;
        if live.fetch_request != execution.fetch().request_id()
            || live.execution != *execution
            || !live.event_taken
            || live.outcome == O3LiveDataAccessOutcome::Resident
        {
            return None;
        }
        let admitted_writeback_tick = self
            .memory_result_writeback_reservation(live.sequence)
            .map(|reservation| reservation.admitted_tick());
        let fetch_request = live.fetch_request;
        self.pending_data_accesses.remove(&fetch_request);
        let live = self.live_data_accesses.remove(0);
        if live.outcome == O3LiveDataAccessOutcome::Completed {
            self.last_live_commit_tick = live.commit_tick;
        }
        self.finalize_writeback_publication(live.sequence);
        Some((live, admitted_writeback_tick))
    }

    pub(crate) fn ready_live_memory_result_completion(&self) -> Option<RiscvDataCompletion> {
        let live = self.live_data_accesses.first()?;
        if live.outcome != O3LiveDataAccessOutcome::Completed || live.event_taken {
            return None;
        }
        live.memory_result.clone()
    }

    pub(crate) fn discard_live_data_access_suffix(
        &mut self,
        fetch_request: MemoryRequestId,
        data_request: MemoryRequestId,
    ) -> bool {
        let Some(index) = self.live_data_accesses.iter().position(|live| {
            live.fetch_request == fetch_request && live.data_request == data_request
        }) else {
            return false;
        };
        if index == 0 {
            return false;
        }
        let sequence = self.live_data_accesses[index].sequence;
        let removed = self.live_data_accesses.drain(index..).collect::<Vec<_>>();
        for live in &removed {
            self.pending_data_accesses.remove(&live.fetch_request);
        }
        if self
            .deferred_live_data_access_execution
            .is_some_and(|request| removed.iter().any(|live| live.fetch_request == request))
        {
            self.deferred_live_data_access_execution = None;
        }
        self.discard_live_data_access_window_rows(sequence);
        true
    }

    pub(crate) fn discard_live_data_access_lifecycle(&mut self) {
        self.discard_live_writeback_reservations();
        self.deferred_live_data_access_execution = None;
        let live = std::mem::take(&mut self.live_data_accesses);
        for live in &live {
            self.pending_data_accesses.remove(&live.fetch_request);
        }
        let boundary_sequence = live
            .first()
            .map(|live| live.sequence)
            .or_else(|| self.live_data_access_younger_sequences.first().copied());
        if let Some(sequence) = boundary_sequence {
            self.discard_live_data_access_window_rows(sequence);
        }
    }

    pub(super) fn discard_live_data_access_lifecycle_at(&mut self, now: u64) {
        self.deferred_live_data_access_execution = None;
        let live = std::mem::take(&mut self.live_data_accesses);
        for live in &live {
            self.pending_data_accesses.remove(&live.fetch_request);
        }
        let boundary_sequence = live
            .first()
            .map(|live| live.sequence)
            .or_else(|| self.live_data_access_younger_sequences.first().copied());
        if let Some(sequence) = boundary_sequence {
            self.discard_live_data_access_window_rows_at(sequence, now);
        }
    }

    fn discard_live_data_access_window_rows(&mut self, sequence: u64) {
        self.discard_live_staged_window_from(sequence);
        self.snapshot
            .load_store_queue
            .retain(|entry| entry.sequence() < sequence);
    }

    pub(super) fn discard_live_data_access_window_rows_at(&mut self, sequence: u64, now: u64) {
        self.discard_live_staged_window_from_at(sequence, now);
        self.snapshot
            .load_store_queue
            .retain(|entry| entry.sequence() < sequence);
    }

    pub(super) fn remove_live_data_access_rows(&mut self, sequence: u64, sequence_span: u64) {
        let sequence_end = sequence.saturating_add(sequence_span);
        self.snapshot
            .reorder_buffer
            .retain(|entry| entry.sequence() != sequence);
        self.live_staged_fetch_identities.remove(&sequence);
        self.snapshot
            .load_store_queue
            .retain(|entry| entry.sequence() < sequence || entry.sequence() >= sequence_end);
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
    }
}

impl crate::RiscvCore {
    pub fn o3_live_data_access_lifecycle_is_quiescent(&self) -> bool {
        self.with_o3_runtime(|runtime| runtime.live_data_access_lifecycle_is_quiescent())
    }

    pub fn has_pending_o3_live_data_access_retirement(&self) -> bool {
        self.with_o3_runtime(|runtime| runtime.has_pending_live_data_access_retirement())
    }

    pub fn pending_o3_live_data_access_retirement_count(&self) -> usize {
        self.with_o3_runtime(|runtime| runtime.pending_live_data_access_retirement_count())
    }

    pub fn owns_pending_o3_live_data_access_retirement(
        &self,
        fetch_request: MemoryRequestId,
    ) -> bool {
        self.with_o3_runtime(|runtime| {
            runtime.owns_pending_live_data_access_retirement(fetch_request)
        })
    }

    pub fn ready_o3_live_data_access_event_kind(&self) -> Option<RiscvDataAccessEventKind> {
        self.with_o3_runtime(|runtime| runtime.ready_live_data_access_event_kind())
    }

    pub(crate) fn clear_deferred_o3_live_data_access_execution(&self) -> bool {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(fetch_request) = state.o3_runtime.deferred_live_data_access_execution() else {
            return false;
        };
        state.abort_deferred_o3_live_data_access_execution(fetch_request)
    }
}
