use super::*;

use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

const O3_LIVE_CHECKPOINT_CHUNK: &str = "o3-live-checkpoint";
const O3_RUNTIME_CHUNK: &str = "o3-runtime-state";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThreePendingLiveSchedule {
    checkpoint_delivery_tick: u64,
    checkpoint_source_tick: u64,
    restore_source_tick: u64,
}

impl ThreePendingLiveSchedule {
    fn discover(baseline: &Value) -> Self {
        let head = memory_result_event_at_pc(baseline, HEAD_PC);
        let pending = pending_memory_events(baseline);
        let checkpoint_delivery_tick = event_u64(head, "commit_tick");
        let checkpoint_source_tick = checkpoint_delivery_tick
            .checked_sub(1)
            .expect("head commit must occur after tick zero");
        let restore_source_tick = pending
            .iter()
            .map(|event| event_u64(event, "commit_tick"))
            .max()
            .expect("three pending events")
            .checked_add(1)
            .expect("pending commits must leave room for restore");
        Self {
            checkpoint_delivery_tick,
            checkpoint_source_tick,
            restore_source_tick,
        }
    }

    fn restore_delivery_tick(self) -> u64 {
        self.restore_source_tick
            .checked_add(1)
            .expect("restore source must leave room for delivery")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ThreePendingLiveCalibration {
    checkpoint_source_tick: u64,
    checkpoint_delivery_tick: u64,
    restore_source_tick: u64,
    restore_delivery_tick: u64,
}

impl ThreePendingLiveCalibration {
    pub(super) const fn new(
        checkpoint_source_tick: u64,
        checkpoint_delivery_tick: u64,
        restore_source_tick: u64,
        restore_delivery_tick: u64,
    ) -> Self {
        Self {
            checkpoint_source_tick,
            checkpoint_delivery_tick,
            restore_source_tick,
            restore_delivery_tick,
        }
    }
}

pub(super) fn run_three_pending_live_checkpoint_row(
    row: ThreePendingRow,
    calibration: ThreePendingLiveCalibration,
) {
    let fixture = ThreePendingFixture::new(row);
    let baseline = fixture.run(row.max_tick);
    let resident = assert_three_pending_resident(&fixture, &baseline);
    assert_three_pending_completed(&fixture, &baseline, &resident);
    let schedule = ThreePendingLiveSchedule::discover(&baseline);
    assert_live_schedule(schedule, calibration);
    assert_addressless_stable_graph(&fixture, &baseline, schedule);

    let label = format!("three-pending-live-{:?}", row.topology).to_lowercase();
    let checkpoint = format!("{}:{label}", schedule.checkpoint_source_tick);
    let restore = format!("{}:{label}", schedule.restore_source_tick);
    let captured = fixture.run_mode(
        row.max_tick,
        "detailed",
        &["--host-checkpoint", checkpoint.as_str()],
    );
    let restored = fixture.run_mode(
        row.max_tick,
        "detailed",
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );

    assert_live_capture(&captured, schedule);
    assert_live_restore(&restored, schedule);
    assert_exact_replay(&restored, &baseline, row);
}

pub(super) fn assert_three_pending_live_checkpoint_source_progress() {
    let row = row(ThreePendingTopology::Sibling, "direct", 2, 2, 9, 1_200);
    let fixture = ThreePendingFixture::new(row);
    let baseline = fixture.run(row.max_tick);
    let schedule = ThreePendingLiveSchedule::discover(&baseline);
    assert_live_schedule(
        schedule,
        ThreePendingLiveCalibration::new(307, 308, 330, 331),
    );
    let pre_publication_source_tick = schedule
        .checkpoint_source_tick
        .checked_sub(1)
        .expect("publication boundary must follow tick zero");
    assert_host_action_rejected(
        fixture.command(row.max_tick, "detailed"),
        "--host-checkpoint",
        &format!("{pre_publication_source_tick}:three-pending-pre-publication"),
        "three-pending pre-publication checkpoint",
    );

    assert_addressless_stable_graph(&fixture, &baseline, schedule);
    let checkpoint = format!(
        "{}:three-pending-post-publication",
        schedule.checkpoint_source_tick
    );
    let captured = fixture.run_mode(
        row.max_tick,
        "detailed",
        &["--host-checkpoint", checkpoint.as_str()],
    );
    assert_live_capture(&captured, schedule);
}

pub(super) fn assert_post_publication_graph_checkpoint_supported() {
    let row = row(
        ThreePendingTopology::MixedFanout,
        "cache-fabric-dram",
        2,
        2,
        80,
        12_000,
    );
    let fixture = ThreePendingFixture::new(row);
    let baseline = fixture.run(row.max_tick);
    let schedule = ThreePendingLiveSchedule::discover(&baseline);
    assert_live_schedule(
        schedule,
        ThreePendingLiveCalibration::new(2_798, 2_799, 3_149, 3_150),
    );
    assert_addressless_stable_graph(&fixture, &baseline, schedule);
    let checkpoint = format!(
        "{}:three-pending-post-publication-boundary",
        schedule.checkpoint_source_tick
    );
    let captured = fixture.run_mode(
        row.max_tick,
        "detailed",
        &["--host-checkpoint", checkpoint.as_str()],
    );
    assert_live_capture(&captured, schedule);
}

pub(super) fn assert_live_graph_handoff_rejected() {
    let row = row(ThreePendingTopology::Sibling, "direct", 2, 2, 9, 1_200);
    let fixture = ThreePendingFixture::new(row);
    let baseline = fixture.run(row.max_tick);
    let schedule = ThreePendingLiveSchedule::discover(&baseline);
    assert_addressless_stable_graph(&fixture, &baseline, schedule);
    assert_host_action_rejected(
        fixture.command(row.max_tick, "detailed"),
        "--host-switch-cpu-mode",
        &format!("{}:cpu0:timing", schedule.checkpoint_source_tick),
        "three-pending live-graph handoff",
    );
}

fn assert_live_schedule(
    schedule: ThreePendingLiveSchedule,
    calibration: ThreePendingLiveCalibration,
) {
    assert_eq!(
        schedule.checkpoint_source_tick,
        calibration.checkpoint_source_tick
    );
    assert_eq!(
        schedule.checkpoint_delivery_tick,
        calibration.checkpoint_delivery_tick
    );
    assert_eq!(
        schedule.restore_source_tick,
        calibration.restore_source_tick
    );
    assert_eq!(
        schedule.restore_delivery_tick(),
        calibration.restore_delivery_tick
    );
}

fn assert_addressless_stable_graph(
    fixture: &ThreePendingFixture,
    baseline: &Value,
    schedule: ThreePendingLiveSchedule,
) {
    let stable = fixture.run(schedule.checkpoint_delivery_tick);
    assert_snapshot_counts(&stable, 3, 3);
    let pending_sequences =
        pending_memory_events(baseline).map(|event| event_u64(event, "sequence"));
    assert_eq!(addressless_sequences(&stable), pending_sequences);
    let entries = lsq_entries(&stable);
    assert_eq!(entries.len(), 3, "stable pending LSQ rows: {stable}");
    for (entry, sequence) in entries.iter().zip(pending_sequences) {
        assert_eq!(event_u64(entry, "sequence"), sequence);
        assert_eq!(event_str(entry, "kind"), "load");
        assert_eq!(event_u64(entry, "bytes"), 8);
        assert!(entry.pointer("/address").is_some_and(Value::is_null));
    }
}

fn assert_live_capture(captured: &Value, schedule: ThreePendingLiveSchedule) {
    assert_eq!(json_u64(captured, "/host_actions/checkpoint_count"), 1);
    assert_eq!(
        json_u64(captured, "/host_actions/checkpoint_restored_count"),
        0
    );
    let checkpoint = captured
        .pointer("/host_actions/checkpoints/0")
        .expect("three-pending checkpoint action");
    assert_eq!(
        event_u64(checkpoint, "tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_eq!(
        event_u64(checkpoint, "manifest_tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_live_action(checkpoint, 0, schedule.checkpoint_delivery_tick);
}

fn assert_live_restore(restored: &Value, schedule: ThreePendingLiveSchedule) {
    assert_eq!(json_u64(restored, "/host_actions/checkpoint_count"), 1);
    assert_eq!(
        json_u64(restored, "/host_actions/checkpoint_restored_count"),
        1
    );
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("three-pending checkpoint action");
    let restore = restored
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("three-pending restore action");
    assert_eq!(
        event_u64(checkpoint, "tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_eq!(event_u64(restore, "tick"), schedule.restore_delivery_tick());
    assert_eq!(
        event_u64(restore, "manifest_tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_live_action(checkpoint, 0, schedule.checkpoint_delivery_tick);
    assert_live_action(restore, 1, schedule.checkpoint_delivery_tick);
}

fn assert_live_action(action: &Value, rebound_wakes: u64, wake_tick: u64) {
    let chunks = cpu_checkpoint_chunks(action);
    let live_chunks = chunks
        .iter()
        .filter(|chunk| {
            chunk.pointer("/name").and_then(Value::as_str) == Some(O3_LIVE_CHECKPOINT_CHUNK)
        })
        .collect::<Vec<_>>();
    let [chunk] = live_chunks.as_slice() else {
        panic!("expected exactly one O3LC chunk: {chunks:?}");
    };
    assert!(chunks.iter().all(|chunk| {
        chunk.pointer("/name").and_then(Value::as_str) != Some(RISCV_O3_LIVE_DATA_HANDOFF_CHUNK)
    }));
    let live = chunk
        .pointer("/o3_live_checkpoint")
        .expect("decoded three-pending O3LC chunk");
    assert_eq!(
        live.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(live.pointer("/version").and_then(Value::as_u64), Some(3));
    assert_eq!(
        live.pointer("/profile").and_then(Value::as_str),
        Some("pending_data_address")
    );
    for (field, expected) in [
        ("resident_rows", 3),
        ("event_count", 0),
        ("wake_tick", wake_tick),
        ("rebound_wakes", rebound_wakes),
    ] {
        assert_eq!(
            live.pointer(&format!("/{field}")).and_then(Value::as_u64),
            Some(expected),
            "O3LC {field}: {live}"
        );
    }
    let runtime_chunks = chunks
        .iter()
        .filter(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some(O3_RUNTIME_CHUNK))
        .collect::<Vec<_>>();
    let [runtime] = runtime_chunks.as_slice() else {
        panic!("expected exactly one O3 runtime chunk: {chunks:?}");
    };
    let runtime = runtime
        .pointer("/o3_runtime")
        .expect("decoded three-pending O3 runtime chunk");
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(3)
    );
}

fn assert_exact_replay(replay: &Value, baseline: &Value, row: ThreePendingRow) {
    for field in [
        "issue_tick",
        "lsq_data_response_tick",
        "writeback_tick",
        "commit_tick",
    ] {
        let actual = pending_memory_events(replay).map(|event| event_u64(event, field));
        let expected = pending_memory_events(baseline).map(|event| event_u64(event, field));
        assert_eq!(actual, expected, "pending PC timing field {field}");
    }
    for (actual, expected) in pending_memory_events(replay)
        .into_iter()
        .zip(pending_memory_events(baseline))
    {
        assert_eq!(
            actual.pointer("/lsq_load_address"),
            expected.pointer("/lsq_load_address"),
            "pending load address"
        );
    }
    assert_eq!(
        data_requests_sent(replay),
        data_requests_sent(baseline),
        "exact data request sequence/order"
    );
    for pointer in [
        "/cores/0/registers",
        "/cores/0/committed_instructions",
        "/debug/data_trace",
        "/memory",
        "/memory_resources",
        "/cores/0/o3_runtime/issue",
        "/cores/0/o3_runtime/writeback_port",
        "/cores/0/o3_runtime/lsq",
    ] {
        assert_eq!(
            replay.pointer(pointer),
            baseline.pointer(pointer),
            "exact replay parity at {pointer}"
        );
    }
    assert_three_pending_architecture(row, replay);
    assert_route_activity(replay, row.memory_system);
    assert_three_pending_drained(row, replay);
}

fn cpu_checkpoint_chunks(action: &Value) -> &[Value] {
    action
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str) == Some("cpu0")
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .map(Vec::as_slice)
        .expect("cpu0 checkpoint chunks")
}
