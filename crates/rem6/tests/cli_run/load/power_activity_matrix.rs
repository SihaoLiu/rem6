use std::{fs, process::Command};

use rem6_power::{PowerAnalysisRecord, PowerStateKind};
use serde_json::Value;

use super::*;

#[derive(Clone, Copy)]
struct PowerActivityCase {
    name: &'static str,
    extra_args: &'static [&'static str],
    format: &'static str,
    expected_active_targets: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct CanonicalPowerTarget {
    target: &'static str,
    active_path: &'static str,
    base_temperature_c: f64,
}

const POWER_ACTIVITY_CASES: &[PowerActivityCase] = &[
    PowerActivityCase {
        name: "direct",
        extra_args: &["--memory-system", "direct"],
        format: "mcpat-xml",
        expected_active_targets: &["memory.transport"],
    },
    PowerActivityCase {
        name: "dram",
        extra_args: &["--dram-memory"],
        format: "dsent-csv",
        expected_active_targets: &["memory.transport", "memory.dram"],
    },
    PowerActivityCase {
        name: "cache",
        extra_args: &[
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--instruction-cache-l3-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--data-cache-l3-protocol",
            "msi",
        ],
        format: "mcpat-xml",
        expected_active_targets: &[
            "cpu.instruction_cache",
            "cpu.data_cache",
            "memory.cache.l2",
            "memory.cache.l3",
            "memory.transport",
        ],
    },
    PowerActivityCase {
        name: "hierarchy",
        extra_args: &["--memory-system", "cache-fabric-dram"],
        format: "dsent-csv",
        expected_active_targets: &[
            "cpu.instruction_cache",
            "cpu.data_cache",
            "memory.cache.l2",
            "memory.cache.l3",
            "memory.transport",
            "memory.fabric",
            "memory.dram",
        ],
    },
];

const CANONICAL_POWER_TARGETS: &[CanonicalPowerTarget] = &[
    CanonicalPowerTarget {
        target: "cpu.instruction_cache",
        active_path: "/memory_resources/cache/instruction/l1/active",
        base_temperature_c: 39.0,
    },
    CanonicalPowerTarget {
        target: "cpu.data_cache",
        active_path: "/memory_resources/cache/data/l1/active",
        base_temperature_c: 39.0,
    },
    CanonicalPowerTarget {
        target: "memory.cache.l2",
        active_path: "/memory_resources/cache/l2/active",
        base_temperature_c: 38.5,
    },
    CanonicalPowerTarget {
        target: "memory.cache.l3",
        active_path: "/memory_resources/cache/l3/active",
        base_temperature_c: 38.5,
    },
    CanonicalPowerTarget {
        target: "memory.transport",
        active_path: "/memory_resources/transport/active",
        base_temperature_c: 37.0,
    },
    CanonicalPowerTarget {
        target: "memory.fabric",
        active_path: "/memory_resources/fabric/active",
        base_temperature_c: 37.5,
    },
    CanonicalPowerTarget {
        target: "memory.dram",
        active_path: "/memory_resources/dram/active",
        base_temperature_c: 38.0,
    },
];

fn imported_power_analysis(format: &str, contents: &str) -> PowerAnalysisExport {
    match format {
        "mcpat-xml" => PowerAnalysisExport::from_mcpat_compatible_xml(contents).unwrap(),
        "dsent-csv" => PowerAnalysisExport::from_dsent_compatible_csv(contents).unwrap(),
        _ => panic!("unsupported power format {format}"),
    }
}

fn json_u64(artifact: &Value, path: &str, row: &str) -> u64 {
    artifact
        .pointer(path)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("row {row} missing unsigned JSON path {path}"))
}

fn record_for_target<'a>(
    records: &'a [PowerAnalysisRecord],
    target: &str,
) -> Option<&'a PowerAnalysisRecord> {
    records.iter().find(|record| record.target() == target)
}

fn assert_active_power_record(row: &str, record: &PowerAnalysisRecord, base_temperature_c: f64) {
    assert!(
        record.dynamic_watts() > 0.0,
        "row {row} target {} has non-positive dynamic watts",
        record.target()
    );
    assert!(
        record.residency_ticks(PowerStateKind::On) > 0,
        "row {row} target {} has no On residency",
        record.target()
    );
    assert!(
        record.temperature_c() >= base_temperature_c,
        "row {row} target {} temperature {} is below base {base_temperature_c}",
        record.target(),
        record.temperature_c()
    );
}

#[test]
fn rem6_run_power_activity_matches_canonical_resource_matrix() {
    let program = riscv64_load_store_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let mut dram_mismatches = Vec::new();

    for case in POWER_ACTIVITY_CASES.iter().copied() {
        let binary_name = format!("power-activity-matrix-{}", case.name);
        let artifact_name = format!("power-activity-matrix-{}-artifact", case.name);
        let power_name = format!("power-activity-matrix-{}-power", case.name);
        let path = temp_binary(&binary_name, &elf);
        let artifact_path = temp_output(&artifact_name);
        let power_path = temp_output(&power_name);

        let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
        command.args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "400",
            "--execute",
            "--output",
            artifact_path.to_str().unwrap(),
            "--power-format",
            case.format,
            "--power-output",
            power_path.to_str().unwrap(),
        ]);
        command.args(case.extra_args);
        let output = command.output().unwrap();

        assert!(
            output.status.success(),
            "row {} stderr: {}",
            case.name,
            String::from_utf8_lossy(&output.stderr)
        );
        let artifact_text = fs::read_to_string(&artifact_path).unwrap();
        let artifact: Value = serde_json::from_str(&artifact_text).unwrap();
        let power_text = fs::read_to_string(&power_path).unwrap();
        let imported = imported_power_analysis(case.format, &power_text);
        let records = imported.records();

        let core = record_for_target(records, "cpu0.core")
            .unwrap_or_else(|| panic!("row {} missing cpu0.core", case.name));
        assert_active_power_record(case.name, core, 40.0);

        for mapping in CANONICAL_POWER_TARGETS.iter().copied() {
            let active = json_u64(&artifact, mapping.active_path, case.name) > 0;
            let expected_active = case.expected_active_targets.contains(&mapping.target);
            assert_eq!(
                active, expected_active,
                "row {} canonical activity mismatch for {} at {}",
                case.name, mapping.target, mapping.active_path
            );

            let record = record_for_target(records, mapping.target);
            assert_eq!(
                record.is_some(),
                active,
                "row {} target {} presence must follow {}",
                case.name,
                mapping.target,
                mapping.active_path
            );
            if let Some(record) = record {
                assert_active_power_record(case.name, record, mapping.base_temperature_c);
            }
        }

        if let Some(dram_record) = record_for_target(records, "memory.dram") {
            let events = json_u64(&artifact, "/memory_resources/dram/activity", case.name);
            let commands = json_u64(&artifact, "/memory_resources/dram/commands", case.name);
            let refreshes = json_u64(&artifact, "/memory_resources/dram/refreshes", case.name);
            let active_powerdown_entries = json_u64(
                &artifact,
                "/memory_resources/dram/low_power/active_powerdown/entries",
                case.name,
            );
            let precharge_powerdown_entries = json_u64(
                &artifact,
                "/memory_resources/dram/low_power/precharge_powerdown/entries",
                case.name,
            );
            let self_refresh_entries = json_u64(
                &artifact,
                "/memory_resources/dram/low_power/self_refresh/entries",
                case.name,
            );
            let exits = json_u64(
                &artifact,
                "/memory_resources/dram/low_power/exits",
                case.name,
            );
            let reads = json_u64(&artifact, "/memory_resources/dram/reads", case.name);
            let writes = json_u64(&artifact, "/memory_resources/dram/writes", case.name);
            let read_bytes = json_u64(&artifact, "/memory_resources/dram/read_bytes", case.name);
            let write_bytes = json_u64(&artifact, "/memory_resources/dram/write_bytes", case.name);
            let operations = commands
                .saturating_add(refreshes)
                .saturating_add(active_powerdown_entries)
                .saturating_add(precharge_powerdown_entries)
                .saturating_add(self_refresh_entries)
                .saturating_add(exits);
            let bytes = read_bytes.saturating_add(write_bytes);
            let legacy_bytes = reads.saturating_add(writes).saturating_mul(64);
            let expected = 0.001
                + (events as f64 * 0.000_004)
                + (operations as f64 * 0.000_003)
                + (bytes as f64 * 0.000_000_5);
            let tolerance = 0.000_001;

            assert_ne!(
                bytes, legacy_bytes,
                "row {} fixture must distinguish canonical bytes from fixed 64-byte estimates",
                case.name
            );
            if (dram_record.dynamic_watts() - expected).abs() > tolerance {
                dram_mismatches.push(format!(
                    "row {} memory.dram dynamic watts {} != canonical {expected} from events={events}, operations={operations}, bytes={bytes}",
                    case.name,
                    dram_record.dynamic_watts()
                ));
            }
        }
    }

    assert!(
        dram_mismatches.is_empty(),
        "canonical DRAM power mismatches:\n{}",
        dram_mismatches.join("\n")
    );
}

fn assert_power_component_dynamic_watts_positive(power: &str, component: &str) {
    let marker = format!("<component id=\"{component}\"");
    let component_start = power
        .find(&marker)
        .unwrap_or_else(|| panic!("missing power component {component}:\n{power}"));
    let component_body = &power[component_start..];
    let component_end = component_body
        .find("</component>")
        .unwrap_or_else(|| panic!("unterminated power component {component}:\n{power}"));
    let component_body = &component_body[..component_end];
    let dynamic_marker = "dynamic_watts=\"";
    let dynamic_start = component_body
        .find(dynamic_marker)
        .map(|start| start + dynamic_marker.len())
        .unwrap_or_else(|| panic!("missing dynamic watts for {component}:\n{power}"));
    let dynamic_value = &component_body[dynamic_start..];
    let dynamic_end = dynamic_value
        .find('"')
        .unwrap_or_else(|| panic!("unterminated dynamic watts for {component}:\n{power}"));
    let dynamic_watts = dynamic_value[..dynamic_end].parse::<f64>().unwrap();
    assert!(
        dynamic_watts > 0.0,
        "expected positive dynamic watts for {component}: {component_body}"
    );
}

#[test]
fn rem6_run_power_analysis_includes_dram_activity() {
    let program = riscv64_load_store_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("power-output-dram", &elf);
    let power_path = temp_output("power-output-dram");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--execute",
            "--dram-memory",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let power = fs::read_to_string(&power_path).unwrap();
    assert!(power.contains("<component id=\"cpu0.core\""));
    assert!(power.contains("<component id=\"memory.dram\""));
    assert!(power.contains("total_watts="));
}

#[test]
fn rem6_run_power_analysis_includes_cache_activity() {
    let program = riscv64_load_store_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("power-output-cache", &elf);
    let power_path = temp_output("power-output-cache");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--execute",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let power = fs::read_to_string(&power_path).unwrap();
    assert!(power.contains("<component id=\"cpu0.core\""));
    assert!(power.contains("<component id=\"cpu.instruction_cache\""));
    assert!(power.contains("<component id=\"cpu.data_cache\""));
    assert!(power.contains("total_watts="));
}

#[test]
fn rem6_run_power_analysis_includes_shared_cache_activity() {
    let program = riscv64_load_store_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("power-output-shared-cache", &elf);
    let power_path = temp_output("power-output-shared-cache");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--execute",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--instruction-cache-l3-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--data-cache-l3-protocol",
            "msi",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let power = fs::read_to_string(&power_path).unwrap();
    assert_power_component_dynamic_watts_positive(&power, "memory.cache.l2");
    assert_power_component_dynamic_watts_positive(&power, "memory.cache.l3");
}

#[test]
fn rem6_run_power_analysis_includes_fabric_activity() {
    let program = riscv64_load_store_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("power-output-fabric", &elf);
    let power_path = temp_output("power-output-fabric");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
            "--execute",
            "--memory-system",
            "cache-fabric-dram",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let power = fs::read_to_string(&power_path).unwrap();
    assert_power_component_dynamic_watts_positive(&power, "memory.fabric");
}

#[test]
fn rem6_run_power_analysis_includes_transport_activity() {
    let program = riscv64_load_store_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("power-output-transport", &elf);
    let power_path = temp_output("power-output-transport");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--execute",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--power-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let power = fs::read_to_string(&power_path).unwrap();
    assert!(power.contains("<component id=\"cpu0.core\""));
    assert!(power.contains("<component id=\"memory.transport\""));
    assert!(power.contains("total_watts="));
}
