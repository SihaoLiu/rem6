use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionedScheduler, SchedulerError, Tick};
use rem6_memory::{MemoryRequestId, MemoryResponse, ResponseStatus};
use rem6_traffic::{
    TrafficController, TrafficControllerEventBatch, TrafficGeneratorError, TrafficRequestKind,
    TrafficStateId,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport, RequestDelivery,
    ResponseDelivery, TargetOutcome, TransportError,
};

pub type TrafficGupsTargetResponder =
    Arc<dyn Fn(&RequestDelivery) -> TargetOutcome + Send + Sync + 'static>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficGupsTransportRun {
    scheduled_count: usize,
    response_deliveries: Vec<ResponseDelivery>,
    memory_trace_events: Vec<MemoryTraceEvent>,
    final_tick: Tick,
}

impl TrafficGupsTransportRun {
    fn new(
        scheduled_count: usize,
        response_deliveries: Vec<ResponseDelivery>,
        memory_trace_events: Vec<MemoryTraceEvent>,
        final_tick: Tick,
    ) -> Self {
        Self {
            scheduled_count,
            response_deliveries,
            memory_trace_events,
            final_tick,
        }
    }

    pub const fn scheduled_count(&self) -> usize {
        self.scheduled_count
    }

    pub fn response_deliveries(&self) -> &[ResponseDelivery] {
        &self.response_deliveries
    }

    pub fn memory_trace_events(&self) -> &[MemoryTraceEvent] {
        &self.memory_trace_events
    }

    pub const fn final_tick(&self) -> Tick {
        self.final_tick
    }
}

pub fn traffic_gups_controller_transport_run(
    controller: &mut TrafficController,
    state: TrafficStateId,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    route: MemoryRouteId,
    trace: MemoryTrace,
    target: TrafficGupsTargetResponder,
) -> Result<TrafficGupsTransportRun, TrafficGupsTransportError> {
    let route_config =
        transport
            .route(route)
            .cloned()
            .ok_or(TrafficGupsTransportError::Transport(
                TransportError::UnknownRoute { route },
            ))?;
    if route_uses_fabric(&route_config) {
        return Err(TrafficGupsTransportError::UnsupportedFabricRoute { unsupported: route });
    }
    let request_latency = route_config.request_latency();
    let response_latency = route_config.response_latency();
    let mut scheduled_count = 0;
    let mut response_deliveries = Vec::new();
    let mut controller_tick = scheduler.now();

    loop {
        if controller.current_state() != Some(state) {
            break;
        }
        let checkpoint = controller.snapshot();
        let Some(batch) = controller.next_event(controller_tick, 0)? else {
            break;
        };
        let Some(request) = batch.request().cloned() else {
            if controller.current_state() != Some(state) {
                break;
            }
            restore_controller(controller, checkpoint)?;
            return Err(TrafficGupsTransportError::UnsupportedControllerBatch {
                state,
                event_count: batch.events().len(),
            });
        };

        let request_tick = request.tick();
        let target_arrival_tick = match checked_tick_add(request_tick, request_latency) {
            Ok(tick) => tick,
            Err(error) => {
                restore_controller(controller, checkpoint)?;
                return Err(error);
            }
        };
        let settlement = Arc::new(Mutex::new(GupsRequestSettlement::default()));
        let response_log = Arc::clone(&settlement);
        let target_progress = Arc::clone(&settlement);
        let target = Arc::clone(&target);
        let submitted_request = request.request().clone();
        if let Err(error) = transport.submit_parallel_at(
            scheduler,
            request_tick,
            route,
            submitted_request,
            trace.clone(),
            move |delivery, _context| {
                let outcome = target(&delivery);
                match &outcome {
                    TargetOutcome::Respond(_) => {
                        target_progress
                            .lock()
                            .expect("GUPS request settlement lock")
                            .record_expected_response(response_settlement_tick(
                                delivery.tick(),
                                0,
                                response_latency,
                            ));
                    }
                    TargetOutcome::RespondAfter { delay, .. } => {
                        target_progress
                            .lock()
                            .expect("GUPS request settlement lock")
                            .record_expected_response(response_settlement_tick(
                                delivery.tick(),
                                *delay,
                                response_latency,
                            ));
                    }
                    TargetOutcome::NoResponse => {
                        target_progress
                            .lock()
                            .expect("GUPS request settlement lock")
                            .record_no_response(delivery.tick());
                    }
                }
                outcome
            },
            move |delivery| {
                response_log
                    .lock()
                    .expect("GUPS request settlement lock")
                    .record_response(delivery);
            },
        ) {
            restore_controller(controller, checkpoint)?;
            return Err(error.into());
        }
        scheduled_count += 1;

        run_until_gups_request_settled(scheduler, &settlement, target_arrival_tick)?;
        let settlement = take_request_settlement(settlement);
        controller_tick = settlement.settled_tick(request.request().id(), scheduler.now());
        let responses = settlement.into_responses();
        if request.kind() == TrafficRequestKind::Read {
            complete_gups_read_from_responses(controller, state, &batch, &responses)?;
        }
        response_deliveries.extend(responses);
    }

    Ok(TrafficGupsTransportRun::new(
        scheduled_count,
        response_deliveries,
        trace.snapshot(),
        scheduler.now(),
    ))
}

fn route_uses_fabric(route: &MemoryRoute) -> bool {
    route
        .hops()
        .iter()
        .any(|hop| hop.request_fabric_path().is_some() || hop.response_fabric_path().is_some())
}

fn restore_controller(
    controller: &mut TrafficController,
    checkpoint: rem6_traffic::TrafficControllerSnapshot,
) -> Result<(), TrafficGeneratorError> {
    *controller = TrafficController::restore(checkpoint)?;
    Ok(())
}

#[derive(Debug, Default)]
struct GupsRequestSettlement {
    responses: Vec<ResponseDelivery>,
    no_response_tick: Option<Tick>,
    expected_response_tick: Option<Tick>,
}

impl GupsRequestSettlement {
    fn record_response(&mut self, delivery: ResponseDelivery) {
        self.responses.push(delivery);
    }

    fn record_no_response(&mut self, tick: Tick) {
        self.no_response_tick = Some(tick);
    }

    fn record_expected_response(&mut self, tick: Tick) {
        self.expected_response_tick = Some(tick);
    }

    fn is_settled(&self) -> bool {
        self.no_response_tick.is_some() || !self.responses.is_empty()
    }

    fn tick_limit(&self, target_arrival_tick: Tick) -> Tick {
        self.expected_response_tick.unwrap_or(target_arrival_tick)
    }

    fn settled_tick(&self, request: MemoryRequestId, fallback: Tick) -> Tick {
        if let Some(delivery) = self
            .responses
            .iter()
            .find(|delivery| delivery.response().request_id() == request)
        {
            return delivery.tick();
        }
        if let Some(delivery) = self.responses.first() {
            return delivery.tick();
        }
        self.no_response_tick.unwrap_or(fallback)
    }

    fn into_responses(self) -> Vec<ResponseDelivery> {
        self.responses
    }
}

fn run_until_gups_request_settled(
    scheduler: &mut PartitionedScheduler,
    settlement: &Arc<Mutex<GupsRequestSettlement>>,
    target_arrival_tick: Tick,
) -> Result<(), SchedulerError> {
    loop {
        let (is_settled, tick_limit) = {
            let settlement = settlement.lock().expect("GUPS request settlement lock");
            (
                settlement.is_settled(),
                settlement.tick_limit(target_arrival_tick),
            )
        };

        if is_settled {
            return Ok(());
        }
        let before = scheduler.now();
        let Some((_plan, recorded)) =
            scheduler.run_next_epoch_parallel_recorded_until(tick_limit)?
        else {
            return Ok(());
        };
        let summary = recorded.summary();
        if summary.final_tick() == before && summary.executed_events() == 0 {
            return Ok(());
        }
    }
}

fn take_request_settlement(settlement: Arc<Mutex<GupsRequestSettlement>>) -> GupsRequestSettlement {
    Arc::try_unwrap(settlement)
        .expect("GUPS request settlement has no outstanding references")
        .into_inner()
        .expect("GUPS request settlement lock")
}

fn complete_gups_read_from_responses(
    controller: &mut TrafficController,
    state: TrafficStateId,
    batch: &TrafficControllerEventBatch,
    responses: &[ResponseDelivery],
) -> Result<(), TrafficGupsTransportError> {
    let request = batch
        .request()
        .expect("GUPS read completion is only called for request batches");
    let response = responses
        .iter()
        .find(|delivery| delivery.response().request_id() == request.request().id())
        .ok_or(TrafficGupsTransportError::ReadResponseMissing {
            request: request.request().id(),
        })?
        .response();
    let value = read_response_value(response)?;
    controller.complete_gups_read(state, request.sequence(), value)?;
    Ok(())
}

fn read_response_value(response: &MemoryResponse) -> Result<u64, TrafficGupsTransportError> {
    if response.status() != ResponseStatus::Completed {
        return Err(TrafficGupsTransportError::ReadResponseNotCompleted {
            request: response.request_id(),
            status: response.status(),
        });
    }
    let data = response
        .data()
        .ok_or(TrafficGupsTransportError::ReadResponseMissingData {
            request: response.request_id(),
        })?;
    let bytes: [u8; 8] =
        data.try_into()
            .map_err(|_| TrafficGupsTransportError::ReadResponseDataSize {
                request: response.request_id(),
                actual: data.len(),
            })?;
    Ok(u64::from_le_bytes(bytes))
}

fn checked_tick_add(tick: Tick, delta: Tick) -> Result<Tick, TrafficGupsTransportError> {
    tick.checked_add(delta)
        .ok_or(TrafficGupsTransportError::Scheduler(
            SchedulerError::TickOverflow {
                now: tick,
                delay: delta,
            },
        ))
}

fn response_settlement_tick(
    delivery_tick: Tick,
    target_delay: Tick,
    response_latency: Tick,
) -> Tick {
    delivery_tick
        .checked_add(target_delay)
        .and_then(|tick| tick.checked_add(response_latency))
        .unwrap_or(Tick::MAX)
}

#[derive(Debug)]
pub enum TrafficGupsTransportError {
    Generator(TrafficGeneratorError),
    Transport(TransportError),
    Scheduler(SchedulerError),
    ReadResponseMissing {
        request: MemoryRequestId,
    },
    ReadResponseNotCompleted {
        request: MemoryRequestId,
        status: ResponseStatus,
    },
    ReadResponseMissingData {
        request: MemoryRequestId,
    },
    ReadResponseDataSize {
        request: MemoryRequestId,
        actual: usize,
    },
    UnsupportedControllerBatch {
        state: TrafficStateId,
        event_count: usize,
    },
    UnsupportedFabricRoute {
        unsupported: MemoryRouteId,
    },
}

impl fmt::Display for TrafficGupsTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generator(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::ReadResponseMissing { request } => {
                write!(
                    formatter,
                    "GUPS read response for request {request:?} was not delivered"
                )
            }
            Self::ReadResponseNotCompleted { request, status } => write!(
                formatter,
                "GUPS read response for request {request:?} completed with status {status:?}"
            ),
            Self::ReadResponseMissingData { request } => {
                write!(
                    formatter,
                    "GUPS read response for request {request:?} has no data"
                )
            }
            Self::ReadResponseDataSize { request, actual } => write!(
                formatter,
                "GUPS read response for request {request:?} has {actual} data bytes"
            ),
            Self::UnsupportedControllerBatch { state, event_count } => write!(
                formatter,
                "GUPS transport state {state:?} produced unsupported controller batch with {event_count} events"
            ),
            Self::UnsupportedFabricRoute { unsupported } => write!(
                formatter,
                "GUPS transport route {unsupported:?} uses fabric timing"
            ),
        }
    }
}

impl Error for TrafficGupsTransportError {}

impl From<TrafficGeneratorError> for TrafficGupsTransportError {
    fn from(error: TrafficGeneratorError) -> Self {
        Self::Generator(error)
    }
}

impl From<TransportError> for TrafficGupsTransportError {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

impl From<SchedulerError> for TrafficGupsTransportError {
    fn from(error: SchedulerError) -> Self {
        Self::Scheduler(error)
    }
}
