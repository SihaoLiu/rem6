# O3 Handoff Legacy Writer Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove synthetic writers for retired O3 live-data handoff schemas while preserving and strengthening frozen v1-v6 decode compatibility and the sole v7 runtime writer.

**Architecture:** Historical compatibility will be owned by literal payload fixtures in `legacy_payload_fixtures.rs`. Every fixture will decode to its typed handoff, migrate through `encode()` to v7, and decode again; `codec.rs` will retain only the current writer and the multi-version decoder.

**Tech Stack:** Rust workspace, `rem6-cpu`, source-policy tests, frozen binary fixtures, real `rem6` CLI execution-mode handoff tests.

---

### Task 1: Add the RED source-policy boundary

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs:3283`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Ban retired-schema writers in the handoff module family**

Update `riscv_live_data_handoff_codec_lives_in_focused_module` to retain the root path and scan every Rust file in the handoff module family, including `#[cfg(test)]` items:

```rust
let root_path = crate_dir.join("src/riscv_execution_mode_handoff.rs");
let root = fs::read_to_string(&root_path).unwrap();
```

After the existing codec-owner assertions, add:

```rust
for path in module_family_rust_source_files(&root_path) {
    let source = fs::read_to_string(&path).unwrap();
    let code = rust_code_without_comments_and_literals(&source);
    for forbidden in ["fn encode_legacy", "fn write_legacy", "_legacy_for_test"] {
        assert!(
            !code.contains(forbidden),
            "RISC-V live-data handoff compatibility must use frozen decode fixtures instead of retired-schema writer `{forbidden}` in {}",
            path.display()
        );
    }
}
```

Do not use `production_rust_source`; the obsolete writer is under `#[cfg(test)]` and must remain visible to this policy.

- [ ] **Step 2: Run the exact policy test and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --test source_policy riscv_live_data_handoff_codec_lives_in_focused_module -- --exact --nocapture
```

Expected: FAIL on `encode_legacy_for_test` in `riscv_execution_mode_handoff/codec.rs`.

### Task 2: Freeze every retained legacy valid shape

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff/legacy_payload_fixtures.rs`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs:22-32,1037-1044,1249-1319`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff/completed_partial_overlay_tests.rs:170-194`
- Test: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`

- [ ] **Step 1: Centralize the existing v1 and v6 payloads**

Move `LEGACY_V1_SINGLE_ENTRY_PAYLOAD` unchanged from the root test module into `legacy_payload_fixtures.rs` and make it `pub(super) const ...: &[u8]`.

Move `V6_MULTI_SOURCE_PENDING_PAYLOAD` unchanged from `completed_partial_overlay_tests.rs` into the same fixture module, rename it `LEGACY_V6_MULTI_SOURCE_PENDING_PAYLOAD`, and make it `pub(super) const ...: &[u8]`.

Update the root test-only fixture import to include both names:

```rust
use legacy_payload_fixtures::{
    LEGACY_V1_SINGLE_ENTRY_PAYLOAD, LEGACY_V2_TYPED_TARGET_PAYLOAD,
    LEGACY_V3_FORWARDED_PAYLOAD, LEGACY_V4_PARTIAL_OVERLAY_PAYLOAD,
    LEGACY_V5_FORWARDED_PAYLOAD, LEGACY_V5_SINGLE_SOURCE_PARTIAL_OVERLAY_PAYLOAD,
    LEGACY_V5_TYPED_TARGET_PAYLOAD, LEGACY_V6_MULTI_SOURCE_PENDING_PAYLOAD,
};
```

- [ ] **Step 2: Add frozen v5 typed-target bytes**

Add this fixture to `legacy_payload_fixtures.rs`:

```rust
pub(super) const LEGACY_V5_TYPED_TARGET_PAYLOAD: &[u8] = &[
    0x4f, 0x33, 0x44, 0x48, 0x05, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x04, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1e, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x04, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x03, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
    0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x02, 0x00, 0x00, 0x00, 0x00, 0x01, 0x05, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x80, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x16,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
```

- [ ] **Step 3: Add frozen v5 forwarded-row bytes**

Add this fixture to `legacy_payload_fixtures.rs`:

```rust
pub(super) const LEGACY_V5_FORWARDED_PAYLOAD: &[u8] = &[
    0x4f, 0x33, 0x44, 0x48, 0x05, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x04, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1e, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x04, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x03, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
    0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x0b, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x04, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00,
    0x2a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x16, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];
```

- [ ] **Step 4: Verify the new fixtures against the still-present writer**

Extend the frozen fixture table in `live_data_handoff_decodes_legacy_v2_through_v5_bytes` with:

```rust
(
    LEGACY_V5_TYPED_TARGET_PAYLOAD,
    typed.clone(),
    VERSION_SINGLE_SOURCE_CURRENT,
),
(
    LEGACY_V5_FORWARDED_PAYLOAD,
    forwarded.clone(),
    VERSION_SINGLE_SOURCE_CURRENT,
),
```

Keep the existing `assert_eq!(handoff.encode_legacy_for_test(version), payload)` temporarily. Run:

```bash
cargo test -p rem6-cpu live_data_handoff_decodes_legacy_v2_through_v5_bytes --lib -- --exact --nocapture
```

Expected: PASS, proving the literal v5 fixtures exactly match the current retired writer before that writer is removed.

### Task 3: Replace legacy generation with frozen decode-to-current migration

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs:1226-1319`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff/completed_partial_overlay_tests.rs:443-460`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff/codec.rs:135-244`
- Test: `crates/rem6-cpu` library tests

- [ ] **Step 1: Convert the v2-v5 fixture table**

Rename the test to `live_data_handoff_decodes_legacy_v2_through_v5_and_migrates_to_current` and replace the writer equality plus generated second loop with:

```rust
let (decoded, decoded_version) =
    RiscvO3LiveDataHandoff::decode_with_version(payload).unwrap();
assert_eq!(decoded_version, version);
assert_eq!(decoded, handoff);

let current = decoded.encode();
assert_eq!(current[MAGIC.len()], VERSION_CURRENT);
assert_ne!(current.as_slice(), payload);
assert_eq!(
    RiscvO3LiveDataHandoff::decode_with_version(&current),
    Ok((decoded, VERSION_CURRENT))
);
```

Delete the generated `(handoff, version)` loop entirely.

- [ ] **Step 2: Strengthen the v6 frozen migration row**

Update `live_data_handoff_decodes_v6_multi_source_partial_overlay` to use `LEGACY_V6_MULTI_SOURCE_PENDING_PAYLOAD`, assert `VERSION_MULTI_SOURCE_CURRENT`, and add:

```rust
assert_eq!(decoded.entries().len(), 3);
assert_eq!(decoded.partial_overlays().len(), 1);
assert_eq!(decoded.partial_overlays()[0].sources().len(), 2);
assert_eq!(decoded.partial_overlays()[0].forwarded_mask(), 0x0c);
assert!(decoded.completed_partial_overlays().is_empty());

let current = decoded.encode();
assert_eq!(current[MAGIC.len()], VERSION_CURRENT);
assert_ne!(current.as_slice(), LEGACY_V6_MULTI_SOURCE_PENDING_PAYLOAD);
assert_eq!(
    RiscvO3LiveDataHandoff::decode_with_version(&current),
    Ok((handoff, VERSION_CURRENT))
);
```

Delete `legacy_v6_encoder_matches_frozen_multi_source_payload`.

- [ ] **Step 3: Delete the retired writer**

Delete the complete `#[cfg(test)] pub(super) fn encode_legacy_for_test` method from `codec.rs`. Do not change version constants or any decode branch.

- [ ] **Step 4: Verify policy and compatibility tests**

Run:

```bash
cargo test -p rem6-cpu --test source_policy riscv_live_data_handoff_codec_lives_in_focused_module -- --exact --nocapture
cargo test -p rem6-cpu live_data_handoff --lib -- --nocapture
cargo test -p rem6-cpu current_live_data_handoff_writer_uses_one_latest_typed_schema --lib -- --exact --nocapture
```

Expected: all PASS. `rg -n "encode_legacy_for_test" crates/rem6-cpu` returns no matches.

### Task 4: Verify the real v7 handoff matrix

**Files:**
- Verify only: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/scalar_load.rs`
- Verify only: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/mmio_scalar_load.rs`
- Verify only: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs`

- [ ] **Step 1: Run positive current-writer rows**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::scalar_load::rem6_run_host_switch_transfers_outstanding_o3_scalar_load_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::scalar_load::rem6_run_host_switch_transfers_outstanding_o3_scalar_load_cache_fabric_dram -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::mmio_scalar_load::rem6_run_host_switch_transfers_outstanding_o3_scalar_load_mmio -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding::rem6_run_host_switch_transfers_multi_source_partial_forwarded_store_load_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding::rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_cache_fabric_dram -- --exact --nocapture
```

Expected: all PASS with schema version 7, decoded handoff ownership/timing, architectural witnesses, and direct versus hierarchy resource activity.

- [ ] **Step 2: Run negative younger-row suppression rows**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding::rem6_run_host_switch_rejects_multi_source_partial_forwarded_store_load_with_younger_row -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding::rem6_run_host_switch_rejects_completed_partial_forwarded_store_load_with_younger_row -- --exact --nocapture
```

Expected: both PASS while proving unsupported live shapes still fail closed.

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
rg -n "encode_legacy_for_test|fn encode_legacy|fn write_legacy" crates/rem6-cpu/src/riscv_execution_mode_handoff.rs crates/rem6-cpu/src/riscv_execution_mode_handoff
wc -l crates/rem6-cpu/src/riscv_execution_mode_handoff.rs crates/rem6-cpu/src/riscv_execution_mode_handoff/codec.rs docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: no retired-writer matches, both production files remain below 1,800 lines, the ledger remains exactly 1,200 lines, and protected paths are untouched.

- [ ] **Step 3: Request independent read-only review**

Reviewers must check that no decode branch or malformed-payload assertion was weakened, all valid v1-v6 shapes are frozen, the current writer remains v7-only, CLI evidence is real, and the ledger is unchanged.

- [ ] **Step 4: Commit and push**

```bash
git add docs/superpowers/specs/2026-07-19-o3-handoff-legacy-writer-removal-design.md docs/superpowers/plans/2026-07-19-o3-handoff-legacy-writer-removal.md crates/rem6-cpu
git commit -m "refactor: remove legacy O3 handoff writer"
git push origin main
```

Verify `git status --short --branch` is clean and `git rev-parse HEAD` equals `git rev-parse origin/main`.
