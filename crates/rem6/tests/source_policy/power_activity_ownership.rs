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
fn normal_run_power_builder_uses_only_canonical_memory_resources() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let power_source = fs::read_to_string(crate_dir.join(POWER_OUTPUT)).unwrap();
    let power_syntax = syn::parse_file(&power_source).unwrap();
    let builder = function_named(&power_syntax, "run_power_analysis_records_from_parts");
    let parameter_names = builder
        .sig
        .inputs
        .iter()
        .map(function_parameter_name)
        .collect::<Vec<_>>();

    assert_eq!(
        parameter_names,
        vec!["final_tick", "cores", "memory_resources"],
        "normal-run power assembly must accept only tick, cores, and canonical memory resources"
    );
    assert!(
        builder
            .sig
            .inputs
            .iter()
            .any(|input| function_parameter_type_mentions(input, "Rem6MemoryResourceSummary")),
        "normal-run power assembly must receive Rem6MemoryResourceSummary"
    );
    for forbidden in ["CliDataCacheSummary", "Rem6DramSummary"] {
        assert!(
            builder
                .sig
                .inputs
                .iter()
                .all(|input| !function_parameter_type_mentions(input, forbidden)),
            "normal-run power assembly must not receive raw {forbidden} inputs"
        );
    }

    let summary_source = fs::read_to_string(crate_dir.join(RUN_EXECUTION_SUMMARY)).unwrap();
    let summary_syntax = syn::parse_file(&summary_source).unwrap();
    let mut calls = NamedCallVisitor::new("run_power_analysis_records_from_parts");
    calls.visit_file(&summary_syntax);
    assert_eq!(
        calls.calls.len(),
        1,
        "{RUN_EXECUTION_SUMMARY} must contain exactly one normal-run power builder call"
    );
    let call = calls.calls[0];
    assert_eq!(
        call.args.len(),
        3,
        "run execution summary must pass only final_tick, cores, and memory_resources"
    );
    assert!(expression_is_ident(&call.args[0], "final_tick"));
    assert!(expression_is_reference_to_ident(&call.args[1], "cores"));
    assert!(expression_is_reference_to_ident(
        &call.args[2],
        "memory_resources"
    ));
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

fn function_parameter_name(input: &syn::FnArg) -> String {
    let syn::FnArg::Typed(parameter) = input else {
        panic!("normal-run power builder must not have a receiver");
    };
    let syn::Pat::Ident(ident) = parameter.pat.as_ref() else {
        panic!("normal-run power builder parameters must use identifier patterns");
    };
    ident.ident.to_string()
}

fn function_parameter_type_mentions(input: &syn::FnArg, expected: &str) -> bool {
    let syn::FnArg::Typed(parameter) = input else {
        return false;
    };
    type_mentions(parameter.ty.as_ref(), expected)
}

fn type_mentions(ty: &syn::Type, expected: &str) -> bool {
    match ty {
        syn::Type::Group(group) => type_mentions(&group.elem, expected),
        syn::Type::Paren(paren) => type_mentions(&paren.elem, expected),
        syn::Type::Path(path) => path
            .path
            .segments
            .iter()
            .any(|segment| segment.ident == expected),
        syn::Type::Reference(reference) => type_mentions(&reference.elem, expected),
        syn::Type::Slice(slice) => type_mentions(&slice.elem, expected),
        syn::Type::Tuple(tuple) => tuple
            .elems
            .iter()
            .any(|element| type_mentions(element, expected)),
        _ => false,
    }
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

fn expression_is_ident(expression: &syn::Expr, expected: &str) -> bool {
    matches!(
        expression,
        syn::Expr::Path(path) if path.qself.is_none() && path.path.is_ident(expected)
    )
}

fn expression_is_reference_to_ident(expression: &syn::Expr, expected: &str) -> bool {
    matches!(
        expression,
        syn::Expr::Reference(reference) if expression_is_ident(&reference.expr, expected)
    )
}

fn expression_mentions_ident(expression: &syn::Expr, expected: &str) -> bool {
    struct IdentifierVisitor<'a> {
        expected: &'a str,
        found: bool,
    }

    impl<'ast> Visit<'ast> for IdentifierVisitor<'_> {
        fn visit_ident(&mut self, ident: &'ast syn::Ident) {
            self.found |= ident == self.expected;
        }
    }

    let mut visitor = IdentifierVisitor {
        expected,
        found: false,
    };
    visitor.visit_expr(expression);
    visitor.found
}
