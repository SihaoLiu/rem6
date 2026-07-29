use super::*;

const POLICY: &str = "tests/source_policy/live_checkpoint/pending_address.rs";
const CODEC: &str = "src/riscv_live_checkpoint/codec.rs";
const FETCH: &str = "src/riscv_live_checkpoint/fetch/pending_address.rs";
const CAPTURE: &str = "src/o3_runtime_live_checkpoint/pending_address.rs";
const CAPTURE_GRAPH: &str = "src/o3_runtime_live_checkpoint/pending_address/graph.rs";
const TESTS: &str = "src/riscv_live_checkpoint_tests/pending_address/runtime.rs";
const CODEC_TESTS: &str = "src/riscv_live_checkpoint_tests/pending_address/runtime/codec.rs";
const RESTORE_TESTS: &str = "src/riscv_live_checkpoint_tests/pending_address/runtime/restore.rs";
const GRAPH_TESTS: &str = "src/riscv_live_checkpoint_tests/pending_address/graph/restore.rs";
const MATERIALIZATION_TESTS: &str =
    "src/riscv_live_checkpoint_tests/pending_address/graph/restore/materialization.rs";

#[test]
fn pending_address_live_checkpoint_cpu_sources_are_attached_and_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_unconditional_attachment(
        crate_dir,
        "tests/source_policy/live_checkpoint.rs",
        POLICY,
        "live_checkpoint/pending_address.rs",
        "pending_address",
    );

    for (owner, child, path, module) in [
        (
            "src/riscv_live_checkpoint_tests.rs",
            "src/riscv_live_checkpoint_tests/pending_address.rs",
            "riscv_live_checkpoint_tests/pending_address.rs",
            "pending_address",
        ),
        (
            "src/riscv_live_checkpoint_tests/pending_address.rs",
            TESTS,
            "pending_address/runtime.rs",
            "runtime",
        ),
        (TESTS, CODEC_TESTS, "runtime/codec.rs", "codec"),
        (TESTS, RESTORE_TESTS, "runtime/restore.rs", "restore"),
        (
            "src/riscv_live_checkpoint/fetch.rs",
            FETCH,
            "fetch/pending_address.rs",
            "pending_address",
        ),
        (
            "src/riscv_live_checkpoint/fetch.rs",
            "src/riscv_live_checkpoint/fetch/selection.rs",
            "fetch/selection.rs",
            "selection",
        ),
        (CAPTURE, CAPTURE_GRAPH, "pending_address/graph.rs", "graph"),
        (
            "src/riscv_live_checkpoint_tests/pending_address/graph.rs",
            GRAPH_TESTS,
            "graph/restore.rs",
            "restore",
        ),
        (
            GRAPH_TESTS,
            MATERIALIZATION_TESTS,
            "restore/materialization.rs",
            "materialization",
        ),
    ] {
        assert_unconditional_attachment(crate_dir, owner, child, path, module);
    }

    for (relative, maximum) in [
        (POLICY, 260),
        ("src/riscv_live_checkpoint/pending_address.rs", 180),
        ("src/riscv_live_checkpoint/codec/pending_address.rs", 260),
        ("src/riscv_live_checkpoint/fetch.rs", 220),
        ("src/riscv_live_checkpoint/fetch/pending_address.rs", 130),
        ("src/riscv_live_checkpoint/fetch/selection.rs", 100),
        (CAPTURE, 400),
        (CAPTURE_GRAPH, 650),
        ("src/o3_runtime_live_checkpoint/support.rs", 100),
        ("src/riscv_core_checkpoint_restore/pending_address.rs", 180),
        ("src/riscv_live_checkpoint_tests/pending_address.rs", 500),
        (TESTS, 500),
        (CODEC_TESTS, 500),
        (RESTORE_TESTS, 500),
        (GRAPH_TESTS, 750),
        (MATERIALIZATION_TESTS, 240),
    ] {
        let path = crate_dir.join(relative);
        let lines = line_count(&path);
        assert!(
            lines <= maximum,
            "{relative} has {lines} lines, exceeding its {maximum}-line cap"
        );
    }
}

#[test]
fn pending_address_wire_version_and_profile_are_mutation_locked() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codec = fs::read_to_string(crate_dir.join(CODEC)).unwrap();
    let tests = pending_address_tests(crate_dir);
    assert!(wire_contract(&codec, &tests));

    for (from, to) in [
        (
            "const VERSION_CURRENT: u8 = 3;",
            "const VERSION_CURRENT: u8 = 4;",
        ),
        (
            "const VERSION_PENDING_SINGLE: u8 = 2;",
            "const VERSION_PENDING_SINGLE: u8 = 4;",
        ),
        (
            "const VERSION_LEGACY: u8 = 1;",
            "const VERSION_LEGACY: u8 = 0;",
        ),
        (
            "RiscvO3LiveCheckpointProfile::PendingDataAddress => 2,",
            "RiscvO3LiveCheckpointProfile::PendingDataAddress => 3,",
        ),
        (
            "(VERSION_PENDING_SINGLE | VERSION_CURRENT, 2) => {",
            "(VERSION_CURRENT, 2) => {",
        ),
    ] {
        let mutated = codec.replacen(from, to, 1);
        assert_ne!(mutated, codec, "wire mutation must apply: {from}");
        assert!(!wire_contract(&mutated, &tests));
    }
}

#[test]
fn pending_address_capture_and_fetch_ownership_are_mutation_locked() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fetch = fs::read_to_string(crate_dir.join(FETCH)).unwrap();
    let capture = fs::read_to_string(crate_dir.join(CAPTURE)).unwrap();
    let capture_graph = fs::read_to_string(crate_dir.join(CAPTURE_GRAPH)).unwrap();
    let tests = pending_address_tests(crate_dir);
    assert!(pending_fetch_contract(&fetch, &tests));
    assert!(pending_capture_contract(&capture, &capture_graph, &tests));

    let generic = fetch.replacen("events: Vec::new(),", "events: state.events.clone(),", 1);
    assert_ne!(generic, fetch, "generic-event mutation must apply");
    assert!(!pending_fetch_contract(&generic, &tests));

    let materialized = capture.replacen(
        "row.materialized.is_some()",
        "row.materialized.is_none()",
        1,
    );
    assert_ne!(materialized, capture, "materialization mutation must apply");
    assert!(!pending_capture_contract(
        &materialized,
        &capture_graph,
        &tests
    ));

    let atomic = capture_graph.replacen("|| row.root_head.atomic_head", "|| false", 1);
    assert_ne!(atomic, capture_graph, "atomic graph mutation must apply");
    assert!(!pending_capture_contract(&capture, &atomic, &tests));
}

fn wire_contract(codec: &str, tests: &str) -> bool {
    let active = compact_rust_code(&production_rust_source(codec));
    active.contains("constVERSION_LEGACY:u8=1;")
        && active.contains("constVERSION_PENDING_SINGLE:u8=2;")
        && active.contains("constVERSION_CURRENT:u8=3;")
        && active.contains("byte(&mutout,VERSION_CURRENT);")
        && active.contains("if!(VERSION_LEGACY..=VERSION_CURRENT).contains(&version)")
        && active.contains("(_,0)=>RiscvO3LiveCheckpointProfile::ComputeQueue")
        && active.contains("(_,1)=>RiscvO3LiveCheckpointProfile::CompletedFpLoad")
        && active.contains("(VERSION_PENDING_SINGLE|VERSION_CURRENT,2)=>{RiscvO3LiveCheckpointProfile::PendingDataAddress}")
        && active.contains("VERSION_PENDING_SINGLE=>pending_address::read_v2_single(&mutreader)?")
        && active.contains("VERSION_CURRENT=>pending_address::read_v3_rows(&mutreader)?")
        && active.contains("RiscvO3LiveCheckpointProfile::PendingDataAddress=>2")
        && tests.contains("o3_live_checkpoint_v3_round_trips_pending_store_and_decodes_v1")
        && tests.contains("o3_live_checkpoint_v1_rejects_pending_profile_tag")
}

fn pending_fetch_contract(source: &str, tests: &str) -> bool {
    let Some(project) =
        unconditional_rust_function_definition(source, "project_pending_live_fetch")
    else {
        return false;
    };
    let project = compact_rust_code(&project);
    project.contains("events:Vec::new(),")
        && project.contains("executed_fetch_requests:Vec::new(),")
        && project.contains("issued_fetch_requests:Vec::new(),")
        && project.contains("state.events.iter().any(")
        && project.contains("state.executed_fetches.contains(")
        && project.contains("state.issued_data_for_fetches.contains(")
        && tests.contains("pending_store_capture_keeps_generic_execution_events_empty")
}

fn pending_capture_contract(source: &str, graph_source: &str, tests: &str) -> bool {
    let Some(capture) = unconditional_rust_function_definition(source, "capture") else {
        return false;
    };
    let Some(store_capture) = unconditional_rust_function_definition(source, "capture_store")
    else {
        return false;
    };
    let capture = compact_rust_code(&capture);
    let store_capture = compact_rust_code(&store_capture);
    let graph_capture = compact_rust_code(&production_rust_source(graph_source));
    capture.contains("runtime.pending_data_addresses.is_empty()")
        && capture.contains("capture_store(runtime,captured_tick,resident_sequences)")
        && capture.contains("graph::capture(runtime,captured_tick,resident_sequences)")
        && store_capture.contains("runtime.pending_data_addresses.len()!=1")
        && store_capture.contains("row.selected_issue_tick.is_some()")
        && store_capture.contains("row.materialized.is_some()")
        && store_capture.contains("runtime.pending_data_accesses.is_empty()")
        && store_capture.contains("runtime.live_data_accesses.is_empty()")
        && graph_capture.contains("row.selected_issue_tick.is_some()")
        && graph_capture.contains("row.materialized.is_some()")
        && graph_capture.contains("||row.root_head.atomic_head")
        && graph_capture.contains("runtime.pending_data_accesses.is_empty()")
        && graph_capture.contains("runtime.live_data_accesses.is_empty()")
        && graph_capture.contains("live_data_access_younger_sequences")
        && tests.contains("pending_store_capture_uses_pending_profile_after_producer_commit")
        && tests.contains("pending_store_rebound_wake_materializes_restored_request")
        && tests.contains("pending_load_graph_capture_projects_all_addressless_owners")
}

fn pending_address_tests(crate_dir: &Path) -> String {
    [
        TESTS,
        CODEC_TESTS,
        RESTORE_TESTS,
        GRAPH_TESTS,
        MATERIALIZATION_TESTS,
    ]
    .map(|path| fs::read_to_string(crate_dir.join(path)).unwrap())
    .join("\n")
}

fn assert_unconditional_attachment(
    crate_dir: &Path,
    owner_path: &str,
    child_path: &str,
    attribute_path: &str,
    module: &str,
) {
    let owner = fs::read_to_string(crate_dir.join(owner_path)).unwrap();
    let child = fs::read_to_string(crate_dir.join(child_path)).unwrap();
    let count = active_unconditional_path_owned_module_declaration_count;
    let attached = |owner: &str| count(owner, &child, attribute_path, module) == 1;
    assert!(attached(&owner));

    let declaration = format!("#[path = \"{attribute_path}\"]\nmod {module};");
    for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
        let mutated = owner.replacen(&declaration, &format!("{conditional}{declaration}"), 1);
        assert_ne!(mutated, owner, "attachment mutation must apply: {module}");
        assert!(!attached(&mutated));
    }
}
