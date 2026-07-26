use serde_json::Value;

use super::fp_load_forwarding_fixture::*;
use super::mixed_compute_fixture::queue_event_at_pc;
use super::*;

#[test]
fn rem6_run_o3_fp_load_forwarding_width_one_flw_direct() {
    let run = FpLoadForwardingRun::width_one_flw_direct();
    let completed = run.completed_json();
    assert_completed_architecture(&completed);
    let admitted_tick = assert_completed_lifecycle(&completed);
    assert_bounded_no_early_publication(run.bounded_json(admitted_tick - 1), admitted_tick);
}

fn assert_completed_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(FP_LOAD_RESULT_HEX),
        "FP load forwarding result: {json}",
    );
    assert_eq!(register_value(json, "x6"), 0, "fflags must remain clear");
}

fn assert_completed_lifecycle(json: &Value) -> u64 {
    let load = super::mixed_compute::o3_event_at_pc(json, FP_LOAD_PC);
    let multiply = super::mixed_compute::o3_event_at_pc(json, FP_LOAD_MUL_PC);
    let add = super::mixed_compute::o3_event_at_pc(json, FP_LOAD_ADD_PC);
    let store = super::mixed_compute::o3_event_at_pc(json, FP_LOAD_STORE_PC);
    assert_eq!(
        load.pointer("/lsq_operation").and_then(Value::as_str),
        Some("float_load"),
    );
    assert_eq!(load.pointer("/lsq_loads").and_then(Value::as_u64), Some(1));
    assert!(
        load.pointer("/rob_occupancy")
            .and_then(Value::as_u64)
            .is_some_and(|entries| entries >= 1),
        "FP load must occupy the ROB: {load}",
    );
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let admitted_tick = event_u64(load, "writeback_tick");
    assert!(response_tick < admitted_tick);
    assert!(admitted_tick <= event_u64(load, "commit_tick"));

    let load_requests = data_trace(json)
        .iter()
        .filter(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("load")
                && record.pointer("/address").and_then(Value::as_str) == Some("0x800000c0")
                && record.pointer("/size").and_then(Value::as_u64) == Some(4)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        load_requests.len(),
        1,
        "one exact FLW request: {:#?}",
        data_trace(json),
    );
    assert_eq!(event_u64(load_requests[0], "tick"), response_tick);
    assert!(event_u64(load, "issue_tick") < response_tick);

    let multiply_queued = queue_event_at_pc(json, FP_LOAD_MUL_PC, "queued");
    let multiply_selected = queue_event_at_pc(json, FP_LOAD_MUL_PC, "selected");
    let multiply_sequence = event_u64(multiply_queued, "sequence");
    let load_sequence = event_u64(load, "sequence");
    assert_eq!(multiply_sequence, load_sequence + 1);
    assert!(
        event_u64(multiply_queued, "service_tick") < response_tick,
        "consumer must queue before response: queued={multiply_queued}; load={load}",
    );
    assert_dependency_wake(json, multiply_sequence, load_sequence, admitted_tick);
    assert_eq!(event_u64(multiply_selected, "service_tick"), admitted_tick);
    assert_eq!(
        multiply_selected
            .pointer("/issue_class")
            .and_then(Value::as_str),
        Some("scalar_float"),
    );

    let add_queued = queue_event_at_pc(json, FP_LOAD_ADD_PC, "queued");
    let add_selected = queue_event_at_pc(json, FP_LOAD_ADD_PC, "selected");
    let add_sequence = event_u64(add_queued, "sequence");
    assert_eq!(add_sequence, multiply_sequence + 1);
    assert!(
        event_u64(add_queued, "service_tick") >= event_u64(multiply, "writeback_tick"),
        "post-boundary add must wait for the multiply writeback",
    );
    assert_eq!(
        add_selected.pointer("/issue_class").and_then(Value::as_str),
        Some("scalar_float"),
    );

    let multiply_issue = event_u64(multiply, "issue_tick");
    let add_issue = event_u64(add, "issue_tick");
    assert_eq!(multiply_issue, event_u64(multiply_selected, "service_tick"));
    assert_eq!(add_issue, event_u64(add_selected, "service_tick"));
    assert!(
        multiply_issue < add_issue,
        "width-one FP rows must serialize"
    );
    assert!(event_u64(load, "commit_tick") <= event_u64(multiply, "commit_tick"));
    assert!(event_u64(multiply, "commit_tick") <= event_u64(add, "commit_tick"));
    assert!(event_u64(add, "commit_tick") <= event_u64(store, "commit_tick"));
    admitted_tick
}

fn assert_dependency_wake(
    json: &Value,
    sequence: u64,
    producer_sequence: u64,
    producer_writeback: u64,
) {
    let lifecycle = super::queue_events(json)
        .iter()
        .filter(|event| event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence))
        .collect::<Vec<_>>();
    let retained = lifecycle
        .iter()
        .filter(|event| {
            event.pointer("/action").and_then(Value::as_str) == Some("retained_dependency")
        })
        .collect::<Vec<_>>();
    assert!(
        retained
            .iter()
            .any(|event| event_u64(event, "service_tick") < producer_writeback),
        "missing retained dependency before {producer_writeback}: {lifecycle:#?}",
    );
    let advertised = retained
        .iter()
        .find(|event| {
            event.pointer("/next_wake_tick").and_then(Value::as_u64) == Some(producer_writeback)
        })
        .unwrap_or_else(|| {
            panic!("dependency must advertise wake {producer_writeback}: {lifecycle:#?}")
        });
    let producers = advertised
        .pointer("/data_producers")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing typed data producers: {advertised}"));
    assert_eq!(
        producers.len(),
        1,
        "one exact FP load producer: {advertised}"
    );
    assert_eq!(event_u64(&producers[0], "sequence"), producer_sequence);
    assert_eq!(
        producers[0]
            .pointer("/register_class")
            .and_then(Value::as_str),
        Some("floating_point"),
    );
    assert_eq!(
        producers[0]
            .pointer("/architectural")
            .and_then(Value::as_u64),
        Some(1),
    );
    let selected = lifecycle
        .iter()
        .filter(|event| event.pointer("/action").and_then(Value::as_str) == Some("selected"))
        .collect::<Vec<_>>();
    assert_eq!(
        selected.len(),
        1,
        "one dependency wake selection: {lifecycle:#?}"
    );
    assert_eq!(event_u64(selected[0], "service_tick"), producer_writeback);
}

fn assert_bounded_no_early_publication(json: Value, admitted_tick: u64) {
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("00000000"),
        "bounded run must not publish the dependent store: {json}",
    );
    assert!(super::queue_events(&json).iter().all(|event| {
        !matches!(
            event.pointer("/pc").and_then(Value::as_str),
            Some(FP_LOAD_MUL_PC | FP_LOAD_ADD_PC | FP_LOAD_STORE_PC)
        ) || event.pointer("/action").and_then(Value::as_str) != Some("selected")
    }));
    let completed_events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    assert!(completed_events.iter().all(|event| {
        !matches!(
            event.pointer("/pc").and_then(Value::as_str),
            Some(FP_LOAD_MUL_PC | FP_LOAD_ADD_PC | FP_LOAD_STORE_PC)
        )
    }));
    assert_eq!(register_value(&json, "x6"), 0);
    assert_eq!(
        json.pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(admitted_tick - 1),
    );
}

fn data_trace(json: &Value) -> &[Value] {
    json.pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("missing FP load data trace: {json}"))
}
