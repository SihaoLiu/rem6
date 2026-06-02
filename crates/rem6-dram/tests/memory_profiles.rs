use rem6_dram::{
    DramControllerConfig, DramError, DramGeometry, DramMemoryController, DramMemorySnapshot,
    DramMemoryTargetSnapshot, DramMemoryTechnology, DramProfileField, DramTiming,
    ExternalMemoryProfile, ExternalMemoryTopology, NvmMediaTiming,
};
use rem6_kernel::{WaitForEdgeKind, WaitForNode};
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

fn writeback_dirty(address: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::writeback_dirty(
        request_id(sequence),
        Address::new(address),
        vec![0xcc; layout().bytes() as usize],
        layout(),
    )
    .unwrap()
}

fn snapshot_for_profile(profile: ExternalMemoryProfile) -> DramMemorySnapshot {
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller.snapshot()
}

fn snapshot_with_replaced_profile(
    snapshot: &DramMemorySnapshot,
    profile: ExternalMemoryProfile,
) -> DramMemorySnapshot {
    let target = &snapshot.targets()[0];
    DramMemorySnapshot::new(
        snapshot.store().clone(),
        vec![DramMemoryTargetSnapshot::with_profile(
            target.target(),
            target.controller().clone(),
            profile,
        )],
    )
}

fn assert_restore_error_contains(snapshot: &DramMemorySnapshot, expected: &[&str]) {
    let error = DramMemoryController::from_snapshot(snapshot)
        .unwrap_err()
        .to_string();
    for text in expected {
        assert!(
            error.contains(text),
            "restore error {error:?} did not contain {text:?}"
        );
    }
}

#[test]
fn external_memory_profiles_name_ddr_hbm_and_lpddr_topologies() {
    let ddr = ExternalMemoryProfile::ddr(target(1), layout(), 2, 2, geometry(), timing()).unwrap();
    let hbm = ExternalMemoryProfile::hbm(target(2), layout(), 4, 2, geometry(), timing()).unwrap();
    let lpddr =
        ExternalMemoryProfile::lpddr(target(3), layout(), 2, 4, geometry(), timing()).unwrap();
    let nvm = ExternalMemoryProfile::nvm(target(4), layout(), 3, 6, geometry(), timing()).unwrap();

    assert_eq!(ddr.technology(), DramMemoryTechnology::Ddr);
    assert_eq!(ddr.technology().as_str(), "ddr");
    assert_eq!(
        ddr.topology(),
        ExternalMemoryTopology::Ddr {
            channels: 2,
            ranks_per_channel: 2,
        },
    );
    assert_eq!(ddr.topology().kind(), DramMemoryTechnology::Ddr);
    assert_eq!(ddr.topology().as_str(), "ddr");
    assert_eq!(ddr.topology().parallel_port_label(), "channel");
    assert_eq!(ddr.topology().topology_unit_label(), "rank");
    assert_eq!(hbm.technology(), DramMemoryTechnology::Hbm);
    assert_eq!(hbm.technology().as_str(), "hbm");
    assert_eq!(
        hbm.topology(),
        ExternalMemoryTopology::Hbm {
            stacks: 4,
            pseudo_channels_per_stack: 2,
        },
    );
    assert_eq!(hbm.topology().kind(), DramMemoryTechnology::Hbm);
    assert_eq!(hbm.topology().as_str(), "hbm");
    assert_eq!(hbm.topology().parallel_port_label(), "pseudo_channel");
    assert_eq!(hbm.topology().topology_unit_label(), "pseudo_channel");
    assert_eq!(lpddr.technology(), DramMemoryTechnology::Lpddr);
    assert_eq!(lpddr.technology().as_str(), "lpddr");
    assert_eq!(
        lpddr.topology(),
        ExternalMemoryTopology::Lpddr {
            channels: 2,
            dies_per_channel: 4,
        },
    );
    assert_eq!(lpddr.topology().kind(), DramMemoryTechnology::Lpddr);
    assert_eq!(lpddr.topology().as_str(), "lpddr");
    assert_eq!(lpddr.topology().parallel_port_label(), "channel");
    assert_eq!(lpddr.topology().topology_unit_label(), "die");
    assert_eq!(nvm.technology(), DramMemoryTechnology::Nvm);
    assert_eq!(nvm.technology().as_str(), "nvm");
    assert_eq!(
        nvm.topology(),
        ExternalMemoryTopology::Nvm {
            controllers: 3,
            media_banks_per_controller: 6,
        },
    );
    assert_eq!(nvm.topology().kind(), DramMemoryTechnology::Nvm);
    assert_eq!(nvm.topology().as_str(), "nvm");
    assert_eq!(nvm.topology().parallel_port_label(), "controller");
    assert_eq!(nvm.topology().topology_unit_label(), "media_bank");

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
fn external_memory_profiles_report_parallel_resource_capacity() {
    let grouped_geometry = geometry().with_bank_groups(4).unwrap();
    let ddr =
        ExternalMemoryProfile::ddr(target(30), layout(), 2, 2, grouped_geometry, timing()).unwrap();
    let hbm =
        ExternalMemoryProfile::hbm(target(31), layout(), 2, 4, grouped_geometry, timing()).unwrap();
    let lpddr =
        ExternalMemoryProfile::lpddr(target(32), layout(), 3, 2, geometry(), timing()).unwrap();
    let nvm = ExternalMemoryProfile::nvm(target(33), layout(), 2, 8, geometry(), timing()).unwrap();

    let ddr_summary = ddr.parallel_resource_summary();
    assert_eq!(ddr_summary.target(), target(30));
    assert_eq!(ddr_summary.technology(), DramMemoryTechnology::Ddr);
    assert_eq!(ddr_summary.parallel_port_count(), 2);
    assert_eq!(ddr_summary.topology_unit_count(), 4);
    assert_eq!(ddr_summary.banks_per_topology_unit(), 16);
    assert_eq!(ddr_summary.total_topology_bank_count(), 64);
    assert_eq!(ddr_summary.scheduler_bank_count(), 32);
    assert_eq!(ddr_summary.bank_groups_per_port(), Some(4));
    assert_eq!(ddr_summary.scheduler_bank_group_count(), Some(8));

    let hbm_summary = hbm.parallel_resource_summary();
    assert_eq!(hbm_summary.parallel_port_count(), 8);
    assert_eq!(hbm_summary.topology_unit_count(), 8);
    assert_eq!(hbm_summary.total_topology_bank_count(), 128);
    assert_eq!(hbm_summary.scheduler_bank_count(), 128);
    assert_eq!(hbm_summary.scheduler_bank_group_count(), Some(32));

    let lpddr_summary = lpddr.parallel_resource_summary();
    assert_eq!(lpddr_summary.parallel_port_count(), 3);
    assert_eq!(lpddr_summary.topology_unit_count(), 6);
    assert_eq!(lpddr_summary.total_topology_bank_count(), 96);
    assert_eq!(lpddr_summary.scheduler_bank_count(), 48);
    assert_eq!(lpddr_summary.bank_groups_per_port(), None);
    assert_eq!(lpddr_summary.scheduler_bank_group_count(), None);

    let nvm_summary = nvm.parallel_resource_summary();
    assert_eq!(nvm_summary.parallel_port_count(), 2);
    assert_eq!(nvm_summary.topology_unit_count(), 16);
    assert_eq!(nvm_summary.total_topology_bank_count(), 256);
    assert_eq!(nvm_summary.scheduler_bank_count(), 32);
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
fn dram_memory_controller_rejects_profile_metadata_drift_in_snapshots() {
    let base_profile =
        ExternalMemoryProfile::hbm(target(20), layout(), 2, 4, geometry(), timing()).unwrap();
    let snapshot = snapshot_for_profile(base_profile);

    let wrong_target =
        ExternalMemoryProfile::hbm(target(21), layout(), 2, 4, geometry(), timing()).unwrap();
    assert_restore_error_contains(
        &snapshot_with_replaced_profile(&snapshot, wrong_target),
        &["DRAM target 20", "profile target", "21"],
    );

    let wrong_layout = ExternalMemoryProfile::hbm(
        target(20),
        CacheLineLayout::new(128).unwrap(),
        2,
        4,
        geometry(),
        timing(),
    )
    .unwrap();
    assert_restore_error_contains(
        &snapshot_with_replaced_profile(&snapshot, wrong_layout),
        &["DRAM target 20", "profile line layout", "128", "64"],
    );

    let wrong_geometry = ExternalMemoryProfile::hbm(
        target(20),
        layout(),
        2,
        4,
        DramGeometry::new(8, 1024, 64).unwrap(),
        timing(),
    )
    .unwrap();
    assert_restore_error_contains(
        &snapshot_with_replaced_profile(&snapshot, wrong_geometry),
        &["DRAM target 20", "profile geometry"],
    );

    let wrong_timing = ExternalMemoryProfile::hbm(
        target(20),
        layout(),
        2,
        4,
        geometry(),
        DramTiming::new(5, 8, 10, 3, 5).unwrap(),
    )
    .unwrap();
    assert_restore_error_contains(
        &snapshot_with_replaced_profile(&snapshot, wrong_timing),
        &["DRAM target 20", "profile timing"],
    );

    let wrong_ports =
        ExternalMemoryProfile::hbm(target(20), layout(), 1, 4, geometry(), timing()).unwrap();
    assert_restore_error_contains(
        &snapshot_with_replaced_profile(&snapshot, wrong_ports),
        &["DRAM target 20", "profile parallel ports", "4", "8"],
    );

    let nvm_media_timing = NvmMediaTiming::new(30, 50, 6, 4, 1).unwrap();
    let nvm_profile = ExternalMemoryProfile::nvm(target(22), layout(), 2, 8, geometry(), timing())
        .unwrap()
        .with_nvm_media_timing(nvm_media_timing)
        .unwrap();
    let nvm_snapshot = snapshot_for_profile(nvm_profile);
    let missing_media =
        ExternalMemoryProfile::nvm(target(22), layout(), 2, 8, geometry(), timing()).unwrap();
    assert_restore_error_contains(
        &snapshot_with_replaced_profile(&nvm_snapshot, missing_media),
        &["DRAM target 22", "profile NVM media timing"],
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
    assert_eq!(
        activity.parallel_resource_summary().unwrap(),
        profile.parallel_resource_summary(),
    );
    assert_eq!(activity.profile().access_count(), 1);
}

#[test]
fn dram_memory_controller_lists_profile_parallel_resource_summaries_by_target() {
    let ddr = ExternalMemoryProfile::ddr(target(40), layout(), 2, 2, geometry(), timing()).unwrap();
    let hbm = ExternalMemoryProfile::hbm(target(41), layout(), 2, 4, geometry(), timing()).unwrap();
    let mut controller = DramMemoryController::new();

    controller.add_profile(hbm).unwrap();
    controller.add_profile(ddr).unwrap();

    let summaries = controller.profile_parallel_resource_summaries();

    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].target(), target(40));
    assert_eq!(summaries[0].parallel_port_count(), 2);
    assert_eq!(summaries[0].topology_unit_count(), 4);
    assert_eq!(summaries[1].target(), target(41));
    assert_eq!(summaries[1].parallel_port_count(), 8);
    assert_eq!(summaries[1].topology_unit_count(), 8);
    assert_eq!(
        controller
            .profile_parallel_resource_summary(target(41))
            .unwrap(),
        hbm.parallel_resource_summary(),
    );
    assert!(controller
        .profile_parallel_resource_summary(target(42))
        .is_none());
}

#[test]
fn dram_memory_activity_profile_summarizes_profile_parallel_capacity() {
    let grouped_geometry = geometry().with_bank_groups(4).unwrap();
    let ddr =
        ExternalMemoryProfile::ddr(target(50), layout(), 2, 2, grouped_geometry, timing()).unwrap();
    let hbm =
        ExternalMemoryProfile::hbm(target(51), layout(), 2, 4, grouped_geometry, timing()).unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(hbm).unwrap();
    controller.add_profile(ddr).unwrap();
    for (profile, base) in [(ddr, 0x0000), (hbm, 0x8000)] {
        controller
            .map_region(
                profile.target(),
                Address::new(base),
                AccessSize::new(0x4000).unwrap(),
            )
            .unwrap();
        controller
            .insert_line(profile.target(), Address::new(base), vec![0x5a; 64])
            .unwrap();
    }
    controller.accept(10, &read(0x0008, 90)).unwrap();

    let profile = controller.activity_profile();

    assert_eq!(profile.active_target_count(), 1);
    assert_eq!(profile.profiled_target_count(), 2);
    assert_eq!(profile.profile_parallel_port_capacity(), 10);
    assert_eq!(profile.profile_topology_unit_capacity(), 12);
    assert_eq!(profile.profile_scheduler_bank_capacity(), 160);
    assert_eq!(profile.profile_topology_bank_capacity(), 192);
    assert_eq!(profile.profile_scheduler_bank_group_capacity(), 40);

    let marker = controller.mark_activity();
    let since_marker = controller.activity_profile_since(&marker);

    assert_eq!(since_marker.active_target_count(), 0);
    assert_eq!(since_marker.profiled_target_count(), 2);
    assert_eq!(since_marker.profile_parallel_port_capacity(), 10);
    assert_eq!(since_marker.profile_topology_unit_capacity(), 12);
    assert_eq!(since_marker.profile_scheduler_bank_capacity(), 160);
    assert_eq!(since_marker.profile_topology_bank_capacity(), 192);
    assert_eq!(since_marker.profile_scheduler_bank_group_capacity(), 40);
    assert!(since_marker.is_empty());
}

#[test]
fn nvm_media_timing_delays_reads_and_tracks_persistent_write_queue() {
    let media_timing = NvmMediaTiming::new(30, 50, 6, 4, 1).unwrap();
    let profile = ExternalMemoryProfile::nvm(target(10), layout(), 2, 8, geometry(), timing())
        .unwrap()
        .with_nvm_media_timing(media_timing)
        .unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0000), vec![0x11; 64])
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0040), vec![0x22; 64])
        .unwrap();

    let read_access = controller.accept(0, &read(0x0008, 65)).unwrap();
    let first_write = controller.accept(0, &write(0x0008, 66)).unwrap();
    let second_write = controller.accept(0, &write(0x0048, 67)).unwrap();

    assert_eq!(profile.nvm_media_timing(), Some(media_timing));
    assert_eq!(
        profile.controller_config().nvm_media_timing(),
        Some(media_timing)
    );
    assert_eq!(read_access.ready_cycle(), 40);
    assert_eq!(read_access.dram_access().persistent_ready_cycle(), None);
    assert_eq!(first_write.ready_cycle(), 46);
    assert_eq!(first_write.dram_access().persistent_ready_cycle(), Some(96));
    assert_eq!(second_write.dram_access().command_cycle(), 96);
    assert_eq!(second_write.ready_cycle(), 102);
    assert_eq!(
        second_write.dram_access().persistent_ready_cycle(),
        Some(152)
    );

    let activity = controller.target_activity(profile.target()).unwrap();
    assert_eq!(activity.max_pending_persistent_writes(), 1);
}

#[test]
fn nvm_media_timing_limits_pending_reads() {
    let media_timing = NvmMediaTiming::new(30, 50, 6, 1, 4).unwrap();
    let profile = ExternalMemoryProfile::nvm(target(11), layout(), 2, 8, geometry(), timing())
        .unwrap()
        .with_nvm_media_timing(media_timing)
        .unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0000), vec![0x11; 64])
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0040), vec![0x22; 64])
        .unwrap();

    let first_read = controller.accept(0, &read(0x0000, 80)).unwrap();
    let second_read = controller.accept(0, &read(0x0040, 81)).unwrap();

    assert_eq!(first_read.dram_access().parallel_port(), 0);
    assert_eq!(first_read.dram_access().command_cycle(), 4);
    assert_eq!(first_read.ready_cycle(), 40);
    assert_eq!(first_read.dram_access().pending_nvm_read_count(), 1);

    assert_eq!(second_read.dram_access().parallel_port(), 1);
    assert_eq!(second_read.dram_access().command_cycle(), 40);
    assert_eq!(second_read.ready_cycle(), 76);
    assert_eq!(second_read.dram_access().pending_nvm_read_count(), 1);

    let activity = controller.target_activity(profile.target()).unwrap();
    assert_eq!(activity.max_pending_nvm_reads(), 1);
}

#[test]
fn nvm_media_timing_reports_pending_read_waits() {
    let media_timing = NvmMediaTiming::new(30, 50, 6, 1, 4).unwrap();
    let profile = ExternalMemoryProfile::nvm(target(12), layout(), 2, 8, geometry(), timing())
        .unwrap()
        .with_nvm_media_timing(media_timing)
        .unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0000), vec![0x11; 64])
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0040), vec![0x22; 64])
        .unwrap();
    let marker = controller.mark_wait_for();

    controller.accept(0, &read(0x0000, 90)).unwrap();
    controller.accept(0, &read(0x0040, 91)).unwrap();

    let graph = controller
        .target_wait_for_graph_since(&marker, profile.target())
        .unwrap()
        .snapshot();
    let request = WaitForNode::transaction("dram.target.12.agent.9.request.91").unwrap();
    let read_buffer = WaitForNode::resource("dram.target.12.nvm.read_buffer").unwrap();

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.first_observed_tick(), Some(4));
    assert_eq!(graph.last_observed_tick(), Some(39));
    assert!(graph.contains_edge(&request, &read_buffer, WaitForEdgeKind::Resource));
}

#[test]
fn nvm_media_timing_reports_pending_write_waits() {
    let media_timing = NvmMediaTiming::new(30, 50, 6, 4, 1).unwrap();
    let profile = ExternalMemoryProfile::nvm(target(13), layout(), 2, 8, geometry(), timing())
        .unwrap()
        .with_nvm_media_timing(media_timing)
        .unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0000), vec![0x11; 64])
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0040), vec![0x22; 64])
        .unwrap();
    let marker = controller.mark_wait_for();

    controller.accept(0, &write(0x0000, 100)).unwrap();
    controller.accept(0, &write(0x0040, 101)).unwrap();

    let graph = controller
        .target_wait_for_graph_since(&marker, profile.target())
        .unwrap()
        .snapshot();
    let request = WaitForNode::transaction("dram.target.13.agent.9.request.101").unwrap();
    let write_queue = WaitForNode::resource("dram.target.13.nvm.write_queue").unwrap();

    assert_eq!(graph.edge_count(), 1);
    assert_eq!(graph.first_observed_tick(), Some(4));
    assert_eq!(graph.last_observed_tick(), Some(59));
    assert!(graph.contains_edge(&request, &write_queue, WaitForEdgeKind::Resource));
}

#[test]
fn nvm_activity_reports_typed_media_bytes_and_persistent_writes() {
    let profile =
        ExternalMemoryProfile::nvm(target(14), layout(), 2, 8, geometry(), timing()).unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0000), vec![0x11; 64])
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0040), vec![0x22; 64])
        .unwrap();

    let read_access = controller.accept(0, &read(0x0008, 70)).unwrap();
    let write_access = controller.accept(0, &write(0x0010, 71)).unwrap();
    let writeback_access = controller.accept(0, &writeback_dirty(0x0040, 72)).unwrap();

    assert_eq!(read_access.dram_access().byte_count(), 8);
    assert_eq!(write_access.dram_access().byte_count(), 4);
    assert_eq!(writeback_access.dram_access().byte_count(), 64);

    let activity = controller.target_activity(profile.target()).unwrap();
    let activity_profile = activity.profile();

    assert_eq!(activity_profile.read_byte_count(), 8);
    assert_eq!(activity_profile.write_byte_count(), 68);
    assert_eq!(activity.persistent_write_count(), 2);
    assert_eq!(activity.persistent_write_byte_count(), 68);
}

#[test]
fn volatile_memory_activity_keeps_persistent_write_counters_zero() {
    let profile =
        ExternalMemoryProfile::ddr(target(15), layout(), 1, 1, geometry(), timing()).unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0000), vec![0x33; 64])
        .unwrap();

    controller.accept(0, &write(0x0004, 80)).unwrap();

    let activity = controller.target_activity(profile.target()).unwrap();

    assert_eq!(activity.profile().write_byte_count(), 4);
    assert_eq!(activity.persistent_write_count(), 0);
    assert_eq!(activity.persistent_write_byte_count(), 0);
}

#[test]
fn dram_target_activity_merge_preserves_unique_port_and_bank_coverage() {
    let profile =
        ExternalMemoryProfile::hbm(target(16), layout(), 1, 2, geometry(), timing()).unwrap();
    let mut controller = DramMemoryController::new();
    controller.add_profile(profile).unwrap();
    controller
        .map_region(
            profile.target(),
            Address::new(0x0000),
            AccessSize::new(0x4000).unwrap(),
        )
        .unwrap();
    controller
        .insert_line(profile.target(), Address::new(0x0000), vec![0x33; 64])
        .unwrap();

    let first_start = controller.mark_activity();
    controller.accept(0, &read(0x0000, 90)).unwrap();
    let first = controller
        .target_activity_since(&first_start, profile.target())
        .unwrap();
    let second_start = controller.mark_activity();
    controller.accept(20, &read(0x0008, 91)).unwrap();
    let second = controller
        .target_activity_since(&second_start, profile.target())
        .unwrap();
    let merged = first.merge_window(second);

    assert_eq!(merged.profile().access_count(), 2);
    assert_eq!(merged.profile().active_port_count(), 1);
    assert_eq!(merged.profile().active_bank_count(), 1);
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
