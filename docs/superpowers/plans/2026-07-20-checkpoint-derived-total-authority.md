# Checkpoint Derived-Total Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make checkpoint, execution-mode transfer, and top-level host-action hierarchy totals derive from their owned component and chunk projections while preserving every CLI schema and value.

**Architecture:** Payload size remains authoritative only on chunk summaries. Component totals derive from chunks, manifest or transfer totals derive from components, and top-level restore aggregates derive from restore summaries. Quiescence capture snapshots and `rem6-workload` aggregate-only expectation types remain stored because they have independent snapshot or requirement semantics.

**Tech Stack:** Rust 2021, Cargo workspace tests, source-policy integration tests, JSON/text/stats/debug CLI integration tests.

---

## File Map

- `crates/rem6-checkpoint/src/lib.rs`: checkpoint summary representation and accessors.
- `crates/rem6-checkpoint/tests/checkpoint_registry.rs`: projection-total behavior.
- `crates/rem6-checkpoint/tests/source_policy.rs`: checkpoint authority shape.
- `crates/rem6-system/src/host.rs`: mode-switch transfer representation and accessors.
- `crates/rem6-system/src/host/execution_mode_transfer.rs`: transfer construction and quiescence snapshot capture.
- `crates/rem6-system/tests/system_actions.rs`: public transfer projection behavior.
- `crates/rem6-system/tests/source_policy.rs`: transfer authority shape.
- `crates/rem6/src/host_actions.rs`: top-level checkpoint, transfer, and restore aggregate summaries.
- `crates/rem6/src/host_actions/summary_totals.rs`: derived hierarchy and restore-total accessors.
- `crates/rem6/src/host_actions/summary_projection_tests.rs`: focused top-level projection tests.
- `crates/rem6/src/artifact_json/checkpoint.rs`: checkpoint JSON consumer.
- `crates/rem6/src/artifact_json.rs`: host-action and transfer JSON consumers.
- `crates/rem6/src/host_actions/transfer_stats.rs`: transfer stats aggregation consumer.
- `crates/rem6/src/stats_output/host_actions.rs`: host-action stats consumer.
- `crates/rem6/src/stats_output/o3_runtime_snapshot_restore.rs`: restore projection consumer.
- `crates/rem6/src/debug_output/checkpoint_components_json.rs`: component debug JSON consumer.
- `crates/rem6/src/debug_output/host_action.rs`: host-action debug consumer and fixtures.
- `crates/rem6/src/debug_output/o3_checkpoint_restore_json.rs`: O3 restore debug consumer and fixtures.
- `crates/rem6/src/multi_run_cli.rs`: child-run checkpoint aggregate consumer.
- `crates/rem6/src/stats_output/host_actions/tests.rs`: host-action stats fixtures.
- `crates/rem6/tests/source_policy.rs`: focused source-policy module registration.
- `crates/rem6/tests/source_policy/checkpoint_total_authority.rs`: top-level authority shape.

### Task 1: Derive `rem6-checkpoint` summary totals

**Files:**
- Modify: `crates/rem6-checkpoint/src/lib.rs:90-195`
- Modify: `crates/rem6-checkpoint/tests/checkpoint_registry.rs:70-160`
- Create: `crates/rem6-checkpoint/tests/source_policy.rs`

- [x] **Step 1: Add the failing checkpoint source-policy test**

Create `crates/rem6-checkpoint/tests/source_policy.rs` with a small struct-body
extractor and this policy:

```rust
use std::fs;
use std::path::Path;

#[test]
fn checkpoint_summaries_derive_hierarchy_totals_from_projections() {
    let source = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs"),
    )
    .unwrap();
    let component = struct_body(&source, "CheckpointComponentSummary");
    let manifest = struct_body(&source, "CheckpointManifestSummary");

    for forbidden in ["chunk_count:", "payload_bytes:"] {
        assert!(!component.contains(forbidden), "component summary caches {forbidden}");
        assert!(!manifest.contains(forbidden), "manifest summary caches {forbidden}");
    }
    assert!(
        !source.contains("pub fn new(component: CheckpointComponentId, chunk_count:"),
        "aggregate-only checkpoint component summaries must stay retired"
    );
    assert!(source.contains("self.chunk_summaries.len()"));
    assert!(source.contains(".map(CheckpointChunkSummary::payload_bytes)"));
    assert!(source.contains(".map(CheckpointComponentSummary::chunk_count)"));
    assert!(source.contains(".map(CheckpointComponentSummary::payload_bytes)"));
}

fn struct_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("pub struct {name} {{"))
        .unwrap_or_else(|| panic!("missing struct {name}"));
    let body = &source[start..];
    let end = body.find("\n}").expect("struct must close");
    &body[..end]
}
```

- [x] **Step 2: Run the policy test and observe RED**

Run:

```bash
cargo test -p rem6-checkpoint --test source_policy checkpoint_summaries_derive_hierarchy_totals_from_projections -- --exact
```

Expected: FAIL because both summary structs still declare cached totals and the
aggregate-only constructor still exists.

- [x] **Step 3: Strengthen the behavior test over the owned projection**

In `checkpoint_manifest_reports_component_chunk_and_payload_totals`, add:

```rust
assert_eq!(
    summary.chunk_count(),
    summary
        .component_summaries()
        .iter()
        .map(CheckpointComponentSummary::chunk_count)
        .sum()
);
assert_eq!(
    summary.payload_bytes(),
    summary
        .component_summaries()
        .iter()
        .map(CheckpointComponentSummary::payload_bytes)
        .sum()
);
for component in summary.component_summaries() {
    assert_eq!(component.chunk_count(), component.chunk_summaries().len());
    assert_eq!(
        component.payload_bytes(),
        component
            .chunk_summaries()
            .iter()
            .map(CheckpointChunkSummary::payload_bytes)
            .sum()
    );
}
```

- [x] **Step 4: Remove cached checkpoint totals**

Change the two representations and accessors to:

```rust
pub struct CheckpointComponentSummary {
    component: CheckpointComponentId,
    chunk_summaries: Vec<CheckpointChunkSummary>,
}

impl CheckpointComponentSummary {
    pub fn with_chunk_summaries(
        component: CheckpointComponentId,
        chunk_summaries: Vec<CheckpointChunkSummary>,
    ) -> Self {
        Self {
            component,
            chunk_summaries,
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.chunk_summaries.len()
    }

    pub fn payload_bytes(&self) -> usize {
        self.chunk_summaries
            .iter()
            .map(CheckpointChunkSummary::payload_bytes)
            .sum()
    }
}

pub struct CheckpointManifestSummary {
    component_summaries: Vec<CheckpointComponentSummary>,
}

impl CheckpointManifestSummary {
    pub fn new(component_summaries: Vec<CheckpointComponentSummary>) -> Self {
        Self {
            component_summaries,
        }
    }

    pub fn chunk_count(&self) -> usize {
        self.component_summaries
            .iter()
            .map(CheckpointComponentSummary::chunk_count)
            .sum()
    }

    pub fn payload_bytes(&self) -> usize {
        self.component_summaries
            .iter()
            .map(CheckpointComponentSummary::payload_bytes)
            .sum()
    }
}
```

Delete `CheckpointComponentSummary::new` entirely. Keep component lookup,
component count, chunk lookup, and manifest construction behavior unchanged.

- [x] **Step 5: Run checkpoint GREEN verification**

Run:

```bash
cargo test -p rem6-checkpoint --test source_policy
cargo test -p rem6-checkpoint --test checkpoint_registry
cargo test -p rem6-checkpoint --all-targets
```

Expected: all tests PASS.

- [x] **Step 6: Commit the checkpoint layer**

```bash
git add crates/rem6-checkpoint/src/lib.rs crates/rem6-checkpoint/tests/checkpoint_registry.rs crates/rem6-checkpoint/tests/source_policy.rs
git diff --cached --check
git commit -m "refactor: derive checkpoint manifest totals"
```

### Task 2: Derive `rem6-system` transfer totals

**Files:**
- Modify: `crates/rem6-system/src/host.rs:53-198`
- Modify: `crates/rem6-system/src/host/execution_mode_transfer.rs:28-60`
- Modify: `crates/rem6-system/tests/system_actions.rs:1947-2003`
- Modify: `crates/rem6-system/tests/source_policy.rs`

- [x] **Step 1: Add the failing transfer source-policy row**

Add a local `struct_body` helper to `crates/rem6-system/tests/source_policy.rs`
if one is not already present, then add:

```rust
#[test]
fn execution_mode_switch_transfers_derive_hierarchy_totals_from_components() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = fs::read_to_string(crate_dir.join("src/host.rs")).unwrap();
    let transfer = struct_body(&host, "ExecutionModeSwitchStateTransfer");
    let component = struct_body(&host, "ExecutionModeSwitchStateTransferComponent");

    for forbidden in ["component_count:", "chunk_count:", "payload_bytes:"] {
        assert!(!transfer.contains(forbidden), "transfer caches {forbidden}");
    }
    for forbidden in ["chunk_count:", "payload_bytes:"] {
        assert!(!component.contains(forbidden), "component caches {forbidden}");
    }
    assert!(host.contains("self.components.len() as u64"));
    assert!(host.contains(".map(ExecutionModeSwitchStateTransferComponent::chunk_count)"));
    assert!(host.contains(".map(ExecutionModeSwitchStateTransferComponent::payload_bytes)"));
    assert!(host.contains("self.chunks.len() as u64"));
    assert!(host.contains(".map(ExecutionModeSwitchStateTransferChunk::payload_bytes)"));

    let constructor = fs::read_to_string(
        crate_dir.join("src/host/execution_mode_transfer.rs"),
    )
    .unwrap();
    assert!(constructor.contains("let captured_component_count = components.len() as u64;"));
    assert!(constructor.contains("captured_component_count,"));
    assert!(!constructor.contains("component_count: summary.component_count()"));
}
```

- [x] **Step 2: Run the policy test and observe RED**

Run:

```bash
cargo test -p rem6-system --test source_policy execution_mode_switch_transfers_derive_hierarchy_totals_from_components -- --exact
```

Expected: FAIL because the transfer structs still cache hierarchy totals.

- [x] **Step 3: Add public behavior assertions**

Extend `execution_mode_switch_transfer_can_be_scheduler_only` after obtaining
`transfer`:

```rust
assert_eq!(transfer.component_count(), transfer.components().len() as u64);
assert_eq!(
    transfer.chunk_count(),
    transfer.components().iter().map(|component| component.chunk_count()).sum()
);
assert_eq!(
    transfer.payload_bytes(),
    transfer
        .components()
        .iter()
        .map(|component| component.payload_bytes())
        .sum()
);
for component in transfer.components() {
    assert_eq!(component.chunk_count(), component.chunks().len() as u64);
    assert_eq!(
        component.payload_bytes(),
        component.chunks().iter().map(|chunk| chunk.payload_bytes()).sum()
    );
}
assert_eq!(
    transfer.quiescence_gate().captured_component_count(),
    transfer.component_count()
);
assert_eq!(
    transfer.quiescence_gate().captured_chunk_count(),
    transfer.chunk_count()
);
assert_eq!(
    transfer.quiescence_gate().captured_payload_bytes(),
    transfer.payload_bytes()
);
```

- [x] **Step 4: Remove cached transfer totals**

In `host.rs`, remove the five cached hierarchy fields and implement:

```rust
pub fn component_count(&self) -> u64 {
    self.components.len() as u64
}

pub fn chunk_count(&self) -> u64 {
    self.components
        .iter()
        .map(ExecutionModeSwitchStateTransferComponent::chunk_count)
        .sum()
}

pub fn payload_bytes(&self) -> u64 {
    self.components
        .iter()
        .map(ExecutionModeSwitchStateTransferComponent::payload_bytes)
        .sum()
}

pub fn chunk_count(&self) -> u64 {
    self.chunks.len() as u64
}

pub fn payload_bytes(&self) -> u64 {
    self.chunks
        .iter()
        .map(ExecutionModeSwitchStateTransferChunk::payload_bytes)
        .sum()
}
```

The first three methods belong to `ExecutionModeSwitchStateTransfer`; the last
two belong to `ExecutionModeSwitchStateTransferComponent`. Remove the now-dead
precomputed component payload local from `from_state`.

- [x] **Step 5: Build the quiescence snapshot from the component projection**

In `execution_mode_transfer.rs`, remove `let summary = manifest.summary()` and
compute before constructing `Self`:

```rust
let captured_component_count = components.len() as u64;
let captured_chunk_count = components
    .iter()
    .map(ExecutionModeSwitchStateTransferComponent::chunk_count)
    .sum();
let captured_payload_bytes = components
    .iter()
    .map(ExecutionModeSwitchStateTransferComponent::payload_bytes)
    .sum();
```

Use those three locals only in `ExecutionModeSwitchQuiescenceGate`. Do not add
them back to `ExecutionModeSwitchStateTransfer`.

- [x] **Step 6: Run system GREEN verification**

Run:

```bash
cargo test -p rem6-system --test source_policy
cargo test -p rem6-system --test system_actions execution_mode_switch_transfer_can_be_scheduler_only -- --exact
cargo test -p rem6-system --all-targets
```

Expected: all tests PASS.

- [x] **Step 7: Commit the system layer**

```bash
git add crates/rem6-system/src/host.rs crates/rem6-system/src/host/execution_mode_transfer.rs crates/rem6-system/tests/system_actions.rs crates/rem6-system/tests/source_policy.rs
git diff --cached --check
git commit -m "refactor: derive mode switch transfer totals"
```

### Task 3: Derive top-level checkpoint and restore totals

**Files:**
- Modify: `crates/rem6/src/host_actions.rs`
- Create: `crates/rem6/src/host_actions/summary_totals.rs`
- Create: `crates/rem6/src/host_actions/summary_projection_tests.rs`
- Modify: `crates/rem6/src/artifact_json/checkpoint.rs`
- Modify: `crates/rem6/src/artifact_json.rs`
- Modify: `crates/rem6/src/host_actions/transfer_stats.rs`
- Modify: `crates/rem6/src/stats_output/host_actions.rs`
- Modify: `crates/rem6/src/stats_output/o3_runtime_snapshot_restore.rs`
- Modify: `crates/rem6/src/debug_output/checkpoint_components_json.rs`
- Modify: `crates/rem6/src/debug_output/host_action.rs`
- Modify: `crates/rem6/src/debug_output/o3_checkpoint_restore_json.rs`
- Modify: `crates/rem6/src/multi_run_cli.rs`
- Modify: `crates/rem6/src/stats_output/host_actions/tests.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Create: `crates/rem6/tests/source_policy/checkpoint_total_authority.rs`

- [x] **Step 1: Add the failing top-level source-policy module**

Register this module in `crates/rem6/tests/source_policy.rs`:

```rust
#[path = "source_policy/checkpoint_total_authority.rs"]
mod checkpoint_total_authority;
```

Create `checkpoint_total_authority.rs` with a `struct_body` helper and this
test:

```rust
use std::fs;
use std::path::Path;

#[test]
fn checkpoint_output_summaries_derive_hierarchy_totals_from_projections() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = fs::read_to_string(crate_dir.join("src/host_actions.rs")).unwrap();
    let totals = fs::read_to_string(
        crate_dir.join("src/host_actions/summary_totals.rs"),
    )
    .unwrap();

    for (name, forbidden) in [
        ("Rem6HostCheckpointSummary", &["component_count:", "chunk_count:", "payload_bytes:"][..]),
        ("Rem6HostCheckpointComponentSummary", &["chunk_count:", "payload_bytes:"][..]),
        ("Rem6ExecutionModeStateTransferSummary", &["component_count:", "chunk_count:", "payload_bytes:"][..]),
        ("Rem6HostActionSummary", &[
            "checkpoint_restored_count:",
            "checkpoint_restored_component_count:",
            "checkpoint_restored_chunk_count:",
            "checkpoint_restored_payload_bytes:",
        ][..]),
    ] {
        let body = struct_body(&host, name);
        for field in forbidden {
            assert!(!body.contains(field), "{name} caches {field}");
        }
    }

    for authority in [
        "pub(crate) const fn checkpoint_restored_count(&self) -> u64",
        "pub(crate) fn checkpoint_restored_component_count(&self) -> u64",
        "pub(crate) fn checkpoint_restored_chunk_count(&self) -> u64",
        "pub(crate) fn checkpoint_restored_payload_bytes(&self) -> u64",
        "pub(crate) const fn component_count(&self) -> u64",
        "pub(crate) const fn chunk_count(&self) -> u64",
        "pub(crate) fn chunk_count(&self) -> u64",
        "pub(crate) fn payload_bytes(&self) -> u64",
    ] {
        assert!(totals.contains(authority), "missing derived accessor {authority}");
    }

    assert!(host.contains("mod summary_totals;"));

    let checkpoint_json = fs::read_to_string(
        crate_dir.join("src/artifact_json/checkpoint.rs"),
    )
    .unwrap();
    assert!(checkpoint_json.contains("self.component_count()"));
    assert!(checkpoint_json.contains("self.chunk_count()"));
    assert!(checkpoint_json.contains("self.payload_bytes()"));
}

fn struct_body<'a>(source: &'a str, name: &str) -> &'a str {
    let start = source
        .find(&format!("struct {name} {{"))
        .unwrap_or_else(|| panic!("missing struct {name}"));
    let body = &source[start..];
    let end = body.find("\n}").expect("struct must close");
    &body[..end]
}
```

- [x] **Step 2: Run the policy test and observe RED**

Run:

```bash
cargo test -p rem6 --test source_policy checkpoint_output_summaries_derive_hierarchy_totals_from_projections -- --exact
```

Expected: FAIL because all four top-level summary types still cache totals.

- [x] **Step 3: Add a focused top-level projection test**

Declare the test module near the existing `host_actions` submodules:

```rust
#[cfg(test)]
#[path = "host_actions/summary_projection_tests.rs"]
mod summary_projection_tests;
```

In the new file, add the complete focused fixture and test:

```rust
use super::*;

fn chunk(name: &str, payload_bytes: u64) -> Rem6HostCheckpointChunkSummary {
    Rem6HostCheckpointChunkSummary {
        name: name.to_string(),
        payload_bytes,
        payload_checksum: payload_bytes,
        o3_runtime: None,
        o3_live_data_handoff: None,
    }
}

fn component(
    name: &str,
    chunks: Vec<Rem6HostCheckpointChunkSummary>,
) -> Rem6HostCheckpointComponentSummary {
    Rem6HostCheckpointComponentSummary {
        component: name.to_string(),
        chunks,
    }
}

fn checkpoint(
    label: &str,
    components: Vec<Rem6HostCheckpointComponentSummary>,
) -> Rem6HostCheckpointSummary {
    Rem6HostCheckpointSummary {
        tick: 10,
        event: 1,
        source: 2,
        label: label.to_string(),
        manifest_tick: 9,
        execution_mode_authority_present: false,
        execution_mode_authority_cleared: false,
        execution_mode_authority_decode_error: false,
        execution_modes: Vec::new(),
        components,
    }
}

#[test]
fn checkpoint_and_transfer_totals_follow_owned_projections() {
    let first = component("cpu0", vec![chunk("pc", 3), chunk("regs", 5)]);
    let second = component("memory0", vec![chunk("lines", 7)]);
    let captured = checkpoint("captured", vec![first.clone()]);
    let restored = checkpoint("restored", vec![second.clone()]);
    let transfer = Rem6ExecutionModeStateTransferSummary {
        manifest_label: "execution-mode-switch-1".to_string(),
        manifest_tick: 11,
        restorable: true,
        live_data_handoff: false,
        writeback_width: None,
        reserved_future_completions: None,
        earliest_unpublished_writeback_tick: None,
        quiescence_gate: Rem6ExecutionModeQuiescenceGateSummary {
            validated: true,
            target: "cpu0".to_string(),
            captured_component_count: 1,
            captured_chunk_count: 1,
            captured_payload_bytes: 7,
            checker: None,
        },
        components: vec![second],
    };
    let actions = Rem6HostActionSummary {
        checkpoint_restores: vec![captured.clone(), restored],
        ..Rem6HostActionSummary::default()
    };

    assert_eq!(first.chunk_count(), first.chunks.len() as u64);
    assert_eq!(first.payload_bytes(), 8);
    assert_eq!(captured.component_count(), captured.components.len() as u64);
    assert_eq!(captured.chunk_count(), 2);
    assert_eq!(captured.payload_bytes(), 8);
    assert_eq!(transfer.component_count(), transfer.components.len() as u64);
    assert_eq!(transfer.chunk_count(), 1);
    assert_eq!(transfer.payload_bytes(), 7);
    assert_eq!(actions.checkpoint_restored_count(), 2);
    assert_eq!(actions.checkpoint_restored_component_count(), 2);
    assert_eq!(actions.checkpoint_restored_chunk_count(), 3);
    assert_eq!(actions.checkpoint_restored_payload_bytes(), 15);
}
```

The optional O3 fields stay `None`, execution-mode vectors stay empty, and
authority flags stay `false`; no production constructor is needed for this
private test.

- [x] **Step 4: Remove top-level cached totals and add accessors**

In `host_actions.rs`:

1. Remove the four `checkpoint_restored_*` fields and their increments from
   `Rem6HostActionSummary::from_outcomes`.
2. Remove hierarchy totals from `Rem6HostCheckpointSummary`,
   `Rem6HostCheckpointComponentSummary`, and
   `Rem6ExecutionModeStateTransferSummary`.
3. Stop copying totals in `checkpoint_summary_from_manifest`,
   `from_checkpoint_state`, `from_execution_mode_transfer_component`, and
   `Rem6ExecutionModeStateTransferSummary::from_transfer`.
4. Declare `mod summary_totals;` and add these projection methods in the
   focused `host_actions/summary_totals.rs` module so `host_actions.rs` remains
   under its enforced 1,200-line cap:

```rust
use super::*;

impl Rem6HostActionSummary {
    pub(crate) const fn checkpoint_restored_count(&self) -> u64 {
        self.checkpoint_restores.len() as u64
    }

    pub(crate) fn checkpoint_restored_component_count(&self) -> u64 {
        self.checkpoint_restores
            .iter()
            .map(Rem6HostCheckpointSummary::component_count)
            .sum()
    }

    pub(crate) fn checkpoint_restored_chunk_count(&self) -> u64 {
        self.checkpoint_restores
            .iter()
            .map(Rem6HostCheckpointSummary::chunk_count)
            .sum()
    }

    pub(crate) fn checkpoint_restored_payload_bytes(&self) -> u64 {
        self.checkpoint_restores
            .iter()
            .map(Rem6HostCheckpointSummary::payload_bytes)
            .sum()
    }
}

impl Rem6HostCheckpointSummary {
    pub(crate) const fn component_count(&self) -> u64 {
        self.components.len() as u64
    }

    pub(crate) fn chunk_count(&self) -> u64 {
        self.components
            .iter()
            .map(Rem6HostCheckpointComponentSummary::chunk_count)
            .sum()
    }

    pub(crate) fn payload_bytes(&self) -> u64 {
        self.components
            .iter()
            .map(Rem6HostCheckpointComponentSummary::payload_bytes)
            .sum()
    }
}

impl Rem6HostCheckpointComponentSummary {
    pub(crate) const fn chunk_count(&self) -> u64 {
        self.chunks.len() as u64
    }

    pub(crate) fn payload_bytes(&self) -> u64 {
        self.chunks
            .iter()
            .map(|chunk| chunk.payload_bytes)
            .sum()
    }
}
```

Give `Rem6ExecutionModeStateTransferSummary` the same three accessors as
`Rem6HostCheckpointSummary`, with a const component count and sums projected
through `Rem6HostCheckpointComponentSummary`.

- [x] **Step 5: Migrate every output consumer and fixture**

Replace direct hierarchy reads with method calls. The mechanical mapping is:

```text
summary.checkpoint_restored_count              -> summary.checkpoint_restored_count()
summary.checkpoint_restored_component_count    -> summary.checkpoint_restored_component_count()
summary.checkpoint_restored_chunk_count        -> summary.checkpoint_restored_chunk_count()
summary.checkpoint_restored_payload_bytes      -> summary.checkpoint_restored_payload_bytes()
checkpoint.component_count                    -> checkpoint.component_count()
checkpoint.chunk_count                        -> checkpoint.chunk_count()
checkpoint.payload_bytes                      -> checkpoint.payload_bytes()
transfer.component_count                      -> transfer.component_count()
transfer.chunk_count                          -> transfer.chunk_count()
transfer.payload_bytes                        -> transfer.payload_bytes()
component.chunk_count                         -> component.chunk_count()
component.payload_bytes                       -> component.payload_bytes()
```

Apply the mapping only when the receiver is one of the changed summary types;
do not alter independent aggregation structs that intentionally store totals.
Delete removed fields from all private test fixtures. Keep JSON format strings,
field order, stats paths, units, and debug names byte-for-byte unchanged.

- [x] **Step 6: Run focused top-level GREEN verification**

Run:

```bash
cargo test -p rem6 --test source_policy checkpoint_output_summaries_derive_hierarchy_totals_from_projections -- --exact
cargo test -p rem6 host_actions::summary_projection_tests::checkpoint_and_transfer_totals_follow_owned_projections -- --exact
cargo test -p rem6 stats_output::host_actions::tests
cargo test -p rem6 debug_output::host_action::tests
cargo test -p rem6 debug_output::o3_checkpoint_restore_json::tests
cargo test -p rem6 --lib
```

Expected: all tests PASS. If the compiler reports a removed field, migrate that
receiver only after confirming it is one of the changed projection summaries.

- [x] **Step 7: Commit the top-level layer**

```bash
git add crates/rem6/src crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/checkpoint_total_authority.rs
git diff --cached --check
git commit -m "refactor: derive host checkpoint totals"
```

### Task 4: Prove schema compatibility through real CLI surfaces

**Files:**
- Test only unless a regression is found.

- [x] **Step 1: Run the mode-switch stats row**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::rem6_run_scopes_multicore_o3_switch_transfer_stats_by_target -- --exact
```

Expected: PASS with the existing target/component/chunk state-transfer stats.

- [x] **Step 2: Run the quiescence debug row**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run debug_flags::host_action::rem6_run_host_action_debug_flag_emits_checker_quiescence_switch_scope -- --exact
```

Expected: PASS, proving stored quiescence capture fields remain unchanged.

- [x] **Step 3: Run the checkpoint restore debug row**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run debug_flags::o3_checkpoint_restore::rem6_run_o3_debug_flag_marks_checkpoint_restore_replay_scope -- --exact
```

Expected: PASS with existing restore totals and replay markers.

- [x] **Step 4: Run checkpoint aggregate JSON rows**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run multi_run::rem6_multi_run_reports_run_child_checkpoint_actions -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run multi_run::rem6_multi_run_reports_trace_replay_child_checkpoint_actions -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run m5_host_actions::o3::restore::rem6_run_host_action_trace_restores_multicore_o3_checkpoint_components_by_active_hart -- --exact
```

Expected: PASS with unchanged JSON keys, values, and component/chunk evidence.

- [x] **Step 5: Run affected-crate and workspace verification**

```bash
cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-checkpoint --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
TMPDIR=$PWD/target/tmp cargo test --workspace --all-targets
git diff --check
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
git status --short
```

Expected: formatting and all tests PASS, the ledger prints no failure, and the
status contains no protected `temp/` path.

### Task 5: Independent review, plan closeout, and push

**Files:**
- Modify: `docs/superpowers/plans/2026-07-20-checkpoint-derived-total-authority.md`

- [x] **Step 1: Dispatch a high-intensity read-only review**

Give the reviewer the design, plan, implementation commit range, and these
questions:

```text
Review only; do not edit. Find correctness regressions, stale cached checkpoint
totals, schema/value drift, accidental changes to quiescence snapshot semantics,
workload API changes, insufficient negative tests, and missing CLI coverage.
Report findings by severity with file and line references. Explicitly say when
there are no findings and list residual risks.
```

- [x] **Step 2: Resolve every finding and rerun affected verification**

For each finding, reproduce it locally, add or strengthen a failing test when
appropriate, make the smallest fix, rerun the focused test, then rerun the
workspace command from Task 4 Step 5. Commit fixes with a scoped message.

- [x] **Step 3: Mark this plan complete and commit the closeout**

Change every checkbox in this file to `[x]`, then run:

```bash
git add docs/superpowers/plans/2026-07-20-checkpoint-derived-total-authority.md
git diff --cached --check
git commit -m "docs: close checkpoint derived total plan"
```

- [x] **Step 4: Verify the final branch and push**

```bash
git status --short --branch
git log --oneline --decorate -8
git push origin main
git status --short --branch
```

Expected: the first status is clean, push succeeds, and the final status shows
`main...origin/main` with no ahead/behind count.
