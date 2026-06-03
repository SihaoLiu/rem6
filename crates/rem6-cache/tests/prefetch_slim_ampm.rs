use rem6_cache::{
    AmpmPrefetcherConfig, DcptPrefetcherConfig, SlimAmpmPrefetchAccess, SlimAmpmPrefetchSource,
    SlimAmpmPrefetcher, SlimAmpmPrefetcherConfig,
};
use rem6_memory::{Address, AgentId};

fn access(agent: u32, pc: u64, address: u64) -> SlimAmpmPrefetchAccess {
    SlimAmpmPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn small_config() -> SlimAmpmPrefetcherConfig {
    let ampm = AmpmPrefetcherConfig::new(64, 256, 2, 8)
        .unwrap()
        .with_limit_stride(4)
        .unwrap();
    let dcpt = DcptPrefetcherConfig::new(6, 12, 4, 4, true).unwrap();
    SlimAmpmPrefetcherConfig::new(ampm, dcpt)
}

#[test]
fn slim_ampm_defaults_match_gem5_slim_parameters() {
    let config = SlimAmpmPrefetcherConfig::gem5_defaults(64).unwrap();

    assert_eq!(config.ampm().line_size(), 64);
    assert_eq!(config.ampm().hot_zone_size(), 2048);
    assert_eq!(config.ampm().degree(), 2);
    assert_eq!(config.ampm().limit_stride(), Some(4));
    assert_eq!(config.ampm().table_entries(), 256);
    assert_eq!(config.ampm().table_assoc(), 8);
    assert_eq!(config.ampm().table_sets(), 32);
    assert_eq!(config.dcpt().deltas_per_entry(), 9);
    assert_eq!(config.dcpt().delta_bits(), 12);
    assert_eq!(config.dcpt().delta_mask_bits(), 8);
    assert_eq!(config.dcpt().table_entries(), 256);
    assert_eq!(config.dcpt().table_assoc(), 256);
    assert_eq!(config.dcpt().table_sets(), 1);
    assert!(!config.dcpt().use_requestor_id());
}

#[test]
fn slim_ampm_falls_back_to_ampm_when_dcpt_has_no_candidates() {
    let mut prefetcher = SlimAmpmPrefetcher::new(small_config());

    assert!(prefetcher
        .observe(access(3, 0x700, 0x10c0))
        .unwrap()
        .is_empty());
    let candidates = prefetcher
        .observe(access(3, 0x704, 0x1100))
        .unwrap()
        .to_vec();

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].source(), SlimAmpmPrefetchSource::Ampm);
    assert_eq!(candidates[0].address(), Address::new(0x1140));
    assert_eq!(candidates[0].source_address(), Address::new(0x1100));
    assert_eq!(candidates[0].context(), AgentId::new(3));
    assert_eq!(candidates[0].pc(), 0x704);
    assert_eq!(candidates[0].stride(), 64);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(prefetcher.ampm().issued_prefetch_count(), 1);
    assert_eq!(prefetcher.last_candidates(), candidates.as_slice());
}

#[test]
fn slim_ampm_uses_dcpt_when_dcpt_has_candidates() {
    let mut prefetcher = SlimAmpmPrefetcher::new(small_config());
    let pc = 0x440;

    for address in [0x1000, 0x1041, 0x10c3, 0x1112] {
        prefetcher.observe(access(5, pc, address)).unwrap();
    }
    let ampm_issued_before_dcpt_hit = prefetcher.ampm().issued_prefetch_count();

    let candidates = prefetcher.observe(access(5, pc, 0x11a0)).unwrap().to_vec();

    assert_eq!(
        candidates
            .iter()
            .map(|candidate| (candidate.source(), candidate.address()))
            .collect::<Vec<_>>(),
        vec![
            (SlimAmpmPrefetchSource::Dcpt, Address::new(0x11ef)),
            (SlimAmpmPrefetchSource::Dcpt, Address::new(0x127d))
        ]
    );
    assert_eq!(candidates[0].source_address(), Address::new(0x11a0));
    assert_eq!(candidates[0].context(), AgentId::new(5));
    assert_eq!(candidates[0].pc(), pc);
    assert_eq!(candidates[0].stride(), 0x4f);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(
        prefetcher.ampm().issued_prefetch_count(),
        ampm_issued_before_dcpt_hit
    );
    assert_eq!(prefetcher.dcpt().last_candidates().len(), 2);
}

#[test]
fn slim_ampm_gem5_defaults_share_dcpt_history_across_requestors() {
    let config = SlimAmpmPrefetcherConfig::gem5_defaults(64).unwrap();
    let mut prefetcher = SlimAmpmPrefetcher::new(config);
    let pc = 0x880;

    for (agent, address) in [(5, 0x1000), (5, 0x1041), (5, 0x10c3), (6, 0x1112)] {
        prefetcher.observe(access(agent, pc, address)).unwrap();
    }
    let candidates = prefetcher.observe(access(6, pc, 0x11a0)).unwrap().to_vec();

    assert!(!candidates.is_empty());
    assert!(candidates
        .iter()
        .all(|candidate| candidate.source() == SlimAmpmPrefetchSource::Dcpt));
    assert_eq!(candidates[0].source_address(), Address::new(0x11a0));
    assert_eq!(candidates[0].context(), AgentId::new(0));
    assert_eq!(candidates[0].pc(), pc);
}

#[test]
fn slim_ampm_restores_ampm_and_dcpt_state() {
    let config = small_config();
    let mut prefetcher = SlimAmpmPrefetcher::new(config.clone());

    prefetcher.observe(access(3, 0x700, 0x10c0)).unwrap();
    let candidates = prefetcher
        .observe(access(3, 0x704, 0x1100))
        .unwrap()
        .to_vec();
    assert_eq!(candidates[0].source(), SlimAmpmPrefetchSource::Ampm);

    let dcpt_pc = 0x440;
    for address in [0x1000, 0x1041, 0x10c3, 0x1112, 0x11a0] {
        prefetcher.observe(access(5, dcpt_pc, address)).unwrap();
    }

    let snapshot = prefetcher.snapshot();
    assert!(!snapshot.ampm().entries().is_empty());
    assert!(!snapshot.dcpt().contexts().is_empty());

    let mut restored = SlimAmpmPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let restored_candidates = restored.observe(access(3, 0x708, 0x1140)).unwrap().to_vec();
    assert_eq!(
        restored_candidates
            .iter()
            .map(|candidate| (candidate.source(), candidate.address()))
            .collect::<Vec<_>>(),
        vec![(SlimAmpmPrefetchSource::Ampm, Address::new(0x1180))]
    );
    assert_eq!(restored.ampm().useful_prefetch_count(), 1);

    let dcpt_candidates = restored
        .observe(access(5, dcpt_pc, 0x11ef))
        .unwrap()
        .to_vec();
    assert_eq!(
        dcpt_candidates
            .iter()
            .map(|candidate| (candidate.source(), candidate.address()))
            .collect::<Vec<_>>(),
        vec![
            (SlimAmpmPrefetchSource::Dcpt, Address::new(0x127d)),
            (SlimAmpmPrefetchSource::Dcpt, Address::new(0x12cc))
        ]
    );
}
