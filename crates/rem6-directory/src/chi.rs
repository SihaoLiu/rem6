use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId, MemoryOperation, MemoryRequest, MemoryRequestId};
use rem6_protocol_chi::{
    ChiError, ChiEvent, ChiLineId, ChiState, DirectoryLineSnapshot as ChiDirectorySnapshot,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChiDirectoryDataSource {
    BackingMemory,
    OwnerCache(AgentId),
    NoData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChiDirectoryGrant {
    request: MemoryRequestId,
    line: ChiLineId,
    state: ChiState,
    data_source: ChiDirectoryDataSource,
}

impl ChiDirectoryGrant {
    pub const fn new(
        request: MemoryRequestId,
        line: ChiLineId,
        state: ChiState,
        data_source: ChiDirectoryDataSource,
    ) -> Self {
        Self {
            request,
            line,
            state,
            data_source,
        }
    }

    pub const fn request(self) -> MemoryRequestId {
        self.request
    }

    pub const fn line(self) -> ChiLineId {
        self.line
    }

    pub const fn state(self) -> ChiState {
        self.state
    }

    pub const fn data_source(self) -> ChiDirectoryDataSource {
        self.data_source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChiDirectorySnoop {
    target: AgentId,
    event: ChiEvent,
}

impl ChiDirectorySnoop {
    pub const fn new(target: AgentId, event: ChiEvent) -> Self {
        Self { target, event }
    }

    pub const fn target(self) -> AgentId {
        self.target
    }

    pub const fn event(self) -> ChiEvent {
        self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiDirectoryLineState {
    line: ChiLineId,
    unique_owner: Option<(AgentId, ChiState)>,
    sharers: Vec<(AgentId, ChiState)>,
}

impl ChiDirectoryLineState {
    pub fn new(line: ChiLineId) -> Self {
        Self {
            line,
            unique_owner: None,
            sharers: Vec::new(),
        }
    }

    pub fn with_unique_owner(mut self, owner: AgentId, state: ChiState) -> Self {
        self.unique_owner = Some((owner, state));
        self.sharers.clear();
        self
    }

    pub fn with_sharer(mut self, sharer: AgentId, state: ChiState) -> Self {
        self.unique_owner = None;
        if let Some((_, stored_state)) = self
            .sharers
            .iter_mut()
            .find(|(stored_sharer, _)| *stored_sharer == sharer)
        {
            *stored_state = state;
        } else {
            self.sharers.push((sharer, state));
            self.sharers
                .sort_by(|(left, _), (right, _)| left.cmp(right));
        }
        self
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn unique_owner(&self) -> Option<AgentId> {
        match self.unique_owner {
            Some((owner, _)) => Some(owner),
            None => None,
        }
    }

    pub const fn unique_owner_state(&self) -> Option<ChiState> {
        match self.unique_owner {
            Some((_, state)) => Some(state),
            None => None,
        }
    }

    pub fn sharers(&self) -> &[(AgentId, ChiState)] {
        &self.sharers
    }

    pub fn protocol_snapshot(&self) -> ChiDirectorySnapshot {
        let mut snapshot = ChiDirectorySnapshot::new(self.line);
        if let Some((owner, state)) = self.unique_owner {
            snapshot = snapshot.with_cache(owner, state);
        }
        for (sharer, state) in &self.sharers {
            snapshot = snapshot.with_cache(*sharer, *state);
        }

        snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiDirectoryDecision {
    line: ChiLineId,
    request: MemoryRequestId,
    before: ChiDirectoryLineState,
    after: ChiDirectoryLineState,
    snoops: Vec<ChiDirectorySnoop>,
    grant: Option<ChiDirectoryGrant>,
}

impl ChiDirectoryDecision {
    pub fn new(
        line: ChiLineId,
        request: MemoryRequestId,
        before: ChiDirectoryLineState,
        after: ChiDirectoryLineState,
        snoops: Vec<ChiDirectorySnoop>,
        grant: Option<ChiDirectoryGrant>,
    ) -> Self {
        Self {
            line,
            request,
            before,
            after,
            snoops,
            grant,
        }
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn before(&self) -> &ChiDirectoryLineState {
        &self.before
    }

    pub const fn after(&self) -> &ChiDirectoryLineState {
        &self.after
    }

    pub fn snoops(&self) -> &[ChiDirectorySnoop] {
        &self.snoops
    }

    pub const fn grant(&self) -> Option<&ChiDirectoryGrant> {
        self.grant.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiEvictHazard {
    line: ChiLineId,
    requester: AgentId,
    retained_state: ChiDirectoryLineState,
}

impl ChiEvictHazard {
    fn new(line: ChiLineId, requester: AgentId, retained_state: ChiDirectoryLineState) -> Self {
        Self {
            line,
            requester,
            retained_state,
        }
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn requester(&self) -> AgentId {
        self.requester
    }

    pub const fn retained_state(&self) -> &ChiDirectoryLineState {
        &self.retained_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiEvictHazardRestore {
    line: ChiLineId,
    requester: AgentId,
    retained_state: ChiDirectoryLineState,
    current_state: ChiDirectoryLineState,
    request_became_stale: bool,
}

impl ChiEvictHazardRestore {
    fn new(
        line: ChiLineId,
        requester: AgentId,
        retained_state: ChiDirectoryLineState,
        current_state: ChiDirectoryLineState,
        request_became_stale: bool,
    ) -> Self {
        Self {
            line,
            requester,
            retained_state,
            current_state,
            request_became_stale,
        }
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn requester(&self) -> AgentId {
        self.requester
    }

    pub const fn acknowledgement_target(&self) -> AgentId {
        self.requester
    }

    pub const fn retained_state(&self) -> &ChiDirectoryLineState {
        &self.retained_state
    }

    pub const fn current_state(&self) -> &ChiDirectoryLineState {
        &self.current_state
    }

    pub const fn request_became_stale(&self) -> bool {
        self.request_became_stale
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChiDirectoryError {
    UpgradeRequesterNotSharer {
        line: ChiLineId,
        requester: AgentId,
    },
    InvalidSnapshotOwnerState {
        line: ChiLineId,
        state: ChiState,
    },
    InvalidSnapshotSharerState {
        line: ChiLineId,
        sharer: AgentId,
        state: ChiState,
    },
    InvalidSnapshotProtocol {
        line: ChiLineId,
        error: ChiError,
    },
    WritebackFromNonOwner {
        line: ChiLineId,
        requester: AgentId,
        owner: Option<AgentId>,
    },
    EvictFromNonHolder {
        line: ChiLineId,
        requester: AgentId,
    },
}

impl fmt::Display for ChiDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpgradeRequesterNotSharer { line, requester } => write!(
                formatter,
                "agent {} cannot upgrade CHI line {:#x} without shared ownership",
                requester.get(),
                line.address().get()
            ),
            Self::InvalidSnapshotOwnerState { line, state } => write!(
                formatter,
                "snapshot owner for CHI line {:#x} cannot be restored in {state:?}",
                line.address().get()
            ),
            Self::InvalidSnapshotSharerState {
                line,
                sharer,
                state,
            } => write!(
                formatter,
                "snapshot sharer {} for CHI line {:#x} cannot be restored in {state:?}",
                sharer.get(),
                line.address().get()
            ),
            Self::InvalidSnapshotProtocol { line, error } => write!(
                formatter,
                "snapshot for CHI line {:#x} violates directory invariants: {error}",
                line.address().get()
            ),
            Self::WritebackFromNonOwner {
                line,
                requester,
                owner,
            } => write!(
                formatter,
                "agent {} cannot write back dirty CHI line {:#x}; owner is {:?}",
                requester.get(),
                line.address().get(),
                owner.map(AgentId::get)
            ),
            Self::EvictFromNonHolder { line, requester } => write!(
                formatter,
                "agent {} cannot evict CHI line {:#x} without ownership",
                requester.get(),
                line.address().get()
            ),
        }
    }
}

impl Error for ChiDirectoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidSnapshotProtocol { error, .. } => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ChiStoredLine {
    unique_owner: Option<(AgentId, ChiState)>,
    sharers: BTreeMap<AgentId, ChiState>,
}

impl ChiStoredLine {
    fn is_empty(&self) -> bool {
        self.unique_owner.is_none() && self.sharers.is_empty()
    }

    fn from_snapshot(snapshot: &ChiDirectoryLineState) -> Result<Self, ChiDirectoryError> {
        if let Some((_, state)) = snapshot.unique_owner {
            if !state.is_stable() || !state.is_unique() {
                return Err(ChiDirectoryError::InvalidSnapshotOwnerState {
                    line: snapshot.line(),
                    state,
                });
            }
        }

        let mut sharers = BTreeMap::new();
        for (sharer, state) in snapshot.sharers() {
            if !state.is_stable() || !state.is_shared() {
                return Err(ChiDirectoryError::InvalidSnapshotSharerState {
                    line: snapshot.line(),
                    sharer: *sharer,
                    state: *state,
                });
            }
            sharers.insert(*sharer, *state);
        }

        snapshot.protocol_snapshot().validate().map_err(|error| {
            ChiDirectoryError::InvalidSnapshotProtocol {
                line: snapshot.line(),
                error,
            }
        })?;

        Ok(Self {
            unique_owner: snapshot.unique_owner,
            sharers,
        })
    }

    fn snapshot(&self, line: ChiLineId) -> ChiDirectoryLineState {
        let mut snapshot = ChiDirectoryLineState::new(line);
        if let Some((owner, state)) = self.unique_owner {
            snapshot = snapshot.with_unique_owner(owner, state);
        }
        for (sharer, state) in &self.sharers {
            snapshot = snapshot.with_sharer(*sharer, *state);
        }

        snapshot
    }

    fn contains_sharer(&self, requester: AgentId) -> bool {
        self.sharers.contains_key(&requester)
    }

    fn first_dirty_sharer_except(&self, except: Option<AgentId>) -> Option<AgentId> {
        self.sharers
            .iter()
            .find(|(sharer, state)| Some(**sharer) != except && state.is_dirty())
            .map(|(sharer, _)| *sharer)
    }

    fn dirty_holder(&self) -> Option<AgentId> {
        self.unique_owner
            .filter(|(_, state)| state.is_dirty())
            .map(|(owner, _)| owner)
            .or_else(|| self.first_dirty_sharer_except(None))
    }

    fn contains_holder(&self, requester: AgentId) -> bool {
        self.unique_owner
            .is_some_and(|(owner, _)| owner == requester)
            || self.sharers.contains_key(&requester)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChiDirectory {
    lines: BTreeMap<ChiLineId, ChiStoredLine>,
}

impl ChiDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn line_state(&self, line: ChiLineId) -> ChiDirectoryLineState {
        self.lines
            .get(&line)
            .map(|stored| stored.snapshot(line))
            .unwrap_or_else(|| ChiDirectoryLineState::new(line))
    }

    pub fn line_addresses(&self) -> Vec<Address> {
        self.lines.keys().map(|line| line.address()).collect()
    }

    pub fn line_states(&self) -> Vec<ChiDirectoryLineState> {
        self.lines
            .iter()
            .map(|(line, stored)| stored.snapshot(*line))
            .collect()
    }

    pub fn restore_line_state(
        &mut self,
        snapshot: &ChiDirectoryLineState,
    ) -> Result<(), ChiDirectoryError> {
        let line = snapshot.line();
        let stored = ChiStoredLine::from_snapshot(snapshot)?;
        if stored.is_empty() {
            self.lines.remove(&line);
        } else {
            self.lines.insert(line, stored);
        }
        Ok(())
    }

    pub fn restore_line_states(
        &mut self,
        snapshots: &[ChiDirectoryLineState],
    ) -> Result<(), ChiDirectoryError> {
        let mut restored = BTreeMap::new();
        for snapshot in snapshots {
            let stored = ChiStoredLine::from_snapshot(snapshot)?;
            if !stored.is_empty() {
                restored.insert(snapshot.line(), stored);
            }
        }
        self.lines = restored;
        Ok(())
    }

    pub fn accept(
        &mut self,
        request: MemoryRequest,
    ) -> Result<ChiDirectoryDecision, ChiDirectoryError> {
        let line = ChiLineId::new(request.line_address());
        let request_id = request.id();
        let requester = request_id.agent();
        let before_line = self.lines.get(&line).cloned().unwrap_or_default();
        let before = before_line.snapshot(line);
        let mut after_line = before_line;

        let (snoops, grant) = match request.operation() {
            MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::PrefetchRead => {
                self.accept_read_shared(line, request_id, requester, &mut after_line)
            }
            MemoryOperation::ReadUnique
            | MemoryOperation::Write
            | MemoryOperation::Atomic
            | MemoryOperation::PrefetchWrite => {
                self.accept_read_unique(line, request_id, requester, &mut after_line)
            }
            MemoryOperation::Upgrade => {
                self.accept_upgrade(line, request_id, requester, &mut after_line)?
            }
            MemoryOperation::WritebackDirty => {
                self.accept_dirty_writeback(line, requester, &mut after_line)?
            }
            MemoryOperation::WriteClean => {
                self.accept_write_clean(line, requester, &mut after_line)?
            }
            MemoryOperation::WritebackClean
            | MemoryOperation::CleanEvict
            | MemoryOperation::Invalidate => {
                self.accept_clean_departure(line, requester, &mut after_line)?
            }
        };

        let after = after_line.snapshot(line);
        if after_line.is_empty() {
            self.lines.remove(&line);
        } else {
            self.lines.insert(line, after_line);
        }

        Ok(ChiDirectoryDecision::new(
            line, request_id, before, after, snoops, grant,
        ))
    }

    pub fn begin_evict_hazard(
        &self,
        line: ChiLineId,
        requester: AgentId,
    ) -> Result<ChiEvictHazard, ChiDirectoryError> {
        let retained_line = self.lines.get(&line).cloned().unwrap_or_default();
        if !retained_line.contains_holder(requester) {
            return Err(ChiDirectoryError::EvictFromNonHolder { line, requester });
        }

        Ok(ChiEvictHazard::new(
            line,
            requester,
            retained_line.snapshot(line),
        ))
    }

    pub fn restore_evict_hazard(
        &self,
        hazard: &ChiEvictHazard,
    ) -> Result<ChiEvictHazardRestore, ChiDirectoryError> {
        let current_line = self.lines.get(&hazard.line()).cloned().unwrap_or_default();
        let request_became_stale = !current_line.contains_holder(hazard.requester());

        Ok(ChiEvictHazardRestore::new(
            hazard.line(),
            hazard.requester(),
            hazard.retained_state().clone(),
            current_line.snapshot(hazard.line()),
            request_became_stale,
        ))
    }

    fn accept_read_shared(
        &self,
        line: ChiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut ChiStoredLine,
    ) -> (Vec<ChiDirectorySnoop>, Option<ChiDirectoryGrant>) {
        if let Some((owner, owner_state)) = state.unique_owner {
            if owner == requester {
                return (
                    Vec::new(),
                    Some(ChiDirectoryGrant::new(
                        request,
                        line,
                        owner_state,
                        ChiDirectoryDataSource::NoData,
                    )),
                );
            }

            state.unique_owner = None;
            state.sharers.insert(owner, ChiState::SharedClean);
            state.sharers.insert(requester, ChiState::SharedClean);
            return (
                vec![ChiDirectorySnoop::new(owner, ChiEvent::SnoopShared)],
                Some(ChiDirectoryGrant::new(
                    request,
                    line,
                    ChiState::SharedClean,
                    ChiDirectoryDataSource::OwnerCache(owner),
                )),
            );
        }

        if let Some(requester_state) = state.sharers.get(&requester).copied() {
            return (
                Vec::new(),
                Some(ChiDirectoryGrant::new(
                    request,
                    line,
                    requester_state,
                    ChiDirectoryDataSource::NoData,
                )),
            );
        }

        let dirty_source = state.first_dirty_sharer_except(Some(requester));
        state.sharers.insert(requester, ChiState::SharedClean);

        match dirty_source {
            Some(source) => (
                vec![ChiDirectorySnoop::new(source, ChiEvent::SnoopShared)],
                Some(ChiDirectoryGrant::new(
                    request,
                    line,
                    ChiState::SharedClean,
                    ChiDirectoryDataSource::OwnerCache(source),
                )),
            ),
            None => (
                Vec::new(),
                Some(ChiDirectoryGrant::new(
                    request,
                    line,
                    ChiState::SharedClean,
                    ChiDirectoryDataSource::BackingMemory,
                )),
            ),
        }
    }

    fn accept_read_unique(
        &self,
        line: ChiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut ChiStoredLine,
    ) -> (Vec<ChiDirectorySnoop>, Option<ChiDirectoryGrant>) {
        let mut snoops = Vec::new();
        let data_source = match state.unique_owner {
            Some((owner, _)) if owner == requester => ChiDirectoryDataSource::NoData,
            Some((owner, _)) => {
                snoops.push(ChiDirectorySnoop::new(owner, ChiEvent::SnoopUnique));
                ChiDirectoryDataSource::OwnerCache(owner)
            }
            None => match state.first_dirty_sharer_except(Some(requester)) {
                Some(owner) => ChiDirectoryDataSource::OwnerCache(owner),
                None if state.contains_sharer(requester) => ChiDirectoryDataSource::NoData,
                None => ChiDirectoryDataSource::BackingMemory,
            },
        };

        snoops.extend(
            state
                .sharers
                .keys()
                .copied()
                .filter(|sharer| *sharer != requester)
                .map(|sharer| ChiDirectorySnoop::new(sharer, ChiEvent::SnoopUnique)),
        );
        state.unique_owner = Some((requester, ChiState::UniqueDirty));
        state.sharers.clear();

        (
            snoops,
            Some(ChiDirectoryGrant::new(
                request,
                line,
                ChiState::UniqueDirty,
                data_source,
            )),
        )
    }

    fn accept_upgrade(
        &self,
        line: ChiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut ChiStoredLine,
    ) -> Result<(Vec<ChiDirectorySnoop>, Option<ChiDirectoryGrant>), ChiDirectoryError> {
        if state
            .unique_owner
            .is_some_and(|(owner, _)| owner == requester)
        {
            state.unique_owner = Some((requester, ChiState::UniqueDirty));
            return Ok((
                Vec::new(),
                Some(ChiDirectoryGrant::new(
                    request,
                    line,
                    ChiState::UniqueDirty,
                    ChiDirectoryDataSource::NoData,
                )),
            ));
        }

        if !state.contains_sharer(requester) {
            return Err(ChiDirectoryError::UpgradeRequesterNotSharer { line, requester });
        }

        let data_source = match state.first_dirty_sharer_except(Some(requester)) {
            Some(owner) => ChiDirectoryDataSource::OwnerCache(owner),
            None => ChiDirectoryDataSource::NoData,
        };
        let snoops = state
            .sharers
            .keys()
            .copied()
            .filter(|sharer| *sharer != requester)
            .map(|sharer| ChiDirectorySnoop::new(sharer, ChiEvent::SnoopUnique))
            .collect();
        state.unique_owner = Some((requester, ChiState::UniqueDirty));
        state.sharers.clear();

        Ok((
            snoops,
            Some(ChiDirectoryGrant::new(
                request,
                line,
                ChiState::UniqueDirty,
                data_source,
            )),
        ))
    }

    fn accept_dirty_writeback(
        &self,
        line: ChiLineId,
        requester: AgentId,
        state: &mut ChiStoredLine,
    ) -> Result<(Vec<ChiDirectorySnoop>, Option<ChiDirectoryGrant>), ChiDirectoryError> {
        if state
            .unique_owner
            .is_some_and(|(owner, _)| owner == requester)
        {
            state.unique_owner = None;
            return Ok((Vec::new(), None));
        }

        if state
            .sharers
            .get(&requester)
            .is_some_and(|sharer_state| sharer_state.is_dirty())
        {
            state.sharers.remove(&requester);
            return Ok((Vec::new(), None));
        }

        Err(ChiDirectoryError::WritebackFromNonOwner {
            line,
            requester,
            owner: state.dirty_holder(),
        })
    }

    fn accept_write_clean(
        &self,
        line: ChiLineId,
        requester: AgentId,
        state: &mut ChiStoredLine,
    ) -> Result<(Vec<ChiDirectorySnoop>, Option<ChiDirectoryGrant>), ChiDirectoryError> {
        if state
            .unique_owner
            .is_some_and(|(owner, _)| owner == requester)
        {
            state.unique_owner = None;
            state.sharers.insert(requester, ChiState::SharedClean);
            return Ok((Vec::new(), None));
        }

        if let Some(sharer_state) = state.sharers.get_mut(&requester) {
            *sharer_state = ChiState::SharedClean;
            return Ok((Vec::new(), None));
        }

        Err(ChiDirectoryError::EvictFromNonHolder { line, requester })
    }

    fn accept_clean_departure(
        &self,
        line: ChiLineId,
        requester: AgentId,
        state: &mut ChiStoredLine,
    ) -> Result<(Vec<ChiDirectorySnoop>, Option<ChiDirectoryGrant>), ChiDirectoryError> {
        if state
            .unique_owner
            .is_some_and(|(owner, _)| owner == requester)
        {
            state.unique_owner = None;
            return Ok((Vec::new(), None));
        }

        if state.sharers.remove(&requester).is_some() {
            return Ok((Vec::new(), None));
        }

        Err(ChiDirectoryError::EvictFromNonHolder { line, requester })
    }
}
