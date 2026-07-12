use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{Address, AgentId, MemoryRequestId};
use rem6_mmio::MmioRoute;
use rem6_transport::MemoryRouteId;

use crate::RiscvCore;

pub const RISCV_O3_LIVE_DATA_HANDOFF_CHUNK: &str = "o3-live-data-handoff";

const MAGIC: [u8; 4] = *b"O3DH";
const VERSION_MEMORY_ROUTE: u8 = 1;
const VERSION_TYPED_TARGET: u8 = 2;
const VERSION_FORWARDING: u8 = 3;
const HEADER_BYTES: usize = MAGIC.len() + 1 + 4 + 4;
const V1_ENTRY_BYTES: usize = 73;
const MAX_ROWS: usize = 4;
#[cfg(test)]
const V1_O3_SEQUENCE_OFFSET: usize = 56;
#[cfg(test)]
const V1_TRACE_SEQUENCE_OFFSET: usize = 65;
#[cfg(test)]
const ISSUE_TICK_OFFSET: usize = 24;
#[cfg(test)]
const V2_TARGET_KIND_OFFSET: usize = 36;
#[cfg(test)]
const V2_MMIO_REQUEST_LATENCY_OFFSET: usize = 41;

const TARGET_MEMORY: u8 = 0;
const TARGET_MMIO: u8 = 1;
const OPERATION_LOAD: u8 = 0;
const OPERATION_STORE: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveDataHandoffOperation {
    Load,
    Store,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveDataHandoffTarget {
    Memory { route: MemoryRouteId },
    Mmio { route: MmioRoute },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffEntry {
    fetch_request: MemoryRequestId,
    data_request: MemoryRequestId,
    issue_tick: Tick,
    partition: PartitionId,
    operation: RiscvO3LiveDataHandoffOperation,
    target: RiscvO3LiveDataHandoffTarget,
    address: Address,
    bytes: u32,
    o3_sequence: u64,
    trace_sequence: Option<u64>,
}

impl RiscvO3LiveDataHandoffEntry {
    pub const fn fetch_request(self) -> MemoryRequestId {
        self.fetch_request
    }

    pub const fn data_request(self) -> MemoryRequestId {
        self.data_request
    }

    pub const fn issue_tick(self) -> Tick {
        self.issue_tick
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn operation(self) -> RiscvO3LiveDataHandoffOperation {
        self.operation
    }

    pub const fn target(self) -> RiscvO3LiveDataHandoffTarget {
        self.target
    }

    pub const fn memory_route(self) -> Option<MemoryRouteId> {
        match self.target {
            RiscvO3LiveDataHandoffTarget::Memory { route } => Some(route),
            RiscvO3LiveDataHandoffTarget::Mmio { .. } => None,
        }
    }

    pub const fn mmio_route(self) -> Option<MmioRoute> {
        match self.target {
            RiscvO3LiveDataHandoffTarget::Memory { .. } => None,
            RiscvO3LiveDataHandoffTarget::Mmio { route } => Some(route),
        }
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn bytes(self) -> u32 {
        self.bytes
    }

    pub const fn o3_sequence(self) -> u64 {
        self.o3_sequence
    }

    pub const fn trace_sequence(self) -> Option<u64> {
        self.trace_sequence
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffForwardedRow {
    fetch_request: MemoryRequestId,
    data_request: MemoryRequestId,
    source_data_request: MemoryRequestId,
    issue_tick: Tick,
    response_tick: Tick,
    address: Address,
    bytes: u32,
    data: [u8; 8],
    o3_sequence: u64,
    trace_sequence: Option<u64>,
}

impl RiscvO3LiveDataHandoffForwardedRow {
    pub const fn fetch_request(self) -> MemoryRequestId {
        self.fetch_request
    }

    pub const fn data_request(self) -> MemoryRequestId {
        self.data_request
    }

    pub const fn source_data_request(self) -> MemoryRequestId {
        self.source_data_request
    }

    pub const fn issue_tick(self) -> Tick {
        self.issue_tick
    }

    pub const fn response_tick(self) -> Tick {
        self.response_tick
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn bytes(self) -> u32 {
        self.bytes
    }

    pub fn data(&self) -> &[u8] {
        &self.data[..self.bytes as usize]
    }

    pub const fn o3_sequence(self) -> u64 {
        self.o3_sequence
    }

    pub const fn trace_sequence(self) -> Option<u64> {
        self.trace_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoff {
    entries: Vec<RiscvO3LiveDataHandoffEntry>,
    forwarded_rows: Vec<RiscvO3LiveDataHandoffForwardedRow>,
    younger_rows: u32,
}

impl RiscvO3LiveDataHandoff {
    fn new(entries: Vec<RiscvO3LiveDataHandoffEntry>, younger_rows: usize) -> Option<Self> {
        let row_count = entries.len().checked_add(younger_rows)?;
        (!entries.is_empty() && row_count <= MAX_ROWS).then_some(Self {
            entries,
            forwarded_rows: Vec::new(),
            younger_rows: u32::try_from(younger_rows).ok()?,
        })
    }

    fn with_forwarded_rows(
        entries: Vec<RiscvO3LiveDataHandoffEntry>,
        forwarded_rows: Vec<RiscvO3LiveDataHandoffForwardedRow>,
        younger_rows: usize,
    ) -> Option<Self> {
        if entries.len() != 1 || forwarded_rows.len() != 1 || younger_rows != 0 {
            return None;
        }
        let row_count = entries
            .len()
            .checked_add(forwarded_rows.len())?
            .checked_add(younger_rows)?;
        (!entries.is_empty() && row_count <= MAX_ROWS).then_some(Self {
            entries,
            forwarded_rows,
            younger_rows: u32::try_from(younger_rows).ok()?,
        })
    }

    pub fn entries(&self) -> &[RiscvO3LiveDataHandoffEntry] {
        &self.entries
    }

    pub fn forwarded_rows(&self) -> &[RiscvO3LiveDataHandoffForwardedRow] {
        &self.forwarded_rows
    }

    pub fn resident_rows(&self) -> usize {
        self.entries.len() + self.forwarded_rows.len()
    }

    pub const fn younger_rows(&self) -> u32 {
        self.younger_rows
    }

    pub fn encode(&self) -> Vec<u8> {
        let entry_count = u32::try_from(self.entries.len()).expect("handoff entry count fits u32");
        let forwarded_count =
            u32::try_from(self.forwarded_rows.len()).expect("handoff forwarded row count fits u32");
        let version = if !self.forwarded_rows.is_empty()
            || self
                .entries
                .iter()
                .any(|entry| entry.operation != RiscvO3LiveDataHandoffOperation::Load)
        {
            VERSION_FORWARDING
        } else if self
            .entries
            .iter()
            .all(|entry| matches!(entry.target, RiscvO3LiveDataHandoffTarget::Memory { .. }))
        {
            VERSION_MEMORY_ROUTE
        } else {
            VERSION_TYPED_TARGET
        };
        let mut payload = Vec::with_capacity(HEADER_BYTES + self.entries.len() * V1_ENTRY_BYTES);
        payload.extend_from_slice(&MAGIC);
        payload.push(version);
        payload.extend_from_slice(&entry_count.to_le_bytes());
        payload.extend_from_slice(&self.younger_rows.to_le_bytes());
        if version == VERSION_FORWARDING {
            payload.extend_from_slice(&forwarded_count.to_le_bytes());
        }
        for entry in &self.entries {
            write_request(&mut payload, entry.fetch_request);
            write_request(&mut payload, entry.data_request);
            payload.extend_from_slice(&entry.issue_tick.to_le_bytes());
            payload.extend_from_slice(&entry.partition.index().to_le_bytes());
            if version == VERSION_FORWARDING {
                payload.push(match entry.operation {
                    RiscvO3LiveDataHandoffOperation::Load => OPERATION_LOAD,
                    RiscvO3LiveDataHandoffOperation::Store => OPERATION_STORE,
                });
            }
            match (version, entry.target) {
                (VERSION_MEMORY_ROUTE, RiscvO3LiveDataHandoffTarget::Memory { route }) => {
                    payload.extend_from_slice(&route.get().to_le_bytes())
                }
                (VERSION_TYPED_TARGET | VERSION_FORWARDING, target) => {
                    write_target(&mut payload, target)
                }
                (VERSION_MEMORY_ROUTE, RiscvO3LiveDataHandoffTarget::Mmio { .. }) => {
                    unreachable!("MMIO handoffs require typed-target encoding")
                }
                _ => unreachable!("selected a supported live-data handoff version"),
            }
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
        payload
    }

    pub fn decode(payload: &[u8]) -> Result<Self, RiscvO3LiveDataHandoffError> {
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
            VERSION_MEMORY_ROUTE | VERSION_TYPED_TARGET | VERSION_FORWARDING
        ) {
            return Err(RiscvO3LiveDataHandoffError::UnsupportedVersion { version });
        }
        let mut offset = MAGIC.len() + 1;
        let entry_count = read_u32(payload, &mut offset)? as usize;
        let younger_rows = read_u32(payload, &mut offset)?;
        let forwarded_count = if version == VERSION_FORWARDING {
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
        let resident_rows = entry_count.checked_add(forwarded_count).ok_or(
            RiscvO3LiveDataHandoffError::TooManyRows {
                entries: usize::MAX,
                younger_rows,
                maximum: MAX_ROWS,
            },
        )?;
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

        let mut entries = Vec::with_capacity(entry_count);
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
            let operation = if version == VERSION_FORWARDING {
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
            let target = read_target(payload, &mut offset, version, issue_tick, partition)?;
            if operation == RiscvO3LiveDataHandoffOperation::Store
                && !matches!(target, RiscvO3LiveDataHandoffTarget::Memory { .. })
            {
                return Err(RiscvO3LiveDataHandoffError::InvalidStoreTarget);
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
        if offset != payload.len() {
            return Err(RiscvO3LiveDataHandoffError::InvalidPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }
        Ok(Self {
            entries,
            forwarded_rows,
            younger_rows,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveDataHandoffError {
    InvalidPayloadSize {
        expected: usize,
        actual: usize,
    },
    InvalidMagic,
    UnsupportedVersion {
        version: u8,
    },
    InvalidTargetKind {
        value: u8,
    },
    InvalidOperationKind {
        value: u8,
    },
    InvalidStoreTarget,
    InvalidForwardingShape {
        entries: usize,
        forwarded_rows: usize,
        younger_rows: u32,
    },
    InvalidMmioRoute {
        request_latency: Tick,
        response_latency: Tick,
    },
    MmioRouteTickOverflow {
        issue_tick: Tick,
        request_latency: Tick,
        response_latency: Tick,
    },
    EmptyEntries,
    TooManyRows {
        entries: usize,
        younger_rows: u32,
        maximum: usize,
    },
    DuplicateFetchRequest {
        request: MemoryRequestId,
    },
    DuplicateDataRequest {
        request: MemoryRequestId,
    },
    DuplicateO3Sequence {
        sequence: u64,
    },
    NonIncreasingO3Sequence {
        previous: u64,
        current: u64,
    },
    DuplicateTraceSequence {
        sequence: u64,
    },
    InvalidScalarBytes {
        bytes: u32,
    },
    AddressRangeOverflow {
        address: Address,
        bytes: u32,
    },
    ForwardedResponseBeforeIssue {
        issue_tick: Tick,
        response_tick: Tick,
    },
    NonZeroForwardedDataPadding {
        index: usize,
        value: u8,
    },
    InvalidForwardingSource {
        request: MemoryRequestId,
    },
    InvalidTracePresence {
        value: u8,
    },
    UnexpectedTraceSequence {
        sequence: u64,
    },
}

impl fmt::Display for RiscvO3LiveDataHandoffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPayloadSize { expected, actual } => write!(
                formatter,
                "live-data handoff has {actual} bytes; expected {expected}"
            ),
            Self::InvalidMagic => write!(formatter, "live-data handoff has invalid magic"),
            Self::UnsupportedVersion { version } => {
                write!(
                    formatter,
                    "live-data handoff version {version} is unsupported"
                )
            }
            Self::InvalidTargetKind { value } => {
                write!(formatter, "live-data handoff target kind {value} is invalid")
            }
            Self::InvalidOperationKind { value } => {
                write!(formatter, "live-data handoff operation kind {value} is invalid")
            }
            Self::InvalidStoreTarget => {
                write!(formatter, "live-data handoff store target must be memory")
            }
            Self::InvalidForwardingShape {
                entries,
                forwarded_rows,
                younger_rows,
            } => write!(
                formatter,
                "live-data handoff forwarding shape has {entries} transport entries, {forwarded_rows} forwarded rows, and {younger_rows} younger rows"
            ),
            Self::InvalidMmioRoute {
                request_latency,
                response_latency,
            } => write!(
                formatter,
                "live-data handoff MMIO route has request latency {request_latency} and response latency {response_latency}"
            ),
            Self::MmioRouteTickOverflow {
                issue_tick,
                request_latency,
                response_latency,
            } => write!(
                formatter,
                "live-data handoff MMIO route at tick {issue_tick} with request latency {request_latency} and response latency {response_latency} overflows the tick range"
            ),
            Self::EmptyEntries => write!(formatter, "live-data handoff has no entries"),
            Self::TooManyRows {
                entries,
                younger_rows,
                maximum,
            } => write!(
                formatter,
                "live-data handoff has {entries} entries and {younger_rows} younger rows; maximum is {maximum} total rows"
            ),
            Self::DuplicateFetchRequest { request } => write!(
                formatter,
                "live-data handoff repeats fetch request {}:{}",
                request.agent().get(),
                request.sequence()
            ),
            Self::DuplicateDataRequest { request } => write!(
                formatter,
                "live-data handoff repeats data request {}:{}",
                request.agent().get(),
                request.sequence()
            ),
            Self::DuplicateO3Sequence { sequence } => {
                write!(formatter, "live-data handoff repeats O3 sequence {sequence}")
            }
            Self::NonIncreasingO3Sequence { previous, current } => write!(
                formatter,
                "live-data handoff O3 sequence {current} does not follow {previous}"
            ),
            Self::DuplicateTraceSequence { sequence } => write!(
                formatter,
                "live-data handoff repeats trace sequence {sequence}"
            ),
            Self::InvalidScalarBytes { bytes } => write!(
                formatter,
                "live-data handoff scalar entry has invalid width {bytes} bytes"
            ),
            Self::AddressRangeOverflow { address, bytes } => write!(
                formatter,
                "live-data handoff entry at 0x{:x} with {bytes} bytes overflows the address range",
                address.get()
            ),
            Self::ForwardedResponseBeforeIssue {
                issue_tick,
                response_tick,
            } => write!(
                formatter,
                "live-data handoff forwarded response tick {response_tick} precedes issue tick {issue_tick}"
            ),
            Self::NonZeroForwardedDataPadding { index, value } => write!(
                formatter,
                "live-data handoff forwarded data padding byte {index} has value {value}"
            ),
            Self::InvalidForwardingSource { request } => write!(
                formatter,
                "live-data handoff forwarding source {}:{} is invalid",
                request.agent().get(),
                request.sequence()
            ),
            Self::InvalidTracePresence { value } => write!(
                formatter,
                "live-data handoff trace-presence value {value} is invalid"
            ),
            Self::UnexpectedTraceSequence { sequence } => write!(
                formatter,
                "live-data handoff has absent trace presence with sequence {sequence}"
            ),
        }
    }
}

impl Error for RiscvO3LiveDataHandoffError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvIssuedScalarMemoryHandoff {
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) data_request: MemoryRequestId,
    pub(crate) issue_tick: Tick,
    pub(crate) partition: PartitionId,
    pub(crate) operation: RiscvO3LiveDataHandoffOperation,
    pub(crate) target: RiscvO3LiveDataHandoffTarget,
    pub(crate) address: Address,
    pub(crate) bytes: u32,
    pub(crate) store_data: Option<[u8; 8]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvResidentScalarMemoryHandoff {
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) data_request: MemoryRequestId,
    pub(crate) issue_tick: Tick,
    pub(crate) operation: RiscvO3LiveDataHandoffOperation,
    pub(crate) o3_sequence: u64,
    pub(crate) trace_sequence: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvForwardedScalarLoadHandoff {
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) data_request: MemoryRequestId,
    pub(crate) issue_tick: Tick,
    pub(crate) response_tick: Tick,
    pub(crate) address: Address,
    pub(crate) bytes: u32,
    pub(crate) data: [u8; 8],
    pub(crate) o3_sequence: u64,
    pub(crate) trace_sequence: Option<u64>,
}

impl RiscvCore {
    pub fn capture_o3_live_data_handoff(&self) -> Option<RiscvO3LiveDataHandoff> {
        let state = self.state.lock().expect("riscv core lock");
        if state
            .data_translation
            .as_ref()
            .is_some_and(|frontend| !frontend.is_empty())
            || !state.pending_data_translations.is_empty()
            || !state.ready_translated_data.is_empty()
            || !state.buffered_o3_stores.is_empty()
            || state.outstanding_data.is_empty()
            || state.events.iter().any(|event| {
                event.execution().memory_access().is_some()
                    && !state
                        .issued_data_for_fetches
                        .contains(&event.fetch().request_id())
            })
        {
            return None;
        }
        let (resident_rows, forwarded_rows, younger_rows) =
            state.o3_runtime.live_scalar_memory_handoff()?;
        let mut rows = resident_rows
            .into_iter()
            .map(|row| (row.data_request, row))
            .collect::<BTreeMap<_, _>>();
        if rows.len() != state.outstanding_data.len() {
            return None;
        }

        let mut entries = Vec::with_capacity(rows.len());
        let mut issued_rows = Vec::with_capacity(rows.len());
        for issued in state.outstanding_data.values() {
            let issued = issued.scalar_memory_handoff()?;
            let resident = rows.remove(&issued.data_request)?;
            if resident.fetch_request != issued.fetch_request
                || resident.issue_tick != issued.issue_tick
                || resident.operation != issued.operation
            {
                return None;
            }
            match issued.target {
                RiscvO3LiveDataHandoffTarget::Memory { .. }
                    if state
                        .pma
                        .is_uncacheable(issued.address.get(), u64::from(issued.bytes))
                        .ok()? =>
                {
                    return None;
                }
                RiscvO3LiveDataHandoffTarget::Mmio { route }
                    if route.source_partition() != issued.partition =>
                {
                    return None;
                }
                _ => {}
            }
            entries.push(RiscvO3LiveDataHandoffEntry {
                fetch_request: issued.fetch_request,
                data_request: issued.data_request,
                issue_tick: issued.issue_tick,
                partition: issued.partition,
                operation: issued.operation,
                target: issued.target,
                address: issued.address,
                bytes: issued.bytes,
                o3_sequence: resident.o3_sequence,
                trace_sequence: resident.trace_sequence,
            });
            issued_rows.push(issued);
        }
        if !rows.is_empty() {
            return None;
        }
        entries.sort_by_key(|entry| entry.o3_sequence);
        if forwarded_rows.is_empty() {
            if entries
                .iter()
                .any(|entry| entry.operation != RiscvO3LiveDataHandoffOperation::Load)
            {
                return None;
            }
            return RiscvO3LiveDataHandoff::new(entries, younger_rows);
        }
        if forwarded_rows.len() != 1 || entries.len() != 1 || younger_rows != 0 {
            return None;
        }
        let forwarded = forwarded_rows[0];
        let source = issued_rows.iter().find(|issued| {
            issued.operation == RiscvO3LiveDataHandoffOperation::Store
                && matches!(issued.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                && issued.address == forwarded.address
                && issued.bytes == forwarded.bytes
                && issued.store_data.is_some_and(|data| {
                    data[..forwarded.bytes as usize] == forwarded.data[..forwarded.bytes as usize]
                })
        })?;
        let source_entry = entries
            .iter()
            .find(|entry| entry.data_request == source.data_request)?;
        if source_entry.o3_sequence >= forwarded.o3_sequence {
            return None;
        }
        let forwarded_rows = vec![RiscvO3LiveDataHandoffForwardedRow {
            fetch_request: forwarded.fetch_request,
            data_request: forwarded.data_request,
            source_data_request: source.data_request,
            issue_tick: forwarded.issue_tick,
            response_tick: forwarded.response_tick,
            address: forwarded.address,
            bytes: forwarded.bytes,
            data: forwarded.data,
            o3_sequence: forwarded.o3_sequence,
            trace_sequence: forwarded.trace_sequence,
        }];
        RiscvO3LiveDataHandoff::with_forwarded_rows(entries, forwarded_rows, younger_rows)
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

#[cfg(test)]
mod tests {
    use super::*;

    const LEGACY_V1_SINGLE_ENTRY_PAYLOAD: [u8; HEADER_BYTES + V1_ENTRY_BYTES] = [
        0x4f, 0x33, 0x44, 0x48, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00,
        0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x0b,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1e, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x80, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x01, 0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    fn entry(sequence: u64) -> RiscvO3LiveDataHandoffEntry {
        RiscvO3LiveDataHandoffEntry {
            fetch_request: MemoryRequestId::new(AgentId::new(3), sequence),
            data_request: MemoryRequestId::new(AgentId::new(4), sequence + 10),
            issue_tick: 29 + sequence,
            partition: PartitionId::new(2),
            operation: RiscvO3LiveDataHandoffOperation::Load,
            target: RiscvO3LiveDataHandoffTarget::Memory {
                route: MemoryRouteId::new(7),
            },
            address: Address::new(0x8000 + sequence * 4),
            bytes: 4,
            o3_sequence: sequence,
            trace_sequence: Some(sequence + 20),
        }
    }

    fn mmio_entry(sequence: u64) -> RiscvO3LiveDataHandoffEntry {
        RiscvO3LiveDataHandoffEntry {
            target: RiscvO3LiveDataHandoffTarget::Mmio {
                route: rem6_mmio::MmioRoute::new(PartitionId::new(2), PartitionId::new(5), 7, 11)
                    .unwrap(),
            },
            ..entry(sequence)
        }
    }

    fn forwarded_row(sequence: u64, source: MemoryRequestId) -> RiscvO3LiveDataHandoffForwardedRow {
        RiscvO3LiveDataHandoffForwardedRow {
            fetch_request: MemoryRequestId::new(AgentId::new(3), sequence),
            data_request: MemoryRequestId::new(AgentId::new(4), sequence + 10),
            source_data_request: source,
            issue_tick: 29 + sequence,
            response_tick: 31 + sequence,
            address: Address::new(0x8004),
            bytes: 4,
            data: [0x2a, 0, 0, 0, 0, 0, 0, 0],
            o3_sequence: sequence,
            trace_sequence: Some(sequence + 20),
        }
    }

    #[test]
    fn live_data_handoff_round_trips_entries() {
        let handoff = RiscvO3LiveDataHandoff::new(vec![entry(1), entry(2)], 2).unwrap();
        let payload = handoff.encode();

        assert_eq!(payload[MAGIC.len()], VERSION_MEMORY_ROUTE);
        assert_eq!(payload.len(), HEADER_BYTES + 2 * V1_ENTRY_BYTES);
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&payload),
            Ok(handoff.clone())
        );
        assert_eq!(
            handoff.entries()[0].memory_route(),
            Some(MemoryRouteId::new(7))
        );
        assert_eq!(handoff.entries()[0].mmio_route(), None);
    }

    #[test]
    fn live_data_handoff_decodes_and_reencodes_legacy_v1_bytes() {
        let decoded = RiscvO3LiveDataHandoff::decode(&LEGACY_V1_SINGLE_ENTRY_PAYLOAD).unwrap();

        assert_eq!(
            decoded,
            RiscvO3LiveDataHandoff::new(vec![entry(1)], 0).unwrap()
        );
        assert_eq!(
            decoded.entries()[0].memory_route(),
            Some(MemoryRouteId::new(7))
        );
        assert_eq!(decoded.encode(), LEGACY_V1_SINGLE_ENTRY_PAYLOAD);
    }

    #[test]
    fn live_data_handoff_round_trips_typed_memory_and_mmio_targets() {
        let memory = entry(1);
        let mmio = mmio_entry(2);
        let handoff = RiscvO3LiveDataHandoff::new(vec![memory, mmio], 1).unwrap();
        let payload = handoff.encode();

        assert_eq!(payload[MAGIC.len()], VERSION_TYPED_TARGET);
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&payload),
            Ok(handoff.clone())
        );
        assert_eq!(
            handoff.entries()[0].target(),
            RiscvO3LiveDataHandoffTarget::Memory {
                route: MemoryRouteId::new(7),
            }
        );
        assert_eq!(
            handoff.entries()[1].mmio_route(),
            Some(
                rem6_mmio::MmioRoute::new(PartitionId::new(2), PartitionId::new(5), 7, 11,)
                    .unwrap()
            )
        );
        assert_eq!(handoff.entries()[1].memory_route(), None);
    }

    #[test]
    fn live_data_handoff_round_trips_transport_store_and_forwarded_load() {
        let mut store = entry(1);
        store.operation = RiscvO3LiveDataHandoffOperation::Store;
        store.address = Address::new(0x8004);
        let forwarded = forwarded_row(2, store.data_request);
        let handoff =
            RiscvO3LiveDataHandoff::with_forwarded_rows(vec![store], vec![forwarded], 0).unwrap();
        let payload = handoff.encode();

        assert_eq!(payload[MAGIC.len()], VERSION_FORWARDING);
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&payload),
            Ok(handoff.clone())
        );
        assert_eq!(
            handoff.entries()[0].operation(),
            RiscvO3LiveDataHandoffOperation::Store
        );
        assert_eq!(handoff.forwarded_rows(), &[forwarded]);
        assert_eq!(handoff.resident_rows(), 2);
        assert_eq!(forwarded.data(), &[0x2a, 0, 0, 0]);
    }

    #[test]
    fn live_data_handoff_rejects_unknown_or_invalid_typed_targets() {
        let payload = RiscvO3LiveDataHandoff::new(vec![mmio_entry(1)], 0)
            .unwrap()
            .encode();
        let mut unknown_kind = payload.clone();
        unknown_kind[HEADER_BYTES + V2_TARGET_KIND_OFFSET] = 7;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&unknown_kind),
            Err(RiscvO3LiveDataHandoffError::InvalidTargetKind { value: 7 })
        );

        let mut zero_request_latency = payload;
        let request_latency = HEADER_BYTES + V2_MMIO_REQUEST_LATENCY_OFFSET;
        zero_request_latency[request_latency..request_latency + 8]
            .copy_from_slice(&0_u64.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&zero_request_latency),
            Err(RiscvO3LiveDataHandoffError::InvalidMmioRoute {
                request_latency: 0,
                response_latency: 11,
            })
        );

        let mut overflowing_tick = RiscvO3LiveDataHandoff::new(vec![mmio_entry(1)], 0)
            .unwrap()
            .encode();
        let issue_tick = u64::MAX - 5;
        let issue_offset = HEADER_BYTES + ISSUE_TICK_OFFSET;
        overflowing_tick[issue_offset..issue_offset + 8].copy_from_slice(&issue_tick.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&overflowing_tick),
            Err(RiscvO3LiveDataHandoffError::MmioRouteTickOverflow {
                issue_tick,
                request_latency: 7,
                response_latency: 11,
            })
        );
    }

    #[test]
    fn live_data_handoff_rejects_bad_magic_version_and_size() {
        let payload = RiscvO3LiveDataHandoff::new(vec![entry(1)], 0)
            .unwrap()
            .encode();
        let mut bad_magic = payload.clone();
        bad_magic[0] = b'X';
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&bad_magic),
            Err(RiscvO3LiveDataHandoffError::InvalidMagic)
        );
        let mut bad_version = payload.clone();
        bad_version[MAGIC.len()] = VERSION_FORWARDING + 1;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&bad_version),
            Err(RiscvO3LiveDataHandoffError::UnsupportedVersion {
                version: VERSION_FORWARDING + 1,
            })
        );
        assert!(matches!(
            RiscvO3LiveDataHandoff::decode(&payload[..payload.len() - 1]),
            Err(RiscvO3LiveDataHandoffError::InvalidPayloadSize { .. })
        ));
        let mut trailing = payload;
        trailing.push(0);
        assert!(matches!(
            RiscvO3LiveDataHandoff::decode(&trailing),
            Err(RiscvO3LiveDataHandoffError::InvalidPayloadSize { .. })
        ));
    }

    #[test]
    fn live_data_handoff_rejects_unknown_forwarding_source() {
        let mut store = entry(1);
        store.operation = RiscvO3LiveDataHandoffOperation::Store;
        store.address = Address::new(0x8004);
        let unknown = MemoryRequestId::new(AgentId::new(9), 99);
        let handoff = RiscvO3LiveDataHandoff::with_forwarded_rows(
            vec![store],
            vec![forwarded_row(2, unknown)],
            0,
        )
        .unwrap();

        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&handoff.encode()),
            Err(RiscvO3LiveDataHandoffError::InvalidForwardingSource { request: unknown })
        );
    }

    #[test]
    fn live_data_handoff_rejects_store_only_v3_shape() {
        let mut store = entry(1);
        store.operation = RiscvO3LiveDataHandoffOperation::Store;
        let handoff = RiscvO3LiveDataHandoff {
            entries: vec![store],
            forwarded_rows: Vec::new(),
            younger_rows: 0,
        };

        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&handoff.encode()),
            Err(RiscvO3LiveDataHandoffError::InvalidForwardingShape {
                entries: 1,
                forwarded_rows: 0,
                younger_rows: 0,
            })
        );
    }

    #[test]
    fn live_data_handoff_rejects_duplicate_requests() {
        let duplicate = RiscvO3LiveDataHandoff::new(vec![entry(1), entry(1)], 0).unwrap();

        assert!(matches!(
            RiscvO3LiveDataHandoff::decode(&duplicate.encode()),
            Err(RiscvO3LiveDataHandoffError::DuplicateFetchRequest { .. })
        ));
    }

    #[test]
    fn live_data_handoff_rejects_unbounded_or_invalid_rows() {
        let payload = RiscvO3LiveDataHandoff::new(vec![entry(1)], 0)
            .unwrap()
            .encode();
        let mut too_many_rows = payload.clone();
        too_many_rows[MAGIC.len() + 1 + 4..HEADER_BYTES]
            .copy_from_slice(&(MAX_ROWS as u32).to_le_bytes());
        assert!(matches!(
            RiscvO3LiveDataHandoff::decode(&too_many_rows),
            Err(RiscvO3LiveDataHandoffError::TooManyRows { .. })
        ));

        let mut invalid_width = payload.clone();
        let bytes_offset = HEADER_BYTES + 52;
        invalid_width[bytes_offset..bytes_offset + 4].copy_from_slice(&3_u32.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&invalid_width),
            Err(RiscvO3LiveDataHandoffError::InvalidScalarBytes { bytes: 3 })
        );

        let mut overflowing = payload;
        let address_offset = HEADER_BYTES + 44;
        overflowing[address_offset..address_offset + 8]
            .copy_from_slice(&(u64::MAX - 2).to_le_bytes());
        assert!(matches!(
            RiscvO3LiveDataHandoff::decode(&overflowing),
            Err(RiscvO3LiveDataHandoffError::AddressRangeOverflow { .. })
        ));
    }

    #[test]
    fn live_data_handoff_rejects_duplicate_o3_and_trace_sequences() {
        let handoff = RiscvO3LiveDataHandoff {
            entries: vec![entry(1), entry(2)],
            forwarded_rows: Vec::new(),
            younger_rows: 0,
        };
        let payload = handoff.encode();
        let mut duplicate_o3 = payload.clone();
        let second_o3 = HEADER_BYTES + V1_ENTRY_BYTES + V1_O3_SEQUENCE_OFFSET;
        duplicate_o3[second_o3..second_o3 + 8].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&duplicate_o3),
            Err(RiscvO3LiveDataHandoffError::DuplicateO3Sequence { sequence: 1 })
        );

        let mut duplicate_trace = payload;
        let second_trace = HEADER_BYTES + V1_ENTRY_BYTES + V1_TRACE_SEQUENCE_OFFSET;
        duplicate_trace[second_trace..second_trace + 8].copy_from_slice(&21_u64.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&duplicate_trace),
            Err(RiscvO3LiveDataHandoffError::DuplicateTraceSequence { sequence: 21 })
        );
    }
}
