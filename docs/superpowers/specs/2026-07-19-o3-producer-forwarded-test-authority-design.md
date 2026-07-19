# O3 Producer-Forwarded Test Authority Design

## Context

The focused producer-forwarded O3 owner contains four `#[cfg(test)]` methods
in production-owned source. Three methods on `O3RuntimeState` retire a synthetic
data head, inspect a live scalar-return issue tick, or replace a speculative
fetch identity. One method on `O3ProducerForwardedScalarChain` repeats the last
descendant so a retained-chain test can construct an invalid longer candidate.

The methods do not compile into production, but their definitions still mix
test authority into the production owner files. The current source policy uses
`production_rust_source`, which deliberately strips `#[cfg(test)]` items and
therefore cannot enforce source placement.

## Ledger Boundary

This cleanup belongs to `CPU Execution Models - 74% representative`. It adds
no producer-forwarded control shape, runtime matrix axis, CLI artifact, or
checklist evidence. The migration ledger remains unchanged and exactly 1,200
lines.

## Approaches

### Keep the hooks in production-owned files

This preserves all tests but leaves source-level test authority in the focused
runtime and value owners. The existing policy proves only that the hooks are
compiled out of production.

### Rewrite every caller through production behavior

The fetch-ahead callers cross the `riscv_fetch_ahead` to `o3_runtime` privacy
boundary. Replacing synthetic data-head retirement with full response,
retirement, wake, trace, rename, and commit flows would change the behavior
under test. The live scalar-return issue tick is not exposed through a stable
production inspection surface. This is broader than a source cleanup.

### Relocate hooks to owner-local test modules

This is the selected design. The three runtime hooks move unchanged into a
test-only sibling module declared from `o3_runtime.rs`, where private runtime
fields remain visible. The repeated-chain helper moves unchanged into a
test-only child of `value.rs`, where the private `push` operation remains
visible. Call sites do not change.

## Runtime Test Support Boundary

`o3_runtime_producer_forwarded_chain_tests.rs` owns:

- `retire_producer_forwarded_data_head_for_test`
- `producer_forwarded_scalar_return_issue_tick_for_test`
- `replace_producer_forwarded_chain_fetch_identity_for_test`

The file is declared only under `#[cfg(test)]` from `o3_runtime.rs`. It remains
a child of the runtime owner, so it can access `live_data_accesses`, the ROB,
live speculative executions, and `last_live_commit_tick` without widening
production field visibility.

## Value Test Support Boundary

`o3_runtime_producer_forwarded_chain/value_tests.rs` owns
`O3ProducerForwardedScalarChain::repeated_last_for_test`. It is declared only
under `#[cfg(test)]` from `value.rs`, so it can call the private chain `push`
operation without making descendants or mutation public.

The file stays small because the producer-forwarded module family has a tight
aggregate line budget. Existing broad behavior tests remain in their current
focused test modules.

## Source Policy

A new focused source-policy test scans raw Rust source after removing comments
and literals, skips paths classified by `is_test_only_rust_source`, and rejects
the four hook names when defined in any production-owned path. This catches
`#[cfg(test)]` definitions that `production_rust_source` intentionally hides.

The existing production-visibility check inside
`producer_forwarded_chain_authority_stays_focused` remains useful for compiled
production authority, but the new path-aware policy becomes the source
placement gate.

## Behavioral Preservation

No caller or assertion changes:

- Fetch-ahead tests continue synthetic data-head retirement under the caller's
  existing `RiscvCore` lock.
- Scalar-return issue-tick tests continue inspecting the live speculative row
  and its retirement-tick clamp.
- Runtime and fetch-ahead validation tests continue replacing consumed request
  identity to prove stale lineage fails closed.
- Retained-chain validation continues constructing a repeated descendant and
  rejecting a longer candidate.

## Representative Runtime Evidence

The strongest real-binary evidence remains:

- The producer-forwarded return-descendant link-shape/route matrix.
- The producer-forwarded scalar-return link-shape/route matrix.
- The non-link scalar negative row.

These rows prove the underlying runtime behavior is unchanged; this increment
does not claim new matrix coverage.

## Files

- `crates/rem6-cpu/src/o3_runtime.rs`: declare the test-only runtime support
  sibling.
- `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs`: remove the
  three test hooks.
- `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain_tests.rs`: own the
  three runtime test hooks.
- `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value.rs`: declare
  the test-only value child and remove the repeated-chain helper.
- `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value_tests.rs`:
  own the repeated-chain helper.
- `crates/rem6-cpu/tests/source_policy.rs`: reject all four helpers in
  production-owned source paths.

## Verification

Verification covers the RED/GREEN source policy, the exact fetch-identity,
retirement-tick, and retained-chain tests, all `rem6-cpu` targets, the three
representative CLI rows, the full workspace, formatting, helper-location and
line-budget checks, protected-path checks, and independent read-only review.
