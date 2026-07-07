use rem6_cpu::{O3RuntimeLsqOperation, O3RuntimeTraceRecord};

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

    format!(
        "{{\"records\":{records},\"first_tick\":{first_tick},\"last_tick\":{last_tick},\"span_ticks\":{},\"max_rob_occupancy\":{max_rob_occupancy},\"max_lsq_occupancy\":{max_lsq_occupancy},\"max_rename_map_entries\":{max_rename_map_entries},\"system_events\":{system_events},\"rob_allocations\":{rob_allocations},\"rob_commits\":{rob_commits},\"rename_writes\":{rename_writes},\"lsq_loads\":{lsq_loads},\"lsq_stores\":{lsq_stores},\"lsq_operation_load\":{lsq_operation_load},\"lsq_operation_store\":{lsq_operation_store}}}",
        last_tick.saturating_sub(first_tick),
    )
}
