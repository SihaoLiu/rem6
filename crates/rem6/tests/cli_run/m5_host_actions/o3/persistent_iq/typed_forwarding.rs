use serde_json::Value;

use super::mixed_compute_fixture::*;
use super::typed_forwarding_fixture::*;
use super::*;

pub(super) fn assert_typed_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(TYPED_RESULTS),
        "typed forwarding architectural bytes: {json}",
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x13")
            .and_then(Value::as_str),
        Some("0xa"),
    );
}

pub(super) fn selected_event<'a>(json: &'a Value, pc: &str) -> &'a Value {
    queue_event_at_pc(json, pc, "selected")
}

pub(super) fn event_tick(event: &Value) -> u64 {
    event
        .pointer("/service_tick")
        .and_then(Value::as_u64)
        .expect("typed forwarding service tick")
}

fn event_class(event: &Value) -> &str {
    event
        .pointer("/issue_class")
        .and_then(Value::as_str)
        .expect("typed forwarding issue class")
}

pub(super) fn assert_typed_dependencies(json: &Value) {
    for (producer_pc, consumer_pc, producer_class, consumer_class) in [
        (
            TYPED_FP_PRODUCER_PC,
            TYPED_FP_CONSUMER_PC,
            "scalar_float",
            "scalar_float",
        ),
        (
            TYPED_VECTOR_PRODUCER_PC,
            TYPED_INTEGER_CONSUMER_PC,
            "vector_to_scalar",
            "scalar_integer",
        ),
    ] {
        let queued = queue_event_at_pc(json, consumer_pc, "queued");
        let selected = selected_event(json, consumer_pc);
        assert_eq!(queued.pointer("/sequence"), selected.pointer("/sequence"));
        let sequence = queued.pointer("/sequence").and_then(Value::as_u64).unwrap();
        let lifecycle = super::queue_events(json)
            .iter()
            .filter(|event| event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence))
            .collect::<Vec<_>>();
        assert_eq!(
            lifecycle
                .iter()
                .filter(|event| {
                    event.pointer("/action").and_then(Value::as_str) == Some("queued")
                })
                .count(),
            1,
        );
        assert_eq!(
            lifecycle
                .iter()
                .filter(|event| {
                    event.pointer("/action").and_then(Value::as_str) == Some("selected")
                })
                .count(),
            1,
        );
        assert!(lifecycle.iter().any(|event| {
            event.pointer("/action").and_then(Value::as_str) == Some("retained_dependency")
        }));
        assert_eq!(event_class(selected), consumer_class);
        assert_eq!(
            event_class(queue_event_at_pc(json, producer_pc, "selected")),
            producer_class,
        );
        let producer_writeback = event_u64(
            super::mixed_compute::o3_event_at_pc(json, producer_pc),
            "writeback_tick",
        );
        let retained = lifecycle
            .iter()
            .find(|event| {
                event.pointer("/action").and_then(Value::as_str) == Some("retained_dependency")
                    && event.pointer("/next_wake_tick").and_then(Value::as_u64)
                        == Some(producer_writeback)
            })
            .unwrap_or_else(|| panic!("missing typed dependency wake for {consumer_pc}: {json}"));
        assert_eq!(event_class(retained), consumer_class);
        assert!(event_tick(selected) >= producer_writeback);
    }
}

#[test]
fn rem6_run_o3_typed_live_forwarding_width_one_direct() {
    let json = run_typed_forwarding_json(1, "direct", "detailed", &[]);
    assert_typed_architecture(&json);
    assert_typed_dependencies(&json);
    let fp = selected_event(&json, TYPED_FP_CONSUMER_PC);
    let integer = selected_event(&json, TYPED_INTEGER_CONSUMER_PC);
    assert_ne!(event_tick(fp), event_tick(integer));
}

#[test]
fn rem6_run_o3_typed_live_forwarding_width_two_direct() {
    let json = run_typed_forwarding_json(2, "direct", "detailed", &[]);
    assert_typed_architecture(&json);
    assert_typed_dependencies(&json);
    assert!(json
        .pointer("/cores/0/o3_runtime/issue/dependency_blocked_row_cycles")
        .and_then(Value::as_u64)
        .is_some_and(|rows| rows >= 2),);
}

#[test]
fn rem6_run_o3_typed_live_forwarding_width_four_hierarchy() {
    let json = run_typed_forwarding_json(4, "cache-fabric-dram", "detailed", &[]);
    assert_typed_architecture(&json);
    assert_typed_dependencies(&json);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|activity| activity > 0),
            "missing hierarchy activity {pointer}: {json}",
        );
    }
}
