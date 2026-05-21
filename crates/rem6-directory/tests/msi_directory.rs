use rem6_directory::{
    DirectoryDataSource, DirectoryDecision, DirectoryError, DirectoryGrant, DirectoryLineState,
    DirectorySnoop, MsiDirectory,
};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId};
use rem6_protocol_msi::{MsiEvent, MsiLineId, MsiState};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MsiLineId {
    MsiLineId::new(Address::new(0x1000))
}

fn id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn read_shared(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        id(agent, sequence),
        Address::new(0x1000),
        line_size(),
        layout(),
    )
    .unwrap()
}

fn read_unique(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_unique(
        id(agent, sequence),
        Address::new(0x1000),
        line_size(),
        layout(),
    )
    .unwrap()
}

fn upgrade(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::upgrade(
        id(agent, sequence),
        Address::new(0x1000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap()
}

fn writeback_dirty(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::writeback_dirty(
        id(agent, sequence),
        Address::new(0x1000),
        (0..64).collect(),
        layout(),
    )
    .unwrap()
}

fn line_size() -> AccessSize {
    AccessSize::new(64).unwrap()
}

fn grant(request: MemoryRequestId, state: MsiState, source: DirectoryDataSource) -> DirectoryGrant {
    DirectoryGrant::new(request, line(), state, source)
}

#[test]
fn directory_adds_clean_readers_as_ordered_sharers() {
    let mut directory = MsiDirectory::new();

    let first = directory.accept(read_shared(2, 0)).unwrap();
    assert_eq!(
        first,
        DirectoryDecision::new(
            line(),
            id(2, 0),
            DirectoryLineState::new(line()),
            DirectoryLineState::new(line()).with_sharer(AgentId::new(2)),
            Vec::new(),
            Some(grant(
                id(2, 0),
                MsiState::Shared,
                DirectoryDataSource::BackingMemory,
            )),
        )
    );

    let second = directory.accept(read_shared(1, 0)).unwrap();
    assert_eq!(second.snoops(), &[]);
    assert_eq!(
        second.grant(),
        Some(&grant(
            id(1, 0),
            MsiState::Shared,
            DirectoryDataSource::BackingMemory,
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        DirectoryLineState::new(line())
            .with_sharer(AgentId::new(1))
            .with_sharer(AgentId::new(2))
    );
}

#[test]
fn directory_read_unique_invalidates_clean_sharers_deterministically() {
    let mut directory = MsiDirectory::new();
    directory.accept(read_shared(3, 0)).unwrap();
    directory.accept(read_shared(1, 0)).unwrap();

    let decision = directory.accept(read_unique(2, 0)).unwrap();

    assert_eq!(
        decision.snoops(),
        &[
            DirectorySnoop::new(AgentId::new(1), MsiEvent::SnoopWrite),
            DirectorySnoop::new(AgentId::new(3), MsiEvent::SnoopWrite),
        ]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 0),
            MsiState::Modified,
            DirectoryDataSource::BackingMemory,
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        DirectoryLineState::new(line()).with_owner(AgentId::new(2))
    );
    directory
        .line_state(line())
        .protocol_snapshot()
        .validate()
        .unwrap();
}

#[test]
fn directory_read_shared_downgrades_modified_owner() {
    let mut directory = MsiDirectory::new();
    directory.accept(read_unique(1, 0)).unwrap();

    let decision = directory.accept(read_shared(2, 0)).unwrap();

    assert_eq!(
        decision.snoops(),
        &[DirectorySnoop::new(AgentId::new(1), MsiEvent::SnoopRead)]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 0),
            MsiState::Shared,
            DirectoryDataSource::ModifiedOwner(AgentId::new(1)),
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        DirectoryLineState::new(line())
            .with_sharer(AgentId::new(1))
            .with_sharer(AgentId::new(2))
    );
}

#[test]
fn directory_upgrade_requires_existing_sharer_and_invalidates_peers() {
    let mut directory = MsiDirectory::new();
    directory.accept(read_shared(1, 0)).unwrap();
    directory.accept(read_shared(3, 0)).unwrap();

    let miss = directory.accept(upgrade(2, 0)).unwrap_err();
    assert_eq!(
        miss,
        DirectoryError::UpgradeRequesterNotSharer {
            line: line(),
            requester: AgentId::new(2),
        }
    );

    let decision = directory.accept(upgrade(3, 0)).unwrap();
    assert_eq!(
        decision.snoops(),
        &[DirectorySnoop::new(AgentId::new(1), MsiEvent::SnoopWrite)]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(3, 0),
            MsiState::Modified,
            DirectoryDataSource::NoData,
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        DirectoryLineState::new(line()).with_owner(AgentId::new(3))
    );
}

#[test]
fn directory_dirty_writeback_clears_owner_and_rejects_non_owner() {
    let mut directory = MsiDirectory::new();
    directory.accept(read_unique(1, 0)).unwrap();

    let non_owner = directory.accept(writeback_dirty(2, 0)).unwrap_err();
    assert_eq!(
        non_owner,
        DirectoryError::WritebackFromNonOwner {
            line: line(),
            requester: AgentId::new(2),
            owner: Some(AgentId::new(1)),
        }
    );

    let decision = directory.accept(writeback_dirty(1, 0)).unwrap();
    assert_eq!(decision.snoops(), &[]);
    assert_eq!(decision.grant(), None);
    assert_eq!(
        directory.line_state(line()),
        DirectoryLineState::new(line())
    );
}

#[test]
fn directory_restores_line_state_for_later_requests() {
    let mut directory = MsiDirectory::new();
    let snapshot = DirectoryLineState::new(line()).with_owner(AgentId::new(2));

    directory.restore_line_state(&snapshot);
    assert_eq!(directory.line_state(line()), snapshot);

    let decision = directory.accept(read_shared(1, 2)).unwrap();
    assert_eq!(
        decision.snoops(),
        &[DirectorySnoop::new(AgentId::new(2), MsiEvent::SnoopRead)]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(1, 2),
            MsiState::Shared,
            DirectoryDataSource::ModifiedOwner(AgentId::new(2)),
        ))
    );

    directory.restore_line_state(&DirectoryLineState::new(line()));
    assert_eq!(
        directory.line_state(line()),
        DirectoryLineState::new(line())
    );
    let clean = directory.accept(read_shared(3, 0)).unwrap();
    assert_eq!(clean.snoops(), &[]);
    assert_eq!(
        clean.grant(),
        Some(&grant(
            id(3, 0),
            MsiState::Shared,
            DirectoryDataSource::BackingMemory,
        ))
    );
}
