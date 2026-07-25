use serde_json::Value;

use super::mixed_compute_fixture::*;

#[test]
fn rem6_run_o3_persistent_iq_width_one_serializes_fp_vector_results_direct() {
    let json = run_mixed_compute_json(1, "direct", "detailed", &[]);
    assert_exact_architectural_results(&json);
    let admission_tick = assert_mixed_batch_admission(&json);
    assert_width_one_class_order(&json, admission_tick);
    assert_mixed_issue_reconciliation(&json);
    assert_fp_add_timing(&json);
}

#[test]
fn rem6_run_o3_persistent_iq_width_two_coissues_fp_vector_and_blocks_second_fp_direct() {
    let json = run_mixed_compute_json(2, "direct", "detailed", &[]);
    assert_exact_architectural_results(&json);
    let admission_tick = assert_mixed_batch_admission(&json);
    assert_fp_vector_coissue_and_fp_resource_block(&json, admission_tick);
    assert_mixed_issue_reconciliation(&json);
    assert_fp_add_timing(&json);
}

#[test]
fn rem6_run_o3_persistent_iq_width_four_mixed_compute_hierarchy() {
    let json = run_width_four_mixed_compute_json();
    assert_exact_architectural_results(&json);
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/issue/max_rows_per_cycle")
            .and_then(Value::as_u64),
        Some(4),
    );
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
    assert_mixed_width_four_batch(&json);
}

fn assert_mixed_batch_admission(json: &Value) -> u64 {
    let queued = [DIV_PC, FP_ADD_PC, VECTOR_RESULT_PC, SECOND_FP_PC]
        .map(|pc| queue_event_at_pc(json, pc, "queued"));
    let admission_tick = event_tick(queued[0]);
    assert!(
        queued
            .iter()
            .all(|event| event_tick(event) == admission_tick),
        "mixed rows must enter one bounded queue batch: {queued:#?}",
    );
    let load = o3_event_at_pc(json, LOAD_HEAD_PC);
    assert_eq!(
        load.pointer("/lsq_loads").and_then(Value::as_u64),
        Some(1),
        "batching head must be one real scalar load: {load}",
    );
    assert_eq!(
        load.pointer("/issue_tick").and_then(Value::as_u64),
        Some(admission_tick),
        "load issue must publish the complete younger queue batch: {load}",
    );
    admission_tick
}

fn assert_width_one_class_order(json: &Value, admission_tick: u64) {
    let selected = [DIV_PC, FP_ADD_PC, VECTOR_RESULT_PC, SECOND_FP_PC]
        .map(|pc| selected_event_at_pc(json, pc));
    assert_eq!(
        selected.map(event_class),
        [
            "integer_mul_div",
            "scalar_float",
            "vector_to_scalar",
            "scalar_float",
        ],
    );
    let selected_ticks = selected.map(event_tick);
    assert!(selected_ticks[0] >= admission_tick);
    assert!(
        selected_ticks.windows(2).all(|pair| pair[0] < pair[1]),
        "width-one mixed rows must serialize in queue order: {selected:#?}",
    );
}

fn assert_mixed_issue_reconciliation(json: &Value) {
    let issue = json
        .pointer("/cores/0/o3_runtime/issue")
        .expect("mixed-compute issue summary");
    assert_eq!(
        issue
            .pointer("/queue/enqueued_rows")
            .and_then(Value::as_u64),
        Some(4),
    );
    let issued_rows = issue
        .pointer("/issued_rows")
        .and_then(Value::as_u64)
        .expect("mixed-compute issued rows");
    assert_eq!(issued_rows, 4);

    let expected_classes = [
        ("scalar_integer", 0),
        ("integer_mul_div", 1),
        ("memory_agu", 0),
        ("control", 0),
        ("scalar_float", 2),
        ("vector_to_scalar", 1),
    ];
    let issued_by_class = issue
        .pointer("/queue/issued_by_class")
        .expect("mixed-compute class counters");
    let mut class_total = 0;
    for (class, expected) in expected_classes {
        let actual = issued_by_class
            .pointer(&format!("/{class}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing mixed-compute class counter {class}"));
        assert_eq!(actual, expected, "unexpected {class} issue count");
        class_total += actual;
    }
    assert_eq!(class_total, issued_rows);

    for pc in [DIV_PC, FP_ADD_PC, VECTOR_RESULT_PC, SECOND_FP_PC] {
        let selected = selected_event_at_pc(json, pc);
        let issued = o3_event_at_pc(json, pc);
        assert_eq!(
            issued.pointer("/issue_tick").and_then(Value::as_u64),
            Some(event_tick(selected)),
            "queue selection must match the O3 issue tick at {pc}",
        );
    }
}

fn assert_fp_add_timing(json: &Value) {
    let selected = selected_event_at_pc(json, FP_ADD_PC);
    let event = o3_event_at_pc(json, FP_ADD_PC);
    let issue_tick = event
        .pointer("/issue_tick")
        .and_then(Value::as_u64)
        .expect("FP add issue tick");
    let writeback_tick = event
        .pointer("/writeback_tick")
        .and_then(Value::as_u64)
        .expect("FP add writeback tick");
    assert!(
        event
            .pointer("/commit_tick")
            .and_then(Value::as_u64)
            .is_some_and(|commit_tick| commit_tick >= writeback_tick),
        "FP add must commit no earlier than writeback: {event}",
    );
    assert_eq!(issue_tick, event_tick(selected));
    assert_eq!(writeback_tick, issue_tick + 1);
    assert_eq!(
        event.pointer("/fu_latency_class").and_then(Value::as_str),
        Some("scalar_float_add"),
    );
    assert_eq!(
        event.pointer("/fu_latency_cycles").and_then(Value::as_u64),
        Some(1),
    );
}

fn assert_fp_vector_coissue_and_fp_resource_block(json: &Value, admission_tick: u64) {
    let divide = selected_event_at_pc(json, DIV_PC);
    let fp = selected_event_at_pc(json, FP_ADD_PC);
    let vector = selected_event_at_pc(json, VECTOR_RESULT_PC);
    let lifecycle = mixed_compute_lifecycle(json);
    assert_eq!(event_class(divide), "integer_mul_div");
    assert_eq!(event_tick(divide), admission_tick);
    assert_eq!(event_class(fp), "scalar_float");
    assert_eq!(event_class(vector), "vector_to_scalar");
    let coissue_tick = event_tick(fp);
    assert_eq!(coissue_tick, admission_tick + 1);
    assert_eq!(
        event_tick(vector),
        coissue_tick,
        "mixed-compute queue lifecycle: {lifecycle:#?}",
    );

    let retained = super::queue_events(json)
        .iter()
        .find(|event| {
            event.pointer("/pc").and_then(Value::as_str) == Some(SECOND_FP_PC)
                && event.pointer("/action").and_then(Value::as_str) == Some("retained_resource")
                && event.pointer("/service_tick").and_then(Value::as_u64) == Some(coissue_tick)
        })
        .unwrap_or_else(|| panic!("missing retained second FP row: {lifecycle:#?}"));
    assert_eq!(event_class(retained), "scalar_float");
    let next_wake = retained
        .pointer("/next_wake_tick")
        .and_then(Value::as_u64)
        .expect("retained FP row must request a wake");
    assert!(next_wake > coissue_tick);

    let second_fp = selected_event_at_pc(json, SECOND_FP_PC);
    assert!(event_tick(second_fp) >= next_wake);
    assert!(
        json.pointer("/cores/0/o3_runtime/issue/resource_blocked_row_cycles")
            .and_then(Value::as_u64)
            .is_some_and(|cycles| cycles > 0),
        "mixed-class fixture must expose resource pressure: {json}",
    );
}

fn mixed_compute_lifecycle(json: &Value) -> Vec<&Value> {
    super::queue_events(json)
        .iter()
        .filter(|event| {
            matches!(
                event.pointer("/pc").and_then(Value::as_str),
                Some(DIV_PC | FP_ADD_PC | VECTOR_RESULT_PC | SECOND_FP_PC)
            )
        })
        .collect()
}

fn selected_event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    queue_event_at_pc(json, pc, "selected")
}

fn event_class(event: &Value) -> &str {
    event
        .pointer("/issue_class")
        .and_then(Value::as_str)
        .expect("mixed-compute event class")
}

fn event_tick(event: &Value) -> u64 {
    event
        .pointer("/service_tick")
        .and_then(Value::as_u64)
        .expect("mixed-compute service tick")
}

pub(super) fn o3_event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    let matches = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .map(|events| {
            events
                .iter()
                .filter(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| panic!("missing mixed-compute O3 trace at {pc}"));
    assert_eq!(
        matches.len(),
        1,
        "expected one mixed-compute O3 event at {pc}: {matches:#?}",
    );
    matches[0]
}
