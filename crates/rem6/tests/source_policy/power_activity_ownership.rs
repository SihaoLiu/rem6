use super::*;
use syn::visit::Visit;

const CLI_RUN_DRIVER: &str = "tests/cli_run.rs";
const CLI_RUN_MODULES: &str = "tests/cli_run";
const LOAD_ROOT: &str = "tests/cli_run/load.rs";
const POWER_MATRIX: &str = "tests/cli_run/load/power_activity_matrix.rs";
const POWER_OUTPUT: &str = "src/power_output.rs";
const RUN_EXECUTION_SUMMARY: &str = "src/run_execution_summary.rs";
const POWER_ACTIVITY_TESTS: [&str; 6] = [
    "rem6_run_power_analysis_includes_dram_activity",
    "rem6_run_power_analysis_includes_cache_activity",
    "rem6_run_power_analysis_includes_shared_cache_activity",
    "rem6_run_power_analysis_includes_fabric_activity",
    "rem6_run_power_analysis_includes_transport_activity",
    "rem6_run_power_activity_matches_canonical_resource_matrix",
];

#[test]
fn run_power_activity_matrix_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join(LOAD_ROOT)).unwrap();
    let inventory = cli_run_test_function_inventory(crate_dir);

    assert!(
        module_has_path_attribute(
            &root,
            "power_activity_matrix",
            "load/power_activity_matrix.rs"
        ),
        "{LOAD_ROOT} must declare the focused power activity matrix module"
    );
    for test in POWER_ACTIVITY_TESTS {
        let owners = test_owners(&inventory, test);
        assert_eq!(
            owners,
            vec![POWER_MATRIX],
            "`fn {test}` must have exactly one CLI test owner: {POWER_MATRIX}"
        );
    }
}

#[test]
fn run_power_activity_owner_inventory_counts_only_test_functions() {
    let test = "rem6_run_power_activity_matches_canonical_resource_matrix";
    let non_owner = format!(
        r#"
            // fn {test}() {{}}
            const FAKE_TEST: &str = "fn {test}() {{}}";
            fn {test}() {{}}
        "#
    );
    let owner = format!("#[test]\nfn {test}() {{}}\n");
    let inventory = vec![
        (
            LOAD_ROOT.to_string(),
            rust_test_function_definition_names(LOAD_ROOT, &non_owner),
        ),
        (
            POWER_MATRIX.to_string(),
            rust_test_function_definition_names(POWER_MATRIX, &owner),
        ),
    ];

    assert_eq!(test_owners(&inventory, test), vec![POWER_MATRIX]);
}

#[test]
fn gem5_stats_score_ratchet_tracks_canonical_power_evidence() {
    let migration =
        fs::read_to_string(repo_root().join("docs/architecture/gem5-to-rem6-migration.md"))
            .unwrap();
    let table = markdown_section(&migration, "## Test Migration Ledger")
        .expect("missing test migration ledger");
    let cells = markdown_table_rows(table)
        .into_iter()
        .find(|cells| {
            cells
                .first()
                .is_some_and(|cell| *cell == "`tests/gem5/stats`")
        })
        .expect("missing tests/gem5/stats migration row");

    assert_eq!(cells.len(), 5, "tests/gem5/stats row must have five cells");
    assert_eq!(cells[2], "74% representative");
    assert!(
        cells[3].contains("`rem6_run_power_activity_matches_canonical_resource_matrix`"),
        "tests/gem5/stats evidence must retain the canonical power matrix test anchor"
    );
    assert!(
        cells[3].contains("canonical actual-byte DRAM dynamic watts"),
        "tests/gem5/stats evidence must retain canonical actual-byte DRAM power calibration"
    );
    assert!(
        cells[4].contains("physical fabrication/vendor coefficient calibration"),
        "tests/gem5/stats next evidence must retain the physical coefficient calibration gap"
    );
}

#[test]
fn normal_run_power_builder_uses_only_canonical_memory_resources() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let power_source = fs::read_to_string(crate_dir.join(POWER_OUTPUT)).unwrap();
    let power_syntax = syn::parse_file(&power_source).unwrap();
    let builder = function_named(&power_syntax, "run_power_analysis_records_from_parts");
    assert_normal_run_power_builder_signature(builder);

    let summary_source = fs::read_to_string(crate_dir.join(RUN_EXECUTION_SUMMARY)).unwrap();
    let summary_syntax = syn::parse_file(&summary_source).unwrap();
    let mut calls = NamedCallVisitor::new("run_power_analysis_records_from_parts");
    calls.visit_file(&summary_syntax);
    assert_eq!(
        calls.calls.len(),
        1,
        "{RUN_EXECUTION_SUMMARY} must contain exactly one normal-run power builder call"
    );
    assert_normal_run_power_builder_call(calls.calls[0]);
}

#[test]
fn normal_run_power_policy_allows_harmless_binding_and_call_renames() {
    let power_syntax = syn::parse_file(
        r#"
            fn run_power_analysis_records_from_parts(
                tick_count: u64,
                cpu_summaries: &[crate::Rem6CoreSummary],
                resources: &crate::Rem6MemoryResourceSummary,
            ) {}
        "#,
    )
    .unwrap();
    assert_normal_run_power_builder_signature(function_named(
        &power_syntax,
        "run_power_analysis_records_from_parts",
    ));

    let call_syntax = syn::parse_file(
        r#"
            fn build(
                tick_count: u64,
                cpu_summaries: &[crate::Rem6CoreSummary],
                summary: &RunSummary,
            ) {
                run_power_analysis_records_from_parts(
                    tick_count,
                    cpu_summaries,
                    &summary.canonical_memory,
                );
            }
        "#,
    )
    .unwrap();
    let mut calls = NamedCallVisitor::new("run_power_analysis_records_from_parts");
    calls.visit_file(&call_syntax);
    assert_eq!(
        calls.calls.len(),
        1,
        "renamed fixture must contain one normal-run power builder call"
    );
    assert_normal_run_power_builder_call(calls.calls[0]);
}

#[test]
fn normal_run_power_policy_type_scan_handles_nested_generics() {
    let ty = syn::parse_str::<syn::Type>("&Option<Vec<CliDataCacheSummary>>").unwrap();

    assert!(type_mentions(&ty, "CliDataCacheSummary"));
}

fn assert_normal_run_power_builder_signature(builder: &syn::ItemFn) {
    assert_eq!(
        builder.sig.inputs.len(),
        3,
        "normal-run power assembly must accept exactly three typed parameters"
    );
    let parameter_types = builder
        .sig
        .inputs
        .iter()
        .map(|input| {
            let syn::FnArg::Typed(parameter) = input else {
                panic!("normal-run power assembly parameters must all be typed");
            };
            parameter.ty.as_ref()
        })
        .collect::<Vec<_>>();

    assert!(
        type_is_named_path(parameter_types[0], "u64"),
        "normal-run power assembly parameter 1 must be the final tick"
    );
    assert!(
        type_is_reference_to_slice_of(parameter_types[1], "Rem6CoreSummary"),
        "normal-run power assembly parameter 2 must reference a Rem6CoreSummary slice"
    );
    assert!(
        type_is_reference_to_named_path(parameter_types[2], "Rem6MemoryResourceSummary"),
        "normal-run power assembly parameter 3 must reference Rem6MemoryResourceSummary"
    );
    for forbidden in ["CliDataCacheSummary", "Rem6DramSummary"] {
        assert!(
            parameter_types
                .iter()
                .all(|ty| !type_mentions(ty, forbidden)),
            "normal-run power assembly must not receive raw {forbidden} inputs"
        );
    }
}

fn assert_normal_run_power_builder_call(call: &syn::ExprCall) {
    assert_eq!(
        call.args.len(),
        3,
        "run execution summary must pass only final_tick, cores, and memory_resources"
    );
    for forbidden in ["instruction_cache", "data_cache", "dram"] {
        assert!(
            call.args
                .iter()
                .all(|argument| !expression_mentions_ident(argument, forbidden)),
            "run execution summary must not pass raw {forbidden} activity to normal-run power assembly"
        );
    }
}

fn cli_run_test_function_inventory(crate_dir: &Path) -> Vec<(String, BTreeSet<String>)> {
    let mut paths = rust_source_files(&crate_dir.join(CLI_RUN_MODULES));
    paths.push(crate_dir.join(CLI_RUN_DRIVER));
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(crate_dir)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let source = fs::read_to_string(&path).unwrap();
            let test_functions = rust_test_function_definition_names(&relative, &source);
            (relative, test_functions)
        })
        .collect()
}

fn rust_test_function_definition_names(relative: &str, source: &str) -> BTreeSet<String> {
    let syntax = syn::parse_file(source).unwrap_or_else(|error| {
        panic!("failed to parse {relative} for run power activity test ownership: {error}")
    });
    syntax
        .items
        .into_iter()
        .filter_map(|item| {
            let syn::Item::Fn(function) = item else {
                return None;
            };
            function
                .attrs
                .iter()
                .any(|attribute| attribute.path().is_ident("test"))
                .then(|| function.sig.ident.to_string())
        })
        .collect()
}

fn test_owners<'a>(inventory: &'a [(String, BTreeSet<String>)], function: &str) -> Vec<&'a str> {
    inventory
        .iter()
        .filter_map(|(relative, functions)| {
            functions.contains(function).then_some(relative.as_str())
        })
        .collect()
}

fn function_named<'a>(syntax: &'a syn::File, name: &str) -> &'a syn::ItemFn {
    syntax
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function) if function.sig.ident == name => Some(function),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn transparent_type(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Group(group) => transparent_type(&group.elem),
        syn::Type::Paren(paren) => transparent_type(&paren.elem),
        _ => ty,
    }
}

fn type_is_named_path(ty: &syn::Type, expected: &str) -> bool {
    let syn::Type::Path(path) = transparent_type(ty) else {
        return false;
    };
    path.qself.is_none()
        && path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == expected)
}

fn type_is_reference_to_slice_of(ty: &syn::Type, expected: &str) -> bool {
    let syn::Type::Reference(reference) = transparent_type(ty) else {
        return false;
    };
    let syn::Type::Slice(slice) = transparent_type(&reference.elem) else {
        return false;
    };
    type_is_named_path(&slice.elem, expected)
}

fn type_is_reference_to_named_path(ty: &syn::Type, expected: &str) -> bool {
    let syn::Type::Reference(reference) = transparent_type(ty) else {
        return false;
    };
    type_is_named_path(&reference.elem, expected)
}

struct IdentifierVisitor<'a> {
    expected: &'a str,
    found: bool,
}

impl<'ast> Visit<'ast> for IdentifierVisitor<'_> {
    fn visit_ident(&mut self, ident: &'ast syn::Ident) {
        self.found |= ident == self.expected;
    }
}

fn type_mentions(ty: &syn::Type, expected: &str) -> bool {
    let mut visitor = IdentifierVisitor {
        expected,
        found: false,
    };
    visitor.visit_type(ty);
    visitor.found
}

struct NamedCallVisitor<'ast> {
    name: &'static str,
    calls: Vec<&'ast syn::ExprCall>,
}

impl<'ast> NamedCallVisitor<'ast> {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            calls: Vec::new(),
        }
    }
}

impl<'ast> Visit<'ast> for NamedCallVisitor<'ast> {
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(function) = call.func.as_ref() {
            if function
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == self.name)
            {
                self.calls.push(call);
            }
        }
        syn::visit::visit_expr_call(self, call);
    }
}

fn expression_mentions_ident(expression: &syn::Expr, expected: &str) -> bool {
    let mut visitor = IdentifierVisitor {
        expected,
        found: false,
    };
    visitor.visit_expr(expression);
    visitor.found
}
