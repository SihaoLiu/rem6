use rem6_cpu::O3RuntimeTraceRecord;

use super::o3_branch_repair::{
    o3_branch_repair_kind, o3_branch_targetless_mismatch, o3_branch_wrong_target,
};

pub(super) fn o3_event_to_json(event: &O3RuntimeTraceRecord) -> String {
    let fu_latency_class = event.fu_latency_class().map_or_else(
        || "null".to_string(),
        |class| format!("\"{}\"", class.as_str()),
    );
    let lsq_load_address =
        o3_optional_address_to_json(event.lsq_load_address().map(|address| address.get()));
    let lsq_store_address =
        o3_optional_address_to_json(event.lsq_store_address().map(|address| address.get()));
    let branch_predicted_target =
        o3_optional_address_to_json(event.branch_predicted_target().map(|address| address.get()));
    let branch_resolved_target =
        o3_optional_address_to_json(event.branch_resolved_target().map(|address| address.get()));
    let branch_squashed_target =
        o3_optional_address_to_json(event.branch_squashed_target().map(|address| address.get()));
    let branch_targetless_mismatch = o3_branch_targetless_mismatch(event);
    let branch_wrong_target = o3_branch_wrong_target(event);
    let branch_repair = o3_branch_repair_kind(event);
    format!(
        "{{\"sequence\":{},\"tick\":{},\"pc\":\"0x{:x}\",\"rob_allocated\":{},\"rob_committed\":{},\"rob_occupancy\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"lsq_occupancy\":{},\"lsq_operation\":\"{}\",\"lsq_ordering\":\"{}\",\"lsq_acquire\":{},\"lsq_release\":{},\"lsq_load_address\":{},\"lsq_store_address\":{},\"lsq_load_bytes\":{},\"lsq_store_bytes\":{},\"lsq_store_conditional_failed\":{},\"lsq_data_response_tick\":{},\"lsq_data_latency_ticks\":{},\"rename_map_entries\":{},\"store_load_forwarding_candidate\":{},\"store_load_forwarding_match\":{},\"store_load_forwarding_suppressed\":{},\"store_load_forwarding_address_mismatch\":{},\"store_load_forwarding_byte_mismatch\":{},\"branch_event\":{},\"branch_kind\":\"{}\",\"branch_predicted_taken\":{},\"branch_resolved_taken\":{},\"branch_mispredicted\":{},\"branch_targetless_mismatch\":{},\"branch_wrong_target\":{},\"branch_repair\":\"{}\",\"branch_link_register_write\":{},\"branch_predicted_target\":{},\"branch_resolved_target\":{},\"branch_squash\":{},\"branch_squashed_target\":{},\"fu_latency_class\":{},\"fu_latency_cycles\":{},\"system_event\":{}}}",
        event.sequence(),
        event.tick(),
        event.pc().get(),
        event.rob_allocated(),
        event.rob_committed(),
        event.rob_occupancy(),
        event.rename_writes(),
        event.lsq_loads(),
        event.lsq_stores(),
        event.lsq_occupancy(),
        event.lsq_operation().as_str(),
        event.lsq_ordering().as_str(),
        event.lsq_ordering().acquire(),
        event.lsq_ordering().release(),
        lsq_load_address,
        lsq_store_address,
        event.lsq_load_bytes(),
        event.lsq_store_bytes(),
        event.lsq_store_conditional_failed(),
        event.lsq_data_response_tick(),
        event.lsq_data_latency_ticks(),
        event.rename_map_entries(),
        event.store_load_forwarding_candidate(),
        event.store_load_forwarding_match(),
        event.store_load_forwarding_suppressed(),
        event.store_load_forwarding_address_mismatch(),
        event.store_load_forwarding_byte_mismatch(),
        event.branch_event(),
        event.branch_kind().canonical_stat_name(),
        event.branch_predicted_taken(),
        event.branch_resolved_taken(),
        event.branch_mispredicted(),
        branch_targetless_mismatch,
        branch_wrong_target,
        branch_repair.as_str(),
        event.branch_link_register_write(),
        branch_predicted_target,
        branch_resolved_target,
        event.branch_squash(),
        branch_squashed_target,
        fu_latency_class,
        event.fu_latency_cycles(),
        event.system_event(),
    )
}

fn o3_optional_address_to_json(address: Option<u64>) -> String {
    address.map_or_else(
        || "null".to_string(),
        |address| format!("\"0x{address:x}\""),
    )
}
