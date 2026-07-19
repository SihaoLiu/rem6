use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};
use syn::{Fields, ImplItem, Item, Type, Visibility};

const MAX_FACADE_LINES: usize = 1300;
const MAX_SOURCE_LINES: usize = 1800;

#[derive(Default)]
struct DramQosAccessPublicMethodVisitor {
    public_methods: BTreeSet<String>,
}

impl<'ast> Visit<'ast> for DramQosAccessPublicMethodVisitor {
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        if item_impl.trait_.is_none() {
            if let Type::Path(self_ty) = item_impl.self_ty.as_ref() {
                if self_ty
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "DramQosAccess")
                {
                    for item in &item_impl.items {
                        if let ImplItem::Fn(method) = item {
                            if matches!(method.vis, Visibility::Public(_)) {
                                self.public_methods.insert(method.sig.ident.to_string());
                            }
                        }
                    }
                }
            }
        }
        visit::visit_item_impl(self, item_impl);
    }
}

#[test]
fn dram_lib_rs_remains_a_facade() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let lines = line_count(&path);

    assert!(
        lines <= MAX_FACADE_LINES,
        "src/lib.rs should remain a facade over focused DRAM modules, but it has {lines} lines"
    );
}

#[test]
fn dram_error_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let error_rs = crate_dir.join("src/error.rs");

    assert!(error_rs.exists(), "DRAM error code belongs in src/error.rs");
    assert!(
        !lib_rs.contains("pub enum DramError {"),
        "src/lib.rs should re-export DRAM error types from a focused module"
    );
}

#[test]
fn dram_memory_controller_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib_rs = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let memory_controller_rs = crate_dir.join("src/memory_controller.rs");

    assert!(
        memory_controller_rs.exists(),
        "DRAM memory controller state belongs in src/memory_controller.rs"
    );
    let memory_controller = fs::read_to_string(&memory_controller_rs).unwrap();
    assert!(
        !lib_rs.contains("pub struct DramMemoryController {"),
        "src/lib.rs should re-export DRAM memory controller state from a focused module"
    );
    assert!(
        memory_controller.contains("pub struct DramMemoryController {"),
        "src/memory_controller.rs should own DRAM memory controller state"
    );
}

#[test]
fn dram_qos_access_does_not_cache_access_byte_count() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_dir.join("src/qos.rs")).unwrap();
    let syntax = syn::parse_file(&source).unwrap();
    let fields = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == "DramQosAccess" => Some(&item.fields),
            _ => None,
        })
        .expect("src/qos.rs must define DramQosAccess");
    let Fields::Named(fields) = fields else {
        panic!("DramQosAccess must remain a named-field struct");
    };
    let field_shapes = fields
        .named
        .iter()
        .map(|field| {
            assert!(
                matches!(field.vis, Visibility::Inherited),
                "DramQosAccess metadata fields must remain private"
            );
            let Type::Path(field_type) = &field.ty else {
                panic!("DramQosAccess metadata fields must use named path types");
            };
            assert!(
                field_type.qself.is_none() && field_type.path.segments.len() == 1,
                "DramQosAccess metadata fields must use direct named types"
            );
            (
                field.ident.as_ref().unwrap().to_string(),
                field_type.path.segments[0].ident.to_string(),
            )
        })
        .collect::<BTreeSet<_>>();

    assert_eq!(
        field_shapes,
        [
            ("assigned_priority", "QosPriority"),
            ("effective_priority", "QosPriority"),
            ("requestor", "QosRequestorId"),
        ]
        .into_iter()
        .map(|(name, field_type)| (name.to_owned(), field_type.to_owned()))
        .collect(),
        "DramQosAccess must not cache metadata already owned by DramAccess"
    );

    let mut visitor = DramQosAccessPublicMethodVisitor::default();
    for path in rust_source_files(&crate_dir.join("src")) {
        let source = fs::read_to_string(&path).unwrap();
        let syntax = syn::parse_file(&source).unwrap();
        visitor.visit_file(&syntax);
    }
    assert_eq!(
        visitor.public_methods,
        [
            "assigned_priority",
            "effective_priority",
            "escalated",
            "requestor",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        "DramQosAccess public access must remain limited to QoS metadata"
    );
}

#[test]
fn dram_source_files_stay_within_size_limit() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut oversized = Vec::new();

    for path in rust_source_files(&src_dir) {
        let lines = line_count(&path);
        if lines > MAX_SOURCE_LINES {
            oversized.push(format!(
                "{} has {lines} lines",
                path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
                    .unwrap()
                    .display()
            ));
        }
    }

    assert!(
        oversized.is_empty(),
        "source files exceed {MAX_SOURCE_LINES} lines: {}",
        oversized.join(", ")
    );
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_rust_source_files(root, &mut paths);
    paths.sort();
    paths
}

fn collect_rust_source_files(root: &Path, paths: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(&path, paths);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            paths.push(path);
        }
    }
}

fn line_count(path: &Path) -> usize {
    fs::read_to_string(path).unwrap().lines().count()
}
