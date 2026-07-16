use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{Address, AgentId, MemoryRequestId};
use rem6_mmio::MmioRoute;
use rem6_transport::MemoryRouteId;

use crate::{RiscvCore, RiscvCoreState};

mod codec;
#[cfg(test)]
#[path = "riscv_execution_mode_handoff/completed_partial_overlay_tests.rs"]
mod completed_partial_overlay_tests;
#[cfg(test)]
#[path = "riscv_execution_mode_handoff/legacy_payload_fixtures.rs"]
mod legacy_payload_fixtures;
#[path = "riscv_execution_mode_handoff/partial_overlay.rs"]
mod partial_overlay;

#[cfg(test)]
use codec::{
    HEADER_BYTES, MAGIC, V1_ENTRY_BYTES, VERSION_CURRENT, VERSION_FORWARDING,
    VERSION_MULTI_SOURCE_CURRENT, VERSION_PARTIAL_OVERLAY, VERSION_SINGLE_SOURCE_CURRENT,
    VERSION_TYPED_TARGET,
};
#[cfg(test)]
use legacy_payload_fixtures::{
    LEGACY_V2_TYPED_TARGET_PAYLOAD, LEGACY_V3_FORWARDED_PAYLOAD, LEGACY_V4_PARTIAL_OVERLAY_PAYLOAD,
    LEGACY_V5_SINGLE_SOURCE_PARTIAL_OVERLAY_PAYLOAD,
};

pub(crate) use partial_overlay::RiscvPendingPartialScalarLoadHandoff;
use partial_overlay::{
    completed_partial_overlay_is_valid, compose_completed_partial_overlay_sources,
    compose_partial_overlay_sources, partial_overlay_mask, scalar_byte_mask,
    validate_partial_overlay_data, validate_partial_overlay_payload,
};
pub use partial_overlay::{
    RiscvO3LiveDataHandoffCompletedPartialOverlay, RiscvO3LiveDataHandoffPartialOverlay,
    RiscvO3LiveDataHandoffPartialOverlaySource,
};

pub const RISCV_O3_LIVE_DATA_HANDOFF_CHUNK: &str = "o3-live-data-handoff";

const MAX_ROWS: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveDataHandoffCapture {
    NoLiveDataAuthority,
    Captured(RiscvO3LiveDataHandoff),
    Rejected,
}

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
pub enum RiscvO3LiveDataHandoffOwnership {
    Transport,
    BufferedStore { predecessor: MemoryRequestId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffEntry {
    fetch_request: MemoryRequestId,
    data_request: MemoryRequestId,
    issue_tick: Tick,
    partition: PartitionId,
    operation: RiscvO3LiveDataHandoffOperation,
    ownership: RiscvO3LiveDataHandoffOwnership,
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

    pub const fn ownership(self) -> RiscvO3LiveDataHandoffOwnership {
        self.ownership
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
    partial_overlays: Vec<RiscvO3LiveDataHandoffPartialOverlay>,
    completed_partial_overlays: Vec<RiscvO3LiveDataHandoffCompletedPartialOverlay>,
    younger_rows: u32,
}

impl RiscvO3LiveDataHandoff {
    fn new(entries: Vec<RiscvO3LiveDataHandoffEntry>, younger_rows: usize) -> Option<Self> {
        let row_count = entries.len().checked_add(younger_rows)?;
        (!entries.is_empty()
            && entries
                .iter()
                .all(|entry| entry.operation == RiscvO3LiveDataHandoffOperation::Load)
            && row_count <= MAX_ROWS)
            .then_some(Self {
                entries,
                forwarded_rows: Vec::new(),
                partial_overlays: Vec::new(),
                completed_partial_overlays: Vec::new(),
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
            partial_overlays: Vec::new(),
            completed_partial_overlays: Vec::new(),
            younger_rows: u32::try_from(younger_rows).ok()?,
        })
    }

    fn with_partial_overlay(
        entries: Vec<RiscvO3LiveDataHandoffEntry>,
        overlay: RiscvO3LiveDataHandoffPartialOverlay,
        younger_rows: usize,
    ) -> Option<Self> {
        let load = entries.last()?;
        if !(2..=MAX_ROWS).contains(&entries.len())
            || younger_rows != 0
            || load.operation != RiscvO3LiveDataHandoffOperation::Load
            || load.ownership != RiscvO3LiveDataHandoffOwnership::Transport
            || load.data_request != overlay.load_data_request
            || overlay.sources.len() != entries.len() - 1
            || !entries[..entries.len() - 1]
                .iter()
                .enumerate()
                .zip(&overlay.sources)
                .all(|((index, entry), source)| {
                    entry.operation == RiscvO3LiveDataHandoffOperation::Store
                        && entry.data_request == source.source_data_request
                        && match entry.ownership {
                            RiscvO3LiveDataHandoffOwnership::Transport => true,
                            RiscvO3LiveDataHandoffOwnership::BufferedStore { predecessor } => {
                                index > 0 && entries[index - 1].data_request == predecessor
                            }
                        }
                })
        {
            return None;
        }
        Some(Self {
            entries,
            forwarded_rows: Vec::new(),
            partial_overlays: vec![overlay],
            completed_partial_overlays: Vec::new(),
            younger_rows: 0,
        })
    }

    fn with_completed_partial_overlay(
        entries: Vec<RiscvO3LiveDataHandoffEntry>,
        overlay: RiscvO3LiveDataHandoffCompletedPartialOverlay,
        younger_rows: usize,
    ) -> Option<Self> {
        if younger_rows != 0 || !completed_partial_overlay_is_valid(&entries, &overlay) {
            return None;
        }
        Some(Self {
            entries,
            forwarded_rows: Vec::new(),
            partial_overlays: Vec::new(),
            completed_partial_overlays: vec![overlay],
            younger_rows: 0,
        })
    }

    pub fn entries(&self) -> &[RiscvO3LiveDataHandoffEntry] {
        &self.entries
    }

    pub fn forwarded_rows(&self) -> &[RiscvO3LiveDataHandoffForwardedRow] {
        &self.forwarded_rows
    }

    pub fn partial_overlays(&self) -> &[RiscvO3LiveDataHandoffPartialOverlay] {
        &self.partial_overlays
    }

    pub fn completed_partial_overlays(&self) -> &[RiscvO3LiveDataHandoffCompletedPartialOverlay] {
        &self.completed_partial_overlays
    }

    pub fn resident_rows(&self) -> usize {
        self.entries.len() + self.forwarded_rows.len() + self.completed_partial_overlays.len()
    }

    pub const fn younger_rows(&self) -> u32 {
        self.younger_rows
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
    InvalidOwnershipKind {
        value: u8,
    },
    NonZeroTransportOwnershipPadding,
    InvalidBufferedStorePredecessor {
        request: MemoryRequestId,
    },
    InvalidStoreTarget,
    InvalidCurrentShape {
        entries: usize,
        forwarded_rows: usize,
        partial_overlays: usize,
        completed_partial_overlays: usize,
        younger_rows: u32,
    },
    InvalidForwardingShape {
        entries: usize,
        forwarded_rows: usize,
        younger_rows: u32,
    },
    InvalidPartialOverlayShape {
        entries: usize,
        forwarded_rows: usize,
        partial_overlays: usize,
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
    InvalidPartialOverlayLoad {
        request: MemoryRequestId,
    },
    InvalidPartialOverlaySourceCount {
        sources: usize,
        expected: usize,
    },
    DuplicatePartialOverlaySource {
        request: MemoryRequestId,
    },
    InvalidPartialOverlayMask {
        mask: u8,
        bytes: u32,
    },
    InvalidCompletedPartialOverlayLiveMask {
        original: u8,
        live: u8,
    },
    InvalidCompletedPartialOverlaySequence {
        source: u64,
        load: u64,
    },
    PartialOverlayMaskMismatch {
        expected: u8,
        actual: u8,
    },
    InvalidPartialOverlayData {
        index: usize,
    },
    InvalidPartialOverlaySourceData {
        index: usize,
    },
    OverlappingPartialOverlayOwnership {
        mask: u8,
    },
    IncompletePartialOverlayOwnership {
        expected: u8,
        actual: u8,
    },
    NonZeroPartialOverlaySourceDataPadding {
        index: usize,
        value: u8,
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
            Self::InvalidOwnershipKind { value } => {
                write!(formatter, "live-data handoff ownership kind {value} is invalid")
            }
            Self::NonZeroTransportOwnershipPadding => write!(
                formatter,
                "live-data handoff transport ownership has nonzero predecessor padding"
            ),
            Self::InvalidBufferedStorePredecessor { request } => write!(
                formatter,
                "live-data handoff buffered store predecessor {}:{} is invalid",
                request.agent().get(),
                request.sequence()
            ),
            Self::InvalidStoreTarget => {
                write!(formatter, "live-data handoff store target must be memory")
            }
            Self::InvalidCurrentShape {
                entries,
                forwarded_rows,
                partial_overlays,
                completed_partial_overlays,
                younger_rows,
            } => write!(
                formatter,
                "current live-data handoff shape has {entries} transport entries, {forwarded_rows} forwarded rows, {partial_overlays} pending overlay rows, {completed_partial_overlays} completed overlay rows, and {younger_rows} younger rows"
            ),
            Self::InvalidForwardingShape {
                entries,
                forwarded_rows,
                younger_rows,
            } => write!(
                formatter,
                "live-data handoff forwarding shape has {entries} transport entries, {forwarded_rows} forwarded rows, and {younger_rows} younger rows"
            ),
            Self::InvalidPartialOverlayShape {
                entries,
                forwarded_rows,
                partial_overlays,
                younger_rows,
            } => write!(
                formatter,
                "live-data handoff partial-overlay shape has {entries} transport entries, {forwarded_rows} forwarded rows, {partial_overlays} overlay rows, and {younger_rows} younger rows"
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
            Self::InvalidPartialOverlayLoad { request } => write!(
                formatter,
                "live-data handoff partial-overlay load {}:{} is invalid",
                request.agent().get(),
                request.sequence()
            ),
            Self::InvalidPartialOverlaySourceCount { sources, expected } => write!(
                formatter,
                "live-data handoff partial overlay has {sources} sources; expected {expected}"
            ),
            Self::DuplicatePartialOverlaySource { request } => write!(
                formatter,
                "live-data handoff repeats partial-overlay source {}:{}",
                request.agent().get(),
                request.sequence()
            ),
            Self::InvalidPartialOverlayMask { mask, bytes } => write!(
                formatter,
                "live-data handoff partial-overlay mask {mask:#04x} is invalid for {bytes} bytes"
            ),
            Self::InvalidCompletedPartialOverlayLiveMask { original, live } => write!(
                formatter,
                "live-data handoff completed partial-overlay live mask {live:#04x} is not a nonzero partial subset of original mask {original:#04x}"
            ),
            Self::InvalidCompletedPartialOverlaySequence { source, load } => write!(
                formatter,
                "live-data handoff completed partial-overlay load sequence {load} does not follow source sequence {source}"
            ),
            Self::PartialOverlayMaskMismatch { expected, actual } => write!(
                formatter,
                "live-data handoff partial-overlay mask {actual:#04x} does not match source overlap {expected:#04x}"
            ),
            Self::InvalidPartialOverlayData { index } => write!(
                formatter,
                "live-data handoff partial-overlay data byte {index} does not match its source ownership"
            ),
            Self::InvalidPartialOverlaySourceData { index } => write!(
                formatter,
                "live-data handoff partial-overlay source data byte {index} is not owned by the overlay"
            ),
            Self::OverlappingPartialOverlayOwnership { mask } => write!(
                formatter,
                "live-data handoff partial-overlay sources both own mask {mask:#04x}"
            ),
            Self::IncompletePartialOverlayOwnership { expected, actual } => write!(
                formatter,
                "live-data handoff partial-overlay sources own mask {actual:#04x}; expected {expected:#04x}"
            ),
            Self::NonZeroPartialOverlaySourceDataPadding { index, value } => write!(
                formatter,
                "live-data handoff partial-overlay source padding byte {index} has value {value}"
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
    pub(crate) partial_overlay: Option<RiscvPendingPartialScalarLoadHandoff>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvCompletedPartialScalarLoadHandoff {
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) data_request: MemoryRequestId,
    pub(crate) issue_tick: Tick,
    pub(crate) response_tick: Tick,
    pub(crate) address: Address,
    pub(crate) bytes: u32,
    pub(crate) original_forwarded_mask: u8,
    pub(crate) data: [u8; 8],
    pub(crate) o3_sequence: u64,
    pub(crate) trace_sequence: Option<u64>,
}

fn build_completed_partial_overlay_handoff(
    entries: Vec<RiscvO3LiveDataHandoffEntry>,
    issued_rows: &[RiscvIssuedScalarMemoryHandoff],
    forwarded_rows: &[RiscvForwardedScalarLoadHandoff],
    completed_partial_rows: &[RiscvCompletedPartialScalarLoadHandoff],
    younger_rows: usize,
) -> Option<RiscvO3LiveDataHandoff> {
    if completed_partial_rows.len() != 1
        || !forwarded_rows.is_empty()
        || issued_rows
            .iter()
            .any(|issued| issued.partial_overlay.is_some())
        || entries.is_empty()
        || entries.len() >= MAX_ROWS
        || younger_rows != 0
    {
        return None;
    }
    let completed = completed_partial_rows[0];
    let first_source = issued_rows
        .iter()
        .find(|issued| issued.data_request == entries[0].data_request)
        .copied()?;
    let sources = entries
        .iter()
        .map(|entry| {
            let source = issued_rows
                .iter()
                .find(|issued| issued.data_request == entry.data_request)
                .copied()?;
            (source.operation == RiscvO3LiveDataHandoffOperation::Store
                && matches!(source.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                && source.partition == first_source.partition
                && source.target == first_source.target
                && source.partial_overlay.is_none())
            .then_some(source)
        })
        .collect::<Option<Vec<_>>>()?;
    let (live_forwarded_mask, sources) = compose_completed_partial_overlay_sources(
        &sources,
        completed.address,
        completed.bytes,
        completed.original_forwarded_mask,
        &completed.data,
    )?;
    RiscvO3LiveDataHandoff::with_completed_partial_overlay(
        entries,
        RiscvO3LiveDataHandoffCompletedPartialOverlay {
            fetch_request: completed.fetch_request,
            load_data_request: completed.data_request,
            issue_tick: completed.issue_tick,
            response_tick: completed.response_tick,
            address: completed.address,
            bytes: completed.bytes,
            original_forwarded_mask: completed.original_forwarded_mask,
            live_forwarded_mask,
            data: completed.data,
            o3_sequence: completed.o3_sequence,
            trace_sequence: completed.trace_sequence,
            sources,
        },
        younger_rows,
    )
}

impl RiscvCore {
    pub fn capture_o3_live_data_handoff(&self) -> Option<RiscvO3LiveDataHandoff> {
        match self.capture_o3_live_data_handoff_status() {
            RiscvO3LiveDataHandoffCapture::Captured(handoff) => Some(handoff),
            RiscvO3LiveDataHandoffCapture::NoLiveDataAuthority
            | RiscvO3LiveDataHandoffCapture::Rejected => None,
        }
    }

    pub fn capture_o3_live_data_handoff_status(&self) -> RiscvO3LiveDataHandoffCapture {
        let state = self.state.lock().expect("riscv core lock");
        if !Self::has_o3_live_data_authority(&state) {
            return RiscvO3LiveDataHandoffCapture::NoLiveDataAuthority;
        }
        Self::capture_o3_live_data_handoff_from_state(&state)
            .map(RiscvO3LiveDataHandoffCapture::Captured)
            .unwrap_or(RiscvO3LiveDataHandoffCapture::Rejected)
    }

    fn has_o3_live_data_authority(state: &RiscvCoreState) -> bool {
        !state.o3_runtime.live_data_access_lifecycle_is_quiescent()
            || !state.outstanding_data.is_empty()
            || !state.buffered_o3_stores.is_empty()
            || !state.pending_data_translations.is_empty()
            || !state.ready_translated_data.is_empty()
            || state
                .data_translation
                .as_ref()
                .is_some_and(|frontend| !frontend.is_empty())
            || state.events.iter().any(|event| {
                event.execution().memory_access().is_some()
                    && !state
                        .issued_data_for_fetches
                        .contains(&event.fetch().request_id())
            })
    }

    fn capture_o3_live_data_handoff_from_state(
        state: &RiscvCoreState,
    ) -> Option<RiscvO3LiveDataHandoff> {
        if state
            .data_translation
            .as_ref()
            .is_some_and(|frontend| !frontend.is_empty())
            || !state.pending_data_translations.is_empty()
            || !state.ready_translated_data.is_empty()
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
        let (resident_rows, forwarded_rows, completed_partial_rows, younger_rows) =
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
            let ownership = match state.buffered_o3_stores.get(&issued.data_request) {
                Some(buffered) => {
                    let (buffered_issue, predecessor) = buffered.scalar_memory_handoff()?;
                    if buffered_issue != issued {
                        return None;
                    }
                    RiscvO3LiveDataHandoffOwnership::BufferedStore { predecessor }
                }
                None => RiscvO3LiveDataHandoffOwnership::Transport,
            };
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
                ownership,
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
        if !completed_partial_rows.is_empty() {
            return build_completed_partial_overlay_handoff(
                entries,
                &issued_rows,
                &forwarded_rows,
                &completed_partial_rows,
                younger_rows,
            );
        }
        let partial_rows = issued_rows
            .iter()
            .filter_map(|issued| issued.partial_overlay.map(|overlay| (*issued, overlay)))
            .collect::<Vec<_>>();
        if !partial_rows.is_empty() {
            if partial_rows.len() != 1
                || !forwarded_rows.is_empty()
                || !(2..=MAX_ROWS).contains(&entries.len())
                || younger_rows != 0
            {
                return None;
            }
            let (load, overlay) = partial_rows[0];
            if load.operation != RiscvO3LiveDataHandoffOperation::Load
                || !matches!(load.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                || load.address != overlay.address
                || load.bytes != overlay.bytes
            {
                return None;
            }
            let load_entry = entries
                .iter()
                .find(|entry| entry.data_request == load.data_request)?;
            if entries.last() != Some(load_entry) {
                return None;
            }
            let sources = entries[..entries.len() - 1]
                .iter()
                .map(|entry| {
                    let source = issued_rows
                        .iter()
                        .find(|issued| issued.data_request == entry.data_request)
                        .copied()?;
                    (source.operation == RiscvO3LiveDataHandoffOperation::Store
                        && source.partition == load.partition
                        && matches!(source.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                        && source.target == load.target
                        && source.partial_overlay.is_none()
                        && entry.o3_sequence < load_entry.o3_sequence)
                        .then_some(source)
                })
                .collect::<Option<Vec<_>>>()?;
            let sources = compose_partial_overlay_sources(&sources, overlay)?;
            return RiscvO3LiveDataHandoff::with_partial_overlay(
                entries,
                RiscvO3LiveDataHandoffPartialOverlay {
                    load_data_request: load.data_request,
                    address: overlay.address,
                    bytes: overlay.bytes,
                    forwarded_mask: overlay.forwarded_mask,
                    data: overlay.data,
                    sources,
                },
                younger_rows,
            );
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    const CURRENT_HEADER_BYTES: usize = HEADER_BYTES + 12;
    const CURRENT_MEMORY_ENTRY_BYTES: usize = V1_ENTRY_BYTES + 2 + 13;
    const CURRENT_ISSUE_TICK_OFFSET: usize = 24;
    const CURRENT_TARGET_KIND_OFFSET: usize = 50;
    const CURRENT_MMIO_REQUEST_LATENCY_OFFSET: usize = 55;
    const CURRENT_ADDRESS_OFFSET: usize = 59;
    const CURRENT_BYTES_OFFSET: usize = 67;
    const CURRENT_O3_SEQUENCE_OFFSET: usize = 71;
    const CURRENT_TRACE_SEQUENCE_OFFSET: usize = 80;

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
            ownership: RiscvO3LiveDataHandoffOwnership::Transport,
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

    fn partial_overlay(
        load: RiscvO3LiveDataHandoffEntry,
        source: MemoryRequestId,
    ) -> RiscvO3LiveDataHandoffPartialOverlay {
        RiscvO3LiveDataHandoffPartialOverlay {
            load_data_request: load.data_request,
            address: load.address,
            bytes: load.bytes,
            forwarded_mask: 0b0010,
            data: [0, 0x5a, 0, 0, 0, 0, 0, 0],
            sources: vec![RiscvO3LiveDataHandoffPartialOverlaySource {
                source_data_request: source,
                source_address: Address::new(0x8001),
                source_bytes: 1,
                ownership_mask: 0b0010,
                source_data: [0x5a, 0, 0, 0, 0, 0, 0, 0],
            }],
        }
    }

    fn multi_source_partial_overlay_handoff() -> RiscvO3LiveDataHandoff {
        let mut oldest = entry(1);
        oldest.operation = RiscvO3LiveDataHandoffOperation::Store;
        oldest.address = Address::new(0x8000);
        oldest.bytes = 4;
        let mut middle = entry(2);
        middle.operation = RiscvO3LiveDataHandoffOperation::Store;
        middle.ownership = RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: oldest.data_request,
        };
        middle.address = Address::new(0x8002);
        middle.bytes = 2;
        let mut youngest = entry(3);
        youngest.operation = RiscvO3LiveDataHandoffOperation::Store;
        youngest.ownership = RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: middle.data_request,
        };
        youngest.address = Address::new(0x8002);
        youngest.bytes = 1;
        let mut load = entry(4);
        load.address = Address::new(0x8000);
        load.bytes = 8;
        let overlay = RiscvO3LiveDataHandoffPartialOverlay {
            load_data_request: load.data_request,
            address: load.address,
            bytes: load.bytes,
            forwarded_mask: 0b0000_1111,
            data: [0xaa, 0, 0xdd, 0x06, 0, 0, 0, 0],
            sources: vec![
                RiscvO3LiveDataHandoffPartialOverlaySource {
                    source_data_request: oldest.data_request,
                    source_address: oldest.address,
                    source_bytes: oldest.bytes,
                    ownership_mask: 0b0000_0011,
                    source_data: [0xaa, 0, 0, 0, 0, 0, 0, 0],
                },
                RiscvO3LiveDataHandoffPartialOverlaySource {
                    source_data_request: middle.data_request,
                    source_address: middle.address,
                    source_bytes: middle.bytes,
                    ownership_mask: 0b0000_1000,
                    source_data: [0, 0x06, 0, 0, 0, 0, 0, 0],
                },
                RiscvO3LiveDataHandoffPartialOverlaySource {
                    source_data_request: youngest.data_request,
                    source_address: youngest.address,
                    source_bytes: youngest.bytes,
                    ownership_mask: 0b0000_0100,
                    source_data: [0xdd, 0, 0, 0, 0, 0, 0, 0],
                },
            ],
        };
        RiscvO3LiveDataHandoff::with_partial_overlay(
            vec![oldest, middle, youngest, load],
            overlay,
            0,
        )
        .unwrap()
    }

    #[test]
    fn current_live_data_handoff_writer_uses_one_latest_typed_schema() {
        let plain = RiscvO3LiveDataHandoff::new(vec![entry(1)], 0).unwrap();
        let mmio = RiscvO3LiveDataHandoff::new(vec![mmio_entry(1)], 0).unwrap();

        let mut forwarding_store = entry(1);
        forwarding_store.operation = RiscvO3LiveDataHandoffOperation::Store;
        forwarding_store.address = Address::new(0x8004);
        let forwarded = RiscvO3LiveDataHandoff::with_forwarded_rows(
            vec![forwarding_store],
            vec![forwarded_row(2, forwarding_store.data_request)],
            0,
        )
        .unwrap();

        let mut overlay_store = entry(1);
        overlay_store.operation = RiscvO3LiveDataHandoffOperation::Store;
        overlay_store.address = Address::new(0x8001);
        overlay_store.bytes = 1;
        let mut overlay_load = entry(2);
        overlay_load.address = Address::new(0x8000);
        let pending_overlay = RiscvO3LiveDataHandoff::with_partial_overlay(
            vec![overlay_store, overlay_load],
            partial_overlay(overlay_load, overlay_store.data_request),
            0,
        )
        .unwrap();

        for handoff in [plain, mmio, forwarded, pending_overlay] {
            let payload = handoff.encode();
            assert_eq!(payload[MAGIC.len()], VERSION_CURRENT);
            assert_eq!(RiscvO3LiveDataHandoff::decode(&payload), Ok(handoff));
        }
    }

    #[test]
    fn live_data_handoff_round_trips_entries() {
        let handoff = RiscvO3LiveDataHandoff::new(vec![entry(1), entry(2)], 2).unwrap();
        let payload = handoff.encode();

        assert_eq!(payload[MAGIC.len()], VERSION_CURRENT);
        assert_eq!(
            payload.len(),
            CURRENT_HEADER_BYTES + 2 * CURRENT_MEMORY_ENTRY_BYTES
        );
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
    fn live_data_handoff_decodes_legacy_v1_and_reencodes_current_bytes() {
        let (decoded, version) =
            RiscvO3LiveDataHandoff::decode_with_version(&LEGACY_V1_SINGLE_ENTRY_PAYLOAD).unwrap();

        assert_eq!(version, 1);
        assert_eq!(
            decoded,
            RiscvO3LiveDataHandoff::new(vec![entry(1)], 0).unwrap()
        );
        assert_eq!(
            decoded.entries()[0].memory_route(),
            Some(MemoryRouteId::new(7))
        );
        let current = decoded.encode();
        assert_eq!(current[MAGIC.len()], VERSION_CURRENT);
        assert_ne!(current, LEGACY_V1_SINGLE_ENTRY_PAYLOAD);
        assert_eq!(
            RiscvO3LiveDataHandoff::decode_with_version(&current),
            Ok((decoded, VERSION_CURRENT))
        );
    }

    #[test]
    fn live_data_handoff_decodes_legacy_v2_through_v5_bytes() {
        let typed = RiscvO3LiveDataHandoff::new(vec![entry(1), mmio_entry(2)], 1).unwrap();

        let mut store = entry(1);
        store.operation = RiscvO3LiveDataHandoffOperation::Store;
        store.address = Address::new(0x8004);
        let forwarded = RiscvO3LiveDataHandoff::with_forwarded_rows(
            vec![store],
            vec![forwarded_row(2, store.data_request)],
            0,
        )
        .unwrap();

        let mut overlay_store = entry(1);
        overlay_store.operation = RiscvO3LiveDataHandoffOperation::Store;
        overlay_store.address = Address::new(0x8001);
        overlay_store.bytes = 1;
        let mut overlay_load = entry(2);
        overlay_load.address = Address::new(0x8000);
        let overlay = RiscvO3LiveDataHandoff::with_partial_overlay(
            vec![overlay_store, overlay_load],
            partial_overlay(overlay_load, overlay_store.data_request),
            0,
        )
        .unwrap();

        for (payload, handoff, version) in [
            (
                LEGACY_V2_TYPED_TARGET_PAYLOAD,
                typed.clone(),
                VERSION_TYPED_TARGET,
            ),
            (
                LEGACY_V3_FORWARDED_PAYLOAD,
                forwarded.clone(),
                VERSION_FORWARDING,
            ),
            (
                LEGACY_V4_PARTIAL_OVERLAY_PAYLOAD,
                overlay.clone(),
                VERSION_PARTIAL_OVERLAY,
            ),
            (
                LEGACY_V5_SINGLE_SOURCE_PARTIAL_OVERLAY_PAYLOAD,
                overlay.clone(),
                VERSION_SINGLE_SOURCE_CURRENT,
            ),
        ] {
            assert_eq!(
                RiscvO3LiveDataHandoff::decode_with_version(payload),
                Ok((handoff.clone(), version))
            );
            assert_eq!(handoff.encode_legacy_for_test(version), payload);
        }

        for (handoff, version) in [
            (typed.clone(), VERSION_TYPED_TARGET),
            (forwarded.clone(), VERSION_FORWARDING),
            (overlay.clone(), VERSION_PARTIAL_OVERLAY),
            (typed, VERSION_SINGLE_SOURCE_CURRENT),
            (forwarded, VERSION_SINGLE_SOURCE_CURRENT),
            (overlay, VERSION_SINGLE_SOURCE_CURRENT),
        ] {
            let payload = handoff.encode_legacy_for_test(version);
            assert_eq!(
                RiscvO3LiveDataHandoff::decode_with_version(&payload),
                Ok((handoff.clone(), version))
            );
            assert_eq!(handoff.encode()[MAGIC.len()], VERSION_CURRENT);
        }
    }

    #[test]
    fn live_data_handoff_round_trips_typed_memory_and_mmio_targets() {
        let memory = entry(1);
        let mmio = mmio_entry(2);
        let handoff = RiscvO3LiveDataHandoff::new(vec![memory, mmio], 1).unwrap();
        let payload = handoff.encode();

        assert_eq!(payload[MAGIC.len()], VERSION_CURRENT);
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

        assert_eq!(payload[MAGIC.len()], VERSION_CURRENT);
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
    fn live_data_handoff_round_trips_pending_partial_overlay() {
        let mut store = entry(1);
        store.operation = RiscvO3LiveDataHandoffOperation::Store;
        store.address = Address::new(0x8001);
        store.bytes = 1;
        let mut load = entry(2);
        load.address = Address::new(0x8000);
        let overlay = partial_overlay(load, store.data_request);
        let handoff =
            RiscvO3LiveDataHandoff::with_partial_overlay(vec![store, load], overlay.clone(), 0)
                .unwrap();
        let payload = handoff.encode();

        assert_eq!(payload[MAGIC.len()], VERSION_CURRENT);
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&payload),
            Ok(handoff.clone())
        );
        assert_eq!(handoff.partial_overlays(), std::slice::from_ref(&overlay));
        assert_eq!(handoff.resident_rows(), 2);
        assert_eq!(overlay.forwarded_mask(), 0b0010);
        assert_eq!(overlay.forwarded_bytes(), 1);
        assert_eq!(overlay.data(), &[0, 0x5a, 0, 0]);
        assert_eq!(overlay.source_address(), Address::new(0x8001));
        assert_eq!(overlay.source_bytes(), 1);
        assert_eq!(overlay.source_data(), &[0x5a]);
    }

    #[test]
    fn live_data_handoff_round_trips_multi_source_partial_overlay() {
        let handoff = multi_source_partial_overlay_handoff();
        let payload = handoff.encode();

        assert_eq!(payload[MAGIC.len()], VERSION_CURRENT);
        assert_eq!(
            RiscvO3LiveDataHandoff::decode_with_version(&payload),
            Ok((handoff.clone(), VERSION_CURRENT))
        );
        assert_eq!(handoff.resident_rows(), 4);
        assert_eq!(
            handoff.entries()[1].ownership(),
            RiscvO3LiveDataHandoffOwnership::BufferedStore {
                predecessor: handoff.entries()[0].data_request(),
            }
        );
        let overlay = &handoff.partial_overlays()[0];
        assert_eq!(overlay.forwarded_mask(), 0b0000_1111);
        assert_eq!(overlay.response_owned_mask(), 0b1111_0000);
        assert_eq!(
            overlay
                .sources()
                .iter()
                .map(|source| source.ownership_mask())
                .collect::<Vec<_>>(),
            vec![0b0000_0011, 0b0000_1000, 0b0000_0100]
        );
    }

    #[test]
    fn live_data_handoff_round_trips_fully_overwritten_source() {
        let mut handoff = multi_source_partial_overlay_handoff();
        handoff.entries[2].bytes = 2;
        handoff.partial_overlays[0].sources[1].ownership_mask = 0;
        handoff.partial_overlays[0].sources[1].source_data = [0; 8];
        handoff.partial_overlays[0].sources[2].source_bytes = 2;
        handoff.partial_overlays[0].sources[2].ownership_mask = 0b0000_1100;
        handoff.partial_overlays[0].sources[2].source_data = [0xdd, 0x06, 0, 0, 0, 0, 0, 0];

        let payload = handoff.encode();
        assert_eq!(
            RiscvO3LiveDataHandoff::decode_with_version(&payload),
            Ok((handoff, VERSION_CURRENT))
        );
    }

    #[test]
    fn live_data_handoff_rejects_overlapping_or_incomplete_source_ownership() {
        let mut overlapping = multi_source_partial_overlay_handoff();
        overlapping.partial_overlays[0].sources[0].ownership_mask = 0b0000_1011;
        overlapping.partial_overlays[0].sources[0].source_data[3] = 0x06;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&overlapping.encode()),
            Err(
                RiscvO3LiveDataHandoffError::OverlappingPartialOverlayOwnership {
                    mask: 0b0000_1000,
                }
            )
        );

        let mut incomplete = multi_source_partial_overlay_handoff();
        incomplete.partial_overlays[0].sources[2].ownership_mask = 0;
        incomplete.partial_overlays[0].sources[2].source_data = [0; 8];
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&incomplete.encode()),
            Err(
                RiscvO3LiveDataHandoffError::IncompletePartialOverlayOwnership {
                    expected: 0b0000_1111,
                    actual: 0b0000_1011,
                }
            )
        );
    }

    #[test]
    fn live_data_handoff_rejects_invalid_partial_overlay_provenance() {
        let mut store = entry(1);
        store.operation = RiscvO3LiveDataHandoffOperation::Store;
        store.address = Address::new(0x8001);
        store.bytes = 1;
        let mut load = entry(2);
        load.address = Address::new(0x8000);
        let unknown = MemoryRequestId::new(AgentId::new(9), 99);
        let invalid_source = RiscvO3LiveDataHandoff {
            entries: vec![store, load],
            forwarded_rows: Vec::new(),
            partial_overlays: vec![partial_overlay(load, unknown)],
            completed_partial_overlays: Vec::new(),
            younger_rows: 0,
        };
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&invalid_source.encode()),
            Err(RiscvO3LiveDataHandoffError::InvalidForwardingSource { request: unknown })
        );

        let mut full_mask = partial_overlay(load, store.data_request);
        full_mask.forwarded_mask = 0b1111;
        let invalid_mask =
            RiscvO3LiveDataHandoff::with_partial_overlay(vec![store, load], full_mask, 0).unwrap();
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&invalid_mask.encode()),
            Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayMask {
                mask: 0b1111,
                bytes: 4,
            })
        );

        let valid = RiscvO3LiveDataHandoff::with_partial_overlay(
            vec![store, load],
            partial_overlay(load, store.data_request),
            0,
        )
        .unwrap()
        .encode();
        let invalid_shape = RiscvO3LiveDataHandoff {
            entries: vec![store, load],
            forwarded_rows: Vec::new(),
            partial_overlays: Vec::new(),
            completed_partial_overlays: Vec::new(),
            younger_rows: 0,
        }
        .encode();
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&invalid_shape),
            Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: 2,
                forwarded_rows: 0,
                partial_overlays: 0,
                completed_partial_overlays: 0,
                younger_rows: 0,
            })
        );

        let overlay_data_offset =
            CURRENT_HEADER_BYTES + 2 * CURRENT_MEMORY_ENTRY_BYTES + 12 + 8 + 4 + 1;
        let mut invalid_data = valid.clone();
        invalid_data[overlay_data_offset] = 1;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&invalid_data),
            Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayData { index: 0 })
        );

        let mut mismatched_source_data = valid;
        let source_data_offset = overlay_data_offset + 8 + 4 + 12 + 1;
        mismatched_source_data[source_data_offset] = 0x6b;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&mismatched_source_data),
            Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlaySourceData { index: 0 })
        );

        let mut wider_store = entry(1);
        wider_store.operation = RiscvO3LiveDataHandoffOperation::Store;
        wider_store.address = Address::new(0x8001);
        wider_store.bytes = 4;
        let forged_nonoverlap = RiscvO3LiveDataHandoff::with_partial_overlay(
            vec![wider_store, load],
            RiscvO3LiveDataHandoffPartialOverlay {
                load_data_request: load.data_request,
                address: load.address,
                bytes: load.bytes,
                forwarded_mask: 0b1110,
                data: [0, 0x11, 0x22, 0x33, 0, 0, 0, 0],
                sources: vec![RiscvO3LiveDataHandoffPartialOverlaySource {
                    source_data_request: wider_store.data_request,
                    source_address: wider_store.address,
                    source_bytes: wider_store.bytes,
                    ownership_mask: 0b1110,
                    source_data: [0x11, 0x22, 0x33, 0x44, 0, 0, 0, 0],
                }],
            },
            0,
        )
        .unwrap();
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&forged_nonoverlap.encode()),
            Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlaySourceData { index: 3 })
        );
    }

    #[test]
    fn live_data_handoff_rejects_unknown_or_invalid_typed_targets() {
        let payload = RiscvO3LiveDataHandoff::new(vec![mmio_entry(1)], 0)
            .unwrap()
            .encode();
        let mut unknown_kind = payload.clone();
        unknown_kind[CURRENT_HEADER_BYTES + CURRENT_TARGET_KIND_OFFSET] = 7;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&unknown_kind),
            Err(RiscvO3LiveDataHandoffError::InvalidTargetKind { value: 7 })
        );

        let mut zero_request_latency = payload;
        let request_latency = CURRENT_HEADER_BYTES + CURRENT_MMIO_REQUEST_LATENCY_OFFSET;
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
        let issue_offset = CURRENT_HEADER_BYTES + CURRENT_ISSUE_TICK_OFFSET;
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
        bad_version[MAGIC.len()] = VERSION_CURRENT + 1;
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&bad_version),
            Err(RiscvO3LiveDataHandoffError::UnsupportedVersion {
                version: VERSION_CURRENT + 1,
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
    fn live_data_handoff_rejects_current_count_shape_before_body_parse() {
        let mut payload = RiscvO3LiveDataHandoff::new(vec![entry(1)], 0)
            .unwrap()
            .encode();
        let partial_overlay_count_offset = HEADER_BYTES + 4;
        payload[partial_overlay_count_offset..partial_overlay_count_offset + 4]
            .copy_from_slice(&2_u32.to_le_bytes());

        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&payload),
            Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: 1,
                forwarded_rows: 0,
                partial_overlays: 2,
                completed_partial_overlays: 0,
                younger_rows: 0,
            })
        );
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
    fn live_data_handoff_rejects_store_only_current_shape() {
        let mut store = entry(1);
        store.operation = RiscvO3LiveDataHandoffOperation::Store;
        store.address = Address::new(0x8004);
        let payload = RiscvO3LiveDataHandoff {
            entries: vec![store],
            forwarded_rows: Vec::new(),
            partial_overlays: Vec::new(),
            completed_partial_overlays: Vec::new(),
            younger_rows: 0,
        }
        .encode();

        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&payload),
            Err(RiscvO3LiveDataHandoffError::InvalidCurrentShape {
                entries: 1,
                forwarded_rows: 0,
                partial_overlays: 0,
                completed_partial_overlays: 0,
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
        let bytes_offset = CURRENT_HEADER_BYTES + CURRENT_BYTES_OFFSET;
        invalid_width[bytes_offset..bytes_offset + 4].copy_from_slice(&3_u32.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&invalid_width),
            Err(RiscvO3LiveDataHandoffError::InvalidScalarBytes { bytes: 3 })
        );

        let mut overflowing = payload;
        let address_offset = CURRENT_HEADER_BYTES + CURRENT_ADDRESS_OFFSET;
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
            partial_overlays: Vec::new(),
            completed_partial_overlays: Vec::new(),
            younger_rows: 0,
        };
        let payload = handoff.encode();
        let mut duplicate_o3 = payload.clone();
        let second_o3 =
            CURRENT_HEADER_BYTES + CURRENT_MEMORY_ENTRY_BYTES + CURRENT_O3_SEQUENCE_OFFSET;
        duplicate_o3[second_o3..second_o3 + 8].copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&duplicate_o3),
            Err(RiscvO3LiveDataHandoffError::DuplicateO3Sequence { sequence: 1 })
        );

        let mut duplicate_trace = payload;
        let second_trace =
            CURRENT_HEADER_BYTES + CURRENT_MEMORY_ENTRY_BYTES + CURRENT_TRACE_SEQUENCE_OFFSET;
        duplicate_trace[second_trace..second_trace + 8].copy_from_slice(&21_u64.to_le_bytes());
        assert_eq!(
            RiscvO3LiveDataHandoff::decode(&duplicate_trace),
            Err(RiscvO3LiveDataHandoffError::DuplicateTraceSequence { sequence: 21 })
        );
    }
}
