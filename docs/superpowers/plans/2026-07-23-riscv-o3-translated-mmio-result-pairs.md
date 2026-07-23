# RISC-V O3 Translated And MMIO Result-Pair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Admit exactly two translated scalar result loads in the detailed RISC-V O3 memory-result window, including a translated cacheable-memory plus translated readfile-MMIO pair, with bounded issue/writeback behavior, request-keyed completion, lifecycle coverage, and top-level CLI evidence.

**Architecture:** Extend the existing memory-result authorization with an unresolved translated virtual-range state that binds once to a physical range and then once to a memory or MMIO target. Use the resident head O3 row as the canonical window identity, derive the second row's issue tick from the existing total/memory issue calendar, and replace only the translated-driver blanket outstanding gates with a focused admission result. Existing request-keyed completion, oldest-first publication, checkpoint rejection, and typed multi-target handoff remain authoritative.

**Tech Stack:** Rust 2021 workspace, `rem6-cpu` detailed RISC-V O3 runtime, translation frontend and TLB/page map, `MmioBus`, memory transport, `rem6 run` TOML configuration, JSON/debug/checkpoint artifacts, source-policy tests, Cargo, and Git.

---

## File Map

Create production owners:

- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_translation.rs` - translated result probing and virtual-only younger authorization, extracted from the full `data_access_result.rs` owner.
- `crates/rem6-cpu/src/riscv_fetch_ahead/memory_result_authorization/translated.rs` - translated virtual/physical/target binding methods for the canonical result-window authorization.
- `crates/rem6-cpu/src/o3_runtime_memory_result_admission.rs` - next legal memory-result issue tick derived from the existing live issue calendar.
- `crates/rem6-cpu/src/riscv_data_issue/o3_result_pair_admission.rs` - ordinary/ready/wait/blocked translated data-progress decision.
- `crates/rem6-cpu/src/riscv_translation/o3_result_pair.rs` - authorization preservation, translation binding, target binding, and sequence-suffix cleanup helpers.

Create focused CPU tests:

- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/translated_result_pair.rs`
- `crates/rem6-cpu/src/riscv_translation_tests/translated_mmio_result_pair.rs`
- `crates/rem6-cpu/src/riscv_data_issue_tests/translated_mmio_result_pair.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests/translated_mmio_result_pair.rs`

Create CLI evidence:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/fixture.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/boundaries.rs`
- `crates/rem6/tests/source_policy/o3_translated_mmio_pair_ownership.rs`

Modify authorization, translation, and issue flow:

- `crates/rem6-cpu/src/riscv_fetch_ahead/memory_result_authorization.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_effect_policy.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_pair_policy.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- `crates/rem6-cpu/src/riscv_memory_result_window.rs`
- `crates/rem6-cpu/src/o3_runtime.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- `crates/rem6-cpu/src/riscv_data_issue.rs`
- `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- `crates/rem6-cpu/src/riscv_translation.rs`
- `crates/rem6-cpu/src/riscv_translation/helpers.rs`
- `crates/rem6-cpu/src/riscv_translation_tests.rs`
- `crates/rem6-cpu/src/riscv_cluster.rs`
- `crates/rem6-cpu/src/riscv_cluster_translation.rs`
- `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- `crates/rem6-cpu/src/riscv_live_retire_gate.rs`
- `crates/rem6-cpu/src/riscv_fetch.rs`
- `crates/rem6-cpu/tests/source_policy.rs`

Modify CLI ownership and migration accounting:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs`
- `crates/rem6/tests/source_policy.rs`
- `crates/rem6/tests/source_policy/checkpoint_total_authority.rs`
- `crates/rem6/tests/source_policy/writeback_ownership.rs`
- `crates/rem6/tests/source_policy/core_test_anchors.txt`
- `docs/architecture/gem5-to-rem6-migration.md`

Do not modify `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`; it is exactly 1,800 lines and already supports multiple typed memory/MMIO entries. Do not relax `prepare_mmio_data_access()` for untranslated MMIO; ordinary untranslated MMIO remains single-outstanding.

## Execution Preconditions

The implementation worktree already exists at:

```text
/home/sihao/.config/superpowers/worktrees/rem6/o3-translated-mmio-result-pairs
```

It is on branch `o3-translated-mmio-result-pairs` and contains design commit `dbb77426`.

The root filesystem and `/tmp` are full. Before any Cargo or Git command:

```bash
mkdir -p "$PWD/target/tmp"
export TMPDIR="$PWD/target/tmp"
```

Do not edit or commit `temp/`. Keep `temp/reference_designs/gem5` read-only and do not build or execute it.

Before each commit:

1. Run the task's focused tests.
2. Run `cargo fmt --all -- --check` and `git diff --check`.
3. Review the staged diff against the approved design and this plan.
4. Use `env TMPDIR="$PWD/target/tmp" git commit ...` so Git does not use `/tmp`.
5. Push the feature branch after the commit is verified.

### Task 1: Create Source Headroom And Extract Translation Probing

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/checkpoint_total_authority.rs`

- [ ] **Step 1: Move the checkpoint projection test into its owner**

Change the function in `checkpoint_total_authority.rs` to own its test attribute:

```rust
#[test]
pub(crate) fn checkpoint_output_summaries_derive_hierarchy_totals_from_projections() {
    // Existing body remains unchanged.
}
```

Delete the forwarding test from `crates/rem6/tests/source_policy.rs`:

```rust
#[test]
fn checkpoint_output_summaries_derive_hierarchy_totals_from_projections() {
    checkpoint_total_authority::checkpoint_output_summaries_derive_hierarchy_totals_from_projections();
}
```

Expected result: `source_policy.rs` gains at least four lines of headroom without raising `MAX_SOURCE_POLICY_DRIVER_LINES`.

- [ ] **Step 2: Extract translated result probing without behavior changes**

Attach the child from `detailed_o3.rs`:

```rust
#[path = "detailed_o3/data_access_result_translation.rs"]
mod data_access_result_translation;
```

Move these existing items from `data_access_result.rs` into the new child with `use super::*;` and equivalent imports:

```text
DataAccessResultHeadPhysicalProbe
data_access_result_head_physical_probe
DataAccessResultHeadProbe
data_access_result_head_probe
DataAccessResultTranslationProbe
data_access_result_translation_probe
```

Re-export only the existing public probe from `detailed_o3.rs` and import the private helpers back into `data_access_result.rs`. Keep `data_access_result.rs <= 450` lines and set a focused cap of 250 lines for the new child.

- [ ] **Step 3: Run behavior-preserving verification**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::data_access_result -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::data_access_result_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test source_policy checkpoint_total_authority -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --test source_policy detailed_o3 -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Expected: all existing behavior remains green; `data_access_result.rs` is below its cap and `source_policy.rs` remains below 1,400 lines.

- [ ] **Step 4: Commit and push**

```bash
git add crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_translation.rs \
  crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/checkpoint_total_authority.rs
env TMPDIR="$PWD/target/tmp" git commit -m "refactor: isolate translated result probing"
git push -u origin o3-translated-mmio-result-pairs
```

### Task 2: Add Two-Stage Translated Result Authorization

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/memory_result_authorization/translated.rs`
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/translated_result_pair.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/memory_result_authorization.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_effect_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_pair_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_memory_result_window.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Write authorization RED tests**

Attach the new test child in `riscv_fetch_ahead/tests.rs`:

```rust
mod translated_result_pair;
```

Create these exact tests:

```text
translated_result_pair_authorizes_two_virtual_rows_without_physical_targets
translated_result_pair_binds_each_physical_range_and_target_once
translated_result_pair_rejects_wrong_virtual_span_rebind_and_target_change
translated_result_pair_rejects_dependent_second_address_and_third_result
```

The binding test must exercise this contract:

```rust
let virtual_range = AddressRange::new(
    Address::new(0x5000),
    AccessSize::new(8).unwrap(),
)
.unwrap();
let mut authorization = O3MemoryResultWindowAuthorization::translated_unbound(
    Some(Register::new(12).unwrap()),
    virtual_range,
    O3MemoryResultWindowRole::YoungerRead,
);

assert!(authorization.bind_translated(
    virtual_range.start(),
    Address::new(0x8000_2000),
    virtual_range.size(),
));
assert!(authorization.bind_target(O3MemoryResultWindowRoute::Memory));
assert!(authorization.matches_bound_target(
    O3MemoryResultWindowRoute::Memory,
    Address::new(0x8000_2000),
    virtual_range.size(),
));
assert!(!authorization.bind_target(O3MemoryResultWindowRoute::Mmio));
```

The virtual-window test must build a translated head plus a younger scalar `LD` on another virtual page, leave the younger TLB entry absent, and require two authorizations with roles `Head` and `YoungerRead`.

- [ ] **Step 2: Run RED**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::translated_result_pair -- --nocapture
```

Expected: compile failures for `translated_unbound`, `bind_translated`, `bind_target`, and `matches_bound_target`, followed by behavioral failures because translated younger reads are currently rejected.

- [ ] **Step 3: Extend the canonical authorization**

Add one route state and one address state:

```rust
pub(crate) enum O3MemoryResultWindowRoute {
    Memory,
    Mmio,
    Translated,
}

pub(crate) enum O3MemoryResultWindowAddressAuthority {
    ResolvedRange(AddressRange),
    TranslatedRange {
        virtual_range: AddressRange,
        physical_range: Option<AddressRange>,
        target: Option<O3MemoryResultWindowRoute>,
    },
    DependentSource {
        register: Register,
        width: MemoryWidth,
        immediate: Immediate,
    },
}
```

Attach `memory_result_authorization/translated.rs` and implement these exact methods there:

```rust
pub(in crate::riscv_fetch_ahead) const fn translated_unbound(
    integer_destination: Option<Register>,
    virtual_range: AddressRange,
    role: O3MemoryResultWindowRole,
) -> Self;

pub(crate) fn bind_translated(
    &mut self,
    virtual_address: Address,
    physical_address: Address,
    size: AccessSize,
) -> bool;

pub(crate) fn bind_target(&mut self, route: O3MemoryResultWindowRoute) -> bool;
pub(crate) const fn is_translated(self) -> bool;
pub(crate) const fn virtual_range(self) -> Option<AddressRange>;
pub(crate) fn matches_virtual_range(self, address: Address, size: AccessSize) -> bool;
pub(crate) fn matches_bound_target(
    self,
    route: O3MemoryResultWindowRoute,
    physical_address: Address,
    size: AccessSize,
) -> bool;
```

`bind_translated` and `bind_target` are idempotent only for the same value. A different physical range or target returns `false` without mutation. Existing resolved and dependent constructors retain their current semantics.

- [ ] **Step 4: Authorize a translated younger scalar result by virtual span**

Move translated-only construction into `data_access_result_translation.rs`. Add:

```rust
pub(super) fn translated_younger_result_authorization(
    state: &RiscvCoreState,
    instruction: &RiscvCompletedFetchInstruction,
) -> Option<O3MemoryResultWindowAuthorization>;
```

It must:

- require configured data translation and detailed O3 mode;
- accept only a four-byte scalar `LD` with a nonzero destination;
- execute on a cloned hart to derive the virtual access;
- compute the exact masked request span;
- create `translated_unbound(..., YoungerRead)` even when the TLB has no entry; and
- reject dependent address sources, stores, LR/SC, atomics, FP/vector loads, zero destinations, and a third result.

Change `data_access_result_younger_authorization()` to use this function when translation is configured. Keep untranslated behavior unchanged.

`result_head_allows_younger_read()` must continue to reject translated younger rows behind atomic heads unless both physical ranges are resolved and disjoint. Scalar load heads may authorize the translated younger row.

- [ ] **Step 5: Let the overlap predicate validate virtual authority**

In `riscv_memory_result_window.rs`:

- remove the unconditional `data_translation.is_some()` rejection;
- accept a translated younger authorization only when `matches_virtual_range()` matches the actual access;
- keep result destination, row-count, role, dependency, and runtime window checks; and
- continue requiring resolved cacheable memory for untranslated rows.

The resident head O3 row remains the canonical shared window identity. Do not add a second persistent queue or a duplicate window map.

- [ ] **Step 6: Turn the authorization tests GREEN**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::translated_result_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::data_access_result_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --test source_policy riscv_memory_result_authorization_has_focused_ownership -- --exact
cargo fmt --all -- --check
git diff --check
```

Expected: new tests pass; all existing untranslated pair tests remain green; the parent authorization owner stays within 150 lines and the new child stays within 220 lines.

- [ ] **Step 7: Commit and push**

```bash
git add crates/rem6-cpu/src/riscv_fetch_ahead \
  crates/rem6-cpu/src/riscv_memory_result_window.rs \
  crates/rem6-cpu/tests/source_policy.rs
env TMPDIR="$PWD/target/tmp" git commit -m "feat: authorize translated o3 result pairs"
git push
```

### Task 3: Make Memory-Result Issue Width Authoritative

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_memory_result_admission.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_memory_result_tests/translated_mmio_result_pair.rs`
- Create: `crates/rem6-cpu/src/riscv_data_issue/o3_result_pair_admission.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_memory_result_window.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Write issue-calendar RED tests**

Attach the new runtime test child and add:

```text
translated_result_pair_memory_width_one_selects_the_next_tick
translated_result_pair_memory_width_two_reuses_the_head_tick
translated_result_pair_total_width_one_still_selects_the_next_tick
```

Each test stages one memory-result head at tick 40, configures total and memory widths, and calls:

```rust
assert_eq!(runtime.next_memory_result_issue_tick(40), Some(expected_tick));
```

Expected values:

```text
total=4 memory=1 -> 41
total=4 memory=2 -> 40
total=1 memory=2 -> 41
```

- [ ] **Step 2: Run RED**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib o3_runtime_memory_result_tests::translated_mmio_result_pair -- --nocapture
```

Expected: compile failure because `next_memory_result_issue_tick` does not exist.

- [ ] **Step 3: Add a focused calendar query**

Add this method inside `O3LiveIssueCalendar`:

```rust
pub(in crate::o3_runtime) fn next_memory_slot_at_or_after(&self, earliest_tick: u64) -> u64;
```

It must scan ticks monotonically and return the first tick where both are true:

```text
reserved total rows < configured total issue width
reserved memory rows < configured memory issue width
```

Use saturating tick increment and return the current tick if increment cannot advance.

Create `o3_runtime_memory_result_admission.rs` with:

```rust
impl O3RuntimeState {
    pub(crate) fn next_memory_result_issue_tick(&self, earliest_tick: u64) -> Option<u64> {
        let head = self.live_data_accesses.first()?;
        if self.live_data_accesses.len() != 1
            || !self.can_consider_memory_result_younger()
        {
            return None;
        }
        let reservation = O3LiveIssueHeadReservation::memory(head.sequence, head.issue_tick);
        Some(
            O3LiveIssueCalendar::capture(self, reservation)
                .next_memory_slot_at_or_after(earliest_tick),
        )
    }
}
```

Attach this owner from `o3_runtime.rs`. Keep `o3_runtime.rs < 1,200` lines.

- [ ] **Step 4: Define the shared progress decision**

Create `riscv_data_issue/o3_result_pair_admission.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3ResultPairProgress {
    Ordinary,
    Ready { issue_tick: Tick },
    WaitUntil(Tick),
    Blocked,
}

impl RiscvCore {
    pub(crate) fn translated_result_pair_progress(
        &self,
        now: Tick,
    ) -> O3ResultPairProgress;
}
```

The query must return:

- `Ordinary` when no request is outstanding;
- `Ready` only for one resident head row, one matching older outstanding request, one exact translated younger authorization, no unrelated buffered/pending row, available ROB/LSQ depth, and selected issue tick `<= now`;
- `WaitUntil(tick)` when the exact pair is valid but the calendar selects a future tick; and
- `Blocked` for every other nonempty outstanding state.

The check derives window identity from the resident head O3 sequence and request identity. It must not accept an unrelated outstanding request merely because a younger authorization exists.

- [ ] **Step 5: Verify GREEN**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib o3_runtime_memory_result_tests::translated_mmio_result_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib o3_runtime_issue::calendar_tests -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --test source_policy o3 -- --nocapture
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 6: Commit and push**

```bash
git add crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar.rs \
  crates/rem6-cpu/src/o3_runtime_memory_result_admission.rs \
  crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs \
  crates/rem6-cpu/src/o3_runtime_memory_result_tests/translated_mmio_result_pair.rs \
  crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_data_issue/o3_result_pair_admission.rs \
  crates/rem6-cpu/src/riscv_memory_result_window.rs \
  crates/rem6-cpu/tests/source_policy.rs
env TMPDIR="$PWD/target/tmp" git commit -m "feat: select translated result pair issue ticks"
git push
```

### Task 4: Add Translated-Memory Top-Level Rows As RED Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/fixture.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/boundaries.rs`
- Create: `crates/rem6/tests/source_policy/o3_translated_mmio_pair_ownership.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`

- [ ] **Step 1: Attach the focused CLI and policy modules**

In `result_classes.rs` add:

```rust
#[path = "result_classes/translated_mmio_pairs.rs"]
mod translated_mmio_pairs;
```

In the new parent add:

```rust
use super::super::*;

#[path = "translated_mmio_pairs/boundaries.rs"]
mod boundaries;
#[path = "translated_mmio_pairs/fixture.rs"]
mod fixture;
```

Attach `o3_translated_mmio_pair_ownership.rs` from `source_policy.rs`. Extend only the closed `RESULT_CLASS_CHILD_MODULES` inventory in `writeback_ownership.rs`; caps and anchors for the new family belong in the focused owner.

- [ ] **Step 2: Build the translated pair fixture**

Use these constants and exact pair PCs:

```rust
const FIRST_PC: &str = "0x80000030";
const SECOND_PC: &str = "0x80000034";
const DIV_PC: &str = "0x80000038";
const DEPENDENT_PC: &str = "0x8000003c";
const FIRST_VIRTUAL_PAGE: u64 = 0x4000;
const SECOND_VIRTUAL_PAGE: u64 = 0x5000;
const FIRST_PHYSICAL_PAGE: u64 = 0x8000_1000;
const SECOND_PHYSICAL_PAGE: u64 = 0x8000_2000;
```

Build one ELF whose four-row O3 window is:

```text
LD x11, first virtual data address
LD x12, second virtual data address
DIV x3, x1, x2
ADDI x13, x12, 1
```

Pad setup so those instructions remain at the exact PCs above. Both mappings target disjoint cacheable-memory ranges in this task. Task 6 extends the same fixture with the readfile-MMIO target.

Write a temporary TOML config with:

```toml
[run]
isa = "riscv"
execute = true
stats_format = "json"
debug_flags = ["O3", "Data", "Fetch", "Memory", "HostAction"]
memory_system = "<direct-or-cache-fabric-dram>"
memory_route_delay = <calibrated>
m5_switch_cpu_mode = "detailed"
riscv_o3_issue_width = 4
riscv_o3_memory_issue_width = 2
riscv_o3_writeback_width = <1-or-2>
riscv_o3_scalar_memory_depth = 4

[run.riscv_data_translation]
queue_capacity = 4
latency = 2
tlb_capacity = 4
page_size = 4096

[[run.riscv_data_translation.mappings]]
virtual_base = 16384
physical_base = 2147487744
pages = 1
read = true
write = true

[[run.riscv_data_translation.mappings]]
virtual_base = 20480
physical_base = 2147491840
pages = 1
read = true
write = true
```

- [ ] **Step 3: Add the three translated-memory positive anchors**

Create these tests in this exact order:

```text
rem6_run_o3_translated_memory_result_pair_width_one_direct
rem6_run_o3_translated_memory_result_pair_width_two_exact_fit_direct
rem6_run_o3_translated_memory_result_pair_width_one_cache_fabric_dram
```

Every row must assert:

- both pair requests issue before the earliest pair response;
- exact four-row ROB and two-row LSQ residency before the first response;
- distinct fetch and data request identities;
- width-one serializes colliding result writeback and width-two admits the exact pair;
- `x11`, `x12`, `x13`, and the final memory witnesses are exact;
- commit remains oldest-first; and
- direct and hierarchy resource activity match the selected route.

Filter debug traces by the two O3 pair PCs so setup traffic cannot satisfy pair assertions.

- [ ] **Step 4: Add structural ownership policy**

The focused owner must initially enforce:

```text
CLI parent <= 500 lines
fixture <= 600 lines
boundaries <= 650 lines
family aggregate <= 1,500 lines
ownership policy <= 350 lines
```

It must also enforce exact external module paths, no top-level `include!`, no duplicate owner, and rustfmt-clean files. Anchor registration is added only in Task 8 after all tests pass.

- [ ] **Step 5: Run RED and do not commit the failing state**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test cli_run rem6_run_o3_translated_memory_result_pair -- --nocapture
```

Expected: all three translated-memory rows fail because the driver still blocks a second outstanding translated request.

### Task 5: Enable Two Outstanding Translated Memory Results

**Files:**
- Create: `crates/rem6-cpu/src/riscv_translation/o3_result_pair.rs`
- Create: `crates/rem6-cpu/src/riscv_translation_tests/translated_mmio_result_pair.rs`
- Create: `crates/rem6-cpu/src/riscv_data_issue_tests/translated_mmio_result_pair.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation/helpers.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_memory_result_window.rs`
- Include the uncommitted RED CLI files from Task 4.

- [ ] **Step 1: Add CPU RED tests for memory-memory overlap**

Attach the new CPU test children and add:

```text
translation_preserves_and_binds_each_result_pair_authorization_once
translated_memory_pair_issues_two_requests_before_first_response
translated_result_pair_rejects_unrelated_outstanding_request
translated_result_pair_waits_for_the_calendar_selected_tick
translated_cluster_turns_emit_one_action_per_pass_and_two_requests_before_response
```

The request-overlap test must:

```rust
assert_eq!(core.outstanding_data_request_count_for_test(), 2);
assert_eq!(core.o3_runtime_snapshot().reorder_buffer().len(), 2);
assert_eq!(core.o3_runtime_snapshot().load_store_queue().len(), 2);
assert_ne!(first_data_request, second_data_request);
```

The cluster test must call the translated cluster driver twice at the selected tick, require one `DataAccessIssued` action per call, and assert both requests are outstanding before any response callback runs.

- [ ] **Step 2: Run CPU RED**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib translated_mmio_result_pair -- --nocapture
```

Expected: the authorization disappears on a pending translation or the second translated request is blocked by the serial/cluster outstanding guard.

- [ ] **Step 3: Preserve and bind translated authority**

Attach `riscv_translation/o3_result_pair.rs`. Implement:

```rust
impl RiscvCoreState {
    pub(crate) fn translated_result_authorization_is_pending(
        &self,
        fetch_request: MemoryRequestId,
        virtual_address: Address,
        size: AccessSize,
    ) -> bool;

    pub(crate) fn bind_translated_result_range(
        &mut self,
        translated: &TranslatedDataAccess,
    ) -> bool;

    pub(crate) fn bind_translated_result_target(
        &mut self,
        fetch_request: MemoryRequestId,
        route: O3MemoryResultWindowRoute,
    ) -> bool;
}
```

Change `enqueue_next_data_translation()` so the cold pending branch removes a result authorization only when it is not an exact translated-unbound authorization for that fetch and virtual span. On immediate and asynchronous translation completion, call `bind_translated_result_range()` before inserting into `ready_translated_data`. A mismatch fails closed through the existing preparation-error cleanup.

- [ ] **Step 4: Replace only translated-driver blanket gates**

In serial translated drive and both translated cluster loops, replace:

```rust
if core.has_outstanding_data_request() || core.has_pending_trap() {
    continue;
}
```

with logic based on `translated_result_pair_progress(now)`:

```rust
match core.translated_result_pair_progress(scheduler.now()) {
    O3ResultPairProgress::Ordinary | O3ResultPairProgress::Ready { .. } => {}
    O3ResultPairProgress::WaitUntil(_) | O3ResultPairProgress::Blocked => continue,
}
```

Use the equivalent `return Ok(None)` form in the serial driver. Keep one action per core per pass. Do not alter untranslated `prepare_mmio_data_access()`.

- [ ] **Step 5: Bind the memory target before issue recording**

After translated PMP/PMA/route/line-layout validation succeeds in `prepare_translated_data_access()`, bind the authorization target to `Memory`. `try_record_data_issue_state()` must continue consuming the same authorization through `matches_bound_target()` and staging the row with `O3DataAccessWindowPolicy::MemoryResultWindow`.

If transport submission fails, retry may reuse the same identical binding; a different route/range must fail.

- [ ] **Step 6: Turn CPU and translated-memory CLI rows GREEN**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib translated_mmio_result_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test cli_run rem6_run_o3_translated_memory_result_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test source_policy o3_translated_mmio_pair_ownership -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Expected: all CPU tests and the three memory-memory CLI rows pass. No mixed-MMIO CLI test exists yet, so the committed milestone is fully green.

- [ ] **Step 7: Commit and push the green memory pair**

```bash
git add crates/rem6-cpu \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/fixture.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/boundaries.rs \
  crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/o3_translated_mmio_pair_ownership.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs
env TMPDIR="$PWD/target/tmp" git commit -m "feat: overlap translated o3 result requests"
git push
```

### Task 6: Add Mixed Memory/MMIO Completion And Handoff

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation/o3_result_pair.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/translated_mmio_result_pair.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation_tests/translated_mmio_result_pair.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/fixture.rs`

- [ ] **Step 1: Extend the fixture and add mixed-target RED tests**

Add the target selector and MMIO address to the fixture:

```rust
const MMIO_PAGE: u64 = 0x1000_0000;

enum TranslatedPairTarget {
    Memory,
    Mmio,
}
```

Let the second translation mapping select `SECOND_PHYSICAL_PAGE` or `MMIO_PAGE`. For MMIO rows add one readfile at `0x10000000:0x100:<payload>`.

Add these exact top-level tests after the three translated-memory rows:

```text
rem6_run_o3_translated_memory_mmio_result_pair_width_one_direct
rem6_run_o3_translated_memory_mmio_result_pair_width_one_cache_fabric_dram
```

Both rows must prove one ordinary memory request and one MMIO request, distinct request identities, two rows issued before the first response, no ordinary memory request for `SECOND_PC`, and hierarchy activity only for the cacheable row. Filter every pair assertion by the exact two O3 PCs so setup traffic cannot satisfy it.

Add the CPU tests:

```text
translated_memory_mmio_pair_records_independent_targets_and_request_ids
memory_and_mmio_completions_cannot_cross_complete_pair_rows
younger_mmio_completion_before_memory_keeps_architecture_unpublished
translated_memory_mmio_pair_handoff_preserves_both_typed_targets
translated_result_pair_prebind_state_rejects_handoff
```

The handoff test must inspect `handoff.entries()` directly and require:

```rust
assert_eq!(handoff.entries().len(), 2);
assert!(matches!(handoff.entries()[0].target(), RiscvO3LiveDataHandoffTarget::Memory { .. }));
assert!(matches!(handoff.entries()[1].target(), RiscvO3LiveDataHandoffTarget::Mmio { .. }));
```

The younger-first test completes the MMIO request first, requires `x12` absent/old, then completes the older memory request and drains both rows in order.

- [ ] **Step 2: Run RED**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib translated_memory_mmio_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test cli_run rem6_run_o3_translated_memory_mmio_result_pair -- --nocapture
```

Expected: the translated MMIO preparation path returns no second issued O3 row or the unresolved authorization cannot bind to MMIO. Do not commit this RED state.

- [ ] **Step 3: Bind an exact MMIO target**

In `prepare_ready_translated_mmio_data_access()`:

1. validate PMP/PMA and construct the exact `MmioRequest`;
2. require an exact route from `MmioBus`;
3. validate source partition and scheduler lookahead;
4. bind the authorization target to `Mmio`; and
5. remove the ready translation only after all checks pass.

If the bus reports `UnmappedAddress`, leave the ready translation and authorization untouched so the ordinary translated memory preparation can bind `Memory`.

Do not add target-specific completion state. `record_data_response()` and `record_mmio_completion()` must continue to locate rows by `MemoryRequestId`.

- [ ] **Step 4: Turn mixed CPU and CLI rows GREEN**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib translated_memory_mmio_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test cli_run rem6_run_o3_translated_memory_mmio_result_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib live_data_handoff_round_trips_typed_memory_and_mmio_targets -- --exact
cargo fmt --all -- --check
git diff --check
```

Expected: both mixed CLI route rows pass; the existing handoff codec test remains unchanged and green.

- [ ] **Step 5: Commit and push**

```bash
git add crates/rem6-cpu/src/riscv_translation.rs \
  crates/rem6-cpu/src/riscv_translation/o3_result_pair.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/translated_mmio_result_pair.rs \
  crates/rem6-cpu/src/riscv_translation_tests/translated_mmio_result_pair.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/fixture.rs
env TMPDIR="$PWD/target/tmp" git commit -m "feat: pair translated memory and mmio results"
git push
```

### Task 7: Add Fault, Cleanup, Checkpoint, Transfer, Restore, And Timing Boundaries

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_translation/o3_result_pair.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation/helpers.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_gate.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/translated_mmio_result_pair.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs/fixture.rs`

- [ ] **Step 1: Add CPU cleanup RED tests**

Add:

```text
younger_translation_fault_preserves_older_request_and_allocates_no_younger_request
older_retry_discards_younger_translation_and_ignores_stale_completion
younger_retry_preserves_completed_older_result
redirect_clears_translated_pair_authorization_binding_and_ready_state
translated_result_pair_live_checkpoint_rejects_and_drained_capture_succeeds
```

Every retry/fault test must assert exact request counts before and after cleanup. A recovered run with duplicate request emission is a failure.

- [ ] **Step 2: Add one sequence-suffix cleanup helper**

Implement in `riscv_translation/o3_result_pair.rs`:

```rust
impl RiscvCoreState {
    pub(crate) fn discard_translated_result_pair_from(
        &mut self,
        fetch_request: MemoryRequestId,
    );
}
```

It must remove the matching fetch and younger same-agent fetch sequences from:

```text
memory_result_window_authorizations
pending_data_translations
ready_translated_data
translated_scalar_load_window_fetches
```

It must then invoke existing O3/runtime suffix cleanup rather than duplicating ROB, LSQ, rename, writeback, or live-data removal.

Use this helper from translation-fault, retry/failure, redirect, fetch reset, restart, and detailed-mode abort paths where those paths currently remove only one authorization or translation entry.

- [ ] **Step 3: Add the six boundary anchors**

Create these tests in exact order:

```text
rem6_run_o3_translated_result_pair_dependency_and_fault_boundaries
rem6_run_o3_translated_result_pair_target_ordering_and_capacity_boundaries
rem6_run_o3_translated_result_pair_live_checkpoint_and_prebind_switch_reject
rem6_run_host_switch_transfers_o3_translated_memory_mmio_result_pair
rem6_run_o3_translated_result_pair_drained_restore
rem6_run_timing_suppresses_o3_translated_result_pairs
```

Boundary requirements:

- dependent second address issues only after first admitted writeback;
- missing/denied second translation emits no second request and preserves the older request once;
- acquire/release atomic head, target mismatch, PMA/PMP denial, and third result remain outside the pair;
- live checkpoint and pre-bind switch fail without partial stdout or mutated authority;
- two fully bound transport-owned rows transfer to timing mode with `outstanding_requests=2`, `resident_rows=2`, baseline-equal issue/response/writeback/commit ticks, and CPU-level proof of memory plus MMIO typed targets;
- drained restore reports zero live ROB/LSQ rows while preserving maximum occupancy and final architecture; and
- timing mode produces the same final architecture with no O3 runtime, O3 trace rows, or `sim.cpu0.o3.*` stats.

- [ ] **Step 4: Run focused lifecycle GREEN**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --lib translated_mmio_result_pair -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test cli_run rem6_run_o3_translated_result_pair_ -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_o3_translated_memory_mmio_result_pair -- --exact --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_translated_result_pairs -- --exact --nocapture
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 5: Commit and push**

```bash
git add crates/rem6-cpu/src/riscv_translation \
  crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs \
  crates/rem6-cpu/src/riscv_live_retire_gate.rs \
  crates/rem6-cpu/src/riscv_fetch.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/translated_mmio_result_pair.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/translated_mmio_pairs
env TMPDIR="$PWD/target/tmp" git commit -m "test: complete translated result pair lifecycle"
git push
```

### Task 8: Ratchet Ownership, Register Evidence, Update The Ledger, And Finish

**Files:**
- Modify: `crates/rem6/tests/source_policy/o3_translated_mmio_pair_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Register exact anchor ownership**

Add these arrays to the focused source-policy owner:

```rust
const POSITIVE_ANCHORS: [&str; 5] = [
    "rem6_run_o3_translated_memory_result_pair_width_one_direct",
    "rem6_run_o3_translated_memory_result_pair_width_two_exact_fit_direct",
    "rem6_run_o3_translated_memory_result_pair_width_one_cache_fabric_dram",
    "rem6_run_o3_translated_memory_mmio_result_pair_width_one_direct",
    "rem6_run_o3_translated_memory_mmio_result_pair_width_one_cache_fabric_dram",
];

const BOUNDARY_ANCHORS: [&str; 6] = [
    "rem6_run_o3_translated_result_pair_dependency_and_fault_boundaries",
    "rem6_run_o3_translated_result_pair_target_ordering_and_capacity_boundaries",
    "rem6_run_o3_translated_result_pair_live_checkpoint_and_prebind_switch_reject",
    "rem6_run_host_switch_transfers_o3_translated_memory_mmio_result_pair",
    "rem6_run_o3_translated_result_pair_drained_restore",
    "rem6_run_timing_suppresses_o3_translated_result_pairs",
];
```

Require exact top-level test inventories in the parent and boundary modules, one occurrence per owner, no duplicate owners elsewhere, and contiguous core-anchor registration.

Insert all eleven anchors immediately after the existing five untranslated result-pair anchors and before generic result-boundary anchors.

- [ ] **Step 2: Ratchet production ownership**

Update `crates/rem6-cpu/tests/source_policy.rs` to require:

```text
data_access_result_translation.rs <= 250
memory_result_authorization/translated.rs <= 220
o3_runtime_memory_result_admission.rs <= 220
riscv_data_issue/o3_result_pair_admission.rs <= 300
riscv_translation/o3_result_pair.rs <= 500
```

Require exact module paths and prohibit duplicate translated-pair admission/binding functions in roots. Preserve the existing strict limits on `data_access_result.rs`, `riscv_translation.rs`, `riscv_cluster.rs`, `riscv_data_issue.rs`, and `riscv_execution_mode_handoff.rs`.

- [ ] **Step 3: Update the CPU ledger and derive the score**

After all executable evidence passes:

- keep the checklist count at `8 of 10` unless the existing source-policy scoring owner proves a different count;
- recompute the displayed percentage and status label from the repository's existing scoring rules rather than predeclaring a number in this plan;
- remove the `74% representative` cap only if the completed translated-memory and mixed memory/MMIO matrix satisfies the scoring owner's cap-removal conditions;
- describe the resulting status as matrix-gapped only if that exact label follows from the existing ledger conventions;
- add all eleven exact anchors to Migrated/Evidence prose;
- remove only `translated or MMIO result pairs` from Not migrated and Next evidence;
- update the `tests/gem5/cpu_tests` crosswalk row to the same boundary; and
- keep the general running-O3 checklist item unchecked.

Do not raise the score from prose or anchor count alone. Add or update a source-policy assertion for the derived heading so an unsupported score change fails verification.

Keep arbitrary translated result depth, broader result classes, dependent translated stores/atomics, page-table-walk transport, arbitrary mixed-target graphs, restorable in-flight checkpoints, fifth/deeper requests, and a general O3 engine open.

Preserve exactly 1,200 ledger lines.

- [ ] **Step 4: Run focused policy and ledger verification**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test source_policy o3_translated_mmio_pair_ownership -- --nocapture
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --exact
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --test source_policy
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
cargo fmt --all -- --check
git diff --check
git status --short -- temp
```

Expected: all policy tests pass, the ledger is exactly 1,200 lines, and `temp/` is unchanged.

- [ ] **Step 5: Run package and workspace verification**

```bash
env TMPDIR="$PWD/target/tmp" cargo test -p rem6-cpu --all-targets
env TMPDIR="$PWD/target/tmp" cargo test -p rem6 --all-targets
env TMPDIR="$PWD/target/tmp" cargo test --workspace --all-targets
```

Expected: zero failures.

- [ ] **Step 6: Dispatch final high-intensity review**

Dispatch four fresh read-only reviewers in parallel:

```text
Reviewer 1: production authorization, translation, and issue-calendar correctness
Reviewer 2: fault/retry/redirect/checkpoint/handoff lifecycle correctness
Reviewer 3: CLI matrix, trace assertions, negative cases, and mutation strength
Reviewer 4: source ownership, anchors, ledger score, and open-boundary accuracy
```

Give each reviewer the approved design, this plan, `temp/improve-rem6-0.md`, and the full branch diff from `55506fdf`. Fix every actionable finding, rerun affected focused tests, then rerun the three package/workspace commands above.

- [ ] **Step 7: Commit final evidence and push the feature branch**

```bash
git add crates/rem6/tests/source_policy \
  crates/rem6/tests/source_policy.rs \
  crates/rem6-cpu/tests/source_policy.rs \
  docs/architecture/gem5-to-rem6-migration.md
env TMPDIR="$PWD/target/tmp" git commit -m "docs: record translated mmio o3 result pairs"
git push
```

- [ ] **Step 8: Fast-forward `main`, verify, and push**

From `/home/sihao/github.com/SihaoLiu/rem6`:

```bash
git status --short --branch
git merge --ff-only o3-translated-mmio-result-pairs
env TMPDIR=/home/sihao/.cache/codex-tmp/rem6-o3-translated-mmio-result-pairs cargo test --workspace --all-targets
git push origin main
```

Confirm `main` and `origin/main` point to the same commit. Remove only this owned worktree and local feature branch after the pushed `main` verification succeeds; do not alter unrelated worktrees or branches.
