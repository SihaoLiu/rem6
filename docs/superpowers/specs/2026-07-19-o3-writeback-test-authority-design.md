# O3 Writeback Test Authority Design

## Context

The focused O3 writeback module currently contains four `#[cfg(test)]`
helpers in production-owned source. Three methods mutate private reservation
source or raw-ready state so sibling tests can manufacture impossible owner
inconsistencies. A fourth method exposes fixed-FU reservation setup on
`RiscvCore` for tests in several CPU modules.

The three corruption helpers widen `O3RuntimeState` with mutation authority
that production cannot exercise. Their tests then trigger private owner
validation indirectly by adding an unrelated earlier reservation. The benign
core setup helper uses the real reservation path, but it still belongs in the
existing test-only writeback module family rather than the production module.

## Ledger Boundary

This cleanup belongs to `CPU Execution Models - 74% representative`. It adds
no O3 capability, executable matrix axis, artifact field, checklist item, or
bucket-cap evidence. The migration ledger remains unchanged and exactly 1,200
lines.

## Approaches

### Keep the helpers in production-owned source

This preserves the current tests but leaves test mutation and setup authority
mixed into the production module. The corruption helpers also obscure the
private invariant actually under test.

### Relocate all helpers unchanged

Moving all four methods into a test-only file would remove them from production
source. It would still preserve a crate-wide corruption API for three tests,
even though those tests can live beside the private replan authority.

### Keep corruption private and relocate only legitimate setup

This is the selected design. The three corruption helpers are deleted. Their
error coverage moves to a child test module of `replan.rs`, which can construct
inconsistent private reservations without widening visibility. The benign
`RiscvCore::reserve_test_fixed_fu_writeback` extension moves unchanged into the
existing `o3_runtime_writeback_tests` module family because multiple test
modules use the real reservation path for setup.

## Production Boundary

Production source removes:

- `force_test_writeback_reservation_to_memory_result`
- `force_test_writeback_reservation_to_fixed_fu`
- `force_test_writeback_reservation_raw_ready_tick`
- The test-only `RiscvCore` implementation containing
  `reserve_test_fixed_fu_writeback`

Production behavior retains:

- `O3WritebackReplanTransaction`
- `sync_writeback_reservation_owners`
- `validate_live_memory_result_writeback_owner`
- `sync_live_fixed_fu_writeback_owner`
- `WritebackOwnerSourceMismatch` and
  `WritebackOwnerReservationMismatch`

The core setup extension remains available only in a test-only source path and
continues to call `reserve_writeback_completions` with a typed fixed-FU row.

## Invariant Tests

`o3_runtime_writeback/replan_tests.rs` is a child of the private replan module.
It directly covers:

- A fixed-FU owner paired with a memory-result reservation source.
- A completed memory-result owner paired with a fixed-FU reservation source.
- A completed memory-result owner whose reservation raw-ready tick differs
  from `response_tick + 1`.

The tests assert the same error variants and fields as the current tests. They
do not add an unrelated reservation merely to reach owner synchronization.
Existing `owner_validation_error_leaves_writeback_state_unchanged` continues to
cover the transaction wrapper's no-commit behavior after owner validation
fails.

## Source Policy

A focused source-policy test scans non-test Rust paths without stripping
`#[cfg(test)]` items. It rejects the four named helpers when they are defined in
production-owned source files. Test-only paths remain exempt so the legitimate
core setup extension can live under `o3_runtime_writeback_tests/`.

The policy is intentionally narrow. A generic `_for_test` or `#[cfg(test)]`
ban would reject established CPU test setup and read-only inspection helpers
outside this boundary.

## Runtime Evidence

The cleanup changes no runtime selection or scheduling behavior. Representative
real-binary evidence reruns:

- The direct scalar-load/fixed-FU writeback collision, proving architecture is
  blocked until shared-port admission.
- The cache/fabric/DRAM scalar-load/fixed-FU collision, proving the same owner
  relation across the hierarchy route.
- The wrong-path reservation row, proving discarded speculative ownership never
  publishes.

## Files

- `crates/rem6-cpu/src/o3_runtime_writeback.rs`: remove production-owned test
  helpers and declare no new test authority.
- `crates/rem6-cpu/src/o3_runtime_writeback/replan.rs`: declare the private
  child invariant-test module.
- `crates/rem6-cpu/src/o3_runtime_writeback/replan_tests.rs`: directly test
  private owner validation.
- `crates/rem6-cpu/src/o3_runtime_writeback_tests.rs`: remove the three indirect
  corruption tests and declare the test-only core setup module.
- `crates/rem6-cpu/src/o3_runtime_writeback_tests/core.rs`: own the benign
  `RiscvCore` setup extension.
- `crates/rem6-cpu/tests/source_policy.rs`: reject writeback test helpers in
  production-owned source.

## Verification

Verification covers the RED/GREEN source policy, focused private invariant
tests, all `rem6-cpu` targets, representative direct and hierarchy CLI rows,
the full workspace, formatting, removed-helper search, source line budgets,
protected-path checks, and independent read-only review. No migration-ledger
edit is permitted for this increment.
