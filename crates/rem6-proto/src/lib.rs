use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;
use rem6_memory::Address;

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
    EmptyTraceIdString { key: u32 },
    ZeroTickFrequency,
    EmptyInstructionBytes,
    ZeroMemoryAccessSize,
    ZeroPacketSize,
    UnexpectedInstructionMemoryAccess { kind: InstructionKind },
    MissingInstructionMemoryAccess { kind: InstructionKind },
    DuplicateTraceIdString { key: u32 },
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
