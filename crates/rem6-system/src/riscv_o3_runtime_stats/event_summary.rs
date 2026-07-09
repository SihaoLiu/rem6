use std::collections::BTreeMap;

use rem6_cpu::O3RuntimeTraceRecord;
use rem6_stats::{StatId, StatsError, StatsRegistry};

use super::helpers::register_o3_counter;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeEventSummarySnapshot {
    events: BTreeMap<u64, O3RuntimeTraceRecord>,
}

impl RiscvO3RuntimeEventSummarySnapshot {
    pub(super) fn observe(&mut self, event: &O3RuntimeTraceRecord) {
        self.events.insert(event.sequence(), *event);
    }

    fn records(&self) -> u64 {
        self.events.len() as u64
    }

    fn span_ticks(&self) -> u64 {
        let first = self.events.values().next().map_or(0, |event| event.tick());
        let last = self
            .events
            .values()
            .next_back()
            .map_or(0, |event| event.tick());
        last.saturating_sub(first)
    }

    fn rob_allocations(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::rob_allocated)
    }

    fn rename_writes(&self) -> u64 {
        self.sum(O3RuntimeTraceRecord::rename_writes)
    }

    fn branch_event_mispredictions(&self) -> u64 {
        self.events
            .values()
            .filter(|event| event.branch_event() && event.branch_mispredicted())
            .count() as u64
    }

    fn branch_repair_direction_only_mismatches(&self) -> u64 {
        self.events
            .values()
            .filter(|event| {
                event.branch_event()
                    && !branch_targetless_mismatch(event)
                    && !branch_wrong_target(event)
                    && event.branch_predicted_taken() != event.branch_resolved_taken()
            })
            .count() as u64
    }

    fn fu_latency_instructions(&self) -> u64 {
        self.events
            .values()
            .filter(|event| event.fu_latency_cycles() != 0)
            .count() as u64
    }

    fn count<F>(&self, matches: F) -> u64
    where
        F: Fn(O3RuntimeTraceRecord) -> bool,
    {
        self.events
            .values()
            .copied()
            .filter(|event| matches(*event))
            .count() as u64
    }

    fn sum<F>(&self, value: F) -> u64
    where
        F: Fn(O3RuntimeTraceRecord) -> u64,
    {
        self.events.values().copied().map(value).sum()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeEventSummaryStats {
    records: StatId,
    span_ticks: StatId,
    rob_allocations: StatId,
    rename_writes: StatId,
    branch_event_mispredictions: StatId,
    branch_repair_direction_only_mismatches: StatId,
    iew_branch_mispredicts: StatId,
    fu_latency_instructions: StatId,
}

impl RiscvO3RuntimeEventSummaryStats {
    pub(super) fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        let prefix = format!("{prefix}.event_summary");
        Ok(Self {
            records: register_o3_counter(registry, &prefix, "records", "Count")?,
            span_ticks: register_o3_counter(registry, &prefix, "span_ticks", "Tick")?,
            rob_allocations: register_o3_counter(registry, &prefix, "rob.allocations", "Count")?,
            rename_writes: register_o3_counter(registry, &prefix, "rename.writes", "Count")?,
            branch_event_mispredictions: register_o3_counter(
                registry,
                &prefix,
                "branch_event.mispredictions",
                "Count",
            )?,
            branch_repair_direction_only_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_repair.direction_only_mismatches",
                "Count",
            )?,
            iew_branch_mispredicts: register_o3_counter(
                registry,
                &prefix,
                "iew.branch_mispredicts",
                "Count",
            )?,
            fu_latency_instructions: register_o3_counter(
                registry,
                &prefix,
                "fu_latency.instructions",
                "Count",
            )?,
        })
    }

    pub(super) fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: &RiscvO3RuntimeEventSummarySnapshot,
    ) -> Result<(), StatsError> {
        let branch_mispredicts = snapshot.branch_event_mispredictions();
        for (stat, value) in [
            (self.records, snapshot.records()),
            (self.span_ticks, snapshot.span_ticks()),
            (self.rob_allocations, snapshot.rob_allocations()),
            (self.rename_writes, snapshot.rename_writes()),
            (self.branch_event_mispredictions, branch_mispredicts),
            (
                self.branch_repair_direction_only_mismatches,
                snapshot.branch_repair_direction_only_mismatches(),
            ),
            (self.iew_branch_mispredicts, branch_mispredicts),
            (
                self.fu_latency_instructions,
                snapshot.fu_latency_instructions(),
            ),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        Ok(())
    }
}

fn branch_targetless_mismatch(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_predicted_target().is_some() && event.branch_resolved_target().is_none()
}

fn branch_wrong_target(event: &O3RuntimeTraceRecord) -> bool {
    event
        .branch_predicted_target()
        .zip(event.branch_resolved_target())
        .is_some_and(|(predicted, resolved)| predicted != resolved)
}
