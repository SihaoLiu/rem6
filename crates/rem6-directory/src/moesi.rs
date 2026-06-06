use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_memory::{AgentId, MemoryOperation, MemoryRequest, MemoryRequestId};
use rem6_protocol_moesi::{
    DirectoryLineSnapshot as MoesiDirectoryLineSnapshot, MoesiEvent, MoesiLineId, MoesiState,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoesiDirectoryDataSource {
    BackingMemory,
    OwnerCache(AgentId),
    NoData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoesiDirectoryGrant {
    request: MemoryRequestId,
    line: MoesiLineId,
    state: MoesiState,
    data_source: MoesiDirectoryDataSource,
}

impl MoesiDirectoryGrant {
    pub const fn new(
        request: MemoryRequestId,
        line: MoesiLineId,
        state: MoesiState,
        data_source: MoesiDirectoryDataSource,
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

    pub const fn line(self) -> MoesiLineId {
        self.line
    }

    pub const fn state(self) -> MoesiState {
        self.state
    }

    pub const fn data_source(self) -> MoesiDirectoryDataSource {
        self.data_source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoesiDirectorySnoop {
    target: AgentId,
    event: MoesiEvent,
}

impl MoesiDirectorySnoop {
    pub const fn new(target: AgentId, event: MoesiEvent) -> Self {
        Self { target, event }
    }

    pub const fn target(self) -> AgentId {
        self.target
    }

    pub const fn event(self) -> MoesiEvent {
        self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiDirectoryLineState {
    line: MoesiLineId,
    owner: Option<(AgentId, MoesiState)>,
    sharers: Vec<AgentId>,
}

impl MoesiDirectoryLineState {
    pub fn new(line: MoesiLineId) -> Self {
        Self {
            line,
            owner: None,
            sharers: Vec::new(),
        }
    }

    pub fn with_owner(mut self, owner: AgentId, state: MoesiState) -> Self {
        self.owner = Some((owner, state));
        if state != MoesiState::Owned {
            self.sharers.clear();
        }
        self
    }

    pub fn with_sharer(mut self, sharer: AgentId) -> Self {
        if !self.sharers.contains(&sharer) {
            self.sharers.push(sharer);
            self.sharers.sort();
        }
        self
    }

    pub const fn line(&self) -> MoesiLineId {
        self.line
    }

    pub const fn owner(&self) -> Option<(AgentId, MoesiState)> {
        self.owner
    }

    pub fn sharers(&self) -> &[AgentId] {
        &self.sharers
    }

    pub fn protocol_snapshot(&self) -> MoesiDirectoryLineSnapshot {
        let mut snapshot = MoesiDirectoryLineSnapshot::new(self.line);
        if let Some((owner, state)) = self.owner {
            snapshot = snapshot.with_cache(owner, state);
        }
        for sharer in &self.sharers {
            snapshot = snapshot.with_cache(*sharer, MoesiState::Shared);
        }

        snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiDirectoryDecision {
    line: MoesiLineId,
    request: MemoryRequestId,
    before: MoesiDirectoryLineState,
    after: MoesiDirectoryLineState,
    snoops: Vec<MoesiDirectorySnoop>,
    grant: Option<MoesiDirectoryGrant>,
}

impl MoesiDirectoryDecision {
    pub fn new(
        line: MoesiLineId,
        request: MemoryRequestId,
        before: MoesiDirectoryLineState,
        after: MoesiDirectoryLineState,
        snoops: Vec<MoesiDirectorySnoop>,
        grant: Option<MoesiDirectoryGrant>,
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

    pub const fn line(&self) -> MoesiLineId {
        self.line
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn before(&self) -> &MoesiDirectoryLineState {
        &self.before
    }

    pub const fn after(&self) -> &MoesiDirectoryLineState {
        &self.after
    }

    pub fn snoops(&self) -> &[MoesiDirectorySnoop] {
        &self.snoops
    }

    pub const fn grant(&self) -> Option<&MoesiDirectoryGrant> {
        self.grant.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoesiDirectoryError {
    UpgradeRequesterNotSharer {
        line: MoesiLineId,
        requester: AgentId,
    },
    InvalidSnapshotOwnerState {
        line: MoesiLineId,
        state: MoesiState,
    },
    WritebackFromNonOwner {
        line: MoesiLineId,
        requester: AgentId,
        owner: Option<AgentId>,
    },
    EvictFromNonHolder {
        line: MoesiLineId,
        requester: AgentId,
    },
}

impl fmt::Display for MoesiDirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpgradeRequesterNotSharer { line, requester } => write!(
                formatter,
                "agent {} cannot upgrade line {:#x} without shared ownership",
                requester.get(),
                line.address().get()
            ),
            Self::InvalidSnapshotOwnerState { line, state } => write!(
                formatter,
                "snapshot owner for line {:#x} cannot be restored in {state:?}",
                line.address().get()
            ),
            Self::WritebackFromNonOwner {
                line,
                requester,
                owner,
            } => write!(
                formatter,
                "agent {} cannot write back dirty line {:#x}; owner is {:?}",
                requester.get(),
                line.address().get(),
                owner.map(AgentId::get)
            ),
            Self::EvictFromNonHolder { line, requester } => write!(
                formatter,
                "agent {} cannot evict line {:#x} without ownership",
                requester.get(),
                line.address().get()
            ),
        }
    }
}

impl Error for MoesiDirectoryError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MoesiStoredLine {
    owner: Option<(AgentId, MoesiState)>,
    sharers: BTreeSet<AgentId>,
}

impl MoesiStoredLine {
    fn is_empty(&self) -> bool {
        self.owner.is_none() && self.sharers.is_empty()
    }

    fn from_snapshot(snapshot: &MoesiDirectoryLineState) -> Result<Self, MoesiDirectoryError> {
        if let Some((_, state)) = snapshot.owner() {
            if !matches!(
                state,
                MoesiState::Exclusive | MoesiState::Owned | MoesiState::Modified
            ) {
                return Err(MoesiDirectoryError::InvalidSnapshotOwnerState {
                    line: snapshot.line(),
                    state,
                });
            }
        }

        Ok(Self {
            owner: snapshot.owner(),
            sharers: snapshot.sharers().iter().copied().collect(),
        })
    }

    fn snapshot(&self, line: MoesiLineId) -> MoesiDirectoryLineState {
        let mut snapshot = MoesiDirectoryLineState::new(line);
        if let Some((owner, state)) = self.owner {
            snapshot = snapshot.with_owner(owner, state);
        }
        for sharer in &self.sharers {
            snapshot = snapshot.with_sharer(*sharer);
        }

        snapshot
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MoesiDirectory {
    lines: BTreeMap<MoesiLineId, MoesiStoredLine>,
}

impl MoesiDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn line_state(&self, line: MoesiLineId) -> MoesiDirectoryLineState {
        self.lines
            .get(&line)
            .map(|stored| stored.snapshot(line))
            .unwrap_or_else(|| MoesiDirectoryLineState::new(line))
    }

    pub fn restore_line_state(
        &mut self,
        snapshot: &MoesiDirectoryLineState,
    ) -> Result<(), MoesiDirectoryError> {
        let line = snapshot.line();
        let stored = MoesiStoredLine::from_snapshot(snapshot)?;
        if stored.is_empty() {
            self.lines.remove(&line);
        } else {
            self.lines.insert(line, stored);
        }
        Ok(())
    }

    pub fn accept(
        &mut self,
        request: MemoryRequest,
    ) -> Result<MoesiDirectoryDecision, MoesiDirectoryError> {
        let line = MoesiLineId::new(request.line_address());
        let request_id = request.id();
        let requester = request_id.agent();
        let before_line = self.lines.get(&line).cloned().unwrap_or_default();
        let before = before_line.snapshot(line);
        let mut after_line = before_line;

        let (snoops, grant) = match request.operation() {
            MemoryOperation::NoAccess => (Vec::new(), None),
            MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::LoadLocked
            | MemoryOperation::PrefetchRead => {
                self.accept_read_shared(line, request_id, requester, &mut after_line)
            }
            MemoryOperation::ReadUnique
            | MemoryOperation::LockedRmwRead
            | MemoryOperation::StoreConditionalUpgradeFail
            | MemoryOperation::Write
            | MemoryOperation::CacheBlockZero
            | MemoryOperation::StoreConditional
            | MemoryOperation::StoreConditionalFail
            | MemoryOperation::LockedRmwWrite
            | MemoryOperation::Atomic
            | MemoryOperation::PrefetchWrite
            | MemoryOperation::InvalidateWritable => {
                self.accept_read_unique(line, request_id, requester, &mut after_line)
            }
            MemoryOperation::StoreConditionalUpgrade | MemoryOperation::Upgrade => {
                self.accept_upgrade(line, request_id, requester, &mut after_line)?
            }
            MemoryOperation::WritebackDirty => {
                self.accept_dirty_writeback(line, requester, &mut after_line)?
            }
            MemoryOperation::WriteClean | MemoryOperation::CleanShared => {
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

        Ok(MoesiDirectoryDecision::new(
            line, request_id, before, after, snoops, grant,
        ))
    }

    fn accept_read_shared(
        &self,
        line: MoesiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut MoesiStoredLine,
    ) -> (Vec<MoesiDirectorySnoop>, Option<MoesiDirectoryGrant>) {
        if let Some((owner, owner_state)) = state.owner {
            if owner == requester {
                return (
                    Vec::new(),
                    Some(MoesiDirectoryGrant::new(
                        request,
                        line,
                        owner_state,
                        MoesiDirectoryDataSource::NoData,
                    )),
                );
            }

            let owner_after = match owner_state {
                MoesiState::Modified | MoesiState::Owned => Some((owner, MoesiState::Owned)),
                MoesiState::Exclusive => {
                    state.sharers.insert(owner);
                    None
                }
                _ => None,
            };
            state.owner = owner_after;
            state.sharers.insert(requester);
            return (
                vec![MoesiDirectorySnoop::new(owner, MoesiEvent::SnoopRead)],
                Some(MoesiDirectoryGrant::new(
                    request,
                    line,
                    MoesiState::Shared,
                    MoesiDirectoryDataSource::OwnerCache(owner),
                )),
            );
        }

        if state.sharers.is_empty() {
            state.owner = Some((requester, MoesiState::Exclusive));
            return (
                Vec::new(),
                Some(MoesiDirectoryGrant::new(
                    request,
                    line,
                    MoesiState::Exclusive,
                    MoesiDirectoryDataSource::BackingMemory,
                )),
            );
        }

        let requester_already_had_copy = state.sharers.contains(&requester);
        state.sharers.insert(requester);
        let data_source = if requester_already_had_copy {
            MoesiDirectoryDataSource::NoData
        } else {
            MoesiDirectoryDataSource::BackingMemory
        };
        (
            Vec::new(),
            Some(MoesiDirectoryGrant::new(
                request,
                line,
                MoesiState::Shared,
                data_source,
            )),
        )
    }

    fn accept_read_unique(
        &self,
        line: MoesiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut MoesiStoredLine,
    ) -> (Vec<MoesiDirectorySnoop>, Option<MoesiDirectoryGrant>) {
        let mut snoops = Vec::new();
        let data_source = match state.owner {
            Some((owner, _)) if owner == requester => MoesiDirectoryDataSource::NoData,
            Some((owner, _)) => {
                snoops.push(MoesiDirectorySnoop::new(owner, MoesiEvent::SnoopWrite));
                MoesiDirectoryDataSource::OwnerCache(owner)
            }
            None if state.sharers.contains(&requester) => MoesiDirectoryDataSource::NoData,
            None => MoesiDirectoryDataSource::BackingMemory,
        };

        snoops.extend(
            state
                .sharers
                .iter()
                .copied()
                .filter(|sharer| *sharer != requester)
                .map(|sharer| MoesiDirectorySnoop::new(sharer, MoesiEvent::SnoopWrite)),
        );
        state.owner = Some((requester, MoesiState::Modified));
        state.sharers.clear();

        (
            snoops,
            Some(MoesiDirectoryGrant::new(
                request,
                line,
                MoesiState::Modified,
                data_source,
            )),
        )
    }

    fn accept_upgrade(
        &self,
        line: MoesiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut MoesiStoredLine,
    ) -> Result<(Vec<MoesiDirectorySnoop>, Option<MoesiDirectoryGrant>), MoesiDirectoryError> {
        if state.owner.is_some_and(|(owner, _)| owner == requester) {
            state.owner = Some((requester, MoesiState::Modified));
            return Ok((
                Vec::new(),
                Some(MoesiDirectoryGrant::new(
                    request,
                    line,
                    MoesiState::Modified,
                    MoesiDirectoryDataSource::NoData,
                )),
            ));
        }

        if !state.sharers.contains(&requester) {
            return Err(MoesiDirectoryError::UpgradeRequesterNotSharer { line, requester });
        }

        let mut snoops = Vec::new();
        let data_source = if let Some((owner, _)) = state.owner {
            snoops.push(MoesiDirectorySnoop::new(owner, MoesiEvent::SnoopWrite));
            MoesiDirectoryDataSource::OwnerCache(owner)
        } else {
            MoesiDirectoryDataSource::NoData
        };
        snoops.extend(
            state
                .sharers
                .iter()
                .copied()
                .filter(|sharer| *sharer != requester)
                .map(|sharer| MoesiDirectorySnoop::new(sharer, MoesiEvent::SnoopWrite)),
        );
        state.owner = Some((requester, MoesiState::Modified));
        state.sharers.clear();

        Ok((
            snoops,
            Some(MoesiDirectoryGrant::new(
                request,
                line,
                MoesiState::Modified,
                data_source,
            )),
        ))
    }

    fn accept_dirty_writeback(
        &self,
        line: MoesiLineId,
        requester: AgentId,
        state: &mut MoesiStoredLine,
    ) -> Result<(Vec<MoesiDirectorySnoop>, Option<MoesiDirectoryGrant>), MoesiDirectoryError> {
        if state.owner.is_none_or(|(owner, _)| owner != requester) {
            return Err(MoesiDirectoryError::WritebackFromNonOwner {
                line,
                requester,
                owner: state.owner.map(|(owner, _)| owner),
            });
        }

        state.owner = None;
        Ok((Vec::new(), None))
    }

    fn accept_write_clean(
        &self,
        line: MoesiLineId,
        requester: AgentId,
        state: &mut MoesiStoredLine,
    ) -> Result<(Vec<MoesiDirectorySnoop>, Option<MoesiDirectoryGrant>), MoesiDirectoryError> {
        if state.owner.is_some_and(|(owner, _)| owner == requester) {
            state.owner = None;
            state.sharers.insert(requester);
            return Ok((Vec::new(), None));
        }

        if state.sharers.contains(&requester) {
            return Ok((Vec::new(), None));
        }

        Err(MoesiDirectoryError::EvictFromNonHolder { line, requester })
    }

    fn accept_clean_departure(
        &self,
        line: MoesiLineId,
        requester: AgentId,
        state: &mut MoesiStoredLine,
    ) -> Result<(Vec<MoesiDirectorySnoop>, Option<MoesiDirectoryGrant>), MoesiDirectoryError> {
        if state.owner.is_some_and(|(owner, _)| owner == requester) {
            state.owner = None;
            return Ok((Vec::new(), None));
        }

        if state.sharers.remove(&requester) {
            return Ok((Vec::new(), None));
        }

        Err(MoesiDirectoryError::EvictFromNonHolder { line, requester })
    }
}
