use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::indexing::CacheIndexingLocation;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CachePartitionId(u64);

impl CachePartitionId {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CachePartitionCandidate {
    location: CacheIndexingLocation,
    owner: Option<CachePartitionId>,
}

impl CachePartitionCandidate {
    pub const fn new(location: CacheIndexingLocation, owner: Option<CachePartitionId>) -> Self {
        Self { location, owner }
    }

    pub const fn location(self) -> CacheIndexingLocation {
        self.location
    }

    pub const fn set(self) -> usize {
        self.location.set()
    }

    pub const fn way(self) -> usize {
        self.location.way()
    }

    pub const fn owner(self) -> Option<CachePartitionId> {
        self.owner
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WayPartitionAllocation {
    partition: CachePartitionId,
    ways: Vec<usize>,
}

impl WayPartitionAllocation {
    pub fn new<I>(partition: CachePartitionId, ways: I) -> Self
    where
        I: IntoIterator<Item = usize>,
    {
        Self {
            partition,
            ways: ways.into_iter().collect(),
        }
    }

    pub const fn partition(&self) -> CachePartitionId {
        self.partition
    }

    pub fn ways(&self) -> &[usize] {
        &self.ways
    }
}

#[derive(Clone, Debug)]
pub enum CachePartitioningError {
    ZeroWays,
    ZeroTotalBlocks,
    WayOutOfRange {
        partition: CachePartitionId,
        way: usize,
        ways: usize,
    },
    InvalidCapacityFraction {
        partition: CachePartitionId,
        fraction: f64,
    },
    DuplicatePartition {
        partition: CachePartitionId,
    },
    CapacityExceeded {
        partition: CachePartitionId,
        current: usize,
        maximum: usize,
    },
    CapacityUnderflow {
        partition: CachePartitionId,
    },
}

impl PartialEq for CachePartitioningError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ZeroWays, Self::ZeroWays) | (Self::ZeroTotalBlocks, Self::ZeroTotalBlocks) => {
                true
            }
            (
                Self::WayOutOfRange {
                    partition,
                    way,
                    ways,
                },
                Self::WayOutOfRange {
                    partition: other_partition,
                    way: other_way,
                    ways: other_ways,
                },
            ) => partition == other_partition && way == other_way && ways == other_ways,
            (
                Self::InvalidCapacityFraction {
                    partition,
                    fraction,
                },
                Self::InvalidCapacityFraction {
                    partition: other_partition,
                    fraction: other_fraction,
                },
            ) => partition == other_partition && fraction.to_bits() == other_fraction.to_bits(),
            (
                Self::DuplicatePartition { partition },
                Self::DuplicatePartition {
                    partition: other_partition,
                },
            ) => partition == other_partition,
            (
                Self::CapacityExceeded {
                    partition,
                    current,
                    maximum,
                },
                Self::CapacityExceeded {
                    partition: other_partition,
                    current: other_current,
                    maximum: other_maximum,
                },
            ) => {
                partition == other_partition && current == other_current && maximum == other_maximum
            }
            (
                Self::CapacityUnderflow { partition },
                Self::CapacityUnderflow {
                    partition: other_partition,
                },
            ) => partition == other_partition,
            _ => false,
        }
    }
}

impl Eq for CachePartitioningError {}

impl fmt::Display for CachePartitioningError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroWays => write!(formatter, "cache partitioning policy has zero ways"),
            Self::ZeroTotalBlocks => {
                write!(formatter, "cache partitioning policy has zero total blocks")
            }
            Self::WayOutOfRange {
                partition,
                way,
                ways,
            } => write!(
                formatter,
                "cache partition {:?} allocates way {way} outside {ways} ways",
                partition
            ),
            Self::InvalidCapacityFraction {
                partition,
                fraction,
            } => write!(
                formatter,
                "cache partition {:?} has invalid capacity fraction {fraction}",
                partition
            ),
            Self::DuplicatePartition { partition } => write!(
                formatter,
                "cache partition {:?} is configured more than once",
                partition
            ),
            Self::CapacityExceeded {
                partition,
                current,
                maximum,
            } => write!(
                formatter,
                "cache partition {:?} capacity {current} exceeds maximum {maximum}",
                partition
            ),
            Self::CapacityUnderflow { partition } => write!(
                formatter,
                "cache partition {:?} capacity release underflows",
                partition
            ),
        }
    }
}

impl Error for CachePartitioningError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WayPartitioningPolicy {
    ways: usize,
    partition_ways: BTreeMap<CachePartitionId, Vec<usize>>,
}

impl WayPartitioningPolicy {
    pub fn new(
        ways: usize,
        allocations: &[WayPartitionAllocation],
    ) -> Result<Self, CachePartitioningError> {
        if ways == 0 {
            return Err(CachePartitioningError::ZeroWays);
        }

        let mut partition_sets: BTreeMap<CachePartitionId, BTreeSet<usize>> = BTreeMap::new();
        for allocation in allocations {
            for &way in allocation.ways() {
                if way >= ways {
                    return Err(CachePartitioningError::WayOutOfRange {
                        partition: allocation.partition(),
                        way,
                        ways,
                    });
                }
                partition_sets
                    .entry(allocation.partition())
                    .or_default()
                    .insert(way);
            }
        }

        let partition_ways = partition_sets
            .into_iter()
            .map(|(partition, ways)| (partition, ways.into_iter().collect()))
            .collect();

        Ok(Self {
            ways,
            partition_ways,
        })
    }

    pub const fn ways(&self) -> usize {
        self.ways
    }

    pub fn ways_for(&self, partition: CachePartitionId) -> Option<&[usize]> {
        self.partition_ways.get(&partition).map(Vec::as_slice)
    }

    pub fn filter_candidates(
        &self,
        partition: CachePartitionId,
        candidates: &[CachePartitionCandidate],
    ) -> Vec<CachePartitionCandidate> {
        let Some(ways) = self.ways_for(partition) else {
            return candidates.to_vec();
        };

        candidates
            .iter()
            .copied()
            .filter(|candidate| ways.binary_search(&candidate.way()).is_ok())
            .collect()
    }

    pub fn notify_acquire(
        &mut self,
        _partition: CachePartitionId,
    ) -> Result<(), CachePartitioningError> {
        Ok(())
    }

    pub fn notify_release(
        &mut self,
        _partition: CachePartitionId,
    ) -> Result<(), CachePartitioningError> {
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MaxCapacityPartitioningPolicy {
    total_blocks: usize,
    max_capacity: BTreeMap<CachePartitionId, usize>,
    current_capacity: BTreeMap<CachePartitionId, usize>,
}

impl MaxCapacityPartitioningPolicy {
    pub fn new(
        total_blocks: usize,
        capacities: &[(CachePartitionId, f64)],
    ) -> Result<Self, CachePartitioningError> {
        if total_blocks == 0 {
            return Err(CachePartitioningError::ZeroTotalBlocks);
        }

        let mut max_capacity = BTreeMap::new();
        for &(partition, fraction) in capacities {
            if !(0.0..=1.0).contains(&fraction) {
                return Err(CachePartitioningError::InvalidCapacityFraction {
                    partition,
                    fraction,
                });
            }
            let maximum = (fraction * total_blocks as f64) as usize;
            if max_capacity.insert(partition, maximum).is_some() {
                return Err(CachePartitioningError::DuplicatePartition { partition });
            }
        }

        Ok(Self {
            total_blocks,
            max_capacity,
            current_capacity: BTreeMap::new(),
        })
    }

    pub const fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    pub fn max_capacity(&self, partition: CachePartitionId) -> Option<usize> {
        self.max_capacity.get(&partition).copied()
    }

    pub fn current_capacity(&self, partition: CachePartitionId) -> Option<usize> {
        self.max_capacity
            .contains_key(&partition)
            .then_some(self.current_capacity.get(&partition).copied().unwrap_or(0))
    }

    pub fn filter_candidates(
        &self,
        partition: CachePartitionId,
        candidates: &[CachePartitionCandidate],
    ) -> Vec<CachePartitionCandidate> {
        let Some(maximum) = self.max_capacity(partition) else {
            return candidates.to_vec();
        };
        let Some(current) = self.current_capacity.get(&partition).copied() else {
            return candidates.to_vec();
        };
        if current < maximum {
            return candidates.to_vec();
        }

        candidates
            .iter()
            .copied()
            .filter(|candidate| candidate.owner() == Some(partition))
            .collect()
    }

    pub fn notify_acquire(
        &mut self,
        partition: CachePartitionId,
    ) -> Result<(), CachePartitioningError> {
        let Some(maximum) = self.max_capacity(partition) else {
            return Ok(());
        };
        let current = self.current_capacity(partition).unwrap_or(0);
        if current > maximum {
            return Err(CachePartitioningError::CapacityExceeded {
                partition,
                current,
                maximum,
            });
        }
        self.current_capacity.insert(partition, current + 1);
        Ok(())
    }

    pub fn notify_release(
        &mut self,
        partition: CachePartitionId,
    ) -> Result<(), CachePartitioningError> {
        if !self.max_capacity.contains_key(&partition) {
            return Ok(());
        }
        let current = self.current_capacity(partition).unwrap_or(0);
        if current == 0 {
            return Err(CachePartitioningError::CapacityUnderflow { partition });
        }
        if current == 1 {
            self.current_capacity.remove(&partition);
        } else {
            self.current_capacity.insert(partition, current - 1);
        }
        Ok(())
    }

    pub fn rebuild_current_capacity<I>(&mut self, owners: I)
    where
        I: IntoIterator<Item = CachePartitionId>,
    {
        self.current_capacity.clear();
        for owner in owners {
            if self.max_capacity.contains_key(&owner) {
                *self.current_capacity.entry(owner).or_insert(0) += 1;
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CachePartitionPolicy {
    Way(WayPartitioningPolicy),
    MaxCapacity(MaxCapacityPartitioningPolicy),
}

impl CachePartitionPolicy {
    pub fn filter_candidates(
        &self,
        partition: CachePartitionId,
        candidates: &[CachePartitionCandidate],
    ) -> Vec<CachePartitionCandidate> {
        match self {
            Self::Way(policy) => policy.filter_candidates(partition, candidates),
            Self::MaxCapacity(policy) => policy.filter_candidates(partition, candidates),
        }
    }

    pub fn notify_acquire(
        &mut self,
        partition: CachePartitionId,
    ) -> Result<(), CachePartitioningError> {
        match self {
            Self::Way(policy) => policy.notify_acquire(partition),
            Self::MaxCapacity(policy) => policy.notify_acquire(partition),
        }
    }

    pub fn notify_release(
        &mut self,
        partition: CachePartitionId,
    ) -> Result<(), CachePartitioningError> {
        match self {
            Self::Way(policy) => policy.notify_release(partition),
            Self::MaxCapacity(policy) => policy.notify_release(partition),
        }
    }

    pub fn rebuild_current_capacity(&mut self, owners: &[CachePartitionId]) {
        match self {
            Self::Way(_) => {}
            Self::MaxCapacity(policy) => {
                policy.rebuild_current_capacity(owners.iter().copied());
            }
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CachePartitionManager {
    policies: Vec<CachePartitionPolicy>,
}

impl CachePartitionManager {
    pub fn new<I>(policies: I) -> Self
    where
        I: IntoIterator<Item = CachePartitionPolicy>,
    {
        Self {
            policies: policies.into_iter().collect(),
        }
    }

    pub fn policies(&self) -> &[CachePartitionPolicy] {
        &self.policies
    }

    pub fn filter_candidates(
        &self,
        partition: CachePartitionId,
        candidates: &[CachePartitionCandidate],
    ) -> Vec<CachePartitionCandidate> {
        let mut filtered = candidates.to_vec();
        for policy in &self.policies {
            filtered = policy.filter_candidates(partition, &filtered);
        }
        filtered
    }

    pub fn notify_acquire(
        &mut self,
        partition: CachePartitionId,
    ) -> Result<(), CachePartitioningError> {
        for policy in &mut self.policies {
            policy.notify_acquire(partition)?;
        }
        Ok(())
    }

    pub fn notify_release(
        &mut self,
        partition: CachePartitionId,
    ) -> Result<(), CachePartitioningError> {
        for policy in &mut self.policies {
            policy.notify_release(partition)?;
        }
        Ok(())
    }

    pub fn rebuild_current_capacity<I>(&mut self, owners: I)
    where
        I: IntoIterator<Item = CachePartitionId>,
    {
        let owners = owners.into_iter().collect::<Vec<_>>();
        for policy in &mut self.policies {
            policy.rebuild_current_capacity(&owners);
        }
    }
}
