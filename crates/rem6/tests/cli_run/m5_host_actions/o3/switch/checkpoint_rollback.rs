use super::*;

#[test]
fn rem6_run_omits_execution_mode_authority_after_switch_transfer_rollback() {
    let program = riscv64_program(&[
        m5op(M5_SWITCH_CPU),
        i_type(7, 0, 0x0, 5, 0x13),
        i_type(1, 5, 0x0, 5, 0x13),
        i_type(1, 5, 0x0, 5, 0x13),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("m5-switch-cpu-transfer-rollback-checkpoint", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--host-checkpoint",
            "6:with-authority",
            "--host-restore-checkpoint",
            "8:execution-mode-switch-cpu0-3",
            "--host-checkpoint",
            "10:post-rollback",
            "--debug-flags",
            "HostAction",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"));
    let host_actions = json
        .pointer("/host_actions")
        .expect("run JSON should include host action outcomes");

    assert_eq!(
        host_actions
            .pointer("/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let restore = host_actions
        .pointer("/checkpoint_restores/0")
        .unwrap_or_else(|| panic!("missing switch-transfer rollback: {host_actions}"));
    assert_eq!(
        restore
            .pointer("/execution_mode_authority_present")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        restore
            .pointer("/execution_mode_authority_cleared")
            .and_then(Value::as_bool),
        Some(true)
    );

    assert_eq!(
        host_actions
            .pointer("/checkpoint_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    let checkpoints = host_actions
        .pointer("/checkpoints")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing checkpoints: {host_actions}"));
    let authority_checkpoint = checkpoints
        .iter()
        .find(|checkpoint| {
            checkpoint.pointer("/label").and_then(Value::as_str) == Some("with-authority")
        })
        .unwrap_or_else(|| panic!("missing authority checkpoint: {host_actions}"));
    assert_eq!(
        authority_checkpoint
            .pointer("/execution_mode_authority_present")
            .and_then(Value::as_bool),
        Some(true),
        "the pre-rollback checkpoint must register real authority: {authority_checkpoint}"
    );
    let checkpoint = checkpoints
        .iter()
        .find(|checkpoint| {
            checkpoint.pointer("/label").and_then(Value::as_str) == Some("post-rollback")
        })
        .unwrap_or_else(|| panic!("missing post-rollback checkpoint: {host_actions}"));
    assert_eq!(
        checkpoint.pointer("/label").and_then(Value::as_str),
        Some("post-rollback")
    );
    assert_eq!(
        checkpoint
            .pointer("/execution_mode_authority_present")
            .and_then(Value::as_bool),
        Some(false),
        "a checkpoint after authority rollback must not revive empty authority: {checkpoint}"
    );
    assert_eq!(
        checkpoint
            .pointer("/execution_mode_authority_decode_error")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        checkpoint
            .pointer("/execution_modes")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    assert!(
        checkpoint
            .pointer("/components")
            .and_then(Value::as_array)
            .is_some_and(|components| components.iter().all(|component| {
                component.pointer("/component").and_then(Value::as_str)
                    != Some("host.execution_modes")
            })),
        "a checkpoint after authority rollback must omit host.execution_modes: {checkpoint}"
    );
    for path in [
        "sim.host_actions.checkpoint.component.host_execution_modes.components",
        "sim.host_actions.checkpoint.component.host_execution_modes.chunk.modes.chunks",
        "sim.debug.host_action_trace.checkpoint.component.host_execution_modes.components",
        "sim.debug.host_action_trace.checkpoint.component.host_execution_modes.chunk.modes.chunks",
    ] {
        assert_json_stat(&json, path, "Count", 1, "monotonic");
    }
    for path in [
        "sim.host_actions.checkpoint.latest_component.host_execution_modes.components",
        "sim.debug.host_action_trace.checkpoint.latest_component.host_execution_modes.components",
        "sim.host_actions.checkpoint_restore.component.host_execution_modes.components",
        "sim.host_actions.checkpoint_restore.component.host_execution_modes.chunk.modes.chunks",
        "sim.debug.host_action_trace.checkpoint_restore.component.host_execution_modes.components",
        "sim.debug.host_action_trace.checkpoint_restore.component.host_execution_modes.chunk.modes.chunks",
    ] {
        assert_json_stat_absent(&json, path);
    }
    assert_eq!(
        host_actions
            .pointer("/execution_modes")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0),
        "final execution-mode authority should remain cleared: {host_actions}"
    );
}
