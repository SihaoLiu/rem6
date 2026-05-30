use rem6_cpu::{
    CpuId, RiscvStoreConditionalFailureDiagnostic, RiscvStoreConditionalProgress,
    RiscvStoreConditionalProgressConfig,
};
use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address};

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
fn riscv_sc_progress_rejects_zero_diagnostic_threshold() {
    assert_eq!(
        RiscvStoreConditionalProgressConfig::new(0)
            .unwrap_err()
            .to_string(),
        "RISC-V store-conditional diagnostic threshold must be nonzero"
    );
}
