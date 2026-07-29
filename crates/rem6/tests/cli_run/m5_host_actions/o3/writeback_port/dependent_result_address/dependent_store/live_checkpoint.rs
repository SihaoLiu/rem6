use super::*;

#[path = "live_checkpoint_support.rs"]
mod live_checkpoint_support;
use live_checkpoint_support::*;

#[test]
fn rem6_run_o3_dependent_store_live_checkpoint_ld_direct() {
    let row = DEPENDENT_STORE_ROWS[0];
    let fixture = DependentStoreFixture::new(row);
    let baseline = fixture.run(row.max_tick, "detailed", &[]);
    let schedule = PendingStoreLiveSchedule::discover(&baseline);
    assert_pending_store_live_window(&fixture, &baseline, schedule);

    let checkpoint = format!("{}:pending-store-live", schedule.checkpoint_source_tick);
    let restore = format!("{}:pending-store-live", schedule.restore_source_tick);
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
