use rem6_cpu::{
    BranchTargetKind, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeTraceRecord,
};

use super::o3_branch_direction_mismatch::o3_branch_direction_mismatch_to_json;
use super::o3_branch_repair::o3_branch_repair_events_to_json;
use super::o3_branch_target_mismatch::o3_branch_target_mismatch_to_json;
use super::o3_event_iew::Rem6O3EventIewTotals;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3EventSummaryFuLatency {
    instructions: u64,
    cycles: u64,
    max_cycles: u64,
    min_cycles: u64,
}

impl O3EventSummaryFuLatency {
    fn add(&mut self, cycles: u64) {
        self.instructions = self.instructions.saturating_add(1);
        self.cycles = self.cycles.saturating_add(cycles);
        self.max_cycles = self.max_cycles.max(cycles);
        self.min_cycles = min_latency_cycles(self.min_cycles, cycles);
    }

    const fn avg_cycles(self) -> u64 {
        if self.instructions == 0 {
            0
        } else {
            self.cycles / self.instructions
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3EventSummaryLsqLatency {
    samples: u64,
    ticks: u64,
    max_ticks: u64,
    min_ticks: u64,
}

impl O3EventSummaryLsqLatency {
    fn add(&mut self, ticks: u64) {
        let previous_samples = self.samples;
        self.samples = self.samples.saturating_add(1);
        self.ticks = self.ticks.saturating_add(ticks);
        self.max_ticks = self.max_ticks.max(ticks);
        if previous_samples == 0 || ticks < self.min_ticks {
            self.min_ticks = ticks;
        }
    }

    const fn avg_ticks(self) -> u64 {
        if self.samples == 0 {
            0
        } else {
            self.ticks / self.samples
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3EventSummaryLsqForwarding {
    candidates: u64,
    matches: u64,
    suppressed: u64,
    address_mismatches: u64,
    byte_mismatches: u64,
}

impl O3EventSummaryLsqForwarding {
    fn add_event(&mut self, event: O3RuntimeTraceRecord) {
        self.candidates = self
            .candidates
            .saturating_add(u64::from(event.store_load_forwarding_candidate()));
        self.matches = self
            .matches
            .saturating_add(u64::from(event.store_load_forwarding_match()));
        self.suppressed = self
            .suppressed
            .saturating_add(u64::from(event.store_load_forwarding_suppressed()));
        self.address_mismatches = self
            .address_mismatches
            .saturating_add(u64::from(event.store_load_forwarding_address_mismatch()));
        self.byte_mismatches = self
            .byte_mismatches
            .saturating_add(u64::from(event.store_load_forwarding_byte_mismatch()));
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3EventSummaryBranchEvent {
    branches: u64,
    taken: u64,
    predicted_taken: u64,
    predicted_targets: u64,
    predicted_target_matches: u64,
    predicted_target_mismatches: u64,
    resolved_targets: u64,
    mispredictions: u64,
    squashes: u64,
    link_writes: u64,
    squashed_targets: u64,
    squashed_target_without_link_writes: u64,
    kinds: [u64; BranchTargetKind::COUNT],
    taken_kinds: [u64; BranchTargetKind::COUNT],
    not_taken_kinds: [u64; BranchTargetKind::COUNT],
    predicted_taken_kinds: [u64; BranchTargetKind::COUNT],
    predicted_not_taken_kinds: [u64; BranchTargetKind::COUNT],
    predicted_target_kinds: [u64; BranchTargetKind::COUNT],
    predicted_target_match_kinds: [u64; BranchTargetKind::COUNT],
    predicted_target_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    resolved_target_kinds: [u64; BranchTargetKind::COUNT],
    misprediction_kinds: [u64; BranchTargetKind::COUNT],
    squash_kinds: [u64; BranchTargetKind::COUNT],
    link_write_kinds: [u64; BranchTargetKind::COUNT],
    squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    squashed_target_link_write_kinds: [u64; BranchTargetKind::COUNT],
    squashed_target_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
}

impl O3EventSummaryBranchEvent {
    fn add_event(&mut self, event: O3RuntimeTraceRecord) {
        if !event.branch_event() {
            return;
        }

        let predicted_target_matches = event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) == event.branch_resolved_target());
        let predicted_target_mismatches = event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) != event.branch_resolved_target());
        let branch_kind = event.branch_kind();
        let index = branch_kind.index();
        let link_write = event.branch_link_register_write();
        let squashed_target = event.branch_squashed_target().is_some();
        let squashed_target_without_link_write = squashed_target && !link_write;

        self.branches = self.branches.saturating_add(1);
        self.taken = self
            .taken
            .saturating_add(u64::from(event.branch_resolved_taken()));
        self.predicted_taken = self
            .predicted_taken
            .saturating_add(u64::from(event.branch_predicted_taken()));
        self.predicted_targets = self
            .predicted_targets
            .saturating_add(u64::from(event.branch_predicted_target().is_some()));
        self.predicted_target_matches = self
            .predicted_target_matches
            .saturating_add(u64::from(predicted_target_matches));
        self.predicted_target_mismatches = self
            .predicted_target_mismatches
            .saturating_add(u64::from(predicted_target_mismatches));
        self.resolved_targets = self
            .resolved_targets
            .saturating_add(u64::from(event.branch_resolved_target().is_some()));
        self.mispredictions = self
            .mispredictions
            .saturating_add(u64::from(event.branch_mispredicted()));
        self.squashes = self
            .squashes
            .saturating_add(u64::from(event.branch_squash()));
        self.link_writes = self.link_writes.saturating_add(u64::from(link_write));
        self.squashed_targets = self
            .squashed_targets
            .saturating_add(u64::from(squashed_target));
        self.squashed_target_without_link_writes = self
            .squashed_target_without_link_writes
            .saturating_add(u64::from(squashed_target_without_link_write));

        self.kinds[index] = self.kinds[index].saturating_add(1);
        if event.branch_resolved_taken() {
            self.taken_kinds[index] = self.taken_kinds[index].saturating_add(1);
        } else {
            self.not_taken_kinds[index] = self.not_taken_kinds[index].saturating_add(1);
        }
        if event.branch_predicted_taken() {
            self.predicted_taken_kinds[index] = self.predicted_taken_kinds[index].saturating_add(1);
        } else {
            self.predicted_not_taken_kinds[index] =
                self.predicted_not_taken_kinds[index].saturating_add(1);
        }
        if event.branch_predicted_target().is_some() {
            self.predicted_target_kinds[index] =
                self.predicted_target_kinds[index].saturating_add(1);
        }
        if predicted_target_matches {
            self.predicted_target_match_kinds[index] =
                self.predicted_target_match_kinds[index].saturating_add(1);
        }
        if predicted_target_mismatches {
            self.predicted_target_mismatch_kinds[index] =
                self.predicted_target_mismatch_kinds[index].saturating_add(1);
        }
        if event.branch_resolved_target().is_some() {
            self.resolved_target_kinds[index] = self.resolved_target_kinds[index].saturating_add(1);
        }
        if event.branch_mispredicted() {
            self.misprediction_kinds[index] = self.misprediction_kinds[index].saturating_add(1);
        }
        if event.branch_squash() {
            self.squash_kinds[index] = self.squash_kinds[index].saturating_add(1);
        }
        if link_write {
            self.link_write_kinds[index] = self.link_write_kinds[index].saturating_add(1);
        }
        if squashed_target {
            self.squashed_target_kinds[index] = self.squashed_target_kinds[index].saturating_add(1);
        }
        if squashed_target && link_write {
            self.squashed_target_link_write_kinds[index] =
                self.squashed_target_link_write_kinds[index].saturating_add(1);
        }
        if squashed_target_without_link_write {
            self.squashed_target_without_link_write_kinds[index] =
                self.squashed_target_without_link_write_kinds[index].saturating_add(1);
        }
    }

    const fn not_taken(self) -> u64 {
        self.branches.saturating_sub(self.taken)
    }

    const fn predicted_not_taken(self) -> u64 {
        self.branches.saturating_sub(self.predicted_taken)
    }

    const fn without_link_writes(self) -> u64 {
        self.branches.saturating_sub(self.link_writes)
    }

    const fn squashed_targets_with_link_writes(self) -> u64 {
        self.squashed_targets
            .saturating_sub(self.squashed_target_without_link_writes)
    }
}

pub(crate) fn o3_event_summary_to_json(events: &[O3RuntimeTraceRecord]) -> String {
    let records = events.len() as u64;
    let first_tick = events.first().map_or(0, |event| event.tick());
    let last_tick = events.last().map_or(0, |event| event.tick());
    let max_rob_occupancy = events
        .iter()
        .map(|event| event.rob_occupancy())
        .max()
        .unwrap_or(0);
    let max_lsq_occupancy = events
        .iter()
        .map(|event| event.lsq_occupancy())
        .max()
        .unwrap_or(0);
    let max_rename_map_entries = events
        .iter()
        .map(|event| event.rename_map_entries())
        .max()
        .unwrap_or(0);
    let system_events = events.iter().filter(|event| event.system_event()).count() as u64;
    let rob_allocations = events.iter().filter(|event| event.rob_allocated()).count() as u64;
    let rob_commits = events.iter().filter(|event| event.rob_committed()).count() as u64;
    let rename_writes = events
        .iter()
        .map(|event| event.rename_writes())
        .sum::<u64>();
    let lsq_loads = events.iter().map(|event| event.lsq_loads()).sum::<u64>();
    let lsq_stores = events.iter().map(|event| event.lsq_stores()).sum::<u64>();
    let lsq_load_bytes = events
        .iter()
        .map(|event| event.lsq_load_bytes())
        .sum::<u64>();
    let lsq_store_bytes = events
        .iter()
        .map(|event| event.lsq_store_bytes())
        .sum::<u64>();
    let lsq_store_conditional_failures = events
        .iter()
        .filter(|event| event.lsq_store_conditional_failed())
        .count() as u64;
    let lsq_operation_load = events
        .iter()
        .filter(|event| event.lsq_operation() == O3RuntimeLsqOperation::Load)
        .count() as u64;
    let lsq_operation_store = events
        .iter()
        .filter(|event| event.lsq_operation() == O3RuntimeLsqOperation::Store)
        .count() as u64;
    let (fu_latency, fu_latency_classes) = event_summary_fu_latency(events);
    let fu_latency_class = event_summary_fu_latency_class_json(&fu_latency_classes);
    let (lsq_data_latency, lsq_operation_counts, lsq_operation_latencies) =
        event_summary_lsq_latency(events);
    let lsq_data_latency = event_summary_lsq_latency_json(lsq_data_latency);
    let (lsq_forwarding, lsq_operation_forwarding) = event_summary_lsq_forwarding(events);
    let lsq_operation = event_summary_lsq_operation_json(
        &lsq_operation_counts,
        &lsq_operation_latencies,
        &lsq_operation_forwarding,
    );
    let lsq_ordering = event_summary_lsq_ordering_json(events);
    let branch_event = event_summary_branch_event_json(events);
    let branch_repair = o3_branch_repair_events_to_json(events);
    let branch_direction_mismatch = o3_branch_direction_mismatch_to_json(events);
    let branch_target_mismatch = o3_branch_target_mismatch_to_json(events);
    let span_ticks = last_tick.saturating_sub(first_tick);
    let event_window = event_summary_window_json(events, records, span_ticks);
    let iq = event_summary_iq_json(events);
    let iew = event_summary_iew_json(events, span_ticks);
    let commit = event_summary_commit_json(events);
    let rob = format!(
        "{{\"allocations\":{rob_allocations},\"commits\":{rob_commits},\"max_occupancy\":{max_rob_occupancy}}}"
    );
    let rename = format!("{{\"writes\":{rename_writes},\"map_entries\":{max_rename_map_entries}}}");

    format!(
        "{{\"records\":{records},\"first_tick\":{first_tick},\"last_tick\":{last_tick},\"span_ticks\":{},\"event_window\":{event_window},\"max_rob_occupancy\":{max_rob_occupancy},\"max_lsq_occupancy\":{max_lsq_occupancy},\"max_rename_map_entries\":{max_rename_map_entries},\"system_events\":{system_events},\"rob_allocations\":{rob_allocations},\"rob_commits\":{rob_commits},\"rename_writes\":{rename_writes},\"rob\":{rob},\"rename\":{rename},\"lsq_loads\":{lsq_loads},\"lsq_stores\":{lsq_stores},\"lsq_load_bytes\":{lsq_load_bytes},\"lsq_store_bytes\":{lsq_store_bytes},\"lsq_store_conditional_failures\":{lsq_store_conditional_failures},\"lsq_operation_load\":{lsq_operation_load},\"lsq_operation_store\":{lsq_operation_store},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"store_load_forwarding_suppressed\":{},\"store_load_forwarding_address_mismatches\":{},\"store_load_forwarding_byte_mismatches\":{},\"lsq_data_latency\":{lsq_data_latency},\"lsq_operation\":{lsq_operation},\"lsq_ordering\":{lsq_ordering},\"iq\":{iq},\"iew\":{iew},\"commit\":{commit},\"branch_event\":{branch_event},\"branch_repair\":{branch_repair},\"branch_direction_mismatch\":{branch_direction_mismatch},\"branch_target_mismatch\":{branch_target_mismatch},\"fu_latency_instructions\":{},\"fu_latency_cycles\":{},\"fu_latency_max_cycles\":{},\"fu_latency_min_cycles\":{},\"fu_latency_avg_cycles\":{},\"fu_latency_class\":{fu_latency_class}}}",
        span_ticks,
        lsq_forwarding.candidates,
        lsq_forwarding.matches,
        lsq_forwarding.suppressed,
        lsq_forwarding.address_mismatches,
        lsq_forwarding.byte_mismatches,
        fu_latency.instructions,
        fu_latency.cycles,
        fu_latency.max_cycles,
        fu_latency.min_cycles,
        fu_latency.avg_cycles(),
    )
}

fn event_summary_window_json(
    events: &[O3RuntimeTraceRecord],
    records: u64,
    span_ticks: u64,
) -> String {
    let first = event_summary_window_row_json(events.first());
    let last = event_summary_window_row_json(events.last());
    let max_rob_occupancy =
        event_summary_window_row_json(events.iter().max_by_key(|event| event.rob_occupancy()));
    let max_lsq_occupancy =
        event_summary_window_row_json(events.iter().max_by_key(|event| event.lsq_occupancy()));
    let max_rename_map_entries =
        event_summary_window_row_json(events.iter().max_by_key(|event| event.rename_map_entries()));
    format!(
        "{{\"records\":{records},\"span_ticks\":{span_ticks},\"first\":{first},\"last\":{last},\"max_rob_occupancy\":{max_rob_occupancy},\"max_lsq_occupancy\":{max_lsq_occupancy},\"max_rename_map_entries\":{max_rename_map_entries}}}"
    )
}

fn event_summary_window_row_json(event: Option<&O3RuntimeTraceRecord>) -> String {
    event.map_or_else(
        || "null".to_string(),
        |event| {
            format!(
                "{{\"sequence\":{},\"tick\":{},\"pc\":\"0x{:x}\",\"rob_occupancy\":{},\"lsq_occupancy\":{},\"rename_map_entries\":{}}}",
                event.sequence(),
                event.tick(),
                event.pc().get(),
                event.rob_occupancy(),
                event.lsq_occupancy(),
                event.rename_map_entries()
            )
        },
    )
}

fn event_summary_iq_json(events: &[O3RuntimeTraceRecord]) -> String {
    let issued_inst_type = event_summary_inst_type_json(events);
    format!(
        "{{\"insts_issued\":{},\"mem_insts_issued\":{},\"branch_insts_issued\":{},\"issued_inst_type\":{issued_inst_type}}}",
        events.len(),
        events
            .iter()
            .map(|event| event.lsq_loads().saturating_add(event.lsq_stores()))
            .sum::<u64>(),
        events.iter().filter(|event| event.branch_event()).count(),
    )
}

fn event_summary_commit_json(events: &[O3RuntimeTraceRecord]) -> String {
    let committed_inst_type = event_summary_inst_type_json(events);
    let branch_mispredicts = Rem6O3EventIewTotals::from_events(events).branch_mispredicts();
    format!(
        "{{\"branch_mispredicts\":{branch_mispredicts},\"committed_inst_type\":{committed_inst_type}}}"
    )
}

fn event_summary_inst_type_json(events: &[O3RuntimeTraceRecord]) -> String {
    let mut fu_classes = [0_u64; O3RuntimeFuLatencyClass::COUNT];
    for event in events {
        if let Some(class) = event.fu_latency_class() {
            fu_classes[class.index()] = fu_classes[class.index()].saturating_add(1);
        }
    }
    let mut fields = vec![
        format!(
            "\"mem_read\":{}",
            events.iter().map(|event| event.lsq_loads()).sum::<u64>()
        ),
        format!(
            "\"mem_write\":{}",
            events.iter().map(|event| event.lsq_stores()).sum::<u64>()
        ),
    ];
    fields.extend(O3RuntimeFuLatencyClass::ALL.into_iter().map(|class| {
        format!(
            "\"{}\":{}",
            event_summary_inst_type_stem(class),
            fu_classes[class.index()]
        )
    }));
    format!("{{{}}}", fields.join(","))
}

fn event_summary_inst_type_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "int_div",
        _ => class.stat_stem(),
    }
}

fn event_summary_lsq_latency(
    events: &[O3RuntimeTraceRecord],
) -> (
    O3EventSummaryLsqLatency,
    [u64; O3RuntimeLsqOperation::COUNT],
    [O3EventSummaryLsqLatency; O3RuntimeLsqOperation::COUNT],
) {
    let mut summary = O3EventSummaryLsqLatency::default();
    let mut operation_counts = [0_u64; O3RuntimeLsqOperation::COUNT];
    let mut operation_latencies =
        [O3EventSummaryLsqLatency::default(); O3RuntimeLsqOperation::COUNT];
    for event in events {
        let operation = event.lsq_operation();
        if operation == O3RuntimeLsqOperation::None {
            continue;
        }
        let ticks = event.lsq_data_latency_ticks();
        summary.add(ticks);
        operation_counts[operation.index()] = operation_counts[operation.index()].saturating_add(1);
        operation_latencies[operation.index()].add(ticks);
    }
    (summary, operation_counts, operation_latencies)
}

fn event_summary_lsq_forwarding(
    events: &[O3RuntimeTraceRecord],
) -> (
    O3EventSummaryLsqForwarding,
    [O3EventSummaryLsqForwarding; O3RuntimeLsqOperation::COUNT],
) {
    let mut summary = O3EventSummaryLsqForwarding::default();
    let mut operations = [O3EventSummaryLsqForwarding::default(); O3RuntimeLsqOperation::COUNT];
    for event in events {
        summary.add_event(*event);
        let operation = event.lsq_operation();
        if operation == O3RuntimeLsqOperation::None {
            continue;
        }
        operations[operation.index()].add_event(*event);
    }
    (summary, operations)
}

fn event_summary_lsq_latency_json(latency: O3EventSummaryLsqLatency) -> String {
    format!(
        "{{\"samples\":{},\"ticks\":{},\"max_ticks\":{},\"min_ticks\":{},\"avg_ticks\":{}}}",
        latency.samples,
        latency.ticks,
        latency.max_ticks,
        latency.min_ticks,
        latency.avg_ticks()
    )
}

fn event_summary_lsq_operation_json(
    counts: &[u64; O3RuntimeLsqOperation::COUNT],
    latencies: &[O3EventSummaryLsqLatency; O3RuntimeLsqOperation::COUNT],
    forwarding: &[O3EventSummaryLsqForwarding; O3RuntimeLsqOperation::COUNT],
) -> String {
    let fields = O3RuntimeLsqOperation::TRACKED
        .into_iter()
        .map(|operation| {
            let latency = event_summary_lsq_latency_json(latencies[operation.index()]);
            let forwarding = forwarding[operation.index()];
            format!(
                "\"{}\":{{\"count\":{},\"forwarding_candidates\":{},\"forwarding_matches\":{},\"forwarding_suppressed\":{},\"forwarding_address_mismatches\":{},\"forwarding_byte_mismatches\":{},\"latency\":{latency}}}",
                operation.as_str(),
                counts[operation.index()],
                forwarding.candidates,
                forwarding.matches,
                forwarding.suppressed,
                forwarding.address_mismatches,
                forwarding.byte_mismatches,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn event_summary_branch_event_json(events: &[O3RuntimeTraceRecord]) -> String {
    let mut summary = O3EventSummaryBranchEvent::default();
    for event in events {
        summary.add_event(*event);
    }
    let kind = event_summary_branch_kind_json(|branch_kind| summary.kinds[branch_kind.index()]);
    let taken_kind =
        event_summary_branch_kind_json(|branch_kind| summary.taken_kinds[branch_kind.index()]);
    let not_taken_kind =
        event_summary_branch_kind_json(|branch_kind| summary.not_taken_kinds[branch_kind.index()]);
    let predicted_taken_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.predicted_taken_kinds[branch_kind.index()]
    });
    let predicted_not_taken_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.predicted_not_taken_kinds[branch_kind.index()]
    });
    let predicted_target_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.predicted_target_kinds[branch_kind.index()]
    });
    let predicted_target_match_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.predicted_target_match_kinds[branch_kind.index()]
    });
    let predicted_target_mismatch_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.predicted_target_mismatch_kinds[branch_kind.index()]
    });
    let resolved_target_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.resolved_target_kinds[branch_kind.index()]
    });
    let misprediction_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.misprediction_kinds[branch_kind.index()]
    });
    let squash_kind =
        event_summary_branch_kind_json(|branch_kind| summary.squash_kinds[branch_kind.index()]);
    let link_write_kind =
        event_summary_branch_kind_json(|branch_kind| summary.link_write_kinds[branch_kind.index()]);
    let without_link_write_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.kinds[branch_kind.index()]
            .saturating_sub(summary.link_write_kinds[branch_kind.index()])
    });
    let squashed_target_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.squashed_target_kinds[branch_kind.index()]
    });
    let squashed_target_link_write_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.squashed_target_link_write_kinds[branch_kind.index()]
    });
    let squashed_target_without_link_write_kind = event_summary_branch_kind_json(|branch_kind| {
        summary.squashed_target_without_link_write_kinds[branch_kind.index()]
    });
    format!(
        "{{\"branches\":{},\"taken\":{},\"not_taken\":{},\"predicted_taken\":{},\"predicted_not_taken\":{},\"predicted_targets\":{},\"predicted_target_matches\":{},\"predicted_target_mismatches\":{},\"resolved_targets\":{},\"mispredictions\":{},\"link_writes\":{},\"without_link_writes\":{},\"squashes\":{},\"squashed_targets\":{},\"squashed_targets_with_link_writes\":{},\"squashed_targets_without_link_writes\":{},\"kind\":{kind},\"taken_kind\":{taken_kind},\"not_taken_kind\":{not_taken_kind},\"predicted_taken_kind\":{predicted_taken_kind},\"predicted_not_taken_kind\":{predicted_not_taken_kind},\"predicted_target_kind\":{predicted_target_kind},\"predicted_target_match_kind\":{predicted_target_match_kind},\"predicted_target_mismatch_kind\":{predicted_target_mismatch_kind},\"resolved_target_kind\":{resolved_target_kind},\"misprediction_kind\":{misprediction_kind},\"link_write_kind\":{link_write_kind},\"without_link_write_kind\":{without_link_write_kind},\"squash_kind\":{squash_kind},\"squashed_target_kind\":{squashed_target_kind},\"squashed_target_link_write_kind\":{squashed_target_link_write_kind},\"squashed_target_without_link_write_kind\":{squashed_target_without_link_write_kind}}}",
        summary.branches,
        summary.taken,
        summary.not_taken(),
        summary.predicted_taken,
        summary.predicted_not_taken(),
        summary.predicted_targets,
        summary.predicted_target_matches,
        summary.predicted_target_mismatches,
        summary.resolved_targets,
        summary.mispredictions,
        summary.link_writes,
        summary.without_link_writes(),
        summary.squashes,
        summary.squashed_targets,
        summary.squashed_targets_with_link_writes(),
        summary.squashed_target_without_link_writes,
    )
}

fn event_summary_iew_json(events: &[O3RuntimeTraceRecord], span_ticks: u64) -> String {
    let totals = Rem6O3EventIewTotals::from_events(events);
    let dispatched_insts = totals.dispatched_insts();
    let insts_to_commit = totals.insts_to_commit();
    let writeback_count = totals.writeback_count();
    let writeback_rate_ppm = totals.writeback_rate_ppm(span_ticks);
    let producer_inst = totals.dependency_producers();
    let consumer_inst = totals.dependency_consumers();
    let predicted_taken_incorrect = totals.predicted_taken_incorrect();
    let predicted_not_taken_incorrect = totals.predicted_not_taken_incorrect();
    let branch_mispredicts = totals.branch_mispredicts();
    format!(
        "{{\"dispatched_insts\":{dispatched_insts},\"insts_to_commit\":{insts_to_commit},\"writeback_count\":{writeback_count},\"writeback_rate_ppm\":{writeback_rate_ppm},\"producer_inst\":{producer_inst},\"consumer_inst\":{consumer_inst},\"producer_consumer_fanout_ppm\":{},\"predicted_taken_incorrect\":{predicted_taken_incorrect},\"predicted_not_taken_incorrect\":{predicted_not_taken_incorrect},\"branch_mispredicts\":{branch_mispredicts},\"dependency\":{{\"producer\":{producer_inst},\"consumer\":{consumer_inst}}}}}",
        ratio_ppm(producer_inst, consumer_inst)
    )
}

fn event_summary_branch_kind_json<F>(count: F) -> String
where
    F: Fn(BranchTargetKind) -> u64,
{
    let fields = BranchTargetKind::ALL
        .into_iter()
        .map(|kind| format!("\"{}\":{}", kind.canonical_stat_name(), count(kind)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn event_summary_lsq_ordering_json(events: &[O3RuntimeTraceRecord]) -> String {
    let mut counts = [0_u64; O3RuntimeLsqOrdering::COUNT];
    for event in events {
        let ordering = event.lsq_ordering();
        if ordering == O3RuntimeLsqOrdering::None {
            continue;
        }
        counts[ordering.index()] = counts[ordering.index()].saturating_add(1);
    }
    let fields = O3RuntimeLsqOrdering::TRACKED
        .into_iter()
        .map(|ordering| format!("\"{}\":{}", ordering.as_str(), counts[ordering.index()]))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn event_summary_fu_latency(
    events: &[O3RuntimeTraceRecord],
) -> (
    O3EventSummaryFuLatency,
    [O3EventSummaryFuLatency; O3RuntimeFuLatencyClass::COUNT],
) {
    let mut summary = O3EventSummaryFuLatency::default();
    let mut classes = [O3EventSummaryFuLatency::default(); O3RuntimeFuLatencyClass::COUNT];
    for event in events {
        let cycles = event.fu_latency_cycles();
        if cycles == 0 {
            continue;
        }
        summary.add(cycles);
        if let Some(class) = event.fu_latency_class() {
            classes[class.index()].add(cycles);
        }
    }
    (summary, classes)
}

fn event_summary_fu_latency_class_json(
    classes: &[O3EventSummaryFuLatency; O3RuntimeFuLatencyClass::COUNT],
) -> String {
    let fields = O3RuntimeFuLatencyClass::ALL
        .into_iter()
        .map(|class| {
            let summary = classes[class.index()];
            format!(
                "\"{}\":{{\"instructions\":{},\"cycles\":{},\"max_cycles\":{},\"min_cycles\":{},\"avg_cycles\":{}}}",
                class.stat_stem(),
                summary.instructions,
                summary.cycles,
                summary.max_cycles,
                summary.min_cycles,
                summary.avg_cycles()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

const fn min_latency_cycles(current: u64, sample: u64) -> u64 {
    if current == 0 || sample < current {
        sample
    } else {
        current
    }
}

const fn ratio_ppm(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        0
    } else {
        let ppm = (numerator as u128).saturating_mul(1_000_000) / (denominator as u128);
        if ppm > u64::MAX as u128 {
            u64::MAX
        } else {
            ppm as u64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ratio_ppm;

    #[test]
    fn ratio_ppm_widens_before_clamping() {
        assert_eq!(ratio_ppm(3, 4), 750_000);
        assert_eq!(ratio_ppm(1, 0), 0);
        assert_eq!(ratio_ppm(u64::MAX, 2), u64::MAX);
    }
}
