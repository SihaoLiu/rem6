# RISC-V Linked-Control Test Extraction Design

## Context

At the pre-extraction `HEAD`,
`crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs` contained
1,794 lines, only six lines below the crate-wide 1,800-line source limit. It
mixed generic detailed O3 control-window tests with a
large linked-control family covering calls, returns, coroutines, return-address
stack lineage, and producer-forwarded target validation.

The linked-control family is cohesive, but its size hides that boundary and
leaves no honest headroom for either family. This increment is a test-only
structural cleanup. It does not change production behavior, assertions, or the
migration score.

## Ledger Boundary

The affected evidence belongs to `CPU Execution Models - 74% representative`.
No new runtime shape, matrix axis, artifact, checkpoint behavior, or CLI result
is added. `docs/architecture/gem5-to-rem6-migration.md` remains unchanged and
exactly 1,200 lines.

## Approaches

### Leave the mixed owner at the global cap

This avoids test-path changes but keeps unrelated generic and linked-control
fixtures in one nearly full file. The next small test addition would force an
unplanned split.

### Move only remaining production-owned test helpers

Relocating the two control-lineage inspection helpers is safe, but it does not
address the 1,794-line mixed test owner. That cleanup remains a later narrow
candidate.

### Extract the linked-control family

This is the selected design. A focused child module owns the linked-call,
return, coroutine, RAS-lineage, and producer-forwarded validation family. The
parent retains generic branch-window, nested-control, split-fetch, and
dependency-terminal tests, plus the shared `live_same_link_core` fixture used
by `producer_forwarded_control_validation.rs`.

## Module Boundary

Create:

`crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`

The parent declares it with an explicit path-owned child module and does not
use `include!`.

The child owns these existing blocks unchanged:

- Fixture functions beginning with `recorded_same_window_coroutine_core` and
  ending with `recorded_second_linked_coroutine_pc`.
- Linked-call tests beginning with
  `detailed_scalar_window_direct_call_follows_target_and_pushes_ras` and ending
  with `detailed_scalar_window_forwards_call_ras_to_same_window_coroutine`.
- Recorded coroutine/return tests beginning with
  `detailed_recorded_coroutine_accepts_exact_pop_then_push` and ending with
  `detailed_invalid_recorded_return_does_not_retry_as_fresh_prediction`.

The parent retains:

- `detailed_control_core`, `detailed_linked_control_core`, and
  `live_same_link_core`.
- Nested and three-deep generic control fixtures.
- `detailed_scalar_window_returns_existing_branch_prediction_decision`.
- `detailed_control_target_authority_rejects_non_predicted_decision`.
- Split-fetch, recorded branch-path, lookahead, and dependency-terminal tests.

`producer_forwarded_target` moves with its only callers into the child.

## Privacy And Test Paths

The child uses `use super::*;`, matching existing test-child patterns. It can
access parent-private fixtures without widening any production visibility.
`live_same_link_core` stays `pub(super)` in the parent so the existing sibling
caller remains unchanged.

Test function bodies and assertions do not change. Linked-control unit test
paths gain the `linked_control` module segment; retained parent and sibling
test paths remain stable.

## Source Policy

A focused source-policy test requires:

- The explicit `linked_control.rs` child declaration.
- No `include!` in either owner.
- Linked-control anchor functions only in the child.
- Shared and generic anchor functions only in the parent.
- Parent length at or below 450 lines.
- Child length at or below 1,500 lines.

These limits leave meaningful headroom while remaining above the expected
post-move sizes of roughly 370 and 1,430 lines.

## Behavioral Preservation

This increment moves existing Rust items without editing their bodies. It does
not change fetch-ahead decisions, branch speculation, RAS operations, runtime
authority, checkpoint encoding, stats, or CLI behavior.

Representative real-binary verification remains the same-window link-return,
coroutine round-trip, and producer-forwarded return-descendant rows. The full
workspace test suite remains the final regression gate.

## Files

- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`:
  declare the child and retain generic/shared tests.
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`:
  own the linked-control/RAS test family.
- `crates/rem6-cpu/tests/source_policy.rs`: enforce the module boundary and
  line budgets.

## Verification

Verification covers an observed RED/GREEN source-policy test, representative
linked and retained-parent unit tests, the sibling caller of
`live_same_link_core`, all `rem6-cpu` targets, three real CLI rows, the full
workspace, formatting, line counts, protected-path checks, and independent
read-only review.
