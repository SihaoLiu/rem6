use rem6_directory::{
    MesiDirectory, MesiDirectoryDataSource, MesiDirectoryDecision, MesiDirectoryError,
    MesiDirectoryGrant, MesiDirectoryLineState, MesiDirectorySnoop,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAtomicOp, MemoryRequest,
    MemoryRequestId,
};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn line() -> MesiLineId {
    MesiLineId::new(Address::new(0x3000))
}

fn id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn read_shared(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        id(agent, sequence),
        Address::new(0x3000),
        line_size(),
        layout(),
    )
    .unwrap()
}

fn read_unique(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_unique(
        id(agent, sequence),
        Address::new(0x3000),
        line_size(),
        layout(),
    )
    .unwrap()
}

fn atomic_no_return(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::atomic_no_return(
        id(agent, sequence),
        Address::new(0x3000),
        AccessSize::new(8).unwrap(),
        MemoryAtomicOp::Swap,
        vec![agent as u8; 8],
        ByteMask::full(AccessSize::new(8).unwrap()).unwrap(),
        layout(),
    )
    .unwrap()
}

fn upgrade(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::upgrade(
        id(agent, sequence),
        Address::new(0x3000),
        AccessSize::new(1).unwrap(),
        layout(),
    )
    .unwrap()
}

fn writeback_dirty(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::writeback_dirty(
        id(agent, sequence),
        Address::new(0x3000),
        (0..64).collect(),
        layout(),
    )
    .unwrap()
}

fn write_clean(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::write_clean(
        id(agent, sequence),
        Address::new(0x3000),
        (0..64).collect(),
        layout(),
    )
    .unwrap()
}

fn clean_shared(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::clean_shared(id(agent, sequence), Address::new(0x3000), layout()).unwrap()
}

fn line_size() -> AccessSize {
    AccessSize::new(64).unwrap()
}

fn grant(
    request: MemoryRequestId,
    state: MesiState,
    source: MesiDirectoryDataSource,
) -> MesiDirectoryGrant {
    MesiDirectoryGrant::new(request, line(), state, source)
}

#[test]
fn mesi_directory_grants_exclusive_for_first_reader_and_downgrades_on_peer_read() {
    let mut directory = MesiDirectory::new();

    let first = directory.accept(read_shared(1, 0)).unwrap();
    assert_eq!(
        first,
        MesiDirectoryDecision::new(
            line(),
            id(1, 0),
            MesiDirectoryLineState::new(line()),
            MesiDirectoryLineState::new(line()).with_owner(AgentId::new(1), MesiState::Exclusive),
            Vec::new(),
            Some(grant(
                id(1, 0),
                MesiState::Exclusive,
                MesiDirectoryDataSource::BackingMemory,
            )),
        )
    );

    let second = directory.accept(read_shared(2, 0)).unwrap();
    assert_eq!(
        second.snoops(),
        &[MesiDirectorySnoop::new(
            AgentId::new(1),
            MesiEvent::SnoopRead,
        )]
    );
    assert_eq!(
        second.grant(),
        Some(&grant(
            id(2, 0),
            MesiState::Shared,
            MesiDirectoryDataSource::OwnedCache(AgentId::new(1)),
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MesiDirectoryLineState::new(line())
            .with_sharer(AgentId::new(1))
            .with_sharer(AgentId::new(2))
    );
    directory
        .line_state(line())
        .protocol_snapshot()
        .validate()
        .unwrap();
}

#[test]
fn mesi_directory_read_unique_invalidates_clean_sharers_deterministically() {
    let mut directory = MesiDirectory::new();
    directory.accept(read_shared(3, 0)).unwrap();
    directory.accept(read_shared(1, 0)).unwrap();

    let decision = directory.accept(read_unique(2, 0)).unwrap();

    assert_eq!(
        decision.snoops(),
        &[
            MesiDirectorySnoop::new(AgentId::new(1), MesiEvent::SnoopWrite),
            MesiDirectorySnoop::new(AgentId::new(3), MesiEvent::SnoopWrite),
        ]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 0),
            MesiState::Modified,
            MesiDirectoryDataSource::BackingMemory,
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MesiDirectoryLineState::new(line()).with_owner(AgentId::new(2), MesiState::Modified)
    );
}

#[test]
fn mesi_directory_atomic_no_return_invalidates_clean_sharers_deterministically() {
    let mut directory = MesiDirectory::new();
    directory.accept(read_shared(3, 0)).unwrap();
    directory.accept(read_shared(1, 0)).unwrap();

    let decision = directory.accept(atomic_no_return(2, 0)).unwrap();

    assert_eq!(
        decision.snoops(),
        &[
            MesiDirectorySnoop::new(AgentId::new(1), MesiEvent::SnoopWrite),
            MesiDirectorySnoop::new(AgentId::new(3), MesiEvent::SnoopWrite),
        ]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(2, 0),
            MesiState::Modified,
            MesiDirectoryDataSource::BackingMemory,
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MesiDirectoryLineState::new(line()).with_owner(AgentId::new(2), MesiState::Modified)
    );
}

#[test]
fn mesi_directory_upgrade_requires_existing_sharer_and_invalidates_peers() {
    let mut directory = MesiDirectory::new();
    directory.accept(read_shared(1, 0)).unwrap();
    directory.accept(read_shared(3, 0)).unwrap();

    let miss = directory.accept(upgrade(2, 0)).unwrap_err();
    assert_eq!(
        miss,
        MesiDirectoryError::UpgradeRequesterNotSharer {
            line: line(),
            requester: AgentId::new(2),
        }
    );

    let decision = directory.accept(upgrade(3, 0)).unwrap();
    assert_eq!(
        decision.snoops(),
        &[MesiDirectorySnoop::new(
            AgentId::new(1),
            MesiEvent::SnoopWrite,
        )]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(3, 0),
            MesiState::Modified,
            MesiDirectoryDataSource::NoData,
        ))
    );
    assert_eq!(
        directory.line_state(line()),
        MesiDirectoryLineState::new(line()).with_owner(AgentId::new(3), MesiState::Modified)
    );
}

#[test]
fn mesi_directory_dirty_writeback_clears_modified_owner_and_rejects_non_owner() {
    let mut directory = MesiDirectory::new();
    directory.accept(read_unique(1, 0)).unwrap();

    let non_owner = directory.accept(writeback_dirty(2, 0)).unwrap_err();
    assert_eq!(
        non_owner,
        MesiDirectoryError::WritebackFromNonOwner {
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
        MesiDirectoryLineState::new(line())
    );
}

#[test]
fn mesi_directory_write_clean_keeps_holder_as_clean_sharer() {
    let mut directory = MesiDirectory::new();
    directory.accept(read_unique(1, 0)).unwrap();

    let non_holder = directory.accept(write_clean(2, 0)).unwrap_err();
    assert_eq!(
        non_holder,
        MesiDirectoryError::EvictFromNonHolder {
            line: line(),
            requester: AgentId::new(2),
        }
    );

    let decision = directory.accept(write_clean(1, 1)).unwrap();
    let after = MesiDirectoryLineState::new(line()).with_sharer(AgentId::new(1));

    assert_eq!(decision.snoops(), &[]);
    assert_eq!(decision.grant(), None);
    assert_eq!(
        decision.before(),
        &MesiDirectoryLineState::new(line()).with_owner(AgentId::new(1), MesiState::Modified)
    );
    assert_eq!(decision.after(), &after);
    assert_eq!(directory.line_state(line()), after);
}

#[test]
fn mesi_directory_clean_shared_keeps_holder_as_clean_sharer() {
    let mut directory = MesiDirectory::new();
    directory.accept(read_unique(1, 0)).unwrap();

    let non_holder = directory.accept(clean_shared(2, 0)).unwrap_err();
    assert_eq!(
        non_holder,
        MesiDirectoryError::EvictFromNonHolder {
            line: line(),
            requester: AgentId::new(2),
        }
    );

    let decision = directory.accept(clean_shared(1, 1)).unwrap();
    let after = MesiDirectoryLineState::new(line()).with_sharer(AgentId::new(1));

    assert_eq!(decision.snoops(), &[]);
    assert_eq!(decision.grant(), None);
    assert_eq!(
        decision.before(),
        &MesiDirectoryLineState::new(line()).with_owner(AgentId::new(1), MesiState::Modified)
    );
    assert_eq!(decision.after(), &after);
    assert_eq!(directory.line_state(line()), after);
}

#[test]
fn mesi_directory_restores_line_state_for_later_requests() {
    let mut directory = MesiDirectory::new();
    let snapshot =
        MesiDirectoryLineState::new(line()).with_owner(AgentId::new(2), MesiState::Modified);

    directory.restore_line_state(&snapshot).unwrap();
    assert_eq!(directory.line_state(line()), snapshot);

    let decision = directory.accept(read_shared(1, 2)).unwrap();
    assert_eq!(
        decision.snoops(),
        &[MesiDirectorySnoop::new(
            AgentId::new(2),
            MesiEvent::SnoopRead,
        )]
    );
    assert_eq!(
        decision.grant(),
        Some(&grant(
            id(1, 2),
            MesiState::Shared,
            MesiDirectoryDataSource::OwnedCache(AgentId::new(2)),
        ))
    );

    directory
        .restore_line_state(&MesiDirectoryLineState::new(line()))
        .unwrap();
    assert_eq!(
        directory.line_state(line()),
        MesiDirectoryLineState::new(line())
    );
    let clean = directory.accept(read_shared(3, 0)).unwrap();
    assert_eq!(clean.snoops(), &[]);
    assert_eq!(
        clean.grant(),
        Some(&grant(
            id(3, 0),
            MesiState::Exclusive,
            MesiDirectoryDataSource::BackingMemory,
        ))
    );
}

#[test]
fn mesi_directory_restore_rejects_invalid_owner_state() {
    let mut directory = MesiDirectory::new();
    let snapshot =
        MesiDirectoryLineState::new(line()).with_owner(AgentId::new(2), MesiState::Shared);

    assert_eq!(
        directory.restore_line_state(&snapshot).unwrap_err(),
        MesiDirectoryError::InvalidSnapshotOwnerState {
            line: line(),
            state: MesiState::Shared,
        }
    );
    assert_eq!(
        directory.line_state(line()),
        MesiDirectoryLineState::new(line())
    );
}
