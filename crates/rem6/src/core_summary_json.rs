use rem6_cpu::{
    BranchTargetKind, BranchTargetKindCounts, BranchTargetProvider, BranchTargetProviderCounts,
    O3LoadStoreQueueKind, O3PhysicalRegisterId, O3RegisterClass, O3RuntimeFuLatencyClass,
    O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeStats, O3RuntimeTraceRecord,
};
use rem6_memory::Address;

use crate::{
    formatting::json_escape, pipeline_stats::Rem6InOrderPipelineStageSummary, Rem6CoreSummary,
    Rem6HostCheckpointSummary,
};

fn branch_target_kind_counts_json(counts: BranchTargetKindCounts) -> String {
    branch_target_kind_counts_json_with_total(counts, counts.total())
}

fn branch_target_kind_counts_json_with_total(counts: BranchTargetKindCounts, total: u64) -> String {
    let mut fields = BranchTargetKind::ALL
        .into_iter()
        .map(|kind| format!("\"{}\":{}", kind.canonical_stat_name(), counts.value(kind)))
        .collect::<Vec<_>>();
    fields.push(format!("\"total\":{total}"));
    format!("{{{}}}", fields.join(","))
}

fn branch_target_provider_counts_json(counts: BranchTargetProviderCounts) -> String {
    let mut fields = BranchTargetProvider::ALL
        .into_iter()
        .map(|provider| {
            format!(
                "\"{}\":{}",
                provider.canonical_stat_name(),
                counts.value(provider)
            )
        })
        .collect::<Vec<_>>();
    fields.push(format!("\"total\":{}", counts.total()));
    format!("{{{}}}", fields.join(","))
}

fn branch_predictor_counter_json(
    lookups: u64,
    history_updates: u64,
    updates: u64,
    squashes: u64,
) -> String {
    format!(
        "{{\"lookups\":{lookups},\"history_updates\":{history_updates},\"updates\":{updates},\"squashes\":{squashes}}}"
    )
}

fn in_order_pipeline_stage_summary_json(summary: Rem6InOrderPipelineStageSummary) -> String {
    format!(
        "{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}}",
        summary.fetch1, summary.fetch2, summary.decode, summary.execute, summary.commit
    )
}

fn in_order_pipeline_stall_cause_stage_summary_json(
    records: u64,
    stage_records: Rem6InOrderPipelineStageSummary,
    resource_blocked: Rem6InOrderPipelineStageSummary,
    resource_blocked_cycles: Rem6InOrderPipelineStageSummary,
    ordering_blocked: Rem6InOrderPipelineStageSummary,
    ordering_blocked_cycles: Rem6InOrderPipelineStageSummary,
) -> String {
    format!(
        "{{\"records\":{},\"stage_records\":{},\"stage_resource_blocked\":{},\"stage_resource_blocked_cycles\":{},\"stage_ordering_blocked\":{},\"stage_ordering_blocked_cycles\":{}}}",
        records,
        in_order_pipeline_stage_summary_json(stage_records),
        in_order_pipeline_stage_summary_json(resource_blocked),
        in_order_pipeline_stage_summary_json(resource_blocked_cycles),
        in_order_pipeline_stage_summary_json(ordering_blocked),
        in_order_pipeline_stage_summary_json(ordering_blocked_cycles)
    )
}

fn in_order_pipeline_flush_redirect_cause_stage_summary_json(
    records: u64,
    stage_records: Rem6InOrderPipelineStageSummary,
    flushed: Rem6InOrderPipelineStageSummary,
    flushed_cycles: Rem6InOrderPipelineStageSummary,
) -> String {
    let flushed_total = in_order_pipeline_stage_total(flushed);
    let flushed_cycle_total = in_order_pipeline_stage_total(flushed_cycles);
    format!(
        "{{\"records\":{},\"flushed\":{},\"flushed_cycles\":{},\"stage_records\":{},\"stage_flushed\":{},\"stage_flushed_cycles\":{}}}",
        records,
        flushed_total,
        flushed_cycle_total,
        in_order_pipeline_stage_summary_json(stage_records),
        in_order_pipeline_stage_summary_json(flushed),
        in_order_pipeline_stage_summary_json(flushed_cycles)
    )
}

fn in_order_pipeline_stage_total(summary: Rem6InOrderPipelineStageSummary) -> u64 {
    summary.values().into_iter().sum()
}

fn o3_runtime_fu_latency_class_json(summary: &Rem6CoreSummary) -> String {
    let mut fields = O3RuntimeFuLatencyClass::ALL
        .into_iter()
        .flat_map(|class| {
            [
                format!(
                    "\"fu_{}_instructions\":{}",
                    class.stat_stem(),
                    summary.o3_runtime.fu_latency_class_instructions(class)
                ),
                format!(
                    "\"fu_{}_latency_cycles\":{}",
                    class.stat_stem(),
                    summary.o3_runtime.fu_latency_class_cycles(class)
                ),
                format!(
                    "\"fu_{}_latency_max_cycles\":{}",
                    class.stat_stem(),
                    summary.o3_runtime.fu_latency_class_max_cycles(class)
                ),
                format!(
                    "\"fu_{}_latency_min_cycles\":{}",
                    class.stat_stem(),
                    summary.o3_runtime.fu_latency_class_min_cycles(class)
                ),
                format!(
                    "\"fu_{}_latency_avg_cycles\":{}",
                    class.stat_stem(),
                    summary.o3_runtime.fu_latency_class_avg_cycles(class)
                ),
            ]
        })
        .collect::<Vec<_>>();
    let class_summary = O3RuntimeFuLatencyClass::ALL
        .into_iter()
        .map(|class| {
            format!(
                "\"{}\":{{\"instructions\":{},\"cycles\":{},\"max_cycles\":{},\"min_cycles\":{},\"avg_cycles\":{}}}",
                class.stat_stem(),
                summary.o3_runtime.fu_latency_class_instructions(class),
                summary.o3_runtime.fu_latency_class_cycles(class),
                summary.o3_runtime.fu_latency_class_max_cycles(class),
                summary.o3_runtime.fu_latency_class_min_cycles(class),
                summary.o3_runtime.fu_latency_class_avg_cycles(class)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    fields.push(format!("\"fu_latency_class\":{{{class_summary}}}"));
    fields.join(",")
}

fn o3_runtime_inst_type_json(summary: &Rem6CoreSummary) -> String {
    let mut fields = vec![
        format!("\"mem_read\":{}", summary.o3_runtime.lsq_loads()),
        format!("\"mem_write\":{}", summary.o3_runtime.lsq_stores()),
    ];
    fields.extend(O3RuntimeFuLatencyClass::ALL.into_iter().map(|class| {
        format!(
            "\"{}\":{}",
            class.inst_type_descriptor().source_stem(),
            summary.o3_runtime.fu_latency_class_instructions(class)
        )
    }));
    format!("{{{}}}", fields.join(","))
}

fn o3_runtime_branch_mispredicts(summary: &Rem6CoreSummary) -> u64 {
    summary
        .o3_runtime
        .iew_predicted_taken_incorrect()
        .saturating_add(summary.o3_runtime.iew_predicted_not_taken_incorrect())
}

fn o3_runtime_iq_json(summary: &Rem6CoreSummary) -> String {
    let issued_inst_type = o3_runtime_inst_type_json(summary);
    format!(
        "{{\"insts_issued\":{},\"mem_insts_issued\":{},\"branch_insts_issued\":{},\"issued_inst_type\":{issued_inst_type}}}",
        summary.o3_runtime.instructions(),
        summary
            .o3_runtime
            .lsq_loads()
            .saturating_add(summary.o3_runtime.lsq_stores()),
        summary.o3_runtime.iq_branch_insts_issued(),
    )
}

fn o3_runtime_issue_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "{{\"configured_width\":{},\"configured_memory_width\":{},\"cycles\":{},\"issued_rows\":{},\"resource_blocked_row_cycles\":{},\"dependency_blocked_row_cycles\":{},\"max_rows_per_cycle\":{}}}",
        summary.o3_runtime_issue_width,
        summary.o3_runtime_memory_issue_width,
        summary.o3_runtime.issue_cycles(),
        summary.o3_runtime.issued_rows(),
        summary.o3_runtime.resource_blocked_row_cycles(),
        summary.o3_runtime.dependency_blocked_row_cycles(),
        summary.o3_runtime.max_rows_per_cycle()
    )
}

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

fn o3_runtime_iew_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "{{\"dispatched_insts\":{},\"insts_to_commit\":{},\"writeback_count\":{},\"producer_inst\":{},\"consumer_inst\":{},\"predicted_taken_incorrect\":{},\"predicted_not_taken_incorrect\":{},\"branch_mispredicts\":{}}}",
        summary.o3_runtime.instructions(),
        summary.o3_runtime.rob_commits(),
        summary.o3_runtime.instructions(),
        summary.o3_runtime.iew_producer_insts(),
        summary.o3_runtime.iew_consumer_insts(),
        summary.o3_runtime.iew_predicted_taken_incorrect(),
        summary.o3_runtime.iew_predicted_not_taken_incorrect(),
        o3_runtime_branch_mispredicts(summary),
    )
}

fn o3_runtime_commit_json(summary: &Rem6CoreSummary) -> String {
    let committed_inst_type = o3_runtime_inst_type_json(summary);
    format!(
        "{{\"branch_mispredicts\":{},\"committed_inst_type\":{committed_inst_type}}}",
        o3_runtime_branch_mispredicts(summary)
    )
}

fn o3_runtime_rob_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "{{\"allocations\":{},\"commits\":{},\"max_occupancy\":{}}}",
        summary.o3_runtime.rob_allocations(),
        summary.o3_runtime.rob_commits(),
        summary.o3_runtime.max_rob_occupancy()
    )
}

fn o3_runtime_rename_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "{{\"writes\":{},\"map_entries\":{}}}",
        summary.o3_runtime.rename_writes(),
        summary.o3_runtime.rename_map_entries()
    )
}

fn o3_runtime_register_class_name(register_class: O3RegisterClass) -> &'static str {
    match register_class {
        O3RegisterClass::Integer => "integer",
        O3RegisterClass::FloatingPoint => "floating_point",
        O3RegisterClass::Vector => "vector",
        O3RegisterClass::ConditionCode => "condition_code",
        O3RegisterClass::Misc => "misc",
    }
}

fn o3_runtime_lsq_kind_name(kind: O3LoadStoreQueueKind) -> &'static str {
    match kind {
        O3LoadStoreQueueKind::Load => "load",
        O3LoadStoreQueueKind::Store => "store",
    }
}

fn o3_runtime_address_json(address: Option<Address>) -> String {
    address
        .map(|address| format!("\"0x{:x}\"", address.get()))
        .unwrap_or_else(|| "null".to_string())
}

fn o3_runtime_physical_json(physical: Option<O3PhysicalRegisterId>) -> String {
    physical
        .map(|physical| physical.get().to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn o3_runtime_snapshot_json(summary: &Rem6CoreSummary) -> String {
    let snapshot = &summary.o3_runtime_snapshot;
    let rob_entries = snapshot
        .reorder_buffer()
        .iter()
        .map(|entry| {
            format!(
                "{{\"sequence\":{},\"pc\":\"0x{:x}\",\"destination\":{},\"ready\":{},\"live_staged\":{}}}",
                entry.sequence(),
                entry.pc().get(),
                o3_runtime_physical_json(entry.destination()),
                entry.is_ready(),
                entry.is_live_staged()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let lsq_entries = snapshot
        .load_store_queue()
        .iter()
        .map(|entry| {
            format!(
                "{{\"sequence\":{},\"kind\":\"{}\",\"address\":{},\"bytes\":{},\"completed\":{}}}",
                entry.sequence(),
                o3_runtime_lsq_kind_name(entry.kind()),
                o3_runtime_address_json(entry.address()),
                entry.bytes(),
                entry.is_completed()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let rename_entries = snapshot
        .rename_map()
        .iter()
        .map(|entry| {
            format!(
                "{{\"register_class\":\"{}\",\"architectural\":{},\"physical\":{}}}",
                o3_runtime_register_class_name(entry.register_class()),
                entry.architectural(),
                entry.physical().get()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"rob\":{{\"count\":{},\"entries\":[{}]}},\"lsq\":{{\"count\":{},\"entries\":[{}]}},\"rename_map\":{{\"count\":{},\"entries\":[{}]}}}}",
        snapshot.reorder_buffer().len(),
        rob_entries,
        snapshot.load_store_queue().len(),
        lsq_entries,
        snapshot.rename_map().len(),
        rename_entries
    )
}

fn o3_runtime_writeback_calendar_json(summary: &Rem6CoreSummary) -> String {
    let entries = summary
        .o3_runtime_writeback_reservations
        .iter()
        .map(|reservation| {
            format!(
                "{{\"sequence\":{},\"raw_ready_tick\":{},\"admitted_tick\":{},\"slot\":{}}}",
                reservation.sequence(),
                reservation.raw_ready_tick(),
                reservation.admitted_tick(),
                reservation.slot()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"count\":{},\"entries\":[{}]}}",
        summary.o3_runtime_writeback_reservations.len(),
        entries
    )
}

fn o3_runtime_event_window_row_json(event: Option<&O3RuntimeTraceRecord>) -> String {
    event.map_or_else(
        || "null".to_string(),
        |event| {
            format!(
                "{{\"sequence\":{},\"tick\":{},\"issue_tick\":{},\"writeback_tick\":{},\"commit_tick\":{},\"issue_to_writeback_ticks\":{},\"writeback_to_commit_ticks\":{},\"issue_to_commit_ticks\":{},\"pc\":\"0x{:x}\",\"rob_occupancy\":{},\"rob_commits_at_tick\":{},\"rob_commit_blocked\":{},\"lsq_occupancy\":{},\"lsq_operation\":\"{}\",\"lsq_ordering\":\"{}\",\"rename_map_entries\":{},\"lsq_data_latency_ticks\":{},\"fu_latency_cycles\":{}}}",
                event.sequence(),
                event.tick(),
                event.issue_tick(),
                event.writeback_tick(),
                event.commit_tick(),
                event.issue_to_writeback_ticks(),
                event.writeback_to_commit_ticks(),
                event.issue_to_commit_ticks(),
                event.pc().get(),
                event.rob_occupancy(),
                event.rob_commits_at_tick(),
                event.rob_commit_blocked(),
                event.lsq_occupancy(),
                event.lsq_operation().as_str(),
                event.lsq_ordering().as_str(),
                event.rename_map_entries(),
                event.lsq_data_latency_ticks(),
                event.fu_latency_cycles()
            )
        },
    )
}

fn o3_runtime_event_window_json(summary: &Rem6CoreSummary) -> String {
    let events = &summary.o3_runtime_trace_records;
    if events.is_empty() {
        return "null".to_string();
    }
    let records = events.len() as u64;
    let first_tick = events.first().map_or(0, |event| event.tick());
    let last_tick = events.last().map_or(0, |event| event.tick());
    let span_ticks = last_tick.saturating_sub(first_tick);
    let first = o3_runtime_event_window_row_json(events.first());
    let last = o3_runtime_event_window_row_json(events.last());
    let max_rob_occupancy =
        o3_runtime_event_window_row_json(events.iter().max_by_key(|event| event.rob_occupancy()));
    let max_lsq_occupancy =
        o3_runtime_event_window_row_json(events.iter().max_by_key(|event| event.lsq_occupancy()));
    let max_rename_map_entries = o3_runtime_event_window_row_json(
        events.iter().max_by_key(|event| event.rename_map_entries()),
    );
    let max_structural_pressure = o3_runtime_event_window_row_json(
        events
            .iter()
            .max_by_key(|event| event.structural_pressure_key()),
    );
    let max_lsq_data_latency = o3_runtime_event_window_row_json(
        events
            .iter()
            .max_by_key(|event| event.lsq_data_latency_ticks()),
    );
    let max_fu_latency = o3_runtime_event_window_row_json(
        events.iter().max_by_key(|event| event.fu_latency_cycles()),
    );
    format!(
        "{{\"records\":{records},\"span_ticks\":{span_ticks},\"first\":{first},\"last\":{last},\"max_rob_occupancy\":{max_rob_occupancy},\"max_lsq_occupancy\":{max_lsq_occupancy},\"max_rename_map_entries\":{max_rename_map_entries},\"max_structural_pressure\":{max_structural_pressure},\"max_lsq_data_latency\":{max_lsq_data_latency},\"max_fu_latency\":{max_fu_latency}}}"
    )
}

fn o3_runtime_event_summary_json(summary: &Rem6CoreSummary) -> String {
    if summary.o3_runtime_trace_records.is_empty() {
        return "null".to_string();
    }
    crate::debug_output::o3_event_summary_to_json(&summary.o3_runtime_trace_records)
}

fn o3_runtime_branch_direction_mismatch_json(summary: &Rem6CoreSummary) -> String {
    if summary.o3_runtime_trace_records.is_empty() {
        return "null".to_string();
    }
    crate::debug_output::o3_branch_direction_mismatch_to_json(&summary.o3_runtime_trace_records)
}

fn o3_runtime_branch_target_mismatch_json(summary: &Rem6CoreSummary) -> String {
    if summary.o3_runtime_trace_records.is_empty() {
        return "null".to_string();
    }
    crate::debug_output::o3_branch_target_mismatch_to_json(&summary.o3_runtime_trace_records)
}

fn o3_runtime_latency_json(
    samples: u64,
    ticks: u64,
    max_ticks: u64,
    min_ticks: u64,
    avg_ticks: u64,
) -> String {
    format!(
        "{{\"samples\":{samples},\"ticks\":{ticks},\"max_ticks\":{max_ticks},\"min_ticks\":{min_ticks},\"avg_ticks\":{avg_ticks}}}"
    )
}

fn o3_runtime_lsq_operation_matrix_json(summary: &Rem6CoreSummary) -> String {
    let fields = O3RuntimeLsqOperation::TRACKED
        .into_iter()
        .map(|operation| {
            let latency = o3_runtime_latency_json(
                summary.o3_runtime.lsq_operation_latency_samples(operation),
                summary.o3_runtime.lsq_operation_latency_ticks(operation),
                summary
                    .o3_runtime
                    .lsq_operation_latency_max_ticks(operation),
                summary
                    .o3_runtime
                    .lsq_operation_latency_min_ticks(operation),
                summary
                    .o3_runtime
                    .lsq_operation_latency_avg_ticks(operation),
            );
            format!(
                "\"{}\":{{\"count\":{},\"load_bytes\":{},\"store_bytes\":{},\"store_conditional_failures\":{},\"forwarding_candidates\":{},\"forwarding_matches\":{},\"forwarding_suppressed\":{},\"forwarding_address_mismatches\":{},\"forwarding_byte_mismatches\":{},\"latency\":{latency}}}",
                operation.as_str(),
                summary.o3_runtime.lsq_operation_count(operation),
                summary.o3_runtime.lsq_operation_load_bytes(operation),
                summary.o3_runtime.lsq_operation_store_bytes(operation),
                summary
                    .o3_runtime
                    .lsq_operation_store_conditional_failures(operation),
                summary
                    .o3_runtime
                    .lsq_operation_forwarding_candidates(operation),
                summary.o3_runtime.lsq_operation_forwarding_matches(operation),
                summary
                    .o3_runtime
                    .lsq_operation_forwarding_suppressed(operation),
                summary
                    .o3_runtime
                    .lsq_operation_forwarding_address_mismatches(operation),
                summary
                    .o3_runtime
                    .lsq_operation_forwarding_byte_mismatches(operation),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn o3_runtime_lsq_ordering_matrix_json(summary: &Rem6CoreSummary) -> String {
    let fields = O3RuntimeLsqOrdering::TRACKED
        .into_iter()
        .map(|ordering| {
            format!(
                "\"{}\":{}",
                ordering.as_str(),
                summary.o3_runtime.lsq_ordering_count(ordering)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

fn o3_runtime_lsq_json(summary: &Rem6CoreSummary) -> String {
    let data_latency = o3_runtime_latency_json(
        summary.o3_runtime.lsq_data_latency_samples(),
        summary.o3_runtime.lsq_data_latency_ticks(),
        summary.o3_runtime.lsq_data_latency_max_ticks(),
        summary.o3_runtime.lsq_data_latency_min_ticks(),
        summary.o3_runtime.lsq_data_latency_avg_ticks(),
    );
    let operation = o3_runtime_lsq_operation_matrix_json(summary);
    let ordering = o3_runtime_lsq_ordering_matrix_json(summary);
    format!(
        "{{\"loads\":{},\"stores\":{},\"load_bytes\":{},\"store_bytes\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"store_load_forwarding_suppressed\":{},\"store_load_forwarding_address_mismatches\":{},\"store_load_forwarding_byte_mismatches\":{},\"store_conditional_failures\":{},\"max_occupancy\":{},\"data_latency\":{data_latency},\"operation\":{operation},\"ordering\":{ordering}}}",
        summary.o3_runtime.lsq_loads(),
        summary.o3_runtime.lsq_stores(),
        summary.o3_runtime.lsq_load_bytes(),
        summary.o3_runtime.lsq_store_bytes(),
        summary.o3_runtime.lsq_store_to_load_forwarding_candidates(),
        summary.o3_runtime.lsq_store_to_load_forwarding_matches(),
        summary.o3_runtime.lsq_store_to_load_forwarding_suppressed(),
        summary
            .o3_runtime
            .lsq_store_to_load_forwarding_address_mismatches(),
        summary
            .o3_runtime
            .lsq_store_to_load_forwarding_byte_mismatches(),
        summary.o3_runtime.lsq_store_conditional_failures(),
        summary.o3_runtime.max_lsq_occupancy(),
    )
}

fn o3_runtime_lsq_operation_json(summary: &Rem6CoreSummary) -> String {
    O3RuntimeLsqOperation::TRACKED
        .into_iter()
        .flat_map(|operation| {
            let name = operation.as_str();
            [
                format!(
                    "\"lsq_operation_{name}\":{}",
                    summary.o3_runtime.lsq_operation_count(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_load_bytes\":{}",
                    summary.o3_runtime.lsq_operation_load_bytes(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_store_bytes\":{}",
                    summary.o3_runtime.lsq_operation_store_bytes(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_store_conditional_failures\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_store_conditional_failures(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_forwarding_candidates\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_forwarding_candidates(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_forwarding_matches\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_forwarding_matches(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_forwarding_suppressed\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_forwarding_suppressed(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_forwarding_address_mismatches\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_forwarding_address_mismatches(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_forwarding_byte_mismatches\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_forwarding_byte_mismatches(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_ticks\":{}",
                    summary.o3_runtime.lsq_operation_latency_ticks(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_samples\":{}",
                    summary.o3_runtime.lsq_operation_latency_samples(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_max_ticks\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_latency_max_ticks(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_min_ticks\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_latency_min_ticks(operation)
                ),
                format!(
                    "\"lsq_operation_{name}_latency_avg_ticks\":{}",
                    summary
                        .o3_runtime
                        .lsq_operation_latency_avg_ticks(operation)
                ),
            ]
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn o3_runtime_lsq_data_latency_json(summary: &Rem6CoreSummary) -> String {
    format!(
        "\"lsq_data_latency_samples\":{},\"lsq_data_latency_ticks\":{},\"lsq_data_latency_max_ticks\":{},\"lsq_data_latency_min_ticks\":{},\"lsq_data_latency_avg_ticks\":{}",
        summary.o3_runtime.lsq_data_latency_samples(),
        summary.o3_runtime.lsq_data_latency_ticks(),
        summary.o3_runtime.lsq_data_latency_max_ticks(),
        summary.o3_runtime.lsq_data_latency_min_ticks(),
        summary.o3_runtime.lsq_data_latency_avg_ticks()
    )
}

fn o3_runtime_lsq_ordering_json(summary: &Rem6CoreSummary) -> String {
    O3RuntimeLsqOrdering::TRACKED
        .into_iter()
        .map(|ordering| {
            format!(
                "\"lsq_ordering_{}\":{}",
                ordering.as_str(),
                summary.o3_runtime.lsq_ordering_count(ordering)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn o3_runtime_live_retire_gate_json(summary: &Rem6CoreSummary) -> String {
    let stats = summary.o3_runtime;
    if stats.live_retire_gate_scheduled_waits() == 0
        && stats.live_retire_gate_wait_ticks() == 0
        && stats.live_retire_gate_max_wait_ticks() == 0
    {
        return String::new();
    }
    format!(
        ",\"live_retire_gate\":{{\"scheduled_waits\":{},\"wait_ticks\":{},\"max_wait_ticks\":{}}}",
        stats.live_retire_gate_scheduled_waits(),
        stats.live_retire_gate_wait_ticks(),
        stats.live_retire_gate_max_wait_ticks()
    )
}

fn o3_runtime_branch_repair_kind_json<F>(count: F) -> String
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

fn o3_runtime_branch_event_json(summary: &Rem6CoreSummary) -> String {
    let kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_kind(branch_kind)
    });
    let taken_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_taken_kind(branch_kind)
    });
    let not_taken_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_not_taken_kind(branch_kind)
    });
    let predicted_taken_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_predicted_taken_kind(branch_kind)
    });
    let predicted_not_taken_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_predicted_not_taken_kind(branch_kind)
    });
    let predicted_target_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_predicted_target_kind(branch_kind)
    });
    let predicted_target_match_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_predicted_target_match_kind(branch_kind)
    });
    let predicted_target_mismatch_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_predicted_target_mismatch_kind(branch_kind)
    });
    let resolved_target_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_resolved_target_kind(branch_kind)
    });
    let misprediction_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_misprediction_kind(branch_kind)
    });
    let link_write_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_link_write_kind(branch_kind)
    });
    let without_link_write_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_without_link_write_kind(branch_kind)
    });
    let squash_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary.o3_runtime.branch_event_squash_kind(branch_kind)
    });
    let squashed_target_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_squashed_target_kind(branch_kind)
    });
    let squashed_target_link_write_kind = o3_runtime_branch_repair_kind_json(|branch_kind| {
        summary
            .o3_runtime
            .branch_event_squashed_target_link_write_kind(branch_kind)
    });
    let squashed_target_without_link_write_kind =
        o3_runtime_branch_repair_kind_json(|branch_kind| {
            summary
                .o3_runtime
                .branch_event_squashed_target_without_link_write_kind(branch_kind)
        });
    format!(
        "{{\"branches\":{},\"taken\":{},\"not_taken\":{},\"predicted_taken\":{},\"predicted_not_taken\":{},\"predicted_targets\":{},\"predicted_target_matches\":{},\"predicted_target_mismatches\":{},\"resolved_targets\":{},\"mispredictions\":{},\"kind\":{kind},\"taken_kind\":{taken_kind},\"not_taken_kind\":{not_taken_kind},\"predicted_taken_kind\":{predicted_taken_kind},\"predicted_not_taken_kind\":{predicted_not_taken_kind},\"predicted_target_kind\":{predicted_target_kind},\"predicted_target_match_kind\":{predicted_target_match_kind},\"predicted_target_mismatch_kind\":{predicted_target_mismatch_kind},\"resolved_target_kind\":{resolved_target_kind},\"misprediction_kind\":{misprediction_kind},\"link_writes\":{},\"without_link_writes\":{},\"link_write_kind\":{link_write_kind},\"without_link_write_kind\":{without_link_write_kind},\"squashes\":{},\"squashed_targets\":{},\"squashed_targets_with_link_writes\":{},\"squashed_targets_without_link_writes\":{},\"squash_kind\":{squash_kind},\"squashed_target_kind\":{squashed_target_kind},\"squashed_target_link_write_kind\":{squashed_target_link_write_kind},\"squashed_target_without_link_write_kind\":{squashed_target_without_link_write_kind}}}",
        summary.o3_runtime.branch_events(),
        summary.o3_runtime.branch_event_taken(),
        summary.o3_runtime.branch_event_not_taken(),
        summary.o3_runtime.branch_event_predicted_taken(),
        summary.o3_runtime.branch_event_predicted_not_taken(),
        summary.o3_runtime.branch_event_predicted_targets(),
        summary.o3_runtime.branch_event_predicted_target_matches(),
        summary
            .o3_runtime
            .branch_event_predicted_target_mismatches(),
        summary.o3_runtime.branch_event_resolved_targets(),
        summary.o3_runtime.branch_event_mispredictions(),
        summary.o3_runtime.branch_event_link_writes(),
        summary.o3_runtime.branch_event_without_link_writes(),
        summary.o3_runtime.branch_event_squashes(),
        summary.o3_runtime.branch_event_squashed_targets(),
        summary
            .o3_runtime
            .branch_event_squashed_targets_with_link_writes(),
        summary
            .o3_runtime
            .branch_event_squashed_targets_without_link_writes(),
    )
}

fn o3_runtime_branch_repair_json(summary: &Rem6CoreSummary) -> String {
    let targetless_mismatch_kind = o3_runtime_branch_repair_kind_json(|kind| {
        summary
            .o3_runtime
            .branch_repair_targetless_mismatch_kind(kind)
    });
    let wrong_target_kind = o3_runtime_branch_repair_kind_json(|kind| {
        summary.o3_runtime.branch_repair_wrong_target_kind(kind)
    });
    let direction_only_kind = o3_runtime_branch_repair_kind_json(|kind| {
        summary.o3_runtime.branch_repair_direction_only_kind(kind)
    });
    format!(
        "{{\"targetless_mismatches\":{},\"wrong_targets\":{},\"direction_only_mismatches\":{},\"targetless_mismatch_kind\":{},\"wrong_target_kind\":{},\"direction_only_kind\":{}}}",
        summary.o3_runtime.branch_repair_targetless_mismatches(),
        summary.o3_runtime.branch_repair_wrong_targets(),
        summary.o3_runtime.branch_repair_direction_only_mismatches(),
        targetless_mismatch_kind,
        wrong_target_kind,
        direction_only_kind,
    )
}

impl Rem6CoreSummary {
    pub(crate) fn to_json(&self) -> String {
        let registers = self
            .registers
            .iter()
            .map(|(register, value)| format!("\"x{}\":\"0x{:x}\"", register, value))
            .collect::<Vec<_>>()
            .join(",");
        let checker = self
            .checker
            .as_ref()
            .map(|checker| {
                let execution_mode = checker
                    .execution_mode
                    .map(|mode| format!("\"{}\"", json_escape(mode)))
                    .unwrap_or_else(|| "null".to_string());
                format!(
                    ",\"checker\":{{\"checked_instructions\":{},\"mismatches\":{},\"execution_mode\":{}}}",
                    checker.checked_instructions, checker.mismatches, execution_mode
                )
            })
            .unwrap_or_default();
        let o3_runtime = if self.o3_runtime.has_activity() {
            let execution_mode = self
                .o3_runtime_execution_mode
                .map(|mode| format!("\"{}\"", json_escape(mode)))
                .unwrap_or_else(|| "null".to_string());
            let checkpoint_restore_summary = self.o3_runtime_checkpoint_restore.as_ref();
            let checkpoint_restore = checkpoint_restore_summary
                .map(Rem6HostCheckpointSummary::to_json)
                .unwrap_or_else(|| "null".to_string());
            let checkpoint_restore_count = u64::from(checkpoint_restore_summary.is_some());
            let checkpoint_restore_label = checkpoint_restore_summary
                .map(|restore| format!("\"{}\"", json_escape(&restore.label)))
                .unwrap_or_else(|| "null".to_string());
            let checkpoint_restore_tick = checkpoint_restore_summary
                .map(|restore| restore.tick.to_string())
                .unwrap_or_else(|| "null".to_string());
            let checkpoint_restore_manifest_tick = checkpoint_restore_summary
                .map(|restore| restore.manifest_tick.to_string())
                .unwrap_or_else(|| "null".to_string());
            let checkpoint_restore_payload_bytes = checkpoint_restore_summary
                .map(|restore| restore.payload_bytes().to_string())
                .unwrap_or_else(|| "null".to_string());
            let fu_latency_classes = o3_runtime_fu_latency_class_json(self);
            let lsq_data_latency = o3_runtime_lsq_data_latency_json(self);
            let lsq_operations = o3_runtime_lsq_operation_json(self);
            let lsq_orderings = o3_runtime_lsq_ordering_json(self);
            let live_retire_gate = o3_runtime_live_retire_gate_json(self);
            let branch_event = o3_runtime_branch_event_json(self);
            let branch_repair = o3_runtime_branch_repair_json(self);
            let branch_direction_mismatch = o3_runtime_branch_direction_mismatch_json(self);
            let branch_target_mismatch = o3_runtime_branch_target_mismatch_json(self);
            let iq = o3_runtime_iq_json(self);
            let issue = o3_runtime_issue_json(self);
            let writeback_port = o3_runtime_writeback_port_json(self.o3_runtime);
            let iew = o3_runtime_iew_json(self);
            let commit = o3_runtime_commit_json(self);
            let rob = o3_runtime_rob_json(self);
            let rename = o3_runtime_rename_json(self);
            let lsq = o3_runtime_lsq_json(self);
            let snapshot = o3_runtime_snapshot_json(self);
            let writeback_calendar = o3_runtime_writeback_calendar_json(self);
            let event_window = o3_runtime_event_window_json(self);
            let event_summary = o3_runtime_event_summary_json(self);
            format!(
                ",\"o3_runtime\":{{\"execution_mode\":{},\"stats_epoch\":{},\"stats_reset_tick\":{},\"checkpoint_restore_count\":{},\"checkpoint_restore_label\":{},\"checkpoint_restore_tick\":{},\"checkpoint_restore_manifest_tick\":{},\"checkpoint_restore_payload_bytes\":{},\"checkpoint_restore\":{},\"event_window\":{},\"event_summary\":{},\"instructions\":{},\"rob_allocations\":{},\"rob_commits\":{},\"rename_writes\":{},\"lsq_loads\":{},\"lsq_stores\":{},\"lsq_load_bytes\":{},\"lsq_store_bytes\":{},\"store_load_forwarding_candidates\":{},\"store_load_forwarding_matches\":{},\"store_load_forwarding_suppressed\":{},\"store_load_forwarding_address_mismatches\":{},\"store_load_forwarding_byte_mismatches\":{},\"rob\":{},\"rename\":{},\"lsq\":{},\"iq\":{},\"issue\":{},\"writeback_port\":{},\"iew\":{},\"commit\":{},\"branch_event\":{},\"branch_repair\":{},\"branch_direction_mismatch\":{},\"branch_target_mismatch\":{},\"writeback_calendar\":{},\"snapshot\":{},\"iew_predicted_taken_incorrect\":{},\"iew_predicted_not_taken_incorrect\":{},\"iew_producer_insts\":{},\"iew_consumer_insts\":{},\"iq_branch_insts_issued\":{},\"fu_latency_instructions\":{},\"fu_latency_cycles\":{},{},{},{},{}{},\"lsq_store_conditional_failures\":{},\"max_rob_occupancy\":{},\"max_lsq_occupancy\":{},\"rename_map_entries\":{}}}",
                execution_mode,
                self.o3_runtime_stats_epoch,
                self.o3_runtime_stats_reset_tick,
                checkpoint_restore_count,
                checkpoint_restore_label,
                checkpoint_restore_tick,
                checkpoint_restore_manifest_tick,
                checkpoint_restore_payload_bytes,
                checkpoint_restore,
                event_window,
                event_summary,
                self.o3_runtime.instructions(),
                self.o3_runtime.rob_allocations(),
                self.o3_runtime.rob_commits(),
                self.o3_runtime.rename_writes(),
                self.o3_runtime.lsq_loads(),
                self.o3_runtime.lsq_stores(),
                self.o3_runtime.lsq_load_bytes(),
                self.o3_runtime.lsq_store_bytes(),
                self.o3_runtime.lsq_store_to_load_forwarding_candidates(),
                self.o3_runtime.lsq_store_to_load_forwarding_matches(),
                self.o3_runtime.lsq_store_to_load_forwarding_suppressed(),
                self.o3_runtime
                    .lsq_store_to_load_forwarding_address_mismatches(),
                self.o3_runtime
                    .lsq_store_to_load_forwarding_byte_mismatches(),
                rob,
                rename,
                lsq,
                iq,
                issue,
                writeback_port,
                iew,
                commit,
                branch_event,
                branch_repair,
                branch_direction_mismatch,
                branch_target_mismatch,
                writeback_calendar,
                snapshot,
                self.o3_runtime.iew_predicted_taken_incorrect(),
                self.o3_runtime.iew_predicted_not_taken_incorrect(),
                self.o3_runtime.iew_producer_insts(),
                self.o3_runtime.iew_consumer_insts(),
                self.o3_runtime.iq_branch_insts_issued(),
                self.o3_runtime.fu_latency_instructions(),
                self.o3_runtime.fu_latency_cycles(),
                fu_latency_classes,
                lsq_data_latency,
                lsq_operations,
                lsq_orderings,
                live_retire_gate,
                self.o3_runtime.lsq_store_conditional_failures(),
                self.o3_runtime.max_rob_occupancy(),
                self.o3_runtime.max_lsq_occupancy(),
                self.o3_runtime.rename_map_entries(),
            )
        } else {
            String::new()
        };
        let btb_mispredict_due_to_btb_miss =
            branch_target_kind_counts_json(self.branch_target_buffer_mispredict_due_to_btb_miss);
        let lookups = branch_target_kind_counts_json(self.branch_predictor_lookups);
        let squashes = branch_target_kind_counts_json_with_total(
            self.branch_predictor_squashes,
            self.branch_predictor_squashes_total,
        );
        let target_provider =
            branch_target_provider_counts_json(self.branch_predictor_target_provider);
        let committed = branch_target_kind_counts_json(self.branch_predictor_committed);
        let mispredicted = branch_target_kind_counts_json(self.branch_predictor_mispredicted);
        let corrected = branch_target_kind_counts_json(self.branch_predictor_corrected);
        let target_wrong = branch_target_kind_counts_json(self.branch_predictor_target_wrong);
        let mispredict_due_to_predictor =
            branch_target_kind_counts_json(self.branch_predictor_mispredict_due_to_predictor);
        let ras = format!(
            "{{\"pushes\":{},\"pops\":{},\"squashes\":{},\"used\":{},\"correct\":{},\"incorrect\":{}}}",
            self.branch_predictor_ras.pushes(),
            self.branch_predictor_ras.pops(),
            self.branch_predictor_ras.squashes(),
            self.branch_predictor_ras.used(),
            self.branch_predictor_ras.correct(),
            self.branch_predictor_ras.incorrect()
        );
        let gshare = branch_predictor_counter_json(
            self.branch_predictor_gshare.lookups,
            self.branch_predictor_gshare.history_updates,
            self.branch_predictor_gshare.updates,
            self.branch_predictor_gshare.squashes,
        );
        let bimode = branch_predictor_counter_json(
            self.branch_predictor_bimode.lookups,
            self.branch_predictor_bimode.history_updates,
            self.branch_predictor_bimode.updates,
            self.branch_predictor_bimode.squashes,
        );
        let tage_sc_l = format!(
            "{{\"lookups\":{},\"history_updates\":{},\"updates\":{},\"repairs\":{},\"selected_rollbacks\":{}}}",
            self.branch_predictor_tage_sc_l.lookups,
            self.branch_predictor_tage_sc_l.history_updates,
            self.branch_predictor_tage_sc_l.updates,
            self.branch_predictor_tage_sc_l.repairs,
            self.branch_predictor_tage_sc_l.selected_rollbacks
        );
        let multiperspective_perceptron = format!(
            "{{\"lookups\":{},\"updates\":{},\"selected_rollbacks\":{}}}",
            self.branch_predictor_multiperspective_perceptron.lookups,
            self.branch_predictor_multiperspective_perceptron.updates,
            self.branch_predictor_multiperspective_perceptron
                .selected_rollbacks
        );
        let stage_advanced =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_advanced);
        let stage_advanced_cycles =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_advanced_cycles);
        let stage_retired =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_retired);
        let stage_retired_cycles =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_retired_cycles);
        let stage_resource_blocked_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_resource_blocked_cycles,
        );
        let stall_cause_fetch_wait = in_order_pipeline_stall_cause_stage_summary_json(
            self.in_order_pipeline_fetch_wait_records,
            self.in_order_pipeline_fetch_wait_stage_records,
            self.in_order_pipeline_fetch_wait_stage_resource_blocked,
            self.in_order_pipeline_fetch_wait_stage_resource_blocked_cycles,
            self.in_order_pipeline_fetch_wait_stage_ordering_blocked,
            self.in_order_pipeline_fetch_wait_stage_ordering_blocked_cycles,
        );
        let stall_cause_data_wait = in_order_pipeline_stall_cause_stage_summary_json(
            self.in_order_pipeline_data_wait_records,
            self.in_order_pipeline_data_wait_stage_records,
            self.in_order_pipeline_data_wait_stage_resource_blocked,
            self.in_order_pipeline_data_wait_stage_resource_blocked_cycles,
            self.in_order_pipeline_data_wait_stage_ordering_blocked,
            self.in_order_pipeline_data_wait_stage_ordering_blocked_cycles,
        );
        let stall_cause_execute_wait = in_order_pipeline_stall_cause_stage_summary_json(
            self.in_order_pipeline_execute_wait_records,
            self.in_order_pipeline_execute_wait_stage_records,
            self.in_order_pipeline_execute_wait_stage_resource_blocked,
            self.in_order_pipeline_execute_wait_stage_resource_blocked_cycles,
            self.in_order_pipeline_execute_wait_stage_ordering_blocked,
            self.in_order_pipeline_execute_wait_stage_ordering_blocked_cycles,
        );
        let stage_ordering_blocked_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_ordering_blocked_cycles,
        );
        let stage_flushed_cycles =
            in_order_pipeline_stage_summary_json(self.in_order_pipeline_stage_flushed_cycles);
        let stage_branch_prediction_flushed = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_branch_prediction_flushed,
        );
        let stage_branch_prediction_flushed_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_branch_prediction_flushed_cycles,
        );
        let cause_branch_prediction = in_order_pipeline_flush_redirect_cause_stage_summary_json(
            self.in_order_pipeline_branch_prediction_flush_records,
            self.in_order_pipeline_stage_branch_prediction_records,
            self.in_order_pipeline_stage_branch_prediction_flushed,
            self.in_order_pipeline_stage_branch_prediction_flushed_cycles,
        );
        let stage_trap_redirect_flushed = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_trap_redirect_flushed,
        );
        let stage_trap_redirect_flushed_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_trap_redirect_flushed_cycles,
        );
        let cause_trap_redirect = in_order_pipeline_flush_redirect_cause_stage_summary_json(
            self.in_order_pipeline_trap_redirect_flush_records,
            self.in_order_pipeline_stage_trap_redirect_records,
            self.in_order_pipeline_stage_trap_redirect_flushed,
            self.in_order_pipeline_stage_trap_redirect_flushed_cycles,
        );
        let stage_interrupt_redirect_flushed = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_interrupt_redirect_flushed,
        );
        let stage_interrupt_redirect_flushed_cycles = in_order_pipeline_stage_summary_json(
            self.in_order_pipeline_stage_interrupt_redirect_flushed_cycles,
        );
        let cause_interrupt_redirect = in_order_pipeline_flush_redirect_cause_stage_summary_json(
            self.in_order_pipeline_interrupt_redirect_flush_records,
            self.in_order_pipeline_stage_interrupt_redirect_records,
            self.in_order_pipeline_stage_interrupt_redirect_flushed,
            self.in_order_pipeline_stage_interrupt_redirect_flushed_cycles,
        );
        let redirect_cause_branch_prediction =
            in_order_pipeline_flush_redirect_cause_stage_summary_json(
                self.in_order_pipeline_branch_prediction_redirect_records,
                self.in_order_pipeline_stage_branch_prediction_records,
                self.in_order_pipeline_stage_branch_prediction_flushed,
                self.in_order_pipeline_stage_branch_prediction_flushed_cycles,
            );
        let redirect_cause_trap_redirect =
            in_order_pipeline_flush_redirect_cause_stage_summary_json(
                self.in_order_pipeline_trap_redirect_records,
                self.in_order_pipeline_stage_trap_redirect_records,
                self.in_order_pipeline_stage_trap_redirect_flushed,
                self.in_order_pipeline_stage_trap_redirect_flushed_cycles,
            );
        let redirect_cause_interrupt_redirect =
            in_order_pipeline_flush_redirect_cause_stage_summary_json(
                self.in_order_pipeline_interrupt_redirect_records,
                self.in_order_pipeline_stage_interrupt_redirect_records,
                self.in_order_pipeline_stage_interrupt_redirect_flushed,
                self.in_order_pipeline_stage_interrupt_redirect_flushed_cycles,
            );
        format!(
            "{{\"cpu\":{},\"pc\":\"0x{:x}\",\"committed_instructions\":{},\"in_order_pipeline\":{{\"cycles\":{},\"in_flight\":{},\"stage_widths\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_in_flight\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_max_in_flight\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_occupied_cycles\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_advanced\":{},\"stage_advanced_cycles\":{},\"stage_retired\":{},\"stage_retired_cycles\":{},\"stage_resource_blocked\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_resource_blocked_cycles\":{},\"stall_cause\":{{\"fetch_wait\":{},\"data_wait\":{},\"execute_wait\":{}}},\"stage_ordering_blocked\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_ordering_blocked_cycles\":{},\"stage_flushed\":{{\"fetch1\":{},\"fetch2\":{},\"decode\":{},\"execute\":{},\"commit\":{}}},\"stage_flushed_cycles\":{},\"stage_branch_prediction_flushed\":{},\"stage_branch_prediction_flushed_cycles\":{},\"stage_trap_redirect_flushed\":{},\"stage_trap_redirect_flushed_cycles\":{},\"stage_interrupt_redirect_flushed\":{},\"stage_interrupt_redirect_flushed_cycles\":{},\"flush_cause\":{{\"branch_prediction\":{},\"trap_redirect\":{},\"interrupt_redirect\":{}}},\"redirect_cause\":{{\"branch_prediction\":{},\"trap_redirect\":{},\"interrupt_redirect\":{}}},\"retired\":{},\"advanced\":{},\"flushed\":{},\"flush_cycles\":{},\"resource_blocked\":{},\"ordering_blocked\":{},\"stall_cycles\":{},\"fetch_wait_cycles\":{},\"data_wait_cycles\":{},\"execute_wait_cycles\":{},\"branch_predictions\":{},\"branch_mispredictions\":{},\"conditional_branch_predictions\":{},\"conditional_branch_predicted_taken\":{},\"conditional_branch_mispredictions\":{},\"branch_prediction_flushes\":{},\"branch_prediction_flush_cycles\":{},\"redirects\":{},\"branch_prediction_redirects\":{},\"interrupt_redirects\":{},\"interrupt_redirect_flushes\":{},\"interrupt_redirect_flush_cycles\":{},\"trap_redirects\":{},\"trap_redirect_flushes\":{},\"trap_redirect_flush_cycles\":{},\"branch_speculation_predictions\":{},\"branch_speculation_repairs\":{},\"branch_speculation_removed_youngers\":{},\"branch_speculation_max_pending\":{}}},\"branch_predictor\":{{\"btb\":{{\"lookups\":{},\"hits\":{},\"misses\":{},\"updates\":{},\"evictions\":{},\"mispredictions\":{},\"predicted_taken_misses\":{},\"mispredict_due_to_btb_miss\":{}}},\"lookups\":{},\"squashes\":{},\"target_provider\":{},\"indirect_hits\":{},\"indirect_mispredicted\":{},\"ras\":{},\"committed\":{},\"mispredicted\":{},\"corrected\":{},\"target_wrong\":{},\"mispredict_due_to_predictor\":{},\"gshare\":{},\"bimode\":{},\"tournament\":{{\"lookups\":{},\"history_updates\":{},\"updates\":{},\"squashes\":{},\"local_predictions\":{},\"global_predictions\":{}}},\"tage_sc_l\":{},\"multiperspective_perceptron\":{}}},\"data_loads\":{},\"data_stores\":{},\"data_atomics\":{},\"data_load_bytes\":{},\"data_store_bytes\":{},\"data_atomic_bytes\":{}{}{},\"registers\":{{{}}}}}",
            self.cpu,
            self.pc,
            self.committed_instructions,
            self.in_order_pipeline_cycles,
            self.in_order_pipeline_in_flight,
            self.in_order_pipeline_stage_widths.fetch1,
            self.in_order_pipeline_stage_widths.fetch2,
            self.in_order_pipeline_stage_widths.decode,
            self.in_order_pipeline_stage_widths.execute,
            self.in_order_pipeline_stage_widths.commit,
            self.in_order_pipeline_stage_in_flight.fetch1,
            self.in_order_pipeline_stage_in_flight.fetch2,
            self.in_order_pipeline_stage_in_flight.decode,
            self.in_order_pipeline_stage_in_flight.execute,
            self.in_order_pipeline_stage_in_flight.commit,
            self.in_order_pipeline_stage_max_in_flight.fetch1,
            self.in_order_pipeline_stage_max_in_flight.fetch2,
            self.in_order_pipeline_stage_max_in_flight.decode,
            self.in_order_pipeline_stage_max_in_flight.execute,
            self.in_order_pipeline_stage_max_in_flight.commit,
            self.in_order_pipeline_stage_occupied_cycles.fetch1,
            self.in_order_pipeline_stage_occupied_cycles.fetch2,
            self.in_order_pipeline_stage_occupied_cycles.decode,
            self.in_order_pipeline_stage_occupied_cycles.execute,
            self.in_order_pipeline_stage_occupied_cycles.commit,
            stage_advanced,
            stage_advanced_cycles,
            stage_retired,
            stage_retired_cycles,
            self.in_order_pipeline_stage_resource_blocked.fetch1,
            self.in_order_pipeline_stage_resource_blocked.fetch2,
            self.in_order_pipeline_stage_resource_blocked.decode,
            self.in_order_pipeline_stage_resource_blocked.execute,
            self.in_order_pipeline_stage_resource_blocked.commit,
            stage_resource_blocked_cycles,
            stall_cause_fetch_wait,
            stall_cause_data_wait,
            stall_cause_execute_wait,
            self.in_order_pipeline_stage_ordering_blocked.fetch1,
            self.in_order_pipeline_stage_ordering_blocked.fetch2,
            self.in_order_pipeline_stage_ordering_blocked.decode,
            self.in_order_pipeline_stage_ordering_blocked.execute,
            self.in_order_pipeline_stage_ordering_blocked.commit,
            stage_ordering_blocked_cycles,
            self.in_order_pipeline_stage_flushed.fetch1,
            self.in_order_pipeline_stage_flushed.fetch2,
            self.in_order_pipeline_stage_flushed.decode,
            self.in_order_pipeline_stage_flushed.execute,
            self.in_order_pipeline_stage_flushed.commit,
            stage_flushed_cycles,
            stage_branch_prediction_flushed,
            stage_branch_prediction_flushed_cycles,
            stage_trap_redirect_flushed,
            stage_trap_redirect_flushed_cycles,
            stage_interrupt_redirect_flushed,
            stage_interrupt_redirect_flushed_cycles,
            &cause_branch_prediction,
            &cause_trap_redirect,
            &cause_interrupt_redirect,
            &redirect_cause_branch_prediction,
            &redirect_cause_trap_redirect,
            &redirect_cause_interrupt_redirect,
            self.in_order_pipeline_retired,
            self.in_order_pipeline_advanced,
            self.in_order_pipeline_flushed,
            self.in_order_pipeline_flush_cycles,
            self.in_order_pipeline_resource_blocked,
            self.in_order_pipeline_ordering_blocked,
            self.in_order_pipeline_stall_cycles,
            self.in_order_pipeline_fetch_wait_cycles,
            self.in_order_pipeline_data_wait_cycles,
            self.in_order_pipeline_execute_wait_cycles,
            self.in_order_pipeline_branch_predictions,
            self.in_order_pipeline_branch_mispredictions,
            self.in_order_pipeline_conditional_branch_predictions,
            self.in_order_pipeline_conditional_branch_predicted_taken,
            self.in_order_pipeline_conditional_branch_mispredictions,
            self.in_order_pipeline_branch_prediction_flushes,
            self.in_order_pipeline_branch_prediction_flush_cycles,
            self.in_order_pipeline_redirects,
            self.in_order_pipeline_branch_prediction_redirects,
            self.in_order_pipeline_interrupt_redirects,
            self.in_order_pipeline_interrupt_redirect_flushes,
            self.in_order_pipeline_interrupt_redirect_flush_cycles,
            self.in_order_pipeline_trap_redirects,
            self.in_order_pipeline_trap_redirect_flushes,
            self.in_order_pipeline_trap_redirect_flush_cycles,
            self.in_order_pipeline_branch_speculation_predictions,
            self.in_order_pipeline_branch_speculation_repairs,
            self.in_order_pipeline_branch_speculation_removed_youngers,
            self.in_order_pipeline_branch_speculation_max_pending,
            self.branch_target_buffer_lookups,
            self.branch_target_buffer_hits,
            self.branch_target_buffer_misses,
            self.branch_target_buffer_updates,
            self.branch_target_buffer_evictions,
            self.branch_target_buffer_mispredictions,
            self.branch_target_buffer_predicted_taken_misses,
            btb_mispredict_due_to_btb_miss,
            lookups,
            squashes,
            target_provider,
            self.branch_predictor_indirect_hits,
            self.branch_predictor_indirect_mispredicted,
            ras,
            committed,
            mispredicted,
            corrected,
            target_wrong,
            mispredict_due_to_predictor,
            gshare,
            bimode,
            self.branch_predictor_tournament.lookups,
            self.branch_predictor_tournament.history_updates,
            self.branch_predictor_tournament.updates,
            self.branch_predictor_tournament.squashes,
            self.tournament_local_predictions,
            self.tournament_global_predictions,
            tage_sc_l,
            multiperspective_perceptron,
            self.data_loads,
            self.data_stores,
            self.data_atomics,
            self.data_load_bytes,
            self.data_store_bytes,
            self.data_atomic_bytes,
            o3_runtime,
            checker,
            registers
        )
    }
}
