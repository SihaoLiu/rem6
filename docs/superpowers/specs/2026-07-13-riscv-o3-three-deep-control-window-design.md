# RISC-V O3 Three-Deep Control Window Design

## Goal

Extend the bounded detailed O3 scalar-memory window from two predicted direct
conditional controls to three controls when the configured branch lookahead is
three. The executable shape is one delayed scalar load followed by three mixed
direct conditional branches, exactly filling the existing four-row ROB window.

This slice advances the CPU ledger's explicit third/deeper-branch and wider
control-class boundary. It does not claim a general O3 engine or raise the CPU
score above the existing 74% representative cap.

## Current Boundary

The current implementation has two separate maximum-depth authorities:

- `crates/rem6/src/config/riscv_branch.rs` accepts only lookahead one or two.
- `crates/rem6-cpu/src/riscv_o3_window_policy.rs` independently caps predicted
  control depth at two.

`RiscvCore::set_branch_lookahead` accepts any `usize`, so non-CLI callers can
create a configuration that disagrees with the O3 policy. The existing nested
CLI matrix is already a large focused module and should not absorb another
independent control shape.

## Considered Approaches

### 1. Three-deep heterogeneous control matrix

Raise the supported branch lookahead to three, centralize that maximum in
`rem6-cpu`, and execute a four-row load plus three-branch matrix.

Advantages:

- closes the next explicit multi-branch depth boundary;
- exercises immediate outer/middle/inner ownership;
- adds top-level evidence for branch classes beyond BEQ;
- fits the existing bounded ROB/LSQ authority without inventing a new engine;
- removes duplicated depth constants and enforces the public setter invariant.

Cost: branch lookahead three becomes a supported global frontend setting and
must be verified outside the O3 policy helper.

### 2. Scheduler-owned O3 issue-width contention

Add a real issue arbiter and per-cycle resource contention before widening
control depth.

Advantages: more directly advances the remaining general O3 boundary.

Cost: it requires a new scheduler-owned execution authority, mode-transfer
state, and contention stats. Timestamp arithmetic alone would be weak evidence,
so this is too large to combine with the current cleanup increment.

### 3. Cold translated-load younger windows

Extend detailed O3 fetch and transfer through cold translated memory misses.

Advantages: adds a useful translated-memory matrix axis.

Cost: it leaves the just-identified third-branch and wider-control boundary
open and does not remove the duplicated branch-depth policy.

Approach 1 is selected.

## Shared Depth Authority

`rem6-cpu` will expose one public maximum branch-lookahead constant with value
three. Both the CPU setter and CLI/TOML validation will use it.

The valid range remains inclusive from one through the shared maximum. Zero and
four remain invalid. `RiscvCore::set_branch_lookahead` will assert this invariant
for direct library callers instead of silently accepting a value that the CLI
cannot represent.

The O3 policy will derive its maximum predicted-control depth from the same
authority. The existing four-row scalar-memory window remains unchanged, so a
load plus three controls is full and a fourth control or any descendant remains
outside the window.

## Runtime Ownership

No new rollback data structure is required. Existing immediate ownership scales
naturally:

- middle control depends on outer control;
- inner control depends on middle control;
- any later fetched fall-through work is outside the full four-row O3 window.

Outer rollback discards middle and inner rows. Middle rollback preserves outer
and discards inner. Inner rollback preserves outer and middle. The separate
control-window timing ownership introduced for mode transfer continues to track
all three control rows until they leave the ROB.

The branch-lookahead budget remains authoritative. Lookahead two may fetch the
third branch after predicting the middle branch, but it must not create a third
prediction or stage the third branch in the detailed scalar-memory window.

## Executable Program Shape

The new CLI module will build one deterministic RV64 program with these rows:

1. delayed cacheable scalar load;
2. outer `BNE`;
3. middle signed `BLT`;
4. inner unsigned `BGEU`.

All three branches are not taken in the positive case. Their targets are nested
so a taken branch skips every younger control and fall-through effect owned by
that branch. The first fall-through instruction after the inner branch performs
a scalar multiply and store, giving rollback tests register, Data, Memory, and
memory-dump witnesses even though that instruction is outside the full O3
window.

Target marker registers distinguish outer, middle, and inner redirects.

## CLI Matrix

The new focused `predicted_control/three_deep.rs` module will cover:

- direct positive execution with exact four-ROB/one-LSQ residency and three
  pre-response branch issues;
- cache/fabric/DRAM outer rollback, including hierarchy activity and complete
  younger-control/wrong-path-memory suppression;
- direct middle rollback preserving the outer control;
- direct inner rollback preserving outer and middle controls;
- lookahead-two negative control with exactly two predictor lookups and only
  load, outer, and middle rows resident before the response;
- load-dependent inner control remaining terminal and issuing no earlier than
  the load response;
- detailed-to-timing transfer with four ROB rows, one LSQ row, three younger
  rows, and baseline-equivalent issue/writeback/commit timing;
- live checkpoint rejection plus drained zero-ROB/zero-LSQ capture/restore;
- timing-mode suppression with no O3 runtime or O3 trace surface.

The matrix intentionally uses one hierarchy-backed rollback representative;
duplicating every rollback position across both memory routes would add runtime
without adding a new ownership rule.

## Focused Unit Coverage

Unit tests will prove:

- policy admission of three mixed direct conditional controls and rejection of
  a fourth row;
- detailed fetch follows three recorded predictions;
- lookahead two blocks creation of the third prediction;
- runtime dependency edges are middle-to-outer and inner-to-middle;
- discarding the middle control preserves the outer row and removes the inner
  row;
- control-window timing ownership clears when the three-deep ROB suffix is
  removed.

## Error Handling

Unsupported lookahead values fail before execution through the existing typed
CLI error. Direct CPU callers receive an assertion failure at configuration time
instead of running with mismatched frontend and O3 limits.

All prediction, split-fetch, rollback, checkpoint, and mode-transfer failures
continue to fail closed through existing paths. No legacy payload version or new
checkpoint schema is introduced because live control windows remain
non-restorable.

## Source Boundaries

- The shared maximum lives in `rem6-cpu`, the owner of branch speculation.
- CLI validation imports that authority instead of duplicating a literal.
- Policy changes remain in `riscv_o3_window_policy.rs`.
- Fetch tests remain in the focused detailed-control test module.
- Runtime ownership tests remain in `o3_runtime_control_window_tests.rs`.
- Top-level evidence lives in a new `three_deep.rs` child module rather than
  expanding `nested.rs`.

No `temp/` file is committed.

## Verification

Required focused gates:

- branch-lookahead config and invalid-value CLI tests;
- `rem6-cpu` O3 policy, fetch-ahead control, and runtime-control tests;
- every original three-deep CLI row by exact filter;
- complete predicted-control CLI module;
- `rem6-cpu` library suite;
- `rem6` and `rem6-cpu` source-policy suites;
- full CLI suite and workspace all-targets suite;
- rustfmt, `git diff --check`, 1,200-line ledger count, and clean status;
- xhigh read-only review before push.

## Migration Ledger

The CPU heading stays at 74% representative and the gem5 CPU-test row keeps its
current percentage. The evidence text will add the three-deep mixed-control
matrix and narrow the open boundary to fourth/deeper chains, indirect or
unconditional nested control classes, arbitrary mixed memory/control windows,
issue-width/resource contention, and a general O3 engine.
