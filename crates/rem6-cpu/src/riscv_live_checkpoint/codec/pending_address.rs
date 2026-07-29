use rem6_memory::{AccessSize, AddressRange};

use crate::{O3LoadStoreQueueKind, O3PhysicalRegisterId, O3RenameMapEntry};

use super::{
    boolv, checked_count, count, invalid_field, invalid_tag, read_address, read_fetch,
    read_register, read_register_class, read_request, u32v, u64v, write_address, write_fetch,
    write_register, write_request, Error, Reader, WireTag, MAX_ENDPOINT_BYTES, MAX_FETCH_BYTES,
    MAX_ROWS,
};
use crate::riscv_live_checkpoint::{
    RiscvO3LiveCheckpointPendingDataAddress, MAX_PENDING_ADDRESSES,
};

pub(super) fn preflight(values: &[RiscvO3LiveCheckpointPendingDataAddress]) -> Result<(), Error> {
    checked_count("pending address rows", values.len(), MAX_PENDING_ADDRESSES)?;
    for value in values {
        checked_count(
            "pending consumed requests",
            value.consumed_requests.len(),
            MAX_ROWS,
        )?;
        checked_count(
            "pending fetch endpoint",
            value.fetch.endpoint().as_str().len(),
            MAX_ENDPOINT_BYTES,
        )?;
        checked_count(
            "pending fetch data",
            value.fetch.data().unwrap_or_default().len(),
            MAX_FETCH_BYTES,
        )?;
    }
    Ok(())
}

pub(super) fn write(
    out: &mut Vec<u8>,
    values: &[RiscvO3LiveCheckpointPendingDataAddress],
) -> Result<(), Error> {
    count(
        out,
        "pending address rows",
        values.len(),
        MAX_PENDING_ADDRESSES,
    )?;
    for value in values {
        write_row(out, value)?;
    }
    Ok(())
}

pub(super) fn read_v2_single(
    reader: &mut Reader<'_>,
) -> Result<Vec<RiscvO3LiveCheckpointPendingDataAddress>, Error> {
    if !reader.bool("pending address present")? {
        return Ok(Vec::new());
    }
    Ok(vec![read_row(reader, RowVersion::V2)?])
}

pub(super) fn read_v3_rows(
    reader: &mut Reader<'_>,
) -> Result<Vec<RiscvO3LiveCheckpointPendingDataAddress>, Error> {
    reader.vec("pending address rows", MAX_PENDING_ADDRESSES, |reader| {
        read_row(reader, RowVersion::V3)
    })
}

fn write_row(
    out: &mut Vec<u8>,
    value: &RiscvO3LiveCheckpointPendingDataAddress,
) -> Result<(), Error> {
    u64v(out, value.sequence);
    write_fetch(out, &value.fetch)?;
    count(
        out,
        "pending consumed requests",
        value.consumed_requests.len(),
        MAX_ROWS,
    )?;
    for request in &value.consumed_requests {
        write_request(out, *request);
    }
    write_request(out, value.fetch_predecessor_request);
    write_register(out, value.producer_register);
    write_destination(out, value.destination);
    u64v(out, value.producer_sequence);
    u64v(out, value.root_sequence);
    write_request(out, value.root_fetch_request);
    write_address(out, value.root_range.start());
    u64v(out, value.root_range.size().bytes());
    boolv(out, value.root_atomic);
    match value.lsq_kind {
        O3LoadStoreQueueKind::Load => super::byte(out, 0),
        O3LoadStoreQueueKind::Store => super::byte(out, 1),
    }
    u32v(out, value.expected_lsq_bytes);
    write_optional_tick(out, value.published_producer_ready_tick);
    write_optional_tick(out, value.requested_wake_tick);
    Ok(())
}

#[derive(Clone, Copy)]
enum RowVersion {
    V2,
    V3,
}

fn read_row(
    reader: &mut Reader<'_>,
    version: RowVersion,
) -> Result<RiscvO3LiveCheckpointPendingDataAddress, Error> {
    let sequence = reader.u64("pending sequence")?;
    let fetch = read_fetch(reader)?;
    let consumed_requests = reader.vec("pending consumed requests", MAX_ROWS, |reader| {
        read_request(reader, "pending consumed request")
    })?;
    let fetch_predecessor_request = read_request(reader, "pending fetch predecessor request")?;
    let producer_register = read_register(reader, "pending producer register")?;
    let destination = match version {
        RowVersion::V2 => None,
        RowVersion::V3 => read_destination(reader)?,
    };
    let producer_sequence = reader.u64("pending producer sequence")?;
    let root_sequence = reader.u64("pending root sequence")?;
    let root_fetch_request = read_request(reader, "pending root fetch request")?;
    let root_start = read_address(reader, "pending root address")?;
    let root_size_value = reader.u64("pending root range size")?;
    let root_size = AccessSize::new(root_size_value)
        .map_err(|_| invalid_field("pending root range size", root_size_value))?;
    let root_range = AddressRange::new(root_start, root_size)
        .map_err(|_| invalid_field("pending root range", root_start.get()))?;
    let root_atomic = reader.bool("pending root atomic")?;
    let lsq_kind = match reader.byte("pending LSQ kind")? {
        0 => O3LoadStoreQueueKind::Load,
        1 => O3LoadStoreQueueKind::Store,
        value => return Err(invalid_tag("pending LSQ kind", value)),
    };
    let expected_lsq_bytes = reader.u32("pending expected LSQ bytes")?;
    let published_producer_ready_tick = match version {
        RowVersion::V2 => Some(reader.u64("pending published producer-ready tick")?),
        RowVersion::V3 => read_optional_tick(reader, "pending published producer-ready tick")?,
    };
    let requested_wake_tick = match version {
        RowVersion::V2 => Some(reader.u64("pending requested wake tick")?),
        RowVersion::V3 => read_optional_tick(reader, "pending requested wake tick")?,
    };
    Ok(RiscvO3LiveCheckpointPendingDataAddress {
        sequence,
        fetch,
        consumed_requests,
        fetch_predecessor_request,
        producer_register,
        destination,
        producer_sequence,
        root_sequence,
        root_fetch_request,
        root_range,
        root_atomic,
        lsq_kind,
        expected_lsq_bytes,
        published_producer_ready_tick,
        requested_wake_tick,
    })
}

fn write_destination(out: &mut Vec<u8>, value: Option<O3RenameMapEntry>) {
    boolv(out, value.is_some());
    if let Some(value) = value {
        super::byte(out, value.register_class().wire_tag());
        u32v(out, value.architectural());
        u32v(out, value.physical().get());
    }
}

fn read_destination(reader: &mut Reader<'_>) -> Result<Option<O3RenameMapEntry>, Error> {
    if !reader.bool("pending destination present")? {
        return Ok(None);
    }
    Ok(Some(O3RenameMapEntry::new(
        read_register_class(reader)?,
        reader.u32("pending destination architectural register")?,
        O3PhysicalRegisterId::new(reader.u32("pending destination physical register")?),
    )))
}

fn write_optional_tick(out: &mut Vec<u8>, value: Option<u64>) {
    boolv(out, value.is_some());
    if let Some(value) = value {
        u64v(out, value);
    }
}

fn read_optional_tick(reader: &mut Reader<'_>, field: &'static str) -> Result<Option<u64>, Error> {
    reader.bool(field)?.then(|| reader.u64(field)).transpose()
}
