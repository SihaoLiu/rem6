use std::collections::{BTreeMap, BTreeSet};

use rem6_memory::{Address, MemoryOperation, MemoryRequestId, MemoryResponse, ResponseStatus};

use crate::{
    stream::{apply_stream_ids_to_event, TrafficStreamConfig, TrafficStreamPicker},
    DramTrafficGenerator, GupsTrafficGenerator, HybridTrafficGenerator, LinearTrafficGenerator,
    RandomTrafficGenerator, StridedTrafficGenerator, TrafficDramSnapshot, TrafficExitEvent,
    TrafficExitGenerator, TrafficExitSnapshot, TrafficGeneratorError, TrafficGeneratorSummary,
    TrafficGupsSnapshot, TrafficHybridSnapshot, TrafficIdleGenerator, TrafficIdleSnapshot,
    TrafficLinearSnapshot, TrafficRandomSnapshot, TrafficRequestEvent, TrafficStateGraphConfig,
    TrafficStateId, TrafficStateMachine, TrafficStateSnapshot, TrafficStridedSnapshot,
    TrafficTraceCacheEvent, TrafficTraceDiagnosticEvent, TrafficTraceErrorEvent,
    TrafficTraceErrorKind, TrafficTraceEvent, TrafficTraceExitStatus, TrafficTraceGenerator,
    TrafficTraceHtmEvent, TrafficTraceResponseEvent, TrafficTraceResponseKind,
    TrafficTraceSnapshot, TrafficTraceSyncEvent, TrafficTraceTlbEvent, TrafficTransitionEvent,
};

mod trace_replay_queue;

pub use trace_replay_queue::{
    TrafficTraceControlFailureRecord, TrafficTraceControlFailureSource,
    TrafficTraceMemoryFailureRecord, TrafficTraceMemoryResponseRecord,
    TrafficTraceMemoryWriteCompletionRecord, TrafficTraceReplayActionQueue,
    TrafficTraceReplaySummary,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficControllerConfig {
    graph: TrafficStateGraphConfig,
    states: Vec<TrafficControllerState>,
    stream: Option<TrafficStreamConfig>,
}

impl TrafficControllerConfig {
    pub fn new(
        graph: TrafficStateGraphConfig,
        states: Vec<TrafficControllerState>,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_controller_states(&graph, &states)?;

        let mut states = states;
        states.sort_by_key(TrafficControllerState::id);

        Ok(Self {
            graph,
            states,
            stream: None,
        })
    }

    pub const fn graph(&self) -> &TrafficStateGraphConfig {
        &self.graph
    }

    pub fn states(&self) -> &[TrafficControllerState] {
        &self.states
    }

    pub fn with_stream(mut self, stream: TrafficStreamConfig) -> Self {
        self.stream = Some(stream);
        self
    }

    pub fn stream(&self) -> Option<&TrafficStreamConfig> {
        self.stream.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficControllerState {
    id: TrafficStateId,
    generator: TrafficStateGenerator,
}

impl TrafficControllerState {
    pub const fn new(id: TrafficStateId, generator: TrafficStateGenerator) -> Self {
        Self { id, generator }
    }

    pub const fn id(&self) -> TrafficStateId {
        self.id
    }

    pub const fn generator(&self) -> &TrafficStateGenerator {
        &self.generator
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficStateGenerator {
    Idle(TrafficIdleGenerator),
    Exit(TrafficExitGenerator),
    Linear(LinearTrafficGenerator),
    Random(RandomTrafficGenerator),
    Strided(StridedTrafficGenerator),
    Dram(DramTrafficGenerator),
    Hybrid(HybridTrafficGenerator),
    Gups(GupsTrafficGenerator),
    Trace(TrafficTraceGenerator),
}

impl TrafficStateGenerator {
    fn enter(&mut self, tick: u64) -> Vec<TrafficControllerEvent> {
        match self {
            Self::Idle(generator) => {
                generator.enter();
                Vec::new()
            }
            Self::Exit(generator) => {
                vec![TrafficControllerEvent::Exit(generator.enter(tick))]
            }
            Self::Linear(generator) => {
                generator.enter();
                Vec::new()
            }
            Self::Random(generator) => {
                generator.enter();
                Vec::new()
            }
            Self::Strided(generator) => {
                generator.enter();
                Vec::new()
            }
            Self::Dram(generator) => {
                generator.enter();
                Vec::new()
            }
            Self::Hybrid(generator) => {
                generator.enter();
                Vec::new()
            }
            Self::Gups(generator) => {
                generator.enter();
                Vec::new()
            }
            Self::Trace(generator) => {
                generator.enter(tick);
                Vec::new()
            }
        }
    }

    fn exit(&mut self) -> Option<TrafficTraceExitStatus> {
        match self {
            Self::Trace(generator) => Some(generator.exit()),
            _ => None,
        }
    }

    fn peek_schedule_tick(
        &self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        match self {
            Self::Idle(generator) => generator.schedule_tick(tick, retry_delay),
            Self::Exit(generator) => generator.schedule_tick(tick, retry_delay),
            Self::Linear(generator) => generator.clone().schedule_tick(tick, retry_delay),
            Self::Random(generator) => generator.clone().schedule_tick(tick, retry_delay),
            Self::Strided(generator) => generator.clone().schedule_tick(tick, retry_delay),
            Self::Dram(generator) => generator.clone().schedule_tick(tick, retry_delay),
            Self::Hybrid(generator) => generator.clone().schedule_tick(tick, retry_delay),
            Self::Gups(generator) => generator.schedule_tick(tick, retry_delay),
            Self::Trace(generator) => generator.clone().schedule_tick(tick, retry_delay),
        }
    }

    fn next_event(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<Option<TrafficControllerEvent>, TrafficGeneratorError> {
        let event = match self {
            Self::Idle(generator) => generator
                .next_request(tick, retry_delay)?
                .map(TrafficControllerEvent::Request),
            Self::Exit(generator) => generator
                .next_request(tick, retry_delay)?
                .map(TrafficControllerEvent::Request),
            Self::Linear(generator) => generator
                .next_request(tick, retry_delay)?
                .map(TrafficControllerEvent::Request),
            Self::Random(generator) => generator
                .next_request(tick, retry_delay)?
                .map(TrafficControllerEvent::Request),
            Self::Strided(generator) => generator
                .next_request(tick, retry_delay)?
                .map(TrafficControllerEvent::Request),
            Self::Dram(generator) => generator
                .next_request(tick, retry_delay)?
                .map(TrafficControllerEvent::Request),
            Self::Hybrid(generator) => generator
                .next_request(tick, retry_delay)?
                .map(TrafficControllerEvent::Request),
            Self::Gups(generator) => generator
                .next_request(tick)?
                .map(TrafficControllerEvent::Request),
            Self::Trace(generator) => {
                generator
                    .next_event(tick, retry_delay)?
                    .map(|event| match event {
                        TrafficTraceEvent::Request(request) => {
                            TrafficControllerEvent::Request(request)
                        }
                        TrafficTraceEvent::Sync(sync) => TrafficControllerEvent::TraceSync(sync),
                        TrafficTraceEvent::Tlb(tlb) => TrafficControllerEvent::TraceTlb(tlb),
                        TrafficTraceEvent::Cache(cache) => {
                            TrafficControllerEvent::TraceCache(cache)
                        }
                        TrafficTraceEvent::Htm(htm) => TrafficControllerEvent::TraceHtm(htm),
                        TrafficTraceEvent::Diagnostic(diagnostic) => {
                            TrafficControllerEvent::TraceDiagnostic(diagnostic)
                        }
                        TrafficTraceEvent::Response(response) => {
                            TrafficControllerEvent::TraceResponse(response)
                        }
                        TrafficTraceEvent::Error(error) => {
                            TrafficControllerEvent::TraceError(error)
                        }
                    })
            }
        };
        Ok(event)
    }

    fn summary(&self) -> TrafficGeneratorSummary {
        match self {
            Self::Idle(generator) => generator.summary(),
            Self::Exit(_) => TrafficGeneratorSummary::default(),
            Self::Linear(generator) => generator.summary(),
            Self::Random(generator) => generator.summary(),
            Self::Strided(generator) => generator.summary(),
            Self::Dram(generator) => generator.summary(),
            Self::Hybrid(generator) => generator.summary(),
            Self::Gups(generator) => generator.summary(),
            Self::Trace(generator) => generator.summary(),
        }
    }

    fn blocks_transition(&self) -> bool {
        match self {
            Self::Gups(generator) => !generator.is_complete(),
            _ => false,
        }
    }

    fn snapshot(&self) -> TrafficStateGeneratorSnapshot {
        match self {
            Self::Idle(generator) => TrafficStateGeneratorSnapshot::Idle(generator.snapshot()),
            Self::Exit(generator) => TrafficStateGeneratorSnapshot::Exit(generator.snapshot()),
            Self::Linear(generator) => TrafficStateGeneratorSnapshot::Linear(generator.snapshot()),
            Self::Random(generator) => TrafficStateGeneratorSnapshot::Random(generator.snapshot()),
            Self::Strided(generator) => {
                TrafficStateGeneratorSnapshot::Strided(generator.snapshot())
            }
            Self::Dram(generator) => TrafficStateGeneratorSnapshot::Dram(generator.snapshot()),
            Self::Hybrid(generator) => TrafficStateGeneratorSnapshot::Hybrid(generator.snapshot()),
            Self::Gups(generator) => TrafficStateGeneratorSnapshot::Gups(generator.snapshot()),
            Self::Trace(generator) => TrafficStateGeneratorSnapshot::Trace(generator.snapshot()),
        }
    }

    fn restore(snapshot: TrafficStateGeneratorSnapshot) -> Result<Self, TrafficGeneratorError> {
        match snapshot {
            TrafficStateGeneratorSnapshot::Idle(snapshot) => {
                Ok(Self::Idle(TrafficIdleGenerator::restore(snapshot)))
            }
            TrafficStateGeneratorSnapshot::Exit(snapshot) => {
                Ok(Self::Exit(TrafficExitGenerator::restore(snapshot)))
            }
            TrafficStateGeneratorSnapshot::Linear(snapshot) => {
                Ok(Self::Linear(LinearTrafficGenerator::restore(snapshot)?))
            }
            TrafficStateGeneratorSnapshot::Random(snapshot) => {
                Ok(Self::Random(RandomTrafficGenerator::restore(snapshot)?))
            }
            TrafficStateGeneratorSnapshot::Strided(snapshot) => {
                Ok(Self::Strided(StridedTrafficGenerator::restore(snapshot)?))
            }
            TrafficStateGeneratorSnapshot::Dram(snapshot) => {
                Ok(Self::Dram(DramTrafficGenerator::restore(snapshot)?))
            }
            TrafficStateGeneratorSnapshot::Hybrid(snapshot) => {
                Ok(Self::Hybrid(HybridTrafficGenerator::restore(snapshot)?))
            }
            TrafficStateGeneratorSnapshot::Gups(snapshot) => {
                Ok(Self::Gups(GupsTrafficGenerator::restore(snapshot)?))
            }
            TrafficStateGeneratorSnapshot::Trace(snapshot) => {
                Ok(Self::Trace(TrafficTraceGenerator::restore(snapshot)?))
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficController {
    machine: TrafficStateMachine,
    generators: BTreeMap<TrafficStateId, TrafficStateGenerator>,
    stream_picker: Option<TrafficStreamPicker>,
    trace_pending: Vec<TrafficTraceReplaySource>,
    trace_write_completion_pending: Vec<TrafficTracePendingWriteCompletion>,
    trace_replay_summary: TrafficTraceReplaySummary,
}

impl TrafficController {
    pub fn new(config: TrafficControllerConfig) -> Self {
        let machine = TrafficStateMachine::new(config.graph.clone());
        let stream_picker = config.stream.clone().map(TrafficStreamPicker::new);
        let generators = config
            .states
            .into_iter()
            .map(|state| (state.id, state.generator))
            .collect();
        Self {
            machine,
            generators,
            stream_picker,
            trace_pending: Vec::new(),
            trace_write_completion_pending: Vec::new(),
            trace_replay_summary: TrafficTraceReplaySummary::default(),
        }
    }

    pub fn restore(snapshot: TrafficControllerSnapshot) -> Result<Self, TrafficGeneratorError> {
        let machine = TrafficStateMachine::restore(snapshot.machine().clone())?;
        let states = snapshot
            .generators()
            .iter()
            .map(|state| {
                Ok(TrafficControllerState::new(
                    state.id(),
                    TrafficStateGenerator::restore(state.generator().clone())?,
                ))
            })
            .collect::<Result<Vec<_>, TrafficGeneratorError>>()?;
        let config = TrafficControllerConfig::new(machine.snapshot().config().clone(), states)?;
        let mut controller = Self::new(config);
        controller.machine = machine;
        controller.trace_pending = snapshot.trace_pending().to_vec();
        controller.trace_write_completion_pending = snapshot.trace_write_completion_pending.clone();
        controller.trace_replay_summary = snapshot.trace_replay_summary();
        controller.stream_picker = snapshot.stream().cloned().map(|stream| {
            let rng_state = snapshot
                .stream_rng_state()
                .unwrap_or_else(|| stream.rng_state());
            TrafficStreamPicker::with_rng_state(stream, rng_state)
        });
        Ok(controller)
    }

    pub fn start(
        &mut self,
        tick: u64,
    ) -> Result<TrafficControllerEventBatch, TrafficGeneratorError> {
        self.machine.start(tick)?;
        self.trace_pending.clear();
        self.trace_write_completion_pending.clear();
        self.trace_replay_summary = TrafficTraceReplaySummary::default();
        let events = self.enter_current_state(tick)?;
        Ok(TrafficControllerEventBatch::new(events))
    }

    pub fn next_event(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<Option<TrafficControllerEventBatch>, TrafficGeneratorError> {
        let Some(current) = self.machine.current_state() else {
            return Ok(None);
        };

        let request_tick = self
            .generators
            .get(&current)
            .expect("validated traffic controller has current generator")
            .peek_schedule_tick(tick, retry_delay)?;

        let next_transition_tick = self.machine.next_transition_tick();
        let blocks_transition = self
            .generators
            .get(&current)
            .expect("validated traffic controller has current generator")
            .blocks_transition();
        if blocks_transition && request_tick == u64::MAX {
            return Ok(None);
        }
        if request_tick == u64::MAX && next_transition_tick == u64::MAX {
            let batch = self.transition_at(tick)?;
            return Ok(Some(batch));
        }

        if !blocks_transition && next_transition_tick <= request_tick {
            let batch = self.transition_at(tick.max(next_transition_tick))?;
            return Ok(Some(batch));
        }

        if request_tick == u64::MAX {
            let batch = self.transition_at(tick)?;
            return Ok(Some(batch));
        }

        let event = self
            .generators
            .get_mut(&current)
            .expect("validated traffic controller has current generator")
            .next_event(tick, retry_delay)?;

        let Some(event) = event else {
            return Ok(None);
        };

        let trace_event_source = matches!(
            self.generators.get(&current),
            Some(TrafficStateGenerator::Trace(_))
        );
        let event = self.apply_stream_to_event(event)?;
        let event_tick = event.tick();
        let mut events = self.apply_trace_replay_to_event(event, trace_event_source)?;
        if self.should_force_transition_after_event(current, event_tick)? {
            events.extend(self.transition_at(event_tick)?.into_events());
        }

        Ok(Some(TrafficControllerEventBatch::new(events)))
    }

    pub const fn current_state(&self) -> Option<TrafficStateId> {
        self.machine.current_state()
    }

    pub const fn next_transition_tick(&self) -> u64 {
        self.machine.next_transition_tick()
    }

    pub fn summary(&self) -> TrafficGeneratorSummary {
        self.current_state()
            .and_then(|state| self.generators.get(&state))
            .map_or_else(
                TrafficGeneratorSummary::default,
                TrafficStateGenerator::summary,
            )
    }

    pub const fn trace_replay_summary(&self) -> TrafficTraceReplaySummary {
        self.trace_replay_summary
    }

    pub fn complete_gups_read(
        &mut self,
        state: TrafficStateId,
        sequence: u64,
        value: u64,
    ) -> Result<(), TrafficGeneratorError> {
        match self.generators.get_mut(&state) {
            Some(TrafficStateGenerator::Gups(generator)) => {
                generator.complete_read(sequence, value)
            }
            Some(_) => Err(TrafficGeneratorError::TrafficControllerStateNotGups { state }),
            None => Err(TrafficGeneratorError::TrafficControllerMissingStateGenerator { state }),
        }
    }

    pub fn snapshot(&self) -> TrafficControllerSnapshot {
        let generators = self
            .generators
            .iter()
            .map(|(id, generator)| {
                TrafficStateGeneratorSnapshotEntry::new(*id, generator.snapshot())
            })
            .collect();
        TrafficControllerSnapshot::new(self.machine.snapshot(), generators)
            .with_stream(
                self.stream_picker
                    .as_ref()
                    .map(|picker| picker.config().clone()),
                self.stream_picker
                    .as_ref()
                    .map(TrafficStreamPicker::rng_state),
            )
            .with_trace_pending(self.trace_pending.clone())
            .with_trace_write_completion_pending(self.trace_write_completion_pending.clone())
            .with_trace_replay_summary(self.trace_replay_summary)
    }

    fn transition_at(
        &mut self,
        tick: u64,
    ) -> Result<TrafficControllerEventBatch, TrafficGeneratorError> {
        let from = self
            .machine
            .current_state()
            .expect("active traffic controller has current state");
        let mut events = Vec::new();
        if let Some(status) = self
            .generators
            .get_mut(&from)
            .expect("validated traffic controller has current generator")
            .exit()
        {
            events.push(TrafficControllerEvent::TraceExit(status));
        }
        self.trace_pending.clear();
        self.trace_write_completion_pending.clear();

        let transition = self.machine.transition_now(tick)?;
        events.push(TrafficControllerEvent::Transition(transition));
        events.extend(self.enter_current_state(tick)?);
        Ok(TrafficControllerEventBatch::new(events))
    }

    fn enter_current_state(
        &mut self,
        tick: u64,
    ) -> Result<Vec<TrafficControllerEvent>, TrafficGeneratorError> {
        let state = self
            .machine
            .current_state()
            .expect("started traffic controller has current state");
        Ok(self
            .generators
            .get_mut(&state)
            .expect("validated traffic controller has current generator")
            .enter(tick))
    }

    fn should_force_transition_after_event(
        &self,
        state: TrafficStateId,
        tick: u64,
    ) -> Result<bool, TrafficGeneratorError> {
        let generator = self
            .generators
            .get(&state)
            .expect("validated traffic controller has current generator");
        let request_tick = generator.peek_schedule_tick(tick, 0)?;
        let force_transition =
            request_tick == u64::MAX && self.machine.next_transition_tick() == u64::MAX;
        Ok(force_transition && !generator.blocks_transition())
    }

    fn apply_stream_to_event(
        &mut self,
        event: TrafficControllerEvent,
    ) -> Result<TrafficControllerEvent, TrafficGeneratorError> {
        match (&mut self.stream_picker, event) {
            (Some(stream_picker), TrafficControllerEvent::Request(request)) => {
                Ok(TrafficControllerEvent::Request(apply_stream_ids_to_event(
                    request,
                    stream_picker.next_ids(),
                )?))
            }
            (_, event) => Ok(event),
        }
    }

    fn apply_trace_replay_to_event(
        &mut self,
        event: TrafficControllerEvent,
        trace_event_source: bool,
    ) -> Result<Vec<TrafficControllerEvent>, TrafficGeneratorError> {
        let replay_events = if trace_event_source {
            self.trace_replay_events(&event)?
        } else {
            Vec::new()
        };
        let mut events = vec![event];
        events.extend(replay_events);
        Ok(events)
    }

    fn trace_replay_events(
        &mut self,
        event: &TrafficControllerEvent,
    ) -> Result<Vec<TrafficControllerEvent>, TrafficGeneratorError> {
        match event {
            TrafficControllerEvent::Request(request) if request.request().requires_response() => {
                self.trace_pending
                    .push(TrafficTraceReplaySource::Memory(request.clone()));
                Ok(Vec::new())
            }
            TrafficControllerEvent::TraceSync(sync) if sync.requires_response() => {
                self.trace_pending
                    .push(TrafficTraceReplaySource::Sync(*sync));
                Ok(Vec::new())
            }
            TrafficControllerEvent::TraceTlb(tlb) if should_track_trace_tlb_error(*tlb) => {
                self.trace_pending.push(TrafficTraceReplaySource::Tlb(*tlb));
                Ok(Vec::new())
            }
            TrafficControllerEvent::TraceCache(cache) => {
                self.trace_pending
                    .push(TrafficTraceReplaySource::Cache(*cache));
                Ok(Vec::new())
            }
            TrafficControllerEvent::TraceHtm(htm) if htm.requires_response() => {
                self.trace_pending.push(TrafficTraceReplaySource::Htm(*htm));
                Ok(Vec::new())
            }
            TrafficControllerEvent::TraceHtm(htm) if should_track_trace_htm_error(*htm) => {
                self.trace_pending.push(TrafficTraceReplaySource::Htm(*htm));
                Ok(Vec::new())
            }
            TrafficControllerEvent::TraceDiagnostic(diagnostic)
                if should_track_trace_diagnostic_error(*diagnostic) =>
            {
                self.trace_pending
                    .push(TrafficTraceReplaySource::Diagnostic(*diagnostic));
                Ok(Vec::new())
            }
            TrafficControllerEvent::TraceResponse(response) => {
                if response.kind() == TrafficTraceResponseKind::WriteComplete {
                    if let Some(request) =
                        self.take_matching_trace_write_completion_request(|pending| {
                            response_matches_pending_write_completion(*response, pending)
                        })
                    {
                        let source = TrafficTraceReplaySource::Memory(request.request().clone());
                        let completion = TrafficTraceReplayCompletion::WriteCompletion(
                            TrafficTraceMemoryWriteCompletion::new(
                                request.request().request().id(),
                                request.request().request().line_address(),
                                request.request().request().size().bytes(),
                                *response,
                            ),
                        );
                        self.trace_replay_summary.record_completion(&completion)?;
                        let action =
                            TrafficTraceReplayAction::from_completion(response.tick(), &completion);
                        return Ok(vec![
                            TrafficControllerEvent::TraceResponseMatch(
                                TrafficTraceResponseMatch::new(*response, source, completion),
                            ),
                            TrafficControllerEvent::TraceReplayAction(action),
                        ]);
                    }
                }
                let Some(source) = self.take_matching_trace_source(|source| {
                    response_matches_trace_source(*response, source)
                }) else {
                    return Ok(Vec::new());
                };
                let completion = trace_response_completion(*response, &source)?;
                self.trace_replay_summary.record_completion(&completion)?;
                let action =
                    TrafficTraceReplayAction::from_completion(response.tick(), &completion);
                if response.kind() == TrafficTraceResponseKind::Write {
                    if let TrafficTraceReplaySource::Memory(request) = &source {
                        if let Some(pending) =
                            TrafficTracePendingWriteCompletion::from_write_response(
                                request.clone(),
                                *response,
                            )
                        {
                            self.trace_write_completion_pending.push(pending);
                        }
                    }
                }
                Ok(vec![
                    TrafficControllerEvent::TraceResponseMatch(TrafficTraceResponseMatch::new(
                        *response, source, completion,
                    )),
                    TrafficControllerEvent::TraceReplayAction(action),
                ])
            }
            TrafficControllerEvent::TraceError(error) => {
                let Some(source) = self.take_matching_trace_source(|source| {
                    error_matches_trace_source(*error, source)
                }) else {
                    return Ok(Vec::new());
                };
                let matched = TrafficTraceErrorMatch::new(*error, source);
                self.trace_replay_summary
                    .record_failure(matched.failure())?;
                let action =
                    TrafficTraceReplayAction::from_failure(error.tick(), matched.failure());
                Ok(vec![
                    TrafficControllerEvent::TraceErrorMatch(matched),
                    TrafficControllerEvent::TraceReplayAction(action),
                ])
            }
            _ => Ok(Vec::new()),
        }
    }

    fn take_matching_trace_source(
        &mut self,
        predicate: impl Fn(&TrafficTraceReplaySource) -> bool,
    ) -> Option<TrafficTraceReplaySource> {
        let index = self.trace_pending.iter().position(predicate)?;
        Some(self.trace_pending.remove(index))
    }

    fn take_matching_trace_write_completion_request(
        &mut self,
        predicate: impl Fn(&TrafficTracePendingWriteCompletion) -> bool,
    ) -> Option<TrafficTracePendingWriteCompletion> {
        let index = self
            .trace_write_completion_pending
            .iter()
            .position(predicate)?;
        Some(self.trace_write_completion_pending.remove(index))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrafficTracePendingWriteCompletion {
    request: TrafficRequestEvent,
    packet_id: u64,
}

impl TrafficTracePendingWriteCompletion {
    fn from_write_response(
        request: TrafficRequestEvent,
        response: TrafficTraceResponseEvent,
    ) -> Option<Self> {
        let request_packet_id = request.trace_packet_id()?;
        let response_packet_id = response.trace_packet_id()?;
        if request_packet_id != response_packet_id {
            return None;
        }
        Some(Self {
            request,
            packet_id: request_packet_id,
        })
    }

    const fn request(&self) -> &TrafficRequestEvent {
        &self.request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficControllerEventBatch {
    events: Vec<TrafficControllerEvent>,
}

impl TrafficControllerEventBatch {
    pub const fn new(events: Vec<TrafficControllerEvent>) -> Self {
        Self { events }
    }

    pub fn events(&self) -> &[TrafficControllerEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<TrafficControllerEvent> {
        self.events
    }

    pub fn request(&self) -> Option<&TrafficRequestEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::Request(request) => Some(request),
            _ => None,
        })
    }

    pub fn transition(&self) -> Option<TrafficTransitionEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::Transition(transition) => Some(*transition),
            _ => None,
        })
    }

    pub fn exit(&self) -> Option<TrafficExitEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::Exit(exit) => Some(*exit),
            _ => None,
        })
    }

    pub fn trace_exit(&self) -> Option<TrafficTraceExitStatus> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceExit(status) => Some(*status),
            _ => None,
        })
    }

    pub fn trace_sync(&self) -> Option<TrafficTraceSyncEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceSync(sync) => Some(*sync),
            _ => None,
        })
    }

    pub fn trace_tlb(&self) -> Option<TrafficTraceTlbEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceTlb(tlb) => Some(*tlb),
            _ => None,
        })
    }

    pub fn trace_cache(&self) -> Option<TrafficTraceCacheEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceCache(cache) => Some(*cache),
            _ => None,
        })
    }

    pub fn trace_htm(&self) -> Option<TrafficTraceHtmEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceHtm(htm) => Some(*htm),
            _ => None,
        })
    }

    pub fn trace_diagnostic(&self) -> Option<TrafficTraceDiagnosticEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceDiagnostic(diagnostic) => Some(*diagnostic),
            _ => None,
        })
    }

    pub fn trace_error(&self) -> Option<TrafficTraceErrorEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceError(error) => Some(*error),
            _ => None,
        })
    }

    pub fn trace_response(&self) -> Option<TrafficTraceResponseEvent> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceResponse(response) => Some(*response),
            _ => None,
        })
    }

    pub fn trace_response_match(&self) -> Option<&TrafficTraceResponseMatch> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceResponseMatch(response) => Some(response),
            _ => None,
        })
    }

    pub fn trace_error_match(&self) -> Option<&TrafficTraceErrorMatch> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceErrorMatch(error) => Some(error),
            _ => None,
        })
    }

    pub fn trace_replay_outcome(&self) -> Option<TrafficTraceReplayOutcome<'_>> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceResponseMatch(response) => {
                Some(TrafficTraceReplayOutcome::Completion(response))
            }
            TrafficControllerEvent::TraceErrorMatch(error) => {
                Some(TrafficTraceReplayOutcome::Failure(error))
            }
            _ => None,
        })
    }

    pub fn trace_replay_action(&self) -> Option<&TrafficTraceReplayAction> {
        self.events.iter().find_map(|event| match event {
            TrafficControllerEvent::TraceReplayAction(action) => Some(action),
            _ => None,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficControllerEvent {
    Request(TrafficRequestEvent),
    Transition(TrafficTransitionEvent),
    Exit(TrafficExitEvent),
    TraceExit(TrafficTraceExitStatus),
    TraceSync(TrafficTraceSyncEvent),
    TraceTlb(TrafficTraceTlbEvent),
    TraceCache(TrafficTraceCacheEvent),
    TraceHtm(TrafficTraceHtmEvent),
    TraceDiagnostic(TrafficTraceDiagnosticEvent),
    TraceResponse(TrafficTraceResponseEvent),
    TraceError(TrafficTraceErrorEvent),
    TraceResponseMatch(TrafficTraceResponseMatch),
    TraceErrorMatch(TrafficTraceErrorMatch),
    TraceReplayAction(TrafficTraceReplayAction),
}

impl TrafficControllerEvent {
    fn tick(&self) -> u64 {
        match self {
            Self::Request(request) => request.tick(),
            Self::Transition(transition) => transition.tick(),
            Self::Exit(exit) => exit.tick(),
            Self::TraceExit(_) => u64::MAX,
            Self::TraceSync(sync) => sync.tick(),
            Self::TraceTlb(tlb) => tlb.tick(),
            Self::TraceCache(cache) => cache.tick(),
            Self::TraceHtm(htm) => htm.tick(),
            Self::TraceDiagnostic(diagnostic) => diagnostic.tick(),
            Self::TraceResponse(response) => response.tick(),
            Self::TraceError(error) => error.tick(),
            Self::TraceResponseMatch(response) => response.response().tick(),
            Self::TraceErrorMatch(error) => error.error().tick(),
            Self::TraceReplayAction(action) => action.tick(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplaySource {
    Memory(TrafficRequestEvent),
    Sync(TrafficTraceSyncEvent),
    Tlb(TrafficTraceTlbEvent),
    Cache(TrafficTraceCacheEvent),
    Htm(TrafficTraceHtmEvent),
    Diagnostic(TrafficTraceDiagnosticEvent),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTraceMemoryCompletion {
    response: MemoryResponse,
    trace_data: Option<Vec<u8>>,
}

impl TrafficTraceMemoryCompletion {
    pub fn new(response: MemoryResponse, trace_data: Option<Vec<u8>>) -> Self {
        Self {
            response,
            trace_data,
        }
    }

    pub const fn response(&self) -> &MemoryResponse {
        &self.response
    }

    pub const fn request_id(&self) -> MemoryRequestId {
        self.response.request_id()
    }

    pub const fn status(&self) -> ResponseStatus {
        self.response.status()
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.response.data()
    }

    pub fn trace_data(&self) -> Option<&[u8]> {
        self.trace_data.as_deref()
    }

    pub fn into_response(self) -> MemoryResponse {
        self.response
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceMemoryWriteCompletion {
    request_id: MemoryRequestId,
    request_line: Address,
    request_size_bytes: u64,
    response: TrafficTraceResponseEvent,
}

impl TrafficTraceMemoryWriteCompletion {
    pub const fn new(
        request_id: MemoryRequestId,
        request_line: Address,
        request_size_bytes: u64,
        response: TrafficTraceResponseEvent,
    ) -> Self {
        Self {
            request_id,
            request_line,
            request_size_bytes,
            response,
        }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayCompletion {
    Memory(TrafficTraceMemoryCompletion),
    WriteCompletion(TrafficTraceMemoryWriteCompletion),
    Ack,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayFailure {
    Memory(TrafficTraceMemoryFailure),
    Control(TrafficTraceControlFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayOutcome<'a> {
    Completion(&'a TrafficTraceResponseMatch),
    Failure(&'a TrafficTraceErrorMatch),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceReplayAction {
    MemoryResponse {
        tick: u64,
        response: MemoryResponse,
        trace_data: Option<Vec<u8>>,
    },
    MemoryWriteCompletion {
        tick: u64,
        request: MemoryRequestId,
        request_line: Address,
        request_size_bytes: u64,
        response: TrafficTraceResponseEvent,
    },
    ControlAck {
        tick: u64,
    },
    MemoryFailure {
        tick: u64,
        failure: TrafficTraceMemoryFailure,
    },
    ControlFailure {
        tick: u64,
        failure: TrafficTraceControlFailure,
    },
}

impl TrafficTraceReplayAction {
    fn from_completion(tick: u64, completion: &TrafficTraceReplayCompletion) -> Self {
        match completion {
            TrafficTraceReplayCompletion::Memory(completion) => Self::MemoryResponse {
                tick,
                response: completion.response.clone(),
                trace_data: completion.trace_data.clone(),
            },
            TrafficTraceReplayCompletion::WriteCompletion(completion) => {
                Self::MemoryWriteCompletion {
                    tick,
                    request: completion.request_id(),
                    request_line: completion.request_line(),
                    request_size_bytes: completion.request_size_bytes(),
                    response: completion.response(),
                }
            }
            TrafficTraceReplayCompletion::Ack => Self::ControlAck { tick },
        }
    }

    fn from_failure(tick: u64, failure: &TrafficTraceReplayFailure) -> Self {
        match failure {
            TrafficTraceReplayFailure::Memory(failure) => Self::MemoryFailure {
                tick,
                failure: *failure,
            },
            TrafficTraceReplayFailure::Control(failure) => Self::ControlFailure {
                tick,
                failure: *failure,
            },
        }
    }

    pub const fn tick(&self) -> u64 {
        match self {
            Self::MemoryResponse { tick, .. }
            | Self::MemoryWriteCompletion { tick, .. }
            | Self::ControlAck { tick }
            | Self::MemoryFailure { tick, .. }
            | Self::ControlFailure { tick, .. } => *tick,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceMemoryFailure {
    request_id: MemoryRequestId,
    error: TrafficTraceErrorKind,
}

impl TrafficTraceMemoryFailure {
    pub const fn new(request_id: MemoryRequestId, error: TrafficTraceErrorKind) -> Self {
        Self { request_id, error }
    }

    pub const fn request_id(self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn error(self) -> TrafficTraceErrorKind {
        self.error
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceControlFailure {
    error: TrafficTraceErrorKind,
}

impl TrafficTraceControlFailure {
    pub const fn new(error: TrafficTraceErrorKind) -> Self {
        Self { error }
    }

    pub const fn error(self) -> TrafficTraceErrorKind {
        self.error
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTraceResponseMatch {
    response: TrafficTraceResponseEvent,
    source: TrafficTraceReplaySource,
    completion: TrafficTraceReplayCompletion,
}

impl TrafficTraceResponseMatch {
    pub const fn new(
        response: TrafficTraceResponseEvent,
        source: TrafficTraceReplaySource,
        completion: TrafficTraceReplayCompletion,
    ) -> Self {
        Self {
            response,
            source,
            completion,
        }
    }

    pub const fn response(&self) -> TrafficTraceResponseEvent {
        self.response
    }

    pub const fn source(&self) -> &TrafficTraceReplaySource {
        &self.source
    }

    pub const fn completion(&self) -> &TrafficTraceReplayCompletion {
        &self.completion
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTraceErrorMatch {
    error: TrafficTraceErrorEvent,
    source: TrafficTraceReplaySource,
    failure: TrafficTraceReplayFailure,
}

impl TrafficTraceErrorMatch {
    pub fn new(error: TrafficTraceErrorEvent, source: TrafficTraceReplaySource) -> Self {
        let failure = trace_error_failure(error, &source);
        Self {
            error,
            source,
            failure,
        }
    }

    pub const fn error(&self) -> TrafficTraceErrorEvent {
        self.error
    }

    pub const fn source(&self) -> &TrafficTraceReplaySource {
        &self.source
    }

    pub const fn failure(&self) -> &TrafficTraceReplayFailure {
        &self.failure
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficControllerSnapshot {
    machine: TrafficStateSnapshot,
    generators: Vec<TrafficStateGeneratorSnapshotEntry>,
    stream: Option<TrafficStreamConfig>,
    stream_rng_state: Option<u64>,
    trace_pending: Vec<TrafficTraceReplaySource>,
    trace_write_completion_pending: Vec<TrafficTracePendingWriteCompletion>,
    trace_replay_summary: TrafficTraceReplaySummary,
}

impl TrafficControllerSnapshot {
    pub const fn new(
        machine: TrafficStateSnapshot,
        generators: Vec<TrafficStateGeneratorSnapshotEntry>,
    ) -> Self {
        Self {
            machine,
            generators,
            stream: None,
            stream_rng_state: None,
            trace_pending: Vec::new(),
            trace_write_completion_pending: Vec::new(),
            trace_replay_summary: TrafficTraceReplaySummary {
                memory_completions: 0,
                write_completions: 0,
                control_completions: 0,
                memory_failures: 0,
                control_failures: 0,
            },
        }
    }

    pub fn with_stream(
        mut self,
        stream: Option<TrafficStreamConfig>,
        stream_rng_state: Option<u64>,
    ) -> Self {
        self.stream = stream;
        self.stream_rng_state = stream_rng_state;
        self
    }

    pub fn with_trace_pending(mut self, trace_pending: Vec<TrafficTraceReplaySource>) -> Self {
        self.trace_pending = trace_pending;
        self
    }

    fn with_trace_write_completion_pending(
        mut self,
        trace_write_completion_pending: Vec<TrafficTracePendingWriteCompletion>,
    ) -> Self {
        self.trace_write_completion_pending = trace_write_completion_pending;
        self
    }

    pub const fn with_trace_replay_summary(
        mut self,
        trace_replay_summary: TrafficTraceReplaySummary,
    ) -> Self {
        self.trace_replay_summary = trace_replay_summary;
        self
    }

    pub const fn machine(&self) -> &TrafficStateSnapshot {
        &self.machine
    }

    pub fn generators(&self) -> &[TrafficStateGeneratorSnapshotEntry] {
        &self.generators
    }

    pub fn stream(&self) -> Option<&TrafficStreamConfig> {
        self.stream.as_ref()
    }

    pub const fn stream_rng_state(&self) -> Option<u64> {
        self.stream_rng_state
    }

    pub fn trace_pending(&self) -> &[TrafficTraceReplaySource] {
        &self.trace_pending
    }

    pub const fn trace_replay_summary(&self) -> TrafficTraceReplaySummary {
        self.trace_replay_summary
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficStateGeneratorSnapshotEntry {
    id: TrafficStateId,
    generator: TrafficStateGeneratorSnapshot,
}

impl TrafficStateGeneratorSnapshotEntry {
    pub const fn new(id: TrafficStateId, generator: TrafficStateGeneratorSnapshot) -> Self {
        Self { id, generator }
    }

    pub const fn id(&self) -> TrafficStateId {
        self.id
    }

    pub const fn generator(&self) -> &TrafficStateGeneratorSnapshot {
        &self.generator
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficStateGeneratorSnapshot {
    Idle(TrafficIdleSnapshot),
    Exit(TrafficExitSnapshot),
    Linear(TrafficLinearSnapshot),
    Random(TrafficRandomSnapshot),
    Strided(TrafficStridedSnapshot),
    Dram(TrafficDramSnapshot),
    Hybrid(TrafficHybridSnapshot),
    Gups(TrafficGupsSnapshot),
    Trace(TrafficTraceSnapshot),
}

fn validate_controller_states(
    graph: &TrafficStateGraphConfig,
    states: &[TrafficControllerState],
) -> Result<(), TrafficGeneratorError> {
    let graph_states = graph
        .states()
        .iter()
        .map(|state| state.id())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();

    for state in states {
        if !graph_states.contains(&state.id()) {
            return Err(
                TrafficGeneratorError::TrafficControllerUnknownStateGenerator { state: state.id() },
            );
        }
        if !seen.insert(state.id()) {
            return Err(
                TrafficGeneratorError::TrafficControllerDuplicateStateGenerator {
                    state: state.id(),
                },
            );
        }
    }

    for state in graph_states {
        if !seen.contains(&state) {
            return Err(TrafficGeneratorError::TrafficControllerMissingStateGenerator { state });
        }
    }

    Ok(())
}

fn response_matches_trace_source(
    response: TrafficTraceResponseEvent,
    source: &TrafficTraceReplaySource,
) -> bool {
    match source {
        TrafficTraceReplaySource::Memory(request) => {
            memory_trace_metadata_matches(
                response.trace_packet_id(),
                response.address(),
                response.trace_address_is_physical(),
                response.size_bytes(),
                request.trace_packet_id(),
                request.address(),
                request.request().size().bytes(),
            ) && response_matches_memory_operation(response.kind(), request.request().operation())
        }
        TrafficTraceReplaySource::Sync(sync) => {
            control_packet_ids_match(response.trace_packet_id(), sync.trace_packet_id())
                && matches!(
                    (sync.kind(), response.kind()),
                    (
                        crate::TrafficTraceSyncKind::MemFence,
                        TrafficTraceResponseKind::MemFence
                    ) | (
                        crate::TrafficTraceSyncKind::MemSync,
                        TrafficTraceResponseKind::MemSync
                    )
                )
        }
        TrafficTraceReplaySource::Tlb(_)
        | TrafficTraceReplaySource::Cache(_)
        | TrafficTraceReplaySource::Diagnostic(_) => false,
        TrafficTraceReplaySource::Htm(htm) => {
            control_packet_ids_match(response.trace_packet_id(), htm.trace_packet_id())
                && trace_address_matches(response.address(), htm.address())
                && trace_size_matches(response.size_bytes(), htm.size_bytes())
                && matches!(
                    (htm.kind(), response.kind()),
                    (
                        crate::TrafficTraceHtmKind::Request,
                        TrafficTraceResponseKind::HtmRequest
                    )
                )
        }
    }
}

fn response_matches_pending_write_completion(
    response: TrafficTraceResponseEvent,
    pending: &TrafficTracePendingWriteCompletion,
) -> bool {
    if response.kind() != TrafficTraceResponseKind::WriteComplete {
        return false;
    }

    let size_matches = if response.trace_address_is_physical() {
        response.size_bytes() == Some(pending.request().request().size().bytes())
    } else {
        trace_size_matches(
            response.size_bytes(),
            Some(pending.request().request().size().bytes()),
        )
    };

    response.trace_packet_id() == Some(pending.packet_id)
        && (response.trace_address_is_physical()
            || trace_address_matches(response.address(), Some(pending.request().address())))
        && size_matches
        && matches!(
            pending.request().request().operation(),
            MemoryOperation::Write | MemoryOperation::CacheBlockZero
        )
}

fn error_matches_trace_source(
    error: TrafficTraceErrorEvent,
    source: &TrafficTraceReplaySource,
) -> bool {
    match source {
        TrafficTraceReplaySource::Memory(request) => {
            if !memory_trace_metadata_matches(
                error.trace_packet_id(),
                error.address(),
                error.trace_address_is_physical(),
                error.size_bytes(),
                request.trace_packet_id(),
                request.address(),
                request.request().size().bytes(),
            ) {
                return false;
            }
            error_matches_memory_operation(error, request.request().operation())
        }
        TrafficTraceReplaySource::Sync(sync) => {
            control_packet_ids_match(error.trace_packet_id(), sync.trace_packet_id())
                && !error.is_read()
                && !error.is_write()
        }
        TrafficTraceReplaySource::Tlb(tlb) => {
            control_packet_ids_match(error.trace_packet_id(), tlb.trace_packet_id())
                && !error.is_read()
                && !error.is_write()
        }
        TrafficTraceReplaySource::Cache(cache) => {
            sideband_error_metadata_matches(
                error.trace_packet_id(),
                error.address(),
                error.size_bytes(),
                cache.trace_packet_id(),
                Some(cache.address()),
                Some(cache.size_bytes()),
            ) && error_matches_cache_event(error, *cache)
        }
        TrafficTraceReplaySource::Htm(htm) => {
            if htm.requires_response() {
                control_packet_ids_match(error.trace_packet_id(), htm.trace_packet_id())
                    && trace_address_matches(error.address(), htm.address())
                    && trace_size_matches(error.size_bytes(), htm.size_bytes())
                    && error_matches_htm_event(error, *htm)
            } else {
                sideband_error_metadata_matches(
                    error.trace_packet_id(),
                    error.address(),
                    error.size_bytes(),
                    htm.trace_packet_id(),
                    htm.address(),
                    htm.size_bytes(),
                ) && error_matches_htm_event(error, *htm)
            }
        }
        TrafficTraceReplaySource::Diagnostic(diagnostic) => {
            sideband_error_metadata_matches(
                error.trace_packet_id(),
                error.address(),
                error.size_bytes(),
                diagnostic.trace_packet_id(),
                diagnostic.address(),
                diagnostic.size_bytes(),
            ) && !error.is_read()
                && !error.is_write()
        }
    }
}

fn memory_trace_metadata_matches(
    trace_packet_id: Option<u64>,
    trace_address: Option<rem6_memory::Address>,
    trace_address_is_physical: bool,
    trace_size_bytes: Option<u64>,
    source_packet_id: Option<u64>,
    source_address: rem6_memory::Address,
    source_size_bytes: u64,
) -> bool {
    if trace_address_is_physical {
        return matches!(
            (trace_packet_id, source_packet_id, trace_size_bytes),
            (Some(trace_packet_id), Some(source_packet_id), Some(trace_size_bytes))
                if trace_packet_id == source_packet_id && trace_size_bytes == source_size_bytes
        );
    }

    if let (Some(trace_packet_id), Some(source_packet_id)) = (trace_packet_id, source_packet_id) {
        if trace_packet_id != source_packet_id {
            return false;
        }
        return trace_address_matches(trace_address, Some(source_address))
            && trace_size_matches(trace_size_bytes, Some(source_size_bytes));
    }

    trace_address == Some(source_address) && trace_size_bytes == Some(source_size_bytes)
}

fn sideband_error_metadata_matches(
    trace_packet_id: Option<u64>,
    trace_address: Option<rem6_memory::Address>,
    trace_size_bytes: Option<u64>,
    source_packet_id: Option<u64>,
    source_address: Option<rem6_memory::Address>,
    source_size_bytes: Option<u64>,
) -> bool {
    let address_matches = trace_address_matches(trace_address, source_address);
    let size_matches = trace_size_matches(trace_size_bytes, source_size_bytes);
    match (trace_packet_id, source_packet_id) {
        (Some(trace_packet_id), Some(source_packet_id)) => {
            return trace_packet_id == source_packet_id && address_matches && size_matches;
        }
        (Some(_), None) => return false,
        _ => {}
    }

    let mut matched_metadata = false;
    if let Some(source_address) = source_address {
        if trace_address != Some(source_address) {
            return false;
        }
        matched_metadata = true;
    }
    if let Some(source_size_bytes) = source_size_bytes {
        if trace_size_bytes != Some(source_size_bytes) {
            return false;
        }
        matched_metadata = true;
    }
    matched_metadata
}

fn control_packet_ids_match(error: Option<u64>, source: Option<u64>) -> bool {
    error == source
}

fn trace_address_matches(
    response: Option<rem6_memory::Address>,
    source: Option<rem6_memory::Address>,
) -> bool {
    match (response, source) {
        (Some(response), Some(source)) => response == source,
        _ => true,
    }
}

fn trace_size_matches(response: Option<u64>, source: Option<u64>) -> bool {
    match (response, source) {
        (Some(response), Some(source)) => response == source,
        _ => true,
    }
}

fn response_matches_memory_operation(
    response: TrafficTraceResponseKind,
    operation: MemoryOperation,
) -> bool {
    match response {
        TrafficTraceResponseKind::Read | TrafficTraceResponseKind::ReadWithInvalidate => matches!(
            operation,
            MemoryOperation::InstructionFetch
                | MemoryOperation::ReadShared
                | MemoryOperation::LoadLocked
        ),
        TrafficTraceResponseKind::ReadExclusive => operation == MemoryOperation::ReadUnique,
        TrafficTraceResponseKind::Write => matches!(
            operation,
            MemoryOperation::Write | MemoryOperation::CacheBlockZero
        ),
        TrafficTraceResponseKind::WriteComplete => false,
        TrafficTraceResponseKind::SoftPrefetch | TrafficTraceResponseKind::HardPrefetch => {
            matches!(
                operation,
                MemoryOperation::PrefetchRead | MemoryOperation::PrefetchWrite
            )
        }
        TrafficTraceResponseKind::Upgrade => {
            matches!(
                operation,
                MemoryOperation::Upgrade | MemoryOperation::StoreConditionalUpgrade
            )
        }
        TrafficTraceResponseKind::UpgradeFail => {
            matches!(
                operation,
                MemoryOperation::StoreConditionalUpgrade
                    | MemoryOperation::StoreConditionalUpgradeFail
            )
        }
        TrafficTraceResponseKind::StoreConditional => {
            matches!(
                operation,
                MemoryOperation::StoreConditional | MemoryOperation::StoreConditionalFail
            )
        }
        TrafficTraceResponseKind::LockedRmwRead => operation == MemoryOperation::LockedRmwRead,
        TrafficTraceResponseKind::LockedRmwWrite => operation == MemoryOperation::LockedRmwWrite,
        TrafficTraceResponseKind::Swap => {
            matches!(
                operation,
                MemoryOperation::Atomic | MemoryOperation::AtomicNoReturn
            )
        }
        TrafficTraceResponseKind::CleanShared => operation == MemoryOperation::CleanShared,
        TrafficTraceResponseKind::CleanInvalid => operation == MemoryOperation::Invalidate,
        TrafficTraceResponseKind::Invalidate => operation == MemoryOperation::InvalidateWritable,
        TrafficTraceResponseKind::MemSync
        | TrafficTraceResponseKind::MemFence
        | TrafficTraceResponseKind::HtmRequest => false,
    }
}

fn error_matches_memory_operation(
    error: TrafficTraceErrorEvent,
    operation: MemoryOperation,
) -> bool {
    if error.is_read() {
        return matches!(
            operation,
            MemoryOperation::InstructionFetch
                | MemoryOperation::ReadShared
                | MemoryOperation::ReadUnique
                | MemoryOperation::LoadLocked
                | MemoryOperation::LockedRmwRead
                | MemoryOperation::StoreConditionalUpgradeFail
                | MemoryOperation::Atomic
                | MemoryOperation::AtomicNoReturn
                | MemoryOperation::PrefetchRead
        );
    }
    if error.is_write() {
        return matches!(
            operation,
            MemoryOperation::Write
                | MemoryOperation::CacheBlockZero
                | MemoryOperation::StoreConditional
                | MemoryOperation::StoreConditionalFail
                | MemoryOperation::LockedRmwWrite
                | MemoryOperation::Atomic
                | MemoryOperation::AtomicNoReturn
                | MemoryOperation::PrefetchWrite
                | MemoryOperation::WriteClean
                | MemoryOperation::WritebackClean
                | MemoryOperation::WritebackDirty
        );
    }
    true
}

fn error_matches_cache_event(error: TrafficTraceErrorEvent, event: TrafficTraceCacheEvent) -> bool {
    if error.is_read() {
        return false;
    }
    if error.is_write() {
        return event.requires_writable();
    }
    true
}

fn error_matches_htm_event(error: TrafficTraceErrorEvent, event: TrafficTraceHtmEvent) -> bool {
    if error.is_read() {
        return event.is_read();
    }
    if error.is_write() {
        return false;
    }
    true
}

fn should_track_trace_tlb_error(event: TrafficTraceTlbEvent) -> bool {
    event.trace_packet_id().is_some()
}

fn should_track_trace_htm_error(event: TrafficTraceHtmEvent) -> bool {
    event.trace_packet_id().is_some() || event.address().is_some() || event.size_bytes().is_some()
}

fn should_track_trace_diagnostic_error(event: TrafficTraceDiagnosticEvent) -> bool {
    event.trace_packet_id().is_some() || event.address().is_some() || event.size_bytes().is_some()
}

fn trace_response_completion(
    response: TrafficTraceResponseEvent,
    source: &TrafficTraceReplaySource,
) -> Result<TrafficTraceReplayCompletion, TrafficGeneratorError> {
    match source {
        TrafficTraceReplaySource::Memory(request) => {
            if request.request().operation() == MemoryOperation::StoreConditionalFail
                || (request.request().operation() == MemoryOperation::StoreConditionalUpgrade
                    && response.kind() == TrafficTraceResponseKind::UpgradeFail)
            {
                let trace_data = if response.returns_data() {
                    let size = usize::try_from(request.request().size().bytes())
                        .expect("memory request size fits usize after construction");
                    Some(vec![0; size])
                } else {
                    None
                };
                return MemoryResponse::store_conditional_failed(request.request())
                    .map(|response| {
                        TrafficTraceReplayCompletion::Memory(TrafficTraceMemoryCompletion::new(
                            response, trace_data,
                        ))
                    })
                    .map_err(Into::into);
            }
            let trace_data = if response.returns_data() {
                let size = usize::try_from(request.request().size().bytes())
                    .expect("memory request size fits usize after construction");
                Some(vec![0; size])
            } else {
                None
            };
            let data = if request.request().returns_data() {
                trace_data.clone()
            } else {
                None
            };
            MemoryResponse::completed(request.request(), data)
                .map(|response| {
                    TrafficTraceReplayCompletion::Memory(TrafficTraceMemoryCompletion::new(
                        response, trace_data,
                    ))
                })
                .map_err(Into::into)
        }
        TrafficTraceReplaySource::Sync(_)
        | TrafficTraceReplaySource::Tlb(_)
        | TrafficTraceReplaySource::Cache(_)
        | TrafficTraceReplaySource::Htm(_)
        | TrafficTraceReplaySource::Diagnostic(_) => {
            debug_assert!(!response.is_write());
            Ok(TrafficTraceReplayCompletion::Ack)
        }
    }
}

fn trace_error_failure(
    error: TrafficTraceErrorEvent,
    source: &TrafficTraceReplaySource,
) -> TrafficTraceReplayFailure {
    match source {
        TrafficTraceReplaySource::Memory(request) => TrafficTraceReplayFailure::Memory(
            TrafficTraceMemoryFailure::new(request.request().id(), error.kind()),
        ),
        TrafficTraceReplaySource::Sync(_)
        | TrafficTraceReplaySource::Tlb(_)
        | TrafficTraceReplaySource::Cache(_)
        | TrafficTraceReplaySource::Htm(_)
        | TrafficTraceReplaySource::Diagnostic(_) => {
            TrafficTraceReplayFailure::Control(TrafficTraceControlFailure::new(error.kind()))
        }
    }
}
