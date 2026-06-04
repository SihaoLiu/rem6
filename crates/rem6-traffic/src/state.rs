use std::collections::{BTreeMap, BTreeSet};

use crate::{
    common::{checked_counter_add, TrafficRng},
    TrafficGeneratorError,
};

pub const TRAFFIC_TRANSITION_PROBABILITY_SCALE: u32 = 1_000_000;
const DEFAULT_STATE_RNG: u64 = 0x517c_c1b7_2722_0a95;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TrafficStateId(u32);

impl TrafficStateId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficStateSpec {
    id: TrafficStateId,
    duration: u64,
}

impl TrafficStateSpec {
    pub const fn new(id: TrafficStateId, duration: u64) -> Self {
        Self { id, duration }
    }

    pub const fn id(self) -> TrafficStateId {
        self.id
    }

    pub const fn duration(self) -> u64 {
        self.duration
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTransitionProbability {
    micros: u32,
}

impl TrafficTransitionProbability {
    pub fn from_micros(micros: u32) -> Result<Self, TrafficGeneratorError> {
        if micros > TRAFFIC_TRANSITION_PROBABILITY_SCALE {
            return Err(
                TrafficGeneratorError::TrafficTransitionProbabilityOutOfRange {
                    probability: micros,
                    scale: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
                },
            );
        }

        Ok(Self { micros })
    }

    pub fn from_ratio(numerator: u32, denominator: u32) -> Result<Self, TrafficGeneratorError> {
        if denominator == 0 {
            return Err(TrafficGeneratorError::TrafficTransitionRatioZeroDenominator);
        }

        let scaled = (u128::from(numerator) * u128::from(TRAFFIC_TRANSITION_PROBABILITY_SCALE))
            / u128::from(denominator);
        if scaled > u128::from(TRAFFIC_TRANSITION_PROBABILITY_SCALE) {
            let probability = u32::try_from(scaled).unwrap_or(u32::MAX);
            return Err(
                TrafficGeneratorError::TrafficTransitionProbabilityOutOfRange {
                    probability,
                    scale: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
                },
            );
        }

        Ok(Self {
            micros: scaled as u32,
        })
    }

    pub const fn micros(self) -> u32 {
        self.micros
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTransition {
    from: TrafficStateId,
    to: TrafficStateId,
    probability: TrafficTransitionProbability,
}

impl TrafficTransition {
    pub const fn new(
        from: TrafficStateId,
        to: TrafficStateId,
        probability: TrafficTransitionProbability,
    ) -> Self {
        Self {
            from,
            to,
            probability,
        }
    }

    pub const fn from(self) -> TrafficStateId {
        self.from
    }

    pub const fn to(self) -> TrafficStateId {
        self.to
    }

    pub const fn probability(self) -> TrafficTransitionProbability {
        self.probability
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficStateGraphConfig {
    states: Vec<TrafficStateSpec>,
    transitions: Vec<TrafficTransition>,
    initial_state: TrafficStateId,
    rng_state: u64,
}

impl TrafficStateGraphConfig {
    pub fn new(
        states: Vec<TrafficStateSpec>,
        initial_state: TrafficStateId,
        transitions: Vec<TrafficTransition>,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_graph(&states, initial_state, &transitions)?;

        let mut states = states;
        states.sort_by_key(|state| state.id());
        let mut transitions = transitions;
        transitions.sort_by_key(|transition| (transition.from(), transition.to()));

        Ok(Self {
            states,
            transitions,
            initial_state,
            rng_state: DEFAULT_STATE_RNG,
        })
    }

    pub const fn initial_state(&self) -> TrafficStateId {
        self.initial_state
    }

    pub fn states(&self) -> &[TrafficStateSpec] {
        &self.states
    }

    pub fn transitions(&self) -> &[TrafficTransition] {
        &self.transitions
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng_state
    }

    pub const fn with_rng_state(mut self, rng_state: u64) -> Self {
        self.rng_state = rng_state;
        self
    }

    fn state_duration(&self, state: TrafficStateId) -> Option<u64> {
        self.states
            .iter()
            .find(|spec| spec.id() == state)
            .map(|spec| spec.duration())
    }

    fn has_state(&self, state: TrafficStateId) -> bool {
        self.state_duration(state).is_some()
    }

    fn transitions_from(
        &self,
        state: TrafficStateId,
    ) -> impl Iterator<Item = TrafficTransition> + '_ {
        self.transitions
            .iter()
            .copied()
            .filter(move |transition| transition.from() == state)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficStateSnapshot {
    config: TrafficStateGraphConfig,
    current_state: Option<TrafficStateId>,
    next_transition_tick: u64,
    next_sequence: u64,
    rng_state: u64,
    active: bool,
}

impl TrafficStateSnapshot {
    pub const fn new(
        config: TrafficStateGraphConfig,
        current_state: Option<TrafficStateId>,
        next_transition_tick: u64,
        next_sequence: u64,
        rng_state: u64,
        active: bool,
    ) -> Self {
        Self {
            config,
            current_state,
            next_transition_tick,
            next_sequence,
            rng_state,
            active,
        }
    }

    pub const fn config(&self) -> &TrafficStateGraphConfig {
        &self.config
    }

    pub const fn current_state(&self) -> Option<TrafficStateId> {
        self.current_state
    }

    pub const fn next_transition_tick(&self) -> u64 {
        self.next_transition_tick
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng_state
    }

    pub const fn active(&self) -> bool {
        self.active
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTransitionEvent {
    tick: u64,
    sequence: u64,
    from: TrafficStateId,
    to: TrafficStateId,
    next_transition_tick: u64,
}

impl TrafficTransitionEvent {
    pub const fn new(
        tick: u64,
        sequence: u64,
        from: TrafficStateId,
        to: TrafficStateId,
        next_transition_tick: u64,
    ) -> Self {
        Self {
            tick,
            sequence,
            from,
            to,
            next_transition_tick,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn from(self) -> TrafficStateId {
        self.from
    }

    pub const fn to(self) -> TrafficStateId {
        self.to
    }

    pub const fn next_transition_tick(self) -> u64 {
        self.next_transition_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficStateMachine {
    config: TrafficStateGraphConfig,
    current_state: Option<TrafficStateId>,
    next_transition_tick: u64,
    next_sequence: u64,
    rng: TrafficRng,
    active: bool,
}

impl TrafficStateMachine {
    pub fn new(config: TrafficStateGraphConfig) -> Self {
        let rng = TrafficRng::new(config.rng_state());
        Self {
            config,
            current_state: None,
            next_transition_tick: u64::MAX,
            next_sequence: 0,
            rng,
            active: false,
        }
    }

    pub fn restore(snapshot: TrafficStateSnapshot) -> Result<Self, TrafficGeneratorError> {
        validate_snapshot(&snapshot)?;

        Ok(Self {
            config: snapshot.config().clone(),
            current_state: snapshot.current_state(),
            next_transition_tick: snapshot.next_transition_tick(),
            next_sequence: snapshot.next_sequence(),
            rng: TrafficRng::new(snapshot.rng_state()),
            active: snapshot.active(),
        })
    }

    pub fn start(&mut self, tick: u64) -> Result<(), TrafficGeneratorError> {
        let initial = self.config.initial_state();
        let duration = self
            .config
            .state_duration(initial)
            .expect("validated traffic state graph has initial state");
        self.current_state = Some(initial);
        self.next_transition_tick = transition_tick_after(tick, duration)?;
        self.next_sequence = 0;
        self.rng = TrafficRng::new(self.config.rng_state());
        self.active = true;
        Ok(())
    }

    pub fn transition_if_due(
        &mut self,
        tick: u64,
    ) -> Result<Option<TrafficTransitionEvent>, TrafficGeneratorError> {
        if self.current_state.is_none() {
            return Ok(None);
        };

        if !self.active || self.next_transition_tick == u64::MAX || tick < self.next_transition_tick
        {
            return Ok(None);
        }

        self.transition_now(tick).map(Some)
    }

    pub(crate) fn transition_now(
        &mut self,
        tick: u64,
    ) -> Result<TrafficTransitionEvent, TrafficGeneratorError> {
        let from = self
            .current_state
            .expect("active traffic state machine has current state before transition");
        let sequence = self.next_sequence;
        let next_sequence = checked_counter_add("state.next_sequence", sequence, 1)?;
        let mut next_rng = self.rng.clone();
        let to = select_next_state(&self.config, from, &mut next_rng);
        let duration = self
            .config
            .state_duration(to)
            .expect("validated transition target has state duration");
        let next_transition_tick = transition_tick_after(tick, duration)?;
        let event = TrafficTransitionEvent::new(tick, sequence, from, to, next_transition_tick);

        self.current_state = Some(to);
        self.next_transition_tick = next_transition_tick;
        self.next_sequence = next_sequence;
        self.rng = next_rng;

        Ok(event)
    }

    pub const fn current_state(&self) -> Option<TrafficStateId> {
        self.current_state
    }

    pub const fn next_transition_tick(&self) -> u64 {
        self.next_transition_tick
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng.state()
    }

    pub fn snapshot(&self) -> TrafficStateSnapshot {
        TrafficStateSnapshot::new(
            self.config.clone(),
            self.current_state,
            self.next_transition_tick,
            self.next_sequence,
            self.rng.state(),
            self.active,
        )
    }
}

fn validate_graph(
    states: &[TrafficStateSpec],
    initial_state: TrafficStateId,
    transitions: &[TrafficTransition],
) -> Result<(), TrafficGeneratorError> {
    if states.is_empty() {
        return Err(TrafficGeneratorError::TrafficStateGraphEmpty);
    }

    let mut state_ids = BTreeSet::new();
    for state in states {
        if !state_ids.insert(state.id()) {
            return Err(TrafficGeneratorError::TrafficStateDuplicate { state: state.id() });
        }
    }

    if !state_ids.contains(&initial_state) {
        return Err(TrafficGeneratorError::TrafficStateUnknownInitial {
            state: initial_state,
        });
    }

    let mut transition_pairs = BTreeSet::new();
    let mut row_sums = BTreeMap::new();
    for state in &state_ids {
        row_sums.insert(*state, 0u32);
    }

    for transition in transitions {
        if !state_ids.contains(&transition.from()) {
            return Err(TrafficGeneratorError::TrafficStateUnknownTransition {
                state: transition.from(),
                role: "source",
            });
        }
        if !state_ids.contains(&transition.to()) {
            return Err(TrafficGeneratorError::TrafficStateUnknownTransition {
                state: transition.to(),
                role: "target",
            });
        }
        if !transition_pairs.insert((transition.from(), transition.to())) {
            return Err(TrafficGeneratorError::TrafficStateDuplicateTransition {
                from: transition.from(),
                to: transition.to(),
            });
        }

        let sum = row_sums
            .get_mut(&transition.from())
            .expect("transition source was inserted into row sums");
        *sum = sum
            .checked_add(transition.probability().micros())
            .unwrap_or(u32::MAX);
    }

    for (state, sum) in row_sums {
        if sum != TRAFFIC_TRANSITION_PROBABILITY_SCALE {
            return Err(
                TrafficGeneratorError::TrafficStateTransitionRowSumMismatch {
                    state,
                    sum,
                    expected: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
                },
            );
        }
    }

    Ok(())
}

fn validate_snapshot(snapshot: &TrafficStateSnapshot) -> Result<(), TrafficGeneratorError> {
    if snapshot.active() && snapshot.current_state().is_none() {
        return Err(TrafficGeneratorError::TrafficStateSnapshotMissingCurrentState);
    }
    if let Some(state) = snapshot.current_state() {
        if !snapshot.config().has_state(state) {
            return Err(TrafficGeneratorError::TrafficStateSnapshotUnknownState { state });
        }
    }

    Ok(())
}

fn select_next_state(
    config: &TrafficStateGraphConfig,
    from: TrafficStateId,
    rng: &mut TrafficRng,
) -> TrafficStateId {
    let sample = rng.next_inclusive(0, u64::from(TRAFFIC_TRANSITION_PROBABILITY_SCALE - 1));
    let mut cumulative = 0u64;

    for transition in config.transitions_from(from) {
        cumulative += u64::from(transition.probability().micros());
        if sample < cumulative {
            return transition.to();
        }
    }

    config
        .transitions_from(from)
        .last()
        .expect("validated traffic state graph row has transitions")
        .to()
}

fn transition_tick_after(tick: u64, duration: u64) -> Result<u64, TrafficGeneratorError> {
    if duration == 0 || duration == u64::MAX {
        return Ok(u64::MAX);
    }

    tick.checked_add(duration)
        .ok_or(TrafficGeneratorError::TickOverflow {
            tick,
            delta: duration,
        })
}
