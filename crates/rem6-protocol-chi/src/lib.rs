use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChiLineId(Address);

impl ChiLineId {
    pub const fn new(address: Address) -> Self {
        Self(address)
    }

    pub const fn address(self) -> Address {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChiState {
    Invalid,
    SharedClean,
    SharedDirty,
    UniqueClean,
    UniqueDirty,
    InvalidToSharedClean,
    InvalidToUniqueDirty,
    SharedCleanToUniqueClean,
    SharedDirtyToUniqueDirty,
}

impl ChiState {
    pub const fn is_stable(self) -> bool {
        matches!(
            self,
            Self::Invalid
                | Self::SharedClean
                | Self::SharedDirty
                | Self::UniqueClean
                | Self::UniqueDirty
        )
    }

    pub const fn is_valid(self) -> bool {
        matches!(
            self,
            Self::SharedClean | Self::SharedDirty | Self::UniqueClean | Self::UniqueDirty
        )
    }

    pub const fn is_shared(self) -> bool {
        matches!(self, Self::SharedClean | Self::SharedDirty)
    }

    pub const fn is_unique(self) -> bool {
        matches!(self, Self::UniqueClean | Self::UniqueDirty)
    }

    pub const fn is_dirty(self) -> bool {
        matches!(self, Self::SharedDirty | Self::UniqueDirty)
    }

    pub const fn is_transient(self) -> bool {
        !self.is_stable()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChiEvent {
    CpuRead,
    CpuWrite,
    CompDataSharedClean,
    CompDataSharedDirty,
    CompDataUniqueClean,
    CompDataUniqueDirty,
    SnoopShared,
    SnoopUnique,
}

impl ChiEvent {
    pub const fn is_cpu_request(self) -> bool {
        matches!(self, Self::CpuRead | Self::CpuWrite)
    }

    pub const fn is_data_response(self) -> bool {
        matches!(
            self,
            Self::CompDataSharedClean
                | Self::CompDataSharedDirty
                | Self::CompDataUniqueClean
                | Self::CompDataUniqueDirty
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChiAction {
    ReadHit { line: ChiLineId },
    WriteHit { line: ChiLineId },
    SendReadShared { line: ChiLineId },
    SendReadUnique { line: ChiLineId },
    SendMakeReadUnique { line: ChiLineId },
    InstallSharedClean { line: ChiLineId },
    InstallSharedDirty { line: ChiLineId },
    InstallUniqueClean { line: ChiLineId },
    InstallUniqueDirty { line: ChiLineId },
    WakeRequester { line: ChiLineId },
    SnoopData { line: ChiLineId },
    DowngradeToSharedClean { line: ChiLineId },
    Invalidate { line: ChiLineId },
    Ignore { line: ChiLineId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChiError {
    Busy {
        agent: AgentId,
        line: ChiLineId,
        state: ChiState,
        event: ChiEvent,
    },
    UnexpectedEvent {
        agent: AgentId,
        line: ChiLineId,
        state: ChiState,
        event: ChiEvent,
    },
    ForcedTransientState {
        agent: AgentId,
        line: ChiLineId,
        state: ChiState,
    },
    MultipleUniqueOwners {
        line: ChiLineId,
        owners: Vec<AgentId>,
    },
    UniqueWithSharers {
        line: ChiLineId,
        owner: AgentId,
        sharers: Vec<AgentId>,
    },
}

impl fmt::Display for ChiError {
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
            Self::MultipleUniqueOwners { line, owners } => write!(
                formatter,
                "line {:#x} has {} unique owners",
                line.address().get(),
                owners.len()
            ),
            Self::UniqueWithSharers {
                line,
                owner,
                sharers,
            } => write!(
                formatter,
                "line {:#x} has unique owner {} with {} sharers",
                line.address().get(),
                owner.get(),
                sharers.len()
            ),
        }
    }
}

impl Error for ChiError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiTransition {
    before: ChiState,
    event: ChiEvent,
    after: ChiState,
    actions: Vec<ChiAction>,
}

impl ChiTransition {
    fn new(before: ChiState, event: ChiEvent, after: ChiState, actions: Vec<ChiAction>) -> Self {
        Self {
            before,
            event,
            after,
            actions,
        }
    }

    pub const fn before(&self) -> ChiState {
        self.before
    }

    pub const fn event(&self) -> ChiEvent {
        self.event
    }

    pub const fn after(&self) -> ChiState {
        self.after
    }

    pub fn actions(&self) -> &[ChiAction] {
        &self.actions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChiTraceEntry {
    agent: AgentId,
    line: ChiLineId,
    before: ChiState,
    event: ChiEvent,
    after: ChiState,
}

impl ChiTraceEntry {
    pub const fn new(
        agent: AgentId,
        line: ChiLineId,
        before: ChiState,
        event: ChiEvent,
        after: ChiState,
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

    pub const fn line(self) -> ChiLineId {
        self.line
    }

    pub const fn before(self) -> ChiState {
        self.before
    }

    pub const fn event(self) -> ChiEvent {
        self.event
    }

    pub const fn after(self) -> ChiState {
        self.after
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChiReservationInvalidationReason {
    RemoteWrite,
    RemoteAtomic,
    SnoopInvalidation,
    Eviction,
    StoreConditionalSuccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChiReservation {
    agent: AgentId,
    line: ChiLineId,
    address: Address,
    size: rem6_memory::AccessSize,
}

impl ChiReservation {
    pub const fn new(
        agent: AgentId,
        line: ChiLineId,
        address: Address,
        size: rem6_memory::AccessSize,
    ) -> Self {
        Self {
            agent,
            line,
            address,
            size,
        }
    }

    pub const fn agent(self) -> AgentId {
        self.agent
    }

    pub const fn line(self) -> ChiLineId {
        self.line
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn size(self) -> rem6_memory::AccessSize {
        self.size
    }

    pub fn overlaps(self, address: Address, size: rem6_memory::AccessSize) -> bool {
        let reservation_start = self.address.get();
        let reservation_end = reservation_start.saturating_add(self.size.bytes());
        let access_start = address.get();
        let access_end = access_start.saturating_add(size.bytes());
        reservation_start < access_end && access_start < reservation_end
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChiReservationInvalidation {
    agent: AgentId,
    line: ChiLineId,
    address: Address,
    size: rem6_memory::AccessSize,
    reason: ChiReservationInvalidationReason,
}

impl ChiReservationInvalidation {
    pub const fn new(
        agent: AgentId,
        line: ChiLineId,
        address: Address,
        size: rem6_memory::AccessSize,
        reason: ChiReservationInvalidationReason,
    ) -> Self {
        Self {
            agent,
            line,
            address,
            size,
            reason,
        }
    }

    pub const fn agent(self) -> AgentId {
        self.agent
    }

    pub const fn line(self) -> ChiLineId {
        self.line
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn size(self) -> rem6_memory::AccessSize {
        self.size
    }

    pub const fn reason(self) -> ChiReservationInvalidationReason {
        self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiStoreConditionalOutcome {
    agent: AgentId,
    line: ChiLineId,
    address: Address,
    size: rem6_memory::AccessSize,
    succeeded: bool,
    invalidations: Vec<ChiReservationInvalidation>,
}

impl ChiStoreConditionalOutcome {
    fn new(
        agent: AgentId,
        line: ChiLineId,
        address: Address,
        size: rem6_memory::AccessSize,
        succeeded: bool,
        invalidations: Vec<ChiReservationInvalidation>,
    ) -> Self {
        Self {
            agent,
            line,
            address,
            size,
            succeeded,
            invalidations,
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn size(&self) -> rem6_memory::AccessSize {
        self.size
    }

    pub const fn succeeded(&self) -> bool {
        self.succeeded
    }

    pub fn invalidations(&self) -> &[ChiReservationInvalidation] {
        &self.invalidations
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChiReservationTable {
    reservations: BTreeMap<ChiLineId, BTreeMap<AgentId, ChiReservation>>,
}

impl ChiReservationTable {
    pub fn reserve(
        &mut self,
        agent: AgentId,
        line: ChiLineId,
        address: Address,
        size: rem6_memory::AccessSize,
    ) -> ChiReservation {
        let reservation = ChiReservation::new(agent, line, address, size);
        self.reservations
            .entry(line)
            .or_default()
            .insert(agent, reservation);
        reservation
    }

    pub fn is_reserved(&self, agent: AgentId, line: ChiLineId) -> bool {
        self.reservation(agent, line).is_some()
    }

    pub fn reservation(&self, agent: AgentId, line: ChiLineId) -> Option<ChiReservation> {
        self.reservations
            .get(&line)
            .and_then(|line_reservations| line_reservations.get(&agent))
            .copied()
    }

    pub fn store_conditional(
        &mut self,
        agent: AgentId,
        line: ChiLineId,
        address: Address,
        size: rem6_memory::AccessSize,
    ) -> ChiStoreConditionalOutcome {
        let succeeded = self
            .reservation(agent, line)
            .is_some_and(|reservation| reservation.overlaps(address, size));
        self.remove_agent_reservation(agent, line);

        let invalidations = if succeeded {
            self.invalidate_overlapping(
                line,
                address,
                size,
                ChiReservationInvalidationReason::StoreConditionalSuccess,
            )
        } else {
            Vec::new()
        };

        ChiStoreConditionalOutcome::new(agent, line, address, size, succeeded, invalidations)
    }

    pub fn invalidate_overlapping(
        &mut self,
        line: ChiLineId,
        address: Address,
        size: rem6_memory::AccessSize,
        reason: ChiReservationInvalidationReason,
    ) -> Vec<ChiReservationInvalidation> {
        let Some(line_reservations) = self.reservations.get_mut(&line) else {
            return Vec::new();
        };

        let invalidated_agents = line_reservations
            .iter()
            .filter_map(|(agent, reservation)| {
                reservation.overlaps(address, size).then_some(*agent)
            })
            .collect::<Vec<_>>();
        let invalidations = invalidated_agents
            .iter()
            .filter_map(|agent| line_reservations.remove(agent))
            .map(|reservation| {
                ChiReservationInvalidation::new(
                    reservation.agent(),
                    reservation.line(),
                    reservation.address(),
                    reservation.size(),
                    reason,
                )
            })
            .collect::<Vec<_>>();

        if line_reservations.is_empty() {
            self.reservations.remove(&line);
        }

        invalidations
    }

    pub fn discard(
        &mut self,
        agent: AgentId,
        line: ChiLineId,
        reason: ChiReservationInvalidationReason,
    ) -> Option<ChiReservationInvalidation> {
        self.remove_agent_reservation(agent, line)
            .map(|reservation| {
                ChiReservationInvalidation::new(
                    reservation.agent(),
                    reservation.line(),
                    reservation.address(),
                    reservation.size(),
                    reason,
                )
            })
    }

    fn remove_agent_reservation(
        &mut self,
        agent: AgentId,
        line: ChiLineId,
    ) -> Option<ChiReservation> {
        let line_reservations = self.reservations.get_mut(&line)?;
        let reservation = line_reservations.remove(&agent);
        if line_reservations.is_empty() {
            self.reservations.remove(&line);
        }
        reservation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiCacheLine {
    agent: AgentId,
    line: ChiLineId,
    state: ChiState,
    trace: Vec<ChiTraceEntry>,
}

impl ChiCacheLine {
    pub fn new(agent: AgentId, line: ChiLineId) -> Self {
        Self {
            agent,
            line,
            state: ChiState::Invalid,
            trace: Vec::new(),
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn state(&self) -> ChiState {
        self.state
    }

    pub fn trace(&self) -> &[ChiTraceEntry] {
        &self.trace
    }

    pub fn force_state(&mut self, state: ChiState) -> Result<(), ChiError> {
        if state.is_transient() {
            return Err(ChiError::ForcedTransientState {
                agent: self.agent,
                line: self.line,
                state,
            });
        }

        self.state = state;
        Ok(())
    }

    pub fn apply(&mut self, event: ChiEvent) -> Result<ChiTransition, ChiError> {
        let before = self.state;
        let transition = self.transition(event)?;
        self.state = transition.after();
        self.trace.push(ChiTraceEntry::new(
            self.agent, self.line, before, event, self.state,
        ));
        Ok(transition)
    }

    pub fn replay(agent: AgentId, line: ChiLineId, events: &[ChiEvent]) -> Result<Self, ChiError> {
        let mut cache = Self::new(agent, line);
        for event in events {
            cache.apply(*event)?;
        }

        Ok(cache)
    }

    fn transition(&self, event: ChiEvent) -> Result<ChiTransition, ChiError> {
        let line = self.line;
        let one = |action| vec![action];
        let two = |left, right| vec![left, right];

        match (self.state, event) {
            (ChiState::Invalid, ChiEvent::CpuRead) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::InvalidToSharedClean,
                one(ChiAction::SendReadShared { line }),
            )),
            (ChiState::Invalid, ChiEvent::CpuWrite) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::InvalidToUniqueDirty,
                one(ChiAction::SendReadUnique { line }),
            )),
            (ChiState::Invalid, ChiEvent::SnoopShared | ChiEvent::SnoopUnique) => {
                Ok(ChiTransition::new(
                    self.state,
                    event,
                    ChiState::Invalid,
                    one(ChiAction::Ignore { line }),
                ))
            }
            (ChiState::SharedClean, ChiEvent::CpuRead)
            | (ChiState::SharedDirty, ChiEvent::CpuRead)
            | (ChiState::UniqueClean, ChiEvent::CpuRead)
            | (ChiState::UniqueDirty, ChiEvent::CpuRead) => Ok(ChiTransition::new(
                self.state,
                event,
                self.state,
                one(ChiAction::ReadHit { line }),
            )),
            (ChiState::SharedClean, ChiEvent::CpuWrite) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::SharedCleanToUniqueClean,
                one(ChiAction::SendMakeReadUnique { line }),
            )),
            (ChiState::SharedDirty, ChiEvent::CpuWrite) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::SharedDirtyToUniqueDirty,
                one(ChiAction::SendMakeReadUnique { line }),
            )),
            (ChiState::UniqueClean, ChiEvent::CpuWrite) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::UniqueDirty,
                one(ChiAction::WriteHit { line }),
            )),
            (ChiState::UniqueDirty, ChiEvent::CpuWrite) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::UniqueDirty,
                one(ChiAction::WriteHit { line }),
            )),
            (ChiState::SharedClean, ChiEvent::SnoopShared) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::SharedClean,
                one(ChiAction::Ignore { line }),
            )),
            (ChiState::SharedDirty, ChiEvent::SnoopShared) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::SharedDirty,
                one(ChiAction::SnoopData { line }),
            )),
            (ChiState::UniqueClean | ChiState::UniqueDirty, ChiEvent::SnoopShared) => {
                Ok(ChiTransition::new(
                    self.state,
                    event,
                    ChiState::SharedClean,
                    two(
                        ChiAction::SnoopData { line },
                        ChiAction::DowngradeToSharedClean { line },
                    ),
                ))
            }
            (ChiState::SharedClean | ChiState::SharedDirty, ChiEvent::SnoopUnique) => {
                Ok(ChiTransition::new(
                    self.state,
                    event,
                    ChiState::Invalid,
                    one(ChiAction::Invalidate { line }),
                ))
            }
            (ChiState::UniqueClean, ChiEvent::SnoopUnique) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::Invalid,
                one(ChiAction::Invalidate { line }),
            )),
            (ChiState::UniqueDirty, ChiEvent::SnoopUnique) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::Invalid,
                two(
                    ChiAction::SnoopData { line },
                    ChiAction::Invalidate { line },
                ),
            )),
            (ChiState::InvalidToSharedClean, ChiEvent::CompDataSharedClean) => {
                Ok(ChiTransition::new(
                    self.state,
                    event,
                    ChiState::SharedClean,
                    two(
                        ChiAction::InstallSharedClean { line },
                        ChiAction::WakeRequester { line },
                    ),
                ))
            }
            (ChiState::InvalidToSharedClean, ChiEvent::CompDataSharedDirty) => {
                Ok(ChiTransition::new(
                    self.state,
                    event,
                    ChiState::SharedDirty,
                    two(
                        ChiAction::InstallSharedDirty { line },
                        ChiAction::WakeRequester { line },
                    ),
                ))
            }
            (ChiState::SharedCleanToUniqueClean, ChiEvent::CompDataUniqueClean) => {
                Ok(ChiTransition::new(
                    self.state,
                    event,
                    ChiState::UniqueClean,
                    two(
                        ChiAction::InstallUniqueClean { line },
                        ChiAction::WakeRequester { line },
                    ),
                ))
            }
            (
                ChiState::InvalidToUniqueDirty
                | ChiState::SharedCleanToUniqueClean
                | ChiState::SharedDirtyToUniqueDirty,
                ChiEvent::CompDataUniqueDirty,
            ) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::UniqueDirty,
                two(
                    ChiAction::InstallUniqueDirty { line },
                    ChiAction::WakeRequester { line },
                ),
            )),
            (
                ChiState::InvalidToUniqueDirty | ChiState::SharedDirtyToUniqueDirty,
                ChiEvent::CompDataUniqueClean,
            ) => Ok(ChiTransition::new(
                self.state,
                event,
                ChiState::UniqueDirty,
                two(
                    ChiAction::InstallUniqueDirty { line },
                    ChiAction::WakeRequester { line },
                ),
            )),
            (state, event) if state.is_transient() && event.is_cpu_request() => {
                Err(ChiError::Busy {
                    agent: self.agent,
                    line,
                    state,
                    event,
                })
            }
            (state, event) => Err(ChiError::UnexpectedEvent {
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
    line: ChiLineId,
    caches: Vec<(AgentId, ChiState)>,
}

impl DirectoryLineSnapshot {
    pub fn new(line: ChiLineId) -> Self {
        Self {
            line,
            caches: Vec::new(),
        }
    }

    pub fn with_cache(mut self, agent: AgentId, state: ChiState) -> Self {
        self.caches.push((agent, state));
        self
    }

    pub fn validate(&self) -> Result<(), ChiError> {
        let unique_owners: Vec<_> = self
            .caches
            .iter()
            .filter_map(|(agent, state)| state.is_unique().then_some(*agent))
            .collect();
        if unique_owners.len() > 1 {
            return Err(ChiError::MultipleUniqueOwners {
                line: self.line,
                owners: unique_owners,
            });
        }

        if let Some(owner) = unique_owners.first().copied() {
            let sharers: Vec<_> = self
                .caches
                .iter()
                .filter_map(|(agent, state)| state.is_shared().then_some(*agent))
                .collect();
            if !sharers.is_empty() {
                return Err(ChiError::UniqueWithSharers {
                    line: self.line,
                    owner,
                    sharers,
                });
            }
        }

        Ok(())
    }

    pub fn caches(&self) -> &[(AgentId, ChiState)] {
        &self.caches
    }
}
