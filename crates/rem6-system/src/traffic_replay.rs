use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{SchedulerContext, Tick};
use rem6_memory::MemoryRequestId;
use rem6_traffic::{
    TrafficController, TrafficControllerEvent, TrafficControllerEventBatch, TrafficGeneratorError,
    TrafficTraceMemoryFailureRecord, TrafficTraceReplayAction, TrafficTraceReplayActionQueue,
};
use rem6_transport::{RequestDelivery, TargetOutcome};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceReplayScheduledMemoryFailure {
    tick: Tick,
    record: TrafficTraceMemoryFailureRecord,
}

impl TrafficTraceReplayScheduledMemoryFailure {
    pub const fn new(tick: Tick, record: TrafficTraceMemoryFailureRecord) -> Self {
        Self { tick, record }
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn record(self) -> TrafficTraceMemoryFailureRecord {
        self.record
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TrafficTraceReplayTargetRuntime {
    action_queue: TrafficTraceReplayActionQueue,
    memory_failures: Vec<TrafficTraceReplayScheduledMemoryFailure>,
    request_ticks: BTreeMap<MemoryRequestId, Tick>,
}

impl TrafficTraceReplayTargetRuntime {
    pub fn record_batch(
        &mut self,
        batch: &TrafficControllerEventBatch,
    ) -> Result<(), rem6_traffic::TrafficGeneratorError> {
        if let Some(request) = batch
            .request()
            .filter(|request| request.request().requires_response())
        {
            self.request_ticks
                .insert(request.request().id(), request.tick());
        }
        for event in batch.events() {
            let TrafficControllerEvent::TraceReplayAction(action) = event else {
                continue;
            };
            if matches!(
                action,
                TrafficTraceReplayAction::MemoryResponse { .. }
                    | TrafficTraceReplayAction::MemoryFailure { .. }
            ) {
                self.action_queue.record_action(action.clone())?;
            }
        }
        Ok(())
    }

    pub fn target_event(
        &mut self,
        delivery: &RequestDelivery,
    ) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
        let event = traffic_trace_replay_target_event(&mut self.action_queue, delivery)?;
        self.request_ticks.remove(&delivery.request().id());
        Ok(event)
    }

    pub fn record_memory_failure(&mut self, tick: Tick, record: TrafficTraceMemoryFailureRecord) {
        self.memory_failures
            .push(TrafficTraceReplayScheduledMemoryFailure::new(tick, record));
    }

    pub fn memory_failures(&self) -> &[TrafficTraceReplayScheduledMemoryFailure] {
        &self.memory_failures
    }

    pub fn is_empty(&self) -> bool {
        self.action_queue.is_empty()
    }

    pub fn request_tick(&self, request: MemoryRequestId) -> Option<Tick> {
        self.request_ticks.get(&request).copied()
    }
}

pub fn traffic_trace_replay_runtime_target_outcome(
    runtime: Arc<Mutex<TrafficTraceReplayTargetRuntime>>,
    delivery: &RequestDelivery,
    context: &mut SchedulerContext<'_>,
) -> Result<TargetOutcome, TrafficTraceReplayTargetError> {
    let event = runtime
        .lock()
        .expect("trace replay target runtime lock")
        .target_event(delivery)?;
    match event {
        TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => Ok(outcome),
        TrafficTraceReplayTargetEvent::MemoryFailure { delay, record } => {
            context
                .schedule_local_after(delay, move |context| {
                    runtime
                        .lock()
                        .expect("trace replay target runtime lock")
                        .record_memory_failure(context.now(), record);
                })
                .expect("validated trace replay failure delay");
            Ok(TargetOutcome::NoResponse)
        }
    }
}

pub fn traffic_trace_replay_controller_target_outcome(
    runtime: Arc<Mutex<TrafficTraceReplayTargetRuntime>>,
    controller: Arc<Mutex<TrafficController>>,
    delivery: &RequestDelivery,
    context: &mut SchedulerContext<'_>,
    retry_delay: Tick,
) -> Result<TargetOutcome, TrafficTraceReplayControllerTargetError> {
    if !delivery.request().requires_response() {
        return Ok(TargetOutcome::NoResponse);
    }

    let controller_tick = runtime
        .lock()
        .expect("trace replay target runtime lock")
        .request_tick(delivery.request().id())
        .unwrap_or_else(|| delivery.tick());
    loop {
        match traffic_trace_replay_runtime_target_outcome(Arc::clone(&runtime), delivery, context) {
            Ok(outcome) => return Ok(outcome),
            Err(TrafficTraceReplayTargetError::ActionQueueEmpty { .. }) => {}
            Err(error) => return Err(error.into()),
        }

        let batch = controller
            .lock()
            .expect("traffic controller lock")
            .next_event(controller_tick, retry_delay)?;
        let Some(batch) = batch else {
            return Err(
                TrafficTraceReplayControllerTargetError::ReplayActionMissing {
                    request: delivery.request().id(),
                },
            );
        };

        let trace_exited = batch.trace_exit().is_some();
        let target_action_available = {
            let mut runtime = runtime.lock().expect("trace replay target runtime lock");
            runtime.record_batch(&batch)?;
            !runtime.is_empty()
        };
        if trace_exited && !target_action_available {
            return Err(
                TrafficTraceReplayControllerTargetError::ReplayActionMissing {
                    request: delivery.request().id(),
                },
            );
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayControllerTargetError {
    Target(TrafficTraceReplayTargetError),
    Generator(TrafficGeneratorError),
    ReplayActionMissing { request: MemoryRequestId },
}

impl fmt::Display for TrafficTraceReplayControllerTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => write!(formatter, "{error}"),
            Self::Generator(error) => write!(formatter, "{error}"),
            Self::ReplayActionMissing { request } => {
                write!(
                    formatter,
                    "trace replay controller has no response or failure action for {request:?}"
                )
            }
        }
    }
}

impl Error for TrafficTraceReplayControllerTargetError {}

impl From<TrafficTraceReplayTargetError> for TrafficTraceReplayControllerTargetError {
    fn from(error: TrafficTraceReplayTargetError) -> Self {
        Self::Target(error)
    }
}

impl From<TrafficGeneratorError> for TrafficTraceReplayControllerTargetError {
    fn from(error: TrafficGeneratorError) -> Self {
        Self::Generator(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayTargetEvent {
    MemoryResponse(TargetOutcome),
    MemoryFailure {
        delay: Tick,
        record: TrafficTraceMemoryFailureRecord,
    },
}

impl TrafficTraceReplayTargetEvent {
    pub fn target_delay(&self) -> Tick {
        match self {
            Self::MemoryResponse(outcome) => match outcome {
                TargetOutcome::Respond(_) | TargetOutcome::NoResponse => 0,
                TargetOutcome::RespondAfter { delay, .. } => *delay,
            },
            Self::MemoryFailure { delay, .. } => *delay,
        }
    }

    pub fn into_target_outcome(self) -> TargetOutcome {
        match self {
            Self::MemoryResponse(outcome) => outcome,
            Self::MemoryFailure { .. } => TargetOutcome::NoResponse,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayTargetError {
    ActionQueueEmpty {
        request: MemoryRequestId,
    },
    UnexpectedAction {
        request: MemoryRequestId,
        action: TrafficTraceReplayAction,
    },
    RequestMismatch {
        request: MemoryRequestId,
        response: MemoryRequestId,
    },
    FailureRequestMismatch {
        request: MemoryRequestId,
        failure: MemoryRequestId,
    },
    ResponseBeforeRequest {
        request: MemoryRequestId,
        delivery_tick: Tick,
        response_tick: Tick,
    },
    FailureBeforeRequest {
        request: MemoryRequestId,
        delivery_tick: Tick,
        failure_tick: Tick,
    },
}

impl fmt::Display for TrafficTraceReplayTargetError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActionQueueEmpty { request } => {
                write!(
                    formatter,
                    "trace replay action queue is empty for {request:?}"
                )
            }
            Self::UnexpectedAction { request, action } => {
                write!(
                    formatter,
                    "trace replay action {action:?} cannot answer memory request {request:?}"
                )
            }
            Self::RequestMismatch { request, response } => {
                write!(
                    formatter,
                    "trace replay response {response:?} does not match memory request {request:?}"
                )
            }
            Self::FailureRequestMismatch { request, failure } => {
                write!(
                    formatter,
                    "trace replay failure {failure:?} does not match memory request {request:?}"
                )
            }
            Self::ResponseBeforeRequest {
                request,
                delivery_tick,
                response_tick,
            } => {
                write!(
                    formatter,
                    "trace replay response for {request:?} at tick {response_tick} precedes delivery tick {delivery_tick}"
                )
            }
            Self::FailureBeforeRequest {
                request,
                delivery_tick,
                failure_tick,
            } => {
                write!(
                    formatter,
                    "trace replay failure for {request:?} at tick {failure_tick} precedes delivery tick {delivery_tick}"
                )
            }
        }
    }
}

impl Error for TrafficTraceReplayTargetError {}

pub fn traffic_trace_replay_target_outcome(
    queue: &mut TrafficTraceReplayActionQueue,
    delivery: &RequestDelivery,
) -> Result<TargetOutcome, TrafficTraceReplayTargetError> {
    let request = delivery.request().id();
    let action = queue
        .peek_action()
        .ok_or(TrafficTraceReplayTargetError::ActionQueueEmpty { request })?;
    let (tick, response_request) = match action {
        TrafficTraceReplayAction::MemoryResponse { tick, response } => {
            (*tick, response.request_id())
        }
        action => {
            return Err(TrafficTraceReplayTargetError::UnexpectedAction {
                request,
                action: action.clone(),
            });
        }
    };

    if response_request != request {
        return Err(TrafficTraceReplayTargetError::RequestMismatch {
            request,
            response: response_request,
        });
    }

    let delay = memory_response_delay(request, delivery.tick(), tick)?;
    let response = pop_validated_memory_response(queue);
    Ok(memory_response_outcome(delay, response))
}

pub fn traffic_trace_replay_target_event(
    queue: &mut TrafficTraceReplayActionQueue,
    delivery: &RequestDelivery,
) -> Result<TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetError> {
    let request = delivery.request().id();
    let action = queue
        .peek_action()
        .ok_or(TrafficTraceReplayTargetError::ActionQueueEmpty { request })?;
    match action {
        TrafficTraceReplayAction::MemoryResponse { tick, response } => {
            let response_request = response.request_id();
            if response_request != request {
                return Err(TrafficTraceReplayTargetError::RequestMismatch {
                    request,
                    response: response_request,
                });
            }
            let delay = memory_response_delay(request, delivery.tick(), *tick)?;
            let response = pop_validated_memory_response(queue);
            Ok(TrafficTraceReplayTargetEvent::MemoryResponse(
                memory_response_outcome(delay, response),
            ))
        }
        TrafficTraceReplayAction::MemoryFailure { tick, failure } => {
            let failure_request = failure.request_id();
            if failure_request != request {
                return Err(TrafficTraceReplayTargetError::FailureRequestMismatch {
                    request,
                    failure: failure_request,
                });
            }
            let delay = memory_failure_delay(request, delivery.tick(), *tick)?;
            Ok(TrafficTraceReplayTargetEvent::MemoryFailure {
                delay,
                record: pop_validated_memory_failure(queue),
            })
        }
        action => Err(TrafficTraceReplayTargetError::UnexpectedAction {
            request,
            action: action.clone(),
        }),
    }
}

fn memory_response_delay(
    request: MemoryRequestId,
    delivery_tick: Tick,
    response_tick: Tick,
) -> Result<Tick, TrafficTraceReplayTargetError> {
    response_tick.checked_sub(delivery_tick).ok_or(
        TrafficTraceReplayTargetError::ResponseBeforeRequest {
            request,
            delivery_tick,
            response_tick,
        },
    )
}

fn memory_failure_delay(
    request: MemoryRequestId,
    delivery_tick: Tick,
    failure_tick: Tick,
) -> Result<Tick, TrafficTraceReplayTargetError> {
    failure_tick.checked_sub(delivery_tick).ok_or(
        TrafficTraceReplayTargetError::FailureBeforeRequest {
            request,
            delivery_tick,
            failure_tick,
        },
    )
}

fn memory_response_outcome(delay: Tick, response: rem6_memory::MemoryResponse) -> TargetOutcome {
    if delay == 0 {
        TargetOutcome::Respond(response)
    } else {
        TargetOutcome::RespondAfter { delay, response }
    }
}

fn pop_validated_memory_response(
    queue: &mut TrafficTraceReplayActionQueue,
) -> rem6_memory::MemoryResponse {
    match queue
        .pop_action()
        .expect("validated trace replay action remains queued")
    {
        TrafficTraceReplayAction::MemoryResponse { response, .. } => response,
        _ => unreachable!("validated trace replay action is a memory response"),
    }
}

fn pop_validated_memory_failure(
    queue: &mut TrafficTraceReplayActionQueue,
) -> TrafficTraceMemoryFailureRecord {
    match queue
        .pop_action()
        .expect("validated trace replay action remains queued")
    {
        TrafficTraceReplayAction::MemoryFailure { tick, failure } => {
            TrafficTraceMemoryFailureRecord::new(tick, failure)
        }
        _ => unreachable!("validated trace replay action is a memory failure"),
    }
}
