use rem6_dram::{
    DramControllerConfig, DramError, DramGeometry, DramMemoryController, DramMemoryTechnology,
    DramProfileField, DramTiming, ExternalMemoryProfile, ExternalMemoryTopology,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryTargetId,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn geometry() -> DramGeometry {
    DramGeometry::new(16, 1024, 64).unwrap()
}

fn timing() -> DramTiming {
    DramTiming::new(4, 8, 10, 3, 5).unwrap()
}

fn target(id: u32) -> MemoryTargetId {
    MemoryTargetId::new(id)
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(9), sequence)
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

#[test]
fn external_memory_profiles_name_ddr_hbm_and_lpddr_topologies() {
    let ddr = ExternalMemoryProfile::ddr(target(1), layout(), 2, 2, geometry(), timing()).unwrap();
    let hbm = ExternalMemoryProfile::hbm(target(2), layout(), 4, 2, geometry(), timing()).unwrap();
    let lpddr =
        ExternalMemoryProfile::lpddr(target(3), layout(), 2, 4, geometry(), timing()).unwrap();

    assert_eq!(ddr.technology(), DramMemoryTechnology::Ddr);
    assert_eq!(
        ddr.topology(),
        ExternalMemoryTopology::Ddr {
            channels: 2,
            ranks_per_channel: 2,
        },
    );
    assert_eq!(hbm.technology(), DramMemoryTechnology::Hbm);
    assert_eq!(
        hbm.topology(),
        ExternalMemoryTopology::Hbm {
            stacks: 4,
            pseudo_channels_per_stack: 2,
        },
    );
    assert_eq!(lpddr.technology(), DramMemoryTechnology::Lpddr);
    assert_eq!(
        lpddr.topology(),
        ExternalMemoryTopology::Lpddr {
            channels: 2,
            dies_per_channel: 4,
        },
    );

    assert_eq!(
        ddr.controller_config(),
        DramControllerConfig::new(target(1), layout(), geometry(), timing()),
    );
    assert_eq!(hbm.target(), target(2));
    assert_eq!(lpddr.line_layout(), layout());
}

#[test]
fn external_memory_profiles_reject_zero_topology_counts() {
    assert_eq!(
        ExternalMemoryProfile::ddr(target(1), layout(), 0, 1, geometry(), timing()).unwrap_err(),
        DramError::ZeroProfileTopology {
            technology: DramMemoryTechnology::Ddr,
            field: DramProfileField::Channels,
        },
    );
    assert_eq!(
        ExternalMemoryProfile::ddr(target(1), layout(), 1, 0, geometry(), timing()).unwrap_err(),
        DramError::ZeroProfileTopology {
            technology: DramMemoryTechnology::Ddr,
            field: DramProfileField::RanksPerChannel,
        },
    );
    assert_eq!(
        ExternalMemoryProfile::hbm(target(2), layout(), 0, 1, geometry(), timing()).unwrap_err(),
        DramError::ZeroProfileTopology {
            technology: DramMemoryTechnology::Hbm,
            field: DramProfileField::Stacks,
        },
    );
    assert_eq!(
        ExternalMemoryProfile::hbm(target(2), layout(), 1, 0, geometry(), timing()).unwrap_err(),
        DramError::ZeroProfileTopology {
            technology: DramMemoryTechnology::Hbm,
            field: DramProfileField::PseudoChannelsPerStack,
        },
    );
    assert_eq!(
        ExternalMemoryProfile::lpddr(target(3), layout(), 1, 0, geometry(), timing()).unwrap_err(),
        DramError::ZeroProfileTopology {
            technology: DramMemoryTechnology::Lpddr,
            field: DramProfileField::DiesPerChannel,
        },
    );
}

#[test]
fn dram_memory_controller_adds_profiled_targets_and_restores_profile_metadata() {
    let profile =
        ExternalMemoryProfile::hbm(target(7), layout(), 2, 4, geometry(), timing()).unwrap();
    let mut controller = DramMemoryController::new();

    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x2000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0000), vec![0x5a; 64])
        .unwrap();
    let outcome = controller.accept(10, &read(0x0008, 40)).unwrap();

    assert_eq!(outcome.target(), profile.target());
    assert_eq!(outcome.ready_cycle(), 22);
    assert_eq!(controller.memory_profile(profile.target()), Some(&profile));

    let snapshot = controller.snapshot();
    assert_eq!(snapshot.targets()[0].profile(), Some(&profile));

    let restored = DramMemoryController::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.memory_profile(profile.target()), Some(&profile));
    assert_eq!(
        restored.dram_controller(profile.target()).unwrap().timing(),
        timing(),
    );
}
