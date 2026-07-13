# Completed Partial-Overlay O3 Handoff Design

## Status

Approved for implementation on 2026-07-13 under the standing instruction to
continue the current cleanup and migration work.

This document is an implementation design, not a migration-progress ledger.
The only progress authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

Detailed RISC-V execution can currently transfer three bounded scalar-memory
shapes across a detailed-to-timing mode switch:

1. outstanding transport-owned loads,
2. a completed fully forwarded load behind one live store, and
3. a pending partial-overlay load behind one or more live stores.

The fourth bounded shape is deliberately rejected. A partial-overlay load may
receive its transport response, merge the response bytes with forwarded store
bytes, and become ready while one or more source stores remain live. The load
is still resident in the ROB and LSQ because ordered retirement waits for those
stores, but `O3RuntimeState::live_scalar_memory_handoff` returns `None` whenever
the completed load's forwarding plan is partial.

This is not a unit-only state. The existing multi-source CLI fixture reaches it
deterministically on both supported memory paths:

- direct memory: the load completes at tick 356, while later source-store
  responses occur at ticks 388 and 420;
- cache/fabric/DRAM: the load completes at tick 379, while later source-store
  responses occur at ticks 412 and 446.

At a direct tick limit of 372, the live snapshot contains exactly three rows:
two unready stores at PCs `0x8000001c` and `0x80000020`, followed by one ready
load at PC `0x80000024`. The load carries the final value
`0x8877665506dd00aa`, but cannot retire before the stores.

## Alternatives Considered

### Explicit completed-overlay authority

Add a distinct completed partial-overlay row to the live-data handoff schema.
It preserves the completed load identity and final data, the original
forwarding mask, the subset still owned by live stores, and ordered live-source
provenance. This is the selected design because it describes the state without
pretending that a transport-backed partial load was fully forwarded.

### Reuse the fully forwarded row

The existing forwarded row has one source request and one data payload. Using
it would erase response-owned bytes, multi-source provenance, and the
difference between the original forwarding mask and live ownership at transfer
time. That would make the artifact easier to encode but materially less true.

### Delay switching until all stores retire

This avoids the rejected state by moving the host action. It does not implement
the documented completed partial-overlay handoff boundary and provides no new
mode-transfer capability.

## Ledger Target

The target is `CPU Execution Models`, currently 8 of 10 raw and capped at 74%
representative.

The ledger explicitly lists completed partial-overlay handoff as open. This
increment removes that bounded gap with direct and cache/fabric/DRAM execution,
plus a younger-row rejection. It does not provide a general O3 engine,
arbitrary mixed windows, restorable transport ownership, or KVM-style fast
forwarding. The checklist and score therefore remain unchanged.

The ledger update must:

1. record only the executable completed-overlay matrix;
2. keep the general O3 checklist item unchecked;
3. keep the score at 74% representative; and
4. preserve the exact 1,200-line source-policy boundary.

## Goals

1. Capture a completed partial-overlay load while contributing source stores
   remain live.
2. Preserve exact load fetch/data request identity, issue/response ticks,
   address, width, final merged data, O3 sequence, and trace sequence.
3. Preserve the forwarding mask observed when the load completed.
4. Separately preserve the byte mask still owned by live source stores at the
   transfer tick.
5. Preserve ordered source request, address, width, ownership mask, and source
   data for every live contributing store.
6. Transfer the same three-row ROB/LSQ state across detailed-to-timing mode
   switching without changing issue, writeback, or commit ticks.
7. Keep the handoff explicitly non-restorable and outside normal checkpoint
   capture.
8. Reject a completed partial overlay when an unrelated younger O3 row is also
   resident.
9. Decode historical live-data handoff versions 1 through 6 unchanged.
10. Prove the behavior through real `rem6 run --execute` CLI tests.

## Non-Goals

1. Do not make live scalar-memory handoff restorable.
2. Do not replay the completed load from the serialized handoff.
3. Do not broaden the four-row scalar-memory window.
4. Do not admit unrelated memory, branch, vector, float, or MMIO rows.
5. Do not change store drain, forwarding, response merge, or retirement timing.
6. Do not retain retired stores as live transfer entries.
7. Do not change timing-mode O3 suppression.
8. Do not increase migration scores or check the general O3 item.

## Runtime Model

### O3 extraction

`O3RuntimeState::live_scalar_memory_handoff` will return completed partial rows
separately from ordinary resident rows and fully forwarded rows. A completed
partial row is eligible only when:

1. its outcome is `Completed`;
2. the event has not been taken and the row has not committed;
3. response and latency ticks are present;
4. the forwarding plan is partial;
5. final load data is present, has the scalar width, and matches the plan's
   forwarded bytes; and
6. no deferred scalar-memory execution exists.

The internal row carries the original forwarding mask and final merged data.
It does not decide which source stores remain authoritative.

### Core capture

`RiscvCore::capture_o3_live_data_handoff` remains the authority that joins O3
rows to issued transport state. For a completed partial row it will:

1. build ordinary entries only for currently live source stores;
2. require one completed row, no pending partial row, no fully forwarded row,
   and zero unrelated younger rows;
3. require every live entry to be a scalar memory store on the same target and
   partition;
4. recompute youngest-store byte ownership from the live stores and final load
   data;
5. require the live ownership mask to be nonzero, partial, and a subset of the
   original completion-time forwarding mask;
6. require every live store to own at least one final byte; and
7. create one completed-overlay authority row.

Retired stores are not serialized as live entries. Their completed effects are
already represented in the load's final data and in the bytes no longer owned
by the live-source mask.

## Public Handoff Shape

Add `RiscvO3LiveDataHandoffCompletedPartialOverlay` with:

- load fetch and data request IDs;
- issue and response ticks;
- load address and scalar byte width;
- `original_forwarded_mask` from load completion;
- `live_forwarded_mask` owned by serialized source stores;
- final merged load data;
- O3 sequence and optional trace sequence; and
- ordered `RiscvO3LiveDataHandoffPartialOverlaySource` rows for live sources.

The full-width mask minus `original_forwarded_mask` is the original transport
response mask. `original_forwarded_mask & !live_forwarded_mask` is the retired
forwarded mask: bytes that came from older stores at load completion but whose
stores have retired before transfer. These masks must remain distinct. The
full-width complement of `live_forwarded_mask` is only a non-live mask and must
not be labeled response-owned.

`RiscvO3LiveDataHandoff` gains a `completed_partial_overlays` collection. The
bounded current schema permits exactly one of these shapes:

1. ordinary outstanding loads;
2. one live store plus one fully forwarded completed load;
3. two to four transport rows with one pending partial overlay; or
4. one to three live source stores plus one completed partial overlay.

No shape may mix forwarded, pending-overlay, and completed-overlay rows.
`resident_rows()` counts the completed load because it remains in the ROB and
LSQ even though its transport request has finished.

## Wire Format

Version 7 becomes the current `o3-live-data-handoff` schema. The header adds a
`completed_partial_overlay_count` after the existing pending-overlay count.
Versions 1 through 6 retain their exact historical layouts and validation.

Each completed-overlay record encodes:

1. load fetch request;
2. load data request;
3. issue tick;
4. response tick;
5. address and byte width;
6. original and live forwarding masks;
7. fixed eight-byte final-data storage with zero padding beyond the width;
8. O3 sequence;
9. trace presence and sequence;
10. source count; and
11. ordered source request, ownership mask, and canonical source data.

Source address and width remain derived from matching live store entries, as
they are for current multi-source pending overlays. Decode rejects duplicate
request or sequence identity, invalid scalar widths, nonzero padding,
response-before-issue timing, masks outside the scalar width, empty/full
original masks, empty/full live masks, live masks outside the original mask, a
completed load sequence that is not younger than every source-store sequence,
missing or extra sources, overlapping ownership, incomplete live-mask
ownership, source data mismatches, and invalid row-count combinations.

Version 6 receives an explicit legacy constant and a round-trip regression so
the version bump cannot silently drop current multi-source payload support.

## CLI Summary

The decoded HostAction chunk summary keeps pending and completed overlays
distinct. Add:

- `completed_partial_overlay_rows`;
- `completed_partial_overlay_source_rows`;
- first completed load request IDs;
- issue and response ticks;
- address and bytes;
- original-forwarded, original-response, live-forwarded, and retired-forwarded
  masks;
- original and live forwarded-byte counts;
- final merged data;
- O3 and trace sequence; and
- ordered live-source summaries.

`outstanding_requests` continues to count transport-owned live entries only.
For the representative completed row it is one: the middle store is submitted
to transport while the youngest store remains buffered. `resident_rows` is
three because both stores and the completed load remain in the O3 window.

Generic `last_issue_tick` must include completed-overlay issue ticks in addition
to ordinary entries and fully forwarded rows, so it continues to identify the
youngest issued row for every handoff family.

The same decoded JSON must appear in both the execution-mode switch artifact
and the HostAction debug trace. Existing pending-overlay and fully forwarded
fields remain behaviorally unchanged.

## Test-First Matrix

### Focused CPU tests

1. Replace the current suppression test with a red test proving a completed
   partial row is extracted with final data and original mask.
2. Add a capture/codec round trip with two live source stores, one completed
   load, original mask `0x0f`, original response mask `0xf0`, live mask `0x0c`,
   retired-forwarded mask `0x03`, and final data `aa00dd0655667788`.
3. Decode a version-6 multi-source pending overlay and prove its old shape is
   unchanged.
4. Reject a completed row with no live source, a live mask outside the original
   mask, an empty or full original mask, a noncontributing source, a completed
   sequence not younger than every source, mismatched final/source data,
   duplicate identity, nonzero padding, response-before-issue, or unrelated
   younger rows.

### Top-level CLI tests

Reuse the existing three-store/eight-byte-load fixture.

1. Direct completed handoff: schedule the switch after load response 356 and
   before source-store response 388.
2. Cache/fabric/DRAM completed handoff: schedule the switch after load response
   379 and before source-store response 412.
3. In both rows assert three transferred ROB/LSQ residents, two outstanding
   source stores, one transport-owned row, one buffered row, one completed
   overlay, exact original/response/live/retired masks, exact final data and
   source provenance, baseline issue/writeback/commit ticks, final
   registers/memory, one forwarding match, and route-appropriate resource
   activity.
4. Assert the HostAction debug transfer decodes to exactly the same completed
   overlay JSON as the run artifact.
5. Direct negative row: use the existing younger-ALU fixture, switch after the
   load response while a source store and the ALU remain resident, and require
   the existing non-quiescent `cpu0` rejection.

Committed-instruction counts alone are insufficient. The tests must retain
exact byte, request, mask, timing, row-count, debug, and resource witnesses.

## Failure Handling

1. If the live store set cannot reconstruct a nonzero subset of the original
   forwarded mask, reject capture rather than inventing provenance.
2. If a live store contributes no final byte, reject the bounded shape rather
   than serializing unrelated authority.
3. If the completed load has already committed, omit it from handoff.
4. If any unrelated younger row is resident, retain the existing quiescence
   failure.
5. If version-6 decode changes, fix compatibility before proceeding.
6. If direct and hierarchy timing no longer provide a post-load/pre-store
   interval, fail the test with the observed ticks rather than hard-coding a
   stale switch point.

## Expected Files

Production changes should remain focused in:

1. `crates/rem6-cpu/src/o3_runtime_handoff.rs`
2. `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`
3. `crates/rem6-cpu/src/riscv_execution_mode_handoff/partial_overlay.rs`
4. `crates/rem6-cpu/src/riscv_execution_mode_handoff/codec.rs`
5. `crates/rem6/src/host_actions/live_data_handoff.rs`

CLI evidence remains in
`crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs`.
Core anchor and ledger text may change only after executable evidence passes.

## Verification

Focused verification:

```text
cargo test -p rem6-cpu --lib completed_partial_overlay
cargo test -p rem6-cpu --lib live_data_handoff_round_trips_completed_partial_overlay
cargo test -p rem6 --test cli_run completed_multi_source_partial_forwarded_store_load
cargo test -p rem6 --test cli_run rejects_completed_partial_forwarded_store_load_with_younger_row
```

Completion verification:

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
cargo test --workspace --all-targets --quiet
git diff --check
```

The increment is complete only when the full matrix passes, the ledger remains
honest and exactly 1,200 lines, and a high-intensity read-only review finds no
unresolved correctness, compatibility, or abstraction issue.
