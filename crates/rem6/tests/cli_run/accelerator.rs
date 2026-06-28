use std::process::Command;

use crate::support::assert_stat;

#[test]
fn rem6_accelerator_run_executes_npu_and_gpu_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "accelerator-run",
            "--engine",
            "7",
            "--lanes",
            "2",
            "--command-delay",
            "1",
            "--npu-inference",
            "10:4:3",
            "--gpu-kernel",
            "11:2:5",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.accelerator_run.v1\""));
    assert!(stdout.contains("\"engine\":7"));
    assert!(stdout.contains("\"lanes\":2"));
    assert!(stdout.contains("\"command_delay\":1"));
    assert!(stdout.contains("\"command_count\":2"));
    assert!(stdout.contains("\"npu_inference_command_count\":1"));
    assert!(stdout.contains("\"gpu_kernel_command_count\":1"));
    assert!(stdout.contains("\"completion_count\":2"));
    assert!(stdout.contains("\"npu_inference_completion_count\":1"));
    assert!(stdout.contains("\"gpu_kernel_completion_count\":1"));
    assert!(stdout.contains("\"trace_event_count\":6"));
    assert_stat(
        &stdout,
        "sim.accelerator_run.commands",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.accelerator_run.commands.npu_inference",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.accelerator_run.completions.gpu_kernel",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.accelerator_run.trace_events",
        "Count",
        6,
        "monotonic",
    );
}
