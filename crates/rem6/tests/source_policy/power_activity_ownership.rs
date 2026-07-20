use super::*;

const CLI_RUN_DRIVER: &str = "tests/cli_run.rs";
const CLI_RUN_MODULES: &str = "tests/cli_run";
const LOAD_ROOT: &str = "tests/cli_run/load.rs";
const POWER_MATRIX: &str = "tests/cli_run/load/power_activity_matrix.rs";
const POWER_ACTIVITY_TESTS: [&str; 5] = [
    "rem6_run_power_analysis_includes_dram_activity",
    "rem6_run_power_analysis_includes_cache_activity",
    "rem6_run_power_analysis_includes_shared_cache_activity",
    "rem6_run_power_analysis_includes_fabric_activity",
    "rem6_run_power_analysis_includes_transport_activity",
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
    let test = POWER_ACTIVITY_TESTS[0];
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
