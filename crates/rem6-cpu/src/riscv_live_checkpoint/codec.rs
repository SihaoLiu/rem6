use std::collections::{BTreeMap, BTreeSet};

use rem6_isa_riscv::{
    FloatRegister, FloatRegisterWrite, MemoryAccessKind, MemoryWidth, Register, RegisterWrite,
};
use rem6_kernel::{PartitionId, ScheduledEventKind};
use rem6_memory::{AccessSize, Address, AddressRange, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use crate::o3_runtime::{o3_live_compute_operands, O3LiveComputeClass};
use crate::{
    CpuFetchEvent, CpuFetchEventKind, CpuFetchRecord, O3PhysicalRegisterId, O3RegisterClass,
    O3RenameMapEntry, RiscvDataAccessEventKind,
};

use super::{
    RiscvO3LiveCheckpointCompletedFpLoad, RiscvO3LiveCheckpointError as Error,
    RiscvO3LiveCheckpointEvent, RiscvO3LiveCheckpointFinalizedWriteback,
    RiscvO3LiveCheckpointIssueRow, RiscvO3LiveCheckpointPayload, RiscvO3LiveCheckpointProfile,
    RiscvO3LiveCheckpointReservation, RiscvO3LiveCheckpointService, RiscvO3LiveCheckpointTelemetry,
    RiscvO3LiveCheckpointWake, RiscvO3LiveCheckpointWritebackSource,
};

const MAGIC: [u8; 4] = *b"O3LC";
const VERSION: u8 = 1;
const MAX_ROWS: usize = 65_536;
const MAX_EVENTS: usize = 4_096;
const MAX_ENDPOINT_BYTES: usize = 1_024;
const MAX_FETCH_BYTES: usize = 4;
const MAX_RESPONSE_BYTES: usize = 64;
const MAX_WRITES: usize = 32;

trait WireTag: Sized {
    fn wire_tag(self) -> u8;
    fn from_wire_tag(value: u8, field: &'static str) -> Result<Self, Error>;
}

macro_rules! wire_tags {
    ($ty:ty; $($tag:literal => $variant:path),+ $(,)?) => {
        impl WireTag for $ty {
            fn wire_tag(self) -> u8 {
                match self { $($variant => $tag),+ }
            }
            fn from_wire_tag(value: u8, field: &'static str) -> Result<Self, Error> {
                match value { $($tag => Ok($variant),)+ value => Err(invalid_tag(field, value)) }
            }
        }
    };
}

wire_tags!(O3RegisterClass; 0 => O3RegisterClass::Integer, 1 => O3RegisterClass::FloatingPoint, 2 => O3RegisterClass::Vector, 3 => O3RegisterClass::ConditionCode, 4 => O3RegisterClass::Misc);
wire_tags!(CpuFetchEventKind; 0 => CpuFetchEventKind::Issued, 1 => CpuFetchEventKind::Completed, 2 => CpuFetchEventKind::Retry, 3 => CpuFetchEventKind::Failed);
wire_tags!(MemoryWidth; 0 => MemoryWidth::Byte, 1 => MemoryWidth::Halfword, 2 => MemoryWidth::Word, 3 => MemoryWidth::Doubleword);
wire_tags!(RiscvDataAccessEventKind; 0 => RiscvDataAccessEventKind::Issued, 1 => RiscvDataAccessEventKind::Completed, 2 => RiscvDataAccessEventKind::Retry, 3 => RiscvDataAccessEventKind::ConditionalFailed, 4 => RiscvDataAccessEventKind::Failed);
wire_tags!(RiscvO3LiveCheckpointWritebackSource; 0 => RiscvO3LiveCheckpointWritebackSource::FixedFunction, 1 => RiscvO3LiveCheckpointWritebackSource::MemoryResult);
wire_tags!(ScheduledEventKind; 0 => ScheduledEventKind::Serial, 1 => ScheduledEventKind::Parallel);

pub(super) fn encode(checkpoint: &RiscvO3LiveCheckpointPayload) -> Result<Vec<u8>, Error> {
    encode_impl(checkpoint, true)
}

#[cfg(test)]
pub(super) fn encode_without_validation(
    checkpoint: &RiscvO3LiveCheckpointPayload,
) -> Result<Vec<u8>, Error> {
    encode_impl(checkpoint, false)
}

fn encode_impl(
    checkpoint: &RiscvO3LiveCheckpointPayload,
    validate_semantics: bool,
) -> Result<Vec<u8>, Error> {
    preflight(checkpoint)?;
    if validate_semantics {
        validate(checkpoint)?;
    }
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC);
    byte(&mut out, VERSION);
    byte(&mut out, profile_tag(checkpoint.profile));
    u64v(&mut out, checkpoint.captured_tick);
    u64v(&mut out, checkpoint.next_fetch_pc.get());
    u64v(&mut out, checkpoint.next_fetch_request_sequence);
    count(&mut out, "events", checkpoint.events.len(), MAX_EVENTS)?;
    for event in &checkpoint.events {
        write_event(&mut out, event)?;
    }
    count(
        &mut out,
        "issue rows",
        checkpoint.issue_rows.len(),
        MAX_ROWS,
    )?;
    for row in &checkpoint.issue_rows {
        u64v(&mut out, row.sequence);
        write_request(&mut out, row.fetch_request);
    }
    count(
        &mut out,
        "rename rows",
        checkpoint.rename_rows.len(),
        MAX_ROWS,
    )?;
    for row in &checkpoint.rename_rows {
        byte(&mut out, row.register_class().wire_tag());
        u32v(&mut out, row.architectural());
        u32v(&mut out, row.physical().get());
    }
    write_u64_vec(
        &mut out,
        "resident sequences",
        &checkpoint.resident_sequences,
    )?;
    write_request_vec(
        &mut out,
        "executed fetch requests",
        &checkpoint.executed_fetch_requests,
    )?;
    write_request_vec(
        &mut out,
        "issued fetch requests",
        &checkpoint.issued_fetch_requests,
    )?;
    write_service(&mut out, checkpoint.service);
    write_finalized(&mut out, &checkpoint.finalized_writeback)?;
    write_u64_vec(
        &mut out,
        "writeback counted sequences",
        &checkpoint.writeback_counted_sequences,
    )?;
    write_u64_vec(
        &mut out,
        "writeback published sequences",
        &checkpoint.writeback_published_sequences,
    )?;
    write_reservation(&mut out, checkpoint.reservation);
    write_completed_result(&mut out, checkpoint.completed_result.as_ref())?;
    write_wake(&mut out, checkpoint.wake);
    Ok(out)
}

pub(super) fn decode(payload: &[u8]) -> Result<RiscvO3LiveCheckpointPayload, Error> {
    let mut reader = Reader::new(payload);
    if reader.bytes("magic", MAGIC.len())? != MAGIC {
        return Err(Error::InvalidMagic);
    }
    let version = reader.byte("version")?;
    if version != VERSION {
        return Err(Error::UnsupportedVersion { version });
    }
    let profile = match reader.byte("profile")? {
        0 => RiscvO3LiveCheckpointProfile::ComputeQueue,
        1 => RiscvO3LiveCheckpointProfile::CompletedFpLoad,
        profile => return Err(Error::UnsupportedProfile { profile }),
    };
    let captured_tick = reader.u64("captured tick")?;
    let next_fetch_pc = Address::new(reader.u64("next fetch PC")?);
    let next_fetch_request_sequence = reader.u64("next fetch request sequence")?;
    let events = reader.vec("events", MAX_EVENTS, read_event)?;
    let issue_rows = reader.vec("issue rows", MAX_ROWS, |reader| {
        Ok(RiscvO3LiveCheckpointIssueRow {
            sequence: reader.u64("issue row sequence")?,
            fetch_request: read_request(reader, "issue row fetch request")?,
        })
    })?;
    let rename_rows = reader.vec("rename rows", MAX_ROWS, |reader| {
        Ok(O3RenameMapEntry::new(
            read_register_class(reader)?,
            reader.u32("rename architectural register")?,
            O3PhysicalRegisterId::new(reader.u32("rename physical register")?),
        ))
    })?;
    let resident_sequences = read_u64_vec(&mut reader, "resident sequences")?;
    let executed_fetch_requests = read_request_vec(&mut reader, "executed fetch requests")?;
    let issued_fetch_requests = read_request_vec(&mut reader, "issued fetch requests")?;
    let service = read_service(&mut reader)?;
    let finalized_writeback = read_finalized(&mut reader)?;
    let writeback_counted_sequences = read_u64_vec(&mut reader, "writeback counted sequences")?;
    let writeback_published_sequences = read_u64_vec(&mut reader, "writeback published sequences")?;
    let reservation = read_reservation(&mut reader)?;
    let completed_result = read_completed_result(&mut reader)?;
    let wake = read_wake(&mut reader)?;
    if reader.remaining() != 0 {
        return Err(Error::TrailingBytes {
            remaining: reader.remaining(),
        });
    }
    let checkpoint = RiscvO3LiveCheckpointPayload {
        profile,
        captured_tick,
        next_fetch_pc,
        next_fetch_request_sequence,
        events,
        issue_rows,
        rename_rows,
        resident_sequences,
        executed_fetch_requests,
        issued_fetch_requests,
        service,
        finalized_writeback,
        writeback_counted_sequences,
        writeback_published_sequences,
        reservation,
        completed_result,
        wake,
    };
    validate(&checkpoint)?;
    Ok(checkpoint)
}

fn write_event(out: &mut Vec<u8>, event: &RiscvO3LiveCheckpointEvent) -> Result<(), Error> {
    u64v(out, event.fetch.tick());
    u32v(out, event.fetch.partition().index());
    u64v(out, event.fetch.route().get());
    blob(
        out,
        "fetch endpoint",
        event.fetch.endpoint().as_str().as_bytes(),
        MAX_ENDPOINT_BYTES,
    )?;
    write_request(out, event.fetch.request_id());
    u64v(out, event.fetch.pc().get());
    u64v(out, event.fetch.size().bytes());
    byte(out, event.fetch.kind().wire_tag());
    blob(
        out,
        "fetch data",
        event.fetch.data().unwrap_or_default(),
        MAX_FETCH_BYTES,
    )?;
    u64v(out, event.execution_pc);
    u64v(out, event.next_pc);
    byte(out, event.instruction_bytes);
    count(
        out,
        "integer writes",
        event.register_writes.len(),
        MAX_WRITES,
    )?;
    for write in &event.register_writes {
        byte(out, write.register().index());
        u64v(out, write.value());
    }
    count(
        out,
        "float writes",
        event.float_register_writes.len(),
        MAX_WRITES,
    )?;
    for write in &event.float_register_writes {
        byte(out, write.register().index());
        u64v(out, write.value());
    }
    match &event.memory_access {
        None => byte(out, 0),
        Some(MemoryAccessKind::FloatLoad { rd, address, width }) => {
            byte(out, 1);
            byte(out, rd.index());
            u64v(out, *address);
            byte(out, width.wire_tag());
        }
        Some(_) => {
            return Err(unsupported("memory access is not a scalar FP load"));
        }
    }
    byte(
        out,
        event
            .data_access_event_kind
            .map_or(0, |kind| kind.wire_tag() + 1),
    );
    boolv(out, event.counts_as_retired_instruction);
    Ok(())
}

fn read_event(reader: &mut Reader<'_>) -> Result<RiscvO3LiveCheckpointEvent, Error> {
    let tick = reader.u64("fetch tick")?;
    let partition = PartitionId::new(reader.u32("fetch partition")?);
    let route = MemoryRouteId::new(reader.u64("fetch route")?);
    let endpoint_bytes = reader.blob("fetch endpoint", MAX_ENDPOINT_BYTES)?;
    let endpoint = std::str::from_utf8(endpoint_bytes)
        .ok()
        .and_then(|value| TransportEndpointId::new(value).ok())
        .ok_or(Error::InvalidEndpoint)?;
    let request = read_request(reader, "fetch request")?;
    let pc = Address::new(reader.u64("fetch PC")?);
    let size_value = reader.u64("fetch access size")?;
    let size =
        AccessSize::new(size_value).map_err(|_| invalid_field("fetch access size", size_value))?;
    let kind =
        CpuFetchEventKind::from_wire_tag(reader.byte("fetch event kind")?, "fetch event kind")?;
    let data = reader.blob("fetch data", MAX_FETCH_BYTES)?.to_vec();
    let record = CpuFetchRecord::new(tick, partition, route, endpoint, request, pc, size);
    let fetch = match kind {
        CpuFetchEventKind::Completed => CpuFetchEvent::completed(record, data),
        CpuFetchEventKind::Issued if data.is_empty() => CpuFetchEvent::issued(record),
        CpuFetchEventKind::Retry if data.is_empty() => CpuFetchEvent::retry(record),
        CpuFetchEventKind::Failed if data.is_empty() => CpuFetchEvent::failed(record),
        _ => return Err(unsupported("non-completed fetch carries bytes")),
    };
    let execution_pc = reader.u64("event execution PC")?;
    let next_pc = reader.u64("event next PC")?;
    let instruction_bytes = reader.byte("event instruction width")?;
    let register_writes = reader.vec("integer writes", MAX_WRITES, |reader| {
        let index = reader.byte("integer write register")?;
        let register =
            Register::new(index).map_err(|_| invalid_register("integer write", index))?;
        Ok(RegisterWrite::new(
            register,
            reader.u64("integer write value")?,
        ))
    })?;
    let float_register_writes = reader.vec("float writes", MAX_WRITES, |reader| {
        let index = reader.byte("float write register")?;
        let register =
            FloatRegister::new(index).map_err(|_| invalid_register("float write", index))?;
        Ok(FloatRegisterWrite::new(
            register,
            reader.u64("float write value")?,
        ))
    })?;
    let memory_access = match reader.byte("memory access")? {
        0 => None,
        1 => Some(MemoryAccessKind::FloatLoad {
            rd: read_float_register(reader, "memory access destination")?,
            address: reader.u64("memory access address")?,
            width: read_width(reader, "memory access width")?,
        }),
        value => {
            return Err(Error::InvalidTag {
                field: "memory access",
                value,
            })
        }
    };
    let data_tag = reader.byte("data access event kind")?;
    let data_access_event_kind = (data_tag != 0)
        .then(|| RiscvDataAccessEventKind::from_wire_tag(data_tag - 1, "data access event kind"))
        .transpose()?;
    let event = RiscvO3LiveCheckpointEvent {
        fetch,
        execution_pc,
        next_pc,
        instruction_bytes,
        register_writes,
        float_register_writes,
        memory_access,
        data_access_event_kind,
        counts_as_retired_instruction: reader.bool("event counts as retired instruction")?,
    };
    event.rebuild()?;
    Ok(event)
}

fn write_service(out: &mut Vec<u8>, service: RiscvO3LiveCheckpointService) {
    u64v(out, service.requested_tick);
    u64v(out, service.mutation_generation);
    boolv(out, service.last_service_generation.is_some());
    if let Some((tick, generation)) = service.last_service_generation {
        u64v(out, tick);
        u64v(out, generation);
    }
    for value in [
        service.telemetry.enqueued_rows,
        service.telemetry.service_turns,
        service.telemetry.wake_requests,
        service.telemetry.current_occupancy,
        service.telemetry.peak_occupancy,
        service.telemetry.scalar_integer_issued_rows,
        service.telemetry.integer_mul_div_issued_rows,
        service.telemetry.memory_agu_issued_rows,
        service.telemetry.control_issued_rows,
        service.telemetry.scalar_float_issued_rows,
        service.telemetry.vector_to_scalar_issued_rows,
    ] {
        u64v(out, value);
    }
}

fn read_service(reader: &mut Reader<'_>) -> Result<RiscvO3LiveCheckpointService, Error> {
    let requested_tick = reader.u64("service requested tick")?;
    let mutation_generation = reader.u64("service mutation generation")?;
    let last_service_generation = reader
        .bool("service last identity present")?
        .then(|| {
            Ok((
                reader.u64("service last tick")?,
                reader.u64("service last generation")?,
            ))
        })
        .transpose()?;
    Ok(RiscvO3LiveCheckpointService {
        requested_tick,
        mutation_generation,
        last_service_generation,
        telemetry: RiscvO3LiveCheckpointTelemetry {
            enqueued_rows: reader.u64("telemetry enqueued rows")?,
            service_turns: reader.u64("telemetry service turns")?,
            wake_requests: reader.u64("telemetry wake requests")?,
            current_occupancy: reader.u64("telemetry current occupancy")?,
            peak_occupancy: reader.u64("telemetry peak occupancy")?,
            scalar_integer_issued_rows: reader.u64("telemetry scalar integer rows")?,
            integer_mul_div_issued_rows: reader.u64("telemetry integer mul-div rows")?,
            memory_agu_issued_rows: reader.u64("telemetry memory AGU rows")?,
            control_issued_rows: reader.u64("telemetry control rows")?,
            scalar_float_issued_rows: reader.u64("telemetry scalar float rows")?,
            vector_to_scalar_issued_rows: reader.u64("telemetry vector-to-scalar rows")?,
        },
    })
}

fn write_finalized(
    out: &mut Vec<u8>,
    value: &RiscvO3LiveCheckpointFinalizedWriteback,
) -> Result<(), Error> {
    for field in [
        value.cycles,
        value.admitted_rows,
        value.deferred_rows,
        value.deferred_row_cycles,
        value.max_ready_rows_per_cycle,
        value.max_deferred_rows,
    ] {
        u64v(out, field);
    }
    count(
        out,
        "partial cycle ticks",
        value.partial_cycle_ticks.len(),
        MAX_ROWS,
    )?;
    for tick in &value.partial_cycle_ticks {
        u64v(out, *tick);
    }
    write_tick_map(
        out,
        "partial ready rows by tick",
        &value.partial_ready_rows_by_tick,
    )?;
    write_tick_map(
        out,
        "partial deferred rows by tick",
        &value.partial_deferred_rows_by_tick,
    )?;
    u64v(out, value.closed_before_tick);
    Ok(())
}

fn read_finalized(
    reader: &mut Reader<'_>,
) -> Result<RiscvO3LiveCheckpointFinalizedWriteback, Error> {
    let cycles = reader.u64("finalized cycles")?;
    let admitted_rows = reader.u64("finalized admitted rows")?;
    let deferred_rows = reader.u64("finalized deferred rows")?;
    let deferred_row_cycles = reader.u64("finalized deferred row cycles")?;
    let max_ready_rows_per_cycle = reader.u64("finalized max ready rows")?;
    let max_deferred_rows = reader.u64("finalized max deferred rows")?;
    let ticks = reader.vec("partial cycle ticks", MAX_ROWS, |reader| {
        reader.u64("partial cycle tick")
    })?;
    let partial_cycle_ticks = collect_set("partial cycle ticks", ticks)?;
    let partial_ready_rows_by_tick = read_tick_map(reader, "partial ready rows by tick")?;
    let partial_deferred_rows_by_tick = read_tick_map(reader, "partial deferred rows by tick")?;
    Ok(RiscvO3LiveCheckpointFinalizedWriteback {
        cycles,
        admitted_rows,
        deferred_rows,
        deferred_row_cycles,
        max_ready_rows_per_cycle,
        max_deferred_rows,
        partial_cycle_ticks,
        partial_ready_rows_by_tick,
        partial_deferred_rows_by_tick,
        closed_before_tick: reader.u64("finalized closed-before tick")?,
    })
}

fn write_reservation(out: &mut Vec<u8>, value: Option<RiscvO3LiveCheckpointReservation>) {
    boolv(out, value.is_some());
    if let Some(value) = value {
        u64v(out, value.sequence);
        u64v(out, value.raw_ready_tick);
        u64v(out, value.admitted_tick);
        u32v(out, value.slot);
        byte(out, value.source.wire_tag());
        boolv(out, value.decision_counted);
    }
}

fn read_reservation(
    reader: &mut Reader<'_>,
) -> Result<Option<RiscvO3LiveCheckpointReservation>, Error> {
    if !reader.bool("reservation present")? {
        return Ok(None);
    }
    Ok(Some(RiscvO3LiveCheckpointReservation {
        sequence: reader.u64("reservation sequence")?,
        raw_ready_tick: reader.u64("reservation raw-ready tick")?,
        admitted_tick: reader.u64("reservation admitted tick")?,
        slot: reader.u32("reservation slot")?,
        source: RiscvO3LiveCheckpointWritebackSource::from_wire_tag(
            reader.byte("reservation source")?,
            "reservation source",
        )?,
        decision_counted: reader.bool("reservation decision counted")?,
    }))
}

fn write_completed_result(
    out: &mut Vec<u8>,
    value: Option<&RiscvO3LiveCheckpointCompletedFpLoad>,
) -> Result<(), Error> {
    boolv(out, value.is_some());
    let Some(value) = value else {
        return Ok(());
    };
    write_request(out, value.fetch_request);
    write_request(out, value.data_request);
    for field in [
        value.sequence,
        value.lsq_sequence,
        value.rob_first_sequence,
        value.rob_last_sequence,
        value.issue_tick,
        value.response_tick,
        value.raw_ready_tick,
        value.admitted_tick,
        value.latency_ticks,
        value.physical_address.get(),
        value.access_size.bytes(),
    ] {
        u64v(out, field);
    }
    u32v(out, value.request_byte_offset);
    blob(
        out,
        "completed response bytes",
        &value.response_bytes,
        MAX_RESPONSE_BYTES,
    )?;
    byte(out, value.destination.index());
    byte(out, value.width.wire_tag());
    Ok(())
}

fn read_completed_result(
    reader: &mut Reader<'_>,
) -> Result<Option<RiscvO3LiveCheckpointCompletedFpLoad>, Error> {
    if !reader.bool("completed result present")? {
        return Ok(None);
    }
    let fetch_request = read_request(reader, "completed fetch request")?;
    let data_request = read_request(reader, "completed data request")?;
    let sequence = reader.u64("completed sequence")?;
    let lsq_sequence = reader.u64("completed LSQ sequence")?;
    let rob_first_sequence = reader.u64("completed first ROB sequence")?;
    let rob_last_sequence = reader.u64("completed last ROB sequence")?;
    let issue_tick = reader.u64("completed issue tick")?;
    let response_tick = reader.u64("completed response tick")?;
    let raw_ready_tick = reader.u64("completed raw-ready tick")?;
    let admitted_tick = reader.u64("completed admitted tick")?;
    let latency_ticks = reader.u64("completed latency ticks")?;
    let physical_address = Address::new(reader.u64("completed physical address")?);
    let size_value = reader.u64("completed access size")?;
    let access_size = AccessSize::new(size_value)
        .map_err(|_| invalid_field("completed access size", size_value))?;
    Ok(Some(RiscvO3LiveCheckpointCompletedFpLoad {
        fetch_request,
        data_request,
        sequence,
        lsq_sequence,
        rob_first_sequence,
        rob_last_sequence,
        issue_tick,
        response_tick,
        raw_ready_tick,
        admitted_tick,
        latency_ticks,
        physical_address,
        access_size,
        request_byte_offset: reader.u32("completed request byte offset")?,
        response_bytes: reader
            .blob("completed response bytes", MAX_RESPONSE_BYTES)?
            .to_vec(),
        destination: read_float_register(reader, "completed destination")?,
        width: read_width(reader, "completed width")?,
    }))
}

fn write_wake(out: &mut Vec<u8>, wake: RiscvO3LiveCheckpointWake) {
    u64v(out, wake.scheduler_instance_raw);
    u32v(out, wake.partition.index());
    u64v(out, wake.tick);
    u64v(out, wake.scheduler_order);
    byte(out, wake.kind.wire_tag());
}

fn read_wake(reader: &mut Reader<'_>) -> Result<RiscvO3LiveCheckpointWake, Error> {
    let scheduler_instance_raw = reader.u64("wake scheduler instance")?;
    if scheduler_instance_raw == 0 {
        return Err(invalid_field("wake scheduler instance", 0));
    }
    Ok(RiscvO3LiveCheckpointWake {
        scheduler_instance_raw,
        partition: PartitionId::new(reader.u32("wake partition")?),
        tick: reader.u64("wake tick")?,
        scheduler_order: reader.u64("wake scheduler order")?,
        kind: ScheduledEventKind::from_wire_tag(
            reader.byte("wake event kind")?,
            "wake event kind",
        )?,
    })
}

fn validate(value: &RiscvO3LiveCheckpointPayload) -> Result<(), Error> {
    if value.wake.scheduler_instance_raw == 0 {
        return Err(invalid_field("wake scheduler instance", 0));
    }
    if value.events.is_empty() {
        return Err(invalid_shape("live event list is empty"));
    }
    if value.service.has_invalid_identity_at(value.captured_tick) {
        return Err(invalid_shape("last service identity is invalid"));
    }
    for event in &value.events {
        event.rebuild()?;
    }
    for rename in &value.rename_rows {
        if rename.physical().is_invalid() {
            return Err(invalid_field(
                "rename physical register",
                u64::from(rename.physical().get()),
            ));
        }
        if rename.architectural() >= 32
            || !matches!(
                rename.register_class(),
                O3RegisterClass::Integer | O3RegisterClass::FloatingPoint
            )
        {
            return Err(invalid_shape(
                "rename row is not a version-1 scalar register",
            ));
        }
    }
    match value.profile {
        RiscvO3LiveCheckpointProfile::ComputeQueue => {
            let event_requests = value
                .events
                .iter()
                .map(|event| event.fetch.request_id())
                .collect::<BTreeSet<_>>();
            let executed_requests = value
                .executed_fetch_requests
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            if value.reservation.is_some()
                || value.completed_result.is_some()
                || !value.issued_fetch_requests.is_empty()
                || !value.writeback_counted_sequences.is_empty()
                || !value.writeback_published_sequences.is_empty()
                || executed_requests != event_requests
                || !value
                    .finalized_writeback
                    .is_valid_without_live_calendar_at(value.captured_tick)
                || value.events.iter().any(|event| {
                    event.memory_access.is_some() || event.data_access_event_kind.is_some()
                })
            {
                return Err(invalid_shape("compute queue ownership is inconsistent"));
            }
        }
        RiscvO3LiveCheckpointProfile::CompletedFpLoad => {
            let (Some(reservation), Some(result)) =
                (value.reservation, value.completed_result.as_ref())
            else {
                return Err(invalid_shape(
                    "completed FP load lacks its result or reservation",
                ));
            };
            validate_completed_fp_load(value, reservation, result)?;
        }
    }
    ensure_unique(
        "event fetch requests",
        value
            .events
            .iter()
            .map(|event| event.fetch.request_id().sequence()),
    )?;
    ensure_unique(
        "resident sequences",
        value.resident_sequences.iter().copied(),
    )?;
    ensure_unique(
        "writeback counted sequences",
        value.writeback_counted_sequences.iter().copied(),
    )?;
    ensure_unique(
        "writeback published sequences",
        value.writeback_published_sequences.iter().copied(),
    )?;
    Ok(())
}

macro_rules! check_lengths {
    ($maximum:expr; $($field:literal => $value:expr),+ $(,)?) => {
        $(checked_count($field, ($value).len(), $maximum)?;)+
    };
}

fn preflight(value: &RiscvO3LiveCheckpointPayload) -> Result<(), Error> {
    checked_count("events", value.events.len(), MAX_EVENTS)?;
    let finalized = &value.finalized_writeback;
    check_lengths!(MAX_ROWS;
        "issue rows" => value.issue_rows,
        "rename rows" => value.rename_rows,
        "resident sequences" => value.resident_sequences,
        "executed fetch requests" => value.executed_fetch_requests,
        "issued fetch requests" => value.issued_fetch_requests,
        "partial cycle ticks" => finalized.partial_cycle_ticks,
        "partial ready rows by tick" => finalized.partial_ready_rows_by_tick,
        "partial deferred rows by tick" => finalized.partial_deferred_rows_by_tick,
        "writeback counted sequences" => value.writeback_counted_sequences,
        "writeback published sequences" => value.writeback_published_sequences,
    );
    for event in &value.events {
        check_lengths!(MAX_ENDPOINT_BYTES; "fetch endpoint" => event.fetch.endpoint().as_str());
        check_lengths!(MAX_FETCH_BYTES; "fetch data" => event.fetch.data().unwrap_or_default());
        check_lengths!(MAX_WRITES;
            "integer writes" => event.register_writes,
            "float writes" => event.float_register_writes,
        );
    }
    if let Some(result) = &value.completed_result {
        check_lengths!(MAX_RESPONSE_BYTES; "completed response bytes" => result.response_bytes);
    }
    Ok(())
}

fn validate_completed_fp_load(
    value: &RiscvO3LiveCheckpointPayload,
    reservation: RiscvO3LiveCheckpointReservation,
    result: &RiscvO3LiveCheckpointCompletedFpLoad,
) -> Result<(), Error> {
    let mut loads = value.events.iter().filter_map(|event| {
        if let Some(MemoryAccessKind::FloatLoad { rd, address, width }) = event.memory_access {
            Some((event, rd, address, width))
        } else {
            None
        }
    });
    let Some((event, destination, address, width)) = loads.next() else {
        return Err(invalid_shape("completed FP result has no FP-load event"));
    };
    if loads.next().is_some()
        || event.fetch.kind() != CpuFetchEventKind::Completed
        || event.data_access_event_kind != Some(RiscvDataAccessEventKind::Completed)
        || event.fetch.request_id() != result.fetch_request
        || destination != result.destination
        || width != result.width
        || address != result.physical_address.get()
        || reservation.source != RiscvO3LiveCheckpointWritebackSource::MemoryResult
        || reservation.sequence != result.sequence
        || reservation.raw_ready_tick != result.raw_ready_tick
        || reservation.admitted_tick != result.admitted_tick
        || result.issue_tick.checked_add(result.latency_ticks) != Some(result.response_tick)
        || result.response_tick > result.raw_ready_tick
        || result.raw_ready_tick > result.admitted_tick
        || AddressRange::new(result.physical_address, result.access_size).is_err()
        || result.width.bytes() != result.response_bytes.len()
        || result.access_size.bytes() != result.response_bytes.len() as u64
        || !matches!(result.width, MemoryWidth::Word | MemoryWidth::Doubleword)
    {
        return Err(invalid_shape(
            "completed FP load projection is inconsistent",
        ));
    }
    for companion in value
        .events
        .iter()
        .filter(|event| event.memory_access.is_none())
    {
        let instruction = companion.rebuild()?.instruction();
        if !matches!(
            o3_live_compute_operands(instruction).map(|operands| operands.class()),
            Some(O3LiveComputeClass::ScalarFloat)
        ) {
            return Err(invalid_shape(
                "completed FP load companion is not scalar FP compute",
            ));
        }
    }
    Ok(())
}

fn ensure_unique(field: &'static str, values: impl IntoIterator<Item = u64>) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(Error::DuplicateValue { field, value });
        }
    }
    Ok(())
}

fn write_request(out: &mut Vec<u8>, value: MemoryRequestId) {
    u32v(out, value.agent().get());
    u64v(out, value.sequence());
}

fn read_request(reader: &mut Reader<'_>, field: &'static str) -> Result<MemoryRequestId, Error> {
    Ok(MemoryRequestId::new(
        AgentId::new(reader.u32(field)?),
        reader.u64(field)?,
    ))
}

fn write_u64_vec(out: &mut Vec<u8>, field: &'static str, values: &[u64]) -> Result<(), Error> {
    count(out, field, values.len(), MAX_ROWS)?;
    for value in values {
        u64v(out, *value);
    }
    Ok(())
}

fn read_u64_vec(reader: &mut Reader<'_>, field: &'static str) -> Result<Vec<u64>, Error> {
    reader.vec(field, MAX_ROWS, |reader| reader.u64(field))
}

fn write_request_vec(
    out: &mut Vec<u8>,
    field: &'static str,
    values: &[MemoryRequestId],
) -> Result<(), Error> {
    count(out, field, values.len(), MAX_ROWS)?;
    for value in values {
        write_request(out, *value);
    }
    Ok(())
}

fn read_request_vec(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<Vec<MemoryRequestId>, Error> {
    reader.vec(field, MAX_ROWS, |reader| read_request(reader, field))
}

fn write_tick_map(
    out: &mut Vec<u8>,
    field: &'static str,
    values: &BTreeMap<u64, u64>,
) -> Result<(), Error> {
    count(out, field, values.len(), MAX_ROWS)?;
    for (tick, value) in values {
        u64v(out, *tick);
        u64v(out, *value);
    }
    Ok(())
}

fn read_tick_map(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<BTreeMap<u64, u64>, Error> {
    let entries = reader.vec(field, MAX_ROWS, |reader| {
        Ok((reader.u64(field)?, reader.u64(field)?))
    })?;
    let mut values = BTreeMap::new();
    for (tick, value) in entries {
        if values.insert(tick, value).is_some() {
            return Err(Error::DuplicateValue { field, value: tick });
        }
    }
    Ok(values)
}

fn collect_set(field: &'static str, entries: Vec<u64>) -> Result<BTreeSet<u64>, Error> {
    let mut values = BTreeSet::new();
    for value in entries {
        if !values.insert(value) {
            return Err(Error::DuplicateValue { field, value });
        }
    }
    Ok(values)
}

fn read_register_class(reader: &mut Reader<'_>) -> Result<O3RegisterClass, Error> {
    O3RegisterClass::from_wire_tag(
        reader.byte("rename register class")?,
        "rename register class",
    )
}

fn read_float_register(
    reader: &mut Reader<'_>,
    field: &'static str,
) -> Result<FloatRegister, Error> {
    let index = reader.byte(field)?;
    FloatRegister::new(index).map_err(|_| invalid_register(field, index))
}

fn read_width(reader: &mut Reader<'_>, field: &'static str) -> Result<MemoryWidth, Error> {
    MemoryWidth::from_wire_tag(reader.byte(field)?, field)
}

fn profile_tag(value: RiscvO3LiveCheckpointProfile) -> u8 {
    match value {
        RiscvO3LiveCheckpointProfile::ComputeQueue => 0,
        RiscvO3LiveCheckpointProfile::CompletedFpLoad => 1,
    }
}
fn invalid_tag(field: &'static str, value: u8) -> Error {
    Error::InvalidTag { field, value }
}
fn invalid_field(field: &'static str, value: u64) -> Error {
    Error::InvalidField { field, value }
}
fn invalid_register(field: &'static str, index: u8) -> Error {
    Error::InvalidRegister { field, index }
}
fn invalid_shape(reason: &'static str) -> Error {
    Error::InvalidProfileShape { reason }
}
fn unsupported(reason: &'static str) -> Error {
    Error::UnsupportedEvent { reason }
}
fn byte(out: &mut Vec<u8>, value: u8) {
    out.push(value);
}
fn boolv(out: &mut Vec<u8>, value: bool) {
    byte(out, u8::from(value));
}
fn u32v(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn u64v(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn count(
    out: &mut Vec<u8>,
    field: &'static str,
    value: usize,
    maximum: usize,
) -> Result<(), Error> {
    u32v(out, checked_count(field, value, maximum)?);
    Ok(())
}

fn checked_count(field: &'static str, value: usize, maximum: usize) -> Result<u32, Error> {
    let count = u64::try_from(value).map_err(|_| Error::IntegerConversion { field })?;
    if value > maximum {
        return Err(Error::ExcessiveCount {
            field,
            count,
            maximum,
        });
    }
    u32::try_from(value).map_err(|_| Error::IntegerConversion { field })
}

fn blob(out: &mut Vec<u8>, field: &'static str, value: &[u8], maximum: usize) -> Result<(), Error> {
    count(out, field, value.len(), maximum)?;
    out.extend_from_slice(value);
    Ok(())
}

struct Reader<'a> {
    payload: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(payload: &'a [u8]) -> Self {
        Self { payload, offset: 0 }
    }
    fn remaining(&self) -> usize {
        self.payload.len() - self.offset
    }
    fn bytes(&mut self, field: &'static str, len: usize) -> Result<&'a [u8], Error> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(Error::IntegerConversion { field })?;
        let value = self
            .payload
            .get(self.offset..end)
            .ok_or(Error::Truncated { field })?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self, field: &'static str) -> Result<u8, Error> {
        Ok(self.bytes(field, 1)?[0])
    }
    fn u32(&mut self, field: &'static str) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.bytes(field, 4)?.try_into().expect("four bytes"),
        ))
    }
    fn u64(&mut self, field: &'static str) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.bytes(field, 8)?.try_into().expect("eight bytes"),
        ))
    }
    fn bool(&mut self, field: &'static str) -> Result<bool, Error> {
        match self.byte(field)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(Error::InvalidBoolean { field, value }),
        }
    }
    fn count(&mut self, field: &'static str, maximum: usize) -> Result<usize, Error> {
        let raw = self.u32(field)?;
        let count = usize::try_from(raw).map_err(|_| Error::IntegerConversion { field })?;
        if count > maximum {
            return Err(Error::ExcessiveCount {
                field,
                count: u64::from(raw),
                maximum,
            });
        }
        Ok(count)
    }
    fn blob(&mut self, field: &'static str, maximum: usize) -> Result<&'a [u8], Error> {
        let len = self.count(field, maximum)?;
        self.bytes(field, len)
    }
    fn vec<T>(
        &mut self,
        field: &'static str,
        maximum: usize,
        mut read: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        let count = self.count(field, maximum)?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(read(self)?);
        }
        Ok(values)
    }
}
