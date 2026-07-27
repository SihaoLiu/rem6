use super::*;

const POLICY: &str = "tests/source_policy/o3_fp_load_forwarding_ownership.rs";
const PARENT: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq.rs";
const FIXTURE: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_fixture.rs";
const POSITIVE: &str = "tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding.rs";
const BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_boundaries.rs";
const RUNTIME_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_runtime_boundaries.rs";
const COMPATIBILITY: &str =
    "tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_compatibility.rs";
const LEDGER: &str = "docs/architecture/gem5-to-rem6-migration.md";
const POSITIVE_OWNER: &str =
    "crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding.rs";
const BOUNDARY_OWNER: &str =
    "crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_boundaries.rs";
const RUNTIME_OWNER: &str = "crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq/fp_load_forwarding_runtime_boundaries.rs";
const POSITIVE_ANCHORS: [&str; 3] = [
    "rem6_run_o3_fp_load_forwarding_width_one_flw_direct",
    "rem6_run_o3_fp_load_forwarding_width_two_fld_direct",
    "rem6_run_o3_fp_load_forwarding_width_four_precision_matrix_hierarchy",
];
const BOUNDARY_ANCHORS: [&str; 4] = [
    "rem6_run_o3_fp_load_forwarding_denied_load_cleans_dependency",
    "rem6_run_o3_fp_load_forwarding_class_mismatch_uses_normal_execution",
    "rem6_run_o3_fp_load_forwarding_unsupported_fp_shapes_use_normal_execution",
    "rem6_run_o3_fp_load_forwarding_vector_load_boundary_uses_normal_execution",
];
const RUNTIME_ANCHORS: [&str; 4] = [
    "rem6_run_o3_fp_load_forwarding_checkpoint_boundaries",
    "rem6_run_o3_fp_load_forwarding_handoff_rejects_live_state",
    "rem6_run_o3_fp_load_forwarding_drained_restore",
    "rem6_run_timing_suppresses_o3_fp_load_forwarding",
];

#[test]
fn o3_fp_load_forwarding_owners_are_unconditional_unique_and_bounded() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = repo_root();
    let workspace_definitions = workspace_definition_owners(&repo);
    let policy_parent = read(&rem6.join("tests/source_policy.rs"));
    let policy = read(&rem6.join(POLICY));
    assert_eq!(
        unconditional_module_attachment_count(
            &policy_parent,
            "o3_fp_load_forwarding_ownership",
            "source_policy/o3_fp_load_forwarding_ownership.rs",
        ),
        1,
    );
    assert!(policy.lines().count() <= 550);

    let parent = read(&rem6.join(PARENT));
    let children = [
        (
            "fp_load_forwarding_fixture",
            "persistent_iq/fp_load_forwarding_fixture.rs",
            FIXTURE,
            525,
        ),
        (
            "fp_load_forwarding",
            "persistent_iq/fp_load_forwarding.rs",
            POSITIVE,
            850,
        ),
        (
            "fp_load_forwarding_boundaries",
            "persistent_iq/fp_load_forwarding_boundaries.rs",
            BOUNDARIES,
            625,
        ),
        (
            "fp_load_forwarding_runtime_boundaries",
            "persistent_iq/fp_load_forwarding_runtime_boundaries.rs",
            RUNTIME_BOUNDARIES,
            675,
        ),
        (
            "fp_load_forwarding_compatibility",
            "persistent_iq/fp_load_forwarding_compatibility.rs",
            COMPATIBILITY,
            125,
        ),
    ];
    for (module, path, owner, maximum) in children {
        assert_eq!(
            unconditional_module_attachment_count(&parent, module, path),
            1,
            "{PARENT} must attach {module} unconditionally",
        );
        let child = read(&rem6.join(owner));
        assert!(
            child.lines().count() <= maximum,
            "{owner} exceeds its {maximum}-line cap",
        );
        assert!(!child.contains("include!("), "{owner} must be a real owner");

        let attachment = format!("#[path = \"{path}\"]\nmod {module};");
        for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
            let mutated = parent.replacen(&attachment, &format!("{conditional}{attachment}"), 1);
            assert_ne!(
                mutated, parent,
                "attachment mutation must apply for {module}"
            );
            assert_eq!(
                unconditional_module_attachment_count(&mutated, module, path),
                0,
            );
        }
    }

    let owners = [
        (POSITIVE, POSITIVE_OWNER, POSITIVE_ANCHORS.as_slice()),
        (BOUNDARIES, BOUNDARY_OWNER, BOUNDARY_ANCHORS.as_slice()),
        (
            RUNTIME_BOUNDARIES,
            RUNTIME_OWNER,
            RUNTIME_ANCHORS.as_slice(),
        ),
    ];
    for (relative, expected_owner, anchors) in owners {
        let source = read(&rem6.join(relative));
        let enabled = enabled_top_level_tests(relative, &source);
        assert_eq!(
            enabled.len(),
            anchors.len(),
            "unexpected test inventory in {relative}"
        );
        for anchor in anchors {
            assert_eq!(enabled.iter().filter(|name| name == anchor).count(), 1);
            assert_eq!(
                workspace_definitions
                    .get(*anchor)
                    .cloned()
                    .unwrap_or_default(),
                vec![expected_owner.to_string()],
                "{anchor} must have one workspace test owner",
            );
            assert_eq!(
                CORE_TEST_ANCHORS
                    .lines()
                    .filter(|line| line == anchor)
                    .count(),
                1,
                "{anchor} must be registered exactly once",
            );
        }
    }
}

#[test]
fn o3_fp_load_forwarding_matrix_is_exact_and_mutation_resistant() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = read(&rem6.join(FIXTURE));
    let positive = read(&rem6.join(POSITIVE));
    assert!(matrix_contract(&fixture, &positive));

    let wrong_width = fixture.replacen("issue_width: 4,", "issue_width: 3,", 1);
    assert_ne!(wrong_width, fixture, "width mutation must apply");
    assert!(!matrix_contract(&wrong_width, &positive));

    let runtime_width_assertion = [
        "issue.pointer(\"/configured",
        "width\").and_then(Value::as_u64),\n        Some(4),",
    ]
    .join("_");
    let wrong_runtime_width = positive.replacen(
        &runtime_width_assertion,
        &runtime_width_assertion.replace("Some(4)", "Some(3)"),
        1,
    );
    assert_ne!(
        wrong_runtime_width, positive,
        "runtime-width mutation must apply"
    );
    assert!(!matrix_contract(&fixture, &wrong_runtime_width));

    let weak_hierarchy_counter = positive.replacen(
        ".is_some_and(|activity| activity > 0)",
        ".is_some_and(|_| true)",
        1,
    );
    assert_ne!(
        weak_hierarchy_counter, positive,
        "counter mutation must apply"
    );
    assert!(!matrix_contract(&fixture, &weak_hierarchy_counter));

    let wrong_single = fixture.replacen("\"00002041\"", "\"00001041\"", 1);
    assert_ne!(wrong_single, fixture, "single-result mutation must apply");
    assert!(!matrix_contract(&wrong_single, &positive));

    let wrong_double = fixture.replacen("\"0000000000002440\"", "\"0000000000001040\"", 1);
    assert_ne!(wrong_double, fixture, "double-result mutation must apply");
    assert!(!matrix_contract(&wrong_double, &positive));
}

#[test]
fn o3_fp_load_forwarding_boundaries_lock_cleanup_compatibility_and_timing() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let boundaries = read(&rem6.join(BOUNDARIES));
    let runtime = read(&rem6.join(RUNTIME_BOUNDARIES));
    let compatibility = read(&rem6.join(COMPATIBILITY));
    assert!(boundary_contract(&boundaries, &runtime, &compatibility));

    let relaxed_o3ps =
        compatibility.replacen("assert_eq!(encoded[4], 2,", "assert!(encoded[4] >= 2,", 1);
    assert_ne!(relaxed_o3ps, compatibility, "O3PS relaxation must apply");
    assert!(!boundary_contract(&boundaries, &runtime, &relaxed_o3ps));

    let relaxed_o3rt = runtime.replacen(
        "(\"checkpoint_version\", 23)",
        "(\"checkpoint_version\", 22)",
        1,
    );
    assert_ne!(relaxed_o3rt, runtime, "O3RT mutation must apply");
    assert!(!boundary_contract(
        &boundaries,
        &relaxed_o3rt,
        &compatibility,
    ));

    let relaxed_o3dh =
        compatibility.replacen("(\"schema_version\", 7)", "(\"schema_version\", 6)", 1);
    assert_ne!(relaxed_o3dh, compatibility, "O3DH mutation must apply");
    assert!(!boundary_contract(&boundaries, &runtime, &relaxed_o3dh));
}

#[test]
fn o3_fp_load_forwarding_ledger_claim_is_bounded_and_score_neutral() {
    let repo = repo_root();
    let ledger_path = repo.join(LEDGER);
    let ledger = read(&ledger_path);
    assert_eq!(line_count(&ledger_path), 1200);
    let cpu = component_section(&ledger, "### CPU Execution Models - 74% representative");
    assert!(cpu.contains(
        "**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap."
    ));
    for claim in [
        "bounded scalar FLW/FLD completion feeds supported S/D arithmetic through the persistent live queue",
        "issue widths 1, 2, and 4 across direct and cache/fabric/DRAM routes",
        "exact FLW `00002041` and FLD `0000000000002440` result bytes",
    ] {
        assert!(cpu.contains(claim), "CPU ledger missing `{claim}`");
    }
    for anchor in POSITIVE_ANCHORS
        .into_iter()
        .chain(BOUNDARY_ANCHORS)
        .chain(RUNTIME_ANCHORS)
    {
        assert!(cpu.contains(anchor), "CPU ledger missing `{anchor}`");
    }
    assert!(cpu.contains(
        "broader FP load shapes, conversions, comparisons, moves, classification, dynamic-CSR/status-sensitive chains, true vector-register/load/VCSR forwarding, arbitrary dependency graphs, positive system issue rows, a general load/store queue scheduler, dependent stores or arbitrary atomics, checkpoint-restorable live IQ/transport state, and a general O3 engine remain incomplete"
    ));
    let normalized = normalized_policy_text(cpu);
    for overclaim in [
        "general fp load forwarding",
        "vector load forwarding complete",
        "checkpoint restorable live fp issue queue",
    ] {
        assert!(
            !normalized.contains(overclaim),
            "ledger overclaims `{overclaim}`"
        );
    }
}

fn matrix_contract(fixture: &str, positive: &str) -> bool {
    let fixture = compact(fixture);
    let positive = compact(positive);
    let configured_width_pointer = ["issue.pointer(\"/configured", "width\")"].join("_");
    let configured_memory_width_pointer =
        [".pointer(\"/configured", "memory", "width\")"].join("_");
    fixture.contains("pub(super)constFP_LOAD_RESULT_HEX:&str=\"00002041\";")
        && fixture.contains("pub(super)constFP_LOAD_D_RESULT_HEX:&str=\"0000000000002440\";")
        && fixture.contains("pub(super)constfnwidth_one_flw_direct()->Self{Self{precision:FpLoadPrecision::Single,issue_width:1,writeback_width:1,memory_system:\"direct\",switch_mode:\"detailed\",}}")
        && fixture.contains("pub(super)constfnwidth_two_fld_direct()->Self{Self{precision:FpLoadPrecision::Double,issue_width:2,writeback_width:2,memory_system:\"direct\",switch_mode:\"detailed\",}}")
        && fixture.contains("pub(super)constfnwidth_four_hierarchy(precision:FpLoadPrecision)->Self{Self{precision,issue_width:4,writeback_width:1,memory_system:\"cache-fabric-dram\",switch_mode:\"detailed\",}}")
        && fixture.contains("fnmemory_issue_width(self)->usize{1}")
        && fixture.matches("\"--riscv-o3-issue-width\"").count() == 1
        && fixture.matches("\"--riscv-o3-memory-issue-width\"").count() == 1
        && fixture.matches("\"--riscv-o3-writeback-width\"").count() == 1
        && positive.matches("FpLoadForwardingRun::width_one_flw_direct()").count() == 1
        && positive.matches("FpLoadForwardingRun::width_two_fld_direct()").count() == 1
        && positive.matches("FpLoadForwardingRun::width_four_hierarchy(precision)").count() == 1
        && positive.contains("forprecisionin[FpLoadPrecision::Single,FpLoadPrecision::Double]")
        && positive.contains("Some(precision.result_hex())")
        && positive.contains(&format!("{configured_width_pointer}.and_then(Value::as_u64),Some(4)"))
        && positive.contains(&format!("{configured_memory_width_pointer}.and_then(Value::as_u64),Some(1)"))
        && positive.contains("writeback.pointer(\"/deferred_rows\").and_then(Value::as_u64),Some(1)")
        && positive.contains("/memory_resources/cache/data/activity")
        && positive.contains("/memory_resources/transport/data/activity")
        && positive.contains("/memory_resources/fabric/activity")
        && positive.contains("/memory_resources/dram/activity")
        && positive.contains("json.pointer(pointer).and_then(Value::as_u64).is_some_and(|activity|activity>0)")
        && positive.contains("/issued_by_class/scalar_float")
}

fn boundary_contract(boundaries: &str, runtime: &str, compatibility: &str) -> bool {
    let boundaries = compact(boundaries);
    let runtime = compact(runtime);
    let compatibility = compact(compatibility);
    boundaries.contains("rem6.cli.riscv_data_pmp_failure.v1")
        && boundaries.contains("completed_cpu_data_events")
        && boundaries.contains("writeback_reservations")
        && boundaries.contains("assert_no_queue_lifecycle_at")
        && boundaries.contains("unsupportedscalarFPforms")
        && boundaries.contains("vectorload")
        && runtime.contains("(\"checkpoint_version\",23)")
        && runtime.contains(
            "failedtoexecuterun:hostactionfailed:checkpointcomponentisnotquiescent:cpu0\\n",
        )
        && runtime.contains("timing_json.pointer(\"/cores/0/o3_runtime\").is_none()")
        && runtime.contains("assert_no_fp_load_o3_stats(&timing_json,timing)")
        && compatibility.contains("assert_eq!(&encoded[..4],b\"O3PS\")")
        && compatibility.contains("assert_eq!(encoded[4],2,\"currentO3PScompatibilityversion\")")
        && compatibility.contains("(\"schema_version\",7)")
        && compatibility.contains("Some(\"load\")")
        && compatibility.contains("Some(\"0x800000c0\")")
}

fn unconditional_module_attachment_count(source: &str, module: &str, path: &str) -> usize {
    let syntax = syn::parse_file(source).expect("source-policy module parses");
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
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            let is_test = function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("test"));
            let ignored = function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("ignore"));
            (is_test && !ignored && !conditional(&function.attrs))
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

fn compact(source: &str) -> String {
    source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
