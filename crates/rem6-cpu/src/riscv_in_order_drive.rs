use rem6_isa_riscv::RiscvInstruction;
use rem6_kernel::{
    PartitionEventId, PartitionedScheduler, PendingEventSnapshot, SchedulerError,
    SchedulerInstanceId,
};
use rem6_memory::Address;

use crate::riscv_execute::{oldest_completed_fetch_at, RiscvPendingFetchPrefix};
use crate::{
    riscv_fu_latency::riscv_pipeline_execute_wait_cycles, CpuFetchEvent, CpuFetchEventKind,
    InOrderPipelineStage, RiscvCore, RiscvCoreDriveAction, RiscvCpuError,
};

const PIPELINE_CYCLE_TICKS: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvInOrderDriveStatus {
    Unavailable,
    Pending,
    Ready,
    Reserved { sequence: u64, generation: u64 },
    Scheduled(PartitionEventId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvInOrderFetchAdmission {
    Admitted,
    PipelineCyclePending,
    AdvanceBeforeFetch,
    RetireBeforeFetch,
}

impl RiscvInOrderFetchAdmission {
    pub(crate) const fn allows_fetch(self) -> bool {
        matches!(self, Self::Admitted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvInOrderPipelineWake {
    generation: u64,
    scheduler: SchedulerInstanceId,
    event: PendingEventSnapshot,
}

impl RiscvInOrderPipelineWake {
    const fn new(
        generation: u64,
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) -> Self {
        Self {
            generation,
            scheduler,
            event,
        }
    }

    const fn generation(self) -> u64 {
        self.generation
    }

    const fn scheduler(self) -> SchedulerInstanceId {
        self.scheduler
    }

    const fn event(self) -> PendingEventSnapshot {
        self.event
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvInOrderWakeKind {
    Serial,
    Parallel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvPipelineCandidate {
    BypassForPrefixAssembly,
    Sequence(u64),
}

impl RiscvCore {
    pub(crate) fn in_order_fetch_admission(&self) -> RiscvInOrderFetchAdmission {
        let state = self.state.lock().expect("riscv core lock");
        if state.pending_in_order_pipeline_advance.is_some() {
            return RiscvInOrderFetchAdmission::PipelineCyclePending;
        }
        if state.pending_fetch_prefix.is_some() || state.in_order_pipeline.fetch1_has_slot() {
            return RiscvInOrderFetchAdmission::Admitted;
        }
        if state.in_order_pipeline.commit_is_occupied() {
            RiscvInOrderFetchAdmission::RetireBeforeFetch
        } else {
            RiscvInOrderFetchAdmission::AdvanceBeforeFetch
        }
    }

    pub(crate) fn drive_next_completed_fetch_serial_action(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError> {
        self.drive_next_completed_fetch_action(scheduler, RiscvInOrderWakeKind::Serial)
    }

    pub(crate) fn drive_next_completed_fetch_parallel_action(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError> {
        self.drive_next_completed_fetch_action(scheduler, RiscvInOrderWakeKind::Parallel)
    }

    fn drive_next_completed_fetch_action(
        &self,
        scheduler: &mut PartitionedScheduler,
        wake_kind: RiscvInOrderWakeKind,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError> {
        if self.detailed_o3_window_prefers_fetch_ahead()
            || self.o3_retirement_suppresses_normal_pipeline()
        {
            let execution = match wake_kind {
                RiscvInOrderWakeKind::Serial => {
                    self.execute_next_completed_fetch_serial(scheduler)?
                }
                RiscvInOrderWakeKind::Parallel => {
                    self.execute_next_completed_fetch_parallel(scheduler)?
                }
            };
            return Ok(
                execution.map(|event| RiscvCoreDriveAction::InstructionExecuted(Box::new(event)))
            );
        }
        match self.schedule_next_completed_fetch_pipeline_cycle(scheduler, wake_kind)? {
            RiscvInOrderDriveStatus::Unavailable | RiscvInOrderDriveStatus::Pending => Ok(None),
            RiscvInOrderDriveStatus::Ready => {
                let execution = match wake_kind {
                    RiscvInOrderWakeKind::Serial => {
                        self.execute_next_completed_fetch_serial(scheduler)?
                    }
                    RiscvInOrderWakeKind::Parallel => {
                        self.execute_next_completed_fetch_parallel(scheduler)?
                    }
                };
                Ok(execution
                    .map(|event| RiscvCoreDriveAction::InstructionExecuted(Box::new(event))))
            }
            RiscvInOrderDriveStatus::Scheduled(event) => {
                Ok(Some(RiscvCoreDriveAction::PipelineCycleScheduled { event }))
            }
            RiscvInOrderDriveStatus::Reserved { .. } => {
                unreachable!("pipeline reservation is scheduled before returning")
            }
        }
    }

    pub(crate) fn schedule_next_completed_fetch_pipeline_cycle_serial(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<RiscvInOrderDriveStatus, RiscvCpuError> {
        self.schedule_next_completed_fetch_pipeline_cycle(scheduler, RiscvInOrderWakeKind::Serial)
    }

    pub(crate) fn schedule_next_completed_fetch_pipeline_cycle_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
    ) -> Result<RiscvInOrderDriveStatus, RiscvCpuError> {
        self.schedule_next_completed_fetch_pipeline_cycle(scheduler, RiscvInOrderWakeKind::Parallel)
    }

    fn schedule_next_completed_fetch_pipeline_cycle(
        &self,
        scheduler: &mut PartitionedScheduler,
        wake_kind: RiscvInOrderWakeKind,
    ) -> Result<RiscvInOrderDriveStatus, RiscvCpuError> {
        let status = self.reserve_next_completed_fetch_pipeline_cycle()?;
        let RiscvInOrderDriveStatus::Reserved {
            sequence,
            generation,
        } = status
        else {
            return Ok(status);
        };
        let event =
            self.schedule_reserved_pipeline_cycle(scheduler, wake_kind, sequence, generation)?;
        Ok(RiscvInOrderDriveStatus::Scheduled(event))
    }

    fn reserve_next_completed_fetch_pipeline_cycle(
        &self,
    ) -> Result<RiscvInOrderDriveStatus, RiscvCpuError> {
        self.sync_in_order_fetch_state()?;
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_in_order_pipeline_advance.is_some() {
            return Ok(RiscvInOrderDriveStatus::Pending);
        }
        let Some(candidate) = next_pipeline_candidate(&state, &fetch_events) else {
            return Ok(RiscvInOrderDriveStatus::Unavailable);
        };
        let RiscvPipelineCandidate::Sequence(sequence) = candidate else {
            return Ok(RiscvInOrderDriveStatus::Ready);
        };
        let Some((stage, execute_wait_total_cycles, execute_wait_key)) = state
            .in_order_pipeline
            .in_flight()
            .iter()
            .find(|instruction| instruction.sequence() == sequence)
            .map(|instruction| {
                (
                    instruction.stage(),
                    instruction.execute_wait_total_cycles(),
                    instruction.execute_wait_key(),
                )
            })
        else {
            return Err(RiscvCpuError::MissingInOrderPipelineInstruction { sequence });
        };
        let detailed = state.live_retire_gate.detailed_policy_enabled();
        let execute_wait_rebound = state.rebound_in_order_execute_waits.contains(&sequence);
        if execute_wait_total_cycles.is_some()
            || (stage == InOrderPipelineStage::Execute && !detailed)
        {
            let raw = completed_fetch_raw(&state, &fetch_events, sequence)?;
            let decoded = RiscvInstruction::decode_with_length(raw).map_err(RiscvCpuError::Isa)?;
            let wait_cycles = riscv_pipeline_execute_wait_cycles(decoded.instruction());
            let expected_wait_cycles = (wait_cycles > 0).then_some(wait_cycles);
            let wait_key = u64::from(raw) + 1;
            let instruction_identity_matches = match execute_wait_key {
                Some(execute_wait_key) => execute_wait_key == wait_key,
                None => !execute_wait_rebound || !detailed,
            };
            let wait_configuration_matches =
                execute_wait_total_cycles == expected_wait_cycles && instruction_identity_matches;
            if !wait_configuration_matches {
                let configured_wait_cycles = if detailed { 0 } else { wait_cycles };
                state.in_order_pipeline.configure_execute_wait(
                    sequence,
                    configured_wait_cycles,
                    wait_key,
                );
            } else if wait_cycles > 0 && execute_wait_key.is_none() {
                state
                    .in_order_pipeline
                    .bind_execute_wait_key(sequence, wait_key);
            }
            state.rebound_in_order_execute_waits.remove(&sequence);
        }
        let stage = state
            .in_order_pipeline
            .in_flight()
            .iter()
            .find(|instruction| instruction.sequence() == sequence)
            .expect("pipeline candidate remains in flight")
            .stage();
        if stage == InOrderPipelineStage::Commit {
            return Ok(RiscvInOrderDriveStatus::Ready);
        }
        let generation = state.reserve_in_order_pipeline_advance(sequence);
        Ok(RiscvInOrderDriveStatus::Reserved {
            sequence,
            generation,
        })
    }

    fn schedule_reserved_pipeline_cycle(
        &self,
        scheduler: &mut PartitionedScheduler,
        wake_kind: RiscvInOrderWakeKind,
        sequence: u64,
        generation: u64,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let core = self.clone();
        let scheduled = match wake_kind {
            RiscvInOrderWakeKind::Serial => {
                scheduler.schedule_after(self.partition(), PIPELINE_CYCLE_TICKS, move |_context| {
                    core.complete_reserved_pipeline_cycle(sequence, generation)
                })
            }
            RiscvInOrderWakeKind::Parallel => {
                let tick = scheduler
                    .now()
                    .checked_add(PIPELINE_CYCLE_TICKS)
                    .ok_or(SchedulerError::TickOverflow {
                        now: scheduler.now(),
                        delay: PIPELINE_CYCLE_TICKS,
                    })
                    .map_err(RiscvCpuError::Scheduler)?;
                scheduler.schedule_parallel_at(self.partition(), tick, move |_context| {
                    core.complete_reserved_pipeline_cycle(sequence, generation)
                })
            }
        };
        match scheduled {
            Ok(event) => {
                let snapshot = scheduler
                    .pending_event_snapshot(event)
                    .expect("newly scheduled in-order pipeline wake is pending");
                let wake =
                    RiscvInOrderPipelineWake::new(generation, scheduler.instance_id(), snapshot);
                self.state
                    .lock()
                    .expect("riscv core lock")
                    .mark_in_order_pipeline_wake(sequence, generation, wake);
                Ok(event)
            }
            Err(error) => {
                let mut state = self.state.lock().expect("riscv core lock");
                state.cancel_in_order_pipeline_advance(sequence, generation);
                Err(RiscvCpuError::Scheduler(error))
            }
        }
    }

    fn complete_reserved_pipeline_cycle(&self, sequence: u64, generation: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        if !state.complete_in_order_pipeline_advance(sequence, generation) {
            return;
        }
        let stage = state
            .in_order_pipeline
            .in_flight()
            .iter()
            .find(|instruction| instruction.sequence() == sequence)
            .map(|instruction| instruction.stage());
        if stage.is_none() || stage == Some(InOrderPipelineStage::Commit) {
            return;
        }
        let execute_wait_pending = stage == Some(InOrderPipelineStage::Execute)
            && state
                .in_order_pipeline
                .in_flight()
                .iter()
                .any(|instruction| {
                    instruction.sequence() == sequence
                        && instruction
                            .execute_wait_remaining_cycles()
                            .is_some_and(|remaining| remaining > 0)
                });
        let record = if execute_wait_pending {
            state
                .in_order_pipeline
                .try_record_execute_wait_cycle(sequence)
                .expect("scheduled execute-wait cycle remains valid")
        } else {
            state
                .in_order_pipeline
                .try_advance_cycle_recorded_without_retirement()
                .expect("scheduled in-order pipeline cycle advance remains valid")
        };
        state.in_order_pipeline_cycle_records.push(record);
    }

    #[doc(hidden)]
    pub fn checkpoint_owned_in_order_pipeline_wakes(
        &self,
    ) -> Vec<(SchedulerInstanceId, PendingEventSnapshot)> {
        self.state
            .lock()
            .expect("riscv core lock")
            .owned_in_order_pipeline_wakes()
            .into_iter()
            .map(|wake| (wake.scheduler(), wake.event()))
            .collect()
    }

    pub(crate) fn cancel_scheduled_in_order_pipeline_cycle(
        &self,
        scheduler: &mut PartitionedScheduler,
        event: PartitionEventId,
    ) -> Result<(), RiscvCpuError> {
        let (sequence, generation) = {
            let state = self.state.lock().expect("riscv core lock");
            let (sequence, generation) = state
                .pending_in_order_pipeline_advance
                .expect("prepared pipeline cycle keeps its reservation");
            let wake = state
                .pending_in_order_pipeline_wake
                .expect("prepared pipeline cycle keeps its scheduler wake");
            assert_eq!(wake.scheduler(), scheduler.instance_id());
            assert_eq!(wake.event().id(), event);
            (sequence, generation)
        };
        scheduler
            .cancel_event(event)
            .map_err(RiscvCpuError::Scheduler)?;
        self.state
            .lock()
            .expect("riscv core lock")
            .cancel_in_order_pipeline_advance(sequence, generation);
        Ok(())
    }

    #[doc(hidden)]
    pub fn forget_discarded_in_order_pipeline_wakes(&self) {
        self.state
            .lock()
            .expect("riscv core lock")
            .forget_in_order_pipeline_wakes();
    }
}

impl crate::RiscvCoreState {
    fn reserve_in_order_pipeline_advance(&mut self, sequence: u64) -> u64 {
        let generation = self.next_in_order_pipeline_wake_generation;
        self.next_in_order_pipeline_wake_generation = generation.wrapping_add(1);
        self.pending_in_order_pipeline_advance = Some((sequence, generation));
        self.pending_in_order_pipeline_wake = None;
        generation
    }

    fn mark_in_order_pipeline_wake(
        &mut self,
        sequence: u64,
        generation: u64,
        wake: RiscvInOrderPipelineWake,
    ) {
        if self.pending_in_order_pipeline_advance == Some((sequence, generation)) {
            self.pending_in_order_pipeline_wake = Some(wake);
        } else {
            self.remember_detached_in_order_pipeline_wake(wake);
        }
    }

    fn cancel_in_order_pipeline_advance(&mut self, sequence: u64, generation: u64) {
        if self.pending_in_order_pipeline_advance == Some((sequence, generation)) {
            self.pending_in_order_pipeline_advance = None;
            self.pending_in_order_pipeline_wake = None;
        }
    }

    fn complete_in_order_pipeline_advance(&mut self, sequence: u64, generation: u64) -> bool {
        self.detached_in_order_pipeline_wakes
            .retain(|wake| wake.generation() != generation);
        if self.pending_in_order_pipeline_advance != Some((sequence, generation)) {
            return false;
        }
        self.pending_in_order_pipeline_advance = None;
        self.pending_in_order_pipeline_wake = None;
        true
    }

    pub(crate) fn detach_pending_in_order_pipeline_advance(&mut self) {
        if let Some(wake) = self.pending_in_order_pipeline_wake.take() {
            self.remember_detached_in_order_pipeline_wake(wake);
        }
        self.pending_in_order_pipeline_advance = None;
    }

    pub(crate) fn forget_in_order_pipeline_wakes(&mut self) {
        self.pending_in_order_pipeline_advance = None;
        self.pending_in_order_pipeline_wake = None;
        self.detached_in_order_pipeline_wakes.clear();
    }

    fn remember_detached_in_order_pipeline_wake(&mut self, wake: RiscvInOrderPipelineWake) {
        if !self
            .detached_in_order_pipeline_wakes
            .iter()
            .any(|candidate| candidate.generation() == wake.generation())
        {
            self.detached_in_order_pipeline_wakes.push(wake);
        }
    }

    fn owned_in_order_pipeline_wakes(&self) -> Vec<RiscvInOrderPipelineWake> {
        self.detached_in_order_pipeline_wakes
            .iter()
            .copied()
            .chain(self.pending_in_order_pipeline_wake)
            .collect()
    }
}

fn next_pipeline_candidate(
    state: &crate::RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Option<RiscvPipelineCandidate> {
    if let Some(prefix) = state.pending_fetch_prefix.as_ref() {
        return next_prefix_suffix(state, fetch_events, prefix)
            .map(|_| RiscvPipelineCandidate::Sequence(prefix.fetch.request_id().sequence()));
    }
    let architectural = Address::new(state.hart.pc());
    let fetch = fetch_events
        .iter()
        .filter(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && event.pc() == architectural
                && !state.executed_fetches.contains(&event.request_id())
        })
        .min_by_key(|event| event.request_id().sequence())?;
    let data = fetch.data()?;
    if matches!(data, [low, _] if low & 0x3 == 0x3) {
        return Some(RiscvPipelineCandidate::BypassForPrefixAssembly);
    }
    Some(RiscvPipelineCandidate::Sequence(
        fetch.request_id().sequence(),
    ))
}

fn next_prefix_suffix<'a>(
    state: &crate::RiscvCoreState,
    fetch_events: &'a [CpuFetchEvent],
    prefix: &RiscvPendingFetchPrefix,
) -> Option<&'a CpuFetchEvent> {
    oldest_completed_fetch_at(
        &state.executed_fetches,
        fetch_events,
        prefix.fetch.request_id(),
        Address::new(prefix.fetch.pc().get() + 2),
    )
}

fn completed_fetch_raw(
    state: &crate::RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
    sequence: u64,
) -> Result<u32, RiscvCpuError> {
    if let Some(prefix) = state
        .pending_fetch_prefix
        .as_ref()
        .filter(|prefix| prefix.fetch.request_id().sequence() == sequence)
    {
        let suffix = next_prefix_suffix(state, fetch_events, prefix)
            .expect("pipeline candidate has a completed fetch suffix");
        let data = suffix.data().ok_or(RiscvCpuError::MissingFetchData {
            request: suffix.request_id(),
        })?;
        let [low, high] = data else {
            return Err(RiscvCpuError::InvalidFetchWidth {
                request: suffix.request_id(),
                bytes: data.len() as u64,
            });
        };
        return Ok(u32::from_le_bytes([
            prefix.bytes[0],
            prefix.bytes[1],
            *low,
            *high,
        ]));
    }

    let fetch = fetch_events
        .iter()
        .find(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && event.request_id().sequence() == sequence
        })
        .expect("pipeline candidate has a completed fetch event");
    let data = fetch.data().ok_or(RiscvCpuError::MissingFetchData {
        request: fetch.request_id(),
    })?;
    match data {
        [low, high] if low & 0x3 != 0x3 => Ok(u32::from(u16::from_le_bytes([*low, *high]))),
        [a, b, c, d] => Ok(u32::from_le_bytes([*a, *b, *c, *d])),
        _ => Err(RiscvCpuError::InvalidFetchWidth {
            request: fetch.request_id(),
            bytes: data.len() as u64,
        }),
    }
}

#[cfg(test)]
#[path = "riscv_in_order_drive_tests.rs"]
mod tests;
