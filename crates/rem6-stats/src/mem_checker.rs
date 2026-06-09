use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::Tick;

use crate::StatsError;

const INITIAL_SERIAL: u64 = 0;
const FIRST_SERIAL: u64 = 1;
const INITIAL_TICK: Tick = 0;
const FUTURE_TICK: Tick = Tick::MAX;
const INITIAL_DATA: u8 = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemCheckerTransaction {
    serial: u64,
    start_tick: Tick,
    complete_tick: Tick,
    data: u8,
}

impl MemCheckerTransaction {
    pub const fn new(serial: u64, start_tick: Tick, complete_tick: Tick, data: u8) -> Self {
        Self {
            serial,
            start_tick,
            complete_tick,
            data,
        }
    }

    pub const fn read(serial: u64, start_tick: Tick) -> Self {
        Self::new(serial, start_tick, FUTURE_TICK, INITIAL_DATA)
    }

    pub const fn observed_read(
        serial: u64,
        start_tick: Tick,
        complete_tick: Tick,
        data: u8,
    ) -> Self {
        Self::new(serial, start_tick, complete_tick, data)
    }

    pub const fn write(serial: u64, start_tick: Tick, complete_tick: Tick, data: u8) -> Self {
        Self::new(serial, start_tick, complete_tick, data)
    }

    pub const fn serial(self) -> u64 {
        self.serial
    }

    pub const fn start_tick(self) -> Tick {
        self.start_tick
    }

    pub const fn complete_tick(self) -> Tick {
        self.complete_tick
    }

    pub const fn data(self) -> u8 {
        self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemCheckerReadFailure {
    address: u64,
    observed: u8,
    expected: Vec<u8>,
}

impl MemCheckerReadFailure {
    pub fn new(address: u64, observed: u8, expected: Vec<u8>) -> Self {
        Self {
            address,
            observed,
            expected,
        }
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub const fn observed(&self) -> u8 {
        self.observed
    }

    pub fn expected(&self) -> &[u8] {
        &self.expected
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemCheckerReadResult {
    serial: u64,
    checked_bytes: usize,
    ignored_bytes: usize,
    failures: Vec<MemCheckerReadFailure>,
}

impl MemCheckerReadResult {
    pub fn valid(serial: u64, checked_bytes: usize, ignored_bytes: usize) -> Self {
        Self {
            serial,
            checked_bytes,
            ignored_bytes,
            failures: Vec::new(),
        }
    }

    pub fn invalid(
        serial: u64,
        checked_bytes: usize,
        ignored_bytes: usize,
        failures: Vec<MemCheckerReadFailure>,
    ) -> Self {
        Self {
            serial,
            checked_bytes,
            ignored_bytes,
            failures,
        }
    }

    pub const fn serial(&self) -> u64 {
        self.serial
    }

    pub const fn checked_bytes(&self) -> usize {
        self.checked_bytes
    }

    pub const fn ignored_bytes(&self) -> usize {
        self.ignored_bytes
    }

    pub fn failures(&self) -> &[MemCheckerReadFailure] {
        &self.failures
    }

    pub fn is_valid(&self) -> bool {
        self.failures.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemCheckerWriteResult {
    serial: u64,
    completed_bytes: usize,
    ignored_bytes: usize,
}

impl MemCheckerWriteResult {
    pub const fn new(serial: u64, completed_bytes: usize, ignored_bytes: usize) -> Self {
        Self {
            serial,
            completed_bytes,
            ignored_bytes,
        }
    }

    pub const fn serial(self) -> u64 {
        self.serial
    }

    pub const fn completed_bytes(self) -> usize {
        self.completed_bytes
    }

    pub const fn ignored_bytes(self) -> usize {
        self.ignored_bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemCheckerWriteClusterSnapshot {
    start_tick: Tick,
    complete_tick: Tick,
    complete_max_tick: Tick,
    incomplete_writes: usize,
    writes: Vec<MemCheckerTransaction>,
}

impl MemCheckerWriteClusterSnapshot {
    pub fn new(
        start_tick: Tick,
        complete_tick: Tick,
        complete_max_tick: Tick,
        incomplete_writes: usize,
        writes: Vec<MemCheckerTransaction>,
    ) -> Self {
        Self {
            start_tick,
            complete_tick,
            complete_max_tick,
            incomplete_writes,
            writes,
        }
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn complete_tick(&self) -> Tick {
        self.complete_tick
    }

    pub const fn complete_max_tick(&self) -> Tick {
        self.complete_max_tick
    }

    pub const fn incomplete_writes(&self) -> usize {
        self.incomplete_writes
    }

    pub fn writes(&self) -> &[MemCheckerTransaction] {
        &self.writes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemCheckerByteSnapshot {
    address: u64,
    outstanding_reads: Vec<MemCheckerTransaction>,
    read_observations: Vec<MemCheckerTransaction>,
    write_clusters: Vec<MemCheckerWriteClusterSnapshot>,
}

impl MemCheckerByteSnapshot {
    pub fn initial(address: u64) -> Self {
        Self::new(
            address,
            Vec::new(),
            vec![MemCheckerTransaction::observed_read(
                INITIAL_SERIAL,
                INITIAL_TICK,
                INITIAL_TICK,
                INITIAL_DATA,
            )],
            Vec::new(),
        )
    }

    pub fn new(
        address: u64,
        outstanding_reads: Vec<MemCheckerTransaction>,
        read_observations: Vec<MemCheckerTransaction>,
        write_clusters: Vec<MemCheckerWriteClusterSnapshot>,
    ) -> Self {
        Self {
            address,
            outstanding_reads,
            read_observations,
            write_clusters,
        }
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub fn outstanding_reads(&self) -> &[MemCheckerTransaction] {
        &self.outstanding_reads
    }

    pub fn read_observations(&self) -> &[MemCheckerTransaction] {
        &self.read_observations
    }

    pub fn write_clusters(&self) -> &[MemCheckerWriteClusterSnapshot] {
        &self.write_clusters
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemCheckerSnapshot {
    next_serial: u64,
    bytes: Vec<MemCheckerByteSnapshot>,
}

impl MemCheckerSnapshot {
    pub const fn new(next_serial: u64, bytes: Vec<MemCheckerByteSnapshot>) -> Self {
        Self { next_serial, bytes }
    }

    pub const fn next_serial(&self) -> u64 {
        self.next_serial
    }

    pub fn bytes(&self) -> &[MemCheckerByteSnapshot] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemCheckerStartedTransaction {
    serial: u64,
    address: u64,
    size: usize,
}

impl MemCheckerStartedTransaction {
    const fn new(serial: u64, address: u64, size: usize) -> Self {
        Self {
            serial,
            address,
            size,
        }
    }

    pub const fn serial(self) -> u64 {
        self.serial
    }

    pub const fn address(self) -> u64 {
        self.address
    }

    pub const fn size(self) -> usize {
        self.size
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemChecker {
    next_serial: u64,
    byte_trackers: BTreeMap<u64, ByteTracker>,
}

impl MemChecker {
    pub fn new() -> Self {
        Self {
            next_serial: FIRST_SERIAL,
            byte_trackers: BTreeMap::new(),
        }
    }

    pub fn from_snapshot(snapshot: &MemCheckerSnapshot) -> Result<Self, StatsError> {
        validate_snapshot(snapshot)?;
        let mut byte_trackers = BTreeMap::new();
        for byte in snapshot.bytes() {
            byte_trackers.insert(byte.address(), ByteTracker::from_snapshot(byte));
        }
        Ok(Self {
            next_serial: snapshot.next_serial(),
            byte_trackers,
        })
    }

    pub const fn next_serial(&self) -> u64 {
        self.next_serial
    }

    pub fn start_read(
        &mut self,
        start_tick: Tick,
        address: u64,
        size: usize,
    ) -> Result<MemCheckerStartedTransaction, StatsError> {
        validate_range(address, size)?;
        let serial = self.allocate_serial()?;
        for offset in 0..size {
            self.byte_tracker(address + offset as u64)
                .start_read(serial, start_tick);
        }
        Ok(MemCheckerStartedTransaction::new(serial, address, size))
    }

    pub fn start_write(
        &mut self,
        start_tick: Tick,
        address: u64,
        data: &[u8],
    ) -> Result<MemCheckerStartedTransaction, StatsError> {
        validate_range(address, data.len())?;
        self.validate_write_start(start_tick, address, data.len())?;
        let serial = self.allocate_serial()?;
        for (offset, byte) in data.iter().enumerate() {
            self.byte_tracker(address + offset as u64)
                .start_write(serial, start_tick, *byte)?;
        }
        Ok(MemCheckerStartedTransaction::new(
            serial,
            address,
            data.len(),
        ))
    }

    pub fn complete_read(
        &mut self,
        serial: u64,
        complete_tick: Tick,
        address: u64,
        data: &[u8],
    ) -> Result<MemCheckerReadResult, StatsError> {
        validate_range(address, data.len())?;
        self.validate_read_completion_time(serial, complete_tick, address, data.len())?;

        let mut checked_bytes = 0;
        let mut ignored_bytes = 0;
        let mut failures = Vec::new();
        for (offset, byte) in data.iter().enumerate() {
            let byte_address = address + offset as u64;
            match self.byte_trackers.get_mut(&byte_address) {
                Some(tracker) => match tracker.complete_read(serial, complete_tick, *byte)? {
                    ByteReadResult::Checked { valid, expected } => {
                        checked_bytes += 1;
                        if !valid {
                            failures.push(MemCheckerReadFailure::new(
                                byte_address,
                                *byte,
                                expected,
                            ));
                        }
                    }
                    ByteReadResult::Ignored => ignored_bytes += 1,
                },
                None => ignored_bytes += 1,
            }
        }

        if failures.is_empty() {
            Ok(MemCheckerReadResult::valid(
                serial,
                checked_bytes,
                ignored_bytes,
            ))
        } else {
            Ok(MemCheckerReadResult::invalid(
                serial,
                checked_bytes,
                ignored_bytes,
                failures,
            ))
        }
    }

    pub fn complete_write(
        &mut self,
        serial: u64,
        complete_tick: Tick,
        address: u64,
        size: usize,
    ) -> Result<MemCheckerWriteResult, StatsError> {
        validate_range(address, size)?;
        self.validate_write_completion_time(serial, complete_tick, address, size)?;
        self.validate_write_pending(serial, address, size)?;

        let mut completed_bytes = 0;
        let mut ignored_bytes = 0;
        for offset in 0..size {
            if let Some(tracker) = self.byte_trackers.get_mut(&(address + offset as u64)) {
                if tracker.complete_write(serial, complete_tick)? {
                    completed_bytes += 1;
                } else {
                    ignored_bytes += 1;
                }
            } else {
                ignored_bytes += 1;
            }
        }

        Ok(MemCheckerWriteResult::new(
            serial,
            completed_bytes,
            ignored_bytes,
        ))
    }

    pub fn abort_write(
        &mut self,
        serial: u64,
        address: u64,
        size: usize,
    ) -> Result<MemCheckerWriteResult, StatsError> {
        validate_range(address, size)?;
        self.validate_write_pending(serial, address, size)?;
        let mut completed_bytes = 0;
        let mut ignored_bytes = 0;
        for offset in 0..size {
            if let Some(tracker) = self.byte_trackers.get_mut(&(address + offset as u64)) {
                if tracker.abort_write(serial)? {
                    completed_bytes += 1;
                } else {
                    ignored_bytes += 1;
                }
            } else {
                ignored_bytes += 1;
            }
        }
        Ok(MemCheckerWriteResult::new(
            serial,
            completed_bytes,
            ignored_bytes,
        ))
    }

    pub fn reset(&mut self) {
        self.byte_trackers.clear();
    }

    pub fn reset_range(&mut self, address: u64, size: usize) -> Result<(), StatsError> {
        validate_range(address, size)?;
        for offset in 0..size {
            self.byte_trackers.remove(&(address + offset as u64));
        }
        Ok(())
    }

    pub fn snapshot(&self) -> MemCheckerSnapshot {
        MemCheckerSnapshot::new(
            self.next_serial,
            self.byte_trackers
                .iter()
                .map(|(address, tracker)| tracker.snapshot(*address))
                .collect(),
        )
    }

    fn byte_tracker(&mut self, address: u64) -> &mut ByteTracker {
        self.byte_trackers.entry(address).or_default()
    }

    fn allocate_serial(&mut self) -> Result<u64, StatsError> {
        let serial = self.next_serial;
        self.next_serial = self
            .next_serial
            .checked_add(1)
            .ok_or(StatsError::MemCheckerSerialOverflow)?;
        Ok(serial)
    }

    fn validate_read_completion_time(
        &self,
        serial: u64,
        complete_tick: Tick,
        address: u64,
        size: usize,
    ) -> Result<(), StatsError> {
        for offset in 0..size {
            let Some(tracker) = self.byte_trackers.get(&(address + offset as u64)) else {
                continue;
            };
            let Some(transaction) = tracker.outstanding_reads.get(&serial) else {
                continue;
            };
            validate_completion_time(serial, transaction.start_tick(), complete_tick)?;
        }
        Ok(())
    }

    fn validate_write_completion_time(
        &self,
        serial: u64,
        complete_tick: Tick,
        address: u64,
        size: usize,
    ) -> Result<(), StatsError> {
        for offset in 0..size {
            let Some(tracker) = self.byte_trackers.get(&(address + offset as u64)) else {
                continue;
            };
            let Some(transaction) = tracker.write_transaction(serial) else {
                continue;
            };
            validate_completion_time(serial, transaction.start_tick(), complete_tick)?;
        }
        Ok(())
    }

    fn validate_write_pending(
        &self,
        serial: u64,
        address: u64,
        size: usize,
    ) -> Result<(), StatsError> {
        for offset in 0..size {
            let Some(tracker) = self.byte_trackers.get(&(address + offset as u64)) else {
                continue;
            };
            tracker.validate_write_pending(serial)?;
        }
        Ok(())
    }

    fn validate_write_start(
        &self,
        start_tick: Tick,
        address: u64,
        size: usize,
    ) -> Result<(), StatsError> {
        for offset in 0..size {
            let Some(tracker) = self.byte_trackers.get(&(address + offset as u64)) else {
                continue;
            };
            tracker.validate_write_start(self.next_serial, start_tick)?;
        }
        Ok(())
    }
}

impl Default for MemChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ByteTracker {
    outstanding_reads: BTreeMap<u64, MemCheckerTransaction>,
    read_observations: Vec<MemCheckerTransaction>,
    write_clusters: Vec<WriteCluster>,
}

impl ByteTracker {
    fn from_snapshot(snapshot: &MemCheckerByteSnapshot) -> Self {
        Self {
            outstanding_reads: snapshot
                .outstanding_reads()
                .iter()
                .map(|transaction| (transaction.serial(), *transaction))
                .collect(),
            read_observations: snapshot.read_observations().to_vec(),
            write_clusters: snapshot
                .write_clusters()
                .iter()
                .map(WriteCluster::from_snapshot)
                .collect(),
        }
    }

    fn start_read(&mut self, serial: u64, start_tick: Tick) {
        self.outstanding_reads
            .insert(serial, MemCheckerTransaction::read(serial, start_tick));
    }

    fn complete_read(
        &mut self,
        serial: u64,
        complete_tick: Tick,
        data: u8,
    ) -> Result<ByteReadResult, StatsError> {
        let Some(transaction) = self.outstanding_reads.remove(&serial) else {
            return Ok(ByteReadResult::Ignored);
        };
        validate_completion_time(serial, transaction.start_tick(), complete_tick)?;

        let expected = self.expected_data(transaction.start_tick(), complete_tick, data);
        self.read_observations
            .push(MemCheckerTransaction::observed_read(
                serial,
                transaction.start_tick(),
                complete_tick,
                data,
            ));

        Ok(ByteReadResult::Checked {
            valid: expected.valid,
            expected: expected.values,
        })
    }

    fn start_write(&mut self, serial: u64, start_tick: Tick, data: u8) -> Result<(), StatsError> {
        self.incomplete_write_cluster()
            .start_write(serial, start_tick, data)
    }

    fn validate_write_start(&self, serial: u64, start_tick: Tick) -> Result<(), StatsError> {
        if self
            .write_clusters
            .last()
            .is_some_and(|cluster| !cluster.is_complete())
        {
            self.write_clusters
                .last()
                .expect("last write cluster existed")
                .validate_write_start(serial, start_tick)?;
        }
        Ok(())
    }

    fn complete_write(&mut self, serial: u64, complete_tick: Tick) -> Result<bool, StatsError> {
        let Some(index) = self.write_cluster_index_for_serial(serial) else {
            return Ok(false);
        };
        self.write_clusters[index].complete_write(serial, complete_tick)?;
        Ok(true)
    }

    fn abort_write(&mut self, serial: u64) -> Result<bool, StatsError> {
        let Some(index) = self.write_cluster_index_for_serial(serial) else {
            return Ok(false);
        };
        self.write_clusters[index].abort_write(serial)?;
        Ok(true)
    }

    fn write_transaction(&self, serial: u64) -> Option<MemCheckerTransaction> {
        self.write_clusters
            .iter()
            .find_map(|cluster| cluster.writes.get(&serial).copied())
    }

    fn validate_write_pending(&self, serial: u64) -> Result<(), StatsError> {
        for cluster in &self.write_clusters {
            if let Some(transaction) = cluster.writes.get(&serial) {
                if transaction.complete_tick() != FUTURE_TICK {
                    return Err(StatsError::MemCheckerWriteAlreadyCompleted { serial });
                }
            }
        }
        Ok(())
    }

    fn snapshot(&self, address: u64) -> MemCheckerByteSnapshot {
        MemCheckerByteSnapshot::new(
            address,
            self.outstanding_reads.values().copied().collect(),
            self.read_observations.clone(),
            self.write_clusters
                .iter()
                .filter(|cluster| !cluster.is_pristine())
                .map(WriteCluster::snapshot)
                .collect(),
        )
    }

    fn incomplete_write_cluster(&mut self) -> &mut WriteCluster {
        if self
            .write_clusters
            .last()
            .is_none_or(WriteCluster::is_complete)
        {
            self.write_clusters.push(WriteCluster::new());
        }
        self.write_clusters
            .last_mut()
            .expect("write cluster was just inserted")
    }

    fn write_cluster_index_for_serial(&self, serial: u64) -> Option<usize> {
        self.write_clusters
            .iter()
            .position(|cluster| cluster.writes.contains_key(&serial))
    }

    fn expected_data(&self, start_tick: Tick, _complete_tick: Tick, data: u8) -> ExpectedData {
        let mut expected = Vec::new();
        let mut write_clusters_overlap = true;
        let last_observation = self.last_completed_read_before(start_tick);
        let mut last_observation_valid = last_observation.complete_tick() != INITIAL_TICK;

        for cluster in self.write_clusters.iter().rev() {
            if !write_clusters_overlap {
                break;
            }
            for write in cluster.writes.values().rev() {
                if write.complete_tick() < last_observation.start_tick() {
                    continue;
                }
                if write.data() == data {
                    return ExpectedData::valid();
                }
                push_unique(&mut expected, write.data());
                if write.complete_tick() > start_tick {
                    continue;
                }
                write_clusters_overlap = false;
                if last_observation.complete_tick() < write.start_tick() {
                    last_observation_valid = false;
                }
            }
        }

        if last_observation_valid {
            if last_observation.data() == data {
                return ExpectedData::valid();
            }
            push_unique(&mut expected, last_observation.data());
        } else if !self.write_clusters.is_empty() && write_clusters_overlap {
            return ExpectedData::valid();
        }

        if expected.is_empty() {
            ExpectedData::valid()
        } else {
            ExpectedData::invalid(expected)
        }
    }

    fn last_completed_read_before(&self, before: Tick) -> MemCheckerTransaction {
        self.read_observations
            .iter()
            .rev()
            .find(|transaction| transaction.complete_tick() < before)
            .copied()
            .unwrap_or_else(|| self.read_observations[0])
    }
}

impl Default for ByteTracker {
    fn default() -> Self {
        Self {
            outstanding_reads: BTreeMap::new(),
            read_observations: vec![MemCheckerTransaction::observed_read(
                INITIAL_SERIAL,
                INITIAL_TICK,
                INITIAL_TICK,
                INITIAL_DATA,
            )],
            write_clusters: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WriteCluster {
    start_tick: Tick,
    complete_tick: Tick,
    complete_max_tick: Tick,
    incomplete_writes: usize,
    writes: BTreeMap<u64, MemCheckerTransaction>,
}

impl WriteCluster {
    const fn new() -> Self {
        Self {
            start_tick: FUTURE_TICK,
            complete_tick: FUTURE_TICK,
            complete_max_tick: INITIAL_TICK,
            incomplete_writes: 0,
            writes: BTreeMap::new(),
        }
    }

    fn from_snapshot(snapshot: &MemCheckerWriteClusterSnapshot) -> Self {
        Self {
            start_tick: snapshot.start_tick(),
            complete_tick: snapshot.complete_tick(),
            complete_max_tick: snapshot.complete_max_tick(),
            incomplete_writes: snapshot.incomplete_writes(),
            writes: snapshot
                .writes()
                .iter()
                .map(|transaction| (transaction.serial(), *transaction))
                .collect(),
        }
    }

    const fn is_complete(&self) -> bool {
        self.complete_tick != FUTURE_TICK
    }

    fn is_pristine(&self) -> bool {
        self.start_tick == FUTURE_TICK
            && self.complete_tick == FUTURE_TICK
            && self.complete_max_tick == INITIAL_TICK
            && self.incomplete_writes == 0
            && self.writes.is_empty()
    }

    fn start_write(&mut self, serial: u64, start_tick: Tick, data: u8) -> Result<(), StatsError> {
        self.validate_write_start(serial, start_tick)?;
        if self.start_tick == FUTURE_TICK {
            self.start_tick = start_tick;
        }
        if self.complete_tick != FUTURE_TICK {
            self.complete_tick = FUTURE_TICK;
        }
        if self
            .writes
            .insert(
                serial,
                MemCheckerTransaction::write(serial, start_tick, FUTURE_TICK, data),
            )
            .is_some()
        {
            return Err(StatsError::DuplicateMemCheckerSnapshotSerial { serial });
        }
        self.incomplete_writes =
            self.incomplete_writes
                .checked_add(1)
                .ok_or(StatsError::MemCheckerCounterOverflow {
                    name: "incomplete_writes",
                })?;
        Ok(())
    }

    fn validate_write_start(&self, serial: u64, start_tick: Tick) -> Result<(), StatsError> {
        if self.start_tick != FUTURE_TICK && start_tick < self.start_tick {
            return Err(StatsError::MemCheckerTransactionTimeWentBack {
                serial,
                start_tick: self.start_tick,
                complete_tick: start_tick,
            });
        }
        Ok(())
    }

    fn complete_write(&mut self, serial: u64, complete_tick: Tick) -> Result<(), StatsError> {
        let Some(transaction) = self.writes.get_mut(&serial) else {
            return Ok(());
        };
        if transaction.complete_tick() != FUTURE_TICK {
            return Err(StatsError::MemCheckerWriteAlreadyCompleted { serial });
        }
        validate_completion_time(serial, transaction.start_tick(), complete_tick)?;
        transaction.complete_tick = complete_tick;
        self.complete_max_tick = self.complete_max_tick.max(complete_tick);
        if self.incomplete_writes > 0 {
            self.incomplete_writes -= 1;
        }
        if self.incomplete_writes == 0 {
            self.complete_tick = self.complete_max_tick;
        }
        Ok(())
    }

    fn abort_write(&mut self, serial: u64) -> Result<(), StatsError> {
        let Some(transaction) = self.writes.remove(&serial) else {
            return Ok(());
        };
        if transaction.complete_tick() == FUTURE_TICK && self.incomplete_writes > 0 {
            self.incomplete_writes -= 1;
        }
        if self.writes.is_empty() {
            *self = Self::new();
            return Ok(());
        }
        if self.incomplete_writes == 0 && !self.writes.is_empty() {
            self.complete_tick = self.complete_max_tick;
        }
        Ok(())
    }

    fn snapshot(&self) -> MemCheckerWriteClusterSnapshot {
        MemCheckerWriteClusterSnapshot::new(
            self.start_tick,
            self.complete_tick,
            self.complete_max_tick,
            self.incomplete_writes,
            self.writes.values().copied().collect(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ByteReadResult {
    Checked { valid: bool, expected: Vec<u8> },
    Ignored,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpectedData {
    valid: bool,
    values: Vec<u8>,
}

impl ExpectedData {
    const fn valid() -> Self {
        Self {
            valid: true,
            values: Vec::new(),
        }
    }

    const fn invalid(values: Vec<u8>) -> Self {
        Self {
            valid: false,
            values,
        }
    }
}

fn validate_snapshot(snapshot: &MemCheckerSnapshot) -> Result<(), StatsError> {
    let mut addresses = BTreeSet::new();
    let mut highest_serial = INITIAL_SERIAL;
    for byte in snapshot.bytes() {
        if !addresses.insert(byte.address()) {
            return Err(StatsError::DuplicateMemCheckerSnapshotAddress {
                address: byte.address(),
            });
        }
        highest_serial = highest_serial.max(validate_byte_snapshot(byte)?);
    }
    if snapshot.next_serial() < FIRST_SERIAL || snapshot.next_serial() <= highest_serial {
        return Err(StatsError::MemCheckerSnapshotSerialCursorBehind {
            next_serial: snapshot.next_serial(),
            highest_serial,
        });
    }
    Ok(())
}

fn validate_byte_snapshot(snapshot: &MemCheckerByteSnapshot) -> Result<u64, StatsError> {
    if snapshot.read_observations().is_empty() {
        return Err(StatsError::EmptyMemCheckerReadObservations {
            address: snapshot.address(),
        });
    }

    let mut serials = BTreeSet::new();
    let mut highest_serial = INITIAL_SERIAL;
    for read in snapshot.outstanding_reads() {
        validate_transaction_time(*read)?;
        if !serials.insert(read.serial()) {
            return Err(StatsError::DuplicateMemCheckerSnapshotSerial {
                serial: read.serial(),
            });
        }
        highest_serial = highest_serial.max(read.serial());
    }
    for read in snapshot.read_observations() {
        validate_transaction_time(*read)?;
        if read.serial() != INITIAL_SERIAL && !serials.insert(read.serial()) {
            return Err(StatsError::DuplicateMemCheckerSnapshotSerial {
                serial: read.serial(),
            });
        }
        highest_serial = highest_serial.max(read.serial());
    }
    for cluster in snapshot.write_clusters() {
        validate_write_cluster_snapshot(cluster)?;
        for write in cluster.writes() {
            if !serials.insert(write.serial()) {
                return Err(StatsError::DuplicateMemCheckerSnapshotSerial {
                    serial: write.serial(),
                });
            }
            highest_serial = highest_serial.max(write.serial());
        }
    }
    Ok(highest_serial)
}

fn validate_write_cluster_snapshot(
    snapshot: &MemCheckerWriteClusterSnapshot,
) -> Result<(), StatsError> {
    let mut serials = BTreeSet::new();
    let mut incomplete = 0;
    let mut complete_max = INITIAL_TICK;
    for write in snapshot.writes() {
        validate_transaction_time(*write)?;
        if !serials.insert(write.serial()) {
            return Err(StatsError::DuplicateMemCheckerSnapshotSerial {
                serial: write.serial(),
            });
        }
        if write.complete_tick() == FUTURE_TICK {
            incomplete += 1;
        } else {
            complete_max = complete_max.max(write.complete_tick());
        }
    }
    if incomplete != snapshot.incomplete_writes() {
        return Err(StatsError::MemCheckerSnapshotClusterIncompleteMismatch {
            expected: incomplete,
            observed: snapshot.incomplete_writes(),
        });
    }
    let expected_complete_tick = if incomplete == 0 {
        complete_max
    } else {
        FUTURE_TICK
    };
    if snapshot.complete_tick() != expected_complete_tick {
        return Err(StatsError::MemCheckerSnapshotClusterCompletionMismatch {
            expected: expected_complete_tick,
            observed: snapshot.complete_tick(),
        });
    }
    if snapshot.complete_max_tick() != complete_max {
        return Err(StatsError::MemCheckerSnapshotClusterCompletionMismatch {
            expected: complete_max,
            observed: snapshot.complete_max_tick(),
        });
    }
    Ok(())
}

fn validate_transaction_time(transaction: MemCheckerTransaction) -> Result<(), StatsError> {
    if transaction.complete_tick() != FUTURE_TICK
        && transaction.complete_tick() < transaction.start_tick()
    {
        return Err(StatsError::MemCheckerTransactionTimeWentBack {
            serial: transaction.serial(),
            start_tick: transaction.start_tick(),
            complete_tick: transaction.complete_tick(),
        });
    }
    Ok(())
}

fn validate_completion_time(
    serial: u64,
    start_tick: Tick,
    complete_tick: Tick,
) -> Result<(), StatsError> {
    if complete_tick < start_tick {
        return Err(StatsError::MemCheckerTransactionTimeWentBack {
            serial,
            start_tick,
            complete_tick,
        });
    }
    Ok(())
}

fn validate_range(address: u64, size: usize) -> Result<(), StatsError> {
    if size == 0 {
        return Err(StatsError::InvalidMemCheckerAccessSize { size });
    }
    let bytes = u64::try_from(size)
        .map_err(|_| StatsError::MemCheckerAddressRangeOverflow { address, size })?;
    address
        .checked_add(bytes - 1)
        .ok_or(StatsError::MemCheckerAddressRangeOverflow { address, size })?;
    Ok(())
}

fn push_unique(values: &mut Vec<u8>, value: u8) {
    if !values.contains(&value) {
        values.push(value);
    }
}
