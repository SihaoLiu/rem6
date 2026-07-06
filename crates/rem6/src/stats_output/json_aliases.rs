use rem6_stats::StatSnapshot;

use super::{json_record_for_derived_counter, snapshot_sample, snapshot_sample_value};

pub(super) fn append_gem5_json_alias_stats(snapshot: &StatSnapshot, records: &mut Vec<String>) {
    let mut next_id = snapshot
        .samples()
        .iter()
        .map(|sample| sample.id().get())
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    append_gem5_o3_json_alias_stats(snapshot, records, &mut next_id);
    append_gem5_in_order_pipeline_json_alias_stats(snapshot, records, &mut next_id);
}

fn append_gem5_o3_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
) {
    let Some(core_count) = snapshot_sample_value(snapshot, "sim.cores") else {
        return;
    };

    for cpu in 0..core_count {
        let alias_prefix = gem5_json_cpu_alias_prefix(core_count, cpu);
        append_gem5_o3_op_class_json_alias_stats(
            snapshot,
            records,
            next_id,
            cpu,
            core_count,
            &alias_prefix,
        );
        for (source_suffix, alias_suffix) in [
            ("iew.insts_to_commit", "iew.instsToCommit.total"),
            ("iew.writeback_count", "iew.writebackCount.total"),
            ("iew.producer_inst", "iew.producerInst.total"),
            ("iew.consumer_inst", "iew.consumerInst.total"),
        ] {
            append_gem5_o3_json_alias_from_sample(
                snapshot,
                records,
                next_id,
                cpu,
                source_suffix,
                &alias_prefix,
                alias_suffix,
            );
        }
        for (source_suffix, alias_suffix) in [
            ("iew.writeback_rate_ppm", "iew.wbRate"),
            ("iew.producer_consumer_fanout_ppm", "iew.wbFanout"),
        ] {
            append_gem5_o3_json_alias_from_sample(
                snapshot,
                records,
                next_id,
                cpu,
                source_suffix,
                &alias_prefix,
                alias_suffix,
            );
        }
    }
}

fn append_gem5_in_order_pipeline_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
) {
    let Some(core_count) = snapshot_sample_value(snapshot, "sim.cores") else {
        return;
    };
    for cpu in 0..core_count {
        let pipeline_alias_prefix = format!(
            "{}.pipeline.inOrder",
            gem5_json_cpu_alias_prefix(core_count, cpu)
        );
        for (source_cause, alias_cause) in [
            ("fetch_wait", "fetchWait"),
            ("data_wait", "dataWait"),
            ("execute_wait", "executeWait"),
        ] {
            for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
                for (source_name, alias_name) in [
                    ("resource_blocked", "resourceBlocked"),
                    ("resource_blocked_cycles", "resourceBlockedCycles"),
                    ("ordering_blocked", "orderingBlocked"),
                    ("ordering_blocked_cycles", "orderingBlockedCycles"),
                ] {
                    append_gem5_json_alias_from_paths(
                        snapshot,
                        records,
                        next_id,
                        &format!(
                            "sim.cpu{cpu}.pipeline.in_order.stall_cause.{source_cause}.stage.{stage}.{source_name}"
                        ),
                        &format!(
                            "{pipeline_alias_prefix}.stallCause.{alias_cause}.stage.{stage}.{alias_name}"
                        ),
                    );
                }
            }
        }
        for (source_family, alias_family) in [
            ("flush_cause", "flushCause"),
            ("redirect_cause", "redirectCause"),
        ] {
            for (source_cause, alias_cause) in [
                ("branch_prediction", "branchPrediction"),
                ("interrupt_redirect", "interruptRedirect"),
                ("trap_redirect", "trapRedirect"),
            ] {
                for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
                    for (source_name, alias_name) in
                        [("flushed", "flushed"), ("flushed_cycles", "flushedCycles")]
                    {
                        append_gem5_json_alias_from_paths(
                            snapshot,
                            records,
                            next_id,
                            &format!(
                                "sim.cpu{cpu}.pipeline.in_order.{source_family}.{source_cause}.stage.{stage}.{source_name}"
                            ),
                            &format!(
                                "{pipeline_alias_prefix}.{alias_family}.{alias_cause}.stage.{stage}.{alias_name}"
                            ),
                        );
                    }
                }
            }
        }
    }
    for cpu in 0..core_count {
        let pipeline_alias_prefix = format!(
            "{}.pipeline.inOrder",
            gem5_json_cpu_alias_prefix(core_count, cpu)
        );
        for (source_name, alias_name) in [
            ("advanced", "advanced"),
            ("flushed", "flushed"),
            ("flush_cycles", "flushCycles"),
            ("resource_blocked", "resourceBlocked"),
            ("ordering_blocked", "orderingBlocked"),
            ("stall_cycles", "stallCycles"),
            ("fetch_wait_cycles", "fetchWaitCycles"),
            ("data_wait_cycles", "dataWaitCycles"),
            ("execute_wait_cycles", "executeWaitCycles"),
            ("branch_prediction_flushes", "branchPredictionFlushes"),
            (
                "branch_prediction_flush_cycles",
                "branchPredictionFlushCycles",
            ),
            ("redirects", "redirects"),
            ("branch_prediction_redirects", "branchPredictionRedirects"),
            ("interrupt_redirects", "interruptRedirects"),
            ("interrupt_redirect_flushes", "interruptRedirectFlushes"),
            (
                "interrupt_redirect_flush_cycles",
                "interruptRedirectFlushCycles",
            ),
            ("trap_redirects", "trapRedirects"),
            ("trap_redirect_flushes", "trapRedirectFlushes"),
            ("trap_redirect_flush_cycles", "trapRedirectFlushCycles"),
            (
                "branch_speculation_predictions",
                "branchSpeculationPredictions",
            ),
            ("branch_speculation_repairs", "branchSpeculationRepairs"),
            (
                "branch_speculation_removed_youngers",
                "branchSpeculationRemovedYoungers",
            ),
            (
                "branch_speculation_max_pending",
                "branchSpeculationMaxPending",
            ),
        ] {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!("sim.cpu{cpu}.pipeline.in_order.{source_name}"),
                &format!("{pipeline_alias_prefix}.{alias_name}"),
            );
        }
    }
    for cpu in 0..core_count {
        let pipeline_alias_prefix = format!(
            "{}.pipeline.inOrder",
            gem5_json_cpu_alias_prefix(core_count, cpu)
        );
        for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
            for (source_name, alias_name) in [
                ("occupied_cycles", "occupiedCycles"),
                ("advanced", "advanced"),
                ("advanced_cycles", "advancedCycles"),
                ("retired", "retired"),
                ("retired_cycles", "retiredCycles"),
                ("resource_blocked", "resourceBlocked"),
                ("resource_blocked_cycles", "resourceBlockedCycles"),
                ("ordering_blocked", "orderingBlocked"),
                ("ordering_blocked_cycles", "orderingBlockedCycles"),
                ("flushed", "flushed"),
                ("flushed_cycles", "flushedCycles"),
                ("branch_prediction_flushed", "branchPredictionFlushed"),
                (
                    "branch_prediction_flushed_cycles",
                    "branchPredictionFlushedCycles",
                ),
                ("interrupt_redirect_flushed", "interruptRedirectFlushed"),
                (
                    "interrupt_redirect_flushed_cycles",
                    "interruptRedirectFlushedCycles",
                ),
                ("trap_redirect_flushed", "trapRedirectFlushed"),
                ("trap_redirect_flushed_cycles", "trapRedirectFlushedCycles"),
            ] {
                append_gem5_json_alias_from_paths(
                    snapshot,
                    records,
                    next_id,
                    &format!("sim.cpu{cpu}.pipeline.in_order.stage.{stage}.{source_name}"),
                    &format!("{pipeline_alias_prefix}.stage.{stage}.{alias_name}"),
                );
            }
        }
    }
}

fn append_gem5_o3_op_class_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    core_count: u64,
    alias_prefix: &str,
) {
    for (source_suffix, alias_suffix) in [
        ("iq.issued_inst_type.mem_read", "iq.issuedInstType.MemRead"),
        (
            "iq.issued_inst_type.mem_write",
            "iq.issuedInstType.MemWrite",
        ),
        ("iq.issued_inst_type.int_mul", "iq.issuedInstType.IntMult"),
        ("iq.issued_inst_type.int_div", "iq.issuedInstType.IntDiv"),
        (
            "commit.committed_inst_type.mem_read",
            "commit.committedInstType.MemRead",
        ),
        (
            "commit.committed_inst_type.mem_write",
            "commit.committedInstType.MemWrite",
        ),
        (
            "commit.committed_inst_type.int_mul",
            "commit.committedInstType.IntMult",
        ),
        (
            "commit.committed_inst_type.int_div",
            "commit.committedInstType.IntDiv",
        ),
    ] {
        append_gem5_o3_json_alias_from_sample(
            snapshot,
            records,
            next_id,
            cpu,
            source_suffix,
            alias_prefix,
            alias_suffix,
        );
    }
    if core_count != 1 {
        return;
    }
    for (source_suffix, alias_suffix) in [
        (
            "iq.issued_inst_type.float_add",
            "iq.issuedInstType.FloatAdd",
        ),
        (
            "iq.issued_inst_type.float_compare",
            "iq.issuedInstType.FloatCmp",
        ),
        (
            "iq.issued_inst_type.float_misc",
            "iq.issuedInstType.FloatMisc",
        ),
        (
            "iq.issued_inst_type.float_mul",
            "iq.issuedInstType.FloatMult",
        ),
        (
            "iq.issued_inst_type.float_fma",
            "iq.issuedInstType.FloatMultAcc",
        ),
        (
            "iq.issued_inst_type.float_div",
            "iq.issuedInstType.FloatDiv",
        ),
        (
            "iq.issued_inst_type.float_sqrt",
            "iq.issuedInstType.FloatSqrt",
        ),
        (
            "iq.issued_inst_type.vector_integer_mul",
            "iq.issuedInstType.SimdMult",
        ),
        (
            "iq.issued_inst_type.vector_integer_div",
            "iq.issuedInstType.SimdDiv",
        ),
        (
            "iq.issued_inst_type.vector_float_add",
            "iq.issuedInstType.SimdFloatAdd",
        ),
        (
            "iq.issued_inst_type.vector_float_compare",
            "iq.issuedInstType.SimdFloatCmp",
        ),
        (
            "iq.issued_inst_type.vector_float_misc",
            "iq.issuedInstType.SimdFloatMisc",
        ),
        (
            "iq.issued_inst_type.vector_float_mul",
            "iq.issuedInstType.SimdFloatMult",
        ),
        (
            "iq.issued_inst_type.vector_float_fma",
            "iq.issuedInstType.SimdFloatMultAcc",
        ),
        (
            "iq.issued_inst_type.vector_float_div",
            "iq.issuedInstType.SimdFloatDiv",
        ),
        (
            "iq.issued_inst_type.vector_float_sqrt",
            "iq.issuedInstType.SimdFloatSqrt",
        ),
        (
            "commit.committed_inst_type.float_add",
            "commit.committedInstType.FloatAdd",
        ),
        (
            "commit.committed_inst_type.float_compare",
            "commit.committedInstType.FloatCmp",
        ),
        (
            "commit.committed_inst_type.float_misc",
            "commit.committedInstType.FloatMisc",
        ),
        (
            "commit.committed_inst_type.float_mul",
            "commit.committedInstType.FloatMult",
        ),
        (
            "commit.committed_inst_type.float_fma",
            "commit.committedInstType.FloatMultAcc",
        ),
        (
            "commit.committed_inst_type.float_div",
            "commit.committedInstType.FloatDiv",
        ),
        (
            "commit.committed_inst_type.float_sqrt",
            "commit.committedInstType.FloatSqrt",
        ),
        (
            "commit.committed_inst_type.vector_integer_mul",
            "commit.committedInstType.SimdMult",
        ),
        (
            "commit.committed_inst_type.vector_integer_div",
            "commit.committedInstType.SimdDiv",
        ),
        (
            "commit.committed_inst_type.vector_float_add",
            "commit.committedInstType.SimdFloatAdd",
        ),
        (
            "commit.committed_inst_type.vector_float_compare",
            "commit.committedInstType.SimdFloatCmp",
        ),
        (
            "commit.committed_inst_type.vector_float_misc",
            "commit.committedInstType.SimdFloatMisc",
        ),
        (
            "commit.committed_inst_type.vector_float_mul",
            "commit.committedInstType.SimdFloatMult",
        ),
        (
            "commit.committed_inst_type.vector_float_fma",
            "commit.committedInstType.SimdFloatMultAcc",
        ),
        (
            "commit.committed_inst_type.vector_float_div",
            "commit.committedInstType.SimdFloatDiv",
        ),
        (
            "commit.committed_inst_type.vector_float_sqrt",
            "commit.committedInstType.SimdFloatSqrt",
        ),
    ] {
        append_gem5_o3_json_alias_from_sample(
            snapshot,
            records,
            next_id,
            cpu,
            source_suffix,
            alias_prefix,
            alias_suffix,
        );
    }
}

fn append_gem5_o3_json_alias_from_sample(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    source_suffix: &str,
    alias_prefix: &str,
    alias_suffix: &str,
) {
    let source_path = format!("sim.cpu{cpu}.o3.{source_suffix}");
    let alias_path = format!("{alias_prefix}.{alias_suffix}");
    append_gem5_json_alias_from_paths(snapshot, records, next_id, &source_path, &alias_path);
}

fn append_gem5_json_alias_from_paths(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    source_path: &str,
    alias_path: &str,
) {
    let Some(source) = snapshot_sample(snapshot, source_path) else {
        return;
    };
    if snapshot_sample(snapshot, alias_path).is_none() {
        records.push(json_record_for_derived_counter(
            *next_id,
            alias_path,
            source.unit(),
            source.value(),
            source.reset_policy(),
        ));
        *next_id = next_id.saturating_add(1);
    }
}

fn gem5_json_cpu_alias_prefix(core_count: u64, cpu: u64) -> String {
    if core_count == 1 {
        "system.cpu".to_string()
    } else {
        format!("system.cpu{cpu}")
    }
}
