use std::collections::VecDeque;

use rem6_memory::{Address, MemoryRequestId, MemoryResponse};

use crate::{
    common::checked_counter_add, TrafficGeneratorError, TrafficTraceCacheEvent,
    TrafficTraceDiagnosticEvent, TrafficTraceHtmEvent, TrafficTraceResponseEvent,
    TrafficTraceSyncEvent, TrafficTraceTlbEvent,
};

use super::{
    TrafficControllerEvent, TrafficControllerEventBatch, TrafficTraceControlFailure,
    TrafficTraceMemoryFailure, TrafficTraceReplayAction, TrafficTraceReplayCompletion,
    TrafficTraceReplayFailure, TrafficTraceReplaySource,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTraceMemoryResponseRecord {
    tick: u64,
    response: MemoryResponse,
    trace_data: Option<Vec<u8>>,
}

impl TrafficTraceMemoryResponseRecord {
    pub fn new(tick: u64, response: MemoryResponse) -> Self {
        Self {
            tick,
            response,
            trace_data: None,
        }
    }

    pub fn with_trace_data(
        tick: u64,
        response: MemoryResponse,
        trace_data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            tick,
            response,
            trace_data,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn response(&self) -> &MemoryResponse {
        &self.response
    }

    pub fn trace_data(&self) -> Option<&[u8]> {
        self.trace_data.as_deref()
    }

    pub fn into_response(self) -> MemoryResponse {
        self.response
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceMemoryWriteCompletionRecord {
    tick: u64,
    request_id: MemoryRequestId,
    request_line: Address,
    request_size_bytes: u64,
    response: TrafficTraceResponseEvent,
}

impl TrafficTraceMemoryWriteCompletionRecord {
    pub const fn new(
        tick: u64,
        request_id: MemoryRequestId,
        request_line: Address,
        request_size_bytes: u64,
        response: TrafficTraceResponseEvent,
    ) -> Self {
        Self {
            tick,
            request_id,
            request_line,
            request_size_bytes,
            response,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn request_id(self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn request_line(self) -> Address {
        self.request_line
    }

    pub const fn request_size_bytes(self) -> u64 {
        self.request_size_bytes
    }

    pub const fn response(self) -> TrafficTraceResponseEvent {
        self.response
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceMemoryFailureRecord {
    tick: u64,
    failure: TrafficTraceMemoryFailure,
}

impl TrafficTraceMemoryFailureRecord {
    pub const fn new(tick: u64, failure: TrafficTraceMemoryFailure) -> Self {
        Self { tick, failure }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn failure(self) -> TrafficTraceMemoryFailure {
        self.failure
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceControlFailureSource {
    Sync(TrafficTraceSyncEvent),
    Tlb(TrafficTraceTlbEvent),
    Cache(TrafficTraceCacheEvent),
    Htm(TrafficTraceHtmEvent),
    Diagnostic(TrafficTraceDiagnosticEvent),
}

impl TrafficTraceControlFailureSource {
    pub const fn from_replay_source(source: &TrafficTraceReplaySource) -> Option<Self> {
        match source {
            TrafficTraceReplaySource::Memory(_) => None,
            TrafficTraceReplaySource::Sync(event) => Some(Self::Sync(*event)),
            TrafficTraceReplaySource::Tlb(event) => Some(Self::Tlb(*event)),
            TrafficTraceReplaySource::Cache(event) => Some(Self::Cache(*event)),
            TrafficTraceReplaySource::Htm(event) => Some(Self::Htm(*event)),
            TrafficTraceReplaySource::Diagnostic(event) => Some(Self::Diagnostic(*event)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceControlFailureRecord {
    tick: u64,
    failure: TrafficTraceControlFailure,
    source: Option<TrafficTraceControlFailureSource>,
}

impl TrafficTraceControlFailureRecord {
    pub const fn new(tick: u64, failure: TrafficTraceControlFailure) -> Self {
        Self {
            tick,
            failure,
            source: None,
        }
    }

    pub const fn with_source(
        tick: u64,
        failure: TrafficTraceControlFailure,
        source: Option<TrafficTraceControlFailureSource>,
    ) -> Self {
        Self {
            tick,
            failure,
            source,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn failure(self) -> TrafficTraceControlFailure {
        self.failure
    }

    pub const fn source(self) -> Option<TrafficTraceControlFailureSource> {
        self.source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrafficTraceReplayQueuedAction {
    action: TrafficTraceReplayAction,
    control_failure_source: Option<TrafficTraceControlFailureSource>,
}

impl TrafficTraceReplayQueuedAction {
    const fn new(
        action: TrafficTraceReplayAction,
        control_failure_source: Option<TrafficTraceControlFailureSource>,
    ) -> Self {
        Self {
            action,
            control_failure_source,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplayActionQueue {
    actions: VecDeque<TrafficTraceReplayQueuedAction>,
    summary: TrafficTraceReplaySummary,
}

impl TrafficTraceReplayActionQueue {
    pub fn record_batch(
        &mut self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<(), TrafficGeneratorError> {
        let mut control_failure_source = None;
        for event in batch.events() {
            match event {
                TrafficControllerEvent::TraceErrorMatch(error) => {
                    control_failure_source =
                        TrafficTraceControlFailureSource::from_replay_source(error.source());
                }
                TrafficControllerEvent::TraceReplayAction(action) => {
                    let source =
                        if matches!(action, TrafficTraceReplayAction::ControlFailure { .. }) {
                            control_failure_source.take()
                        } else {
                            None
                        };
                    self.record_action_with_control_failure_source(action.clone(), source)?;
                    control_failure_source = None;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn record_action(
        &mut self,
        action: TrafficTraceReplayAction,
    ) -> Result<(), TrafficGeneratorError> {
        self.record_action_with_control_failure_source(action, None)
    }

    fn record_action_with_control_failure_source(
        &mut self,
        action: TrafficTraceReplayAction,
        control_failure_source: Option<TrafficTraceControlFailureSource>,
    ) -> Result<(), TrafficGeneratorError> {
        self.summary.record_action(&action)?;
        self.actions.push_back(TrafficTraceReplayQueuedAction::new(
            action,
            control_failure_source,
        ));
        Ok(())
    }

    pub fn pop_action(&mut self) -> Option<TrafficTraceReplayAction> {
        self.actions.pop_front().map(|queued| queued.action)
    }

    pub fn peek_action(&self) -> Option<&TrafficTraceReplayAction> {
        self.actions.front().map(|queued| &queued.action)
    }

    pub fn pop_memory_response(&mut self) -> Option<TrafficTraceMemoryResponseRecord> {
        let index = self.actions.iter().position(|queued| {
            matches!(
                queued.action,
                TrafficTraceReplayAction::MemoryResponse { .. }
            )
        })?;
        match self.actions.remove(index)?.action {
            TrafficTraceReplayAction::MemoryResponse {
                tick,
                response,
                trace_data,
            } => Some(TrafficTraceMemoryResponseRecord::with_trace_data(
                tick, response, trace_data,
            )),
            _ => unreachable!("selected memory response action"),
        }
    }

    pub fn pop_control_ack_tick(&mut self) -> Option<u64> {
        let index = self.actions.iter().position(|queued| {
            matches!(queued.action, TrafficTraceReplayAction::ControlAck { .. })
        })?;
        match self.actions.remove(index)?.action {
            TrafficTraceReplayAction::ControlAck { tick } => Some(tick),
            _ => unreachable!("selected control acknowledgement action"),
        }
    }

    pub fn pop_memory_write_completion(
        &mut self,
    ) -> Option<TrafficTraceMemoryWriteCompletionRecord> {
        let index = self.actions.iter().position(|queued| {
            matches!(
                queued.action,
                TrafficTraceReplayAction::MemoryWriteCompletion { .. }
            )
        })?;
        match self.actions.remove(index)?.action {
            TrafficTraceReplayAction::MemoryWriteCompletion {
                tick,
                request,
                request_line,
                request_size_bytes,
                response,
            } => Some(TrafficTraceMemoryWriteCompletionRecord::new(
                tick,
                request,
                request_line,
                request_size_bytes,
                response,
            )),
            _ => unreachable!("selected memory write completion action"),
        }
    }

    pub fn pop_memory_failure(&mut self) -> Option<TrafficTraceMemoryFailureRecord> {
        let index = self.actions.iter().position(|queued| {
            matches!(
                queued.action,
                TrafficTraceReplayAction::MemoryFailure { .. }
            )
        })?;
        match self.actions.remove(index)?.action {
            TrafficTraceReplayAction::MemoryFailure { tick, failure } => {
                Some(TrafficTraceMemoryFailureRecord::new(tick, failure))
            }
            _ => unreachable!("selected memory failure action"),
        }
    }

    pub fn pop_control_failure(&mut self) -> Option<TrafficTraceControlFailureRecord> {
        let index = self.actions.iter().position(|queued| {
            matches!(
                queued.action,
                TrafficTraceReplayAction::ControlFailure { .. }
            )
        })?;
        let queued = self.actions.remove(index)?;
        match queued.action {
            TrafficTraceReplayAction::ControlFailure { tick, failure } => {
                Some(TrafficTraceControlFailureRecord::with_source(
                    tick,
                    failure,
                    queued.control_failure_source,
                ))
            }
            _ => unreachable!("selected control failure action"),
        }
    }

    pub const fn summary(&self) -> TrafficTraceReplaySummary {
        self.summary
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplaySummary {
    pub(super) memory_completions: u64,
    pub(super) write_completions: u64,
    pub(super) control_completions: u64,
    pub(super) memory_failures: u64,
    pub(super) control_failures: u64,
}

impl TrafficTraceReplaySummary {
    pub const fn memory_completions(self) -> u64 {
        self.memory_completions
    }

    pub const fn write_completions(self) -> u64 {
        self.write_completions
    }

    pub const fn control_completions(self) -> u64 {
        self.control_completions
    }

    pub const fn memory_failures(self) -> u64 {
        self.memory_failures
    }

    pub const fn control_failures(self) -> u64 {
        self.control_failures
    }

    pub(super) fn record_completion(
        &mut self,
        completion: &TrafficTraceReplayCompletion,
    ) -> Result<(), TrafficGeneratorError> {
        match completion {
            TrafficTraceReplayCompletion::Memory(_) => {
                self.memory_completions = checked_counter_add(
                    "trace_replay.memory_completions",
                    self.memory_completions,
                    1,
                )?;
            }
            TrafficTraceReplayCompletion::WriteCompletion(_) => {
                self.write_completions = checked_counter_add(
                    "trace_replay.write_completions",
                    self.write_completions,
                    1,
                )?;
            }
            TrafficTraceReplayCompletion::Ack => {
                self.control_completions = checked_counter_add(
                    "trace_replay.control_completions",
                    self.control_completions,
                    1,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn record_failure(
        &mut self,
        failure: &TrafficTraceReplayFailure,
    ) -> Result<(), TrafficGeneratorError> {
        match failure {
            TrafficTraceReplayFailure::Memory(_) => {
                self.memory_failures =
                    checked_counter_add("trace_replay.memory_failures", self.memory_failures, 1)?;
            }
            TrafficTraceReplayFailure::Control(_) => {
                self.control_failures =
                    checked_counter_add("trace_replay.control_failures", self.control_failures, 1)?;
            }
        }
        Ok(())
    }

    fn record_action(
        &mut self,
        action: &TrafficTraceReplayAction,
    ) -> Result<(), TrafficGeneratorError> {
        match action {
            TrafficTraceReplayAction::MemoryResponse { .. } => {
                self.memory_completions = checked_counter_add(
                    "trace_replay.memory_completions",
                    self.memory_completions,
                    1,
                )?;
            }
            TrafficTraceReplayAction::MemoryWriteCompletion { .. } => {
                self.write_completions = checked_counter_add(
                    "trace_replay.write_completions",
                    self.write_completions,
                    1,
                )?;
            }
            TrafficTraceReplayAction::ControlAck { .. } => {
                self.control_completions = checked_counter_add(
                    "trace_replay.control_completions",
                    self.control_completions,
                    1,
                )?;
            }
            TrafficTraceReplayAction::MemoryFailure { .. } => {
                self.memory_failures =
                    checked_counter_add("trace_replay.memory_failures", self.memory_failures, 1)?;
            }
            TrafficTraceReplayAction::ControlFailure { .. } => {
                self.control_failures =
                    checked_counter_add("trace_replay.control_failures", self.control_failures, 1)?;
            }
        }
        Ok(())
    }
}
