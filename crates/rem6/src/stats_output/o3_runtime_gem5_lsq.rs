use rem6_cpu::{O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeStats};
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::Rem6CliError;

pub(super) fn emit_gem5_o3_lsq_alias_stats(
    stats: &mut StatsRegistry,
    gem5_cpu_alias_prefix: &str,
    o3: O3RuntimeStats,
) -> Result<(), Rem6CliError> {
    for (name, value) in [
        ("lsq0.loadBytes", o3.lsq_load_bytes()),
        ("lsq0.storeBytes", o3.lsq_store_bytes()),
    ] {
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.{name}"),
            "Byte",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }

    let mut lsq_operation_total = 0_u64;
    for operation in O3RuntimeLsqOperation::TRACKED {
        let value = o3.lsq_operation_count(operation);
        lsq_operation_total = lsq_operation_total.saturating_add(value);
        let operation_alias = o3_lsq_operation_alias(operation);
        increment_count_stat(
            stats,
            format!("{gem5_cpu_alias_prefix}.lsq0.operation.{operation_alias}"),
            value,
        )?;
        for (name, value) in [
            ("loadBytes", o3.lsq_operation_load_bytes(operation)),
            ("storeBytes", o3.lsq_operation_store_bytes(operation)),
        ] {
            increment_stat(
                stats,
                &format!("{gem5_cpu_alias_prefix}.lsq0.operation.{operation_alias}.{name}"),
                "Byte",
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
        for (name, value) in [
            (
                "storeConditionalFailures",
                o3.lsq_operation_store_conditional_failures(operation),
            ),
            (
                "storeLoadForwardingCandidates",
                o3.lsq_operation_forwarding_candidates(operation),
            ),
            (
                "storeLoadForwardingMatches",
                o3.lsq_operation_forwarding_matches(operation),
            ),
            (
                "storeLoadForwardingSuppressed",
                o3.lsq_operation_forwarding_suppressed(operation),
            ),
            (
                "storeLoadForwardingAddressMismatches",
                o3.lsq_operation_forwarding_address_mismatches(operation),
            ),
            (
                "storeLoadForwardingByteMismatches",
                o3.lsq_operation_forwarding_byte_mismatches(operation),
            ),
        ] {
            increment_count_stat(
                stats,
                format!("{gem5_cpu_alias_prefix}.lsq0.operation.{operation_alias}.{name}"),
                value,
            )?;
        }
    }
    increment_count_stat(
        stats,
        format!("{gem5_cpu_alias_prefix}.lsq0.operation.total"),
        lsq_operation_total,
    )?;

    let mut lsq_ordering_total = 0_u64;
    for ordering in O3RuntimeLsqOrdering::TRACKED {
        let value = o3.lsq_ordering_count(ordering);
        lsq_ordering_total = lsq_ordering_total.saturating_add(value);
        increment_count_stat(
            stats,
            format!(
                "{gem5_cpu_alias_prefix}.lsq0.ordering.{}",
                o3_lsq_ordering_alias(ordering)
            ),
            value,
        )?;
    }
    increment_count_stat(
        stats,
        format!("{gem5_cpu_alias_prefix}.lsq0.ordering.total"),
        lsq_ordering_total,
    )?;

    for (name, unit, value) in [
        ("samples", "Count", o3.lsq_data_latency_samples()),
        ("totalLatency", "Tick", o3.lsq_data_latency_ticks()),
        ("maxLatency", "Tick", o3.lsq_data_latency_max_ticks()),
        ("minLatency", "Tick", o3.lsq_data_latency_min_ticks()),
        ("avgLatency", "Tick", o3.lsq_data_latency_avg_ticks()),
    ] {
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.lsq0.dataResponse.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        let operation_alias = o3_lsq_operation_alias(operation);
        for (name, unit, value) in [
            (
                "samples",
                "Count",
                o3.lsq_operation_latency_samples(operation),
            ),
            (
                "totalLatency",
                "Tick",
                o3.lsq_operation_latency_ticks(operation),
            ),
            (
                "maxLatency",
                "Tick",
                o3.lsq_operation_latency_max_ticks(operation),
            ),
            (
                "minLatency",
                "Tick",
                o3.lsq_operation_latency_min_ticks(operation),
            ),
            (
                "avgLatency",
                "Tick",
                o3.lsq_operation_latency_avg_ticks(operation),
            ),
        ] {
            increment_stat(
                stats,
                &format!("{gem5_cpu_alias_prefix}.lsq0.dataResponse.{operation_alias}.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    Ok(())
}

fn increment_count_stat(
    stats: &mut StatsRegistry,
    name: String,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(stats, &name, "Count", StatResetPolicy::Monotonic, value)
}

fn o3_lsq_operation_alias(operation: O3RuntimeLsqOperation) -> &'static str {
    match operation {
        O3RuntimeLsqOperation::None => "none",
        O3RuntimeLsqOperation::Load => "load",
        O3RuntimeLsqOperation::Store => "store",
        O3RuntimeLsqOperation::LoadReserved => "loadReserved",
        O3RuntimeLsqOperation::StoreConditional => "storeConditional",
        O3RuntimeLsqOperation::Atomic => "atomic",
        O3RuntimeLsqOperation::FloatLoad => "floatLoad",
        O3RuntimeLsqOperation::FloatStore => "floatStore",
        O3RuntimeLsqOperation::VectorLoad => "vectorLoad",
        O3RuntimeLsqOperation::VectorStore => "vectorStore",
    }
}

fn o3_lsq_ordering_alias(ordering: O3RuntimeLsqOrdering) -> &'static str {
    match ordering {
        O3RuntimeLsqOrdering::None => "none",
        O3RuntimeLsqOrdering::Acquire => "acquire",
        O3RuntimeLsqOrdering::Release => "release",
        O3RuntimeLsqOrdering::AcquireRelease => "acquireRelease",
    }
}
