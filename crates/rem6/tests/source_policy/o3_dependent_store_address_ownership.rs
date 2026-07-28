use std::fs;
use std::path::Path;

use super::{
    line_count, module_has_path_attribute, rust_function_definition_names, CORE_TEST_ANCHORS,
};

const POLICY: &str = "tests/source_policy/o3_dependent_store_address_ownership.rs";
const PARENT: &str = "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs";
const OWNER: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/dependent_store.rs";
const BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/boundaries.rs";
const SHARED_BOUNDARY_ANCHOR: &str =
    "rem6_run_o3_dependent_result_address_boundaries_and_live_actions";
const CPU_FETCH_TESTS: &str =
    "crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address/dependent_store.rs";
const CPU_PENDING_TESTS: &str =
    "crates/rem6-cpu/src/o3_runtime_pending_address_tests/dependent_store.rs";
const ANCHORS: [&str; 4] = [
    "rem6_run_o3_dependent_store_address_matrix_direct",
    "rem6_run_o3_dependent_store_address_matrix_cache_fabric_dram",
    "rem6_run_timing_suppresses_o3_dependent_store_address",
    "rem6_run_o3_dependent_store_address_overlap_and_live_actions",
];

#[test]
fn o3_dependent_store_address_cli_owner_is_bounded_and_canonical() {
    let rem6 = Path::new(env!("CARGO_MANIFEST_DIR"));
    let policy_root = read(&rem6.join("tests/source_policy.rs"));
    let parent = read(&rem6.join(PARENT));
    let owner_path = rem6.join(OWNER);
    let owner = read(&owner_path);
    let boundaries_path = rem6.join(BOUNDARIES);
    let boundaries = read(&boundaries_path);

    assert!(module_has_path_attribute(
        &policy_root,
        "o3_dependent_store_address_ownership",
        "source_policy/o3_dependent_store_address_ownership.rs",
    ));
    assert!(module_has_path_attribute(
        &parent,
        "dependent_store",
        "dependent_result_address/dependent_store.rs",
    ));
    for (path, maximum) in [
        (rem6.join(PARENT), 650),
        (owner_path, 500),
        (boundaries_path, 425),
        (rem6.join(POLICY), 225),
    ] {
        assert!(
            line_count(&path) <= maximum,
            "{} exceeds {maximum} lines",
            path.display()
        );
    }
    assert!(!owner.contains("include!("));
    let tests = rust_function_definition_names(&owner)
        .into_iter()
        .filter(|name| name.starts_with("rem6_run_"))
        .collect::<Vec<_>>();
    assert_eq!(
        tests.len(),
        ANCHORS.len(),
        "unexpected dependent-store inventory"
    );
    for anchor in ANCHORS {
        assert_eq!(
            tests.iter().filter(|name| name.as_str() == anchor).count(),
            1
        );
        assert_eq!(
            CORE_TEST_ANCHORS
                .lines()
                .filter(|line| *line == anchor)
                .count(),
            1,
            "{anchor} must be a canonical migration anchor"
        );
    }
    for boundary in [
        "DependentStoreAlias",
        "DependentAtomic",
        "DependentStoreConditional",
    ] {
        assert!(boundaries.contains(boundary), "missing boundary {boundary}");
    }
    assert_eq!(
        rust_function_definition_names(&boundaries)
            .iter()
            .filter(|name| name.as_str() == SHARED_BOUNDARY_ANCHOR)
            .count(),
        1
    );
    assert_eq!(
        CORE_TEST_ANCHORS
            .lines()
            .filter(|line| *line == SHARED_BOUNDARY_ANCHOR)
            .count(),
        1
    );
}

#[test]
fn o3_dependent_store_address_runtime_keeps_typed_terminal_ownership() {
    let repo = repo_root();
    let authorization =
        read(&repo.join("crates/rem6-cpu/src/riscv_fetch_ahead/memory_result_authorization.rs"));
    let authorizer = read(
        &repo.join("crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs"),
    );
    let pending = read(&repo.join("crates/rem6-cpu/src/o3_runtime_pending_address.rs"));
    let staging = read(&repo.join("crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs"));
    let queue = read(&repo.join("crates/rem6-cpu/src/o3_runtime_issue/queue.rs"));
    let fetch_tests = read(&repo.join(CPU_FETCH_TESTS));
    let pending_tests = read(&repo.join(CPU_PENDING_TESTS));

    for anchor in [
        "YoungerDependentEffect",
        "const fn dependent(\n        integer_destination: Option<Register>,",
        "role: if integer_destination.is_some()",
        "O3MemoryResultWindowRole::YoungerDependentEffect",
    ] {
        assert!(
            authorization.contains(anchor),
            "missing authorization {anchor}"
        );
    }
    for anchor in [
        "terminal_effect: bool",
        "self.dependent_rows == 0",
        "rs1 != rs2",
        "op: AtomicMemoryOp::Swap",
        "width: MemoryWidth::Doubleword",
        "O3MemoryResultWindowAuthorization::dependent(\n            destination,",
    ] {
        assert!(
            authorizer.contains(anchor),
            "missing terminal rule {anchor}"
        );
    }
    for anchor in [
        "destination: Option<O3RenameMapEntry>",
        "lsq_kind: O3LoadStoreQueueKind",
        "RiscvInstruction::Store {",
        "self.destination.is_none()",
    ] {
        assert!(
            pending.contains(anchor),
            "missing pending-row shape {anchor}"
        );
    }
    for anchor in [
        "O3LoadStoreQueueKind::Store",
        "O3LoadStoreQueueEntry::store(",
        "(None, None) => None",
        "self.pending_data_addresses.is_empty()",
        "let mut terminal_effect = false",
        "op: AtomicMemoryOp::Swap",
    ] {
        assert!(
            staging.contains(anchor),
            "missing staged-store shape {anchor}"
        );
    }
    assert!(queue.contains("PendingDataAddress(Option<O3RenameMapEntry>)"));
    for anchor in [
        "dependent_scalar_sd_authorizes_addressless_terminal_effect",
        "dependent_address_store_rejects_non_exact_widths",
        "dependent_address_atomic_head_accepts_only_unordered_amoswap_d",
    ] {
        assert!(fetch_tests.contains(anchor), "missing fetch test {anchor}");
    }
    for anchor in [
        "pending_terminal_store_rejects_a_younger_pending_row_atomically",
        "pending_store_alone_keeps_checkpoint_and_handoff_nonquiescent",
    ] {
        assert!(
            pending_tests.contains(anchor),
            "missing pending test {anchor}"
        );
    }
}

#[test]
fn o3_dependent_store_address_ledger_stays_honest() {
    let ledger = read(&repo_root().join("docs/architecture/gem5-to-rem6-migration.md"));
    assert!(ledger.contains("### CPU Execution Models - 74% representative"));
    for anchor in ANCHORS {
        assert!(ledger.contains(anchor), "ledger missing {anchor}");
    }
    assert!(ledger.contains(SHARED_BOUNDARY_ANCHOR));
    for open in [
        "dependent atomics and deeper or multiple dependent stores",
        "general load/store queue scheduler",
        "checkpoint-restorable live IQ/transport state",
    ] {
        assert!(
            ledger.contains(open),
            "ledger must retain open boundary {open}"
        );
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}
