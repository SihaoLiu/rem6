use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::text::stats_snapshot_text;
use super::{
    increment_stat, stats_snapshot_json, Rem6CliError, Rem6ResourceAcquireStatsInputs,
    Rem6StatsOutput,
};

pub(crate) fn resource_acquire_stats_output(
    inputs: Rem6ResourceAcquireStatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.resource_acquire.boot_entry",
        "Address",
        StatResetPolicy::Constant,
        inputs.artifact.config.boot_entry(),
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.resources",
        "Count",
        StatResetPolicy::Constant,
        inputs.artifact.config.resource_count() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.required_resources",
        "Count",
        StatResetPolicy::Constant,
        inputs.artifact.required_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.acquired_resources",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.artifact.acquired_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.resolved_resources",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.artifact.resolved_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.acquired_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        inputs.artifact.acquired_bytes,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.suite_manifests",
        "Count",
        StatResetPolicy::Constant,
        inputs.artifact.suite_manifests,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.suite_required_resources",
        "Count",
        StatResetPolicy::Constant,
        inputs.artifact.suite_required_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.suite_acquired_resources",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.artifact.suite_acquired_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.suite_acquired_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        inputs.artifact.suite_acquired_bytes,
    )?;
    for (index, resource) in inputs.artifact.resources.iter().enumerate() {
        increment_stat(
            &mut stats,
            &format!("sim.resource_acquire.resource{index}.bytes"),
            "Byte",
            StatResetPolicy::Monotonic,
            resource.size_bytes,
        )?;
    }

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}
