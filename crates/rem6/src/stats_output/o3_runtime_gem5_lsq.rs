use rem6_cpu::{O3RuntimeLsqOperation, O3RuntimeStats};
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::o3_lsq_aliases::{
    O3LsqDataResponseMetric, O3_LSQ_DATA_RESPONSE_GEM5_ALIASES, O3_LSQ_OPERATION_GEM5_ALIASES,
    O3_LSQ_ORDERING_GEM5_ALIASES, O3_LSQ_TOTAL_ALIAS,
};
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
    for alias in O3_LSQ_OPERATION_GEM5_ALIASES {
        let operation = alias.operation();
        let value = o3.lsq_operation_count(operation);
        lsq_operation_total = lsq_operation_total.saturating_add(value);
        let operation_alias = alias.alias();
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
        format!("{gem5_cpu_alias_prefix}.lsq0.operation.{O3_LSQ_TOTAL_ALIAS}"),
        lsq_operation_total,
    )?;

    let mut lsq_ordering_total = 0_u64;
    for alias in O3_LSQ_ORDERING_GEM5_ALIASES {
        let ordering = alias.ordering();
        let value = o3.lsq_ordering_count(ordering);
        lsq_ordering_total = lsq_ordering_total.saturating_add(value);
        increment_count_stat(
            stats,
            format!("{gem5_cpu_alias_prefix}.lsq0.ordering.{}", alias.alias()),
            value,
        )?;
    }
    increment_count_stat(
        stats,
        format!("{gem5_cpu_alias_prefix}.lsq0.ordering.{O3_LSQ_TOTAL_ALIAS}"),
        lsq_ordering_total,
    )?;

    for alias in O3_LSQ_DATA_RESPONSE_GEM5_ALIASES {
        let name = alias.alias();
        let unit = alias.unit();
        let value = o3_lsq_data_response_value(o3, alias.metric());
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.lsq0.dataResponse.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
        increment_stat(
            stats,
            &format!(
                "{gem5_cpu_alias_prefix}.lsq0.operation.{O3_LSQ_TOTAL_ALIAS}.dataResponse.{name}"
            ),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for operation_alias in O3_LSQ_OPERATION_GEM5_ALIASES {
        let operation = operation_alias.operation();
        let operation_alias = operation_alias.alias();
        for alias in O3_LSQ_DATA_RESPONSE_GEM5_ALIASES {
            let name = alias.alias();
            let unit = alias.unit();
            let value = o3_lsq_operation_data_response_value(o3, operation, alias.metric());
            increment_stat(
                stats,
                &format!("{gem5_cpu_alias_prefix}.lsq0.dataResponse.{operation_alias}.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
            increment_stat(
                stats,
                &format!(
                    "{gem5_cpu_alias_prefix}.lsq0.operation.{operation_alias}.dataResponse.{name}"
                ),
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

fn o3_lsq_data_response_value(o3: O3RuntimeStats, metric: O3LsqDataResponseMetric) -> u64 {
    match metric {
        O3LsqDataResponseMetric::Samples => o3.lsq_data_latency_samples(),
        O3LsqDataResponseMetric::Ticks => o3.lsq_data_latency_ticks(),
        O3LsqDataResponseMetric::MaxTicks => o3.lsq_data_latency_max_ticks(),
        O3LsqDataResponseMetric::MinTicks => o3.lsq_data_latency_min_ticks(),
        O3LsqDataResponseMetric::AvgTicks => o3.lsq_data_latency_avg_ticks(),
    }
}

fn o3_lsq_operation_data_response_value(
    o3: O3RuntimeStats,
    operation: O3RuntimeLsqOperation,
    metric: O3LsqDataResponseMetric,
) -> u64 {
    match metric {
        O3LsqDataResponseMetric::Samples => o3.lsq_operation_latency_samples(operation),
        O3LsqDataResponseMetric::Ticks => o3.lsq_operation_latency_ticks(operation),
        O3LsqDataResponseMetric::MaxTicks => o3.lsq_operation_latency_max_ticks(operation),
        O3LsqDataResponseMetric::MinTicks => o3.lsq_operation_latency_min_ticks(operation),
        O3LsqDataResponseMetric::AvgTicks => o3.lsq_operation_latency_avg_ticks(operation),
    }
}
