use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DramTrafficGenerator, GupsTrafficGenerator, HybridTrafficGenerator, LinearTrafficGenerator,
    RandomTrafficGenerator, StridedTrafficGenerator, TrafficDramSnapshot, TrafficExitEvent,
    TrafficExitGenerator, TrafficExitSnapshot, TrafficGeneratorError, TrafficGeneratorSummary,
    TrafficGupsSnapshot, TrafficHybridSnapshot, TrafficIdleGenerator, TrafficIdleSnapshot,
    TrafficLinearSnapshot, TrafficRandomSnapshot, TrafficRequestEvent, TrafficStateGraphConfig,
    TrafficStateId, TrafficStateMachine, TrafficStateSnapshot, TrafficStridedSnapshot,
    TrafficTraceCacheEvent, TrafficTraceDiagnosticEvent, TrafficTraceErrorEvent, TrafficTraceEvent,
    TrafficTraceExitStatus, TrafficTraceGenerator, TrafficTraceHtmEvent, TrafficTraceResponseEvent,
    TrafficTraceSnapshot, TrafficTraceSyncEvent, TrafficTraceTlbEvent, TrafficTransitionEvent,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficControllerConfig {
    graph: TrafficStateGraphConfig,
    states: Vec<TrafficControllerState>,
}

impl TrafficControllerConfig {
    pub fn new(
        graph: TrafficStateGraphConfig,
        states: Vec<TrafficControllerState>,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_controller_states(&graph, &states)?;

        let mut states = states;
        states.sort_by_key(TrafficControllerState::id);

        Ok(Self { graph, states })
    }

    pub const fn graph(&self) -> &TrafficStateGraphConfig {
        &self.graph
    }

    pub fn states(&self) -> &[TrafficControllerState] {
        &self.states
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
}

impl TrafficController {
    pub fn new(config: TrafficControllerConfig) -> Self {
        let machine = TrafficStateMachine::new(config.graph.clone());
        let generators = config
            .states
            .into_iter()
            .map(|state| (state.id, state.generator))
            .collect();
        Self {
            machine,
            generators,
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
        Ok(controller)
    }

    pub fn start(
        &mut self,
        tick: u64,
    ) -> Result<TrafficControllerEventBatch, TrafficGeneratorError> {
        self.machine.start(tick)?;
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

        let event_tick = event.tick();
        let mut events = vec![event];
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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficControllerSnapshot {
    machine: TrafficStateSnapshot,
    generators: Vec<TrafficStateGeneratorSnapshotEntry>,
}

impl TrafficControllerSnapshot {
    pub const fn new(
        machine: TrafficStateSnapshot,
        generators: Vec<TrafficStateGeneratorSnapshotEntry>,
    ) -> Self {
        Self {
            machine,
            generators,
        }
    }

    pub const fn machine(&self) -> &TrafficStateSnapshot {
        &self.machine
    }

    pub fn generators(&self) -> &[TrafficStateGeneratorSnapshotEntry] {
        &self.generators
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
