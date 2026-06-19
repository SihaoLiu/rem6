use rem6_cache::{
    ChiCacheControllerSnapshot, MesiCacheControllerSnapshot, MoesiCacheControllerSnapshot,
    MsiCacheControllerSnapshot,
};
use rem6_coherence::{
    ChiHarnessError, LineBackingStore, MesiHarnessError, MoesiHarnessError,
    PartitionedChiDirectoryLineHarness, PartitionedDirectoryLineHarness,
    PartitionedDirectoryLineHarnessSnapshot, PartitionedMesiDirectoryLineHarness,
    PartitionedMesiDirectoryLineHarnessSnapshot, PartitionedMoesiDirectoryLineHarness,
    PartitionedMoesiDirectoryLineHarnessSnapshot,
};
use rem6_directory::{
    ChiDirectoryLineState, DirectoryLineState, MesiDirectoryLineState, MoesiDirectoryLineState,
};
use rem6_dram::DramMemorySnapshot;
use rem6_memory::{Address, AgentId, MemoryTargetId};
use rem6_protocol_chi::{ChiCacheLine, ChiState};
use rem6_protocol_mesi::{MesiCacheLine, MesiState};
use rem6_protocol_moesi::{MoesiCacheLine, MoesiState};
use rem6_protocol_msi::{MsiCacheLine, MsiState};

pub(super) fn flush_msi_harness(
    harness: &mut PartitionedDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), rem6_coherence::HarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = msi_flush_data(&snapshot, target, line)
        .ok_or(rem6_coherence::HarnessError::MissingBackingMemory { line })?;
    replace_msi_snapshot_data(harness, &snapshot, target, line, data)
}

pub(super) fn flush_mesi_harness(
    harness: &mut PartitionedMesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), MesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = mesi_flush_data(&snapshot, line).ok_or(MesiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    replace_mesi_snapshot_data(harness, &snapshot, target, line, data)
}

pub(super) fn flush_moesi_harness(
    harness: &mut PartitionedMoesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), MoesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = moesi_flush_data(&snapshot, line).ok_or(MoesiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    replace_moesi_snapshot_data(harness, &snapshot, target, line, data)
}

pub(super) fn flush_chi_harness(
    harness: &mut PartitionedChiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), ChiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = chi_flush_data(&snapshot, line).ok_or(ChiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    replace_chi_snapshot_data(harness, &snapshot, target, line, data)
}

pub(super) fn clean_msi_harness(
    harness: &mut PartitionedDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), rem6_coherence::HarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = msi_flush_data(&snapshot, target, line)
        .ok_or(rem6_coherence::HarnessError::MissingBackingMemory { line })?;
    let backing = replace_optional_backing(snapshot.backing(), data.clone())?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(rem6_coherence::HarnessError::Dram)?;
    let directory = clean_msi_directory(snapshot.directory());
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| (*agent, clean_msi_cache_snapshot(*agent, cache)))
        .collect();
    let cleaned = PartitionedDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        directory,
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&cleaned)
}

pub(super) fn clean_mesi_harness(
    harness: &mut PartitionedMesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), MesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = mesi_flush_data(&snapshot, line).ok_or(MesiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(MesiHarnessError::Backing)?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(MesiHarnessError::Dram)?;
    let directory = clean_mesi_directory(snapshot.directory());
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| (*agent, clean_mesi_cache_snapshot(*agent, cache)))
        .collect();
    let cleaned = PartitionedMesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        directory,
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&cleaned)
}

pub(super) fn clean_moesi_harness(
    harness: &mut PartitionedMoesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), MoesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = moesi_flush_data(&snapshot, line).ok_or(MoesiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(MoesiHarnessError::Backing)?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(MoesiHarnessError::Dram)?;
    let directory = clean_moesi_directory(snapshot.directory());
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| (*agent, clean_moesi_cache_snapshot(*agent, cache)))
        .collect();
    let cleaned = PartitionedMoesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        directory,
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&cleaned)
}

pub(super) fn clean_chi_harness(
    harness: &mut PartitionedChiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), ChiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = chi_flush_data(&snapshot, line).ok_or(ChiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(ChiHarnessError::Backing)?;
    let dram_memory =
        replace_optional_dram_controller_line(snapshot.dram_memory(), target, line, data)
            .map_err(ChiHarnessError::Dram)?;
    let directory = clean_chi_directory(snapshot.directory());
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| (*agent, clean_chi_cache_snapshot(*agent, cache)))
        .collect();
    let cleaned = rem6_coherence::PartitionedChiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        directory,
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
    );
    harness.restore_quiescent(&cleaned)
}

pub(super) fn replace_msi_harness_data(
    harness: &mut PartitionedDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<(), rem6_coherence::HarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    if snapshot.backing().is_none() && snapshot.dram_memory().is_none() {
        return Err(rem6_coherence::HarnessError::MissingBackingMemory { line });
    }
    replace_msi_snapshot_data(harness, &snapshot, target, line, data)
}

pub(super) fn replace_mesi_harness_data(
    harness: &mut PartitionedMesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<(), MesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    replace_mesi_snapshot_data(harness, &snapshot, target, line, data)
}

pub(super) fn replace_moesi_harness_data(
    harness: &mut PartitionedMoesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<(), MoesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    replace_moesi_snapshot_data(harness, &snapshot, target, line, data)
}

pub(super) fn replace_chi_harness_data(
    harness: &mut PartitionedChiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<(), ChiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    replace_chi_snapshot_data(harness, &snapshot, target, line, data)
}

fn replace_msi_snapshot_data(
    harness: &mut PartitionedDirectoryLineHarness,
    snapshot: &PartitionedDirectoryLineHarnessSnapshot,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<(), rem6_coherence::HarnessError> {
    let backing = replace_optional_backing(snapshot.backing(), data.clone())?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(rem6_coherence::HarnessError::Dram)?;
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| {
            (
                *agent,
                MsiCacheControllerSnapshot::new(
                    MsiCacheLine::new(*agent, cache.line()),
                    cache.layout(),
                    cache.next_sequence(),
                    None,
                    None,
                ),
            )
        })
        .collect();
    let replaced = PartitionedDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        DirectoryLineState::new(snapshot.directory().line()),
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&replaced)
}

fn replace_mesi_snapshot_data(
    harness: &mut PartitionedMesiDirectoryLineHarness,
    snapshot: &PartitionedMesiDirectoryLineHarnessSnapshot,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<(), MesiHarnessError> {
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(MesiHarnessError::Backing)?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(MesiHarnessError::Dram)?;
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| {
            (
                *agent,
                MesiCacheControllerSnapshot::new(
                    MesiCacheLine::new(*agent, cache.line()),
                    cache.layout(),
                    cache.next_sequence(),
                    None,
                    None,
                ),
            )
        })
        .collect();
    let replaced = PartitionedMesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        MesiDirectoryLineState::new(snapshot.directory().line()),
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&replaced)
}

fn replace_moesi_snapshot_data(
    harness: &mut PartitionedMoesiDirectoryLineHarness,
    snapshot: &PartitionedMoesiDirectoryLineHarnessSnapshot,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<(), MoesiHarnessError> {
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(MoesiHarnessError::Backing)?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(MoesiHarnessError::Dram)?;
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| {
            (
                *agent,
                MoesiCacheControllerSnapshot::new(
                    MoesiCacheLine::new(*agent, cache.line()),
                    cache.layout(),
                    cache.next_sequence(),
                    None,
                    None,
                ),
            )
        })
        .collect();
    let replaced = PartitionedMoesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        MoesiDirectoryLineState::new(snapshot.directory().line()),
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&replaced)
}

fn replace_chi_snapshot_data(
    harness: &mut PartitionedChiDirectoryLineHarness,
    snapshot: &rem6_coherence::PartitionedChiDirectoryLineHarnessSnapshot,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<(), ChiHarnessError> {
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(ChiHarnessError::Backing)?;
    let dram_memory =
        replace_optional_dram_controller_line(snapshot.dram_memory(), target, line, data)
            .map_err(ChiHarnessError::Dram)?;
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| {
            (
                *agent,
                ChiCacheControllerSnapshot::new(
                    ChiCacheLine::new(*agent, cache.line()),
                    cache.layout(),
                    cache.next_sequence(),
                    None,
                    None,
                ),
            )
        })
        .collect();
    let replaced = rem6_coherence::PartitionedChiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        ChiDirectoryLineState::new(snapshot.directory().line()),
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
    );
    harness.restore_quiescent(&replaced)
}

fn clean_msi_directory(snapshot: &DirectoryLineState) -> DirectoryLineState {
    let mut clean = DirectoryLineState::new(snapshot.line());
    if let Some(owner) = snapshot.owner() {
        clean = clean.with_sharer(owner);
    }
    for sharer in snapshot.sharers() {
        clean = clean.with_sharer(*sharer);
    }
    clean
}

fn clean_mesi_directory(snapshot: &MesiDirectoryLineState) -> MesiDirectoryLineState {
    let mut clean = MesiDirectoryLineState::new(snapshot.line());
    if let Some((owner, state)) = snapshot.owner() {
        clean = clean.with_owner(owner, clean_mesi_state(state));
    }
    for sharer in snapshot.sharers() {
        clean = clean.with_sharer(*sharer);
    }
    clean
}

fn clean_moesi_directory(snapshot: &MoesiDirectoryLineState) -> MoesiDirectoryLineState {
    let mut clean = MoesiDirectoryLineState::new(snapshot.line());
    if let Some((owner, state)) = snapshot.owner() {
        match clean_moesi_state(state) {
            MoesiState::Shared => clean = clean.with_sharer(owner),
            state => clean = clean.with_owner(owner, state),
        }
    }
    for sharer in snapshot.sharers() {
        clean = clean.with_sharer(*sharer);
    }
    clean
}

fn clean_chi_directory(snapshot: &ChiDirectoryLineState) -> ChiDirectoryLineState {
    let mut clean = ChiDirectoryLineState::new(snapshot.line());
    if let (Some(owner), Some(state)) = (snapshot.unique_owner(), snapshot.unique_owner_state()) {
        clean = clean.with_unique_owner(owner, clean_chi_state(state));
    }
    for (sharer, state) in snapshot.sharers() {
        clean = clean.with_sharer(*sharer, clean_chi_state(*state));
    }
    clean
}

fn clean_msi_cache_snapshot(
    agent: AgentId,
    snapshot: &MsiCacheControllerSnapshot,
) -> MsiCacheControllerSnapshot {
    let data = snapshot.cached_data().map(<[_]>::to_vec);
    let mut line = MsiCacheLine::new(agent, snapshot.line());
    line.force_state(clean_msi_state(snapshot.state()))
        .expect("clean MSI trace response selects a stable cache state");
    MsiCacheControllerSnapshot::new(
        line,
        snapshot.layout(),
        snapshot.next_sequence(),
        data,
        None,
    )
}

fn clean_mesi_cache_snapshot(
    agent: AgentId,
    snapshot: &MesiCacheControllerSnapshot,
) -> MesiCacheControllerSnapshot {
    let data = snapshot.cached_data().map(<[_]>::to_vec);
    let mut line = MesiCacheLine::new(agent, snapshot.line());
    line.force_state(clean_mesi_state(snapshot.state()))
        .expect("clean MESI trace response selects a stable cache state");
    MesiCacheControllerSnapshot::new(
        line,
        snapshot.layout(),
        snapshot.next_sequence(),
        data,
        None,
    )
}

fn clean_moesi_cache_snapshot(
    agent: AgentId,
    snapshot: &MoesiCacheControllerSnapshot,
) -> MoesiCacheControllerSnapshot {
    let data = snapshot.cached_data().map(<[_]>::to_vec);
    let mut line = MoesiCacheLine::new(agent, snapshot.line());
    line.force_state(clean_moesi_state(snapshot.state()))
        .expect("clean MOESI trace response selects a stable cache state");
    MoesiCacheControllerSnapshot::new(
        line,
        snapshot.layout(),
        snapshot.next_sequence(),
        data,
        None,
    )
}

fn clean_chi_cache_snapshot(
    agent: AgentId,
    snapshot: &ChiCacheControllerSnapshot,
) -> ChiCacheControllerSnapshot {
    let data = snapshot.cached_data().map(<[_]>::to_vec);
    let mut line = ChiCacheLine::new(agent, snapshot.line());
    line.force_state(clean_chi_state(snapshot.state()))
        .expect("clean CHI trace response selects a stable cache state");
    ChiCacheControllerSnapshot::new(
        line,
        snapshot.layout(),
        snapshot.next_sequence(),
        data,
        None,
    )
}

fn clean_msi_state(state: MsiState) -> MsiState {
    match state {
        MsiState::Modified => MsiState::Shared,
        state => state,
    }
}

fn clean_mesi_state(state: MesiState) -> MesiState {
    match state {
        MesiState::Modified => MesiState::Exclusive,
        state => state,
    }
}

fn clean_moesi_state(state: MoesiState) -> MoesiState {
    match state {
        MoesiState::Modified => MoesiState::Exclusive,
        MoesiState::Owned => MoesiState::Shared,
        state => state,
    }
}

fn clean_chi_state(state: ChiState) -> ChiState {
    match state {
        ChiState::SharedDirty => ChiState::SharedClean,
        ChiState::UniqueDirty => ChiState::UniqueClean,
        state => state,
    }
}

fn msi_flush_data(
    snapshot: &PartitionedDirectoryLineHarnessSnapshot,
    target: MemoryTargetId,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .caches()
        .values()
        .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
        .or_else(|| snapshot.backing().map(|backing| backing.data().to_vec()))
        .or_else(|| {
            snapshot
                .dram_memory()
                .and_then(|dram| dram_snapshot_line_data(dram, target, line))
        })
}

fn mesi_flush_data(
    snapshot: &PartitionedMesiDirectoryLineHarnessSnapshot,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .caches()
        .values()
        .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
        .or_else(|| {
            (snapshot.backing().line_address() == line).then(|| snapshot.backing().data().to_vec())
        })
}

fn moesi_flush_data(
    snapshot: &PartitionedMoesiDirectoryLineHarnessSnapshot,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .caches()
        .values()
        .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
        .or_else(|| {
            (snapshot.backing().line_address() == line).then(|| snapshot.backing().data().to_vec())
        })
}

fn chi_flush_data(
    snapshot: &rem6_coherence::PartitionedChiDirectoryLineHarnessSnapshot,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .caches()
        .values()
        .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
        .or_else(|| {
            (snapshot.backing().line_address() == line).then(|| snapshot.backing().data().to_vec())
        })
}

pub(super) fn dram_snapshot_line_data(
    snapshot: &DramMemorySnapshot,
    target: MemoryTargetId,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .store()
        .partitions()
        .iter()
        .find(|partition| partition.target() == target)
        .and_then(|partition| {
            partition
                .lines()
                .iter()
                .find(|candidate| candidate.line() == line)
        })
        .map(|line| line.data().to_vec())
}

fn replace_backing(
    backing: &LineBackingStore,
    data: Vec<u8>,
) -> Result<LineBackingStore, rem6_coherence::HarnessError> {
    let mut backing = backing.clone();
    backing.replace_data(data)?;
    Ok(backing)
}

fn replace_optional_backing(
    backing: Option<&LineBackingStore>,
    data: Vec<u8>,
) -> Result<Option<LineBackingStore>, rem6_coherence::HarnessError> {
    backing
        .map(|backing| replace_backing(backing, data))
        .transpose()
}

fn replace_optional_dram_line(
    snapshot: Option<&DramMemorySnapshot>,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<Option<DramMemorySnapshot>, rem6_dram::DramMemoryError> {
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    let mut dram = rem6_dram::DramMemoryController::from_snapshot(snapshot)?;
    dram.insert_line(target, line, data)?;
    Ok(Some(dram.snapshot()))
}

fn replace_optional_dram_controller_line(
    snapshot: Option<&rem6_dram::DramMemoryController>,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<Option<rem6_dram::DramMemoryController>, rem6_dram::DramMemoryError> {
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    let mut dram = snapshot.clone();
    dram.insert_line(target, line, data)?;
    Ok(Some(dram))
}
