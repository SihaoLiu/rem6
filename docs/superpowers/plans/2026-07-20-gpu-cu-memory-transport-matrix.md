# GPU CU Memory Transport Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Derive per-compute-unit GPU memory-transport response activity from the authoritative top-level memory trace and prove queued scheduling, cross-line coalescing, cache/DRAM, and fabric interaction through one representative CLI matrix.

**Architecture:** Remove the duplicate synchronized response-status vector from `gpu_cli.rs`. Match final source response events to source request events in `MemoryTrace`, attribute them through the request agent ID, and attach one nested typed `memory_transport` summary to each `Rem6GpuComputeUnitActivity`. Keep the current post-compute memory phase explicit; this increment adds attribution evidence, not memory-latency backpressure into GPU scheduling.

**Tech Stack:** Rust 2021, rem6 partitioned scheduler and memory transport trace, manual JSON emitters, rem6 stats registry, Cargo integration/source-policy/workspace tests, real `rem6 gpu-run` subprocess tests.

---

## File Map

- `crates/rem6/src/gpu_cli.rs`: trace-derived per-CU memory transport type, validation, JSON, and duplicate response-vector removal.
- `crates/rem6/src/stats_output/gpu_run.rs`: nested per-CU memory transport counters and response-window suppression.
- `crates/rem6/tests/cli_run/gpu.rs`: focused child-module declaration and removal of extracted per-CU tests.
- `crates/rem6/tests/cli_run/gpu/representative_matrix.rs`: existing per-CU tests plus new route matrix and negative/suppression cases.
- `docs/architecture/gem5-to-rem6-migration.md`: representative GPU checklist, cap, evidence, remaining gap, and crosswalk update.

### Task 1: Extract focused GPU CU test ownership

**Files:**
- Modify: `crates/rem6/tests/cli_run/gpu.rs`
- Create: `crates/rem6/tests/cli_run/gpu/representative_matrix.rs`

- [ ] **Step 1: Declare the focused child module**

Add this declaration after the shared imports/helpers in `gpu.rs`:

```rust
#[path = "gpu/representative_matrix.rs"]
mod representative_matrix;
```

The child starts with:

```rust
use super::*;
```

- [ ] **Step 2: Move the existing per-CU tests without changing them**

Move these complete test functions from `gpu.rs` into the child module:

```text
rem6_gpu_run_reports_per_compute_unit_activity
rem6_gpu_run_reports_per_compute_unit_memory_activity
rem6_gpu_run_merges_overlapping_wave_slots_for_compute_unit_busy_cycles
rem6_gpu_run_omits_activity_window_stats_for_inactive_compute_units
```

Do not rename tests, weaken assertions, or duplicate helpers. The child uses
`Command`, `Value`, and `assert_stat` through `use super::*`.

- [ ] **Step 3: Run the extracted tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run gpu::representative_matrix:: -- --nocapture
```

Expected: all four moved tests PASS with unchanged behavior.

- [ ] **Step 4: Commit the mechanical extraction**

```bash
git add crates/rem6/tests/cli_run/gpu.rs crates/rem6/tests/cli_run/gpu/representative_matrix.rs
TMPDIR=$PWD/target/tmp git commit -m "test: extract gpu cu activity coverage"
```

### Task 2: Add the failing representative matrix

**Files:**
- Modify: `crates/rem6/tests/cli_run/gpu/representative_matrix.rs`

- [ ] **Step 1: Add route descriptors and shared artifact helpers**

Add a compact row type:

```rust
struct GpuHierarchyRow {
    name: &'static str,
    extra_args: &'static [&'static str],
    expected_cache_runs: u64,
    expected_dram_accesses: u64,
    expected_fabric_transfers: u64,
}
```

Add helpers that return the parsed simulation activity array and read a nested
unsigned value with a failure message naming the row and path:

```rust
fn compute_unit_activity(artifact: &Value) -> &[Value] {
    artifact["simulation"]["compute_unit_activity"]
        .as_array()
        .expect("GPU compute-unit activity array")
}

fn nested_u64<'a>(value: &'a Value, path: &[&str]) -> u64 {
    path.iter()
        .fold(value, |current, key| &current[*key])
        .as_u64()
        .unwrap_or_else(|| panic!("missing unsigned JSON path {}", path.join("/")))
}
```

- [ ] **Step 2: Add the three-row real CLI matrix**

Use these rows:

```rust
const ROWS: &[GpuHierarchyRow] = &[
    GpuHierarchyRow {
        name: "direct",
        extra_args: &[],
        expected_cache_runs: 0,
        expected_dram_accesses: 0,
        expected_fabric_transfers: 0,
    },
    GpuHierarchyRow {
        name: "cache-dram",
        extra_args: &["--data-cache-protocol", "msi", "--dram-memory"],
        expected_cache_runs: 9,
        expected_dram_accesses: 6,
        expected_fabric_transfers: 0,
    },
    GpuHierarchyRow {
        name: "fabric-cache-dram",
        extra_args: &[
            "--data-cache-protocol", "msi",
            "--dram-memory",
            "--fabric-link", "gpu_mem",
            "--fabric-bandwidth-bytes-per-tick", "16",
            "--fabric-request-virtual-network", "7",
            "--fabric-response-virtual-network", "8",
            "--fabric-credit-depth", "2",
        ],
        expected_cache_runs: 9,
        expected_dram_accesses: 6,
        expected_fabric_transfers: 18,
    },
];
```

For every row invoke the binary with this common workload:

```text
gpu-run
--workgroups 3
--compute-units 2
--wave-slots-per-compute-unit 1
--workgroup-cycles 4
--global-load 0x303f:1:0:4
--global-store 0x3080:4:4:4
--memory-start 0x3000
--memory-size 256
--max-tick 200
--stats-format json
```

Assert exact common ownership:

```text
workgroup_completions = 3
workgroup_queue_wait_count = 1
workgroup_queue_wait_ticks = 4
coalesced_memory_accesses = 9
global_memory_requests = 9
memory_responses = 9
CU0 completions/busy/accesses/reads/writes/responses = 2/8/6/4/2/6
CU1 completions/busy/accesses/reads/writes/responses = 1/4/3/2/1/3
```

Read each CU's nested `memory_transport` object. Assert the per-CU response and
round-trip totals sum to aggregate `transport.responses` and
`transport.round_trip_ticks`; assert the largest per-CU maximum equals aggregate
`transport.max_round_trip_ticks`; assert first/last response ticks are ordered
and do not exceed `simulation.final_tick`.

For hierarchy evidence assert:

```rust
assert_eq!(artifact["data_cache"]["data_cache_runs"].as_u64(), Some(row.expected_cache_runs));
assert_eq!(artifact["dram"]["accesses"].as_u64(), Some(row.expected_dram_accesses));
assert_eq!(artifact["fabric"]["transfers"].as_u64(), Some(row.expected_fabric_transfers));
```

For the fabric row also require positive queue delay and two active virtual
networks through the existing fabric activity/stats surface.

- [ ] **Step 3: Extend inactive-CU suppression assertions**

In `rem6_gpu_run_omits_activity_window_stats_for_inactive_compute_units`, require:

```rust
assert_eq!(compute_units[1]["memory_transport"]["responses"], 0);
assert_eq!(compute_units[1]["memory_transport"]["round_trip_ticks"], 0);
assert_eq!(compute_units[1]["memory_transport"]["max_round_trip_ticks"], 0);
assert!(compute_units[1]["memory_transport"]["first_response_at"].is_null());
assert!(compute_units[1]["memory_transport"]["last_response_at"].is_null());
assert!(!stdout.contains("sim.gpu_run.compute_unit.cu1.memory_transport.first_response_at"));
assert!(!stdout.contains("sim.gpu_run.compute_unit.cu1.memory_transport.last_response_at"));
```

Require zero-valued counter stats for the inactive CU.

- [ ] **Step 4: Add the bounded max-tick failure row**

Run the fabric/cache/DRAM common workload with `--max-tick 20`. Assert failure,
empty stdout, and stderr containing both `GPU final tick` and
`exceeded max tick 20`.

- [ ] **Step 5: Run the focused tests and observe RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run gpu::representative_matrix:: -- --nocapture
```

Expected: the new matrix and inactive-CU assertions FAIL because per-CU
`memory_transport` JSON/stats do not exist. The max-tick row should already pass.

### Task 3: Derive per-CU transport activity from `MemoryTrace`

**Files:**
- Modify: `crates/rem6/src/gpu_cli.rs`
- Modify: `crates/rem6/src/stats_output/gpu_run.rs`

- [ ] **Step 1: Add the typed nested activity**

Add beside `Rem6GpuComputeUnitActivity`:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Rem6GpuComputeUnitMemoryTransportActivity {
    responses: u64,
    round_trip_ticks: u64,
    max_round_trip_ticks: u64,
    first_response_at: Option<u64>,
    last_response_at: Option<u64>,
}
```

Add it as `memory_transport` in `Rem6GpuComputeUnitActivity`, initialize with
`Default::default()`, expose a const reference getter, and serialize:

```json
"memory_transport":{"responses":0,"round_trip_ticks":0,"max_round_trip_ticks":0,"first_response_at":null,"last_response_at":null}
```

Add private getters on the nested type for stats output.

- [ ] **Step 2: Replace callback storage with trace-derived validation**

Delete `use std::sync::{Arc, Mutex};`, the `memory_responses` vector, its cloned
callback handles, and the post-run lock/status scan. Use a no-op final response
sink:

```rust
move |_delivery| {}
```

Extend `gpu_compute_unit_activity` to receive `&MemoryTrace`. After completion,
access, and queue-wait accounting, call a focused helper that:

```rust
let events = trace.snapshot();
let mut requests = BTreeMap::<(MemoryRouteId, MemoryRequestId), (u64, String)>::new();
```

On `RequestSent`, store tick and endpoint. On `ResponseArrived`, look up the
source tuple; ignore intermediate endpoints; validate final status; convert
`request_id.agent().get()` to the CU index; calculate
`event.tick().checked_sub(sent_tick)`; and update response count, sum, maximum,
first tick, and last tick.

Use checked additions for response count and round-trip sum and return
`execute_error` on overflow. Preserve the existing error text:

```rust
return Err(execute_error(format!(
    "GPU memory request completed with {status:?}"
)));
```

After attribution, require summed per-CU responses to equal `accesses.len()`.
Report both counts in the error if they differ.

- [ ] **Step 3: Reuse aggregate transport as the execution response count**

Construct `let transport = memory_transport_summary(&trace);` before assembling
`Rem6GpuRunExecutionSummary`, and set:

```rust
memory_responses: transport.counters.responses,
```

Pass the trace into `gpu_compute_unit_activity` only after the memory scheduler
has drained. Do not change workgroup scheduling or submission order.

- [ ] **Step 4: Emit nested per-CU stats**

In `stats_output/gpu_run.rs`, emit these counters for every CU:

```text
memory_transport.responses Count
memory_transport.round_trip_ticks Tick
memory_transport.max_round_trip_ticks Tick
```

Emit `first_response_at` and `last_response_at` only when their options are
present, matching the existing first-start/last-completion pattern.

- [ ] **Step 5: Run focused tests and make them GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run gpu::representative_matrix:: -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run gpu::rem6_gpu_run_routes_coalesced_global_memory_through_cache_and_dram -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run gpu::rem6_gpu_run_routes_global_memory_through_configured_fabric -- --exact
```

Expected: all PASS. If exact deterministic round-trip values are stable across
two consecutive matrix runs, add exact row expectations; otherwise retain the
strong reconciliation invariants and exact route activity counts.

- [ ] **Step 6: Commit behavior and matrix evidence**

```bash
git add crates/rem6/src/gpu_cli.rs crates/rem6/src/stats_output/gpu_run.rs crates/rem6/tests/cli_run/gpu/representative_matrix.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: attribute gpu transport responses per cu"
```

### Task 4: Update the migration ledger honestly

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update the component score and checklist**

Change the heading to:

```markdown
### Configuration, Resources, Suites, GPU, and Accelerators - 74% representative
```

Change the score calculation to 17 of 21, 81% raw, capped to 74% by the
representative bucket. Check the representative GPU item.

- [ ] **Step 2: Record exact evidence and remaining gaps**

Update `Migrated`, `Evidence`, and `Next evidence` to name the three-row
top-level matrix, cross-line load plus store coalescing, queued 2-CU assignment,
per-CU transport reconciliation, inactive-CU suppression, and bounded max-tick
failure.

Replace the old representative-GPU gap with these explicit remaining gaps:

```text
memory-response-gated wave-slot/workgroup completion
cache/DRAM backpressure into CU scheduling
broader GPU ISA and topology/protocol matrices
```

Update the GPU test-migration crosswalk row from `40% single-axis` to
`60% representative` and describe the same bounded evidence.

- [ ] **Step 3: Preserve the ledger boundary and run policy**

```bash
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: exact 1,200 lines and all source-policy tests PASS.

- [ ] **Step 4: Commit the ledger**

```bash
git add docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp git commit -m "docs: record representative gpu transport matrix"
```

### Task 5: Verify, review, and push

**Files:**
- Review all files changed since `314330f7`

- [ ] **Step 1: Run formatting and focused GPU coverage**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run gpu:: -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: all PASS.

- [ ] **Step 2: Run crate and workspace verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
TMPDIR=$PWD/target/tmp cargo test --workspace
```

Expected: all PASS within the configured development timeout.

- [ ] **Step 3: Run deterministic runtime and reconciliation probes**

Run the fabric/cache/DRAM matrix command twice and compare the compact JSON
projection containing simulation CU activity, data-cache counters, DRAM,
fabric, and transport. Expected: byte-identical projections and per-CU response
counts/totals reconciling to aggregate transport counters.

- [ ] **Step 4: Perform a read-only review**

Inspect:

```bash
git diff 314330f7..HEAD --check
git diff 314330f7..HEAD --stat
git diff 314330f7..HEAD
git status --short --branch
```

Review specifically for duplicate response authorities, accidental backpressure
claims, unchecked arithmetic, trace endpoint mistakes, inactive-CU stat leaks,
NoMali/power schema drift, weakened assertions, ledger overclaim, and unrelated
worktree changes. Resolve every concrete finding and rerun affected tests.

- [ ] **Step 5: Push and verify the remote**

```bash
git push origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

Expected: push succeeds, local and remote hashes match, and the branch is clean
and synchronized.
