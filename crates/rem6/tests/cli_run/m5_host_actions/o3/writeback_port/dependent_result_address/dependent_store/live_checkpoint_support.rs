use super::*;

const O3_LIVE_CHECKPOINT_CHUNK: &str = "o3-live-checkpoint";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PendingStoreLiveSchedule {
    pub(super) checkpoint_delivery_tick: u64,
    pub(super) checkpoint_source_tick: u64,
    pub(super) restore_source_tick: u64,
}

impl PendingStoreLiveSchedule {
    pub(super) fn discover(baseline: &Value) -> Self {
        let store = memory_result_event_at_pc(baseline, STORE_PC);
        let checkpoint_delivery_tick = event_u64(store, "issue_tick");
        let checkpoint_source_tick = checkpoint_delivery_tick
            .checked_sub(1)
            .expect("store must issue after tick zero");
        let restore_source_tick = event_u64(store, "commit_tick")
            .checked_add(1)
            .expect("store commit tick must leave room for restore");
        Self {
            checkpoint_delivery_tick,
            checkpoint_source_tick,
            restore_source_tick,
        }
    }
}

pub(super) fn assert_pending_store_live_window(
    fixture: &DependentStoreFixture,
    baseline: &Value,
    schedule: PendingStoreLiveSchedule,
) {
    let head = memory_result_event_at_pc(baseline, HEAD_PC);
    let store = memory_result_event_at_pc(baseline, STORE_PC);
    assert_eq!(
        event_u64(head, "writeback_tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_eq!(
        event_u64(head, "commit_tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_eq!(
        event_u64(store, "issue_tick"),
        schedule.checkpoint_delivery_tick
    );
    assert!(event_u64(store, "commit_tick") < schedule.restore_source_tick);

    let resident = fixture.run(schedule.checkpoint_source_tick, "detailed", &[]);
    let stores = rob_entries(&resident)
        .iter()
        .filter(|entry| {
            entry.pointer("/pc").and_then(Value::as_str) == Some(STORE_PC)
                && entry.pointer("/destination").is_some_and(Value::is_null)
        })
        .collect::<Vec<_>>();
    let [store_rob] = stores.as_slice() else {
        panic!("expected one destinationless store before checkpoint: {resident}");
    };
    let sequence = event_u64(store_rob, "sequence");
    assert_eq!(
        lsq_entries(&resident)
            .iter()
            .filter(|entry| {
                event_u64(entry, "sequence") == sequence
                    && event_str(entry, "kind") == "store"
                    && entry.pointer("/address").is_some_and(Value::is_null)
                    && event_u64(entry, "bytes") == 8
            })
            .count(),
        1
    );
    assert_eq!(data_requests_sent(&resident).len(), 1);
    assert!(data_trace(&resident)
        .iter()
        .all(|record| event_str(record, "kind") != "store"));
}

pub(super) fn assert_pending_store_live_restore(
    restored: &Value,
    baseline: &Value,
    row: DependentStoreRow,
    schedule: PendingStoreLiveSchedule,
) {
    assert_eq!(json_u64(restored, "/host_actions/checkpoint_count"), 1);
    assert_eq!(
        json_u64(restored, "/host_actions/checkpoint_restored_count"),
        1
    );
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("pending-store checkpoint action");
    let restore = restored
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("pending-store restore action");
    assert_eq!(
        event_u64(checkpoint, "tick"),
        schedule.checkpoint_delivery_tick
    );
    assert!(event_u64(restore, "tick") > schedule.checkpoint_delivery_tick);
    assert_eq!(
        event_u64(restore, "manifest_tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_live_chunk(checkpoint, 0, schedule.checkpoint_delivery_tick);
    assert_live_chunk(restore, 1, schedule.checkpoint_delivery_tick);

    let baseline_store = memory_result_event_at_pc(baseline, STORE_PC);
    let restored_store = memory_result_event_at_pc(restored, STORE_PC);
    for field in ["issue_tick", "commit_tick"] {
        assert_eq!(
            event_u64(restored_store, field),
            event_u64(baseline_store, field),
            "restored store {field}"
        );
    }
    let requests = data_requests_sent(restored);
    let [producer, store] = requests.as_slice() else {
        panic!("expected one producer and one restored store request: {requests:?}");
    };
    assert_eq!(
        event_u64(producer, "tick"),
        event_u64(memory_result_event_at_pc(baseline, HEAD_PC), "issue_tick")
    );
    assert_eq!(event_u64(store, "tick"), schedule.checkpoint_delivery_tick);
    assert!(event_u64(store, "tick") >= schedule.checkpoint_delivery_tick);

    let address = row.pointer.wrapping_add_signed(i64::from(row.offset));
    let address = format!("0x{address:x}");
    assert_eq!(
        data_trace(restored)
            .iter()
            .filter(|record| {
                event_str(record, "kind") == "store" && event_str(record, "address") == address
            })
            .count(),
        1
    );
    assert_eq!(
        restored.pointer("/cores/0/registers"),
        baseline.pointer("/cores/0/registers")
    );
    assert_eq!(restored.pointer("/memory"), baseline.pointer("/memory"));
}

pub(super) fn assert_pending_store_live_capture(
    captured: &Value,
    schedule: PendingStoreLiveSchedule,
) {
    assert_eq!(json_u64(captured, "/host_actions/checkpoint_count"), 1);
    assert_eq!(
        json_u64(captured, "/host_actions/checkpoint_restored_count"),
        0
    );
    let checkpoint = captured
        .pointer("/host_actions/checkpoints/0")
        .expect("pending-store checkpoint action");
    assert_eq!(
        event_u64(checkpoint, "tick"),
        schedule.checkpoint_delivery_tick
    );
    assert_live_chunk(checkpoint, 0, schedule.checkpoint_delivery_tick);
}

fn assert_live_chunk(action: &Value, rebound_wakes: u64, wake_tick: u64) {
    let chunks = action
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.pointer("/component").and_then(Value::as_str) == Some("cpu0")
            })
        })
        .and_then(|component| component.pointer("/chunks"))
        .and_then(Value::as_array)
        .expect("cpu0 checkpoint chunks");
    let matches = chunks
        .iter()
        .filter(|chunk| {
            chunk.pointer("/name").and_then(Value::as_str) == Some(O3_LIVE_CHECKPOINT_CHUNK)
        })
        .collect::<Vec<_>>();
    let [chunk] = matches.as_slice() else {
        panic!("expected one O3LC chunk: {chunks:?}");
    };
    let live = chunk
        .pointer("/o3_live_checkpoint")
        .expect("decoded pending-store O3LC chunk");
    assert_eq!(
        live.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(live.pointer("/version").and_then(Value::as_u64), Some(2));
    assert_eq!(
        live.pointer("/profile").and_then(Value::as_str),
        Some("pending_data_address")
    );
    assert_eq!(
        live.pointer("/event_count").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        live.pointer("/resident_rows").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        live.pointer("/wake_tick").and_then(Value::as_u64),
        Some(wake_tick)
    );
    assert_eq!(
        live.pointer("/rebound_wakes").and_then(Value::as_u64),
        Some(rebound_wakes)
    );
}
