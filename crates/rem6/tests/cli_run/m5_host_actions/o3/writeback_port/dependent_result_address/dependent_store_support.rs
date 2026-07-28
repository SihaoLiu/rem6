use super::boundaries::BoundaryCase;
use super::*;

pub(super) fn data_memory_trace_event<'a>(
    json: &'a Value,
    route: u64,
    request: u64,
    kind: &str,
) -> &'a Value {
    let matches = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("memory trace")
        .iter()
        .filter(|record| {
            event_str(record, "channel") == "data"
                && event_str(record, "kind") == kind
                && event_u64(record, "route") == route
                && event_u64(record, "request") == request
        })
        .collect::<Vec<_>>();
    let [event] = matches.as_slice() else {
        panic!("expected one data {kind} route {route} request {request}: {matches:?}");
    };
    event
}

pub(super) fn assert_timing_has_no_o3_surfaces(timing: &Value) {
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let aliases = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(aliases.is_empty(), "timing O3 aliases: {aliases:?}");
}

pub(super) fn assert_store_boundary_counts<'a>(
    case: BoundaryCase,
    completed: &'a Value,
) -> Vec<&'a Value> {
    let trace_count = match case {
        BoundaryCase::DependentStoreAlias | BoundaryCase::SecondDependentLoad => 4,
        BoundaryCase::DependentStoreConditional => 2,
        _ => 3,
    };
    let request_count = match case {
        BoundaryCase::DependentStoreAlias => 3,
        BoundaryCase::MmioPointer => 2,
        _ => trace_count,
    };
    let sent = data_requests_sent(completed);
    assert_eq!(data_trace(completed).len(), trace_count, "{case:?}");
    assert_eq!(sent.len(), request_count, "{case:?}");
    if case == BoundaryCase::DependentStoreAlias {
        assert_eq!(
            data_trace(completed)
                .iter()
                .filter(|record| {
                    event_str(record, "kind") == "store"
                        && event_str(record, "address") == "0x80000140"
                })
                .count(),
            1
        );
    }
    sent
}
