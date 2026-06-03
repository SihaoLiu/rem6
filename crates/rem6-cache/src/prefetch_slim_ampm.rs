use std::error::Error;
use std::fmt;

use rem6_memory::{Address, AgentId};

use crate::prefetch::PrefetchCandidate;
use crate::{
    AmpmPrefetchAccess, AmpmPrefetchCandidate, AmpmPrefetcher, AmpmPrefetcherConfig,
    AmpmPrefetcherError, AmpmPrefetcherSnapshot, DcptPrefetchAccess, DcptPrefetchCandidate,
    DcptPrefetcher, DcptPrefetcherConfig, DcptPrefetcherError, DcptPrefetcherSnapshot,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlimAmpmPrefetcherConfig {
    ampm: AmpmPrefetcherConfig,
    dcpt: DcptPrefetcherConfig,
}

impl SlimAmpmPrefetcherConfig {
    pub const fn new(ampm: AmpmPrefetcherConfig, dcpt: DcptPrefetcherConfig) -> Self {
        Self { ampm, dcpt }
    }

    pub fn gem5_defaults(line_size: u64) -> Result<Self, SlimAmpmPrefetcherError> {
        let ampm = AmpmPrefetcherConfig::new(line_size, 2048, 2, 256)?.with_limit_stride(4)?;
        let dcpt = DcptPrefetcherConfig::new(9, 12, 8, 256, false)?;
        Ok(Self::new(ampm, dcpt))
    }

    pub const fn ampm(&self) -> &AmpmPrefetcherConfig {
        &self.ampm
    }

    pub const fn dcpt(&self) -> &DcptPrefetcherConfig {
        &self.dcpt
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SlimAmpmPrefetcherError {
    Ampm(AmpmPrefetcherError),
    Dcpt(DcptPrefetcherError),
    SnapshotConfigMismatch {
        expected: Box<SlimAmpmPrefetcherConfig>,
        actual: Box<SlimAmpmPrefetcherConfig>,
    },
}

impl fmt::Display for SlimAmpmPrefetcherError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ampm(error) => write!(formatter, "Slim AMPM AMPM component failed: {error}"),
            Self::Dcpt(error) => write!(formatter, "Slim AMPM DCPT component failed: {error}"),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "Slim AMPM snapshot config {actual:?} does not match {expected:?}"
            ),
        }
    }
}

impl Error for SlimAmpmPrefetcherError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Ampm(error) => Some(error),
            Self::Dcpt(error) => Some(error),
            Self::SnapshotConfigMismatch { .. } => None,
        }
    }
}

impl From<AmpmPrefetcherError> for SlimAmpmPrefetcherError {
    fn from(error: AmpmPrefetcherError) -> Self {
        Self::Ampm(error)
    }
}

impl From<DcptPrefetcherError> for SlimAmpmPrefetcherError {
    fn from(error: DcptPrefetcherError) -> Self {
        Self::Dcpt(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlimAmpmPrefetchAccess {
    requestor: AgentId,
    pc: u64,
    address: Address,
    secure: bool,
}

impl SlimAmpmPrefetchAccess {
    pub const fn new(requestor: AgentId, pc: u64, address: Address, secure: bool) -> Self {
        Self {
            requestor,
            pc,
            address,
            secure,
        }
    }

    pub const fn requestor(&self) -> AgentId {
        self.requestor
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    fn ampm(self) -> AmpmPrefetchAccess {
        AmpmPrefetchAccess::new(self.requestor, self.pc, self.address, self.secure)
    }

    fn dcpt(self) -> DcptPrefetchAccess {
        DcptPrefetchAccess::new(self.requestor, self.pc, self.address, self.secure)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlimAmpmPrefetchSource {
    Dcpt,
    Ampm,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlimAmpmPrefetchCandidate {
    source: SlimAmpmPrefetchSource,
    address: Address,
    source_address: Address,
    context: AgentId,
    pc: u64,
    secure: bool,
    stride: i64,
    degree_index: u32,
}

impl SlimAmpmPrefetchCandidate {
    fn from_dcpt(candidate: &DcptPrefetchCandidate) -> Self {
        Self {
            source: SlimAmpmPrefetchSource::Dcpt,
            address: candidate.address(),
            source_address: candidate.source_address(),
            context: candidate.context(),
            pc: candidate.pc(),
            secure: candidate.secure(),
            stride: candidate.stride(),
            degree_index: candidate.degree_index(),
        }
    }

    fn from_ampm(candidate: &AmpmPrefetchCandidate) -> Self {
        Self {
            source: SlimAmpmPrefetchSource::Ampm,
            address: candidate.address(),
            source_address: candidate.source_address(),
            context: candidate.context(),
            pc: candidate.pc(),
            secure: candidate.secure(),
            stride: candidate.stride(),
            degree_index: candidate.degree_index(),
        }
    }

    pub const fn source(&self) -> SlimAmpmPrefetchSource {
        self.source
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn source_address(&self) -> Address {
        self.source_address
    }

    pub const fn context(&self) -> AgentId {
        self.context
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn secure(&self) -> bool {
        self.secure
    }

    pub const fn stride(&self) -> i64 {
        self.stride
    }

    pub const fn degree_index(&self) -> u32 {
        self.degree_index
    }
}

impl PrefetchCandidate for SlimAmpmPrefetchCandidate {
    fn address(&self) -> Address {
        self.address()
    }

    fn source_address(&self) -> Address {
        self.source_address()
    }

    fn context(&self) -> AgentId {
        self.context()
    }

    fn pc(&self) -> u64 {
        self.pc()
    }

    fn secure(&self) -> bool {
        self.secure()
    }

    fn stride(&self) -> i64 {
        self.stride()
    }

    fn degree_index(&self) -> u32 {
        self.degree_index()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlimAmpmPrefetcherSnapshot {
    config: SlimAmpmPrefetcherConfig,
    ampm: AmpmPrefetcherSnapshot,
    dcpt: DcptPrefetcherSnapshot,
    last_candidates: Vec<SlimAmpmPrefetchCandidate>,
}

impl SlimAmpmPrefetcherSnapshot {
    pub const fn config(&self) -> &SlimAmpmPrefetcherConfig {
        &self.config
    }

    pub const fn ampm(&self) -> &AmpmPrefetcherSnapshot {
        &self.ampm
    }

    pub const fn dcpt(&self) -> &DcptPrefetcherSnapshot {
        &self.dcpt
    }

    pub fn last_candidates(&self) -> &[SlimAmpmPrefetchCandidate] {
        &self.last_candidates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlimAmpmPrefetcher {
    config: SlimAmpmPrefetcherConfig,
    ampm: AmpmPrefetcher,
    dcpt: DcptPrefetcher,
    last_candidates: Vec<SlimAmpmPrefetchCandidate>,
}

impl SlimAmpmPrefetcher {
    pub fn new(config: SlimAmpmPrefetcherConfig) -> Self {
        Self {
            ampm: AmpmPrefetcher::new(config.ampm().clone()),
            dcpt: DcptPrefetcher::new(config.dcpt().clone()),
            config,
            last_candidates: Vec::new(),
        }
    }

    pub const fn config(&self) -> &SlimAmpmPrefetcherConfig {
        &self.config
    }

    pub const fn ampm(&self) -> &AmpmPrefetcher {
        &self.ampm
    }

    pub const fn dcpt(&self) -> &DcptPrefetcher {
        &self.dcpt
    }

    pub fn last_candidates(&self) -> &[SlimAmpmPrefetchCandidate] {
        &self.last_candidates
    }

    pub fn observe(
        &mut self,
        access: SlimAmpmPrefetchAccess,
    ) -> Result<&[SlimAmpmPrefetchCandidate], SlimAmpmPrefetcherError> {
        let dcpt_candidates = self
            .dcpt
            .observe(access.dcpt())?
            .iter()
            .map(SlimAmpmPrefetchCandidate::from_dcpt)
            .collect::<Vec<_>>();
        if !dcpt_candidates.is_empty() {
            self.last_candidates = dcpt_candidates;
            return Ok(&self.last_candidates);
        }

        self.last_candidates = self
            .ampm
            .observe(access.ampm())?
            .iter()
            .map(SlimAmpmPrefetchCandidate::from_ampm)
            .collect();
        Ok(&self.last_candidates)
    }

    pub fn snapshot(&self) -> SlimAmpmPrefetcherSnapshot {
        SlimAmpmPrefetcherSnapshot {
            config: self.config.clone(),
            ampm: self.ampm.snapshot(),
            dcpt: self.dcpt.snapshot(),
            last_candidates: self.last_candidates.clone(),
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &SlimAmpmPrefetcherSnapshot,
    ) -> Result<(), SlimAmpmPrefetcherError> {
        if snapshot.config() != &self.config {
            return Err(SlimAmpmPrefetcherError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config().clone()),
            });
        }

        self.ampm.restore(snapshot.ampm())?;
        self.dcpt.restore(snapshot.dcpt())?;
        self.last_candidates = snapshot.last_candidates().to_vec();
        Ok(())
    }
}
