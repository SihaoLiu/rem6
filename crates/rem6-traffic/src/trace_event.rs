use rem6_memory::{AccessSize, Address};

use crate::common::TrafficRequestEvent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceSyncKind {
    MemFence,
    MemSync,
}

impl TrafficTraceSyncKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::MemFence => "MemFenceReq",
            Self::MemSync => "MemSyncReq",
        }
    }

    pub const fn is_request(self) -> bool {
        match self {
            Self::MemFence | Self::MemSync => true,
        }
    }

    pub const fn requires_response(self) -> bool {
        match self {
            Self::MemFence | Self::MemSync => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceTlbKind {
    ExternalSync,
}

impl TrafficTraceTlbKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::ExternalSync => "TlbiExtSync",
        }
    }

    pub const fn is_request(self) -> bool {
        match self {
            Self::ExternalSync => true,
        }
    }

    pub const fn requires_response(self) -> bool {
        match self {
            Self::ExternalSync => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceCacheKind {
    Flush,
}

impl TrafficTraceCacheKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::Flush => "FlushReq",
        }
    }

    pub const fn is_request(self) -> bool {
        match self {
            Self::Flush => true,
        }
    }

    pub const fn is_flush(self) -> bool {
        match self {
            Self::Flush => true,
        }
    }

    pub const fn requires_writable(self) -> bool {
        match self {
            Self::Flush => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceHtmKind {
    Request,
    Abort,
}

impl TrafficTraceHtmKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::Request => "HTMReq",
            Self::Abort => "HTMAbort",
        }
    }

    pub const fn is_request(self) -> bool {
        match self {
            Self::Request | Self::Abort => true,
        }
    }

    pub const fn is_read(self) -> bool {
        match self {
            Self::Request | Self::Abort => true,
        }
    }

    pub const fn requires_response(self) -> bool {
        match self {
            Self::Request => true,
            Self::Abort => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceDiagnosticKind {
    Print,
}

impl TrafficTraceDiagnosticKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::Print => "PrintReq",
        }
    }

    pub const fn is_request(self) -> bool {
        match self {
            Self::Print => true,
        }
    }

    pub const fn is_print(self) -> bool {
        match self {
            Self::Print => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceResponseKind {
    Read,
    ReadWithInvalidate,
    Write,
    WriteComplete,
    SoftPrefetch,
    HardPrefetch,
    Upgrade,
    UpgradeFail,
    ReadExclusive,
    StoreConditional,
    LockedRmwRead,
    LockedRmwWrite,
    Swap,
    MemSync,
    MemFence,
    CleanShared,
    CleanInvalid,
    Invalidate,
    HtmRequest,
}

impl TrafficTraceResponseKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::Read => "ReadResp",
            Self::ReadWithInvalidate => "ReadRespWithInvalidate",
            Self::Write => "WriteResp",
            Self::WriteComplete => "WriteCompleteResp",
            Self::SoftPrefetch => "SoftPFResp",
            Self::HardPrefetch => "HardPFResp",
            Self::Upgrade => "UpgradeResp",
            Self::UpgradeFail => "UpgradeFailResp",
            Self::ReadExclusive => "ReadExResp",
            Self::StoreConditional => "StoreCondResp",
            Self::LockedRmwRead => "LockedRMWReadResp",
            Self::LockedRmwWrite => "LockedRMWWriteResp",
            Self::Swap => "SwapResp",
            Self::MemSync => "MemSyncResp",
            Self::MemFence => "MemFenceResp",
            Self::CleanShared => "CleanSharedResp",
            Self::CleanInvalid => "CleanInvalidResp",
            Self::Invalidate => "InvalidateResp",
            Self::HtmRequest => "HTMReqResp",
        }
    }

    pub const fn returns_data(self) -> bool {
        matches!(
            self,
            Self::Read
                | Self::ReadWithInvalidate
                | Self::SoftPrefetch
                | Self::HardPrefetch
                | Self::UpgradeFail
                | Self::ReadExclusive
                | Self::LockedRmwRead
                | Self::Swap
        )
    }

    pub const fn is_read(self) -> bool {
        matches!(
            self,
            Self::Read
                | Self::ReadWithInvalidate
                | Self::SoftPrefetch
                | Self::HardPrefetch
                | Self::UpgradeFail
                | Self::ReadExclusive
                | Self::LockedRmwRead
                | Self::Swap
                | Self::HtmRequest
        )
    }

    pub const fn is_write(self) -> bool {
        matches!(
            self,
            Self::Write
                | Self::WriteComplete
                | Self::StoreConditional
                | Self::LockedRmwWrite
                | Self::Swap
        )
    }

    pub const fn invalidates_line(self) -> bool {
        matches!(
            self,
            Self::ReadWithInvalidate | Self::CleanInvalid | Self::Invalidate
        )
    }

    pub const fn cleans_line(self) -> bool {
        matches!(self, Self::CleanShared | Self::CleanInvalid)
    }

    pub const fn is_software_prefetch(self) -> bool {
        matches!(self, Self::SoftPrefetch)
    }

    pub const fn is_hardware_prefetch(self) -> bool {
        matches!(self, Self::HardPrefetch)
    }

    pub const fn is_prefetch(self) -> bool {
        self.is_software_prefetch() || self.is_hardware_prefetch()
    }

    pub const fn is_upgrade(self) -> bool {
        matches!(self, Self::Upgrade)
    }

    pub const fn is_llsc(self) -> bool {
        matches!(self, Self::StoreConditional)
    }

    pub const fn is_locked_rmw(self) -> bool {
        matches!(self, Self::LockedRmwRead | Self::LockedRmwWrite)
    }

    pub const fn requires_writable(self) -> bool {
        matches!(self, Self::LockedRmwRead | Self::LockedRmwWrite)
    }

    pub const fn carries_writable_intent(self) -> bool {
        self.is_write()
            || self.requires_writable()
            || matches!(self, Self::Upgrade | Self::ReadExclusive)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceErrorKind {
    InvalidDestination,
    BadAddress,
    Read,
    Write,
    FunctionalRead,
    FunctionalWrite,
}

impl TrafficTraceErrorKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::InvalidDestination => "InvalidDestError",
            Self::BadAddress => "BadAddressError",
            Self::Read => "ReadError",
            Self::Write => "WriteError",
            Self::FunctionalRead => "FunctionalReadError",
            Self::FunctionalWrite => "FunctionalWriteError",
        }
    }

    pub const fn is_response(self) -> bool {
        match self {
            Self::InvalidDestination
            | Self::BadAddress
            | Self::Read
            | Self::Write
            | Self::FunctionalRead
            | Self::FunctionalWrite => true,
        }
    }

    pub const fn is_error(self) -> bool {
        match self {
            Self::InvalidDestination
            | Self::BadAddress
            | Self::Read
            | Self::Write
            | Self::FunctionalRead
            | Self::FunctionalWrite => true,
        }
    }

    pub const fn is_read(self) -> bool {
        matches!(self, Self::Read | Self::FunctionalRead)
    }

    pub const fn is_write(self) -> bool {
        matches!(self, Self::Write | Self::FunctionalWrite)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceSyncEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceSyncKind,
    kernel_sync: bool,
    invalidates_l1: bool,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceSyncEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceSyncKind,
        kernel_sync: bool,
        invalidates_l1: bool,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            kernel_sync,
            invalidates_l1,
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceSyncKind {
        self.kind
    }

    pub const fn kernel_sync(self) -> bool {
        self.kernel_sync
    }

    pub const fn invalidates_l1(self) -> bool {
        self.invalidates_l1
    }

    pub const fn is_request(self) -> bool {
        self.kind.is_request()
    }

    pub const fn requires_response(self) -> bool {
        self.kind.requires_response()
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceTlbEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceTlbKind,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceTlbEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceTlbKind,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceTlbKind {
        self.kind
    }

    pub const fn is_request(self) -> bool {
        self.kind.is_request()
    }

    pub const fn requires_response(self) -> bool {
        self.kind.requires_response()
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceCacheEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceCacheKind,
    address: Address,
    size_bytes: u64,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceCacheEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceCacheKind,
        address: Address,
        size: AccessSize,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            address,
            size_bytes: size.bytes(),
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceCacheKind {
        self.kind
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn size_bytes(self) -> u64 {
        self.size_bytes
    }

    pub const fn is_request(self) -> bool {
        self.kind.is_request()
    }

    pub const fn is_flush(self) -> bool {
        self.kind.is_flush()
    }

    pub const fn requires_writable(self) -> bool {
        self.kind.requires_writable()
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceDiagnosticEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceDiagnosticKind,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceDiagnosticEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceDiagnosticKind,
        address: Option<Address>,
        size: Option<AccessSize>,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            address,
            size_bytes: match size {
                Some(size) => Some(size.bytes()),
                None => None,
            },
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceDiagnosticKind {
        self.kind
    }

    pub const fn address(self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn is_request(self) -> bool {
        self.kind.is_request()
    }

    pub const fn is_print(self) -> bool {
        self.kind.is_print()
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceResponseEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceResponseKind,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceResponseEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceResponseKind,
        address: Option<Address>,
        size: Option<AccessSize>,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            address,
            size_bytes: match size {
                Some(size) => Some(size.bytes()),
                None => None,
            },
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceResponseKind {
        self.kind
    }

    pub const fn address(self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn returns_data(self) -> bool {
        self.kind.returns_data()
    }

    pub const fn is_read(self) -> bool {
        self.kind.is_read()
    }

    pub const fn is_write(self) -> bool {
        self.kind.is_write()
    }

    pub const fn invalidates_line(self) -> bool {
        self.kind.invalidates_line()
    }

    pub const fn cleans_line(self) -> bool {
        self.kind.cleans_line()
    }

    pub const fn is_software_prefetch(self) -> bool {
        self.kind.is_software_prefetch()
    }

    pub const fn is_hardware_prefetch(self) -> bool {
        self.kind.is_hardware_prefetch()
    }

    pub const fn is_prefetch(self) -> bool {
        self.kind.is_prefetch()
    }

    pub const fn is_upgrade(self) -> bool {
        self.kind.is_upgrade()
    }

    pub const fn is_llsc(self) -> bool {
        self.kind.is_llsc()
    }

    pub const fn is_locked_rmw(self) -> bool {
        self.kind.is_locked_rmw()
    }

    pub const fn requires_writable(self) -> bool {
        self.kind.requires_writable()
    }

    pub const fn carries_writable_intent(self) -> bool {
        self.kind.carries_writable_intent()
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceErrorEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceErrorKind,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceErrorEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceErrorKind,
        address: Option<Address>,
        size: Option<AccessSize>,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            address,
            size_bytes: match size {
                Some(size) => Some(size.bytes()),
                None => None,
            },
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceErrorKind {
        self.kind
    }

    pub const fn address(self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn is_response(self) -> bool {
        self.kind.is_response()
    }

    pub const fn is_error(self) -> bool {
        self.kind.is_error()
    }

    pub const fn is_read(self) -> bool {
        self.kind.is_read()
    }

    pub const fn is_write(self) -> bool {
        self.kind.is_write()
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceHtmEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceHtmKind,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceHtmEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceHtmKind,
        address: Option<Address>,
        size: Option<AccessSize>,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            address,
            size_bytes: match size {
                Some(size) => Some(size.bytes()),
                None => None,
            },
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceHtmKind {
        self.kind
    }

    pub const fn address(self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn is_request(self) -> bool {
        self.kind.is_request()
    }

    pub const fn is_read(self) -> bool {
        self.kind.is_read()
    }

    pub const fn requires_response(self) -> bool {
        self.kind.requires_response()
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceEvent {
    Request(TrafficRequestEvent),
    Sync(TrafficTraceSyncEvent),
    Tlb(TrafficTraceTlbEvent),
    Cache(TrafficTraceCacheEvent),
    Htm(TrafficTraceHtmEvent),
    Diagnostic(TrafficTraceDiagnosticEvent),
    Response(TrafficTraceResponseEvent),
    Error(TrafficTraceErrorEvent),
}

impl TrafficTraceEvent {
    pub const fn tick(&self) -> u64 {
        match self {
            Self::Request(event) => event.tick(),
            Self::Sync(event) => event.tick(),
            Self::Tlb(event) => event.tick(),
            Self::Cache(event) => event.tick(),
            Self::Htm(event) => event.tick(),
            Self::Diagnostic(event) => event.tick(),
            Self::Response(event) => event.tick(),
            Self::Error(event) => event.tick(),
        }
    }

    pub const fn sequence(&self) -> u64 {
        match self {
            Self::Request(event) => event.sequence(),
            Self::Sync(event) => event.sequence(),
            Self::Tlb(event) => event.sequence(),
            Self::Cache(event) => event.sequence(),
            Self::Htm(event) => event.sequence(),
            Self::Diagnostic(event) => event.sequence(),
            Self::Response(event) => event.sequence(),
            Self::Error(event) => event.sequence(),
        }
    }
}
