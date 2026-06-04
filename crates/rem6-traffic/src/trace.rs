use std::io::Read;

use flate2::read::GzDecoder;
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
};

use crate::{
    common::{
        checked_counter_add, TrafficGeneratorSummary, TrafficRequestEvent, TrafficRequestKind,
    },
    TrafficGeneratorError,
};

const GEM5_PROTO_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];
const GEM5_READ_REQ: u32 = 1;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_WRITEBACK_DIRTY: u32 = 7;
const GEM5_WRITEBACK_CLEAN: u32 = 8;
const GEM5_WRITE_CLEAN: u32 = 9;
const GEM5_CLEAN_EVICT: u32 = 10;
const GEM5_SOFT_PF_REQ: u32 = 11;
const GEM5_SOFT_PF_EX_REQ: u32 = 12;
const GEM5_HARD_PF_REQ: u32 = 13;
const GEM5_WRITE_LINE_REQ: u32 = 16;
const GEM5_UPGRADE_REQ: u32 = 17;
const GEM5_READ_EX_REQ: u32 = 22;
const GEM5_READ_CLEAN_REQ: u32 = 24;
const GEM5_READ_SHARED_REQ: u32 = 25;
const GEM5_CLEAN_SHARED_REQ: u32 = 42;
const GEM5_CLEAN_INVALID_REQ: u32 = 44;
const GEM5_INVALIDATE_REQ: u32 = 54;
const WIRE_VARINT: u64 = 0;
const WIRE_FIXED64: u64 = 1;
const WIRE_LENGTH_DELIMITED: u64 = 2;
const WIRE_START_GROUP: u64 = 3;
const WIRE_END_GROUP: u64 = 4;
const WIRE_FIXED32: u64 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrafficTraceCommand {
    ReadShared,
    ReadUnique,
    SoftPrefetchRead,
    HardPrefetchRead,
    PrefetchWrite,
    Write,
    WriteLine,
    WritebackDirty,
    WritebackClean,
    WriteClean,
    CleanEvict,
    CleanShared,
    CleanInvalid,
    Invalidate,
    Upgrade,
}

impl TrafficTraceCommand {
    const fn request_kind(self) -> TrafficRequestKind {
        match self {
            Self::ReadShared
            | Self::ReadUnique
            | Self::SoftPrefetchRead
            | Self::HardPrefetchRead
            | Self::PrefetchWrite => TrafficRequestKind::Read,
            Self::Write
            | Self::WriteLine
            | Self::WritebackDirty
            | Self::WritebackClean
            | Self::WriteClean => TrafficRequestKind::Write,
            Self::CleanEvict
            | Self::CleanShared
            | Self::CleanInvalid
            | Self::Invalidate
            | Self::Upgrade => TrafficRequestKind::Maintenance,
        }
    }

    const fn gem5_name(self) -> &'static str {
        match self {
            Self::ReadShared => "ReadReq",
            Self::ReadUnique => "ReadExReq",
            Self::SoftPrefetchRead => "SoftPFReq",
            Self::HardPrefetchRead => "HardPFReq",
            Self::PrefetchWrite => "SoftPFExReq",
            Self::Write => "WriteReq",
            Self::WriteLine => "WriteLineReq",
            Self::WritebackDirty => "WritebackDirty",
            Self::WritebackClean => "WritebackClean",
            Self::WriteClean => "WriteClean",
            Self::CleanEvict => "CleanEvict",
            Self::CleanShared => "CleanSharedReq",
            Self::CleanInvalid => "CleanInvalidReq",
            Self::Invalidate => "InvalidateReq",
            Self::Upgrade => "UpgradeReq",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrafficTraceElement {
    tick: u64,
    command: TrafficTraceCommand,
    address: Address,
    size: AccessSize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTrace {
    tick_frequency: u64,
    elements: Vec<TrafficTraceElement>,
}

impl TrafficTrace {
    pub fn from_gem5_packet_trace(
        bytes: &[u8],
        expected_tick_frequency: u64,
    ) -> Result<Self, TrafficGeneratorError> {
        let decompressed;
        let trace_bytes = if is_gzip_stream(bytes) {
            decompressed = decompress_gzip_trace(bytes)?;
            decompressed.as_slice()
        } else {
            bytes
        };
        let mut stream = Gem5PacketTraceReader::new(trace_bytes)?;
        let header = stream
            .next_message()?
            .ok_or(TrafficGeneratorError::TraceMissingHeader)?;
        let tick_frequency = parse_header(header)?;

        if tick_frequency != expected_tick_frequency {
            return Err(TrafficGeneratorError::TraceTickFrequencyMismatch {
                expected: expected_tick_frequency,
                actual: tick_frequency,
            });
        }

        let mut elements = Vec::new();
        while let Some(message) = stream.next_message()? {
            elements.push(parse_packet(message)?);
        }

        Ok(Self {
            tick_frequency,
            elements,
        })
    }

    pub const fn tick_frequency(&self) -> u64 {
        self.tick_frequency
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTraceConfig {
    agent: AgentId,
    line_layout: CacheLineLayout,
    duration: u64,
    trace: TrafficTrace,
    addr_offset: u64,
    elastic: bool,
}

impl TrafficTraceConfig {
    pub fn new(
        agent: AgentId,
        line_layout: CacheLineLayout,
        duration: u64,
        trace: TrafficTrace,
    ) -> Result<Self, TrafficGeneratorError> {
        Ok(Self {
            agent,
            line_layout,
            duration,
            trace,
            addr_offset: 0,
            elastic: false,
        })
    }

    pub fn with_addr_offset(mut self, addr_offset: u64) -> Result<Self, TrafficGeneratorError> {
        self.addr_offset = addr_offset;
        Ok(self)
    }

    pub fn with_elastic(mut self, elastic: bool) -> Self {
        self.elastic = elastic;
        self
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn duration(&self) -> u64 {
        self.duration
    }

    pub const fn addr_offset(&self) -> u64 {
        self.addr_offset
    }

    pub const fn elastic(&self) -> bool {
        self.elastic
    }

    pub const fn trace(&self) -> &TrafficTrace {
        &self.trace
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTraceSnapshot {
    config: TrafficTraceConfig,
    cursor: usize,
    next_sequence: u64,
    summary: TrafficGeneratorSummary,
    tick_offset: u64,
    active: bool,
}

impl TrafficTraceSnapshot {
    pub fn new(
        config: TrafficTraceConfig,
        cursor: usize,
        next_sequence: u64,
        summary: TrafficGeneratorSummary,
        tick_offset: u64,
        active: bool,
    ) -> Self {
        Self {
            config,
            cursor,
            next_sequence,
            summary,
            tick_offset,
            active,
        }
    }

    pub const fn config(&self) -> &TrafficTraceConfig {
        &self.config
    }

    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    pub const fn tick_offset(&self) -> u64 {
        self.tick_offset
    }

    pub const fn active(&self) -> bool {
        self.active
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceExitStatus {
    completed: bool,
}

impl TrafficTraceExitStatus {
    pub const fn completed() -> Self {
        Self { completed: true }
    }

    pub const fn incomplete() -> Self {
        Self { completed: false }
    }

    pub const fn is_completed(self) -> bool {
        self.completed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTraceGenerator {
    config: TrafficTraceConfig,
    cursor: usize,
    next_sequence: u64,
    summary: TrafficGeneratorSummary,
    tick_offset: u64,
    active: bool,
}

impl TrafficTraceGenerator {
    pub fn new(config: TrafficTraceConfig) -> Self {
        Self {
            config,
            cursor: 0,
            next_sequence: 0,
            summary: TrafficGeneratorSummary::default(),
            tick_offset: 0,
            active: false,
        }
    }

    pub fn restore(snapshot: TrafficTraceSnapshot) -> Result<Self, TrafficGeneratorError> {
        validate_cursor(snapshot.config(), snapshot.cursor())?;

        Ok(Self {
            config: snapshot.config().clone(),
            cursor: snapshot.cursor(),
            next_sequence: snapshot.next_sequence(),
            summary: snapshot.summary(),
            tick_offset: snapshot.tick_offset(),
            active: snapshot.active(),
        })
    }

    pub fn enter(&mut self, tick: u64) {
        self.cursor = 0;
        self.next_sequence = 0;
        self.summary = TrafficGeneratorSummary::default();
        self.tick_offset = tick;
        self.active = true;
    }

    pub fn exit(&mut self) -> TrafficTraceExitStatus {
        let completed = !self.active || self.is_complete();
        self.cursor = 0;
        self.next_sequence = 0;
        self.tick_offset = 0;
        self.active = false;

        if completed {
            TrafficTraceExitStatus::completed()
        } else {
            TrafficTraceExitStatus::incomplete()
        }
    }

    pub fn next_request(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        let Some(element) = self.next_element() else {
            return Ok(None);
        };

        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("next_sequence", sequence, 1)?;
        let (next_tick_offset, event_tick) =
            self.next_packet_tick_from(self.tick_offset, element.tick, tick, retry_delay)?;
        let kind = element.command.request_kind();
        let address = checked_trace_address(element.address, self.config.addr_offset())?;
        let request = self.build_request(sequence, element, kind, address)?;
        let mut next_summary = self.summary;
        next_summary.record(event_tick, kind, element.size.bytes())?;

        self.cursor += 1;
        self.next_sequence = next_sequence;
        self.summary = next_summary;
        self.tick_offset = next_tick_offset;

        Ok(Some(TrafficRequestEvent::new(
            event_tick, sequence, kind, address, request,
        )))
    }

    pub fn schedule_tick(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        let Some(element) = self.next_element() else {
            return Ok(u64::MAX);
        };

        let (_next_tick_offset, event_tick) =
            self.next_packet_tick_from(self.tick_offset, element.tick, tick, retry_delay)?;
        Ok(event_tick)
    }

    pub const fn config(&self) -> &TrafficTraceConfig {
        &self.config
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    pub fn snapshot(&self) -> TrafficTraceSnapshot {
        TrafficTraceSnapshot::new(
            self.config.clone(),
            self.cursor,
            self.next_sequence,
            self.summary,
            self.tick_offset,
            self.active,
        )
    }

    fn next_element(&self) -> Option<TrafficTraceElement> {
        if !self.active {
            return None;
        }

        self.config.trace.elements.get(self.cursor).copied()
    }

    fn is_complete(&self) -> bool {
        self.cursor >= self.config.trace.elements.len()
    }

    fn next_packet_tick_from(
        &self,
        tick_offset: u64,
        element_tick: u64,
        tick: u64,
        retry_delay: u64,
    ) -> Result<(u64, u64), TrafficGeneratorError> {
        let next_tick_offset = if self.config.elastic() {
            checked_tick_add(tick_offset, retry_delay)?
        } else {
            tick_offset
        };
        let scheduled = checked_tick_add(next_tick_offset, element_tick)?;

        Ok((next_tick_offset, scheduled.max(tick)))
    }

    fn build_request(
        &self,
        sequence: u64,
        element: TrafficTraceElement,
        kind: TrafficRequestKind,
        address: Address,
    ) -> Result<MemoryRequest, TrafficGeneratorError> {
        let id = MemoryRequestId::new(self.config.agent(), sequence);
        let layout = self.config.line_layout();

        match kind {
            TrafficRequestKind::Read if element.command == TrafficTraceCommand::ReadUnique => {
                MemoryRequest::read_unique(id, address, element.size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read
                if matches!(
                    element.command,
                    TrafficTraceCommand::SoftPrefetchRead | TrafficTraceCommand::HardPrefetchRead
                ) =>
            {
                MemoryRequest::prefetch_read(id, address, element.size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read if element.command == TrafficTraceCommand::PrefetchWrite => {
                MemoryRequest::prefetch_write(id, address, element.size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read => {
                MemoryRequest::read_shared(id, address, element.size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Write if element.command == TrafficTraceCommand::WriteLine => {
                validate_write_line_request(address, element.size, layout)?;
                build_write_request(self.config.agent(), id, address, element.size, layout)
            }
            TrafficRequestKind::Write
                if matches!(
                    element.command,
                    TrafficTraceCommand::WritebackDirty
                        | TrafficTraceCommand::WritebackClean
                        | TrafficTraceCommand::WriteClean
                ) =>
            {
                validate_writeback_request(element.command, address, element.size, layout)?;
                build_writeback_request(
                    element.command,
                    self.config.agent(),
                    id,
                    address,
                    element.size,
                    layout,
                )
            }
            TrafficRequestKind::Write => {
                build_write_request(self.config.agent(), id, address, element.size, layout)
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::CleanEvict =>
            {
                validate_clean_evict_request(address, element.size, layout)?;
                MemoryRequest::clean_evict(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::CleanShared =>
            {
                validate_clean_maintenance_request(element.command, address, element.size, layout)?;
                MemoryRequest::clean_shared(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::CleanInvalid =>
            {
                validate_clean_maintenance_request(element.command, address, element.size, layout)?;
                MemoryRequest::invalidate(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::Invalidate =>
            {
                validate_invalidate_request(address, element.size, layout)?;
                MemoryRequest::invalidate_writable(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance if element.command == TrafficTraceCommand::Upgrade => {
                validate_upgrade_request(address, element.size, layout)?;
                MemoryRequest::upgrade(id, address, element.size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance => {
                unreachable!("maintenance trace kind has no request builder")
            }
        }
    }
}

fn validate_write_line_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceWriteLineSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceWriteLineUnalignedAddress {
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

fn build_write_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::write(id, address, size, data, mask, layout).map_err(Into::into)
}

fn validate_writeback_request(
    command: TrafficTraceCommand,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceWritebackSizeMismatch {
            command: command.gem5_name(),
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceWritebackUnalignedAddress {
            command: command.gem5_name(),
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

fn build_writeback_request(
    command: TrafficTraceCommand,
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let data_len =
        usize::try_from(size.bytes()).expect("access size fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    match command {
        TrafficTraceCommand::WritebackDirty => {
            MemoryRequest::writeback_dirty(id, address, data, layout).map_err(Into::into)
        }
        TrafficTraceCommand::WritebackClean => {
            MemoryRequest::writeback_clean(id, address, data, layout).map_err(Into::into)
        }
        TrafficTraceCommand::WriteClean => {
            MemoryRequest::write_clean(id, address, data, layout).map_err(Into::into)
        }
        _ => unreachable!("writeback builder is only called for writeback trace commands"),
    }
}

fn validate_clean_evict_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceCleanEvictSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceCleanEvictUnalignedAddress {
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

fn validate_clean_maintenance_request(
    command: TrafficTraceCommand,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceCleanMaintenanceSizeMismatch {
            command: command.gem5_name(),
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(
            TrafficGeneratorError::TraceCleanMaintenanceUnalignedAddress {
                command: command.gem5_name(),
                address,
                line_size: layout.bytes(),
            },
        );
    }
    Ok(())
}

fn validate_upgrade_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceUpgradeSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceUpgradeUnalignedAddress {
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

fn validate_invalidate_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceInvalidateSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceInvalidateUnalignedAddress {
            address,
            line_size: layout.bytes(),
        });
    }
    Ok(())
}

struct Gem5PacketTraceReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Gem5PacketTraceReader<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, TrafficGeneratorError> {
        if bytes.len() < GEM5_PROTO_MAGIC.len() {
            return Err(TrafficGeneratorError::TraceTruncatedMagic {
                length: bytes.len(),
            });
        }

        let actual = [bytes[0], bytes[1], bytes[2], bytes[3]];
        if actual != GEM5_PROTO_MAGIC {
            return Err(TrafficGeneratorError::TraceBadMagic { actual });
        }

        Ok(Self {
            bytes,
            offset: GEM5_PROTO_MAGIC.len(),
        })
    }

    fn next_message(&mut self) -> Result<Option<&'a [u8]>, TrafficGeneratorError> {
        if self.offset == self.bytes.len() {
            return Ok(None);
        }

        let length_offset = self.offset;
        let length = read_varint_u32(self.bytes, &mut self.offset)?;
        let length = usize::try_from(length).expect("u32 message length fits usize");
        let remaining = self.bytes.len() - self.offset;
        if length > remaining {
            return Err(TrafficGeneratorError::TraceTruncatedMessage {
                offset: length_offset,
                length,
                remaining,
            });
        }

        let start = self.offset;
        self.offset += length;
        Ok(Some(&self.bytes[start..self.offset]))
    }
}

fn is_gzip_stream(bytes: &[u8]) -> bool {
    bytes.starts_with(&GZIP_MAGIC)
}

fn decompress_gzip_trace(bytes: &[u8]) -> Result<Vec<u8>, TrafficGeneratorError> {
    let mut decoder = GzDecoder::new(bytes);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).map_err(|error| {
        TrafficGeneratorError::TraceGzipDecode {
            message: error.to_string(),
        }
    })?;
    Ok(decompressed)
}

fn parse_header(message: &[u8]) -> Result<u64, TrafficGeneratorError> {
    let mut parser = ProtoMessageParser::new(message);
    let mut tick_frequency = None;

    while let Some(field) = parser.next_field()? {
        if field.number == 3 {
            tick_frequency = Some(field.varint("PacketHeader", "tick_freq")?);
        }
        parser.skip(field)?;
    }

    tick_frequency.ok_or(TrafficGeneratorError::TraceMissingField {
        message: "PacketHeader",
        field: "tick_freq",
    })
}

fn parse_packet(message: &[u8]) -> Result<TrafficTraceElement, TrafficGeneratorError> {
    let mut parser = ProtoMessageParser::new(message);
    let mut tick = None;
    let mut command = None;
    let mut address = None;
    let mut size = None;
    let mut flags = 0;

    while let Some(field) = parser.next_field()? {
        match field.number {
            1 => tick = Some(field.varint("Packet", "tick")?),
            2 => command = Some(read_u32_field(field, "Packet", "cmd")?),
            3 => address = Some(field.varint("Packet", "addr")?),
            4 => size = Some(read_u32_field(field, "Packet", "size")?),
            5 => flags = read_u32_field(field, "Packet", "flags")?,
            _ => {}
        }
        parser.skip(field)?;
    }

    if flags != 0 {
        return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags });
    }

    let tick = tick.ok_or(TrafficGeneratorError::TraceMissingField {
        message: "Packet",
        field: "tick",
    })?;
    let command = match command.ok_or(TrafficGeneratorError::TraceMissingField {
        message: "Packet",
        field: "cmd",
    })? {
        GEM5_READ_REQ | GEM5_READ_CLEAN_REQ | GEM5_READ_SHARED_REQ => {
            TrafficTraceCommand::ReadShared
        }
        GEM5_READ_EX_REQ => TrafficTraceCommand::ReadUnique,
        GEM5_SOFT_PF_REQ => TrafficTraceCommand::SoftPrefetchRead,
        GEM5_HARD_PF_REQ => TrafficTraceCommand::HardPrefetchRead,
        GEM5_SOFT_PF_EX_REQ => TrafficTraceCommand::PrefetchWrite,
        GEM5_WRITE_REQ => TrafficTraceCommand::Write,
        GEM5_WRITEBACK_DIRTY => TrafficTraceCommand::WritebackDirty,
        GEM5_WRITEBACK_CLEAN => TrafficTraceCommand::WritebackClean,
        GEM5_WRITE_CLEAN => TrafficTraceCommand::WriteClean,
        GEM5_CLEAN_EVICT => TrafficTraceCommand::CleanEvict,
        GEM5_WRITE_LINE_REQ => TrafficTraceCommand::WriteLine,
        GEM5_UPGRADE_REQ => TrafficTraceCommand::Upgrade,
        GEM5_CLEAN_SHARED_REQ => TrafficTraceCommand::CleanShared,
        GEM5_CLEAN_INVALID_REQ => TrafficTraceCommand::CleanInvalid,
        GEM5_INVALIDATE_REQ => TrafficTraceCommand::Invalidate,
        command => return Err(TrafficGeneratorError::TraceUnsupportedCommand { command }),
    };
    let address = address.ok_or(TrafficGeneratorError::TraceMissingField {
        message: "Packet",
        field: "addr",
    })?;
    let size = size.ok_or(TrafficGeneratorError::TraceMissingField {
        message: "Packet",
        field: "size",
    })?;
    if size == 0 {
        return Err(TrafficGeneratorError::TraceZeroSize);
    }

    Ok(TrafficTraceElement {
        tick,
        command,
        address: Address::new(address),
        size: AccessSize::new(u64::from(size))?,
    })
}

#[derive(Clone, Copy)]
struct ProtoField {
    number: u32,
    wire_type: u64,
    value_offset: usize,
    varint_value: Option<u64>,
}

impl ProtoField {
    fn varint(
        self,
        message: &'static str,
        field: &'static str,
    ) -> Result<u64, TrafficGeneratorError> {
        if self.wire_type != WIRE_VARINT {
            return Err(TrafficGeneratorError::TraceInvalidFieldWireType {
                message,
                field,
                wire_type: self.wire_type,
            });
        }

        self.varint_value
            .ok_or(TrafficGeneratorError::TraceInvalidFieldWireType {
                message,
                field,
                wire_type: self.wire_type,
            })
    }
}

struct ProtoMessageParser<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ProtoMessageParser<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn next_field(&mut self) -> Result<Option<ProtoField>, TrafficGeneratorError> {
        if self.offset == self.bytes.len() {
            return Ok(None);
        }

        let tag = read_varint_u64(self.bytes, &mut self.offset)?;
        let number = tag >> 3;
        let wire_type = tag & 0x7;
        if number == 0 {
            return Err(TrafficGeneratorError::TraceInvalidFieldNumber);
        }
        let number = u32::try_from(number)
            .map_err(|_| TrafficGeneratorError::TraceFieldNumberTooLarge { number })?;
        let value_offset = self.offset;
        let varint_value = if wire_type == WIRE_VARINT {
            let mut value_end = value_offset;
            Some(read_varint_u64(self.bytes, &mut value_end)?)
        } else {
            None
        };

        Ok(Some(ProtoField {
            number,
            wire_type,
            value_offset,
            varint_value,
        }))
    }

    fn skip(&mut self, field: ProtoField) -> Result<(), TrafficGeneratorError> {
        match field.wire_type {
            WIRE_VARINT => {
                self.offset = field.value_offset;
                let _ = read_varint_u64(self.bytes, &mut self.offset)?;
                Ok(())
            }
            WIRE_FIXED64 => self.skip_bytes(field.value_offset, 8),
            WIRE_LENGTH_DELIMITED => {
                self.offset = field.value_offset;
                let length = read_varint_u64(self.bytes, &mut self.offset)?;
                let length = usize::try_from(length).map_err(|_| {
                    TrafficGeneratorError::TraceLengthDelimitedFieldTooLarge {
                        offset: field.value_offset,
                        length,
                    }
                })?;
                self.skip_bytes(self.offset, length)
            }
            WIRE_FIXED32 => self.skip_bytes(field.value_offset, 4),
            WIRE_START_GROUP | WIRE_END_GROUP => {
                Err(TrafficGeneratorError::TraceUnsupportedWireType {
                    wire_type: field.wire_type,
                })
            }
            wire_type => Err(TrafficGeneratorError::TraceInvalidWireType { wire_type }),
        }
    }

    fn skip_bytes(&mut self, start: usize, length: usize) -> Result<(), TrafficGeneratorError> {
        let remaining = self.bytes.len().saturating_sub(start);
        if length > remaining {
            return Err(TrafficGeneratorError::TraceTruncatedField {
                offset: start,
                length,
                remaining,
            });
        }

        self.offset = start + length;
        Ok(())
    }
}

fn read_u32_field(
    field: ProtoField,
    message: &'static str,
    name: &'static str,
) -> Result<u32, TrafficGeneratorError> {
    let value = field.varint(message, name)?;
    u32::try_from(value).map_err(|_| TrafficGeneratorError::TraceFieldOutOfRange {
        message,
        field: name,
        value,
    })
}

fn read_varint_u64(bytes: &[u8], offset: &mut usize) -> Result<u64, TrafficGeneratorError> {
    let start = *offset;
    let mut value = 0u64;

    for byte_index in 0..10 {
        let byte = *bytes
            .get(*offset)
            .ok_or(TrafficGeneratorError::TraceTruncatedVarint { offset: start })?;
        *offset += 1;

        let payload = u64::from(byte & 0x7f);
        if byte_index == 9 && payload > 1 {
            return Err(TrafficGeneratorError::TraceVarintTooLong { offset: start });
        }
        value |= payload << (byte_index * 7);

        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }

    Err(TrafficGeneratorError::TraceVarintTooLong { offset: start })
}

fn read_varint_u32(bytes: &[u8], offset: &mut usize) -> Result<u32, TrafficGeneratorError> {
    let start = *offset;
    let mut value = 0u64;

    for byte_index in 0..5 {
        let byte = *bytes
            .get(*offset)
            .ok_or(TrafficGeneratorError::TraceTruncatedVarint { offset: start })?;
        *offset += 1;

        let payload = u64::from(byte & 0x7f);
        value |= payload << (byte_index * 7);

        if byte & 0x80 == 0 {
            if value > u64::from(u32::MAX) {
                return Err(TrafficGeneratorError::TraceMessageTooLarge {
                    offset: start,
                    length: value,
                });
            }
            return Ok(value as u32);
        }
    }

    Err(TrafficGeneratorError::TraceVarint32TooLong { offset: start })
}

fn checked_trace_address(address: Address, offset: u64) -> Result<Address, TrafficGeneratorError> {
    address.get().checked_add(offset).map(Address::new).ok_or(
        TrafficGeneratorError::AddressOverflow {
            label: "trace_address",
            value: address.get(),
            increment: offset,
        },
    )
}

fn checked_tick_add(tick: u64, delta: u64) -> Result<u64, TrafficGeneratorError> {
    tick.checked_add(delta)
        .ok_or(TrafficGeneratorError::TickOverflow { tick, delta })
}

fn validate_cursor(
    config: &TrafficTraceConfig,
    cursor: usize,
) -> Result<(), TrafficGeneratorError> {
    let length = config.trace().len();
    if cursor > length {
        return Err(TrafficGeneratorError::TraceSnapshotCursorOutsideTrace { cursor, length });
    }

    Ok(())
}
