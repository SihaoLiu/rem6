use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MsiLineId(Address);

impl MsiLineId {
    pub const fn new(address: Address) -> Self {
        Self(address)
    }

    pub const fn address(self) -> Address {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MsiState {
    Invalid,
    Shared,
    Modified,
    InvalidToShared,
    InvalidToModified,
    SharedToModified,
}

impl MsiState {
    pub const fn is_stable(self) -> bool {
        matches!(self, Self::Invalid | Self::Shared | Self::Modified)
    }

    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Shared | Self::Modified)
    }

    pub const fn is_modified(self) -> bool {
        matches!(self, Self::Modified)
    }

    pub const fn is_transient(self) -> bool {
        !self.is_stable()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MsiEvent {
    CpuRead,
    CpuWrite,
    DataShared,
    DataModified,
    SnoopRead,
    SnoopWrite,
}

impl MsiEvent {
    pub const fn is_cpu_request(self) -> bool {
        matches!(self, Self::CpuRead | Self::CpuWrite)
    }

    pub const fn is_data_response(self) -> bool {
        matches!(self, Self::DataShared | Self::DataModified)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MsiAction {
    ReadHit { line: MsiLineId },
    WriteHit { line: MsiLineId },
    SendGetShared { line: MsiLineId },
    SendGetModified { line: MsiLineId },
    InstallShared { line: MsiLineId },
    InstallModified { line: MsiLineId },
    WakeRequester { line: MsiLineId },
    SupplyData { line: MsiLineId },
    DowngradeToShared { line: MsiLineId },
    Invalidate { line: MsiLineId },
    Ignore { line: MsiLineId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MsiError {
    Busy {
        agent: AgentId,
        line: MsiLineId,
        state: MsiState,
        event: MsiEvent,
    },
    UnexpectedEvent {
        agent: AgentId,
        line: MsiLineId,
        state: MsiState,
        event: MsiEvent,
    },
    ForcedTransientState {
        agent: AgentId,
        line: MsiLineId,
        state: MsiState,
    },
    MultipleModifiedOwners {
        line: MsiLineId,
        owners: Vec<AgentId>,
    },
    ModifiedWithSharers {
        line: MsiLineId,
        modified: AgentId,
        sharers: Vec<AgentId>,
    },
}

impl fmt::Display for MsiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy {
                agent,
                line,
                state,
                event,
            } => write!(
                formatter,
                "agent {} line {:#x} is busy in {state:?} for {event:?}",
                agent.get(),
                line.address().get()
            ),
            Self::UnexpectedEvent {
                agent,
                line,
                state,
                event,
            } => write!(
                formatter,
                "agent {} line {:#x} cannot handle {event:?} in {state:?}",
                agent.get(),
                line.address().get()
            ),
            Self::ForcedTransientState { agent, line, state } => write!(
                formatter,
                "agent {} line {:#x} cannot be forced into transient state {state:?}",
                agent.get(),
                line.address().get()
            ),
            Self::MultipleModifiedOwners { line, owners } => write!(
                formatter,
                "line {:#x} has {} modified owners",
                line.address().get(),
                owners.len()
            ),
            Self::ModifiedWithSharers {
                line,
                modified,
                sharers,
            } => write!(
                formatter,
                "line {:#x} has modified owner {} with {} sharers",
                line.address().get(),
                modified.get(),
                sharers.len()
            ),
        }
    }
}

impl Error for MsiError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiTransition {
    before: MsiState,
    event: MsiEvent,
    after: MsiState,
    actions: Vec<MsiAction>,
}

impl MsiTransition {
    fn new(before: MsiState, event: MsiEvent, after: MsiState, actions: Vec<MsiAction>) -> Self {
        Self {
            before,
            event,
            after,
            actions,
        }
    }

    pub const fn before(&self) -> MsiState {
        self.before
    }

    pub const fn event(&self) -> MsiEvent {
        self.event
    }

    pub const fn after(&self) -> MsiState {
        self.after
    }

    pub fn actions(&self) -> &[MsiAction] {
        &self.actions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MsiTraceEntry {
    agent: AgentId,
    line: MsiLineId,
    before: MsiState,
    event: MsiEvent,
    after: MsiState,
}

impl MsiTraceEntry {
    pub const fn new(
        agent: AgentId,
        line: MsiLineId,
        before: MsiState,
        event: MsiEvent,
        after: MsiState,
    ) -> Self {
        Self {
            agent,
            line,
            before,
            event,
            after,
        }
    }

    pub const fn agent(self) -> AgentId {
        self.agent
    }

    pub const fn line(self) -> MsiLineId {
        self.line
    }

    pub const fn before(self) -> MsiState {
        self.before
    }

    pub const fn event(self) -> MsiEvent {
        self.event
    }

    pub const fn after(self) -> MsiState {
        self.after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiCacheLine {
    agent: AgentId,
    line: MsiLineId,
    state: MsiState,
    trace: Vec<MsiTraceEntry>,
}

impl MsiCacheLine {
    pub fn new(agent: AgentId, line: MsiLineId) -> Self {
        Self {
            agent,
            line,
            state: MsiState::Invalid,
            trace: Vec::new(),
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn line(&self) -> MsiLineId {
        self.line
    }

    pub const fn state(&self) -> MsiState {
        self.state
    }

    pub fn trace(&self) -> &[MsiTraceEntry] {
        &self.trace
    }

    pub fn force_state(&mut self, state: MsiState) -> Result<(), MsiError> {
        if state.is_transient() {
            return Err(MsiError::ForcedTransientState {
                agent: self.agent,
                line: self.line,
                state,
            });
        }

        self.state = state;
        Ok(())
    }

    pub fn apply(&mut self, event: MsiEvent) -> Result<MsiTransition, MsiError> {
        let before = self.state;
        let transition = self.transition(event)?;
        self.state = transition.after();
        self.trace.push(MsiTraceEntry::new(
            self.agent, self.line, before, event, self.state,
        ));
        Ok(transition)
    }

    pub fn replay(agent: AgentId, line: MsiLineId, events: &[MsiEvent]) -> Result<Self, MsiError> {
        let mut cache = Self::new(agent, line);
        for event in events {
            cache.apply(*event)?;
        }

        Ok(cache)
    }

    fn transition(&self, event: MsiEvent) -> Result<MsiTransition, MsiError> {
        let line = self.line;
        let action = |action| vec![action];
        let actions = |left, right| vec![left, right];

        match (self.state, event) {
            (MsiState::Invalid, MsiEvent::CpuRead) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::InvalidToShared,
                action(MsiAction::SendGetShared { line }),
            )),
            (MsiState::Invalid, MsiEvent::CpuWrite) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::InvalidToModified,
                action(MsiAction::SendGetModified { line }),
            )),
            (MsiState::Invalid, MsiEvent::SnoopRead | MsiEvent::SnoopWrite) => {
                Ok(MsiTransition::new(
                    self.state,
                    event,
                    MsiState::Invalid,
                    action(MsiAction::Ignore { line }),
                ))
            }
            (MsiState::Shared, MsiEvent::CpuRead) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::Shared,
                action(MsiAction::ReadHit { line }),
            )),
            (MsiState::Shared, MsiEvent::CpuWrite) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::SharedToModified,
                action(MsiAction::SendGetModified { line }),
            )),
            (MsiState::Shared, MsiEvent::SnoopRead) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::Shared,
                action(MsiAction::Ignore { line }),
            )),
            (MsiState::Shared, MsiEvent::SnoopWrite) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::Invalid,
                action(MsiAction::Invalidate { line }),
            )),
            (MsiState::Modified, MsiEvent::CpuRead) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::Modified,
                action(MsiAction::ReadHit { line }),
            )),
            (MsiState::Modified, MsiEvent::CpuWrite) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::Modified,
                action(MsiAction::WriteHit { line }),
            )),
            (MsiState::Modified, MsiEvent::SnoopRead) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::Shared,
                actions(
                    MsiAction::SupplyData { line },
                    MsiAction::DowngradeToShared { line },
                ),
            )),
            (MsiState::Modified, MsiEvent::SnoopWrite) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::Invalid,
                actions(
                    MsiAction::SupplyData { line },
                    MsiAction::Invalidate { line },
                ),
            )),
            (MsiState::InvalidToShared, MsiEvent::DataShared) => Ok(MsiTransition::new(
                self.state,
                event,
                MsiState::Shared,
                actions(
                    MsiAction::InstallShared { line },
                    MsiAction::WakeRequester { line },
                ),
            )),
            (MsiState::InvalidToModified | MsiState::SharedToModified, MsiEvent::DataModified) => {
                Ok(MsiTransition::new(
                    self.state,
                    event,
                    MsiState::Modified,
                    actions(
                        MsiAction::InstallModified { line },
                        MsiAction::WakeRequester { line },
                    ),
                ))
            }
            (state, event) if state.is_transient() && event.is_cpu_request() => {
                Err(MsiError::Busy {
                    agent: self.agent,
                    line,
                    state,
                    event,
                })
            }
            (state, event) => Err(MsiError::UnexpectedEvent {
                agent: self.agent,
                line,
                state,
                event,
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryLineSnapshot {
    line: MsiLineId,
    caches: Vec<(AgentId, MsiState)>,
}

impl DirectoryLineSnapshot {
    pub fn new(line: MsiLineId) -> Self {
        Self {
            line,
            caches: Vec::new(),
        }
    }

    pub fn with_cache(mut self, agent: AgentId, state: MsiState) -> Self {
        self.caches.push((agent, state));
        self
    }

    pub fn validate(&self) -> Result<(), MsiError> {
        let owners: Vec<_> = self
            .caches
            .iter()
            .filter_map(|(agent, state)| state.is_modified().then_some(*agent))
            .collect();
        if owners.len() > 1 {
            return Err(MsiError::MultipleModifiedOwners {
                line: self.line,
                owners,
            });
        }

        if let Some(modified) = owners.first().copied() {
            let sharers: Vec<_> = self
                .caches
                .iter()
                .filter_map(|(agent, state)| (*state == MsiState::Shared).then_some(*agent))
                .collect();
            if !sharers.is_empty() {
                return Err(MsiError::ModifiedWithSharers {
                    line: self.line,
                    modified,
                    sharers,
                });
            }
        }

        Ok(())
    }

    pub fn caches(&self) -> &[(AgentId, MsiState)] {
        &self.caches
    }
}
