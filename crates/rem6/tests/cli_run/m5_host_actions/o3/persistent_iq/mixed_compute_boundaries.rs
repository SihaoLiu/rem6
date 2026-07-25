use serde_json::Value;

use super::mixed_compute_fixture::*;
use super::*;

const DEPENDENT_FP_PC: &str = "0x80000048";
const VECTOR_DESTINATION_PC: &str = "0x80000044";
const DUMP_STATS_PC: &str = "0x8000006c";
const EXIT_PC: &str = "0x80000070";

#[test]
fn rem6_run_o3_persistent_iq_dependent_fp_boundary() {
    let json = run_dependent_fp_mixed_compute_json();
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("00001041"),
        "dependent fmul.s result bytes: {json}",
    );
    let producer_queued = queue_event_at_pc(&json, FP_ADD_PC, "queued");
    let producer_selected = queue_event_at_pc(&json, FP_ADD_PC, "selected");
    assert_eq!(
        producer_queued.pointer("/sequence"),
        producer_selected.pointer("/sequence"),
    );
    assert!(super::queue_events(&json)
        .iter()
        .all(|event| { event.pointer("/pc").and_then(Value::as_str) != Some(DEPENDENT_FP_PC) }));
    let dependent = super::mixed_compute::o3_event_at_pc(&json, DEPENDENT_FP_PC);
    assert_eq!(
        dependent
            .pointer("/fu_latency_class")
            .and_then(Value::as_str),
        Some("scalar_float_mul"),
    );
    assert!(
        event_u64(dependent, "commit_tick")
            > event_u64(event_at_pc(&json, FP_ADD_PC), "commit_tick"),
        "dependent fmul.s must retire after its queued producer: {json}",
    );
}

#[test]
fn rem6_run_o3_persistent_iq_vector_destination_boundary() {
    let json = run_vector_destination_mixed_compute_json();
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("1500000015000000"),
        "vector vmul.vv result bytes: {json}",
    );
    assert!(super::queue_events(&json).iter().all(|event| {
        event.pointer("/pc").and_then(Value::as_str) != Some(VECTOR_DESTINATION_PC)
    }));
    let vector = super::mixed_compute::o3_event_at_pc(&json, VECTOR_DESTINATION_PC);
    assert_eq!(
        vector.pointer("/fu_latency_class").and_then(Value::as_str),
        Some("vector_integer_mul"),
    );
}

#[test]
fn rem6_run_o3_persistent_iq_mixed_compute_checkpoint_boundary() {
    let path = mixed_compute_binary("o3-persistent-iq-mixed-compute-checkpoint");
    let baseline = run_mixed_compute_path_json(&path, 2, "direct", "detailed", 12, &[]);

    assert_mixed_compute_system_boundary(&baseline);
    assert_live_mixed_compute_checkpoint_rejected(&path, &baseline);
    assert_drained_mixed_compute_restore(&path, &baseline);
}

#[test]
fn rem6_run_timing_suppresses_o3_mixed_compute_surface() {
    let path = mixed_compute_binary("o3-persistent-iq-mixed-compute-timing");
    let timing = run_mixed_compute_path_json(&path, 2, "direct", "timing", 12, &[]);

    assert_exact_architectural_results(&timing);
    assert_timing_mixed_compute_surface_is_absent(&timing);
}

fn assert_timing_mixed_compute_surface_is_absent(json: &Value) {
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json.pointer("/debug/o3_trace/0/issue_queue").is_none());
    let leaked = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing mixed-compute stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| path.starts_with("sim.cpu0.o3.issue_queue."))
        .collect::<Vec<_>>();
    assert!(
        leaked.is_empty(),
        "timing mode leaked queue stats: {leaked:?}"
    );
}

fn assert_mixed_compute_system_boundary(json: &Value) {
    for pc in [DUMP_STATS_PC, EXIT_PC] {
        assert_eq!(
            event_at_pc(json, pc)
                .pointer("/system_event")
                .and_then(Value::as_bool),
            Some(true),
            "expected a system instruction at {pc}",
        );
        assert!(
            super::queue_events(json)
                .iter()
                .all(|event| event.pointer("/pc").and_then(Value::as_str) != Some(pc)),
            "system row {pc} entered the issue queue",
        );
    }
    assert!(json
        .pointer("/cores/0/o3_runtime/issue/queue/issued_by_class/system")
        .is_none());
}

fn assert_live_mixed_compute_checkpoint_rejected(path: &std::path::Path, baseline: &Value) {
    let events = super::queue_events(baseline);
    let live_tick = events
        .iter()
        .find(|event| event.pointer("/action").and_then(Value::as_str) == Some("retained_resource"))
        .and_then(|event| event.pointer("/service_tick").and_then(Value::as_u64))
        .expect("mixed-compute live queue tick");
    assert!(events.iter().any(|queued| {
        queued.pointer("/action").and_then(Value::as_str) == Some("queued")
            && queued
                .pointer("/service_tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick <= live_tick)
            && !events.iter().any(|selected| {
                selected.pointer("/sequence") == queued.pointer("/sequence")
                    && selected.pointer("/action").and_then(Value::as_str) == Some("selected")
                    && selected
                        .pointer("/service_tick")
                        .and_then(Value::as_u64)
                        .is_some_and(|tick| tick <= live_tick)
            })
    }));

    let checkpoint = format!("{live_tick}:mixed-compute-live");
    let artifact = temp_output("o3-persistent-iq-mixed-compute-live.json");
    let mut command = mixed_compute_command(path, 2, "direct", "detailed", 12);
    command.args([
        "--host-checkpoint",
        checkpoint.as_str(),
        "--output",
        artifact.to_str().unwrap(),
    ]);
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(2), "live checkpoint: {output:?}");
    assert!(output.stdout.is_empty(), "live checkpoint: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
    );
    assert!(
        !artifact.exists(),
        "live checkpoint emitted {}",
        artifact.display()
    );
}

fn assert_drained_mixed_compute_restore(path: &std::path::Path, baseline: &Value) {
    let checkpoint_tick = [DIV_PC, FP_ADD_PC, VECTOR_RESULT_PC, SECOND_FP_PC]
        .into_iter()
        .map(|pc| event_u64(event_at_pc(baseline, pc), "commit_tick"))
        .max()
        .unwrap()
        + 1;
    let checkpoint = format!("{checkpoint_tick}:mixed-compute-drained");
    let restore = format!("{}:mixed-compute-drained", checkpoint_tick + 1);
    let restored = run_mixed_compute_path_json(
        path,
        2,
        "direct",
        "detailed",
        12,
        &[
            "--host-checkpoint",
            checkpoint.as_str(),
            "--host-restore-checkpoint",
            restore.as_str(),
        ],
    );
    assert_exact_architectural_results(&restored);
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1),
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1),
    );
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("drained mixed-compute checkpoint");
    let runtime = checkpoint_component_chunks(checkpoint_component(checkpoint, "cpu0"))
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .expect("decoded drained mixed-compute O3 runtime");
    for (field, expected) in [
        ("checkpoint_version", 23),
        ("snapshot_rob_entries", 0),
        ("snapshot_lsq_entries", 0),
    ] {
        assert_eq!(
            runtime
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "drained checkpoint field {field}: {runtime}",
        );
    }
    let queue = restored
        .pointer("/cores/0/o3_runtime/issue/queue")
        .expect("restored mixed-compute queue");
    for class in [
        "scalar_integer",
        "integer_mul_div",
        "memory_agu",
        "control",
        "scalar_float",
        "vector_to_scalar",
    ] {
        assert_eq!(
            queue
                .pointer(&format!("/issued_by_class/{class}"))
                .and_then(Value::as_u64),
            Some(0),
            "restored class counter {class}: {queue}",
        );
    }
    assert_eq!(
        queue.pointer("/current_occupancy").and_then(Value::as_u64),
        Some(0),
    );
}

fn run_dependent_fp_mixed_compute_json() -> Value {
    let mut words = mixed_compute_prefix();
    append_mixed_compute_head(&mut words);
    words.extend([
        fp_add_s(4, 1, 2),
        fp_mul_s(5, 4, 3),
        fp_r_type(0x70, 0, 5, 0, 13),
        s_type(0, 13, 12, 0b010),
        i_type(0, 0, 0, 10, 0x13),
        i_type(0, 0, 0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    let path = append_mixed_compute_data("o3-persistent-iq-dependent-fp", words);
    run_mixed_compute_path_json(&path, 2, "direct", "detailed", 4, &[])
}

fn run_vector_destination_mixed_compute_json() -> Value {
    let mut words = vec![
        i_type(3, 0, 0, 6, 0x13),
        i_type(7, 0, 0, 7, 0x13),
        i_type(2, 0, 0, 10, 0x13),
        vsetvli_type(0xd0, 10, 5),
        vector_arith_type(0b010111, 0b100, 0, 6, 1),
        vector_arith_type(0b010111, 0b100, 0, 7, 2),
        i_type(84, 0, 0, 1, 0x13),
        i_type(7, 0, 0, 2, 0x13),
        i_type(0, 0, 0, 0, 0x13),
        i_type(0, 0, 0, 0, 0x13),
        i_type(0, 0, 0, 0, 0x13),
        i_type(0, 0, 0, 0, 0x13),
    ];
    append_mixed_compute_head(&mut words);
    words.extend([
        vector_arith_type(0b100101, 0b010, 2, 1, 4),
        vector_unit_stride_store_type(true, 0b110, 12, 4),
        i_type(0, 0, 0, 10, 0x13),
        i_type(0, 0, 0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    let path = append_mixed_compute_data("o3-persistent-iq-vector-destination", words);
    run_mixed_compute_path_json(&path, 2, "direct", "detailed", 8, &[])
}
