use std::collections::BTreeSet;

use super::lsq_fu_branch::{event_at_pc, event_u64};
use super::predicted_control::{
    checkpoint_component, checkpoint_component_chunks, predicted_control_binary,
    predicted_control_command, register_value, run_predicted_control_json,
    transfer_live_data_handoff_chunk, transfer_o3_runtime_chunk, ADD_PC, BRANCH_PC, LOAD_PC,
    MUL_PC,
};
use super::*;

const PERSISTENT_IQ_QUEUE_STATS: [(&str, &str); 11] = [
    ("enqueued_rows", "enqueued_rows"),
    ("service_turns", "service_turns"),
    ("wake_requests", "wake_requests"),
    ("current_occupancy", "current_occupancy"),
    ("peak_occupancy", "peak_occupancy"),
    (
        "issued_by_class/scalar_integer",
        "issued_by_class.scalar_integer",
    ),
    (
        "issued_by_class/integer_mul_div",
        "issued_by_class.integer_mul_div",
    ),
    ("issued_by_class/memory_agu", "issued_by_class.memory_agu"),
    ("issued_by_class/control", "issued_by_class.control"),
    (
        "issued_by_class/scalar_float",
        "issued_by_class.scalar_float",
    ),
    (
        "issued_by_class/vector_to_scalar",
        "issued_by_class.vector_to_scalar",
    ),
];
const WIDTH_FOUR_CLASS_HEAD_PC: &str = "0x80000040";

#[test]
fn rem6_run_o3_persistent_iq_width_one_oldest_ready_cross_class_direct() {
    assert_persistent_iq_oldest_ready(1);
}

#[test]
fn rem6_run_o3_persistent_iq_width_two_coissues_ready_cross_class_direct() {
    assert_persistent_iq_oldest_ready(2);
}

fn assert_persistent_iq_oldest_ready(issue_width: usize) {
    let json = super::scoped_issue::persistent_iq_oldest_ready_fixture(issue_width);
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .unwrap_or_else(|| panic!("missing persistent-IQ queue summary: {json}"));

    assert!(
        queue
            .pointer("/enqueued_rows")
            .and_then(Value::as_u64)
            .unwrap()
            >= 4
    );
    assert!(
        queue
            .pointer("/service_turns")
            .and_then(Value::as_u64)
            .unwrap()
            >= 2
    );
    assert_eq!(
        queue.pointer("/current_occupancy").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        queue
            .pointer("/issued_by_class/integer_mul_div")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        queue
            .pointer("/issued_by_class/scalar_integer")
            .and_then(Value::as_u64),
        Some(2)
    );
}

#[test]
fn rem6_run_o3_persistent_iq_width_four_respects_class_caps_hierarchy() {
    let json = super::writeback_port::dependent_result_address::two_pending::
        persistent_iq_width_four_hierarchy_json();
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .expect("persistent-IQ hierarchy queue summary");
    let configured_key = ["configured", "width"].join("_");
    assert_eq!(
        json.pointer(&format!("/cores/0/o3_runtime/issue/{configured_key}"))
            .and_then(Value::as_u64),
        Some(4),
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/issue/max_rows_per_cycle")
            .and_then(Value::as_u64),
        Some(4),
    );
    let selected = queue_events(&json)
        .iter()
        .filter(|event| event.pointer("/action").and_then(Value::as_str) == Some("selected"))
        .collect::<Vec<_>>();
    let selected_ticks = selected
        .iter()
        .map(|event| event_u64(event, "service_tick"))
        .collect::<BTreeSet<_>>();
    let width_four_queue_batch = selected_ticks
        .iter()
        .map(|tick| {
            selected
                .iter()
                .copied()
                .filter(|event| event_u64(event, "service_tick") == *tick)
                .collect::<Vec<_>>()
        })
        .find(|batch| {
            batch.len() == 3
                && batch
                    .iter()
                    .map(|event| {
                        event
                            .pointer("/issue_class")
                            .and_then(Value::as_str)
                            .unwrap()
                    })
                    .collect::<BTreeSet<_>>()
                    == BTreeSet::from(["control", "integer_mul_div", "scalar_integer"])
        })
        .unwrap_or_else(|| panic!("missing three-class queue batch: {selected:?}"));
    let service_tick = event_u64(width_four_queue_batch[0], "service_tick");
    let class_head = event_at_pc(&json, WIDTH_FOUR_CLASS_HEAD_PC);
    assert_eq!(
        event_u64(class_head, "issue_tick"),
        service_tick,
        "memory head must consume the fourth issue slot beside the three queued classes: {class_head}",
    );
    assert!(class_head.pointer("/lsq_load_address").is_some());
    assert_eq!(
        width_four_queue_batch
            .iter()
            .map(|event| event
                .pointer("/issue_class")
                .and_then(Value::as_str)
                .unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["control", "integer_mul_div", "scalar_integer"]),
    );
    assert!(
        queue
            .pointer("/issued_by_class/scalar_integer")
            .and_then(Value::as_u64)
            .unwrap()
            > 0
    );
    assert!(
        queue
            .pointer("/issued_by_class/integer_mul_div")
            .and_then(Value::as_u64)
            .unwrap()
            > 0
    );
    assert!(
        queue
            .pointer("/issued_by_class/memory_agu")
            .and_then(Value::as_u64)
            .unwrap()
            > 0
    );
    assert!(
        queue
            .pointer("/issued_by_class/control")
            .and_then(Value::as_u64)
            .unwrap()
            > 0
    );
}

#[test]
fn rem6_run_o3_persistent_iq_cross_class_wakeup_matrix_direct() {
    let producer_and_resource = super::scoped_issue::persistent_iq_oldest_ready_fixture(1);
    let replanned = super::writeback_port::persistent_iq_writeback_replan_json();
    let pending = super::writeback_port::dependent_result_address::two_pending::
        persistent_iq_width_four_hierarchy_json();

    assert_queue_wakeup_transition(
        &producer_and_resource,
        "retained_dependency",
        "scalar_integer",
    );
    assert_queue_wakeup_transition(
        &producer_and_resource,
        "retained_resource",
        "integer_mul_div",
    );
    assert_queue_wakeup_transition(&replanned, "retained_dependency", "scalar_integer");
    assert_queue_wakeup_transition(&pending, "retained_resource", "memory_agu");
}

fn assert_queue_wakeup_transition(json: &Value, retained_action: &str, issue_class: &str) {
    let events = queue_events(json);
    let observed = events
        .iter()
        .map(|event| {
            (
                event.pointer("/action").and_then(Value::as_str),
                event.pointer("/issue_class").and_then(Value::as_str),
                event.pointer("/sequence").and_then(Value::as_u64),
                event.pointer("/service_tick").and_then(Value::as_u64),
                event.pointer("/next_wake_tick").and_then(Value::as_u64),
            )
        })
        .collect::<Vec<_>>();
    let retained_events = events
        .iter()
        .enumerate()
        .filter(|(_, retained)| {
            retained.pointer("/action").and_then(Value::as_str) == Some(retained_action)
                && retained.pointer("/issue_class").and_then(Value::as_str) == Some(issue_class)
        })
        .collect::<Vec<_>>();
    assert!(
        !retained_events.is_empty(),
        "missing {retained_action}/{issue_class} transition: {observed:?}"
    );
    for (index, retained) in retained_events {
        let sequence = event_u64(retained, "sequence");
        let retained_tick = event_u64(retained, "service_tick");
        let advertised_wake = retained.pointer("/next_wake_tick").and_then(Value::as_u64);
        let eligible_tick = advertised_wake.unwrap_or_else(|| retained_tick.saturating_add(1));
        let lifecycle = events
            .iter()
            .filter(|event| event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence))
            .collect::<Vec<_>>();
        let next = events[index + 1..]
            .iter()
            .find(|event| {
                event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence)
                    && event
                        .pointer("/service_tick")
                        .and_then(Value::as_u64)
                        .is_some_and(|tick| tick >= eligible_tick)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{retained_action}/{issue_class} sequence {sequence} is never reconsidered at or after tick {eligible_tick}: lifecycle={lifecycle:?}"
                )
            });
        let next_tick = event_u64(next, "service_tick");
        if let Some(wake) = advertised_wake {
            assert_eq!(
                next_tick, wake,
                "{retained_action}/{issue_class} sequence {sequence} must be reconsidered at its advertised wake: lifecycle={lifecycle:?}",
            );
        } else {
            assert!(
                next_tick > retained_tick,
                "external {retained_action}/{issue_class} sequence {sequence} must be reconsidered on a later wake: lifecycle={lifecycle:?}",
            );
        }
        assert!(
            events[index + 1..].iter().any(|event| {
                event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence)
                    && event.pointer("/action").and_then(Value::as_str) == Some("selected")
                    && event
                        .pointer("/service_tick")
                        .and_then(Value::as_u64)
                        .is_some_and(|tick| tick >= eligible_tick)
            }),
            "{retained_action}/{issue_class} sequence {sequence} never reaches selection: lifecycle={lifecycle:?}"
        );
    }
}

fn queue_events(json: &Value) -> &[Value] {
    json.pointer("/debug/o3_trace/0/issue_queue/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("missing persistent-IQ events: {json}"))
}

#[test]
fn rem6_run_o3_persistent_iq_squash_discards_wrong_path_queue_suffix() {
    let (wrong_path, boundary) =
        super::writeback_port::fixed_fu::persistent_iq_wrong_path_cleanup_json();
    let replay = super::writeback_port::dependent_result_address::two_pending::boundaries::
        persistent_iq_first_replay_json();

    let squashed = queue_events(&wrong_path)
        .iter()
        .filter(|event| event.pointer("/action").and_then(Value::as_str) == Some("squashed"))
        .collect::<Vec<_>>();
    assert!(
        squashed.len() >= 2,
        "wrong-path queue cleanup: {:?}",
        queue_events(&wrong_path),
    );
    assert!(squashed.iter().all(|event| {
        event.pointer("/cleanup_boundary").and_then(Value::as_u64) == Some(boundary)
    }));
    assert!(squashed.iter().any(|event| {
        event.pointer("/issue_class").and_then(Value::as_str) == Some("integer_mul_div")
    }));
    assert!(squashed.iter().any(|event| {
        matches!(
            event.pointer("/issue_class").and_then(Value::as_str),
            Some("control") | Some("scalar_integer")
        )
    }));
    let removed = squashed
        .iter()
        .filter_map(|event| event.pointer("/sequence").and_then(Value::as_u64))
        .collect::<BTreeSet<_>>();
    assert!(!removed.is_empty());
    let first_squash = queue_events(&wrong_path)
        .iter()
        .position(|event| event.pointer("/action").and_then(Value::as_str) == Some("squashed"))
        .expect("wrong-path squash position");
    assert!(queue_events(&wrong_path)
        .iter()
        .skip(first_squash + 1)
        .all(|event| {
            event.pointer("/action").and_then(Value::as_str) != Some("selected")
                || event
                    .pointer("/sequence")
                    .and_then(Value::as_u64)
                    .is_none_or(|sequence| !removed.contains(&sequence))
        }));
    assert_eq!(
        wrong_path
            .pointer("/cores/0/o3_runtime/issue/queue/current_occupancy")
            .and_then(Value::as_u64),
        Some(0),
    );

    let replayed = queue_events(&replay)
        .iter()
        .find(|event| {
            event.pointer("/action").and_then(Value::as_str) == Some("replayed")
                && event.pointer("/sequence") == event.pointer("/cleanup_boundary")
        })
        .unwrap_or_else(|| panic!("missing exact pending-address replay: {replay}"));
    let replay_boundary = replayed
        .pointer("/cleanup_boundary")
        .and_then(Value::as_u64)
        .expect("replay cleanup boundary");
    assert_eq!(
        replayed.pointer("/sequence").and_then(Value::as_u64),
        Some(replay_boundary),
    );
    let replayed_sequences = queue_events(&replay)
        .iter()
        .filter(|event| event.pointer("/action").and_then(Value::as_str) == Some("replayed"))
        .filter_map(|event| event.pointer("/sequence").and_then(Value::as_u64))
        .collect::<BTreeSet<_>>();
    assert!(queue_events(&replay).iter().all(|event| {
        event.pointer("/action").and_then(Value::as_str) != Some("squashed")
            || event
                .pointer("/sequence")
                .and_then(Value::as_u64)
                .is_none_or(|sequence| !replayed_sequences.contains(&sequence))
    }));
    let first_replay = queue_events(&replay)
        .iter()
        .position(|event| event.pointer("/action").and_then(Value::as_str) == Some("replayed"))
        .expect("pending-address replay position");
    assert!(queue_events(&replay)
        .iter()
        .skip(first_replay + 1)
        .all(|event| {
            event.pointer("/action").and_then(Value::as_str) != Some("selected")
                || event
                    .pointer("/sequence")
                    .and_then(Value::as_u64)
                    .is_none_or(|sequence| !replayed_sequences.contains(&sequence))
        }));
    assert_eq!(
        replay
            .pointer("/cores/0/o3_runtime/issue/queue/current_occupancy")
            .and_then(Value::as_u64),
        Some(0),
    );
}

#[test]
fn rem6_run_o3_persistent_iq_text_stats_expose_queue_counters() {
    let (json, stdout) = super::scoped_issue::persistent_iq_text_stats_fixture();
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .unwrap_or_else(|| panic!("missing persistent-IQ queue summary: {json}"));

    for (json_field, stat_field) in PERSISTENT_IQ_QUEUE_STATS {
        let value = queue
            .pointer(&format!("/{json_field}"))
            .and_then(Value::as_u64)
            .unwrap_or_else(|| panic!("missing queue field {json_field}: {queue}"));
        let path = format!("sim.cpu0.o3.issue_queue.{stat_field}");
        assert_text_resettable_count_stat(&stdout, &path, value);
        assert_text_stat_occurs_once(&stdout, &path);
    }
}

#[test]
fn rem6_run_o3_persistent_iq_stats_dump_exposes_queue_counters() {
    let json = super::scoped_issue::persistent_iq_stats_dump_fixture();
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .unwrap_or_else(|| panic!("missing persistent-IQ queue summary: {json}"));
    let dump = json
        .pointer("/host_actions/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing persistent-IQ stats dump: {json}"));

    for (json_field, stat_field) in PERSISTENT_IQ_QUEUE_STATS {
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
}

#[test]
fn rem6_run_o3_persistent_iq_debug_exposes_residency_and_cleanup() {
    let (json, _) = super::writeback_port::fixed_fu::persistent_iq_wrong_path_cleanup_json();
    let actions = queue_events(&json)
        .iter()
        .filter_map(|event| event.pointer("/action").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    for required in [
        "queued",
        "selected",
        "retained_resource",
        "retained_dependency",
    ] {
        assert!(actions.contains(required), "missing {required}: {json}");
    }
    assert!(
        actions.contains("squashed"),
        "missing cleanup action: {json}"
    );
    for event in queue_events(&json) {
        assert!(event.pointer("/sequence").and_then(Value::as_u64).is_some());
        assert!(event.pointer("/pc").and_then(Value::as_str).is_some());
        assert!(event
            .pointer("/service_tick")
            .and_then(Value::as_u64)
            .is_some());
        assert!(event
            .pointer("/issue_class")
            .and_then(Value::as_str)
            .is_some());
    }
}

#[test]
fn rem6_run_host_switch_preserves_o3_persistent_iq_ticks() {
    let path = predicted_control_binary("o3-persistent-iq-switch", false, false, false);
    let issue_args = ["--riscv-o3-issue-width", "1"];
    let baseline = run_predicted_control_json(&path, "direct", 1_500, "detailed", &issue_args);
    let load = event_at_pc(&baseline, LOAD_PC);
    let branch = event_at_pc(&baseline, BRANCH_PC);
    let multiply = event_at_pc(&baseline, MUL_PC);
    let add = event_at_pc(&baseline, ADD_PC);
    assert_eq!(
        baseline
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host"),
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x2a"),
        ("x14", "0x2d"),
        ("x15", "0x1"),
        ("x16", "0x2"),
    ] {
        assert_eq!(
            baseline
                .pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "unexpected final {register}: {baseline}",
        );
    }
    assert_eq!(
        event_u64(add, "issue_tick"),
        event_u64(multiply, "writeback_tick")
    );
    assert_eq!(
        branch
            .pointer("/branch_predicted_taken")
            .and_then(Value::as_bool),
        Some(false),
    );
    assert_eq!(
        branch
            .pointer("/branch_resolved_taken")
            .and_then(Value::as_bool),
        Some(false),
    );
    assert_eq!(
        branch
            .pointer("/branch_mispredicted")
            .and_then(Value::as_bool),
        Some(false),
    );
    assert!([load, branch, multiply, add]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
    let live_iq_tick = event_u64(load, "issue_tick") + 1;
    let queued_before_switch = queue_events(&baseline).iter().any(|event| {
        event.pointer("/action").and_then(Value::as_str) == Some("queued")
            && event
                .pointer("/service_tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick <= live_iq_tick)
            && !queue_events(&baseline).iter().any(|later| {
                later.pointer("/sequence") == event.pointer("/sequence")
                    && later.pointer("/action").and_then(Value::as_str) == Some("selected")
                    && later
                        .pointer("/service_tick")
                        .and_then(Value::as_u64)
                        .is_some_and(|tick| tick <= live_iq_tick)
            })
    });
    assert!(
        queued_before_switch,
        "fixture must have a resident IQ row at {live_iq_tick}: {baseline}",
    );

    let artifact = temp_output("o3-persistent-iq-live-switch.json");
    let switch_arg = format!("{live_iq_tick}:cpu0:timing");
    let mut command = predicted_control_command(&path, "direct", 1_500, "detailed");
    command.args(issue_args);
    command.args([
        "--host-switch-cpu-mode",
        switch_arg.as_str(),
        "--output",
        artifact.to_str().unwrap(),
    ]);
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(2), "live IQ switch: {output:?}");
    assert!(output.stdout.is_empty(), "live IQ switch: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
    );
    assert!(
        !artifact.exists(),
        "live IQ switch emitted {}",
        artifact.display()
    );

    let switch_tick = event_u64(event_at_pc(&baseline, ADD_PC), "issue_tick") + 1;
    assert!(switch_tick < event_u64(load, "lsq_data_response_tick"));
    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let switched = run_predicted_control_json(
        &path,
        "direct",
        1_500,
        "detailed",
        &[
            "--riscv-o3-issue-width",
            "1",
            "--host-switch-cpu-mode",
            &switch_arg,
        ],
    );
    let timing_switch = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing persistent-IQ timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("persistent-IQ state transfer");
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false),
    );
    let runtime = transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(4),
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(1),
    );
    let handoff = transfer_live_data_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(7),
    );
    assert_eq!(
        handoff.pointer("/resident_rows").and_then(Value::as_u64),
        Some(1),
    );
    assert_eq!(
        handoff.pointer("/younger_rows").and_then(Value::as_u64),
        Some(3),
    );
    for pc in [LOAD_PC, BRANCH_PC, MUL_PC, ADD_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "mode transfer must preserve {field} for {pc}: expected={expected} actual={actual}",
            );
        }
    }
}

#[test]
fn rem6_run_o3_persistent_iq_checkpoint_boundary() {
    let path = predicted_control_binary("o3-persistent-iq-checkpoint", false, false, false);
    let baseline = run_predicted_control_json(&path, "direct", 1_500, "detailed", &[]);
    let live_iq_tick = event_u64(event_at_pc(&baseline, LOAD_PC), "issue_tick") + 1;
    let queued_before_checkpoint = queue_events(&baseline).iter().any(|event| {
        event.pointer("/action").and_then(Value::as_str) == Some("queued")
            && event
                .pointer("/service_tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick <= live_iq_tick)
            && !queue_events(&baseline).iter().any(|later| {
                later.pointer("/sequence") == event.pointer("/sequence")
                    && later.pointer("/action").and_then(Value::as_str) == Some("selected")
                    && later
                        .pointer("/service_tick")
                        .and_then(Value::as_u64)
                        .is_some_and(|tick| tick <= live_iq_tick)
            })
    });
    assert!(
        queued_before_checkpoint,
        "fixture must have a resident IQ row at {live_iq_tick}: {baseline}",
    );
    let checkpoint_arg = format!("{live_iq_tick}:persistent-iq-live");
    let artifact = temp_output("o3-persistent-iq-live-checkpoint.json");
    let mut command = predicted_control_command(&path, "direct", 1_500, "detailed");
    command.args([
        "--host-checkpoint",
        checkpoint_arg.as_str(),
        "--output",
        artifact.to_str().unwrap(),
    ]);
    let output = command.output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(2),
        "live IQ checkpoint: {output:?}",
    );
    assert!(output.stdout.is_empty(), "live IQ checkpoint: {output:?}");
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
    );
    assert!(
        !artifact.exists(),
        "live IQ checkpoint emitted {}",
        artifact.display(),
    );

    let checkpoint_tick = event_u64(event_at_pc(&baseline, ADD_PC), "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:persistent-iq-drained");
    let restore_arg = format!("{restore_tick}:persistent-iq-drained");
    let restored = run_predicted_control_json(
        &path,
        "direct",
        1_500,
        "detailed",
        &[
            "--host-checkpoint",
            &checkpoint_arg,
            "--host-restore-checkpoint",
            &restore_arg,
        ],
    );
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
        .expect("drained persistent-IQ checkpoint");
    let cpu0 = checkpoint_component(checkpoint, "cpu0");
    let chunks = checkpoint_component_chunks(cpu0);
    assert!(chunks.iter().all(|chunk| {
        chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
    }));
    let runtime = chunks
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .expect("decoded drained O3 runtime checkpoint");
    assert_eq!(
        runtime
            .pointer("/checkpoint_version")
            .and_then(Value::as_u64),
        Some(23),
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(0),
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(0),
    );
    let queue = restored
        .pointer("/cores/0/o3_runtime/issue/queue")
        .expect("post-restore transient queue telemetry");
    for (field, expected) in [
        ("enqueued_rows", 0),
        ("service_turns", 0),
        ("wake_requests", 0),
        ("current_occupancy", 0),
        ("peak_occupancy", 0),
        ("issued_by_class/scalar_integer", 0),
        ("issued_by_class/integer_mul_div", 0),
        ("issued_by_class/memory_agu", 0),
        ("issued_by_class/control", 0),
        ("issued_by_class/scalar_float", 0),
        ("issued_by_class/vector_to_scalar", 0),
    ] {
        assert_eq!(
            queue.pointer(&format!("/{field}")).and_then(Value::as_u64),
            Some(expected),
            "post-restore queue field {field}: {queue}",
        );
    }
    assert_eq!(register_value(&restored, "x14"), 0x2d,);
}

#[test]
fn rem6_run_timing_suppresses_o3_persistent_iq_surface() {
    let path = predicted_control_binary("o3-persistent-iq-timing", false, false, false);
    let timing = run_predicted_control_json(&path, "direct", 1_500, "timing", &[]);
    assert_eq!(register_value(&timing, "x12"), 0x2a,);
    assert_eq!(register_value(&timing, "x13"), 0x2a,);
    assert_eq!(register_value(&timing, "x14"), 0x2d,);
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert!(timing.pointer("/cores/0/o3_runtime/issue/queue").is_none());
    assert!(timing.pointer("/debug/o3_trace/0/issue_queue").is_none());
    let unexpected = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing persistent-IQ stats")
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
    assert!(
        unexpected.is_empty(),
        "timing mode leaked O3 stats: {unexpected:?}",
    );
    let leaked_queue_stats = timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| path.starts_with("sim.cpu0.o3.issue_queue."))
        .collect::<Vec<_>>();
    assert!(
        leaked_queue_stats.is_empty(),
        "timing mode leaked persistent-IQ stats: {leaked_queue_stats:?}",
    );
}
