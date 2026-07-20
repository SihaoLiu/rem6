use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::Visit;
use syn::{Attribute, Expr, ImplItem, Item, Lit, Meta, Pat, Type, UseTree};

use super::{line_count, rust_source_files};

const MAX_CLI_CONFIG_LINES: usize = 500;
const MAX_CONFIG_ROOT_LINES: usize = 1700;

#[test]
fn core_cli_config_mechanics_have_one_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_path = crate_dir.join("src/lib.rs");
    let lib = fs::read_to_string(&lib_path).unwrap();
    let lib_syntax = parse_rust(&lib, &lib_path);
    let config_path = crate_dir.join("src/config.rs");
    let config = fs::read_to_string(&config_path).unwrap();
    let config_syntax = parse_rust(&config, &config_path);
    let parse_path = crate_dir.join("src/config/parse.rs");
    let parse = fs::read_to_string(&parse_path).unwrap();
    let parse_syntax = parse_rust(&parse, &parse_path);
    let cli_config_path = crate_dir.join("src/cli_config.rs");
    let cli_config = fs::read_to_string(&cli_config_path).unwrap();
    let cli_config_syntax = parse_rust(&cli_config, &cli_config_path);

    assert!(
        declares_module(&lib_syntax, "cli_config"),
        "src/lib.rs must declare the shared CLI config authority"
    );
    assert!(
        cli_config_path.is_file(),
        "core CLI config mechanics belong in src/cli_config.rs"
    );
    assert!(
        !crate_dir.join("src/config/file_scan.rs").exists(),
        "src/config/file_scan.rs must not remain as a second config-scan authority"
    );
    assert!(
        line_count(&cli_config_path) <= MAX_CLI_CONFIG_LINES,
        "src/cli_config.rs must stay focused"
    );
    assert!(
        line_count(&config_path) < MAX_CONFIG_ROOT_LINES,
        "src/config.rs must stay below {MAX_CONFIG_ROOT_LINES} lines"
    );

    let cli_config_functions = function_definition_names(&cli_config_syntax);
    for function in [
        "config_path_from_args",
        "run_file_config_from_args",
        "gups_file_config_from_args",
        "trace_replay_file_config_from_args",
        "read_toml_config",
        "required_value",
        "resolve_config_path",
    ] {
        assert!(
            cli_config_functions.contains(function),
            "src/cli_config.rs must own `{function}`"
        );
    }

    let config_functions = function_definition_names(&config_syntax);
    assert!(
        !config_functions.contains("resolve_config_path"),
        "src/config.rs must delegate path resolution to src/cli_config.rs"
    );
    let config_usage = syntax_usage(&config_syntax);
    assert!(!config_usage.read_config_error);
    assert!(!config_usage.parse_config_error);

    let parse_functions = function_definition_names(&parse_syntax);
    assert!(
        !parse_functions.contains("required_value"),
        "src/config/parse.rs must not define its own required_value helper"
    );
    assert!(
        cli_config_imports(&parse_syntax).contains("required_value"),
        "src/config/parse.rs must re-export crate::cli_config::required_value"
    );
}

#[test]
fn auxiliary_commands_consume_cli_config_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli_config_path = crate_dir.join("src/cli_config.rs");
    let authority = fs::read_to_string(&cli_config_path).unwrap();
    let authority_syntax = parse_rust(&authority, &cli_config_path);
    let authority_functions = function_definition_names(&authority_syntax);

    for wrapper in [
        "gpu_run_file_config_from_args",
        "accelerator_run_file_config_from_args",
        "multi_run_file_config_from_args",
        "resource_acquire_file_config_from_args",
    ] {
        assert!(
            authority_functions.contains(wrapper)
                && has_pub_crate_function(&authority_syntax, wrapper),
            "src/cli_config.rs must own `{wrapper}`"
        );
    }

    for (relative, wrapper) in [
        ("src/gpu_cli.rs", "gpu_run_file_config_from_args"),
        (
            "src/accelerator_cli.rs",
            "accelerator_run_file_config_from_args",
        ),
        ("src/multi_run_cli.rs", "multi_run_file_config_from_args"),
        (
            "src/resource_acquire_config.rs",
            "resource_acquire_file_config_from_args",
        ),
    ] {
        let path = crate_dir.join(relative);
        let source = fs::read_to_string(&path).unwrap();
        let syntax = parse_rust(&source, &path);
        let imports = cli_config_imports(&syntax);
        for import in [
            wrapper,
            "read_toml_config",
            "required_value",
            "resolve_config_path",
        ] {
            assert!(
                imports.contains(import),
                "{relative} must import `{import}` from crate::cli_config"
            );
        }
        let functions = function_definition_names(&syntax);
        for helper in [
            "config_path_from_args",
            "required_value",
            "resolve_config_path",
        ] {
            assert!(
                !functions.contains(helper),
                "{relative} must not define `{helper}`"
            );
        }
        let usage = syntax_usage(&syntax);
        assert!(
            !usage.read_config_error,
            "{relative} must not construct Rem6CliError::ReadConfig"
        );
        assert!(
            !usage.parse_config_error,
            "{relative} must not construct Rem6CliError::ParseConfig"
        );
        assert!(
            !usage.is_relative_call,
            "{relative} must delegate config path resolution"
        );
    }

    let power_import_path = crate_dir.join("src/power_import_cli.rs");
    let power_import = fs::read_to_string(&power_import_path).unwrap();
    let power_import_syntax = parse_rust(&power_import, &power_import_path);
    assert!(
        cli_config_imports(&power_import_syntax).contains("required_value"),
        "src/power_import_cli.rs must import required_value from crate::cli_config"
    );
    assert!(
        calls_function(&power_import_syntax, "required_value"),
        "src/power_import_cli.rs must call shared required_value"
    );
    assert!(
        !function_definition_names(&power_import_syntax).contains("required_value"),
        "src/power_import_cli.rs must not define required_value"
    );

    let cli_config_tests_path = crate_dir.join("src/cli_config/tests.rs");
    assert!(
        test_gated_external_module_paths(&cli_config_path, &authority_syntax)
            .contains(&cli_config_tests_path),
        "src/cli_config/tests.rs exclusion must derive from #[cfg(test)] mod tests"
    );
    let production_sources = production_rust_source_files(&crate_dir.join("src"));
    assert!(
        !production_sources.contains(&cli_config_tests_path),
        "src/cli_config/tests.rs must not enter the production helper inventory"
    );
    let data_cache_runtime_path = crate_dir.join("src/data_cache_runtime.rs");
    let data_cache_runtime = fs::read_to_string(&data_cache_runtime_path).unwrap();
    let data_cache_runtime_syntax = parse_rust(&data_cache_runtime, &data_cache_runtime_path);
    let data_cache_runtime_tests_path = crate_dir.join("src/data_cache_runtime_tests.rs");
    assert!(
        test_gated_external_module_paths(&data_cache_runtime_path, &data_cache_runtime_syntax)
            .contains(&data_cache_runtime_tests_path),
        "src/data_cache_runtime_tests.rs exclusion must honor its #[path] declaration"
    );
    assert!(
        !production_sources.contains(&data_cache_runtime_tests_path),
        "src/data_cache_runtime_tests.rs must not enter the production helper inventory"
    );
    for path in production_sources {
        let source = fs::read_to_string(&path).unwrap();
        let syntax = parse_rust(&source, &path);
        let functions = function_definition_names(&syntax);
        for function in [
            "config_path_from_args",
            "required_value",
            "resolve_config_path",
        ] {
            if functions.contains(function) {
                assert_eq!(
                    path,
                    cli_config_path,
                    "{} must not define `{function}`",
                    path.strip_prefix(crate_dir).unwrap().display()
                );
            }
        }
    }
}

#[test]
fn auxiliary_explicit_profiles_match_command_parsers() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let authority = fs::read_to_string(crate_dir.join("src/cli_config.rs")).unwrap();

    for (relative, config_type, value_profile, bool_profile) in [
        (
            "src/gpu_cli.rs",
            "Rem6GpuRunConfig",
            "GPU_RUN_VALUE_FLAGS",
            Some("GPU_RUN_BOOL_FLAGS"),
        ),
        (
            "src/accelerator_cli.rs",
            "Rem6AcceleratorRunConfig",
            "ACCELERATOR_RUN_VALUE_FLAGS",
            None,
        ),
        (
            "src/multi_run_cli.rs",
            "Rem6MultiRunConfig",
            "MULTI_RUN_VALUE_FLAGS",
            Some("MULTI_RUN_BOOL_FLAGS"),
        ),
        (
            "src/resource_acquire_config.rs",
            "Rem6ResourceAcquireConfig",
            "RESOURCE_ACQUIRE_VALUE_FLAGS",
            None,
        ),
    ] {
        assert_explicit_profile_matches_parser(
            &authority,
            &fs::read_to_string(crate_dir.join(relative)).unwrap(),
            config_type,
            value_profile,
            bool_profile,
            relative,
        );
    }
}

#[test]
fn auxiliary_wrappers_bind_designated_explicit_profiles() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = crate_dir.join("src/cli_config.rs");
    let source = fs::read_to_string(&path).unwrap();
    let syntax = parse_rust(&source, &path);

    for (wrapper, value_flags, bool_flags) in [
        (
            "gpu_run_file_config_from_args",
            "GPU_RUN_VALUE_FLAGS",
            Some("GPU_RUN_BOOL_FLAGS"),
        ),
        (
            "accelerator_run_file_config_from_args",
            "ACCELERATOR_RUN_VALUE_FLAGS",
            None,
        ),
        (
            "multi_run_file_config_from_args",
            "MULTI_RUN_VALUE_FLAGS",
            Some("MULTI_RUN_BOOL_FLAGS"),
        ),
        (
            "resource_acquire_file_config_from_args",
            "RESOURCE_ACQUIRE_VALUE_FLAGS",
            None,
        ),
    ] {
        assert_eq!(
            explicit_wrapper_profile(&syntax, wrapper),
            ExplicitProfileReference {
                value_flags: value_flags.to_string(),
                bool_flags: bool_flags.map(str::to_string),
            },
            "{wrapper} must bind its designated explicit profile"
        );
    }
}

#[test]
fn synthetic_explicit_wrapper_profile_extraction_detects_swaps() {
    let syntax = syn::parse_file(
        r#"
        pub(crate) fn correct(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
            config_path_from_args(
                args,
                ConfigPrescanProfile::explicit(GPU_RUN_VALUE_FLAGS, GPU_RUN_BOOL_FLAGS),
            )
        }

        pub(crate) fn swapped(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
            config_path_from_args(
                args,
                ConfigPrescanProfile::explicit(ACCELERATOR_RUN_VALUE_FLAGS, &[]),
            )
        }
        "#,
    )
    .unwrap();
    let expected = ExplicitProfileReference {
        value_flags: "GPU_RUN_VALUE_FLAGS".to_string(),
        bool_flags: Some("GPU_RUN_BOOL_FLAGS".to_string()),
    };

    assert_eq!(explicit_wrapper_profile(&syntax, "correct"), expected);
    assert_ne!(explicit_wrapper_profile(&syntax, "swapped"), expected);
}

#[test]
fn synthetic_production_facts_exclude_test_gated_items_and_modules() {
    let syntax = syn::parse_file(
        r#"
        use crate::cli_config::required_value;

        fn production_helper() {
            required_value("--flag", None);
        }

        #[cfg(test)]
        use crate::cli_config::read_toml_config;

        #[cfg(any(test, feature = "synthetic"))]
        pub(crate) fn production_reachable() {
            let _ = Rem6CliError::ReadConfig { path, error };
            path.is_relative();
            reachable_call();
        }

        #[cfg(all(test, feature = "synthetic"))]
        pub(crate) fn required_value() {
            let _ = Rem6CliError::ParseConfig { path, error };
            hidden_call();
        }

        #[cfg(test)]
        mod tests {
            use crate::cli_config::resolve_config_path;

            fn duplicate_helper() {
                hidden_call();
            }
        }
        "#,
    )
    .unwrap();

    assert_eq!(
        function_definition_names(&syntax),
        BTreeSet::from([
            "production_helper".to_string(),
            "production_reachable".to_string(),
        ])
    );
    assert_eq!(
        cli_config_imports(&syntax),
        BTreeSet::from(["required_value".to_string()])
    );
    assert!(calls_function(&syntax, "required_value"));
    assert!(calls_function(&syntax, "reachable_call"));
    assert!(!calls_function(&syntax, "hidden_call"));
    assert!(!has_pub_crate_function(&syntax, "required_value"));
    assert!(has_pub_crate_function(&syntax, "production_reachable"));
    assert_eq!(
        syntax_usage(&syntax),
        SyntaxUsage {
            read_config_error: true,
            parse_config_error: false,
            is_relative_call: true,
        }
    );

    let owner = Path::new("/repo/src/cli_config.rs");
    let module_syntax = syn::parse_file(
        r#"
        #[cfg(test)]
        mod tests;

        #[cfg(test)]
        #[path = "custom_tests.rs"]
        mod custom_tests;
        "#,
    )
    .unwrap();
    assert_eq!(
        test_gated_external_module_paths(owner, &module_syntax),
        BTreeSet::from([
            Path::new("/repo/src/cli_config/tests.rs").to_path_buf(),
            Path::new("/repo/src/custom_tests.rs").to_path_buf(),
        ])
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExplicitProfileReference {
    value_flags: String,
    bool_flags: Option<String>,
}

fn explicit_wrapper_profile(syntax: &syn::File, wrapper_name: &str) -> ExplicitProfileReference {
    let wrappers = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function)
                if !is_test_gated(&function.attrs) && function.sig.ident == wrapper_name =>
            {
                Some(function)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        wrappers.len(),
        1,
        "must define exactly one production wrapper `{wrapper_name}`"
    );
    let returned = wrappers[0]
        .block
        .stmts
        .last()
        .and_then(|statement| match statement {
            syn::Stmt::Expr(expression, None) => Some(expression),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{wrapper_name} must return a tail expression"));
    let Expr::Call(config_call) = unwrap_expr(returned) else {
        panic!("{wrapper_name} must return config_path_from_args(...)");
    };
    assert!(
        expression_path_ends_with(&config_call.func, &["config_path_from_args"]),
        "{wrapper_name} must return config_path_from_args(...)"
    );
    assert_eq!(
        config_call.args.len(),
        2,
        "{wrapper_name} config_path_from_args call must have two arguments"
    );
    assert!(
        expression_is_ident(&config_call.args[0], "args"),
        "{wrapper_name} must scan its args parameter"
    );

    let Expr::Call(profile_call) = unwrap_expr(&config_call.args[1]) else {
        panic!("{wrapper_name} must use ConfigPrescanProfile::explicit(...)");
    };
    assert!(
        expression_path_ends_with(&profile_call.func, &["ConfigPrescanProfile", "explicit"]),
        "{wrapper_name} must use ConfigPrescanProfile::explicit(...)"
    );
    assert_eq!(
        profile_call.args.len(),
        2,
        "{wrapper_name} explicit profile must have value and boolean arguments"
    );

    ExplicitProfileReference {
        value_flags: profile_constant_name(&profile_call.args[0], wrapper_name, "value"),
        bool_flags: profile_bool_constant_name(&profile_call.args[1], wrapper_name),
    }
}

fn profile_constant_name(expression: &Expr, wrapper_name: &str, kind: &str) -> String {
    let Expr::Path(path) = unwrap_expr(expression) else {
        panic!("{wrapper_name} {kind} profile must reference a named constant");
    };
    assert!(
        path.qself.is_none() && path.path.segments.len() == 1,
        "{wrapper_name} {kind} profile must reference one named constant"
    );
    path.path.segments[0].ident.to_string()
}

fn profile_bool_constant_name(expression: &Expr, wrapper_name: &str) -> Option<String> {
    match unwrap_expr(expression) {
        Expr::Array(array) if array.elems.is_empty() => None,
        expression => Some(profile_constant_name(expression, wrapper_name, "boolean")),
    }
}

fn expression_path_ends_with(expression: &Expr, expected: &[&str]) -> bool {
    let Expr::Path(path) = unwrap_expr(expression) else {
        return false;
    };
    let actual = path
        .path
        .segments
        .iter()
        .rev()
        .take(expected.len())
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    actual.len() == expected.len()
        && actual
            .iter()
            .rev()
            .map(String::as_str)
            .eq(expected.iter().copied())
}

fn parse_rust(source: &str, path: &Path) -> syn::File {
    syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn is_test_gated(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("cfg"))
        .any(|attribute| {
            let meta = attribute
                .parse_args::<Meta>()
                .unwrap_or_else(|error| panic!("failed to parse cfg attribute: {error}"));
            cfg_requires_test(&meta)
        })
}

fn cfg_requires_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => path.is_ident("test"),
        Meta::List(list) if list.path.is_ident("not") => false,
        Meta::List(list) if list.path.is_ident("all") => {
            cfg_predicates(list).iter().any(cfg_requires_test)
        }
        Meta::List(list) if list.path.is_ident("any") => {
            let predicates = cfg_predicates(list);
            !predicates.is_empty() && predicates.iter().all(cfg_requires_test)
        }
        Meta::List(_) => false,
        Meta::NameValue(_) => false,
    }
}

fn cfg_predicates(list: &syn::MetaList) -> syn::punctuated::Punctuated<Meta, syn::Token![,]> {
    list.parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        .unwrap_or_else(|error| panic!("failed to parse nested cfg attribute: {error}"))
}

fn item_attributes(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(item) => &item.attrs,
        Item::Enum(item) => &item.attrs,
        Item::ExternCrate(item) => &item.attrs,
        Item::Fn(item) => &item.attrs,
        Item::ForeignMod(item) => &item.attrs,
        Item::Impl(item) => &item.attrs,
        Item::Macro(item) => &item.attrs,
        Item::Mod(item) => &item.attrs,
        Item::Static(item) => &item.attrs,
        Item::Struct(item) => &item.attrs,
        Item::Trait(item) => &item.attrs,
        Item::TraitAlias(item) => &item.attrs,
        Item::Type(item) => &item.attrs,
        Item::Union(item) => &item.attrs,
        Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

fn impl_item_attributes(item: &ImplItem) -> &[Attribute] {
    match item {
        ImplItem::Const(item) => &item.attrs,
        ImplItem::Fn(item) => &item.attrs,
        ImplItem::Type(item) => &item.attrs,
        ImplItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}

fn trait_item_attributes(item: &syn::TraitItem) -> &[Attribute] {
    match item {
        syn::TraitItem::Const(item) => &item.attrs,
        syn::TraitItem::Fn(item) => &item.attrs,
        syn::TraitItem::Type(item) => &item.attrs,
        syn::TraitItem::Macro(item) => &item.attrs,
        _ => &[],
    }
}

fn test_gated_external_module_paths(owner: &Path, syntax: &syn::File) -> BTreeSet<PathBuf> {
    syntax
        .items
        .iter()
        .filter_map(|item| {
            let Item::Mod(module) = item else {
                return None;
            };
            (module.content.is_none() && is_test_gated(&module.attrs))
                .then(|| external_module_path(owner, module))
        })
        .collect()
}

fn external_module_path(owner: &Path, module: &syn::ItemMod) -> PathBuf {
    let parent = owner.parent().unwrap_or_else(|| Path::new(""));
    if let Some(path) = module_path_override(&module.attrs) {
        return parent.join(path);
    }
    let module_dir = match owner.file_name().and_then(|name| name.to_str()) {
        Some("lib.rs" | "main.rs" | "mod.rs") => parent.to_path_buf(),
        _ => parent.join(owner.file_stem().unwrap()),
    };
    let module_name = module.ident.to_string();
    let flat = module_dir.join(format!("{module_name}.rs"));
    let nested = module_dir.join(&module_name).join("mod.rs");
    if flat.is_file() || !nested.is_file() {
        flat
    } else {
        nested
    }
}

fn module_path_override(attributes: &[Attribute]) -> Option<PathBuf> {
    attributes
        .iter()
        .find(|attribute| attribute.path().is_ident("path"))
        .map(|attribute| {
            let Meta::NameValue(path) = &attribute.meta else {
                panic!("module #[path] must be a name-value attribute");
            };
            let Expr::Lit(literal) = &path.value else {
                panic!("module #[path] must use a string literal");
            };
            let Lit::Str(path) = &literal.lit else {
                panic!("module #[path] must use a string literal");
            };
            PathBuf::from(path.value())
        })
}

fn production_rust_source_files(root: &Path) -> Vec<PathBuf> {
    let paths = rust_source_files(root);
    let excluded = paths
        .iter()
        .flat_map(|path| {
            let source = fs::read_to_string(path).unwrap();
            let syntax = parse_rust(&source, path);
            test_gated_external_module_paths(path, &syntax)
        })
        .collect::<BTreeSet<_>>();
    paths
        .into_iter()
        .filter(|path| !excluded.contains(path))
        .collect()
}

fn declares_module(syntax: &syn::File, module_name: &str) -> bool {
    syntax
        .items
        .iter()
        .any(|item| matches!(item, Item::Mod(module) if !is_test_gated(&module.attrs) && module.ident == module_name))
}

fn has_pub_crate_function(syntax: &syn::File, function_name: &str) -> bool {
    syntax.items.iter().any(|item| {
        let Item::Fn(function) = item else {
            return false;
        };
        !is_test_gated(&function.attrs)
            && function.sig.ident == function_name
            && matches!(
                &function.vis,
                syn::Visibility::Restricted(visibility)
                    if visibility.in_token.is_none() && visibility.path.is_ident("crate")
            )
    })
}

fn function_definition_names(syntax: &syn::File) -> BTreeSet<String> {
    let mut visitor = FunctionDefinitionVisitor::default();
    visitor.visit_file(syntax);
    visitor.names
}

#[derive(Default)]
struct FunctionDefinitionVisitor {
    names: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for FunctionDefinitionVisitor {
    fn visit_item(&mut self, item: &'ast Item) {
        if !is_test_gated(item_attributes(item)) {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if !is_test_gated(impl_item_attributes(item)) {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if !is_test_gated(trait_item_attributes(item)) {
            syn::visit::visit_trait_item(self, item);
        }
    }

    fn visit_item_fn(&mut self, function: &'ast syn::ItemFn) {
        self.names.insert(function.sig.ident.to_string());
        syn::visit::visit_item_fn(self, function);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        self.names.insert(function.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, function);
    }

    fn visit_trait_item_fn(&mut self, function: &'ast syn::TraitItemFn) {
        self.names.insert(function.sig.ident.to_string());
        syn::visit::visit_trait_item_fn(self, function);
    }
}

fn cli_config_imports(syntax: &syn::File) -> BTreeSet<String> {
    let mut imports = BTreeSet::new();
    for item in &syntax.items {
        if let Item::Use(item_use) = item {
            if is_test_gated(&item_use.attrs) {
                continue;
            }
            collect_cli_config_imports(&item_use.tree, &mut Vec::new(), &mut imports);
        }
    }
    imports
}

fn collect_cli_config_imports(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    imports: &mut BTreeSet<String>,
) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_cli_config_imports(&path.tree, prefix, imports);
            prefix.pop();
        }
        UseTree::Name(name) => {
            if is_crate_cli_config_prefix(prefix) {
                imports.insert(name.ident.to_string());
            }
        }
        UseTree::Rename(rename) => {
            if is_crate_cli_config_prefix(prefix) {
                imports.insert(rename.ident.to_string());
            }
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_cli_config_imports(item, prefix, imports);
            }
        }
        UseTree::Glob(_) => {
            if is_crate_cli_config_prefix(prefix) {
                imports.insert("*".to_string());
            }
        }
    }
}

fn is_crate_cli_config_prefix(prefix: &[String]) -> bool {
    prefix == ["crate", "cli_config"]
}

fn syntax_usage(syntax: &syn::File) -> SyntaxUsage {
    let mut visitor = SyntaxUsage::default();
    visitor.visit_file(syntax);
    visitor
}

fn calls_function(syntax: &syn::File, function_name: &str) -> bool {
    let mut visitor = FunctionCallVisitor {
        function_name,
        found: false,
    };
    visitor.visit_file(syntax);
    visitor.found
}

struct FunctionCallVisitor<'a> {
    function_name: &'a str,
    found: bool,
}

impl<'ast> Visit<'ast> for FunctionCallVisitor<'_> {
    fn visit_item(&mut self, item: &'ast Item) {
        if !is_test_gated(item_attributes(item)) {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if !is_test_gated(impl_item_attributes(item)) {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if !is_test_gated(trait_item_attributes(item)) {
            syn::visit::visit_trait_item(self, item);
        }
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Expr::Path(path) = unwrap_expr(&call.func) {
            self.found |= path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == self.function_name);
        }
        syn::visit::visit_expr_call(self, call);
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
struct SyntaxUsage {
    read_config_error: bool,
    parse_config_error: bool,
    is_relative_call: bool,
}

impl<'ast> Visit<'ast> for SyntaxUsage {
    fn visit_item(&mut self, item: &'ast Item) {
        if !is_test_gated(item_attributes(item)) {
            syn::visit::visit_item(self, item);
        }
    }

    fn visit_impl_item(&mut self, item: &'ast ImplItem) {
        if !is_test_gated(impl_item_attributes(item)) {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if !is_test_gated(trait_item_attributes(item)) {
            syn::visit::visit_trait_item(self, item);
        }
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let mut segments = path.segments.iter().rev();
        let variant = segments.next().map(|segment| segment.ident.to_string());
        let error_type = segments.next().map(|segment| segment.ident.to_string());
        match (error_type.as_deref(), variant.as_deref()) {
            (Some("Rem6CliError"), Some("ReadConfig")) => self.read_config_error = true,
            (Some("Rem6CliError"), Some("ParseConfig")) => self.parse_config_error = true,
            _ => {}
        }
        syn::visit::visit_path(self, path);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "is_relative" {
            self.is_relative_call = true;
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
struct FlagSets {
    value_flags: BTreeSet<String>,
    bool_flags: BTreeSet<String>,
}

fn assert_explicit_profile_matches_parser(
    authority_source: &str,
    parser_source: &str,
    config_type: &str,
    value_profile: &str,
    bool_profile: Option<&str>,
    relative: &str,
) {
    let authority = syn::parse_file(authority_source)
        .unwrap_or_else(|error| panic!("failed to parse src/cli_config.rs: {error}"));
    let parser = syn::parse_file(parser_source)
        .unwrap_or_else(|error| panic!("failed to parse {relative}: {error}"));
    let profile_flags = FlagSets {
        value_flags: string_array_constant(&authority, value_profile),
        bool_flags: bool_profile
            .map(|profile| string_array_constant(&authority, profile))
            .unwrap_or_default(),
    };
    let parser_flags = parse_args_flag_sets(&parser, config_type, relative);

    assert_eq!(
        profile_flags.value_flags, parser_flags.value_flags,
        "{relative} value-taking flags must exactly match {value_profile}"
    );
    assert_eq!(
        profile_flags.bool_flags,
        parser_flags.bool_flags,
        "{relative} boolean flags must exactly match {}",
        bool_profile.unwrap_or("an empty profile")
    );
}

fn string_array_constant(syntax: &syn::File, constant_name: &str) -> BTreeSet<String> {
    let constant = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Const(constant)
                if !is_test_gated(&constant.attrs) && constant.ident == constant_name =>
            {
                Some(constant)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("src/cli_config.rs must define {constant_name}"));
    let Expr::Array(array) = unwrap_expr(&constant.expr) else {
        panic!("{constant_name} must be a string array");
    };

    let mut values = BTreeSet::new();
    for element in &array.elems {
        let Expr::Lit(literal) = unwrap_expr(element) else {
            panic!("{constant_name} must contain only string literals");
        };
        let Lit::Str(value) = &literal.lit else {
            panic!("{constant_name} must contain only string literals");
        };
        assert!(
            values.insert(value.value()),
            "{constant_name} must not contain duplicate flags"
        );
    }
    values
}

fn unwrap_expr(mut expression: &Expr) -> &Expr {
    loop {
        expression = match expression {
            Expr::Group(group) => &group.expr,
            Expr::Paren(paren) => &paren.expr,
            Expr::Reference(reference) => &reference.expr,
            _ => return expression,
        };
    }
}

fn parse_args_flag_sets(syntax: &syn::File, config_type: &str, relative: &str) -> FlagSets {
    let mut parse_args_methods = Vec::new();
    for item in &syntax.items {
        let Item::Impl(item_impl) = item else {
            continue;
        };
        if is_test_gated(&item_impl.attrs)
            || type_ident(&item_impl.self_ty).as_deref() != Some(config_type)
        {
            continue;
        }
        for item in &item_impl.items {
            if let ImplItem::Fn(function) = item {
                if !is_test_gated(&function.attrs) && function.sig.ident == "parse_args" {
                    parse_args_methods.push(function);
                }
            }
        }
    }
    assert_eq!(
        parse_args_methods.len(),
        1,
        "{relative} must define exactly one {config_type}::parse_args"
    );

    let mut visitor = ParserFlagVisitor::default();
    visitor.visit_block(&parse_args_methods[0].block);
    assert_eq!(
        visitor.flag_match_count, 1,
        "{relative} {config_type}::parse_args must contain one match on flag.as_str()"
    );
    FlagSets {
        value_flags: visitor.value_flags,
        bool_flags: visitor.bool_flags,
    }
}

fn type_ident(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

#[derive(Default)]
struct ParserFlagVisitor {
    flag_match_count: usize,
    value_flags: BTreeSet<String>,
    bool_flags: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for ParserFlagVisitor {
    fn visit_expr_match(&mut self, expression: &'ast syn::ExprMatch) {
        if is_flag_as_str(&expression.expr) {
            self.flag_match_count += 1;
            for arm in &expression.arms {
                let value_taking = consumes_args_next(&arm.body);
                for flag in string_pattern_literals(&arm.pat) {
                    if flag == "--config" {
                        continue;
                    }
                    let (target, other) = if value_taking {
                        (&mut self.value_flags, &self.bool_flags)
                    } else {
                        (&mut self.bool_flags, &self.value_flags)
                    };
                    assert!(
                        !other.contains(&flag),
                        "parser flag {flag} must have only one value/boolean classification"
                    );
                    assert!(target.insert(flag.clone()), "duplicate parser flag {flag}");
                }
            }
            return;
        }
        syn::visit::visit_expr_match(self, expression);
    }
}

fn is_flag_as_str(expression: &Expr) -> bool {
    let Expr::MethodCall(call) = unwrap_expr(expression) else {
        return false;
    };
    call.method == "as_str" && call.args.is_empty() && expression_is_ident(&call.receiver, "flag")
}

fn expression_is_ident(expression: &Expr, expected: &str) -> bool {
    let Expr::Path(path) = unwrap_expr(expression) else {
        return false;
    };
    path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == expected
}

fn string_pattern_literals(pattern: &Pat) -> Vec<String> {
    match pattern {
        Pat::Lit(literal) => match &literal.lit {
            Lit::Str(value) => vec![value.value()],
            _ => Vec::new(),
        },
        Pat::Or(or) => or.cases.iter().flat_map(string_pattern_literals).collect(),
        Pat::Paren(paren) => string_pattern_literals(&paren.pat),
        _ => Vec::new(),
    }
}

fn consumes_args_next(expression: &Expr) -> bool {
    let mut visitor = ArgsNextVisitor::default();
    visitor.visit_expr(expression);
    visitor.found
}

#[derive(Default)]
struct ArgsNextVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for ArgsNextVisitor {
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "next" && expression_is_ident(&call.receiver, "args") {
            self.found = true;
        }
        syn::visit::visit_expr_method_call(self, call);
    }
}
