use rem6_memory::{Address, AgentId};
use rem6_protocol_chi::{
    ChiAction, ChiCacheLine, ChiError, ChiEvent, ChiLineId, ChiReservationInvalidation,
    ChiReservationInvalidationReason, ChiReservationTable, ChiState, ChiTraceEntry,
    DirectoryLineSnapshot,
};

fn agent(index: u32) -> AgentId {
    AgentId::new(index)
}

fn line() -> ChiLineId {
    ChiLineId::new(Address::new(0x6000))
}

#[test]
fn chi_read_unique_upgrade_and_snoop_paths_record_ordered_transitions() {
    let mut cache = ChiCacheLine::new(agent(0), line());

    let read = cache.apply(ChiEvent::CpuRead).unwrap();
    assert_eq!(cache.state(), ChiState::InvalidToSharedClean);
    assert_eq!(
        read.actions(),
        &[ChiAction::SendReadShared { line: line() }]
    );

    let read_fill = cache.apply(ChiEvent::CompDataSharedClean).unwrap();
    assert_eq!(cache.state(), ChiState::SharedClean);
    assert_eq!(
        read_fill.actions(),
        &[
            ChiAction::InstallSharedClean { line: line() },
            ChiAction::WakeRequester { line: line() },
        ]
    );

    let upgrade = cache.apply(ChiEvent::CpuWrite).unwrap();
    assert_eq!(cache.state(), ChiState::SharedCleanToUniqueClean);
    assert_eq!(
        upgrade.actions(),
        &[ChiAction::SendMakeReadUnique { line: line() }]
    );

    let upgrade_fill = cache.apply(ChiEvent::CompDataUniqueClean).unwrap();
    assert_eq!(cache.state(), ChiState::UniqueClean);
    assert_eq!(
        upgrade_fill.actions(),
        &[
            ChiAction::InstallUniqueClean { line: line() },
            ChiAction::WakeRequester { line: line() },
        ]
    );

    let store_hit = cache.apply(ChiEvent::CpuWrite).unwrap();
    assert_eq!(cache.state(), ChiState::UniqueDirty);
    assert_eq!(store_hit.actions(), &[ChiAction::WriteHit { line: line() }]);

    let snoop_read = cache.apply(ChiEvent::SnoopShared).unwrap();
    assert_eq!(cache.state(), ChiState::SharedClean);
    assert_eq!(
        snoop_read.actions(),
        &[
            ChiAction::SnoopData { line: line() },
            ChiAction::DowngradeToSharedClean { line: line() },
        ]
    );

    let snoop_unique = cache.apply(ChiEvent::SnoopUnique).unwrap();
    assert_eq!(cache.state(), ChiState::Invalid);
    assert_eq!(
        snoop_unique.actions(),
        &[ChiAction::Invalidate { line: line() }]
    );

    assert_eq!(
        cache.trace(),
        &[
            ChiTraceEntry::new(
                agent(0),
                line(),
                ChiState::Invalid,
                ChiEvent::CpuRead,
                ChiState::InvalidToSharedClean,
            ),
            ChiTraceEntry::new(
                agent(0),
                line(),
                ChiState::InvalidToSharedClean,
                ChiEvent::CompDataSharedClean,
                ChiState::SharedClean,
            ),
            ChiTraceEntry::new(
                agent(0),
                line(),
                ChiState::SharedClean,
                ChiEvent::CpuWrite,
                ChiState::SharedCleanToUniqueClean,
            ),
            ChiTraceEntry::new(
                agent(0),
                line(),
                ChiState::SharedCleanToUniqueClean,
                ChiEvent::CompDataUniqueClean,
                ChiState::UniqueClean,
            ),
            ChiTraceEntry::new(
                agent(0),
                line(),
                ChiState::UniqueClean,
                ChiEvent::CpuWrite,
                ChiState::UniqueDirty,
            ),
            ChiTraceEntry::new(
                agent(0),
                line(),
                ChiState::UniqueDirty,
                ChiEvent::SnoopShared,
                ChiState::SharedClean,
            ),
            ChiTraceEntry::new(
                agent(0),
                line(),
                ChiState::SharedClean,
                ChiEvent::SnoopUnique,
                ChiState::Invalid,
            ),
        ]
    );
}

#[test]
fn chi_rejects_busy_unexpected_and_forced_transient_states() {
    let mut cache = ChiCacheLine::new(agent(1), line());

    cache.apply(ChiEvent::CpuRead).unwrap();
    assert_eq!(
        cache.apply(ChiEvent::CpuWrite).unwrap_err(),
        ChiError::Busy {
            agent: agent(1),
            line: line(),
            state: ChiState::InvalidToSharedClean,
            event: ChiEvent::CpuWrite,
        }
    );

    let mut invalid_cache = ChiCacheLine::new(agent(2), line());
    assert_eq!(
        invalid_cache
            .apply(ChiEvent::CompDataUniqueClean)
            .unwrap_err(),
        ChiError::UnexpectedEvent {
            agent: agent(2),
            line: line(),
            state: ChiState::Invalid,
            event: ChiEvent::CompDataUniqueClean,
        }
    );

    assert_eq!(
        invalid_cache
            .force_state(ChiState::InvalidToUniqueDirty)
            .unwrap_err(),
        ChiError::ForcedTransientState {
            agent: agent(2),
            line: line(),
            state: ChiState::InvalidToUniqueDirty,
        }
    );
}

#[test]
fn chi_directory_snapshot_rejects_multiple_unique_or_unique_with_sharers() {
    DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), ChiState::UniqueDirty)
        .validate()
        .unwrap();

    DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), ChiState::SharedClean)
        .with_cache(agent(1), ChiState::SharedDirty)
        .validate()
        .unwrap();

    let unique_with_sharer = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), ChiState::UniqueClean)
        .with_cache(agent(1), ChiState::SharedClean);
    assert_eq!(
        unique_with_sharer.validate().unwrap_err(),
        ChiError::UniqueWithSharers {
            line: line(),
            owner: agent(0),
            sharers: vec![agent(1)],
        }
    );

    let multiple_unique = DirectoryLineSnapshot::new(line())
        .with_cache(agent(0), ChiState::UniqueDirty)
        .with_cache(agent(1), ChiState::UniqueClean);
    assert_eq!(
        multiple_unique.validate().unwrap_err(),
        ChiError::MultipleUniqueOwners {
            line: line(),
            owners: vec![agent(0), agent(1)],
        }
    );
}

#[test]
fn chi_reservations_serialize_store_conditionals_for_gem5_issue_2688() {
    let mut reservations = ChiReservationTable::default();
    let address = Address::new(0x6008);
    let size = rem6_memory::AccessSize::new(8).unwrap();

    reservations.reserve(agent(0), line(), address, size);
    reservations.reserve(agent(1), line(), address, size);

    let first = reservations.store_conditional(agent(0), line(), address, size);
    assert!(first.succeeded());
    assert_eq!(
        first.invalidations(),
        &[ChiReservationInvalidation::new(
            agent(1),
            line(),
            address,
            size,
            ChiReservationInvalidationReason::StoreConditionalSuccess,
        )]
    );
    assert!(!reservations.is_reserved(agent(0), line()));
    assert!(!reservations.is_reserved(agent(1), line()));

    let second = reservations.store_conditional(agent(1), line(), address, size);
    assert!(!second.succeeded());
    assert!(second.invalidations().is_empty());
}

#[test]
fn chi_reservations_clear_on_coherence_invalidations() {
    let mut reservations = ChiReservationTable::default();
    let address = Address::new(0x6008);
    let size = rem6_memory::AccessSize::new(4).unwrap();

    reservations.reserve(agent(1), line(), address, size);
    assert_eq!(
        reservations.invalidate_overlapping(
            line(),
            address,
            size,
            ChiReservationInvalidationReason::RemoteAtomic
        ),
        vec![ChiReservationInvalidation::new(
            agent(1),
            line(),
            address,
            size,
            ChiReservationInvalidationReason::RemoteAtomic,
        )]
    );
    assert!(!reservations.is_reserved(agent(1), line()));

    reservations.reserve(agent(1), line(), address, size);
    assert_eq!(
        reservations.discard(agent(1), line(), ChiReservationInvalidationReason::Eviction),
        Some(ChiReservationInvalidation::new(
            agent(1),
            line(),
            address,
            size,
            ChiReservationInvalidationReason::Eviction,
        ))
    );
    assert!(!reservations.is_reserved(agent(1), line()));
}
