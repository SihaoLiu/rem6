use rem6_cache::{
    AmpmEpochConfig, AmpmPrefetchAccess, AmpmPrefetcher, AmpmPrefetcherConfig, AmpmPrefetcherError,
    BopDelayQueueConfig, BopDelayQueueEntrySnapshot, BopPrefetchAccess, BopPrefetcher,
    BopPrefetcherConfig, BopPrefetcherConfigOptions, BopPrefetcherError, DcptPrefetchAccess,
    DcptPrefetcher, DcptPrefetcherConfig, DcptPrefetcherError, QueuedPrefetchConfig,
    QueuedPrefetchDemandAccess, QueuedPrefetchFullPolicy, QueuedPrefetchRedundantLine,
    QueuedPrefetchThrottle, QueuedPrefetchThrottleConfig, QueuedPrefetchThrottleError,
    QueuedPrefetcher, SbooePrefetchAccess, SbooePrefetcher, SbooePrefetcherConfig,
    SbooePrefetcherError, SbooeSandboxEntrySnapshot, SbooeSandboxSnapshot,
    SignaturePathPatternEntrySnapshot, SignaturePathPatternStrideSnapshot,
    SignaturePathPrefetchAccess, SignaturePathPrefetcher, SignaturePathPrefetcherConfig,
    SignaturePathPrefetcherConfigOptions, SignaturePathPrefetcherError, SignaturePathRatio,
    SignaturePathSignatureEntrySnapshot, SmsActiveEntrySnapshot, SmsPatternEntrySnapshot,
    SmsPrefetchAccess, SmsPrefetchCandidate, SmsPrefetcher, SmsPrefetcherConfig,
    SmsPrefetcherError, StridePrefetchAccess, StridePrefetcher, StridePrefetcherConfig,
    TaggedPrefetchAccess, TaggedPrefetcher, TaggedPrefetcherConfig,
};
use rem6_memory::{Address, AgentId};

const U64_VECTOR_BYTE_OVERFLOW_LENGTH: usize = isize::MAX as usize / std::mem::size_of::<u64>() + 1;
const U32_VECTOR_BYTE_OVERFLOW_LENGTH: usize = isize::MAX as usize / std::mem::size_of::<u32>() + 1;
const BOP_DELAY_QUEUE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<BopDelayQueueEntrySnapshot>() + 1;
const I64_VECTOR_BYTE_OVERFLOW_LENGTH: usize = isize::MAX as usize / std::mem::size_of::<i64>() + 1;
const DCPT_CANDIDATE_MIN_BYTES: usize = std::mem::size_of::<Address>() * 2
    + std::mem::size_of::<AgentId>()
    + std::mem::size_of::<u64>()
    + std::mem::size_of::<bool>()
    + std::mem::size_of::<i64>()
    + std::mem::size_of::<u32>();
const DCPT_CANDIDATE_BYTE_OVERFLOW_DELTAS: usize =
    isize::MAX as usize / DCPT_CANDIDATE_MIN_BYTES + 3;
const OVERSIZED_VECTOR_LENGTH: usize = isize::MAX as usize + 1;
const AMPM_ACCESS_MAP_BYTE_OVERFLOW_LINES: u64 = isize::MAX as u64 + 1;
const AMPM_WINDOW_BYTE_OVERFLOW_LINES: u64 = 1_u64 << 62;
const SIGNATURE_PATH_SIGNATURE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<SignaturePathSignatureEntrySnapshot>() + 1;
const SIGNATURE_PATH_PATTERN_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<SignaturePathPatternEntrySnapshot>() + 1;
const SIGNATURE_PATH_STRIDE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<SignaturePathPatternStrideSnapshot>() + 1;
const SBOOE_SANDBOX_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<SbooeSandboxSnapshot>() + 1;
const SBOOE_SANDBOX_ENTRY_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<SbooeSandboxEntrySnapshot>() + 1;
const U64_BYTE_OVERFLOW_LENGTH: usize = isize::MAX as usize / std::mem::size_of::<u64>() + 1;
const SMS_CANDIDATE_BYTE_OVERFLOW_LINES: u64 =
    (isize::MAX as usize / std::mem::size_of::<SmsPrefetchCandidate>() + 1) as u64;
const SMS_CONTEXT_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<SmsActiveEntrySnapshot>() + 1;
const SMS_PATTERN_HISTORY_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<SmsPatternEntrySnapshot>() + 1;

fn access(agent: u32, pc: u64, address: u64) -> StridePrefetchAccess {
    StridePrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn tagged_access(agent: u32, pc: u64, address: u64) -> TaggedPrefetchAccess {
    TaggedPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn ampm_access(agent: u32, pc: u64, address: u64) -> AmpmPrefetchAccess {
    AmpmPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn dcpt_access(agent: u32, pc: u64, address: u64) -> DcptPrefetchAccess {
    DcptPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn bop_access(agent: u32, pc: u64, address: u64) -> BopPrefetchAccess {
    BopPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn bop_options() -> BopPrefetcherConfigOptions {
    BopPrefetcherConfigOptions {
        line_size: 64,
        score_max: 1,
        round_max: 8,
        bad_score: 0,
        rr_entries: 8,
        tag_bits: 12,
        offset_list_size: 1,
        negative_offsets: false,
        degree: 2,
        delay_queue: None,
    }
}

fn sbooe_access(agent: u32, pc: u64, address: u64) -> SbooePrefetchAccess {
    SbooePrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn signature_path_access(agent: u32, pc: u64, address: u64) -> SignaturePathPrefetchAccess {
    SignaturePathPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn sms_access(agent: u32, pc: u64, address: u64) -> SmsPrefetchAccess {
    SmsPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

#[test]
fn bop_prefetcher_learns_best_offset_and_restores_rr_state() {
    let config = BopPrefetcherConfig::new(BopPrefetcherConfigOptions {
        line_size: 64,
        score_max: 1,
        round_max: 8,
        bad_score: 0,
        rr_entries: 8,
        tag_bits: 12,
        offset_list_size: 1,
        negative_offsets: false,
        degree: 2,
        delay_queue: None,
    })
    .unwrap();
    let mut prefetcher = BopPrefetcher::new(config.clone());

    assert_eq!(prefetcher.offsets(), &[1]);
    assert_eq!(prefetcher.best_offset(), 1);
    assert!(!prefetcher.issue_prefetch_requests());
    assert!(prefetcher
        .observe(bop_access(4, 0x900, 0x1000))
        .unwrap()
        .is_empty());

    let candidates = prefetcher
        .observe(bop_access(4, 0x904, 0x1040))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1080), Address::new(0x10c0)]
    );
    assert_eq!(candidates[0].source_address(), Address::new(0x1040));
    assert_eq!(candidates[0].context(), AgentId::new(4));
    assert_eq!(candidates[0].pc(), 0x904);
    assert_eq!(candidates[0].offset(), 1);
    assert_eq!(candidates[0].stride(), 64);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(candidates[1].offset(), 1);
    assert_eq!(candidates[1].stride(), 64);
    assert_eq!(candidates[1].degree_index(), 2);
    assert!(prefetcher.issue_prefetch_requests());
    assert_eq!(prefetcher.best_offset(), 1);
    assert_eq!(prefetcher.best_score(), 0);
    assert_eq!(prefetcher.round(), 0);
    assert_eq!(prefetcher.last_candidates(), candidates.as_slice());

    let snapshot = prefetcher.snapshot();
    let mut restored = BopPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert!(restored.issue_prefetch_requests());

    let restored_candidates = restored
        .observe(bop_access(4, 0x908, 0x1080))
        .unwrap()
        .to_vec();
    assert_eq!(
        restored_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x10c0), Address::new(0x1100)]
    );
    assert_eq!(
        restored_candidates[0].source_address(),
        Address::new(0x1080)
    );
    assert_eq!(restored_candidates[0].offset(), 1);
    assert_eq!(restored_candidates[1].degree_index(), 2);
}

#[test]
fn bop_prefetcher_delays_rr_training_and_restores_delay_queue() {
    let config = BopPrefetcherConfig::new(BopPrefetcherConfigOptions {
        line_size: 64,
        score_max: 1,
        round_max: 8,
        bad_score: 0,
        rr_entries: 8,
        tag_bits: 12,
        offset_list_size: 1,
        negative_offsets: false,
        degree: 1,
        delay_queue: Some(BopDelayQueueConfig::new(2, 3).unwrap()),
    })
    .unwrap();
    let mut prefetcher = BopPrefetcher::new(config.clone());

    assert!(prefetcher
        .observe_at(0, bop_access(4, 0xa00, 0x1000))
        .unwrap()
        .is_empty());
    assert_eq!(prefetcher.delay_queue_len(), 1);
    assert_eq!(prefetcher.next_delay_ready_tick(), Some(3));

    assert!(prefetcher
        .observe_at(1, bop_access(4, 0xa04, 0x1040))
        .unwrap()
        .is_empty());
    assert!(!prefetcher.issue_prefetch_requests());
    assert_eq!(prefetcher.delay_queue_len(), 2);

    let snapshot = prefetcher.snapshot();
    assert_eq!(snapshot.delay_queue().len(), 2);
    let mut restored = BopPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let candidates = restored
        .observe_at(3, bop_access(4, 0xa08, 0x1040))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1080)]
    );
    assert!(restored.issue_prefetch_requests());
    assert_eq!(restored.delay_queue_len(), 2);
    assert_eq!(restored.next_delay_ready_tick(), Some(4));
}

#[test]
fn bop_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    let mut oversized_rr = bop_options();
    oversized_rr.rr_entries = U64_VECTOR_BYTE_OVERFLOW_LENGTH;
    assert_eq!(
        BopPrefetcherConfig::new(oversized_rr),
        Err(BopPrefetcherError::VectorLengthTooLarge {
            field: "RR entries",
            length: U64_VECTOR_BYTE_OVERFLOW_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<u64>(),
        })
    );

    let mut oversized_offsets = bop_options();
    oversized_offsets.offset_list_size = U32_VECTOR_BYTE_OVERFLOW_LENGTH;
    assert_eq!(
        BopPrefetcherConfig::new(oversized_offsets),
        Err(BopPrefetcherError::VectorLengthTooLarge {
            field: "offset list size",
            length: U32_VECTOR_BYTE_OVERFLOW_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<u32>(),
        })
    );

    assert_eq!(
        BopDelayQueueConfig::new(BOP_DELAY_QUEUE_BYTE_OVERFLOW_LENGTH, 1),
        Err(BopPrefetcherError::VectorLengthTooLarge {
            field: "delay queue entries",
            length: BOP_DELAY_QUEUE_BYTE_OVERFLOW_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<BopDelayQueueEntrySnapshot>(),
        })
    );
}

#[test]
fn sbooe_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        SbooePrefetcherConfig::new(64, SBOOE_SANDBOX_BYTE_OVERFLOW_LENGTH, 4, 25, 2),
        Err(SbooePrefetcherError::VectorLengthTooLarge {
            field: "sequential prefetchers",
            length: SBOOE_SANDBOX_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert_eq!(
        SbooePrefetcherConfig::new(64, 3, SBOOE_SANDBOX_ENTRY_BYTE_OVERFLOW_LENGTH, 25, 2),
        Err(SbooePrefetcherError::VectorLengthTooLarge {
            field: "sandbox entries",
            length: SBOOE_SANDBOX_ENTRY_BYTE_OVERFLOW_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<SbooeSandboxEntrySnapshot>(),
        })
    );
    assert_eq!(
        SbooePrefetcherConfig::new(64, 3, 4, 25, U64_BYTE_OVERFLOW_LENGTH),
        Err(SbooePrefetcherError::VectorLengthTooLarge {
            field: "latency buffer entries",
            length: U64_BYTE_OVERFLOW_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<u64>(),
        })
    );
}

#[test]
fn sbooe_prefetcher_selects_best_sandbox_stride_and_restores_state() {
    let config = SbooePrefetcherConfig::new(64, 3, 4, 25, 2).unwrap();
    let mut prefetcher = SbooePrefetcher::new(config.clone());

    assert!(prefetcher
        .observe_at(0, sbooe_access(6, 0xb00, 0x1000))
        .unwrap()
        .is_empty());
    assert!(prefetcher
        .observe_at(1, sbooe_access(6, 0xb04, 0x1040))
        .unwrap()
        .is_empty());

    let candidates = prefetcher
        .observe_at(2, sbooe_access(6, 0xb08, 0x1080))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x10c0)]
    );
    assert_eq!(candidates[0].source_address(), Address::new(0x1080));
    assert_eq!(candidates[0].context(), AgentId::new(6));
    assert_eq!(candidates[0].pc(), 0xb08);
    assert_eq!(candidates[0].sandbox_stride(), 1);
    assert_eq!(candidates[0].stride(), 64);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(prefetcher.best_sandbox_stride(), Some(1));
    assert_eq!(prefetcher.sandbox_scores(), vec![0, 0, 2]);
    assert_eq!(prefetcher.last_candidates(), candidates.as_slice());

    let snapshot = prefetcher.snapshot();
    let mut restored = SbooePrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let restored_candidates = restored
        .observe_at(3, sbooe_access(6, 0xb0c, 0x10c0))
        .unwrap()
        .to_vec();
    assert_eq!(
        restored_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1100)]
    );
    assert_eq!(restored_candidates[0].sandbox_stride(), 1);
    assert_eq!(restored.sandbox_scores(), vec![0, 0, 3]);
}

#[test]
fn sbooe_prefetcher_tracks_latency_and_late_sandbox_hits() {
    let config = SbooePrefetcherConfig::new(64, 3, 4, 25, 2).unwrap();
    let mut prefetcher = SbooePrefetcher::new(config);

    assert!(prefetcher
        .observe_at(0, sbooe_access(6, 0xc00, 0x2000))
        .unwrap()
        .is_empty());
    assert_eq!(prefetcher.pending_demand_count(), 1);
    prefetcher.observe_fill_at(10, Address::new(0x2000));
    assert_eq!(prefetcher.average_access_latency(), 10);
    assert_eq!(prefetcher.pending_demand_count(), 0);

    assert!(prefetcher
        .observe_at(20, sbooe_access(6, 0xc04, 0x3000))
        .unwrap()
        .is_empty());
    assert!(prefetcher
        .observe_at(21, sbooe_access(6, 0xc08, 0x3040))
        .unwrap()
        .is_empty());
    assert_eq!(prefetcher.sandbox_raw_scores(), vec![0, 0, 1]);
    assert_eq!(prefetcher.sandbox_late_scores(), vec![0, 0, 1]);
    assert_eq!(prefetcher.sandbox_scores(), vec![0, 0, 0]);
    assert_eq!(prefetcher.best_sandbox_stride(), Some(-1));
    assert_eq!(prefetcher.last_candidates(), &[]);
}

#[test]
fn signature_path_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    let base = SignaturePathPrefetcherConfigOptions {
        line_size: 64,
        page_bytes: 4096,
        signature_shift: 3,
        signature_bits: 12,
        signature_table_entries: 8,
        pattern_table_entries: 8,
        strides_per_pattern_entry: 2,
        counter_bits: 1,
        prefetch_confidence_threshold: SignaturePathRatio::new(1, 1).unwrap(),
        lookahead_confidence_threshold: SignaturePathRatio::new(3, 4).unwrap(),
    };

    let signature_entries = SignaturePathPrefetcherConfigOptions {
        signature_table_entries: SIGNATURE_PATH_SIGNATURE_BYTE_OVERFLOW_LENGTH,
        ..base
    };
    assert!(matches!(
        SignaturePathPrefetcherConfig::new(signature_entries),
        Err(SignaturePathPrefetcherError::VectorLengthTooLarge {
            field: "signature table entries",
            length: SIGNATURE_PATH_SIGNATURE_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));

    let pattern_entries = SignaturePathPrefetcherConfigOptions {
        pattern_table_entries: SIGNATURE_PATH_PATTERN_BYTE_OVERFLOW_LENGTH,
        ..base
    };
    assert!(matches!(
        SignaturePathPrefetcherConfig::new(pattern_entries),
        Err(SignaturePathPrefetcherError::VectorLengthTooLarge {
            field: "pattern table entries",
            length: SIGNATURE_PATH_PATTERN_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));

    let stride_entries = SignaturePathPrefetcherConfigOptions {
        strides_per_pattern_entry: SIGNATURE_PATH_STRIDE_BYTE_OVERFLOW_LENGTH,
        ..base
    };
    assert!(matches!(
        SignaturePathPrefetcherConfig::new(stride_entries),
        Err(SignaturePathPrefetcherError::VectorLengthTooLarge {
            field: "strides per pattern entry",
            length: SIGNATURE_PATH_STRIDE_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn signature_path_prefetcher_trains_lookahead_and_restores_state() {
    let config = SignaturePathPrefetcherConfig::new(SignaturePathPrefetcherConfigOptions {
        line_size: 64,
        page_bytes: 4096,
        signature_shift: 3,
        signature_bits: 12,
        signature_table_entries: 8,
        pattern_table_entries: 8,
        strides_per_pattern_entry: 2,
        counter_bits: 1,
        prefetch_confidence_threshold: SignaturePathRatio::new(1, 1).unwrap(),
        lookahead_confidence_threshold: SignaturePathRatio::new(3, 4).unwrap(),
    })
    .unwrap();
    let mut prefetcher = SignaturePathPrefetcher::new(config.clone());

    for address in [0x1000, 0x1040, 0x1080, 0x10c0] {
        prefetcher
            .observe(signature_path_access(7, 0xd00, address))
            .unwrap();
    }

    assert!(prefetcher
        .observe(signature_path_access(7, 0xd10, 0x2000))
        .unwrap()
        .is_empty());
    let candidates = prefetcher
        .observe(signature_path_access(7, 0xd14, 0x2040))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x2080), Address::new(0x20c0)]
    );
    assert_eq!(candidates[0].source_address(), Address::new(0x2040));
    assert_eq!(candidates[0].context(), AgentId::new(7));
    assert_eq!(candidates[0].pc(), 0xd14);
    assert_eq!(candidates[0].delta_blocks(), 1);
    assert_eq!(candidates[0].stride(), 64);
    assert_eq!(candidates[0].signature(), 1);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(candidates[0].path_confidence_ppm(), 1_000_000);
    assert_eq!(candidates[1].signature(), 9);
    assert_eq!(candidates[1].degree_index(), 2);
    assert_eq!(candidates[1].path_confidence_ppm(), 950_000);
    assert!(!candidates[0].auxiliary());
    assert_eq!(prefetcher.signature_for_page(2, false), Some(1));
    assert_eq!(prefetcher.last_candidates(), candidates.as_slice());

    let snapshot = prefetcher.snapshot();
    let mut restored = SignaturePathPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let restored_candidates = restored
        .observe(signature_path_access(7, 0xd18, 0x2080))
        .unwrap()
        .to_vec();
    assert_eq!(
        restored_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x20c0)]
    );
    assert_eq!(restored_candidates[0].signature(), 9);
    assert_eq!(restored.signature_for_page(2, false), Some(9));
}

#[test]
fn signature_path_prefetcher_replaces_low_confidence_stride_entries() {
    let config = SignaturePathPrefetcherConfig::new(SignaturePathPrefetcherConfigOptions {
        line_size: 64,
        page_bytes: 4096,
        signature_shift: 3,
        signature_bits: 12,
        signature_table_entries: 4,
        pattern_table_entries: 4,
        strides_per_pattern_entry: 2,
        counter_bits: 2,
        prefetch_confidence_threshold: SignaturePathRatio::new(1, 2).unwrap(),
        lookahead_confidence_threshold: SignaturePathRatio::new(3, 4).unwrap(),
    })
    .unwrap();
    let mut prefetcher = SignaturePathPrefetcher::new(config);

    for address in [0x1000, 0x1040, 0x2000, 0x2080] {
        prefetcher
            .observe(signature_path_access(8, 0xd20, address))
            .unwrap();
    }

    assert_eq!(prefetcher.pattern_strides(0), vec![(1, 0), (2, 1)]);
    let snapshot = prefetcher.snapshot();
    assert_eq!(snapshot.pattern_entries().len(), 1);
    assert_eq!(snapshot.signature_entries().len(), 2);
}

#[test]
fn sms_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        SmsPrefetcherConfig::new(1, SMS_CANDIDATE_BYTE_OVERFLOW_LINES, 4, 4),
        Err(SmsPrefetcherError::VectorLengthTooLarge {
            field: "region lines",
            length,
            ..
        }) if length == SMS_CANDIDATE_BYTE_OVERFLOW_LINES as usize
    ));
    assert!(matches!(
        SmsPrefetcherConfig::new(64, 4096, SMS_CONTEXT_BYTE_OVERFLOW_LENGTH, 4),
        Err(SmsPrefetcherError::VectorLengthTooLarge {
            field: "context entries",
            length: SMS_CONTEXT_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        SmsPrefetcherConfig::new(64, 4096, 4, SMS_PATTERN_HISTORY_BYTE_OVERFLOW_LENGTH),
        Err(SmsPrefetcherError::VectorLengthTooLarge {
            field: "pattern history entries",
            length: SMS_PATTERN_HISTORY_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn sms_prefetcher_commits_region_pattern_on_eviction_and_restores_state() {
    let config = SmsPrefetcherConfig::new(64, 4096, 4, 8).unwrap();
    let mut prefetcher = SmsPrefetcher::new(config.clone());

    assert!(prefetcher
        .observe(sms_access(9, 0xe00, 0x1000))
        .unwrap()
        .is_empty());
    assert_eq!(prefetcher.filter_entry_count(), 1);
    assert_eq!(prefetcher.active_entry_count(), 0);

    assert!(prefetcher
        .observe(sms_access(9, 0xe04, 0x1040))
        .unwrap()
        .is_empty());
    assert_eq!(prefetcher.filter_entry_count(), 0);
    assert_eq!(prefetcher.active_entry_count(), 1);
    assert_eq!(prefetcher.active_offsets(0x1000), vec![0, 64]);

    assert!(prefetcher
        .observe(sms_access(9, 0xe08, 0x1080))
        .unwrap()
        .is_empty());
    assert_eq!(prefetcher.active_offsets(0x1000), vec![0, 64, 128]);

    prefetcher.observe_evict(Address::new(0x10c0));
    assert_eq!(prefetcher.active_entry_count(), 0);
    assert_eq!(prefetcher.pattern_entry_count(), 1);
    assert_eq!(prefetcher.pattern_offsets(0xe00, 0), vec![0, 64, 128]);

    let snapshot = prefetcher.snapshot();
    let mut restored = SmsPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let candidates = restored
        .observe(sms_access(9, 0xe00, 0x2000))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x2000),
            Address::new(0x2040),
            Address::new(0x2080)
        ]
    );
    assert_eq!(candidates[0].source_address(), Address::new(0x2000));
    assert_eq!(candidates[0].context(), AgentId::new(9));
    assert_eq!(candidates[0].pc(), 0xe00);
    assert_eq!(candidates[0].region_base(), Address::new(0x2000));
    assert_eq!(candidates[0].pattern_pc(), 0xe00);
    assert_eq!(candidates[0].trigger_offset(), 0);
    assert_eq!(candidates[0].pattern_offset(), 0);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(candidates[1].pattern_offset(), 64);
    assert_eq!(candidates[2].pattern_offset(), 128);
    assert_eq!(restored.last_candidates(), candidates.as_slice());
}

#[test]
fn sms_prefetcher_applies_filter_fifo_and_pattern_lru_capacity() {
    let config = SmsPrefetcherConfig::new(64, 4096, 1, 1).unwrap();
    let mut prefetcher = SmsPrefetcher::new(config);

    prefetcher.observe(sms_access(10, 0xe20, 0x7000)).unwrap();
    prefetcher.observe(sms_access(10, 0xe24, 0x8000)).unwrap();
    assert_eq!(prefetcher.filter_entry_count(), 1);
    assert_eq!(prefetcher.filter_trigger(0x7000), None);
    assert_eq!(prefetcher.filter_trigger(0x8000), Some((0xe24, 0)));

    for address in [0x3000, 0x3040] {
        prefetcher.observe(sms_access(10, 0xe30, address)).unwrap();
    }
    prefetcher.observe_evict(Address::new(0x3000));
    assert_eq!(prefetcher.pattern_offsets(0xe30, 0), vec![0, 64]);

    for address in [0x4000, 0x4080] {
        prefetcher.observe(sms_access(10, 0xe40, address)).unwrap();
    }
    prefetcher.observe_evict(Address::new(0x4000));
    assert_eq!(prefetcher.pattern_entry_count(), 1);
    assert_eq!(prefetcher.pattern_offsets(0xe30, 0), Vec::<u64>::new());
    assert_eq!(prefetcher.pattern_offsets(0xe40, 0), vec![0, 128]);

    assert!(prefetcher
        .observe(sms_access(10, 0xe30, 0x5000))
        .unwrap()
        .is_empty());
    let candidates = prefetcher
        .observe(sms_access(10, 0xe40, 0x6000))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x6000), Address::new(0x6080)]
    );
}

#[test]
fn dcpt_prefetcher_matches_masked_delta_pairs_and_restores_state() {
    let config = DcptPrefetcherConfig::new(6, 12, 4, 4, true).unwrap();
    let mut prefetcher = DcptPrefetcher::new(config.clone());
    let pc = 0x440;

    for address in [0x1000, 0x1041, 0x10c3, 0x1112] {
        assert!(prefetcher
            .observe(dcpt_access(5, pc, address))
            .unwrap()
            .is_empty(),);
        assert_eq!(prefetcher.last_candidates(), &[]);
        assert_eq!(prefetcher.entry_count(AgentId::new(5)), 1);
    }

    let candidates = prefetcher
        .observe(dcpt_access(5, pc, 0x11a0))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x11ef), Address::new(0x127d)]
    );
    assert_eq!(candidates[0].source_address(), Address::new(0x11a0));
    assert_eq!(candidates[0].context(), AgentId::new(5));
    assert_eq!(candidates[0].pc(), pc);
    assert_eq!(candidates[0].delta(), 0x4f);
    assert_eq!(candidates[0].stride(), 0x4f);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(candidates[1].delta(), 0x8e);
    assert_eq!(candidates[1].stride(), 0x8e);
    assert_eq!(candidates[1].degree_index(), 2);
    assert_eq!(prefetcher.context_count(), 1);
    assert_eq!(prefetcher.entry_count(AgentId::new(6)), 0);
    assert_eq!(prefetcher.last_candidates(), candidates.as_slice());

    let snapshot = prefetcher.snapshot();
    let mut restored = DcptPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let first_restored_candidates = restored
        .observe(dcpt_access(5, pc, 0x11ef))
        .unwrap()
        .to_vec();
    assert_eq!(
        first_restored_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x127d), Address::new(0x12cc)]
    );
    assert_eq!(first_restored_candidates[0].delta(), 0x8e);
    assert_eq!(first_restored_candidates[1].delta(), 0x4f);

    let restored_candidates = restored
        .observe(dcpt_access(5, pc, 0x127d))
        .unwrap()
        .to_vec();
    assert_eq!(
        restored_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x12cc),
            Address::new(0x135a),
            Address::new(0x13a9),
            Address::new(0x1437)
        ]
    );
    assert_eq!(restored_candidates[0].delta(), 0x4f);
    assert_eq!(restored_candidates[1].delta(), 0x8e);
    assert_eq!(restored_candidates[2].delta(), 0x4f);
    assert_eq!(restored_candidates[3].delta(), 0x8e);
}

#[test]
fn dcpt_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    assert_eq!(
        DcptPrefetcherConfig::new(I64_VECTOR_BYTE_OVERFLOW_LENGTH, 12, 4, 4, true),
        Err(DcptPrefetcherError::VectorLengthTooLarge {
            field: "deltas per entry",
            length: I64_VECTOR_BYTE_OVERFLOW_LENGTH,
            maximum: isize::MAX as usize / std::mem::size_of::<i64>(),
        })
    );
    assert!(matches!(
        DcptPrefetcherConfig::new(4, 12, 4, I64_VECTOR_BYTE_OVERFLOW_LENGTH, true),
        Err(DcptPrefetcherError::VectorLengthTooLarge {
            field: "table entries",
            length: I64_VECTOR_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        DcptPrefetcherConfig::new(DCPT_CANDIDATE_BYTE_OVERFLOW_DELTAS, 12, 4, 4, true),
        Err(DcptPrefetcherError::VectorLengthTooLarge {
            field: "candidate results",
            length,
            ..
        }) if length == DCPT_CANDIDATE_BYTE_OVERFLOW_DELTAS - 2
    ));
}

#[test]
fn ampm_prefetcher_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        AmpmPrefetcherConfig::new(1, AMPM_ACCESS_MAP_BYTE_OVERFLOW_LINES, 1, 3),
        Err(AmpmPrefetcherError::VectorLengthTooLarge {
            field: "hot zone lines",
            length,
            ..
        }) if length == AMPM_ACCESS_MAP_BYTE_OVERFLOW_LINES as usize
    ));
    assert!(matches!(
        AmpmPrefetcherConfig::new(1, AMPM_WINDOW_BYTE_OVERFLOW_LINES, 1, 3),
        Err(AmpmPrefetcherError::VectorLengthTooLarge {
            field: "hot zone lines",
            length,
            ..
        }) if length == AMPM_WINDOW_BYTE_OVERFLOW_LINES as usize
    ));
    assert!(matches!(
        AmpmPrefetcherConfig::new(64, 256, 1, OVERSIZED_VECTOR_LENGTH),
        Err(AmpmPrefetcherError::VectorLengthTooLarge {
            field: "table entries",
            length: OVERSIZED_VECTOR_LENGTH,
            ..
        })
    ));
}

#[test]
fn ampm_prefetcher_matches_cross_zone_access_map_patterns_and_restores_state() {
    let config = AmpmPrefetcherConfig::new(64, 256, 2, 8).unwrap();
    let mut prefetcher = AmpmPrefetcher::new(config.clone());

    assert!(prefetcher
        .observe(ampm_access(3, 0x700, 0x10c0))
        .unwrap()
        .is_empty());
    let candidates = prefetcher
        .observe(ampm_access(3, 0x704, 0x1100))
        .unwrap()
        .to_vec();

    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1140)]
    );
    assert_eq!(candidates[0].source_address(), Address::new(0x1100));
    assert_eq!(candidates[0].context(), AgentId::new(3));
    assert_eq!(candidates[0].pc(), 0x704);
    assert_eq!(candidates[0].stride(), 64);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(prefetcher.last_candidates(), candidates.as_slice());
    assert_eq!(prefetcher.zone_count(), 4);
    assert_eq!(prefetcher.issued_prefetch_count(), 1);
    assert_eq!(prefetcher.useful_prefetch_count(), 0);
    assert_eq!(prefetcher.raw_cache_miss_count(), 2);
    assert_eq!(prefetcher.raw_cache_hit_count(), 0);

    let snapshot = prefetcher.snapshot();
    let mut restored = AmpmPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let restored_candidates = restored
        .observe(ampm_access(3, 0x708, 0x1140))
        .unwrap()
        .to_vec();
    assert_eq!(
        restored_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1180)]
    );
    assert_eq!(restored.issued_prefetch_count(), 2);
    assert_eq!(restored.useful_prefetch_count(), 1);
    assert_eq!(restored.raw_cache_miss_count(), 3);
}

#[test]
fn ampm_epoch_control_adjusts_degree_with_typed_stats_and_restores_state() {
    let epoch = AmpmEpochConfig::gem5_defaults(100, 100).unwrap();
    let config = AmpmPrefetcherConfig::new(64, 256, 2, 8)
        .unwrap()
        .with_epoch_control(epoch);
    let mut prefetcher = AmpmPrefetcher::new(config.clone());

    assert_eq!(prefetcher.current_degree(), 2);
    assert_eq!(prefetcher.useful_degree(), 2);
    for (index, address) in [0x10c0, 0x1100, 0x1140, 0x1180].into_iter().enumerate() {
        prefetcher
            .observe(ampm_access(3, 0x800 + index as u64, address))
            .unwrap();
    }

    let report = prefetcher.process_epoch().unwrap();
    assert_eq!(report.previous_degree(), 2);
    assert_eq!(report.previous_useful_degree(), 2);
    assert_eq!(report.next_useful_degree(), 3);
    assert_eq!(report.memory_bandwidth_degree(), 5);
    assert_eq!(report.next_degree(), 3);
    assert_eq!(report.stats().issued_prefetches(), 3);
    assert_eq!(report.stats().useful_prefetches(), 2);
    assert_eq!(report.stats().raw_cache_misses(), 4);
    assert_eq!(report.stats().raw_cache_hits(), 0);
    assert_eq!(prefetcher.current_degree(), 3);
    assert_eq!(prefetcher.useful_degree(), 3);
    assert_eq!(prefetcher.epoch_issued_prefetch_count(), 0);
    assert_eq!(prefetcher.epoch_useful_prefetch_count(), 0);
    assert_eq!(prefetcher.epoch_raw_cache_miss_count(), 0);
    assert_eq!(prefetcher.epoch_raw_cache_hit_count(), 0);
    assert_eq!(prefetcher.last_epoch_report(), Some(&report));

    let snapshot = prefetcher.snapshot();
    let mut restored = AmpmPrefetcher::new(config.clone());
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.current_degree(), 3);
    assert_eq!(restored.last_epoch_report(), Some(&report));

    let mut conservative = AmpmPrefetcher::new(config);
    for index in 0..10 {
        conservative
            .observe(ampm_access(7, 0x900 + index, 0x2000))
            .unwrap();
    }
    let low_report = conservative.process_epoch().unwrap();
    assert_eq!(low_report.previous_useful_degree(), 2);
    assert_eq!(low_report.next_useful_degree(), 1);
    assert_eq!(low_report.memory_bandwidth_degree(), 1);
    assert_eq!(low_report.next_degree(), 1);
    assert_eq!(low_report.stats().raw_cache_misses(), 1);
    assert_eq!(low_report.stats().raw_cache_hits(), 9);
}

#[test]
fn tagged_prefetcher_generates_next_lines_and_queues_candidates() {
    let config = TaggedPrefetcherConfig::new(64, 3).unwrap();
    let mut prefetcher = TaggedPrefetcher::new(config.clone());

    let candidates = prefetcher
        .observe(tagged_access(4, 0x90, 0x1018))
        .unwrap()
        .to_vec();

    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x1040),
            Address::new(0x1080),
            Address::new(0x10c0)
        ]
    );
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(candidates[0].context(), AgentId::new(4));
    assert_eq!(candidates[0].pc(), 0x90);
    assert!(!candidates[0].secure());
    assert_eq!(prefetcher.last_candidates(), candidates.as_slice());

    let snapshot = prefetcher.snapshot();
    let mut restored = TaggedPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 2, 3, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    assert_eq!(queue.enqueue_candidates(5, &candidates).unwrap(), 3);

    let issued = queue.issue_ready(7);
    assert_eq!(
        issued
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x1040),
            Address::new(0x1080),
            Address::new(0x10c0)
        ]
    );
}

#[test]
fn queued_prefetcher_drops_cross_page_candidates_without_translation() {
    let config = TaggedPrefetcherConfig::new(64, 5).unwrap();
    let mut prefetcher = TaggedPrefetcher::new(config);
    let candidates = prefetcher
        .observe(tagged_access(4, 0x90, 0x0f18))
        .unwrap()
        .to_vec();

    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x0f40),
            Address::new(0x0f80),
            Address::new(0x0fc0),
            Address::new(0x1000),
            Address::new(0x1040)
        ]
    );

    let queue_config = QueuedPrefetchConfig::with_line_size(8, 2, 8, true, 64)
        .unwrap()
        .with_page_size(4096)
        .unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    let result = queue
        .enqueue_candidates_filtered(5, &candidates, &[])
        .unwrap();

    assert_eq!(result.accepted(), 3);
    assert_eq!(result.dropped_page_crossing(), 2);

    let issued = queue.issue_ready(7);
    assert_eq!(
        issued
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x0f40),
            Address::new(0x0f80),
            Address::new(0x0fc0)
        ]
    );
}

#[test]
fn stride_prefetcher_trains_per_requestor_context_and_snapshots_state() {
    let config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut prefetcher = StridePrefetcher::new(config.clone());

    assert_eq!(prefetcher.observe(access(1, 0x80, 0x1000)).unwrap(), &[]);
    assert_eq!(prefetcher.observe(access(1, 0x80, 0x1040)).unwrap(), &[]);

    let candidates = prefetcher.observe(access(1, 0x80, 0x1080)).unwrap();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x10c0), Address::new(0x1100)]
    );
    assert_eq!(candidates[0].stride(), 64);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(prefetcher.context_count(), 1);
    assert_eq!(prefetcher.entry_count(AgentId::new(1)), 1);

    assert_eq!(prefetcher.observe(access(2, 0x80, 0x2000)).unwrap(), &[]);
    assert_eq!(prefetcher.context_count(), 2);
    assert_eq!(prefetcher.entry_count(AgentId::new(2)), 1);

    let snapshot = prefetcher.snapshot();
    let mut restored = StridePrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let restored_candidates = restored.observe(access(1, 0x80, 0x10c0)).unwrap();
    assert_eq!(
        restored_candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1100), Address::new(0x1140)]
    );
}

#[test]
fn queued_prefetcher_delays_deduplicates_and_snapshots_candidates() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::new(4, 3, 1, true).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config.clone());
    assert_eq!(queue.enqueue_candidates(10, &candidates).unwrap(), 2);
    assert_eq!(queue.enqueue_candidates(11, &candidates).unwrap(), 0);
    assert_eq!(queue.pending_count(), 2);
    assert!(queue.issue_ready(12).is_empty());

    let first = queue.issue_ready(13);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].address(), Address::new(0x10c0));
    assert_eq!(first[0].context(), AgentId::new(1));
    assert_eq!(first[0].pc(), 0x80);
    assert_eq!(first[0].ready_tick(), 13);
    assert_eq!(queue.pending_count(), 1);

    let snapshot = queue.snapshot();
    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let second = restored.issue_ready(13);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].address(), Address::new(0x1100));
    assert_eq!(second[0].degree_index(), 2);
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn queued_prefetcher_orders_same_tick_candidates_by_priority() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();
    assert_eq!(candidates[0].address(), Address::new(0x10c0));
    assert_eq!(candidates[1].address(), Address::new(0x1100));

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 2, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    assert_eq!(
        queue
            .enqueue_candidates_filtered(10, &candidates[1..], &[])
            .unwrap()
            .accepted(),
        1
    );
    assert_eq!(
        queue
            .enqueue_candidates_filtered(10, &candidates[..1], &[])
            .unwrap()
            .accepted(),
        1
    );

    let issued = queue.issue_ready(13);
    assert_eq!(
        issued
            .iter()
            .map(|entry| entry.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x10c0), Address::new(0x1100)]
    );
}

#[test]
fn queued_prefetcher_squashes_same_line_demand_accesses() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 2, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config.clone());
    assert_eq!(queue.enqueue_candidates(10, &candidates).unwrap(), 2);

    let demand = QueuedPrefetchDemandAccess::new(Address::new(0x10d8), false);
    assert_eq!(queue.squash_demand_access(demand), 1);
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(
        queue.snapshot().pending()[0].address(),
        Address::new(0x1100)
    );

    let snapshot = queue.snapshot();
    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();

    let issued = restored.issue_ready(13);
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].address(), Address::new(0x1100));
    assert_eq!(restored.pending_count(), 0);
}

#[test]
fn queued_prefetcher_filters_candidates_already_in_cache_resources() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 2, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    let result = queue
        .enqueue_candidates_filtered(
            10,
            &candidates,
            &[
                QueuedPrefetchRedundantLine::in_cache(Address::new(0x10f8), false),
                QueuedPrefetchRedundantLine::in_miss_queue(Address::new(0x1108), true),
            ],
        )
        .unwrap();

    assert_eq!(result.accepted(), 1);
    assert_eq!(result.dropped_redundant(), 1);
    assert_eq!(queue.pending_count(), 1);
    assert_eq!(
        queue.snapshot().pending()[0].address(),
        Address::new(0x1100)
    );

    let issued = queue.issue_ready(13);
    assert_eq!(issued.len(), 1);
    assert_eq!(issued[0].address(), Address::new(0x1100));
}

#[test]
fn queued_prefetcher_updates_duplicate_candidate_priority() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 3, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let initial_candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 4, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    assert_eq!(
        queue.enqueue_candidates(10, &initial_candidates).unwrap(),
        3
    );
    let old_priority = queue
        .snapshot()
        .pending()
        .iter()
        .find(|entry| entry.address() == Address::new(0x1100))
        .unwrap()
        .priority();

    let next_candidates = stride.observe(access(1, 0x80, 0x10c0)).unwrap().to_vec();
    assert_eq!(next_candidates[0].address(), Address::new(0x1100));
    assert_eq!(next_candidates[1].address(), Address::new(0x1140));
    assert_eq!(next_candidates[2].address(), Address::new(0x1180));
    let result = queue
        .enqueue_candidates_filtered(11, &next_candidates, &[])
        .unwrap();

    assert_eq!(result.accepted(), 1);
    assert_eq!(result.duplicate_hits(), 2);
    assert_eq!(result.updated_priorities(), 2);
    assert_eq!(queue.pending_count(), 4);

    let snapshot = queue.snapshot();
    let updated = snapshot
        .pending()
        .iter()
        .find(|entry| entry.address() == Address::new(0x1100))
        .unwrap();
    assert!(updated.priority() > old_priority);
    assert!(snapshot
        .pending()
        .iter()
        .any(|entry| entry.address() == Address::new(0x1180)));
}

#[test]
fn queued_prefetcher_can_evict_oldest_lowest_priority_entry_when_full() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let initial_candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(2, 3, 4, true, 64)
        .unwrap()
        .with_full_policy(QueuedPrefetchFullPolicy::EvictOldestLowestPriority);
    let mut queue = QueuedPrefetcher::new(queue_config);
    assert_eq!(
        queue.enqueue_candidates(10, &initial_candidates).unwrap(),
        2
    );

    let next_candidates = stride.observe(access(1, 0x80, 0x10c0)).unwrap().to_vec();
    assert_eq!(next_candidates[1].address(), Address::new(0x1140));
    let result = queue
        .enqueue_candidates_filtered(11, &next_candidates[1..], &[])
        .unwrap();

    assert_eq!(result.accepted(), 1);
    assert_eq!(result.evicted_full(), 1);
    assert_eq!(queue.pending_count(), 2);
    assert_eq!(
        queue
            .snapshot()
            .pending()
            .iter()
            .map(|entry| entry.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x10c0), Address::new(0x1140)]
    );
}

#[test]
fn queued_prefetcher_exposes_next_ready_tick_for_scheduler_planning() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 2, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();

    let queue_config = QueuedPrefetchConfig::with_line_size(4, 3, 1, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config.clone());
    assert_eq!(queue.next_ready_tick(), None);
    assert_eq!(queue.enqueue_candidates(10, &candidates).unwrap(), 2);
    assert_eq!(queue.next_ready_tick(), Some(13));
    assert!(queue.issue_ready(12).is_empty());
    assert_eq!(queue.next_ready_tick(), Some(13));

    assert_eq!(queue.issue_ready(13).len(), 1);
    assert_eq!(queue.next_ready_tick(), Some(13));

    let snapshot = queue.snapshot();
    let mut restored = QueuedPrefetcher::new(queue_config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.next_ready_tick(), Some(13));

    assert_eq!(restored.issue_ready(13).len(), 1);
    assert_eq!(restored.next_ready_tick(), None);
}

#[test]
fn queued_prefetcher_applies_accuracy_throttle_before_candidate_enqueue() {
    let stride_config = StridePrefetcherConfig::new(64, 4, 2, 5, 0, true).unwrap();
    let mut stride = StridePrefetcher::new(stride_config);
    assert!(stride.observe(access(1, 0x80, 0x1000)).unwrap().is_empty());
    assert!(stride.observe(access(1, 0x80, 0x1040)).unwrap().is_empty());
    let candidates = stride.observe(access(1, 0x80, 0x1080)).unwrap().to_vec();
    assert_eq!(candidates.len(), 5);

    let mut throttle = QueuedPrefetchThrottle::new(QueuedPrefetchThrottleConfig::new(60).unwrap());
    throttle.record_issued(10).unwrap();
    throttle.record_useful(5).unwrap();

    let queue_config = QueuedPrefetchConfig::with_line_size(8, 3, 8, true, 64).unwrap();
    let mut queue = QueuedPrefetcher::new(queue_config);
    let result = queue
        .enqueue_candidates_throttled(10, &candidates, &[], &throttle)
        .unwrap();

    assert_eq!(result.accepted(), 3);
    assert_eq!(result.dropped_throttled(), 2);
    assert_eq!(queue.pending_count(), 3);
    assert_eq!(
        queue
            .snapshot()
            .pending()
            .iter()
            .map(|entry| entry.address())
            .collect::<Vec<_>>(),
        vec![
            Address::new(0x10c0),
            Address::new(0x1100),
            Address::new(0x1140)
        ]
    );
}

#[test]
fn queued_prefetch_throttle_uses_accuracy_and_snapshots_counters() {
    let config = QueuedPrefetchThrottleConfig::new(60).unwrap();
    let mut throttle = QueuedPrefetchThrottle::new(config.clone());

    assert_eq!(throttle.max_permitted(5), 5);

    throttle.record_issued(10).unwrap();
    throttle.record_useful(5).unwrap();
    assert_eq!(throttle.max_permitted(5), 3);
    assert_eq!(throttle.issued_prefetches(), 10);
    assert_eq!(throttle.useful_prefetches(), 5);

    let snapshot = throttle.snapshot();
    let mut restored = QueuedPrefetchThrottle::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.max_permitted(5), 3);

    let mut always_keeps_one =
        QueuedPrefetchThrottle::new(QueuedPrefetchThrottleConfig::new(100).unwrap());
    assert_eq!(always_keeps_one.max_permitted(4), 4);
    always_keeps_one.record_issued(4).unwrap();
    assert_eq!(always_keeps_one.max_permitted(4), 1);
}

#[test]
fn queued_prefetch_throttle_rejects_useful_counts_above_issued() {
    let mut throttle = QueuedPrefetchThrottle::new(QueuedPrefetchThrottleConfig::new(60).unwrap());

    assert_eq!(
        throttle.record_useful(1).unwrap_err(),
        QueuedPrefetchThrottleError::UsefulExceedsIssued {
            issued_prefetches: 0,
            useful_prefetches: 1,
        }
    );
    assert_eq!(throttle.max_permitted(4), 4);

    throttle.record_issued(2).unwrap();
    throttle.record_useful(2).unwrap();
    assert_eq!(
        throttle.record_useful(1).unwrap_err(),
        QueuedPrefetchThrottleError::UsefulExceedsIssued {
            issued_prefetches: 2,
            useful_prefetches: 3,
        }
    );
    assert_eq!(throttle.useful_prefetches(), 2);
    assert_eq!(throttle.max_permitted(4), 4);
}
