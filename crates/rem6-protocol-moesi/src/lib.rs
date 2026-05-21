use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MoesiLineId(Address);

impl MoesiLineId {
    pub const fn new(address: Address) -> Self {
        Self(address)
    }

    pub const fn address(self) -> Address {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoesiState {
    Invalid,
    Shared,
    Exclusive,
    Owned,
    Modified,
    InvalidToShared,
    InvalidToExclusive,
    InvalidToModified,
    SharedToModified,
    OwnedToModified,
}

impl MoesiState {
    pub const fn is_stable(self) -> bool {
        matches!(
            self,
            Self::Invalid | Self::Shared | Self::Exclusive | Self::Owned | Self::Modified
        )
    }

    pub const fn is_valid(self) -> bool {
        matches!(
            self,
            Self::Shared | Self::Exclusive | Self::Owned | Self::Modified
        )
    }

    pub const fn is_dirty_owner(self) -> bool {
        matches!(self, Self::Owned | Self::Modified)
    }

    pub const fn is_private_owner(self) -> bool {
        matches!(self, Self::Exclusive | Self::Modified)
    }

    pub const fn is_transient(self) -> bool {
        !self.is_stable()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoesiEvent {
    CpuRead,
    CpuWrite,
    DataShared,
    DataExclusive,
    DataModified,
    SnoopRead,
    SnoopWrite,
}

impl MoesiEvent {
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
pub enum MoesiAction {
    ReadHit { line: MoesiLineId },
    WriteHit { line: MoesiLineId },
    SendGetShared { line: MoesiLineId },
    SendGetModified { line: MoesiLineId },
    InstallShared { line: MoesiLineId },
    InstallExclusive { line: MoesiLineId },
    InstallModified { line: MoesiLineId },
    WakeRequester { line: MoesiLineId },
    SupplyData { line: MoesiLineId },
    DowngradeToShared { line: MoesiLineId },
    DowngradeToOwned { line: MoesiLineId },
    Invalidate { line: MoesiLineId },
    SilentUpgrade { line: MoesiLineId },
    Ignore { line: MoesiLineId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoesiError {
    Busy {
        agent: AgentId,
        line: MoesiLineId,
        state: MoesiState,
        event: MoesiEvent,
    },
    UnexpectedEvent {
        agent: AgentId,
        line: MoesiLineId,
        state: MoesiState,
        event: MoesiEvent,
    },
    ForcedTransientState {
        agent: AgentId,
        line: MoesiLineId,
        state: MoesiState,
    },
    MultipleDirtyOwners {
        line: MoesiLineId,
        owners: Vec<AgentId>,
    },
    MultipleOwners {
        line: MoesiLineId,
        owners: Vec<AgentId>,
    },
    ExclusiveWithSharers {
        line: MoesiLineId,
        owner: AgentId,
        sharers: Vec<AgentId>,
    },
    ModifiedWithSharers {
        line: MoesiLineId,
        owner: AgentId,
        sharers: Vec<AgentId>,
    },
}

impl fmt::Display for MoesiError {
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
            Self::MultipleDirtyOwners { line, owners } => write!(
                formatter,
                "line {:#x} has {} dirty owners",
                line.address().get(),
                owners.len()
            ),
            Self::MultipleOwners { line, owners } => write!(
                formatter,
                "line {:#x} has {} owners",
                line.address().get(),
                owners.len()
            ),
            Self::ExclusiveWithSharers {
                line,
                owner,
                sharers,
            } => write!(
                formatter,
                "line {:#x} has exclusive owner {} with {} sharers",
                line.address().get(),
                owner.get(),
                sharers.len()
            ),
            Self::ModifiedWithSharers {
                line,
                owner,
                sharers,
            } => write!(
                formatter,
                "line {:#x} has modified owner {} with {} sharers",
                line.address().get(),
                owner.get(),
                sharers.len()
            ),
        }
    }
}

impl Error for MoesiError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiTransition {
    before: MoesiState,
    event: MoesiEvent,
    after: MoesiState,
    actions: Vec<MoesiAction>,
}

impl MoesiTransition {
    fn new(
        before: MoesiState,
        event: MoesiEvent,
        after: MoesiState,
        actions: Vec<MoesiAction>,
    ) -> Self {
        Self {
            before,
            event,
            after,
            actions,
        }
    }

    pub const fn before(&self) -> MoesiState {
        self.before
    }

    pub const fn event(&self) -> MoesiEvent {
        self.event
    }

    pub const fn after(&self) -> MoesiState {
        self.after
    }

    pub fn actions(&self) -> &[MoesiAction] {
        &self.actions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoesiTraceEntry {
    agent: AgentId,
    line: MoesiLineId,
    before: MoesiState,
    event: MoesiEvent,
    after: MoesiState,
}

impl MoesiTraceEntry {
    pub const fn new(
        agent: AgentId,
        line: MoesiLineId,
        before: MoesiState,
        event: MoesiEvent,
        after: MoesiState,
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

    pub const fn line(self) -> MoesiLineId {
        self.line
    }

    pub const fn before(self) -> MoesiState {
        self.before
    }

    pub const fn event(self) -> MoesiEvent {
        self.event
    }

    pub const fn after(self) -> MoesiState {
        self.after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiCacheLine {
    agent: AgentId,
    line: MoesiLineId,
    state: MoesiState,
    trace: Vec<MoesiTraceEntry>,
}

impl MoesiCacheLine {
    pub fn new(agent: AgentId, line: MoesiLineId) -> Self {
        Self {
            agent,
            line,
            state: MoesiState::Invalid,
            trace: Vec::new(),
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn line(&self) -> MoesiLineId {
        self.line
    }

    pub const fn state(&self) -> MoesiState {
        self.state
    }

    pub fn trace(&self) -> &[MoesiTraceEntry] {
        &self.trace
    }

    pub fn force_state(&mut self, state: MoesiState) -> Result<(), MoesiError> {
        if state.is_transient() {
            return Err(MoesiError::ForcedTransientState {
                agent: self.agent,
                line: self.line,
                state,
            });
        }

        self.state = state;
        Ok(())
    }

    pub fn apply(&mut self, event: MoesiEvent) -> Result<MoesiTransition, MoesiError> {
        let before = self.state;
        let transition = self.transition(event)?;
        self.state = transition.after();
        self.trace.push(MoesiTraceEntry::new(
            self.agent, self.line, before, event, self.state,
        ));
        Ok(transition)
    }

    pub fn replay(
        agent: AgentId,
        line: MoesiLineId,
        events: &[MoesiEvent],
    ) -> Result<Self, MoesiError> {
        let mut cache = Self::new(agent, line);
        for event in events {
            cache.apply(*event)?;
        }

        Ok(cache)
    }

    fn transition(&self, event: MoesiEvent) -> Result<MoesiTransition, MoesiError> {
        let line = self.line;
        let action = |action| vec![action];
        let actions = |left, right| vec![left, right];
        let dirty_invalidate = || {
            vec![
                MoesiAction::SupplyData { line },
                MoesiAction::Invalidate { line },
            ]
        };

        match (self.state, event) {
            (MoesiState::Invalid, MoesiEvent::CpuRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::InvalidToExclusive,
                action(MoesiAction::SendGetShared { line }),
            )),
            (MoesiState::Invalid, MoesiEvent::CpuWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::InvalidToModified,
                action(MoesiAction::SendGetModified { line }),
            )),
            (MoesiState::Invalid, MoesiEvent::SnoopRead | MoesiEvent::SnoopWrite) => {
                Ok(MoesiTransition::new(
                    self.state,
                    event,
                    MoesiState::Invalid,
                    action(MoesiAction::Ignore { line }),
                ))
            }
            (MoesiState::Shared, MoesiEvent::CpuRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Shared,
                action(MoesiAction::ReadHit { line }),
            )),
            (MoesiState::Shared, MoesiEvent::CpuWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::SharedToModified,
                action(MoesiAction::SendGetModified { line }),
            )),
            (MoesiState::Shared, MoesiEvent::SnoopRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Shared,
                action(MoesiAction::Ignore { line }),
            )),
            (MoesiState::Shared, MoesiEvent::SnoopWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Invalid,
                action(MoesiAction::Invalidate { line }),
            )),
            (MoesiState::Exclusive, MoesiEvent::CpuRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Exclusive,
                action(MoesiAction::ReadHit { line }),
            )),
            (MoesiState::Exclusive, MoesiEvent::CpuWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Modified,
                actions(
                    MoesiAction::SilentUpgrade { line },
                    MoesiAction::WriteHit { line },
                ),
            )),
            (MoesiState::Exclusive, MoesiEvent::SnoopRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Shared,
                actions(
                    MoesiAction::SupplyData { line },
                    MoesiAction::DowngradeToShared { line },
                ),
            )),
            (MoesiState::Exclusive, MoesiEvent::SnoopWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Invalid,
                action(MoesiAction::Invalidate { line }),
            )),
            (MoesiState::Owned, MoesiEvent::CpuRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Owned,
                action(MoesiAction::ReadHit { line }),
            )),
            (MoesiState::Owned, MoesiEvent::CpuWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::OwnedToModified,
                action(MoesiAction::SendGetModified { line }),
            )),
            (MoesiState::Owned, MoesiEvent::SnoopRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Owned,
                action(MoesiAction::SupplyData { line }),
            )),
            (MoesiState::Owned, MoesiEvent::SnoopWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Invalid,
                dirty_invalidate(),
            )),
            (MoesiState::Modified, MoesiEvent::CpuRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Modified,
                action(MoesiAction::ReadHit { line }),
            )),
            (MoesiState::Modified, MoesiEvent::CpuWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Modified,
                action(MoesiAction::WriteHit { line }),
            )),
            (MoesiState::Modified, MoesiEvent::SnoopRead) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Owned,
                actions(
                    MoesiAction::SupplyData { line },
                    MoesiAction::DowngradeToOwned { line },
                ),
            )),
            (MoesiState::Modified, MoesiEvent::SnoopWrite) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Invalid,
                dirty_invalidate(),
            )),
            (MoesiState::InvalidToShared, MoesiEvent::DataShared) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Shared,
                actions(
                    MoesiAction::InstallShared { line },
                    MoesiAction::WakeRequester { line },
                ),
            )),
            (MoesiState::InvalidToExclusive, MoesiEvent::DataShared) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Shared,
                actions(
                    MoesiAction::InstallShared { line },
                    MoesiAction::WakeRequester { line },
                ),
            )),
            (MoesiState::InvalidToExclusive, MoesiEvent::DataExclusive) => {
                Ok(MoesiTransition::new(
                    self.state,
                    event,
                    MoesiState::Exclusive,
                    actions(
                        MoesiAction::InstallExclusive { line },
                        MoesiAction::WakeRequester { line },
                    ),
                ))
            }
            (
                MoesiState::InvalidToModified
                | MoesiState::SharedToModified
                | MoesiState::OwnedToModified,
                MoesiEvent::DataModified,
            ) => Ok(MoesiTransition::new(
                self.state,
                event,
                MoesiState::Modified,
                actions(
                    MoesiAction::InstallModified { line },
                    MoesiAction::WakeRequester { line },
                ),
            )),
            (state, event) if state.is_transient() && event.is_cpu_request() => {
                Err(MoesiError::Busy {
                    agent: self.agent,
                    line,
                    state,
                    event,
                })
            }
            (state, event) => Err(MoesiError::UnexpectedEvent {
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
    line: MoesiLineId,
    caches: Vec<(AgentId, MoesiState)>,
}

impl DirectoryLineSnapshot {
    pub fn new(line: MoesiLineId) -> Self {
        Self {
            line,
            caches: Vec::new(),
        }
    }

    pub fn with_cache(mut self, agent: AgentId, state: MoesiState) -> Self {
        self.caches.push((agent, state));
        self
    }

    pub fn validate(&self) -> Result<(), MoesiError> {
        let dirty_owners: Vec<_> = self
            .caches
            .iter()
            .filter_map(|(agent, state)| state.is_dirty_owner().then_some(*agent))
            .collect();
        if dirty_owners.len() > 1 {
            return Err(MoesiError::MultipleDirtyOwners {
                line: self.line,
                owners: dirty_owners,
            });
        }

        let owners: Vec<_> = self
            .caches
            .iter()
            .filter_map(|(agent, state)| {
                matches!(
                    state,
                    MoesiState::Exclusive | MoesiState::Owned | MoesiState::Modified
                )
                .then_some(*agent)
            })
            .collect();
        if owners.len() > 1 {
            return Err(MoesiError::MultipleOwners {
                line: self.line,
                owners,
            });
        }

        if let Some(exclusive) = self
            .caches
            .iter()
            .find_map(|(agent, state)| (*state == MoesiState::Exclusive).then_some(*agent))
        {
            let sharers: Vec<_> = self
                .caches
                .iter()
                .filter_map(|(agent, state)| {
                    matches!(state, MoesiState::Shared | MoesiState::Owned).then_some(*agent)
                })
                .collect();
            if !sharers.is_empty() {
                return Err(MoesiError::ExclusiveWithSharers {
                    line: self.line,
                    owner: exclusive,
                    sharers,
                });
            }
        }

        if let Some(modified) = self
            .caches
            .iter()
            .find_map(|(agent, state)| (*state == MoesiState::Modified).then_some(*agent))
        {
            let sharers: Vec<_> = self
                .caches
                .iter()
                .filter_map(|(agent, state)| (*state == MoesiState::Shared).then_some(*agent))
                .collect();
            if !sharers.is_empty() {
                return Err(MoesiError::ModifiedWithSharers {
                    line: self.line,
                    owner: modified,
                    sharers,
                });
            }
        }

        Ok(())
    }

    pub fn caches(&self) -> &[(AgentId, MoesiState)] {
        &self.caches
    }
}
