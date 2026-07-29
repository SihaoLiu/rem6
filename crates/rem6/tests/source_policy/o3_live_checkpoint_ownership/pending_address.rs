use super::*;

const POLICY: &str = "tests/source_policy/o3_live_checkpoint_ownership/pending_address.rs";
const DEPENDENT_OWNER: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store.rs";
const OWNER: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint.rs";
const SUPPORT: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_support.rs";
const BOUNDARIES: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_boundaries.rs";
const HIERARCHY: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_hierarchy.rs";
const TIMING: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store/live_checkpoint_timing.rs";
const DEPENDENT_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/boundaries.rs";
const MULTIPLE_BOUNDARIES: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs";
const VALIDATION_OWNER: &str = "tests/cli_run/validation.rs";
const FABRIC_VALIDATION: &str = "tests/cli_run/validation/fabric_checkpoint.rs";
const REGISTRY: &str = "tests/source_policy/core_test_anchors.txt";

const ANCHORS: [&str; 5] = [
    "rem6_run_o3_dependent_store_live_checkpoint_ld_direct",
    "rem6_run_o3_dependent_store_live_checkpoint_window_is_natural",
    "rem6_run_o3_dependent_store_live_checkpoint_amoswap_hierarchy",
    "rem6_run_o3_dependent_store_live_checkpoint_timing_control",
    "rem6_run_o3_dependent_store_live_checkpoint_boundaries",
];

#[test]
fn pending_address_live_checkpoint_cli_policy_is_attached_and_focused() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (owner, module, path) in [
        (
            "tests/source_policy/o3_live_checkpoint_ownership.rs",
            "pending_address",
            "o3_live_checkpoint_ownership/pending_address.rs",
        ),
        (
            DEPENDENT_OWNER,
            "live_checkpoint",
            "dependent_store/live_checkpoint.rs",
        ),
        (
            OWNER,
            "live_checkpoint_support",
            "live_checkpoint_support.rs",
        ),
        (
            OWNER,
            "live_checkpoint_boundaries",
            "live_checkpoint_boundaries.rs",
        ),
        (
            OWNER,
            "live_checkpoint_hierarchy",
            "live_checkpoint_hierarchy.rs",
        ),
        (OWNER, "live_checkpoint_timing", "live_checkpoint_timing.rs"),
        (
            VALIDATION_OWNER,
            "fabric_checkpoint",
            "validation/fabric_checkpoint.rs",
        ),
    ] {
        assert_unconditional_attachment(&read(&rem6.join(owner)), module, path);
    }

    for (relative, maximum) in [
        (POLICY, 350),
        (OWNER, 500),
        (SUPPORT, 350),
        (BOUNDARIES, 150),
        (HIERARCHY, 150),
        (TIMING, 150),
        (FABRIC_VALIDATION, 100),
    ] {
        let path = rem6.join(relative);
        let lines = line_count(&path);
        assert!(
            lines <= maximum,
            "{relative} has {lines} lines, exceeding its {maximum}-line cap"
        );
    }
}

#[test]
fn pending_address_live_checkpoint_cli_anchors_are_real_unique_and_registered() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = repo_root();
    let owner = read(&rem6.join(OWNER));
    let registry = read(&rem6.join(REGISTRY));
    let owners = workspace_definition_owners(&repo);
    assert!(cli_anchor_contract(&owner));
    assert!(registry_contract(&registry));
    for anchor in ANCHORS {
        assert_eq!(
            owners.get(anchor).cloned().unwrap_or_default(),
            vec![format!("crates/rem6/{OWNER}")],
            "{anchor} must have exactly one workspace definition owner"
        );
        assert_eq!(registered_anchor_count(anchor), 1);
    }
}

#[test]
fn pending_address_live_checkpoint_cli_anchor_mutations_fail_policy() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let owner = read(&rem6.join(OWNER));
    let registry = read(&rem6.join(REGISTRY));
    for anchor in ANCHORS {
        let noop = noop_enabled_function(&owner, anchor).unwrap();
        assert!(!cli_anchor_contract(&noop), "no-op {anchor} must fail");

        let test = format!("#[test]\nfn {anchor}");
        let disabled = owner.replacen(&test, &format!("#[test]\n#[cfg(any())]\nfn {anchor}"), 1);
        assert_ne!(disabled, owner, "disable mutation must apply: {anchor}");
        assert!(!cli_anchor_contract(&disabled));

        let renamed = owner.replacen(&format!("fn {anchor}"), &format!("fn renamed_{anchor}"), 1);
        assert_ne!(renamed, owner, "rename mutation must apply: {anchor}");
        assert!(!cli_anchor_contract(&renamed));

        let unregistered = registry.replacen(anchor, "", 1);
        assert_ne!(
            unregistered, registry,
            "registry mutation must apply: {anchor}"
        );
        assert!(!registry_contract(&unregistered));
    }
    let reordered = registry
        .replacen(ANCHORS[0], "__first_pending_address_anchor__", 1)
        .replacen(ANCHORS[1], ANCHORS[0], 1)
        .replacen("__first_pending_address_anchor__", ANCHORS[1], 1);
    assert_ne!(reordered, registry, "registry reorder mutation must apply");
    assert!(!registry_contract(&reordered));
}

#[test]
fn pending_address_live_checkpoint_cli_proves_hierarchy_timing_and_boundaries() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let support = compact_rust(&read(&rem6.join(SUPPORT)));
    let timing = compact_rust(&read(&rem6.join(TIMING)));
    let boundaries = compact_rust(&read(&rem6.join(BOUNDARIES)));
    let hierarchy = compact_rust(&read(&rem6.join(HIERARCHY)));
    let dependent = compact_rust(&read(&rem6.join(DEPENDENT_BOUNDARIES)));
    let multiple = compact_rust(&read(&rem6.join(MULTIPLE_BOUNDARIES)));

    for marker in [
        "forfieldin[\"issue_tick\",\"lsq_data_response_tick\",\"writeback_tick\",\"commit_tick\",]",
        "assert!(event_u64(restored_store,\"issue_tick\")<event_u64(restored_store,\"lsq_data_response_tick\"))",
        "assert_eq!(requests,data_requests_sent(baseline))",
        "Some(\"pending_data_address\")",
        "Some(2)",
        "Some(0)",
        "Some(1)",
        "assert_eq!(restored.pointer(\"/memory\"),baseline.pointer(\"/memory\"))",
        "restoredexactly-oncestats{pointer}",
    ] {
        assert!(support.contains(marker), "restore support is missing `{marker}`");
    }
    for marker in [
        "Some(\"fabric0\")",
        "Some(\"fabric-runtime-state\")",
        "assert_eq!(event_u64(fabric,\"chunk_count\"),2)",
        "assert_store_request_transport(restored,baseline_head,restored_store,row.memory_system)",
        "\"/memory_resources/cache/data\"",
        "\"/memory_resources/fabric/activity\"",
        "\"/memory_resources/fabric/bytes\"",
        "\"/memory_resources/dram/writes\"",
        "\"/memory_resources/dram/read_bytes\"",
        "restoredhierarchyworkparity{pointer}",
    ] {
        assert!(
            hierarchy.contains(marker),
            "hierarchy proof is missing `{marker}`"
        );
    }
    for marker in [
        "chunk.pointer(\"/name\").and_then(Value::as_str)!=Some(O3_LIVE_CHECKPOINT_CHUNK)",
        "timing.pointer(pointer),timing_baseline.pointer(pointer)",
        "timing.pointer(pointer),detailed.pointer(pointer)",
        "assert_timing_has_no_o3_surfaces(timing)",
    ] {
        assert!(
            timing.contains(marker),
            "timing control is missing `{marker}`"
        );
    }
    for marker in [
        "assert_eq!(output.status.code(),Some(2)",
        "assert!(output.stdout.is_empty()",
        "assert!(!artifact.exists()",
        "--host-switch-cpu-mode",
        "assert_live_checkpoint_rejects_multiple_pending_rows",
        "assert_live_checkpoint_rejects_dependent_atomic_and_mmio",
    ] {
        assert!(
            boundaries.contains(marker),
            "boundary proof is missing `{marker}`"
        );
    }
    assert_eq!(
        boundaries
            .matches("assert_store_target_unchanged(&control,row)")
            .count(),
        3
    );
    for source in [&dependent, &multiple] {
        for marker in [
            "assert_eq!(output.status.code(),Some(2)",
            "assert!(output.stdout.is_empty()",
            "assert!(!artifact.exists()",
        ] {
            assert!(
                source.contains(marker),
                "retained boundary is missing `{marker}`"
            );
        }
    }
    assert!(dependent.contains("targetchangedbeforetherejectedaction"));
    assert!(
        multiple.contains("resident.pointer(\"/memory\"),delivered_control.pointer(\"/memory\")")
    );
}

#[test]
fn pending_address_live_checkpoint_ledger_is_exact_and_score_neutral() {
    let ledger_path = repo_root().join(LEDGER);
    let ledger = read(&ledger_path);
    assert_eq!(line_count(&ledger_path), 1_200);
    assert!(ledger_contract(&ledger));

    for (from, to) in [
        ("exactly one post-publication", "post-publication"),
        (ANCHORS[2], "renamed_pending_address_hierarchy_anchor"),
        (
            "Pre-response producer transport",
            "Restored pre-response producer transport",
        ),
    ] {
        let mutated = ledger.replacen(from, to, 1);
        assert_ne!(mutated, ledger, "ledger mutation must apply: {from}");
        assert!(!ledger_contract(&mutated));
    }
}

fn cli_anchor_contract(source: &str) -> bool {
    if enabled_top_level_tests(OWNER, source)
        != ANCHORS
            .iter()
            .map(|anchor| (*anchor).to_string())
            .collect::<Vec<_>>()
    {
        return false;
    }
    let markers: [(&str, &[&str]); 5] = [
        (
            ANCHORS[0],
            &[
                "assert_pending_store_live_restore",
                "assert_pending_store_restore_replaces_divergent_mode",
            ],
        ),
        (
            ANCHORS[1],
            &[
                "assert_pending_store_live_window",
                "assert_pending_store_live_capture",
            ],
        ),
        (
            ANCHORS[2],
            &[
                "assert_pending_store_live_restore",
                "assert_pending_store_live_hierarchy",
            ],
        ),
        (ANCHORS[3], &["assert_pending_store_timing_control"]),
        (ANCHORS[4], &["assert_pending_store_live_boundaries"]),
    ];
    markers.into_iter().all(|(anchor, required)| {
        enabled_anchor_has_markers(source, anchor, required.iter().copied())
    })
}

fn registry_contract(registry: &str) -> bool {
    let registered = registry
        .lines()
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let Some(start) = registered.iter().position(|line| *line == ANCHORS[0]) else {
        return false;
    };
    registered.get(start..start + ANCHORS.len()) == Some(ANCHORS.as_slice())
        && ANCHORS
            .iter()
            .all(|anchor| registered.iter().filter(|line| *line == anchor).count() == 1)
}

fn assert_unconditional_attachment(source: &str, module: &str, path: &str) {
    assert_eq!(
        unconditional_module_attachment_count(source, module, path),
        1
    );
    let multiline = format!("#[path = \"{path}\"]\nmod {module};");
    let one_line = format!("#[path = \"{path}\"] mod {module};");
    let attachment = if source.contains(&multiline) {
        multiline
    } else {
        one_line
    };
    assert!(source.contains(&attachment), "missing attachment: {module}");
    for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
        let disabled = source.replacen(&attachment, &format!("{conditional}{attachment}"), 1);
        assert_ne!(disabled, source, "attachment mutation must apply: {module}");
        assert_eq!(
            unconditional_module_attachment_count(&disabled, module, path),
            0
        );
    }
}

fn ledger_contract(ledger: &str) -> bool {
    let cpu = component_section(ledger, "### CPU Execution Models - 74% representative");
    cpu.contains("**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap.")
        && cpu.contains("exactly one post-publication, committed-producer, unmaterialized dependent `SD`")
        && cpu.contains("Pre-response producer transport, general IQ shapes, multiple pending-address rows, materialized or submitted stores, dependent atomics, translated/MMIO memory, broader memory/result state, broad O3 restoration, and a general O3 engine remain non-restorable.")
        && ANCHORS.iter().all(|anchor| cpu.contains(anchor))
        && !ledger.contains(", and addressless pending-state serialization")
        && !normalized_policy_text(cpu).contains("checkpoint restorable pre response producer transport")
}
