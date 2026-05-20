use rem6_memory::{Address, AgentId};
use rem6_protocol_mesi::{
    DirectoryLineSnapshot, MesiAction, MesiCacheLine, MesiError, MesiEvent, MesiLineId, MesiState,
    MesiTraceEntry,
};

fn agent(index: u32) -> AgentId {
    AgentId::new(index)
}

fn line() -> MesiLineId {
    MesiLineId::new(Address::new(0x2000))
}

#[test]
fn mesi_read_miss_can_install_exclusive_then_silent_store_upgrade() {
    let mut cache = MesiCacheLine::new(agent(0), line());

    let read_miss = cache.apply(MesiEvent::CpuRead).unwrap();
    assert_eq!(cache.state(), MesiState::InvalidToExclusive);
    assert_eq!(
        read_miss.actions(),
        &[MesiAction::SendGetShared { line: line() }]
    );

    let exclusive_fill = cache.apply(MesiEvent::DataExclusive).unwrap();
    assert_eq!(cache.state(), MesiState::Exclusive);
    assert_eq!(
        exclusive_fill.actions(),
        &[
            MesiAction::InstallExclusive { line: line() },
            MesiAction::WakeRequester { line: line() },
        ]
    );

    let read_hit = cache.apply(MesiEvent::CpuRead).unwrap();
    assert_eq!(read_hit.actions(), &[MesiAction::ReadHit { line: line() }]);

    let silent_upgrade = cache.apply(MesiEvent::CpuWrite).unwrap();
    assert_eq!(cache.state(), MesiState::Modified);
    assert_eq!(
        silent_upgrade.actions(),
        &[
            MesiAction::SilentUpgrade { line: line() },
            MesiAction::WriteHit { line: line() },
        ]
    );

    assert_eq!(
        cache.trace(),
        &[
            MesiTraceEntry::new(
                agent(0),
                line(),
                MesiState::Invalid,
                MesiEvent::CpuRead,
                MesiState::InvalidToExclusive
            ),
            MesiTraceEntry::new(
                agent(0),
                line(),
                MesiState::InvalidToExclusive,
                MesiEvent::DataExclusive,
                MesiState::Exclusive
            ),
            MesiTraceEntry::new(
                agent(0),
                line(),
                MesiState::Exclusive,
                MesiEvent::CpuRead,
                MesiState::Exclusive
            ),
            MesiTraceEntry::new(
                agent(0),
                line(),
                MesiState::Exclusive,
                MesiEvent::CpuWrite,
                MesiState::Modified
            ),
        ]
    );
}

#[test]
fn mesi_shared_store_requests_upgrade_before_installing_modified() {
    let mut cache = MesiCacheLine::new(agent(1), line());

    cache.force_state(MesiState::Shared).unwrap();
    let store_miss = cache.apply(MesiEvent::CpuWrite).unwrap();
    assert_eq!(cache.state(), MesiState::SharedToModified);
    assert_eq!(
        store_miss.actions(),
        &[MesiAction::SendGetModified { line: line() }]
    );

    let fill = cache.apply(MesiEvent::DataModified).unwrap();
    assert_eq!(cache.state(), MesiState::Modified);
    assert_eq!(
        fill.actions(),
        &[
            MesiAction::InstallModified { line: line() },
            MesiAction::WakeRequester { line: line() },
        ]
    );
}

#[test]
fn mesi_snoop_read_downgrades_owned_lines_to_shared() {
    let mut exclusive = MesiCacheLine::new(agent(2), line());
    exclusive.force_state(MesiState::Exclusive).unwrap();
    let exclusive_result = exclusive.apply(MesiEvent::SnoopRead).unwrap();
    assert_eq!(exclusive.state(), MesiState::Shared);
    assert_eq!(
        exclusive_result.actions(),
        &[
            MesiAction::SupplyData { line: line() },
            MesiAction::DowngradeToShared { line: line() },
        ]
    );

    let mut modified = MesiCacheLine::new(agent(3), line());
    modified.force_state(MesiState::Modified).unwrap();
    let modified_result = modified.apply(MesiEvent::SnoopRead).unwrap();
    assert_eq!(modified.state(), MesiState::Shared);
    assert_eq!(
        modified_result.actions(),
        &[
            MesiAction::SupplyData { line: line() },
            MesiAction::DowngradeToShared { line: line() },
        ]
    );
}

#[test]
fn mesi_snoop_write_invalidates_valid_copies() {
    let mut shared = MesiCacheLine::new(agent(4), line());
    shared.force_state(MesiState::Shared).unwrap();
    let shared_result = shared.apply(MesiEvent::SnoopWrite).unwrap();
    assert_eq!(shared.state(), MesiState::Invalid);
    assert_eq!(
        shared_result.actions(),
        &[MesiAction::Invalidate { line: line() }]
    );

    let mut exclusive = MesiCacheLine::new(agent(5), line());
    exclusive.force_state(MesiState::Exclusive).unwrap();
    let exclusive_result = exclusive.apply(MesiEvent::SnoopWrite).unwrap();
    assert_eq!(exclusive.state(), MesiState::Invalid);
    assert_eq!(
        exclusive_result.actions(),
        &[MesiAction::Invalidate { line: line() }]
    );

    let mut modified = MesiCacheLine::new(agent(6), line());
    modified.force_state(MesiState::Modified).unwrap();
    let modified_result = modified.apply(MesiEvent::SnoopWrite).unwrap();
    assert_eq!(modified.state(), MesiState::Invalid);
    assert_eq!(
        modified_result.actions(),
        &[
            MesiAction::SupplyData { line: line() },
            MesiAction::Invalidate { line: line() },
        ]
    );
}

#[test]
fn mesi_transient_states_reject_new_cpu_requests_until_fill() {
    let mut cache = MesiCacheLine::new(agent(7), line());

    cache.apply(MesiEvent::CpuRead).unwrap();
    assert_eq!(
        cache.apply(MesiEvent::CpuWrite).unwrap_err(),
        MesiError::Busy {
            agent: agent(7),
            line: line(),
            state: MesiState::InvalidToExclusive,
            event: MesiEvent::CpuWrite,
        }
    );

    assert_eq!(
        cache.apply(MesiEvent::DataModified).unwrap_err(),
        MesiError::UnexpectedEvent {
            agent: agent(7),
            line: line(),
            state: MesiState::InvalidToExclusive,
            event: MesiEvent::DataModified,
        }
    );
}

#[test]
fn mesi_replay_stops_on_first_invalid_transition() {
    let replay = MesiCacheLine::replay(
        agent(8),
        line(),
        &[
            MesiEvent::CpuRead,
            MesiEvent::DataExclusive,
            MesiEvent::DataShared,
        ],
    )
    .unwrap_err();

    assert_eq!(
        replay,
        MesiError::UnexpectedEvent {
            agent: agent(8),
            line: line(),
            state: MesiState::Exclusive,
            event: MesiEvent::DataShared,
        }
    );
}

#[test]
fn mesi_directory_snapshot_checks_single_owned_copy_invariant() {
    DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MesiState::Shared)
        .with_cache(agent(1), MesiState::Shared)
        .validate()
        .unwrap();
    DirectoryLineSnapshot::new(line())
        .with_cache(agent(2), MesiState::Exclusive)
        .validate()
        .unwrap();
    DirectoryLineSnapshot::new(line())
        .with_cache(agent(3), MesiState::Modified)
        .validate()
        .unwrap();

    let mixed = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MesiState::Shared)
        .with_cache(agent(1), MesiState::Exclusive);
    assert_eq!(
        mixed.validate().unwrap_err(),
        MesiError::OwnedWithSharers {
            line: line(),
            owner: agent(1),
            sharers: vec![agent(0)],
        }
    );

    let multiple = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MesiState::Exclusive)
        .with_cache(agent(1), MesiState::Modified);
    assert_eq!(
        multiple.validate().unwrap_err(),
        MesiError::MultipleOwnedCopies {
            line: line(),
            owners: vec![agent(0), agent(1)],
        }
    );
}
