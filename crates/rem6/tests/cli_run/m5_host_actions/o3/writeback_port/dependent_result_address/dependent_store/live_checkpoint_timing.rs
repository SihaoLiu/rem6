use super::*;

pub(super) fn assert_pending_store_timing_control(
    timing: &Value,
    timing_baseline: &Value,
    detailed: &Value,
    schedule: PendingStoreLiveSchedule,
    host_event_delay: u64,
) {
    assert_eq!(json_u64(timing, "/host_actions/checkpoint_count"), 1);
    assert_eq!(
        json_u64(timing, "/host_actions/checkpoint_restored_count"),
        1
    );
    let checkpoint = timing
        .pointer("/host_actions/checkpoints/0")
        .expect("timing pending-store checkpoint");
    let restore = timing
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("timing pending-store restore");
    assert_eq!(
        event_u64(checkpoint, "tick"),
        schedule.checkpoint_source_tick + host_event_delay
    );
    assert_eq!(
        event_u64(restore, "tick"),
        schedule.restore_source_tick + host_event_delay
    );
    assert_eq!(
        event_u64(restore, "manifest_tick"),
        schedule.checkpoint_source_tick + host_event_delay
    );
    for action in [checkpoint, restore] {
        let chunks = cpu_checkpoint_chunks(action);
        assert!(chunks.iter().all(|chunk| {
            chunk.pointer("/name").and_then(Value::as_str) != Some(O3_LIVE_CHECKPOINT_CHUNK)
        }));
    }
    for pointer in [
        "/cores/0/committed_instructions",
        "/cores/0/registers",
        "/memory",
    ] {
        assert_eq!(
            timing.pointer(pointer),
            timing_baseline.pointer(pointer),
            "{pointer}"
        );
        assert_eq!(
            timing.pointer(pointer),
            detailed.pointer(pointer),
            "{pointer}"
        );
    }
    assert_timing_has_no_o3_surfaces(timing);
}

pub(super) fn pending_store_timing_control_delay(
    timing_baseline: &Value,
    schedule: PendingStoreLiveSchedule,
) -> u64 {
    let stop_tick = event_u64(
        timing_baseline
            .pointer("/host_actions/stops/0")
            .expect("timing baseline stop action"),
        "tick",
    );
    let delay = stop_tick
        .checked_sub(schedule.checkpoint_source_tick)
        .expect("timing stop follows detailed checkpoint source");
    assert!(schedule.restore_source_tick < stop_tick - 1);
    assert!(delay > 1);
    delay
}
