use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_memory::{AgentId, MemoryOperation, MemoryRequest, MemoryRequestId};
use rem6_protocol_mesi::{
    DirectoryLineSnapshot as MesiDirectoryLineSnapshot, MesiEvent, MesiLineId, MesiState,
};
use rem6_protocol_msi::{DirectoryLineSnapshot, MsiEvent, MsiLineId, MsiState};

mod chi;
mod moesi;

pub use chi::{
    ChiDirectory, ChiDirectoryDataSource, ChiDirectoryDecision, ChiDirectoryError,
    ChiDirectoryGrant, ChiDirectoryLineState, ChiDirectorySnoop, ChiEvictHazard,
    ChiEvictHazardRestore,
};
pub use moesi::{
    MoesiDirectory, MoesiDirectoryDataSource, MoesiDirectoryDecision, MoesiDirectoryError,
    MoesiDirectoryGrant, MoesiDirectoryLineState, MoesiDirectorySnoop,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectoryDataSource {
    BackingMemory,
    ModifiedOwner(AgentId),
    NoData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectoryGrant {
    request: MemoryRequestId,
    line: MsiLineId,
    state: MsiState,
    data_source: DirectoryDataSource,
}

impl DirectoryGrant {
    pub const fn new(
        request: MemoryRequestId,
        line: MsiLineId,
        state: MsiState,
        data_source: DirectoryDataSource,
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

    pub const fn line(self) -> MsiLineId {
        self.line
    }

    pub const fn state(self) -> MsiState {
        self.state
    }

    pub const fn data_source(self) -> DirectoryDataSource {
        self.data_source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectorySnoop {
    target: AgentId,
    event: MsiEvent,
}

impl DirectorySnoop {
    pub const fn new(target: AgentId, event: MsiEvent) -> Self {
        Self { target, event }
    }

    pub const fn target(self) -> AgentId {
        self.target
    }

    pub const fn event(self) -> MsiEvent {
        self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryLineState {
    line: MsiLineId,
    owner: Option<AgentId>,
    sharers: Vec<AgentId>,
}

impl DirectoryLineState {
    pub fn new(line: MsiLineId) -> Self {
        Self {
            line,
            owner: None,
            sharers: Vec::new(),
        }
    }

    pub fn with_owner(mut self, owner: AgentId) -> Self {
        self.owner = Some(owner);
        self.sharers.clear();
        self
    }

    pub fn with_sharer(mut self, sharer: AgentId) -> Self {
        if !self.sharers.contains(&sharer) {
            self.sharers.push(sharer);
            self.sharers.sort();
        }
        self
    }

    pub const fn line(&self) -> MsiLineId {
        self.line
    }

    pub const fn owner(&self) -> Option<AgentId> {
        self.owner
    }

    pub fn sharers(&self) -> &[AgentId] {
        &self.sharers
    }

    pub fn protocol_snapshot(&self) -> DirectoryLineSnapshot {
        let mut snapshot = DirectoryLineSnapshot::new(self.line);
        if let Some(owner) = self.owner {
            snapshot = snapshot.with_cache(owner, MsiState::Modified);
        }
        for sharer in &self.sharers {
            snapshot = snapshot.with_cache(*sharer, MsiState::Shared);
        }

        snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryDecision {
    line: MsiLineId,
    request: MemoryRequestId,
    before: DirectoryLineState,
    after: DirectoryLineState,
    snoops: Vec<DirectorySnoop>,
    grant: Option<DirectoryGrant>,
}

impl DirectoryDecision {
    pub fn new(
        line: MsiLineId,
        request: MemoryRequestId,
        before: DirectoryLineState,
        after: DirectoryLineState,
        snoops: Vec<DirectorySnoop>,
        grant: Option<DirectoryGrant>,
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

    pub const fn line(&self) -> MsiLineId {
        self.line
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn before(&self) -> &DirectoryLineState {
        &self.before
    }

    pub const fn after(&self) -> &DirectoryLineState {
        &self.after
    }

    pub fn snoops(&self) -> &[DirectorySnoop] {
        &self.snoops
    }

    pub const fn grant(&self) -> Option<&DirectoryGrant> {
        self.grant.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DirectoryError {
    UpgradeRequesterNotSharer {
        line: MsiLineId,
        requester: AgentId,
    },
    WritebackFromNonOwner {
        line: MsiLineId,
        requester: AgentId,
        owner: Option<AgentId>,
    },
    EvictFromNonHolder {
        line: MsiLineId,
        requester: AgentId,
    },
}

impl fmt::Display for DirectoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpgradeRequesterNotSharer { line, requester } => write!(
                formatter,
                "agent {} cannot upgrade line {:#x} without shared ownership",
                requester.get(),
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

impl Error for DirectoryError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct DirectoryLine {
    owner: Option<AgentId>,
    sharers: BTreeSet<AgentId>,
}

impl DirectoryLine {
    fn is_empty(&self) -> bool {
        self.owner.is_none() && self.sharers.is_empty()
    }

    fn from_snapshot(snapshot: &DirectoryLineState) -> Self {
        Self {
            owner: snapshot.owner(),
            sharers: snapshot.sharers().iter().copied().collect(),
        }
    }

    fn snapshot(&self, line: MsiLineId) -> DirectoryLineState {
        let mut snapshot = DirectoryLineState::new(line);
        if let Some(owner) = self.owner {
            snapshot = snapshot.with_owner(owner);
        }
        for sharer in &self.sharers {
            snapshot = snapshot.with_sharer(*sharer);
        }

        snapshot
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MsiDirectory {
    lines: BTreeMap<MsiLineId, DirectoryLine>,
}

impl MsiDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn line_state(&self, line: MsiLineId) -> DirectoryLineState {
        self.lines
            .get(&line)
            .map(|stored| stored.snapshot(line))
            .unwrap_or_else(|| DirectoryLineState::new(line))
    }

    pub fn line_addresses(&self) -> Vec<rem6_memory::Address> {
        self.lines.keys().map(|line| line.address()).collect()
    }

    pub fn line_states(&self) -> Vec<DirectoryLineState> {
        self.lines
            .iter()
            .map(|(line, stored)| stored.snapshot(*line))
            .collect()
    }

    pub fn restore_line_state(&mut self, snapshot: &DirectoryLineState) {
        let line = snapshot.line();
        let stored = DirectoryLine::from_snapshot(snapshot);
        if stored.is_empty() {
            self.lines.remove(&line);
        } else {
            self.lines.insert(line, stored);
        }
    }

    pub fn restore_line_states(&mut self, snapshots: &[DirectoryLineState]) {
        self.lines.clear();
        for snapshot in snapshots {
            self.restore_line_state(snapshot);
        }
    }

    pub fn accept(&mut self, request: MemoryRequest) -> Result<DirectoryDecision, DirectoryError> {
        let line = MsiLineId::new(request.line_address());
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

        Ok(DirectoryDecision::new(
            line, request_id, before, after, snoops, grant,
        ))
    }

    fn accept_read_shared(
        &self,
        line: MsiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut DirectoryLine,
    ) -> (Vec<DirectorySnoop>, Option<DirectoryGrant>) {
        if state.owner == Some(requester) {
            return (
                Vec::new(),
                Some(DirectoryGrant::new(
                    request,
                    line,
                    MsiState::Modified,
                    DirectoryDataSource::NoData,
                )),
            );
        }

        if let Some(owner) = state.owner.take() {
            state.sharers.insert(owner);
            state.sharers.insert(requester);
            return (
                vec![DirectorySnoop::new(owner, MsiEvent::SnoopRead)],
                Some(DirectoryGrant::new(
                    request,
                    line,
                    MsiState::Shared,
                    DirectoryDataSource::ModifiedOwner(owner),
                )),
            );
        }

        state.sharers.insert(requester);
        (
            Vec::new(),
            Some(DirectoryGrant::new(
                request,
                line,
                MsiState::Shared,
                DirectoryDataSource::BackingMemory,
            )),
        )
    }

    fn accept_read_unique(
        &self,
        line: MsiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut DirectoryLine,
    ) -> (Vec<DirectorySnoop>, Option<DirectoryGrant>) {
        let mut snoops = Vec::new();
        let data_source = match state.owner {
            Some(owner) if owner == requester => DirectoryDataSource::NoData,
            Some(owner) => {
                snoops.push(DirectorySnoop::new(owner, MsiEvent::SnoopWrite));
                DirectoryDataSource::ModifiedOwner(owner)
            }
            None if state.sharers.contains(&requester) => DirectoryDataSource::NoData,
            None => DirectoryDataSource::BackingMemory,
        };

        snoops.extend(
            state
                .sharers
                .iter()
                .copied()
                .filter(|sharer| *sharer != requester)
                .map(|sharer| DirectorySnoop::new(sharer, MsiEvent::SnoopWrite)),
        );
        state.owner = Some(requester);
        state.sharers.clear();

        (
            snoops,
            Some(DirectoryGrant::new(
                request,
                line,
                MsiState::Modified,
                data_source,
            )),
        )
    }

    fn accept_upgrade(
        &self,
        line: MsiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut DirectoryLine,
    ) -> Result<(Vec<DirectorySnoop>, Option<DirectoryGrant>), DirectoryError> {
        if state.owner == Some(requester) {
            return Ok((
                Vec::new(),
                Some(DirectoryGrant::new(
                    request,
                    line,
                    MsiState::Modified,
                    DirectoryDataSource::NoData,
                )),
            ));
        }

        if !state.sharers.contains(&requester) {
            return Err(DirectoryError::UpgradeRequesterNotSharer { line, requester });
        }

        let snoops = state
            .sharers
            .iter()
            .copied()
            .filter(|sharer| *sharer != requester)
            .map(|sharer| DirectorySnoop::new(sharer, MsiEvent::SnoopWrite))
            .collect();
        state.owner = Some(requester);
        state.sharers.clear();

        Ok((
            snoops,
            Some(DirectoryGrant::new(
                request,
                line,
                MsiState::Modified,
                DirectoryDataSource::NoData,
            )),
        ))
    }

    fn accept_dirty_writeback(
        &self,
        line: MsiLineId,
        requester: AgentId,
        state: &mut DirectoryLine,
    ) -> Result<(Vec<DirectorySnoop>, Option<DirectoryGrant>), DirectoryError> {
        if state.owner != Some(requester) {
            return Err(DirectoryError::WritebackFromNonOwner {
                line,
                requester,
                owner: state.owner,
            });
        }

        state.owner = None;
        Ok((Vec::new(), None))
    }

    fn accept_write_clean(
        &self,
        line: MsiLineId,
        requester: AgentId,
        state: &mut DirectoryLine,
    ) -> Result<(Vec<DirectorySnoop>, Option<DirectoryGrant>), DirectoryError> {
        if state.owner == Some(requester) {
            state.owner = None;
            state.sharers.insert(requester);
            return Ok((Vec::new(), None));
        }

        if state.sharers.contains(&requester) {
            return Ok((Vec::new(), None));
        }

        Err(DirectoryError::EvictFromNonHolder { line, requester })
    }

    fn accept_clean_departure(
        &self,
        line: MsiLineId,
        requester: AgentId,
        state: &mut DirectoryLine,
    ) -> Result<(Vec<DirectorySnoop>, Option<DirectoryGrant>), DirectoryError> {
        if state.owner == Some(requester) {
            state.owner = None;
            return Ok((Vec::new(), None));
        }

        if state.sharers.remove(&requester) {
            return Ok((Vec::new(), None));
        }

        Err(DirectoryError::EvictFromNonHolder { line, requester })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MesiDirectoryDataSource {
    BackingMemory,
    OwnedCache(AgentId),
    NoData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MesiDirectoryGrant {
    request: MemoryRequestId,
    line: MesiLineId,
    state: MesiState,
    data_source: MesiDirectoryDataSource,
}

impl MesiDirectoryGrant {
    pub const fn new(
        request: MemoryRequestId,
        line: MesiLineId,
        state: MesiState,
        data_source: MesiDirectoryDataSource,
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

    pub const fn line(self) -> MesiLineId {
        self.line
    }

    pub const fn state(self) -> MesiState {
        self.state
    }

    pub const fn data_source(self) -> MesiDirectoryDataSource {
        self.data_source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MesiDirectorySnoop {
    target: AgentId,
    event: MesiEvent,
}

impl MesiDirectorySnoop {
    pub const fn new(target: AgentId, event: MesiEvent) -> Self {
        Self { target, event }
    }

    pub const fn target(self) -> AgentId {
        self.target
    }

    pub const fn event(self) -> MesiEvent {
        self.event
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiDirectoryLineState {
    line: MesiLineId,
    owner: Option<(AgentId, MesiState)>,
    sharers: Vec<AgentId>,
}

impl MesiDirectoryLineState {
    pub fn new(line: MesiLineId) -> Self {
        Self {
            line,
            owner: None,
            sharers: Vec::new(),
        }
    }

    pub fn with_owner(mut self, owner: AgentId, state: MesiState) -> Self {
        self.owner = Some((owner, state));
        self.sharers.clear();
        self
    }

    pub fn with_sharer(mut self, sharer: AgentId) -> Self {
        if !self.sharers.contains(&sharer) {
            self.sharers.push(sharer);
            self.sharers.sort();
        }
        self
    }

    pub const fn line(&self) -> MesiLineId {
        self.line
    }

    pub const fn owner(&self) -> Option<(AgentId, MesiState)> {
        self.owner
    }

    pub fn sharers(&self) -> &[AgentId] {
        &self.sharers
    }

    pub fn protocol_snapshot(&self) -> MesiDirectoryLineSnapshot {
        let mut snapshot = MesiDirectoryLineSnapshot::new(self.line);
        if let Some((owner, state)) = self.owner {
            snapshot = snapshot.with_cache(owner, state);
        }
        for sharer in &self.sharers {
            snapshot = snapshot.with_cache(*sharer, MesiState::Shared);
        }

        snapshot
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiDirectoryDecision {
    line: MesiLineId,
    request: MemoryRequestId,
    before: MesiDirectoryLineState,
    after: MesiDirectoryLineState,
    snoops: Vec<MesiDirectorySnoop>,
    grant: Option<MesiDirectoryGrant>,
}

impl MesiDirectoryDecision {
    pub fn new(
        line: MesiLineId,
        request: MemoryRequestId,
        before: MesiDirectoryLineState,
        after: MesiDirectoryLineState,
        snoops: Vec<MesiDirectorySnoop>,
        grant: Option<MesiDirectoryGrant>,
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

    pub const fn line(&self) -> MesiLineId {
        self.line
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn before(&self) -> &MesiDirectoryLineState {
        &self.before
    }

    pub const fn after(&self) -> &MesiDirectoryLineState {
        &self.after
    }

    pub fn snoops(&self) -> &[MesiDirectorySnoop] {
        &self.snoops
    }

    pub const fn grant(&self) -> Option<&MesiDirectoryGrant> {
        self.grant.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MesiDirectoryError {
    UpgradeRequesterNotSharer {
        line: MesiLineId,
        requester: AgentId,
    },
    InvalidSnapshotOwnerState {
        line: MesiLineId,
        state: MesiState,
    },
    WritebackFromNonOwner {
        line: MesiLineId,
        requester: AgentId,
        owner: Option<AgentId>,
    },
    EvictFromNonHolder {
        line: MesiLineId,
        requester: AgentId,
    },
}

impl fmt::Display for MesiDirectoryError {
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

impl Error for MesiDirectoryError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MesiStoredLine {
    owner: Option<(AgentId, MesiState)>,
    sharers: BTreeSet<AgentId>,
}

impl MesiStoredLine {
    fn is_empty(&self) -> bool {
        self.owner.is_none() && self.sharers.is_empty()
    }

    fn from_snapshot(snapshot: &MesiDirectoryLineState) -> Result<Self, MesiDirectoryError> {
        if let Some((_, state)) = snapshot.owner() {
            if !state.is_owned() {
                return Err(MesiDirectoryError::InvalidSnapshotOwnerState {
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

    fn snapshot(&self, line: MesiLineId) -> MesiDirectoryLineState {
        let mut snapshot = MesiDirectoryLineState::new(line);
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
pub struct MesiDirectory {
    lines: BTreeMap<MesiLineId, MesiStoredLine>,
}

impl MesiDirectory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn line_state(&self, line: MesiLineId) -> MesiDirectoryLineState {
        self.lines
            .get(&line)
            .map(|stored| stored.snapshot(line))
            .unwrap_or_else(|| MesiDirectoryLineState::new(line))
    }

    pub fn restore_line_state(
        &mut self,
        snapshot: &MesiDirectoryLineState,
    ) -> Result<(), MesiDirectoryError> {
        let line = snapshot.line();
        let stored = MesiStoredLine::from_snapshot(snapshot)?;
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
    ) -> Result<MesiDirectoryDecision, MesiDirectoryError> {
        let line = MesiLineId::new(request.line_address());
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

        Ok(MesiDirectoryDecision::new(
            line, request_id, before, after, snoops, grant,
        ))
    }

    fn accept_read_shared(
        &self,
        line: MesiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut MesiStoredLine,
    ) -> (Vec<MesiDirectorySnoop>, Option<MesiDirectoryGrant>) {
        if let Some((owner, owner_state)) = state.owner {
            if owner == requester {
                return (
                    Vec::new(),
                    Some(MesiDirectoryGrant::new(
                        request,
                        line,
                        owner_state,
                        MesiDirectoryDataSource::NoData,
                    )),
                );
            }

            state.owner = None;
            state.sharers.insert(owner);
            state.sharers.insert(requester);
            return (
                vec![MesiDirectorySnoop::new(owner, MesiEvent::SnoopRead)],
                Some(MesiDirectoryGrant::new(
                    request,
                    line,
                    MesiState::Shared,
                    MesiDirectoryDataSource::OwnedCache(owner),
                )),
            );
        }

        if state.sharers.is_empty() {
            state.owner = Some((requester, MesiState::Exclusive));
            return (
                Vec::new(),
                Some(MesiDirectoryGrant::new(
                    request,
                    line,
                    MesiState::Exclusive,
                    MesiDirectoryDataSource::BackingMemory,
                )),
            );
        }

        state.sharers.insert(requester);
        (
            Vec::new(),
            Some(MesiDirectoryGrant::new(
                request,
                line,
                MesiState::Shared,
                MesiDirectoryDataSource::BackingMemory,
            )),
        )
    }

    fn accept_read_unique(
        &self,
        line: MesiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut MesiStoredLine,
    ) -> (Vec<MesiDirectorySnoop>, Option<MesiDirectoryGrant>) {
        let mut snoops = Vec::new();
        let data_source = match state.owner {
            Some((owner, _)) if owner == requester => MesiDirectoryDataSource::NoData,
            Some((owner, _)) => {
                snoops.push(MesiDirectorySnoop::new(owner, MesiEvent::SnoopWrite));
                MesiDirectoryDataSource::OwnedCache(owner)
            }
            None if state.sharers.contains(&requester) => MesiDirectoryDataSource::NoData,
            None => MesiDirectoryDataSource::BackingMemory,
        };

        snoops.extend(
            state
                .sharers
                .iter()
                .copied()
                .filter(|sharer| *sharer != requester)
                .map(|sharer| MesiDirectorySnoop::new(sharer, MesiEvent::SnoopWrite)),
        );
        state.owner = Some((requester, MesiState::Modified));
        state.sharers.clear();

        (
            snoops,
            Some(MesiDirectoryGrant::new(
                request,
                line,
                MesiState::Modified,
                data_source,
            )),
        )
    }

    fn accept_upgrade(
        &self,
        line: MesiLineId,
        request: MemoryRequestId,
        requester: AgentId,
        state: &mut MesiStoredLine,
    ) -> Result<(Vec<MesiDirectorySnoop>, Option<MesiDirectoryGrant>), MesiDirectoryError> {
        if state.owner.is_some_and(|(owner, _)| owner == requester) {
            state.owner = Some((requester, MesiState::Modified));
            return Ok((
                Vec::new(),
                Some(MesiDirectoryGrant::new(
                    request,
                    line,
                    MesiState::Modified,
                    MesiDirectoryDataSource::NoData,
                )),
            ));
        }

        if !state.sharers.contains(&requester) {
            return Err(MesiDirectoryError::UpgradeRequesterNotSharer { line, requester });
        }

        let snoops = state
            .sharers
            .iter()
            .copied()
            .filter(|sharer| *sharer != requester)
            .map(|sharer| MesiDirectorySnoop::new(sharer, MesiEvent::SnoopWrite))
            .collect();
        state.owner = Some((requester, MesiState::Modified));
        state.sharers.clear();

        Ok((
            snoops,
            Some(MesiDirectoryGrant::new(
                request,
                line,
                MesiState::Modified,
                MesiDirectoryDataSource::NoData,
            )),
        ))
    }

    fn accept_dirty_writeback(
        &self,
        line: MesiLineId,
        requester: AgentId,
        state: &mut MesiStoredLine,
    ) -> Result<(Vec<MesiDirectorySnoop>, Option<MesiDirectoryGrant>), MesiDirectoryError> {
        if state.owner.is_none_or(|(owner, _)| owner != requester) {
            return Err(MesiDirectoryError::WritebackFromNonOwner {
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
        line: MesiLineId,
        requester: AgentId,
        state: &mut MesiStoredLine,
    ) -> Result<(Vec<MesiDirectorySnoop>, Option<MesiDirectoryGrant>), MesiDirectoryError> {
        if state.owner.is_some_and(|(owner, _)| owner == requester) {
            state.owner = None;
            state.sharers.insert(requester);
            return Ok((Vec::new(), None));
        }

        if state.sharers.contains(&requester) {
            return Ok((Vec::new(), None));
        }

        Err(MesiDirectoryError::EvictFromNonHolder { line, requester })
    }

    fn accept_clean_departure(
        &self,
        line: MesiLineId,
        requester: AgentId,
        state: &mut MesiStoredLine,
    ) -> Result<(Vec<MesiDirectorySnoop>, Option<MesiDirectoryGrant>), MesiDirectoryError> {
        if state.owner.is_some_and(|(owner, _)| owner == requester) {
            state.owner = None;
            return Ok((Vec::new(), None));
        }

        if state.sharers.remove(&requester) {
            return Ok((Vec::new(), None));
        }

        Err(MesiDirectoryError::EvictFromNonHolder { line, requester })
    }
}
