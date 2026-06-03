use rem6_cache::{
    SignaturePathPrefetchAccess, SignaturePathPrefetcherConfig,
    SignaturePathPrefetcherConfigOptions, SignaturePathRatio,
    SignaturePathV2GlobalHistoryEntrySnapshot, SignaturePathV2Prefetcher,
    SignaturePathV2PrefetcherConfig, SignaturePathV2PrefetcherError,
};
use rem6_memory::{Address, AgentId};

const SIGNATURE_PATH_V2_GLOBAL_HISTORY_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<SignaturePathV2GlobalHistoryEntrySnapshot>() + 1;

fn access(agent: u32, pc: u64, address: u64) -> SignaturePathPrefetchAccess {
    SignaturePathPrefetchAccess::new(AgentId::new(agent), pc, Address::new(address), false)
}

fn base_config(
    page_bytes: u64,
    counter_bits: u32,
) -> Result<SignaturePathPrefetcherConfig, SignaturePathV2PrefetcherError> {
    Ok(SignaturePathPrefetcherConfig::new(
        SignaturePathPrefetcherConfigOptions {
            line_size: 64,
            page_bytes,
            signature_shift: 2,
            signature_bits: 8,
            signature_table_entries: 8,
            pattern_table_entries: 8,
            strides_per_pattern_entry: 2,
            counter_bits,
            prefetch_confidence_threshold: SignaturePathRatio::new(3, 10).unwrap(),
            lookahead_confidence_threshold: SignaturePathRatio::new(3, 10).unwrap(),
        },
    )?)
}

fn v2_config(
    page_bytes: u64,
    counter_bits: u32,
    global_history_register_entries: usize,
) -> Result<SignaturePathV2PrefetcherConfig, SignaturePathV2PrefetcherError> {
    SignaturePathV2PrefetcherConfig::new(
        base_config(page_bytes, counter_bits)?,
        global_history_register_entries,
    )
}

#[test]
fn signature_path_v2_config_rejects_invalid_global_history_capacity() {
    let base = base_config(4096, 2).unwrap();
    assert_eq!(
        SignaturePathV2PrefetcherConfig::new(base.clone(), 0),
        Err(SignaturePathV2PrefetcherError::ZeroGlobalHistoryRegisterEntries)
    );
    assert!(matches!(
        SignaturePathV2PrefetcherConfig::new(
            base,
            SIGNATURE_PATH_V2_GLOBAL_HISTORY_BYTE_OVERFLOW_LENGTH
        ),
        Err(SignaturePathV2PrefetcherError::VectorLengthTooLarge {
            field: "global history register entries",
            length: SIGNATURE_PATH_V2_GLOBAL_HISTORY_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn signature_path_v2_does_not_issue_auxiliary_next_line_on_cold_path() {
    let config = v2_config(4096, 2, 4).unwrap();
    let mut prefetcher = SignaturePathV2Prefetcher::new(config);

    assert!(prefetcher
        .observe(access(7, 0x100, 0x1000))
        .unwrap()
        .is_empty());
    assert!(prefetcher
        .observe(access(7, 0x104, 0x1040))
        .unwrap()
        .is_empty());
    assert_eq!(prefetcher.last_candidates(), &[]);
    assert_eq!(prefetcher.pattern_total_counter(0), Some(1));
    assert_eq!(prefetcher.pattern_strides(0), vec![(1, 1)]);
}

#[test]
fn signature_path_v2_breaks_lookahead_signature_cycles() {
    let config = v2_config(512, 2, 4).unwrap();
    let mut prefetcher = SignaturePathV2Prefetcher::new(config);

    for block in [0, 1] {
        assert!(prefetcher
            .observe(access(7, 0x180, 0x1000 + block * 64))
            .unwrap()
            .is_empty());
    }

    let candidates = prefetcher
        .observe(access(7, 0x184, 0x1000 + 6 * 64))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x1000 + 11 * 64)]
    );
    assert_eq!(candidates[0].signature(), 1);
    assert_eq!(candidates[0].delta_blocks(), 5);
    assert_eq!(candidates[0].prefetch_confidence_ppm(), 1_000_000);

    let global_history = prefetcher.global_history_entries();
    assert_eq!(global_history.len(), 1);
    assert_eq!(global_history[0].signature(), 1);
    assert_eq!(global_history[0].last_block(), 6);
    assert_eq!(global_history[0].delta(), 5);
    assert_eq!(global_history[0].confidence_ppm(), 1_000_000);
}

#[test]
fn signature_path_v2_uses_pattern_entry_ratio_for_prefetch_confidence() {
    let config = v2_config(4096, 3, 4).unwrap();
    let mut prefetcher = SignaturePathV2Prefetcher::new(config);

    for address in [0x1000, 0x1040, 0x1080] {
        prefetcher.observe(access(7, 0x200, address)).unwrap();
    }
    for address in [0x2000, 0x2040, 0x20c0] {
        prefetcher.observe(access(7, 0x204, address)).unwrap();
    }
    for address in [0x4000, 0x4040, 0x4080] {
        prefetcher.observe(access(7, 0x208, address)).unwrap();
    }

    assert_eq!(prefetcher.pattern_total_counter(1), Some(3));
    assert_eq!(prefetcher.pattern_strides(1), vec![(1, 1), (2, 1)]);

    assert!(prefetcher
        .observe(access(7, 0x20c, 0x3000))
        .unwrap()
        .is_empty());
    let candidates = prefetcher
        .observe(access(7, 0x210, 0x3040))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x3080), Address::new(0x30c0)]
    );
    assert_eq!(candidates[0].signature(), 1);
    assert_eq!(candidates[0].path_confidence_ppm(), 1_000_000);
    assert_eq!(candidates[0].prefetch_confidence_ppm(), 333_333);
    assert_eq!(candidates[0].delta_blocks(), 1);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(candidates[1].signature(), 1);
    assert_eq!(candidates[1].prefetch_confidence_ppm(), 333_333);
    assert_eq!(candidates[1].delta_blocks(), 2);
    assert_eq!(candidates[1].degree_index(), 2);
}

fn train_pattern_one_stride_one(prefetcher: &mut SignaturePathV2Prefetcher, page_base: u64) {
    for address in [page_base, page_base + 0x40, page_base + 0x80] {
        prefetcher.observe(access(8, 0x300, address)).unwrap();
    }
}

#[test]
fn signature_path_v2_halves_pattern_counters_on_saturation_and_restores_state() {
    let config = v2_config(4096, 2, 2).unwrap();
    let mut prefetcher = SignaturePathV2Prefetcher::new(config.clone());

    for page_base in [0x1000, 0x2000, 0x3000, 0x4000] {
        train_pattern_one_stride_one(&mut prefetcher, page_base);
    }
    assert_eq!(prefetcher.pattern_total_counter(1), Some(2));
    assert_eq!(prefetcher.pattern_strides(1), vec![(1, 2)]);
    assert!(prefetcher.global_history_entries().is_empty());

    let snapshot = prefetcher.snapshot();
    let mut restored = SignaturePathV2Prefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.pattern_total_counter(1), Some(2));
    assert_eq!(restored.pattern_strides(1), vec![(1, 2)]);
}

#[test]
fn signature_path_v2_uses_recorded_prefetch_accuracy_for_lookahead() {
    let config = v2_config(4096, 2, 2).unwrap();
    let mut prefetcher = SignaturePathV2Prefetcher::new(config);

    assert_eq!(
        prefetcher.set_prefetch_accuracy_counts(2, 3),
        Err(
            SignaturePathV2PrefetcherError::UsefulPrefetchesExceedIssued {
                useful: 3,
                issued: 2,
            }
        )
    );
    prefetcher.set_prefetch_accuracy_counts(2, 1).unwrap();
    assert_eq!(prefetcher.issued_prefetches(), 2);
    assert_eq!(prefetcher.useful_prefetches(), 1);
    assert_eq!(prefetcher.prefetch_accuracy_ppm(), 500_000);

    for address in [0x1000, 0x1040, 0x1080, 0x10c0] {
        prefetcher.observe(access(9, 0x400, address)).unwrap();
    }

    assert!(prefetcher
        .observe(access(9, 0x404, 0x2000))
        .unwrap()
        .is_empty());
    let candidates = prefetcher
        .observe(access(9, 0x408, 0x2040))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x2080), Address::new(0x20c0)]
    );
    assert_eq!(candidates[0].path_confidence_ppm(), 1_000_000);
    assert_eq!(candidates[1].path_confidence_ppm(), 500_000);
}
