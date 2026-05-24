use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;
use rem6_memory::Address;

mod frame;
mod frame_stream;

pub use frame::{TraceFrame, TraceFrameKind};
pub use frame_stream::{
    TraceFrameStream, TraceFrameStreamCursor, TraceFrameStreamIndex, TraceFrameStreamIndexRecord,
    TraceFrameStreamRecord, TraceFrameStreamShard, TraceFrameStreamShardCursor,
    TraceFrameStreamShardPlan,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraceSourceId(String);

impl TraceSourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProtoError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ProtoError::EmptyTraceSource);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceHeader {
    source: TraceSourceId,
    version: u32,
    tick_frequency_hz: u64,
}

impl TraceHeader {
    pub fn new(source: TraceSourceId, tick_frequency_hz: u64) -> Result<Self, ProtoError> {
        if tick_frequency_hz == 0 {
            return Err(ProtoError::ZeroTickFrequency);
        }
        Ok(Self {
            source,
            version: 0,
            tick_frequency_hz,
        })
    }

    pub const fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    pub fn source(&self) -> &TraceSourceId {
        &self.source
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn tick_frequency_hz(&self) -> u64 {
        self.tick_frequency_hz
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceIdString {
    key: u32,
    value: String,
}

impl TraceIdString {
    pub fn new(key: u32, value: impl Into<String>) -> Result<Self, ProtoError> {
        let value = value.into();
        if value.is_empty() {
            return Err(ProtoError::EmptyTraceIdString { key });
        }
        Ok(Self { key, value })
    }

    pub const fn key(&self) -> u32 {
        self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InstructionEncoding {
    Word(u32),
    Bytes(Vec<u8>),
}

impl InstructionEncoding {
    pub const fn word(value: u32) -> Self {
        Self::Word(value)
    }

    pub fn bytes(value: Vec<u8>) -> Result<Self, ProtoError> {
        if value.is_empty() {
            return Err(ProtoError::EmptyInstructionBytes);
        }
        Ok(Self::Bytes(value))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InstructionKind {
    None,
    IntAlu,
    IntMul,
    IntDiv,
    FloatAdd,
    MemRead,
    MemWrite,
    InstPrefetch,
    Other(u32),
}

impl InstructionKind {
    const fn accepts_memory_access(self) -> bool {
        matches!(self, Self::MemRead | Self::MemWrite | Self::InstPrefetch)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryAccess {
    address: Address,
    size: u32,
    flags: u32,
}

impl MemoryAccess {
    pub const fn new(address: Address, size: u32, flags: u32) -> Result<Self, ProtoError> {
        if size == 0 {
            return Err(ProtoError::ZeroMemoryAccessSize);
        }
        Ok(Self {
            address,
            size,
            flags,
        })
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn size(&self) -> u32 {
        self.size
    }

    pub const fn flags(&self) -> u32 {
        self.flags
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionRecord {
    pc: u64,
    encoding: InstructionEncoding,
    node_id: u32,
    cpu_id: u32,
    tick: Tick,
    kind: InstructionKind,
    memory_accesses: Vec<MemoryAccess>,
}

impl InstructionRecord {
    pub fn new(
        pc: u64,
        encoding: InstructionEncoding,
        node_id: u32,
        cpu_id: u32,
        tick: Tick,
        kind: InstructionKind,
    ) -> Result<Self, ProtoError> {
        Ok(Self {
            pc,
            encoding,
            node_id,
            cpu_id,
            tick,
            kind,
            memory_accesses: Vec::new(),
        })
    }

    pub fn with_memory_access(mut self, access: MemoryAccess) -> Result<Self, ProtoError> {
        if !self.kind.accepts_memory_access() {
            return Err(ProtoError::UnexpectedInstructionMemoryAccess { kind: self.kind });
        }
        self.memory_accesses.push(access);
        Ok(self)
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn node_id(&self) -> u32 {
        self.node_id
    }

    pub const fn cpu_id(&self) -> u32 {
        self.cpu_id
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn kind(&self) -> InstructionKind {
        self.kind
    }

    pub fn encoding(&self) -> &InstructionEncoding {
        &self.encoding
    }

    pub fn memory_accesses(&self) -> &[MemoryAccess] {
        &self.memory_accesses
    }

    fn validate_for_trace(&self) -> Result<(), ProtoError> {
        if self.kind.accepts_memory_access() && self.memory_accesses.is_empty() {
            return Err(ProtoError::MissingInstructionMemoryAccess { kind: self.kind });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PacketCommand {
    Read,
    Write,
    InstFetch,
    Prefetch,
    Writeback,
    Other(u32),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyTraceHeader {
    source: TraceSourceId,
    version: u32,
    tick_frequency_hz: u64,
    window_size: u32,
}

impl DependencyTraceHeader {
    pub fn new(
        source: TraceSourceId,
        tick_frequency_hz: u64,
        window_size: u32,
    ) -> Result<Self, ProtoError> {
        if tick_frequency_hz == 0 {
            return Err(ProtoError::ZeroTickFrequency);
        }
        if window_size == 0 {
            return Err(ProtoError::ZeroDependencyWindowSize);
        }
        Ok(Self {
            source,
            version: 0,
            tick_frequency_hz,
            window_size,
        })
    }

    pub const fn with_version(mut self, version: u32) -> Self {
        self.version = version;
        self
    }

    pub fn source(&self) -> &TraceSourceId {
        &self.source
    }

    pub const fn version(&self) -> u32 {
        self.version
    }

    pub const fn tick_frequency_hz(&self) -> u64 {
        self.tick_frequency_hz
    }

    pub const fn window_size(&self) -> u32 {
        self.window_size
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DependencyRecordKind {
    Invalid,
    Load,
    Store,
    Compute,
}

impl DependencyRecordKind {
    const fn uses_memory(self) -> bool {
        matches!(self, Self::Load | Self::Store)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyRecord {
    sequence: u64,
    kind: DependencyRecordKind,
    physical_address: Option<Address>,
    virtual_address: Option<Address>,
    size: Option<u32>,
    flags: u32,
    order_dependencies: Vec<u64>,
    register_dependencies: Vec<u64>,
    compute_delay: u64,
    weight: u32,
    pc: Option<u64>,
    asid: Option<u32>,
}

impl DependencyRecord {
    pub fn new(sequence: u64, kind: DependencyRecordKind) -> Result<Self, ProtoError> {
        if sequence == 0 {
            return Err(ProtoError::ZeroDependencySequence);
        }
        if kind == DependencyRecordKind::Invalid {
            return Err(ProtoError::InvalidDependencyRecordKind);
        }
        Ok(Self {
            sequence,
            kind,
            physical_address: None,
            virtual_address: None,
            size: None,
            flags: 0,
            order_dependencies: Vec::new(),
            register_dependencies: Vec::new(),
            compute_delay: 0,
            weight: 1,
            pc: None,
            asid: None,
        })
    }

    pub fn with_physical_address(mut self, address: Address) -> Self {
        self.physical_address = Some(address);
        self
    }

    pub fn with_virtual_address(mut self, address: Address) -> Self {
        self.virtual_address = Some(address);
        self
    }

    pub const fn with_asid(mut self, asid: u32) -> Self {
        self.asid = Some(asid);
        self
    }

    pub fn with_size(mut self, size: u32) -> Result<Self, ProtoError> {
        if !self.kind.uses_memory() {
            return Err(ProtoError::UnexpectedDependencyMemoryAccess { kind: self.kind });
        }
        if size == 0 {
            return Err(ProtoError::ZeroDependencyAccessSize);
        }
        self.size = Some(size);
        Ok(self)
    }

    pub const fn with_flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    pub fn with_order_dependency(mut self, dependency: u64) -> Result<Self, ProtoError> {
        if dependency == self.sequence {
            return Err(ProtoError::SelfDependency {
                sequence: self.sequence,
            });
        }
        self.order_dependencies.push(dependency);
        Ok(self)
    }

    pub fn with_register_dependency(mut self, dependency: u64) -> Result<Self, ProtoError> {
        if dependency == self.sequence {
            return Err(ProtoError::SelfDependency {
                sequence: self.sequence,
            });
        }
        self.register_dependencies.push(dependency);
        Ok(self)
    }

    pub const fn with_compute_delay(mut self, delay: u64) -> Self {
        self.compute_delay = delay;
        self
    }

    pub fn with_weight(mut self, weight: u32) -> Result<Self, ProtoError> {
        if weight == 0 {
            return Err(ProtoError::ZeroDependencyWeight);
        }
        self.weight = weight;
        Ok(self)
    }

    pub const fn with_pc(mut self, pc: u64) -> Self {
        self.pc = Some(pc);
        self
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn kind(&self) -> DependencyRecordKind {
        self.kind
    }

    pub const fn physical_address(&self) -> Option<Address> {
        self.physical_address
    }

    pub const fn virtual_address(&self) -> Option<Address> {
        self.virtual_address
    }

    pub const fn size(&self) -> Option<u32> {
        self.size
    }

    pub const fn flags(&self) -> u32 {
        self.flags
    }

    pub fn order_dependencies(&self) -> &[u64] {
        &self.order_dependencies
    }

    pub fn register_dependencies(&self) -> &[u64] {
        &self.register_dependencies
    }

    pub const fn compute_delay(&self) -> u64 {
        self.compute_delay
    }

    pub const fn weight(&self) -> u32 {
        self.weight
    }

    pub const fn pc(&self) -> Option<u64> {
        self.pc
    }

    pub const fn asid(&self) -> Option<u32> {
        self.asid
    }

    fn validate_for_trace(&self) -> Result<(), ProtoError> {
        if self.kind.uses_memory() && (self.physical_address.is_none() || self.size.is_none()) {
            return Err(ProtoError::MissingDependencyMemoryAccess { kind: self.kind });
        }
        if !self.kind.uses_memory()
            && (self.physical_address.is_some()
                || self.virtual_address.is_some()
                || self.size.is_some()
                || self.asid.is_some())
        {
            return Err(ProtoError::UnexpectedDependencyMemoryAccess { kind: self.kind });
        }
        Ok(())
    }

    fn dependencies(&self) -> impl Iterator<Item = u64> + '_ {
        self.order_dependencies
            .iter()
            .chain(self.register_dependencies.iter())
            .copied()
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DependencyTraceIdentity(String);

impl DependencyTraceIdentity {
    fn new(value: u64) -> Self {
        Self(format!("{value:016x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyTrace {
    header: DependencyTraceHeader,
    records: Vec<DependencyRecord>,
    identity: DependencyTraceIdentity,
}

impl DependencyTrace {
    pub fn builder(header: DependencyTraceHeader) -> DependencyTraceBuilder {
        DependencyTraceBuilder::new(header)
    }

    pub fn header(&self) -> &DependencyTraceHeader {
        &self.header
    }

    pub fn records(&self) -> &[DependencyRecord] {
        &self.records
    }

    pub fn identity(&self) -> DependencyTraceIdentity {
        self.identity.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyTraceBuilder {
    header: DependencyTraceHeader,
    records: Vec<DependencyRecord>,
}

impl DependencyTraceBuilder {
    fn new(header: DependencyTraceHeader) -> Self {
        Self {
            header,
            records: Vec::new(),
        }
    }

    pub fn add_record(mut self, record: DependencyRecord) -> Self {
        self.records.push(record);
        self
    }

    pub fn build(self) -> Result<DependencyTrace, ProtoError> {
        let mut records = BTreeMap::new();
        for record in self.records {
            record.validate_for_trace()?;
            let sequence = record.sequence();
            if records.insert(sequence, record).is_some() {
                return Err(ProtoError::DuplicateDependencyRecord { sequence });
            }
        }
        let records = records.into_values().collect::<Vec<_>>();
        for record in &records {
            for dependency in record.dependencies() {
                if dependency > record.sequence() {
                    return Err(ProtoError::UnknownDependency {
                        sequence: record.sequence(),
                        dependency,
                    });
                }
                if record.sequence().saturating_sub(dependency)
                    > u64::from(self.header.window_size())
                {
                    return Err(ProtoError::DependencyOutsideWindow {
                        sequence: record.sequence(),
                        dependency,
                        window_size: self.header.window_size(),
                    });
                }
            }
        }
        let identity = dependency_trace_identity(&self.header, &records);
        Ok(DependencyTrace {
            header: self.header,
            records,
            identity,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacketRecord {
    tick: Tick,
    command: PacketCommand,
    address: Address,
    size: u32,
    flags: u32,
    packet_id: Option<u64>,
    pc: Option<u64>,
}

impl PacketRecord {
    pub const fn new(
        tick: Tick,
        command: PacketCommand,
        address: Address,
        size: u32,
    ) -> Result<Self, ProtoError> {
        if size == 0 {
            return Err(ProtoError::ZeroPacketSize);
        }
        Ok(Self {
            tick,
            command,
            address,
            size,
            flags: 0,
            packet_id: None,
            pc: None,
        })
    }

    pub const fn with_flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }

    pub const fn with_packet_id(mut self, packet_id: u64) -> Self {
        self.packet_id = Some(packet_id);
        self
    }

    pub const fn with_pc(mut self, pc: u64) -> Self {
        self.pc = Some(pc);
        self
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn command(&self) -> PacketCommand {
        self.command
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn size(&self) -> u32 {
        self.size
    }

    pub const fn flags(&self) -> u32 {
        self.flags
    }

    pub const fn packet_id(&self) -> Option<u64> {
        self.packet_id
    }

    pub const fn pc(&self) -> Option<u64> {
        self.pc
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtoTraceIdentity(String);

impl ProtoTraceIdentity {
    fn new(value: u64) -> Self {
        Self(format!("{value:016x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtoTrace {
    header: TraceHeader,
    id_strings: Vec<TraceIdString>,
    instructions: Vec<InstructionRecord>,
    packets: Vec<PacketRecord>,
    identity: ProtoTraceIdentity,
}

impl ProtoTrace {
    pub fn builder(header: TraceHeader) -> ProtoTraceBuilder {
        ProtoTraceBuilder::new(header)
    }

    pub fn header(&self) -> &TraceHeader {
        &self.header
    }

    pub fn id_strings(&self) -> &[TraceIdString] {
        &self.id_strings
    }

    pub fn instructions(&self) -> &[InstructionRecord] {
        &self.instructions
    }

    pub fn packets(&self) -> &[PacketRecord] {
        &self.packets
    }

    pub fn identity(&self) -> ProtoTraceIdentity {
        self.identity.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProtoTraceBuilder {
    header: TraceHeader,
    id_strings: Vec<TraceIdString>,
    instructions: Vec<InstructionRecord>,
    packets: Vec<PacketRecord>,
}

impl ProtoTraceBuilder {
    fn new(header: TraceHeader) -> Self {
        Self {
            header,
            id_strings: Vec::new(),
            instructions: Vec::new(),
            packets: Vec::new(),
        }
    }

    pub fn add_id_string(mut self, id_string: TraceIdString) -> Self {
        self.id_strings.push(id_string);
        self
    }

    pub fn add_instruction(mut self, instruction: InstructionRecord) -> Self {
        self.instructions.push(instruction);
        self
    }

    pub fn add_packet(mut self, packet: PacketRecord) -> Self {
        self.packets.push(packet);
        self
    }

    pub fn build(self) -> Result<ProtoTrace, ProtoError> {
        let mut id_string_map = BTreeMap::new();
        for id_string in self.id_strings {
            let key = id_string.key();
            if id_string_map.insert(key, id_string).is_some() {
                return Err(ProtoError::DuplicateTraceIdString { key });
            }
        }
        let id_strings = id_string_map.into_values().collect::<Vec<_>>();
        for instruction in &self.instructions {
            instruction.validate_for_trace()?;
        }
        let identity = trace_identity(&self.header, &id_strings, &self.instructions, &self.packets);
        Ok(ProtoTrace {
            header: self.header,
            id_strings,
            instructions: self.instructions,
            packets: self.packets,
            identity,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtoError {
    EmptyTraceSource,
    EmptyTraceIdString {
        key: u32,
    },
    ZeroTickFrequency,
    EmptyInstructionBytes,
    ZeroMemoryAccessSize,
    ZeroPacketSize,
    UnexpectedInstructionMemoryAccess {
        kind: InstructionKind,
    },
    MissingInstructionMemoryAccess {
        kind: InstructionKind,
    },
    DuplicateTraceIdString {
        key: u32,
    },
    ZeroDependencyWindowSize,
    ZeroDependencySequence,
    InvalidDependencyRecordKind,
    ZeroDependencyAccessSize,
    ZeroDependencyWeight,
    UnexpectedDependencyMemoryAccess {
        kind: DependencyRecordKind,
    },
    MissingDependencyMemoryAccess {
        kind: DependencyRecordKind,
    },
    SelfDependency {
        sequence: u64,
    },
    UnknownDependency {
        sequence: u64,
        dependency: u64,
    },
    DependencyOutsideWindow {
        sequence: u64,
        dependency: u64,
        window_size: u32,
    },
    DuplicateDependencyRecord {
        sequence: u64,
    },
    EmptyFrameIdentity,
    FrameIdentityTooLong {
        bytes: usize,
    },
    InvalidFrameIdentity,
    EmptyFramePayload,
    InvalidFrameMagic,
    UnsupportedFrameVersion {
        version: u16,
    },
    UnknownFrameKind {
        kind: u8,
    },
    TruncatedFrame,
    FrameChecksumMismatch,
    EmptyFrameStream,
    ZeroFrameStreamShardBudget,
    InvalidFrameStreamMagic,
    UnsupportedFrameStreamVersion {
        version: u16,
    },
    TruncatedFrameStream,
    InvalidFrameStreamLength,
}

impl fmt::Display for ProtoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTraceSource => write!(formatter, "trace source id must not be empty"),
            Self::EmptyTraceIdString { key } => {
                write!(formatter, "trace id string {key} must not be empty")
            }
            Self::ZeroTickFrequency => write!(formatter, "trace tick frequency must be positive"),
            Self::EmptyInstructionBytes => write!(formatter, "instruction bytes must not be empty"),
            Self::ZeroMemoryAccessSize => write!(formatter, "memory access size must be positive"),
            Self::ZeroPacketSize => write!(formatter, "packet size must be positive"),
            Self::UnexpectedInstructionMemoryAccess { kind } => {
                write!(
                    formatter,
                    "instruction kind {kind:?} cannot include memory accesses"
                )
            }
            Self::MissingInstructionMemoryAccess { kind } => {
                write!(
                    formatter,
                    "instruction kind {kind:?} requires at least one memory access"
                )
            }
            Self::DuplicateTraceIdString { key } => {
                write!(formatter, "trace id string key {key} is duplicated")
            }
            Self::ZeroDependencyWindowSize => {
                write!(formatter, "dependency trace window size must be positive")
            }
            Self::ZeroDependencySequence => {
                write!(formatter, "dependency record sequence must be positive")
            }
            Self::InvalidDependencyRecordKind => {
                write!(formatter, "dependency record kind must not be invalid")
            }
            Self::ZeroDependencyAccessSize => {
                write!(formatter, "dependency memory access size must be positive")
            }
            Self::ZeroDependencyWeight => {
                write!(formatter, "dependency record weight must be positive")
            }
            Self::UnexpectedDependencyMemoryAccess { kind } => {
                write!(
                    formatter,
                    "dependency record kind {kind:?} cannot include memory access fields"
                )
            }
            Self::MissingDependencyMemoryAccess { kind } => {
                write!(
                    formatter,
                    "dependency record kind {kind:?} requires physical address and size"
                )
            }
            Self::SelfDependency { sequence } => {
                write!(formatter, "dependency record {sequence} depends on itself")
            }
            Self::UnknownDependency {
                sequence,
                dependency,
            } => {
                write!(
                    formatter,
                    "dependency record {sequence} depends on future record {dependency}"
                )
            }
            Self::DependencyOutsideWindow {
                sequence,
                dependency,
                window_size,
            } => {
                write!(
                    formatter,
                    "dependency record {sequence} dependency {dependency} exceeds window {window_size}"
                )
            }
            Self::DuplicateDependencyRecord { sequence } => {
                write!(
                    formatter,
                    "dependency record sequence {sequence} is duplicated"
                )
            }
            Self::EmptyFrameIdentity => write!(formatter, "trace frame identity must not be empty"),
            Self::FrameIdentityTooLong { bytes } => write!(
                formatter,
                "trace frame identity has {bytes} bytes, exceeding frame limit"
            ),
            Self::InvalidFrameIdentity => write!(formatter, "trace frame identity is not utf-8"),
            Self::EmptyFramePayload => write!(formatter, "trace frame payload must not be empty"),
            Self::InvalidFrameMagic => write!(formatter, "trace frame magic is invalid"),
            Self::UnsupportedFrameVersion { version } => {
                write!(formatter, "trace frame version {version} is not supported")
            }
            Self::UnknownFrameKind { kind } => {
                write!(formatter, "trace frame kind {kind} is unknown")
            }
            Self::TruncatedFrame => write!(formatter, "trace frame is truncated"),
            Self::FrameChecksumMismatch => write!(formatter, "trace frame checksum does not match"),
            Self::EmptyFrameStream => write!(formatter, "trace frame stream must not be empty"),
            Self::ZeroFrameStreamShardBudget => {
                write!(
                    formatter,
                    "trace frame stream shard budget must be positive"
                )
            }
            Self::InvalidFrameStreamMagic => {
                write!(formatter, "trace frame stream magic is invalid")
            }
            Self::UnsupportedFrameStreamVersion { version } => {
                write!(
                    formatter,
                    "trace frame stream version {version} is not supported"
                )
            }
            Self::TruncatedFrameStream => write!(formatter, "trace frame stream is truncated"),
            Self::InvalidFrameStreamLength => {
                write!(formatter, "trace frame stream record length is invalid")
            }
        }
    }
}

impl Error for ProtoError {}

fn trace_identity(
    header: &TraceHeader,
    id_strings: &[TraceIdString],
    instructions: &[InstructionRecord],
    packets: &[PacketRecord],
) -> ProtoTraceIdentity {
    let mut hash = FNV_OFFSET;
    hash_str(&mut hash, "rem6.proto.trace.v1");
    hash_str(&mut hash, header.source().as_str());
    hash_u64(&mut hash, u64::from(header.version()));
    hash_u64(&mut hash, header.tick_frequency_hz());
    hash_u64(&mut hash, id_strings.len() as u64);
    for id_string in id_strings {
        hash_u64(&mut hash, u64::from(id_string.key()));
        hash_str(&mut hash, id_string.value());
    }
    hash_u64(&mut hash, instructions.len() as u64);
    for instruction in instructions {
        hash_instruction(&mut hash, instruction);
    }
    hash_u64(&mut hash, packets.len() as u64);
    for packet in packets {
        hash_packet(&mut hash, packet);
    }
    ProtoTraceIdentity::new(hash)
}

fn dependency_trace_identity(
    header: &DependencyTraceHeader,
    records: &[DependencyRecord],
) -> DependencyTraceIdentity {
    let mut hash = FNV_OFFSET;
    hash_str(&mut hash, "rem6.proto.dependency_trace.v1");
    hash_str(&mut hash, header.source().as_str());
    hash_u64(&mut hash, u64::from(header.version()));
    hash_u64(&mut hash, header.tick_frequency_hz());
    hash_u64(&mut hash, u64::from(header.window_size()));
    hash_u64(&mut hash, records.len() as u64);
    for record in records {
        hash_dependency_record(&mut hash, record);
    }
    DependencyTraceIdentity::new(hash)
}

fn hash_dependency_record(hash: &mut u64, record: &DependencyRecord) {
    hash_u64(hash, record.sequence());
    hash_dependency_record_kind(hash, record.kind());
    hash_optional_address(hash, record.physical_address());
    hash_optional_address(hash, record.virtual_address());
    hash_optional_u32(hash, record.size());
    hash_u64(hash, u64::from(record.flags()));
    hash_u64(hash, record.order_dependencies().len() as u64);
    for dependency in record.order_dependencies() {
        hash_u64(hash, *dependency);
    }
    hash_u64(hash, record.register_dependencies().len() as u64);
    for dependency in record.register_dependencies() {
        hash_u64(hash, *dependency);
    }
    hash_u64(hash, record.compute_delay());
    hash_u64(hash, u64::from(record.weight()));
    hash_optional_u64(hash, record.pc());
    hash_optional_u32(hash, record.asid());
}

fn hash_dependency_record_kind(hash: &mut u64, kind: DependencyRecordKind) {
    match kind {
        DependencyRecordKind::Invalid => hash_str(hash, "invalid"),
        DependencyRecordKind::Load => hash_str(hash, "load"),
        DependencyRecordKind::Store => hash_str(hash, "store"),
        DependencyRecordKind::Compute => hash_str(hash, "compute"),
    }
}

fn hash_instruction(hash: &mut u64, instruction: &InstructionRecord) {
    hash_u64(hash, instruction.pc());
    match instruction.encoding() {
        InstructionEncoding::Word(word) => {
            hash_str(hash, "word");
            hash_u64(hash, u64::from(*word));
        }
        InstructionEncoding::Bytes(bytes) => {
            hash_str(hash, "bytes");
            hash_bytes(hash, bytes);
        }
    }
    hash_u64(hash, u64::from(instruction.node_id()));
    hash_u64(hash, u64::from(instruction.cpu_id()));
    hash_u64(hash, instruction.tick());
    hash_instruction_kind(hash, instruction.kind());
    hash_u64(hash, instruction.memory_accesses().len() as u64);
    for access in instruction.memory_accesses() {
        hash_u64(hash, access.address().get());
        hash_u64(hash, u64::from(access.size()));
        hash_u64(hash, u64::from(access.flags()));
    }
}

fn hash_packet(hash: &mut u64, packet: &PacketRecord) {
    hash_u64(hash, packet.tick());
    hash_packet_command(hash, packet.command());
    hash_u64(hash, packet.address().get());
    hash_u64(hash, u64::from(packet.size()));
    hash_u64(hash, u64::from(packet.flags()));
    hash_optional_u64(hash, packet.packet_id());
    hash_optional_u64(hash, packet.pc());
}

fn hash_instruction_kind(hash: &mut u64, kind: InstructionKind) {
    match kind {
        InstructionKind::None => hash_str(hash, "none"),
        InstructionKind::IntAlu => hash_str(hash, "int-alu"),
        InstructionKind::IntMul => hash_str(hash, "int-mul"),
        InstructionKind::IntDiv => hash_str(hash, "int-div"),
        InstructionKind::FloatAdd => hash_str(hash, "float-add"),
        InstructionKind::MemRead => hash_str(hash, "mem-read"),
        InstructionKind::MemWrite => hash_str(hash, "mem-write"),
        InstructionKind::InstPrefetch => hash_str(hash, "inst-prefetch"),
        InstructionKind::Other(value) => {
            hash_str(hash, "other");
            hash_u64(hash, u64::from(value));
        }
    }
}

fn hash_packet_command(hash: &mut u64, command: PacketCommand) {
    match command {
        PacketCommand::Read => hash_str(hash, "read"),
        PacketCommand::Write => hash_str(hash, "write"),
        PacketCommand::InstFetch => hash_str(hash, "inst-fetch"),
        PacketCommand::Prefetch => hash_str(hash, "prefetch"),
        PacketCommand::Writeback => hash_str(hash, "writeback"),
        PacketCommand::Other(value) => {
            hash_str(hash, "other");
            hash_u64(hash, u64::from(value));
        }
    }
}

fn hash_optional_u64(hash: &mut u64, value: Option<u64>) {
    match value {
        Some(value) => {
            hash_str(hash, "some");
            hash_u64(hash, value);
        }
        None => hash_str(hash, "none"),
    }
}

fn hash_optional_u32(hash: &mut u64, value: Option<u32>) {
    match value {
        Some(value) => {
            hash_str(hash, "some");
            hash_u64(hash, u64::from(value));
        }
        None => hash_str(hash, "none"),
    }
}

fn hash_optional_address(hash: &mut u64, value: Option<Address>) {
    match value {
        Some(value) => {
            hash_str(hash, "some");
            hash_u64(hash, value.get());
        }
        None => hash_str(hash, "none"),
    }
}

fn hash_str(hash: &mut u64, value: &str) {
    hash_bytes(hash, value.as_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    hash_u64(hash, bytes.len() as u64);
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}
