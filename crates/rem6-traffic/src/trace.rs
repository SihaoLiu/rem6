use std::io::Read;

use flate2::read::GzDecoder;
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAccessOrdering, MemoryAtomicOp,
    MemoryBarrierSet, MemoryRequest, MemoryRequestId,
};

use crate::{
    common::{
        checked_counter_add, TrafficGeneratorSummary, TrafficRequestEvent, TrafficRequestKind,
    },
    trace_event::{
        TrafficTraceDiagnosticEvent, TrafficTraceDiagnosticKind, TrafficTraceEvent,
        TrafficTraceHtmEvent, TrafficTraceHtmKind, TrafficTraceSyncEvent, TrafficTraceSyncKind,
        TrafficTraceTlbEvent, TrafficTraceTlbKind,
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
const GEM5_SC_UPGRADE_REQ: u32 = 18;
const GEM5_SC_UPGRADE_FAIL_REQ: u32 = 20;
const GEM5_READ_EX_REQ: u32 = 22;
const GEM5_READ_CLEAN_REQ: u32 = 24;
const GEM5_READ_SHARED_REQ: u32 = 25;
const GEM5_LOAD_LOCKED_REQ: u32 = 26;
const GEM5_STORE_COND_REQ: u32 = 27;
const GEM5_STORE_COND_FAIL_REQ: u32 = 28;
const GEM5_LOCKED_RMW_READ_REQ: u32 = 30;
const GEM5_LOCKED_RMW_WRITE_REQ: u32 = 32;
const GEM5_SWAP_REQ: u32 = 34;
const GEM5_MEM_FENCE_REQ: u32 = 38;
const GEM5_MEM_SYNC_REQ: u32 = 39;
const GEM5_CLEAN_SHARED_REQ: u32 = 42;
const GEM5_CLEAN_INVALID_REQ: u32 = 44;
const GEM5_PRINT_REQ: u32 = 52;
const GEM5_INVALIDATE_REQ: u32 = 54;
const GEM5_HTM_REQ: u32 = 56;
const GEM5_HTM_ABORT: u32 = 58;
const GEM5_TLBI_EXT_SYNC: u32 = 59;
const GEM5_FLAG_INST_FETCH: u32 = 0x0000_0100;
const GEM5_FLAG_PHYSICAL: u32 = 0x0000_0200;
const GEM5_FLAG_UNCACHEABLE: u32 = 0x0000_0400;
const GEM5_FLAG_STRICT_ORDER: u32 = 0x0000_0800;
const GEM5_FLAG_KERNEL: u32 = 0x0000_1000;
const GEM5_FLAG_PRIVILEGED: u32 = 0x0000_8000;
const GEM5_FLAG_ACQUIRE_PC: u32 = 0x0000_2000;
const GEM5_FLAG_CACHE_BLOCK_ZERO: u32 = 0x0001_0000;
const GEM5_FLAG_ACQUIRE: u32 = 0x0002_0000;
const GEM5_FLAG_RELEASE: u32 = 0x0004_0000;
const GEM5_FLAG_NO_ACCESS: u32 = 0x0008_0000;
const GEM5_FLAG_LOCKED_RMW: u32 = 0x0010_0000;
const GEM5_FLAG_LLSC: u32 = 0x0020_0000;
const GEM5_FLAG_MEM_SWAP: u32 = 0x0040_0000;
const GEM5_FLAG_MEM_SWAP_COND: u32 = 0x0080_0000;
const GEM5_FLAG_PREFETCH: u32 = 0x0100_0000;
const GEM5_FLAG_PF_EXCLUSIVE: u32 = 0x0200_0000;
const GEM5_FLAG_EVICT_NEXT: u32 = 0x0400_0000;
const GEM5_FLAG_SECURE: u32 = 0x1000_0000;
const GEM5_FLAG_PT_WALK: u32 = 0x2000_0000;
const GEM5_SUPPORTED_TRACE_FLAGS: u32 = GEM5_FLAG_INST_FETCH
    | GEM5_FLAG_PHYSICAL
    | GEM5_FLAG_UNCACHEABLE
    | GEM5_FLAG_STRICT_ORDER
    | GEM5_FLAG_KERNEL
    | GEM5_FLAG_PRIVILEGED
    | GEM5_FLAG_ACQUIRE_PC
    | GEM5_FLAG_CACHE_BLOCK_ZERO
    | GEM5_FLAG_ACQUIRE
    | GEM5_FLAG_RELEASE
    | GEM5_FLAG_NO_ACCESS
    | GEM5_FLAG_LOCKED_RMW
    | GEM5_FLAG_LLSC
    | GEM5_FLAG_MEM_SWAP
    | GEM5_FLAG_MEM_SWAP_COND
    | GEM5_FLAG_PREFETCH
    | GEM5_FLAG_PF_EXCLUSIVE
    | GEM5_FLAG_EVICT_NEXT
    | GEM5_FLAG_SECURE
    | GEM5_FLAG_PT_WALK;
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
    LoadLocked,
    StoreConditional,
    StoreConditionalFail,
    StoreConditionalUpgrade,
    StoreConditionalUpgradeFail,
    LockedRmwRead,
    LockedRmwWrite,
    Write,
    WriteLine,
    WritebackDirty,
    WritebackClean,
    WriteClean,
    Swap,
    CleanEvict,
    CleanShared,
    CleanInvalid,
    Invalidate,
    Upgrade,
    MemFence,
    MemSync,
    HtmRequest,
    HtmAbort,
    Print,
    TlbiExtSync,
}

impl TrafficTraceCommand {
    const fn request_kind(self) -> TrafficRequestKind {
        match self {
            Self::ReadShared
            | Self::ReadUnique
            | Self::SoftPrefetchRead
            | Self::HardPrefetchRead
            | Self::PrefetchWrite
            | Self::LoadLocked
            | Self::StoreConditionalUpgradeFail
            | Self::LockedRmwRead => TrafficRequestKind::Read,
            Self::Write
            | Self::StoreConditional
            | Self::StoreConditionalFail
            | Self::LockedRmwWrite
            | Self::WriteLine
            | Self::WritebackDirty
            | Self::WritebackClean
            | Self::WriteClean => TrafficRequestKind::Write,
            Self::Swap => TrafficRequestKind::Atomic,
            Self::CleanEvict
            | Self::CleanShared
            | Self::CleanInvalid
            | Self::Invalidate
            | Self::StoreConditionalUpgrade
            | Self::Upgrade
            | Self::MemFence
            | Self::MemSync
            | Self::HtmRequest
            | Self::HtmAbort
            | Self::Print
            | Self::TlbiExtSync => TrafficRequestKind::Maintenance,
        }
    }

    const fn sync_kind(self) -> Option<TrafficTraceSyncKind> {
        match self {
            Self::MemFence => Some(TrafficTraceSyncKind::MemFence),
            Self::MemSync => Some(TrafficTraceSyncKind::MemSync),
            _ => None,
        }
    }

    const fn tlb_kind(self) -> Option<TrafficTraceTlbKind> {
        match self {
            Self::TlbiExtSync => Some(TrafficTraceTlbKind::ExternalSync),
            _ => None,
        }
    }

    const fn htm_kind(self) -> Option<TrafficTraceHtmKind> {
        match self {
            Self::HtmRequest => Some(TrafficTraceHtmKind::Request),
            Self::HtmAbort => Some(TrafficTraceHtmKind::Abort),
            _ => None,
        }
    }

    const fn diagnostic_kind(self) -> Option<TrafficTraceDiagnosticKind> {
        match self {
            Self::Print => Some(TrafficTraceDiagnosticKind::Print),
            _ => None,
        }
    }

    const fn gem5_name(self) -> &'static str {
        match self {
            Self::ReadShared => "ReadReq",
            Self::ReadUnique => "ReadExReq",
            Self::SoftPrefetchRead => "SoftPFReq",
            Self::HardPrefetchRead => "HardPFReq",
            Self::PrefetchWrite => "SoftPFExReq",
            Self::LoadLocked => "LoadLockedReq",
            Self::StoreConditional => "StoreCondReq",
            Self::StoreConditionalFail => "StoreCondFailReq",
            Self::StoreConditionalUpgrade => "SCUpgradeReq",
            Self::StoreConditionalUpgradeFail => "SCUpgradeFailReq",
            Self::LockedRmwRead => "LockedRMWReadReq",
            Self::LockedRmwWrite => "LockedRMWWriteReq",
            Self::Write => "WriteReq",
            Self::WriteLine => "WriteLineReq",
            Self::WritebackDirty => "WritebackDirty",
            Self::WritebackClean => "WritebackClean",
            Self::WriteClean => "WriteClean",
            Self::Swap => "SwapReq",
            Self::CleanEvict => "CleanEvict",
            Self::CleanShared => "CleanSharedReq",
            Self::CleanInvalid => "CleanInvalidReq",
            Self::Invalidate => "InvalidateReq",
            Self::Upgrade => "UpgradeReq",
            Self::MemFence => "MemFenceReq",
            Self::MemSync => "MemSyncReq",
            Self::HtmRequest => "HTMReq",
            Self::HtmAbort => "HTMAbort",
            Self::Print => "PrintReq",
            Self::TlbiExtSync => "TlbiExtSync",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrafficTraceElement {
    tick: u64,
    command: TrafficTraceCommand,
    address: Option<Address>,
    size: Option<AccessSize>,
    flags: TrafficTraceRequestFlags,
    packet_id: Option<u64>,
    pc: Option<Address>,
}

impl TrafficTraceElement {
    const fn request_kind(self) -> TrafficRequestKind {
        if self.flags.is_prefetch() {
            TrafficRequestKind::Read
        } else {
            self.command.request_kind()
        }
    }

    const fn sync_kind(self) -> Option<TrafficTraceSyncKind> {
        self.command.sync_kind()
    }

    const fn tlb_kind(self) -> Option<TrafficTraceTlbKind> {
        self.command.tlb_kind()
    }

    const fn htm_kind(self) -> Option<TrafficTraceHtmKind> {
        self.command.htm_kind()
    }

    const fn diagnostic_kind(self) -> Option<TrafficTraceDiagnosticKind> {
        self.command.diagnostic_kind()
    }

    fn request_address(self) -> Address {
        self.address
            .expect("validated trace request element has an address")
    }

    fn request_size(self) -> AccessSize {
        self.size
            .expect("validated trace request element has an access size")
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TrafficTraceRequestFlags {
    bits: u32,
    inst_fetch: bool,
    prefetch: bool,
    prefetch_exclusive: bool,
    uncacheable: bool,
    strict_order: bool,
    kernel_sync: bool,
    privileged: bool,
    cache_block_zero: bool,
    no_access: bool,
    acquire: bool,
    release: bool,
    locked_rmw: bool,
    llsc: bool,
    mem_swap: bool,
    mem_swap_cond: bool,
    evict_next: bool,
    secure: bool,
    page_table_walk: bool,
}

impl TrafficTraceRequestFlags {
    fn from_gem5(bits: u32) -> Result<Self, TrafficGeneratorError> {
        let unsupported = bits & !GEM5_SUPPORTED_TRACE_FLAGS;
        if unsupported != 0
            || (bits & GEM5_FLAG_STRICT_ORDER != 0 && bits & GEM5_FLAG_UNCACHEABLE == 0)
        {
            return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: bits });
        }

        Ok(Self {
            bits,
            inst_fetch: bits & GEM5_FLAG_INST_FETCH != 0,
            prefetch: bits & GEM5_FLAG_PREFETCH != 0,
            prefetch_exclusive: bits & GEM5_FLAG_PF_EXCLUSIVE != 0,
            uncacheable: bits & GEM5_FLAG_UNCACHEABLE != 0,
            strict_order: bits & GEM5_FLAG_STRICT_ORDER != 0,
            kernel_sync: bits & GEM5_FLAG_KERNEL != 0,
            privileged: bits & GEM5_FLAG_PRIVILEGED != 0,
            cache_block_zero: bits & GEM5_FLAG_CACHE_BLOCK_ZERO != 0,
            no_access: bits & GEM5_FLAG_NO_ACCESS != 0,
            acquire: bits & (GEM5_FLAG_ACQUIRE | GEM5_FLAG_ACQUIRE_PC) != 0,
            release: bits & GEM5_FLAG_RELEASE != 0,
            locked_rmw: bits & GEM5_FLAG_LOCKED_RMW != 0,
            llsc: bits & GEM5_FLAG_LLSC != 0,
            mem_swap: bits & GEM5_FLAG_MEM_SWAP != 0,
            mem_swap_cond: bits & GEM5_FLAG_MEM_SWAP_COND != 0,
            evict_next: bits & GEM5_FLAG_EVICT_NEXT != 0,
            secure: bits & GEM5_FLAG_SECURE != 0,
            page_table_walk: bits & GEM5_FLAG_PT_WALK != 0,
        })
    }

    const fn is_inst_fetch(self) -> bool {
        self.inst_fetch
    }

    const fn is_prefetch(self) -> bool {
        self.prefetch || self.prefetch_exclusive
    }

    const fn is_prefetch_exclusive(self) -> bool {
        self.prefetch_exclusive
    }

    fn validate_for_command(
        self,
        command: TrafficTraceCommand,
    ) -> Result<(), TrafficGeneratorError> {
        if command.sync_kind().is_some() {
            if self.bits & !GEM5_FLAG_KERNEL != 0 {
                return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
            }
            return Ok(());
        }

        if command.tlb_kind().is_some() {
            if self.bits != 0 {
                return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
            }
            return Ok(());
        }

        if command.htm_kind().is_some() {
            if self.bits != 0 {
                return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
            }
            return Ok(());
        }

        if command.diagnostic_kind().is_some() {
            if self.bits != 0 {
                return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
            }
            return Ok(());
        }

        if self.inst_fetch && command != TrafficTraceCommand::ReadShared {
            return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
        }
        if self.is_prefetch() {
            let supported_command = match command {
                TrafficTraceCommand::ReadShared
                | TrafficTraceCommand::SoftPrefetchRead
                | TrafficTraceCommand::HardPrefetchRead
                | TrafficTraceCommand::PrefetchWrite => true,
                TrafficTraceCommand::Write => self.prefetch_exclusive,
                _ => false,
            };
            if !supported_command {
                return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
            }
        }
        if self.prefetch && command == TrafficTraceCommand::Write && !self.prefetch_exclusive {
            return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
        }
        if self.cache_block_zero && command != TrafficTraceCommand::Write {
            return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
        }
        if self.no_access
            && (!matches!(
                command,
                TrafficTraceCommand::ReadShared
                    | TrafficTraceCommand::ReadUnique
                    | TrafficTraceCommand::Write
            ) || self.is_prefetch()
                || self.cache_block_zero
                || self.llsc
                || self.locked_rmw
                || self.mem_swap
                || self.mem_swap_cond)
        {
            return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
        }
        if self.llsc
            && !matches!(
                command,
                TrafficTraceCommand::LoadLocked
                    | TrafficTraceCommand::StoreConditional
                    | TrafficTraceCommand::StoreConditionalFail
                    | TrafficTraceCommand::StoreConditionalUpgrade
                    | TrafficTraceCommand::StoreConditionalUpgradeFail
            )
        {
            return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
        }
        if self.locked_rmw
            && !matches!(
                command,
                TrafficTraceCommand::LockedRmwRead | TrafficTraceCommand::LockedRmwWrite
            )
        {
            return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
        }
        if (self.mem_swap || self.mem_swap_cond) && command != TrafficTraceCommand::Swap {
            return Err(TrafficGeneratorError::TraceUnsupportedFlags { flags: self.bits });
        }
        Ok(())
    }

    fn apply(self, request: MemoryRequest) -> MemoryRequest {
        let ordered = request.with_ordering(MemoryAccessOrdering::new(
            self.release.then_some(MemoryBarrierSet::memory()),
            self.acquire.then_some(MemoryBarrierSet::memory()),
        ));

        let mut mapped = if self.strict_order {
            ordered.with_uncacheable_strict_order()
        } else if self.uncacheable {
            ordered.with_uncacheable()
        } else {
            ordered
        };

        if self.privileged {
            mapped = mapped.with_privileged();
        }
        if self.secure {
            mapped = mapped.with_secure();
        }
        if self.page_table_walk {
            mapped = mapped.with_page_table_walk();
        }
        if self.evict_next {
            mapped = mapped.with_evict_next();
        }
        if self.kernel_sync {
            mapped = mapped.with_kernel_sync();
        }
        mapped
    }
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
        if let Some(kind) = element.sync_kind() {
            return Err(TrafficGeneratorError::TraceSyncEventRequiresNextEvent {
                command: kind.gem5_name(),
            });
        }
        if let Some(kind) = element.tlb_kind() {
            return Err(TrafficGeneratorError::TraceTlbEventRequiresNextEvent {
                command: kind.gem5_name(),
            });
        }
        if let Some(kind) = element.htm_kind() {
            return Err(TrafficGeneratorError::TraceHtmEventRequiresNextEvent {
                command: kind.gem5_name(),
            });
        }
        if let Some(kind) = element.diagnostic_kind() {
            return Err(
                TrafficGeneratorError::TraceDiagnosticEventRequiresNextEvent {
                    command: kind.gem5_name(),
                },
            );
        }

        let Some(event) = self.next_event(tick, retry_delay)? else {
            return Ok(None);
        };
        match event {
            TrafficTraceEvent::Request(request) => Ok(Some(request)),
            TrafficTraceEvent::Sync(_) => {
                unreachable!("sync trace event was rejected before advancing")
            }
            TrafficTraceEvent::Tlb(_) => {
                unreachable!("TLB trace event was rejected before advancing")
            }
            TrafficTraceEvent::Htm(_) => {
                unreachable!("HTM trace event was rejected before advancing")
            }
            TrafficTraceEvent::Diagnostic(_) => {
                unreachable!("diagnostic trace event was rejected before advancing")
            }
        }
    }

    pub fn next_event(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<Option<TrafficTraceEvent>, TrafficGeneratorError> {
        let Some(element) = self.next_element() else {
            return Ok(None);
        };

        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("next_sequence", sequence, 1)?;
        let (next_tick_offset, event_tick) =
            self.next_packet_tick_from(self.tick_offset, element.tick, tick, retry_delay)?;
        let mut next_summary = self.summary;
        let event = if let Some(kind) = element.sync_kind() {
            next_summary.record(event_tick, TrafficRequestKind::Maintenance, 0)?;
            TrafficTraceEvent::Sync(TrafficTraceSyncEvent::new(
                event_tick,
                sequence,
                kind,
                element.flags.kernel_sync,
                element.packet_id,
                element.pc,
            ))
        } else if let Some(kind) = element.tlb_kind() {
            next_summary.record(event_tick, TrafficRequestKind::Maintenance, 0)?;
            TrafficTraceEvent::Tlb(TrafficTraceTlbEvent::new(
                event_tick,
                sequence,
                kind,
                element.packet_id,
                element.pc,
            ))
        } else if let Some(kind) = element.htm_kind() {
            next_summary.record(event_tick, TrafficRequestKind::Maintenance, 0)?;
            TrafficTraceEvent::Htm(TrafficTraceHtmEvent::new(
                event_tick,
                sequence,
                kind,
                element.address,
                element.size,
                element.packet_id,
                element.pc,
            ))
        } else if let Some(kind) = element.diagnostic_kind() {
            next_summary.record(event_tick, TrafficRequestKind::Maintenance, 0)?;
            TrafficTraceEvent::Diagnostic(TrafficTraceDiagnosticEvent::new(
                event_tick,
                sequence,
                kind,
                element.address,
                element.size,
                element.packet_id,
                element.pc,
            ))
        } else {
            let kind = element.request_kind();
            let address =
                checked_trace_address(element.request_address(), self.config.addr_offset())?;
            let request = self.build_request(sequence, element, kind, address)?;
            next_summary.record(event_tick, kind, element.request_size().bytes())?;
            TrafficTraceEvent::Request(
                TrafficRequestEvent::new(event_tick, sequence, kind, address, request)
                    .with_trace_metadata(element.packet_id, element.pc),
            )
        };

        self.cursor += 1;
        self.next_sequence = next_sequence;
        self.summary = next_summary;
        self.tick_offset = next_tick_offset;

        Ok(Some(event))
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
        let size = element.request_size();

        let request = match kind {
            TrafficRequestKind::Read | TrafficRequestKind::Write if element.flags.no_access => {
                MemoryRequest::no_access(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read if element.flags.is_prefetch_exclusive() => {
                MemoryRequest::prefetch_write(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read if element.flags.is_prefetch() => {
                MemoryRequest::prefetch_read(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read if element.flags.is_inst_fetch() => {
                MemoryRequest::instruction_fetch(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read if element.command == TrafficTraceCommand::ReadUnique => {
                MemoryRequest::read_unique(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read
                if element.command == TrafficTraceCommand::StoreConditionalUpgradeFail =>
            {
                validate_upgrade_request(address, size, layout)?;
                MemoryRequest::store_conditional_upgrade_fail(id, address, size, layout)
                    .map_err(Into::into)
            }
            TrafficRequestKind::Read
                if matches!(
                    element.command,
                    TrafficTraceCommand::SoftPrefetchRead | TrafficTraceCommand::HardPrefetchRead
                ) =>
            {
                MemoryRequest::prefetch_read(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read if element.command == TrafficTraceCommand::PrefetchWrite => {
                MemoryRequest::prefetch_write(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read if element.command == TrafficTraceCommand::LoadLocked => {
                MemoryRequest::load_locked(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read if element.command == TrafficTraceCommand::LockedRmwRead => {
                MemoryRequest::locked_rmw_read(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Read => {
                MemoryRequest::read_shared(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Write if element.command == TrafficTraceCommand::WriteLine => {
                validate_write_line_request(address, size, layout)?;
                build_write_request(self.config.agent(), id, address, size, layout)
            }
            TrafficRequestKind::Write if element.flags.cache_block_zero => {
                validate_cache_block_zero_request(address, size, layout)?;
                MemoryRequest::cache_block_zero(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Write
                if matches!(
                    element.command,
                    TrafficTraceCommand::WritebackDirty
                        | TrafficTraceCommand::WritebackClean
                        | TrafficTraceCommand::WriteClean
                ) =>
            {
                validate_writeback_request(element.command, address, size, layout)?;
                build_writeback_request(
                    element.command,
                    self.config.agent(),
                    id,
                    address,
                    size,
                    layout,
                )
            }
            TrafficRequestKind::Write => {
                if element.command == TrafficTraceCommand::StoreConditional {
                    build_store_conditional_request(self.config.agent(), id, address, size, layout)
                } else if element.command == TrafficTraceCommand::StoreConditionalFail {
                    build_store_conditional_fail_request(
                        self.config.agent(),
                        id,
                        address,
                        size,
                        layout,
                    )
                } else if element.command == TrafficTraceCommand::LockedRmwWrite {
                    build_locked_rmw_write_request(self.config.agent(), id, address, size, layout)
                } else {
                    build_write_request(self.config.agent(), id, address, size, layout)
                }
            }
            TrafficRequestKind::Atomic if element.command == TrafficTraceCommand::Swap => {
                build_atomic_swap_request(self.config.agent(), id, address, size, layout)
            }
            TrafficRequestKind::Atomic => {
                unreachable!("atomic trace kind has no request builder")
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::CleanEvict =>
            {
                validate_clean_evict_request(address, size, layout)?;
                MemoryRequest::clean_evict(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::CleanShared =>
            {
                validate_clean_maintenance_request(element.command, address, size, layout)?;
                MemoryRequest::clean_shared(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::CleanInvalid =>
            {
                validate_clean_maintenance_request(element.command, address, size, layout)?;
                MemoryRequest::invalidate(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::Invalidate =>
            {
                validate_invalidate_request(address, size, layout)?;
                MemoryRequest::invalidate_writable(id, address, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance if element.command == TrafficTraceCommand::Upgrade => {
                validate_upgrade_request(address, size, layout)?;
                MemoryRequest::upgrade(id, address, size, layout).map_err(Into::into)
            }
            TrafficRequestKind::Maintenance
                if element.command == TrafficTraceCommand::StoreConditionalUpgrade =>
            {
                validate_upgrade_request(address, size, layout)?;
                MemoryRequest::store_conditional_upgrade(id, address, size, layout)
                    .map_err(Into::into)
            }
            TrafficRequestKind::Maintenance => {
                unreachable!("maintenance trace kind has no request builder")
            }
        }?;
        Ok(element.flags.apply(request))
    }
}

fn build_atomic_swap_request(
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
    MemoryRequest::atomic_with_op(id, address, size, MemoryAtomicOp::Swap, data, mask, layout)
        .map_err(Into::into)
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

fn validate_cache_block_zero_request(
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<(), TrafficGeneratorError> {
    if size.bytes() != layout.bytes() {
        return Err(TrafficGeneratorError::TraceCacheBlockZeroSizeMismatch {
            size: size.bytes(),
            line_size: layout.bytes(),
        });
    }
    if layout.line_offset(address) != 0 {
        return Err(TrafficGeneratorError::TraceCacheBlockZeroUnalignedAddress {
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

fn build_locked_rmw_write_request(
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
    MemoryRequest::locked_rmw_write(id, address, size, data, mask, layout).map_err(Into::into)
}

fn build_store_conditional_request(
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
    MemoryRequest::store_conditional(id, address, size, data, mask, layout).map_err(Into::into)
}

fn build_store_conditional_fail_request(
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
    MemoryRequest::store_conditional_fail(id, address, size, data, mask, layout).map_err(Into::into)
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
    let mut packet_id = None;
    let mut pc = None;

    while let Some(field) = parser.next_field()? {
        match field.number {
            1 => tick = Some(field.varint("Packet", "tick")?),
            2 => command = Some(read_u32_field(field, "Packet", "cmd")?),
            3 => address = Some(field.varint("Packet", "addr")?),
            4 => size = Some(read_u32_field(field, "Packet", "size")?),
            5 => flags = read_u32_field(field, "Packet", "flags")?,
            6 => packet_id = Some(field.varint("Packet", "pkt_id")?),
            7 => pc = Some(Address::new(field.varint("Packet", "pc")?)),
            _ => {}
        }
        parser.skip(field)?;
    }

    let flags = TrafficTraceRequestFlags::from_gem5(flags)?;

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
        GEM5_LOAD_LOCKED_REQ => TrafficTraceCommand::LoadLocked,
        GEM5_STORE_COND_REQ => TrafficTraceCommand::StoreConditional,
        GEM5_STORE_COND_FAIL_REQ => TrafficTraceCommand::StoreConditionalFail,
        GEM5_LOCKED_RMW_READ_REQ => TrafficTraceCommand::LockedRmwRead,
        GEM5_LOCKED_RMW_WRITE_REQ => TrafficTraceCommand::LockedRmwWrite,
        GEM5_WRITE_REQ => TrafficTraceCommand::Write,
        GEM5_WRITEBACK_DIRTY => TrafficTraceCommand::WritebackDirty,
        GEM5_WRITEBACK_CLEAN => TrafficTraceCommand::WritebackClean,
        GEM5_WRITE_CLEAN => TrafficTraceCommand::WriteClean,
        GEM5_SWAP_REQ => TrafficTraceCommand::Swap,
        GEM5_CLEAN_EVICT => TrafficTraceCommand::CleanEvict,
        GEM5_WRITE_LINE_REQ => TrafficTraceCommand::WriteLine,
        GEM5_UPGRADE_REQ => TrafficTraceCommand::Upgrade,
        GEM5_SC_UPGRADE_REQ => TrafficTraceCommand::StoreConditionalUpgrade,
        GEM5_SC_UPGRADE_FAIL_REQ => TrafficTraceCommand::StoreConditionalUpgradeFail,
        GEM5_MEM_FENCE_REQ => TrafficTraceCommand::MemFence,
        GEM5_MEM_SYNC_REQ => TrafficTraceCommand::MemSync,
        GEM5_CLEAN_SHARED_REQ => TrafficTraceCommand::CleanShared,
        GEM5_CLEAN_INVALID_REQ => TrafficTraceCommand::CleanInvalid,
        GEM5_PRINT_REQ => TrafficTraceCommand::Print,
        GEM5_INVALIDATE_REQ => TrafficTraceCommand::Invalidate,
        GEM5_HTM_REQ => TrafficTraceCommand::HtmRequest,
        GEM5_HTM_ABORT => TrafficTraceCommand::HtmAbort,
        GEM5_TLBI_EXT_SYNC => TrafficTraceCommand::TlbiExtSync,
        command => return Err(TrafficGeneratorError::TraceUnsupportedCommand { command }),
    };
    flags.validate_for_command(command)?;
    if command.sync_kind().is_some() || command.tlb_kind().is_some() {
        return Ok(TrafficTraceElement {
            tick,
            command,
            address: None,
            size: None,
            flags,
            packet_id,
            pc,
        });
    }
    if command.htm_kind().is_some() || command.diagnostic_kind().is_some() {
        let size = match size {
            Some(0) | None => None,
            Some(size) => Some(AccessSize::new(u64::from(size))?),
        };
        return Ok(TrafficTraceElement {
            tick,
            command,
            address: address.map(Address::new),
            size,
            flags,
            packet_id,
            pc,
        });
    }

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
        address: Some(Address::new(address)),
        size: Some(AccessSize::new(u64::from(size))?),
        flags,
        packet_id,
        pc,
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
