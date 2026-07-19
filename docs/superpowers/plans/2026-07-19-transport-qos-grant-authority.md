# Transport QoS Grant Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the duplicated selected QoS grant stored in each transport activity while preserving arbitration behavior, JSON artifacts, and statistics.

**Architecture:** `FabricQosGrantActivity` will retain the candidate queue and selected queue index as its sole grant authority. The public `grant()` accessor will derive the selected request from that queue, and construction will enforce a valid selected index in every build profile. Existing transport and CLI tests will lock the derived invariant without changing the artifact schema or migration score.

**Tech Stack:** Rust workspace, `rem6-transport`, `rem6` CLI integration tests, Cargo test, repository source-policy tests.

---

### Task 1: Add the RED authority policy

**Files:**
- Modify: `crates/rem6-transport/tests/source_policy.rs`
- Test: `crates/rem6-transport/tests/source_policy.rs`

- [ ] **Step 1: Extend the focused QoS activity policy**

Inside `fabric_qos_activity_contracts_live_in_focused_module`, extract the `FabricQosGrantActivity` struct and `grant()` sections. Require exactly one `QosQueuedRequest` type occurrence in the struct, owned by `candidates`, so a renamed direct selected-request field also fails policy. Require `grant()` to consume both `self.candidates` and `self.selected_queue_index`.

```rust
let grant_activity = source_section(
    &qos_activity,
    "pub struct FabricQosGrantActivity {",
    "pub struct SharedFabricQosState {",
);
assert_eq!(
    grant_activity.matches("QosQueuedRequest").count(),
    1,
    "fabric QoS activity must not store a selected request beside its candidate queue"
);

let grant_accessor = source_section(
    &qos_activity,
    "pub fn grant(&self) -> &QosQueuedRequest {",
    "pub fn lrg_requestors_before",
);
for anchor in ["self.candidates", "self.selected_queue_index"] {
    assert!(
        grant_accessor.contains(anchor),
        "fabric QoS grant access must derive from `{anchor}`"
    );
}
```

Add a small local `source_section` helper because this source-policy file does not already provide one.

- [ ] **Step 2: Run the exact policy test and confirm RED**

Run:

```bash
cargo test -p rem6-transport --test source_policy fabric_qos_activity_contracts_live_in_focused_module -- --exact --nocapture
```

Expected: FAIL because `FabricQosGrantActivity` still contains a second direct `QosQueuedRequest` field and `grant()` returns `&self.grant`.

### Task 2: Make the candidate queue the sole grant authority

**Files:**
- Modify: `crates/rem6-transport/src/qos_activity.rs`
- Modify: `crates/rem6-transport/src/ordering.rs`
- Test: `crates/rem6-transport/tests/source_policy.rs`

- [ ] **Step 1: Remove the redundant activity field and constructor parameter**

Change `FabricQosGrantActivity` to store only `candidates` and `selected_queue_index` for selected-request ownership. In `new`, replace the debug-only equality check with an unconditional bounds assertion before moving `candidates` into the activity.

```rust
assert!(
    selected_queue_index < candidates.len(),
    "fabric QoS selected queue index must identify a candidate"
);
```

Delete the `grant` field, constructor argument, and initializer.

- [ ] **Step 2: Derive the grant accessor**

Replace the cached accessor with:

```rust
pub fn grant(&self) -> &QosQueuedRequest {
    &self.candidates[self.selected_queue_index]
}
```

- [ ] **Step 3: Remove call-site clones**

In both `transmit_qos_fabric_batch` and `transmit_ordered_qos_fabric_batch`, delete the local `selected = queue[grant.queue_index()].clone()` and stop passing that duplicate request into `FabricQosGrantActivity::new`.

- [ ] **Step 4: Run focused policy and transport tests**

Run:

```bash
cargo test -p rem6-transport --test source_policy fabric_qos_activity_contracts_live_in_focused_module -- --exact --nocapture
cargo test -p rem6-transport --test memory_transport transport_records_fifo_lifo_and_lrg_fabric_qos_grant_decisions -- --exact --nocapture
cargo test -p rem6-transport --test memory_ordering shared_fabric_qos_activity_records_memory_order_suppression -- --exact --nocapture
```

Expected: all PASS with unchanged arbitration order, selected indexes, and suppression behavior.

### Task 3: Lock derived behavior at transport and CLI boundaries

**Files:**
- Modify: `crates/rem6-transport/tests/memory_transport.rs`
- Modify: `crates/rem6-transport/tests/memory_ordering.rs`
- Modify: `crates/rem6/tests/cli_run/data_cache_multicore/fabric_qos.rs`

- [ ] **Step 1: Assert the transport activity invariant**

For each recorded activity, assert the public grant equals the indexed candidate:

```rust
assert!(activities.iter().all(|activity| {
    activity.grant() == &activity.candidates()[activity.selected_queue_index()]
}));
```

Add this to the FIFO/LIFO/LRG request matrix, the memory-order suppression test, and the multi-candidate response arbitration matrix so both constructors and both directions are covered.

- [ ] **Step 2: Assert the real CLI artifact invariant**

In `rem6_run_routes_multicore_two_hop_fabric_with_qos_queue_policy_matrix`, compare every emitted `grant` object against `candidates[selected_queue_index]` for `packet`, `request_id`, `requestor`, `priority`, `bytes`, and `order`. Do not compare candidate-only `queue_index`.

```rust
let selected_queue_index = activity["selected_queue_index"].as_u64().unwrap() as usize;
let selected = &activity["candidates"].as_array().unwrap()[selected_queue_index];
for field in ["packet", "request_id", "requestor", "priority", "bytes", "order"] {
    assert_eq!(activity["grant"][field], selected[field], "field {field}: {stdout}");
}
```

- [ ] **Step 3: Run the focused behavior matrix**

Run:

```bash
cargo test -p rem6-transport --test memory_transport transport_records_fifo_lifo_and_lrg_fabric_qos_grant_decisions -- --exact --nocapture
cargo test -p rem6-transport --test memory_ordering shared_fabric_qos_activity_records_memory_order_suppression -- --exact --nocapture
cargo test -p rem6-transport --test memory_transport transport_fabric_qos_arbitrates_same_tick_response_hops -- --exact --nocapture
cargo test -p rem6 --test cli_run data_cache_multicore::fabric_qos::rem6_run_routes_multicore_two_hop_fabric_with_qos_queue_policy_matrix -- --exact --nocapture
```

Expected: all PASS; the CLI JSON schema and request/response QoS matrices remain unchanged.

### Task 4: Verify, review, commit, and push

**Files:**
- Verify only: `docs/architecture/gem5-to-rem6-migration.md`
- Verify only: `temp/improve-rem6-0.md`

- [ ] **Step 1: Format and run package verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6-transport --all-targets
cargo test -p rem6 --test cli_run data_cache_multicore::fabric_qos::rem6_run_routes_multicore_two_hop_fabric_with_qos_queue_policy_matrix -- --exact --nocapture
```

Expected: all PASS.

- [ ] **Step 2: Run full workspace verification**

Run:

```bash
cargo test --workspace --all-targets -q
```

Expected: exit 0.

- [ ] **Step 3: Run final hygiene checks**

Run:

```bash
git diff --check
rg -n "grant: QosQueuedRequest|let selected = queue\[grant\.queue_index\(\)\]\.clone\(\)" crates/rem6-transport
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: no production duplicate-grant matches, the migration ledger remains exactly 1,200 lines and untouched, and `temp/**` has no staged or tracked changes.

- [ ] **Step 4: Request independent read-only review**

Ask reviewers to inspect authority correctness, constructor invariants, public JSON/stats compatibility, dead code, and test sufficiency. Address actionable findings and rerun affected tests.

- [ ] **Step 5: Commit and push**

```bash
git add docs/superpowers/plans/2026-07-19-transport-qos-grant-authority.md crates/rem6-transport crates/rem6/tests/cli_run/data_cache_multicore/fabric_qos.rs
git commit -m "refactor: derive transport QoS grant activity"
git push origin main
```

Verify `git status --short --branch` is clean and `git rev-parse HEAD` equals `git rev-parse origin/main`.
