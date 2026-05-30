use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MoesiProtocolStateId(String);

impl MoesiProtocolStateId {
    pub fn new(value: impl Into<String>) -> Result<Self, MoesiTransitionResourceError> {
        let value = value.into();
        if value.is_empty() {
            return Err(MoesiTransitionResourceError::EmptyStateId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MoesiProtocolEventId(String);

impl MoesiProtocolEventId {
    pub fn new(value: impl Into<String>) -> Result<Self, MoesiTransitionResourceError> {
        let value = value.into();
        if value.is_empty() {
            return Err(MoesiTransitionResourceError::EmptyEventId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MoesiProtocolResourceId(String);

impl MoesiProtocolResourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, MoesiTransitionResourceError> {
        let value = value.into();
        if value.is_empty() {
            return Err(MoesiTransitionResourceError::EmptyResourceId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoesiResourceEffect {
    Allocate(MoesiProtocolResourceId),
    Release(MoesiProtocolResourceId),
}

impl MoesiResourceEffect {
    fn resource(&self) -> &MoesiProtocolResourceId {
        match self {
            Self::Allocate(resource) | Self::Release(resource) => resource,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiTransitionResourceRule {
    state: MoesiProtocolStateId,
    event: MoesiProtocolEventId,
    target: MoesiProtocolStateId,
    effects: Vec<MoesiResourceEffect>,
}

impl MoesiTransitionResourceRule {
    pub fn new(
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
    ) -> Self {
        Self {
            state,
            event,
            target,
            effects: Vec::new(),
        }
    }

    pub fn with_effect(mut self, effect: MoesiResourceEffect) -> Self {
        self.effects.push(effect);
        self
    }

    pub fn state(&self) -> &MoesiProtocolStateId {
        &self.state
    }

    pub fn event(&self) -> &MoesiProtocolEventId {
        &self.event
    }

    pub fn target(&self) -> &MoesiProtocolStateId {
        &self.target
    }

    pub fn effects(&self) -> &[MoesiResourceEffect] {
        &self.effects
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiTransitionResourceContract {
    state_resources: Vec<(MoesiProtocolStateId, MoesiProtocolResourceId)>,
    rules: Vec<MoesiTransitionResourceRule>,
}

impl MoesiTransitionResourceContract {
    pub fn new() -> Self {
        Self {
            state_resources: Vec::new(),
            rules: Vec::new(),
        }
    }

    pub fn with_state_resource(
        mut self,
        state: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    ) -> Self {
        self.state_resources.push((state, resource));
        self
    }

    pub fn with_rule(mut self, rule: MoesiTransitionResourceRule) -> Self {
        self.rules.push(rule);
        self
    }

    pub fn state_resources(&self) -> &[(MoesiProtocolStateId, MoesiProtocolResourceId)] {
        &self.state_resources
    }

    pub fn rules(&self) -> &[MoesiTransitionResourceRule] {
        &self.rules
    }

    pub fn validate(&self) -> Result<MoesiTransitionResourceReport, MoesiTransitionResourceError> {
        let state_resources = self.validate_state_resources()?;
        self.validate_unique_transition_keys()?;

        for rule in &self.rules {
            let effects = RuleEffects::from_rule(rule)?;
            self.validate_rule(rule, &state_resources, &effects)?;
        }

        Ok(MoesiTransitionResourceReport {
            transition_count: self.rules.len(),
            state_resource_count: self.state_resources.len(),
            resource_count: self.resource_ids().len(),
        })
    }

    fn validate_state_resources(
        &self,
    ) -> Result<
        BTreeMap<MoesiProtocolStateId, BTreeSet<MoesiProtocolResourceId>>,
        MoesiTransitionResourceError,
    > {
        let mut state_resources =
            BTreeMap::<MoesiProtocolStateId, BTreeSet<MoesiProtocolResourceId>>::new();
        for (state, resource) in &self.state_resources {
            let resources = state_resources.entry(state.clone()).or_default();
            if !resources.insert(resource.clone()) {
                return Err(MoesiTransitionResourceError::DuplicateStateResource {
                    state: state.clone(),
                    resource: resource.clone(),
                });
            }
        }
        Ok(state_resources)
    }

    fn validate_unique_transition_keys(&self) -> Result<(), MoesiTransitionResourceError> {
        let mut keys = BTreeSet::<(MoesiProtocolStateId, MoesiProtocolEventId)>::new();
        for rule in &self.rules {
            let key = (rule.state.clone(), rule.event.clone());
            if !keys.insert(key) {
                return Err(MoesiTransitionResourceError::DuplicateTransition {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                });
            }
        }
        Ok(())
    }

    fn validate_rule(
        &self,
        rule: &MoesiTransitionResourceRule,
        state_resources: &BTreeMap<MoesiProtocolStateId, BTreeSet<MoesiProtocolResourceId>>,
        effects: &RuleEffects,
    ) -> Result<(), MoesiTransitionResourceError> {
        let source_resources = state_resources.get(&rule.state);
        let target_resources = state_resources.get(&rule.target);
        for resource in resources_referenced_by(rule, source_resources, target_resources) {
            let source_owns =
                source_resources.is_some_and(|resources| resources.contains(resource));
            let target_owns =
                target_resources.is_some_and(|resources| resources.contains(resource));
            let allocates = effects.allocations.contains(resource);
            let releases = effects.releases.contains(resource);

            if allocates && releases {
                return Err(MoesiTransitionResourceError::ConflictingResourceEffect {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                    target: rule.target.clone(),
                    resource: resource.clone(),
                });
            }
            if !source_owns && releases {
                return Err(MoesiTransitionResourceError::ReleaseWithoutResource {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                    target: rule.target.clone(),
                    resource: resource.clone(),
                });
            }
            if source_owns && allocates {
                return Err(MoesiTransitionResourceError::DuplicateResourceAllocation {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                    target: rule.target.clone(),
                    resource: resource.clone(),
                });
            }
            if !source_owns && target_owns && !allocates {
                return Err(MoesiTransitionResourceError::MissingResourceAllocation {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                    target: rule.target.clone(),
                    resource: resource.clone(),
                });
            }
            if source_owns && !target_owns && !releases {
                return Err(MoesiTransitionResourceError::MissingResourceRelease {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                    target: rule.target.clone(),
                    resource: resource.clone(),
                });
            }
            if !target_owns && allocates {
                return Err(MoesiTransitionResourceError::ResourceAllocationLeaked {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                    target: rule.target.clone(),
                    resource: resource.clone(),
                });
            }
            if target_owns && releases {
                return Err(MoesiTransitionResourceError::ResourceReleaseBreaksTarget {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                    target: rule.target.clone(),
                    resource: resource.clone(),
                });
            }
        }

        Ok(())
    }

    fn resource_ids(&self) -> BTreeSet<MoesiProtocolResourceId> {
        let mut resources = BTreeSet::new();
        for (_, resource) in &self.state_resources {
            resources.insert(resource.clone());
        }
        for rule in &self.rules {
            for effect in &rule.effects {
                resources.insert(effect.resource().clone());
            }
        }
        resources
    }
}

impl Default for MoesiTransitionResourceContract {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoesiTransitionResourceReport {
    transition_count: usize,
    state_resource_count: usize,
    resource_count: usize,
}

impl MoesiTransitionResourceReport {
    pub const fn transition_count(self) -> usize {
        self.transition_count
    }

    pub const fn state_resource_count(self) -> usize {
        self.state_resource_count
    }

    pub const fn resource_count(self) -> usize {
        self.resource_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoesiTransitionResourceError {
    EmptyStateId,
    EmptyEventId,
    EmptyResourceId,
    DuplicateStateResource {
        state: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
    DuplicateTransition {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
    },
    DuplicateResourceEffect {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
    ConflictingResourceEffect {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
    MissingResourceAllocation {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
    MissingResourceRelease {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
    ReleaseWithoutResource {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
    DuplicateResourceAllocation {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
    ResourceAllocationLeaked {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
    ResourceReleaseBreaksTarget {
        state: MoesiProtocolStateId,
        event: MoesiProtocolEventId,
        target: MoesiProtocolStateId,
        resource: MoesiProtocolResourceId,
    },
}

impl fmt::Display for MoesiTransitionResourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStateId => write!(formatter, "MOESI protocol state id must not be empty"),
            Self::EmptyEventId => write!(formatter, "MOESI protocol event id must not be empty"),
            Self::EmptyResourceId => {
                write!(formatter, "MOESI protocol resource id must not be empty")
            }
            Self::DuplicateStateResource { state, resource } => write!(
                formatter,
                "MOESI state {} declares resource {} more than once",
                state.as_str(),
                resource.as_str()
            ),
            Self::DuplicateTransition { state, event } => write!(
                formatter,
                "MOESI transition {} on {} is declared more than once",
                state.as_str(),
                event.as_str()
            ),
            Self::DuplicateResourceEffect {
                state,
                event,
                target,
                resource,
            } => write!(
                formatter,
                "MOESI transition {} on {} to {} mentions resource {} more than once for one effect kind",
                state.as_str(),
                event.as_str(),
                target.as_str(),
                resource.as_str()
            ),
            Self::ConflictingResourceEffect {
                state,
                event,
                target,
                resource,
            } => write!(
                formatter,
                "MOESI transition {} on {} to {} both allocates and releases resource {}",
                state.as_str(),
                event.as_str(),
                target.as_str(),
                resource.as_str()
            ),
            Self::MissingResourceAllocation {
                state,
                event,
                target,
                resource,
            } => write!(
                formatter,
                "MOESI transition {} on {} to {} enters resource {} ownership without allocation",
                state.as_str(),
                event.as_str(),
                target.as_str(),
                resource.as_str()
            ),
            Self::MissingResourceRelease {
                state,
                event,
                target,
                resource,
            } => write!(
                formatter,
                "MOESI transition {} on {} to {} leaves resource {} ownership without release",
                state.as_str(),
                event.as_str(),
                target.as_str(),
                resource.as_str()
            ),
            Self::ReleaseWithoutResource {
                state,
                event,
                target,
                resource,
            } => write!(
                formatter,
                "MOESI transition {} on {} to {} releases resource {} without owning it",
                state.as_str(),
                event.as_str(),
                target.as_str(),
                resource.as_str()
            ),
            Self::DuplicateResourceAllocation {
                state,
                event,
                target,
                resource,
            } => write!(
                formatter,
                "MOESI transition {} on {} to {} allocates already-owned resource {}",
                state.as_str(),
                event.as_str(),
                target.as_str(),
                resource.as_str()
            ),
            Self::ResourceAllocationLeaked {
                state,
                event,
                target,
                resource,
            } => write!(
                formatter,
                "MOESI transition {} on {} to {} allocates resource {} for a target that does not own it",
                state.as_str(),
                event.as_str(),
                target.as_str(),
                resource.as_str()
            ),
            Self::ResourceReleaseBreaksTarget {
                state,
                event,
                target,
                resource,
            } => write!(
                formatter,
                "MOESI transition {} on {} to {} releases resource {} required by the target state",
                state.as_str(),
                event.as_str(),
                target.as_str(),
                resource.as_str()
            ),
        }
    }
}

impl Error for MoesiTransitionResourceError {}

#[derive(Default)]
struct RuleEffects {
    allocations: BTreeSet<MoesiProtocolResourceId>,
    releases: BTreeSet<MoesiProtocolResourceId>,
}

impl RuleEffects {
    fn from_rule(rule: &MoesiTransitionResourceRule) -> Result<Self, MoesiTransitionResourceError> {
        let mut effects = Self::default();
        for effect in &rule.effects {
            let (resources, resource) = match effect {
                MoesiResourceEffect::Allocate(resource) => (&mut effects.allocations, resource),
                MoesiResourceEffect::Release(resource) => (&mut effects.releases, resource),
            };
            if !resources.insert(resource.clone()) {
                return Err(MoesiTransitionResourceError::DuplicateResourceEffect {
                    state: rule.state.clone(),
                    event: rule.event.clone(),
                    target: rule.target.clone(),
                    resource: resource.clone(),
                });
            }
        }
        Ok(effects)
    }
}

fn resources_referenced_by<'a>(
    rule: &'a MoesiTransitionResourceRule,
    source_resources: Option<&'a BTreeSet<MoesiProtocolResourceId>>,
    target_resources: Option<&'a BTreeSet<MoesiProtocolResourceId>>,
) -> BTreeSet<&'a MoesiProtocolResourceId> {
    let mut resources = BTreeSet::new();
    if let Some(source_resources) = source_resources {
        resources.extend(source_resources);
    }
    if let Some(target_resources) = target_resources {
        resources.extend(target_resources);
    }
    resources.extend(rule.effects.iter().map(MoesiResourceEffect::resource));
    resources
}
