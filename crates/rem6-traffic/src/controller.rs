use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DramTrafficGenerator, LinearTrafficGenerator, RandomTrafficGenerator, StridedTrafficGenerator,
    TrafficDramSnapshot, TrafficExitEvent, TrafficExitGenerator, TrafficExitSnapshot,
    TrafficGeneratorError, TrafficGeneratorSummary, TrafficIdleGenerator, TrafficIdleSnapshot,
    TrafficLinearSnapshot, TrafficRandomSnapshot, TrafficRequestEvent, TrafficStateGraphConfig,
    TrafficStateId, TrafficStateMachine, TrafficStateSnapshot, TrafficStridedSnapshot,
    TrafficTraceExitStatus, TrafficTraceGenerator, TrafficTraceSnapshot, TrafficTransitionEvent,
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
            Self::Trace(generator) => generator.clone().schedule_tick(tick, retry_delay),
        }
    }

    fn next_request(
        &mut self,
        tick: u64,
        retry_delay: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        match self {
            Self::Idle(generator) => generator.next_request(tick, retry_delay),
            Self::Exit(generator) => generator.next_request(tick, retry_delay),
            Self::Linear(generator) => generator.next_request(tick, retry_delay),
            Self::Random(generator) => generator.next_request(tick, retry_delay),
            Self::Strided(generator) => generator.next_request(tick, retry_delay),
            Self::Dram(generator) => generator.next_request(tick, retry_delay),
            Self::Trace(generator) => generator.next_request(tick, retry_delay),
        }
    }

    fn summary(&self) -> TrafficGeneratorSummary {
        match self {
            Self::Idle(generator) => generator.summary(),
            Self::Exit(_) => TrafficGeneratorSummary::default(),
            Self::Linear(generator) => generator.summary(),
            Self::Random(generator) => generator.summary(),
            Self::Strided(generator) => generator.summary(),
            Self::Dram(generator) => generator.summary(),
            Self::Trace(generator) => generator.summary(),
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
        if request_tick == u64::MAX && next_transition_tick == u64::MAX {
            let batch = self.transition_at(tick)?;
            return Ok(Some(batch));
        }

        if next_transition_tick <= request_tick {
            let batch = self.transition_at(tick.max(next_transition_tick))?;
            return Ok(Some(batch));
        }

        if request_tick == u64::MAX {
            let batch = self.transition_at(tick)?;
            return Ok(Some(batch));
        }

        let request = self
            .generators
            .get_mut(&current)
            .expect("validated traffic controller has current generator")
            .next_request(tick, retry_delay)?;

        let Some(request) = request else {
            return Ok(None);
        };

        let request_tick = request.tick();
        let mut events = vec![TrafficControllerEvent::Request(request)];
        if self.should_force_transition_after_request(current, request_tick)? {
            events.extend(self.transition_at(request_tick)?.into_events());
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

    fn should_force_transition_after_request(
        &self,
        state: TrafficStateId,
        tick: u64,
    ) -> Result<bool, TrafficGeneratorError> {
        let request_tick = self
            .generators
            .get(&state)
            .expect("validated traffic controller has current generator")
            .peek_schedule_tick(tick, 0)?;
        Ok(request_tick == u64::MAX && self.machine.next_transition_tick() == u64::MAX)
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
