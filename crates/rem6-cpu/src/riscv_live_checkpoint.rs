use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_isa_riscv::{FloatRegister, MemoryWidth};
use rem6_kernel::{PartitionId, ScheduledEventKind, Tick};
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::O3RenameMapEntry;

#[path = "riscv_live_checkpoint/codec.rs"]
mod codec;
#[path = "riscv_live_checkpoint/event.rs"]
mod event;

pub use event::RiscvO3LiveCheckpointEvent;

pub const RISCV_O3_LIVE_CHECKPOINT_CHUNK: &str = "o3-live-checkpoint";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveCheckpointProfile {
    ComputeQueue,
    CompletedFpLoad,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointIssueRow {
    pub sequence: u64,
    pub fetch_request: MemoryRequestId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointTelemetry {
    pub enqueued_rows: u64,
    pub service_turns: u64,
    pub wake_requests: u64,
    pub current_occupancy: u64,
    pub peak_occupancy: u64,
    pub scalar_integer_issued_rows: u64,
    pub integer_mul_div_issued_rows: u64,
    pub memory_agu_issued_rows: u64,
    pub control_issued_rows: u64,
    pub scalar_float_issued_rows: u64,
    pub vector_to_scalar_issued_rows: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointService {
    pub requested_tick: Tick,
    pub mutation_generation: u64,
    pub last_service_generation: u64,
    pub telemetry: RiscvO3LiveCheckpointTelemetry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointFinalizedWriteback {
    pub cycles: u64,
    pub admitted_rows: u64,
    pub deferred_rows: u64,
    pub deferred_row_cycles: u64,
    pub max_ready_rows_per_cycle: u64,
    pub max_deferred_rows: u64,
    pub partial_cycle_ticks: BTreeSet<Tick>,
    pub partial_ready_rows_by_tick: BTreeMap<Tick, u64>,
    pub partial_deferred_rows_by_tick: BTreeMap<Tick, u64>,
    pub closed_before_tick: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveCheckpointWritebackSource {
    FixedFunction,
    MemoryResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointReservation {
    pub sequence: u64,
    pub raw_ready_tick: Tick,
    pub admitted_tick: Tick,
    pub slot: u32,
    pub source: RiscvO3LiveCheckpointWritebackSource,
    pub decision_counted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointCompletedFpLoad {
    pub fetch_request: MemoryRequestId,
    pub data_request: MemoryRequestId,
    pub sequence: u64,
    pub lsq_sequence: u64,
    pub rob_first_sequence: u64,
    pub rob_last_sequence: u64,
    pub issue_tick: Tick,
    pub response_tick: Tick,
    pub raw_ready_tick: Tick,
    pub admitted_tick: Tick,
    pub latency_ticks: Tick,
    pub physical_address: Address,
    pub access_size: AccessSize,
    pub request_byte_offset: u32,
    pub response_bytes: Vec<u8>,
    pub destination: FloatRegister,
    pub width: MemoryWidth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointWake {
    pub scheduler_instance_raw: u64,
    pub partition: PartitionId,
    pub tick: Tick,
    pub scheduler_order: u64,
    pub kind: ScheduledEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveCheckpointPayload {
    pub profile: RiscvO3LiveCheckpointProfile,
    pub captured_tick: Tick,
    pub next_fetch_pc: Address,
    pub next_fetch_request_sequence: u64,
    pub events: Vec<RiscvO3LiveCheckpointEvent>,
    pub issue_rows: Vec<RiscvO3LiveCheckpointIssueRow>,
    pub rename_rows: Vec<O3RenameMapEntry>,
    pub resident_sequences: Vec<u64>,
    pub executed_fetch_requests: Vec<MemoryRequestId>,
    pub issued_fetch_requests: Vec<MemoryRequestId>,
    pub service: RiscvO3LiveCheckpointService,
    pub finalized_writeback: RiscvO3LiveCheckpointFinalizedWriteback,
    pub writeback_counted_sequences: Vec<u64>,
    pub writeback_published_sequences: Vec<u64>,
    pub reservation: Option<RiscvO3LiveCheckpointReservation>,
    pub completed_result: Option<RiscvO3LiveCheckpointCompletedFpLoad>,
    pub wake: RiscvO3LiveCheckpointWake,
}

impl RiscvO3LiveCheckpointPayload {
    pub fn encode(&self) -> Result<Vec<u8>, RiscvO3LiveCheckpointError> {
        codec::encode(self)
    }

    pub fn decode(payload: &[u8]) -> Result<Self, RiscvO3LiveCheckpointError> {
        codec::decode(payload)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveCheckpointError {
    InvalidMagic,
    UnsupportedVersion {
        version: u8,
    },
    UnsupportedProfile {
        profile: u8,
    },
    Truncated {
        field: &'static str,
    },
    TrailingBytes {
        remaining: usize,
    },
    ExcessiveCount {
        field: &'static str,
        count: u64,
        maximum: usize,
    },
    IntegerConversion {
        field: &'static str,
    },
    InvalidBoolean {
        field: &'static str,
        value: u8,
    },
    InvalidTag {
        field: &'static str,
        value: u8,
    },
    InvalidRegister {
        field: &'static str,
        index: u8,
    },
    InvalidField {
        field: &'static str,
        value: u64,
    },
    DuplicateValue {
        field: &'static str,
        value: u64,
    },
    InvalidEndpoint,
    InstructionDecode {
        raw: u32,
    },
    InstructionWidth {
        encoded: u8,
        decoded: u8,
    },
    InstructionMismatch,
    UnsupportedEvent {
        reason: &'static str,
    },
    InvalidProfileShape {
        reason: &'static str,
    },
}

impl fmt::Display for RiscvO3LiveCheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("O3 live checkpoint ")?;
        match self {
            Self::InvalidMagic => f.write_str("magic is invalid"),
            Self::UnsupportedVersion { version } => write!(f, "version {version} is unsupported"),
            Self::UnsupportedProfile { profile } => write!(f, "profile {profile} is unsupported"),
            Self::Truncated { field } => write!(f, "{field} is truncated"),
            Self::TrailingBytes { remaining } => write!(f, "has {remaining} trailing bytes"),
            Self::ExcessiveCount {
                field,
                count,
                maximum,
            } => write!(f, "{field} count {count} exceeds {maximum}"),
            Self::IntegerConversion { field } => write!(f, "{field} does not fit its host type"),
            Self::InvalidBoolean { field, value } => {
                write!(f, "{field} boolean value {value} is invalid")
            }
            Self::InvalidTag { field, value } => write!(f, "{field} tag {value} is invalid"),
            Self::InvalidRegister { field, index } => {
                write!(f, "{field} register {index} is invalid")
            }
            Self::InvalidField { field, value } => write!(f, "{field} value {value} is invalid"),
            Self::DuplicateValue { field, value } => {
                write!(f, "{field} value {value} is duplicated")
            }
            Self::InvalidEndpoint => f.write_str("endpoint is invalid"),
            Self::InstructionDecode { raw } => write!(f, "instruction 0x{raw:08x} does not decode"),
            Self::InstructionWidth { encoded, decoded } => write!(
                f,
                "instruction width {encoded} does not match decoded width {decoded}"
            ),
            Self::InstructionMismatch => f.write_str("instruction projection is inconsistent"),
            Self::UnsupportedEvent { reason } => write!(f, "event is unsupported: {reason}"),
            Self::InvalidProfileShape { reason } => write!(f, "profile is invalid: {reason}"),
        }
    }
}

impl Error for RiscvO3LiveCheckpointError {}
