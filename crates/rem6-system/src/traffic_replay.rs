use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;
use rem6_memory::MemoryRequestId;
use rem6_traffic::{
    TrafficTraceMemoryFailureRecord, TrafficTraceReplayAction, TrafficTraceReplayActionQueue,
};
use rem6_transport::{RequestDelivery, TargetOutcome};

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
