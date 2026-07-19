use std::collections::BTreeSet;

use super::*;

pub(super) const MAGIC: [u8; 4] = *b"O3DH";
pub(super) const VERSION_MEMORY_ROUTE: u8 = 1;
pub(super) const VERSION_TYPED_TARGET: u8 = 2;
pub(super) const VERSION_FORWARDING: u8 = 3;
pub(super) const VERSION_PARTIAL_OVERLAY: u8 = 4;
pub(super) const VERSION_SINGLE_SOURCE_CURRENT: u8 = 5;
pub(super) const VERSION_MULTI_SOURCE_CURRENT: u8 = 6;
pub(super) const VERSION_CURRENT: u8 = 7;
pub(super) const HEADER_BYTES: usize = MAGIC.len() + 1 + 4 + 4;
pub(super) const V1_ENTRY_BYTES: usize = 73;

const TARGET_MEMORY: u8 = 0;
const TARGET_MMIO: u8 = 1;
const OPERATION_LOAD: u8 = 0;
const OPERATION_STORE: u8 = 1;
const OWNERSHIP_TRANSPORT: u8 = 0;
const OWNERSHIP_BUFFERED_STORE: u8 = 1;

const fn has_current_ownership(version: u8) -> bool {
    matches!(version, VERSION_MULTI_SOURCE_CURRENT | VERSION_CURRENT)
}

const fn has_multi_source_partial_overlay(version: u8) -> bool {
    matches!(version, VERSION_MULTI_SOURCE_CURRENT | VERSION_CURRENT)
}

const fn has_completed_partial_overlay(version: u8) -> bool {
    version == VERSION_CURRENT
}

impl RiscvO3LiveDataHandoff {
    pub fn encode(&self) -> Vec<u8> {
        let entry_count = u32::try_from(self.entries.len()).expect("handoff entry count fits u32");
        let forwarded_count =
            u32::try_from(self.forwarded_rows.len()).expect("handoff forwarded row count fits u32");
        let partial_overlay_count = u32::try_from(self.partial_overlays.len())
            .expect("handoff partial-overlay count fits u32");
        let completed_partial_overlay_count = u32::try_from(self.completed_partial_overlays.len())
            .expect("handoff completed partial-overlay count fits u32");
        let mut payload = Vec::with_capacity(HEADER_BYTES + self.entries.len() * V1_ENTRY_BYTES);
        payload.extend_from_slice(&MAGIC);
        payload.push(VERSION_CURRENT);
        payload.extend_from_slice(&entry_count.to_le_bytes());
        payload.extend_from_slice(&self.younger_rows.to_le_bytes());
        payload.extend_from_slice(&forwarded_count.to_le_bytes());
        payload.extend_from_slice(&partial_overlay_count.to_le_bytes());
        payload.extend_from_slice(&completed_partial_overlay_count.to_le_bytes());
        for entry in &self.entries {
            write_request(&mut payload, entry.fetch_request);
            write_request(&mut payload, entry.data_request);
            payload.extend_from_slice(&entry.issue_tick.to_le_bytes());
            payload.extend_from_slice(&entry.partition.index().to_le_bytes());
            payload.push(match entry.operation {
                RiscvO3LiveDataHandoffOperation::Load => OPERATION_LOAD,
                RiscvO3LiveDataHandoffOperation::Store => OPERATION_STORE,
            });
            match entry.ownership {
                RiscvO3LiveDataHandoffOwnership::Transport => {
                    payload.push(OWNERSHIP_TRANSPORT);
                    payload.extend_from_slice(&[0; 12]);
                }
                RiscvO3LiveDataHandoffOwnership::BufferedStore { predecessor } => {
                    payload.push(OWNERSHIP_BUFFERED_STORE);
                    write_request(&mut payload, predecessor);
                }
            }
            write_target(&mut payload, entry.target);
            payload.extend_from_slice(&entry.address.get().to_le_bytes());
            payload.extend_from_slice(&entry.bytes.to_le_bytes());
            payload.extend_from_slice(&entry.o3_sequence.to_le_bytes());
            payload.push(u8::from(entry.trace_sequence.is_some()));
            payload.extend_from_slice(&entry.trace_sequence.unwrap_or_default().to_le_bytes());
        }
        for row in &self.forwarded_rows {
            write_request(&mut payload, row.fetch_request);
            write_request(&mut payload, row.data_request);
            write_request(&mut payload, row.source_data_request);
            payload.extend_from_slice(&row.issue_tick.to_le_bytes());
            payload.extend_from_slice(&row.response_tick.to_le_bytes());
            payload.extend_from_slice(&row.address.get().to_le_bytes());
            payload.extend_from_slice(&row.bytes.to_le_bytes());
            payload.extend_from_slice(&row.data);
            payload.extend_from_slice(&row.o3_sequence.to_le_bytes());
            payload.push(u8::from(row.trace_sequence.is_some()));
            payload.extend_from_slice(&row.trace_sequence.unwrap_or_default().to_le_bytes());
        }
        for overlay in &self.partial_overlays {
            write_request(&mut payload, overlay.load_data_request);
            payload.extend_from_slice(&overlay.address.get().to_le_bytes());
            payload.extend_from_slice(&overlay.bytes.to_le_bytes());
            payload.push(overlay.forwarded_mask);
            payload.extend_from_slice(&overlay.data);
            payload.extend_from_slice(
                &u32::try_from(overlay.sources.len())
                    .expect("handoff partial-overlay source count fits u32")
                    .to_le_bytes(),
            );
            for source in &overlay.sources {
                write_request(&mut payload, source.source_data_request);
                payload.push(source.ownership_mask);
                payload.extend_from_slice(&source.source_data);
            }
        }
        for overlay in &self.completed_partial_overlays {
            write_request(&mut payload, overlay.fetch_request);
            write_request(&mut payload, overlay.load_data_request);
            payload.extend_from_slice(&overlay.issue_tick.to_le_bytes());
            payload.extend_from_slice(&overlay.response_tick.to_le_bytes());
            payload.extend_from_slice(&overlay.address.get().to_le_bytes());
            payload.extend_from_slice(&overlay.bytes.to_le_bytes());
            payload.push(overlay.original_forwarded_mask);
            payload.push(overlay.live_forwarded_mask);
            payload.extend_from_slice(&overlay.data);
            payload.extend_from_slice(&overlay.o3_sequence.to_le_bytes());
            payload.push(u8::from(overlay.trace_sequence.is_some()));
            payload.extend_from_slice(&overlay.trace_sequence.unwrap_or_default().to_le_bytes());
            payload.extend_from_slice(
                &u32::try_from(overlay.sources.len())
                    .expect("handoff completed partial-overlay source count fits u32")
                    .to_le_bytes(),
            );
            for source in &overlay.sources {
                write_request(&mut payload, source.source_data_request);
                payload.push(source.ownership_mask);
                payload.extend_from_slice(&source.source_data);
            }
        }
        payload
    }

    pub fn decode(payload: &[u8]) -> Result<Self, RiscvO3LiveDataHandoffError> {
        Self::decode_with_version(payload).map(|(handoff, _)| handoff)
    }

    pub fn decode_with_version(payload: &[u8]) -> Result<(Self, u8), RiscvO3LiveDataHandoffError> {
        if payload.len() < HEADER_BYTES {
            return Err(RiscvO3LiveDataHandoffError::InvalidPayloadSize {
                expected: HEADER_BYTES,
                actual: payload.len(),
            });
        }
        if payload[..MAGIC.len()] != MAGIC {
            return Err(RiscvO3LiveDataHandoffError::InvalidMagic);
        }
        let version = payload[MAGIC.len()];
        if !matches!(
            version,
            VERSION_MEMORY_ROUTE
                | VERSION_TYPED_TARGET
                | VERSION_FORWARDING
                | VERSION_PARTIAL_OVERLAY
                | VERSION_SINGLE_SOURCE_CURRENT
                | VERSION_MULTI_SOURCE_CURRENT
                | VERSION_CURRENT
        ) {
            return Err(RiscvO3LiveDataHandoffError::UnsupportedVersion { version });
        }
        let mut offset = MAGIC.len() + 1;
        let entry_count = read_u32(payload, &mut offset)? as usize;
        let younger_rows = read_u32(payload, &mut offset)?;
        let forwarded_count = if matches!(
            version,
            VERSION_FORWARDING
                | VERSION_PARTIAL_OVERLAY
                | VERSION_SINGLE_SOURCE_CURRENT
                | VERSION_MULTI_SOURCE_CURRENT
                | VERSION_CURRENT
        ) {
            read_u32(payload, &mut offset)? as usize
        } else {
            0
        };
        let partial_overlay_count = if matches!(
            version,
            VERSION_PARTIAL_OVERLAY
                | VERSION_SINGLE_SOURCE_CURRENT
                | VERSION_MULTI_SOURCE_CURRENT
                | VERSION_CURRENT
        ) {
            read_u32(payload, &mut offset)? as usize
        } else {
            0
        };
        let completed_partial_overlay_count = if has_completed_partial_overlay(version) {
            read_u32(payload, &mut offset)? as usize
        } else {
            0
        };
        if entry_count == 0 {
            return Err(RiscvO3LiveDataHandoffError::EmptyEntries);
        }
        if version == VERSION_FORWARDING
            && (entry_count != 1 || forwarded_count != 1 || younger_rows != 0)
        {
            return Err(RiscvO3LiveDataHandoffError::InvalidForwardingShape {
                entries: entry_count,
                forwarded_rows: forwarded_count,
                younger_rows,
            });
        }
        if version == VERSION_PARTIAL_OVERLAY
            && (entry_count != 2
                || forwarded_count != 0
                || partial_overlay_count != 1
                || younger_rows != 0)
        {
            return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayShape {
                entries: entry_count,
                forwarded_rows: forwarded_count,
                partial_overlays: partial_overlay_count,
                younger_rows,
            });
        }
        if version == VERSION_SINGLE_SOURCE_CURRENT
            && !single_source_count_shape_is_valid(
                entry_count,
                forwarded_count,
                partial_overlay_count,
                younger_rows,
            )
        {
            return Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: entry_count,
                forwarded_rows: forwarded_count,
                partial_overlays: partial_overlay_count,
                completed_partial_overlays: completed_partial_overlay_count,
                younger_rows,
            });
        }
        if version == VERSION_MULTI_SOURCE_CURRENT
            && !current_count_shape_is_valid(
                entry_count,
                forwarded_count,
                partial_overlay_count,
                0,
                younger_rows,
            )
        {
            return Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: entry_count,
                forwarded_rows: forwarded_count,
                partial_overlays: partial_overlay_count,
                completed_partial_overlays: 0,
                younger_rows,
            });
        }
        if version == VERSION_CURRENT
            && !current_count_shape_is_valid(
                entry_count,
                forwarded_count,
                partial_overlay_count,
                completed_partial_overlay_count,
                younger_rows,
            )
        {
            return Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: entry_count,
                forwarded_rows: forwarded_count,
                partial_overlays: partial_overlay_count,
                completed_partial_overlays: completed_partial_overlay_count,
                younger_rows,
            });
        }
        let resident_rows = entry_count
            .checked_add(forwarded_count)
            .and_then(|rows| rows.checked_add(completed_partial_overlay_count))
            .ok_or(RiscvO3LiveDataHandoffError::TooManyRows {
                entries: usize::MAX,
                younger_rows,
                maximum: MAX_ROWS,
            })?;
        let row_count = resident_rows.checked_add(younger_rows as usize).ok_or(
            RiscvO3LiveDataHandoffError::TooManyRows {
                entries: resident_rows,
                younger_rows,
                maximum: MAX_ROWS,
            },
        )?;
        if row_count > MAX_ROWS {
            return Err(RiscvO3LiveDataHandoffError::TooManyRows {
                entries: resident_rows,
                younger_rows,
                maximum: MAX_ROWS,
            });
        }
        if version == VERSION_MEMORY_ROUTE {
            let expected = HEADER_BYTES + entry_count * V1_ENTRY_BYTES;
            if payload.len() != expected {
                return Err(RiscvO3LiveDataHandoffError::InvalidPayloadSize {
                    expected,
                    actual: payload.len(),
                });
            }
        }

        let mut entries = Vec::<RiscvO3LiveDataHandoffEntry>::with_capacity(entry_count);
        let mut fetch_requests = BTreeSet::new();
        let mut data_requests = BTreeSet::new();
        let mut o3_sequences = BTreeSet::new();
        let mut trace_sequences = BTreeSet::new();
        let mut previous_o3_sequence = None;
        for _ in 0..entry_count {
            let fetch_request = read_request(payload, &mut offset)?;
            let data_request = read_request(payload, &mut offset)?;
            if !fetch_requests.insert(fetch_request) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateFetchRequest {
                    request: fetch_request,
                });
            }
            if !data_requests.insert(data_request) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateDataRequest {
                    request: data_request,
                });
            }
            let issue_tick = read_u64(payload, &mut offset)?;
            let partition = PartitionId::new(read_u32(payload, &mut offset)?);
            let operation = if matches!(
                version,
                VERSION_FORWARDING
                    | VERSION_PARTIAL_OVERLAY
                    | VERSION_SINGLE_SOURCE_CURRENT
                    | VERSION_MULTI_SOURCE_CURRENT
                    | VERSION_CURRENT
            ) {
                match read_u8(payload, &mut offset)? {
                    OPERATION_LOAD => RiscvO3LiveDataHandoffOperation::Load,
                    OPERATION_STORE => RiscvO3LiveDataHandoffOperation::Store,
                    value => {
                        return Err(RiscvO3LiveDataHandoffError::InvalidOperationKind { value })
                    }
                }
            } else {
                RiscvO3LiveDataHandoffOperation::Load
            };
            let ownership = if has_current_ownership(version) {
                match read_u8(payload, &mut offset)? {
                    OWNERSHIP_TRANSPORT => {
                        if read_array::<12>(payload, &mut offset)? != [0; 12] {
                            return Err(
                                RiscvO3LiveDataHandoffError::NonZeroTransportOwnershipPadding,
                            );
                        }
                        RiscvO3LiveDataHandoffOwnership::Transport
                    }
                    OWNERSHIP_BUFFERED_STORE => RiscvO3LiveDataHandoffOwnership::BufferedStore {
                        predecessor: read_request(payload, &mut offset)?,
                    },
                    value => {
                        return Err(RiscvO3LiveDataHandoffError::InvalidOwnershipKind { value })
                    }
                }
            } else {
                RiscvO3LiveDataHandoffOwnership::Transport
            };
            let target = read_target(payload, &mut offset, version, issue_tick, partition)?;
            if operation == RiscvO3LiveDataHandoffOperation::Store
                && !matches!(target, RiscvO3LiveDataHandoffTarget::Memory { .. })
            {
                return Err(RiscvO3LiveDataHandoffError::InvalidStoreTarget);
            }
            if let RiscvO3LiveDataHandoffOwnership::BufferedStore { predecessor } = ownership {
                let valid_predecessor = operation == RiscvO3LiveDataHandoffOperation::Store
                    && matches!(target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                    && entries.iter().any(|entry| {
                        entry.data_request == predecessor
                            && entry.operation == RiscvO3LiveDataHandoffOperation::Store
                            && entry.partition == partition
                            && entry.target == target
                    });
                if !valid_predecessor {
                    return Err(
                        RiscvO3LiveDataHandoffError::InvalidBufferedStorePredecessor {
                            request: predecessor,
                        },
                    );
                }
            }
            let address = Address::new(read_u64(payload, &mut offset)?);
            let bytes = read_u32(payload, &mut offset)?;
            if !matches!(bytes, 1 | 2 | 4 | 8) {
                return Err(RiscvO3LiveDataHandoffError::InvalidScalarBytes { bytes });
            }
            if address.get().checked_add(u64::from(bytes) - 1).is_none() {
                return Err(RiscvO3LiveDataHandoffError::AddressRangeOverflow { address, bytes });
            }
            let o3_sequence = read_u64(payload, &mut offset)?;
            if !o3_sequences.insert(o3_sequence) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateO3Sequence {
                    sequence: o3_sequence,
                });
            }
            if let Some(previous) = previous_o3_sequence.filter(|previous| *previous >= o3_sequence)
            {
                return Err(RiscvO3LiveDataHandoffError::NonIncreasingO3Sequence {
                    previous,
                    current: o3_sequence,
                });
            }
            previous_o3_sequence = Some(o3_sequence);
            let trace_present = read_u8(payload, &mut offset)?;
            if trace_present > 1 {
                return Err(RiscvO3LiveDataHandoffError::InvalidTracePresence {
                    value: trace_present,
                });
            }
            let trace_sequence = read_u64(payload, &mut offset)?;
            if trace_present == 0 && trace_sequence != 0 {
                return Err(RiscvO3LiveDataHandoffError::UnexpectedTraceSequence {
                    sequence: trace_sequence,
                });
            }
            if trace_present == 1 && !trace_sequences.insert(trace_sequence) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateTraceSequence {
                    sequence: trace_sequence,
                });
            }
            entries.push(RiscvO3LiveDataHandoffEntry {
                fetch_request,
                data_request,
                issue_tick,
                partition,
                operation,
                ownership,
                target,
                address,
                bytes,
                o3_sequence,
                trace_sequence: (trace_present == 1).then_some(trace_sequence),
            });
        }
        let mut forwarded_rows = Vec::with_capacity(forwarded_count);
        for _ in 0..forwarded_count {
            let fetch_request = read_request(payload, &mut offset)?;
            let data_request = read_request(payload, &mut offset)?;
            let source_data_request = read_request(payload, &mut offset)?;
            if !fetch_requests.insert(fetch_request) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateFetchRequest {
                    request: fetch_request,
                });
            }
            if !data_requests.insert(data_request) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateDataRequest {
                    request: data_request,
                });
            }
            let issue_tick = read_u64(payload, &mut offset)?;
            let response_tick = read_u64(payload, &mut offset)?;
            if response_tick < issue_tick {
                return Err(RiscvO3LiveDataHandoffError::ForwardedResponseBeforeIssue {
                    issue_tick,
                    response_tick,
                });
            }
            let address = Address::new(read_u64(payload, &mut offset)?);
            let bytes = read_u32(payload, &mut offset)?;
            if !matches!(bytes, 1 | 2 | 4 | 8) {
                return Err(RiscvO3LiveDataHandoffError::InvalidScalarBytes { bytes });
            }
            if address.get().checked_add(u64::from(bytes) - 1).is_none() {
                return Err(RiscvO3LiveDataHandoffError::AddressRangeOverflow { address, bytes });
            }
            let data = read_array::<8>(payload, &mut offset)?;
            if let Some((index, value)) = data
                .iter()
                .copied()
                .enumerate()
                .skip(bytes as usize)
                .find(|(_, value)| *value != 0)
            {
                return Err(RiscvO3LiveDataHandoffError::NonZeroForwardedDataPadding {
                    index,
                    value,
                });
            }
            let o3_sequence = read_u64(payload, &mut offset)?;
            if !o3_sequences.insert(o3_sequence) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateO3Sequence {
                    sequence: o3_sequence,
                });
            }
            if let Some(previous) = previous_o3_sequence.filter(|previous| *previous >= o3_sequence)
            {
                return Err(RiscvO3LiveDataHandoffError::NonIncreasingO3Sequence {
                    previous,
                    current: o3_sequence,
                });
            }
            previous_o3_sequence = Some(o3_sequence);
            let source = entries
                .iter()
                .find(|entry| entry.data_request == source_data_request);
            if !source.is_some_and(|source| {
                source.operation == RiscvO3LiveDataHandoffOperation::Store
                    && source.address == address
                    && source.bytes == bytes
                    && source.o3_sequence < o3_sequence
            }) {
                return Err(RiscvO3LiveDataHandoffError::InvalidForwardingSource {
                    request: source_data_request,
                });
            }
            let trace_present = read_u8(payload, &mut offset)?;
            if trace_present > 1 {
                return Err(RiscvO3LiveDataHandoffError::InvalidTracePresence {
                    value: trace_present,
                });
            }
            let trace_sequence = read_u64(payload, &mut offset)?;
            if trace_present == 0 && trace_sequence != 0 {
                return Err(RiscvO3LiveDataHandoffError::UnexpectedTraceSequence {
                    sequence: trace_sequence,
                });
            }
            if trace_present == 1 && !trace_sequences.insert(trace_sequence) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateTraceSequence {
                    sequence: trace_sequence,
                });
            }
            forwarded_rows.push(RiscvO3LiveDataHandoffForwardedRow {
                fetch_request,
                data_request,
                source_data_request,
                issue_tick,
                response_tick,
                address,
                bytes,
                data,
                o3_sequence,
                trace_sequence: (trace_present == 1).then_some(trace_sequence),
            });
        }
        let mut partial_overlays = Vec::with_capacity(partial_overlay_count);
        for _ in 0..partial_overlay_count {
            let load_data_request = read_request(payload, &mut offset)?;
            let legacy_source_data_request = (!has_multi_source_partial_overlay(version))
                .then(|| read_request(payload, &mut offset))
                .transpose()?;
            let address = Address::new(read_u64(payload, &mut offset)?);
            let bytes = read_u32(payload, &mut offset)?;
            if !matches!(bytes, 1 | 2 | 4 | 8) {
                return Err(RiscvO3LiveDataHandoffError::InvalidScalarBytes { bytes });
            }
            if address.get().checked_add(u64::from(bytes) - 1).is_none() {
                return Err(RiscvO3LiveDataHandoffError::AddressRangeOverflow { address, bytes });
            }
            let forwarded_mask = read_u8(payload, &mut offset)?;
            let width_mask = scalar_byte_mask(bytes);
            if forwarded_mask == 0
                || forwarded_mask == width_mask
                || forwarded_mask & !width_mask != 0
            {
                return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayMask {
                    mask: forwarded_mask,
                    bytes,
                });
            }
            let data = read_array::<8>(payload, &mut offset)?;
            validate_partial_overlay_payload(bytes, forwarded_mask, &data)?;
            let load = entries
                .iter()
                .find(|entry| entry.data_request == load_data_request);
            if !load.is_some_and(|load| {
                load.operation == RiscvO3LiveDataHandoffOperation::Load
                    && matches!(load.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                    && load.address == address
                    && load.bytes == bytes
            }) {
                return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayLoad {
                    request: load_data_request,
                });
            }
            let load = load.expect("validated partial-overlay load entry");
            let sources = if let Some(source_data_request) = legacy_source_data_request {
                let source_data = read_array::<8>(payload, &mut offset)?;
                let source = entries
                    .iter()
                    .find(|entry| entry.data_request == source_data_request);
                if !source.is_some_and(|source| {
                    source.operation == RiscvO3LiveDataHandoffOperation::Store
                        && matches!(source.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                        && source.partition == load.partition
                        && source.target == load.target
                        && source.o3_sequence < load.o3_sequence
                }) {
                    return Err(RiscvO3LiveDataHandoffError::InvalidForwardingSource {
                        request: source_data_request,
                    });
                }
                let source = source.expect("validated partial-overlay source entry");
                let expected_mask =
                    partial_overlay_mask(source.address, source.bytes, address, bytes);
                if forwarded_mask != expected_mask {
                    return Err(RiscvO3LiveDataHandoffError::PartialOverlayMaskMismatch {
                        expected: expected_mask,
                        actual: forwarded_mask,
                    });
                }
                validate_partial_overlay_data(
                    source.address,
                    source.bytes,
                    address,
                    bytes,
                    forwarded_mask,
                    &data,
                    &source_data,
                )?;
                vec![RiscvO3LiveDataHandoffPartialOverlaySource {
                    source_data_request,
                    source_address: source.address,
                    source_bytes: source.bytes,
                    ownership_mask: forwarded_mask,
                    source_data,
                }]
            } else {
                let source_count = read_u32(payload, &mut offset)? as usize;
                let expected_sources = entries.len().saturating_sub(1).min(MAX_ROWS - 1);
                if source_count == 0 || source_count != expected_sources {
                    return Err(
                        RiscvO3LiveDataHandoffError::InvalidPartialOverlaySourceCount {
                            sources: source_count,
                            expected: expected_sources,
                        },
                    );
                }
                let mut source_requests = BTreeSet::new();
                let mut ownership_union = 0_u8;
                let mut physical_union = 0_u8;
                let mut sources = Vec::with_capacity(source_count);
                for source_index in 0..source_count {
                    let source_data_request = read_request(payload, &mut offset)?;
                    if !source_requests.insert(source_data_request) {
                        return Err(RiscvO3LiveDataHandoffError::DuplicatePartialOverlaySource {
                            request: source_data_request,
                        });
                    }
                    let ownership_mask = read_u8(payload, &mut offset)?;
                    let source_data = read_array::<8>(payload, &mut offset)?;
                    let source = entries.get(source_index);
                    if !source.is_some_and(|source| {
                        source.data_request == source_data_request
                            && source.operation == RiscvO3LiveDataHandoffOperation::Store
                            && matches!(source.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                            && source.partition == load.partition
                            && source.target == load.target
                            && source.o3_sequence < load.o3_sequence
                    }) {
                        return Err(RiscvO3LiveDataHandoffError::InvalidForwardingSource {
                            request: source_data_request,
                        });
                    }
                    let source = source.expect("validated partial-overlay source entry");
                    let physical_mask =
                        partial_overlay_mask(source.address, source.bytes, address, bytes);
                    if physical_mask == 0 || ownership_mask & !physical_mask != 0 {
                        return Err(RiscvO3LiveDataHandoffError::PartialOverlayMaskMismatch {
                            expected: physical_mask,
                            actual: ownership_mask,
                        });
                    }
                    physical_union |= physical_mask;
                    let overlapping = ownership_union & ownership_mask;
                    if overlapping != 0 {
                        return Err(
                            RiscvO3LiveDataHandoffError::OverlappingPartialOverlayOwnership {
                                mask: overlapping,
                            },
                        );
                    }
                    validate_partial_overlay_data(
                        source.address,
                        source.bytes,
                        address,
                        bytes,
                        ownership_mask,
                        &data,
                        &source_data,
                    )?;
                    ownership_union |= ownership_mask;
                    sources.push(RiscvO3LiveDataHandoffPartialOverlaySource {
                        source_data_request,
                        source_address: source.address,
                        source_bytes: source.bytes,
                        ownership_mask,
                        source_data,
                    });
                }
                if ownership_union != forwarded_mask {
                    return Err(
                        RiscvO3LiveDataHandoffError::IncompletePartialOverlayOwnership {
                            expected: forwarded_mask,
                            actual: ownership_union,
                        },
                    );
                }
                if physical_union != forwarded_mask {
                    return Err(RiscvO3LiveDataHandoffError::PartialOverlayMaskMismatch {
                        expected: physical_union,
                        actual: forwarded_mask,
                    });
                }
                sources
            };
            partial_overlays.push(RiscvO3LiveDataHandoffPartialOverlay {
                load_data_request,
                address,
                bytes,
                forwarded_mask,
                data,
                sources,
            });
        }
        let mut completed_partial_overlays = Vec::with_capacity(completed_partial_overlay_count);
        for _ in 0..completed_partial_overlay_count {
            let fetch_request = read_request(payload, &mut offset)?;
            let load_data_request = read_request(payload, &mut offset)?;
            if !fetch_requests.insert(fetch_request) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateFetchRequest {
                    request: fetch_request,
                });
            }
            if !data_requests.insert(load_data_request) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateDataRequest {
                    request: load_data_request,
                });
            }
            let issue_tick = read_u64(payload, &mut offset)?;
            let response_tick = read_u64(payload, &mut offset)?;
            if response_tick < issue_tick {
                return Err(RiscvO3LiveDataHandoffError::ForwardedResponseBeforeIssue {
                    issue_tick,
                    response_tick,
                });
            }
            let address = Address::new(read_u64(payload, &mut offset)?);
            let bytes = read_u32(payload, &mut offset)?;
            if !matches!(bytes, 1 | 2 | 4 | 8) {
                return Err(RiscvO3LiveDataHandoffError::InvalidScalarBytes { bytes });
            }
            if address.get().checked_add(u64::from(bytes) - 1).is_none() {
                return Err(RiscvO3LiveDataHandoffError::AddressRangeOverflow { address, bytes });
            }
            let original_forwarded_mask = read_u8(payload, &mut offset)?;
            let scalar_mask = scalar_byte_mask(bytes);
            if original_forwarded_mask == 0
                || original_forwarded_mask == scalar_mask
                || original_forwarded_mask & !scalar_mask != 0
            {
                return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayMask {
                    mask: original_forwarded_mask,
                    bytes,
                });
            }
            let live_forwarded_mask = read_u8(payload, &mut offset)?;
            if live_forwarded_mask == 0
                || live_forwarded_mask == scalar_mask
                || live_forwarded_mask & !original_forwarded_mask != 0
            {
                return Err(
                    RiscvO3LiveDataHandoffError::InvalidCompletedPartialOverlayLiveMask {
                        original: original_forwarded_mask,
                        live: live_forwarded_mask,
                    },
                );
            }
            let data = read_array::<8>(payload, &mut offset)?;
            if let Some((index, value)) = data
                .iter()
                .copied()
                .enumerate()
                .skip(bytes as usize)
                .find(|(_, value)| *value != 0)
            {
                return Err(RiscvO3LiveDataHandoffError::NonZeroForwardedDataPadding {
                    index,
                    value,
                });
            }
            let o3_sequence = read_u64(payload, &mut offset)?;
            let latest_source_sequence = entries
                .iter()
                .map(|entry| entry.o3_sequence)
                .max()
                .expect("completed overlay has at least one source entry");
            if latest_source_sequence >= o3_sequence {
                return Err(
                    RiscvO3LiveDataHandoffError::InvalidCompletedPartialOverlaySequence {
                        source: latest_source_sequence,
                        load: o3_sequence,
                    },
                );
            }
            if !o3_sequences.insert(o3_sequence) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateO3Sequence {
                    sequence: o3_sequence,
                });
            }
            if let Some(previous) = previous_o3_sequence.filter(|previous| *previous >= o3_sequence)
            {
                return Err(RiscvO3LiveDataHandoffError::NonIncreasingO3Sequence {
                    previous,
                    current: o3_sequence,
                });
            }
            previous_o3_sequence = Some(o3_sequence);
            let trace_present = read_u8(payload, &mut offset)?;
            if trace_present > 1 {
                return Err(RiscvO3LiveDataHandoffError::InvalidTracePresence {
                    value: trace_present,
                });
            }
            let trace_sequence = read_u64(payload, &mut offset)?;
            if trace_present == 0 && trace_sequence != 0 {
                return Err(RiscvO3LiveDataHandoffError::UnexpectedTraceSequence {
                    sequence: trace_sequence,
                });
            }
            if trace_present == 1 && !trace_sequences.insert(trace_sequence) {
                return Err(RiscvO3LiveDataHandoffError::DuplicateTraceSequence {
                    sequence: trace_sequence,
                });
            }
            let source_count = read_u32(payload, &mut offset)? as usize;
            if source_count == 0 || source_count != entries.len() {
                return Err(
                    RiscvO3LiveDataHandoffError::InvalidPartialOverlaySourceCount {
                        sources: source_count,
                        expected: entries.len(),
                    },
                );
            }
            let mut source_requests = BTreeSet::new();
            let mut ownership_union = 0_u8;
            let mut sources = Vec::with_capacity(source_count);
            for source_index in 0..source_count {
                let source_data_request = read_request(payload, &mut offset)?;
                if !source_requests.insert(source_data_request) {
                    return Err(RiscvO3LiveDataHandoffError::DuplicatePartialOverlaySource {
                        request: source_data_request,
                    });
                }
                let ownership_mask = read_u8(payload, &mut offset)?;
                let source_data = read_array::<8>(payload, &mut offset)?;
                let source = entries.get(source_index);
                if !source.is_some_and(|source| {
                    source.data_request == source_data_request
                        && source.operation == RiscvO3LiveDataHandoffOperation::Store
                        && matches!(source.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                        && source.o3_sequence < o3_sequence
                }) {
                    return Err(RiscvO3LiveDataHandoffError::InvalidForwardingSource {
                        request: source_data_request,
                    });
                }
                let source = source.expect("validated completed partial-overlay source entry");
                let physical_mask =
                    partial_overlay_mask(source.address, source.bytes, address, bytes);
                if ownership_mask == 0 || physical_mask == 0 || ownership_mask & !physical_mask != 0
                {
                    return Err(RiscvO3LiveDataHandoffError::PartialOverlayMaskMismatch {
                        expected: physical_mask,
                        actual: ownership_mask,
                    });
                }
                if ownership_mask & !live_forwarded_mask != 0 {
                    return Err(RiscvO3LiveDataHandoffError::PartialOverlayMaskMismatch {
                        expected: live_forwarded_mask,
                        actual: ownership_mask,
                    });
                }
                let overlapping = ownership_union & ownership_mask;
                if overlapping != 0 {
                    return Err(
                        RiscvO3LiveDataHandoffError::OverlappingPartialOverlayOwnership {
                            mask: overlapping,
                        },
                    );
                }
                validate_partial_overlay_data(
                    source.address,
                    source.bytes,
                    address,
                    bytes,
                    ownership_mask,
                    &data,
                    &source_data,
                )?;
                ownership_union |= ownership_mask;
                sources.push(RiscvO3LiveDataHandoffPartialOverlaySource {
                    source_data_request,
                    source_address: source.address,
                    source_bytes: source.bytes,
                    ownership_mask,
                    source_data,
                });
            }
            if ownership_union != live_forwarded_mask {
                return Err(
                    RiscvO3LiveDataHandoffError::IncompletePartialOverlayOwnership {
                        expected: live_forwarded_mask,
                        actual: ownership_union,
                    },
                );
            }
            let overlay = RiscvO3LiveDataHandoffCompletedPartialOverlay {
                fetch_request,
                load_data_request,
                issue_tick,
                response_tick,
                address,
                bytes,
                original_forwarded_mask,
                live_forwarded_mask,
                data,
                o3_sequence,
                trace_sequence: (trace_present == 1).then_some(trace_sequence),
                sources,
            };
            if !completed_partial_overlay_is_valid(&entries, &overlay) {
                return Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                    entries: entries.len(),
                    forwarded_rows: forwarded_rows.len(),
                    partial_overlays: partial_overlays.len(),
                    completed_partial_overlays: completed_partial_overlay_count,
                    younger_rows,
                });
            }
            completed_partial_overlays.push(overlay);
        }
        if offset != payload.len() {
            return Err(RiscvO3LiveDataHandoffError::InvalidPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }
        if version == VERSION_SINGLE_SOURCE_CURRENT
            && !single_source_shape_is_valid(
                &entries,
                forwarded_rows.len(),
                partial_overlays.len(),
                younger_rows,
            )
        {
            return Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: entries.len(),
                forwarded_rows: forwarded_rows.len(),
                partial_overlays: partial_overlays.len(),
                completed_partial_overlays: completed_partial_overlays.len(),
                younger_rows,
            });
        }
        if version == VERSION_MULTI_SOURCE_CURRENT
            && !current_shape_is_valid(
                &entries,
                &forwarded_rows,
                &partial_overlays,
                &[],
                younger_rows,
            )
        {
            return Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: entries.len(),
                forwarded_rows: forwarded_rows.len(),
                partial_overlays: partial_overlays.len(),
                completed_partial_overlays: 0,
                younger_rows,
            });
        }
        if version == VERSION_CURRENT
            && !current_shape_is_valid(
                &entries,
                &forwarded_rows,
                &partial_overlays,
                &completed_partial_overlays,
                younger_rows,
            )
        {
            return Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: entries.len(),
                forwarded_rows: forwarded_rows.len(),
                partial_overlays: partial_overlays.len(),
                completed_partial_overlays: completed_partial_overlays.len(),
                younger_rows,
            });
        }
        Ok((
            Self {
                entries,
                forwarded_rows,
                partial_overlays,
                completed_partial_overlays,
                younger_rows,
            },
            version,
        ))
    }
}

fn single_source_count_shape_is_valid(
    entries: usize,
    forwarded_rows: usize,
    partial_overlays: usize,
    younger_rows: u32,
) -> bool {
    match (forwarded_rows, partial_overlays) {
        (0, 0) => entries > 0,
        (1, 0) => entries == 1 && younger_rows == 0,
        (0, 1) => entries == 2 && younger_rows == 0,
        _ => false,
    }
}

fn current_count_shape_is_valid(
    entries: usize,
    forwarded_rows: usize,
    partial_overlays: usize,
    completed_partial_overlays: usize,
    younger_rows: u32,
) -> bool {
    match (forwarded_rows, partial_overlays, completed_partial_overlays) {
        (0, 0, 0) => entries > 0,
        (1, 0, 0) => entries == 1 && younger_rows == 0,
        (0, 1, 0) => (2..=MAX_ROWS).contains(&entries) && younger_rows == 0,
        (0, 0, 1) => (1..MAX_ROWS).contains(&entries) && younger_rows == 0,
        _ => false,
    }
}

fn single_source_shape_is_valid(
    entries: &[RiscvO3LiveDataHandoffEntry],
    forwarded_rows: usize,
    partial_overlays: usize,
    younger_rows: u32,
) -> bool {
    match (forwarded_rows, partial_overlays) {
        (0, 0) => entries.iter().all(|entry| {
            entry.operation == RiscvO3LiveDataHandoffOperation::Load
                && entry.ownership == RiscvO3LiveDataHandoffOwnership::Transport
        }),
        (1, 0) => {
            entries.len() == 1
                && younger_rows == 0
                && entries[0].operation == RiscvO3LiveDataHandoffOperation::Store
                && entries[0].ownership == RiscvO3LiveDataHandoffOwnership::Transport
        }
        (0, 1) => {
            entries.len() == 2
                && younger_rows == 0
                && entries[0].operation == RiscvO3LiveDataHandoffOperation::Store
                && entries[1].operation == RiscvO3LiveDataHandoffOperation::Load
        }
        _ => false,
    }
}

fn current_shape_is_valid(
    entries: &[RiscvO3LiveDataHandoffEntry],
    forwarded_rows: &[RiscvO3LiveDataHandoffForwardedRow],
    partial_overlays: &[RiscvO3LiveDataHandoffPartialOverlay],
    completed_partial_overlays: &[RiscvO3LiveDataHandoffCompletedPartialOverlay],
    younger_rows: u32,
) -> bool {
    match (
        forwarded_rows.len(),
        partial_overlays.len(),
        completed_partial_overlays.len(),
    ) {
        (0, 0, 0) => entries
            .iter()
            .all(|entry| entry.operation == RiscvO3LiveDataHandoffOperation::Load),
        (1, 0, 0) => {
            entries.len() == 1
                && younger_rows == 0
                && entries[0].operation == RiscvO3LiveDataHandoffOperation::Store
        }
        (0, 1, 0) => {
            let overlay = &partial_overlays[0];
            younger_rows == 0
                && (2..=MAX_ROWS).contains(&entries.len())
                && entries[..entries.len() - 1]
                    .iter()
                    .all(|entry| entry.operation == RiscvO3LiveDataHandoffOperation::Store)
                && entries[entries.len() - 1].operation == RiscvO3LiveDataHandoffOperation::Load
                && entries[entries.len() - 1].ownership
                    == RiscvO3LiveDataHandoffOwnership::Transport
                && overlay.load_data_request == entries[entries.len() - 1].data_request
                && overlay.sources.len() == entries.len() - 1
                && entries[..entries.len() - 1]
                    .iter()
                    .enumerate()
                    .zip(&overlay.sources)
                    .all(|((index, entry), source)| {
                        entry.data_request == source.source_data_request
                            && match entry.ownership {
                                RiscvO3LiveDataHandoffOwnership::Transport => true,
                                RiscvO3LiveDataHandoffOwnership::BufferedStore { predecessor } => {
                                    index > 0 && entries[index - 1].data_request == predecessor
                                }
                            }
                    })
        }
        (0, 0, 1) => {
            younger_rows == 0
                && completed_partial_overlay_is_valid(entries, &completed_partial_overlays[0])
        }
        _ => false,
    }
}
fn write_target(payload: &mut Vec<u8>, target: RiscvO3LiveDataHandoffTarget) {
    match target {
        RiscvO3LiveDataHandoffTarget::Memory { route } => {
            payload.push(TARGET_MEMORY);
            payload.extend_from_slice(&route.get().to_le_bytes());
        }
        RiscvO3LiveDataHandoffTarget::Mmio { route } => {
            payload.push(TARGET_MMIO);
            payload.extend_from_slice(&route.target_partition().index().to_le_bytes());
            payload.extend_from_slice(&route.request_latency().to_le_bytes());
            payload.extend_from_slice(&route.response_latency().to_le_bytes());
        }
    }
}

fn read_target(
    payload: &[u8],
    offset: &mut usize,
    version: u8,
    issue_tick: Tick,
    partition: PartitionId,
) -> Result<RiscvO3LiveDataHandoffTarget, RiscvO3LiveDataHandoffError> {
    if version == VERSION_MEMORY_ROUTE {
        return Ok(RiscvO3LiveDataHandoffTarget::Memory {
            route: MemoryRouteId::new(read_u64(payload, offset)?),
        });
    }

    match read_u8(payload, offset)? {
        TARGET_MEMORY => Ok(RiscvO3LiveDataHandoffTarget::Memory {
            route: MemoryRouteId::new(read_u64(payload, offset)?),
        }),
        TARGET_MMIO => {
            let target_partition = PartitionId::new(read_u32(payload, offset)?);
            let request_latency = read_u64(payload, offset)?;
            let response_latency = read_u64(payload, offset)?;
            let route = MmioRoute::new(
                partition,
                target_partition,
                request_latency,
                response_latency,
            )
            .map_err(|_| RiscvO3LiveDataHandoffError::InvalidMmioRoute {
                request_latency,
                response_latency,
            })?;
            issue_tick
                .checked_add(request_latency)
                .and_then(|tick| tick.checked_add(response_latency))
                .ok_or(RiscvO3LiveDataHandoffError::MmioRouteTickOverflow {
                    issue_tick,
                    request_latency,
                    response_latency,
                })?;
            Ok(RiscvO3LiveDataHandoffTarget::Mmio { route })
        }
        value => Err(RiscvO3LiveDataHandoffError::InvalidTargetKind { value }),
    }
}

fn write_request(payload: &mut Vec<u8>, request: MemoryRequestId) {
    payload.extend_from_slice(&request.agent().get().to_le_bytes());
    payload.extend_from_slice(&request.sequence().to_le_bytes());
}

fn read_request(
    payload: &[u8],
    offset: &mut usize,
) -> Result<MemoryRequestId, RiscvO3LiveDataHandoffError> {
    Ok(MemoryRequestId::new(
        AgentId::new(read_u32(payload, offset)?),
        read_u64(payload, offset)?,
    ))
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, RiscvO3LiveDataHandoffError> {
    let value =
        payload
            .get(*offset)
            .copied()
            .ok_or(RiscvO3LiveDataHandoffError::InvalidPayloadSize {
                expected: offset.saturating_add(1),
                actual: payload.len(),
            })?;
    *offset += 1;
    Ok(value)
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, RiscvO3LiveDataHandoffError> {
    let bytes = read_array::<4>(payload, offset)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, RiscvO3LiveDataHandoffError> {
    let bytes = read_array::<8>(payload, offset)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_array<const N: usize>(
    payload: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], RiscvO3LiveDataHandoffError> {
    let end = offset.saturating_add(N);
    let bytes =
        payload
            .get(*offset..end)
            .ok_or(RiscvO3LiveDataHandoffError::InvalidPayloadSize {
                expected: end,
                actual: payload.len(),
            })?;
    *offset = end;
    Ok(bytes.try_into().expect("checked fixed-width payload slice"))
}
