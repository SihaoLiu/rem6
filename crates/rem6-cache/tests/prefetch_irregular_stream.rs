use rem6_cache::{
    IrregularStreamBufferAccess, IrregularStreamBufferCandidate, IrregularStreamBufferConfig,
    IrregularStreamBufferError, IrregularStreamBufferMappingEntrySnapshot,
    IrregularStreamBufferPrefetcher, IrregularStreamBufferTrainingEntrySnapshot,
};
use rem6_memory::{Address, AgentId};

const ISB_TRAINING_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<IrregularStreamBufferTrainingEntrySnapshot>() + 1;
const ISB_ADDRESS_MAP_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<IrregularStreamBufferMappingEntrySnapshot>() + 1;
const ISB_PREFETCH_CANDIDATE_BYTE_OVERFLOW_LENGTH: usize =
    isize::MAX as usize / std::mem::size_of::<IrregularStreamBufferCandidate>() + 1;
const ISB_PREFETCH_CANDIDATE_POWER_OF_TWO_OVERFLOW_LENGTH: usize = 1usize << (usize::BITS - 2);
const _: () = assert!(
    ISB_PREFETCH_CANDIDATE_POWER_OF_TWO_OVERFLOW_LENGTH
        > ISB_PREFETCH_CANDIDATE_BYTE_OVERFLOW_LENGTH
);

fn isb_access(agent: u32, pc: u64, address: u64, secure: bool) -> IrregularStreamBufferAccess {
    IrregularStreamBufferAccess::new(AgentId::new(agent), pc, Address::new(address), secure)
}

#[test]
fn irregular_stream_buffer_config_rejects_vector_lengths_above_host_limit() {
    assert!(matches!(
        IrregularStreamBufferConfig::new(64, 2, 16, 3, ISB_TRAINING_BYTE_OVERFLOW_LENGTH, 8, 4,),
        Err(IrregularStreamBufferError::VectorLengthTooLarge {
            field: "training entries",
            length: ISB_TRAINING_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        IrregularStreamBufferConfig::new(64, 2, 16, 3, 4, ISB_ADDRESS_MAP_BYTE_OVERFLOW_LENGTH, 4,),
        Err(IrregularStreamBufferError::VectorLengthTooLarge {
            field: "address map entries",
            length: ISB_ADDRESS_MAP_BYTE_OVERFLOW_LENGTH,
            ..
        })
    ));
    assert!(matches!(
        IrregularStreamBufferConfig::new(
            64,
            2,
            16,
            3,
            4,
            8,
            ISB_PREFETCH_CANDIDATE_POWER_OF_TWO_OVERFLOW_LENGTH,
        ),
        Err(IrregularStreamBufferError::VectorLengthTooLarge {
            field: "prefetch candidates per entry",
            length: ISB_PREFETCH_CANDIDATE_POWER_OF_TWO_OVERFLOW_LENGTH,
            ..
        })
    ));
}

#[test]
fn irregular_stream_buffer_linearizes_correlated_addresses_and_restores_state() {
    let config = IrregularStreamBufferConfig::new(64, 2, 16, 3, 4, 8, 4).unwrap();
    let mut prefetcher = IrregularStreamBufferPrefetcher::new(config.clone());

    for address in [0x1000, 0x5000, 0x9000] {
        assert!(prefetcher
            .observe(isb_access(4, 0xa00, address, false))
            .unwrap()
            .is_empty());
    }

    assert_eq!(prefetcher.structural_address_counter(), 16);
    assert_eq!(
        prefetcher.physical_to_structural(Address::new(0x1000), false),
        Some((0, 1))
    );
    assert_eq!(
        prefetcher.physical_to_structural(Address::new(0x5000), false),
        Some((1, 1))
    );
    assert_eq!(
        prefetcher.physical_to_structural(Address::new(0x9000), false),
        Some((2, 1))
    );
    assert_eq!(
        prefetcher.structural_to_physical(1, false),
        Some((Address::new(0x5000), 1))
    );

    let snapshot = prefetcher.snapshot();
    let mut restored = IrregularStreamBufferPrefetcher::new(config);
    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);

    let candidates = restored
        .observe(isb_access(4, 0xb00, 0x1000, false))
        .unwrap()
        .to_vec();
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.address())
            .collect::<Vec<_>>(),
        vec![Address::new(0x5000), Address::new(0x9000)]
    );
    assert_eq!(candidates[0].source_address(), Address::new(0x1000));
    assert_eq!(candidates[0].context(), AgentId::new(4));
    assert_eq!(candidates[0].pc(), 0xb00);
    assert_eq!(candidates[0].structural_address(), 1);
    assert_eq!(candidates[0].degree_index(), 1);
    assert_eq!(candidates[1].structural_address(), 2);
    assert_eq!(restored.last_candidates(), candidates.as_slice());

    assert!(restored
        .observe(isb_access(4, 0xb00, 0x1000, true))
        .unwrap()
        .is_empty());
}

#[test]
fn irregular_stream_buffer_applies_training_and_mapping_capacity() {
    let config = IrregularStreamBufferConfig::new(64, 2, 16, 2, 1, 1, 4).unwrap();
    let mut prefetcher = IrregularStreamBufferPrefetcher::new(config);

    prefetcher
        .observe(isb_access(5, 0xc00, 0x1000, false))
        .unwrap();
    prefetcher
        .observe(isb_access(5, 0xd00, 0x2000, false))
        .unwrap();
    prefetcher
        .observe(isb_access(5, 0xc00, 0x1040, false))
        .unwrap();
    assert_eq!(prefetcher.training_entry_count(), 1);
    assert_eq!(
        prefetcher.physical_to_structural(Address::new(0x1000), false),
        None
    );

    prefetcher
        .observe(isb_access(5, 0xe00, 0x3000, false))
        .unwrap();
    prefetcher
        .observe(isb_access(5, 0xe00, 0x3040, false))
        .unwrap();
    assert_eq!(prefetcher.physical_mapping_entry_count(), 1);
    assert_eq!(prefetcher.structural_mapping_entry_count(), 1);
    assert_eq!(
        prefetcher.physical_to_structural(Address::new(0x3000), false),
        Some((0, 1))
    );

    prefetcher
        .observe(isb_access(5, 0xf00, 0x4000, false))
        .unwrap();
    prefetcher
        .observe(isb_access(5, 0xf00, 0x4040, false))
        .unwrap();
    assert_eq!(
        prefetcher.physical_to_structural(Address::new(0x3000), false),
        None
    );
    assert_eq!(
        prefetcher.physical_to_structural(Address::new(0x4000), false),
        Some((16, 1))
    );
}
