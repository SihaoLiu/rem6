use rem6_cpu::{O3LiveIssueTelemetry, O3LiveIssueTraceRecord};

use crate::formatting::json_escape;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3IssueQueueTelemetryValue {
    enqueued_rows: u64,
    service_turns: u64,
    wake_requests: u64,
    current_occupancy: u64,
    peak_occupancy: u64,
    scalar_integer_issued_rows: u64,
    integer_mul_div_issued_rows: u64,
    memory_agu_issued_rows: u64,
    control_issued_rows: u64,
    scalar_float_issued_rows: u64,
    vector_to_scalar_issued_rows: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3IssueQueueEventValue {
    sequence: u64,
    pc: u64,
    action: &'static str,
    issue_class: &'static str,
    service_tick: u64,
    next_wake_tick: Option<u64>,
    raw_writeback_tick: Option<u64>,
    admitted_writeback_tick: Option<u64>,
    cleanup_boundary: Option<u64>,
}

impl From<O3LiveIssueTelemetry> for O3IssueQueueTelemetryValue {
    fn from(telemetry: O3LiveIssueTelemetry) -> Self {
        Self {
            enqueued_rows: telemetry.enqueued_rows(),
            service_turns: telemetry.service_turns(),
            wake_requests: telemetry.wake_requests(),
            current_occupancy: telemetry.current_occupancy(),
            peak_occupancy: telemetry.peak_occupancy(),
            scalar_integer_issued_rows: telemetry.scalar_integer_issued_rows(),
            integer_mul_div_issued_rows: telemetry.integer_mul_div_issued_rows(),
            memory_agu_issued_rows: telemetry.memory_agu_issued_rows(),
            control_issued_rows: telemetry.control_issued_rows(),
            scalar_float_issued_rows: telemetry.scalar_float_issued_rows(),
            vector_to_scalar_issued_rows: telemetry.vector_to_scalar_issued_rows(),
        }
    }
}

impl From<&O3LiveIssueTraceRecord> for O3IssueQueueEventValue {
    fn from(event: &O3LiveIssueTraceRecord) -> Self {
        Self {
            sequence: event.sequence(),
            pc: event.pc().get(),
            action: event.action().name(),
            issue_class: event.issue_class().name(),
            service_tick: event.service_tick(),
            next_wake_tick: event.next_wake_tick(),
            raw_writeback_tick: event.raw_writeback_tick(),
            admitted_writeback_tick: event.admitted_writeback_tick(),
            cleanup_boundary: event.cleanup_boundary(),
        }
    }
}

pub(super) fn o3_issue_queue_to_json(
    telemetry: O3LiveIssueTelemetry,
    events: &[O3LiveIssueTraceRecord],
) -> String {
    let events = events
        .iter()
        .map(O3IssueQueueEventValue::from)
        .collect::<Vec<_>>();
    issue_queue_values_to_json(telemetry.into(), &events)
}

fn issue_queue_values_to_json(
    telemetry: O3IssueQueueTelemetryValue,
    events: &[O3IssueQueueEventValue],
) -> String {
    let events = events
        .iter()
        .map(|event| {
            format!(
                "{{\"sequence\":{},\"pc\":\"{:#x}\",\"action\":\"{}\",\"issue_class\":\"{}\",\"service_tick\":{},\"next_wake_tick\":{},\"raw_writeback_tick\":{},\"admitted_writeback_tick\":{},\"cleanup_boundary\":{}}}",
                event.sequence,
                event.pc,
                json_escape(event.action),
                json_escape(event.issue_class),
                event.service_tick,
                optional_u64_json(event.next_wake_tick),
                optional_u64_json(event.raw_writeback_tick),
                optional_u64_json(event.admitted_writeback_tick),
                optional_u64_json(event.cleanup_boundary),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"telemetry\":{},\"events\":[{}]}}",
        telemetry_json(telemetry),
        events
    )
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn telemetry_json(telemetry: O3IssueQueueTelemetryValue) -> String {
    let issued_by_class = format!(
        "{{\"scalar_integer\":{},\"integer_mul_div\":{},\"memory_agu\":{},\"control\":{},\"scalar_float\":{},\"vector_to_scalar\":{}}}",
        telemetry.scalar_integer_issued_rows,
        telemetry.integer_mul_div_issued_rows,
        telemetry.memory_agu_issued_rows,
        telemetry.control_issued_rows,
        telemetry.scalar_float_issued_rows,
        telemetry.vector_to_scalar_issued_rows,
    );
    format!(
        "{{\"enqueued_rows\":{},\"service_turns\":{},\"wake_requests\":{},\"current_occupancy\":{},\"peak_occupancy\":{},\"issued_by_class\":{}}}",
        telemetry.enqueued_rows,
        telemetry.service_turns,
        telemetry.wake_requests,
        telemetry.current_occupancy,
        telemetry.peak_occupancy,
        issued_by_class,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    const EVENT_KEYS: [&str; 9] = [
        "sequence",
        "pc",
        "action",
        "issue_class",
        "service_tick",
        "next_wake_tick",
        "raw_writeback_tick",
        "admitted_writeback_tick",
        "cleanup_boundary",
    ];

    #[test]
    fn o3_issue_queue_debug_json_serializes_exact_telemetry_fixture() {
        let json = issue_queue_values_to_json(
            O3IssueQueueTelemetryValue {
                enqueued_rows: 4,
                service_turns: 3,
                wake_requests: 3,
                current_occupancy: 0,
                peak_occupancy: 4,
                scalar_integer_issued_rows: 1,
                integer_mul_div_issued_rows: 1,
                memory_agu_issued_rows: 1,
                control_issued_rows: 1,
                scalar_float_issued_rows: 5,
                vector_to_scalar_issued_rows: 6,
            },
            &[],
        );

        assert_eq!(
            serde_json::from_str::<Value>(&json).unwrap(),
            json!({
                "telemetry": {
                    "enqueued_rows": 4,
                    "service_turns": 3,
                    "wake_requests": 3,
                    "current_occupancy": 0,
                    "peak_occupancy": 4,
                    "issued_by_class": {
                        "scalar_integer": 1,
                        "integer_mul_div": 1,
                        "memory_agu": 1,
                        "control": 1,
                        "scalar_float": 5,
                        "vector_to_scalar": 6,
                    },
                },
                "events": [],
            })
        );
    }

    #[test]
    fn o3_issue_queue_debug_json_serializes_stable_lifecycle_event_actions() {
        let expected_actions = [
            "queued",
            "selected",
            "retained_resource",
            "retained_dependency",
            "replayed",
            "squashed",
            "retired",
        ];
        let events = expected_actions
            .iter()
            .enumerate()
            .map(|(index, action)| O3IssueQueueEventValue {
                sequence: u64::try_from(index + 1).unwrap(),
                pc: 0x8000_0000 + u64::try_from(index * 4).unwrap(),
                action: *action,
                issue_class: "scalar_integer",
                service_tick: 10 + u64::try_from(index).unwrap(),
                next_wake_tick: Some(20 + u64::try_from(index).unwrap()),
                raw_writeback_tick: (*action == "selected").then_some(30),
                admitted_writeback_tick: (*action == "selected").then_some(31),
                cleanup_boundary: matches!(*action, "replayed" | "squashed" | "retired")
                    .then_some(40),
            })
            .collect::<Vec<_>>();
        let json = issue_queue_values_to_json(O3IssueQueueTelemetryValue::default(), &events);
        let value = serde_json::from_str::<Value>(&json).unwrap();
        let serialized = value.pointer("/events").unwrap().as_array().unwrap();

        assert_eq!(serialized.len(), expected_actions.len());
        for (event, expected_action) in serialized.iter().zip(expected_actions) {
            let object = event.as_object().unwrap();
            assert_eq!(
                object.len(),
                EVENT_KEYS.len(),
                "unexpected event keys: {event}"
            );
            for key in EVENT_KEYS {
                assert!(object.contains_key(key), "event is missing {key}: {event}");
            }
            assert_eq!(
                event.pointer("/action").and_then(Value::as_str),
                Some(expected_action)
            );
        }
    }
}
