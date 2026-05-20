use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MesiLineId(Address);

impl MesiLineId {
    pub const fn new(address: Address) -> Self {
        Self(address)
    }

    pub const fn address(self) -> Address {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MesiState {
    Invalid,
    Shared,
    Exclusive,
    Modified,
    InvalidToShared,
    InvalidToExclusive,
    InvalidToModified,
    SharedToModified,
}

impl MesiState {
    pub const fn is_stable(self) -> bool {
        matches!(
            self,
            Self::Invalid | Self::Shared | Self::Exclusive | Self::Modified
        )
    }

    pub const fn is_valid(self) -> bool {
        matches!(self, Self::Shared | Self::Exclusive | Self::Modified)
    }

    pub const fn is_owned(self) -> bool {
        matches!(self, Self::Exclusive | Self::Modified)
    }

    pub const fn is_dirty(self) -> bool {
        matches!(self, Self::Modified)
    }

    pub const fn is_transient(self) -> bool {
        !self.is_stable()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MesiEvent {
    CpuRead,
    CpuWrite,
    DataShared,
    DataExclusive,
    DataModified,
    SnoopRead,
    SnoopWrite,
}

impl MesiEvent {
    pub const fn is_cpu_request(self) -> bool {
        matches!(self, Self::CpuRead | Self::CpuWrite)
    }

    pub const fn is_data_response(self) -> bool {
        matches!(
            self,
            Self::DataShared | Self::DataExclusive | Self::DataModified
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MesiAction {
    ReadHit { line: MesiLineId },
    WriteHit { line: MesiLineId },
    SendGetShared { line: MesiLineId },
    SendGetModified { line: MesiLineId },
    InstallShared { line: MesiLineId },
    InstallExclusive { line: MesiLineId },
    InstallModified { line: MesiLineId },
    WakeRequester { line: MesiLineId },
    SupplyData { line: MesiLineId },
    DowngradeToShared { line: MesiLineId },
    Invalidate { line: MesiLineId },
    SilentUpgrade { line: MesiLineId },
    Ignore { line: MesiLineId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MesiError {
    Busy {
        agent: AgentId,
        line: MesiLineId,
        state: MesiState,
        event: MesiEvent,
    },
    UnexpectedEvent {
        agent: AgentId,
        line: MesiLineId,
        state: MesiState,
        event: MesiEvent,
    },
    ForcedTransientState {
        agent: AgentId,
        line: MesiLineId,
        state: MesiState,
    },
    MultipleOwnedCopies {
        line: MesiLineId,
        owners: Vec<AgentId>,
    },
    OwnedWithSharers {
        line: MesiLineId,
        owner: AgentId,
        sharers: Vec<AgentId>,
    },
}

impl fmt::Display for MesiError {
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
            Self::MultipleOwnedCopies { line, owners } => write!(
                formatter,
                "line {:#x} has {} owned copies",
                line.address().get(),
                owners.len()
            ),
            Self::OwnedWithSharers {
                line,
                owner,
                sharers,
            } => write!(
                formatter,
                "line {:#x} has owner {} with {} sharers",
                line.address().get(),
                owner.get(),
                sharers.len()
            ),
        }
    }
}

impl Error for MesiError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiTransition {
    before: MesiState,
    event: MesiEvent,
    after: MesiState,
    actions: Vec<MesiAction>,
}

impl MesiTransition {
    fn new(
        before: MesiState,
        event: MesiEvent,
        after: MesiState,
        actions: Vec<MesiAction>,
    ) -> Self {
        Self {
            before,
            event,
            after,
            actions,
        }
    }

    pub const fn before(&self) -> MesiState {
        self.before
    }

    pub const fn event(&self) -> MesiEvent {
        self.event
    }

    pub const fn after(&self) -> MesiState {
        self.after
    }

    pub fn actions(&self) -> &[MesiAction] {
        &self.actions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MesiTraceEntry {
    agent: AgentId,
    line: MesiLineId,
    before: MesiState,
    event: MesiEvent,
    after: MesiState,
}

impl MesiTraceEntry {
    pub const fn new(
        agent: AgentId,
        line: MesiLineId,
        before: MesiState,
        event: MesiEvent,
        after: MesiState,
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

    pub const fn line(self) -> MesiLineId {
        self.line
    }

    pub const fn before(self) -> MesiState {
        self.before
    }

    pub const fn event(self) -> MesiEvent {
        self.event
    }

    pub const fn after(self) -> MesiState {
        self.after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiCacheLine {
    agent: AgentId,
    line: MesiLineId,
    state: MesiState,
    trace: Vec<MesiTraceEntry>,
}

impl MesiCacheLine {
    pub fn new(agent: AgentId, line: MesiLineId) -> Self {
        Self {
            agent,
            line,
            state: MesiState::Invalid,
            trace: Vec::new(),
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn line(&self) -> MesiLineId {
        self.line
    }

    pub const fn state(&self) -> MesiState {
        self.state
    }

    pub fn trace(&self) -> &[MesiTraceEntry] {
        &self.trace
    }

    pub fn force_state(&mut self, state: MesiState) -> Result<(), MesiError> {
        if state.is_transient() {
            return Err(MesiError::ForcedTransientState {
                agent: self.agent,
                line: self.line,
                state,
            });
        }

        self.state = state;
        Ok(())
    }

    pub fn apply(&mut self, event: MesiEvent) -> Result<MesiTransition, MesiError> {
        let before = self.state;
        let transition = self.transition(event)?;
        self.state = transition.after();
        self.trace.push(MesiTraceEntry::new(
            self.agent, self.line, before, event, self.state,
        ));
        Ok(transition)
    }

    pub fn replay(
        agent: AgentId,
        line: MesiLineId,
        events: &[MesiEvent],
    ) -> Result<Self, MesiError> {
        let mut cache = Self::new(agent, line);
        for event in events {
            cache.apply(*event)?;
        }

        Ok(cache)
    }

    fn transition(&self, event: MesiEvent) -> Result<MesiTransition, MesiError> {
        let line = self.line;
        let action = |action| vec![action];
        let actions = |left, right| vec![left, right];

        match (self.state, event) {
            (MesiState::Invalid, MesiEvent::CpuRead) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::InvalidToExclusive,
                action(MesiAction::SendGetShared { line }),
            )),
            (MesiState::Invalid, MesiEvent::CpuWrite) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::InvalidToModified,
                action(MesiAction::SendGetModified { line }),
            )),
            (MesiState::Invalid, MesiEvent::SnoopRead | MesiEvent::SnoopWrite) => {
                Ok(MesiTransition::new(
                    self.state,
                    event,
                    MesiState::Invalid,
                    action(MesiAction::Ignore { line }),
                ))
            }
            (MesiState::Shared, MesiEvent::CpuRead) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Shared,
                action(MesiAction::ReadHit { line }),
            )),
            (MesiState::Shared, MesiEvent::CpuWrite) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::SharedToModified,
                action(MesiAction::SendGetModified { line }),
            )),
            (MesiState::Shared, MesiEvent::SnoopRead) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Shared,
                action(MesiAction::Ignore { line }),
            )),
            (MesiState::Shared, MesiEvent::SnoopWrite) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Invalid,
                action(MesiAction::Invalidate { line }),
            )),
            (MesiState::Exclusive, MesiEvent::CpuRead) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Exclusive,
                action(MesiAction::ReadHit { line }),
            )),
            (MesiState::Exclusive, MesiEvent::CpuWrite) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Modified,
                actions(
                    MesiAction::SilentUpgrade { line },
                    MesiAction::WriteHit { line },
                ),
            )),
            (MesiState::Exclusive, MesiEvent::SnoopRead) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Shared,
                actions(
                    MesiAction::SupplyData { line },
                    MesiAction::DowngradeToShared { line },
                ),
            )),
            (MesiState::Exclusive, MesiEvent::SnoopWrite) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Invalid,
                action(MesiAction::Invalidate { line }),
            )),
            (MesiState::Modified, MesiEvent::CpuRead) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Modified,
                action(MesiAction::ReadHit { line }),
            )),
            (MesiState::Modified, MesiEvent::CpuWrite) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Modified,
                action(MesiAction::WriteHit { line }),
            )),
            (MesiState::Modified, MesiEvent::SnoopRead) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Shared,
                actions(
                    MesiAction::SupplyData { line },
                    MesiAction::DowngradeToShared { line },
                ),
            )),
            (MesiState::Modified, MesiEvent::SnoopWrite) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Invalid,
                actions(
                    MesiAction::SupplyData { line },
                    MesiAction::Invalidate { line },
                ),
            )),
            (MesiState::InvalidToShared, MesiEvent::DataShared) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Shared,
                actions(
                    MesiAction::InstallShared { line },
                    MesiAction::WakeRequester { line },
                ),
            )),
            (MesiState::InvalidToExclusive, MesiEvent::DataShared) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Shared,
                actions(
                    MesiAction::InstallShared { line },
                    MesiAction::WakeRequester { line },
                ),
            )),
            (MesiState::InvalidToExclusive, MesiEvent::DataExclusive) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Exclusive,
                actions(
                    MesiAction::InstallExclusive { line },
                    MesiAction::WakeRequester { line },
                ),
            )),
            (
                MesiState::InvalidToModified | MesiState::SharedToModified,
                MesiEvent::DataModified,
            ) => Ok(MesiTransition::new(
                self.state,
                event,
                MesiState::Modified,
                actions(
                    MesiAction::InstallModified { line },
                    MesiAction::WakeRequester { line },
                ),
            )),
            (state, event) if state.is_transient() && event.is_cpu_request() => {
                Err(MesiError::Busy {
                    agent: self.agent,
                    line,
                    state,
                    event,
                })
            }
            (state, event) => Err(MesiError::UnexpectedEvent {
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
    line: MesiLineId,
    caches: Vec<(AgentId, MesiState)>,
}

impl DirectoryLineSnapshot {
    pub fn new(line: MesiLineId) -> Self {
        Self {
            line,
            caches: Vec::new(),
        }
    }

    pub fn with_cache(mut self, agent: AgentId, state: MesiState) -> Self {
        self.caches.push((agent, state));
        self
    }

    pub fn validate(&self) -> Result<(), MesiError> {
        let owners: Vec<_> = self
            .caches
            .iter()
            .filter_map(|(agent, state)| state.is_owned().then_some(*agent))
            .collect();
        if owners.len() > 1 {
            return Err(MesiError::MultipleOwnedCopies {
                line: self.line,
                owners,
            });
        }

        if let Some(owner) = owners.first().copied() {
            let sharers: Vec<_> = self
                .caches
                .iter()
                .filter_map(|(agent, state)| (*state == MesiState::Shared).then_some(*agent))
                .collect();
            if !sharers.is_empty() {
                return Err(MesiError::OwnedWithSharers {
                    line: self.line,
                    owner,
                    sharers,
                });
            }
        }

        Ok(())
    }

    pub fn caches(&self) -> &[(AgentId, MesiState)] {
        &self.caches
    }
}
