use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use syn::visit::Visit;
use syn::{Expr, ImplItem, Item, Lit, Pat, Type, UseTree};

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

    for path in rust_source_files(&crate_dir.join("src")) {
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

fn parse_rust(source: &str, path: &Path) -> syn::File {
    syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn declares_module(syntax: &syn::File, module_name: &str) -> bool {
    syntax
        .items
        .iter()
        .any(|item| matches!(item, Item::Mod(module) if module.ident == module_name))
}

fn has_pub_crate_function(syntax: &syn::File, function_name: &str) -> bool {
    syntax.items.iter().any(|item| {
        let Item::Fn(function) = item else {
            return false;
        };
        function.sig.ident == function_name
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

#[derive(Default)]
struct SyntaxUsage {
    read_config_error: bool,
    parse_config_error: bool,
    is_relative_call: bool,
}

impl<'ast> Visit<'ast> for SyntaxUsage {
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
            Item::Const(constant) if constant.ident == constant_name => Some(constant),
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
        if type_ident(&item_impl.self_ty).as_deref() != Some(config_type) {
            continue;
        }
        for item in &item_impl.items {
            if let ImplItem::Fn(function) = item {
                if function.sig.ident == "parse_args" {
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
