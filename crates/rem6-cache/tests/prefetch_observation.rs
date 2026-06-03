use rem6_cache::{
    PrefetchAccessKind, PrefetchObservation, PrefetchObservationConfig,
    PrefetchObservationConfigOptions,
};

fn data_read(miss: bool) -> PrefetchObservation {
    PrefetchObservation::new(PrefetchAccessKind::Read, miss, false)
}

fn instruction_read(miss: bool) -> PrefetchObservation {
    PrefetchObservation::new(PrefetchAccessKind::Read, miss, true)
}

fn data_write(miss: bool) -> PrefetchObservation {
    PrefetchObservation::new(PrefetchAccessKind::Write, miss, false)
}

#[test]
fn prefetch_observation_policy_matches_gem5_base_access_filters() {
    let default_config = PrefetchObservationConfig::default();

    assert!(default_config.should_observe(data_read(true)));
    assert!(!default_config.should_observe(data_read(false)));
    assert!(default_config.should_observe(data_read(false).with_prefetched(true)));
    assert!(default_config.should_observe(instruction_read(true)));

    let on_miss = PrefetchObservationConfig::new(PrefetchObservationConfigOptions {
        on_miss: true,
        ..PrefetchObservationConfigOptions::default()
    });
    assert!(on_miss.should_observe(data_read(true)));
    assert!(!on_miss.should_observe(data_read(false)));
    assert!(on_miss.should_observe(data_read(false).with_prefetched(true)));

    let no_prefetched_hit = PrefetchObservationConfig::new(PrefetchObservationConfigOptions {
        prefetch_on_prefetch_hit: false,
        ..PrefetchObservationConfigOptions::default()
    });
    assert!(!no_prefetched_hit.should_observe(data_read(false).with_prefetched(true)));

    let all_accesses = PrefetchObservationConfig::new(PrefetchObservationConfigOptions {
        prefetch_on_access: true,
        prefetch_on_prefetch_hit: false,
        ..PrefetchObservationConfigOptions::default()
    });
    assert!(all_accesses.should_observe(data_read(false)));

    let on_miss_all_accesses = PrefetchObservationConfig::new(PrefetchObservationConfigOptions {
        on_miss: true,
        prefetch_on_access: true,
        prefetch_on_prefetch_hit: false,
        ..PrefetchObservationConfigOptions::default()
    });
    assert!(!on_miss_all_accesses.should_observe(data_read(false)));

    let data_only = PrefetchObservationConfig::new(PrefetchObservationConfigOptions {
        on_inst: false,
        ..PrefetchObservationConfigOptions::default()
    });
    assert!(!data_only.should_observe(instruction_read(true)));

    let inst_only = PrefetchObservationConfig::new(PrefetchObservationConfigOptions {
        on_data: false,
        ..PrefetchObservationConfigOptions::default()
    });
    assert!(!inst_only.should_observe(data_read(true)));

    let no_reads = PrefetchObservationConfig::new(PrefetchObservationConfigOptions {
        on_read: false,
        ..PrefetchObservationConfigOptions::default()
    });
    assert!(!no_reads.should_observe(data_read(true)));
    assert!(no_reads.should_observe(data_write(true)));

    let no_writes = PrefetchObservationConfig::new(PrefetchObservationConfigOptions {
        on_write: false,
        ..PrefetchObservationConfigOptions::default()
    });
    assert!(!no_writes.should_observe(data_write(true)));

    assert!(!default_config.should_observe(data_read(true).with_uncacheable(true)));
    assert!(!default_config.should_observe(data_read(true).with_software_prefetch(true)));
    assert!(!default_config.should_observe(data_read(true).with_cache_maintenance(true)));
    assert!(!default_config.should_observe(data_read(true).with_clean_eviction(true)));
    assert!(!default_config.should_observe(data_write(true).with_write_coalesced(true)));
    assert!(!default_config.should_observe(PrefetchObservation::new(
        PrefetchAccessKind::Invalidate,
        true,
        false,
    )));
}
