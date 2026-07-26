use super::*;

const MAX_PERSISTENT_IQ_CLI_LINES: usize = 900;
const MAX_PERSISTENT_IQ_POLICY_LINES: usize = 500;
const MAX_PERSISTENT_IQ_MIXED_COMPUTE_FIXTURE_LINES: usize = 320;
const MAX_PERSISTENT_IQ_MIXED_COMPUTE_TEST_LINES: usize = 320;
const MAX_PERSISTENT_IQ_MIXED_COMPUTE_BOUNDARY_LINES: usize = 320;
const MAX_TYPED_FORWARDING_FIXTURE_LINES: usize = 220;
const MAX_TYPED_FORWARDING_TEST_LINES: usize = 320;
const MAX_TYPED_FORWARDING_BOUNDARY_LINES: usize = 420;
const PERSISTENT_IQ_CLI: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq.rs";
const MIXED_COMPUTE_FIXTURE: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_fixture.rs";
const MIXED_COMPUTE_TESTS: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute.rs";
const MIXED_COMPUTE_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/mixed_compute_boundaries.rs";
const TYPED_FORWARDING_FIXTURE: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_fixture.rs";
const TYPED_FORWARDING_TESTS: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding.rs";
const TYPED_FORWARDING_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/typed_forwarding_boundaries.rs";
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
const MIXED_COMPUTE_ANCHORS: [&str; 7] = [
    "rem6_run_o3_persistent_iq_width_one_serializes_fp_vector_results_direct",
    "rem6_run_o3_persistent_iq_width_two_coissues_fp_vector_and_blocks_second_fp_direct",
    "rem6_run_o3_persistent_iq_width_four_mixed_compute_hierarchy",
    "rem6_run_o3_persistent_iq_dependent_fp_forwards_direct",
    "rem6_run_o3_persistent_iq_vector_destination_boundary",
    "rem6_run_o3_persistent_iq_mixed_compute_checkpoint_boundary",
    "rem6_run_timing_suppresses_o3_mixed_compute_surface",
];
const TYPED_FORWARDING_ANCHORS: [&str; 7] = [
    "rem6_run_o3_typed_live_forwarding_width_one_direct",
    "rem6_run_o3_typed_live_forwarding_width_two_direct",
    "rem6_run_o3_typed_live_forwarding_width_four_hierarchy",
    "rem6_run_o3_typed_live_forwarding_checkpoint_boundaries",
    "rem6_run_o3_typed_live_forwarding_handoff_rejects_live_state",
    "rem6_run_o3_typed_live_forwarding_drained_restore",
    "rem6_run_timing_suppresses_o3_typed_live_forwarding",
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
    assert!(crate_dir.join(MIXED_COMPUTE_FIXTURE).is_file());
    assert!(crate_dir.join(MIXED_COMPUTE_TESTS).is_file());
    assert!(crate_dir.join(MIXED_COMPUTE_BOUNDARIES).is_file());
    assert!(crate_dir.join(TYPED_FORWARDING_FIXTURE).is_file());
    assert!(crate_dir.join(TYPED_FORWARDING_TESTS).is_file());
    assert!(crate_dir.join(TYPED_FORWARDING_BOUNDARIES).is_file());
    assert!(line_count(&cli) <= MAX_PERSISTENT_IQ_CLI_LINES);
    assert!(
        line_count(&crate_dir.join(MIXED_COMPUTE_FIXTURE))
            <= MAX_PERSISTENT_IQ_MIXED_COMPUTE_FIXTURE_LINES
    );
    assert!(
        line_count(&crate_dir.join(MIXED_COMPUTE_TESTS))
            <= MAX_PERSISTENT_IQ_MIXED_COMPUTE_TEST_LINES
    );
    assert!(
        line_count(&crate_dir.join(MIXED_COMPUTE_BOUNDARIES))
            <= MAX_PERSISTENT_IQ_MIXED_COMPUTE_BOUNDARY_LINES
    );
    assert!(
        line_count(&crate_dir.join(TYPED_FORWARDING_FIXTURE)) <= MAX_TYPED_FORWARDING_FIXTURE_LINES
    );
    assert!(line_count(&crate_dir.join(TYPED_FORWARDING_TESTS)) <= MAX_TYPED_FORWARDING_TEST_LINES);
    assert!(
        line_count(&crate_dir.join(TYPED_FORWARDING_BOUNDARIES))
            <= MAX_TYPED_FORWARDING_BOUNDARY_LINES
    );
    assert!(line_count(&policy) <= MAX_PERSISTENT_IQ_POLICY_LINES);
    for retired in RETIRED_GENERAL_IQ_OWNERS {
        assert!(
            !crate_dir.join(retired).exists(),
            "retired general-IQ owner still exists: {retired}",
        );
    }

    let source = fs::read_to_string(&cli).unwrap();
    for (module, path) in [
        (
            "mixed_compute_fixture",
            "persistent_iq/mixed_compute_fixture.rs",
        ),
        ("mixed_compute", "persistent_iq/mixed_compute.rs"),
        (
            "mixed_compute_boundaries",
            "persistent_iq/mixed_compute_boundaries.rs",
        ),
        (
            "typed_forwarding_fixture",
            "persistent_iq/typed_forwarding_fixture.rs",
        ),
        ("typed_forwarding", "persistent_iq/typed_forwarding.rs"),
        (
            "typed_forwarding_boundaries",
            "persistent_iq/typed_forwarding_boundaries.rs",
        ),
    ] {
        assert_eq!(
            module_path_attachment_count(&source, module, path),
            1,
            "{PERSISTENT_IQ_CLI} must attach {module} from {path} exactly once",
        );
    }
    let mixed_compute_source = fs::read_to_string(crate_dir.join(MIXED_COMPUTE_TESTS)).unwrap();
    let mixed_compute_tests =
        parsed_enabled_test_definition_names(MIXED_COMPUTE_TESTS, &mixed_compute_source);
    for anchor in [
        "rem6_run_o3_persistent_iq_width_one_serializes_fp_vector_results_direct",
        "rem6_run_o3_persistent_iq_width_two_coissues_fp_vector_and_blocks_second_fp_direct",
        "rem6_run_o3_persistent_iq_width_four_mixed_compute_hierarchy",
    ] {
        assert_eq!(function_definition_count(&mixed_compute_tests, anchor), 1);
    }
    let boundary_source = fs::read_to_string(crate_dir.join(MIXED_COMPUTE_BOUNDARIES)).unwrap();
    let boundary_tests =
        parsed_enabled_test_definition_names(MIXED_COMPUTE_BOUNDARIES, &boundary_source);
    for anchor in [
        "rem6_run_o3_persistent_iq_dependent_fp_forwards_direct",
        "rem6_run_o3_persistent_iq_vector_destination_boundary",
        "rem6_run_o3_persistent_iq_mixed_compute_checkpoint_boundary",
        "rem6_run_timing_suppresses_o3_mixed_compute_surface",
    ] {
        assert_eq!(function_definition_count(&boundary_tests, anchor), 1);
    }
    let typed_source = fs::read_to_string(crate_dir.join(TYPED_FORWARDING_TESTS)).unwrap();
    let typed_tests = parsed_enabled_test_definition_names(TYPED_FORWARDING_TESTS, &typed_source);
    let typed_boundary_source =
        fs::read_to_string(crate_dir.join(TYPED_FORWARDING_BOUNDARIES)).unwrap();
    let typed_boundary_tests =
        parsed_enabled_test_definition_names(TYPED_FORWARDING_BOUNDARIES, &typed_boundary_source);
    for anchor in &TYPED_FORWARDING_ANCHORS[..3] {
        assert_eq!(function_definition_count(&typed_tests, anchor), 1);
    }
    for anchor in &TYPED_FORWARDING_ANCHORS[3..] {
        assert_eq!(function_definition_count(&typed_boundary_tests, anchor), 1);
    }
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
    for anchor in MIXED_COMPUTE_ANCHORS {
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
    for anchor in TYPED_FORWARDING_ANCHORS {
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
    for anchor in PERSISTENT_IQ_ANCHORS
        .into_iter()
        .chain(MIXED_COMPUTE_ANCHORS)
        .chain(TYPED_FORWARDING_ANCHORS)
    {
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
    for claim in [
        "issued_by_class.scalar_float",
        "issued_by_class.vector_to_scalar",
        "scalar FP and vector-to-scalar",
    ] {
        assert!(cpu.contains(claim), "CPU evidence is missing `{claim}`");
    }
    for claim in [
        "exact FP `00001041` bytes",
        "exact vector-result bridge bytes",
        "typed dependency wakes",
        "vmul.vv -> vmv.x.s",
    ] {
        assert!(cpu.contains(claim), "CPU evidence is missing `{claim}`");
    }
    assert!(cpu.contains(
        "true vector-register producers and destinations, vector LMUL/mask/tail/v0/load/VCSR-aware forwarding, FP loads, double precision, conversions, broader or status-sensitive FP chains, arbitrary unbounded mixed dependency graphs, positive system issue rows, a general load/store queue scheduler, dependent stores or arbitrary atomics, checkpoint-restorable live IQ/transport state, and a general O3 engine remain incomplete"
    ));
    let normalized_ledger = normalized_policy_text(&ledger);
    for broad_claim in ["persistent vector arithmetic iq", "system issue support"] {
        assert!(
            !normalized_ledger.contains(broad_claim),
            "migration ledger overclaims `{broad_claim}`",
        );
    }

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
    for class in [
        "issued_by_class.scalar_float",
        "issued_by_class.vector_to_scalar",
    ] {
        assert!(
            host_note.contains(class),
            "O3 host-action note is missing `{class}`",
        );
    }
}

#[test]
fn o3_persistent_iq_policy_descends_and_rejects_conditional_evidence() {
    let source = r#"
        fn anchor() {}
        mod nested {
            #[test] fn anchor() {}
            mod deeper { fn descendant() {} }
        }
    "#;
    let definitions = parsed_function_definition_names("synthetic.rs", source);
    assert_eq!(function_definition_count(&definitions, "anchor"), 2);
    assert_eq!(function_definition_count(&definitions, "descendant"), 1);
    let count = |source: &str| module_path_attachment_count(source, "typed", "typed.rs");
    for conditional in ["cfg(any())", "cfg_attr(all(), cfg(any()))"] {
        for marker in ["#", "#!"] {
            let module = format!("{marker}[{conditional}]\n#[path = \"typed.rs\"]\nmod typed;");
            let test = format!("{marker}[{conditional}]\n#[test]\nfn typed_anchor() {{}}");
            assert_eq!(count(&module), 0);
            assert!(parsed_enabled_test_definition_names("synthetic.rs", &test).is_empty());
        }
    }
    assert_eq!(count("#[path = \"typed.rs\"] mod typed {}"), 0);
}

fn parsed_function_definition_names(relative: &str, source: &str) -> Vec<String> {
    let syntax = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"))
        .items;
    let mut definitions = Vec::new();
    collect_function_definition_names(&syntax, &mut definitions);
    definitions
}

fn collect_function_definition_names(items: &[syn::Item], definitions: &mut Vec<String>) {
    for item in items {
        match item {
            syn::Item::Fn(function) => definitions.push(function.sig.ident.to_string()),
            syn::Item::Mod(module) => {
                if let Some((_, items)) = &module.content {
                    collect_function_definition_names(items, definitions);
                }
            }
            _ => {}
        }
    }
}

fn function_definition_count(definitions: &[String], anchor: &str) -> usize {
    definitions
        .iter()
        .filter(|definition| definition.as_str() == anchor)
        .count()
}

fn module_path_attachment_count(source: &str, module_name: &str, expected_path: &str) -> usize {
    let syntax = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse Rust source for module policy: {error}"));
    if has_conditional_compilation_attribute(&syntax.attrs) {
        return 0;
    }
    syntax
        .items
        .iter()
        .filter(|item| {
            let syn::Item::Mod(module) = item else {
                return false;
            };
            module.ident == module_name
                && module.content.is_none()
                && !has_conditional_compilation_attribute(&module.attrs)
                && module.attrs.iter().any(|attribute| {
                    let syn::Meta::NameValue(value) = &attribute.meta else {
                        return false;
                    };
                    let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(path),
                        ..
                    }) = &value.value
                    else {
                        return false;
                    };
                    attribute.path().is_ident("path") && path.value() == expected_path
                })
        })
        .count()
}

fn parsed_enabled_test_definition_names(relative: &str, source: &str) -> Vec<String> {
    let syntax = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"));
    if has_conditional_compilation_attribute(&syntax.attrs) {
        return Vec::new();
    }
    syntax
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            let is_test = function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("test"));
            let is_ignored = function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("ignore"));
            (is_test && !is_ignored && !has_conditional_compilation_attribute(&function.attrs))
                .then(|| function.sig.ident.to_string())
        })
        .collect()
}

fn has_conditional_compilation_attribute(attributes: &[syn::Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

fn component_section<'a>(ledger: &'a str, heading: &str) -> &'a str {
    let after = ledger
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing component heading `{heading}`"))
        .1;
    after.split("\n### ").next().unwrap_or(after)
}

fn normalized_policy_text(source: &str) -> String {
    source
        .chars()
        .map(|character| {
            character
                .is_ascii_alphanumeric()
                .then(|| character.to_ascii_lowercase())
                .unwrap_or(' ')
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
