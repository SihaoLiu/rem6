use std::{collections::BTreeSet, fs, path::Path};

use super::collect_rust_source_files;
use syn::{parse::Parser, visit::Visit};

const PROTOCOL_SPELLINGS: [&str; 4] = ["msi", "mesi", "moesi", "chi"];

#[test]
fn protocol_source_scan_uses_rust_items_and_literals() {
    let source = r#"
        // enum Ignored { Msi }
        enum Protocol { Msi, Mesi }
        impl Protocol {
            const ALL: [Self; 2] = [Self::Msi, Self::Mesi];
            fn parse(value: &str) -> Option<Self> {
                Self::ALL.into_iter().find(|protocol| protocol.as_str() == value)
            }
            const fn as_str(self) -> &'static str {
                match self {
                    Self::Msi => "msi",
                    Self::Mesi => "mesi",
                }
            }
        }
    "#;

    let contract = protocol_type_contract(source, "Protocol");
    assert_eq!(contract.variants, ["Msi", "Mesi"]);
    assert_eq!(contract.associated_items, ["ALL", "as_str", "parse"]);
    assert_eq!(contract.parse_literals, Vec::<String>::new());
    assert_eq!(contract.as_str_literals, ["msi", "mesi"]);
    assert_eq!(rust_string_literals(source), ["msi", "mesi"]);
}

#[test]
fn protocol_consumer_scan_uses_production_rust_syntax() {
    let source = r#"
        // fn parse_data_cache_protocol() {}
        fn consume() {
            for protocol in Protocol::ALL {
                let _ = protocol.as_str();
            }
        }
        mod nested {
            fn parse_data_cache_protocol() {}
        }
        const DUPLICATE: [Protocol; 2] = [Protocol::Msi, Protocol::Mesi];
        #[cfg(test)]
        mod tests {
            const FIXTURE: &str = "msi";
        }
    "#;

    let callables = rust_callable_definition_names(source);
    assert!(callables.contains("parse_data_cache_protocol"));
    assert!(rust_method_call_names(source).contains("as_str"));
    assert!(rust_path_references(source).contains("Protocol::ALL"));
    assert_eq!(manual_protocol_inventory_count(source), 1);
    assert_eq!(
        rust_production_string_literals(source),
        Vec::<String>::new()
    );
}

#[test]
fn data_cache_protocol_spelling_lives_with_protocol_types() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let system_owner = read(&workspace, "crates/rem6-system/src/data_cache_run.rs");
    let workload_owner = read(&workspace, "crates/rem6-workload/src/result.rs");

    for (owner_name, owner) in [
        ("RiscvDataCacheProtocol", &system_owner),
        ("WorkloadDataCacheProtocol", &workload_owner),
    ] {
        let contract = protocol_type_contract(owner, owner_name);
        assert_eq!(
            contract.variants,
            ["Msi", "Mesi", "Moesi", "Chi"],
            "{owner_name} must own the complete protocol inventory in stable order"
        );
        assert_eq!(
            contract.associated_items,
            ["ALL", "as_str", "parse"],
            "{owner_name} must own exactly the protocol inventory, parser, and stable spelling API"
        );
        assert!(
            contract.parse_literals.is_empty(),
            "{owner_name} parsing must derive from the stable spelling authority"
        );
        assert_eq!(
            contract.as_str_literals, PROTOCOL_SPELLINGS,
            "{owner_name} stable spelling must cover every protocol exactly once"
        );
        let owner_literals = rust_string_literals(owner);
        for spelling in PROTOCOL_SPELLINGS {
            assert_eq!(
                owner_literals
                    .iter()
                    .filter(|literal| literal.as_str() == spelling)
                    .count(),
                1,
                "{owner_name} must define `{spelling}` exactly once and derive parsing from as_str"
            );
        }
    }

    let consumers = [
        "crates/rem6/src/config/cache.rs",
        "crates/rem6/src/gpu_cli.rs",
        "crates/rem6/src/cli_error.rs",
        "crates/rem6/src/artifact_json/run.rs",
    ];
    for relative in consumers {
        let source = read(&workspace, relative);
        let literals = rust_string_literals(&source);
        for spelling in PROTOCOL_SPELLINGS {
            assert!(
                !literals.iter().any(|literal| literal == spelling),
                "{relative} duplicates the `{spelling}` protocol spelling authority"
            );
        }
    }
}

#[test]
fn data_cache_protocol_consumers_use_enum_authority() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = read(&workspace, "crates/rem6/src/config/cache.rs");
    let config_functions = rust_callable_definition_names(&config);
    assert!(!config_functions.contains("parse_data_cache_protocol"));
    assert!(!config_functions.contains("parse_run_data_cache_protocol"));

    let run_config = read(&workspace, "crates/rem6/src/config.rs");
    assert!(
        rust_path_references(&run_config).contains("RiscvDataCacheProtocol::parse"),
        "run config parsing must use RiscvDataCacheProtocol::parse"
    );

    let trace_config = read(&workspace, "crates/rem6/src/config/trace_replay.rs");
    assert!(
        rust_path_references(&trace_config).contains("WorkloadDataCacheProtocol::parse"),
        "trace-replay config parsing must use WorkloadDataCacheProtocol::parse"
    );

    let gpu = read(&workspace, "crates/rem6/src/gpu_cli.rs");
    let gpu_functions = rust_callable_definition_names(&gpu);
    assert!(!gpu_functions.contains("parse_data_cache_protocol"));
    assert!(!gpu_functions.contains("data_cache_protocol_name"));
    assert!(
        rust_path_references(&gpu).contains("RiscvDataCacheProtocol::parse"),
        "GPU CLI parsing must use RiscvDataCacheProtocol::parse"
    );
    assert!(rust_method_call_names(&gpu).contains("as_str"));

    let cli_error = read(&workspace, "crates/rem6/src/cli_error.rs");
    let cli_error_functions = rust_callable_definition_names(&cli_error);
    assert!(!cli_error_functions.contains("riscv_data_cache_protocol_name"));
    let cli_error_method_calls = rust_method_call_names(&cli_error);
    assert!(
        cli_error_method_calls.contains("as_str"),
        "CLI error rendering must use protocol.as_str(); calls: {cli_error_method_calls:?}"
    );

    let artifact = read(&workspace, "crates/rem6/src/artifact_json/run.rs");
    let artifact_functions = rust_callable_definition_names(&artifact);
    assert!(!artifact_functions.contains("riscv_cache_protocol_name"));
    assert!(rust_method_call_names(&artifact).contains("as_str"));

    let stats = read(&workspace, "crates/rem6/src/stats_output/trace_replay.rs");
    assert!(
        rust_path_references(&stats).contains("WorkloadDataCacheProtocol::ALL"),
        "trace-replay stats must iterate the canonical workload protocol inventory"
    );

    let system_owner = read(&workspace, "crates/rem6-system/src/data_cache_run.rs");
    assert!(
        rust_path_references(&system_owner).contains("RiscvDataCacheProtocol::ALL"),
        "system data-cache histories must iterate the canonical protocol inventory"
    );

    let topology = read(
        &workspace,
        "crates/rem6-system/src/topology/data_cache_history.rs",
    );
    assert!(
        rust_path_references(&topology).contains("RiscvDataCacheProtocol::ALL"),
        "topology data-cache histories must iterate the canonical protocol inventory"
    );
}

#[test]
fn data_cache_protocol_has_no_parallel_production_authority() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let owner_paths = BTreeSet::from([
        "crates/rem6-system/src/data_cache_run.rs",
        "crates/rem6-workload/src/result.rs",
    ]);
    let banned_helpers = [
        "parse_data_cache_protocol",
        "parse_run_data_cache_protocol",
        "data_cache_protocol_name",
        "riscv_cache_protocol_name",
        "riscv_data_cache_protocol_name",
    ];
    let mut violations = Vec::new();

    for (relative, source) in production_rust_sources(&workspace) {
        let mentions_protocol_authority = source.contains("DataCacheProtocol")
            || banned_helpers.iter().any(|helper| source.contains(helper))
            || PROTOCOL_SPELLINGS
                .iter()
                .any(|spelling| source.contains(&format!("\"{spelling}\"")));
        if !mentions_protocol_authority {
            continue;
        }
        let scan = production_source_scan(&source);
        if !owner_paths.contains(relative.as_str()) {
            for spelling in PROTOCOL_SPELLINGS {
                if scan
                    .string_literals
                    .iter()
                    .any(|literal| literal == spelling)
                {
                    violations.push(format!(
                        "{relative} duplicates protocol spelling `{spelling}`"
                    ));
                }
            }
            if scan.manual_protocol_inventories != 0 {
                violations.push(format!(
                    "{relative} contains {} manual protocol inventories",
                    scan.manual_protocol_inventories
                ));
            }
        }
        for helper in banned_helpers {
            if scan.callables.contains(helper) {
                violations.push(format!("{relative} defines obsolete helper `{helper}`"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "data-cache protocol authority violations:\n{}",
        violations.join("\n")
    );
}

fn read(workspace: &Path, relative: &str) -> String {
    fs::read_to_string(workspace.join(relative))
        .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

fn production_rust_sources(workspace: &Path) -> Vec<(String, String)> {
    let mut paths = Vec::new();
    collect_rust_source_files(&workspace.join("crates"), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .filter(|path| {
            path.components()
                .any(|component| component.as_os_str() == "src")
        })
        .filter(|path| {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or_default();
            stem != "tests" && !stem.ends_with("_tests")
        })
        .map(|path| {
            let relative = path
                .strip_prefix(workspace)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"));
            (relative, source)
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
struct ProtocolTypeContract {
    variants: Vec<String>,
    associated_items: Vec<String>,
    parse_literals: Vec<String>,
    as_str_literals: Vec<String>,
}

fn protocol_type_contract(source: &str, type_name: &str) -> ProtocolTypeContract {
    let syntax = syn::parse_file(source)
        .unwrap_or_else(|error| panic!("failed to parse {type_name} source: {error}"));
    let variants = syntax
        .items
        .iter()
        .find_map(|item| {
            let syn::Item::Enum(item) = item else {
                return None;
            };
            (item.ident == type_name).then(|| {
                item.variants
                    .iter()
                    .map(|variant| variant.ident.to_string())
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_else(|| panic!("missing enum {type_name}"));

    let mut associated_items = BTreeSet::new();
    let mut parse_literals = Vec::new();
    let mut as_str_literals = Vec::new();
    for item in &syntax.items {
        let syn::Item::Impl(item) = item else {
            continue;
        };
        let syn::Type::Path(target) = item.self_ty.as_ref() else {
            continue;
        };
        if target
            .path
            .segments
            .last()
            .is_none_or(|segment| segment.ident != type_name)
        {
            continue;
        }
        for associated in &item.items {
            match associated {
                syn::ImplItem::Const(item) => {
                    associated_items.insert(item.ident.to_string());
                }
                syn::ImplItem::Fn(item) => {
                    let name = item.sig.ident.to_string();
                    associated_items.insert(name.clone());
                    if name == "parse" {
                        parse_literals = string_literals_in_block(&item.block);
                    } else if name == "as_str" {
                        as_str_literals = string_literals_in_block(&item.block);
                    }
                }
                _ => {}
            }
        }
    }

    ProtocolTypeContract {
        variants,
        associated_items: associated_items.into_iter().collect(),
        parse_literals,
        as_str_literals,
    }
}

fn rust_string_literals(source: &str) -> Vec<String> {
    let syntax =
        syn::parse_file(source).unwrap_or_else(|error| panic!("failed to parse source: {error}"));
    let mut visitor = StringLiteralVisitor::default();
    visitor.visit_file(&syntax);
    visitor.values
}

fn rust_callable_definition_names(source: &str) -> BTreeSet<String> {
    production_source_scan(source).callables
}

fn rust_method_call_names(source: &str) -> BTreeSet<String> {
    production_source_scan(source).method_calls
}

fn rust_path_references(source: &str) -> BTreeSet<String> {
    production_source_scan(source).paths
}

fn manual_protocol_inventory_count(source: &str) -> usize {
    production_source_scan(source).manual_protocol_inventories
}

fn rust_production_string_literals(source: &str) -> Vec<String> {
    production_source_scan(source).string_literals
}

fn production_source_scan(source: &str) -> ProductionSourceVisitor {
    let syntax =
        syn::parse_file(source).unwrap_or_else(|error| panic!("failed to parse source: {error}"));
    let mut visitor = ProductionSourceVisitor::default();
    scan_production_items(&syntax.items, &mut visitor);
    visitor
}

fn scan_production_items(items: &[syn::Item], visitor: &mut ProductionSourceVisitor) {
    for item in items {
        match item {
            syn::Item::Const(item) if !cfg_requires_test(&item.attrs) => {
                visitor.visit_expr(&item.expr);
            }
            syn::Item::Fn(item) if !cfg_requires_test(&item.attrs) => {
                visitor.callables.insert(item.sig.ident.to_string());
                visitor.visit_block(&item.block);
            }
            syn::Item::Impl(item) if !cfg_requires_test(&item.attrs) => {
                for associated in &item.items {
                    match associated {
                        syn::ImplItem::Const(item) if !cfg_requires_test(&item.attrs) => {
                            visitor.visit_expr(&item.expr);
                        }
                        syn::ImplItem::Fn(item) if !cfg_requires_test(&item.attrs) => {
                            visitor.callables.insert(item.sig.ident.to_string());
                            visitor.visit_block(&item.block);
                        }
                        _ => {}
                    }
                }
            }
            syn::Item::Mod(item) if !cfg_requires_test(&item.attrs) => {
                if let Some((_, items)) = &item.content {
                    scan_production_items(items, visitor);
                }
            }
            syn::Item::Static(item) if !cfg_requires_test(&item.attrs) => {
                visitor.visit_expr(&item.expr);
            }
            syn::Item::Trait(item) if !cfg_requires_test(&item.attrs) => {
                for associated in &item.items {
                    let syn::TraitItem::Fn(item) = associated else {
                        continue;
                    };
                    if cfg_requires_test(&item.attrs) {
                        continue;
                    }
                    visitor.callables.insert(item.sig.ident.to_string());
                    if let Some(block) = &item.default {
                        visitor.visit_block(block);
                    }
                }
            }
            _ => {}
        }
    }
}

fn string_literals_in_block(block: &syn::Block) -> Vec<String> {
    let mut visitor = StringLiteralVisitor::default();
    visitor.visit_block(block);
    visitor.values
}

#[derive(Default)]
struct StringLiteralVisitor {
    values: Vec<String>,
}

impl<'ast> Visit<'ast> for StringLiteralVisitor {
    fn visit_lit_str(&mut self, literal: &'ast syn::LitStr) {
        self.values.push(literal.value());
    }
}

#[derive(Default)]
struct ProductionSourceVisitor {
    callables: BTreeSet<String>,
    method_calls: BTreeSet<String>,
    paths: BTreeSet<String>,
    string_literals: Vec<String>,
    manual_protocol_inventories: usize,
}

impl<'ast> Visit<'ast> for ProductionSourceVisitor {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        match item {
            syn::Item::Fn(item) => self.visit_item_fn(item),
            syn::Item::Impl(item) => self.visit_item_impl(item),
            syn::Item::Mod(item) => self.visit_item_mod(item),
            _ => syn::visit::visit_item(self, item),
        }
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if cfg_requires_test(&item.attrs) {
            return;
        }
        syn::visit::visit_item_mod(self, item);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if cfg_requires_test(&item.attrs) {
            return;
        }
        self.callables.insert(item.sig.ident.to_string());
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if cfg_requires_test(&item.attrs) {
            return;
        }
        for associated in &item.items {
            match associated {
                syn::ImplItem::Const(item) if !cfg_requires_test(&item.attrs) => {
                    self.visit_expr(&item.expr);
                }
                syn::ImplItem::Fn(item) if !cfg_requires_test(&item.attrs) => {
                    self.callables.insert(item.sig.ident.to_string());
                    self.visit_block(&item.block);
                }
                _ => {}
            }
        }
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if cfg_requires_test(&item.attrs) {
            return;
        }
        self.callables.insert(item.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        if cfg_requires_test(&item.attrs) {
            return;
        }
        self.callables.insert(item.sig.ident.to_string());
        syn::visit::visit_trait_item_fn(self, item);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast syn::ExprMethodCall) {
        self.method_calls.insert(expression.method.to_string());
        syn::visit::visit_expr_method_call(self, expression);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.paths.insert(
            path.segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::"),
        );
        syn::visit::visit_path(self, path);
    }

    fn visit_lit_str(&mut self, literal: &'ast syn::LitStr) {
        self.string_literals.push(literal.value());
    }

    fn visit_macro(&mut self, item: &'ast syn::Macro) {
        self.visit_path(&item.path);
        let parser = syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated;
        if let Ok(arguments) = parser.parse2(item.tokens.clone()) {
            for argument in &arguments {
                self.visit_expr(argument);
            }
        }
    }

    fn visit_expr_array(&mut self, expression: &'ast syn::ExprArray) {
        let variants = expression
            .elems
            .iter()
            .filter_map(protocol_variant_path)
            .collect::<Vec<_>>();
        if variants.len() >= 2 && variants.iter().all(|(owner, _)| owner == &variants[0].0) {
            self.manual_protocol_inventories += 1;
        }
        syn::visit::visit_expr_array(self, expression);
    }
}

fn protocol_variant_path(expression: &syn::Expr) -> Option<(String, String)> {
    let syn::Expr::Path(expression) = expression else {
        return None;
    };
    let mut segments = expression.path.segments.iter().rev();
    let variant = segments.next()?.ident.to_string();
    let owner = segments.next()?.ident.to_string();
    if !matches!(
        owner.as_str(),
        "RiscvDataCacheProtocol" | "WorkloadDataCacheProtocol" | "Protocol"
    ) || !matches!(variant.as_str(), "Msi" | "Mesi" | "Moesi" | "Chi")
    {
        return None;
    }
    Some((owner, variant))
}

fn cfg_requires_test(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if !attribute.path().is_ident("cfg") {
            return false;
        }
        attribute
            .parse_args::<syn::Meta>()
            .is_ok_and(|predicate| cfg_predicate_requires_test(&predicate))
    })
}

fn cfg_predicate_requires_test(predicate: &syn::Meta) -> bool {
    match predicate {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) if list.path.is_ident("all") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|predicates| predicates.iter().any(cfg_predicate_requires_test)),
        syn::Meta::List(list) if list.path.is_ident("any") => list
            .parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .is_ok_and(|predicates| {
                !predicates.is_empty() && predicates.iter().all(cfg_predicate_requires_test)
            }),
        syn::Meta::List(_) | syn::Meta::NameValue(_) => false,
    }
}
