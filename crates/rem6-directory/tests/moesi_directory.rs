use rem6_directory::{
    MoesiDirectory, MoesiDirectoryDataSource, MoesiDirectoryDecision, MoesiDirectoryError,
    MoesiDirectoryGrant, MoesiDirectoryLineState, MoesiDirectorySnoop,
};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId};
use rem6_protocol_moesi::{MoesiEvent, MoesiLineId, MoesiState};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MoesiLineId {
    MoesiLineId::new(Address::new(0x4000))
}

fn id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn line_size() -> AccessSize {
    AccessSize::new(64).unwrap()
}

fn read_shared(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        id(agent, sequence),
        Address::new(0x4000),
        line_size(),
        layout(),
    )
    .unwrap()
}

fn read_unique(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_unique(
        id(agent, sequence),
        Address::new(0x4000),
        line_size(),
        layout(),
    )
    .unwrap()
}

fn upgrade(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::upgrade(
        id(agent, sequence),
        Address::new(0x4000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap()
}

fn writeback_dirty(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::writeback_dirty(
        id(agent, sequence),
        Address::new(0x4000),
        (0..64).collect(),
        layout(),
    )
    .unwrap()
}

fn grant(
    request: MemoryRequestId,
    state: MoesiState,
    source: MoesiDirectoryDataSource,
) -> MoesiDirectoryGrant {
    MoesiDirectoryGrant::new(request, line(), state, source)
}

#[test]
fn moesi_directory_grants_exclusive_first_read_then_owned_on_peer_read() {
    let mut directory = MoesiDirectory::new();

    let first = directory.accept(read_shared(1, 0)).unwrap();
    assert_eq!(
        first,
        MoesiDirectoryDecision::new(
            line(),
            id(1, 0),
            MoesiDirectoryLineState::new(line()),
            MoesiDirectoryLineState::new(line()).with_owner(AgentId::new(1), MoesiState::Exclusive),
            Vec::new(),
            Some(grant(
                id(1, 0),
                MoesiState::Exclusive,
                MoesiDirectoryDataSource::BackingMemory,
            )),
        )
    );

    directory.accept(upgrade(1, 1)).unwrap();
    let second = directory.accept(read_shared(2, 0)).unwrap();

    assert_eq!(
        second.snoops(),
        &[MoesiDirectorySnoop::new(
            AgentId::new(1),
            MoesiEvent::SnoopRead,
        )]
    );
    assert_eq!(
        second.grant(),
        Some(&grant(
            id(2, 0),
            MoesiState::Shared,
            MoesiDirectoryDataSource::OwnerCache(AgentId::new(1)),
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MoesiDirectoryLineState::new(line())
            .with_owner(AgentId::new(1), MoesiState::Owned)
            .with_sharer(AgentId::new(2))
    );
    directory
        .line_state(line())
        .protocol_snapshot()
        .validate()
        .unwrap();
}

#[test]
fn moesi_directory_clean_peer_read_removes_exclusive_owner() {
    let mut directory = MoesiDirectory::new();
    directory.accept(read_shared(4, 0)).unwrap();

    let decision = directory.accept(read_shared(2, 0)).unwrap();

    assert_eq!(
        decision.snoops(),
        &[MoesiDirectorySnoop::new(
            AgentId::new(4),
            MoesiEvent::SnoopRead,
        )]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 0),
            MoesiState::Shared,
            MoesiDirectoryDataSource::OwnerCache(AgentId::new(4)),
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MoesiDirectoryLineState::new(line())
            .with_sharer(AgentId::new(2))
            .with_sharer(AgentId::new(4))
    );
}

#[test]
fn moesi_directory_dirty_writeback_from_owned_owner_preserves_sharers() {
    let mut directory = MoesiDirectory::new();
    directory.accept(read_unique(1, 0)).unwrap();
    directory.accept(read_shared(2, 0)).unwrap();
    directory.accept(read_shared(3, 0)).unwrap();

    let non_owner = directory.accept(writeback_dirty(2, 1)).unwrap_err();
    assert_eq!(
        non_owner,
        MoesiDirectoryError::WritebackFromNonOwner {
            line: line(),
            requester: AgentId::new(2),
            owner: Some(AgentId::new(1)),
        }
    );

    let decision = directory.accept(writeback_dirty(1, 1)).unwrap();
    assert_eq!(decision.snoops(), &[]);
    assert_eq!(decision.grant(), None);
    assert_eq!(
        directory.line_state(line()),
        MoesiDirectoryLineState::new(line())
            .with_sharer(AgentId::new(2))
            .with_sharer(AgentId::new(3))
    );
    directory
        .line_state(line())
        .protocol_snapshot()
        .validate()
        .unwrap();
}

#[test]
fn moesi_directory_new_reader_after_owned_writeback_uses_backing_memory() {
    let mut directory = MoesiDirectory::new();
    directory.accept(read_unique(1, 0)).unwrap();
    directory.accept(read_shared(2, 0)).unwrap();
    directory.accept(writeback_dirty(1, 1)).unwrap();

    let decision = directory.accept(read_shared(3, 0)).unwrap();

    assert_eq!(decision.snoops(), &[]);
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(3, 0),
            MoesiState::Shared,
            MoesiDirectoryDataSource::BackingMemory,
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MoesiDirectoryLineState::new(line())
            .with_sharer(AgentId::new(2))
            .with_sharer(AgentId::new(3))
    );
}

#[test]
fn moesi_directory_read_unique_invalidates_dirty_owner_and_sharers_in_order() {
    let mut directory = MoesiDirectory::new();
    directory.accept(read_unique(3, 0)).unwrap();
    directory.accept(read_shared(1, 0)).unwrap();
    directory.accept(read_shared(4, 0)).unwrap();

    let decision = directory.accept(read_unique(2, 0)).unwrap();

    assert_eq!(
        decision.snoops(),
        &[
            MoesiDirectorySnoop::new(AgentId::new(3), MoesiEvent::SnoopWrite),
            MoesiDirectorySnoop::new(AgentId::new(1), MoesiEvent::SnoopWrite),
            MoesiDirectorySnoop::new(AgentId::new(4), MoesiEvent::SnoopWrite),
        ]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 0),
            MoesiState::Modified,
            MoesiDirectoryDataSource::OwnerCache(AgentId::new(3)),
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MoesiDirectoryLineState::new(line()).with_owner(AgentId::new(2), MoesiState::Modified)
    );
}

#[test]
fn moesi_directory_upgrade_from_sharer_invalidates_owner_and_peers() {
    let mut directory = MoesiDirectory::new();
    directory.accept(read_unique(3, 0)).unwrap();
    directory.accept(read_shared(1, 0)).unwrap();
    directory.accept(read_shared(4, 0)).unwrap();

    let miss = directory.accept(upgrade(2, 0)).unwrap_err();
    assert_eq!(
        miss,
        MoesiDirectoryError::UpgradeRequesterNotSharer {
            line: line(),
            requester: AgentId::new(2),
        }
    );

    let decision = directory.accept(upgrade(4, 0)).unwrap();
    assert_eq!(
        decision.snoops(),
        &[
            MoesiDirectorySnoop::new(AgentId::new(3), MoesiEvent::SnoopWrite),
            MoesiDirectorySnoop::new(AgentId::new(1), MoesiEvent::SnoopWrite),
        ]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(4, 0),
            MoesiState::Modified,
            MoesiDirectoryDataSource::OwnerCache(AgentId::new(3)),
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MoesiDirectoryLineState::new(line()).with_owner(AgentId::new(4), MoesiState::Modified)
    );
}

#[test]
fn moesi_directory_restores_line_state_for_later_requests() {
    let mut directory = MoesiDirectory::new();
    let snapshot = MoesiDirectoryLineState::new(line())
        .with_owner(AgentId::new(2), MoesiState::Owned)
        .with_sharer(AgentId::new(4));

    directory.restore_line_state(&snapshot).unwrap();
    assert_eq!(directory.line_state(line()), snapshot);

    let decision = directory.accept(read_unique(1, 2)).unwrap();
    assert_eq!(
        decision.snoops(),
        &[
            MoesiDirectorySnoop::new(AgentId::new(2), MoesiEvent::SnoopWrite),
            MoesiDirectorySnoop::new(AgentId::new(4), MoesiEvent::SnoopWrite),
        ]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(1, 2),
            MoesiState::Modified,
            MoesiDirectoryDataSource::OwnerCache(AgentId::new(2)),
        ))
    );

    directory
        .restore_line_state(&MoesiDirectoryLineState::new(line()))
        .unwrap();
    assert_eq!(
        directory.line_state(line()),
        MoesiDirectoryLineState::new(line())
    );
    let clean = directory.accept(read_shared(3, 0)).unwrap();
    assert_eq!(clean.snoops(), &[]);
    assert_eq!(
        clean.grant(),
        Some(&grant(
            id(3, 0),
            MoesiState::Exclusive,
            MoesiDirectoryDataSource::BackingMemory,
        ))
    );
}

#[test]
fn moesi_directory_restore_rejects_invalid_owner_state() {
    let mut directory = MoesiDirectory::new();
    let snapshot =
        MoesiDirectoryLineState::new(line()).with_owner(AgentId::new(2), MoesiState::Shared);

    assert_eq!(
        directory.restore_line_state(&snapshot).unwrap_err(),
        MoesiDirectoryError::InvalidSnapshotOwnerState {
            line: line(),
            state: MoesiState::Shared,
        }
    );
    assert_eq!(
        directory.line_state(line()),
        MoesiDirectoryLineState::new(line())
    );
}
