use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

mod expression;

pub use expression::{
    PowerExpression, PowerExpressionInputs, PowerExpressionModel, PowerExpressionModelSnapshot,
    PowerMetricId, PowerStateExpression,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PowerComponentId(u64);

impl PowerComponentId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PowerStateKind {
    Undefined,
    On,
    ClockGated,
    SramRetention,
    Off,
}

impl PowerStateKind {
    const fn performance_rank(self) -> Option<u8> {
        match self {
            Self::Undefined => None,
            Self::On => Some(0),
            Self::ClockGated => Some(1),
            Self::SramRetention => Some(2),
            Self::Off => Some(3),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PowerDomainConfig {
    name: String,
    possible_states: Vec<PowerStateKind>,
    default_state: PowerStateKind,
}

impl PowerDomainConfig {
    pub fn new(
        name: impl Into<String>,
        possible_states: Vec<PowerStateKind>,
        default_state: PowerStateKind,
    ) -> Result<Self, PowerError> {
        let name = name.into();
        validate_power_state_set(&name, &possible_states, default_state)?;
        Ok(Self {
            name,
            possible_states: canonical_states(possible_states),
            default_state,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn possible_states(&self) -> &[PowerStateKind] {
        &self.possible_states
    }

    pub const fn default_state(&self) -> PowerStateKind {
        self.default_state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerComponentRole {
    Leader,
    Follower,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PowerComponentSnapshot {
    id: PowerComponentId,
    name: String,
    role: PowerComponentRole,
    possible_states: Vec<PowerStateKind>,
    state: PowerStateKind,
}

impl PowerComponentSnapshot {
    pub const fn new(
        id: PowerComponentId,
        name: String,
        role: PowerComponentRole,
        possible_states: Vec<PowerStateKind>,
        state: PowerStateKind,
    ) -> Self {
        Self {
            id,
            name,
            role,
            possible_states,
            state,
        }
    }

    pub const fn id(&self) -> PowerComponentId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn role(&self) -> PowerComponentRole {
        self.role
    }

    pub fn possible_states(&self) -> &[PowerStateKind] {
        &self.possible_states
    }

    pub const fn state(&self) -> PowerStateKind {
        self.state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PowerResidency {
    ticks: BTreeMap<PowerStateKind, Tick>,
}

impl PowerResidency {
    pub fn new(ticks: Vec<(PowerStateKind, Tick)>) -> Self {
        Self {
            ticks: ticks.into_iter().collect(),
        }
    }

    pub fn ticks(&self, state: PowerStateKind) -> Tick {
        self.ticks.get(&state).copied().unwrap_or_default()
    }

    pub fn entries(&self) -> &BTreeMap<PowerStateKind, Tick> {
        &self.ticks
    }

    pub fn total_ticks(&self) -> Tick {
        self.ticks.values().copied().sum()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerModelMode {
    All,
    StaticOnly,
    DynamicOnly,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowerStatePower {
    state: PowerStateKind,
    dynamic_watts: f64,
    static_watts: f64,
    static_temperature_coefficient: f64,
}

impl PowerStatePower {
    pub fn new(
        state: PowerStateKind,
        dynamic_watts: f64,
        static_watts: f64,
    ) -> Result<Self, PowerError> {
        if state == PowerStateKind::Undefined {
            return Err(PowerError::UndefinedState);
        }
        validate_power_value(dynamic_watts)?;
        validate_power_value(static_watts)?;
        Ok(Self {
            state,
            dynamic_watts,
            static_watts,
            static_temperature_coefficient: 0.0,
        })
    }

    pub fn with_static_temperature_coefficient(
        mut self,
        coefficient: f64,
    ) -> Result<Self, PowerError> {
        validate_power_value(coefficient)?;
        self.static_temperature_coefficient = coefficient;
        Ok(self)
    }

    pub const fn state(&self) -> PowerStateKind {
        self.state
    }

    pub const fn dynamic_watts(&self) -> f64 {
        self.dynamic_watts
    }

    pub const fn static_watts(&self) -> f64 {
        self.static_watts
    }

    pub const fn static_temperature_coefficient(&self) -> f64 {
        self.static_temperature_coefficient
    }

    fn static_watts_at(&self, ambient_temperature_c: f64, temperature_c: f64) -> f64 {
        self.static_watts
            + self.static_temperature_coefficient * (temperature_c - ambient_temperature_c)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowerEstimate {
    dynamic_watts: f64,
    static_watts: f64,
}

impl PowerEstimate {
    pub const fn new(dynamic_watts: f64, static_watts: f64) -> Self {
        Self {
            dynamic_watts,
            static_watts,
        }
    }

    pub const fn dynamic_watts(&self) -> f64 {
        self.dynamic_watts
    }

    pub const fn static_watts(&self) -> f64 {
        self.static_watts
    }

    pub const fn total_watts(&self) -> f64 {
        self.dynamic_watts + self.static_watts
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerModelSnapshot {
    mode: PowerModelMode,
    ambient_temperature_c: f64,
    current_temperature_c: f64,
    states: Vec<PowerStatePower>,
}

impl PowerModelSnapshot {
    pub fn new(
        mode: PowerModelMode,
        ambient_temperature_c: f64,
        states: Vec<PowerStatePower>,
    ) -> Self {
        Self {
            mode,
            ambient_temperature_c,
            current_temperature_c: ambient_temperature_c,
            states,
        }
    }

    pub const fn mode(&self) -> PowerModelMode {
        self.mode
    }

    pub const fn ambient_temperature_c(&self) -> f64 {
        self.ambient_temperature_c
    }

    pub const fn current_temperature_c(&self) -> f64 {
        self.current_temperature_c
    }

    pub fn states(&self) -> &[PowerStatePower] {
        &self.states
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PowerModel {
    mode: PowerModelMode,
    ambient_temperature_c: f64,
    current_temperature_c: f64,
    states: BTreeMap<PowerStateKind, PowerStatePower>,
}

impl PowerModel {
    pub fn new(
        mode: PowerModelMode,
        ambient_temperature_c: f64,
        states: Vec<PowerStatePower>,
    ) -> Result<Self, PowerError> {
        validate_temperature(ambient_temperature_c)?;
        let states = state_power_map(states)?;
        Ok(Self {
            mode,
            ambient_temperature_c,
            current_temperature_c: ambient_temperature_c,
            states,
        })
    }

    pub const fn mode(&self) -> PowerModelMode {
        self.mode
    }

    pub const fn ambient_temperature_c(&self) -> f64 {
        self.ambient_temperature_c
    }

    pub const fn current_temperature_c(&self) -> f64 {
        self.current_temperature_c
    }

    pub fn update_temperature(&mut self, temperature_c: f64) -> Result<(), PowerError> {
        validate_temperature(temperature_c)?;
        self.current_temperature_c = temperature_c;
        Ok(())
    }

    pub fn estimate(&self, residency: &PowerResidency) -> Result<PowerEstimate, PowerError> {
        let total_ticks = residency.total_ticks();
        if total_ticks == 0 {
            return Err(PowerError::NoPowerResidency);
        }

        let mut dynamic_watts = 0.0;
        let mut static_watts = 0.0;
        for (state, ticks) in residency.entries() {
            if *ticks == 0 {
                continue;
            }
            let Some(state_power) = self.states.get(state) else {
                return Err(PowerError::MissingPowerStateModel { state: *state });
            };
            let weight = *ticks as f64 / total_ticks as f64;
            if self.mode != PowerModelMode::StaticOnly {
                dynamic_watts += state_power.dynamic_watts() * weight;
            }
            if self.mode != PowerModelMode::DynamicOnly {
                static_watts += state_power
                    .static_watts_at(self.ambient_temperature_c, self.current_temperature_c)
                    * weight;
            }
        }

        Ok(PowerEstimate::new(dynamic_watts, static_watts))
    }

    pub fn snapshot(&self) -> PowerModelSnapshot {
        PowerModelSnapshot {
            mode: self.mode,
            ambient_temperature_c: self.ambient_temperature_c,
            current_temperature_c: self.current_temperature_c,
            states: self.states.values().copied().collect(),
        }
    }

    pub fn restore(&mut self, snapshot: &PowerModelSnapshot) -> Result<(), PowerError> {
        if snapshot.mode() != self.mode {
            return Err(PowerError::PowerModelModeMismatch {
                expected: self.mode,
                actual: snapshot.mode(),
            });
        }
        validate_temperature(snapshot.ambient_temperature_c())?;
        validate_temperature(snapshot.current_temperature_c())?;
        self.ambient_temperature_c = snapshot.ambient_temperature_c();
        self.current_temperature_c = snapshot.current_temperature_c();
        self.states = state_power_map(snapshot.states().to_vec())?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PowerDomainSnapshot {
    config: PowerDomainConfig,
    components: Vec<PowerComponentSnapshot>,
    current_state: PowerStateKind,
    leader_target_state: PowerStateKind,
    last_tick: Tick,
    domain_transitions: u64,
    follower_match_transitions: u64,
    leader_calls: u64,
    leader_calls_changing_state: u64,
    residency_ticks: Vec<(PowerStateKind, Tick)>,
}

impl PowerDomainSnapshot {
    pub const fn new(
        config: PowerDomainConfig,
        components: Vec<PowerComponentSnapshot>,
        current_state: PowerStateKind,
        leader_target_state: PowerStateKind,
        last_tick: Tick,
        domain_transitions: u64,
        follower_match_transitions: u64,
        leader_calls: u64,
        leader_calls_changing_state: u64,
        residency_ticks: Vec<(PowerStateKind, Tick)>,
    ) -> Self {
        Self {
            config,
            components,
            current_state,
            leader_target_state,
            last_tick,
            domain_transitions,
            follower_match_transitions,
            leader_calls,
            leader_calls_changing_state,
            residency_ticks,
        }
    }

    pub const fn config(&self) -> &PowerDomainConfig {
        &self.config
    }

    pub fn components(&self) -> &[PowerComponentSnapshot] {
        &self.components
    }

    pub const fn current_state(&self) -> PowerStateKind {
        self.current_state
    }

    pub const fn leader_target_state(&self) -> PowerStateKind {
        self.leader_target_state
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub const fn domain_transitions(&self) -> u64 {
        self.domain_transitions
    }

    pub const fn follower_match_transitions(&self) -> u64 {
        self.follower_match_transitions
    }

    pub const fn leader_calls(&self) -> u64 {
        self.leader_calls
    }

    pub const fn leader_calls_changing_state(&self) -> u64 {
        self.leader_calls_changing_state
    }

    pub fn residency_ticks(&self) -> &[(PowerStateKind, Tick)] {
        &self.residency_ticks
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PowerDomain {
    config: PowerDomainConfig,
    next_component: u64,
    components: BTreeMap<PowerComponentId, PowerComponent>,
    component_names: BTreeSet<String>,
    current_state: PowerStateKind,
    leader_target_state: PowerStateKind,
    last_tick: Tick,
    residency_ticks: BTreeMap<PowerStateKind, Tick>,
    domain_transitions: u64,
    follower_match_transitions: u64,
    leader_calls: u64,
    leader_calls_changing_state: u64,
}

impl PowerDomain {
    pub fn new(config: PowerDomainConfig) -> Self {
        Self {
            current_state: config.default_state(),
            leader_target_state: config.default_state(),
            config,
            next_component: 0,
            components: BTreeMap::new(),
            component_names: BTreeSet::new(),
            last_tick: 0,
            residency_ticks: BTreeMap::new(),
            domain_transitions: 0,
            follower_match_transitions: 0,
            leader_calls: 0,
            leader_calls_changing_state: 0,
        }
    }

    pub const fn config(&self) -> &PowerDomainConfig {
        &self.config
    }

    pub const fn current_state(&self) -> PowerStateKind {
        self.current_state
    }

    pub const fn domain_transitions(&self) -> u64 {
        self.domain_transitions
    }

    pub const fn follower_match_transitions(&self) -> u64 {
        self.follower_match_transitions
    }

    pub const fn leader_calls(&self) -> u64 {
        self.leader_calls
    }

    pub const fn leader_calls_changing_state(&self) -> u64 {
        self.leader_calls_changing_state
    }

    pub fn add_leader(
        &mut self,
        name: impl Into<String>,
        possible_states: Vec<PowerStateKind>,
        default_state: PowerStateKind,
    ) -> Result<PowerComponentId, PowerError> {
        self.add_component(
            name,
            possible_states,
            default_state,
            PowerComponentRole::Leader,
        )
    }

    pub fn add_follower(
        &mut self,
        name: impl Into<String>,
        possible_states: Vec<PowerStateKind>,
        default_state: PowerStateKind,
    ) -> Result<PowerComponentId, PowerError> {
        self.add_component(
            name,
            possible_states,
            default_state,
            PowerComponentRole::Follower,
        )
    }

    pub fn component_state(
        &self,
        component: PowerComponentId,
    ) -> Result<PowerStateKind, PowerError> {
        Ok(self.component(component)?.state)
    }

    pub fn transition_leader(
        &mut self,
        tick: Tick,
        leader: PowerComponentId,
        state: PowerStateKind,
    ) -> Result<(), PowerError> {
        self.reject_time_backwards(tick)?;
        {
            let component = self.component(leader)?;
            if component.role != PowerComponentRole::Leader {
                return Err(PowerError::ComponentIsNotLeader { component: leader });
            }
            component.validate_state(state)?;
        }

        self.account_current_state_until(tick);
        self.components
            .get_mut(&leader)
            .expect("leader component was already validated")
            .state = state;
        self.leader_calls = self.leader_calls.saturating_add(1);

        let next_target = self.calculate_leader_target_state()?;
        if next_target != self.leader_target_state {
            self.leader_calls_changing_state = self.leader_calls_changing_state.saturating_add(1);
        }
        self.leader_target_state = next_target;

        let follower_ids = self
            .components
            .iter()
            .filter_map(|(id, component)| {
                (component.role == PowerComponentRole::Follower).then_some(*id)
            })
            .collect::<Vec<_>>();
        for follower_id in follower_ids {
            let matched = {
                let follower = self
                    .components
                    .get(&follower_id)
                    .expect("follower id came from components");
                follower.match_state(self.leader_target_state)?
            };
            let follower = self
                .components
                .get_mut(&follower_id)
                .expect("follower id came from components");
            if follower.state != matched {
                follower.state = matched;
                self.follower_match_transitions = self.follower_match_transitions.saturating_add(1);
            }
        }

        let next_domain_state = self.calculate_domain_state()?;
        if next_domain_state != self.current_state {
            self.current_state = next_domain_state;
            self.domain_transitions = self.domain_transitions.saturating_add(1);
        }
        Ok(())
    }

    pub fn residency_at(&self, tick: Tick) -> Result<PowerResidency, PowerError> {
        if tick < self.last_tick {
            return Err(PowerError::TimeWentBack {
                tick,
                last_tick: self.last_tick,
            });
        }
        let mut residency = self.residency_ticks.clone();
        *residency.entry(self.current_state).or_default() += tick - self.last_tick;
        Ok(PowerResidency { ticks: residency })
    }

    pub fn snapshot(&self) -> PowerDomainSnapshot {
        PowerDomainSnapshot::new(
            self.config.clone(),
            self.components
                .iter()
                .map(|(id, component)| component.snapshot(*id))
                .collect(),
            self.current_state,
            self.leader_target_state,
            self.last_tick,
            self.domain_transitions,
            self.follower_match_transitions,
            self.leader_calls,
            self.leader_calls_changing_state,
            self.residency_ticks
                .iter()
                .map(|(state, ticks)| (*state, *ticks))
                .collect(),
        )
    }

    pub fn restore(&mut self, snapshot: &PowerDomainSnapshot) -> Result<(), PowerError> {
        if snapshot.config() != &self.config {
            return Err(PowerError::SnapshotConfigMismatch {
                expected: self.config.clone(),
                actual: snapshot.config().clone(),
            });
        }

        let mut components = BTreeMap::new();
        let mut component_names = BTreeSet::new();
        let mut next_component = 0;
        for component in snapshot.components() {
            validate_power_state_set(
                component.name(),
                component.possible_states(),
                component.state(),
            )?;
            if !component_names.insert(component.name().to_string()) {
                return Err(PowerError::DuplicateComponentName {
                    name: component.name().to_string(),
                });
            }
            next_component = next_component.max(component.id().get().saturating_add(1));
            components.insert(
                component.id(),
                PowerComponent {
                    name: component.name().to_string(),
                    role: component.role(),
                    possible_states: component.possible_states().iter().copied().collect(),
                    state: component.state(),
                },
            );
        }

        self.components = components;
        self.component_names = component_names;
        self.next_component = next_component;
        self.current_state = snapshot.current_state();
        self.leader_target_state = snapshot.leader_target_state();
        self.last_tick = snapshot.last_tick();
        self.domain_transitions = snapshot.domain_transitions();
        self.follower_match_transitions = snapshot.follower_match_transitions();
        self.leader_calls = snapshot.leader_calls();
        self.leader_calls_changing_state = snapshot.leader_calls_changing_state();
        self.residency_ticks = snapshot.residency_ticks().iter().copied().collect();
        Ok(())
    }

    fn add_component(
        &mut self,
        name: impl Into<String>,
        possible_states: Vec<PowerStateKind>,
        default_state: PowerStateKind,
        role: PowerComponentRole,
    ) -> Result<PowerComponentId, PowerError> {
        let name = name.into();
        validate_power_state_set(&name, &possible_states, default_state)?;
        if !self.component_names.insert(name.clone()) {
            return Err(PowerError::DuplicateComponentName { name });
        }

        let id = PowerComponentId::new(self.next_component);
        self.next_component = self.next_component.saturating_add(1);
        self.components.insert(
            id,
            PowerComponent {
                name,
                role,
                possible_states: possible_states.into_iter().collect(),
                state: default_state,
            },
        );
        Ok(id)
    }

    fn component(&self, component: PowerComponentId) -> Result<&PowerComponent, PowerError> {
        self.components
            .get(&component)
            .ok_or(PowerError::UnknownComponent { component })
    }

    fn reject_time_backwards(&self, tick: Tick) -> Result<(), PowerError> {
        if tick < self.last_tick {
            return Err(PowerError::TimeWentBack {
                tick,
                last_tick: self.last_tick,
            });
        }
        Ok(())
    }

    fn account_current_state_until(&mut self, tick: Tick) {
        *self.residency_ticks.entry(self.current_state).or_default() += tick - self.last_tick;
        self.last_tick = tick;
    }

    fn calculate_leader_target_state(&self) -> Result<PowerStateKind, PowerError> {
        self.components
            .values()
            .filter(|component| component.role == PowerComponentRole::Leader)
            .map(|component| component.state)
            .min_by_key(|state| state.performance_rank().unwrap_or(u8::MAX))
            .ok_or(PowerError::NoLeaders)
    }

    fn calculate_domain_state(&self) -> Result<PowerStateKind, PowerError> {
        self.components
            .values()
            .map(|component| component.state)
            .min_by_key(|state| state.performance_rank().unwrap_or(u8::MAX))
            .ok_or(PowerError::NoComponents)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PowerComponent {
    name: String,
    role: PowerComponentRole,
    possible_states: BTreeSet<PowerStateKind>,
    state: PowerStateKind,
}

impl PowerComponent {
    fn validate_state(&self, state: PowerStateKind) -> Result<(), PowerError> {
        if !self.possible_states.contains(&state) {
            return Err(PowerError::StateNotAllowed {
                component: self.name.clone(),
                state,
            });
        }
        Ok(())
    }

    fn match_state(&self, requested: PowerStateKind) -> Result<PowerStateKind, PowerError> {
        let requested_rank = requested
            .performance_rank()
            .ok_or(PowerError::UndefinedState)?;
        self.possible_states
            .iter()
            .copied()
            .filter(|state| {
                state
                    .performance_rank()
                    .is_some_and(|rank| rank <= requested_rank)
            })
            .max_by_key(|state| state.performance_rank().unwrap_or_default())
            .ok_or(PowerError::StateNotAllowed {
                component: self.name.clone(),
                state: requested,
            })
    }

    fn snapshot(&self, id: PowerComponentId) -> PowerComponentSnapshot {
        PowerComponentSnapshot::new(
            id,
            self.name.clone(),
            self.role,
            self.possible_states.iter().copied().collect(),
            self.state,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PowerError {
    EmptyName,
    EmptyStateSet {
        component: String,
    },
    UndefinedState,
    MissingOnState {
        component: String,
    },
    DuplicateComponentName {
        name: String,
    },
    UnknownComponent {
        component: PowerComponentId,
    },
    ComponentIsNotLeader {
        component: PowerComponentId,
    },
    StateNotAllowed {
        component: String,
        state: PowerStateKind,
    },
    NoLeaders,
    NoComponents,
    TimeWentBack {
        tick: Tick,
        last_tick: Tick,
    },
    SnapshotConfigMismatch {
        expected: PowerDomainConfig,
        actual: PowerDomainConfig,
    },
    InvalidPowerValue,
    InvalidTemperature,
    DuplicatePowerStateModel {
        state: PowerStateKind,
    },
    MissingPowerStateModel {
        state: PowerStateKind,
    },
    NoPowerResidency,
    PowerModelModeMismatch {
        expected: PowerModelMode,
        actual: PowerModelMode,
    },
    InvalidPowerExpressionInput,
    InvalidClockPeriod,
    MissingPowerMetric {
        metric: PowerMetricId,
    },
    InvalidPowerExpressionResult,
    DuplicatePowerStateExpressionModel {
        state: PowerStateKind,
    },
    MissingPowerStateExpressionModel {
        state: PowerStateKind,
    },
}

impl fmt::Display for PowerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyName => write!(formatter, "power component name must not be empty"),
            Self::EmptyStateSet { component } => {
                write!(formatter, "power component {component} has no states")
            }
            Self::UndefinedState => write!(formatter, "undefined power state is not valid"),
            Self::MissingOnState { component } => {
                write!(
                    formatter,
                    "power component {component} must support the on state"
                )
            }
            Self::DuplicateComponentName { name } => {
                write!(formatter, "power component name already exists: {name}")
            }
            Self::UnknownComponent { component } => {
                write!(formatter, "unknown power component id {}", component.get())
            }
            Self::ComponentIsNotLeader { component } => {
                write!(
                    formatter,
                    "power component {} is not a leader",
                    component.get()
                )
            }
            Self::StateNotAllowed { component, state } => {
                write!(
                    formatter,
                    "power component {component} cannot enter {state:?}"
                )
            }
            Self::NoLeaders => write!(formatter, "power domain has no leaders"),
            Self::NoComponents => write!(formatter, "power domain has no components"),
            Self::TimeWentBack { tick, last_tick } => write!(
                formatter,
                "power transition tick {tick} is before last tick {last_tick}"
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "power snapshot config {actual:?} does not match {expected:?}"
            ),
            Self::InvalidPowerValue => {
                write!(formatter, "power value must be finite and nonnegative")
            }
            Self::InvalidTemperature => write!(formatter, "temperature must be finite"),
            Self::DuplicatePowerStateModel { state } => {
                write!(formatter, "duplicate power model for state {state:?}")
            }
            Self::MissingPowerStateModel { state } => {
                write!(formatter, "missing power model for state {state:?}")
            }
            Self::NoPowerResidency => write!(formatter, "power residency has no elapsed ticks"),
            Self::PowerModelModeMismatch { expected, actual } => write!(
                formatter,
                "power model snapshot mode {actual:?} does not match {expected:?}"
            ),
            Self::InvalidPowerExpressionInput => {
                write!(formatter, "power expression input is not valid")
            }
            Self::InvalidClockPeriod => {
                write!(
                    formatter,
                    "power expression clock period must be finite and positive"
                )
            }
            Self::MissingPowerMetric { metric } => {
                write!(formatter, "missing power metric id {}", metric.get())
            }
            Self::InvalidPowerExpressionResult => {
                write!(
                    formatter,
                    "power expression result is not a valid power value"
                )
            }
            Self::DuplicatePowerStateExpressionModel { state } => {
                write!(
                    formatter,
                    "duplicate power expression model for state {state:?}"
                )
            }
            Self::MissingPowerStateExpressionModel { state } => {
                write!(
                    formatter,
                    "missing power expression model for state {state:?}"
                )
            }
        }
    }
}

impl Error for PowerError {}

fn validate_power_state_set(
    component: &str,
    possible_states: &[PowerStateKind],
    default_state: PowerStateKind,
) -> Result<(), PowerError> {
    if component.is_empty() {
        return Err(PowerError::EmptyName);
    }
    if possible_states.is_empty() {
        return Err(PowerError::EmptyStateSet {
            component: component.to_string(),
        });
    }
    if possible_states.contains(&PowerStateKind::Undefined)
        || default_state == PowerStateKind::Undefined
    {
        return Err(PowerError::UndefinedState);
    }
    if !possible_states.contains(&PowerStateKind::On) {
        return Err(PowerError::MissingOnState {
            component: component.to_string(),
        });
    }
    if !possible_states.contains(&default_state) {
        return Err(PowerError::StateNotAllowed {
            component: component.to_string(),
            state: default_state,
        });
    }
    Ok(())
}

fn canonical_states(states: Vec<PowerStateKind>) -> Vec<PowerStateKind> {
    states
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn state_power_map(
    states: Vec<PowerStatePower>,
) -> Result<BTreeMap<PowerStateKind, PowerStatePower>, PowerError> {
    let mut map = BTreeMap::new();
    for state in states {
        if map.insert(state.state(), state).is_some() {
            return Err(PowerError::DuplicatePowerStateModel {
                state: state.state(),
            });
        }
    }
    Ok(map)
}

fn validate_power_value(value: f64) -> Result<(), PowerError> {
    if !value.is_finite() || value < 0.0 {
        return Err(PowerError::InvalidPowerValue);
    }
    Ok(())
}

fn validate_temperature(value: f64) -> Result<(), PowerError> {
    if !value.is_finite() {
        return Err(PowerError::InvalidTemperature);
    }
    Ok(())
}
