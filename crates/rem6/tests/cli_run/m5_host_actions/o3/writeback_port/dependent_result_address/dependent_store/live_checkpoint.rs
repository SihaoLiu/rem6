use super::*;

#[path = "live_checkpoint_support.rs"]
mod live_checkpoint_support;
use live_checkpoint_support::*;

#[path = "live_checkpoint_boundaries.rs"]
mod live_checkpoint_boundaries;
use live_checkpoint_boundaries::assert_pending_store_live_boundaries;

#[path = "live_checkpoint_hierarchy.rs"]
mod live_checkpoint_hierarchy;
use live_checkpoint_hierarchy::{
    assert_hierarchy_retained_checkpoints_restore_out_of_order, assert_pending_store_live_hierarchy,
};

#[path = "live_checkpoint_timing.rs"]
mod live_checkpoint_timing;
use live_checkpoint_timing::{
    assert_pending_store_timing_control, pending_store_timing_control_delay,
};

#[test]
fn rem6_run_o3_dependent_store_live_checkpoint_ld_direct() {
    let row = DEPENDENT_STORE_ROWS[0];
    let fixture = DependentStoreFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let schedule = PendingStoreLiveSchedule::discover(&baseline);
    assert_pending_store_live_window(&fixture, &baseline, schedule);

    let checkpoint = format!("{}:pending-store-live", schedule.checkpoint_source_tick);
    let restore = format!("{}:pending-store-live", schedule.restore_source_tick);
    let switch = format!(
        "{}:cpu0:timing",
        schedule
            .restore_source_tick
            .checked_sub(1)
            .expect("mode switch must precede restore")
    );
    let no_restore = fixture.run(
        row.max_tick,
        "detailed",
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-switch-cpu-mode",
            switch.as_str(),
        ],
    );
    let restored = fixture.run(
        row.max_tick,
        "detailed",
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-switch-cpu-mode",
            switch.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );
    assert_pending_store_live_restore(&restored, &baseline, row, schedule);
    assert_pending_store_restore_replaces_divergent_mode(&restored, &no_restore);
}

#[test]
fn rem6_run_o3_dependent_store_live_checkpoint_window_is_natural() {
    let row = DEPENDENT_STORE_ROWS[0];
    let fixture = DependentStoreFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let schedule = PendingStoreLiveSchedule::discover(&baseline);
    assert_pending_store_live_window(&fixture, &baseline, schedule);

    let checkpoint = format!(
        "{}:pending-store-natural-window",
        schedule.checkpoint_source_tick
    );
    let captured = fixture.run(
        row.max_tick,
        "detailed",
        &["--host-checkpoint", checkpoint.as_str()],
    );
    assert_pending_store_live_capture(&captured, schedule);
}

#[test]
fn rem6_run_o3_dependent_store_live_checkpoint_amoswap_hierarchy() {
    let row = DEPENDENT_STORE_ROWS[3];
    let fixture = DependentStoreFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let schedule = PendingStoreLiveSchedule::discover_after_transport_drain(&baseline);
    assert_pending_store_live_window(&fixture, &baseline, schedule);

    let checkpoint = format!(
        "{}:pending-store-amoswap-hierarchy",
        schedule.checkpoint_source_tick
    );
    let captured = fixture.run(
        row.max_tick,
        "detailed",
        &["--host-checkpoint", checkpoint.as_str()],
    );
    let restore = format!(
        "{}:pending-store-amoswap-hierarchy",
        schedule.restore_source_tick
    );
    let restored = fixture.run(
        row.max_tick,
        "detailed",
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );
    assert_pending_store_live_restore(&restored, &baseline, row, schedule);
    assert_pending_store_live_hierarchy(&restored, &captured, &baseline, row, schedule);
    assert_hierarchy_retained_checkpoints_restore_out_of_order(&fixture, &baseline);
}

#[test]
fn rem6_run_o3_dependent_store_live_checkpoint_timing_control() {
    let row = DEPENDENT_STORE_ROWS[3];
    let fixture = DependentStoreFixture::new(row);
    let detailed = fixture.run(row.max_tick, "detailed", &[]);
    let schedule = PendingStoreLiveSchedule::discover_after_transport_drain(&detailed);
    let timing_baseline = fixture.run(row.max_tick, "timing", &[]);
    let host_event_delay = pending_store_timing_control_delay(&timing_baseline, schedule);
    let host_event_delay_arg = host_event_delay.to_string();
    let checkpoint = format!(
        "{}:pending-store-timing-control",
        schedule.checkpoint_source_tick
    );
    let restore = format!(
        "{}:pending-store-timing-control",
        schedule.restore_source_tick
    );
    let timing = fixture.run(
        row.max_tick,
        "timing",
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
            "--host-event-delay",
            host_event_delay_arg.as_str(),
        ],
    );

    assert_pending_store_timing_control(
        &timing,
        &timing_baseline,
        &detailed,
        schedule,
        host_event_delay,
    );
}

#[test]
fn rem6_run_o3_dependent_store_live_checkpoint_boundaries() {
    assert_pending_store_live_boundaries();
}
