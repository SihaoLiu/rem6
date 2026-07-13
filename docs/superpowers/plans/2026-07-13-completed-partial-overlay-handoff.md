# Completed Partial-Overlay O3 Handoff Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transfer a completed transport-backed partial-overlay scalar load together with its still-live source stores across a detailed-to-timing CPU mode switch, preserving exact byte provenance, timing, O3 residency, and historical schema compatibility.

**Architecture:** `O3RuntimeState` extracts completed partial-load identity and final data without deciding source ownership. `RiscvCore` joins that row to currently live issued stores, recomputes live youngest-store ownership, and builds a distinct version-7 completed-overlay authority. The codec preserves version 1-6 decoding, HostAction output exposes original/live/retired byte masks, and real direct plus cache/fabric/DRAM CLI tests prove the three-row transfer and younger-row rejection.

**Tech Stack:** Rust 2021 workspace, `rem6-cpu`, `rem6-system`, `rem6` CLI integration tests, typed checkpoint chunks, O3 ROB/LSQ snapshots, JSON HostAction artifacts, Cargo tests, and source-policy ledger gates.

---

## File Map

- Modify `crates/rem6-cpu/src/o3_runtime_handoff.rs`: extract completed partial-load rows from live O3 state.
- Modify `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`: add the completed-row internal contract, current handoff collection, constructor, capture join, and errors.
- Modify `crates/rem6-cpu/src/riscv_execution_mode_handoff/partial_overlay.rs`: own the public completed-overlay type and live-source recomposition helper.
- Modify `crates/rem6-cpu/src/riscv_execution_mode_handoff/codec.rs`: encode/decode version 7 and preserve version 1-6 layouts.
- Create `crates/rem6-cpu/src/riscv_execution_mode_handoff/completed_partial_overlay_tests.rs`: focused domain, codec, compatibility, and negative tests.
- Modify `crates/rem6-cpu/src/public_api.rs`: export the completed-overlay type through the crate facade.
- Modify `crates/rem6/src/host_actions/live_data_handoff.rs`: expose completed-overlay JSON without changing pending/full-forwarded fields.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs`: add direct/hierarchy completed rows and the younger-row negative.
- Modify existing handoff CLI tests that hard-code schema version 6: update only current-writer expectations to version 7.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: replace the old suppression anchor and register executable completed-overlay evidence.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record the delivered matrix without changing 74%, 8/10, or checklist state.

## Invariants To Preserve

- Historical versions 1 through 6 decode exactly as before.
- A completed overlay has at least one live source store and exactly one completed load.
- `original_forwarded_mask` is nonzero, not full-width, and limited to the scalar width.
- `live_forwarded_mask` is nonzero, not full-width, and a subset of the original mask.
- `retired_forwarded_mask = original_forwarded_mask & !live_forwarded_mask`.
- `original_response_mask = scalar_mask & !original_forwarded_mask`.
- Every serialized live source owns at least one final byte.
- Live-source ownership masks are disjoint and their union is `live_forwarded_mask`.
- The completed load sequence is younger than every source-store sequence.
- The completed load response cannot precede its issue tick.
- Final data and source data are zero-padded beyond their scalar widths.
- No shape mixes fully forwarded, pending-overlay, and completed-overlay rows.
- `resident_rows` counts the completed load; `outstanding_requests` counts transport-owned live stores only.
- The representative post-load state is one transport-owned store, one buffered store, and one completed load.

### Task 1: Extract Completed Partial Rows From O3 Runtime

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_handoff.rs`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`

- [ ] **Step 1: Replace the suppression test with a failing extraction test**

Rename `completed_partial_overlay_is_not_live_handoff_authority` to
`completed_partial_overlay_becomes_live_handoff_authority`. Keep the existing
store/load setup, then assert the completed row is returned separately:

```rust
#[test]
fn completed_partial_overlay_becomes_live_handoff_authority() {
    let mut runtime = O3RuntimeState::default();
    let store = scalar_store_event(0x8000, 10, 0x9001);
    let load = scalar_load_event(0x8004, 11, 0x9000);
    assert!(runtime.stage_live_scalar_memory_issue(&store, memory_request(20), 31));
    let plan = runtime
        .scalar_load_forwarding_plan(
            load.instruction(),
            load.execution().memory_access().unwrap(),
        )
        .expect("byte store should partially overlay the word load");
    assert!(plan.is_partial());
    assert!(runtime.stage_live_scalar_memory_issue(&load, memory_request(21), 32));
    let mut completed = load.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime.complete_live_scalar_memory_forwarding(
        &completed,
        memory_request(21),
        40,
        8,
        &[0x11, 0x5a, 0x33, 0x80],
        plan,
    ));

    let (resident, forwarded, completed_partial, younger) = runtime
        .live_scalar_memory_handoff()
        .expect("completed partial row should be transferable");
    assert_eq!(resident.len(), 1);
    assert!(forwarded.is_empty());
    assert_eq!(completed_partial.len(), 1);
    assert_eq!(younger, 0);
    assert_eq!(completed_partial[0].fetch_request, load.fetch().request_id());
    assert_eq!(completed_partial[0].data_request, memory_request(21));
    assert_eq!(completed_partial[0].issue_tick, 32);
    assert_eq!(completed_partial[0].response_tick, 40);
    assert_eq!(completed_partial[0].address, Address::new(0x9000));
    assert_eq!(completed_partial[0].bytes, 4);
    assert_eq!(completed_partial[0].original_forwarded_mask, 0b0010);
    assert_eq!(&completed_partial[0].data[..4], &[0x11, 0x5a, 0x33, 0x80]);
    assert_eq!(completed_partial[0].o3_sequence, 1);
}
```

- [ ] **Step 2: Run the red unit test**

Run:

```text
cargo test -p rem6-cpu --lib completed_partial_overlay_becomes_live_handoff_authority -- --nocapture
```

Expected: compile failure because `live_scalar_memory_handoff` still returns a
three-tuple and no completed-partial internal row exists.

- [ ] **Step 3: Add the internal completed-row contract**

In `riscv_execution_mode_handoff.rs`, add beside
`RiscvForwardedScalarLoadHandoff`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvCompletedPartialScalarLoadHandoff {
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) data_request: MemoryRequestId,
    pub(crate) issue_tick: Tick,
    pub(crate) response_tick: Tick,
    pub(crate) address: Address,
    pub(crate) bytes: u32,
    pub(crate) original_forwarded_mask: u8,
    pub(crate) data: [u8; 8],
    pub(crate) o3_sequence: u64,
    pub(crate) trace_sequence: Option<u64>,
}
```

- [ ] **Step 4: Return completed partial rows from the runtime**

Change `live_scalar_memory_handoff` to return:

```rust
Option<(
    Vec<RiscvResidentScalarMemoryHandoff>,
    Vec<RiscvForwardedScalarLoadHandoff>,
    Vec<RiscvCompletedPartialScalarLoadHandoff>,
    usize,
)>
```

Inside the loop, keep the resident case unchanged. For completed loads, split
the current full-forwarded branch:

```rust
let forwarding_plan = live.forwarding_plan?;
if live.outcome != O3LiveScalarMemoryOutcome::Completed
    || live.event_taken
    || live.commit_tick.is_some()
    || !matches!(
        live.execution.execution().memory_access(),
        Some(MemoryAccessKind::Load { .. })
    )
{
    return None;
}
let response_tick = live.response_tick?;
live.latency_ticks?;
let data = live.load_data.as_deref()?;
let bytes = forwarding_plan.bytes();
if data.len() != bytes as usize || !forwarding_plan.matches_data(data) {
    return None;
}
let mut fixed_data = [0; 8];
fixed_data[..data.len()].copy_from_slice(data);

if forwarding_plan.is_partial() {
    completed_partial_rows.push(RiscvCompletedPartialScalarLoadHandoff {
        fetch_request: live.fetch_request,
        data_request: live.data_request,
        issue_tick: live.issue_tick,
        response_tick,
        address: forwarding_plan.load_range().start(),
        bytes,
        original_forwarded_mask: forwarding_plan.forwarded_mask(),
        data: fixed_data,
        o3_sequence: live.sequence,
        trace_sequence,
    });
    continue;
}
```

Keep the existing full-forwarded row construction after this branch.

- [ ] **Step 5: Run the focused unit tests**

Run:

```text
cargo test -p rem6-cpu --lib completed_partial_overlay -- --nocapture
cargo test -p rem6-cpu --lib o3_runtime_handoff -- --nocapture
```

Expected: the extraction test passes. Capture callers may still fail to compile
until Task 2 consumes the new tuple member.

### Task 2: Build Completed Overlay Authority From Live Sources

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff/partial_overlay.rs`
- Create: `crates/rem6-cpu/src/riscv_execution_mode_handoff/completed_partial_overlay_tests.rs`

- [ ] **Step 1: Register the focused test module**

At module scope in `riscv_execution_mode_handoff.rs`, add:

```rust
#[cfg(test)]
#[path = "riscv_execution_mode_handoff/completed_partial_overlay_tests.rs"]
mod completed_partial_overlay_tests;
```

- [ ] **Step 2: Add failing public-shape tests**

Create `completed_partial_overlay_tests.rs` with `use super::*;` and these
focused constructors:

```rust
fn request(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
}

fn store_entry(
    sequence: u64,
    address: u64,
    bytes: u32,
    ownership: RiscvO3LiveDataHandoffOwnership,
) -> RiscvO3LiveDataHandoffEntry {
    RiscvO3LiveDataHandoffEntry {
        fetch_request: request(3, sequence),
        data_request: request(4, sequence + 10),
        issue_tick: 20 + sequence,
        partition: PartitionId::new(2),
        operation: RiscvO3LiveDataHandoffOperation::Store,
        ownership,
        target: RiscvO3LiveDataHandoffTarget::Memory {
            route: MemoryRouteId::new(7),
        },
        address: Address::new(address),
        bytes,
        o3_sequence: sequence,
        trace_sequence: Some(sequence + 100),
    }
}

fn load_entry(sequence: u64, address: u64, bytes: u32) -> RiscvO3LiveDataHandoffEntry {
    RiscvO3LiveDataHandoffEntry {
        operation: RiscvO3LiveDataHandoffOperation::Load,
        ownership: RiscvO3LiveDataHandoffOwnership::Transport,
        address: Address::new(address),
        bytes,
        ..store_entry(
            sequence,
            address,
            bytes,
            RiscvO3LiveDataHandoffOwnership::Transport,
        )
    }
}

fn source(
    entry: RiscvO3LiveDataHandoffEntry,
    ownership_mask: u8,
    source_data: [u8; 8],
) -> RiscvO3LiveDataHandoffPartialOverlaySource {
    RiscvO3LiveDataHandoffPartialOverlaySource {
        source_data_request: entry.data_request,
        source_address: entry.address,
        source_bytes: entry.bytes,
        ownership_mask,
        source_data,
    }
}

fn completed_overlay(
    sequence: u64,
    address: u64,
    bytes: u32,
    original_forwarded_mask: u8,
    live_forwarded_mask: u8,
    data: [u8; 8],
    sources: Vec<RiscvO3LiveDataHandoffPartialOverlaySource>,
) -> RiscvO3LiveDataHandoffCompletedPartialOverlay {
    RiscvO3LiveDataHandoffCompletedPartialOverlay {
        fetch_request: request(3, sequence),
        load_data_request: request(4, sequence + 10),
        issue_tick: 20 + sequence,
        response_tick: 40 + sequence,
        address: Address::new(address),
        bytes,
        original_forwarded_mask,
        live_forwarded_mask,
        data,
        o3_sequence: sequence,
        trace_sequence: Some(sequence + 100),
        sources,
    }
}

fn representative_completed_partial_handoff() -> RiscvO3LiveDataHandoff {
    let middle = store_entry(6, 0x8000_0102, 2, RiscvO3LiveDataHandoffOwnership::Transport);
    let youngest = store_entry(
        7,
        0x8000_0102,
        1,
        RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: middle.data_request,
        },
    );
    let completed = completed_overlay(
        8,
        0x8000_0100,
        8,
        0x0f,
        0x0c,
        [0xaa, 0x00, 0xdd, 0x06, 0x55, 0x66, 0x77, 0x88],
        vec![
            source(middle, 0x08, [0x00, 0x06, 0, 0, 0, 0, 0, 0]),
            source(youngest, 0x04, [0xdd, 0, 0, 0, 0, 0, 0, 0]),
        ],
    );
    RiscvO3LiveDataHandoff::with_completed_partial_overlay(
        vec![middle, youngest],
        completed,
        0,
    )
    .expect("representative completed partial handoff")
}

fn v6_multi_source_pending_handoff() -> RiscvO3LiveDataHandoff {
    let middle = store_entry(6, 0x8000_0102, 2, RiscvO3LiveDataHandoffOwnership::Transport);
    let youngest = store_entry(
        7,
        0x8000_0102,
        1,
        RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: middle.data_request,
        },
    );
    let load = load_entry(8, 0x8000_0100, 8);
    let overlay = RiscvO3LiveDataHandoffPartialOverlay {
        load_data_request: load.data_request,
        address: load.address,
        bytes: load.bytes,
        forwarded_mask: 0x0c,
        data: [0, 0, 0xdd, 0x06, 0, 0, 0, 0],
        sources: vec![
            source(middle, 0x08, [0x00, 0x06, 0, 0, 0, 0, 0, 0]),
            source(youngest, 0x04, [0xdd, 0, 0, 0, 0, 0, 0, 0]),
        ],
    };
    RiscvO3LiveDataHandoff::with_partial_overlay(
        vec![middle, youngest, load],
        overlay,
        0,
    )
    .expect("version-6 multi-source pending handoff")
}
```

Add a representative test that constructs two live stores and one completed
load:

```rust
#[test]
fn completed_partial_overlay_tracks_original_live_and_retired_masks() {
    let middle = store_entry(6, 0x8000_0102, 2, RiscvO3LiveDataHandoffOwnership::Transport);
    let youngest = store_entry(
        7,
        0x8000_0102,
        1,
        RiscvO3LiveDataHandoffOwnership::BufferedStore {
            predecessor: middle.data_request(),
        },
    );
    let completed = completed_overlay(
        8,
        0x8000_0100,
        8,
        0x0f,
        0x0c,
        [0xaa, 0x00, 0xdd, 0x06, 0x55, 0x66, 0x77, 0x88],
        vec![
            source(middle, 0x08, [0x00, 0x06, 0, 0, 0, 0, 0, 0]),
            source(youngest, 0x04, [0xdd, 0, 0, 0, 0, 0, 0, 0]),
        ],
    );

    let handoff = RiscvO3LiveDataHandoff::with_completed_partial_overlay(
        vec![middle, youngest],
        completed,
        0,
    )
    .expect("representative completed overlay");

    assert_eq!(handoff.resident_rows(), 3);
    assert_eq!(handoff.completed_partial_overlays().len(), 1);
    let row = &handoff.completed_partial_overlays()[0];
    assert_eq!(row.original_forwarded_mask(), 0x0f);
    assert_eq!(row.original_response_mask(), 0xf0);
    assert_eq!(row.live_forwarded_mask(), 0x0c);
    assert_eq!(row.retired_forwarded_mask(), 0x03);
    assert_eq!(row.data(), [0xaa, 0x00, 0xdd, 0x06, 0x55, 0x66, 0x77, 0x88]);
}
```

Also add constructor-negative tests for empty/full original mask, empty/full live
mask, live outside original, noncontributing source, source ownership outside
its physical overlap, incomplete/overlapping ownership, load sequence not
younger than sources, response-before-issue, and nonzero data padding.

- [ ] **Step 3: Run the red public-shape tests**

Run:

```text
cargo test -p rem6-cpu --lib completed_partial_overlay_tests -- --nocapture
```

Expected: compile failure for the missing public type, collection, constructor,
and accessors.

- [ ] **Step 4: Add the public completed-overlay type**

In `partial_overlay.rs`, add:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvO3LiveDataHandoffCompletedPartialOverlay {
    pub(super) fetch_request: MemoryRequestId,
    pub(super) load_data_request: MemoryRequestId,
    pub(super) issue_tick: Tick,
    pub(super) response_tick: Tick,
    pub(super) address: Address,
    pub(super) bytes: u32,
    pub(super) original_forwarded_mask: u8,
    pub(super) live_forwarded_mask: u8,
    pub(super) data: [u8; 8],
    pub(super) o3_sequence: u64,
    pub(super) trace_sequence: Option<u64>,
    pub(super) sources: Vec<RiscvO3LiveDataHandoffPartialOverlaySource>,
}
```

Add accessors for every field plus:

```rust
pub const fn original_response_mask(&self) -> u8 {
    scalar_byte_mask(self.bytes) & !self.original_forwarded_mask
}

pub const fn retired_forwarded_mask(&self) -> u8 {
    self.original_forwarded_mask & !self.live_forwarded_mask
}

pub const fn original_forwarded_bytes(&self) -> u32 {
    self.original_forwarded_mask.count_ones()
}

pub const fn live_forwarded_bytes(&self) -> u32 {
    self.live_forwarded_mask.count_ones()
}
```

Import `Tick` through `super` or directly. Re-export the type from
`riscv_execution_mode_handoff.rs` beside the pending overlay types, then add
`RiscvO3LiveDataHandoffCompletedPartialOverlay` to the existing
`pub use crate::riscv_execution_mode_handoff::{...}` list in
`crates/rem6-cpu/src/public_api.rs`.

- [ ] **Step 5: Add live-source recomposition**

In `partial_overlay.rs`, add:

```rust
pub(super) fn compose_completed_partial_overlay_sources(
    sources: &[RiscvIssuedScalarMemoryHandoff],
    address: Address,
    bytes: u32,
    original_forwarded_mask: u8,
    final_data: &[u8; 8],
) -> Option<(u8, Vec<RiscvO3LiveDataHandoffPartialOverlaySource>)> {
    let scalar_mask = scalar_byte_mask(bytes);
    if !matches!(bytes, 1 | 2 | 4 | 8)
        || original_forwarded_mask == 0
        || original_forwarded_mask == scalar_mask
        || original_forwarded_mask & !scalar_mask != 0
        || final_data[bytes as usize..].iter().any(|value| *value != 0)
    {
        return None;
    }

    let live_forwarded_mask = sources.iter().try_fold(0_u8, |mask, source| {
        let source_mask = source.store_data.map(|_| {
            partial_overlay_mask(source.address, source.bytes, address, bytes)
        })?;
        (source_mask != 0 && source_mask & !original_forwarded_mask == 0)
            .then_some(mask | source_mask)
    })?;
    if live_forwarded_mask == 0 || live_forwarded_mask == scalar_mask {
        return None;
    }

    let mut live_data = [0; 8];
    for index in 0..bytes as usize {
        if live_forwarded_mask & (1 << index) != 0 {
            live_data[index] = final_data[index];
        }
    }
    let sources = compose_partial_overlay_sources(
        sources,
        RiscvPendingPartialScalarLoadHandoff {
            address,
            bytes,
            forwarded_mask: live_forwarded_mask,
            data: live_data,
        },
    )?;
    sources
        .iter()
        .all(|source| source.ownership_mask != 0)
        .then_some((live_forwarded_mask, sources))
}
```

Also add one structural validator shared by the in-memory constructor and the
version-7 decoder:

```rust
pub(super) fn completed_partial_overlay_is_valid(
    entries: &[RiscvO3LiveDataHandoffEntry],
    overlay: &RiscvO3LiveDataHandoffCompletedPartialOverlay,
) -> bool {
    let scalar_mask = scalar_byte_mask(overlay.bytes);
    if entries.is_empty()
        || entries.len() >= MAX_ROWS
        || !matches!(overlay.bytes, 1 | 2 | 4 | 8)
        || overlay.response_tick < overlay.issue_tick
        || overlay.original_forwarded_mask == 0
        || overlay.original_forwarded_mask == scalar_mask
        || overlay.original_forwarded_mask & !scalar_mask != 0
        || overlay.live_forwarded_mask == 0
        || overlay.live_forwarded_mask == scalar_mask
        || overlay.live_forwarded_mask & !overlay.original_forwarded_mask != 0
        || overlay.data[overlay.bytes as usize..]
            .iter()
            .any(|value| *value != 0)
        || overlay.sources.len() != entries.len()
        || entries.iter().any(|entry| {
            entry.operation != RiscvO3LiveDataHandoffOperation::Store
                || entry.partition != entries[0].partition
                || entry.target != entries[0].target
        })
        || entries
            .iter()
            .any(|entry| entry.o3_sequence >= overlay.o3_sequence)
    {
        return false;
    }

    let mut owned = 0_u8;
    for (entry, source) in entries.iter().zip(&overlay.sources) {
        let physical_mask = partial_overlay_mask(
            source.source_address,
            source.source_bytes,
            overlay.address,
            overlay.bytes,
        );
        if physical_mask == 0
            || source.ownership_mask & !physical_mask != 0
            || entry.data_request != source.source_data_request
            || entry.address != source.source_address
            || entry.bytes != source.source_bytes
            || source.ownership_mask == 0
            || source.ownership_mask & !overlay.live_forwarded_mask != 0
            || owned & source.ownership_mask != 0
            || validate_partial_overlay_data(
                source.source_address,
                source.source_bytes,
                overlay.address,
                overlay.bytes,
                source.ownership_mask,
                &overlay.data,
                &source.source_data,
            )
            .is_err()
        {
            return false;
        }
        owned |= source.ownership_mask;
    }
    owned == overlay.live_forwarded_mask
}
```

- [ ] **Step 6: Add the handoff collection and constructor**

In `RiscvO3LiveDataHandoff`, add:

```rust
completed_partial_overlays: Vec<RiscvO3LiveDataHandoffCompletedPartialOverlay>,
```

Initialize it to `Vec::new()` in all existing constructors and direct test
struct literals. The compiler should report every historical literal that must
be updated; no historical test may receive a nonempty completed collection.
Add:

```rust
fn with_completed_partial_overlay(
    entries: Vec<RiscvO3LiveDataHandoffEntry>,
    overlay: RiscvO3LiveDataHandoffCompletedPartialOverlay,
    younger_rows: usize,
) -> Option<Self> {
    if entries.is_empty()
        || entries.len() >= MAX_ROWS
        || younger_rows != 0
        || !completed_partial_overlay_is_valid(&entries, &overlay)
    {
        return None;
    }
    Some(Self {
        entries,
        forwarded_rows: Vec::new(),
        partial_overlays: Vec::new(),
        completed_partial_overlays: vec![overlay],
        younger_rows: 0,
    })
}
```

Add `completed_partial_overlays()` and update `resident_rows()` to add the
completed collection length.

- [ ] **Step 7: Join completed rows to live issued stores during capture**

Update the tuple destructure in `capture_o3_live_data_handoff`. After entries
and `issued_rows` are built, keep the existing `entries.sort_by_key` call before
all overlay branches. Build completed sources by walking those O3-sorted entries
so youngest-store precedence never depends on map iteration order. Handle
completed rows before pending rows:

```rust
if !completed_partial_rows.is_empty() {
    if completed_partial_rows.len() != 1
        || !forwarded_rows.is_empty()
        || issued_rows.iter().any(|issued| issued.partial_overlay.is_some())
        || entries.is_empty()
        || entries.len() >= MAX_ROWS
        || younger_rows != 0
    {
        return None;
    }
    let completed = completed_partial_rows[0];
    let first_source = issued_rows
        .iter()
        .find(|issued| issued.data_request == entries[0].data_request)
        .copied()?;
    let sources = entries
        .iter()
        .map(|entry| {
            let source = issued_rows
                .iter()
                .find(|issued| issued.data_request == entry.data_request)
                .copied()?;
            (source.operation == RiscvO3LiveDataHandoffOperation::Store
                && matches!(source.target, RiscvO3LiveDataHandoffTarget::Memory { .. })
                && source.partition == first_source.partition
                && source.target == first_source.target
                && source.partial_overlay.is_none())
                .then_some(source)
        })
        .collect::<Option<Vec<_>>>()?;
    let (live_forwarded_mask, sources) = compose_completed_partial_overlay_sources(
        &sources,
        completed.address,
        completed.bytes,
        completed.original_forwarded_mask,
        &completed.data,
    )?;
    return RiscvO3LiveDataHandoff::with_completed_partial_overlay(
        entries,
        RiscvO3LiveDataHandoffCompletedPartialOverlay {
            fetch_request: completed.fetch_request,
            load_data_request: completed.data_request,
            issue_tick: completed.issue_tick,
            response_tick: completed.response_tick,
            address: completed.address,
            bytes: completed.bytes,
            original_forwarded_mask: completed.original_forwarded_mask,
            live_forwarded_mask,
            data: completed.data,
            o3_sequence: completed.o3_sequence,
            trace_sequence: completed.trace_sequence,
            sources,
        },
        younger_rows,
    );
}
```

Retain pending and fully forwarded branches unchanged after this branch.

- [ ] **Step 8: Run focused domain tests**

Run:

```text
cargo test -p rem6-cpu --lib completed_partial_overlay_tests -- --nocapture
cargo test -p rem6-cpu --lib o3_runtime_handoff -- --nocapture
```

Expected: constructor/source tests pass. Codec round trips remain red until Task
3.

### Task 3: Encode Version 7 And Preserve Version 6

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff/codec.rs`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff/completed_partial_overlay_tests.rs`
- Modify: `crates/rem6-cpu/src/public_api.rs`
- Modify current-writer schema assertions in:
  - `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`
  - `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs`
  - `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/scalar_load.rs`
  - `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/scalar_load_branch.rs`
  - `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/mmio_scalar_load.rs`

- [ ] **Step 1: Add failing codec and compatibility tests**

Add these tests to the focused test module:

```rust
const V6_MULTI_SOURCE_PENDING_PAYLOAD: [u8; 364] = [
    0x4f, 0x33, 0x44, 0x48, 0x06, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00,
    0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00,
    0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1a, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x01, 0x00, 0x80,
    0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x01, 0x6a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x03, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x04, 0x00, 0x00, 0x00, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x1b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
    0x00, 0x01, 0x01, 0x04, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x02, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00,
    0x07, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x6b, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x12, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x1c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0x00, 0x00,
    0x08, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x6c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00,
    0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
    0x80, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x0c, 0x00, 0x00,
    0xdd, 0x06, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04, 0x00,
    0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0x00,
    0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x11,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0xdd, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
];

#[test]
fn live_data_handoff_round_trips_completed_partial_overlay_v7() {
    let handoff = representative_completed_partial_handoff();
    let payload = handoff.encode();
    assert_eq!(payload[MAGIC.len()], VERSION_CURRENT);
    let (decoded, version) = RiscvO3LiveDataHandoff::decode_with_version(&payload).unwrap();
    assert_eq!(version, 7);
    assert_eq!(decoded, handoff);
}

#[test]
fn live_data_handoff_decodes_v6_multi_source_partial_overlay() {
    let handoff = v6_multi_source_pending_handoff();
    let (decoded, version) =
        RiscvO3LiveDataHandoff::decode_with_version(&V6_MULTI_SOURCE_PENDING_PAYLOAD).unwrap();
    assert_eq!(version, 6);
    assert_eq!(decoded, handoff);
    assert!(decoded.completed_partial_overlays().is_empty());
}

#[test]
fn legacy_v6_encoder_matches_frozen_multi_source_payload() {
    let handoff = v6_multi_source_pending_handoff();
    assert_eq!(
        handoff.encode_legacy_for_test(VERSION_MULTI_SOURCE_CURRENT),
        V6_MULTI_SOURCE_PENDING_PAYLOAD,
    );
}
```

Add mutation tests for original mask zero/full, live mask zero/full/outside
original, response-before-issue, nonzero padding, duplicate IDs, load sequence
not younger than a source, and incomplete/overlapping source ownership.

- [ ] **Step 2: Run the red codec tests**

Run:

```text
cargo test -p rem6-cpu --lib live_data_handoff_round_trips_completed_partial_overlay_v7 -- --nocapture
cargo test -p rem6-cpu --lib live_data_handoff_decodes_v6_multi_source_partial_overlay -- --nocapture
```

Expected: compile failures for the missing version-6 legacy constant and
version-7 completed count/record encoding.

- [ ] **Step 3: Introduce explicit version constants**

In `codec.rs`, change:

```rust
pub(super) const VERSION_MULTI_SOURCE_CURRENT: u8 = 6;
pub(super) const VERSION_CURRENT: u8 = 7;
```

Accept both versions in `decode_with_version`. Version 6 keeps the existing
header and record layout. Version 7 reads and writes an additional
`completed_partial_overlay_count` after `partial_overlay_count`.

Do not leave historical behavior behind `version == VERSION_CURRENT`. Add
explicit predicates and use them for entry ownership and multi-source pending
overlay parsing:

```rust
const fn has_current_ownership(version: u8) -> bool {
    matches!(version, VERSION_MULTI_SOURCE_CURRENT | VERSION_CURRENT)
}

const fn has_multi_source_partial_overlay(version: u8) -> bool {
    matches!(version, VERSION_MULTI_SOURCE_CURRENT | VERSION_CURRENT)
}

const fn has_completed_partial_overlay(version: u8) -> bool {
    version == VERSION_CURRENT
}
```

Update test offsets that describe the current header from `HEADER_BYTES + 8`
to `HEADER_BYTES + 12`. Keep legacy version-6 offsets in separately named
constants so mutation tests never apply version-7 offsets to historical bytes.

- [ ] **Step 4: Encode completed overlay records**

After pending overlays, encode each completed row in this order:

```rust
write_request(&mut payload, overlay.fetch_request);
write_request(&mut payload, overlay.load_data_request);
payload.extend_from_slice(&overlay.issue_tick.to_le_bytes());
payload.extend_from_slice(&overlay.response_tick.to_le_bytes());
payload.extend_from_slice(&overlay.address.get().to_le_bytes());
payload.extend_from_slice(&overlay.bytes.to_le_bytes());
payload.push(overlay.original_forwarded_mask);
payload.push(overlay.live_forwarded_mask);
payload.extend_from_slice(&overlay.data);
payload.extend_from_slice(&overlay.o3_sequence.to_le_bytes());
payload.push(u8::from(overlay.trace_sequence.is_some()));
payload.extend_from_slice(&overlay.trace_sequence.unwrap_or_default().to_le_bytes());
payload.extend_from_slice(&(overlay.sources.len() as u32).to_le_bytes());
for source in &overlay.sources {
    write_request(&mut payload, source.source_data_request);
    payload.push(source.ownership_mask);
    payload.extend_from_slice(&source.source_data);
}
```

- [ ] **Step 5: Decode and validate completed records**

Decode all fixed fields, enforce scalar-width and padding checks, validate both
masks, and resolve every source request to an entry. Reuse
`partial_overlay_mask` and `validate_partial_overlay_data`. Track request, O3,
and trace identity in the same sets used by entries and forwarded rows.

Required checks include:

```rust
if response_tick < issue_tick {
    return Err(RiscvO3LiveDataHandoffError::ForwardedResponseBeforeIssue {
        issue_tick,
        response_tick,
    });
}
let scalar_mask = scalar_byte_mask(bytes);
if original_forwarded_mask == 0
    || original_forwarded_mask == scalar_mask
    || original_forwarded_mask & !scalar_mask != 0
{
    return Err(RiscvO3LiveDataHandoffError::InvalidPartialOverlayMask {
        mask: original_forwarded_mask,
        bytes,
    });
}
if live_forwarded_mask == 0
    || live_forwarded_mask == scalar_mask
    || live_forwarded_mask & !original_forwarded_mask != 0
{
    return Err(RiscvO3LiveDataHandoffError::InvalidCompletedPartialOverlayLiveMask {
        original: original_forwarded_mask,
        live: live_forwarded_mask,
    });
}
if entries.iter().any(|entry| entry.o3_sequence >= o3_sequence) {
    return Err(RiscvO3LiveDataHandoffError::InvalidCompletedPartialOverlaySequence {
        source: entries
            .iter()
            .map(|entry| entry.o3_sequence)
            .max()
            .unwrap(),
        load: o3_sequence,
    });
}
```

Add the two new error variants and exact `Display` messages. Add
`completed_partial_overlays` to `InvalidCurrentShape` diagnostics so every
current count is visible.

Use these variants:

```rust
InvalidCompletedPartialOverlayLiveMask {
    original: u8,
    live: u8,
},
InvalidCompletedPartialOverlaySequence {
    source: u64,
    load: u64,
},
```

and these messages:

```text
live-data handoff completed partial-overlay live mask 0xLL is not a nonzero partial subset of original mask 0xOO
live-data handoff completed partial-overlay load sequence L does not follow source sequence S
```

- [ ] **Step 6: Update current shape and row-count validation**

Version 7 permits only:

```rust
match (
    forwarded_rows.len(),
    partial_overlays.len(),
    completed_partial_overlays.len(),
) {
    (0, 0, 0) => ordinary_load_shape(entries),
    (1, 0, 0) => full_forwarded_shape(entries, younger_rows),
    (0, 1, 0) => pending_partial_shape(entries, &partial_overlays[0], younger_rows),
    (0, 0, 1) => completed_partial_shape(
        entries,
        &completed_partial_overlays[0],
        younger_rows,
    ),
    _ => false,
}
```

The completed shape requires one to three store entries, zero younger rows,
source-entry identity/order equality, disjoint nonzero ownership, source union
equal to the live mask, and source sequences below the completed load. Perform
the final `resident_rows + younger_rows <= MAX_ROWS` check after all records are
decoded.

- [ ] **Step 7: Preserve version-6 legacy encoding**

Extend `encode_legacy_for_test` to accept `VERSION_MULTI_SOURCE_CURRENT`. For
version 6, write the old forwarded/pending counts, entry ownership records, and
multi-source pending overlay source-count records, but never write the new
completed count or completed rows.

- [ ] **Step 8: Run all handoff codec tests**

Run:

```text
cargo test -p rem6-cpu --lib riscv_execution_mode_handoff -- --nocapture
cargo test -p rem6-cpu --lib completed_partial_overlay_tests -- --nocapture
```

Expected: version-7 round trip, version-6 compatibility, and all mutation tests
pass.

- [ ] **Step 9: Update current-writer schema expectations and verify existing handoffs**

Change only `o3-live-data-handoff` current-writer assertions from `6` to `7`
in the listed O3 handoff CLI files. Do not change unrelated schema-version
values in GPU, checker, or pipeline tests.

Run:

```text
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::scalar_load -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::scalar_load_branch -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::mmio_scalar_load -- --nocapture
```

Expected: every pre-existing handoff row passes under schema version 7.

- [ ] **Step 10: Commit the CPU schema implementation**

```bash
git add crates/rem6-cpu/src/o3_runtime_handoff.rs \
  crates/rem6-cpu/src/riscv_execution_mode_handoff.rs \
  crates/rem6-cpu/src/riscv_execution_mode_handoff/partial_overlay.rs \
  crates/rem6-cpu/src/riscv_execution_mode_handoff/codec.rs \
  crates/rem6-cpu/src/riscv_execution_mode_handoff/completed_partial_overlay_tests.rs \
  crates/rem6-cpu/src/public_api.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/switch/scalar_load.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/switch/scalar_load_branch.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/switch/mmio_scalar_load.rs
git commit -m "cpu: encode completed partial overlay handoffs"
```

### Task 4: Expose Completed Overlay HostAction Evidence

**Files:**
- Modify: `crates/rem6/src/host_actions/live_data_handoff.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs`

- [ ] **Step 1: Add a failing direct CLI summary test**

Add the direct completed-overlay test using the existing multi-source fixture.
Derive the switch tick after the load response and before the next source-store
response, run the CLI, and assert the first decoded fields:

```rust
#[test]
fn rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_direct() {
    let json = run_completed_multi_source_partial_handoff("direct");
    let transfer = completed_multi_source_transfer(&json.switched);
    let handoff = transfer_handoff_chunk(transfer, "cpu0");
    assert_eq!(handoff["schema_version"].as_u64(), Some(7));
    assert_eq!(handoff["outstanding_requests"].as_u64(), Some(1));
    assert_eq!(handoff["resident_rows"].as_u64(), Some(3));
    assert_eq!(handoff["transport_owned_rows"].as_u64(), Some(1));
    assert_eq!(handoff["buffered_store_rows"].as_u64(), Some(1));
    assert_eq!(handoff["completed_partial_overlay_rows"].as_u64(), Some(1));
    assert_eq!(
        handoff["first_completed_partial_overlay_data_hex"].as_str(),
        Some("aa00dd0655667788")
    );
}
```

Add the helper contract explicitly:

```rust
struct CompletedMultiSourceRun {
    baseline: Value,
    switched: Value,
    load_response: u64,
    switch_tick: u64,
    next_source_response: u64,
}

fn run_completed_multi_source_partial_handoff(
    memory_system: &str,
) -> CompletedMultiSourceRun {
    let path = multi_source_partial_forwarded_store_load_binary(&format!(
        "host-switch-completed-multi-source-partial-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_store_load_handoff(&path, memory_system, None, 4);
    let load = event_at_pc(&baseline, MULTI_SOURCE_YOUNGER_LOAD_PC);
    let load_response = event_u64(load, "lsq_data_response_tick");
    let next_source_response = [
        MULTI_SOURCE_OLDER_STORE_PC,
        MULTI_SOURCE_MIDDLE_STORE_PC,
        MULTI_SOURCE_YOUNGEST_STORE_PC,
    ]
    .into_iter()
    .map(|pc| event_u64(event_at_pc(&baseline, pc), "lsq_data_response_tick"))
    .filter(|tick| *tick > load_response)
    .min()
    .expect("live source store after completed load");
    let switch_tick = load_response
        .saturating_add(next_source_response.saturating_sub(load_response) / 2);
    assert!(
        load_response < switch_tick && switch_tick < next_source_response,
        "completed partial window must follow load response and precede the next source response: load_response={load_response}, switch_tick={switch_tick}, next_source_response={next_source_response}"
    );
    let switched = run_store_load_handoff(&path, memory_system, Some(switch_tick), 4);
    CompletedMultiSourceRun {
        baseline,
        switched,
        load_response,
        switch_tick,
        next_source_response,
    }
}

fn completed_multi_source_transfer(json: &Value) -> &Value {
    json.pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str)
                        == Some("detailed")
            })
        })
        .and_then(|switch| switch.pointer("/state_transfer"))
        .unwrap_or_else(|| panic!("missing completed partial state transfer: {json}"))
}
```

Task 5 extends the same helper and avoids duplicate ad hoc command logic.

- [ ] **Step 2: Run the red direct CLI test**

Run:

```text
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_direct -- --nocapture
```

Expected after Task 3: the mode switch succeeds and the payload decodes, but
the assertion fails because HostAction JSON does not yet expose completed rows.

- [ ] **Step 3: Add summary fields and JSON**

Extend `Rem6HostO3LiveDataHandoffChunkSummary` with completed-row count, source
count, request IDs, issue/response ticks, address/width, four masks, three byte
counts, final data, O3/trace sequence, and source summaries. Populate them from
`handoff.completed_partial_overlays().first()`.

Use these JSON names exactly:

```text
completed_partial_overlay_rows
completed_partial_overlay_source_rows
first_completed_partial_overlay_operation
first_completed_partial_overlay_fetch_request_agent
first_completed_partial_overlay_fetch_request_sequence
first_completed_partial_overlay_load_data_request_agent
first_completed_partial_overlay_load_data_request_sequence
first_completed_partial_overlay_issue_tick
first_completed_partial_overlay_response_tick
first_completed_partial_overlay_address
first_completed_partial_overlay_bytes
first_completed_partial_overlay_original_forwarded_mask
first_completed_partial_overlay_original_response_mask
first_completed_partial_overlay_live_forwarded_mask
first_completed_partial_overlay_retired_forwarded_mask
first_completed_partial_overlay_original_forwarded_bytes
first_completed_partial_overlay_live_forwarded_bytes
first_completed_partial_overlay_retired_forwarded_bytes
first_completed_partial_overlay_data_hex
first_completed_partial_overlay_o3_sequence
first_completed_partial_overlay_trace_sequence
first_completed_partial_overlay_sources
```

Update `decode_error()` with `None`/empty defaults. Extend `last_issue_tick` with:

```rust
.chain(
    handoff
        .completed_partial_overlays()
        .iter()
        .map(|row| row.issue_tick()),
)
```

Do not change existing pending/full-forwarded JSON names or meanings.

- [ ] **Step 4: Run the direct CLI and historical host-action tests**

Run:

```text
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_direct -- --nocapture
cargo test -p rem6 --lib host_actions -- --nocapture
```

Expected: the direct completed summary passes and historical decode-error,
pending-overlay, and fully forwarded behavior remains green.

### Task 5: Prove Direct, Hierarchy, And Negative CLI Rows

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs`

- [ ] **Step 1: Extend the direct row and add the hierarchy row**

Keep the direct test from Task 4 and add:

```rust
#[test]
fn rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_cache_fabric_dram() {
    assert_completed_multi_source_partial_forwarded_store_load_handoff(
        "cache-fabric-dram",
    );
}
```

Refactor the Task 4 helper into
`assert_completed_multi_source_partial_forwarded_store_load_handoff` and reuse
`multi_source_partial_forwarded_store_load_binary`. Run a baseline and
calculate:

```rust
let load_response = event_u64(load, "lsq_data_response_tick");
let next_source_response = source_pcs
    .iter()
    .map(|pc| event_u64(event_at_pc(&baseline, pc), "lsq_data_response_tick"))
    .filter(|tick| *tick > load_response)
    .min()
    .expect("live source store after completed load");
let switch_tick = load_response
    .saturating_add(next_source_response.saturating_sub(load_response) / 2);
assert!(
    load_response < switch_tick && switch_tick < next_source_response,
    "completed partial window must follow load response and precede the next source response: load_response={load_response}, switch_tick={switch_tick}, next_source_response={next_source_response}"
);
```

Run with depth four and assert:

```rust
assert_eq!(runtime["snapshot_rob_entries"].as_u64(), Some(3));
assert_eq!(runtime["snapshot_lsq_entries"].as_u64(), Some(3));
for (field, expected) in [
    ("schema_version", 7),
    ("outstanding_requests", 1),
    ("resident_rows", 3),
    ("transport_owned_rows", 1),
    ("buffered_store_rows", 1),
    ("forwarded_rows", 0),
    ("partial_overlay_rows", 0),
    ("completed_partial_overlay_rows", 1),
    ("completed_partial_overlay_source_rows", 2),
    ("younger_rows", 0),
    ("first_completed_partial_overlay_bytes", 8),
    ("first_completed_partial_overlay_original_forwarded_mask", 0x0f),
    ("first_completed_partial_overlay_original_response_mask", 0xf0),
    ("first_completed_partial_overlay_live_forwarded_mask", 0x0c),
    ("first_completed_partial_overlay_retired_forwarded_mask", 0x03),
] {
    assert_eq!(handoff[&field].as_u64(), Some(expected), "{field}: {handoff}");
}
assert_eq!(
    handoff["first_completed_partial_overlay_data_hex"].as_str(),
    Some("aa00dd0655667788")
);
```

Assert source summaries are exactly:

```text
middle halfword: address 0x80000102, bytes 2, mask 0x08, data 0006
youngest byte:   address 0x80000102, bytes 1, mask 0x04, data dd
```

Compare issue/writeback/commit ticks for all four original PCs to the baseline,
assert final registers `x14=0x8877665506dd00aa` and
`x15=0x8877665506dd00ab`, final memory
`aa00dd0655667788aa00dd06`, six data request identities, one forwarding match,
route-appropriate cache/transport/fabric/DRAM activity, and exact equality
between run-artifact and HostAction-debug handoff JSON.

- [ ] **Step 2: Add the failing younger-row negative**

Use `multi_source_partial_forwarded_store_load_with_younger_binary`. Derive the
same post-load/pre-source-response switch interval and assert:

```rust
assert!(
    load_response < switch_tick && switch_tick < next_source_response,
    "completed partial younger-row window must follow load response and precede the next source response: load_response={load_response}, switch_tick={switch_tick}, next_source_response={next_source_response}"
);
assert!(!output.status.success());
assert!(String::from_utf8_lossy(&output.stderr)
    .contains("checkpoint component is not quiescent: cpu0"));
```

Name the test:

```text
rem6_run_host_switch_rejects_completed_partial_forwarded_store_load_with_younger_row
```

- [ ] **Step 3: Run the completed matrix tests**

Run:

```text
cargo test -p rem6 --test cli_run completed_multi_source_partial_forwarded_store_load -- --nocapture
cargo test -p rem6 --test cli_run rejects_completed_partial_forwarded_store_load_with_younger_row -- --nocapture
```

Expected: direct and hierarchy transfers pass with completed-overlay evidence;
the unrelated-younger row remains rejected with the exact quiescence error.

- [ ] **Step 4: Run focused CLI coverage**

Run:

```text
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::store_load_forwarding -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::scalar_load -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::scalar_load_branch -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::mmio_scalar_load -- --nocapture
```

Expected: pending partial, multi-source pending, fully forwarded, completed
partial, scalar-load, branch, and MMIO handoff rows all pass under schema 7.

- [ ] **Step 5: Commit executable matrix evidence**

```bash
git add crates/rem6/src/host_actions/live_data_handoff.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs
git commit -m "test: cover completed partial overlay handoffs"
```

### Task 6: Record Honest Ledger Evidence

**Files:**
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update core evidence anchors**

Replace:

```text
completed_partial_overlay_is_not_live_handoff_authority
```

with:

```text
completed_partial_overlay_becomes_live_handoff_authority
live_data_handoff_round_trips_completed_partial_overlay_v7
live_data_handoff_decodes_v6_multi_source_partial_overlay
rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_direct
rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_cache_fabric_dram
rem6_run_host_switch_rejects_completed_partial_forwarded_store_load_with_younger_row
first_completed_partial_overlay_original_forwarded_mask
first_completed_partial_overlay_live_forwarded_mask
first_completed_partial_overlay_retired_forwarded_mask
```

- [ ] **Step 2: Update the CPU ledger without score changes**

In the CPU section:

1. add the completed multi-source partial-overlay direct/hierarchy mode-transfer
   matrix to `Migrated`;
2. state the three-row transfer shape, original/live/retired masks, final data,
   baseline timing preservation, resource activity, and younger-row rejection;
3. remove completed partial-overlay handoff from `Not migrated` and `Next
   evidence` wherever it is listed;
4. keep `8 of 10`, `80% raw`, `74% representative`, both unchecked items, and
   every unrelated open boundary unchanged; and
5. keep the file exactly 1,200 lines by replacing existing prose rather than
   adding section structure.

- [ ] **Step 3: Run ledger/source-policy tests**

Run:

```text
cargo test -p rem6 --test source_policy gem5_migration_sections_are_auditable -- --exact --nocapture
cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --exact --nocapture
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: both tests pass and the ledger reports exactly `1200` lines.

- [ ] **Step 4: Commit the ledger update**

```bash
git add crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record completed partial overlay evidence"
```

### Task 7: Completion Verification And Review

**Files:**
- Verify all changed files.

- [ ] **Step 1: Run focused suites**

```text
cargo test -p rem6-cpu --lib completed_partial_overlay -- --nocapture
cargo test -p rem6-cpu --lib riscv_execution_mode_handoff -- --nocapture
cargo test -p rem6 --lib host_actions -- --nocapture
cargo test -p rem6 --test cli_run completed_multi_source_partial_forwarded_store_load -- --nocapture
cargo test -p rem6 --test cli_run rejects_completed_partial_forwarded_store_load_with_younger_row -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
cargo test -p rem6-cpu --test source_policy -- --nocapture
```

Expected: all focused tests pass.

- [ ] **Step 2: Run crate and workspace suites**

```text
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --quiet
cargo test --workspace --all-targets --quiet
```

Expected: all tests pass with no failures or hangs.

- [ ] **Step 3: Run formatting, diff, ledger, and cap checks**

```text
cargo fmt --all -- --check
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
wc -l crates/rem6-cpu/src/riscv_execution_mode_handoff.rs \
  crates/rem6-cpu/src/riscv_execution_mode_handoff/codec.rs \
  crates/rem6-cpu/src/riscv_execution_mode_handoff/partial_overlay.rs \
  crates/rem6-cpu/src/riscv_execution_mode_handoff/completed_partial_overlay_tests.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/switch/store_load_forwarding.rs
```

Expected: format and diff checks pass, ledger is 1,200 lines, and every Rust
file remains at or below its source-policy cap.

- [ ] **Step 4: Run read-only xhigh whole-diff review**

The reviewer must check:

- completed/live/retired mask semantics;
- source-store ordering and ownership;
- current and historical codec layouts;
- duplicate request/O3/trace rejection;
- row counts and `last_issue_tick`;
- direct/hierarchy CLI byte and timing witnesses;
- younger-row failure preservation;
- no dead compatibility code or duplicate schema authority; and
- honest unchanged ledger scoring.

Resolve every finding, rerun affected tests, and repeat review until it reports
no findings.

- [ ] **Step 5: Push and verify synchronization**

```text
git push origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

Expected: local and remote hashes match and the worktree is clean.
