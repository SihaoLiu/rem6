use rem6_memory::{Address, AgentId};
use rem6_protocol_moesi::{
    DirectoryLineSnapshot, MoesiAction, MoesiCacheLine, MoesiError, MoesiEvent, MoesiLineId,
    MoesiState, MoesiTraceEntry,
};

fn agent(index: u32) -> AgentId {
    AgentId::new(index)
}

fn line() -> MoesiLineId {
    MoesiLineId::new(Address::new(0x4000))
}

#[test]
fn moesi_shared_read_of_modified_line_keeps_dirty_owner() {
    let mut cache = MoesiCacheLine::new(agent(0), line());
    cache.force_state(MoesiState::Modified).unwrap();

    let transition = cache.apply(MoesiEvent::SnoopRead).unwrap();

    assert_eq!(cache.state(), MoesiState::Owned);
    assert_eq!(
        transition.actions(),
        &[
            MoesiAction::SupplyData { line: line() },
            MoesiAction::DowngradeToOwned { line: line() },
        ]
    );
}

#[test]
fn moesi_owned_line_services_reads_and_transfers_dirty_data_on_exclusive_request() {
    let mut cache = MoesiCacheLine::new(agent(1), line());
    cache.force_state(MoesiState::Owned).unwrap();

    let read_probe = cache.apply(MoesiEvent::SnoopRead).unwrap();
    assert_eq!(cache.state(), MoesiState::Owned);
    assert_eq!(
        read_probe.actions(),
        &[MoesiAction::SupplyData { line: line() }]
    );

    let write_probe = cache.apply(MoesiEvent::SnoopWrite).unwrap();
    assert_eq!(cache.state(), MoesiState::Invalid);
    assert_eq!(
        write_probe.actions(),
        &[
            MoesiAction::SupplyData { line: line() },
            MoesiAction::Invalidate { line: line() },
        ]
    );
}

#[test]
fn moesi_owned_store_requires_upgrade_before_modified_install() {
    let mut cache = MoesiCacheLine::new(agent(2), line());
    cache.force_state(MoesiState::Owned).unwrap();

    let store = cache.apply(MoesiEvent::CpuWrite).unwrap();
    assert_eq!(cache.state(), MoesiState::OwnedToModified);
    assert_eq!(
        store.actions(),
        &[MoesiAction::SendGetModified { line: line() }]
    );

    let fill = cache.apply(MoesiEvent::DataModified).unwrap();
    assert_eq!(cache.state(), MoesiState::Modified);
    assert_eq!(
        fill.actions(),
        &[
            MoesiAction::InstallModified { line: line() },
            MoesiAction::WakeRequester { line: line() },
        ]
    );
}

#[test]
fn moesi_private_read_can_install_exclusive_then_silent_upgrade() {
    let mut cache = MoesiCacheLine::new(agent(3), line());

    let read = cache.apply(MoesiEvent::CpuRead).unwrap();
    assert_eq!(cache.state(), MoesiState::InvalidToExclusive);
    assert_eq!(
        read.actions(),
        &[MoesiAction::SendGetShared { line: line() }]
    );

    let fill = cache.apply(MoesiEvent::DataExclusive).unwrap();
    assert_eq!(cache.state(), MoesiState::Exclusive);
    assert_eq!(
        fill.actions(),
        &[
            MoesiAction::InstallExclusive { line: line() },
            MoesiAction::WakeRequester { line: line() },
        ]
    );

    let store = cache.apply(MoesiEvent::CpuWrite).unwrap();
    assert_eq!(cache.state(), MoesiState::Modified);
    assert_eq!(
        store.actions(),
        &[
            MoesiAction::SilentUpgrade { line: line() },
            MoesiAction::WriteHit { line: line() },
        ]
    );
}

#[test]
fn moesi_transient_states_reject_new_cpu_requests_until_fill() {
    let mut cache = MoesiCacheLine::new(agent(4), line());

    cache.apply(MoesiEvent::CpuRead).unwrap();
    assert_eq!(
        cache.apply(MoesiEvent::CpuWrite).unwrap_err(),
        MoesiError::Busy {
            agent: agent(4),
            line: line(),
            state: MoesiState::InvalidToExclusive,
            event: MoesiEvent::CpuWrite,
        }
    );
}

#[test]
fn moesi_replay_records_ordered_transitions() {
    let cache = MoesiCacheLine::replay(
        agent(5),
        line(),
        &[
            MoesiEvent::CpuRead,
            MoesiEvent::DataExclusive,
            MoesiEvent::CpuWrite,
            MoesiEvent::SnoopRead,
        ],
    )
    .unwrap();

    assert_eq!(cache.state(), MoesiState::Owned);
    assert_eq!(
        cache.trace(),
        &[
            MoesiTraceEntry::new(
                agent(5),
                line(),
                MoesiState::Invalid,
                MoesiEvent::CpuRead,
                MoesiState::InvalidToExclusive,
            ),
            MoesiTraceEntry::new(
                agent(5),
                line(),
                MoesiState::InvalidToExclusive,
                MoesiEvent::DataExclusive,
                MoesiState::Exclusive,
            ),
            MoesiTraceEntry::new(
                agent(5),
                line(),
                MoesiState::Exclusive,
                MoesiEvent::CpuWrite,
                MoesiState::Modified,
            ),
            MoesiTraceEntry::new(
                agent(5),
                line(),
                MoesiState::Modified,
                MoesiEvent::SnoopRead,
                MoesiState::Owned,
            ),
        ]
    );
}

#[test]
fn moesi_directory_snapshot_allows_one_dirty_owner_with_clean_sharers() {
    DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MoesiState::Owned)
        .with_cache(agent(1), MoesiState::Shared)
        .with_cache(agent(2), MoesiState::Shared)
        .validate()
        .unwrap();

    let multiple_dirty = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MoesiState::Owned)
        .with_cache(agent(1), MoesiState::Modified);
    assert_eq!(
        multiple_dirty.validate().unwrap_err(),
        MoesiError::MultipleDirtyOwners {
            line: line(),
            owners: vec![agent(0), agent(1)],
        }
    );

    let multiple_private = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MoesiState::Exclusive)
        .with_cache(agent(1), MoesiState::Modified);
    assert_eq!(
        multiple_private.validate().unwrap_err(),
        MoesiError::MultipleOwners {
            line: line(),
            owners: vec![agent(0), agent(1)],
        }
    );

    let exclusive_with_sharer = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MoesiState::Exclusive)
        .with_cache(agent(1), MoesiState::Shared);
    assert_eq!(
        exclusive_with_sharer.validate().unwrap_err(),
        MoesiError::ExclusiveWithSharers {
            line: line(),
            owner: agent(0),
            sharers: vec![agent(1)],
        }
    );

    let modified_with_sharer = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MoesiState::Modified)
        .with_cache(agent(1), MoesiState::Shared);
    assert_eq!(
        modified_with_sharer.validate().unwrap_err(),
        MoesiError::ModifiedWithSharers {
            line: line(),
            owner: agent(0),
            sharers: vec![agent(1)],
        }
    );
}
