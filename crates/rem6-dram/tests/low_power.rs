use rem6_dram::{
    DramController, DramGeometry, DramLowPowerState, DramLowPowerTiming, DramRefreshTiming,
    DramTiming,
};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn geometry() -> DramGeometry {
    DramGeometry::new(16, 1024, 64).unwrap()
}

fn timing() -> DramTiming {
    DramTiming::new(4, 8, 10, 3, 5)
        .unwrap()
        .with_low_power_timing(DramLowPowerTiming::new(20, 80, 7).unwrap())
}

fn timing_with_split_exit_latency() -> DramTiming {
    DramTiming::new(4, 8, 10, 3, 5)
        .unwrap()
        .with_low_power_timing(
            DramLowPowerTiming::new(20, 80, 7)
                .unwrap()
                .with_self_refresh_exit_latency(17)
                .unwrap(),
        )
}

fn timing_with_refresh_and_low_power() -> DramTiming {
    timing()
        .with_refresh_timing(DramRefreshTiming::new(40, 5).unwrap())
        .unwrap()
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

#[test]
fn open_row_idle_records_active_powerdown_without_self_refresh() {
    let mut dram = DramController::new(geometry(), timing());

    let first = dram.schedule(0, &read(0x0000, 1)).unwrap();
    let second = dram.schedule(120, &read(0x0000, 2)).unwrap();

    assert_eq!(first.low_power_events(), &[]);
    assert_eq!(second.command_cycle(), 127);
    assert!(second.row_hit());
    assert_eq!(second.low_power_events().len(), 1);
    assert_eq!(
        second.low_power_events()[0].state(),
        DramLowPowerState::ActivePowerdown
    );
    assert_eq!(second.low_power_events()[0].entry_cycle(), 32);
    assert_eq!(second.low_power_events()[0].exit_cycle(), 120);
    assert_eq!(second.low_power_events()[0].cycle_count(), 88);

    let profile = dram.activity_profile();
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::ActivePowerdown),
        1
    );
    assert_eq!(
        profile.low_power_cycle_count(DramLowPowerState::ActivePowerdown),
        88
    );
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::PrechargePowerdown),
        0
    );
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::SelfRefresh),
        0
    );
    assert_eq!(profile.low_power_exit_count(), 1);
    assert_eq!(profile.low_power_exit_latency_cycles(), 7);
}

#[test]
fn closed_bank_idle_uses_self_refresh_exit_latency() {
    let mut dram = DramController::new(geometry(), timing_with_split_exit_latency());

    let access = dram.schedule(120, &read(0x0000, 1)).unwrap();

    assert!(!access.row_hit());
    assert_eq!(access.low_power_exit_latency_cycles(), 17);
    assert_eq!(access.low_power_events().len(), 2);
    assert_eq!(
        access.low_power_events()[0].state(),
        DramLowPowerState::PrechargePowerdown
    );
    assert_eq!(
        access.low_power_events()[1].state(),
        DramLowPowerState::SelfRefresh
    );

    let profile = dram.activity_profile();
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::PrechargePowerdown),
        1
    );
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::SelfRefresh),
        1
    );
    assert_eq!(profile.low_power_exit_latency_cycles(), 17);
}

#[test]
fn low_power_activity_ignores_idle_windows_below_entry_threshold() {
    let mut dram = DramController::new(geometry(), timing());

    dram.schedule(0, &read(0x0000, 1)).unwrap();
    let next = dram.schedule(20, &read(0x0000, 2)).unwrap();

    assert_eq!(next.command_cycle(), 20);
    assert_eq!(next.low_power_events(), &[]);
    assert_eq!(dram.activity_profile().low_power_exit_count(), 0);
}

#[test]
fn refresh_closes_open_row_before_low_power_accounting() {
    let mut dram = DramController::new(geometry(), timing_with_refresh_and_low_power());

    dram.schedule(0, &read(0x0000, 30)).unwrap();
    let after_refresh = dram.schedule(120, &read(0x0000, 31)).unwrap();

    assert_eq!(after_refresh.refresh_events().len(), 3);
    assert!(!after_refresh.row_hit());
    assert_eq!(after_refresh.command_cycle(), 129);
    assert_eq!(
        after_refresh
            .low_power_events()
            .iter()
            .filter(|event| event.state() == DramLowPowerState::ActivePowerdown)
            .count(),
        1
    );
    assert_eq!(
        after_refresh.low_power_events()[0].state(),
        DramLowPowerState::ActivePowerdown
    );
    assert_eq!(after_refresh.low_power_events()[0].entry_cycle(), 32);
    assert_eq!(after_refresh.low_power_events()[0].exit_cycle(), 40);

    let profile = dram.activity_profile();
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::PrechargePowerdown),
        2
    );
    assert_eq!(
        profile.low_power_cycle_count(DramLowPowerState::PrechargePowerdown),
        30
    );
}

#[test]
fn activity_profile_until_splits_terminal_low_power_around_refresh() {
    let mut dram = DramController::new(geometry(), timing_with_refresh_and_low_power());

    dram.schedule(0, &read(0x0000, 32)).unwrap();
    let profile = dram.activity_profile_until(100);

    assert_eq!(profile.refresh_count(), 32);
    assert_eq!(profile.refresh_cycle_count(), 160);
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::ActivePowerdown),
        1
    );
    assert_eq!(
        profile.low_power_cycle_count(DramLowPowerState::ActivePowerdown),
        8
    );
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::PrechargePowerdown),
        31
    );
    assert_eq!(
        profile.low_power_cycle_count(DramLowPowerState::PrechargePowerdown),
        480
    );
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::SelfRefresh),
        0
    );
}

#[test]
fn low_power_activity_since_marker_reports_only_later_idle_windows() {
    let mut dram = DramController::new(geometry(), timing());

    dram.schedule(0, &read(0x0000, 1)).unwrap();
    let marker = dram.mark_activity();
    dram.schedule(120, &read(0x0000, 2)).unwrap();

    let profile = dram.activity_profile_since(marker);
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::ActivePowerdown),
        1
    );
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::PrechargePowerdown),
        0
    );
    assert_eq!(profile.low_power_exit_latency_cycles(), 7);
}

#[test]
fn activity_profile_until_records_terminal_open_row_idle_window() {
    let mut dram = DramController::new(geometry(), timing());

    dram.schedule(0, &read(0x0000, 1)).unwrap();

    let profile = dram.activity_profile_until(120);
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::ActivePowerdown),
        1
    );
    assert_eq!(
        profile.low_power_cycle_count(DramLowPowerState::ActivePowerdown),
        88
    );
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::PrechargePowerdown),
        15
    );
    assert_eq!(
        profile.low_power_cycle_count(DramLowPowerState::PrechargePowerdown),
        900
    );
    assert_eq!(
        profile.low_power_entry_count(DramLowPowerState::SelfRefresh),
        15
    );
    assert_eq!(
        profile.low_power_cycle_count(DramLowPowerState::SelfRefresh),
        540
    );
    assert_eq!(profile.active_bank_count(), 16);
    assert_eq!(profile.low_power_exit_count(), 0);
    assert_eq!(profile.low_power_exit_latency_cycles(), 0);
}
