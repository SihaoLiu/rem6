use rem6_memory::{Address, AgentId};
use rem6_protocol_msi::{
    DirectoryLineSnapshot, MsiAction, MsiCacheLine, MsiError, MsiEvent, MsiLineId, MsiState,
    MsiTraceEntry,
};

fn agent(index: u32) -> AgentId {
    AgentId::new(index)
}

fn line() -> MsiLineId {
    MsiLineId::new(Address::new(0x1000))
}

#[test]
fn msi_replays_cpu_read_and_write_miss_sequence() {
    let mut cache = MsiCacheLine::new(agent(0), line());

    assert_eq!(cache.state(), MsiState::Invalid);

    let read_miss = cache.apply(MsiEvent::CpuRead).unwrap();
    assert_eq!(cache.state(), MsiState::InvalidToShared);
    assert_eq!(
        read_miss.actions(),
        &[MsiAction::SendGetShared { line: line() }]
    );

    let shared_fill = cache.apply(MsiEvent::DataShared).unwrap();
    assert_eq!(cache.state(), MsiState::Shared);
    assert_eq!(
        shared_fill.actions(),
        &[
            MsiAction::InstallShared { line: line() },
            MsiAction::WakeRequester { line: line() },
        ]
    );

    let read_hit = cache.apply(MsiEvent::CpuRead).unwrap();
    assert_eq!(cache.state(), MsiState::Shared);
    assert_eq!(read_hit.actions(), &[MsiAction::ReadHit { line: line() }]);

    let write_miss = cache.apply(MsiEvent::CpuWrite).unwrap();
    assert_eq!(cache.state(), MsiState::SharedToModified);
    assert_eq!(
        write_miss.actions(),
        &[MsiAction::SendGetModified { line: line() }]
    );

    let modified_fill = cache.apply(MsiEvent::DataModified).unwrap();
    assert_eq!(cache.state(), MsiState::Modified);
    assert_eq!(
        modified_fill.actions(),
        &[
            MsiAction::InstallModified { line: line() },
            MsiAction::WakeRequester { line: line() },
        ]
    );

    let write_hit = cache.apply(MsiEvent::CpuWrite).unwrap();
    assert_eq!(cache.state(), MsiState::Modified);
    assert_eq!(write_hit.actions(), &[MsiAction::WriteHit { line: line() }]);

    assert_eq!(
        cache.trace(),
        &[
            MsiTraceEntry::new(
                agent(0),
                line(),
                MsiState::Invalid,
                MsiEvent::CpuRead,
                MsiState::InvalidToShared
            ),
            MsiTraceEntry::new(
                agent(0),
                line(),
                MsiState::InvalidToShared,
                MsiEvent::DataShared,
                MsiState::Shared
            ),
            MsiTraceEntry::new(
                agent(0),
                line(),
                MsiState::Shared,
                MsiEvent::CpuRead,
                MsiState::Shared
            ),
            MsiTraceEntry::new(
                agent(0),
                line(),
                MsiState::Shared,
                MsiEvent::CpuWrite,
                MsiState::SharedToModified
            ),
            MsiTraceEntry::new(
                agent(0),
                line(),
                MsiState::SharedToModified,
                MsiEvent::DataModified,
                MsiState::Modified
            ),
            MsiTraceEntry::new(
                agent(0),
                line(),
                MsiState::Modified,
                MsiEvent::CpuWrite,
                MsiState::Modified
            ),
        ]
    );
}

#[test]
fn msi_snoop_read_downgrades_modified_owner() {
    let mut cache = MsiCacheLine::new(agent(1), line());

    cache.force_state(MsiState::Modified).unwrap();
    let result = cache.apply(MsiEvent::SnoopRead).unwrap();

    assert_eq!(cache.state(), MsiState::Shared);
    assert_eq!(
        result.actions(),
        &[
            MsiAction::SupplyData { line: line() },
            MsiAction::DowngradeToShared { line: line() },
        ]
    );
}

#[test]
fn msi_snoop_write_invalidates_valid_copies() {
    let mut shared = MsiCacheLine::new(agent(2), line());
    shared.force_state(MsiState::Shared).unwrap();
    let shared_result = shared.apply(MsiEvent::SnoopWrite).unwrap();
    assert_eq!(shared.state(), MsiState::Invalid);
    assert_eq!(
        shared_result.actions(),
        &[MsiAction::Invalidate { line: line() }]
    );

    let mut modified = MsiCacheLine::new(agent(3), line());
    modified.force_state(MsiState::Modified).unwrap();
    let modified_result = modified.apply(MsiEvent::SnoopWrite).unwrap();
    assert_eq!(modified.state(), MsiState::Invalid);
    assert_eq!(
        modified_result.actions(),
        &[
            MsiAction::SupplyData { line: line() },
            MsiAction::Invalidate { line: line() },
        ]
    );
}

#[test]
fn msi_transient_states_reject_new_cpu_requests_until_fill() {
    let mut cache = MsiCacheLine::new(agent(4), line());

    cache.apply(MsiEvent::CpuRead).unwrap();
    assert_eq!(
        cache.apply(MsiEvent::CpuWrite).unwrap_err(),
        MsiError::Busy {
            agent: agent(4),
            line: line(),
            state: MsiState::InvalidToShared,
            event: MsiEvent::CpuWrite,
        }
    );

    assert_eq!(
        cache.apply(MsiEvent::DataModified).unwrap_err(),
        MsiError::UnexpectedEvent {
            agent: agent(4),
            line: line(),
            state: MsiState::InvalidToShared,
            event: MsiEvent::DataModified,
        }
    );
}

#[test]
fn msi_replay_stops_on_first_invalid_transition() {
    let replay = MsiCacheLine::replay(
        agent(5),
        line(),
        &[
            MsiEvent::CpuWrite,
            MsiEvent::DataShared,
            MsiEvent::DataModified,
        ],
    )
    .unwrap_err();

    assert_eq!(
        replay,
        MsiError::UnexpectedEvent {
            agent: agent(5),
            line: line(),
            state: MsiState::InvalidToModified,
            event: MsiEvent::DataShared,
        }
    );
}

#[test]
fn msi_directory_snapshot_checks_single_writer_invariant() {
    let ok_shared = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MsiState::Shared)
        .with_cache(agent(1), MsiState::Shared);
    ok_shared.validate().unwrap();

    let ok_modified = DirectoryLineSnapshot::new(line()).with_cache(agent(2), MsiState::Modified);
    ok_modified.validate().unwrap();

    let mixed = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MsiState::Shared)
        .with_cache(agent(1), MsiState::Modified);
    assert_eq!(
        mixed.validate().unwrap_err(),
        MsiError::ModifiedWithSharers {
            line: line(),
            modified: agent(1),
            sharers: vec![agent(0)],
        }
    );

    let multiple = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), MsiState::Modified)
        .with_cache(agent(1), MsiState::Modified);
    assert_eq!(
        multiple.validate().unwrap_err(),
        MsiError::MultipleModifiedOwners {
            line: line(),
            owners: vec![agent(0), agent(1)],
        }
    );
}
