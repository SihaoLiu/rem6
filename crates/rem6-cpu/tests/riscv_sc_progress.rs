use rem6_cpu::{
    CpuId, RiscvStoreConditionalFailureDiagnostic, RiscvStoreConditionalProgress,
    RiscvStoreConditionalProgressCheckpointPayload, RiscvStoreConditionalProgressConfig,
    RiscvStoreConditionalProgressError,
};
use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address};

const SC_CHECKPOINT_VERSION_OFFSET: usize = 4;
const SC_CHECKPOINT_STREAK_COUNT_OFFSET: usize = 13;
const SC_CHECKPOINT_FIRST_STREAK_OFFSET: usize = 21;
const SC_CHECKPOINT_STREAK_RECORD_BYTES: usize = 44;
const SINGLE_STREAK_CHECKPOINT_BYTES: &[u8] = &[
    b'R', b'S', b'C', b'P', 1, 3, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0xb0,
    0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 0x3c, 0, 0, 0, 0, 0, 0, 0, 0x3c, 0, 0, 0, 0, 0, 0, 0,
    1, 0, 0, 0, 0, 0, 0, 0,
];

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn failure_ticks(
    progress: &mut RiscvStoreConditionalProgress,
    cpu: CpuId,
    address: Address,
    ticks: &[Tick],
) -> Vec<RiscvStoreConditionalFailureDiagnostic> {
    ticks
        .iter()
        .filter_map(|tick| progress.record_failure(cpu, *tick, address, size(8)))
        .collect()
}

#[test]
fn riscv_sc_progress_emits_typed_diagnostic_at_declared_failure_threshold() {
    let mut progress =
        RiscvStoreConditionalProgress::new(RiscvStoreConditionalProgressConfig::new(3).unwrap());
    let cpu = CpuId::new(1);
    let address = Address::new(0x9008);

    let diagnostics = failure_ticks(&mut progress, cpu, address, &[10, 14, 19]);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics, progress.diagnostics());
    assert_eq!(diagnostics[0].cpu(), cpu);
    assert_eq!(diagnostics[0].address(), address);
    assert_eq!(diagnostics[0].size(), size(8));
    assert_eq!(diagnostics[0].failure_count(), 3);
    assert_eq!(diagnostics[0].diagnostic_threshold(), 3);
    assert_eq!(diagnostics[0].first_failure_tick(), 10);
    assert_eq!(diagnostics[0].last_failure_tick(), 19);
    assert_eq!(progress.streak(cpu).unwrap().failure_count(), 3);
}

#[test]
fn riscv_sc_progress_success_resets_consecutive_failure_streak() {
    let mut progress =
        RiscvStoreConditionalProgress::new(RiscvStoreConditionalProgressConfig::new(3).unwrap());
    let cpu = CpuId::new(0);
    let address = Address::new(0x9010);

    assert!(failure_ticks(&mut progress, cpu, address, &[1, 2]).is_empty());
    progress.record_success(cpu);
    assert_eq!(progress.streak(cpu), None);

    assert!(failure_ticks(&mut progress, cpu, address, &[8, 9]).is_empty());
    let diagnostics = failure_ticks(&mut progress, cpu, address, &[10]);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].failure_count(), 3);
    assert_eq!(diagnostics[0].first_failure_tick(), 8);
    assert_eq!(diagnostics[0].last_failure_tick(), 10);
}

#[test]
fn riscv_sc_progress_new_address_starts_a_new_failure_streak() {
    let mut progress =
        RiscvStoreConditionalProgress::new(RiscvStoreConditionalProgressConfig::new(2).unwrap());
    let cpu = CpuId::new(2);

    assert!(failure_ticks(&mut progress, cpu, Address::new(0x9000), &[20]).is_empty());
    assert!(failure_ticks(&mut progress, cpu, Address::new(0x9010), &[21]).is_empty());
    assert_eq!(progress.streak(cpu).unwrap().failure_count(), 1);
    assert_eq!(
        progress.streak(cpu).unwrap().address(),
        Address::new(0x9010)
    );

    let diagnostics = failure_ticks(&mut progress, cpu, Address::new(0x9010), &[22]);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].first_failure_tick(), 21);
    assert_eq!(diagnostics[0].last_failure_tick(), 22);
}

#[test]
fn riscv_sc_progress_snapshot_restore_preserves_streaks_and_diagnostics() {
    let mut progress =
        RiscvStoreConditionalProgress::new(RiscvStoreConditionalProgressConfig::new(3).unwrap());
    let cpu = CpuId::new(3);
    let address = Address::new(0x9020);
    failure_ticks(&mut progress, cpu, address, &[30, 31, 32]);
    let snapshot = progress.snapshot();

    progress.record_success(cpu);
    failure_ticks(&mut progress, cpu, Address::new(0x9030), &[40]);
    progress.restore(&snapshot).unwrap();

    assert_eq!(progress.snapshot(), snapshot);
    assert_eq!(progress.streak(cpu).unwrap().address(), address);
    assert_eq!(progress.streak(cpu).unwrap().failure_count(), 3);
    assert_eq!(progress.diagnostics().len(), 1);
}

#[test]
fn riscv_sc_progress_checkpoint_payload_round_trips_snapshot() {
    let config = RiscvStoreConditionalProgressConfig::new(2).unwrap();
    let mut progress = RiscvStoreConditionalProgress::new(config);
    failure_ticks(
        &mut progress,
        CpuId::new(0),
        Address::new(0xa000),
        &[50, 51],
    );
    failure_ticks(&mut progress, CpuId::new(1), Address::new(0xa040), &[52]);
    let snapshot = progress.snapshot();
    let payload =
        RiscvStoreConditionalProgressCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();

    let decoded =
        RiscvStoreConditionalProgressCheckpointPayload::decode(payload.encode().as_slice())
            .unwrap();
    let mut restored = RiscvStoreConditionalProgress::new(config);
    restored.restore(decoded.snapshot()).unwrap();

    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(decoded.snapshot().streaks().len(), 2);
    assert_eq!(decoded.snapshot().diagnostics().len(), 1);
}

#[test]
fn riscv_sc_progress_checkpoint_payload_has_stable_single_streak_bytes() {
    let config = RiscvStoreConditionalProgressConfig::new(3).unwrap();
    let mut progress = RiscvStoreConditionalProgress::new(config);
    failure_ticks(&mut progress, CpuId::new(2), Address::new(0xb000), &[60]);
    let payload =
        RiscvStoreConditionalProgressCheckpointPayload::from_snapshot(progress.snapshot())
            .unwrap()
            .encode();

    assert_eq!(payload, SINGLE_STREAK_CHECKPOINT_BYTES);
    let decoded =
        RiscvStoreConditionalProgressCheckpointPayload::decode(SINGLE_STREAK_CHECKPOINT_BYTES)
            .unwrap();

    assert_eq!(decoded.snapshot(), &progress.snapshot());
}

#[test]
fn riscv_sc_progress_checkpoint_payload_rejects_unsupported_version() {
    let mut payload = SINGLE_STREAK_CHECKPOINT_BYTES.to_vec();
    payload[SC_CHECKPOINT_VERSION_OFFSET] = 2;

    assert_eq!(
        RiscvStoreConditionalProgressCheckpointPayload::decode(&payload).unwrap_err(),
        RiscvStoreConditionalProgressError::UnsupportedCheckpointVersion { version: 2 }
    );
}

#[test]
fn riscv_sc_progress_checkpoint_payload_rejects_duplicate_streak_cpu() {
    let config = RiscvStoreConditionalProgressConfig::new(3).unwrap();
    let mut progress = RiscvStoreConditionalProgress::new(config);
    failure_ticks(&mut progress, CpuId::new(2), Address::new(0xb000), &[60]);
    let payload =
        RiscvStoreConditionalProgressCheckpointPayload::from_snapshot(progress.snapshot())
            .unwrap()
            .encode();
    let duplicate_streak_payload = duplicate_first_sc_checkpoint_streak(payload);

    assert_eq!(
        RiscvStoreConditionalProgressCheckpointPayload::decode(&duplicate_streak_payload)
            .unwrap_err(),
        RiscvStoreConditionalProgressError::DuplicateSnapshotStreak { cpu: CpuId::new(2) }
    );
}

#[test]
fn riscv_sc_progress_rejects_zero_diagnostic_threshold() {
    assert_eq!(
        RiscvStoreConditionalProgressConfig::new(0)
            .unwrap_err()
            .to_string(),
        "RISC-V store-conditional diagnostic threshold must be nonzero"
    );
}

fn duplicate_first_sc_checkpoint_streak(mut payload: Vec<u8>) -> Vec<u8> {
    payload[SC_CHECKPOINT_STREAK_COUNT_OFFSET..SC_CHECKPOINT_STREAK_COUNT_OFFSET + 4]
        .copy_from_slice(&2_u32.to_le_bytes());
    let first_streak = payload[SC_CHECKPOINT_FIRST_STREAK_OFFSET
        ..SC_CHECKPOINT_FIRST_STREAK_OFFSET + SC_CHECKPOINT_STREAK_RECORD_BYTES]
        .to_vec();
    payload.splice(
        SC_CHECKPOINT_FIRST_STREAK_OFFSET..SC_CHECKPOINT_FIRST_STREAK_OFFSET,
        first_streak,
    );
    payload
}
