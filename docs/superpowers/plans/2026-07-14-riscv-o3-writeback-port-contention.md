# RISC-V O3 Writeback-Port Contention Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one configurable scalar integer FU/load writeback-port width control real ROB readiness, dependency wakeup, trace timing, retirement, stats, checkpoint compatibility, and mode-transfer evidence in the bounded detailed RISC-V O3 live window.

**Architecture:** Keep `O3WritebackTransferPolicy` and `O3WritebackTransferBuffer` generic, extending them only with occupied-slot planning. Add one focused transient `O3WritebackReservationCalendar` for absolute-cycle capacity and one focused per-core scalar-load wake bridge; live FU/load owners retain admitted ticks as timing results after calendar entries are consumed. Preserve the existing ROB/LSQ/rename/issue/retirement authorities, keep live transport non-restorable, and add checkpoint v23 only for cumulative counters plus explicit legacy-origin normalization.

**Tech Stack:** Rust workspace, `rem6-cpu`, `rem6-system`, handwritten `rem6` CLI/TOML configuration, partitioned scheduler, O3 runtime/checkpoint codecs, JSON/text/debug stats, Cargo tests, source-policy tests.

---

## File Structure

- Modify `crates/rem6-cpu/src/riscv_defaults.rs`: own writeback-width minimum, default, and maximum constants.
- Modify `crates/rem6-cpu/src/public_api.rs`: export the shared writeback-width constants.
- Modify `crates/rem6-cpu/src/error.rs`: propagate reservation and tick-overflow failures through `RiscvCpuError`.
- Modify `crates/rem6-cpu/src/lib.rs`: declare the wake module, store wake state, and expose the core width setter and wake bridge.
- Modify `crates/rem6-cpu/src/o3_pipeline.rs`: add generic occupied-slot current-cycle planning without RISC-V policy.
- Modify `crates/rem6-cpu/tests/o3_pipeline.rs`: prove occupied-slot validation and deferred-before-new ordering.
- Modify `crates/rem6-cpu/src/o3_runtime.rs`: declare the focused writeback module and retain the transient calendar/stat-dedup state.
- Create `crates/rem6-cpu/src/o3_runtime_writeback.rs`: own completion classification, reservation calendar, absolute-cycle planning, admitted ticks, cleanup, and counter recording.
- Modify `crates/rem6-cpu/src/o3_runtime_writeback_tests.rs`: extend the focused width tests with calendar, mixed-ready-tick, re-entry, reset, and cleanup coverage.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window.rs`: store raw/admitted FU ticks and consume admitted dependency timing.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`: update fallible live-execution fixtures and assert admitted timing.
- Modify `crates/rem6-cpu/src/o3_runtime_issue.rs`: reserve each fixed completion as it is recorded and schedule descendants from admitted producer ticks.
- Modify `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`: prove issue waits for admitted writeback rather than raw completion.
- Modify `crates/rem6-cpu/src/o3_runtime_live_window.rs`: carry admitted ticks into ROB readiness and retired-row timing.
- Modify `crates/rem6-cpu/src/o3_runtime_handoff.rs`: update scalar-memory handoff completion fixtures for fallible reservation.
- Modify `crates/rem6-cpu/src/o3_runtime_retire.rs`: propagate explicit live writeback ticks into trace construction.
- Modify `crates/rem6-cpu/src/o3_runtime_trace.rs`: store an optional admitted writeback tick while retaining legacy derived timing.
- Modify `crates/rem6-cpu/src/o3_runtime_authority.rs`: include transient writeback reservations in retirement authority.
- Modify `crates/rem6-cpu/src/riscv_live_retire_window.rs`: prepare/reserve fixed-FU rows before arming the live-retire gate.
- Modify `crates/rem6-cpu/src/riscv_live_retire_gate.rs`: expose exact admitted gate timing and wake ownership to cleanup/checkpoint checks.
- Modify `crates/rem6-cpu/src/o3_runtime_memory.rs`: reserve completed load writeback and delay ROB/publication until the admitted tick.
- Modify `crates/rem6-cpu/src/o3_runtime_memory_window.rs`: pass explicit publication ticks in scalar-memory window tests.
- Modify `crates/rem6-cpu/src/riscv_data_issue.rs`: update load-dependent wakeup from the admitted tick and refresh desired wake state.
- Modify `crates/rem6-cpu/src/riscv_data_issue_tests.rs`: prove load-dependent issue consumes admitted writeback timing.
- Modify `crates/rem6-cpu/src/riscv_data_issue_tests/forwarding.rs`: pass explicit publication ticks in forwarding tests.
- Modify `crates/rem6-cpu/src/riscv_data_issue_tests/multi_load.rs`: pass explicit publication ticks in multi-load tests.
- Modify `crates/rem6-cpu/src/riscv_data_issue_tests/store_store_load.rs`: pass explicit publication ticks in store/load tests.
- Create `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`: own desired, scheduled, fired, and detached scalar-load writeback wakes.
- Modify `crates/rem6-cpu/src/riscv_drive.rs`: surface sticky transport-callback reservation failures from direct serial drive.
- Modify `crates/rem6-cpu/src/riscv_cluster.rs`: surface callback failures from every serial/parallel drive path and after scheduler dispatch.
- Modify `crates/rem6-cpu/src/riscv_execute.rs`: pass retirement ticks into continuing-execution rollback cleanup.
- Modify `crates/rem6-cpu/src/riscv_hart_run_state.rs`: clear writeback calendar/wake authority on hart reset and run-state teardown.
- Modify `crates/rem6-cpu/src/riscv_htm.rs`: clear transient writeback authority on HTM abort.
- Modify `crates/rem6-system/src/lib.rs`: schedule serial and parallel requested writeback wakes after each cluster turn.
- Modify `crates/rem6-system/src/riscv_run_stats.rs`: pass the scheduler tick into scalar-memory publication.
- Modify `crates/rem6-system/src/riscv_o3_runtime_stats.rs`: add real serial/parallel wake and publication tests.
- Modify `crates/rem6-cpu/src/o3_runtime_stats.rs`: add six writeback-port counters and max/sum semantics.
- Modify `crates/rem6-cpu/src/o3_runtime_checkpoint.rs`: add v23 stats, private decode origin, legacy normalization, and current-payload validation.
- Modify `crates/rem6-cpu/src/o3_runtime_checkpoint_tests.rs`: prove v23/v22 compatibility and legacy pending-only normalization.
- Modify `crates/rem6-system/src/riscv_checkpoint.rs`: include transient calendar/wake authority in quiescence and expose debug projection.
- Modify `crates/rem6-system/src/riscv_checkpoint/o3_payload.rs`: consume the CPU-owned legacy-pending constructor instead of rebuilding current-origin state.
- Modify `crates/rem6-system/tests/riscv_checkpoint/o3_compatibility.rs`: cover legacy pending, v22, invalid v23, and no-partial-restore behavior.
- Modify `crates/rem6-system/tests/source_policy.rs`: protect the CPU-owned legacy normalization bridge.
- Modify `crates/rem6/src/config.rs`: parse/store CLI and TOML writeback width while remaining below the existing 1,700-line cap.
- Modify `crates/rem6/src/config/riscv_timing.rs`: parse and validate width from shared CPU constants.
- Modify `crates/rem6/src/config/accessors.rs`: expose effective and explicit writeback-width accessors.
- Modify `crates/rem6/src/config/file_scan.rs`: classify the new flag as value-taking.
- Modify `crates/rem6/src/cli_error.rs`: add typed invalid/routing errors.
- Modify `crates/rem6/src/run_validation.rs`: enforce `--execute` and RISC-V requirements.
- Modify `crates/rem6/src/riscv_core_runtime.rs`: configure every constructed core.
- Modify `crates/rem6/tests/cli_run/validation.rs`: prove CLI/TOML acceptance, rejection, routing, and config scanning.
- Modify `crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs`: register/update six native counters.
- Modify `crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs`: snapshot sum and max fields correctly.
- Create `crates/rem6/src/stats_output/o3_runtime_writeback.rs`: emit the six native text/final stats.
- Modify `crates/rem6/src/stats_output/o3_runtime.rs`: delegate writeback-port emission.
- Modify `crates/rem6/src/core_summary_json.rs`: add structured `o3_runtime.writeback_port` JSON.
- Modify `crates/rem6-system/src/host.rs`: carry transient writeback debug state in mode-transfer summaries.
- Modify `crates/rem6-system/src/host/execution_mode_handoff.rs`: capture resident writeback state without serializing it.
- Modify `crates/rem6-system/src/host/execution_mode_transfer.rs`: preserve debug-only writeback projections through transfer construction.
- Modify `crates/rem6/src/host_actions.rs`: expose width, reservations, earliest unpublished tick, and decoded counters in transfer/debug summaries.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`: own the real CLI timing, route, cleanup, checkpoint, transfer, stats, and suppression matrix.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/switch.rs`: expose the existing live-transfer fixture to the focused writeback test.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`: register the focused CLI module.
- Modify `crates/rem6-cpu/tests/source_policy.rs`: protect focused runtime/calendar ownership.
- Modify `crates/rem6/tests/source_policy.rs`: protect focused stats output and module boundaries.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: anchor configuration, artifacts, stats, and every top-level row.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record executable evidence while preserving 74%, 8/10, both unchecked items, and exactly 1,200 lines.

### Task 0: Verify the Clean Baseline

**Files:**
- Read only: current worktree and existing tests.

- [ ] **Step 1: Confirm the execution workspace**

Run:

```bash
git status --short --branch
git rev-parse HEAD origin/main
```

Expected: `main` is clean and both revisions equal the committed plan revision.
Work in this checkout because the user explicitly requested commits and pushes
on the active branch.

- [ ] **Step 2: Run baseline suites before behavior edits**

Run:

```bash
cargo test -p rem6-cpu --test o3_pipeline o3_writeback_transfer -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_memory --lib -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::scoped_issue:: -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::lsq_fu_window:: -- --nocapture
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: every command exits zero. If a baseline fails, isolate the pre-existing
failure before starting Task 1.

### Task 1: Configure RISC-V O3 Writeback Width

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_defaults.rs`
- Modify: `crates/rem6-cpu/src/public_api.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6/src/config.rs`
- Modify: `crates/rem6/src/config/riscv_timing.rs`
- Modify: `crates/rem6/src/config/accessors.rs`
- Modify: `crates/rem6/src/config/file_scan.rs`
- Modify: `crates/rem6/src/cli_error.rs`
- Modify: `crates/rem6/src/run_validation.rs`
- Modify: `crates/rem6/src/riscv_core_runtime.rs`
- Test: `crates/rem6/tests/cli_run/validation.rs`

- [ ] **Step 1: Write failing CLI, TOML, and CPU setter tests**

Mirror the existing issue-width table and add these exact validation rows:

```rust
#[test]
fn rem6_run_accepts_riscv_o3_writeback_width_cli_min_and_max() {
    let path = minimal_riscv_run_binary("riscv-o3-writeback-width-cli");
    for width in ["1", "4"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run", "--isa", "riscv", "--binary", path.to_str().unwrap(),
                "--max-tick", "80", "--stats-format", "json", "--execute",
                "--riscv-o3-writeback-width", width,
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "width {width}: {}", String::from_utf8_lossy(&output.stderr));
    }
}

#[test]
fn rem6_run_rejects_invalid_riscv_o3_writeback_width_values() {
    let path = minimal_riscv_run_binary("riscv-o3-writeback-width-invalid");
    for value in ["0", "5", "wide"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run", "--isa", "riscv", "--binary", path.to_str().unwrap(),
                "--max-tick", "80", "--stats-format", "json", "--execute",
                "--riscv-o3-writeback-width", value,
            ])
            .output()
            .unwrap();
        assert!(!output.status.success(), "value {value} unexpectedly succeeded");
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains(&format!("invalid RISC-V O3 writeback width {value}")));
    }
}
```

Add TOML acceptance for `1` and `4`, TOML rejection for `0` and `5`, rejection
without `--execute`, rejection with `--isa x86`, and
`rem6_run_config_scan_treats_riscv_o3_writeback_width_as_value_taking` beside
the corresponding issue-width tests.

Add CPU tests beside the current issue-width tests:

```rust
#[test]
fn o3_writeback_width_defaults_to_one_and_rejects_out_of_range_updates() {
    let mut runtime = O3RuntimeState::default();
    assert_eq!(runtime.writeback_width(), 1);
    assert!(runtime.set_writeback_width(4));
    assert_eq!(runtime.writeback_width(), 4);
    assert!(!runtime.set_writeback_width(0));
    assert!(!runtime.set_writeback_width(5));
    assert_eq!(runtime.writeback_width(), 4);
}
```

- [ ] **Step 2: Run the new tests and verify RED**

Run:

```bash
cargo test -p rem6-cpu o3_writeback_width --lib -- --nocapture
cargo test -p rem6 --test cli_run riscv_o3_writeback_width -- --nocapture
```

Expected: FAIL because the constants, setter, CLI flag, TOML field, and typed
errors do not exist.

- [ ] **Step 3: Add shared constants and rebuild the pending-state policy**

Add and export:

```rust
pub const MIN_RISCV_O3_WRITEBACK_WIDTH: usize = 1;
pub const DEFAULT_RISCV_O3_WRITEBACK_WIDTH: usize = 1;
pub const MAX_RISCV_O3_WRITEBACK_WIDTH: usize = 4;
```

Add to `O3RuntimeState`:

```rust
pub(crate) fn writeback_width(&self) -> usize {
    self.snapshot
        .pending_state()
        .writeback()
        .policy()
        .writeback_width()
}

pub(crate) fn set_writeback_width(&mut self, width: usize) -> bool {
    if !(MIN_RISCV_O3_WRITEBACK_WIDTH..=MAX_RISCV_O3_WRITEBACK_WIDTH).contains(&width) {
        return false;
    }
    let pending = self.snapshot.pending_state().clone();
    let writeback = O3WritebackTransferSnapshot::new(
        O3WritebackTransferPolicy::new(O3PipelineStage::Iew, width, 0)
            .expect("validated RISC-V O3 writeback width"),
        pending.writeback().deferred().iter().copied(),
    );
    self.snapshot.pending_state = O3PendingStateSnapshot::new(
        pending.resolved_dependency_scopes().iter().copied(),
        pending.ready().iter().cloned(),
        writeback,
    )
    .expect("existing pending O3 state remains valid after a policy-width change");
    true
}
```

Expose `RiscvCore::set_o3_writeback_width` beside `set_o3_issue_width`; assert
the setter succeeds because CLI validation owns user-facing errors.

- [ ] **Step 4: Add handwritten CLI/TOML plumbing**

Add parse helpers in `config/riscv_timing.rs`:

```rust
pub(crate) fn parse_riscv_o3_writeback_width(value: &str) -> Result<usize, Rem6CliError> {
    let width = value.parse().map_err(|_| Rem6CliError::InvalidRiscvO3WritebackWidth {
        value: value.to_string(),
    })?;
    validate_riscv_o3_writeback_width(width, value.to_string())
}

pub(crate) fn validate_optional_riscv_o3_writeback_width(
    width: Option<usize>,
) -> Result<Option<usize>, Rem6CliError> {
    width
        .map(|width| validate_riscv_o3_writeback_width(width, width.to_string()))
        .transpose()
}

fn validate_riscv_o3_writeback_width(
    width: usize,
    value: String,
) -> Result<usize, Rem6CliError> {
    if !(MIN_RISCV_O3_WRITEBACK_WIDTH..=MAX_RISCV_O3_WRITEBACK_WIDTH).contains(&width) {
        return Err(Rem6CliError::InvalidRiscvO3WritebackWidth { value });
    }
    Ok(width)
}
```

Add `riscv_o3_writeback_width: Option<usize>` to both config structs, effective
and explicit accessors, value-taking file scan, parser arm, and final config
construction. Keep `config.rs` under the existing 1,700-line limit with these
specific consolidations in `config/riscv_timing.rs`:

```rust
pub(crate) fn validate_optional_riscv_o3_widths(
    issue: Option<usize>,
    writeback: Option<usize>,
) -> Result<(Option<usize>, Option<usize>), Rem6CliError> {
    Ok((
        validate_optional_riscv_o3_issue_width(issue)?,
        validate_optional_riscv_o3_writeback_width(writeback)?,
    ))
}

pub(crate) fn apply_riscv_o3_width_flag(
    flag: &str,
    value: &str,
    issue: &mut Option<usize>,
    writeback: &mut Option<usize>,
) -> Result<(), Rem6CliError> {
    match flag {
        "--riscv-o3-issue-width" => *issue = Some(parse_riscv_o3_issue_width(value)?),
        "--riscv-o3-writeback-width" => {
            *writeback = Some(parse_riscv_o3_writeback_width(value)?);
        }
        _ => unreachable!("width helper receives a validated flag"),
    }
    Ok(())
}
```

Use one tuple validation call for the file values and one combined CLI match
arm for both width flags. Reflow the adjacent `riscv_timing` imports without
adding import lines. These changes replace the existing issue-width-only
initialization/arm rather than appending a second copy; do not raise the source-
policy cap.

Add exact errors:

```rust
InvalidRiscvO3WritebackWidth { value: String },
RiscvO3WritebackWidthRequiresExecution,
RiscvO3WritebackWidthRequiresRiscv,
```

```text
invalid RISC-V O3 writeback width {value}
--riscv-o3-writeback-width requires --execute
--riscv-o3-writeback-width requires --isa riscv
```

Mirror issue-width gates in `run_validation.rs`, then configure each core:

```rust
core.set_o3_writeback_width(config.riscv_o3_writeback_width());
```

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-cpu o3_writeback_width --lib -- --nocapture
cargo test -p rem6 --test cli_run riscv_o3_writeback_width -- --nocapture
cargo test -p rem6 --test cli_run validation::rem6_run_config_scan_treats_riscv_o3_writeback_width_as_value_taking -- --exact --nocapture
cargo test -p rem6 --test source_policy cli_config_root_stays_focused -- --exact --nocapture
```

Expected: all selected tests pass and `config.rs` remains under policy.

```bash
git add crates/rem6-cpu/src/riscv_defaults.rs \
  crates/rem6-cpu/src/public_api.rs \
  crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6/src/config.rs \
  crates/rem6/src/config/riscv_timing.rs \
  crates/rem6/src/config/accessors.rs \
  crates/rem6/src/config/file_scan.rs \
  crates/rem6/src/cli_error.rs \
  crates/rem6/src/run_validation.rs \
  crates/rem6/src/riscv_core_runtime.rs \
  crates/rem6/tests/cli_run/validation.rs
git commit -m "cli: configure RISC-V O3 writeback width"
```

### Task 2: Add Generic Occupied-Slot Writeback Planning

**Files:**
- Modify: `crates/rem6-cpu/src/o3_pipeline.rs`
- Test: `crates/rem6-cpu/tests/o3_pipeline.rs`

- [ ] **Step 1: Write failing occupied-slot planner tests**

Add:

```rust
#[test]
fn o3_writeback_transfer_buffer_skips_occupied_slots() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);
    let cycle = buffer
        .plan_cycle_with_occupied_slots([0], [O3WritebackCompletion::new(7)])
        .unwrap();
    assert_eq!(cycle.admissions().len(), 1);
    assert_eq!(cycle.admissions()[0].completion().sequence(), 7);
    assert_eq!(cycle.admissions()[0].cycle_offset(), 0);
    assert_eq!(cycle.admissions()[0].slot(), 1);
    assert!(cycle.deferred().is_empty());
}

#[test]
fn o3_writeback_transfer_buffer_preserves_deferred_before_new_ready_with_occupancy() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);
    let first = buffer
        .plan_cycle_with_occupied_slots([0], [O3WritebackCompletion::new(1)])
        .unwrap();
    assert_eq!(first.deferred_sequences().collect::<Vec<_>>(), vec![1]);
    let second = buffer
        .plan_cycle_with_occupied_slots([], [O3WritebackCompletion::new(2)])
        .unwrap();
    assert_eq!(second.admitted_sequences().collect::<Vec<_>>(), vec![1]);
    assert_eq!(second.deferred_sequences().collect::<Vec<_>>(), vec![2]);
}

#[test]
fn o3_writeback_transfer_buffer_uses_future_policy_slots_after_occupied_current_cycle() {
    let policy = O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 1).unwrap();
    let mut buffer = O3WritebackTransferBuffer::new(policy);
    let cycle = buffer
        .plan_cycle_with_occupied_slots([0], [O3WritebackCompletion::new(7)])
        .unwrap();
    assert_eq!(cycle.admissions()[0].cycle_offset(), 1);
    assert_eq!(cycle.admissions()[0].slot(), 0);
    assert!(cycle.deferred().is_empty());
}
```

Also reject duplicate occupied slots and slots greater than or equal to the
configured width.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6-cpu --test o3_pipeline o3_writeback_transfer_buffer_ -- --nocapture
```

Expected: compile failure because `plan_cycle_with_occupied_slots` and occupied-
slot errors do not exist.

- [ ] **Step 3: Add the generic current-cycle planner**

Add `O3PipelineError` variants for duplicate and out-of-range occupied slots,
then implement:

```rust
pub fn plan_cycle_with_occupied_slots<I, O>(
    &mut self,
    occupied_slots: O,
    ready: I,
) -> Result<O3WritebackTransferCycle, O3PipelineError>
where
    I: IntoIterator<Item = O3WritebackCompletion>,
    O: IntoIterator<Item = usize>,
{
    let mut occupied = occupied_slots.into_iter().collect::<Vec<_>>();
    occupied.sort_unstable();
    for slots in occupied.windows(2) {
        if slots[0] == slots[1] {
            return Err(O3PipelineError::DuplicateWritebackOccupiedSlot {
                source: self.policy.source(),
                slot: slots[0],
            });
        }
    }
    if let Some(slot) = occupied
        .iter()
        .copied()
        .find(|slot| *slot >= self.policy.writeback_width())
    {
        return Err(O3PipelineError::WritebackOccupiedSlotOutOfRange {
            source: self.policy.source(),
            slot,
            writeback_width: self.policy.writeback_width(),
        });
    }

    let deferred_before_count = self.deferred.len();
    let new_ready = ready.into_iter().collect::<Vec<_>>();
    let new_ready_count = new_ready.len();
    let mut ordered = self.deferred.drain(..).collect::<Vec<_>>();
    ordered.extend(new_ready);

    let mut free_slots = Vec::with_capacity(self.policy.capacity_entries());
    for cycle_offset in 0..=self.policy.future_cycles() {
        for slot in 0..self.policy.writeback_width() {
            if cycle_offset == 0 && occupied.binary_search(&slot).is_ok() {
                continue;
            }
            free_slots.push((cycle_offset, slot));
        }
    }
    let admitted_count = ordered.len().min(free_slots.len());
    let admissions = ordered
        .iter()
        .take(admitted_count)
        .copied()
        .zip(free_slots)
        .map(|(completion, (cycle_offset, slot))| O3WritebackCompletionAdmission {
            completion,
            cycle_offset,
            slot,
        })
        .collect::<Vec<_>>();
    let deferred = ordered.into_iter().skip(admitted_count).collect::<Vec<_>>();
    self.deferred.extend(deferred.iter().copied());
    Ok(O3WritebackTransferCycle {
        new_ready_count,
        deferred_before_count,
        admissions,
        deferred,
    })
}
```

Replace the existing `plan_cycle` body with a call to
`plan_cycle_with_occupied_slots(std::iter::empty(), ready)` and an `expect`
whose message states that an empty occupied set is valid. This preserves the
policy's bounded future-window behavior exactly. The live runtime uses a policy
with `future_cycles == 0`, so its absolute-cycle loop exposes each deferred
cycle separately.

- [ ] **Step 4: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-cpu --test o3_pipeline o3_writeback_transfer -- --nocapture
```

Expected: all generic writeback tests pass, including existing future-offset
coverage.

```bash
git add crates/rem6-cpu/src/o3_pipeline.rs crates/rem6-cpu/tests/o3_pipeline.rs
git commit -m "cpu: plan occupied O3 writeback slots"
```

### Task 3: Reserve Fixed-FU Writeback Before Gate Scheduling

**Files:**
- Modify: `crates/rem6-cpu/src/error.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_checkpoint.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_writeback.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_stats.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_retire.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_trace.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`

- [ ] **Step 1: Write failing calendar, dependency, and CLI collision tests**

Declare the new runtime test module and add focused tests using synthetic
sequence/raw-ready tuples:

```rust
#[test]
fn writeback_width_one_reserves_oldest_same_cycle_row_first() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_writeback_width(1);
    let reservations = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(4, 20),
            O3LiveWritebackReady::fixed_fu(5, 20),
        ])
        .unwrap();
    assert_eq!(reservations[0].admitted_tick(), 20);
    assert_eq!(reservations[0].slot(), 0);
    assert_eq!(reservations[1].admitted_tick(), 21);
    assert_eq!(reservations[1].slot(), 0);
}

#[test]
fn writeback_width_two_admits_exact_fit_same_cycle_rows() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_writeback_width(2);
    let reservations = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(4, 20),
            O3LiveWritebackReady::fixed_fu(5, 20),
        ])
        .unwrap();
    assert_eq!(reservations[0].admitted_tick(), 20);
    assert_eq!(reservations[0].slot(), 0);
    assert_eq!(reservations[1].admitted_tick(), 20);
    assert_eq!(reservations[1].slot(), 1);
}

#[test]
fn writeback_planner_does_not_introduce_future_raw_ready_rows_early() {
    let mut runtime = O3RuntimeState::default();
    let reservations = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(1, 10),
            O3LiveWritebackReady::fixed_fu(2, 30),
        ])
        .unwrap();
    assert_eq!(reservations[0].admitted_tick(), 10);
    assert_eq!(reservations[1].admitted_tick(), 30);
    assert_eq!(runtime.stats().writeback_port_deferred_rows(), 0);
    assert_eq!(runtime.stats().writeback_port_max_ready_rows_per_cycle(), 1);
}

#[test]
fn writeback_reentry_returns_identical_reservation_without_recounting() {
    let mut runtime = O3RuntimeState::default();
    let first = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(9, 12)])
        .unwrap()[0];
    let stats = runtime.stats();
    let second = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(9, 12)])
        .unwrap()[0];
    assert_eq!(first, second);
    assert_eq!(runtime.stats(), stats);
}

#[test]
fn partial_reentry_cannot_overbook_or_recount_writeback() {
    let mut runtime = O3RuntimeState::default();
    let first = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(4, 20),
            O3LiveWritebackReady::fixed_fu(5, 20),
        ])
        .unwrap();
    let stats = runtime.stats();
    let reentered = runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(5, 20),
            O3LiveWritebackReady::fixed_fu(4, 20),
        ])
        .unwrap();
    assert_eq!(reentered, first);
    assert_eq!(runtime.stats(), stats);
    assert_eq!(runtime.writeback_calendar.occupied_slots(20), vec![0]);
    assert_eq!(runtime.writeback_calendar.occupied_slots(21), vec![0]);
}

#[test]
fn reentry_rejects_changed_raw_ready_tick() {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 20)])
        .unwrap();
    assert!(matches!(
        runtime.reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 21)]),
        Err(O3RuntimeError::WritebackReservationMismatch {
            sequence: 4,
            existing_raw_ready_tick: 20,
            requested_raw_ready_tick: 21,
        })
    ));
}

#[test]
fn writeback_width_change_is_rejected_while_reservations_are_live() {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 20)])
        .unwrap();
    assert!(!runtime.set_writeback_width(2));
    assert_eq!(runtime.writeback_width(), 1);
}
```

Add `scoped_issue_waits_for_admitted_writeback_tick` to the issue tests: with
width one, a zero-latency dependent candidate must use the producer's admitted
tick and its own completion must move to the next slot. Add
`live_retire_gate_arms_fixed_fu_admitted_tick` in
`riscv_live_retire_window.rs`: construct a width-one collision where the head's
raw tick is occupied, call the serial and parallel live-window entry points,
and assert `pending_ready_tick()` equals the reservation's admitted tick before
either path schedules a wake.

Create `writeback_port.rs`, register it from `o3.rs`, and add a real MUL+
dependent ADDI fixture:

```rust
fn writeback_fu_collision_binary(name: &str) -> PathBuf {
    let mut words = vec![
        i_type(6, 0, 0x0, 1, 0x13),
        i_type(7, 0, 0x0, 2, 0x13),
        m5op(M5_SWITCH_CPU),
        r_type(1, 2, 1, 0x0, 3, 0x33),
        i_type(1, 3, 0x0, 4, 0x13),
    ];
    append_host_stop(&mut words);
    temp_binary(name, &riscv64_elf(
        0x8000_0000,
        0x8000_0000,
        &riscv64_program(&words),
    ))
}
```

Define the focused command and trace helpers in the new module rather than
reaching into the private sibling module. Use a route delay of one tick so the
dependent fetch is ready at the two-cycle integer MUL completion; larger route
delays test front-end arrival instead of writeback-port contention:

```rust
fn writeback_json(path: &Path, memory_system: &str, width: usize, max_tick: u64) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            &max_tick.to_string(),
            "--stats-format",
            "json",
            "--execute",
            "--debug-flags",
            "O3,Data,Fetch,Memory,HostAction",
            "--riscv-o3-issue-width",
            "4",
            "--riscv-o3-writeback-width",
            &width.to_string(),
            "--memory-system",
            memory_system,
            "--memory-route-delay",
            "1",
            "--m5-switch-cpu-mode",
            "detailed",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn o3_trace_events(json: &Value) -> &[Value] {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("O3 trace should expose events: {json}"))
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    o3_trace_events(json)
        .iter()
        .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}
```

Add exact tests:

```rust
#[test]
fn rem6_run_o3_writeback_width_one_serializes_direct_fu_dependent_collision() {
    let json = writeback_json(&writeback_fu_collision_binary("wb-width-one"), "direct", 1, 600);
    let multiply = event_at_pc(&json, "0x8000000c");
    let dependent = event_at_pc(&json, "0x80000010");
    assert_eq!(event_u64(dependent, "issue_tick"), event_u64(multiply, "writeback_tick"));
    assert_eq!(event_u64(dependent, "writeback_tick"), event_u64(multiply, "writeback_tick") + 1);
    assert_eq!(json.pointer("/cores/0/registers/x3").and_then(Value::as_str), Some("0x2a"));
    assert_eq!(json.pointer("/cores/0/registers/x4").and_then(Value::as_str), Some("0x2b"));
}

#[test]
fn rem6_run_o3_writeback_width_two_exact_fit_direct_fu_dependent_collision() {
    let json = writeback_json(&writeback_fu_collision_binary("wb-width-two"), "direct", 2, 600);
    let multiply = event_at_pc(&json, "0x8000000c");
    let dependent = event_at_pc(&json, "0x80000010");
    assert_eq!(event_u64(dependent, "issue_tick"), event_u64(multiply, "writeback_tick"));
    assert_eq!(event_u64(dependent, "writeback_tick"), event_u64(multiply, "writeback_tick"));
}
```

Use `--riscv-o3-issue-width 4`, `--riscv-o3-writeback-width <width>`, detailed
mode, O3 debug, and direct memory in `writeback_json`.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6-cpu o3_runtime_writeback --lib -- --nocapture
cargo test -p rem6-cpu scoped_issue_waits_for_admitted_writeback_tick --lib -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_width_ -- --nocapture
```

Expected: compile/test failures because reservation types, counters, explicit
trace ticks, and runtime ownership do not exist.

- [ ] **Step 3: Add the focused reservation calendar and counters**

Declare `o3_runtime_writeback` and its test module. Add these focused types:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3WritebackReservation {
    sequence: u64,
    raw_ready_tick: u64,
    admitted_tick: u64,
    slot: usize,
    decision_counted: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct O3WritebackReservationCalendar {
    by_tick: BTreeMap<u64, Vec<O3WritebackReservation>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3LiveWritebackReady {
    sequence: u64,
    raw_ready_tick: u64,
}
```

Give `O3WritebackReservation` value accessors and a private constructor that
sets `decision_counted = true` at the same successful commit point where the
new reservation's counters are recorded. Add explicit runtime errors:

```rust
impl O3WritebackReservation {
    fn new_counted(
        sequence: u64,
        raw_ready_tick: u64,
        admitted_tick: u64,
        slot: usize,
    ) -> Self {
        Self {
            sequence,
            raw_ready_tick,
            admitted_tick,
            slot,
            decision_counted: true,
        }
    }

    pub(super) const fn sequence(&self) -> u64 { self.sequence }
    pub(super) const fn raw_ready_tick(&self) -> u64 { self.raw_ready_tick }
    pub(super) const fn admitted_tick(&self) -> u64 { self.admitted_tick }
    pub(super) const fn slot(&self) -> usize { self.slot }
    pub(super) const fn decision_counted(&self) -> bool { self.decision_counted }
}

DuplicateWritebackReadySequence { sequence: u64 },
WritebackReservationMismatch {
    sequence: u64,
    existing_raw_ready_tick: u64,
    requested_raw_ready_tick: u64,
},
WritebackCalendarSlotOccupied { tick: u64, slot: usize },
StableWritebackQueueNotEmpty { deferred: usize },
WritebackTickOverflow { tick: u64 },
```

Add `RiscvCpuError::O3Runtime(O3RuntimeError)` with display/source handling.
Change fixed-FU and load reservation call chains to return
`Result<_, O3RuntimeError>` internally and map with
`RiscvCpuError::O3Runtime` at the live-window/data-response boundary. Do not
convert an overflow or invariant mismatch into `false`, saturation, or an
unbounded wait.

The calendar must provide `reservation(sequence)`, `occupied_slots(tick)`,
`insert`, `remove_sequence`, `remove_future_from_sequence(sequence, now)`, `clear`,
`prune_before(tick)`, `reserved_future_count(now)`, and
`earliest_unpublished_tick(now)`.
`prune_before` removes only entries with `admitted_tick < tick`;
`remove_future_from_sequence` removes matching/suffix entries only when
`admitted_tick > now`.
Add runtime accessors `writeback_reservation(sequence) ->
Option<O3WritebackReservation>` and `writeback_reservations() ->
Vec<O3WritebackReservation>` sorted by sequence for focused tests and debug
projection; callers cannot mutate the calendar through them.

Add six fields/accessors to `O3RuntimeStats`:

```rust
writeback_port_cycles: u64,
writeback_port_admitted_rows: u64,
writeback_port_deferred_rows: u64,
writeback_port_deferred_row_cycles: u64,
writeback_port_max_ready_rows_per_cycle: u64,
writeback_port_max_deferred_rows: u64,
```

Add `live_writeback_cycle_ticks: BTreeSet<u64>` and
`live_writeback_ready_rows_by_tick: BTreeMap<u64, BTreeSet<u64>>` to
`O3RuntimeState`, initialize them and the calendar empty in `Default`, and clear
them on restore/full reset. Clear only these auxiliary sets on stats reset; preserve
calendar reservations and each reservation's `decision_counted` marker.
Extend `set_writeback_width` to return `false` while the calendar or live
writeback owner records are nonempty; CLI construction still configures width
before execution begins.

Implement the atomic planner:

```rust
pub(super) fn reserve_writeback_completions<I>(
    &mut self,
    ready: I,
) -> Result<Vec<O3WritebackReservation>, O3RuntimeError>
where
    I: IntoIterator<Item = O3LiveWritebackReady>,
{
    let mut pending = ready.into_iter().collect::<Vec<_>>();
    pending.sort_by_key(|row| (row.raw_ready_tick, row.sequence));
    let mut raw_ready_by_sequence = BTreeMap::new();
    for row in &pending {
        if raw_ready_by_sequence
            .insert(row.sequence, row.raw_ready_tick)
            .is_some()
        {
            return Err(O3RuntimeError::DuplicateWritebackReadySequence {
                sequence: row.sequence,
            });
        }
    }

    let mut results = Vec::with_capacity(pending.len());
    let mut new_rows = Vec::with_capacity(pending.len());
    for row in pending {
        if let Some(existing) = self.writeback_calendar.reservation(row.sequence) {
            if existing.raw_ready_tick() != row.raw_ready_tick {
                return Err(O3RuntimeError::WritebackReservationMismatch {
                    sequence: row.sequence,
                    existing_raw_ready_tick: existing.raw_ready_tick(),
                    requested_raw_ready_tick: row.raw_ready_tick,
                });
            }
            results.push(existing);
        } else {
            new_rows.push(row);
        }
    }
    let mut pending = new_rows;
    if pending.is_empty() {
        results.sort_by_key(O3WritebackReservation::sequence);
        return Ok(results);
    }

    let mut buffer = O3WritebackTransferBuffer::from_snapshot(
        self.snapshot.pending_state().writeback().clone(),
    )
    .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
    if !buffer.is_empty() {
        return Err(O3RuntimeError::StableWritebackQueueNotEmpty {
            deferred: buffer.pending_deferred_count(),
        });
    }
    let mut staged_calendar = self.writeback_calendar.clone();
    let mut staged_plans = Vec::new();
    let mut base_tick = pending[0].raw_ready_tick;
    while !pending.is_empty() || !buffer.is_empty() {
        let eligible_count = pending.partition_point(|row| row.raw_ready_tick <= base_tick);
        let eligible = pending.drain(..eligible_count).collect::<Vec<_>>();
        let cycle = buffer.plan_cycle_with_occupied_slots(
            staged_calendar.occupied_slots(base_tick),
            eligible.iter().map(|row| O3WritebackCompletion::new(row.sequence)),
        )
        .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
        for admission in cycle.admissions() {
            let sequence = admission.completion().sequence();
            let raw_ready_tick = raw_ready_by_sequence
                .get(&sequence)
                .copied()
                .expect("planned writeback row retains raw-ready timing");
            let reservation = O3WritebackReservation::new_counted(
                sequence,
                raw_ready_tick,
                base_tick,
                admission.slot(),
            );
            staged_calendar.insert(reservation)?;
            results.push(reservation);
        }
        staged_plans.push((base_tick, eligible, cycle));
        if buffer.is_empty() && !pending.is_empty() {
            base_tick = pending[0].raw_ready_tick;
        } else if !buffer.is_empty() {
            base_tick = base_tick.checked_add(1).ok_or(O3RuntimeError::WritebackTickOverflow {
                tick: base_tick,
            })?;
        }
    }
    self.replace_writeback_snapshot(buffer.snapshot())?;
    self.writeback_calendar = staged_calendar;
    for (tick, eligible, cycle) in staged_plans {
        self.record_writeback_plan(tick, &eligible, &cycle);
    }
    results.sort_by_key(O3WritebackReservation::sequence);
    Ok(results)
}
```

Add `replace_writeback_snapshot` as a build-then-assign helper so a pending-state
validation failure cannot partially mutate the runtime:

```rust
fn replace_writeback_snapshot(
    &mut self,
    writeback: O3WritebackTransferSnapshot,
) -> Result<(), O3RuntimeError> {
    let pending = self.snapshot.pending_state();
    let replacement = O3PendingStateSnapshot::new(
        pending.resolved_dependency_scopes().iter().copied(),
        pending.ready().iter().cloned(),
        writeback,
    )
    .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
    self.snapshot.pending_state = replacement;
    Ok(())
}
```

The staging calendar, drained buffer, and plan observations commit only after
every cycle succeeds. Future-raw-ready rows remain outside the buffer, and an
existing reservation with different raw timing is an invariant error.

Implement `record_writeback_plan` from plan facts only:

```rust
fn record_writeback_plan(
    &mut self,
    tick: u64,
    newly_ready: &[O3LiveWritebackReady],
    cycle: &O3WritebackTransferCycle,
) {
    if self.live_writeback_cycle_ticks.insert(tick) {
        self.stats.writeback_port_cycles += 1;
    }
    for row in newly_ready {
        let ready = self
            .live_writeback_ready_rows_by_tick
            .entry(row.raw_ready_tick)
            .or_default();
        ready.insert(row.sequence);
        self.stats.writeback_port_max_ready_rows_per_cycle = self
            .stats
            .writeback_port_max_ready_rows_per_cycle
            .max(ready.len() as u64);
    }
    self.stats.writeback_port_max_deferred_rows = self
        .stats
        .writeback_port_max_deferred_rows
        .max(cycle.deferred().len() as u64);
    for admission in cycle.admissions() {
        let reservation = self
            .writeback_calendar
            .reservation(admission.completion().sequence())
            .expect("new admission has a committed reservation");
        assert!(reservation.decision_counted());
        self.stats.writeback_port_admitted_rows += 1;
        if reservation.admitted_tick() > reservation.raw_ready_tick() {
            self.stats.writeback_port_deferred_rows += 1;
            self.stats.writeback_port_deferred_row_cycles +=
                reservation.admitted_tick() - reservation.raw_ready_tick();
        }
    }
}
```

Use checked counter increments if the surrounding stats code already treats
overflow as an invariant; do not derive these fields from retired instruction
counts or compatibility aliases. Re-entry produces no staged plans, and stats
reset preserves counted reservations, so old reservations are not counted in
the new epoch.

- [ ] **Step 4: Thread admitted ticks through fixed-FU issue and trace**

Add `raw_ready_tick: u64`, `admitted_writeback_tick: u64`, and
`writeback_slot: usize` to `O3LiveSpeculativeExecution`. Add
`admitted_writeback_tick: u64` to `O3LiveRetiredInstruction`.
Update every constructor and conversion explicitly; do not derive admitted
timing again when moving a row from speculative to retired ownership.

When `record_live_issue_head_execution` or
`record_live_speculative_execution` creates a fixed scalar register-writing
row, compute raw readiness from issue plus FU latency, reserve it immediately,
and store the returned tick/slot. Destinationless branch rows do not consume a
writeback slot.

Change source forwarding to use `issued.admitted_writeback_tick` instead of
`issue_tick + max(latency, 1)`. Split live-window preparation so the order is:

```rust
let prepared = prepare_o3_live_retire_window(
    state,
    window.request,
    window.pc,
    window.raw,
    now,
    raw_ready_tick,
    window.fetch_events,
)?;
let decision = state.live_retire_gate.before_retire_at_known_ready_tick(
    window.request,
    now,
    prepared.head_admitted_writeback_tick,
);
```

Do not schedule a raw wake first. On actual retirement, mark the ROB row ready
with the stored admitted tick and carry that tick through `O3LiveRetiredInstruction`.
Make `record_live_issue_head_execution` and
`record_live_speculative_execution` return `Result<bool, O3RuntimeError>` so
calendar failures reach `stage_o3_live_retire_window` as
`RiscvCpuError::O3Runtime`.
Update direct unit callers in `o3_runtime_control_window_tests.rs`,
`o3_runtime_issue_tests.rs`, `o3_runtime_live_window.rs`, and
`o3_runtime_memory_window.rs` to unwrap fixture success; internal callers in
`o3_runtime_control_window.rs` and `o3_runtime_issue.rs` propagate with `?`.

Add an optional field to `O3RuntimeTraceRecord`:

```rust
admitted_writeback_tick: Option<u64>,

pub(crate) fn set_admitted_writeback_tick(&mut self, tick: u64) {
    self.admitted_writeback_tick = Some(tick);
    if self.current_instruction_committed() {
        self.commit_tick = self.commit_tick.max(tick);
    }
}

pub fn writeback_tick(self) -> u64 {
    self.admitted_writeback_tick.unwrap_or_else(|| {
        self.tick
            .saturating_add(self.fu_latency_cycles)
            .max(self.tick.saturating_add(self.lsq_data_latency_ticks))
            .max(self.lsq_data_response_tick)
    })
}
```

Initialize the field to `None` in every trace constructor and builder path; it
is transient trace data and does not change checkpoint codecs.
Set it only for live FU/load rows; legacy trace construction keeps the existing
derived behavior. FU latency counters continue to use execution latency only.

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-cpu o3_runtime_writeback --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_live_window --lib -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_width_ -- --nocapture
```

Expected: width one delays only the dependent completion, width two is exact
fit, dependency issue consumes the admitted producer tick, and trace/ROB timing
uses the explicit admitted tick.

```bash
git add crates/rem6-cpu/src/error.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_writeback.rs \
  crates/rem6-cpu/src/o3_runtime_writeback_tests.rs \
  crates/rem6-cpu/src/o3_runtime_stats.rs \
  crates/rem6-cpu/src/o3_runtime_control_window.rs \
  crates/rem6-cpu/src/o3_runtime_control_window_tests.rs \
  crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_live_window.rs \
  crates/rem6-cpu/src/o3_runtime_memory_window.rs \
  crates/rem6-cpu/src/o3_runtime_retire.rs \
  crates/rem6-cpu/src/o3_runtime_trace.rs \
  crates/rem6-cpu/src/riscv_live_retire_window.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs
git commit -m "cpu: reserve live O3 writeback slots"
```

### Task 4: Gate Scalar-Load Publication With a Scheduler Wake

**Files:**
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_drive.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_handoff.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_memory_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_retire.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_source_operands.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/forwarding.rs`
- Create: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/forwarding.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/multi_load.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/store_store_load.rs`
- Create: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`
- Test: `crates/rem6-cpu/tests/riscv_frontend.rs`
- Test: `crates/rem6-cpu/tests/riscv_cluster_data.rs`
- Modify: `crates/rem6-system/src/lib.rs`
- Modify: `crates/rem6-system/src/riscv_run_stats.rs`
- Test: `crates/rem6-system/src/riscv_o3_runtime_stats.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`

- [ ] **Step 1: Write failing load-publication and wake tests**

Add CPU tests:

```rust
#[test]
fn completed_scalar_load_reserves_writeback_before_marking_rob_ready() {
    let mut runtime = completed_live_load_runtime(41);
    let live = &runtime.live_scalar_memories[0];
    assert_eq!(live.raw_ready_tick, Some(42));
    assert_eq!(live.admitted_writeback_tick, Some(42));
    assert!(!runtime.snapshot().reorder_buffer()[0].is_ready());
    assert!(runtime.snapshot().load_store_queue()[0].is_completed());
}

#[test]
fn scalar_load_publication_waits_until_admitted_tick() {
    let mut runtime = completed_live_load_runtime(41);
    assert!(runtime.take_ready_live_scalar_memory_event(41).is_none());
    assert!(!runtime.snapshot().reorder_buffer()[0].is_ready());
    assert!(runtime.take_ready_live_scalar_memory_event(42).is_some());
    assert!(runtime.snapshot().reorder_buffer()[0].is_ready());
    assert_eq!(runtime.snapshot().reorder_buffer()[0].ready_tick(), 42);
}

#[test]
fn late_scalar_load_does_not_displace_fixed_fu_reservation() {
    let mut runtime = O3RuntimeState::default();
    let fixed = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(8, 42)])
        .unwrap()[0];
    let load = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::scalar_load(4, 42)])
        .unwrap()[0];
    assert_eq!(fixed.admitted_tick(), 42);
    assert_eq!(runtime.writeback_reservation(8), Some(fixed));
    assert_eq!(load.admitted_tick(), 43);
}

#[test]
fn scalar_load_reservation_failure_does_not_partially_commit_response() {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
    let sequence = runtime.live_scalar_memories[0].sequence;
    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(sequence, 40)])
        .unwrap();
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(matches!(
        runtime.complete_live_scalar_memory_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        ),
        Err(O3RuntimeError::WritebackReservationMismatch { .. })
    ));
    let live = &runtime.live_scalar_memories[0];
    assert_eq!(live.outcome, O3LiveScalarMemoryOutcome::Resident);
    assert_eq!(live.response_tick, None);
    assert_eq!(live.raw_ready_tick, None);
    assert_eq!(live.admitted_writeback_tick, None);
    assert!(!runtime.snapshot().load_store_queue()[0].is_completed());
    assert!(!runtime.snapshot().reorder_buffer()[0].is_ready());
}
```

Define the CPU fixture in the existing `o3_runtime_memory.rs` test module:

```rust
fn completed_live_load_runtime(response_tick: u64) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    let execution = scalar_load_event(0x8000, 10);
    let data_request = memory_request(20);
    assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
    let mut completed = execution.clone();
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_scalar_memory_response(
            &completed,
            data_request,
            response_tick,
            response_tick - 31,
            Some(&[0x2a, 0, 0, 0]),
        )
        .unwrap());
    runtime
}
```

Add wake-state tests proving identical requests deduplicate, an earlier desired
tick detaches a later scheduled wake, fired wakes clear scheduled ownership, and
detached wakes prune only after a later tick.

Add `data_response_writeback_error_is_sticky_and_surfaces_from_drive` in
`riscv_data_issue_tests.rs`. Build the existing detailed scalar-load fixture,
inject a conflicting reservation for its live sequence, deliver the completed
transport response, and assert that no response data/event, LSQ completion,
ROB readiness, wake, or owner timing was committed. Assert repeated direct
`drive_next_action` calls return the same `RiscvCpuError::O3Runtime`. Add a
cluster variant that proves both serial and parallel `drive_turn` return
`RiscvClusterError::Core` after the scheduler callback records the error.
Add `mmio_response_writeback_error_is_sticky_without_partial_state` to force
the same reservation mismatch through a completed MMIO load and prove the
request, resident event, architectural register, LSQ, and ROB remain unchanged.
Use a `#[cfg(test)] pub(crate)` core helper implemented in
`o3_runtime_writeback.rs` that accepts only `(sequence, raw_ready_tick)` and
reserves one synthetic fixed-FU row under the core lock; do not widen the
production visibility of the calendar, ready-row type, or reservation method.

In `riscv_o3_runtime_stats.rs`, add a real completed-load fixture using the
existing local helpers:

```rust
fn completed_load_waiting_for_writeback() -> (
    RiscvSystemRunDriver,
    RiscvCluster,
    PartitionedScheduler,
    RiscvClusterTurn,
    u64,
) {
    let cpu = CpuId::new(0);
    let (core, cluster, mut scheduler, transport) = scalar_memory_core(cpu);
    let driver = detailed_o3_driver_with_stats(cpu);
    core.write_register(Register::new(2).unwrap(), 0x9000);
    issue_fetch_instruction(
        &core,
        &mut scheduler,
        &transport,
        load_word_instruction(0, 2, 12),
    );
    let execution = core.execute_next_completed_fetch().unwrap().unwrap();
    driver
        .record_run_stats(
            &cluster,
            scheduler.now(),
            &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                cpu,
                RiscvCoreDriveAction::InstructionExecuted(Box::new(execution)),
            )]),
        )
        .unwrap();
    let issued = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |delivery, _context| {
                TargetOutcome::Respond(
                    MemoryResponse::completed(
                        delivery.request(),
                        Some(vec![0x2a, 0, 0, 0]),
                    )
                    .unwrap(),
                )
            },
        )
        .unwrap()
        .unwrap();
    driver
        .record_run_stats(
            &cluster,
            scheduler.now(),
            &RiscvClusterTurn::core(vec![RiscvClusterDriveEvent::new(
                cpu,
                RiscvCoreDriveAction::DataAccessIssued { event: issued },
            )]),
        )
        .unwrap();
    let turn = RiscvClusterTurn::scheduler(scheduler.run_until_idle());
    driver
        .record_run_stats(&cluster, scheduler.now(), &turn)
        .unwrap();
    let wake_tick = core
        .requested_o3_writeback_wake_tick(scheduler.now())
        .unwrap();
    (driver, cluster, scheduler, turn, wake_tick)
}
```

Add serial and parallel system tests:

```rust
#[test]
fn schedule_riscv_system_events_from_turn_schedules_o3_writeback_wake() {
    let (driver, cluster, mut scheduler, turn, wake_tick) =
        completed_load_waiting_for_writeback();
    let events = driver
        .schedule_riscv_system_events_from_turn(&cluster, &mut scheduler, &turn, |_| GuestEventId::new(1))
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(scheduler.pending_event_snapshot(events[0]).unwrap().tick(), wake_tick);
}

#[test]
fn schedule_riscv_system_events_from_turn_parallel_schedules_o3_writeback_wake() {
    let (driver, cluster, mut scheduler, turn, wake_tick) =
        completed_load_waiting_for_writeback();
    let events = driver
        .schedule_riscv_system_events_from_turn_parallel(&cluster, &mut scheduler, &turn, |_| GuestEventId::new(1))
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(scheduler.pending_event_snapshot(events[0]).unwrap().tick(), wake_tick);
}
```

For each adapter, call it a second time before firing and assert it returns no
additional wake, run the scheduler through `wake_tick`, feed the scheduler turn
to `record_run_stats`, and assert the destination register and ROB become ready
exactly then. The wake-state unit tests cover replacement and detached-wake
pruning independently.

Add the direct cross-class CLI row. Run a deterministic route-delay table
`[4, 8, 9, 12, 16, 20]` at width two, select the first run where
`load.lsq_data_response_tick + 1 == fu.issue_tick + 19`, assert exactly one
match, then rerun that route delay at width one. The first younger DIV issues
with the load, and direct memory makes the load raw-ready at
`issue + 2 * route_delay + 1`, so delay nine is the measured collision point.
This keeps calibration inside the test instead of hard-coding a host-dependent
tick. Assert:

```rust
assert_eq!(load_raw_ready, fu_raw_ready);
assert_eq!(event_u64(load, "writeback_tick"), load_raw_ready + 1);
assert_eq!(event_u64(fu, "writeback_tick"), fu_raw_ready);
assert!(event_u64(dependent, "issue_tick") >= event_u64(load, "writeback_tick"));
```

For `rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission`, first run to completion to read the admitted load tick, then rerun
with `--max-tick admitted_tick - 1` and assert the load destination register
retains its pre-load value and the ROB row is not ready. Rerun at
`admitted_tick` and assert the architectural value appears exactly once.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6-cpu scalar_load_publication_waits_until_admitted_tick --lib -- --nocapture
cargo test -p rem6-cpu writeback_error_is_sticky --lib -- --nocapture
cargo test -p rem6-cpu o3_writeback_wake --lib -- --nocapture
cargo test -p rem6-system schedule_riscv_system_events_from_turn_schedules_o3_writeback_wake -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission -- --nocapture
```

Expected: failures because loads still mark ROB ready at response time, scalar
publication has no tick argument, callback failures have no sticky drive-visible
channel, and no writeback wake exists.

- [ ] **Step 3: Reserve load writeback without publishing it**

Extend `O3LiveScalarMemory`:

```rust
pub(super) raw_ready_tick: Option<u64>,
pub(super) admitted_writeback_tick: Option<u64>,
pub(super) writeback_slot: Option<usize>,
```

Initialize all three fields to `None` in `stage_live_scalar_memory_issue`.

On a completed register-writing scalar load, identify the live/ROB/LSQ indices
and derive response data in locals without mutating runtime state. Compute
`raw_ready_tick = response_tick.checked_add(1).ok_or(
O3RuntimeError::WritebackTickOverflow { tick: response_tick })?`, then reserve
through the shared runtime owner. Only after reservation succeeds, commit the
response, raw/admitted tick and slot, completed outcome, load data/forwarding
plan, and LSQ completion together. Change
`complete_live_scalar_memory{,_response,_forwarding}` to return
`Result<bool, O3RuntimeError>` and map the error through
`RiscvCpuError::O3Runtime` in `riscv_data_issue.rs`. Do not call
`mark_ready_at` from `complete_live_scalar_memory`.
Keep `load_store_queue[lsq_index].mark_completed()` at memory-response time as
part of that post-reservation commit; only
ROB readiness, dependent availability, trace writeback, and architectural
publication move to the admitted tick.
Update every direct completion call in `o3_runtime_handoff.rs`,
`o3_runtime_live_window.rs`, `o3_runtime_memory.rs`, and
`o3_runtime_memory_window.rs` to unwrap the expected successful fixture result.

Keep `has_ready_live_scalar_memory_event()` as the existing no-argument
lifecycle predicate: a completed-but-unpublished load remains ready work and
continues to block ordinary execution/fetch-ahead from running past it. Change
only publication to accept the current tick:

```rust
pub(crate) fn take_ready_live_scalar_memory_event(
    &mut self,
    current_tick: u64,
) -> Option<RiscvCpuExecutionEvent> {
    let live = self.live_scalar_memories.first_mut()?;
    if live.outcome == O3LiveScalarMemoryOutcome::Resident || live.event_taken {
        return None;
    }
    if live.outcome == O3LiveScalarMemoryOutcome::Completed {
        let response_tick = live.response_tick?;
        let publication_tick = live.admitted_writeback_tick.unwrap_or(response_tick);
        if publication_tick > current_tick {
            return None;
        }
        let rob = self
            .snapshot
            .reorder_buffer
            .iter_mut()
            .find(|entry| entry.sequence() == live.sequence)?;
        rob.mark_ready_at(publication_tick);
        live.commit_tick = Some(publication_tick.max(
            self.last_scalar_memory_commit_tick
                .unwrap_or(publication_tick),
        ));
    }
    live.event_taken = true;
    Some(live.execution.clone())
}
```

The `None` admitted-tick case preserves response-time publication for
destinationless stores and excluded atomic/MMIO classes; completed cacheable
scalar loads always carry `Some(admitted_tick)`. Retry and failure events remain
immediately consumable and do not mark a removed ROB row ready.
`completed_live_scalar_load_source` must return the stored admitted tick and
require the load ROB row to be ready.

Change `RiscvCore::record_ready_o3_scalar_memory_event_with_trace` to
`(current_tick: u64, trace_enabled: bool)`, and pass `tick` from
`riscv_run_stats.rs`. Apply deferred load
architectural writeback only after `take_ready_live_scalar_memory_event`
succeeds.

Admit the existing scalar integer multiply/divide fixed-FU classes as
speculative younger rows in a scalar-load window. Preserve the existing source
dependency rule: an independent DIV may issue while the load is resident, but
a DIV that reads the unresolved load destination remains blocked/terminal.
Update every direct unit-test caller in `o3_runtime_memory.rs`,
`o3_runtime_memory_window.rs`, `riscv_data_issue_tests.rs`, and its
`forwarding`, `multi_load`, and `store_store_load` children. Existing tests that
are not timing assertions pass the fixture's response/admitted tick explicitly;
only the focused before/at-admission tests probe both sides of the gate.

- [ ] **Step 4: Surface asynchronous callback failures without partial state**

Add a nonserialized sticky field to `RiscvCoreState` and initialize it to
`None`:

```rust
pending_callback_error: Option<RiscvCpuError>,
```

Add helpers that store only the first callback failure and return a clone
without consuming it. Task 4 leaves the field sticky through normal drive,
checkpoint, and mode transfer and includes `pending_callback_error.is_none()`
in data-access quiescence. Task 5 wires explicit hart reset and successful
checkpoint restore to clear it alongside the rest of transient O3 authority.
After locking core state, every data/MMIO/local completion callback returns
without further mutation when the sticky field is already populated, so a
second event in the same scheduler epoch cannot alter the failed state.

For a deferred O3 completed response, clone the matching execution event and
set `Completed` on the clone without mutating `state.events`. Make
`record_o3_data_access_outcome` return `Result<bool, O3RuntimeError>` and call
the atomic runtime completion with that clone. On `Err(error)`, store
`RiscvCpuError::O3Runtime(error)` in `pending_callback_error` and return from the
transport callback before changing the resident event, buffered-store state,
data events, wake state, checker state, or architectural registers. On success,
commit the resident event kind, younger-request cleanup, wake recomputation,
and `RiscvDataAccessEvent::completed` in the existing order.
Handle the new result at every `record_o3_data_access_outcome` call in
`riscv_data_issue.rs`, including retry/failure, local store-conditional, and
MMIO callbacks; excluded classes should return `Ok`, but callback code must not
silently unwrap a future invariant failure.

Add `RiscvCore::pending_callback_error() -> Option<RiscvCpuError>`. Check it at
the start of `RiscvCore::drive_next_action`. Add one deterministic CPU-order
helper in `RiscvCluster` that maps the first pending error to
`RiscvClusterError::Core`; call it at the start of every serial/parallel
`drive_ready_cores*` entry point and immediately after every serial/parallel
scheduler dispatch in `drive_turn*`, before returning a scheduler turn. The
error remains sticky, so a host stop or idle observation cannot hide it. Keep
`riscv_cluster.rs` at or below the existing 1,800-line source-policy cap by
implementing one shared CPU-order helper and using one-line calls rather than
duplicating the scan in each adapter.

- [ ] **Step 5: Add the core-owned wake bridge**

Declare `riscv_o3_writeback_wake` in `lib.rs`, add
`o3_writeback_wake: RiscvO3WritebackWakeState` to `RiscvCoreState`, and initialize
it by default.

Implement the focused state using the same scheduler snapshot types as the live
retire gate:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RiscvO3WritebackWake {
    scheduler: SchedulerInstanceId,
    event: PendingEventSnapshot,
}

impl RiscvO3WritebackWake {
    pub(crate) const fn new(
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) -> Self {
        Self { scheduler, event }
    }

    pub(crate) const fn tick(self) -> Tick {
        self.event.tick()
    }

    pub(crate) const fn scheduler(self) -> SchedulerInstanceId {
        self.scheduler
    }

    pub(crate) const fn event(self) -> PendingEventSnapshot {
        self.event
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RiscvO3WritebackWakeState {
    desired_tick: Option<Tick>,
    scheduled: Option<RiscvO3WritebackWake>,
    detached: Vec<RiscvO3WritebackWake>,
}

impl RiscvO3WritebackWakeState {
    pub(crate) fn set_desired_tick(&mut self, desired: Option<Tick>, now: Tick) {
        self.prune(now);
        if self.desired_tick == desired {
            return;
        }
        if let Some(wake) = self.scheduled.take() {
            if !self.detached.contains(&wake) {
                self.detached.push(wake);
            }
        }
        self.desired_tick = desired;
    }

    pub(crate) fn requested_tick(&mut self, now: Tick) -> Option<Tick> {
        self.prune(now);
        if self.scheduled.is_some() {
            return None;
        }
        self.desired_tick.filter(|tick| *tick > now)
    }

    pub(crate) fn mark_scheduled(
        &mut self,
        scheduler: SchedulerInstanceId,
        event: PendingEventSnapshot,
    ) {
        let wake = RiscvO3WritebackWake::new(scheduler, event);
        if self.desired_tick == Some(wake.tick()) && self.scheduled.is_none() {
            self.scheduled = Some(wake);
        }
    }

    pub(crate) fn mark_fired(&mut self, now: Tick) {
        if self.scheduled.map_or(false, |wake| wake.tick() <= now) {
            self.scheduled = None;
        }
        self.prune(now);
    }

    fn prune(&mut self, now: Tick) {
        self.detached.retain(|wake| wake.tick() >= now);
    }
}
```

After every load completion, retry/failure, rollback, redirect, reset, or mode
cleanup, recompute desired tick from the earliest unpublished scalar-load
reservation. Do not choose slots in the wake module.

Expose core methods to query the request, mark the event scheduled/fired, and
report owned/detached wakes for quiescence checks. The public-to-system bridge
accepts `(SchedulerInstanceId, PendingEventSnapshot)`; the private CPU module
constructs `RiscvO3WritebackWake`, so no private wake type crosses the crate
boundary.
Add a checkpoint-only finalizer on the wake state that clears `detached` only
when `desired_tick` and `scheduled` are both `None`; Task 5 calls it only after
all live writeback owners and pending publications are gone.

- [ ] **Step 6: Schedule serial and parallel wakes from the system driver**

Add one helper called by both system scheduling adapters after normal trap/event
scheduling:

```rust
fn schedule_o3_writeback_wakes(
    cluster: &RiscvCluster,
    scheduler: &mut PartitionedScheduler,
    parallel: bool,
) -> Result<Vec<PartitionEventId>, SystemError> {
    let mut scheduled = Vec::new();
    for cpu in cluster.core_ids() {
        let core = cluster.core(cpu).map_err(SystemError::RiscvCluster)?;
        let Some(tick) = core.requested_o3_writeback_wake_tick(scheduler.now()) else {
            continue;
        };
        let fired = core.clone();
        let event_id = if parallel {
            scheduler.schedule_parallel_at(core.partition(), tick, move |context| {
                fired.mark_o3_writeback_wake_fired(context.now());
            })
        } else {
            scheduler.schedule_at(core.partition(), tick, move |context| {
                fired.mark_o3_writeback_wake_fired(context.now());
            })
        }
        .map_err(SystemError::Scheduler)?;
        let event = scheduler.pending_event_snapshot(event_id)
            .expect("new O3 writeback wake is pending");
        core.mark_o3_writeback_wake_scheduled(scheduler.instance_id(), event);
        scheduled.push(event_id);
    }
    Ok(scheduled)
}
```

Append these IDs to the vectors already returned by
`schedule_riscv_system_events_from_turn` and its parallel variant. The wake is
a scheduler-progress event; publication remains in the run-stats drain at the
current tick.

- [ ] **Step 7: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-cpu scalar_load_publication --lib -- --nocapture
cargo test -p rem6-cpu writeback_error_is_sticky --lib -- --nocapture
cargo test -p rem6-cpu o3_writeback_wake --lib -- --nocapture
cargo test -p rem6-system schedule_riscv_system_events_from_turn_ -- --nocapture
cargo test -p rem6-system record_run_stats_does_not_publish_scalar_load_before_admitted_tick -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission -- --nocapture
```

Expected: load response data may be resident before admission, but ROB readiness,
dependent issue, trace writeback, and architectural register publication all
wait for the admitted tick; serial and parallel runs cannot go idle early, and
an invariant failure surfaces as the same sticky drive error without partial
response state.

```bash
git add crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/riscv_cluster.rs \
  crates/rem6-cpu/src/riscv_cluster_translation.rs \
  crates/rem6-cpu/src/riscv_drive.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_writeback.rs \
  crates/rem6-cpu/src/o3_runtime_handoff.rs \
  crates/rem6-cpu/src/o3_runtime_live_window.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/o3_runtime_memory_tests.rs \
  crates/rem6-cpu/src/o3_runtime_memory_window.rs \
  crates/rem6-cpu/src/o3_runtime_retire.rs \
  crates/rem6-cpu/src/o3_runtime_control_window.rs \
  crates/rem6-cpu/src/o3_source_operands.rs \
  crates/rem6-cpu/src/riscv_o3_window_policy.rs \
  crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_data_issue/forwarding.rs \
  crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/forwarding.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/multi_load.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/store_store_load.rs \
  crates/rem6-cpu/src/riscv_o3_writeback_wake.rs \
  crates/rem6-cpu/tests/riscv_frontend.rs \
  crates/rem6-cpu/tests/riscv_cluster_data.rs \
  crates/rem6-system/src/lib.rs \
  crates/rem6-system/src/riscv_run_stats.rs \
  crates/rem6-system/src/riscv_o3_runtime_stats.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs
git commit -m "cpu: gate scalar load writeback admission"
```

### Task 5: Harden Cleanup and Checkpoint Version 23

**Files:**
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_authority.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_checkpoint_tests.rs`
- Modify: `crates/rem6-cpu/src/public_api.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_gate.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- Modify: `crates/rem6-cpu/src/riscv_execute.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch.rs`
- Modify: `crates/rem6-cpu/src/riscv_hart_run_state.rs`
- Modify: `crates/rem6-cpu/src/riscv_htm.rs`
- Modify: `crates/rem6-cpu/src/riscv_in_order_drive_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`
- Test: `crates/rem6-cpu/tests/o3_runtime.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint/o3_payload.rs`
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats.rs`
- Modify: `crates/rem6-system/src/riscv_sbi.rs`
- Modify: `crates/rem6-system/src/riscv_sbi/tests.rs`
- Test: `crates/rem6-system/tests/live_retire_gate_scheduler_checkpoint.rs`
- Test: `crates/rem6-system/tests/riscv_checkpoint.rs`
- Modify: `crates/rem6-system/tests/riscv_checkpoint/o3_compatibility.rs`
- Modify: `crates/rem6-system/tests/source_policy.rs`
- Modify: `crates/rem6/src/core_summary.rs`
- Modify: `crates/rem6/src/core_summary_json.rs`
- Modify: `crates/rem6/src/host_actions.rs`
- Create: `crates/rem6/src/host_actions/o3_checkpoint_decode.rs`
- Modify: `crates/rem6/src/run_execution_summary.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- Modify: `crates/rem6/tests/source_policy.rs`

- [ ] **Step 1: Write failing cleanup, reset, and compatibility tests**

Add focused runtime tests:

```rust
#[test]
fn rollback_discards_future_writeback_reservation() {
    let mut runtime = runtime_with_reserved_sequences([(4, 20), (5, 21), (6, 22)]);
    runtime.discard_live_staged_window_from_at(5, 19);
    assert!(runtime.writeback_reservation(4).is_some());
    assert!(runtime.writeback_reservation(5).is_none());
    assert!(runtime.writeback_reservation(6).is_none());
}

#[test]
fn stats_reset_preserves_writeback_calendar_without_recounting_reservations() {
    let mut runtime = runtime_with_reserved_sequences([(4, 20), (5, 21)]);
    let reservations = runtime.writeback_reservations();
    runtime.reset_stats();
    assert_eq!(runtime.writeback_reservations(), reservations);
    assert_eq!(runtime.stats().writeback_port_admitted_rows(), 0);
    runtime.reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(4, 20)]).unwrap();
    assert_eq!(runtime.stats().writeback_port_admitted_rows(), 0);
}

#[test]
fn writeback_calendar_prunes_only_before_current_tick() {
    let mut runtime = runtime_with_reserved_sequences([(4, 20), (5, 21)]);
    runtime.prune_writeback_calendar_before(21);
    assert!(runtime.writeback_reservation(4).is_none());
    assert!(runtime.writeback_reservation(5).is_some());
    runtime.prune_writeback_calendar_before(22);
    assert!(runtime.writeback_reservation(5).is_none());
}

#[test]
fn discarded_future_slot_can_be_reused_without_replaying_history() {
    let mut runtime = runtime_with_reserved_sequences([(4, 20), (5, 21)]);
    runtime.discard_future_writeback_sequence(5, 20);
    let replacement = runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(6, 21)])
        .unwrap()[0];
    assert_eq!(replacement.admitted_tick(), 21);
    assert_eq!(runtime.writeback_reservation(4).unwrap().admitted_tick(), 20);
}

fn runtime_with_reserved_sequences<const N: usize>(
    rows: [(u64, u64); N],
) -> O3RuntimeState {
    let mut runtime = O3RuntimeState::default();
    runtime
        .reserve_writeback_completions(
            rows.map(|(sequence, tick)| O3LiveWritebackReady::fixed_fu(sequence, tick)),
        )
        .unwrap();
    runtime
}
```

Add retry/failure, branch-descendant, PC redirect, reset, and full lifecycle
cleanup variants. Each must assert both owner-side admitted ticks and calendar
entries disappear for discarded rows while surviving rows remain.

Add direct, serial RFENCE, and parallel RFENCE regressions. A fetch-only reset
at the current scheduler tick must remove speculative future reservations while
preserving a completed scalar-load or retired-owner reservation and same-tick
history. Reserve a later row at the surviving owner's raw-ready tick and prove
it is deferred instead of reusing the occupied slot.

Add a real wake lifecycle with two completed loads: schedule and fire the first
wake, publish the first load, assert the desired tick advances to the second,
then fire/publish the second and assert checkpoint finalization clears consumed
calendar history and detached no-op wake authority without a manual desired-tick
reset.

Add checkpoint codec tests:

```rust
#[test]
fn checkpoint_v23_payloads_round_trip_writeback_port_stats() {
    let payload = O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        default_o3_runtime_snapshot(),
        O3RuntimeStats {
            writeback_port_cycles: 3,
            writeback_port_admitted_rows: 4,
            writeback_port_deferred_rows: 2,
            writeback_port_deferred_row_cycles: 5,
            writeback_port_max_ready_rows_per_cycle: 3,
            writeback_port_max_deferred_rows: 2,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap();
    let decoded = O3RuntimeCheckpointPayload::decode(&payload.encode()).unwrap();
    assert_eq!(decoded.stats().writeback_port_cycles(), 3);
    assert_eq!(decoded.stats().writeback_port_admitted_rows(), 4);
    assert_eq!(decoded.stats().writeback_port_deferred_rows(), 2);
    assert_eq!(decoded.stats().writeback_port_deferred_row_cycles(), 5);
    assert_eq!(decoded.stats().writeback_port_max_ready_rows_per_cycle(), 3);
    assert_eq!(decoded.stats().writeback_port_max_deferred_rows(), 2);
}
```

Add exact tests:

```text
checkpoint_v22_payloads_decode_without_writeback_port_stats
checkpoint_v1_through_v22_normalize_deferred_writeback_before_restore_or_encode
legacy_pending_only_payload_normalizes_deferred_writeback_before_restore_or_encode
checkpoint_v23_rejects_nonempty_stable_deferred_writeback
from_legacy_pending_state_preserves_inspectable_deferred_snapshot
```

In system compatibility tests, prove a legacy pending-only chunk uses the
public CPU bridge, v22 deferred rows normalize, invalid v23 deferred rows reject
without partial restore, and a normalized re-encode is v23.

Add CLI rows `rem6_run_o3_writeback_wrong_path_reservation_never_publishes` and
`rem6_run_o3_writeback_port_checkpoint_boundary`. The live capture must remain
non-quiescent; the drained capture must have runtime version 23, empty ROB/LSQ,
empty calendar/wake authority, and preserved counters.

The wrong-path row must be a real fixed-FU reservation, not merely an absent
trace row. Expose an immutable owner-side calendar snapshot through the core
summary JSON, capture the issued wrong-path DIV with its sequence, raw/admitted
tick, and slot before squash, then assert that exact sequence is absent after
the branch misprediction and never publishes architecturally.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6-cpu rollback_discards_future_writeback_reservation --lib -- --nocapture
cargo test -p rem6-cpu checkpoint_v23 --lib -- --nocapture
cargo test -p rem6-cpu checkpoint_v22 --lib -- --nocapture
cargo test -p rem6-system --test riscv_checkpoint o3_compatibility -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_wrong_path_reservation_never_publishes -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_port_checkpoint_boundary -- --exact --nocapture
```

Expected: failures because cleanup does not own reservations, the codec is
still v22, decode origin is discarded, and the legacy bridge rebuilds a current-
origin payload.

- [ ] **Step 3: Wire calendar cleanup into every live-owner cleanup path**

Add focused helpers that distinguish unconsumed authority from historical
same-tick occupancy:

```rust
pub(super) fn discard_future_writeback_sequence(&mut self, sequence: u64, now: u64) {
    if self
        .writeback_calendar
        .reservation(sequence)
        .is_some_and(|reservation| reservation.admitted_tick() > now)
    {
        self.writeback_calendar.remove_sequence(sequence);
    }
}

pub(super) fn discard_future_writeback_from_sequence(&mut self, sequence: u64, now: u64) {
    self.writeback_calendar
        .remove_future_from_sequence(sequence, now);
}

pub(super) fn discard_all_writeback_reservations(&mut self) {
    self.writeback_calendar.clear();
    self.live_writeback_cycle_ticks.clear();
    self.live_writeback_ready_rows_by_tick.clear();
}

pub(super) fn prune_writeback_calendar_before(&mut self, tick: u64) {
    self.writeback_calendar.prune_before(tick);
}
```

Thread the current scheduler/response tick through branch rollback, scalar
retry/failure suffix cancellation, and split-fetch rebind so those paths call
the future-only helpers. Full reset, restore, HTM abort, PC replacement, and
detailed-mode teardown may call `discard_all_writeback_reservations` because
the entire detailed authority is being abandoned. Normal FU/load retirement
clears owner-side timing but does not remove its calendar entry; the next later
tick prunes it.
Explicit hart reset and successful checkpoint restore also clear
`pending_callback_error`; all other cleanup leaves a callback failure sticky.

Add `_at` variants for continuing-execution cleanup:

```text
discard_live_staged_instructions_at(now)
discard_live_staged_window_from_at(sequence, now)
discard_live_control_descendants_from_at(branch_sequence, now)
discard_live_scalar_memory_window_rows_at(sequence, now)
```

The existing no-tick full-teardown methods continue to clear all transient
authority. `riscv_execute.rs` passes `retire_tick` to the `_at` variants, and
the scalar response path passes `response_tick`. Update focused control-window
tests to call the `_at` variants with a tick before their future reservations.

Make `reset_instruction_fetch_stream(now)` tick-aware. It is a fetch-only
rebind, so it calls `discard_live_speculative_executions_at(now)` and preserves
reservations owned by completed scalar loads, retired rows, and current-tick
history. Serial and parallel remote FENCE.I callbacks pass `context.now()`.

Audit these paths:

```text
discard_live_retire_window
discard_live_staged_instructions
discard_live_staged_window_from
discard_live_control_descendants_from
invalidate_live_speculative_execution_chain
discard_live_scalar_memory_lifecycle
discard_live_scalar_memory_window_rows
remove_live_scalar_memory_rows
restore
PC redirect, interrupt/reset, HTM abort, and detailed-mode cleanup callers
```

After every scalar-memory cleanup, recompute desired writeback wake state from
surviving unpublished loads. Keep current-tick calendar occupancy until a later
tick or quiescent checkpoint finalization.

Extend `has_pending_retirement_authority` to include live calendar reservations.
Expose a core quiescence query that also checks scheduled/detached writeback
wakes; system checkpoint capture must return the existing non-quiescent error
while either authority exists. Add
`RiscvCore::finalize_quiescent_o3_writeback_for_checkpoint`: under the core
lock, clear consumed calendar history and already-fired detached wake snapshots
when no live FU/load owner, pending publication, desired wake, or scheduled wake
remains. A still-scheduled wake keeps capture non-quiescent. Call the finalizer
at the start of `RiscvCoreCheckpointPort::validate_capture` before the final
quiescence check; this is the only path allowed to remove the last consumed
current-tick entry or detached no-op without observing a later scheduler tick.

When a scheduled writeback wake fires, clear the desired tick only when that
tick has been satisfied. After scalar-load publication, recompute desired wake
authority from the remaining unpublished loads so a second load schedules at
its own tick and the final load leaves checkpoint capture genuinely quiescent.

- [ ] **Step 4: Add checkpoint v23 counters and private decode origin**

Add:

```rust
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS: u8 = 23;
const O3_RUNTIME_CHECKPOINT_VERSION: u8 =
    O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3RuntimeCheckpointDecodeOrigin {
    RuntimeVersion(u8),
    LegacyPendingOnly,
}
```

Store `decode_origin` in `O3RuntimeCheckpointPayload`. Current constructors use
`RuntimeVersion(23)`; `decode` stores the input version. Tail-append six `u64`
counters before the live-retire-gate trailer. Add
`WRITEBACK_PORT_STATS_BYTES = 6 * U64_BYTES` to downgrade helpers, and decode
zeros for versions 1 through 22.

Route public constructors and decode through one private origin-aware
constructor:

```rust
fn from_snapshot_with_stats_and_origin(
    snapshot: O3RuntimeSnapshot,
    stats: O3RuntimeStats,
    dependency_producers_with_consumers: BTreeSet<O3PhysicalRegisterId>,
    decode_origin: O3RuntimeCheckpointDecodeOrigin,
) -> Result<Self, O3RuntimeError> {
    let snapshot = snapshot.into_checkpoint_snapshot();
    validate_runtime_snapshot(&snapshot)?;
    if matches!(
        decode_origin,
        O3RuntimeCheckpointDecodeOrigin::RuntimeVersion(23)
    ) && !snapshot.pending_state().writeback().deferred().is_empty()
    {
        return Err(O3RuntimeError::StableWritebackQueueNotEmpty {
            deferred: snapshot.pending_state().writeback().deferred().len(),
        });
    }
    Ok(Self {
        snapshot,
        stats,
        dependency_producers_with_consumers,
        live_retire_gate: None,
        decode_origin,
    })
}
```

`from_snapshot*` passes `RuntimeVersion(23)`. `decode` passes the decoded
runtime version after rejecting unsupported versions, which lets v1-v22 remain
inspectable while rejecting a nonempty stable v23 queue.

Add the public bridge in `rem6-cpu`:

```rust
pub fn from_legacy_pending_state(
    pending_state: O3PendingStateSnapshot,
) -> Result<Self, O3RuntimeError> {
    let default = default_o3_runtime_snapshot();
    let snapshot = O3RuntimeSnapshot::new(
        default.reorder_buffer().iter().copied(),
        default.load_store_queue().iter().copied(),
        default.rename_map().iter().copied(),
        pending_state,
    )?;
    Self::from_snapshot_with_stats_and_origin(
        snapshot,
        O3RuntimeStats::default(),
        BTreeSet::new(),
        O3RuntimeCheckpointDecodeOrigin::LegacyPendingOnly,
    )
}
```

Keep `snapshot()` inspectable. Add a private normalization helper used by restore
and encode:

```rust
fn normalized_snapshot_for_current_runtime(&self) -> Result<O3RuntimeSnapshot, O3RuntimeError> {
    let legacy = matches!(
        self.decode_origin,
        O3RuntimeCheckpointDecodeOrigin::LegacyPendingOnly
            | O3RuntimeCheckpointDecodeOrigin::RuntimeVersion(1..=22)
    );
    if !legacy && !self.snapshot.pending_state().writeback().deferred().is_empty() {
        return Err(O3RuntimeError::StableWritebackQueueNotEmpty {
            deferred: self.snapshot.pending_state().writeback().deferred().len(),
        });
    }
    if !legacy {
        return Ok(self.snapshot.clone());
    }
    snapshot_with_empty_writeback_deferred(&self.snapshot)
}

fn snapshot_with_empty_writeback_deferred(
    snapshot: &O3RuntimeSnapshot,
) -> Result<O3RuntimeSnapshot, O3RuntimeError> {
    let pending = snapshot.pending_state();
    let normalized_pending = O3PendingStateSnapshot::new(
        pending.resolved_dependency_scopes().iter().copied(),
        pending.ready().iter().cloned(),
        O3WritebackTransferSnapshot::new(
            pending.writeback().policy().clone(),
            [],
        ),
    )
    .map_err(|error| O3RuntimeError::InvalidPendingState { error })?;
    let mut normalized = snapshot.clone();
    normalized.pending_state = normalized_pending;
    Ok(normalized)
}
```

Because `encode()` currently returns `Vec<u8>`, normalize a legacy-origin local
snapshot before encoding and use `expect` only for the invariant already
enforced by current constructors/decode. Reject invalid current v23 state in
those fallible entry points. `restore_checkpoint_payload` consumes the
normalized snapshot and then restores stats/dependency state.

In `rem6-system/src/riscv_checkpoint/o3_payload.rs`, replace local default ROB/
LSQ/rename reconstruction with:

```rust
O3RuntimeCheckpointPayload::from_legacy_pending_state(pending.into_snapshot())
    .map_err(|error| RiscvCoreCheckpointError::InvalidO3RuntimeSnapshot {
        component: component.clone(),
        error,
    })
```

Update system source policy to require this bridge and forbid local
`O3RuntimeSnapshot::new` reconstruction in the legacy pending path.

Move `O3RuntimeState::checkpoint_payload` into the checkpoint module rather
than compressing the runtime root to fit a new re-export. Tighten the rem6
source-policy assertion so `o3_runtime.rs` must remain strictly below its
1700-line ceiling, preserving real headroom for later correctness work.

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-cpu o3_runtime_writeback --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_checkpoint --lib -- --nocapture
cargo test -p rem6-cpu fetch_stream_reset_preserves_retired_writeback_slot_occupancy --lib -- --nocapture
cargo test -p rem6-cpu o3_writeback_wake --lib -- --nocapture
cargo test -p rem6-system remote_fence_i_preserves_target_writeback_reservation --lib -- --nocapture
cargo test -p rem6-system --test riscv_checkpoint o3_compatibility -- --nocapture
cargo test -p rem6-system --test source_policy riscv_checkpoint_emits_one_o3_authority_and_isolates_legacy_decode -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_wrong_path_reservation_never_publishes -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_port_checkpoint_boundary -- --exact --nocapture
```

Expected: legacy snapshots remain inspectable but cannot become live deferred
authority, v23 rejects nonempty stable queues, cleanup removes future slots, and
drained checkpoint/restore preserves only policy plus counters.

```bash
git add crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/o3_runtime_authority.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_checkpoint.rs \
  crates/rem6-cpu/src/o3_runtime_checkpoint_tests.rs \
  crates/rem6-cpu/src/o3_runtime_control_window.rs \
  crates/rem6-cpu/src/o3_runtime_control_window_tests.rs \
  crates/rem6-cpu/src/o3_runtime_live_window.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/o3_runtime_memory_tests.rs \
  crates/rem6-cpu/src/o3_runtime_writeback.rs \
  crates/rem6-cpu/src/o3_runtime_writeback_tests.rs \
  crates/rem6-cpu/src/public_api.rs \
  crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs \
  crates/rem6-cpu/src/riscv_execute.rs \
  crates/rem6-cpu/src/riscv_fetch.rs \
  crates/rem6-cpu/src/riscv_hart_run_state.rs \
  crates/rem6-cpu/src/riscv_htm.rs \
  crates/rem6-cpu/src/riscv_in_order_drive_tests.rs \
  crates/rem6-cpu/src/riscv_live_retire_gate.rs \
  crates/rem6-cpu/src/riscv_o3_writeback_wake.rs \
  crates/rem6-cpu/tests/o3_runtime.rs \
  crates/rem6-system/src/riscv_checkpoint.rs \
  crates/rem6-system/src/riscv_checkpoint/o3_payload.rs \
  crates/rem6-system/src/riscv_o3_runtime_stats.rs \
  crates/rem6-system/src/riscv_sbi.rs \
  crates/rem6-system/src/riscv_sbi/tests.rs \
  crates/rem6-system/tests/live_retire_gate_scheduler_checkpoint.rs \
  crates/rem6-system/tests/riscv_checkpoint.rs \
  crates/rem6-system/tests/riscv_checkpoint/o3_compatibility.rs \
  crates/rem6-system/tests/source_policy.rs \
  crates/rem6/src/core_summary.rs \
  crates/rem6/src/core_summary_json.rs \
  crates/rem6/src/host_actions.rs \
  crates/rem6/src/host_actions/o3_checkpoint_decode.rs \
  crates/rem6/src/run_execution_summary.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs \
  crates/rem6/tests/source_policy.rs
git commit -m "cpu: preserve O3 writeback checkpoint boundaries"
```

### Task 6: Expose Native JSON, Text, and Stats-Dump Evidence

**Files:**
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs`
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs`
- Create: `crates/rem6/src/stats_output/o3_runtime_writeback.rs`
- Modify: `crates/rem6/src/stats_output/o3_runtime.rs`
- Modify: `crates/rem6/src/core_summary_json.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`

- [ ] **Step 1: Add failing structured JSON, text, dump, hierarchy, and suppression assertions**

Define the exact stat table in the focused CLI module:

```rust
const WRITEBACK_PORT_STATS: [(&str, &str); 6] = [
    ("cycles", "Cycle"),
    ("admitted_rows", "Count"),
    ("deferred_rows", "Count"),
    ("deferred_row_cycles", "Cycle"),
    ("max_ready_rows_per_cycle", "Count"),
    ("max_deferred_rows", "Count"),
];
```

Extend the width-one collision test:

```rust
let writeback = json.pointer("/cores/0/o3_runtime/writeback_port")
    .unwrap_or_else(|| panic!("missing O3 writeback-port summary: {json}"));
assert!(writeback["cycles"].as_u64().is_some_and(|value| value > 0));
assert_eq!(writeback["admitted_rows"].as_u64(), Some(2));
assert_eq!(writeback["deferred_rows"].as_u64(), Some(1));
assert_eq!(writeback["deferred_row_cycles"].as_u64(), Some(1));
assert_eq!(writeback["max_ready_rows_per_cycle"].as_u64(), Some(2));
assert_eq!(writeback["max_deferred_rows"].as_u64(), Some(1));
for (field, unit) in WRITEBACK_PORT_STATS {
    assert_json_stat(
        &json,
        &format!("sim.cpu0.o3.writeback_port.{field}"),
        unit,
        writeback[field].as_u64().unwrap(),
        "monotonic",
    );
}
```

Add:

```text
rem6_run_o3_writeback_port_json_exposes_counters
rem6_run_o3_writeback_port_text_stats_expose_counters
rem6_run_o3_writeback_port_stats_dump_exposes_counters
rem6_run_o3_writeback_scalar_load_fu_collision_cache_fabric_dram
rem6_run_timing_suppresses_o3_writeback_port_surface
```

The text test must assert each path once with the exact unit. The dump fixture
must execute a real `m5_dump_stats` after all colliding rows and compare
`sim.host_actions.stats_dump.cpu0.o3.writeback_port.*` samples to the final
native values.

The hierarchy row reuses the scalar-load fixture through
`cache-fabric-dram` and requires positive cache, transport, fabric, and DRAM
activity plus the same load admitted-tick/dependency assertions.

The timing row must preserve final registers/memory while asserting the
structured object and all six native paths are absent.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6 --test cli_run rem6_run_o3_writeback_port_json_exposes_counters -- --exact --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_writeback_port_text_stats_expose_counters -- --exact --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_writeback_port_stats_dump_exposes_counters -- --exact --nocapture
cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_writeback_port_surface -- --exact --nocapture
```

Expected: runtime behavior passes far enough to expose the missing JSON/stat
surfaces, then assertions fail because those surfaces are absent.

- [ ] **Step 3: Register system stats with correct units and aggregation**

Add six `StatId` fields to `RiscvO3RuntimeCpuStats`. Register these exact paths:

```text
sim.cpu{cpu}.o3.writeback_port.cycles                       Cycle
sim.cpu{cpu}.o3.writeback_port.admitted_rows                Count
sim.cpu{cpu}.o3.writeback_port.deferred_rows                Count
sim.cpu{cpu}.o3.writeback_port.deferred_row_cycles          Cycle
sim.cpu{cpu}.o3.writeback_port.max_ready_rows_per_cycle     Count
sim.cpu{cpu}.o3.writeback_port.max_deferred_rows            Count
```

In delta updates, increment the first four and set the last two to the current
maximum. In `cpu/snapshot.rs`, use sum semantics for the first four and max
semantics for the final two, matching issue/extrema patterns already present.

- [ ] **Step 4: Add focused text/final stats and structured JSON**

Create `stats_output/o3_runtime_writeback.rs`:

```rust
use rem6_cpu::O3RuntimeStats;
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::Rem6CliError;

pub(super) fn emit_o3_runtime_writeback_port_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    o3: O3RuntimeStats,
) -> Result<(), Rem6CliError> {
    for (name, unit, value) in [
        ("cycles", "Cycle", o3.writeback_port_cycles()),
        ("admitted_rows", "Count", o3.writeback_port_admitted_rows()),
        ("deferred_rows", "Count", o3.writeback_port_deferred_rows()),
        ("deferred_row_cycles", "Cycle", o3.writeback_port_deferred_row_cycles()),
        ("max_ready_rows_per_cycle", "Count", o3.writeback_port_max_ready_rows_per_cycle()),
        ("max_deferred_rows", "Count", o3.writeback_port_max_deferred_rows()),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.writeback_port.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}
```

Declare/delegate it from `stats_output/o3_runtime.rs`.

Add to `core_summary_json.rs`:

```rust
fn o3_runtime_writeback_port_json(stats: O3RuntimeStats) -> String {
    format!(
        "{{\"cycles\":{},\"admitted_rows\":{},\"deferred_rows\":{},\"deferred_row_cycles\":{},\"max_ready_rows_per_cycle\":{},\"max_deferred_rows\":{}}}",
        stats.writeback_port_cycles(),
        stats.writeback_port_admitted_rows(),
        stats.writeback_port_deferred_rows(),
        stats.writeback_port_deferred_row_cycles(),
        stats.writeback_port_max_ready_rows_per_cycle(),
        stats.writeback_port_max_deferred_rows(),
    )
}
```

Interpolate the returned object under the `"writeback_port"` key beside the
existing `issue` object. Detailed-mode suppression remains inherited from the
existing O3 summary boundary.

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-system riscv_o3_runtime_stats -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port:: -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_writeback_port_surface -- --exact --nocapture
```

Expected: direct and hierarchy rows expose the same six runtime values through
structured JSON, native stats, text, and dump samples; timing mode exposes none.

```bash
git add crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs \
  crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs \
  crates/rem6/src/stats_output/o3_runtime_writeback.rs \
  crates/rem6/src/stats_output/o3_runtime.rs \
  crates/rem6/src/core_summary_json.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs
git commit -m "stats: expose O3 writeback contention"
```

### Task 7: Preserve and Expose Mode-Transfer Writeback State

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/public_api.rs`
- Modify: `crates/rem6-system/src/riscv_checkpoint.rs`
- Modify: `crates/rem6-system/src/host.rs`
- Modify: `crates/rem6-system/src/host/execution_mode_handoff.rs`
- Modify: `crates/rem6-system/src/host/execution_mode_transfer.rs`
- Modify: `crates/rem6/src/host_actions.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`

- [ ] **Step 1: Add a failing transfer continuity/debug test**

Add `run_live_o3_mode_transfer_with_args(path, switches, extra_args)` in
`switch.rs`; keep the current helper as a wrapper with `extra_args = &[]`, and
make that helper, `live_o3_mode_transfer_binary`,
`live_mode_transfer_event{,_if_present}`, and `event_u64_field` `pub(super)`.

Add `rem6_run_host_switch_preserves_o3_writeback_port_ticks` in the focused
module. Reuse `live_o3_mode_transfer_binary`, pass
`["--riscv-o3-writeback-width", "1"]`, derive the switch tick strictly between
the baseline DIV issue and admitted writeback ticks, and rerun with a scheduled
detailed-to-timing switch. For every inherited row, assert identical:

```rust
for field in ["issue_tick", "writeback_tick", "commit_tick"] {
    assert_eq!(
        event_u64(event_at_pc(&switched, pc), field),
        event_u64(event_at_pc(&baseline, pc), field),
        "field {field} diverged for inherited row {pc}",
    );
}
```

Assert the state-transfer summary exposes:

```text
writeback_width = 1
reserved_future_completions > 0
earliest_unpublished_writeback_tick = delayed row writeback tick
stats_writeback_port_cycles
stats_writeback_port_admitted_rows
stats_writeback_port_deferred_rows
stats_writeback_port_deferred_row_cycles
stats_writeback_port_max_ready_rows_per_cycle
stats_writeback_port_max_deferred_rows
```

Require `restorable == false`, `live_data_handoff == true`, and assert that the
first post-window timing-mode instruction at `0x8000001c` is absent from the
detailed O3 trace. Use the existing DIV plus three-younger-row PC table from the
switch test, not a second transfer binary.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6 --test cli_run rem6_run_host_switch_preserves_o3_writeback_port_ticks -- --exact --nocapture
```

Expected: inherited timing may already remain resident, but the transfer/debug
summary lacks explicit writeback width/calendar/counter fields.

- [ ] **Step 3: Add a typed transient debug snapshot**

Add and export:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvO3WritebackDebugState {
    width: usize,
    reserved_future_completions: usize,
    earliest_unpublished_tick: Option<u64>,
}

impl RiscvO3WritebackDebugState {
    pub const fn width(self) -> usize { self.width }
    pub const fn reserved_future_completions(self) -> usize {
        self.reserved_future_completions
    }
    pub const fn earliest_unpublished_tick(self) -> Option<u64> {
        self.earliest_unpublished_tick
    }
}
```

Expose `RiscvCore::o3_writeback_debug_state(now)` by reading the resident
runtime calendar. This is observation only; it must not serialize or restore
calendar authority.

Add `RiscvCoreCheckpointBank::o3_writeback_debug_state_for_target(target, now)`
beside `checker_summary_for_target`.

- [ ] **Step 4: Carry debug-only state in the transfer summary**

Add to `ExecutionModeSwitchStateTransfer`:

```rust
o3_writeback: Option<RiscvO3WritebackDebugState>,
```

Pass it into both `from_manifest` and `from_live_data_handoff_manifest` from
`capture_execution_mode_switch_state_transfer_with_scheduler`, using the host
action record tick as `now`. Keep it outside checkpoint chunks and exclude it
from restore logic.

Expose it through `Rem6ExecutionModeStateTransferSummary`:

```rust
pub(crate) writeback_width: Option<u64>,
pub(crate) reserved_future_completions: Option<u64>,
pub(crate) earliest_unpublished_writeback_tick: Option<u64>,
```

Extend `Rem6HostO3RuntimeCheckpointChunkSummary` with the six decoded v23
counter fields and `writeback_width`, which is available from
`snapshot.pending_state().writeback().policy()`. Populate `decode_error`,
`numeric_fields`, unit mapping, and aggregation: sum the first four counters,
max the two extrema, and treat width as max/equality-preserving metadata.

Do not add calendar rows to the O3 runtime checkpoint or live-data handoff
payload. The resident core remains the only authority through the switch.

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6 --test cli_run rem6_run_host_switch_preserves_o3_writeback_port_ticks -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch:: -- --nocapture
cargo test -p rem6 host_o3_runtime_checkpoint_chunk -- --nocapture
cargo test -p rem6-system execution_mode_handoff -- --nocapture
```

Expected: the switch is non-restorable, the resident calendar/wake continues to
drain, inherited timing matches baseline, and debug fields are projections only.

```bash
git add crates/rem6-cpu/src/o3_runtime_writeback.rs \
  crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/public_api.rs \
  crates/rem6-system/src/riscv_checkpoint.rs \
  crates/rem6-system/src/host.rs \
  crates/rem6-system/src/host/execution_mode_handoff.rs \
  crates/rem6-system/src/host/execution_mode_transfer.rs \
  crates/rem6/src/host_actions.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/switch.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs
git commit -m "debug: expose O3 writeback transfer state"
```

### Task 8: Protect Ownership and Update the Migration Ledger

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add source-policy ownership checks**

Add `o3_runtime_writeback_lives_in_focused_module` beside the issue-owner check.
Require `o3_runtime.rs` to declare the focused module and require only
`o3_runtime_writeback.rs` to own these anchors:

```rust
let writeback_authority_patterns = [
    "struct O3WritebackReservationCalendar",
    "fn reserve_writeback_completions(",
    "fn discard_future_writeback_from_sequence(",
    "writeback_port_deferred_row_cycles",
];
```

Require `riscv_live_retire_window.rs` and `o3_runtime_memory.rs` to delegate to
the focused reservation API without constructing the calendar or generic
buffer. Add a focused wake-owner check requiring
`riscv_o3_writeback_wake.rs` to own `RiscvO3WritebackWakeState`, scheduled/
detached state, and desired-tick replacement.

Separately require `o3_pipeline.rs` to be the only owner of
`pub fn plan_cycle_with_occupied_slots`, and require
`o3_runtime_writeback.rs` to call that API without reimplementing its occupied-
slot validation or deferred-before-new ordering. The generic planner is not a
RISC-V runtime-authority pattern.

Add `cli_stats_output_o3_runtime_writeback_stays_focused`, mirroring the issue
test. Require `stats_output/o3_runtime.rs` to declare/delegate to
`o3_runtime_writeback.rs`, keep the root below its existing cap, keep the new
module below 800 lines, and ensure the six field strings occur in no other
stats-output source file.

Keep `config.rs` below 1,700 lines and the new CLI test module below the existing
1,800-line child-module cap.

- [ ] **Step 2: Add exact source-policy anchors**

Append:

```text
--riscv-o3-writeback-width
riscv_o3_writeback_width
/cores/0/o3_runtime/writeback_port
sim.cpu0.o3.writeback_port.cycles
sim.cpu0.o3.writeback_port.admitted_rows
sim.cpu0.o3.writeback_port.deferred_rows
sim.cpu0.o3.writeback_port.deferred_row_cycles
sim.cpu0.o3.writeback_port.max_ready_rows_per_cycle
sim.cpu0.o3.writeback_port.max_deferred_rows
sim.host_actions.stats_dump.cpu0.o3.writeback_port.cycles
rem6_run_o3_writeback_width_one_serializes_direct_fu_dependent_collision
rem6_run_o3_writeback_width_two_exact_fit_direct_fu_dependent_collision
rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission
rem6_run_o3_writeback_scalar_load_fu_collision_cache_fabric_dram
rem6_run_o3_writeback_wrong_path_reservation_never_publishes
rem6_run_host_switch_preserves_o3_writeback_port_ticks
rem6_run_o3_writeback_port_checkpoint_boundary
rem6_run_timing_suppresses_o3_writeback_port_surface
rem6_run_o3_writeback_port_json_exposes_counters
rem6_run_o3_writeback_port_text_stats_expose_counters
rem6_run_o3_writeback_port_stats_dump_exposes_counters
```

- [ ] **Step 3: Update the ledger without changing the score**

Keep the CPU heading at `74% representative`, raw score at `8 of 10`, and both
unchecked items unchanged. Update the CPU prose with bounded shared scalar FU/
load writeback ownership, widths 1/2/4, deterministic reservation precedence,
dependency wakeup, rollback/retry cleanup, direct and hierarchy routes,
checkpoint v23/v1-v22 compatibility, live checkpoint rejection, mode-transfer
continuity, timing suppression, explicit trace ticks, and JSON/text/dump stats.

Remove only `writeback-port contention` from the CPU `Next evidence` gap. Keep
all of these open verbatim in substance:

```text
general IQ/wakeup/select beyond bounded scoped issue authority
arbitrary mixed memory/control windows
restorable live transport ownership
indirect or unconditional deeper control chains
fourth-and-deeper branch chains
broad FP/vector/atomic/MMIO writeback
a general O3 engine
```

Update only directly supported Test Migration Ledger rows:

```text
tests/gem5/checkpoint_tests
tests/gem5/cpu_tests
tests/gem5/processor_switch_tests
tests/gem5/stats
```

Preserve exactly 1,200 lines by tightening existing CPU prose rather than adding
blank padding or a duplicate ledger.

- [ ] **Step 4: Verify policy and ledger consistency, then commit**

Run:

```bash
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: both policy suites pass and the ledger prints exactly `1200` lines.

```bash
git add crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record bounded O3 writeback evidence"
```

### Task 9: Full Verification, Review, and Push

**Files:**
- Read only: complete diff and verification outputs.
- Modify only if a verification or review finding requires a scoped fix.

- [ ] **Step 1: Run focused regression matrices**

Run:

```bash
cargo test -p rem6-cpu --test o3_pipeline o3_writeback_transfer -- --nocapture
cargo test -p rem6-cpu o3_runtime_writeback --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_memory --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_checkpoint --lib -- --nocapture
cargo test -p rem6-system schedule_riscv_system_events_from_turn_ -- --nocapture
cargo test -p rem6-system --test riscv_checkpoint o3_compatibility -- --nocapture
cargo test -p rem6 --test cli_run riscv_o3_writeback_width -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port:: -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::scoped_issue:: -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::lsq_fu_window:: -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::lsq_fu_branch:: -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch:: -- --nocapture
```

Expected: all focused runtime, scheduler, checkpoint, configuration, route,
cleanup, transfer, stats, and suppression rows pass.

- [ ] **Step 2: Run broad repository gates**

Run:

```bash
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --test cli_run
cargo test --workspace --all-targets
cargo test -p rem6-cpu --test source_policy
cargo test -p rem6 --test source_policy
cargo fmt --check
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short --branch
```

Expected: every command exits zero, the ledger is exactly 1,200 lines, and only
the intended implementation commits differ from the plan baseline.

- [ ] **Step 3: Run a high-intensity read-only review**

Dispatch an xhigh `gpt-5.5` reviewer over the complete implementation range.
Require findings ordered by severity and specifically ask it to verify:

```text
one calendar and no duplicate serialized writeback queue
no early future-raw-ready admission
fixed-FU reservation before gate wake scheduling
scalar load ROB/register publication only at admitted tick
callback reservation errors are sticky, drive-visible, and leave no partial response state
serial/parallel wake dedup, stale-wake pruning, and cleanup
no retroactive displacement of fixed reservations by late loads
explicit live trace tick with legacy fallback
v23 and every v1-v22/LegacyPendingOnly compatibility boundary
sum/max stats aggregation and timing-mode suppression
mode transfer remains resident and non-restorable
source-policy ownership and honest 74% ledger scope
no dead code, orphan APIs, weakened assertions, or temp files
```

Resolve every Critical/Important finding, rerun the affected focused tests, and
request a concise follow-up review. Record Minor findings only when they are
genuinely outside this bounded increment.

- [ ] **Step 4: Verify commit history and push**

Run:

```bash
git log --oneline --decorate -12
git status --short --branch
git push origin main
git rev-parse HEAD origin/main
```

Expected: commits are behavior-scoped, the worktree is clean, push succeeds,
and local/remote revisions match.
