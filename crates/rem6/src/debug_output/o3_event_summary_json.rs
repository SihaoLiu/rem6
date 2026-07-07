use rem6_cpu::{O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeTraceRecord};

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

pub(super) fn o3_event_summary_to_json(events: &[O3RuntimeTraceRecord]) -> String {
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

    format!(
        "{{\"records\":{records},\"first_tick\":{first_tick},\"last_tick\":{last_tick},\"span_ticks\":{},\"max_rob_occupancy\":{max_rob_occupancy},\"max_lsq_occupancy\":{max_lsq_occupancy},\"max_rename_map_entries\":{max_rename_map_entries},\"system_events\":{system_events},\"rob_allocations\":{rob_allocations},\"rob_commits\":{rob_commits},\"rename_writes\":{rename_writes},\"lsq_loads\":{lsq_loads},\"lsq_stores\":{lsq_stores},\"lsq_operation_load\":{lsq_operation_load},\"lsq_operation_store\":{lsq_operation_store},\"fu_latency_instructions\":{},\"fu_latency_cycles\":{},\"fu_latency_max_cycles\":{},\"fu_latency_min_cycles\":{},\"fu_latency_avg_cycles\":{},\"fu_latency_class\":{fu_latency_class}}}",
        last_tick.saturating_sub(first_tick),
        fu_latency.instructions,
        fu_latency.cycles,
        fu_latency.max_cycles,
        fu_latency.min_cycles,
        fu_latency.avg_cycles(),
    )
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
