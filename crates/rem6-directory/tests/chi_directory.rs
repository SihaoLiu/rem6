use rem6_directory::{
    ChiDirectory, ChiDirectoryDataSource, ChiDirectoryError, ChiDirectoryGrant,
    ChiDirectoryLineState, ChiDirectorySnoop,
};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId};
use rem6_protocol_chi::{ChiEvent, ChiLineId, ChiState};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line_at(address: u64) -> ChiLineId {
    ChiLineId::new(Address::new(address))
}

fn line() -> ChiLineId {
    line_at(0x6000)
}

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn id(agent_id: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(agent(agent_id), sequence)
}

fn line_size() -> AccessSize {
    AccessSize::new(64).unwrap()
}

fn read_shared(agent_id: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        id(agent_id, sequence),
        Address::new(0x6004),
        line_size(),
        layout(),
    )
    .unwrap()
}

fn read_unique(agent_id: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_unique(
        id(agent_id, sequence),
        Address::new(0x6008),
        line_size(),
        layout(),
    )
    .unwrap()
}

fn upgrade(agent_id: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::upgrade(
        id(agent_id, sequence),
        Address::new(0x600c),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write_clean(agent_id: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::write_clean(
        id(agent_id, sequence),
        Address::new(0x6000),
        (0..64).collect(),
        layout(),
    )
    .unwrap()
}

fn clean_shared(agent_id: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::clean_shared(id(agent_id, sequence), Address::new(0x6000), layout()).unwrap()
}

fn grant(
    request: MemoryRequestId,
    state: ChiState,
    source: ChiDirectoryDataSource,
) -> ChiDirectoryGrant {
    ChiDirectoryGrant::new(request, line(), state, source)
}

#[test]
fn chi_directory_shared_read_downgrades_unique_dirty_owner() {
    let mut directory = ChiDirectory::new();
    let before =
        ChiDirectoryLineState::new(line()).with_unique_owner(agent(1), ChiState::UniqueDirty);
    directory.restore_line_state(&before).unwrap();

    let decision = directory.accept(read_shared(2, 0)).unwrap();

    let after = ChiDirectoryLineState::new(line())
        .with_sharer(agent(1), ChiState::SharedClean)
        .with_sharer(agent(2), ChiState::SharedClean);
    assert_eq!(decision.before(), &before);
    assert_eq!(decision.after(), &after);
    assert_eq!(
        decision.snoops(),
        &[ChiDirectorySnoop::new(agent(1), ChiEvent::SnoopShared)]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 0),
            ChiState::SharedClean,
            ChiDirectoryDataSource::OwnerCache(agent(1)),
        ))
    );
    assert_eq!(directory.line_state(line()), after);
    directory
        .line_state(line())
        .protocol_snapshot()
        .validate()
        .unwrap();
}

#[test]
fn chi_directory_read_unique_invalidates_sharers_and_uses_dirty_peer_data() {
    let mut directory = ChiDirectory::new();
    directory
        .restore_line_state(
            &ChiDirectoryLineState::new(line())
                .with_sharer(agent(1), ChiState::SharedClean)
                .with_sharer(agent(3), ChiState::SharedDirty),
        )
        .unwrap();

    let decision = directory.accept(read_unique(2, 0)).unwrap();

    assert_eq!(
        decision.snoops(),
        &[
            ChiDirectorySnoop::new(agent(1), ChiEvent::SnoopUnique),
            ChiDirectorySnoop::new(agent(3), ChiEvent::SnoopUnique),
        ]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 0),
            ChiState::UniqueDirty,
            ChiDirectoryDataSource::OwnerCache(agent(3)),
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        ChiDirectoryLineState::new(line()).with_unique_owner(agent(2), ChiState::UniqueDirty)
    );
}

#[test]
fn chi_directory_upgrade_requires_existing_sharer_and_uses_no_data_grant() {
    let mut directory = ChiDirectory::new();
    directory
        .restore_line_state(
            &ChiDirectoryLineState::new(line())
                .with_sharer(agent(2), ChiState::SharedClean)
                .with_sharer(agent(3), ChiState::SharedClean),
        )
        .unwrap();

    let miss = directory.accept(upgrade(4, 0)).unwrap_err();
    assert_eq!(
        miss,
        ChiDirectoryError::UpgradeRequesterNotSharer {
            line: line(),
            requester: agent(4),
        }
    );

    let decision = directory.accept(upgrade(2, 1)).unwrap();
    assert_eq!(
        decision.snoops(),
        &[ChiDirectorySnoop::new(agent(3), ChiEvent::SnoopUnique)]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 1),
            ChiState::UniqueDirty,
            ChiDirectoryDataSource::NoData,
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        ChiDirectoryLineState::new(line()).with_unique_owner(agent(2), ChiState::UniqueDirty)
    );
}

#[test]
fn chi_directory_write_clean_converts_dirty_sharer_to_clean_sharer() {
    let mut directory = ChiDirectory::new();
    let before = ChiDirectoryLineState::new(line())
        .with_sharer(agent(1), ChiState::SharedDirty)
        .with_sharer(agent(2), ChiState::SharedClean);
    directory.restore_line_state(&before).unwrap();

    assert_eq!(
        directory.accept(write_clean(3, 0)).unwrap_err(),
        ChiDirectoryError::EvictFromNonHolder {
            line: line(),
            requester: agent(3),
        }
    );

    let after = ChiDirectoryLineState::new(line())
        .with_sharer(agent(1), ChiState::SharedClean)
        .with_sharer(agent(2), ChiState::SharedClean);
    let decision = directory.accept(write_clean(1, 1)).unwrap();

    assert_eq!(decision.snoops(), &[]);
    assert_eq!(decision.grant(), None);
    assert_eq!(decision.before(), &before);
    assert_eq!(decision.after(), &after);
    assert_eq!(directory.line_state(line()), after);
}

#[test]
fn chi_directory_clean_shared_converts_dirty_sharer_to_clean_sharer() {
    let mut directory = ChiDirectory::new();
    let before = ChiDirectoryLineState::new(line())
        .with_sharer(agent(1), ChiState::SharedDirty)
        .with_sharer(agent(2), ChiState::SharedClean);
    directory.restore_line_state(&before).unwrap();

    assert_eq!(
        directory.accept(clean_shared(3, 0)).unwrap_err(),
        ChiDirectoryError::EvictFromNonHolder {
            line: line(),
            requester: agent(3),
        }
    );

    let after = ChiDirectoryLineState::new(line())
        .with_sharer(agent(1), ChiState::SharedClean)
        .with_sharer(agent(2), ChiState::SharedClean);
    let decision = directory.accept(clean_shared(1, 1)).unwrap();

    assert_eq!(decision.snoops(), &[]);
    assert_eq!(decision.grant(), None);
    assert_eq!(decision.before(), &before);
    assert_eq!(decision.after(), &after);
    assert_eq!(directory.line_state(line()), after);
}

#[test]
fn chi_directory_snapshot_restore_round_trips_sorted_lines() {
    let mut directory = ChiDirectory::new();
    let lower = ChiDirectoryLineState::new(line_at(0x6100))
        .with_sharer(agent(3), ChiState::SharedClean)
        .with_sharer(agent(1), ChiState::SharedClean);
    let upper = ChiDirectoryLineState::new(line_at(0x6200))
        .with_unique_owner(agent(2), ChiState::UniqueClean);

    directory
        .restore_line_states(&[upper.clone(), lower.clone()])
        .unwrap();

    assert_eq!(
        directory.line_addresses(),
        vec![Address::new(0x6100), Address::new(0x6200)]
    );
    assert_eq!(directory.line_states(), vec![lower.clone(), upper.clone()]);

    directory
        .restore_line_state(&ChiDirectoryLineState::new(line_at(0x6100)))
        .unwrap();
    assert_eq!(directory.line_addresses(), vec![Address::new(0x6200)]);
    assert_eq!(directory.line_state(line_at(0x6200)), upper);
}

#[test]
fn chi_directory_evict_hazard_restore_retains_requester_for_gem5_issue_3013() {
    let mut directory = ChiDirectory::new();
    let before = ChiDirectoryLineState::new(line()).with_sharer(agent(1), ChiState::SharedClean);
    directory.restore_line_state(&before).unwrap();

    let hazard = directory.begin_evict_hazard(line(), agent(1)).unwrap();
    directory
        .restore_line_state(&ChiDirectoryLineState::new(line()))
        .unwrap();

    let restored = directory.restore_evict_hazard(&hazard).unwrap();

    assert_eq!(restored.acknowledgement_target(), agent(1));
    assert!(restored.request_became_stale());
    assert_eq!(restored.retained_state(), &before);
    assert_eq!(
        restored.current_state(),
        &ChiDirectoryLineState::new(line())
    );
}

#[test]
fn chi_directory_evict_hazard_rejects_unknown_requester() {
    let mut directory = ChiDirectory::new();
    directory
        .restore_line_state(
            &ChiDirectoryLineState::new(line()).with_sharer(agent(1), ChiState::SharedClean),
        )
        .unwrap();

    assert_eq!(
        directory.begin_evict_hazard(line(), agent(2)).unwrap_err(),
        ChiDirectoryError::EvictFromNonHolder {
            line: line(),
            requester: agent(2),
        }
    );
}
