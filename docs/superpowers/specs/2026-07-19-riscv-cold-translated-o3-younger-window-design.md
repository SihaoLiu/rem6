# RISC-V Cold Translated O3 Younger Window Design

## Context

Detailed O3 scalar-memory windows already admit a cacheable untranslated load
and a TLB-cached translated load with a bounded younger scalar-ALU suffix. A
nonzero-latency cold data-translation miss remains terminal even after the
translation succeeds and the physical memory request issues.

The difference is driver ordering. Cached translated fetch-ahead records the
load fetch request in `translated_scalar_load_window_fetches` and fetches
the bounded suffix before the load executes. A cold miss executes the load,
waits for translation, and then the serial and parallel drivers consume the
ready translation into data issue immediately. No turn exists in which the
validated physical target can authorize and fetch the younger suffix.

## Ledger Boundary

This increment closes the named `cold-miss translated younger windows` gap in
`CPU Execution Models - 74% representative`. It does not provide a general O3
engine, restorable in-flight transport ownership, or broader translated
multicore/device shapes. The CPU checklist remains 8 of 10 and the score stays
at the 74% representative cap.

The migration ledger updates only executable evidence, remaining-gap text,
and test anchors. Its line count remains exactly 1,200.

## Approaches

### Restore the translated-window marker at translation completion

Restoring the marker directly in translation completion would occur before
later PMP, PMA, route, line-layout, target, and request checks finish. It could
leave stale window intent across a later validation failure or an MMIO target.

### Preserve pending translation in handoff schema v7

Schema v7 already represents one resident physical memory load plus a younger
row count after translation has completed. Preserving pending translation,
virtual addresses, or TLB provenance would broaden the handoff contract and
make currently non-restorable frontend state restorable. That is outside this
increment.

### Authorize and fetch after validation but before data issue

This is the selected design. Translation advancement is split from consuming
the ready access. The driver validates the translated request through the same
PMP, PMA, route, line-layout, target, and request preparation used by data
issue. Only a cacheable scalar integer load targeting ordinary memory may then
establish translated-window authority and fetch the bounded scalar suffix.

The existing marker remains the issue-time proof that translated fetch-ahead
was authorized. The cold path retains the ready translated access while one
fetch is pending, repeats bounded fetch-ahead until the window is full or
terminated, and only then consumes the physical request into data issue.

## Runtime Behavior

The production change keeps the condition that a translated load must be in
`translated_scalar_load_window_fetches` before selecting the scalar
memory prefix policy. A new ready-translation phase establishes that marker
only after validation and drives the existing scalar-window classifier. The
gates are:

- Detailed O3 owns the deferred data access.
- The result is not a provisional terminal memory-result shape.
- The access is a scalar load with a nonzero integer destination.
- The translated target is ordinary memory, not MMIO.
- The physical PMA range is cacheable.
- The translation and all pre-issue validation have succeeded.
- No authorized translated-window fetch remains outstanding.

PMA-uncacheable translated loads remain one-row terminal windows. Translation
faults never reach data issue. Translated MMIO remains terminal because its
target is not memory. A younger memory instruction still terminates the scalar
ALU suffix under the existing O3 window policy.

## Serial And Parallel Paths

Shared translation advancement and request validation live in
`riscv_translation`. Shared ready-window classification lives in
`riscv_fetch_ahead`. The serial translated driver, parallel translated memory
driver, and parallel MMIO-aware translated driver all invoke that authority
before consuming a ready memory request. The MMIO-aware path probes and issues
mapped MMIO first, so only an unmapped ordinary-memory target can open the
window. The top-level `rem6 run` matrix proves both direct and hierarchy
parallel paths. A nonzero-latency serial integration row proves the driver
issues the authorized younger fetch before data issue and waits while that
fetch remains pending.

## Focused CPU Evidence

The existing cold translated unit test becomes a positive:

`detailed_translated_cold_cacheable_scalar_load_stages_younger_after_translation_completion`

It executes a cold translated load, completes one younger fetch, advances and
validates the translation, establishes ready-window authority, issues the
physical data request without a response, and requires ROB PCs `0x8000` and
`0x8004` with one LSQ row.

The existing PMA-uncacheable translated test remains a negative and requires
only the load row. Existing translation fault and translated MMIO tests remain
additional suppression coverage.
`riscv_core_translated_driver_fetches_cold_younger_window_before_data_issue`
locks the serial translation-ready ordering and two-row staged result.

## CLI Matrix

All new real-binary rows live in
`tests/cli_run/m5_host_actions/o3/switch/translated_scalar_load.rs`.

### Direct handoff

`rem6_run_host_switch_transfers_cold_translated_scalar_load_younger_window_direct`
uses an empty TLB and nonzero translation latency. Before the physical load
response, one load and three independent scalar ALUs are resident. A scheduled
detailed-to-timing switch transfers schema-v7 authority with four ROB rows,
one LSQ row, one outstanding memory request, and three younger rows.

### Hierarchy handoff

`rem6_run_host_switch_transfers_cold_translated_scalar_load_younger_window_cache_fabric_dram`
proves the same shape through cache, transport, fabric, and DRAM activity.

Both rows preserve baseline issue/writeback/commit timing, bind the handoff
route and typed target to the exact pre-switch physical request, preserve
translated address `0x80000080`, final registers, and final memory.
The fixture stores the three ALU results at offsets 4, 8, and 12 after the load
completes, producing the byte witness
`2a00000005000000100000003a000000`.

### Live checkpoint rejection

`rem6_run_rejects_live_cold_translated_scalar_load_younger_window_checkpoint`
schedules a checkpoint between physical load issue and response. It first
proves the four-row baseline, then requires fail-closed stderr and empty
stdout because CPU0 is not quiescent.

### Drained restore and stats

`rem6_run_restores_drained_cold_translated_scalar_load_younger_window_and_stats`
uses a dedicated fixture that executes the cold window, stores all witnesses,
clears the m5 delay/period registers, issues `m5_checkpoint`, dumps stats, and
increments `x16`. A scheduled restore after that increment commits replays the
post-checkpoint dump and increment. Final `x16=1` proves restored register state
was applied rather than merely replaying control from the checkpoint PC.
Capture and restore O3 payloads match, contain zero live ROB/LSQ rows, preserve
max ROB occupancy four and max LSQ occupancy one, and preserve one load plus
three store operation counts and their gem5-style aliases across both dumps.

### Timing suppression

`rem6_run_timing_suppresses_cold_translated_scalar_load_younger_window_o3_artifacts`
runs the same architectural work with the m5 switch entering timing mode. It
requires the same register/memory witness while omitting the O3 runtime,
per-event trace, raw O3 stats, and gem5-style O3 aliases.

## Source Policy

`translated_scalar_load.rs` has 1,115 lines after this matrix. A focused source-policy
budget of 1,400 lines leaves room for this matrix while preventing it from
quietly reaching the generic 1,800-line child ceiling. New CLI names are added
to `core_test_anchors.txt` and to the migration ledger.

## Files

- `crates/rem6-cpu/src/riscv_translation.rs`: split translation advancement
  from issue consumption and validate ready scalar-load targets.
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`: classify the bounded
  ready translated scalar-load suffix.
- `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`: retain validated window
  authority while bounded fetches complete.
- `crates/rem6-cpu/src/riscv_cluster.rs`: invoke the shared phase from both
  parallel translated drivers.
- `crates/rem6-cpu/src/riscv_cluster_drive.rs` and
  `crates/rem6-cpu/src/riscv_cluster_translation.rs`: keep generic drive
  helpers and translation-specific turn ownership out of the cluster root.
- `crates/rem6-cpu/src/riscv_data_issue_tests/translated.rs`: flip the cold
  focused test to a positive while retaining the uncacheable negative.
- `crates/rem6-cpu/tests/riscv_translation_frontend.rs`: prove nonzero-latency
  serial fetch-before-issue ordering.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/translated_scalar_load.rs`:
  own the five-row CLI matrix and fixtures.
- `crates/rem6/tests/source_policy.rs`: add the focused module budget.
- `crates/rem6/tests/source_policy/core_test_anchors.txt`: replace the old
  cold terminal anchor and add the new lifecycle anchors.
- `docs/architecture/gem5-to-rem6-migration.md`: update executable evidence
  and remove only the closed cold-miss gap.

## Verification

Verification covers observed RED/GREEN focused and CLI positives, the PMA
negative, translated MMIO suppression, direct and hierarchy handoff, live
checkpoint failure, drained restore/stats, timing suppression, all affected
CPU/rem6 targets, the full workspace, source policy, ledger mechanics, and
independent high-intensity read-only review.
