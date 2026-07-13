# RISC-V In-Order Fetch Admission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore cycle-visible configured in-order width by admitting fetches before generic pipeline draining, removing synchronous enqueue time advancement, and preserving capacity-safe progress across serial, translated, parallel, and low-level fetch paths.

**Architecture:** `in_order_pipeline.rs` owns pure capacity-checked row insertion, while `riscv_in_order_drive.rs` owns a typed admission decision and all scheduled normal pipeline wakes. Fetch synchronization defers excess low-level events, and every driver attempts admitted fetch work before invoking the existing scheduler-owned pipeline cycle path.

**Tech Stack:** Rust 2021 workspace, `rem6-cpu`, `rem6` CLI integration tests, `PartitionedScheduler`, existing RISC-V fetch/translation/cluster helpers, Cargo test and source-policy gates.

---

## File Map

- Modify `crates/rem6-cpu/src/in_order_pipeline/error.rs`: add the typed full-stage error used by pure enqueue.
- Modify `crates/rem6-cpu/src/in_order_pipeline.rs`: expose focused Fetch1/Commit queries, delete recorded enqueue, and make enqueue capacity checked without advancing time.
- Modify `crates/rem6-cpu/src/riscv_in_order_drive.rs`: define the typed fetch-admission state and focused core query.
- Modify `crates/rem6-cpu/src/riscv_in_order_drive_tests.rs`: cover admission state and enqueue invariants.
- Modify `crates/rem6-cpu/src/riscv_execute.rs`: defer fetch-event rows when Fetch1 is full.
- Modify `crates/rem6-cpu/tests/riscv_in_order_timing.rs`: cover low-level direct issue at width one and width two.
- Modify `crates/rem6-cpu/src/riscv_drive.rs`: reorder normal serial fetch admission before pipeline draining.
- Modify `crates/rem6-cpu/src/riscv_translation.rs`: apply explicit fetch-admission ordering to translated serial execution.
- Modify `crates/rem6-cpu/tests/riscv_frontend.rs`: restore the raw serial width-two overlap assertion and add width-one backpressure.
- Modify `crates/rem6-cpu/tests/riscv_translation_fetch_ahead.rs`: add translated width-two admission evidence.
- Modify `crates/rem6-cpu/src/riscv_cluster_drive.rs`: centralize the cluster's fetch-before-pipeline admission predicate.
- Modify `crates/rem6-cpu/src/riscv_cluster.rs`: reorder all prepared/direct, translated/untranslated, MMIO, and budgeted parallel loops.
- Modify `crates/rem6-cpu/tests/riscv_cluster.rs`: add raw prepared-parallel width evidence.
- Modify `crates/rem6-cpu/tests/riscv_cluster_translation.rs`: add translated parallel width evidence.
- Modify `crates/rem6-cpu/tests/source_policy.rs`: prohibit recorded enqueue and duplicate timing authority.
- Modify `crates/rem6/tests/cli_run/stats_compat.rs`: restore width occupancy and multi-row movement assertions, including one hierarchy row.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record executable evidence without changing 74%, 8/10, or the 1200-line boundary.

### Task 1: Make Pipeline Fetch Admission Explicit

**Files:**
- Modify: `crates/rem6-cpu/src/in_order_pipeline/error.rs`
- Modify: `crates/rem6-cpu/src/in_order_pipeline.rs:1262-1300`
- Modify: `crates/rem6-cpu/src/riscv_in_order_drive.rs:1-150`
- Test: `crates/rem6-cpu/src/riscv_in_order_drive_tests.rs`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing enqueue and admission tests**

Extend the imports in `riscv_in_order_drive_tests.rs` with
`InOrderPipelineConfig`, `InOrderPipelineError`, and
`InOrderPipelineStageWidth`. Add this helper and tests:

```rust
fn uniform_pipeline_config(width: usize) -> InOrderPipelineConfig {
    InOrderPipelineConfig::new(
        InOrderPipelineStage::ALL
            .map(|stage| InOrderPipelineStageWidth::new(stage, width).unwrap()),
    )
    .unwrap()
}

#[test]
fn enqueue_fetch_never_advances_time_and_rejects_full_fetch1() {
    let mut pipeline = crate::InOrderPipelineState::new(uniform_pipeline_config(1));

    pipeline.enqueue_fetch(0).unwrap();
    assert_eq!(pipeline.cycle(), 0);
    assert_eq!(
        pipeline.in_flight(),
        &[InOrderPipelineInstruction::new(0, InOrderPipelineStage::Fetch1)]
    );
    assert_eq!(
        pipeline.enqueue_fetch(1),
        Err(InOrderPipelineError::StageAtCapacity {
            stage: InOrderPipelineStage::Fetch1,
            width: 1,
        })
    );
    assert_eq!(pipeline.cycle(), 0);

    pipeline.enqueue_fetch(0).unwrap();
    assert_eq!(pipeline.in_flight().len(), 1);
}

#[test]
fn fetch_admission_distinguishes_available_advance_and_retire_states() {
    let core = core_with_completed_fetch();
    core.reset_in_order_pipeline_config(uniform_pipeline_config(1));

    assert_eq!(
        core.in_order_fetch_admission(),
        RiscvInOrderFetchAdmission::Admitted
    );

    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        uniform_pipeline_config(1),
        0,
        [InOrderPipelineInstruction::new(
            0,
            InOrderPipelineStage::Fetch1,
        )],
    ))
    .unwrap();
    assert_eq!(
        core.in_order_fetch_admission(),
        RiscvInOrderFetchAdmission::AdvanceBeforeFetch
    );

    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        uniform_pipeline_config(1),
        0,
        [
            InOrderPipelineInstruction::new(0, InOrderPipelineStage::Commit),
            InOrderPipelineInstruction::new(1, InOrderPipelineStage::Fetch1),
        ],
    ))
    .unwrap();
    assert_eq!(
        core.in_order_fetch_admission(),
        RiscvInOrderFetchAdmission::RetireBeforeFetch
    );
}

#[test]
fn pending_pipeline_wake_blocks_fetch_admission() {
    let core = core_with_completed_fetch();
    let mut scheduler = PartitionedScheduler::new(1).unwrap();
    assert!(matches!(
        core.schedule_next_completed_fetch_pipeline_cycle_serial(&mut scheduler)
            .unwrap(),
        RiscvInOrderDriveStatus::Scheduled(_)
    ));

    assert_eq!(
        core.in_order_fetch_admission(),
        RiscvInOrderFetchAdmission::PipelineCyclePending
    );
}
```

In `source_policy.rs`, extend
`normal_riscv_drivers_delegate_pipeline_time_to_focused_scheduler_authority`
with failing ownership assertions:

```rust
let in_order = fs::read_to_string(crate_dir.join("src/in_order_pipeline.rs")).unwrap();
let enqueue = source_section(
    &in_order,
    "pub fn enqueue_fetch(",
    "pub fn plan_cycle(",
);

assert!(timing.contains("pub(crate) enum RiscvInOrderFetchAdmission"));
assert!(timing.contains("pub(crate) fn in_order_fetch_admission("));
assert!(!in_order.contains("enqueue_fetch_recorded"));
assert!(!enqueue.contains("advance_cycle"));
```

- [ ] **Step 2: Run the tests and verify they fail for the missing authority**

Run:

```text
cargo test -p rem6-cpu --lib riscv_in_order_drive::tests::enqueue_fetch_never_advances_time_and_rejects_full_fetch1 -- --exact
cargo test -p rem6-cpu --lib riscv_in_order_drive::tests::fetch_admission_distinguishes_available_advance_and_retire_states -- --exact
cargo test -p rem6-cpu --test source_policy normal_riscv_drivers_delegate_pipeline_time_to_focused_scheduler_authority -- --exact
```

Expected: compilation or assertion failure because `StageAtCapacity`,
`RiscvInOrderFetchAdmission`, and the pure enqueue boundary do not exist yet.

- [ ] **Step 3: Add the typed capacity error**

Add this variant to `InOrderPipelineError`:

```rust
StageAtCapacity {
    stage: InOrderPipelineStage,
    width: usize,
},
```

Add this `Display` arm:

```rust
Self::StageAtCapacity { stage, width } => write!(
    formatter,
    "in-order {stage} stage is at its configured width {width}"
),
```

- [ ] **Step 4: Replace recorded enqueue with pure capacity-checked enqueue**

Replace the current enqueue methods in `in_order_pipeline.rs` with:

```rust
pub(crate) fn fetch1_has_slot(&self) -> bool {
    let occupancy = self
        .in_flight
        .iter()
        .filter(|instruction| instruction.stage() == InOrderPipelineStage::Fetch1)
        .count();
    occupancy < self.config.width(InOrderPipelineStage::Fetch1)
}

pub(crate) fn commit_is_occupied(&self) -> bool {
    self.in_flight
        .iter()
        .any(|instruction| instruction.stage() == InOrderPipelineStage::Commit)
}

pub fn enqueue_fetch(&mut self, sequence: u64) -> Result<(), InOrderPipelineError> {
    if self.contains_sequence(sequence) {
        return Ok(());
    }
    if !self.fetch1_has_slot() {
        return Err(InOrderPipelineError::StageAtCapacity {
            stage: InOrderPipelineStage::Fetch1,
            width: self.config.width(InOrderPipelineStage::Fetch1),
        });
    }
    self.in_flight.push(InOrderPipelineInstruction::new(
        sequence,
        InOrderPipelineStage::Fetch1,
    ));
    self.in_flight = canonical_in_flight(self.in_flight.iter().copied())?;
    Ok(())
}
```

Delete `enqueue_fetch_recorded` entirely.

- [ ] **Step 5: Add the focused admission state**

Add this enum near `RiscvInOrderDriveStatus`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RiscvInOrderFetchAdmission {
    Admitted,
    PipelineCyclePending,
    AdvanceBeforeFetch,
    RetireBeforeFetch,
}

impl RiscvInOrderFetchAdmission {
    pub(crate) const fn allows_fetch(self) -> bool {
        matches!(self, Self::Admitted)
    }
}
```

Add this method to `impl RiscvCore` before the pipeline scheduling methods:

```rust
pub(crate) fn in_order_fetch_admission(&self) -> RiscvInOrderFetchAdmission {
    let state = self.state.lock().expect("riscv core lock");
    if state.pending_in_order_pipeline_advance.is_some() {
        return RiscvInOrderFetchAdmission::PipelineCyclePending;
    }
    if state.pending_fetch_prefix.is_some() || state.in_order_pipeline.fetch1_has_slot() {
        return RiscvInOrderFetchAdmission::Admitted;
    }
    if state.in_order_pipeline.commit_is_occupied() {
        RiscvInOrderFetchAdmission::RetireBeforeFetch
    } else {
        RiscvInOrderFetchAdmission::AdvanceBeforeFetch
    }
}
```

- [ ] **Step 6: Run focused tests and source policy**

Run:

```text
cargo test -p rem6-cpu --lib riscv_in_order_drive::tests -- --nocapture
cargo test -p rem6-cpu --test source_policy normal_riscv_drivers_delegate_pipeline_time_to_focused_scheduler_authority -- --nocapture
```

Expected: all selected tests pass.

- [ ] **Step 7: Commit the explicit authority**

```text
git add crates/rem6-cpu/src/in_order_pipeline/error.rs crates/rem6-cpu/src/in_order_pipeline.rs crates/rem6-cpu/src/riscv_in_order_drive.rs crates/rem6-cpu/src/riscv_in_order_drive_tests.rs crates/rem6-cpu/tests/source_policy.rs
git commit -m "cpu: make fetch admission explicit"
```

### Task 2: Defer Low-Level Fetch Rows At Capacity

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_execute.rs:1325-1410`
- Test: `crates/rem6-cpu/tests/riscv_in_order_timing.rs:1039-1100`

- [ ] **Step 1: Add width-one deferral and restore width-two direct overlap tests**

Add the uniform config helper to `riscv_in_order_timing.rs` imports and helpers:

```rust
fn uniform_in_order_pipeline_config(width: usize) -> InOrderPipelineConfig {
    InOrderPipelineConfig::new(
        InOrderPipelineStage::ALL
            .map(|stage| InOrderPipelineStageWidth::new(stage, width).unwrap()),
    )
    .unwrap()
}
```

Add this test before the existing completed-fetch overlap test:

```rust
#[test]
fn riscv_direct_fetch_issue_defers_younger_row_without_advancing_time() {
    let (mut scheduler, transport, fetch_route, _) = in_order_routes();
    let core = RiscvCore::new(core(fetch_route, 0x8000));
    core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(1));
    let store = loaded_program(
        0x8000,
        &[
            i_type(5, 0, 0x0, 1, 0x13),
            i_type(7, 1, 0x0, 2, 0x13),
        ],
    );

    fetch_one(&core, Arc::clone(&store), &mut scheduler, &transport);
    fetch_one(&core, store, &mut scheduler, &transport);

    let snapshot = core.in_order_pipeline_snapshot();
    assert_eq!(snapshot.cycle(), 0);
    assert_eq!(
        snapshot.in_flight(),
        &[InOrderPipelineInstruction::new(0, InOrderPipelineStage::Fetch1)]
    );
    assert!(core.in_order_pipeline_cycle_records().is_empty());

    let first = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(first.fetch_pc(), Address::new(0x8000));
    let second = core.execute_next_completed_fetch().unwrap().unwrap();
    assert_eq!(second.fetch_pc(), Address::new(0x8004));
}
```

In `riscv_completed_fetches_overlap_in_order_pipeline_before_retire`, configure
width two and assert two Fetch1 rows immediately after the two direct issues:

```rust
core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(2));
// issue both fetches
assert_eq!(
    core.in_order_pipeline_snapshot().in_flight(),
    &[
        InOrderPipelineInstruction::new(0, InOrderPipelineStage::Fetch1),
        InOrderPipelineInstruction::new(1, InOrderPipelineStage::Fetch1),
    ]
);
assert!(core.in_order_pipeline_cycle_records().is_empty());
```

Update the later expected stage for sequence 1 from Commit to the state produced
by the retirement cycle, preserving the assertion that the younger instruction
does not retire with the older one.

- [ ] **Step 2: Run the new width-one test and verify the current synchronous cycle fails it**

Run:

```text
cargo test -p rem6-cpu --test riscv_in_order_timing riscv_direct_fetch_issue_defers_younger_row_without_advancing_time -- --exact --nocapture
```

Expected: FAIL because the second direct issue currently advances cycle zero and
inserts sequence 1 immediately.

- [ ] **Step 3: Add one capacity-aware synchronization helper**

Add this helper immediately before `sync_in_order_fetch_state`:

```rust
fn enqueue_in_order_fetch_if_available(
    state: &mut RiscvCoreState,
    sequence: u64,
) -> Result<bool, RiscvCpuError> {
    if state.in_order_pipeline.contains_sequence(sequence) {
        return Ok(true);
    }
    if !state.in_order_pipeline.fetch1_has_slot() {
        return Ok(false);
    }
    state
        .in_order_pipeline
        .enqueue_fetch(sequence)
        .map_err(RiscvCpuError::InOrderPipeline)?;
    Ok(true)
}
```

Replace both `enqueue_fetch_recorded` call sites in
`sync_in_order_fetch_state`:

```rust
if let Some(sequence) = state
    .pending_fetch_prefix
    .as_ref()
    .map(|prefix| prefix.fetch.request_id().sequence())
    .filter(|sequence| !state.in_order_pipeline.contains_sequence(*sequence))
{
    if !enqueue_in_order_fetch_if_available(state, sequence)? {
        return Ok(());
    }
}

for fetch in fetches {
    if !enqueue_in_order_fetch_if_available(state, fetch.request_id().sequence())? {
        break;
    }
}
```

Do not create or append any cycle record in this function.

- [ ] **Step 4: Run direct API and execute-wait rebind regressions**

Run:

```text
cargo test -p rem6-cpu --test riscv_in_order_timing riscv_direct_fetch_issue_defers_younger_row_without_advancing_time -- --nocapture
cargo test -p rem6-cpu --test riscv_in_order_timing riscv_completed_fetches_overlap_in_order_pipeline_before_retire -- --nocapture
cargo test -p rem6-cpu --lib riscv_in_order_drive::tests::keyed_head_execute_wait_rebind_preserves_progress_with_orphaned_younger_row -- --exact
```

Expected: all selected tests pass.

- [ ] **Step 5: Commit deferred synchronization**

```text
git add crates/rem6-cpu/src/riscv_execute.rs crates/rem6-cpu/tests/riscv_in_order_timing.rs
git commit -m "cpu: defer fetch rows at pipeline capacity"
```

### Task 3: Restore Serial And Translated Width

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_drive.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation.rs:1013-1130`
- Test: `crates/rem6-cpu/tests/riscv_frontend.rs:5750-5805`
- Test: `crates/rem6-cpu/tests/riscv_translation_fetch_ahead.rs`
- Test: `crates/rem6/tests/cli_run/stats_compat.rs:1828-1865,13607-13639,14210-14380`

- [ ] **Step 1: Restore raw serial width assertions**

Change `riscv_core_driver_in_order_width_allows_frontend_overlap_without_false_retire`
to use `drive_raw_action` for the second action and restore the immediate state:

```rust
assert!(matches!(
    drive_raw_action(&core, store.clone(), &mut scheduler, &transport),
    Some(RiscvCoreDriveAction::FetchIssued { .. })
));
assert_eq!(
    in_order_in_flight(&core),
    vec![
        (0, InOrderPipelineStage::Fetch1),
        (1, InOrderPipelineStage::Fetch1),
    ]
);
assert!(core.in_order_pipeline_cycle_records().is_empty());
```

Add this width-one sibling:

```rust
#[test]
fn riscv_core_driver_width_one_advances_before_second_fetch() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = data_core(fetch_route, data_route, 0x8000);
    core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(1));
    core.set_branch_lookahead(2);
    let store = loaded_program_store(
        0x8000,
        &[
            i_type(7, 0, 0x0, 1, 0x13),
            i_type(9, 0, 0x0, 2, 0x13),
        ],
        &[],
    );

    assert!(matches!(
        drive_raw_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_raw_action(&core, store.clone(), &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::PipelineCycleScheduled { .. })
    ));
    scheduler.run_until_idle_conservative();

    assert!(matches!(
        drive_raw_action(&core, store, &mut scheduler, &transport),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    assert_eq!(
        in_order_in_flight(&core),
        vec![
            (0, InOrderPipelineStage::Fetch2),
            (1, InOrderPipelineStage::Fetch1),
        ]
    );
}
```

- [ ] **Step 2: Add translated serial width-two evidence**

Import `InOrderPipelineConfig`, `InOrderPipelineStage`, and
`InOrderPipelineStageWidth` from `rem6_cpu`, and import `SchedulerContext` from
`rem6_kernel`. Add these helpers:

```rust
fn uniform_in_order_pipeline_config(width: usize) -> InOrderPipelineConfig {
    InOrderPipelineConfig::new(
        InOrderPipelineStage::ALL
            .map(|stage| InOrderPipelineStageWidth::new(stage, width).unwrap()),
    )
    .unwrap()
}

fn translated_responder(
    store: Arc<Mutex<PartitionedMemoryStore>>,
) -> impl FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static {
    move |delivery, _context| {
        let response = store
            .lock()
            .unwrap()
            .respond(delivery.request())
            .unwrap()
            .response()
            .cloned()
            .unwrap();
        TargetOutcome::Respond(response)
    }
}
```

Add this test:

```rust
#[test]
fn riscv_core_translated_driver_admits_width_two_before_pipeline_cycle() {
    let (mut scheduler, transport, fetch_route, data_route) = data_routes();
    let core = translated_data_core(fetch_route, data_route, 0x8000);
    core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(2));
    core.set_branch_lookahead(2);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = loaded_program_store(
        0x8000,
        &[i_type(7, 0, 0x0, 1, 0x13), i_type(9, 0, 0x0, 2, 0x13)],
    );

    assert!(matches!(
        drive_one_translated_action(&core, store.clone(), &mut scheduler, &transport, &page_map),
        Some(RiscvCoreDriveAction::FetchIssued { .. })
    ));
    scheduler.run_until_idle_conservative();

    let action = core
        .drive_next_action_with_data_translation(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            translated_responder(store.clone()),
            translated_responder(store),
        )
        .unwrap();
    assert!(matches!(action, Some(RiscvCoreDriveAction::FetchIssued { .. })));
    assert_eq!(
        core.in_order_pipeline_snapshot()
            .in_flight()
            .iter()
            .map(|row| (row.sequence(), row.stage()))
            .collect::<Vec<_>>(),
        vec![
            (0, InOrderPipelineStage::Fetch1),
            (1, InOrderPipelineStage::Fetch1),
        ]
    );
}
```

- [ ] **Step 3: Restore failing top-level width and movement assertions**

Rename the CLI test back to
`rem6_run_in_order_pipeline_width_changes_executed_stage_occupancy` and restore
the width-two expected value to 2 for all stages.

Replace the movement equality with:

```rust
assert!(
    stage_advanced
        .iter()
        .zip(stage_advanced_cycles.iter())
        .any(|(advanced, cycles)| advanced > cycles),
    "widened pipeline should distinguish movement counts from cycle presence: advanced={stage_advanced:?} cycles={stage_advanced_cycles:?}"
);
```

Generalize the CLI helper:

```rust
fn in_order_pipeline_stats_for_width_and_memory(
    path: &std::path::Path,
    width: u64,
    memory_system: &str,
    max_tick: u64,
) -> String {
    let width = width.to_string();
    let max_tick = max_tick.to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            &max_tick,
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            memory_system,
            "--riscv-branch-lookahead",
            "2",
            "--riscv-in-order-width",
            &width,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"committed_instructions\":5"));
    stdout
}
```

Use direct width-one/direct width-two rows and add a
`cache-fabric-dram` width-two row with `max_tick = 320`; assert all five stage
maxima are 2. Assert these instruction-path resource counters are nonzero with
the existing `assert_stat_greater_than` helper:

```text
sim.memory.resources.cache.instruction.activity
sim.memory.resources.transport.fetch.activity
sim.memory.resources.fabric.activity
sim.memory.resources.dram.activity
```

- [ ] **Step 4: Run serial, translated, and CLI tests to verify they fail**

Run:

```text
cargo test -p rem6-cpu --test riscv_frontend riscv_core_driver_in_order_width -- --nocapture
cargo test -p rem6-cpu --test riscv_translation_fetch_ahead riscv_core_translated_driver_admits_width_two_before_pipeline_cycle -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_in_order_pipeline_width_changes_executed_stage_occupancy -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_text_stats_emit_in_order_stage_movement_aliases -- --nocapture
```

Expected: the immediate width-two actions are pipeline cycles, CLI stage maxima
remain one, and movement counts equal movement-cycle counts.

- [ ] **Step 5: Reorder the normal serial driver**

Import `RiscvInOrderFetchAdmission` with `RiscvInOrderDriveStatus`. Replace the
current pipeline-before-fetch block with this ordering:

```rust
let detailed_o3_fetch = self.detailed_o3_window_prefers_fetch_ahead();
let pending_o3_scalar_memory_retirement = self.has_pending_o3_scalar_memory_retirement();
if !detailed_o3_fetch && pending_o3_scalar_memory_retirement {
    return Ok(None);
}
let fetch_admission = if detailed_o3_fetch {
    RiscvInOrderFetchAdmission::Admitted
} else {
    self.in_order_fetch_admission()
};

if fetch_admission.allows_fetch() {
    if let Some(decision) = self.next_fetch_ahead_before_retire() {
        let fetch_ahead = self.prepare_fetch_ahead_speculation(&decision)?;
        self.set_fetch_ahead_pc(decision.pc());
        let event = self.issue_next_fetch_with_prepared_fetch_ahead(
            scheduler,
            transport,
            fetch_trace,
            fetch_responder,
            fetch_ahead,
        )?;
        return Ok(Some(RiscvCoreDriveAction::FetchIssued { event }));
    }
}

if !detailed_o3_fetch {
    match self.schedule_next_completed_fetch_pipeline_cycle_serial(scheduler)? {
        RiscvInOrderDriveStatus::Scheduled(event) => {
            return Ok(Some(RiscvCoreDriveAction::PipelineCycleScheduled { event }));
        }
        RiscvInOrderDriveStatus::Pending => return Ok(None),
        RiscvInOrderDriveStatus::Ready if self.live_retire_gate_blocks_new_work() => {
            return self.drive_next_completed_fetch_serial_action(scheduler);
        }
        RiscvInOrderDriveStatus::Unavailable if self.live_retire_gate_blocks_new_work() => {
            return Ok(None);
        }
        RiscvInOrderDriveStatus::Unavailable | RiscvInOrderDriveStatus::Ready => {}
        RiscvInOrderDriveStatus::Reserved { .. } => {
            unreachable!("pipeline reservation is scheduled before returning")
        }
    }
}
```

After the completed-fetch drive and live-gate checks, add:

```rust
if !fetch_admission.allows_fetch() {
    return Ok(None);
}
```

before the ordinary `issue_next_fetch` call.

- [ ] **Step 6: Apply the explicit ordering to translated serial execution**

Import `RiscvInOrderFetchAdmission` and replace the translated
pipeline-before-fetch block with:

```rust
let detailed_o3_fetch = self.detailed_o3_window_prefers_fetch_ahead();
let pending_o3_scalar_memory_retirement = self.has_pending_o3_scalar_memory_retirement();
if !detailed_o3_fetch && pending_o3_scalar_memory_retirement {
    return Ok(None);
}
let fetch_admission = if detailed_o3_fetch {
    RiscvInOrderFetchAdmission::Admitted
} else {
    self.in_order_fetch_admission()
};

if fetch_admission.allows_fetch() {
    if let Some(decision) = self.next_cached_translated_memory_fetch_ahead_before_retire() {
        let fetch_ahead = self.prepare_fetch_ahead_speculation(&decision)?;
        self.set_fetch_ahead_pc(decision.pc());
        let event = self.issue_next_fetch_with_prepared_fetch_ahead(
            scheduler,
            transport,
            fetch_trace,
            fetch_responder,
            fetch_ahead,
        )?;
        return Ok(Some(RiscvCoreDriveAction::FetchIssued { event }));
    }
}

if !detailed_o3_fetch {
    match self.schedule_next_completed_fetch_pipeline_cycle_serial(scheduler)? {
        RiscvInOrderDriveStatus::Scheduled(event) => {
            return Ok(Some(RiscvCoreDriveAction::PipelineCycleScheduled { event }));
        }
        RiscvInOrderDriveStatus::Pending => return Ok(None),
        RiscvInOrderDriveStatus::Ready if self.live_retire_gate_blocks_new_work() => {
            return self.drive_next_completed_fetch_serial_action(scheduler);
        }
        RiscvInOrderDriveStatus::Unavailable if self.live_retire_gate_blocks_new_work() => {
            return Ok(None);
        }
        RiscvInOrderDriveStatus::Unavailable | RiscvInOrderDriveStatus::Ready => {}
        RiscvInOrderDriveStatus::Reserved { .. } => {
            unreachable!("pipeline reservation is scheduled before returning")
        }
    }
}
```

After `drive_next_completed_fetch_serial_action` and the live-gate check, add:

```rust
if !fetch_admission.allows_fetch() {
    return Ok(None);
}
```

before the ordinary translated `issue_next_fetch` call.

- [ ] **Step 7: Run the serial matrix and cleanup regressions**

Run:

```text
cargo test -p rem6-cpu --test riscv_frontend riscv_core_driver_in_order_width -- --nocapture
cargo test -p rem6-cpu --test riscv_translation_fetch_ahead -- --nocapture
cargo test -p rem6-cpu --test riscv_frontend riscv_core_driver_removes_ -- --nocapture
cargo test -p rem6-cpu --test riscv_frontend riscv_core_retries_word_fetch_suffix_across_line_end -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_in_order_pipeline_width_changes_executed_stage_occupancy -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_text_stats_emit_in_order_stage_movement_aliases -- --nocapture
```

Expected: all selected tests pass, including direct and hierarchy CLI rows.

- [ ] **Step 8: Commit serial width behavior**

```text
git add crates/rem6-cpu/src/riscv_drive.rs crates/rem6-cpu/src/riscv_translation.rs crates/rem6-cpu/tests/riscv_frontend.rs crates/rem6-cpu/tests/riscv_translation_fetch_ahead.rs crates/rem6/tests/cli_run/stats_compat.rs
git commit -m "cpu: restore in-order fetch width"
```

### Task 4: Apply Admission To Every Parallel Driver

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_cluster_drive.rs:417-500`
- Modify: `crates/rem6-cpu/src/riscv_cluster.rs:280-1235`
- Test: `crates/rem6-cpu/tests/riscv_cluster.rs`
- Test: `crates/rem6-cpu/tests/riscv_cluster_translation.rs`

- [ ] **Step 1: Add prepared-parallel width-two evidence**

Import `InOrderPipelineConfig`, `InOrderPipelineStage`, and
`InOrderPipelineStageWidth` and add this helper in `riscv_cluster.rs`:

```rust
fn uniform_in_order_pipeline_config(width: usize) -> InOrderPipelineConfig {
    InOrderPipelineConfig::new(
        InOrderPipelineStage::ALL
            .map(|stage| InOrderPipelineStageWidth::new(stage, width).unwrap()),
    )
    .unwrap()
}
```

Add this test beside
`riscv_cluster_parallel_fetch_retires_completed_fetch_while_fetch_ahead_is_pending`:

```rust
#[test]
fn riscv_cluster_parallel_fetch_admits_width_two_before_pipeline_cycle() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let core = cluster.core(CpuId::new(0)).unwrap();
    core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(2));
    core.set_branch_lookahead(2);
    let store = store_with_programs(&[
        (0x8000, i_type(3, 0, 0x0, 5, 0x13)),
        (0x8004, i_type(4, 0, 0x0, 6, 0x13)),
    ]);

    let first = cluster
        .drive_ready_cores_parallel_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
        )
        .unwrap();
    assert!(matches!(first[0].action(), RiscvCoreDriveAction::FetchIssued { .. }));
    scheduler.run_until_idle_parallel().unwrap();

    let second = cluster
        .drive_ready_cores_parallel_fetch(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
        )
        .unwrap();
    assert!(matches!(second[0].action(), RiscvCoreDriveAction::FetchIssued { .. }));
    assert_eq!(
        core.in_order_pipeline_snapshot()
            .in_flight()
            .iter()
            .map(|row| (row.sequence(), row.stage()))
            .collect::<Vec<_>>(),
        vec![
            (0, InOrderPipelineStage::Fetch1),
            (1, InOrderPipelineStage::Fetch1),
        ]
    );
}
```

- [ ] **Step 2: Add translated-parallel width-two evidence**

In `riscv_cluster_translation.rs`, import the three in-order config types, add
the same `uniform_in_order_pipeline_config` implementation shown in Step 1, and
add this test:

```rust
#[test]
fn riscv_cluster_parallel_translation_admits_width_two_before_pipeline_cycle() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let data_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.dmem"),
                PartitionId::new(0),
                endpoint("l1d0"),
                PartitionId::new(1),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cluster = RiscvCluster::new([translated_riscv_core(CoreSpec {
        cpu: 0,
        partition: 0,
        agent: 7,
        entry: 0x8000,
        fetch_endpoint: "cpu0.ifetch",
        fetch_route,
        data_endpoint: "cpu0.dmem",
        data_route,
    })])
    .unwrap();
    let core = cluster.core(CpuId::new(0)).unwrap();
    core.reset_in_order_pipeline_config(uniform_in_order_pipeline_config(2));
    core.set_branch_lookahead(2);
    let page_map = single_page_map(0x4000, 0x9000);
    let store = store_with_programs_and_data(
        &[
            (0x8000, i_type(3, 0, 0x0, 5, 0x13)),
            (0x8004, i_type(4, 0, 0x0, 6, 0x13)),
        ],
        &[],
    );

    let first = cluster
        .drive_ready_cores_parallel_with_data_translation(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
        )
        .unwrap();
    assert!(matches!(first[0].action(), RiscvCoreDriveAction::FetchIssued { .. }));
    scheduler.run_until_idle_parallel().unwrap();

    let second = cluster
        .drive_ready_cores_parallel_with_data_translation(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            &page_map,
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
            |_cpu| {
                let store = store.clone();
                move |delivery, _context| memory_response(&store, &delivery)
            },
        )
        .unwrap();
    assert!(matches!(second[0].action(), RiscvCoreDriveAction::FetchIssued { .. }));
    assert_eq!(
        core.in_order_pipeline_snapshot()
            .in_flight()
            .iter()
            .map(|row| (row.sequence(), row.stage()))
            .collect::<Vec<_>>(),
        vec![
            (0, InOrderPipelineStage::Fetch1),
            (1, InOrderPipelineStage::Fetch1),
        ]
    );
}
```

- [ ] **Step 3: Run both tests and verify they fail before parallel reordering**

Run:

```text
cargo test -p rem6-cpu --test riscv_cluster riscv_cluster_parallel_fetch_admits_width_two_before_pipeline_cycle -- --nocapture
cargo test -p rem6-cpu --test riscv_cluster_translation riscv_cluster_parallel_translation_admits_width_two_before_pipeline_cycle -- --nocapture
```

Expected: FAIL because both second raw drives return a pipeline-cycle action.

- [ ] **Step 4: Add one cluster admission predicate**

Add this helper to `riscv_cluster_drive.rs` and export it to the sibling module:

```rust
pub(crate) fn fetch_before_pipeline_is_admitted(core: &RiscvCore) -> bool {
    core.detailed_o3_window_prefers_fetch_ahead()
        || (!core.has_pending_o3_scalar_memory_retirement()
            && core.in_order_fetch_admission().allows_fetch())
}
```

Import it in `riscv_cluster.rs` with the existing cluster-drive helpers.

- [ ] **Step 5: Reorder prepared parallel loops**

In each prepared loop below, compute admission before either fetch-ahead or
pipeline scheduling:

```rust
let fetch_admitted = fetch_before_pipeline_is_admitted(core);
```

For the three untranslated prepared methods, move the existing fetch-ahead
block before `push_prepared_pipeline_cycle_drive_event` and wrap it exactly as:

```rust
if fetch_admitted {
    if let Some(decision) = core.next_fetch_ahead_before_retire() {
        let fetch_ahead = prepare_fetch_ahead_speculation(*cpu, core, &decision)?;
        core.set_fetch_ahead_pc(decision.pc());
        push_prepared_parallel_fetch_action(
            *cpu,
            core,
            scheduler.now(),
            transport,
            fetch_trace.clone(),
            fetch_responder(*cpu),
            &mut prepared_actions,
            &mut transaction_cpus,
            &mut transactions,
            fetch_ahead,
        )?;
        continue;
    }
}
```

For `drive_ready_cores_parallel_with_data_translation`, use this admitted block:

```rust
if fetch_admitted {
    if let Some(decision) = core.next_cached_translated_memory_fetch_ahead_before_retire() {
        let fetch_ahead = prepare_fetch_ahead_speculation(*cpu, core, &decision)?;
        core.set_fetch_ahead_pc(decision.pc());
        push_prepared_parallel_fetch_action(
            *cpu,
            core,
            scheduler.now(),
            transport,
            fetch_trace.clone(),
            fetch_responder(*cpu),
            &mut prepared_actions,
            &mut transaction_cpus,
            &mut transactions,
            fetch_ahead,
        )?;
        continue;
    }
}
```

For `drive_ready_cores_parallel_with_mmio_and_data_translation`, use:

```rust
if fetch_admitted {
    if let Some(decision) =
        core.next_mmio_aware_cached_translated_memory_fetch_ahead_before_retire(bus)
    {
        let fetch_ahead = prepare_fetch_ahead_speculation(*cpu, core, &decision)?;
        core.set_fetch_ahead_pc(decision.pc());
        push_prepared_parallel_fetch_action(
            *cpu,
            core,
            scheduler.now(),
            transport,
            fetch_trace.clone(),
            fetch_responder(*cpu),
            &mut prepared_actions,
            &mut transaction_cpus,
            &mut transactions,
            fetch_ahead,
        )?;
        continue;
    }
}
```

After each admitted block, invoke the existing
`push_prepared_pipeline_cycle_drive_event` block. Immediately before each
ordinary `push_prepared_parallel_fetch_action` path, add:

```rust
if !fetch_admitted {
    continue;
}
```

Apply this exact ordering in:

```text
drive_ready_cores_parallel_fetch
drive_ready_cores_parallel
drive_ready_cores_parallel_with_instruction_budget
drive_ready_cores_parallel_with_data_translation
drive_ready_cores_parallel_with_mmio_and_data_translation
```

For the instruction-budget method, define:

```rust
let fetch_admitted = !instruction_budget_exhausted
    && fetch_before_pipeline_is_admitted(core);
```

Keep both pipeline scheduling and fetch-ahead disabled when the budget is
exhausted. The fetch-ahead selector remains:

```text
next_fetch_ahead_before_retire
next_fetch_ahead_before_retire
next_fetch_ahead_before_retire
next_cached_translated_memory_fetch_ahead_before_retire
next_mmio_aware_cached_translated_memory_fetch_ahead_before_retire
```

in the listed method order.

- [ ] **Step 6: Reorder direct parallel loops**

In `drive_ready_cores_parallel_with_mmio`, compute
`fetch_admitted`, move this block before `push_pipeline_cycle_drive_event`, and
guard the ordinary parallel fetch after the completed-fetch path:

```rust
if fetch_admitted {
    if let Some(decision) = core.next_fetch_ahead_before_retire() {
        let fetch_ahead = prepare_fetch_ahead_speculation(*cpu, core, &decision)?;
        core.set_fetch_ahead_pc(decision.pc());
        let event = core
            .issue_next_fetch_parallel_with_prepared_fetch_ahead(
                scheduler,
                transport,
                fetch_trace.clone(),
                fetch_responder(*cpu),
                fetch_ahead,
            )
            .map_err(|error| RiscvClusterError::Core { cpu: *cpu, error })?;
        actions.push(RiscvClusterDriveEvent::new(
            *cpu,
            RiscvCoreDriveAction::FetchIssued { event },
        ));
        continue;
    }
}
```

Before the ordinary direct parallel fetch, add:

```rust
if !fetch_admitted {
    continue;
}
```

Apply that explicit structure in:

```text
drive_ready_cores_parallel_with_mmio
drive_ready_cores_parallel_with_mmio_and_instruction_budget
```

For the budgeted method define:

```rust
let fetch_admitted = !instruction_budget_exhausted
    && fetch_before_pipeline_is_admitted(core);
```

It uses `next_fetch_ahead_before_retire`, invokes
`push_pipeline_cycle_drive_event` only when the budget is available, and guards
the ordinary `issue_next_fetch_parallel_with_prepared_fetch_ahead` call with the
saved `fetch_admitted` value.

- [ ] **Step 7: Run parallel, failure, compressed, and translated regressions**

Run:

```text
cargo test -p rem6-cpu --test riscv_cluster riscv_cluster_parallel_fetch_admits_width_two_before_pipeline_cycle -- --nocapture
cargo test -p rem6-cpu --test riscv_cluster riscv_cluster_parallel_fetch_retires_completed_fetch_while_fetch_ahead_is_pending -- --nocapture
cargo test -p rem6-cpu --test riscv_cluster riscv_cluster_parallel_fetch_ahead_accepts_compressed_straight_line_instruction -- --nocapture
cargo test -p rem6-cpu --test riscv_cluster_translation -- --nocapture
cargo test -p rem6-cpu --lib riscv_cluster_drive::tests::failed_parallel_batch_cancels_prepared_pipeline_wake -- --exact
```

Expected: all selected tests pass.

- [ ] **Step 8: Commit parallel coverage**

```text
git add crates/rem6-cpu/src/riscv_cluster_drive.rs crates/rem6-cpu/src/riscv_cluster.rs crates/rem6-cpu/tests/riscv_cluster.rs crates/rem6-cpu/tests/riscv_cluster_translation.rs
git commit -m "cpu: apply fetch admission to parallel drivers"
```

### Task 5: Close The Evidence And Documentation Boundary

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md:170-190`
- Verify: all files changed by Tasks 1-4

- [ ] **Step 1: Update the CPU ledger without changing score or line count**

Edit the existing CPU execution-model paragraphs in place. Add these executable
claims to the existing wrapped lines rather than adding document lines:

```text
configured width-one backpressure and width-two multi-row occupancy through Fetch1, Fetch2, Decode, Execute, and Commit in direct and cache/fabric/DRAM CLI rows
```

```text
width-three movement counts exceeding movement-cycle presence, with serial, translated, prepared-parallel, and translated-parallel focused admission coverage
```

Add the exact new test anchors to the existing `Migrated` prose. Keep:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw,
```

Do not change either unchecked checklist item.

- [ ] **Step 2: Format and run focused policy gates**

Run:

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu --test source_policy
cargo test -p rem6 --test source_policy
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: formatting passes, both policy suites pass, and the ledger is exactly
1200 lines.

- [ ] **Step 3: Run complete CPU and CLI regression groups**

Run:

```text
cargo test -p rem6-cpu --lib
cargo test -p rem6-cpu --tests
cargo test -p rem6 --test cli_run
```

Expected: all tests pass with no ignored failure relevant to this increment.

- [ ] **Step 4: Run the full workspace verification**

Run:

```text
cargo test --workspace
```

Expected: every workspace test target passes.

- [ ] **Step 5: Perform the mandatory read-only review**

Use `superpowers:requesting-code-review` and dispatch a high-intensity read-only
review covering:

```text
- duplicate or hidden timing authority
- every serial/translated/parallel driver ordering
- width-one, Commit-blocked, prefix, retry, failure, redirect, and O3 suppression
- dead API or compatibility shim residue
- source line limits and migration-ledger honesty
```

Address every confirmed finding and rerun the affected focused tests plus
`cargo test --workspace` after the final edit.

- [ ] **Step 6: Commit documentation and any review corrections**

```text
git add docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record in-order width evidence"
```

If review corrections touched implementation files after their earlier commits,
stage those exact files in this commit and use:

```text
git commit -m "cpu: harden in-order fetch admission"
```

- [ ] **Step 7: Verify final repository state and push**

Run:

```text
git status --short --branch
git log -5 --oneline
git push origin main
git status --short --branch
```

Expected: the pre-push branch is ahead only by the planned commits, push
succeeds, and the final status is clean with `main...origin/main`.
