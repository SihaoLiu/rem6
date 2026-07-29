use super::*;
#[path = "pending_address_graph.rs"]
mod pending_address_graph;
#[path = "pending_address/reachability.rs"]
mod reachability;
const POLICY: &str = "tests/source_policy/live_o3_checkpoint/pending_address.rs";
const REACHABILITY: &str = "tests/source_policy/live_o3_checkpoint/pending_address/reachability.rs";
const CAPTURE: &str = "src/riscv_checkpoint/capture.rs";

#[test]
fn pending_address_live_o3_system_sources_are_attached_and_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parent = read(crate_dir, "tests/source_policy/live_o3_checkpoint.rs");
    assert_eq!(
        parsed_unconditional_external_module_count(
            &parent,
            "pending_address",
            "live_o3_checkpoint/pending_address.rs",
        ),
        1
    );
    let attachment = "#[path = \"live_o3_checkpoint/pending_address.rs\"]\nmod pending_address;";
    for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
        let mutated = parent.replacen(attachment, &format!("{conditional}{attachment}"), 1);
        assert_ne!(mutated, parent);
        assert_eq!(
            parsed_unconditional_external_module_count(
                &mutated,
                "pending_address",
                "live_o3_checkpoint/pending_address.rs",
            ),
            0
        );
    }
    for (relative, maximum) in [
        (POLICY, 260),
        (REACHABILITY, 60),
        ("src/riscv_checkpoint.rs", 1_800),
        ("src/riscv_checkpoint/live_scheduler.rs", 100),
        ("src/riscv_checkpoint/live_wake.rs", 100),
        ("src/riscv_checkpoint/restore_authority.rs", 180),
        ("tests/support/live_o3_pending_address.rs", 350),
    ] {
        let path = crate_dir.join(relative);
        let lines = line_count(&path);
        assert!(
            lines <= maximum,
            "{relative} has {lines} lines, exceeding its {maximum}-line cap"
        );
    }
}

#[test]
fn pending_address_stable_checkpoint_and_mode_switch_rejections_are_locked() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let riscv = read(crate_dir, "src/riscv_checkpoint.rs");
    let capture = read(crate_dir, CAPTURE);
    assert!(stable_boundary_contract(&riscv, &capture));
    let stable = riscv.replacen(
        "RiscvO3LiveCheckpointCapture::Captured(live) if tick.is_some() => Some(live.clone()),",
        "RiscvO3LiveCheckpointCapture::Captured(live) => Some(live.clone()),",
        1,
    );
    assert_ne!(stable, riscv, "stable-checkpoint mutation must apply");
    assert!(!stable_boundary_contract(&stable, &capture));
    let mode_switch = capture.replacen(
        "record.o3_live_checkpoint().is_none()",
        "record.o3_live_checkpoint().is_some()",
        1,
    );
    assert_ne!(mode_switch, capture, "mode-switch mutation must apply");
    assert!(!stable_boundary_contract(&riscv, &mode_switch));
}

#[test]
fn pending_address_instruction_probe_finalization_uses_exact_manifest_indices() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = read(crate_dir, "src/host.rs");
    let action = read(crate_dir, "src/host/action_apply.rs");
    let action_tests = read(crate_dir, "src/host/action_apply/tests.rs");
    let stats = read(crate_dir, "src/riscv_run_stats.rs");
    let errors = read(crate_dir, "src/system_error.rs");
    assert!(probe_finalization_contract(
        &host,
        &action,
        &action_tests,
        &stats,
        &errors,
    ));

    let labels = host.replacen(
        "pending_riscv_instruction_probe_checkpoint_indices: BTreeSet<usize>",
        "pending_riscv_instruction_probe_checkpoint_indices: BTreeSet<String>",
        1,
    );
    assert_ne!(labels, host);
    assert!(!probe_finalization_contract(
        &labels,
        &action,
        &action_tests,
        &stats,
        &errors,
    ));

    let broad = action.replacen(
        "self.riscv_instruction_probe_checkpoints[*index].1 = snapshot.clone();",
        "for (_, saved) in &mut self.riscv_instruction_probe_checkpoints { *saved = snapshot.clone(); }",
        1,
    );
    assert_ne!(broad, action);
    assert!(!probe_finalization_contract(
        &host,
        &broad,
        &action_tests,
        &stats,
        &errors,
    ));

    let permissive_restore = action.replacen(".contains(&index)", ".contains(&(index + 1))", 1);
    assert_ne!(permissive_restore, action);
    assert!(!probe_finalization_contract(
        &host,
        &permissive_restore,
        &action_tests,
        &stats,
        &errors,
    ));
}

fn stable_boundary_contract(riscv: &str, capture: &str) -> bool {
    let checked =
        unconditional_method_body(riscv, "RiscvCoreCheckpointPort", "capture_checked_record");
    let mode = unconditional_method_body(
        capture,
        "RiscvCoreCheckpointBank",
        "stage_all_for_execution_mode_switch_into",
    );
    checked.contains(
        "RiscvO3LiveCheckpointCapture::Captured(live)iftick.is_some()=>Some(live.clone())",
    ) && checked.contains("accept_stable_rejection&&projection.stable_capture_is_quiescent()")
        && checked.contains(
            "RiscvO3LiveCheckpointCapture::Captured(_)|RiscvO3LiveCheckpointCapture::Rejected=>",
        )
        && checked.contains("CheckpointError::ComponentNotQuiescent")
        && mode
            .contains("Ok(record)ifrecord.o3_live_checkpoint().is_none()=>Ok((port,record,true))")
        && mode.contains("Ok(_)=>Err(CheckpointError::ComponentNotQuiescent")
}

fn probe_finalization_contract(
    host: &str,
    action: &str,
    action_tests: &str,
    stats: &str,
    errors: &str,
) -> bool {
    let prepare = unconditional_method_body(
        action,
        "SystemActionExecutor",
        "prepare_riscv_instruction_probes_for_manifest",
    );
    let finalize = unconditional_method_body(
        action,
        "SystemActionExecutor",
        "finalize_pending_riscv_instruction_probe_checkpoints",
    );
    let record =
        unconditional_method_body(stats, "RiscvSystemRunDriver", "record_instruction_stats");
    let pending_restore = unconditional_function_body(
        action_tests,
        "pending_instruction_probe_checkpoint_rejects_restore_before_finalization",
    );
    host.contains("pending_riscv_instruction_probe_checkpoint_indices: BTreeSet<usize>")
        && action.contains(".insert(checkpoint_index);")
        && prepare.contains("pending_riscv_instruction_probe_checkpoint_indices.contains(&index)")
        && prepare.contains("Err(SystemError::PendingInstructionProbeCheckpoint{")
        && finalize.contains("self.riscv_instruction_probe_checkpoints[*index].1=snapshot.clone();")
        && finalize.contains(".remove(&index);")
        && record.contains("record_retired_instruction_probe(")
        && record.contains("finalize_pending_riscv_instruction_probe_checkpoints(tick)")
        && action_tests
            .contains("pending_probe_finalization_updates_only_the_exact_same_label_manifest")
        && pending_restore.contains("Err(SystemError::PendingInstructionProbeCheckpoint{")
        && pending_restore.contains("retired_instruction_probe_snapshot(),before")
        && errors.contains("PendingInstructionProbeCheckpoint { label: String, tick: u64 }")
        && errors.contains("is awaiting instruction probe finalization")
}
