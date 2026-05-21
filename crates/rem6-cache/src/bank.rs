use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_protocol_msi::MsiState;

use crate::{
    CacheControllerError, CacheControllerResult, MsiCacheController, MsiCacheControllerSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MsiCacheBankError {
    Controller(CacheControllerError),
    WrongAgent {
        expected: AgentId,
        actual: AgentId,
    },
    UnknownPendingFill {
        response: MemoryRequestId,
    },
    SnapshotIdentityMismatch {
        expected_agent: AgentId,
        actual_agent: AgentId,
        expected_layout: CacheLineLayout,
        actual_layout: CacheLineLayout,
    },
    DuplicateSnapshotLine {
        line: Address,
    },
    DuplicateSnapshotPendingFill {
        response: MemoryRequestId,
    },
}

impl fmt::Display for MsiCacheBankError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Controller(error) => write!(formatter, "{error}"),
            Self::WrongAgent { expected, actual } => write!(
                formatter,
                "MSI cache bank for agent {} cannot accept request from agent {}",
                expected.get(),
                actual.get()
            ),
            Self::UnknownPendingFill { response } => write!(
                formatter,
                "MSI cache bank has no pending fill for response {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
            Self::SnapshotIdentityMismatch {
                expected_agent,
                actual_agent,
                expected_layout,
                actual_layout,
            } => write!(
                formatter,
                "MSI cache bank snapshot for agent {}, line size {} cannot restore bank for agent {}, line size {}",
                actual_agent.get(),
                actual_layout.bytes(),
                expected_agent.get(),
                expected_layout.bytes()
            ),
            Self::DuplicateSnapshotLine { line } => {
                write!(formatter, "MSI cache bank snapshot repeats line {:#x}", line.get())
            }
            Self::DuplicateSnapshotPendingFill { response } => write!(
                formatter,
                "MSI cache bank snapshot repeats pending fill {} from agent {}",
                response.sequence(),
                response.agent().get()
            ),
        }
    }
}

impl Error for MsiCacheBankError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Controller(error) => Some(error),
            Self::WrongAgent { .. }
            | Self::UnknownPendingFill { .. }
            | Self::SnapshotIdentityMismatch { .. }
            | Self::DuplicateSnapshotLine { .. }
            | Self::DuplicateSnapshotPendingFill { .. } => None,
        }
    }
}

impl From<CacheControllerError> for MsiCacheBankError {
    fn from(error: CacheControllerError) -> Self {
        Self::Controller(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiCacheBankSnapshot {
    agent: AgentId,
    layout: CacheLineLayout,
    next_sequence: u64,
    lines: Vec<MsiCacheControllerSnapshot>,
}

impl MsiCacheBankSnapshot {
    pub fn new(
        agent: AgentId,
        layout: CacheLineLayout,
        next_sequence: u64,
        lines: Vec<MsiCacheControllerSnapshot>,
    ) -> Self {
        Self {
            agent,
            layout,
            next_sequence,
            lines,
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn lines(&self) -> &[MsiCacheControllerSnapshot] {
        &self.lines
    }

    pub fn line_addresses(&self) -> Vec<Address> {
        self.lines
            .iter()
            .map(|line| line.line().address())
            .collect()
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
}

#[derive(Clone, Debug)]
pub struct MsiCacheBank {
    agent: AgentId,
    layout: CacheLineLayout,
    next_sequence: u64,
    lines: BTreeMap<Address, MsiCacheController>,
    pending_fills: BTreeMap<MemoryRequestId, Address>,
}

impl MsiCacheBank {
    pub fn new(agent: AgentId, layout: CacheLineLayout) -> Self {
        Self {
            agent,
            layout,
            next_sequence: 0,
            lines: BTreeMap::new(),
            pending_fills: BTreeMap::new(),
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn pending_fill_count(&self) -> usize {
        self.pending_fills.len()
    }

    pub fn line_addresses(&self) -> Vec<Address> {
        self.lines.keys().copied().collect()
    }

    pub fn pending_fill_line(&self, response: MemoryRequestId) -> Option<Address> {
        self.pending_fills.get(&response).copied()
    }

    pub fn state(&self, address: Address) -> Option<MsiState> {
        self.lines
            .get(&self.layout.line_address(address))
            .map(MsiCacheController::state)
    }

    pub fn cached_data(&self, address: Address) -> Option<&[u8]> {
        self.lines
            .get(&self.layout.line_address(address))
            .and_then(MsiCacheController::cached_data)
    }

    pub fn snapshot(&self) -> MsiCacheBankSnapshot {
        MsiCacheBankSnapshot::new(
            self.agent,
            self.layout,
            self.next_sequence,
            self.lines
                .values()
                .map(MsiCacheController::snapshot)
                .collect(),
        )
    }

    pub fn restore(&mut self, snapshot: &MsiCacheBankSnapshot) -> Result<(), MsiCacheBankError> {
        if snapshot.agent() != self.agent || snapshot.layout() != self.layout {
            return Err(MsiCacheBankError::SnapshotIdentityMismatch {
                expected_agent: self.agent,
                actual_agent: snapshot.agent(),
                expected_layout: self.layout,
                actual_layout: snapshot.layout(),
            });
        }

        let mut lines = BTreeMap::new();
        let mut pending_fills = BTreeMap::new();
        for line_snapshot in snapshot.lines() {
            let line = line_snapshot.line().address();
            let mut controller = MsiCacheController::new(self.agent, self.layout, line);
            controller.restore(line_snapshot)?;
            if lines.insert(line, controller).is_some() {
                return Err(MsiCacheBankError::DuplicateSnapshotLine { line });
            }
            if let Some(pending) = line_snapshot.pending() {
                let response = pending.downstream();
                if pending_fills.insert(response, line).is_some() {
                    return Err(MsiCacheBankError::DuplicateSnapshotPendingFill { response });
                }
            }
        }

        self.next_sequence = snapshot.next_sequence();
        self.lines = lines;
        self.pending_fills = pending_fills;
        Ok(())
    }

    pub fn accept_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        self.validate_request_agent(&request)?;
        let line = request.line_address();
        let controller = self
            .lines
            .entry(line)
            .or_insert_with(|| MsiCacheController::new(self.agent, self.layout, line));
        controller.set_next_sequence(self.next_sequence);
        let result = controller.accept_cpu_request(request)?;
        self.next_sequence = controller.next_sequence();
        if let Some(downstream) = result.downstream_request() {
            self.pending_fills.insert(downstream.id(), line);
        }
        Ok(result)
    }

    pub fn accept_fill(
        &mut self,
        response: MemoryResponse,
    ) -> Result<CacheControllerResult, MsiCacheBankError> {
        let response_id = response.request_id();
        let line =
            *self
                .pending_fills
                .get(&response_id)
                .ok_or(MsiCacheBankError::UnknownPendingFill {
                    response: response_id,
                })?;
        let controller = self
            .lines
            .get_mut(&line)
            .expect("pending fill references an existing MSI cache line");
        let result = controller.accept_fill(response)?;
        self.pending_fills.remove(&response_id);
        Ok(result)
    }

    fn validate_request_agent(&self, request: &MemoryRequest) -> Result<(), MsiCacheBankError> {
        let actual = request.id().agent();
        if actual != self.agent {
            return Err(MsiCacheBankError::WrongAgent {
                expected: self.agent,
                actual,
            });
        }

        Ok(())
    }
}
