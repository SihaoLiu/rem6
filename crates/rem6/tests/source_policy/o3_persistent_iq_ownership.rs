use super::*;

const MAX_PERSISTENT_IQ_CLI_LINES: usize = 900;
const MAX_PERSISTENT_IQ_POLICY_LINES: usize = 600;
const PERSISTENT_IQ_CLI: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq.rs";
const MIGRATION_LEDGER: &str = "docs/architecture/gem5-to-rem6-migration.md";
const O3_CLI_DIR: &str = "tests/cli_run/m5_host_actions/o3";
const RETIRED_GENERAL_IQ_OWNERS: [&str; 2] = [
    "tests/cli_run/m5_host_actions/o3/scoped_issue/general_iq.rs",
    "tests/cli_run/m5_host_actions/o3/predicted_control/general_iq.rs",
];
const PERSISTENT_IQ_ANCHORS: [&str; 11] = [
    "rem6_run_o3_persistent_iq_width_one_oldest_ready_cross_class_direct",
    "rem6_run_o3_persistent_iq_width_two_coissues_ready_cross_class_direct",
    "rem6_run_o3_persistent_iq_width_four_respects_class_caps_hierarchy",
    "rem6_run_o3_persistent_iq_cross_class_wakeup_matrix_direct",
    "rem6_run_o3_persistent_iq_squash_discards_wrong_path_queue_suffix",
    "rem6_run_o3_persistent_iq_text_stats_expose_queue_counters",
    "rem6_run_o3_persistent_iq_stats_dump_exposes_queue_counters",
    "rem6_run_o3_persistent_iq_debug_exposes_residency_and_cleanup",
    "rem6_run_host_switch_preserves_o3_persistent_iq_ticks",
    "rem6_run_o3_persistent_iq_checkpoint_boundary",
    "rem6_run_timing_suppresses_o3_persistent_iq_surface",
];
const MOVED_GENERAL_IQ_ANCHORS: [&str; 6] = [
    "rem6_run_o3_general_iq_oldest_ready_width_one_direct",
    "rem6_run_o3_general_iq_oldest_ready_width_two_direct",
    "rem6_run_o3_general_iq_control_release_orders_descendant",
    "rem6_run_host_switch_preserves_o3_general_iq_ticks",
    "rem6_run_o3_general_iq_checkpoint_boundary",
    "rem6_run_timing_suppresses_o3_general_iq_surface",
];

#[test]
fn o3_persistent_iq_focused_owners_exist_and_stay_bounded() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli = crate_dir.join(PERSISTENT_IQ_CLI);
    let policy = crate_dir.join("tests/source_policy/o3_persistent_iq_ownership.rs");

    assert!(cli.is_file(), "missing {}", cli.display());
    assert!(line_count(&cli) <= MAX_PERSISTENT_IQ_CLI_LINES);
    assert!(line_count(&policy) <= MAX_PERSISTENT_IQ_POLICY_LINES);
    for retired in RETIRED_GENERAL_IQ_OWNERS {
        assert!(
            !crate_dir.join(retired).exists(),
            "retired general-IQ owner still exists: {retired}",
        );
    }

    let source = fs::read_to_string(&cli).unwrap();
    let definitions = parsed_function_definition_names(PERSISTENT_IQ_CLI, &source);
    let global_definitions = rust_source_files(&crate_dir.join(O3_CLI_DIR))
        .into_iter()
        .flat_map(|path| {
            let relative = path.strip_prefix(crate_dir).unwrap().display().to_string();
            let source = fs::read_to_string(&path).unwrap();
            parsed_function_definition_names(&relative, &source)
        })
        .collect::<Vec<_>>();
    for anchor in PERSISTENT_IQ_ANCHORS {
        assert_eq!(
            function_definition_count(&definitions, anchor),
            1,
            "{PERSISTENT_IQ_CLI} must define exactly one `fn {anchor}`"
        );
        assert_eq!(
            CORE_TEST_ANCHORS
                .lines()
                .filter(|registered| *registered == anchor)
                .count(),
            1,
            "core_test_anchors.txt must register `{anchor}` exactly once"
        );
        assert_eq!(
            function_definition_count(&global_definitions, anchor),
            1,
            "{O3_CLI_DIR} must define `{anchor}` exactly once globally",
        );
    }
    for anchor in MOVED_GENERAL_IQ_ANCHORS {
        assert_eq!(
            function_definition_count(&definitions, anchor),
            0,
            "{PERSISTENT_IQ_CLI} must not retain moved general-IQ anchor `{anchor}`"
        );
        assert_eq!(
            function_definition_count(&global_definitions, anchor),
            0,
            "{O3_CLI_DIR} must not retain moved general-IQ anchor `{anchor}`",
        );
        assert_eq!(
            CORE_TEST_ANCHORS
                .lines()
                .filter(|registered| *registered == anchor)
                .count(),
            0,
            "core_test_anchors.txt must not retain moved general-IQ anchor `{anchor}`",
        );
    }
}

#[test]
fn o3_persistent_iq_ledger_claims_match_executable_evidence() {
    let ledger = fs::read_to_string(repo_root().join(MIGRATION_LEDGER)).unwrap();
    let cpu = component_section(&ledger, "### CPU Execution Models - 74% representative");
    let stats = component_section(
        &ledger,
        "### Stats, Probes, Debug, Host Actions, and Checkpointing - 74% representative",
    );
    assert!(cpu.contains(
        "**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap."
    ));
    assert!(stats
        .contains("**Score calculation:** 24 of 26 items have executable evidence, or 92% raw."));
    assert!(stats.contains("The bucket cap is\nrepresentative"));

    for claim in [
        "Bounded per-run persistent cross-class O3 issue queue evidence",
        "scheduler-turn wakeup/select at configured widths 1, 2, and 4",
        "same-tick projected arbitration",
        "bounded transaction rollback",
        "empty-IQ handoff gating",
        "live checkpoint rejection",
        "drained O3RT v23 restore",
        "timing suppression",
    ] {
        assert!(cpu.contains(claim), "CPU evidence is missing `{claim}`");
    }
    for anchor in PERSISTENT_IQ_ANCHORS {
        assert!(cpu.contains(anchor), "CPU evidence is missing `{anchor}`");
    }
    for retired in MOVED_GENERAL_IQ_ANCHORS {
        assert!(
            !ledger.contains(retired),
            "migration ledger retains retired anchor `{retired}`",
        );
    }
    assert!(!ledger.contains("rem6_run_o3_general_iq_pending_address_and_scalar_hierarchy"));
    assert!(!ledger.contains(
        "persistent and cross-class IQ/wakeup/select beyond the derived scalar/control/capacity-three-pending-address live queue"
    ));
    assert!(cpu.contains(
        "FP/vector arithmetic and system issue rows, a general load/store queue scheduler, dependent stores or arbitrary atomics, arbitrary nonadjacent or unbounded dependency graphs, checkpoint-restorable live IQ/transport state, and a general O3 engine"
    ));

    let queue_surfaces = [
        "/cores/0/o3_runtime/issue/queue",
        "sim.cpu0.o3.issue_queue.*",
        "sim.host_actions.stats_dump.cpu0.o3.issue_queue.*",
        "/debug/o3_trace/0/issue_queue/events",
        "telemetry is transient and absent from O3RT v23",
    ];
    for surface in queue_surfaces {
        assert!(
            stats.contains(surface),
            "Stats evidence is missing `{surface}`"
        );
    }
    let host_note = ledger
        .lines()
        .find(|line| line.starts_with("O3 host-action stats note:"))
        .expect("missing O3 host-action stats note");
    for surface in queue_surfaces {
        assert!(
            host_note.contains(surface),
            "O3 host-action note is missing `{surface}`",
        );
    }
}

fn parsed_function_definition_names(relative: &str, source: &str) -> Vec<String> {
    syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"))
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            Some(function.sig.ident.to_string())
        })
        .collect()
}

fn function_definition_count(definitions: &[String], anchor: &str) -> usize {
    definitions
        .iter()
        .filter(|definition| definition.as_str() == anchor)
        .count()
}

fn component_section<'a>(ledger: &'a str, heading: &str) -> &'a str {
    let after = ledger
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing component heading `{heading}`"))
        .1;
    after.split("\n### ").next().unwrap_or(after)
}
