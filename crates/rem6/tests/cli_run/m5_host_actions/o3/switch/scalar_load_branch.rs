use super::super::lsq_fu_branch::{
    assert_completed_mixed_branch_window, event_at_pc, event_u64, mixed_branch_command,
    mixed_load_alu_branch_binary, run_mixed_branch_json, LOAD_PC,
};
use super::*;

#[test]
fn rem6_run_host_switch_rejects_live_o3_mixed_load_alu_branch() {
    let path = mixed_load_alu_branch_binary("host-switch-o3-mixed-load-alu-branch");
    let baseline = run_mixed_branch_json(&path, "direct", 1_500, "detailed", &[]);
    assert_completed_mixed_branch_window(&baseline);

    let load = event_at_pc(&baseline, LOAD_PC);
    let load_issue = event_u64(load, "issue_tick");
    let load_response = event_u64(load, "lsq_data_response_tick");
    let switch_tick = load_issue + (load_response - load_issue) / 2;
    assert!(load_issue < switch_tick && switch_tick < load_response);

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let artifact = temp_output("o3-mixed-load-alu-branch-live-switch");
    let mut command = mixed_branch_command(&path, "direct", 1_500, "detailed");
    command.args([
        "--host-switch-cpu-mode",
        &switch_arg,
        "--output",
        artifact.to_str().unwrap(),
    ]);
    let output = command.output().unwrap();

    assert_eq!(
        output.status.code(),
        Some(2),
        "mixed load/ALU/branch live switch: {output:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "mixed load/ALU/branch live switch: {output:?}"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n"
    );
    assert!(
        !artifact.exists(),
        "mixed load/ALU/branch live switch emitted {}",
        artifact.display()
    );
}
