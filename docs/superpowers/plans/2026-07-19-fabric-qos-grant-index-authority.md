# Fabric QoS Grant Index Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove copied request metadata from `QosGrant` so the candidate queue and selected index are the sole arbitration authority.

**Architecture:** `QosGrant` will contain only `queue_index` and expose only `queue_index()`. Fabric and transport tests will resolve request metadata from their original queues, while existing runtime callers and CLI grant activities keep their current index-driven behavior and artifact schema.

**Tech Stack:** Rust workspace, `syn` source-policy parsing, `rem6-fabric`, `rem6-transport`, `rem6-dram`, and real `rem6` CLI tests.

---

### Task 1: Add the RED index-only source policy

**Files:**
- Modify: `crates/rem6-fabric/tests/source_policy.rs`
- Test: `crates/rem6-fabric/tests/source_policy.rs`

- [ ] **Step 1: Import the structured syntax nodes**

Extend the existing `syn` import:

```rust
use syn::{Fields, ImplItem, Item, Type, UseTree, Visibility};
```

- [ ] **Step 2: Add the index-only policy test**

Add:

```rust
#[test]
fn qos_grant_keeps_only_selected_queue_index() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_dir.join("src/qos.rs")).unwrap();
    let syntax = syn::parse_file(&source).unwrap();

    let fields = syntax
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == "QosGrant" => Some(&item.fields),
            _ => None,
        })
        .expect("src/qos.rs must define QosGrant");
    let Fields::Named(fields) = fields else {
        panic!("QosGrant must remain a named-field struct");
    };
    let field_names = fields
        .named
        .iter()
        .map(|field| field.ident.as_ref().unwrap().to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        field_names,
        ["queue_index".to_string()].into_iter().collect(),
        "QosGrant must not cache metadata already owned by its candidate queue"
    );

    let mut public_methods = BTreeSet::new();
    for item in &syntax.items {
        let Item::Impl(item_impl) = item else {
            continue;
        };
        let Type::Path(self_ty) = item_impl.self_ty.as_ref() else {
            continue;
        };
        if !self_ty.path.is_ident("QosGrant") {
            continue;
        }
        for item in &item_impl.items {
            if let ImplItem::Fn(method) = item {
                if matches!(method.vis, Visibility::Public(_)) {
                    public_methods.insert(method.sig.ident.to_string());
                }
            }
        }
    }
    assert_eq!(
        public_methods,
        ["queue_index".to_string()].into_iter().collect(),
        "QosGrant public access must expose only the selected queue position"
    );
}
```

- [ ] **Step 3: Run the exact policy test and confirm RED**

Run:

```bash
cargo test -p rem6-fabric --test source_policy qos_grant_keeps_only_selected_queue_index -- --exact --nocapture
```

Expected: FAIL because `QosGrant` still stores `request_id`, `requestor`,
`priority`, and `bytes` and exposes their public accessors.

### Task 2: Make arbitration tests consume the queue authority

**Files:**
- Modify: `crates/rem6-fabric/tests/qos_arbitration.rs:207-284`
- Modify: `crates/rem6-transport/tests/memory_transport.rs:1446-1453,2024-2033`
- Test: the same files

- [ ] **Step 1: Update FIFO and LIFO assertions**

After each queue-index assertion, resolve the selected row:

```rust
let selected = &queue[grant.queue_index()];
assert_eq!(selected.request_id(), QosRequestId::new(2));
assert_eq!(selected.priority(), QosPriority::new(0));
```

Use request `3` for the LIFO row.

- [ ] **Step 2: Update LRG assertions**

For `first`, `second`, and `third`, replace grant metadata access with the
corresponding queue row:

```rust
let selected = &first_queue[first.queue_index()];
assert_eq!(selected.requestor(), QosRequestorId::new(1));
assert_eq!(selected.request_id(), QosRequestId::new(10));
```

Apply the same pattern to `second_queue` and `third_queue` with the existing
expected values.

- [ ] **Step 3: Update transport LRG priming assertions**

Replace the direct requestor accessor in
`transport_parallel_batch_uses_explicit_qos_requestor_for_shared_fabric_lrg`
with:

```rust
let grant = arbiter.grant(&priming).unwrap();
assert_eq!(
    priming[grant.queue_index()].requestor(),
    QosRequestorId::new(200)
);
```

In `response_qos_arbiter_starts_without_request_lrg_history`, name the priming
queue and resolve the selected requestor the same way:

```rust
let priming = [
    qos_request(1, 1, 0, 8, 0),
    qos_request(2, 2, 0, 8, 1),
];
let grant = arbiter.grant(&priming).unwrap();
assert_eq!(
    priming[grant.queue_index()].requestor(),
    QosRequestorId::new(1)
);
```

- [ ] **Step 4: Verify the rewritten tests still pass before production changes**

Run:

```bash
cargo test -p rem6-fabric --test qos_arbitration qos_queue_arbiter -- --nocapture
cargo test -p rem6-transport --test memory_transport transport_parallel_batch_uses_explicit_qos_requestor_for_shared_fabric_lrg -- --exact --nocapture
cargo test -p rem6-transport --test memory_transport response_qos_arbiter_starts_without_request_lrg_history -- --exact --nocapture
```

Expected: all PASS while the old accessors still exist, proving the tests no
longer depend on copied grant metadata.

### Task 3: Collapse `QosGrant` to one field

**Files:**
- Modify: `crates/rem6-fabric/src/qos.rs:474-512,638-689`
- Test: `crates/rem6-fabric/tests/source_policy.rs`
- Test: `crates/rem6-fabric/tests/qos_arbitration.rs`

- [ ] **Step 1: Replace the grant representation**

Use:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QosGrant {
    queue_index: usize,
}

impl QosGrant {
    const fn new(queue_index: usize) -> Self {
        Self { queue_index }
    }

    pub const fn queue_index(&self) -> usize {
        self.queue_index
    }
}
```

Delete `request_id`, `requestor`, `priority`, and `bytes`, their accessors, and
`from_request`.

- [ ] **Step 2: Return index-only grants from every policy**

In `lrg_grant`, replace the copied request constructor with:

```rust
let grant = QosGrant::new(*index);
```

In `fifo_grant` and `lifo_grant`, replace the final maps with:

```rust
.map(|(index, _)| QosGrant::new(index))
```

- [ ] **Step 3: Run policy and focused arbitration tests**

Run:

```bash
cargo test -p rem6-fabric --test source_policy qos_grant_keeps_only_selected_queue_index -- --exact --nocapture
cargo test -p rem6-fabric --test qos_arbitration -- --nocapture
cargo test -p rem6-transport --test memory_transport transport_parallel_batch_uses_explicit_qos_requestor_for_shared_fabric_lrg -- --exact --nocapture
cargo test -p rem6-transport --test memory_transport response_qos_arbiter_starts_without_request_lrg_history -- --exact --nocapture
```

Expected: all PASS and `rg -n "grant\.(request_id|requestor|priority|bytes)" crates/rem6-fabric crates/rem6-transport crates/rem6-dram` returns no matches.

### Task 4: Verify cross-crate and real CLI behavior

**Files:**
- Verify only: `crates/rem6-transport/src/ordering.rs`
- Verify only: `crates/rem6-transport/src/parallel_qos.rs`
- Verify only: `crates/rem6-dram/src/qos.rs`
- Verify only: `crates/rem6/tests/cli_run/data_cache_multicore/fabric_qos.rs`

- [ ] **Step 1: Run affected crate suites**

Run:

```bash
cargo test -p rem6-fabric --all-targets
cargo test -p rem6-transport --all-targets
cargo test -p rem6-dram --all-targets
```

Expected: all PASS, preserving FIFO/LIFO/LRG selection, ordered suppression,
parallel request/response arbitration, and DRAM pending-queue removal.

- [ ] **Step 2: Run the representative CLI matrix**

Run:

```bash
cargo test -p rem6 --test cli_run data_cache_multicore::fabric_qos::rem6_run_routes_multicore_two_hop_fabric_with_qos_queue_policy_matrix -- --exact --nocapture
```

Expected: PASS with every request/response grant artifact equal to
`candidates[selected_queue_index]` for FIFO, LIFO, and LRG.

### Task 5: Verify, review, commit, and push

**Files:**
- Verify only: `docs/architecture/gem5-to-rem6-migration.md`
- Verify only: `temp/improve-rem6-0.md`

- [ ] **Step 1: Run final verification**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets -q
git diff --check
```

Expected: every command exits 0.

- [ ] **Step 2: Run hygiene checks**

Run:

```bash
rg -n "grant\.(request_id|requestor|priority|bytes)" crates/rem6-fabric crates/rem6-transport crates/rem6-dram
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: no removed-accessor matches, the ledger remains exactly 1,200 lines,
and protected paths are untouched.

- [ ] **Step 3: Request independent read-only review**

Reviewers must check arbitration equivalence, public API scope, queue/index
authority, transport and DRAM callers, CLI artifact compatibility, dead code,
and ledger honesty. Address actionable findings and rerun affected tests.

- [ ] **Step 4: Commit and push**

```bash
git add docs/superpowers/specs/2026-07-19-fabric-qos-grant-index-authority-design.md docs/superpowers/plans/2026-07-19-fabric-qos-grant-index-authority.md crates/rem6-fabric crates/rem6-transport/tests/memory_transport.rs
git commit -m "refactor: reduce fabric QoS grants to indexes"
git push origin main
```

Verify `git status --short --branch` is clean and `git rev-parse HEAD` equals
`git rev-parse origin/main`.
