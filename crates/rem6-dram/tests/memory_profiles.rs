use rem6_dram::{
    DramControllerConfig, DramError, DramGeometry, DramMemoryController, DramMemoryTechnology,
    DramProfileField, DramTiming, ExternalMemoryProfile, ExternalMemoryTopology,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    MemoryTargetId,
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

fn write(address: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(4).unwrap(),
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::full(AccessSize::new(4).unwrap()).unwrap(),
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
    let nvm = ExternalMemoryProfile::nvm(target(4), layout(), 3, 6, geometry(), timing()).unwrap();

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
    assert_eq!(nvm.technology(), DramMemoryTechnology::Nvm);
    assert_eq!(
        nvm.topology(),
        ExternalMemoryTopology::Nvm {
            controllers: 3,
            media_banks_per_controller: 6,
        },
    );

    let default_config = DramControllerConfig::new(target(1), layout(), geometry(), timing());
    assert_eq!(default_config.parallel_port_count(), 1);
    assert_eq!(ddr.controller_config().target(), default_config.target());
    assert_eq!(ddr.controller_config().layout(), default_config.layout());
    assert_eq!(
        ddr.controller_config().geometry(),
        default_config.geometry()
    );
    assert_eq!(ddr.controller_config().timing(), default_config.timing());
    assert_eq!(ddr.parallel_port_count(), 2);
    assert_eq!(ddr.controller_config().parallel_port_count(), 2);
    assert_eq!(hbm.target(), target(2));
    assert_eq!(hbm.parallel_port_count(), 8);
    assert_eq!(hbm.controller_config().parallel_port_count(), 8);
    assert_eq!(lpddr.line_layout(), layout());
    assert_eq!(lpddr.parallel_port_count(), 2);
    assert_eq!(lpddr.controller_config().parallel_port_count(), 2);
    assert_eq!(nvm.parallel_port_count(), 3);
    assert_eq!(nvm.controller_config().parallel_port_count(), 3);
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
    assert_eq!(
        ExternalMemoryProfile::nvm(target(4), layout(), 0, 1, geometry(), timing()).unwrap_err(),
        DramError::ZeroProfileTopology {
            technology: DramMemoryTechnology::Nvm,
            field: DramProfileField::Controllers,
        },
    );
    assert_eq!(
        ExternalMemoryProfile::nvm(target(4), layout(), 1, 0, geometry(), timing()).unwrap_err(),
        DramError::ZeroProfileTopology {
            technology: DramMemoryTechnology::Nvm,
            field: DramProfileField::MediaBanksPerController,
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

#[test]
fn dram_memory_controller_reports_profile_metadata_in_target_activity() {
    let profile =
        ExternalMemoryProfile::nvm(target(6), layout(), 2, 8, geometry(), timing()).unwrap();
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
    controller.accept(10, &read(0x0008, 45)).unwrap();

    let activity = controller.target_activity(profile.target()).unwrap();

    assert_eq!(activity.memory_profile(), Some(&profile));
    assert_eq!(
        activity.memory_profile().unwrap().technology(),
        DramMemoryTechnology::Nvm,
    );
    assert_eq!(activity.profile().access_count(), 1);
}

#[test]
fn profiled_hbm_target_uses_independent_parallel_ports_for_turnaround() {
    let profile =
        ExternalMemoryProfile::hbm(target(8), layout(), 1, 2, geometry(), timing()).unwrap();
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
        .insert_line(profile.target(), Address::new(0x0000), vec![0x11; 64])
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0040), vec![0x22; 64])
        .unwrap();

    let first = controller.accept(0, &read(0x0000, 50)).unwrap();
    let second = controller.accept(0, &write(0x0040, 51)).unwrap();

    assert_eq!(first.dram_access().parallel_port(), 0);
    assert_eq!(first.dram_access().command_cycle(), 4);
    assert_eq!(first.ready_cycle(), 12);
    assert_eq!(second.dram_access().parallel_port(), 1);
    assert_eq!(second.dram_access().command_cycle(), 4);
    assert_eq!(second.ready_cycle(), 14);
}

#[test]
fn profiled_single_channel_ddr_target_keeps_turnaround_on_shared_port() {
    let profile =
        ExternalMemoryProfile::ddr(target(9), layout(), 1, 2, geometry(), timing()).unwrap();
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
        .insert_line(profile.target(), Address::new(0x0000), vec![0x11; 64])
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0040), vec![0x22; 64])
        .unwrap();

    let first = controller.accept(0, &read(0x0000, 60)).unwrap();
    let second = controller.accept(0, &write(0x0040, 61)).unwrap();

    assert_eq!(first.dram_access().parallel_port(), 0);
    assert_eq!(first.dram_access().command_cycle(), 4);
    assert_eq!(first.ready_cycle(), 12);
    assert_eq!(second.dram_access().parallel_port(), 0);
    assert_eq!(second.dram_access().command_cycle(), 9);
    assert_eq!(second.ready_cycle(), 19);
}
