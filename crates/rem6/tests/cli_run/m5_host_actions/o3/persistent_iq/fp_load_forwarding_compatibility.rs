use rem6_cpu::{O3PendingStateCheckpointPayload, RiscvCore};
use serde_json::Value;

use super::*;

pub(super) fn assert_pending_state_checkpoint_compatibility() {
    let runtime = RiscvCore::default_o3_runtime_checkpoint_payload();
    let encoded =
        O3PendingStateCheckpointPayload::from_snapshot(runtime.snapshot().pending_state().clone())
            .expect("default O3 pending state must encode")
            .encode();
    assert_eq!(&encoded[..4], b"O3PS");
    assert_eq!(encoded[4], 2, "current O3PS compatibility version");
}

pub(super) fn assert_generic_live_data_handoff_codec_compatibility() {
    // FP-owned live state must reject transfer, so pin the shared O3DH codec on
    // the supported scalar transport handoff path.
    let path = predicted_control_binary(
        "o3-fp-load-forwarding-handoff-compatibility",
        false,
        false,
        false,
    );
    let issue_args = ["--riscv-o3-issue-width", "1"];
    let baseline = run_predicted_control_json(&path, "direct", 1_500, "detailed", &issue_args);
    let load = event_at_pc(&baseline, LOAD_PC);
    let source_tick = event_u64(event_at_pc(&baseline, ADD_PC), "issue_tick") + 1;
    let response_tick = event_u64(load, "lsq_data_response_tick");
    let delivered_tick = source_tick + 1;
    assert!(
        delivered_tick < response_tick,
        "O3DH compatibility action must be delivered before load response: {load}"
    );

    let switch = format!("{source_tick}:cpu0:timing");
    let switched = run_predicted_control_json(
        &path,
        "direct",
        1_500,
        "detailed",
        &[
            "--riscv-o3-issue-width",
            "1",
            "--host-switch-cpu-mode",
            switch.as_str(),
        ],
    );
    assert_eq!(
        switched
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(register_value(&switched, "x12"), 42);
    let action = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|action| {
                action.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && action.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
                    && action.pointer("/mode").and_then(Value::as_str) == Some("timing")
            })
        })
        .expect("O3DH detailed-to-timing switch");
    assert_eq!(
        action.pointer("/tick").and_then(Value::as_u64),
        Some(delivered_tick)
    );
    let transfer = action
        .pointer("/state_transfer")
        .expect("O3DH detailed-to-timing state transfer");
    assert_eq!(
        transfer.pointer("/captured").and_then(Value::as_bool),
        Some(true)
    );
    let handoff = transfer_live_data_handoff_chunk(transfer, "cpu0");
    for (field, expected) in [
        ("schema_version", 7),
        ("resident_rows", 1),
        ("transport_owned_rows", 1),
        ("first_bytes", 4),
    ] {
        assert_eq!(
            handoff
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "O3DH {field}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        handoff.pointer("/first_operation").and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some("0x800000c0")
    );
}
