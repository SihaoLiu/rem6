use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{Address, AgentId, MemoryRequestId};
use rem6_transport::MemoryRouteId;

use crate::RiscvCore;

pub const RISCV_O3_LIVE_DATA_HANDOFF_CHUNK: &str = "o3-live-data-handoff";

const MAGIC: [u8; 4] = *b"O3DH";
const VERSION: u8 = 1;
const HEADER_BYTES: usize = MAGIC.len() + 1 + 4 + 4;
const ENTRY_BYTES: usize = 73;
const MAX_ROWS: usize = 4;
#[cfg(test)]
const O3_SEQUENCE_OFFSET: usize = 56;
#[cfg(test)]
const TRACE_SEQUENCE_OFFSET: usize = 65;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffEntry {
    fetch_request: MemoryRequestId,
    data_request: MemoryRequestId,
    issue_tick: Tick,
    partition: PartitionId,
    route: MemoryRouteId,
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

    pub const fn route(self) -> MemoryRouteId {
        self.route
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoff {
    entries: Vec<RiscvO3LiveDataHandoffEntry>,
    younger_rows: u32,
}

impl RiscvO3LiveDataHandoff {
    fn new(entries: Vec<RiscvO3LiveDataHandoffEntry>, younger_rows: usize) -> Option<Self> {
        let row_count = entries.len().checked_add(younger_rows)?;
        (!entries.is_empty() && row_count <= MAX_ROWS).then_some(Self {
            entries,
            younger_rows: u32::try_from(younger_rows).ok()?,
        })
    }

    pub fn entries(&self) -> &[RiscvO3LiveDataHandoffEntry] {
        &self.entries
    }

    pub const fn younger_rows(&self) -> u32 {
        self.younger_rows
    }

    pub fn encode(&self) -> Vec<u8> {
        let entry_count = u32::try_from(self.entries.len()).expect("handoff entry count fits u32");
        let mut payload = Vec::with_capacity(HEADER_BYTES + self.entries.len() * ENTRY_BYTES);
        payload.extend_from_slice(&MAGIC);
        payload.push(VERSION);
        payload.extend_from_slice(&entry_count.to_le_bytes());
        payload.extend_from_slice(&self.younger_rows.to_le_bytes());
        for entry in &self.entries {
            write_request(&mut payload, entry.fetch_request);
            write_request(&mut payload, entry.data_request);
            payload.extend_from_slice(&entry.issue_tick.to_le_bytes());
            payload.extend_from_slice(&entry.partition.index().to_le_bytes());
            payload.extend_from_slice(&entry.route.get().to_le_bytes());
            payload.extend_from_slice(&entry.address.get().to_le_bytes());
            payload.extend_from_slice(&entry.bytes.to_le_bytes());
            payload.extend_from_slice(&entry.o3_sequence.to_le_bytes());
            payload.push(u8::from(entry.trace_sequence.is_some()));
            payload.extend_from_slice(&entry.trace_sequence.unwrap_or_default().to_le_bytes());
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
        if payload[MAGIC.len()] != VERSION {
            return Err(RiscvO3LiveDataHandoffError::UnsupportedVersion {
                version: payload[MAGIC.len()],
            });
        }
        let mut offset = MAGIC.len() + 1;
        let entry_count = read_u32(payload, &mut offset)? as usize;
        let younger_rows = read_u32(payload, &mut offset)?;
        if entry_count == 0 {
            return Err(RiscvO3LiveDataHandoffError::EmptyEntries);
        }
        let row_count = entry_count.checked_add(younger_rows as usize).ok_or(
            RiscvO3LiveDataHandoffError::TooManyRows {
                entries: entry_count,
                younger_rows,
                maximum: MAX_ROWS,
            },
        )?;
        if row_count > MAX_ROWS {
            return Err(RiscvO3LiveDataHandoffError::TooManyRows {
                entries: entry_count,
                younger_rows,
                maximum: MAX_ROWS,
            });
        }
        let expected = HEADER_BYTES + entry_count * ENTRY_BYTES;
        if payload.len() != expected {
            return Err(RiscvO3LiveDataHandoffError::InvalidPayloadSize {
                expected,
                actual: payload.len(),
            });
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
            let route = MemoryRouteId::new(read_u64(payload, &mut offset)?);
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
                route,
                address,
                bytes,
                o3_sequence,
                trace_sequence: (trace_present == 1).then_some(trace_sequence),
            });
        }
        Ok(Self {
            entries,
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
pub(crate) struct RiscvIssuedScalarLoadHandoff {
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) data_request: MemoryRequestId,
    pub(crate) issue_tick: Tick,
    pub(crate) partition: PartitionId,
    pub(crate) route: MemoryRouteId,
    pub(crate) address: Address,
    pub(crate) bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvResidentScalarMemoryHandoff {
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) data_request: MemoryRequestId,
    pub(crate) issue_tick: Tick,
    pub(crate) o3_sequence: u64,
    pub(crate) trace_sequence: Option<u64>,
}

impl RiscvCore {
    pub fn capture_o3_live_data_handoff(&self) -> Option<RiscvO3LiveDataHandoff> {
        let state = self.state.lock().expect("riscv core lock");
        if state.data_translation.is_some()
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
        let (resident_rows, younger_rows) = state.o3_runtime.resident_scalar_memory_handoff()?;
        let mut rows = resident_rows
            .into_iter()
            .map(|row| (row.data_request, row))
            .collect::<BTreeMap<_, _>>();
        if rows.len() != state.outstanding_data.len() {
            return None;
        }

        let mut entries = Vec::with_capacity(rows.len());
        for issued in state.outstanding_data.values() {
            let issued = issued.scalar_load_handoff()?;
            let resident = rows.remove(&issued.data_request)?;
            if resident.fetch_request != issued.fetch_request
                || resident.issue_tick != issued.issue_tick
                || state
                    .pma
                    .is_uncacheable(issued.address.get(), u64::from(issued.bytes))
                    .ok()?
            {
                return None;
            }
            entries.push(RiscvO3LiveDataHandoffEntry {
                fetch_request: issued.fetch_request,
                data_request: issued.data_request,
                issue_tick: issued.issue_tick,
                partition: issued.partition,
                route: issued.route,
                address: issued.address,
                bytes: issued.bytes,
                o3_sequence: resident.o3_sequence,
                trace_sequence: resident.trace_sequence,
            });
        }
        if !rows.is_empty() {
            return None;
        }
        entries.sort_by_key(|entry| entry.o3_sequence);
        RiscvO3LiveDataHandoff::new(entries, younger_rows)
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

    fn entry(sequence: u64) -> RiscvO3LiveDataHandoffEntry {
        RiscvO3LiveDataHandoffEntry {
            fetch_request: MemoryRequestId::new(AgentId::new(3), sequence),
            data_request: MemoryRequestId::new(AgentId::new(4), sequence + 10),
            issue_tick: 29 + sequence,
            partition: PartitionId::new(2),
            route: MemoryRouteId::new(7),
            address: Address::new(0x8000 + sequence * 4),
            bytes: 4,
            o3_sequence: sequence,
            trace_sequence: Some(sequence + 20),
        }
    }

    #[test]
    fn live_data_handoff_round_trips_entries() {
        let handoff = RiscvO3LiveDataHandoff::new(vec![entry(1), entry(2)], 2).unwrap();

        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&handoff.encode()),
            Ok(handoff)
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
        bad_version[MAGIC.len()] = VERSION + 1;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&bad_version),
            Err(RiscvO3LiveDataHandoffError::UnsupportedVersion {
                version: VERSION + 1,
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
            younger_rows: 0,
        };
        let payload = handoff.encode();
        let mut duplicate_o3 = payload.clone();
        let second_o3 = HEADER_BYTES + ENTRY_BYTES + O3_SEQUENCE_OFFSET;
        duplicate_o3[second_o3..second_o3 + 8].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&duplicate_o3),
            Err(RiscvO3LiveDataHandoffError::DuplicateO3Sequence { sequence: 1 })
        );

        let mut duplicate_trace = payload;
        let second_trace = HEADER_BYTES + ENTRY_BYTES + TRACE_SEQUENCE_OFFSET;
        duplicate_trace[second_trace..second_trace + 8].copy_from_slice(&21_u64.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&duplicate_trace),
            Err(RiscvO3LiveDataHandoffError::DuplicateTraceSequence { sequence: 21 })
        );
    }
}
