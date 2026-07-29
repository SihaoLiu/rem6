use super::*;
#[path = "live_o3_checkpoint/authority.rs"]
mod authority;
#[path = "live_o3_checkpoint/pending_address.rs"]
mod pending_address;
const LEDGER: &str = "docs/architecture/gem5-to-rem6-migration.md";
fn read(crate_dir: &Path, relative: &str) -> String {
    fs::read_to_string(crate_dir.join(relative)).unwrap()
}
fn compact(source: &str) -> String {
    without_whitespace(&rust_code_without_comments_and_literals(source))
}
fn unconditional_function_body(source: &str, name: &str) -> String {
    compact(
        &unconditional_rust_function_definition(source, name)
            .unwrap_or_else(|| panic!("missing unique unconditional function `{name}`")),
    )
}
fn unconditional_method_body(source: &str, owner: &str, name: &str) -> String {
    compact(
        &unconditional_rust_impl_method_definition(source, owner, name)
            .unwrap_or_else(|| panic!("missing unique unconditional `{owner}::{name}` method")),
    )
}

fn ordered(source: &str, first: &str, second: &str) -> bool {
    match (source.find(first), source.find(second)) {
        (Some(first), Some(second)) => first < second,
        _ => false,
    }
}

fn ordered_chain(source: &str, markers: &[&str]) -> bool {
    markers
        .windows(2)
        .all(|pair| ordered(source, pair[0], pair[1]))
}

fn swapped(source: &str, first: &str, second: &str) -> String {
    source
        .replacen(first, "__FIRST_MARKER__", 1)
        .replacen(second, "__SECOND_MARKER__", 1)
        .replace("__FIRST_MARKER__", second)
        .replace("__SECOND_MARKER__", first)
}

fn parsed_unconditional_external_module_count(source: &str, module: &str, path: &str) -> usize {
    let source = strip_comments(source);
    let lines = source.lines().collect::<Vec<_>>();
    let path_attribute = format!("#[path = \"{path}\"]");
    let module_declaration = format!("mod {module};");
    lines
        .iter()
        .enumerate()
        .filter(|(index, line)| {
            if line.trim() != path_attribute {
                return false;
            }
            let mut next = index + 1;
            while next < lines.len() && lines[next].trim().is_empty() {
                next += 1;
            }
            if next >= lines.len() || lines[next].trim() != module_declaration {
                return false;
            }
            let mut previous = *index;
            while previous > 0 {
                let candidate = lines[previous - 1].trim();
                if candidate.is_empty() {
                    previous -= 1;
                    continue;
                }
                if !candidate.starts_with("#[") {
                    break;
                }
                if candidate.starts_with("#[cfg") || candidate.starts_with("#[cfg_attr") {
                    return false;
                }
                previous -= 1;
            }
            true
        })
        .count()
}

fn strip_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut block_depth = 0usize;
    let mut line_comment = false;
    let mut string = false;
    let mut character = false;
    let mut escaped = false;
    while let Some(current) = chars.next() {
        if line_comment {
            if current == '\n' {
                line_comment = false;
                output.push(current);
            }
            continue;
        }
        if block_depth > 0 {
            if current == '/' && chars.peek() == Some(&'*') {
                chars.next();
                block_depth += 1;
            } else if current == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block_depth -= 1;
            } else if current == '\n' {
                output.push('\n');
            }
            continue;
        }
        if !string && !character && current == '/' && chars.peek() == Some(&'/') {
            chars.next();
            line_comment = true;
            continue;
        }
        if !string && !character && current == '/' && chars.peek() == Some(&'*') {
            chars.next();
            block_depth = 1;
            continue;
        }
        output.push(current);
        if escaped {
            escaped = false;
        } else if (string || character) && current == '\\' {
            escaped = true;
        } else if !character && current == '"' {
            string = !string;
        } else if !string && current == '\'' {
            character = !character;
        }
    }
    output
}

#[test]
fn riscv_checkpoint_live_o3_sources_stay_within_caps() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let caps = [
        ("src/riscv_checkpoint.rs", 1800),
        ("src/riscv_checkpoint/capture.rs", 125),
        ("src/scheduler_checkpoint.rs", 1800),
        ("src/scheduler_checkpoint/live_o3.rs", 400),
        ("src/scheduler_checkpoint/locked_bank.rs", 350),
        ("tests/riscv_checkpoint/o3_live.rs", 700),
        ("tests/live_o3_scheduler_checkpoint.rs", 650),
        (
            "src/host/action_apply/tests/checkpoint_atomicity_tests.rs",
            125,
        ),
        ("tests/source_policy/live_o3_checkpoint/authority.rs", 100),
        ("tests/source_policy/live_o3_checkpoint.rs", 500),
    ];
    for (relative, maximum) in caps {
        let path = crate_dir.join(relative);
        assert!(
            line_count(&path) <= maximum,
            "{relative} exceeds {maximum} lines"
        );
    }
    let policy = read(crate_dir, "tests/source_policy.rs");
    assert_eq!(
        parsed_unconditional_external_module_count(
            &policy,
            "live_o3_checkpoint",
            "source_policy/live_o3_checkpoint.rs",
        ),
        1
    );
    for attribute in ["#[cfg(test)]\n", "#[cfg_attr(test, allow(dead_code))]\n"] {
        let mutated = policy.replace(
            "#[path = \"source_policy/live_o3_checkpoint.rs\"]\n",
            &format!("{attribute}#[path = \"source_policy/live_o3_checkpoint.rs\"]\n"),
        );
        assert_eq!(
            parsed_unconditional_external_module_count(
                &mutated,
                "live_o3_checkpoint",
                "source_policy/live_o3_checkpoint.rs",
            ),
            0,
            "cfg mutation must not remain an unconditional attachment"
        );
    }
}

#[test]
fn riscv_checkpoint_live_o3_capture_and_restore_contracts_are_ordered() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let riscv = read(crate_dir, "src/riscv_checkpoint.rs");
    let restore_authority = read(crate_dir, "src/riscv_checkpoint/restore_authority.rs");
    let scheduler = read(crate_dir, "src/scheduler_checkpoint.rs");
    let live_o3 = read(crate_dir, "src/scheduler_checkpoint/live_o3.rs");
    let write_record = unconditional_method_body(&riscv, "RiscvCoreCheckpointPort", "write_record");
    let handoff = unconditional_method_body(
        &riscv,
        "RiscvCoreCheckpointBank",
        "capture_target_for_execution_mode_handoff_into_impl",
    );
    let restore_entry = unconditional_method_body(
        &restore_authority,
        "RiscvCoreCheckpointBank",
        "restore_all_from",
    );
    let restore_install = unconditional_method_body(
        &restore_authority,
        "RiscvCoreCheckpointBank",
        "install_decoded_restores",
    );
    let restore = format!("{restore_entry}{restore_install}");
    let scheduler_capture =
        unconditional_method_body(&scheduler, "SchedulerCheckpointContext", "capture_into");
    let live_o3_validate = unconditional_method_body(
        &live_o3,
        "SchedulerCheckpointContext",
        "validate_live_o3_scheduler_restores",
    );

    let capture_markers = [
        "remove_chunk(&self.component,RISCV_O3_LIVE_DATA_HANDOFF_CHUNK)",
        "remove_chunk(&self.component,RISCV_O3_LIVE_CHECKPOINT_CHUNK)",
        "write_chunk(&self.component,RISCV_O3_LIVE_CHECKPOINT_CHUNK",
    ];
    assert!(ordered_chain(&write_record, &capture_markers));
    assert!(!ordered_chain(
        &swapped(&write_record, capture_markers[0], capture_markers[1]),
        &capture_markers
    ));
    assert!(!ordered_chain(
        &write_record.replace(capture_markers[1], ""),
        &capture_markers
    ));
    let handoff_markers = [
        "target_port.write_record(&mutstaged,&record)",
        "RISCV_O3_LIVE_DATA_HANDOFF_CHUNK",
    ];
    assert!(ordered_chain(&handoff, &handoff_markers));
    assert!(!ordered_chain(
        &swapped(&handoff, handoff_markers[0], handoff_markers[1]),
        &handoff_markers
    ));
    assert!(!ordered_chain(
        &handoff.replace(handoff_markers[0], ""),
        &handoff_markers
    ));
    assert!(ordered_chain(
        &restore,
        &[
            "self.decode_and_prepare_all(registry)",
            "port.validate_low_level_restore_authority(record)",
            "port.core.install_prepared_checkpoint_restore(prepared)",
        ]
    ));
    assert!(scheduler_capture.contains("self.scheduler.snapshot()"));
    assert!(live_o3_validate.contains("self.scheduler.snapshot()"));
}

#[test]
fn riscv_checkpoint_capture_finalization_is_transactional() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let riscv_source = read(crate_dir, "src/riscv_checkpoint.rs");
    let bank_source = read(crate_dir, "src/riscv_checkpoint/capture.rs");
    let transfer_source = read(crate_dir, "src/host/execution_mode_transfer.rs");
    let action_source = read(crate_dir, "src/host/action_apply.rs");
    let riscv = compact(&riscv_source);
    let checked = unconditional_function_body(&riscv_source, "capture_checked_record");
    let port_commit = unconditional_function_body(&riscv_source, "commit_checkpoint_capture");
    let host_capture = unconditional_function_body(
        &transfer_source,
        "capture_attached_checkpoint_banks_into_with_scheduler",
    );
    let commit =
        unconditional_function_body(&transfer_source, "commit_attached_checkpoint_capture");
    let switch_capture = unconditional_function_body(
        &transfer_source,
        "capture_execution_mode_switch_state_transfer_with_scheduler",
    );
    let action_capture =
        unconditional_function_body(&action_source, "apply_with_scheduler_context");

    assert!(!checked.contains("finalize_quiescent_o3_writeback_for_checkpoint"));
    assert!(checked.contains(
        "RiscvO3LiveCheckpointCapture::Rejectedifaccept_stable_rejection&&projection.stable_capture_is_quiescent()=>{None}"
    ));
    assert_eq!(
        riscv
            .matches("finalize_quiescent_o3_writeback_for_checkpoint")
            .count(),
        1
    );
    assert!(port_commit.contains("self.core.finalize_quiescent_o3_writeback_for_checkpoint()"));
    assert!(riscv.contains("modcapture;"));
    let standalone = unconditional_function_body(&bank_source, "capture_all_into_at");
    let staged = unconditional_function_body(&bank_source, "stage_all_into_impl");
    let switch =
        unconditional_function_body(&bank_source, "stage_all_for_execution_mode_switch_into");
    assert!(ordered_chain(
        &standalone,
        &[
            "self.stage_all_into_impl(",
            "self.commit_checkpoint_capture()"
        ]
    ));
    assert!(!staged.contains("commit_checkpoint_capture"));
    assert!(staged.contains("port.capture_checked_record(tick,true)"));
    assert!(!switch.contains("commit_checkpoint_capture"));
    assert!(switch.contains("port.capture_checked_record(Some(tick),accept_stable_rejection)"));
    let host_markers = [
        "stage_all_into_at(staged_checkpoints,tick)",
        "virtio_pci_device_config_checkpoints.capture_all_into(staged_checkpoints)",
    ];
    assert!(ordered_chain(&host_capture, &host_markers));
    assert!(!host_capture.contains("commit_checkpoint_capture"));
    assert!(!host_capture.contains("track_borrowed_scheduler_checkpoint_component"));
    assert!(ordered_chain(
        &host_capture,
        &[
            "stage_all_for_execution_mode_switch_into(",
            "scheduler_checkpoint.is_some()",
        ]
    ));
    assert!(!host_capture.contains(".capture_all_into_at(staged_checkpoints,tick)"));
    assert!(ordered_chain(
        &commit,
        &[
            "riscv_checkpoints.commit_checkpoint_capture()",
            "self.track_borrowed_scheduler_checkpoint_component(component)",
        ]
    ));
    let transaction_markers = [
        "self.capture_execution_modes_into(&mutstaged_checkpoints)",
        "staged_checkpoints.capture(",
        "self.commit_attached_checkpoint_capture(&capture)",
    ];
    assert!(ordered_chain(&action_capture, &transaction_markers));
    assert!(ordered_chain(&switch_capture, &transaction_markers));
    assert!(!ordered_chain(
        &swapped(
            &switch_capture,
            transaction_markers[1],
            transaction_markers[2]
        ),
        &transaction_markers
    ));
}

#[test]
fn riscv_checkpoint_policy_ignores_disabled_same_name_decoys() {
    let source = r#"
        #[cfg(any())]
        fn target() { disabled(); }
        fn target() { enabled(); }
    "#;
    let definition = unconditional_rust_function_definition(source, "target").unwrap();
    assert!(definition.contains("enabled"));
    assert!(!definition.contains("disabled"));
    assert!(unconditional_rust_function_definition(
        "#[cfg(any())] fn target() { disabled(); }",
        "target"
    )
    .is_none());
}

#[test]
fn riscv_checkpoint_method_policy_ignores_disabled_same_name_decoys() {
    let source = r#"
        #[cfg(any())]
        impl Owner { fn target() { disabled(); } }
        impl Owner { fn target() { enabled(); } }
    "#;
    let body = unconditional_method_body(source, "Owner", "target");
    assert!(body.contains("enabled"));
    assert!(!body.contains("disabled"));
}

#[test]
fn riscv_checkpoint_system_validation_and_restore_mutations_are_ordered() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = read(crate_dir, "src/host.rs");
    let validate =
        unconditional_method_body(&host, "SystemActionExecutor", "validate_checkpoint_banks");
    let restore =
        unconditional_method_body(&host, "SystemActionExecutor", "restore_checkpoint_banks");
    let stats_sync = unconditional_method_body(
        &host,
        "SystemActionExecutor",
        "sync_riscv_o3_runtime_stats_after_checkpoint_restore",
    );
    let stats = read(crate_dir, "src/riscv_o3_runtime_stats.rs");
    let stats_preflight = unconditional_method_body(
        &stats,
        "RiscvO3RuntimeStats",
        "validate_cpu_snapshot_schema",
    );
    let manifest_restore = unconditional_method_body(
        &host,
        "SystemActionExecutor",
        "restore_checkpoint_manifest_with_scheduler",
    );
    let riscv_validate = validate
        .find("riscv_checkpoints.validate_restore_from(checkpoints)")
        .unwrap();
    let scheduler_validate = validate
        .find("validate_restore_from_with_owned_events(checkpoints,&owned_scheduler_events)")
        .unwrap();
    let stats_validate = validate
        .find("self.validate_riscv_o3_runtime_stats_checkpoint_restore()")
        .unwrap();
    let live_validate = validate
        .find("self.validate_live_o3_scheduler_restores(")
        .unwrap();
    assert!(
        riscv_validate < stats_validate
            && stats_validate < scheduler_validate
            && scheduler_validate < live_validate
    );
    assert!(!validate.contains("self.checkpoints="));
    assert!(stats_preflight.contains("letmutstaged=registry.clone()"));
    assert!(stats_preflight.contains("self.projected_resettable_pipeline_cycles("));
    assert!(!stats_preflight.contains("self.resettable_pipeline_cycles("));

    let restore_markers = [
        "riscv_checkpoints.restore_all_from_with_scheduler_authority(&self.checkpoints)",
        "restore_all_from_with_owned_events(&self.checkpoints,&owned_scheduler_events)",
        "scheduler_checkpoint.restore_from(&self.checkpoints,&owned_scheduler_events)",
        "Self::rebind_live_o3_scheduler_restores(",
        "self.sync_riscv_o3_runtime_stats_after_checkpoint_restore()",
    ];
    assert!(ordered_chain(&restore, &restore_markers));
    assert!(!ordered_chain(
        &swapped(&restore, restore_markers[1], restore_markers[2]),
        &restore_markers
    ));
    assert!(!ordered_chain(
        &restore.replace(restore_markers[2], ""),
        &restore_markers
    ));
    let validate_call = manifest_restore
        .find("self.validate_checkpoint_banks(")
        .unwrap();
    let mutation = manifest_restore
        .find("self.checkpoints=staged_checkpoints")
        .unwrap();
    let restore_call = manifest_restore
        .find("self.restore_checkpoint_banks(")
        .unwrap();
    assert!(validate_call < mutation && mutation < restore_call);
    assert!(stats_sync.contains(".expect()"));
    assert!(!stats_sync.contains('?'));
}

#[test]
fn riscv_checkpoint_live_o3_ledger_claim_is_exact_and_bounded() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let ledger_path = repo.join(LEDGER);
    let ledger = fs::read_to_string(&ledger_path).unwrap();
    assert_eq!(line_count(&ledger_path), 1200);
    let stats = source_section(
        &ledger,
        "### Stats, Probes, Debug, Host Actions, and Checkpointing - 74% representative",
        "### ",
    );
    assert!(stats
        .contains("**Score calculation:** 24 of 26 items have executable evidence, or 92% raw."));
    assert!(stats.contains("exact live O3 decision/writeback replay"));
}
