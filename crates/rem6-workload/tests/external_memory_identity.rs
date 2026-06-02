use rem6_boot::BootImage;
use rem6_dram::{DramGeometry, DramLowPowerTiming, DramTiming, ExternalMemoryProfile};
use rem6_memory::{AccessSize, Address, CacheLineLayout, MemoryTargetId};
use rem6_workload::{
    WorkloadHostPlacement, WorkloadId, WorkloadManifest, WorkloadMemoryTarget, WorkloadTopology,
};

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn boot_image() -> BootImage {
    BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8000), vec![0x13, 0x05, 0x00, 0x00])
        .unwrap()
}

fn manifest_with_timing(timing: DramTiming) -> WorkloadManifest {
    manifest_with_geometry_and_timing(DramGeometry::new(4, 64, 16).unwrap(), timing)
}

fn manifest_with_geometry_and_timing(
    geometry: DramGeometry,
    timing: DramTiming,
) -> WorkloadManifest {
    let profile =
        ExternalMemoryProfile::hbm(MemoryTargetId::new(0), layout(), 2, 2, geometry, timing)
            .unwrap();
    let target = WorkloadMemoryTarget::new(
        0,
        16,
        rem6_memory::AddressRange::new(Address::new(0x8000), AccessSize::new(0x2000).unwrap())
            .unwrap(),
    )
    .unwrap()
    .with_external_memory_profile(profile)
    .unwrap();

    WorkloadManifest::builder(id("profiled-burst-spacing"), boot_image())
        .with_topology(
            WorkloadTopology::new(4, 2, 2, WorkloadHostPlacement::new(3, 2, 41).unwrap())
                .unwrap()
                .add_memory_target(target)
                .unwrap(),
        )
        .build()
        .unwrap()
}

#[test]
fn workload_manifest_identity_changes_with_external_memory_burst_spacing() {
    let base = manifest_with_timing(DramTiming::new(4, 8, 10, 3, 5).unwrap());
    let spaced = manifest_with_timing(
        DramTiming::new(4, 8, 10, 3, 5)
            .unwrap()
            .with_burst_spacing(2)
            .unwrap(),
    );

    assert_ne!(base.identity(), spaced.identity());
}

#[test]
fn workload_manifest_identity_changes_with_external_memory_command_window() {
    let base = manifest_with_timing(DramTiming::new(4, 8, 10, 3, 5).unwrap());
    let windowed = manifest_with_timing(
        DramTiming::new(4, 8, 10, 3, 5)
            .unwrap()
            .with_command_window(10, 2)
            .unwrap(),
    );

    assert_ne!(base.identity(), windowed.identity());
}

#[test]
fn workload_manifest_identity_changes_with_external_memory_low_power_timing() {
    let base = manifest_with_timing(DramTiming::new(4, 8, 10, 3, 5).unwrap());
    let low_power_timing = DramLowPowerTiming::new(20, 80, 7)
        .unwrap()
        .with_self_refresh_exit_latency(17)
        .unwrap();
    let low_power = manifest_with_timing(
        DramTiming::new(4, 8, 10, 3, 5)
            .unwrap()
            .with_low_power_timing(low_power_timing),
    );

    assert_ne!(base.identity(), low_power.identity());
    for alternate_low_power_timing in [
        DramLowPowerTiming::new(21, 80, 7)
            .unwrap()
            .with_self_refresh_exit_latency(17)
            .unwrap(),
        DramLowPowerTiming::new(20, 81, 7)
            .unwrap()
            .with_self_refresh_exit_latency(17)
            .unwrap(),
        DramLowPowerTiming::new(20, 80, 8)
            .unwrap()
            .with_self_refresh_exit_latency(17)
            .unwrap(),
        DramLowPowerTiming::new(20, 80, 7)
            .unwrap()
            .with_self_refresh_exit_latency(18)
            .unwrap(),
    ] {
        let alternate_low_power = manifest_with_timing(
            DramTiming::new(4, 8, 10, 3, 5)
                .unwrap()
                .with_low_power_timing(alternate_low_power_timing),
        );

        assert_ne!(low_power.identity(), alternate_low_power.identity());
    }
}

#[test]
fn workload_manifest_identity_changes_with_external_memory_bank_groups() {
    let base = manifest_with_timing(DramTiming::new(4, 8, 10, 3, 5).unwrap());
    let bank_grouped = manifest_with_geometry_and_timing(
        DramGeometry::new(4, 64, 16)
            .unwrap()
            .with_bank_groups(2)
            .unwrap(),
        DramTiming::new(4, 8, 10, 3, 5)
            .unwrap()
            .with_same_bank_group_burst_spacing(6)
            .unwrap(),
    );

    assert_ne!(base.identity(), bank_grouped.identity());
}
