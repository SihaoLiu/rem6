use std::collections::BTreeMap;

use rem6_cache::{MsiCacheBankSnapshot, MsiCacheControllerSnapshot, MsiPendingMissSnapshot};
use rem6_directory::{
    DirectoryDataSource, DirectoryDecision, DirectoryGrant, DirectoryLineState, DirectorySnoop,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestId, ResponseStatus,
};
use rem6_protocol_msi::{MsiCacheLine, MsiEvent, MsiLineId, MsiState};

use crate::{
    CpuResponseRecord, MsiBankBackingLineSnapshot, MsiBankCycleAccepted, MsiBankCycleRun,
    MsiBankDirectoryHarnessSnapshot, SubmitKind, SubmitResult,
};

const FORMAT_VERSION: u64 = 2;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

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

    let directory_count = cursor.read_count("MSI directory line count")?;
    let mut directory_states = Vec::with_capacity(directory_count);
    for _ in 0..directory_count {
        directory_states.push(read_directory_state(&mut cursor)?);
    }

    let backing_count = cursor.read_count("MSI backing line count")?;
    let mut backing_lines = Vec::with_capacity(backing_count);
    for _ in 0..backing_count {
        let line_address = Address::new(cursor.read_u64("MSI backing line address")?);
        let data = cursor.read_vec("MSI backing line data")?;
        backing_lines.push(MsiBankBackingLineSnapshot::new(line_address, data));
    }

    let response_count = cursor.read_count("MSI CPU response count")?;
    let mut cpu_responses = Vec::with_capacity(response_count);
    for _ in 0..response_count {
        cpu_responses.push(read_cpu_response(&mut cursor)?);
    }

    let decision_count = cursor.read_count("MSI directory decision count")?;
    let mut directory_decisions = Vec::with_capacity(decision_count);
    for _ in 0..decision_count {
        directory_decisions.push(read_directory_decision(&mut cursor)?);
    }

    let cycle_count = cursor.read_count("MSI parallel cycle count")?;
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
}

fn read_cache_bank(cursor: &mut PayloadCursor<'_>) -> Result<MsiCacheBankSnapshot, String> {
    let agent = AgentId::new(cursor.read_u32("MSI cache bank agent")?);
    let layout = CacheLineLayout::new(cursor.read_u64("MSI cache bank line size")?)
        .map_err(|error| error.to_string())?;
    let next_sequence = cursor.read_u64("MSI cache bank sequence")?;
    let line_count = cursor.read_count("MSI cache bank line count")?;
    let mut lines = Vec::with_capacity(line_count);
    for _ in 0..line_count {
        lines.push(read_cache_controller(cursor)?);
    }
    Ok(MsiCacheBankSnapshot::new(
        agent,
        layout,
        next_sequence,
        lines,
    ))
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

    let trace_len = cursor.read_count("MSI cache line trace length")?;
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
    let sharer_count = cursor.read_count("MSI directory sharer count")?;
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
    let snoop_count = cursor.read_count("MSI decision snoop count")?;
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
    let accepted_count = cursor.read_count("MSI parallel cycle accepted count")?;
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
}

fn read_submit_result(cursor: &mut PayloadCursor<'_>) -> Result<SubmitResult, String> {
    let kind = u8_to_submit_kind(cursor.read_u8("MSI submit result kind")?)?;
    let cache_result = u8_to_cache_result(cursor.read_u8("MSI submit cache result")?)?;
    let result = SubmitResult::new(kind, cache_result);
    if cursor.read_bool("MSI submit directory decision flag")? {
        Ok(result.with_directory_decision(read_directory_decision(cursor)?))
    } else {
        Ok(result)
    }
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
        let bit_count = cursor.read_count("MSI request byte mask length")?;
        let mut bits = Vec::with_capacity(bit_count);
        for _ in 0..bit_count {
            bits.push(cursor.read_bool("MSI request byte mask bit")?);
        }
        Some(ByteMask::from_bits(bits).map_err(|error| error.to_string())?)
    } else {
        None
    };

    match operation {
        MemoryOperation::InstructionFetch => {
            MemoryRequest::instruction_fetch(id, address, size, layout)
        }
        MemoryOperation::ReadShared => MemoryRequest::read_shared(id, address, size, layout),
        MemoryOperation::ReadUnique => MemoryRequest::read_unique(id, address, size, layout),
        MemoryOperation::Write => MemoryRequest::write(
            id,
            address,
            size,
            data.ok_or_else(|| "MSI write request is missing data".to_string())?,
            byte_mask.ok_or_else(|| "MSI write request is missing byte mask".to_string())?,
            layout,
        ),
        MemoryOperation::Upgrade => MemoryRequest::upgrade(id, address, size, layout),
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
        | MemoryOperation::PrefetchRead
        | MemoryOperation::PrefetchWrite
        | MemoryOperation::CleanEvict
        | MemoryOperation::Invalidate => {
            return Err(format!(
                "MSI checkpoint decoder cannot rebuild {operation:?} requests"
            ));
        }
    }
    .map_err(|error| error.to_string())
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
        MemoryOperation::InstructionFetch => 0,
        MemoryOperation::ReadShared => 1,
        MemoryOperation::ReadUnique => 2,
        MemoryOperation::Write => 3,
        MemoryOperation::Upgrade => 4,
        MemoryOperation::Atomic => 5,
        MemoryOperation::PrefetchRead => 6,
        MemoryOperation::PrefetchWrite => 7,
        MemoryOperation::WritebackClean => 8,
        MemoryOperation::WritebackDirty => 9,
        MemoryOperation::CleanEvict => 10,
        MemoryOperation::Invalidate => 11,
    }
}

fn u8_to_memory_operation(value: u8) -> Result<MemoryOperation, String> {
    match value {
        0 => Ok(MemoryOperation::InstructionFetch),
        1 => Ok(MemoryOperation::ReadShared),
        2 => Ok(MemoryOperation::ReadUnique),
        3 => Ok(MemoryOperation::Write),
        4 => Ok(MemoryOperation::Upgrade),
        5 => Ok(MemoryOperation::Atomic),
        6 => Ok(MemoryOperation::PrefetchRead),
        7 => Ok(MemoryOperation::PrefetchWrite),
        8 => Ok(MemoryOperation::WritebackClean),
        9 => Ok(MemoryOperation::WritebackDirty),
        10 => Ok(MemoryOperation::CleanEvict),
        11 => Ok(MemoryOperation::Invalidate),
        _ => Err(format!("unknown memory operation {value}")),
    }
}

fn response_status_to_u8(status: ResponseStatus) -> u8 {
    match status {
        ResponseStatus::Completed => 0,
        ResponseStatus::Retry => 1,
    }
}

fn u8_to_response_status(value: u8) -> Result<ResponseStatus, String> {
    match value {
        0 => Ok(ResponseStatus::Completed),
        1 => Ok(ResponseStatus::Retry),
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
