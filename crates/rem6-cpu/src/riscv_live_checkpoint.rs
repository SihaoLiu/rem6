use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_isa_riscv::{FloatRegister, MemoryWidth};
use rem6_kernel::{PartitionId, ScheduledEventKind, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, MemoryRequestId};

use crate::{O3RenameMapEntry, RiscvCore, RiscvCoreState};

#[path = "riscv_live_checkpoint/codec.rs"]
mod codec;
#[path = "riscv_live_checkpoint/event.rs"]
mod event;
#[path = "riscv_live_checkpoint/fetch.rs"]
mod fetch;
#[path = "riscv_live_checkpoint/pending_address.rs"]
mod pending_address;

pub use event::RiscvO3LiveCheckpointEvent;
use fetch::project_live_fetches;
pub use pending_address::RiscvO3LiveCheckpointPendingDataAddress;
pub(crate) use pending_address::MAX_PENDING_ADDRESSES;

pub const RISCV_O3_LIVE_CHECKPOINT_CHUNK: &str = "o3-live-checkpoint";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveCheckpointCapture {
    Absent,
    Captured(RiscvO3LiveCheckpointPayload),
    Rejected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3CheckpointProjection {
    stable: crate::O3RuntimeCheckpointPayload,
    stable_capture_quiescent: bool,
    live: RiscvO3LiveCheckpointCapture,
    replay: crate::RiscvCoreCheckpointRestoreInput,
}

impl RiscvO3CheckpointProjection {
    pub const fn stable(&self) -> &crate::O3RuntimeCheckpointPayload {
        &self.stable
    }

    pub const fn live_capture(&self) -> &RiscvO3LiveCheckpointCapture {
        &self.live
    }

    #[doc(hidden)]
    pub const fn stable_capture_is_quiescent(&self) -> bool {
        self.stable_capture_quiescent
    }

    #[doc(hidden)]
    pub const fn replay(&self) -> &crate::RiscvCoreCheckpointRestoreInput {
        &self.replay
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvO3LiveCheckpointProfile {
    ComputeQueue,
    CompletedFpLoad,
    PendingDataAddress,
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
    pub last_service_generation: Option<(Tick, u64)>,
    pub telemetry: RiscvO3LiveCheckpointTelemetry,
}

impl RiscvO3LiveCheckpointService {
    pub(crate) fn has_invalid_identity_at(self, captured_tick: Tick) -> bool {
        self.last_service_generation
            .is_some_and(|(tick, generation)| {
                tick > captured_tick
                    || generation > self.mutation_generation
                    || (tick, generation) == (self.requested_tick, self.mutation_generation)
            })
    }
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

impl RiscvO3LiveCheckpointFinalizedWriteback {
    pub(crate) fn is_valid_without_live_calendar_at(&self, captured_tick: Tick) -> bool {
        self.is_valid_without_live_calendar(captured_tick, captured_tick)
    }

    pub(crate) fn is_valid_without_live_calendar_closed_through(
        &self,
        captured_tick: Tick,
    ) -> bool {
        captured_tick
            .checked_add(1)
            .is_some_and(|closed_before_limit| {
                self.is_valid_without_live_calendar(captured_tick, closed_before_limit)
            })
    }

    fn is_valid_without_live_calendar(
        &self,
        captured_tick: Tick,
        closed_before_limit: Tick,
    ) -> bool {
        let tick_is_reopenable =
            |tick: &Tick| self.closed_before_tick <= *tick && *tick <= captured_tick;
        self.closed_before_tick <= closed_before_limit
            && self.partial_cycle_ticks.iter().all(tick_is_reopenable)
            && self.partial_ready_rows_by_tick.iter().all(|(tick, rows)| {
                *rows > 0 && self.partial_cycle_ticks.contains(tick) && tick_is_reopenable(tick)
            })
            && self
                .partial_deferred_rows_by_tick
                .iter()
                .all(|(tick, rows)| {
                    *rows > 0 && self.partial_cycle_ticks.contains(tick) && tick_is_reopenable(tick)
                })
    }
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
    pub pending_addresses: Vec<RiscvO3LiveCheckpointPendingDataAddress>,
    pub wake: RiscvO3LiveCheckpointWake,
}

impl RiscvO3LiveCheckpointPayload {
    pub fn encode(&self) -> Result<Vec<u8>, RiscvO3LiveCheckpointError> {
        codec::encode(self)
    }

    pub fn decode(payload: &[u8]) -> Result<Self, RiscvO3LiveCheckpointError> {
        Self::decode_versioned(payload).map(|(_, checkpoint)| checkpoint)
    }

    pub fn decode_versioned(payload: &[u8]) -> Result<(u8, Self), RiscvO3LiveCheckpointError> {
        codec::decode_versioned(payload)
    }

    #[cfg(test)]
    pub(crate) fn encode_without_validation_for_test(
        &self,
    ) -> Result<Vec<u8>, RiscvO3LiveCheckpointError> {
        codec::encode_without_validation(self)
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

impl RiscvCore {
    pub fn capture_checkpoint_projection(
        &self,
        captured_tick: Tick,
    ) -> RiscvO3CheckpointProjection {
        let core_agent = self.agent();
        let cpu_state = self.core.state.lock().expect("cpu core lock");
        let riscv_state = self.state.lock().expect("riscv core lock");
        let projected_cpu = crate::CpuCore::from_checkpoint_state(
            crate::CpuCore::checkpoint_state_from_guard(&cpu_state),
        );
        let projected = RiscvCore {
            core: projected_cpu,
            state: Arc::new(Mutex::new(riscv_state.clone())),
        };
        drop(riscv_state);
        drop(cpu_state);
        let has_pending_address = projected
            .state
            .lock()
            .expect("riscv core lock")
            .o3_runtime
            .has_pending_data_address();
        if has_pending_address {
            projected.record_ready_o3_data_access_event_with_trace(captured_tick, false);
        }
        let projected_cpu_state = projected.core.state.lock().expect("cpu core lock");
        let mut projected_state = projected.state.lock().expect("riscv core lock").clone();
        projected_state.finalize_quiescent_o3_writeback_state_for_checkpoint();
        capture_checkpoint_projection_from_guards(
            &projected,
            &projected_cpu_state,
            &projected_state,
            core_agent,
            captured_tick,
        )
    }

    #[doc(hidden)]
    pub fn capture_stable_checkpoint_replay(&self) -> crate::RiscvCoreCheckpointRestoreInput {
        let _cpu_state = self.core.state.lock().expect("cpu core lock");
        let state = self.state.lock().expect("riscv core lock");
        let mut projected_state = state.clone();
        projected_state.finalize_quiescent_o3_writeback_state_for_checkpoint();
        checkpoint_replay_from_guarded_state(
            self,
            &projected_state,
            stable_projection(&projected_state),
            None,
            None,
        )
    }

    #[doc(hidden)]
    pub fn checkpoint_projection_data_access_lifecycle_is_quiescent(&self) -> bool {
        let mut projected_state = self.state.lock().expect("riscv core lock").clone();
        projected_state.finalize_quiescent_o3_writeback_state_for_checkpoint();
        projected_state.data_access_lifecycle_is_quiescent()
    }
}

impl RiscvCoreState {
    pub(crate) fn data_access_lifecycle_is_quiescent(&self) -> bool {
        self.pending_callback_error.is_none()
            && self.o3_runtime.live_data_access_lifecycle_is_quiescent()
            && self.o3_runtime.live_issue_is_quiescent()
            && !self.o3_runtime.has_pending_retirement_authority()
            && !self.o3_writeback_wake.has_pending_checkpoint_authority()
            && self.outstanding_data.is_empty()
            && self.buffered_o3_effects.is_empty()
            && self.pending_data_translations.is_empty()
            && self.ready_translated_data.is_empty()
            && self.memory_result_window_authorizations.is_empty()
            && self
                .data_translation
                .as_ref()
                .is_none_or(|frontend| frontend.is_empty())
            && self.events.iter().all(|event| {
                event.execution().memory_access().is_none()
                    || self
                        .issued_data_for_fetches
                        .contains(&event.fetch().request_id())
            })
    }
}

fn capture_checkpoint_projection_from_guards(
    core: &RiscvCore,
    cpu: &crate::cpu_core::CpuCoreState,
    state: &RiscvCoreState,
    core_agent: rem6_memory::AgentId,
    captured_tick: Tick,
) -> RiscvO3CheckpointProjection {
    let mut stable = stable_projection(state);
    let mut projected_hart = None;
    let live = match capture_live_from_guards(&cpu, state, core_agent, captured_tick) {
        Ok(Some((capture, projected_stable, hart))) => {
            stable = projected_stable.with_live_retire_gate(state.live_retire_gate.checkpoint());
            projected_hart = hart;
            RiscvO3LiveCheckpointCapture::Captured(capture)
        }
        Ok(None) => RiscvO3LiveCheckpointCapture::Absent,
        Err(_) => RiscvO3LiveCheckpointCapture::Rejected,
    };
    let captured = match &live {
        RiscvO3LiveCheckpointCapture::Captured(live) => Some(live.clone()),
        _ => None,
    };
    let replay =
        checkpoint_replay_from_guarded_state(core, state, stable.clone(), captured, projected_hart);
    RiscvO3CheckpointProjection {
        stable,
        stable_capture_quiescent: state.data_access_lifecycle_is_quiescent(),
        live,
        replay,
    }
}

fn stable_projection(state: &RiscvCoreState) -> crate::O3RuntimeCheckpointPayload {
    state
        .o3_runtime
        .checkpoint_payload_with_projected_stats(state.o3_runtime.stats())
        .with_live_retire_gate(state.live_retire_gate.checkpoint())
}

fn checkpoint_replay_from_guarded_state(
    core: &RiscvCore,
    state: &RiscvCoreState,
    stable: crate::O3RuntimeCheckpointPayload,
    live: Option<RiscvO3LiveCheckpointPayload>,
    projected_hart: Option<rem6_isa_riscv::RiscvHartState>,
) -> crate::RiscvCoreCheckpointRestoreInput {
    let mut checkpoint_state = state.clone();
    if let Some(hart) = projected_hart {
        checkpoint_state.hart = hart;
    }
    let snapshot = RiscvCore {
        core: core.core.clone(),
        state: Arc::new(Mutex::new(checkpoint_state)),
    };
    crate::RiscvCoreCheckpointRestoreInput::new(
        snapshot.checkpoint_hart_state(),
        snapshot.pmp_snapshot(),
        snapshot.hart_run_state(),
        snapshot.in_order_pipeline_snapshot(),
        snapshot.branch_predictor_checkpoint_payload(),
        snapshot.gshare_branch_predictor_checkpoint_payload(),
        snapshot.bimode_branch_predictor_checkpoint_payload(),
        snapshot.tournament_branch_predictor_checkpoint_payload(),
        snapshot.tage_sc_l_branch_predictor_checkpoint_payload(),
        snapshot.multiperspective_perceptron_checkpoint_payload(),
        stable.clone(),
        live,
    )
}

fn capture_live_from_guards(
    cpu: &crate::cpu_core::CpuCoreState,
    state: &RiscvCoreState,
    core_agent: rem6_memory::AgentId,
    captured_tick: Tick,
) -> Result<
    Option<(
        RiscvO3LiveCheckpointPayload,
        crate::O3RuntimeCheckpointPayload,
        Option<rem6_isa_riscv::RiscvHartState>,
    )>,
    RiscvO3LiveCheckpointError,
> {
    let pending_issued = cpu.events().iter().any(|issued| {
        issued.kind() == crate::CpuFetchEventKind::Issued
            && !cpu.events().iter().any(|event| {
                event.request_id() == issued.request_id()
                    && event.kind() != crate::CpuFetchEventKind::Issued
            })
    });
    if cpu.has_outstanding_fetch() || pending_issued || state.pending_fetch_prefix.is_some() {
        return Err(invalid("unsupported instruction fetch authority"));
    }
    let runtime = match state
        .o3_runtime
        .compute_checkpoint_projection(captured_tick)?
    {
        Some(runtime) => runtime,
        None => {
            if state.data_access_lifecycle_is_quiescent() {
                return Ok(None);
            }
            return Err(invalid("drained capture retains operational authority"));
        }
    };
    let completed_result = runtime.completed_result.as_ref();
    let pending_terminal_matches_completed = state
        .pending_terminal_memory_result
        .as_ref()
        .is_none_or(|pending| {
            let Some(result) = completed_result else {
                return false;
            };
            let event = pending.execution();
            pending.issue_ready()
                && pending.consumed_requests() == [result.fetch_request]
                && pending.decoded().instruction() == event.instruction()
                && event.fetch().request_id() == result.fetch_request
                && state.hart.pc() == event.fetch_pc().get()
                && event.execution().memory_access()
                    == Some(&rem6_isa_riscv::MemoryAccessKind::FloatLoad {
                        rd: result.destination,
                        address: result.physical_address.get(),
                        width: result.width,
                    })
                && event.data_access_event_kind()
                    == Some(crate::RiscvDataAccessEventKind::Completed)
                && event.counts_as_retired_instruction()
        });
    let projected_pending_terminal_fetch = pending_terminal_matches_completed
        .then(|| {
            state
                .pending_terminal_memory_result
                .as_ref()
                .map(|pending| pending.execution().fetch().request_id())
        })
        .flatten();
    let authorization_matches = match completed_result {
        Some(result) => {
            let Ok(range) = AddressRange::new(result.physical_address, result.access_size) else {
                return Err(invalid("completed result authorization range is invalid"));
            };
            state.memory_result_window_authorizations.is_empty()
                || (state.memory_result_window_authorizations.len() == 1
                    && state
                        .memory_result_window_authorizations
                        .get(&result.fetch_request)
                        .is_some_and(|authorization| {
                            authorization.integer_destination().is_none()
                                && authorization.route()
                                    == crate::riscv_fetch_ahead::O3MemoryResultWindowRoute::Memory
                                && authorization.role()
                                    == crate::riscv_fetch_ahead::O3MemoryResultWindowRole::Head
                                && authorization.resolved_range() == Some(range)
                        }))
        }
        None => state.memory_result_window_authorizations.is_empty(),
    };
    if !pending_terminal_matches_completed
        || !state.pending_data_translations.is_empty()
        || !state.ready_translated_data.is_empty()
        || state
            .data_translation
            .as_ref()
            .is_some_and(|frontend| !frontend.is_empty())
        || !state.outstanding_data.is_empty()
        || state.next_unissued_data_access().is_some()
        || !state.buffered_o3_effects.is_empty()
        || !state.translated_scalar_load_window_fetches.is_empty()
        || !authorization_matches
        || state.pending_trap.is_some()
        || state.pending_trap_event.is_some()
        || state.htm_hart_checkpoint.is_some()
        || !state.branch_speculations.is_empty()
        || !state.branch_speculation_kinds.is_empty()
        || !state.return_address_stack_operations.is_empty()
        || state.producer_forwarded_scalar_continuation.is_some()
        || !state.selected_branch_speculations.is_empty()
        || !state.branch_target_predictions.is_empty()
        || state.live_retire_gate.checkpoint().is_some()
        || state.pending_in_order_pipeline_wake.is_some()
        || !state.detached_in_order_pipeline_wakes.is_empty()
        || state.pending_in_order_pipeline_advance.is_some()
        || !state.rebound_in_order_execute_waits.is_empty()
        || !state.o3_force_normal_execute_fetches.is_empty()
        || state.pending_callback_error.is_some()
        || state.reservation.is_some()
    {
        return Err(invalid("unsupported RISC-V core authority"));
    }
    let mut owner_rows = completed_result
        .iter()
        .map(|result| (result.sequence, result.fetch_request))
        .chain(
            runtime
                .finalized_rows
                .iter()
                .map(|row| (row.sequence, row.fetch_request)),
        )
        .chain(
            runtime
                .issue_rows
                .iter()
                .map(|row| (row.sequence, row.fetch_request)),
        )
        .collect::<Vec<_>>();
    owner_rows.sort_by_key(|(sequence, _)| *sequence);
    let expected_requests = owner_rows
        .iter()
        .map(|(_, request)| *request)
        .collect::<Vec<_>>();
    if completed_result.is_some_and(|result| {
        result.data_request.agent() != core_agent
            || result.data_request.sequence() <= result.fetch_request.sequence()
            || result.data_request.sequence() >= cpu.next_sequence()
            || expected_requests.contains(&result.data_request)
    }) {
        return Err(invalid("completed FP data request identity is invalid"));
    }
    let wake = state
        .o3_writeback_wake
        .checkpoint_scheduled_wake()
        .ok_or(invalid("live queue does not own exactly one attached wake"))?;
    if wake.tick() != runtime.service.requested_tick {
        return Err(invalid("scheduled wake does not match queue service"));
    }
    if expected_requests
        .iter()
        .any(|request| request.sequence() >= cpu.next_sequence())
    {
        return Err(invalid(
            "next fetch sequence does not follow restored requests",
        ));
    }
    if !runtime.pending_addresses.is_empty()
        && runtime
            .pending_addresses
            .iter()
            .filter_map(|pending| pending.requested_wake_tick)
            .min()
            != Some(runtime.service.requested_tick)
    {
        return Err(invalid("pending-address wake authority is inconsistent"));
    }
    let pending_addresses =
        (!runtime.pending_addresses.is_empty()).then_some(runtime.pending_addresses.as_slice());
    let fetch_projection = project_live_fetches(
        cpu.events(),
        state,
        &owner_rows,
        &expected_requests,
        completed_result,
        projected_pending_terminal_fetch,
        pending_addresses,
        wake.event().partition(),
    )?;
    let projected_stable = runtime.stable.clone();
    let next_fetch_pc = runtime
        .pending_addresses
        .last()
        .map(|pending| Address::new(pending.fetch.pc().get().saturating_add(4)))
        .unwrap_or_else(|| cpu.pc());
    let pending_addresses = runtime.pending_addresses;
    Ok(Some((
        RiscvO3LiveCheckpointPayload {
            profile: runtime.profile,
            captured_tick,
            next_fetch_pc,
            next_fetch_request_sequence: cpu.next_sequence(),
            events: fetch_projection.events,
            issue_rows: runtime.issue_rows,
            rename_rows: runtime.rename_rows,
            resident_sequences: runtime.resident_sequences,
            executed_fetch_requests: fetch_projection.executed_fetch_requests,
            issued_fetch_requests: fetch_projection.issued_fetch_requests,
            service: runtime.service,
            finalized_writeback: runtime.finalized_writeback,
            writeback_counted_sequences: runtime.writeback_counted_sequences,
            writeback_published_sequences: runtime.writeback_published_sequences,
            reservation: runtime.reservation,
            completed_result: runtime.completed_result,
            pending_addresses,
            wake: RiscvO3LiveCheckpointWake {
                scheduler_instance_raw: wake.scheduler().checkpoint_raw(),
                partition: wake.event().partition(),
                tick: wake.tick(),
                scheduler_order: wake.event().order(),
                kind: wake.event().kind(),
            },
        },
        projected_stable,
        fetch_projection.projected_hart,
    )))
}

fn project_event(event: &crate::RiscvCpuExecutionEvent) -> RiscvO3LiveCheckpointEvent {
    let execution = event.execution();
    RiscvO3LiveCheckpointEvent {
        fetch: event.fetch().clone(),
        execution_pc: execution.pc(),
        next_pc: execution.next_pc(),
        instruction_bytes: execution.instruction_bytes(),
        register_writes: execution.register_writes().to_vec(),
        float_register_writes: execution.float_register_writes().to_vec(),
        memory_access: execution.memory_access().cloned(),
        data_access_event_kind: event.data_access_event_kind(),
        counts_as_retired_instruction: event.counts_as_retired_instruction(),
    }
}

fn invalid(reason: &'static str) -> RiscvO3LiveCheckpointError {
    RiscvO3LiveCheckpointError::InvalidProfileShape { reason }
}
