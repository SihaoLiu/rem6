use rem6_transport::{
    TransportMessageBuffer, TransportMessageBufferConfig, TransportMessageBufferError,
};

#[test]
fn strict_fifo_rejects_arrival_regression_without_mutating_state() {
    let mut buffer = TransportMessageBuffer::new(TransportMessageBufferConfig::strict_fifo());

    let first = buffer.enqueue(100, 5, "older").unwrap();

    assert_eq!(first.arrival_tick(), 105);
    assert_eq!(buffer.len(), 1);
    assert_eq!(buffer.last_arrival_tick(), Some(105));
    assert!(!buffer.last_message_bypassed_strict_fifo());

    let error = buffer.enqueue(101, 1, "younger").unwrap_err();

    assert_eq!(
        error,
        TransportMessageBufferError::StrictFifoArrivalRegression {
            current_tick: 101,
            delta: 1,
            arrival_tick: 102,
            last_arrival_tick: 105,
        }
    );
    assert_eq!(buffer.len(), 1);
    assert_eq!(buffer.last_arrival_tick(), Some(105));
    assert!(!buffer.last_message_bypassed_strict_fifo());
    assert!(buffer.pop_ready(104).is_none());
    assert_eq!(buffer.pop_ready(105).unwrap().into_payload(), "older");
    assert!(buffer.is_empty());
}

#[test]
fn bypass_records_intent_without_losing_deterministic_ready_order() {
    let mut buffer = TransportMessageBuffer::new(TransportMessageBufferConfig::strict_fifo());

    buffer.enqueue(100, 100, "baseline").unwrap();
    let bypassed = buffer
        .enqueue_bypassing_strict_fifo(150, 100, "bypassed")
        .unwrap();
    let after_bypass = buffer.enqueue(160, 50, "after bypass").unwrap();

    assert_eq!(bypassed.arrival_tick(), 250);
    assert_eq!(after_bypass.arrival_tick(), 210);
    assert_eq!(buffer.last_arrival_tick(), Some(210));
    assert!(!buffer.last_message_bypassed_strict_fifo());

    let ready = (0..3)
        .map(|_| buffer.pop_ready(250).unwrap().into_payload())
        .collect::<Vec<_>>();
    assert_eq!(ready, vec!["baseline", "after bypass", "bypassed"]);
}

#[test]
fn zero_latency_policy_is_typed_and_non_mutating() {
    let mut buffer = TransportMessageBuffer::new(TransportMessageBufferConfig::strict_fifo());

    assert_eq!(
        buffer.enqueue(44, 0, "zero").unwrap_err(),
        TransportMessageBufferError::ZeroLatency { current_tick: 44 }
    );
    assert!(buffer.is_empty());
    assert_eq!(buffer.last_arrival_tick(), None);

    let mut zero_allowed = TransportMessageBuffer::new(
        TransportMessageBufferConfig::strict_fifo().with_allow_zero_latency(true),
    );
    let admitted = zero_allowed.enqueue(44, 0, "zero").unwrap();

    assert_eq!(admitted.arrival_tick(), 44);
    assert_eq!(zero_allowed.pop_ready(44).unwrap().into_payload(), "zero");
}

#[test]
fn unordered_buffer_accepts_arrival_regressions() {
    let mut buffer = TransportMessageBuffer::new(TransportMessageBufferConfig::unordered());

    buffer.enqueue(100, 40, "later").unwrap();
    buffer.enqueue(101, 1, "earlier").unwrap();

    let ready = (0..2)
        .map(|_| buffer.pop_ready(140).unwrap().into_payload())
        .collect::<Vec<_>>();
    assert_eq!(ready, vec!["earlier", "later"]);
}

#[test]
fn snapshot_restore_preserves_fifo_guard_and_ready_order() {
    let mut buffer = TransportMessageBuffer::new(TransportMessageBufferConfig::strict_fifo());

    buffer.enqueue(10, 10, "first").unwrap();
    buffer.enqueue(20, 20, "second").unwrap();
    let snapshot = buffer.snapshot();

    assert_eq!(snapshot.len(), 2);
    assert_eq!(snapshot.last_arrival_tick(), Some(40));

    assert_eq!(buffer.pop_ready(40).unwrap().into_payload(), "first");
    buffer.enqueue(41, 10, "mutated").unwrap();
    assert_eq!(buffer.len(), 2);

    buffer.restore(snapshot).unwrap();

    assert_eq!(buffer.len(), 2);
    assert_eq!(
        buffer.enqueue(30, 5, "regression").unwrap_err(),
        TransportMessageBufferError::StrictFifoArrivalRegression {
            current_tick: 30,
            delta: 5,
            arrival_tick: 35,
            last_arrival_tick: 40,
        }
    );

    let ready = (0..2)
        .map(|_| buffer.pop_ready(40).unwrap().into_payload())
        .collect::<Vec<_>>();
    assert_eq!(ready, vec!["first", "second"]);
    assert!(buffer.is_empty());
}
