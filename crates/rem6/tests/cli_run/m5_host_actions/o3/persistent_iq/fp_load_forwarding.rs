use serde_json::Value;

use super::fp_load_forwarding_fixture::*;
use super::mixed_compute_fixture::queue_event_at_pc;
use super::*;

#[test]
fn rem6_run_o3_fp_load_forwarding_width_one_flw_direct() {
    let run = FpLoadForwardingRun::width_one_flw_direct();
    let completed = run.completed_json();
    assert_completed_architecture(run.precision, &completed);
    let admitted_tick = assert_completed_lifecycle(run, &completed);
    assert_queue_and_writeback_reconciliation(&completed);
    assert_bounded_no_early_publication(run, run.bounded_json(admitted_tick - 1), admitted_tick);
}

#[test]
fn rem6_run_o3_fp_load_forwarding_width_two_fld_direct() {
    let run = FpLoadForwardingRun::width_two_fld_direct();
    let completed = run.completed_json();
    assert_completed_architecture(run.precision, &completed);
    let admitted_tick = assert_width_two_exact_fit(run, &completed);
    assert_eq!(assert_completed_lifecycle(run, &completed), admitted_tick);
    assert_queue_and_writeback_reconciliation(&completed);
    assert_bounded_no_early_publication(run, run.bounded_json(admitted_tick - 1), admitted_tick);
}

#[test]
fn rem6_run_o3_fp_load_forwarding_width_four_precision_matrix_hierarchy() {
    for precision in [FpLoadPrecision::Single, FpLoadPrecision::Double] {
        let run = FpLoadForwardingRun::width_four_hierarchy(precision);
        let completed = run.completed_json();
        assert_completed_architecture(precision, &completed);
        let admitted_tick = assert_width_one_fixed_fu_collision_delay(run, &completed);
        let target_request = assert_exact_memory_timing(run, &completed);
        assert_eq!(target_request.admitted_tick, admitted_tick);
        assert_eq!(assert_completed_lifecycle(run, &completed), admitted_tick);
        assert_hierarchy_activity(&completed);
        assert_queue_and_writeback_reconciliation(&completed);
        let before_target = run.bounded_json(target_request.request_tick - 1);
        let through_target_response = run.bounded_json(target_request.response_tick);
        assert_target_hierarchy_activity_window(
            run,
            target_request,
            &before_target,
            &through_target_response,
        );
        assert_bounded_no_early_publication(
            run,
            run.bounded_json(admitted_tick - 1),
            admitted_tick,
        );
    }
}

fn assert_completed_architecture(precision: FpLoadPrecision, json: &Value) {
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(precision.result_hex()),
        "FP load forwarding result: {json}",
    );
    assert_eq!(register_value(json, "x6"), 0, "fflags must remain clear");
}

fn assert_completed_lifecycle(run: FpLoadForwardingRun, json: &Value) -> u64 {
    let load = super::mixed_compute::o3_event_at_pc(json, run.load_pc());
    let multiply = super::mixed_compute::o3_event_at_pc(json, run.multiply_pc());
    let add = super::mixed_compute::o3_event_at_pc(json, run.add_pc());
    let store = super::mixed_compute::o3_event_at_pc(json, run.store_pc());
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
                && record.pointer("/size").and_then(Value::as_u64) == Some(run.precision.bytes())
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

    let multiply_queued = queue_event_at_pc(json, run.multiply_pc(), "queued");
    let multiply_selected = queue_event_at_pc(json, run.multiply_pc(), "selected");
    let multiply_sequence = event_u64(multiply_queued, "sequence");
    let load_sequence = event_u64(load, "sequence");
    let expected_multiply_sequence = load_sequence + 1 + run.collision_rows_before_multiply();
    assert_eq!(multiply_sequence, expected_multiply_sequence);
    assert_eq!(event_u64(multiply, "sequence"), multiply_sequence);
    assert!(
        event_u64(multiply_queued, "service_tick") < response_tick,
        "consumer must queue before response: queued={multiply_queued}; load={load}",
    );
    assert_dependency_lifecycle_and_terminal_selection(
        json,
        run.multiply_pc(),
        multiply_sequence,
        load_sequence,
        1,
        admitted_tick,
    );
    assert_eq!(event_u64(multiply_selected, "service_tick"), admitted_tick);
    assert_eq!(
        multiply_selected
            .pointer("/issue_class")
            .and_then(Value::as_str),
        Some("scalar_float"),
    );

    let add_queued = queue_event_at_pc(json, run.add_pc(), "queued");
    let add_selected = queue_event_at_pc(json, run.add_pc(), "selected");
    let add_sequence = event_u64(add_queued, "sequence");
    assert_eq!(add_sequence, multiply_sequence + 1);
    assert_eq!(event_u64(add, "sequence"), add_sequence);
    assert_eq!(event_u64(store, "sequence"), add_sequence + 1);
    assert_eq!(
        store.pointer("/lsq_operation").and_then(Value::as_str),
        Some("float_store"),
    );
    assert_eq!(
        store.pointer("/lsq_stores").and_then(Value::as_u64),
        Some(1)
    );
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
        "dependent FP rows must issue in producer order"
    );
    assert!(event_u64(load, "commit_tick") <= event_u64(multiply, "commit_tick"));
    assert!(event_u64(multiply, "commit_tick") <= event_u64(add, "commit_tick"));
    assert!(event_u64(add, "commit_tick") <= event_u64(store, "commit_tick"));
    admitted_tick
}

fn assert_dependency_lifecycle_and_terminal_selection(
    json: &Value,
    pc: &str,
    sequence: u64,
    producer_sequence: u64,
    producer_register: u64,
    producer_writeback: u64,
) {
    let lifecycle = super::queue_events(json)
        .iter()
        .filter(|event| event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence))
        .collect::<Vec<_>>();
    assert_eq!(
        lifecycle
            .iter()
            .filter(|event| event.pointer("/action").and_then(Value::as_str) == Some("queued"))
            .count(),
        1,
        "one queue insertion for {pc}: {lifecycle:#?}",
    );
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
    let correlated = retained
        .iter()
        .find(|event| {
            event
                .pointer("/data_producers")
                .and_then(Value::as_array)
                .is_some_and(|producers| {
                    producers.len() == 1
                        && producers[0].pointer("/sequence").and_then(Value::as_u64)
                            == Some(producer_sequence)
                        && producers[0]
                            .pointer("/register_class")
                            .and_then(Value::as_str)
                            == Some("floating_point")
                        && producers[0]
                            .pointer("/architectural")
                            .and_then(Value::as_u64)
                            == Some(producer_register)
                })
                && event.pointer("/next_wake_tick").and_then(Value::as_u64)
                    == Some(producer_writeback)
        })
        .unwrap_or_else(|| panic!("missing exact typed producer wake: {lifecycle:#?}"));
    assert_eq!(
        correlated
            .pointer("/next_wake_tick")
            .and_then(Value::as_u64),
        Some(producer_writeback),
        "dependency must advertise wake {producer_writeback}: {lifecycle:#?}",
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
    let issued = super::mixed_compute::o3_event_at_pc(json, pc);
    assert_eq!(event_u64(issued, "sequence"), sequence);
    assert_eq!(event_u64(issued, "issue_tick"), producer_writeback);
    // Selection destructively removes a live queue row; there is no separate removed action.
    assert_eq!(
        lifecycle.last().and_then(|event| event.pointer("/action")).and_then(Value::as_str),
        Some("selected"),
        "terminal selection must be the destructive removal for consumer {sequence}: {lifecycle:#?}",
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/issue/queue/current_occupancy")
            .and_then(Value::as_u64),
        Some(0),
        "terminal consumer selection must reconcile to an empty queue",
    );
}

fn assert_width_two_exact_fit(run: FpLoadForwardingRun, json: &Value) -> u64 {
    let peer_pc = run.peer_pc().expect("width-two D peer");
    let load = super::mixed_compute::o3_event_at_pc(json, run.load_pc());
    let peer = super::mixed_compute::o3_event_at_pc(json, peer_pc);
    let load_raw_ready = event_u64(load, "lsq_data_response_tick") + 1;
    let peer_raw_ready = raw_ready_tick(peer);
    assert_eq!(
        load_raw_ready, peer_raw_ready,
        "D load and exact FP peer must collide: load={load}; peer={peer}",
    );
    assert_eq!(event_u64(load, "writeback_tick"), load_raw_ready);
    assert_eq!(event_u64(peer, "writeback_tick"), peer_raw_ready);

    let peer_queued = queue_event_at_pc(json, peer_pc, "queued");
    let peer_selected = queue_event_at_pc(json, peer_pc, "selected");
    let multiply_queued = queue_event_at_pc(json, run.multiply_pc(), "queued");
    assert_ne!(
        peer_queued.pointer("/sequence"),
        multiply_queued.pointer("/sequence")
    );
    assert_eq!(
        event_u64(peer_queued, "sequence"),
        event_u64(load, "sequence") + 1
    );
    assert_eq!(
        event_u64(peer, "sequence"),
        event_u64(peer_queued, "sequence")
    );
    assert_eq!(
        event_u64(multiply_queued, "sequence"),
        event_u64(peer_queued, "sequence") + 1,
    );
    assert_eq!(
        event_u64(peer_selected, "raw_writeback_tick"),
        peer_raw_ready
    );
    assert_eq!(
        event_u64(peer_selected, "admitted_writeback_tick"),
        peer_raw_ready,
    );
    assert!(
        super::queue_events(json).iter().all(|event| {
            event.pointer("/sequence") != multiply_queued.pointer("/sequence")
                || event.pointer("/action").and_then(Value::as_str) != Some("selected")
                || event_u64(event, "service_tick") > event_u64(load, "lsq_data_response_tick")
        }),
        "dependent row bypassed the response/admission boundary",
    );

    let writeback = json
        .pointer("/cores/0/o3_runtime/writeback_port")
        .unwrap_or_else(|| panic!("missing width-two writeback surface: {json}"));
    assert_eq!(
        writeback
            .pointer("/max_ready_rows_per_cycle")
            .and_then(Value::as_u64),
        Some(2),
    );
    assert_eq!(
        writeback.pointer("/deferred_rows").and_then(Value::as_u64),
        Some(0),
    );
    load_raw_ready
}

#[derive(Clone, Copy, Debug)]
struct HierarchyTargetRequest {
    request_agent: u64,
    request_sequence: u64,
    request_tick: u64,
    response_tick: u64,
    admitted_tick: u64,
}

fn assert_exact_memory_timing(run: FpLoadForwardingRun, json: &Value) -> HierarchyTargetRequest {
    let load = super::mixed_compute::o3_event_at_pc(json, run.load_pc());
    let data = data_trace(json)
        .iter()
        .filter(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("load")
                && record.pointer("/address").and_then(Value::as_str) == Some("0x800000c0")
                && record.pointer("/size").and_then(Value::as_u64) == Some(run.precision.bytes())
        })
        .collect::<Vec<_>>();
    assert_eq!(data.len(), 1, "one exact post-switch FP load: {data:#?}");
    let request_agent = event_u64(data[0], "request_agent");
    let request_sequence = event_u64(data[0], "request_sequence");
    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing FP hierarchy memory trace: {json}"))
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/request_agent").and_then(Value::as_u64) == Some(request_agent)
                && record.pointer("/request").and_then(Value::as_u64) == Some(request_sequence)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        memory.len(),
        3,
        "one exact request/response identity: {memory:#?}"
    );
    for (record, kind, endpoint) in [
        (memory[0], "request_sent", "cpu0.dmem"),
        (memory[1], "request_arrived", "memory"),
        (memory[2], "response_arrived", "cpu0.dmem"),
    ] {
        assert_eq!(record.pointer("/kind").and_then(Value::as_str), Some(kind));
        assert_eq!(
            record.pointer("/endpoint").and_then(Value::as_str),
            Some(endpoint),
        );
    }
    let request_tick = event_u64(memory[0], "tick");
    let request_arrival_tick = event_u64(memory[1], "tick");
    let response_tick = event_u64(memory[2], "tick");
    let route = event_u64(memory[0], "route");
    assert!(
        memory
            .iter()
            .all(|record| event_u64(record, "route") == route),
        "target request route must remain stable: {memory:#?}",
    );
    let packet = ((route & 0x7fff) << 48)
        | ((request_agent & 0xffff) << 32)
        | (request_sequence & 0xffff_ffff);
    let hops = json
        .pointer("/memory_resources/fabric/hop_activities")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing target fabric hops: {json}"))
        .iter()
        .filter(|hop| event_u64(hop, "packet") == packet)
        .collect::<Vec<_>>();
    assert_eq!(hops.len(), 1, "one target request fabric hop: {hops:#?}");
    assert_eq!(event_u64(hops[0], "ready_tick"), request_tick);
    assert_eq!(event_u64(hops[0], "arrival_tick"), request_arrival_tick);
    assert_eq!(event_u64(hops[0], "bytes"), run.precision.bytes());
    assert_eq!(event_u64(hops[0], "virtual_network"), 1);
    let raw_ready_tick = response_tick + 1;
    let admitted_tick = event_u64(load, "writeback_tick");
    assert_eq!(request_tick, event_u64(load, "issue_tick"));
    assert_eq!(request_tick, 95);
    assert!(request_tick < request_arrival_tick);
    assert!(request_arrival_tick < response_tick);
    assert_eq!(response_tick, 111);
    assert_eq!(event_u64(data[0], "tick"), response_tick);
    assert_eq!(event_u64(load, "lsq_data_response_tick"), response_tick);
    assert_eq!(
        memory[2]
            .pointer("/response_status")
            .and_then(Value::as_str),
        Some("completed"),
    );
    assert_eq!(
        event_u64(memory[2], "response_latency_ticks"),
        response_tick - request_tick,
    );
    assert_eq!(admitted_tick, raw_ready_tick + 1);
    assert_eq!(raw_ready_tick, 112);
    assert_eq!(admitted_tick, 113);
    assert_eq!(event_u64(load, "commit_tick"), admitted_tick);
    assert_eq!(load.pointer("/lsq_loads").and_then(Value::as_u64), Some(1));
    assert_eq!(load.pointer("/lsq_stores").and_then(Value::as_u64), Some(0));
    HierarchyTargetRequest {
        request_agent,
        request_sequence,
        request_tick,
        response_tick,
        admitted_tick,
    }
}

fn assert_target_hierarchy_activity_window(
    run: FpLoadForwardingRun,
    target: HierarchyTargetRequest,
    before_target: &Value,
    through_response: &Value,
) {
    assert_eq!(
        before_target
            .pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(target.request_tick - 1),
    );
    assert_eq!(
        through_response
            .pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(target.response_tick),
    );
    assert!(
        target_data_records(before_target, target).is_empty(),
        "target request must not exist at tick {}",
        target.request_tick - 1,
    );
    let completed = target_data_records(through_response, target);
    assert_eq!(
        completed.len(),
        1,
        "one target response by its exact identity"
    );
    assert_eq!(event_u64(completed[0], "tick"), target.response_tick);
    assert_eq!(
        completed[0].pointer("/address").and_then(Value::as_str),
        Some("0x800000c0"),
    );
    assert_eq!(
        completed[0].pointer("/size").and_then(Value::as_u64),
        Some(run.precision.bytes()),
    );

    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        let before = before_target
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing pre-target activity {pointer}: {before_target}"));
        let after = through_response
            .pointer(pointer)
            .and_then(Value::as_u64)
            .unwrap_or_else(|| {
                panic!("missing through-response activity {pointer}: {through_response}")
            });
        assert!(before > 0, "pre-target {pointer} must already be nonzero");
        assert!(
            after > before,
            "target request must increase {pointer}: before={before}, after={after}",
        );
    }

    let events = through_response
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    for pc in [run.multiply_pc(), run.add_pc(), run.store_pc()] {
        assert!(
            events
                .iter()
                .all(|event| event.pointer("/pc").and_then(Value::as_str) != Some(pc)),
            "dependent row {pc} must not execute by target response: {events:#?}",
        );
    }
}

fn target_data_records(json: &Value, target: HierarchyTargetRequest) -> Vec<&Value> {
    data_trace(json)
        .iter()
        .filter(|record| {
            record.pointer("/request_agent").and_then(Value::as_u64) == Some(target.request_agent)
                && record.pointer("/request_sequence").and_then(Value::as_u64)
                    == Some(target.request_sequence)
        })
        .collect()
}

fn assert_width_one_fixed_fu_collision_delay(run: FpLoadForwardingRun, json: &Value) -> u64 {
    let fixed_rows = [
        (
            FP_LOAD_HIERARCHY_DIVIDE_BLOCKER_ZERO_PC,
            "scalar_integer_div",
            19,
            50,
            69,
        ),
        (
            FP_LOAD_HIERARCHY_BLOCKER_ONE_PC,
            "scalar_integer_div",
            19,
            55,
            74,
        ),
        (
            FP_LOAD_HIERARCHY_BLOCKER_TWO_PC,
            "scalar_integer_div",
            19,
            59,
            78,
        ),
        (
            FP_LOAD_HIERARCHY_BLOCKER_THREE_PC,
            "scalar_integer_div",
            19,
            63,
            82,
        ),
        (
            FP_LOAD_HIERARCHY_AUX_PREPEER_PC,
            "scalar_integer_div",
            19,
            74,
            93,
        ),
        (FP_LOAD_HIERARCHY_GATE_PC, "scalar_integer_mul", 2, 78, 80),
        (
            FP_LOAD_HIERARCHY_FIXED_PC,
            "scalar_integer_div",
            19,
            93,
            112,
        ),
    ];
    let actual_timing = fixed_rows
        .iter()
        .map(|(pc, _, _, _, _)| {
            let event = super::mixed_compute::o3_event_at_pc(json, pc);
            (*pc, event_u64(event, "issue_tick"), raw_ready_tick(event))
        })
        .collect::<Vec<_>>();
    for (expected_sequence, (pc, class, latency, issue_tick, raw_ready)) in
        fixed_rows.into_iter().enumerate()
    {
        let event = super::mixed_compute::o3_event_at_pc(json, pc);
        assert_eq!(
            event_u64(event, "sequence"),
            expected_sequence as u64,
            "{pc}"
        );
        assert_eq!(
            event.pointer("/fu_latency_class").and_then(Value::as_str),
            Some(class),
            "{pc}",
        );
        assert_eq!(event_u64(event, "fu_latency_cycles"), latency, "{pc}");
        assert_eq!(
            event_u64(event, "issue_tick"),
            issue_tick,
            "{pc}: {actual_timing:?}",
        );
        assert_eq!(raw_ready_tick(event), raw_ready, "{pc}: {actual_timing:?}",);
        assert_eq!(event_u64(event, "writeback_tick"), raw_ready, "{pc}");
    }
    let fixed_pc = run
        .hierarchy_fixed_pc()
        .expect("width-one hierarchy fixed-FU peer");
    let fixed = super::mixed_compute::o3_event_at_pc(json, fixed_pc);
    let load = super::mixed_compute::o3_event_at_pc(json, run.load_pc());
    let fixed_sequence = event_u64(fixed, "sequence");
    let load_sequence = event_u64(load, "sequence");
    assert_eq!(fixed_sequence, 6);
    assert_eq!(fixed_sequence + 1, load_sequence);
    assert_eq!(load_sequence, 7);
    assert_eq!(
        load.pointer("/lsq_operation").and_then(Value::as_str),
        Some("float_load"),
    );
    assert_eq!(
        load.pointer("/lsq_load_address").and_then(Value::as_str),
        Some("0x800000c0"),
    );
    assert_eq!(
        load.pointer("/lsq_load_bytes").and_then(Value::as_u64),
        Some(run.precision.bytes()),
    );
    let fixed_issue = event_u64(fixed, "issue_tick");
    let load_issue = event_u64(load, "issue_tick");
    let load_response = event_u64(load, "lsq_data_response_tick");
    let fixed_raw_ready = fixed_issue + event_u64(fixed, "fu_latency_cycles");
    let fixed_admitted = event_u64(fixed, "writeback_tick");
    let load_raw_ready = event_u64(load, "lsq_data_response_tick") + 1;
    let load_admitted_tick = event_u64(load, "writeback_tick");
    let multiply_queued = queue_event_at_pc(json, run.multiply_pc(), "queued");
    let multiply_selected = queue_event_at_pc(json, run.multiply_pc(), "selected");
    let fetch_trace = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing hierarchy fetch trace: {json}"));
    let actual_fetch_timing = [run.load_pc(), run.multiply_pc()].map(|pc| {
        let tick = fetch_trace
            .iter()
            .find(|record| record.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .map(|record| event_u64(record, "tick"));
        (pc, tick)
    });
    for (pc, expected_tick) in [(run.load_pc(), 82), (run.multiply_pc(), 86)] {
        let matches = fetch_trace
            .iter()
            .filter(|record| record.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "one exact fetch for {pc}: {matches:#?}");
        assert_eq!(
            event_u64(matches[0], "tick"),
            expected_tick,
            "{pc}: {actual_fetch_timing:?}",
        );
    }
    assert_eq!(fixed_issue, 93);
    assert_eq!(load_issue, 95);
    assert_eq!(load_response, 111);
    assert!(
        event_u64(multiply_queued, "service_tick") < load_response,
        "target consumer must queue before target response: queued={multiply_queued}; target={load}",
    );
    assert_eq!(fixed_admitted, fixed_raw_ready);
    assert_eq!(
        fixed_raw_ready, load_raw_ready,
        "hierarchy fixed FU and FP load must collide: fixed={fixed}; target={load}",
    );
    assert_eq!(load_admitted_tick, load_raw_ready + 1);
    assert_eq!(event_u64(fixed, "commit_tick"), 112);
    assert_eq!(event_u64(load, "commit_tick"), 113);
    assert_eq!(
        event_u64(multiply_selected, "service_tick"),
        load_admitted_tick,
        "target FMUL must wake only when the delayed FP load is admitted",
    );
    let issue = json
        .pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing hierarchy issue surface: {json}"));
    assert_eq!(
        issue.pointer("/configured_width").and_then(Value::as_u64),
        Some(4),
    );
    assert_eq!(
        issue
            .pointer("/configured_memory_width")
            .and_then(Value::as_u64),
        Some(1),
    );
    let writeback = json
        .pointer("/cores/0/o3_runtime/writeback_port")
        .unwrap_or_else(|| panic!("missing width-one writeback surface: {json}"));
    assert_eq!(
        writeback
            .pointer("/max_ready_rows_per_cycle")
            .and_then(Value::as_u64),
        Some(2),
    );
    assert_eq!(
        writeback.pointer("/deferred_rows").and_then(Value::as_u64),
        Some(1),
    );
    assert_eq!(
        writeback
            .pointer("/max_deferred_rows")
            .and_then(Value::as_u64),
        Some(1),
    );
    load_admitted_tick
}

fn raw_ready_tick(event: &Value) -> u64 {
    if event.pointer("/lsq_loads").and_then(Value::as_u64) == Some(1) {
        event_u64(event, "lsq_data_response_tick") + 1
    } else {
        event_u64(event, "issue_tick") + event_u64(event, "fu_latency_cycles")
    }
}

fn assert_hierarchy_activity(json: &Value) {
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

fn assert_queue_and_writeback_reconciliation(json: &Value) {
    let events = super::queue_events(json);
    let queued = events
        .iter()
        .filter(|event| event.pointer("/action").and_then(Value::as_str) == Some("queued"))
        .count() as u64;
    let selected = events
        .iter()
        .filter(|event| {
            event.pointer("/action").and_then(Value::as_str) == Some("selected")
                && event.pointer("/issue_class").and_then(Value::as_str) == Some("scalar_float")
        })
        .count() as u64;
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .unwrap_or_else(|| panic!("missing FP issue queue summary: {json}"));
    assert_eq!(
        queue.pointer("/enqueued_rows").and_then(Value::as_u64),
        Some(queued),
    );
    assert_eq!(
        queue
            .pointer("/issued_by_class/scalar_float")
            .and_then(Value::as_u64),
        Some(selected),
    );
    assert_eq!(
        queue.pointer("/current_occupancy").and_then(Value::as_u64),
        Some(0),
    );

    let dump = json
        .pointer("/host_actions/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing FP forwarding stats dump: {json}"));
    for (json_field, stat_field) in super::PERSISTENT_IQ_QUEUE_STATS {
        let value = queue
            .pointer(&format!("/{json_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing queue field {json_field}: {queue}"));
        assert_stats_dump_sample(
            dump,
            &format!("sim.host_actions.stats_dump.cpu0.o3.issue_queue.{stat_field}"),
            "counter",
            "Count",
            value,
            "resettable",
        );
    }

    let writeback = json
        .pointer("/cores/0/o3_runtime/writeback_port")
        .unwrap_or_else(|| panic!("missing FP writeback summary: {json}"));
    for (field, unit) in [
        ("cycles", "Cycle"),
        ("admitted_rows", "Count"),
        ("deferred_rows", "Count"),
        ("deferred_row_cycles", "Cycle"),
        ("max_ready_rows_per_cycle", "Count"),
        ("max_deferred_rows", "Count"),
    ] {
        let value = writeback
            .pointer(&format!("/{field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing writeback field {field}: {writeback}"));
        assert_stats_dump_sample(
            dump,
            &format!("sim.host_actions.stats_dump.cpu0.o3.writeback_port.{field}"),
            "counter",
            unit,
            value,
            "resettable",
        );
    }
}

fn assert_bounded_no_early_publication(run: FpLoadForwardingRun, json: Value, admitted_tick: u64) {
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(run.precision.zero_hex()),
        "bounded run must not publish the dependent store: {json}",
    );
    let dependent_pcs = [
        Some(run.multiply_pc()),
        Some(run.add_pc()),
        Some(run.store_pc()),
    ];
    assert!(super::queue_events(&json).iter().all(|event| {
        !event
            .pointer("/pc")
            .and_then(Value::as_str)
            .is_some_and(|pc| dependent_pcs.contains(&Some(pc)))
            || event.pointer("/action").and_then(Value::as_str) != Some("selected")
    }));
    let completed_events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    assert!(completed_events.iter().all(|event| {
        !event
            .pointer("/pc")
            .and_then(Value::as_str)
            .is_some_and(|pc| dependent_pcs.contains(&Some(pc)))
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
