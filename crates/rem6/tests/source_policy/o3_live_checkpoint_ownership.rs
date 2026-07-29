use super::*;

#[path = "o3_live_checkpoint_ownership/pending_address.rs"]
mod pending_address;
#[path = "o3_live_checkpoint_ownership/reachability.rs"]
mod reachability;

use reachability::{
    enabled_anchor_has_markers, noop_enabled_function, reachable_enabled_functions,
    unconditional_rust_const_definition,
};

const POLICY: &str = "tests/source_policy/o3_live_checkpoint_ownership.rs";
const REACHABILITY_POLICY: &str =
    "tests/source_policy/o3_live_checkpoint_ownership/reachability.rs";
const PARENT: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq.rs";
const FIXTURE: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fixture.rs";
const COMPUTE: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_compute.rs";
const FP_RESULT: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_fp.rs";
const BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/live_checkpoint_boundaries.rs";
const RETAINED_REJECTIONS: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_runtime_boundaries.rs";
const LEDGER: &str = "docs/architecture/gem5-to-rem6-migration.md";

const COMPUTE_ANCHORS: [&str; 3] = [
    "rem6_run_o3_live_checkpoint_compute_serial_direct",
    "rem6_run_o3_live_checkpoint_compute_parallel_direct",
    "rem6_run_o3_live_checkpoint_compute_restore_replays_after_source_progress",
];
const FP_RESULT_ANCHORS: [&str; 3] = [
    "rem6_run_o3_live_checkpoint_flw_result_direct",
    "rem6_run_o3_live_checkpoint_fld_result_direct",
    "rem6_run_o3_live_checkpoint_fp_result_hierarchy_matrix",
];
const BOUNDARY_ANCHORS: [&str; 1] =
    ["rem6_run_o3_live_checkpoint_timing_schedule_suppresses_o3_surfaces"];
const RETAINED_REJECTION_ANCHORS: [&str; 2] = [
    "rem6_run_o3_fp_load_forwarding_checkpoint_boundaries",
    "rem6_run_o3_fp_load_forwarding_handoff_rejects_live_state",
];

#[test]
fn o3_live_checkpoint_cli_owners_are_unconditional_unique_and_bounded() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let policy_parent = read(&rem6.join("tests/source_policy.rs"));
    assert_eq!(
        unconditional_module_attachment_count(
            &policy_parent,
            "o3_live_checkpoint_ownership",
            "source_policy/o3_live_checkpoint_ownership.rs",
        ),
        1,
    );

    let parent = read(&rem6.join(PARENT));
    let owners = [
        (
            "live_checkpoint_fixture",
            "persistent_iq/live_checkpoint_fixture.rs",
            FIXTURE,
            450,
        ),
        (
            "live_checkpoint_compute",
            "persistent_iq/live_checkpoint_compute.rs",
            COMPUTE,
            650,
        ),
        (
            "live_checkpoint_fp",
            "persistent_iq/live_checkpoint_fp.rs",
            FP_RESULT,
            600,
        ),
        (
            "live_checkpoint_boundaries",
            "persistent_iq/live_checkpoint_boundaries.rs",
            BOUNDARIES,
            450,
        ),
    ];
    for (module, path, owner, maximum) in owners {
        assert_eq!(
            unconditional_module_attachment_count(&parent, module, path),
            1,
            "{PARENT} must attach {module} unconditionally",
        );
        let source = read(&rem6.join(owner));
        assert!(
            source.lines().count() <= maximum,
            "{owner} exceeds its {maximum}-line cap",
        );
        assert!(
            !source.contains("include!("),
            "{owner} must be a real owner"
        );
    }
    assert!(read(&rem6.join(POLICY)).lines().count() <= 500);
    assert!(read(&rem6.join(REACHABILITY_POLICY)).lines().count() <= 125);
}

#[test]
fn o3_live_checkpoint_cli_evidence_is_real_unique_and_registered() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = repo_root();
    let owners = workspace_definition_owners(&repo);
    let inventories = [
        (COMPUTE, COMPUTE_ANCHORS.as_slice()),
        (FP_RESULT, FP_RESULT_ANCHORS.as_slice()),
        (BOUNDARIES, BOUNDARY_ANCHORS.as_slice()),
    ];
    for (relative, expected) in inventories {
        let source = read(&rem6.join(relative));
        assert_eq!(
            enabled_top_level_tests(relative, &source),
            expected
                .iter()
                .map(|anchor| (*anchor).to_string())
                .collect::<Vec<_>>(),
        );
        for anchor in expected {
            assert_eq!(
                owners.get(*anchor).cloned().unwrap_or_default(),
                vec![format!("crates/rem6/{relative}")],
                "{anchor} must have one workspace definition owner",
            );
            assert_eq!(registered_anchor_count(anchor), 1);
        }
    }

    let retained = read(&rem6.join(RETAINED_REJECTIONS));
    let retained_tests = enabled_top_level_tests(RETAINED_REJECTIONS, &retained);
    for anchor in RETAINED_REJECTION_ANCHORS {
        assert!(retained_tests.iter().any(|candidate| candidate == anchor));
        assert_eq!(
            owners.get(anchor).cloned().unwrap_or_default(),
            vec![format!("crates/rem6/{RETAINED_REJECTIONS}")],
        );
        assert_eq!(registered_anchor_count(anchor), 1);
    }
}

#[test]
fn o3_live_checkpoint_cli_contract_proves_restore_and_timing_boundaries() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture_source = read(&rem6.join(FIXTURE));
    let compute_source = read(&rem6.join(COMPUTE));
    let fp_source = read(&rem6.join(FP_RESULT));
    let boundaries_source = read(&rem6.join(BOUNDARIES));
    let retained_source = read(&rem6.join(RETAINED_REJECTIONS));
    let reachable = |source: &str, roots: &[&str]| {
        roots
            .iter()
            .map(|root| reachable_enabled_functions(source, root))
            .collect::<Option<Vec<_>>>()
            .expect("every evidence root must be enabled")
            .concat()
    };
    let fixture = reachable(
        &fixture_source,
        &[
            "live_compute_binary",
            "live_compute_baseline",
            "run_live_compute_checkpoint",
            "run_live_compute_restore_discriminator",
        ],
    );
    let boundaries = reachable(&boundaries_source, &BOUNDARY_ANCHORS);
    let retained = reachable(&retained_source, &RETAINED_REJECTION_ANCHORS);

    let chunk = unconditional_rust_const_definition(&fixture_source, "O3_LIVE_CHECKPOINT_CHUNK")
        .expect("enabled O3 live checkpoint chunk constant");
    assert!(compact_rust(&chunk)
        .contains("pub(super)constO3_LIVE_CHECKPOINT_CHUNK:&str=\"o3-live-checkpoint\";"));
    assert!(fixture.contains("command.args([\"--host-checkpoint\",checkpoint.as_str()])"));
    assert!(fixture.contains("command.args([\"--host-restore-checkpoint\",restore.as_str()])"));
    for marker in [
        "SBI_HSM_HART_START",
        "i_type(SBI_HSM_HART_START,0,0,16,0x13)",
        "\"--cores\",\"2\"",
    ] {
        assert!(
            fixture.contains(marker),
            "multicore fixture is missing `{marker}`"
        );
    }

    assert!(compute_cli_contract(&compute_source));
    assert!(fp_cli_contract(&fp_source));
    for marker in [
        "Some(O3_LIVE_CHECKPOINT_CHUNK)",
        "timing_scheduled.pointer(\"/cores/0/o3_runtime\").is_none()",
        "assert!(checkpoint_tick<restore_tick)",
    ] {
        assert!(
            boundaries.contains(marker),
            "timing boundary is missing `{marker}`"
        );
    }
    for marker in [
        "checkpointcomponentisnotquiescent:cpu0",
        "run_fp_load_action(run,&path,\"--host-switch-cpu-mode\",&switch,&artifact)",
        "direct_fp_load_boundary_runs().into_iter().chain(live_fp_load_boundary_runs())",
    ] {
        assert!(
            retained.contains(marker),
            "retained rejection is missing `{marker}`"
        );
    }
}

fn compute_cli_contract(source: &str) -> bool {
    let common = [
        "assert_exact_replayed_timing(restored,baseline)",
        "assert_exact_replay_order(restored,baseline.schedule.captured_sequences)",
        "assert_exactly_once_stats(restored,&baseline.json)",
        "json.pointer(\"/parallel/scheduler/max_workers\")",
        "\"/cores/0/o3_runtime/issue\",\"/cores/0/o3_runtime/writeback_port\"",
        "restored.pointer(pointer),baseline.pointer(pointer)",
    ];
    let specific: [(&str, &[&str]); 3] = [
        (
            COMPUTE_ANCHORS[0],
            &[
                "LiveComputeScheduler::Serial",
                "assert_timing_control(&timing,&baseline.schedule)",
            ],
        ),
        (
            COMPUTE_ANCHORS[1],
            &[
                "LiveComputeScheduler::Parallel",
                "assert_detailed_runs_agree(&serial,&parallel)",
            ],
        ),
        (
            COMPUTE_ANCHORS[2],
            &[
                "run_live_compute_restore_discriminator(&path,&baseline.schedule,false)",
                "run_live_compute_restore_discriminator(&path,&baseline.schedule,true)",
            ],
        ),
    ];
    specific.into_iter().all(|(anchor, markers)| {
        enabled_anchor_has_markers(source, anchor, common.iter().chain(markers).copied())
    })
}

fn fp_cli_contract(source: &str) -> bool {
    let common = [
        "assert_live_fp_restore_discriminator(&no_restore,&restored,run)",
        "assert_restored_fp_pipeline(&restored,&baseline,run,restore_tick)",
        "assert_live_fp_final_state(&restored,&baseline,run)",
        ".filter(|record|event_u64(record,\"tick\")>=restore_tick).count(),0",
        "\"/cores/0/o3_runtime/issue\",\"/cores/0/o3_runtime/writeback_port\"",
        "restored.pointer(pointer),baseline.pointer(pointer)",
    ];
    let specific: [(&str, &str); 3] = [
        (
            FP_RESULT_ANCHORS[0],
            "FpLoadForwardingRun::width_one_flw_direct()",
        ),
        (
            FP_RESULT_ANCHORS[1],
            "FpLoadForwardingRun::width_two_fld_direct()",
        ),
        (
            FP_RESULT_ANCHORS[2],
            "FpLoadForwardingRun::width_four_hierarchy(precision)",
        ),
    ];
    specific.into_iter().all(|(anchor, marker)| {
        enabled_anchor_has_markers(source, anchor, common.iter().copied().chain([marker]))
    })
}

#[test]
fn o3_live_checkpoint_contract_ignores_disabled_decoys_and_noop_anchors() {
    let source = r#"
        #[cfg(any())]
        fn proof() { disabled_marker(); }
        fn proof() { enabled_marker(); }
        #[test]
        fn anchored() { proof(); }
        #[test]
        fn noop() {}
    "#;
    let anchored = reachable_enabled_functions(source, "anchored").unwrap();
    assert!(anchored.contains("enabled_marker"));
    assert!(!anchored.contains("disabled_marker"));
    assert!(!reachable_enabled_functions(source, "noop")
        .unwrap()
        .contains("enabled_marker"));

    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compute = read(&rem6.join(COMPUTE));
    for anchor in COMPUTE_ANCHORS {
        let mutated = noop_enabled_function(&compute, anchor).unwrap();
        assert!(!compute_cli_contract(&mutated));
    }
    let fp = read(&rem6.join(FP_RESULT));
    for anchor in FP_RESULT_ANCHORS {
        let mutated = noop_enabled_function(&fp, anchor).unwrap();
        assert!(!fp_cli_contract(&mutated));
    }
}

#[test]
fn o3_live_checkpoint_ledger_claim_is_bounded_and_score_neutral() {
    let repo = repo_root();
    let ledger_path = repo.join(LEDGER);
    let ledger = read(&ledger_path);
    assert_eq!(line_count(&ledger_path), 1200);
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

    for claim in [
        "checkpoint-restorable compute IQ window, exactly one response-admitted scalar FLW/FLD result, exactly one post-publication, committed-producer, unmaterialized dependent `SD`, and an exact capacity-three post-publication addressless scalar-load graph across sibling, chain, and mixed-fanout topologies",
        "Pre-response producer transport, materialized or submitted pending-address rows, dependent atomics, translated/MMIO pending-address rows, nonadjacent or fourth-and-deeper pending-address graphs, broader memory/result state, broad O3 restoration, restorable live transport ownership, and a general O3 engine remain non-restorable.",
    ] {
        assert!(cpu.contains(claim), "CPU ledger is missing `{claim}`");
    }
    for anchor in COMPUTE_ANCHORS
        .into_iter()
        .chain(FP_RESULT_ANCHORS)
        .chain(BOUNDARY_ANCHORS)
        .chain(RETAINED_REJECTION_ANCHORS)
    {
        assert!(cpu.contains(anchor), "CPU ledger is missing `{anchor}`");
    }
    assert!(stats.contains("exact live O3 decision/writeback replay"));

    let normalized = normalized_policy_text(&ledger);
    for overclaim in [
        "checkpoint restorable pre response transport",
        "general checkpoint restorable o3 iq",
        "o3dh carries live checkpoint transport",
    ] {
        assert!(
            !normalized.contains(overclaim),
            "ledger overclaims `{overclaim}`"
        );
    }
}

fn unconditional_module_attachment_count(source: &str, module: &str, path: &str) -> usize {
    let syntax = syn::parse_file(source).expect("Rust owner parses");
    if conditional(&syntax.attrs) {
        return 0;
    }
    syntax
        .items
        .iter()
        .filter(|item| {
            let syn::Item::Mod(item) = item else {
                return false;
            };
            item.ident == module
                && item.content.is_none()
                && !conditional(&item.attrs)
                && item.attrs.iter().any(|attribute| {
                    let syn::Meta::NameValue(value) = &attribute.meta else {
                        return false;
                    };
                    let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(value),
                        ..
                    }) = &value.value
                    else {
                        return false;
                    };
                    attribute.path().is_ident("path") && value.value() == path
                })
        })
        .count()
}

fn enabled_top_level_tests(relative: &str, source: &str) -> Vec<String> {
    let syntax = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"));
    if conditional(&syntax.attrs) {
        return Vec::new();
    }
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            (function
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("test"))
                && !function
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("ignore"))
                && !conditional(&function.attrs))
            .then(|| function.sig.ident.to_string())
        })
        .collect()
}

fn workspace_definition_owners(repo: &Path) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut owners = std::collections::BTreeMap::<String, Vec<String>>::new();
    for path in rust_source_files(&repo.join("crates")) {
        let relative = path
            .strip_prefix(repo)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        for name in parsed_function_names(&read(&path)) {
            owners.entry(name).or_default().push(relative.clone());
        }
    }
    owners
}

fn parsed_function_names(source: &str) -> Vec<String> {
    fn collect(items: &[syn::Item], names: &mut Vec<String>) {
        for item in items {
            match item {
                syn::Item::Fn(function) => names.push(function.sig.ident.to_string()),
                syn::Item::Mod(module) => {
                    if let Some((_, items)) = &module.content {
                        collect(items, names);
                    }
                }
                _ => {}
            }
        }
    }
    let syntax = syn::parse_file(source).expect("Rust source parses");
    let mut names = Vec::new();
    collect(&syntax.items, &mut names);
    names
}

fn registered_anchor_count(anchor: &str) -> usize {
    CORE_TEST_ANCHORS
        .lines()
        .filter(|line| line == &anchor)
        .count()
}

fn component_section<'a>(ledger: &'a str, heading: &str) -> &'a str {
    ledger
        .split_once(heading)
        .unwrap_or_else(|| panic!("missing component heading `{heading}`"))
        .1
        .split("\n### ")
        .next()
        .unwrap()
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

fn conditional(attributes: &[syn::Attribute]) -> bool {
    attributes
        .iter()
        .any(|attribute| attribute.path().is_ident("cfg") || attribute.path().is_ident("cfg_attr"))
}

fn compact_rust(source: &str) -> String {
    source
        .parse::<proc_macro2::TokenStream>()
        .expect("policy input must be valid Rust tokens")
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
