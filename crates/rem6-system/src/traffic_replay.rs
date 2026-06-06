use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;
use rem6_memory::MemoryRequestId;
use rem6_traffic::{TrafficTraceReplayAction, TrafficTraceReplayActionQueue};
use rem6_transport::{RequestDelivery, TargetOutcome};

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
    ResponseBeforeRequest {
        request: MemoryRequestId,
        delivery_tick: Tick,
        response_tick: Tick,
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

    let delay = tick.checked_sub(delivery.tick()).ok_or(
        TrafficTraceReplayTargetError::ResponseBeforeRequest {
            request,
            delivery_tick: delivery.tick(),
            response_tick: tick,
        },
    )?;
    if delay == 0 {
        let response = pop_validated_memory_response(queue);
        Ok(TargetOutcome::Respond(response))
    } else {
        let response = pop_validated_memory_response(queue);
        Ok(TargetOutcome::RespondAfter { delay, response })
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
