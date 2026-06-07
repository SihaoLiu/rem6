use std::collections::BTreeMap;

use rem6_cache::{
    MshrEntry, MshrHandle, MshrQosClass, MshrQueueConfig, MshrQueueSnapshot, MshrTarget,
    MshrTargetSource, MsiCacheBankSnapshot, MsiCacheControllerSnapshot, MsiPendingMissSnapshot,
};
use rem6_directory::{
    DirectoryDataSource, DirectoryDecision, DirectoryGrant, DirectoryLineState, DirectorySnoop,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAccessOrdering,
    MemoryBarrierSet, MemoryOperation, MemoryRequest, MemoryRequestId, ResponseStatus,
};
use rem6_protocol_msi::{MsiCacheLine, MsiEvent, MsiLineId, MsiState};

use crate::{
    CpuResponseRecord, MsiBankBackingLineSnapshot, MsiBankCycleAccepted, MsiBankCycleRun,
    MsiBankDirectoryHarnessSnapshot, SubmitKind, SubmitResult,
};

const FORMAT_VERSION: u64 = 5;
const U8_BYTES: usize = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const DIRECTORY_STATE_PREFIX_BYTES: usize = U64_BYTES + U8_BYTES + U64_BYTES;
const BACKING_LINE_PREFIX_BYTES: usize = U64_BYTES * 2;
const CPU_RESPONSE_PREFIX_BYTES: usize =
    U64_BYTES + U8_BYTES + U32_BYTES + U64_BYTES + U8_BYTES + U8_BYTES;
const DIRECTORY_DECISION_PREFIX_BYTES: usize =
    U64_BYTES + U32_BYTES + U64_BYTES + DIRECTORY_STATE_PREFIX_BYTES * 2 + U64_BYTES + U8_BYTES;
const PARALLEL_CYCLE_PREFIX_BYTES: usize = U64_BYTES * 3;
const CACHE_CONTROLLER_PREFIX_BYTES: usize =
    U32_BYTES + U64_BYTES + U8_BYTES + U64_BYTES * 2 + U8_BYTES + U64_BYTES + U8_BYTES;
const CACHE_LINE_TRACE_RECORD_BYTES: usize = U8_BYTES;
const MSHR_ENTRY_PREFIX_BYTES: usize = U64_BYTES * 4 + U8_BYTES * 2 + U64_BYTES;
const MEMORY_REQUEST_PREFIX_BYTES: usize =
    U32_BYTES + U64_BYTES + U8_BYTES + U64_BYTES * 3 + U8_BYTES * 4;
const MSHR_TARGET_PREFIX_BYTES: usize = MEMORY_REQUEST_PREFIX_BYTES + U64_BYTES * 2 + U8_BYTES * 3;
const DECISION_SNOOP_RECORD_BYTES: usize = U32_BYTES + U8_BYTES;
const PARALLEL_ACCEPTED_PREFIX_BYTES: usize = U32_BYTES * 2 + U64_BYTES * 2 + U8_BYTES * 4;

impl MsiBankDirectoryHarnessSnapshot {
    pub fn to_bytes(&self) -> Vec<u8> {
        encode_snapshot(self)
    }

    pub fn from_bytes(payload: &[u8]) -> Result<Self, String> {
        decode_snapshot(payload)
    }
}

fn encode_snapshot(snapshot: &MsiBankDirectoryHarnessSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, FORMAT_VERSION);
    write_u64(&mut payload, snapshot.layout().bytes());

    write_u64(&mut payload, snapshot.cache_count() as u64);
    for cache in snapshot.cache_snapshots().values() {
        write_cache_bank(&mut payload, cache);
    }

    write_u64(&mut payload, snapshot.directory_line_count() as u64);
    for state in snapshot.directory_states() {
        write_directory_state(&mut payload, state);
    }

    write_u64(&mut payload, snapshot.backing_line_count() as u64);
    for line in snapshot.backing_lines() {
        write_u64(&mut payload, line.line_address().get());
        write_bytes(&mut payload, line.data());
    }

    write_u64(&mut payload, snapshot.cpu_responses().len() as u64);
    for response in snapshot.cpu_responses() {
        write_cpu_response(&mut payload, response);
    }

    write_u64(&mut payload, snapshot.directory_decisions().len() as u64);
    for decision in snapshot.directory_decisions() {
        write_directory_decision(&mut payload, decision);
    }

    write_u64(&mut payload, snapshot.parallel_cycle_runs().len() as u64);
    for run in snapshot.parallel_cycle_runs() {
        write_cycle_run(&mut payload, run);
    }

    payload
}

fn decode_snapshot(payload: &[u8]) -> Result<MsiBankDirectoryHarnessSnapshot, String> {
    let mut cursor = PayloadCursor::new(payload);
    let version = cursor.read_u64("MSI bank checkpoint version")?;
    if version != FORMAT_VERSION {
        return Err(format!("unsupported MSI bank checkpoint version {version}"));
    }
    let layout = CacheLineLayout::new(cursor.read_u64("MSI bank line size")?)
        .map_err(|error| error.to_string())?;

    let cache_count = cursor.read_count("MSI cache bank count")?;
    let mut caches = BTreeMap::new();
    for _ in 0..cache_count {
        let cache = read_cache_bank(&mut cursor)?;
        let agent = cache.agent();
        if caches.insert(agent, cache).is_some() {
            return Err(format!("duplicate MSI cache bank agent {}", agent.get()));
        }
    }

    let directory_count =
        cursor.read_bounded_count("MSI directory line count", DIRECTORY_STATE_PREFIX_BYTES)?;
    let mut directory_states = Vec::with_capacity(directory_count);
    for _ in 0..directory_count {
        directory_states.push(read_directory_state(&mut cursor)?);
    }

    let backing_count =
        cursor.read_bounded_count("MSI backing line count", BACKING_LINE_PREFIX_BYTES)?;
    let mut backing_lines = Vec::with_capacity(backing_count);
    for _ in 0..backing_count {
        let line_address = Address::new(cursor.read_u64("MSI backing line address")?);
        let data = cursor.read_vec("MSI backing line data")?;
        backing_lines.push(MsiBankBackingLineSnapshot::new(line_address, data));
    }

    let response_count =
        cursor.read_bounded_count("MSI CPU response count", CPU_RESPONSE_PREFIX_BYTES)?;
    let mut cpu_responses = Vec::with_capacity(response_count);
    for _ in 0..response_count {
        cpu_responses.push(read_cpu_response(&mut cursor)?);
    }

    let decision_count = cursor.read_bounded_count(
        "MSI directory decision count",
        DIRECTORY_DECISION_PREFIX_BYTES,
    )?;
    let mut directory_decisions = Vec::with_capacity(decision_count);
    for _ in 0..decision_count {
        directory_decisions.push(read_directory_decision(&mut cursor)?);
    }

    let cycle_count =
        cursor.read_bounded_count("MSI parallel cycle count", PARALLEL_CYCLE_PREFIX_BYTES)?;
    let mut parallel_cycle_runs = Vec::with_capacity(cycle_count);
    for _ in 0..cycle_count {
        parallel_cycle_runs.push(read_cycle_run(&mut cursor)?);
    }

    cursor.finish()?;
    Ok(MsiBankDirectoryHarnessSnapshot::new(
        layout,
        caches,
        directory_states,
        backing_lines,
        cpu_responses,
        directory_decisions,
        parallel_cycle_runs,
    ))
}

fn write_cache_bank(payload: &mut Vec<u8>, snapshot: &MsiCacheBankSnapshot) {
    write_u32(payload, snapshot.agent().get());
    write_u64(payload, snapshot.layout().bytes());
    write_u64(payload, snapshot.next_sequence());
    write_u64(payload, snapshot.line_count() as u64);
    for line in snapshot.lines() {
        write_cache_controller(payload, line);
    }
    write_optional_mshr_queue(payload, snapshot.mshr());
}

fn read_cache_bank(cursor: &mut PayloadCursor<'_>) -> Result<MsiCacheBankSnapshot, String> {
    let agent = AgentId::new(cursor.read_u32("MSI cache bank agent")?);
    let layout = CacheLineLayout::new(cursor.read_u64("MSI cache bank line size")?)
        .map_err(|error| error.to_string())?;
    let next_sequence = cursor.read_u64("MSI cache bank sequence")?;
    let line_count =
        cursor.read_bounded_count("MSI cache bank line count", CACHE_CONTROLLER_PREFIX_BYTES)?;
    let mut lines = Vec::with_capacity(line_count);
    for _ in 0..line_count {
        lines.push(read_cache_controller(cursor)?);
    }
    match read_optional_mshr_queue(cursor)? {
        Some(mshr) => Ok(MsiCacheBankSnapshot::new_with_mshr(
            agent,
            layout,
            next_sequence,
            lines,
            mshr,
        )),
        None => Ok(MsiCacheBankSnapshot::new(
            agent,
            layout,
            next_sequence,
            lines,
        )),
    }
}

fn write_cache_controller(payload: &mut Vec<u8>, snapshot: &MsiCacheControllerSnapshot) {
    write_u32(payload, snapshot.agent().get());
    write_u64(payload, snapshot.line().address().get());
    write_u8(payload, msi_state_to_u8(snapshot.state()));
    write_u64(payload, snapshot.layout().bytes());
    write_u64(payload, snapshot.next_sequence());
    write_optional_bytes(payload, snapshot.cached_data());

    write_u64(payload, snapshot.line_state().trace().len() as u64);
    for entry in snapshot.line_state().trace() {
        write_u8(payload, msi_event_to_u8(entry.event()));
    }

    match snapshot.pending() {
        Some(pending) => {
            write_bool(payload, true);
            write_request(payload, pending.original());
            write_request_id(payload, pending.downstream());
            write_u8(payload, msi_event_to_u8(pending.fill_event()));
        }
        None => write_bool(payload, false),
    }
}

fn read_cache_controller(
    cursor: &mut PayloadCursor<'_>,
) -> Result<MsiCacheControllerSnapshot, String> {
    let agent = AgentId::new(cursor.read_u32("MSI cache line agent")?);
    let line = MsiLineId::new(Address::new(cursor.read_u64("MSI cache line address")?));
    let state = u8_to_msi_state(cursor.read_u8("MSI cache line state")?)?;
    let layout = CacheLineLayout::new(cursor.read_u64("MSI cache line size")?)
        .map_err(|error| error.to_string())?;
    let next_sequence = cursor.read_u64("MSI cache line sequence")?;
    let data = cursor.read_optional_vec("MSI cache line data")?;

    let trace_len =
        cursor.read_bounded_count("MSI cache line trace length", CACHE_LINE_TRACE_RECORD_BYTES)?;
    let mut events = Vec::with_capacity(trace_len);
    for _ in 0..trace_len {
        events.push(u8_to_msi_event(
            cursor.read_u8("MSI cache line trace event")?,
        )?);
    }
    let line_state = MsiCacheLine::replay(agent, line, &events).map_err(|error| {
        format!(
            "MSI cache line {:#x} trace does not replay: {error}",
            line.address().get()
        )
    })?;
    if line_state.state() != state {
        return Err(format!(
            "MSI cache line {:#x} trace ends in {:?}, snapshot says {:?}",
            line.address().get(),
            line_state.state(),
            state
        ));
    }

    let pending = if cursor.read_bool("MSI cache pending flag")? {
        Some(MsiPendingMissSnapshot::new(
            read_request(cursor)?,
            read_request_id(cursor)?,
            u8_to_msi_event(cursor.read_u8("MSI cache pending fill event")?)?,
        ))
    } else {
        None
    };

    Ok(MsiCacheControllerSnapshot::new(
        line_state,
        layout,
        next_sequence,
        data,
        pending,
    ))
}

fn write_optional_mshr_queue(payload: &mut Vec<u8>, snapshot: Option<&MshrQueueSnapshot>) {
    let Some(snapshot) = snapshot else {
        write_bool(payload, false);
        return;
    };
    write_bool(payload, true);
    write_u64(payload, snapshot.config().entries() as u64);
    write_u64(payload, snapshot.config().targets_per_mshr() as u64);
    write_u64(payload, snapshot.config().demand_reserve() as u64);
    write_u64(payload, snapshot.next_handle());
    write_u64(payload, snapshot.next_order());
    write_u64(payload, snapshot.entries().len() as u64);
    for entry in snapshot.entries() {
        write_mshr_entry(payload, entry);
    }
}

fn read_optional_mshr_queue(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Option<MshrQueueSnapshot>, String> {
    if !cursor.read_bool("MSI cache bank MSHR flag")? {
        return Ok(None);
    }
    let entries = cursor.read_count("MSI cache bank MSHR entries")?;
    let targets_per_mshr = cursor.read_count("MSI cache bank MSHR targets per entry")?;
    let demand_reserve = cursor.read_count("MSI cache bank MSHR demand reserve")?;
    let config = MshrQueueConfig::new(entries, targets_per_mshr, demand_reserve)
        .map_err(|error| error.to_string())?;
    let next_handle = cursor.read_u64("MSI cache bank MSHR next handle")?;
    let next_order = cursor.read_u64("MSI cache bank MSHR next order")?;
    let entry_count =
        cursor.read_bounded_count("MSI cache bank MSHR entry count", MSHR_ENTRY_PREFIX_BYTES)?;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        entries.push(read_mshr_entry(cursor)?);
    }
    Ok(Some(MshrQueueSnapshot::new(
        config,
        entries,
        next_handle,
        next_order,
    )))
}

fn write_mshr_entry(payload: &mut Vec<u8>, entry: &MshrEntry) {
    write_u64(payload, entry.handle().index());
    write_u64(payload, entry.line().get());
    write_u64(payload, entry.ready_tick());
    write_u64(payload, entry.order());
    write_bool(payload, entry.in_service());
    write_bool(payload, entry.pending_modified());
    write_u64(payload, entry.targets().len() as u64);
    for target in entry.targets() {
        write_mshr_target(payload, target);
    }
}

fn read_mshr_entry(cursor: &mut PayloadCursor<'_>) -> Result<MshrEntry, String> {
    let handle = MshrHandle::new(cursor.read_u64("MSI cache bank MSHR entry handle")?);
    let line = Address::new(cursor.read_u64("MSI cache bank MSHR entry line")?);
    let ready_tick = cursor.read_u64("MSI cache bank MSHR entry ready tick")?;
    let order = cursor.read_u64("MSI cache bank MSHR entry order")?;
    let in_service = cursor.read_bool("MSI cache bank MSHR entry in-service flag")?;
    let pending_modified = cursor.read_bool("MSI cache bank MSHR entry pending-modified flag")?;
    let target_count =
        cursor.read_bounded_count("MSI cache bank MSHR target count", MSHR_TARGET_PREFIX_BYTES)?;
    let mut targets = Vec::with_capacity(target_count);
    for _ in 0..target_count {
        targets.push(read_mshr_target(cursor)?);
    }
    Ok(MshrEntry::from_parts(
        handle,
        line,
        ready_tick,
        order,
        in_service,
        pending_modified,
        targets,
    ))
}

fn write_mshr_target(payload: &mut Vec<u8>, target: &MshrTarget) {
    write_request(payload, target.request());
    write_u64(payload, target.ready_tick());
    write_u64(payload, target.order());
    write_u8(payload, mshr_target_source_to_u8(target.source()));
    write_bool(payload, target.alloc_on_fill());
    write_optional_mshr_qos(payload, target.qos());
}

fn read_mshr_target(cursor: &mut PayloadCursor<'_>) -> Result<MshrTarget, String> {
    let request = read_request(cursor)?;
    let ready_tick = cursor.read_u64("MSI cache bank MSHR target ready tick")?;
    let order = cursor.read_u64("MSI cache bank MSHR target order")?;
    let source = u8_to_mshr_target_source(cursor.read_u8("MSI cache bank MSHR target source")?)?;
    let alloc_on_fill = cursor.read_bool("MSI cache bank MSHR target alloc-on-fill flag")?;
    let qos = read_optional_mshr_qos(cursor)?;
    Ok(MshrTarget::from_parts(
        request,
        ready_tick,
        order,
        source,
        alloc_on_fill,
        qos,
    ))
}

fn write_directory_state(payload: &mut Vec<u8>, state: &DirectoryLineState) {
    write_u64(payload, state.line().address().get());
    match state.owner() {
        Some(owner) => {
            write_bool(payload, true);
            write_u32(payload, owner.get());
        }
        None => write_bool(payload, false),
    }
    write_u64(payload, state.sharers().len() as u64);
    for sharer in state.sharers() {
        write_u32(payload, sharer.get());
    }
}

fn read_directory_state(cursor: &mut PayloadCursor<'_>) -> Result<DirectoryLineState, String> {
    let line = MsiLineId::new(Address::new(cursor.read_u64("MSI directory line")?));
    let mut state = DirectoryLineState::new(line);
    if cursor.read_bool("MSI directory owner flag")? {
        state = state.with_owner(AgentId::new(cursor.read_u32("MSI directory owner")?));
    }
    let sharer_count = cursor.read_bounded_count("MSI directory sharer count", U32_BYTES)?;
    for _ in 0..sharer_count {
        state = state.with_sharer(AgentId::new(cursor.read_u32("MSI directory sharer")?));
    }
    Ok(state)
}

fn write_directory_decision(payload: &mut Vec<u8>, decision: &DirectoryDecision) {
    write_u64(payload, decision.line().address().get());
    write_request_id(payload, decision.request());
    write_directory_state(payload, decision.before());
    write_directory_state(payload, decision.after());
    write_u64(payload, decision.snoops().len() as u64);
    for snoop in decision.snoops() {
        write_u32(payload, snoop.target().get());
        write_u8(payload, msi_event_to_u8(snoop.event()));
    }
    match decision.grant() {
        Some(grant) => {
            write_bool(payload, true);
            write_request_id(payload, grant.request());
            write_u64(payload, grant.line().address().get());
            write_u8(payload, msi_state_to_u8(grant.state()));
            write_data_source(payload, grant.data_source());
        }
        None => write_bool(payload, false),
    }
}

fn read_directory_decision(cursor: &mut PayloadCursor<'_>) -> Result<DirectoryDecision, String> {
    let line = MsiLineId::new(Address::new(cursor.read_u64("MSI decision line")?));
    let request = read_request_id(cursor)?;
    let before = read_directory_state(cursor)?;
    let after = read_directory_state(cursor)?;
    let snoop_count =
        cursor.read_bounded_count("MSI decision snoop count", DECISION_SNOOP_RECORD_BYTES)?;
    let mut snoops = Vec::with_capacity(snoop_count);
    for _ in 0..snoop_count {
        snoops.push(DirectorySnoop::new(
            AgentId::new(cursor.read_u32("MSI decision snoop target")?),
            u8_to_msi_event(cursor.read_u8("MSI decision snoop event")?)?,
        ));
    }
    let grant = if cursor.read_bool("MSI decision grant flag")? {
        Some(DirectoryGrant::new(
            read_request_id(cursor)?,
            MsiLineId::new(Address::new(cursor.read_u64("MSI decision grant line")?)),
            u8_to_msi_state(cursor.read_u8("MSI decision grant state")?)?,
            read_data_source(cursor)?,
        ))
    } else {
        None
    };
    Ok(DirectoryDecision::new(
        line, request, before, after, snoops, grant,
    ))
}

fn write_cpu_response(payload: &mut Vec<u8>, response: &CpuResponseRecord) {
    write_u64(payload, response.tick());
    write_u8(payload, cache_result_to_u8(response.cache_result()));
    write_request_id(payload, response.request());
    write_u8(payload, response_status_to_u8(response.status()));
    write_optional_bytes(payload, response.data());
}

fn read_cpu_response(cursor: &mut PayloadCursor<'_>) -> Result<CpuResponseRecord, String> {
    Ok(CpuResponseRecord::new(
        cursor.read_u64("MSI CPU response tick")?,
        u8_to_cache_result(cursor.read_u8("MSI CPU response cache result")?)?,
        read_request_id(cursor)?,
        u8_to_response_status(cursor.read_u8("MSI CPU response status")?)?,
        cursor.read_optional_vec("MSI CPU response data")?,
    ))
}

fn write_cycle_run(payload: &mut Vec<u8>, run: &MsiBankCycleRun) {
    write_u64(payload, run.tick());
    write_u64(payload, run.response_count() as u64);
    write_u64(payload, run.accepted().len() as u64);
    for accepted in run.accepted() {
        write_cycle_accepted(payload, accepted);
    }
}

fn read_cycle_run(cursor: &mut PayloadCursor<'_>) -> Result<MsiBankCycleRun, String> {
    let tick = cursor.read_u64("MSI parallel cycle tick")?;
    let response_count = cursor.read_count("MSI parallel cycle response count")?;
    let accepted_count = cursor.read_bounded_count(
        "MSI parallel cycle accepted count",
        PARALLEL_ACCEPTED_PREFIX_BYTES,
    )?;
    let mut accepted = Vec::with_capacity(accepted_count);
    for _ in 0..accepted_count {
        accepted.push(read_cycle_accepted(cursor)?);
    }
    Ok(MsiBankCycleRun::new(tick, accepted, response_count))
}

fn write_cycle_accepted(payload: &mut Vec<u8>, accepted: &MsiBankCycleAccepted) {
    write_u32(payload, accepted.agent().get());
    write_request_id(payload, accepted.request());
    write_u64(payload, accepted.line_address().get());
    write_submit_result(payload, accepted.result());
}

fn read_cycle_accepted(cursor: &mut PayloadCursor<'_>) -> Result<MsiBankCycleAccepted, String> {
    let agent = AgentId::new(cursor.read_u32("MSI parallel cycle accepted agent")?);
    let request = read_request_id(cursor)?;
    let line_address = Address::new(cursor.read_u64("MSI parallel cycle accepted line")?);
    let result = read_submit_result(cursor)?;
    Ok(MsiBankCycleAccepted::new(
        agent,
        request,
        line_address,
        result,
    ))
}

fn write_submit_result(payload: &mut Vec<u8>, result: &SubmitResult) {
    write_u8(payload, submit_kind_to_u8(result.kind()));
    write_u8(payload, cache_result_to_u8(result.cache_result()));
    match result.directory_decision() {
        Some(decision) => {
            write_bool(payload, true);
            write_directory_decision(payload, decision);
        }
        None => write_bool(payload, false),
    }
    write_optional_mshr_qos(payload, result.cache_mshr_effective_qos());
}

fn read_submit_result(cursor: &mut PayloadCursor<'_>) -> Result<SubmitResult, String> {
    let kind = u8_to_submit_kind(cursor.read_u8("MSI submit result kind")?)?;
    let cache_result = u8_to_cache_result(cursor.read_u8("MSI submit cache result")?)?;
    let mut result = SubmitResult::new(kind, cache_result);
    if cursor.read_bool("MSI submit directory decision flag")? {
        result = result.with_directory_decision(read_directory_decision(cursor)?);
    }
    if let Some(qos) = read_optional_mshr_qos(cursor)? {
        Ok(result.with_cache_mshr_effective_qos(Some(qos)))
    } else {
        Ok(result)
    }
}

fn write_optional_mshr_qos(payload: &mut Vec<u8>, qos: Option<MshrQosClass>) {
    match qos {
        Some(qos) => {
            write_bool(payload, true);
            write_u32(payload, qos.requestor());
            write_u8(payload, qos.priority());
        }
        None => write_bool(payload, false),
    }
}

fn read_optional_mshr_qos(cursor: &mut PayloadCursor<'_>) -> Result<Option<MshrQosClass>, String> {
    if !cursor.read_bool("MSI submit MSHR QoS flag")? {
        return Ok(None);
    }
    let requestor = cursor.read_u32("MSI submit MSHR QoS requestor")?;
    let priority = cursor.read_u8("MSI submit MSHR QoS priority")?;
    Ok(Some(MshrQosClass::new(requestor, priority)))
}

fn write_request(payload: &mut Vec<u8>, request: &MemoryRequest) {
    write_request_id(payload, request.id());
    write_u8(payload, memory_operation_to_u8(request.operation()));
    write_u64(payload, request.range().start().get());
    write_u64(payload, request.size().bytes());
    write_u64(payload, request.line_layout().bytes());
    write_optional_bytes(payload, request.data());
    match request.byte_mask() {
        Some(mask) => {
            write_bool(payload, true);
            write_u64(payload, mask.bits().len() as u64);
            for bit in mask.bits() {
                write_bool(payload, *bit);
            }
        }
        None => write_bool(payload, false),
    }
    write_memory_access_ordering(payload, request.ordering());
}

fn read_request(cursor: &mut PayloadCursor<'_>) -> Result<MemoryRequest, String> {
    let id = read_request_id(cursor)?;
    let operation = u8_to_memory_operation(cursor.read_u8("MSI request operation")?)?;
    let address = Address::new(cursor.read_u64("MSI request address")?);
    let size =
        AccessSize::new(cursor.read_u64("MSI request size")?).map_err(|error| error.to_string())?;
    let layout = CacheLineLayout::new(cursor.read_u64("MSI request line size")?)
        .map_err(|error| error.to_string())?;
    let data = cursor.read_optional_vec("MSI request data")?;
    let byte_mask = if cursor.read_bool("MSI request byte mask flag")? {
        let bit_count = cursor.read_bounded_count("MSI request byte mask length", U8_BYTES)?;
        let mut bits = Vec::with_capacity(bit_count);
        for _ in 0..bit_count {
            bits.push(cursor.read_bool("MSI request byte mask bit")?);
        }
        Some(ByteMask::from_bits(bits).map_err(|error| error.to_string())?)
    } else {
        None
    };
    let ordering = read_memory_access_ordering(cursor)?;

    let request = match operation {
        MemoryOperation::NoAccess => {
            reject_unexpected_request_payload("no access", &data, &byte_mask)?;
            MemoryRequest::no_access(id, address, size, layout)
        }
        MemoryOperation::InstructionFetch => {
            MemoryRequest::instruction_fetch(id, address, size, layout)
        }
        MemoryOperation::ReadShared => MemoryRequest::read_shared(id, address, size, layout),
        MemoryOperation::ReadUnique => MemoryRequest::read_unique(id, address, size, layout),
        MemoryOperation::LoadLocked => {
            reject_unexpected_request_payload("load locked", &data, &byte_mask)?;
            MemoryRequest::load_locked(id, address, size, layout)
        }
        MemoryOperation::LockedRmwRead => {
            reject_unexpected_request_payload("locked RMW read", &data, &byte_mask)?;
            MemoryRequest::locked_rmw_read(id, address, size, layout)
        }
        MemoryOperation::Write => MemoryRequest::write(
            id,
            address,
            size,
            data.ok_or_else(|| "MSI write request is missing data".to_string())?,
            byte_mask.ok_or_else(|| "MSI write request is missing byte mask".to_string())?,
            layout,
        ),
        MemoryOperation::CacheBlockZero => {
            reject_unexpected_request_payload("cache block zero", &data, &byte_mask)?;
            MemoryRequest::cache_block_zero(id, address, layout)
        }
        MemoryOperation::LockedRmwWrite => MemoryRequest::locked_rmw_write(
            id,
            address,
            size,
            data.ok_or_else(|| "MSI locked RMW write request is missing data".to_string())?,
            byte_mask
                .ok_or_else(|| "MSI locked RMW write request is missing byte mask".to_string())?,
            layout,
        ),
        MemoryOperation::StoreConditional => MemoryRequest::store_conditional(
            id,
            address,
            size,
            data.ok_or_else(|| "MSI store conditional request is missing data".to_string())?,
            byte_mask
                .ok_or_else(|| "MSI store conditional request is missing byte mask".to_string())?,
            layout,
        ),
        MemoryOperation::StoreConditionalFail => MemoryRequest::store_conditional_fail(
            id,
            address,
            size,
            data.ok_or_else(|| "MSI store conditional fail request is missing data".to_string())?,
            byte_mask.ok_or_else(|| {
                "MSI store conditional fail request is missing byte mask".to_string()
            })?,
            layout,
        ),
        MemoryOperation::StoreConditionalUpgrade => {
            reject_unexpected_request_payload("store conditional upgrade", &data, &byte_mask)?;
            MemoryRequest::store_conditional_upgrade(id, address, size, layout)
        }
        MemoryOperation::StoreConditionalUpgradeFail => {
            reject_unexpected_request_payload("store conditional upgrade fail", &data, &byte_mask)?;
            MemoryRequest::store_conditional_upgrade_fail(id, address, size, layout)
        }
        MemoryOperation::Upgrade => MemoryRequest::upgrade(id, address, size, layout),
        MemoryOperation::WriteClean => {
            if byte_mask.is_some() {
                return Err("MSI write clean request cannot carry a byte mask".to_string());
            }
            MemoryRequest::write_clean(
                id,
                address,
                data.ok_or_else(|| "MSI write clean request is missing data".to_string())?,
                layout,
            )
        }
        MemoryOperation::CleanShared => {
            if data.is_some() {
                return Err("MSI clean shared request cannot carry data".to_string());
            }
            if byte_mask.is_some() {
                return Err("MSI clean shared request cannot carry a byte mask".to_string());
            }
            if size.bytes() != layout.bytes() {
                return Err(format!(
                    "MSI clean shared request size {} does not match line size {}",
                    size.bytes(),
                    layout.bytes()
                ));
            }
            MemoryRequest::clean_shared(id, address, layout)
        }
        MemoryOperation::InvalidateWritable => {
            if data.is_some() {
                return Err("MSI writable invalidate request cannot carry data".to_string());
            }
            if byte_mask.is_some() {
                return Err("MSI writable invalidate request cannot carry a byte mask".to_string());
            }
            if size.bytes() != layout.bytes() {
                return Err(format!(
                    "MSI writable invalidate request size {} does not match line size {}",
                    size.bytes(),
                    layout.bytes()
                ));
            }
            MemoryRequest::invalidate_writable(id, address, layout)
        }
        MemoryOperation::WritebackClean => MemoryRequest::writeback_clean(
            id,
            address,
            data.ok_or_else(|| "MSI clean writeback is missing data".to_string())?,
            layout,
        ),
        MemoryOperation::WritebackDirty => MemoryRequest::writeback_dirty(
            id,
            address,
            data.ok_or_else(|| "MSI dirty writeback is missing data".to_string())?,
            layout,
        ),
        MemoryOperation::Atomic
        | MemoryOperation::AtomicNoReturn
        | MemoryOperation::PrefetchRead
        | MemoryOperation::PrefetchWrite
        | MemoryOperation::CleanEvict
        | MemoryOperation::Invalidate => {
            return Err(format!(
                "MSI checkpoint decoder cannot rebuild {operation:?} requests"
            ));
        }
    }
    .map_err(|error| error.to_string())?;
    Ok(request.with_ordering(ordering))
}

fn reject_unexpected_request_payload(
    operation: &str,
    data: &Option<Vec<u8>>,
    byte_mask: &Option<ByteMask>,
) -> Result<(), String> {
    if data.is_some() {
        return Err(format!("MSI {operation} request cannot carry data"));
    }
    if byte_mask.is_some() {
        return Err(format!("MSI {operation} request cannot carry a byte mask"));
    }
    Ok(())
}

fn write_memory_access_ordering(payload: &mut Vec<u8>, ordering: MemoryAccessOrdering) {
    write_optional_memory_barrier_set(payload, ordering.before());
    write_optional_memory_barrier_set(payload, ordering.after());
}

fn read_memory_access_ordering(
    cursor: &mut PayloadCursor<'_>,
) -> Result<MemoryAccessOrdering, String> {
    let before = read_optional_memory_barrier_set(
        cursor,
        "MSI request before-ordering flag",
        "MSI request before-ordering read flag",
        "MSI request before-ordering write flag",
    )?;
    let after = read_optional_memory_barrier_set(
        cursor,
        "MSI request after-ordering flag",
        "MSI request after-ordering read flag",
        "MSI request after-ordering write flag",
    )?;
    Ok(MemoryAccessOrdering::new(before, after))
}

fn write_optional_memory_barrier_set(payload: &mut Vec<u8>, barrier: Option<MemoryBarrierSet>) {
    match barrier {
        Some(barrier) => {
            write_bool(payload, true);
            write_bool(payload, barrier.read());
            write_bool(payload, barrier.write());
        }
        None => write_bool(payload, false),
    }
}

fn read_optional_memory_barrier_set(
    cursor: &mut PayloadCursor<'_>,
    flag_field: &str,
    read_field: &str,
    write_field: &str,
) -> Result<Option<MemoryBarrierSet>, String> {
    if !cursor.read_bool(flag_field)? {
        return Ok(None);
    }
    let read = cursor.read_bool(read_field)?;
    let write = cursor.read_bool(write_field)?;
    Ok(Some(MemoryBarrierSet::new(read, write)))
}

fn write_request_id(payload: &mut Vec<u8>, request: MemoryRequestId) {
    write_u32(payload, request.agent().get());
    write_u64(payload, request.sequence());
}

fn read_request_id(cursor: &mut PayloadCursor<'_>) -> Result<MemoryRequestId, String> {
    Ok(MemoryRequestId::new(
        AgentId::new(cursor.read_u32("MSI request agent")?),
        cursor.read_u64("MSI request sequence")?,
    ))
}

fn write_data_source(payload: &mut Vec<u8>, source: DirectoryDataSource) {
    match source {
        DirectoryDataSource::BackingMemory => write_u8(payload, 0),
        DirectoryDataSource::ModifiedOwner(agent) => {
            write_u8(payload, 1);
            write_u32(payload, agent.get());
        }
        DirectoryDataSource::NoData => write_u8(payload, 2),
    }
}

fn read_data_source(cursor: &mut PayloadCursor<'_>) -> Result<DirectoryDataSource, String> {
    match cursor.read_u8("MSI directory data source")? {
        0 => Ok(DirectoryDataSource::BackingMemory),
        1 => Ok(DirectoryDataSource::ModifiedOwner(AgentId::new(
            cursor.read_u32("MSI directory modified owner")?,
        ))),
        2 => Ok(DirectoryDataSource::NoData),
        value => Err(format!("unknown MSI directory data source {value}")),
    }
}

fn write_optional_bytes(payload: &mut Vec<u8>, bytes: Option<&[u8]>) {
    match bytes {
        Some(bytes) => {
            write_bool(payload, true);
            write_bytes(payload, bytes);
        }
        None => write_bool(payload, false),
    }
}

fn write_bytes(payload: &mut Vec<u8>, bytes: &[u8]) {
    write_u64(payload, bytes.len() as u64);
    payload.extend_from_slice(bytes);
}

fn write_bool(payload: &mut Vec<u8>, value: bool) {
    write_u8(payload, u8::from(value));
}

fn write_u8(payload: &mut Vec<u8>, value: u8) {
    payload.push(value);
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    payload: &'a [u8],
    offset: usize,
}

impl<'a> PayloadCursor<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, offset: 0 }
    }

    fn read_u8(&mut self, field: &str) -> Result<u8, String> {
        Ok(self.read_exact(field, 1)?[0])
    }

    fn read_u32(&mut self, field: &str) -> Result<u32, String> {
        let bytes = self.read_exact(field, U32_BYTES)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("u32 slice")))
    }

    fn read_u64(&mut self, field: &str) -> Result<u64, String> {
        let bytes = self.read_exact(field, U64_BYTES)?;
        Ok(u64::from_le_bytes(bytes.try_into().expect("u64 slice")))
    }

    fn read_bool(&mut self, field: &str) -> Result<bool, String> {
        match self.read_u8(field)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(format!("{field} has invalid bool value {value}")),
        }
    }

    fn read_count(&mut self, field: &str) -> Result<usize, String> {
        let value = self.read_u64(field)?;
        usize::try_from(value).map_err(|_| format!("{field} exceeds usize"))
    }

    fn read_bounded_count(&mut self, field: &str, record_bytes: usize) -> Result<usize, String> {
        let count = self.read_count(field)?;
        let capacity = self.remaining() / record_bytes;
        if count > capacity {
            return Err(format!(
                "{field} {count} exceeds remaining payload capacity {capacity} records"
            ));
        }
        Ok(count)
    }

    fn remaining(&self) -> usize {
        self.payload.len().saturating_sub(self.offset)
    }

    fn read_vec(&mut self, field: &str) -> Result<Vec<u8>, String> {
        let len = self.read_count(field)?;
        Ok(self.read_exact(field, len)?.to_vec())
    }

    fn read_optional_vec(&mut self, field: &str) -> Result<Option<Vec<u8>>, String> {
        if self.read_bool(field)? {
            Ok(Some(self.read_vec(field)?))
        } else {
            Ok(None)
        }
    }

    fn read_exact(&mut self, field: &str, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| format!("{field} length overflows"))?;
        if end > self.payload.len() {
            return Err(format!("{field} is truncated"));
        }
        let bytes = &self.payload[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(self) -> Result<(), String> {
        if self.offset == self.payload.len() {
            Ok(())
        } else {
            Err(format!(
                "{} trailing MSI checkpoint bytes",
                self.payload.len() - self.offset
            ))
        }
    }
}

fn msi_state_to_u8(state: MsiState) -> u8 {
    match state {
        MsiState::Invalid => 0,
        MsiState::Shared => 1,
        MsiState::Modified => 2,
        MsiState::InvalidToShared => 3,
        MsiState::InvalidToModified => 4,
        MsiState::SharedToModified => 5,
    }
}

fn u8_to_msi_state(value: u8) -> Result<MsiState, String> {
    match value {
        0 => Ok(MsiState::Invalid),
        1 => Ok(MsiState::Shared),
        2 => Ok(MsiState::Modified),
        3 => Ok(MsiState::InvalidToShared),
        4 => Ok(MsiState::InvalidToModified),
        5 => Ok(MsiState::SharedToModified),
        _ => Err(format!("unknown MSI state {value}")),
    }
}

fn msi_event_to_u8(event: MsiEvent) -> u8 {
    match event {
        MsiEvent::CpuRead => 0,
        MsiEvent::CpuWrite => 1,
        MsiEvent::DataShared => 2,
        MsiEvent::DataModified => 3,
        MsiEvent::SnoopRead => 4,
        MsiEvent::SnoopWrite => 5,
    }
}

fn u8_to_msi_event(value: u8) -> Result<MsiEvent, String> {
    match value {
        0 => Ok(MsiEvent::CpuRead),
        1 => Ok(MsiEvent::CpuWrite),
        2 => Ok(MsiEvent::DataShared),
        3 => Ok(MsiEvent::DataModified),
        4 => Ok(MsiEvent::SnoopRead),
        5 => Ok(MsiEvent::SnoopWrite),
        _ => Err(format!("unknown MSI event {value}")),
    }
}

fn memory_operation_to_u8(operation: MemoryOperation) -> u8 {
    match operation {
        MemoryOperation::NoAccess => 20,
        MemoryOperation::InstructionFetch => 0,
        MemoryOperation::ReadShared => 1,
        MemoryOperation::ReadUnique => 2,
        MemoryOperation::Write => 3,
        MemoryOperation::Upgrade => 4,
        MemoryOperation::Atomic => 5,
        MemoryOperation::AtomicNoReturn => 24,
        MemoryOperation::PrefetchRead => 6,
        MemoryOperation::PrefetchWrite => 7,
        MemoryOperation::WritebackClean => 8,
        MemoryOperation::WritebackDirty => 9,
        MemoryOperation::CleanEvict => 10,
        MemoryOperation::Invalidate => 11,
        MemoryOperation::WriteClean => 12,
        MemoryOperation::CleanShared => 13,
        MemoryOperation::InvalidateWritable => 14,
        MemoryOperation::LockedRmwRead => 15,
        MemoryOperation::LockedRmwWrite => 16,
        MemoryOperation::LoadLocked => 17,
        MemoryOperation::StoreConditional => 18,
        MemoryOperation::CacheBlockZero => 19,
        MemoryOperation::StoreConditionalUpgrade => 21,
        MemoryOperation::StoreConditionalUpgradeFail => 22,
        MemoryOperation::StoreConditionalFail => 23,
    }
}

fn u8_to_memory_operation(value: u8) -> Result<MemoryOperation, String> {
    match value {
        20 => Ok(MemoryOperation::NoAccess),
        0 => Ok(MemoryOperation::InstructionFetch),
        1 => Ok(MemoryOperation::ReadShared),
        2 => Ok(MemoryOperation::ReadUnique),
        3 => Ok(MemoryOperation::Write),
        4 => Ok(MemoryOperation::Upgrade),
        5 => Ok(MemoryOperation::Atomic),
        24 => Ok(MemoryOperation::AtomicNoReturn),
        6 => Ok(MemoryOperation::PrefetchRead),
        7 => Ok(MemoryOperation::PrefetchWrite),
        8 => Ok(MemoryOperation::WritebackClean),
        9 => Ok(MemoryOperation::WritebackDirty),
        10 => Ok(MemoryOperation::CleanEvict),
        11 => Ok(MemoryOperation::Invalidate),
        12 => Ok(MemoryOperation::WriteClean),
        13 => Ok(MemoryOperation::CleanShared),
        14 => Ok(MemoryOperation::InvalidateWritable),
        15 => Ok(MemoryOperation::LockedRmwRead),
        16 => Ok(MemoryOperation::LockedRmwWrite),
        17 => Ok(MemoryOperation::LoadLocked),
        18 => Ok(MemoryOperation::StoreConditional),
        19 => Ok(MemoryOperation::CacheBlockZero),
        21 => Ok(MemoryOperation::StoreConditionalUpgrade),
        22 => Ok(MemoryOperation::StoreConditionalUpgradeFail),
        23 => Ok(MemoryOperation::StoreConditionalFail),
        _ => Err(format!("unknown memory operation {value}")),
    }
}

fn response_status_to_u8(status: ResponseStatus) -> u8 {
    match status {
        ResponseStatus::Completed => 0,
        ResponseStatus::Retry => 1,
        ResponseStatus::StoreConditionalFailed => 2,
    }
}

fn u8_to_response_status(value: u8) -> Result<ResponseStatus, String> {
    match value {
        0 => Ok(ResponseStatus::Completed),
        1 => Ok(ResponseStatus::Retry),
        2 => Ok(ResponseStatus::StoreConditionalFailed),
        _ => Err(format!("unknown memory response status {value}")),
    }
}

fn submit_kind_to_u8(kind: SubmitKind) -> u8 {
    match kind {
        SubmitKind::ImmediateHit => 0,
        SubmitKind::ScheduledMiss => 1,
        SubmitKind::CoalescedMiss => 2,
    }
}

fn u8_to_submit_kind(value: u8) -> Result<SubmitKind, String> {
    match value {
        0 => Ok(SubmitKind::ImmediateHit),
        1 => Ok(SubmitKind::ScheduledMiss),
        2 => Ok(SubmitKind::CoalescedMiss),
        _ => Err(format!("unknown MSI submit kind {value}")),
    }
}

fn mshr_target_source_to_u8(source: MshrTargetSource) -> u8 {
    match source {
        MshrTargetSource::Demand => 0,
        MshrTargetSource::Snoop => 1,
        MshrTargetSource::Prefetch => 2,
    }
}

fn u8_to_mshr_target_source(value: u8) -> Result<MshrTargetSource, String> {
    match value {
        0 => Ok(MshrTargetSource::Demand),
        1 => Ok(MshrTargetSource::Snoop),
        2 => Ok(MshrTargetSource::Prefetch),
        _ => Err(format!("unknown MSI MSHR target source {value}")),
    }
}

fn cache_result_to_u8(kind: rem6_cache::CacheControllerResultKind) -> u8 {
    match kind {
        rem6_cache::CacheControllerResultKind::Hit => 0,
        rem6_cache::CacheControllerResultKind::Miss => 1,
        rem6_cache::CacheControllerResultKind::Fill => 2,
        rem6_cache::CacheControllerResultKind::Snoop => 3,
    }
}

fn u8_to_cache_result(value: u8) -> Result<rem6_cache::CacheControllerResultKind, String> {
    match value {
        0 => Ok(rem6_cache::CacheControllerResultKind::Hit),
        1 => Ok(rem6_cache::CacheControllerResultKind::Miss),
        2 => Ok(rem6_cache::CacheControllerResultKind::Fill),
        3 => Ok(rem6_cache::CacheControllerResultKind::Snoop),
        _ => Err(format!("unknown MSI cache result kind {value}")),
    }
}
