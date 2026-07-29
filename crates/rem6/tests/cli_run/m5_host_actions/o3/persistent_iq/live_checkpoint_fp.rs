use serde_json::Value;

use super::fp_load_forwarding_fixture::*;
use super::fp_load_forwarding_runtime_boundaries::{
    assert_fp_load_architecture, run_fp_load_path_json, FpConsumerBoundary,
};
use super::live_checkpoint_fixture::*;
use super::*;

const MAX_TICK: u64 = 1_200;

#[test]
fn rem6_run_o3_live_checkpoint_flw_result_direct() {
    assert_live_fp_result_restore(FpLoadForwardingRun::width_one_flw_direct());
}

#[test]
fn rem6_run_o3_live_checkpoint_fld_result_direct() {
    assert_live_fp_result_restore(FpLoadForwardingRun::width_two_fld_direct());
}

#[test]
fn rem6_run_o3_live_checkpoint_fp_result_hierarchy_matrix() {
    for precision in [FpLoadPrecision::Single, FpLoadPrecision::Double] {
        assert_live_fp_result_restore(FpLoadForwardingRun::width_four_hierarchy(precision));
    }
}

fn assert_live_fp_result_restore(run: FpLoadForwardingRun) {
    let path = fp_load_forwarding_binary(run);
    let baseline = run_fp_load_path_json(run, &path, MAX_TICK, &[]);
    let boundary = FpConsumerBoundary::discover(&baseline, run);
    let restore_source_tick = if run.memory_system == "cache-fabric-dram" {
        let source_progress_tick =
            event_u64(o3_event_at_pc(&baseline, run.multiply_pc()), "commit_tick");
        let downstream_issue_tick =
            event_u64(o3_event_at_pc(&baseline, run.add_pc()), "issue_tick");
        let restore_source_tick = source_progress_tick;
        assert!(restore_source_tick + 1 < downstream_issue_tick);
        restore_source_tick
    } else {
        [
            run.load_pc(),
            run.multiply_pc(),
            run.add_pc(),
            run.store_pc(),
        ]
        .into_iter()
        .map(|pc| event_u64(o3_event_at_pc(&baseline, pc), "commit_tick"))
        .max()
        .expect("FP result path commit tick")
    };
    assert!(boundary.response_live_tick < restore_source_tick);

    let label = format!("o3-live-{}-result", run.precision.label());
    let checkpoint_source_tick = boundary.response_live_tick;
    let checkpoint = format!("{checkpoint_source_tick}:{label}");
    let restore = format!("{restore_source_tick}:{label}");
    let switch_source_tick = restore_source_tick
        .checked_sub(1)
        .expect("timing switch source precedes restore source");
    let switch = format!("{switch_source_tick}:cpu0:timing");
    let no_restore = run_fp_load_path_json(
        run,
        &path,
        MAX_TICK,
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-switch-cpu-mode",
            switch.as_str(),
        ],
    );
    let restored = run_fp_load_path_json(
        run,
        &path,
        MAX_TICK,
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-switch-cpu-mode",
            switch.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );

    let checkpoint_tick = boundary.response_live_tick + 1;
    let restore_tick = restore_source_tick + 1;
    let source_at_capture = (run.memory_system == "cache-fabric-dram")
        .then(|| run_fp_load_path_json(run, &path, checkpoint_tick, &[]));
    assert_live_fp_actions(
        &restored,
        run,
        &label,
        checkpoint_tick,
        restore_tick,
        &baseline,
    );
    assert_live_fp_restore_discriminator(&no_restore, &restored, run);
    assert_single_load_transport_identity(
        &restored,
        &baseline,
        source_at_capture.as_ref(),
        run,
        checkpoint_tick,
        restore_tick,
    );
    assert_restored_fp_pipeline(&restored, &baseline, run, restore_tick);
    assert_live_fp_final_state(&restored, &baseline, run);
}

fn assert_live_fp_actions(
    json: &Value,
    run: FpLoadForwardingRun,
    label: &str,
    checkpoint_tick: u64,
    restore_tick: u64,
    baseline: &Value,
) {
    assert_eq!(
        json.pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1),
    );
    assert_eq!(
        json.pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1),
    );
    let checkpoint = json
        .pointer("/host_actions/checkpoints/0")
        .expect("live FP checkpoint action");
    let restore = json
        .pointer("/host_actions/checkpoint_restores/0")
        .expect("live FP restore action");
    for (action, tick) in [(checkpoint, checkpoint_tick), (restore, restore_tick)] {
        assert_eq!(action.pointer("/tick").and_then(Value::as_u64), Some(tick));
        assert_eq!(
            action.pointer("/label").and_then(Value::as_str),
            Some(label)
        );
        assert_eq!(
            action
                .pointer("/execution_modes/0/mode")
                .and_then(Value::as_str),
            Some("detailed"),
        );
        let live =
            decoded_cpu_checkpoint_chunk(action, O3_LIVE_CHECKPOINT_CHUNK, "o3_live_checkpoint");
        assert_eq!(live.pointer("/version").and_then(Value::as_u64), Some(2));
        assert_eq!(
            live.pointer("/profile").and_then(Value::as_str),
            Some("completed_fp_load"),
        );
        assert_eq!(
            live.pointer("/writeback_reservations")
                .and_then(Value::as_u64),
            Some(1),
        );
        assert_eq!(
            live.pointer("/resident_rows").and_then(Value::as_u64),
            Some(1),
            "completed FP O3LC must own exactly the resident FMUL: {live}",
        );
        assert_eq!(
            live.pointer("/event_count").and_then(Value::as_u64),
            live.pointer("/resident_rows")
                .and_then(Value::as_u64)
                .map(|rows| rows + 1 + run.collision_rows_before_multiply()),
        );
        assert_eq!(
            live.pointer("/wake_tick").and_then(Value::as_u64),
            Some(event_u64(
                o3_event_at_pc(baseline, run.load_pc()),
                "writeback_tick"
            )),
        );
        assert_eq!(
            live.pointer("/decode_error").and_then(Value::as_bool),
            Some(false),
        );

        let runtime = decoded_cpu_checkpoint_chunk(action, O3_RUNTIME_CHUNK, "o3_runtime");
        assert_eq!(
            runtime
                .pointer("/checkpoint_version")
                .and_then(Value::as_u64),
            Some(23),
        );
        assert_eq!(
            runtime
                .pointer("/snapshot_lsq_entries")
                .and_then(Value::as_u64),
            Some(1),
        );
        assert_eq!(
            runtime
                .pointer("/snapshot_rob_entries")
                .and_then(Value::as_u64),
            live.pointer("/event_count").and_then(Value::as_u64),
        );
    }
    assert_eq!(
        checkpoint.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(checkpoint_tick),
    );
    assert_eq!(
        restore.pointer("/manifest_tick").and_then(Value::as_u64),
        Some(checkpoint_tick),
    );
}

fn assert_live_fp_restore_discriminator(
    no_restore: &Value,
    restored: &Value,
    run: FpLoadForwardingRun,
) {
    assert_fp_load_architecture(run.precision, no_restore);
    assert_eq!(
        no_restore
            .pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1),
    );
    assert_eq!(
        no_restore
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(0),
    );
    assert_eq!(
        no_restore
            .pointer("/host_actions/execution_modes/0/mode")
            .and_then(Value::as_str),
        Some("timing"),
        "without restore the intervening timing switch must remain installed: {no_restore}",
    );
    assert_eq!(
        no_restore
            .pointer("/cores/0/o3_runtime/execution_mode")
            .and_then(Value::as_str),
        Some("timing"),
    );
    assert_eq!(
        restored
            .pointer("/cores/0/o3_runtime/execution_mode")
            .and_then(Value::as_str),
        Some("detailed"),
        "restore must reinstall the checkpoint's detailed O3 mode: {restored}",
    );
}

fn assert_single_load_transport_identity(
    json: &Value,
    baseline: &Value,
    source_at_capture: Option<&Value>,
    run: FpLoadForwardingRun,
    checkpoint_tick: u64,
    restore_tick: u64,
) {
    let load = target_load_data_trace(json, run);
    let transport = target_load_transport(json, load);
    let baseline_load = target_load_data_trace(baseline, run);
    let baseline_transport = target_load_transport(baseline, baseline_load);
    assert_eq!(
        transport, baseline_transport,
        "checkpoint restore must preserve the target load's exact transport lifecycle",
    );
    assert_eq!(
        transport
            .iter()
            .map(|record| record.pointer("/kind").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        ["request_sent", "request_arrived", "response_arrived"],
        "one exact request/response identity: {transport:#?}",
    );
    if run.memory_system == "cache-fabric-dram" {
        let restored_data_channel = memory_data_channel_trace(json);
        let baseline_data_channel = memory_data_channel_trace(baseline);
        assert_eq!(
            restored_data_channel, baseline_data_channel,
            "hierarchy restore must reproduce every baseline data-channel lifecycle record without a duplicate, fresh, canceled, retried, failed, or incomplete identity",
        );
        let source_at_capture = source_at_capture.expect("bounded hierarchy source snapshot");
        let source_load = target_load_data_trace(source_at_capture, run);
        let source_transport = target_load_transport(source_at_capture, source_load);
        assert_eq!(
            source_transport, baseline_transport,
            "the full target lifecycle must already exist in the source before checkpoint",
        );
        assert!(source_transport
            .iter()
            .all(|record| event_u64(record, "tick") < checkpoint_tick));
        assert_hierarchy_activity(source_at_capture);
    }
    assert!(
        transport
            .iter()
            .all(|record| event_u64(record, "tick") < checkpoint_tick),
        "the completed response must predate capture: {transport:#?}",
    );
    assert!(
        transport
            .iter()
            .all(|record| event_u64(record, "tick") < restore_tick),
        "restore must not issue the captured data identity again: {transport:#?}",
    );
    assert_eq!(
        transport
            .iter()
            .filter(|record| event_u64(record, "tick") >= restore_tick)
            .count(),
        0,
        "captured load must have no transport record after restore: {transport:#?}",
    );
}

fn target_load_data_trace(json: &Value, run: FpLoadForwardingRun) -> &Value {
    let loads = data_trace(json)
        .iter()
        .filter(|record| {
            record.pointer("/kind").and_then(Value::as_str) == Some("load")
                && record.pointer("/address").and_then(Value::as_str) == Some("0x800000c0")
                && record.pointer("/size").and_then(Value::as_u64) == Some(run.precision.bytes())
        })
        .collect::<Vec<_>>();
    assert_eq!(loads.len(), 1, "one completed FP data result: {loads:#?}");
    loads[0]
}

fn target_load_transport<'a>(json: &'a Value, load: &Value) -> Vec<&'a Value> {
    let request_agent = event_u64(load, "request_agent");
    let request_sequence = event_u64(load, "request_sequence");
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("live FP memory trace")
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/request_agent").and_then(Value::as_u64) == Some(request_agent)
                && record.pointer("/request").and_then(Value::as_u64) == Some(request_sequence)
        })
        .collect()
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
            "missing live checkpoint hierarchy activity {pointer}: {json}",
        );
    }
}

fn memory_data_channel_trace(json: &Value) -> Vec<&Value> {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("live FP memory trace")
        .iter()
        .filter(|record| record.pointer("/channel").and_then(Value::as_str) == Some("data"))
        .collect()
}

fn assert_restored_fp_pipeline(
    restored: &Value,
    baseline: &Value,
    run: FpLoadForwardingRun,
    restore_tick: u64,
) {
    let load = o3_event_at_pc(restored, run.load_pc());
    let baseline_load = o3_event_at_pc(baseline, run.load_pc());
    for field in [
        "sequence",
        "issue_tick",
        "lsq_data_response_tick",
        "writeback_tick",
        "commit_tick",
    ] {
        assert_eq!(
            load.pointer(&format!("/{field}")),
            baseline_load.pointer(&format!("/{field}")),
            "restored load publication {field}: {load}",
        );
    }
    assert!(event_u64(load, "commit_tick") < restore_tick);

    let multiply = o3_event_at_pc(restored, run.multiply_pc());
    let add = o3_event_at_pc(restored, run.add_pc());
    let store = o3_event_at_pc(restored, run.store_pc());
    let multiply_sequence = event_u64(multiply, "sequence");
    let load_sequence = event_u64(load, "sequence");
    assert_eq!(
        multiply_sequence,
        load_sequence + 1 + run.collision_rows_before_multiply(),
        "restored FP sequence span for {run:?}: load={load}, multiply={multiply}",
    );
    for (event, pc) in [(multiply, run.multiply_pc()), (add, run.add_pc())] {
        let selected = super::mixed_compute_fixture::queue_event_at_pc(restored, pc, "selected");
        let baseline_event = o3_event_at_pc(baseline, pc);
        let baseline_selected =
            super::mixed_compute_fixture::queue_event_at_pc(baseline, pc, "selected");
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event.pointer(&format!("/{field}")),
                baseline_event.pointer(&format!("/{field}")),
                "restored dependent FP {field} at {pc}",
            );
        }
        assert_eq!(
            selected.pointer("/service_tick"),
            baseline_selected.pointer("/service_tick"),
            "restored dependent FP selection at {pc}",
        );
        assert_eq!(
            event_u64(event, "issue_tick"),
            event_u64(selected, "service_tick"),
            "restored dependent FP issue at {pc}",
        );
        if pc == run.multiply_pc() {
            assert_eq!(
                selected.pointer("/service_tick"),
                load.pointer("/writeback_tick")
            );
        }
        assert_eq!(
            selected.pointer("/issue_class").and_then(Value::as_str),
            Some("scalar_float"),
        );
    }
    let add_queued =
        super::mixed_compute_fixture::queue_event_at_pc(restored, run.add_pc(), "queued");
    let baseline_add_queued =
        super::mixed_compute_fixture::queue_event_at_pc(baseline, run.add_pc(), "queued");
    assert_eq!(
        add_queued.pointer("/service_tick"),
        baseline_add_queued.pointer("/service_tick")
    );
    let multiply_writeback = event_u64(multiply, "writeback_tick");
    assert!([
        event_u64(add_queued, "service_tick"),
        event_u64(
            super::mixed_compute_fixture::queue_event_at_pc(restored, run.add_pc(), "selected"),
            "service_tick"
        ),
        event_u64(add, "issue_tick"),
    ]
    .into_iter()
    .all(|tick| tick >= multiply_writeback));
    assert!(event_u64(load, "commit_tick") <= event_u64(multiply, "commit_tick"));
    assert!(event_u64(multiply, "commit_tick") <= event_u64(add, "commit_tick"));
    assert!(event_u64(add, "commit_tick") <= event_u64(store, "commit_tick"));
    assert!(event_u64(multiply, "issue_tick") < event_u64(add, "issue_tick"));
}

fn assert_live_fp_final_state(restored: &Value, baseline: &Value, run: FpLoadForwardingRun) {
    assert_fp_load_architecture(run.precision, restored);
    for pointer in [
        "/simulation/status",
        "/simulation/instruction_probes",
        "/cores/0/pc",
        "/cores/0/registers",
        "/cores/0/committed_instructions",
        "/memory",
    ] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.pointer(pointer),
            "restored exact FP/integer state at {pointer}",
        );
    }
    assert_eq!(
        restored
            .pointer("/host_actions/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1),
    );
    for pointer in [
        "/cores/0/o3_runtime/instructions",
        "/cores/0/o3_runtime/rob_commits",
        "/cores/0/o3_runtime/issue/issued_rows",
        "/cores/0/o3_runtime/writeback_port/admitted_rows",
        "/cores/0/o3_runtime/lsq/loads",
        "/cores/0/o3_runtime/lsq/load_bytes",
    ] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.pointer(pointer),
            "exactly-once O3 counter {pointer}",
        );
        assert!(
            baseline
                .pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "nonzero baseline counter {pointer}",
        );
    }
    for pointer in [
        "/cores/0/o3_runtime/issue",
        "/cores/0/o3_runtime/writeback_port",
    ] {
        assert_eq!(
            restored.pointer(pointer),
            baseline.pointer(pointer),
            "complete exactly-once O3 stat object {pointer}",
        );
    }
}

fn data_trace(json: &Value) -> &[Value] {
    json.pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("missing FP data trace: {json}"))
}

fn o3_event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    super::mixed_compute::o3_event_at_pc(json, pc)
}
