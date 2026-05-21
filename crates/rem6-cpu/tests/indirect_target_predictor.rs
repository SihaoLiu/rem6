use rem6_cpu::{
    BranchTargetKind, CpuId, IndirectTargetPredictor, IndirectTargetPredictorConfig,
    IndirectTargetPredictorError, IndirectTargetSequence,
};
use rem6_memory::Address;

fn predictor_no_hash(sets: usize, ways: usize) -> IndirectTargetPredictor {
    IndirectTargetPredictor::new(
        IndirectTargetPredictorConfig::with_options(2, sets, ways, 16, 4, 4, 2, 8, false, false)
            .unwrap(),
    )
}

#[test]
fn indirect_target_predictor_records_miss_update_and_hit() {
    let mut predictor = predictor_no_hash(4, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);
    let target = Address::new(0x2080);

    let miss = predictor
        .predict(
            cpu,
            IndirectTargetSequence::new(0),
            pc,
            BranchTargetKind::IndirectUnconditional,
        )
        .unwrap();

    assert!(!miss.hit());
    assert_eq!(miss.cpu(), cpu);
    assert_eq!(miss.pc(), pc);
    assert_eq!(miss.target(), None);
    assert_eq!(miss.lookup_count(), 1);
    assert_eq!(predictor.miss_count(), 1);

    let update = predictor
        .update(
            miss.history(),
            true,
            target,
            BranchTargetKind::IndirectUnconditional,
            true,
        )
        .unwrap();

    assert!(update.indirect_recorded());
    assert!(update.target_recorded());
    assert_eq!(update.ghr_before(), 0);
    assert_eq!(update.ghr_after(), 1);
    assert_eq!(update.replaced(), None);
    assert_eq!(predictor.indirect_record_count(), 1);
    assert_eq!(predictor.target_record_count(), 1);

    predictor.commit(cpu).unwrap();

    let hit = predictor
        .predict(
            cpu,
            IndirectTargetSequence::new(1),
            pc,
            BranchTargetKind::IndirectUnconditional,
        )
        .unwrap();

    assert!(hit.hit());
    assert_eq!(hit.target(), Some(target));
    assert_eq!(hit.history().tag(), miss.history().tag());
    assert_eq!(predictor.hit_count(), 1);
}

#[test]
fn indirect_target_predictor_uses_deterministic_lru_replacement() {
    let mut predictor = predictor_no_hash(1, 2);
    let cpu = CpuId::new(0);
    let first = Address::new(0x1000);
    let second = Address::new(0x2000);
    let third = Address::new(0x3000);

    for (sequence, pc, target) in [
        (0, first, Address::new(0x1100)),
        (1, second, Address::new(0x2200)),
    ] {
        let miss = predictor
            .predict(
                cpu,
                IndirectTargetSequence::new(sequence),
                pc,
                BranchTargetKind::CallIndirect,
            )
            .unwrap();
        predictor
            .update(
                miss.history(),
                true,
                target,
                BranchTargetKind::CallIndirect,
                true,
            )
            .unwrap();
    }

    assert_eq!(
        predictor
            .predict(
                cpu,
                IndirectTargetSequence::new(2),
                first,
                BranchTargetKind::CallIndirect,
            )
            .unwrap()
            .target(),
        Some(Address::new(0x1100))
    );

    let miss = predictor
        .predict(
            cpu,
            IndirectTargetSequence::new(3),
            third,
            BranchTargetKind::CallIndirect,
        )
        .unwrap();
    let replacement = predictor
        .update(
            miss.history(),
            true,
            Address::new(0x3300),
            BranchTargetKind::CallIndirect,
            true,
        )
        .unwrap();

    assert_eq!(replacement.replaced().unwrap().pc(), second);
    assert_eq!(predictor.eviction_count(), 1);
    assert_eq!(
        predictor
            .predict(
                cpu,
                IndirectTargetSequence::new(4),
                second,
                BranchTargetKind::CallIndirect,
            )
            .unwrap()
            .target(),
        None
    );
}

#[test]
fn indirect_target_predictor_tracks_direct_branch_history_without_target_lookup() {
    let mut predictor = predictor_no_hash(4, 2);
    let cpu = CpuId::new(1);
    let direct_pc = Address::new(0x1000);

    let direct = predictor
        .predict(
            cpu,
            IndirectTargetSequence::new(0),
            direct_pc,
            BranchTargetKind::DirectConditional,
        )
        .unwrap();

    assert!(!direct.looked_up_target());
    assert_eq!(predictor.lookup_count(), 0);

    let update = predictor
        .update(
            direct.history(),
            true,
            Address::new(0x1040),
            BranchTargetKind::DirectConditional,
            false,
        )
        .unwrap();

    assert!(!update.indirect_recorded());
    assert!(!update.target_recorded());
    assert_eq!(update.ghr_after(), 1);
    assert_eq!(predictor.snapshot().threads()[1].ghr(), 1);
    assert_eq!(predictor.snapshot().threads()[0].ghr(), 0);
}

#[test]
fn indirect_target_predictor_snapshot_restore_preserves_cache_threads_and_counters() {
    let mut predictor = predictor_no_hash(4, 2);
    let cpu = CpuId::new(0);
    let pc = Address::new(0x1000);
    let target = Address::new(0x2080);

    let miss = predictor
        .predict(
            cpu,
            IndirectTargetSequence::new(0),
            pc,
            BranchTargetKind::IndirectConditional,
        )
        .unwrap();
    predictor
        .update(
            miss.history(),
            true,
            target,
            BranchTargetKind::IndirectConditional,
            true,
        )
        .unwrap();
    let snapshot = predictor.snapshot();

    predictor.reset();
    assert_eq!(predictor.target_record_count(), 0);
    assert_eq!(
        predictor
            .predict(
                cpu,
                IndirectTargetSequence::new(1),
                pc,
                BranchTargetKind::IndirectConditional,
            )
            .unwrap()
            .target(),
        None
    );

    predictor.restore(&snapshot).unwrap();

    assert_eq!(predictor.snapshot().entries(), snapshot.entries());
    assert_eq!(predictor.snapshot().threads(), snapshot.threads());
    assert_eq!(predictor.target_record_count(), 1);
}

#[test]
fn indirect_target_predictor_rejects_bad_config_thread_and_snapshot_shape() {
    assert_eq!(
        IndirectTargetPredictorConfig::with_options(0, 4, 2, 16, 4, 4, 2, 8, false, false),
        Err(IndirectTargetPredictorError::ZeroThreads)
    );
    assert_eq!(
        IndirectTargetPredictorConfig::with_options(1, 0, 2, 16, 4, 4, 2, 8, false, false),
        Err(IndirectTargetPredictorError::ZeroSets)
    );
    assert_eq!(
        IndirectTargetPredictorConfig::with_options(1, 3, 2, 16, 4, 4, 2, 8, false, false),
        Err(IndirectTargetPredictorError::SetCountNotPowerOfTwo { sets: 3 })
    );
    assert_eq!(
        IndirectTargetPredictorConfig::with_options(1, 4, 0, 16, 4, 4, 2, 8, false, false),
        Err(IndirectTargetPredictorError::ZeroWays)
    );
    assert_eq!(
        IndirectTargetPredictorConfig::with_options(1, 4, 2, 0, 4, 4, 2, 8, false, false),
        Err(IndirectTargetPredictorError::TagBitsOutOfRange { bits: 0 })
    );
    assert_eq!(
        IndirectTargetPredictorConfig::with_options(1, 4, 2, 65, 4, 4, 2, 8, false, false),
        Err(IndirectTargetPredictorError::TagBitsOutOfRange { bits: 65 })
    );
    assert_eq!(
        IndirectTargetPredictorConfig::with_options(1, 4, 2, 16, 0, 4, 2, 8, false, false),
        Err(IndirectTargetPredictorError::ZeroPathLength)
    );
    assert_eq!(
        IndirectTargetPredictorConfig::with_options(1, 4, 2, 16, 4, 4, 2, 65, false, false),
        Err(IndirectTargetPredictorError::HistoryBitsOutOfRange { bits: 65 })
    );

    let mut predictor = predictor_no_hash(4, 2);
    assert_eq!(
        predictor.predict(
            CpuId::new(3),
            IndirectTargetSequence::new(0),
            Address::new(0x1000),
            BranchTargetKind::CallIndirect,
        ),
        Err(IndirectTargetPredictorError::UnknownThread { cpu: CpuId::new(3) })
    );

    let snapshot = predictor_no_hash(4, 2).snapshot();
    let mut different_shape = predictor_no_hash(8, 2);

    assert_eq!(
        different_shape.restore(&snapshot),
        Err(IndirectTargetPredictorError::SnapshotShapeMismatch {
            expected_threads: 2,
            actual_threads: 2,
            expected_sets: 8,
            actual_sets: 4,
            expected_ways: 2,
            actual_ways: 2,
        })
    );
}
