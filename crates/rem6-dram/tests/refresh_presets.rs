use rem6_dram::{
    DramController, DramError, DramGeometry, DramJedecRefreshPreset, DramMemoryTechnology,
    DramRefreshTiming, DramTiming, ExternalMemoryProfile,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn geometry() -> DramGeometry {
    DramGeometry::new(8, 512, 64).unwrap()
}

fn timing() -> DramTiming {
    DramTiming::new(4, 8, 10, 3, 5).unwrap()
}

fn target(id: u32) -> MemoryTargetId {
    MemoryTargetId::new(id)
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(3), sequence)
}

fn read(address: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(8).unwrap(),
        layout(),
    )
    .unwrap()
}

fn assert_preset_cycles(
    preset: DramJedecRefreshPreset,
    technology: DramMemoryTechnology,
    clock_mhz: u32,
    t_refi_ps: u64,
    t_rfc_ps: u64,
    t_refi_cycles: u64,
    t_rfc_cycles: u64,
) {
    assert_eq!(preset.technology(), technology);
    assert_eq!(preset.clock_mhz(), clock_mhz);
    assert_eq!(preset.t_refi_ps(), t_refi_ps);
    assert_eq!(preset.t_rfc_ps(), t_rfc_ps);

    let timing = preset.refresh_timing().unwrap();
    assert_eq!(timing.t_refi_cycles(), t_refi_cycles);
    assert_eq!(timing.t_rfc_cycles(), t_rfc_cycles);
    assert_eq!(
        timing,
        DramRefreshTiming::new(t_refi_cycles, t_rfc_cycles).unwrap()
    );
}

#[test]
fn jedec_refresh_presets_convert_trefi_and_trfc_to_cycles() {
    assert_preset_cycles(
        DramJedecRefreshPreset::Ddr4_2400_8Gb,
        DramMemoryTechnology::Ddr,
        1200,
        7_800_000,
        350_000,
        9_360,
        420,
    );
    assert_preset_cycles(
        DramJedecRefreshPreset::Ddr5_4800_16Gb,
        DramMemoryTechnology::Ddr,
        2400,
        3_900_000,
        295_000,
        9_360,
        708,
    );
    assert_preset_cycles(
        DramJedecRefreshPreset::Hbm2_2000_2Gb,
        DramMemoryTechnology::Hbm,
        1000,
        3_900_000,
        220_000,
        3_900,
        220,
    );
}

#[test]
fn jedec_refresh_profile_constructors_attach_validated_timing() {
    let ddr4 =
        ExternalMemoryProfile::ddr4_2400_8gb(target(1), layout(), 2, 2, geometry(), timing())
            .unwrap();
    let ddr5 =
        ExternalMemoryProfile::ddr5_4800_16gb(target(2), layout(), 2, 1, geometry(), timing())
            .unwrap();
    let hbm = ExternalMemoryProfile::hbm2_2000_2gb(target(3), layout(), 2, 4, geometry(), timing())
        .unwrap();

    assert_eq!(
        ddr4.timing().refresh_timing(),
        Some(
            DramJedecRefreshPreset::Ddr4_2400_8Gb
                .refresh_timing()
                .unwrap()
        )
    );
    assert_eq!(
        ddr5.timing().refresh_timing(),
        Some(
            DramJedecRefreshPreset::Ddr5_4800_16Gb
                .refresh_timing()
                .unwrap()
        )
    );
    assert_eq!(
        hbm.timing().refresh_timing(),
        Some(
            DramJedecRefreshPreset::Hbm2_2000_2Gb
                .refresh_timing()
                .unwrap()
        )
    );
    assert_eq!(ddr4.technology(), DramMemoryTechnology::Ddr);
    assert_eq!(ddr5.technology(), DramMemoryTechnology::Ddr);
    assert_eq!(hbm.technology(), DramMemoryTechnology::Hbm);
}

#[test]
fn jedec_refresh_preset_reuses_refresh_slot_validation() {
    let no_activate_slot = DramTiming::new(9_000, 8, 10, 3, 5).unwrap();

    assert_eq!(
        no_activate_slot
            .with_jedec_refresh_preset(DramJedecRefreshPreset::Ddr5_4800_16Gb)
            .unwrap_err(),
        DramError::RefreshRecoveryLeavesNoActivateSlot {
            interval: 9_360,
            recovery: 708,
            activate_latency: 9_000,
        },
    );
}

#[test]
fn dram_controller_uses_preset_refresh_cycles_for_due_refresh() {
    let profile =
        ExternalMemoryProfile::ddr4_2400_8gb(target(4), layout(), 1, 1, geometry(), timing())
            .unwrap();
    let mut controller = DramController::new(profile.geometry(), profile.timing());

    let first = controller.schedule(0, &read(0x0000, 1)).unwrap();
    assert!(first.refresh_events().is_empty());

    let second = controller.schedule(9_361, &read(0x0008, 2)).unwrap();

    assert_eq!(second.refresh_events().len(), 1);
    assert_eq!(second.refresh_events()[0].start_cycle(), 9_360);
    assert_eq!(second.refresh_events()[0].end_cycle(), 9_780);
    assert_eq!(second.refresh_events()[0].cycle_count(), 420);
    assert!(!second.row_hit());
    assert_eq!(second.command_cycle(), 9_784);
    assert_eq!(second.ready_cycle(), 9_792);

    let activity = controller.activity_profile();
    assert_eq!(activity.refresh_count(), 1);
    assert_eq!(activity.refresh_cycle_count(), 420);
}
