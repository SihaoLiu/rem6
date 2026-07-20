# Branch Predictor Legacy Checkpoint Fixtures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace synthetic valid branch-predictor checkpoint versions 1 through 4 with frozen payload fixtures and add explicit frozen version-5 compatibility.

**Architecture:** A focused integration-test fixture module owns literal retired-schema bytes. The branch-predictor tests decode each fixture, compare schema-owned typed state, migrate through the sole version-6 writer, and decode again; source policy bans valid retired-schema byte surgery while malformed-payload mutation remains available.

**Tech Stack:** Rust workspace, `rem6-cpu`, binary codec integration tests, source-policy tests, and real `rem6 run --execute` branch-predictor CLI regression coverage.

---

### Task 1: Add the RED frozen-fixture boundary

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add the focused source-policy test**

Add this test before the helper section near the end of the file:

```rust
#[test]
fn branch_predictor_legacy_checkpoints_use_frozen_payloads() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("tests/branch_predictor.rs");
    let fixture_path =
        crate_dir.join("tests/branch_predictor/legacy_checkpoint_fixtures.rs");
    assert!(
        fixture_path.exists(),
        "legacy branch-predictor checkpoint bytes belong in a focused fixture module"
    );

    let root = fs::read_to_string(&root_path).unwrap();
    let code = rust_code_without_comments_and_literals(&root);
    for forbidden in [
        "fn current_payload_prefix_without_btb_kind_counters",
        "const VERSION_OFFSET",
        "const ACTIVE_SPECULATION_V2_BYTES",
        "const ACTIVE_SPECULATION_V3_BYTES",
        "const ACTIVE_SPECULATION_V4_BYTES",
        "v1_encoded",
        "v2_encoded",
        "v3_encoded",
        "v4_encoded",
        "v5_encoded",
    ] {
        assert!(
            !code.contains(forbidden),
            "valid legacy branch-predictor compatibility must not regenerate retired schema code `{forbidden}`"
        );
    }

    assert!(
        root.contains("mod legacy_checkpoint_fixtures;"),
        "branch_predictor.rs must import the focused legacy fixture module"
    );
    let fixtures = fs::read_to_string(&fixture_path).unwrap();
    for required in [
        "LEGACY_V1_DEFAULT_PAYLOAD",
        "LEGACY_V2_ACTIVE_MAPPING_PAYLOAD",
        "LEGACY_V3_TARGET_PREDICTION_PAYLOAD",
        "LEGACY_V4_RAS_PAYLOAD",
        "LEGACY_V5_BRANCH_KIND_PAYLOAD",
    ] {
        assert!(
            fixtures.contains(&format!("pub(super) const {required}: &[u8] = &[")),
            "legacy branch-predictor fixture module is missing `{required}`"
        );
        assert!(
            root.contains(required),
            "branch predictor compatibility tests must consume `{required}`"
        );
    }
}
```

- [ ] **Step 2: Run the exact policy test and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --test source_policy branch_predictor_legacy_checkpoints_use_frozen_payloads -- --exact --nocapture
```

Expected: FAIL because the fixture child does not exist and the current root
still contains `current_payload_prefix_without_btb_kind_counters`.

### Task 2: Capture and freeze versions 1 through 5

**Files:**
- Create: `crates/rem6-cpu/tests/branch_predictor/legacy_checkpoint_fixtures.rs`
- Temporarily modify: `crates/rem6-cpu/tests/branch_predictor.rs`
- Test: `crates/rem6-cpu/tests/branch_predictor.rs`

- [ ] **Step 1: Add a transient Rust-array printer**

Temporarily add this helper after `current_payload_prefix_without_btb_kind_counters`:

```rust
fn print_legacy_checkpoint_fixture(name: &str, payload: &[u8]) {
    println!("pub(super) const {name}: &[u8] = &[");
    for chunk in payload.chunks(16) {
        print!("    ");
        for byte in chunk {
            print!("0x{byte:02x}, ");
        }
        println!();
    }
    println!("];");
}
```

- [ ] **Step 2: Print the existing v1-v4 valid payloads**

Before printing v2, replace its `from_snapshot` call with the equivalent
compact explicit-BTB constructor:

```rust
let payload = BranchPredictorCheckpointPayload::from_snapshots(
    predictor.snapshot(),
    btb(8, 2).snapshot(),
    [(21, first.id()), (22, second.id())],
)
.unwrap();
```

Immediately before each existing legacy decode, add the matching transient
call:

```rust
print_legacy_checkpoint_fixture("LEGACY_V1_DEFAULT_PAYLOAD", &encoded);
print_legacy_checkpoint_fixture("LEGACY_V2_ACTIVE_MAPPING_PAYLOAD", &v2_encoded);
print_legacy_checkpoint_fixture("LEGACY_V3_TARGET_PREDICTION_PAYLOAD", &v3_encoded);
print_legacy_checkpoint_fixture("LEGACY_V4_RAS_PAYLOAD", &v4_encoded);
```

Run:

```bash
cargo test -p rem6-cpu --test branch_predictor checkpoint_payload_decodes_ -- --nocapture
```

Expected: the four tests pass and print complete Rust byte arrays.

- [ ] **Step 3: Add and print a transient v5 compatibility row**

Add a temporary test that constructs the same nontrivial predictor, BTB, RAS,
active predictions, RAS operations, and branch kinds used by
`checkpoint_payload_round_trips_snapshot_and_active_mapping`. Strip only the
v6 BTB kind-counter block and set the version to 5:

```rust
let encoded = payload.encode();
let mut v5_encoded =
    current_payload_prefix_without_btb_kind_counters(&encoded, 8, 2, encoded.len());
v5_encoded[4] = 5;
print_legacy_checkpoint_fixture("LEGACY_V5_BRANCH_KIND_PAYLOAD", &v5_encoded);

let decoded = BranchPredictorCheckpointPayload::decode(&v5_encoded).unwrap();
assert_eq!(decoded.snapshot(), payload.snapshot());
assert_eq!(decoded.active_speculations(), payload.active_speculations());
assert_eq!(decoded.active_branch_target_predictions(), payload.active_branch_target_predictions());
assert_eq!(decoded.return_address_stack_snapshot(), payload.return_address_stack_snapshot());
assert_eq!(decoded.active_return_address_stack_operations(), payload.active_return_address_stack_operations());
assert_eq!(decoded.active_branch_kinds(), payload.active_branch_kinds());
```

Run:

```bash
cargo test -p rem6-cpu --test branch_predictor checkpoint_payload_decodes_v5_active_mapping_with_branch_kinds_without_btb_kind_counters -- --exact --nocapture
```

Expected: PASS and print `LEGACY_V5_BRANCH_KIND_PAYLOAD`.

- [ ] **Step 4: Create the literal fixture module**

Create `legacy_checkpoint_fixtures.rs` from the five printed arrays. The final
file contains only the five `pub(super) const ...: &[u8]` declarations and a
short module comment; it contains no functions, encoders, offsets, or payload
mutation.

- [ ] **Step 5: Cross-check literals before deleting the transient writer**

Import the fixture module temporarily and add equality assertions beside the
printed rows:

```rust
assert_eq!(LEGACY_V1_DEFAULT_PAYLOAD, encoded);
assert_eq!(LEGACY_V2_ACTIVE_MAPPING_PAYLOAD, v2_encoded);
assert_eq!(LEGACY_V3_TARGET_PREDICTION_PAYLOAD, v3_encoded);
assert_eq!(LEGACY_V4_RAS_PAYLOAD, v4_encoded);
assert_eq!(LEGACY_V5_BRANCH_KIND_PAYLOAD, v5_encoded);
```

Rerun all five exact tests. Expected: PASS, proving each literal matches the
retired shape before the synthetic writer is removed.

### Task 3: Replace valid legacy generation with decode-to-current migration

**Files:**
- Modify: `crates/rem6-cpu/tests/branch_predictor.rs`
- Verify: `crates/rem6-cpu/tests/branch_predictor/legacy_checkpoint_fixtures.rs`
- Test: `crates/rem6-cpu/tests/branch_predictor.rs`

- [ ] **Step 1: Import the frozen fixture owner**

At the top of `branch_predictor.rs`, add:

```rust
mod legacy_checkpoint_fixtures;

use legacy_checkpoint_fixtures::{
    LEGACY_V1_DEFAULT_PAYLOAD, LEGACY_V2_ACTIVE_MAPPING_PAYLOAD,
    LEGACY_V3_TARGET_PREDICTION_PAYLOAD, LEGACY_V4_RAS_PAYLOAD,
    LEGACY_V5_BRANCH_KIND_PAYLOAD,
};
```

- [ ] **Step 2: Add one current-migration assertion helper**

Add:

```rust
fn assert_legacy_checkpoint_migrates_to_current(
    legacy: &[u8],
    decoded: &BranchPredictorCheckpointPayload,
) {
    let current = decoded.encode();
    assert_eq!(current[4], 6);
    assert_ne!(current.as_slice(), legacy);
    assert_eq!(
        BranchPredictorCheckpointPayload::decode(&current),
        Ok(decoded.clone())
    );
}
```

- [ ] **Step 3: Convert the v1-v4 tests**

For each existing v1-v4 test, retain the typed predictor/BTB/RAS setup used to
construct expected state, but delete all current `encode`, offset, truncation,
version rewrite, and record-splicing code. Decode the matching fixture constant
directly and keep the schema-specific assertions.

End each row with:

```rust
assert_legacy_checkpoint_migrates_to_current(LEGACY_V1_DEFAULT_PAYLOAD, &decoded);
assert_legacy_checkpoint_migrates_to_current(LEGACY_V2_ACTIVE_MAPPING_PAYLOAD, &decoded);
assert_legacy_checkpoint_migrates_to_current(LEGACY_V3_TARGET_PREDICTION_PAYLOAD, &decoded);
assert_legacy_checkpoint_migrates_to_current(LEGACY_V4_RAS_PAYLOAD, &decoded);
```

- [ ] **Step 4: Finalize the v5 test**

Delete its transient encoding path and decode
`LEGACY_V5_BRANCH_KIND_PAYLOAD` directly. Assert all v5-owned state equals the
typed expected payload. Additionally require all four v6-only BTB branch-kind
counter families to equal their default zero snapshots. End with:

```rust
assert_legacy_checkpoint_migrates_to_current(LEGACY_V5_BRANCH_KIND_PAYLOAD, &decoded);
```

- [ ] **Step 5: Delete the retired test writer**

Delete:

- `current_payload_prefix_without_btb_kind_counters`;
- the transient array printer;
- all transient fixture-equality assertions;
- per-test `VERSION_OFFSET` and historical active-record byte constants; and
- every valid legacy byte splice/truncate/version rewrite.

Keep layout constants and byte mutation used exclusively by malformed-payload
rejection tests.

- [ ] **Step 6: Run focused compatibility and policy tests**

Run:

```bash
cargo test -p rem6-cpu --test source_policy branch_predictor_legacy_checkpoints_use_frozen_payloads -- --exact --nocapture
cargo test -p rem6-cpu --test branch_predictor checkpoint_payload_decodes_ -- --nocapture
cargo test -p rem6-cpu --test branch_predictor checkpoint_payload_round_trips_snapshot_and_active_mapping -- --exact --nocapture
cargo test -p rem6-cpu --test branch_predictor checkpoint_payload_round_trips_return_address_stack_operation_id_gaps_after_squash -- --exact --nocapture
```

Expected: all PASS with five frozen retired-schema rows and the current v6
writer round trips.

### Task 4: Verify real behavior and malformed boundaries

**Files:**
- Verify only: `crates/rem6-cpu/tests/branch_predictor.rs`
- Verify only: `crates/rem6/tests/cli_run/stats_compat/selected_branch_predictor_matrix.rs`

- [ ] **Step 1: Run the full branch predictor integration suite**

Run:

```bash
cargo test -p rem6-cpu --test branch_predictor
```

Expected: all current, legacy, and malformed-payload rows pass.

- [ ] **Step 2: Run the real CLI branch-predictor row**

Run:

```bash
cargo test -p rem6 --test cli_run stats_compat::selected_branch_predictor_matrix::rem6_run_stats_use_selected_gshare_branch_predictor_for_fetch_steering -- --exact --nocapture
```

Expected: PASS through `rem6 run --execute` with selected branch-predictor
fetch steering and runtime stats.

### Task 5: Verify, review, commit, and push

**Files:**
- Verify only: `docs/architecture/gem5-to-rem6-migration.md`
- Verify only: `temp/improve-rem6-0.md`

- [ ] **Step 1: Run package and workspace verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6-cpu --all-targets
cargo test --workspace --all-targets -q
```

Expected: all commands exit 0.

- [ ] **Step 2: Run hygiene checks**

Run:

```bash
git diff --check
rg -n "current_payload_prefix_without_btb_kind_counters|const VERSION_OFFSET|const ACTIVE_SPECULATION_V[234]_BYTES|v[1-5]_encoded" crates/rem6-cpu/tests/branch_predictor.rs
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: no retired valid-writer matches, the ledger remains exactly 1,200
lines, and protected paths are untouched.

- [ ] **Step 3: Request independent read-only review**

Reviewers must check fixture literal ownership, v1-v5 schema coverage, v5 BTB
counter defaults, current-v6 migration, malformed-test preservation, source
policy robustness, and absence of production codec changes.

- [ ] **Step 4: Commit and push**

Run:

```bash
git add docs/superpowers/specs/2026-07-19-branch-predictor-legacy-checkpoint-fixtures-design.md \
  docs/superpowers/plans/2026-07-19-branch-predictor-legacy-checkpoint-fixtures.md \
  crates/rem6-cpu/tests/branch_predictor.rs \
  crates/rem6-cpu/tests/branch_predictor/legacy_checkpoint_fixtures.rs \
  crates/rem6-cpu/tests/source_policy.rs
git commit -m "refactor: freeze legacy branch checkpoint payloads"
git push origin main
```

Verify `git status --short --branch` is clean and `HEAD` equals `origin/main`.
