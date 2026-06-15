use std::collections::BTreeMap;

use rem6_cpu::{CpuFetchEventKind, RiscvCore};

pub(super) fn in_order_pipeline_retired(core: &RiscvCore) -> u64 {
    core.execution_events()
        .iter()
        .filter_map(|event| event.in_order_pipeline_cycle())
        .map(|cycle| cycle.summary().retired_count() as u64)
        .sum()
}

pub(super) fn in_order_pipeline_fetch_wait_cycles(core: &RiscvCore) -> u64 {
    let mut issued_ticks = BTreeMap::new();
    let mut wait_cycles = 0u64;
    for event in core.inner().fetch_events() {
        match event.kind() {
            CpuFetchEventKind::Issued => {
                issued_ticks.insert(event.request_id(), event.tick());
            }
            CpuFetchEventKind::Completed => {
                if let Some(issued) = issued_ticks.remove(&event.request_id()) {
                    wait_cycles = wait_cycles.saturating_add(event.tick().saturating_sub(issued));
                }
            }
            CpuFetchEventKind::Retry | CpuFetchEventKind::Failed => {
                issued_ticks.remove(&event.request_id());
            }
        }
    }
    wait_cycles
}

pub(super) fn in_order_pipeline_data_wait_cycles(core: &RiscvCore) -> u64 {
    core.execution_events()
        .iter()
        .map(|event| event.in_order_pipeline_data_wait_cycles())
        .sum()
}
