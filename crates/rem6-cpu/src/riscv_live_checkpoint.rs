use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_isa_riscv::{FloatRegister, MemoryWidth};
use rem6_kernel::{PartitionId, ScheduledEventKind, Tick};
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{O3RenameMapEntry, RiscvCore, RiscvCoreState};

#[path = "riscv_live_checkpoint/codec.rs"]
mod codec;
#[path = "riscv_live_checkpoint/event.rs"]
mod event;

pub use event::RiscvO3LiveCheckpointEvent;

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
    pub const fn replay(&self) -> &crate::RiscvCoreCheckpointRestoreInput {
        &self.replay
    }
}

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
        let tick_is_reopenable =
            |tick: &Tick| self.closed_before_tick <= *tick && *tick <= captured_tick;
        self.closed_before_tick <= captured_tick
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
    pub wake: RiscvO3LiveCheckpointWake,
}

impl RiscvO3LiveCheckpointPayload {
    pub fn encode(&self) -> Result<Vec<u8>, RiscvO3LiveCheckpointError> {
        codec::encode(self)
    }

    pub fn decode(payload: &[u8]) -> Result<Self, RiscvO3LiveCheckpointError> {
        codec::decode(payload)
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
        let cpu_state = self.core.state.lock().expect("cpu core lock");
        let riscv_state = self.state.lock().expect("riscv core lock");
        capture_checkpoint_projection_from_guards(self, &cpu_state, &riscv_state, captured_tick)
    }

    #[doc(hidden)]
    pub fn capture_stable_checkpoint_replay(&self) -> crate::RiscvCoreCheckpointRestoreInput {
        let _cpu_state = self.core.state.lock().expect("cpu core lock");
        let state = self.state.lock().expect("riscv core lock");
        checkpoint_replay_from_guarded_state(self, &state, stable_projection(&state), None)
    }
}

fn capture_checkpoint_projection_from_guards(
    core: &RiscvCore,
    cpu: &crate::cpu_core::CpuCoreState,
    state: &RiscvCoreState,
    captured_tick: Tick,
) -> RiscvO3CheckpointProjection {
    let stable = stable_projection(state);
    let live = capture_live_from_guards(&cpu, state, captured_tick).map_or(
        RiscvO3LiveCheckpointCapture::Rejected,
        |capture| {
            capture.map_or(
                RiscvO3LiveCheckpointCapture::Absent,
                RiscvO3LiveCheckpointCapture::Captured,
            )
        },
    );
    let captured = match &live {
        RiscvO3LiveCheckpointCapture::Captured(live) => Some(live.clone()),
        _ => None,
    };
    let replay = checkpoint_replay_from_guarded_state(core, state, stable.clone(), captured);
    RiscvO3CheckpointProjection {
        stable,
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
) -> crate::RiscvCoreCheckpointRestoreInput {
    let snapshot = RiscvCore {
        core: core.core.clone(),
        state: Arc::new(Mutex::new(state.clone())),
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
    captured_tick: Tick,
) -> Result<Option<RiscvO3LiveCheckpointPayload>, RiscvO3LiveCheckpointError> {
    let runtime = match state.o3_runtime.compute_checkpoint_projection()? {
        Some(runtime) => runtime,
        None => {
            let data_quiescent = state.pending_callback_error.is_none()
                && state.o3_runtime.live_data_access_lifecycle_is_quiescent()
                && state.o3_runtime.live_issue_is_quiescent()
                && !state.o3_runtime.has_pending_retirement_authority()
                && !state.o3_writeback_wake.has_pending_checkpoint_authority()
                && state.outstanding_data.is_empty()
                && state.buffered_o3_effects.is_empty()
                && state.pending_data_translations.is_empty()
                && state.ready_translated_data.is_empty()
                && state.memory_result_window_authorizations.is_empty()
                && state
                    .data_translation
                    .as_ref()
                    .is_none_or(|frontend| frontend.is_empty())
                && state.events.iter().all(|event| {
                    event.execution().memory_access().is_none()
                        || state
                            .issued_data_for_fetches
                            .contains(&event.fetch().request_id())
                });
            let pending_issued = cpu.events().iter().any(|issued| {
                issued.kind() == crate::CpuFetchEventKind::Issued
                    && !cpu.events().iter().any(|event| {
                        event.request_id() == issued.request_id()
                            && event.kind() != crate::CpuFetchEventKind::Issued
                    })
            });
            if !cpu.has_outstanding_fetch() && !pending_issued && data_quiescent {
                return Ok(None);
            }
            return Err(invalid("drained capture retains operational authority"));
        }
    };
    if cpu.has_outstanding_fetch()
        || state.pending_fetch_prefix.is_some()
        || state.pending_terminal_memory_result.is_some()
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
        || !state.memory_result_window_authorizations.is_empty()
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
    let wake = state
        .o3_writeback_wake
        .checkpoint_scheduled_wake()
        .ok_or(invalid("live queue does not own exactly one attached wake"))?;
    if wake.tick() != runtime.service.requested_tick
        || wake.event().partition()
            != cpu
                .events()
                .first()
                .map_or(wake.event().partition(), |event| event.partition())
    {
        return Err(invalid("scheduled wake does not match queue service"));
    }
    let expected_requests = runtime
        .issue_rows
        .iter()
        .map(|row| row.fetch_request)
        .collect::<Vec<_>>();
    if cpu.events().len() != expected_requests.len()
        || cpu
            .events()
            .iter()
            .map(|event| event.request_id())
            .ne(expected_requests.iter().copied())
        || cpu.events().iter().any(|event| {
            event.kind() != crate::CpuFetchEventKind::Completed
                || !matches!(event.data().map(<[u8]>::len), Some(2 | 4))
                || event.data().map(|bytes| bytes.len() as u64) != Some(event.size().bytes())
        })
    {
        return Err(invalid(
            "operational fetch stream is not exact completed instructions",
        ));
    }
    if expected_requests
        .iter()
        .any(|request| request.sequence() >= cpu.next_sequence())
    {
        return Err(invalid(
            "next fetch sequence does not follow restored requests",
        ));
    }
    let by_request = state
        .events
        .iter()
        .map(|event| (event.fetch().request_id(), event))
        .collect::<BTreeMap<_, _>>();
    let mut events = Vec::with_capacity(expected_requests.len());
    for request in &expected_requests {
        let event = by_request
            .get(request)
            .ok_or(invalid("issue fetch has no execution event"))?;
        let projected = project_event(event);
        if projected.memory_access.is_some()
            || projected.data_access_event_kind.is_some()
            || projected.rebuild().as_ref() != Ok(*event)
        {
            return Err(invalid(
                "execution event contains unsupported transient state",
            ));
        }
        events.push(projected);
    }
    let event_requests = expected_requests.iter().copied().collect::<BTreeSet<_>>();
    let live_issued_requests = expected_requests
        .iter()
        .filter(|request| state.issued_data_for_fetches.contains(request))
        .copied()
        .collect::<Vec<_>>();
    if !event_requests.is_subset(&state.executed_fetches) || !live_issued_requests.is_empty() {
        return Err(invalid(
            "live replay execution is incomplete or carries data issue state",
        ));
    }
    Ok(Some(RiscvO3LiveCheckpointPayload {
        profile: RiscvO3LiveCheckpointProfile::ComputeQueue,
        captured_tick,
        next_fetch_pc: cpu.pc(),
        next_fetch_request_sequence: cpu.next_sequence(),
        events,
        issue_rows: runtime.issue_rows,
        rename_rows: runtime.rename_rows,
        resident_sequences: runtime.resident_sequences,
        executed_fetch_requests: event_requests.iter().copied().collect(),
        issued_fetch_requests: live_issued_requests,
        service: runtime.service,
        finalized_writeback: runtime.finalized_writeback,
        writeback_counted_sequences: Vec::new(),
        writeback_published_sequences: Vec::new(),
        reservation: None,
        completed_result: None,
        wake: RiscvO3LiveCheckpointWake {
            scheduler_instance_raw: wake.scheduler().checkpoint_raw(),
            partition: wake.event().partition(),
            tick: wake.tick(),
            scheduler_order: wake.event().order(),
            kind: wake.event().kind(),
        },
    }))
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
