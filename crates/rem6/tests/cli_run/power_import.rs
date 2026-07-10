use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn unique_power_import_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rem6-power-import-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

fn run_power_import_fixture(prefix: &str, file_name: &str, format: &str, contents: &str) -> Output {
    let temp_dir = unique_power_import_temp_dir(prefix);
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join(file_name);
    std::fs::write(&input, contents).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "power-import",
            "--format",
            format,
            "--input",
            input.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();
    output
}

fn assert_power_import_failure(output: Output, stderr: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(String::from_utf8(output.stderr).unwrap(), stderr);
}

#[test]
fn rem6_power_import_summarizes_mcpat_xml() {
    let temp_dir = unique_power_import_temp_dir("mcpat");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join("mcpat.xml");
    std::fs::write(
        &input,
        concat!(
            "<mcpat_power tick=\"42\">\n",
            "  <component id=\"system.cpu0\" name=\"system.cpu0\" state=\"On\">\n",
            "    <power dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "    <thermal temperature_c=\"41.250000\"/>\n",
            "    <residency state=\"On\" ticks=\"40\" ratio=\"0.952381\"/>\n",
            "    <residency state=\"ClockGated\" ticks=\"2\" ratio=\"0.047619\"/>\n",
            "  </component>\n",
            "  <totals dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "</mcpat_power>\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "power-import",
            "--format",
            "mcpat-xml",
            "--input",
            input.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json.pointer("/schema").and_then(Value::as_str),
        Some("rem6.power-import.v1")
    );
    assert_eq!(
        json.pointer("/format").and_then(Value::as_str),
        Some("mcpat-xml")
    );
    assert_eq!(json.pointer("/tick").and_then(Value::as_u64), Some(42));
    assert_eq!(
        json.pointer("/records/0/target").and_then(Value::as_str),
        Some("system.cpu0")
    );
    assert_eq!(
        json.pointer("/records/0/residency/on_ticks")
            .and_then(Value::as_u64),
        Some(40)
    );
    assert_eq!(
        json.pointer("/totals/dynamic_watts")
            .and_then(Value::as_f64),
        Some(3.5)
    );
}

#[test]
fn rem6_power_import_accepts_standard_mcpat_xml_syntax() {
    let temp_dir = unique_power_import_temp_dir("mcpat-standard-xml");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join("mcpat.xml");
    std::fs::write(
        &input,
        concat!(
            "<mcpat_power tick='42'>\n",
            "  <component id='system.cpu&#38;cluster' name='system.cpu&#x26;cluster' state='On'>\n",
            "    <power dynamic_watts='3.500000' leakage_watts='1.250000' total_watts='4.750000'/>\n",
            "    <thermal temperature_c='41.250000'/>\n",
            "    <residency state='On' ticks='42' ratio='1.000000'/>\n",
            "  </component>\n",
            "  <totals dynamic_watts='3.500000' leakage_watts='1.250000' total_watts='4.750000'/>\n",
            "</mcpat_power>\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "power-import",
            "--format",
            "mcpat-xml",
            "--input",
            input.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json.pointer("/records/0/target").and_then(Value::as_str),
        Some("system.cpu&cluster")
    );
}

#[test]
fn rem6_power_import_rejects_malformed_mcpat_xml() {
    let output = run_power_import_fixture(
        "mcpat-malformed-xml",
        "mcpat.xml",
        "mcpat-xml",
        concat!(
            "<mcpat_power tick=\"42\">\n",
            "  <component id=\"system.cpu\" name=\"system.cpu\" state=\"On\">\n",
            "    <power dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "    <thermal temperature_c=\"41.250000\"/>\n",
            "    <residency state=\"On\" ticks=\"42\" ratio=\"1.000000\"/>\n",
            "  </component>\n",
            "  <totals dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
        ),
    );

    assert_power_import_failure(
        output,
        "failed to process power analysis artifact: invalid McPat power analysis artifact: invalid XML syntax\n",
    );
}

#[test]
fn rem6_power_import_rejects_duplicate_mcpat_component_power() {
    let output = run_power_import_fixture(
        "mcpat-duplicate-power",
        "mcpat.xml",
        "mcpat-xml",
        concat!(
            "<mcpat_power tick=\"42\">\n",
            "  <component id=\"system.cpu\" name=\"system.cpu\" state=\"On\">\n",
            "    <power dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "    <power dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "    <thermal temperature_c=\"41.250000\"/>\n",
            "    <residency state=\"On\" ticks=\"42\" ratio=\"1.000000\"/>\n",
            "  </component>\n",
            "  <totals dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "</mcpat_power>\n",
        ),
    );

    assert_power_import_failure(
        output,
        "failed to process power analysis artifact: invalid McPat power analysis artifact: component system.cpu has duplicate power tag\n",
    );
}

#[test]
fn rem6_power_import_rejects_dsent_characters_after_closing_quote() {
    let output = run_power_import_fixture(
        "dsent-post-quote",
        "dsent.csv",
        "dsent-csv",
        concat!(
            "record_type,tick,target,state,temperature_c,dynamic_watts,static_watts,total_watts,residency_state,residency_ticks,residency_ratio\n",
            "component,64,\"gpu.fabric\"x,On,44.500000,2.250000,0.500000,2.750000,On,64,1.000000\n",
            "total,64,__total__,All,,2.250000,0.500000,2.750000,,64,1.000000\n",
        ),
    );

    assert_power_import_failure(
        output,
        "failed to process power analysis artifact: invalid Dsent power analysis artifact: CSV field contains characters after a closing quote\n",
    );
}

#[test]
fn rem6_power_import_rejects_mcpat_residency_tick_overflow() {
    let xml = format!(
        concat!(
            "<mcpat_power tick=\"42\">\n",
            "  <component id=\"system.cpu\" name=\"system.cpu\" state=\"On\">\n",
            "    <power dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "    <thermal temperature_c=\"41.250000\"/>\n",
            "    <residency state=\"On\" ticks=\"{}\" ratio=\"1.000000\"/>\n",
            "    <residency state=\"ClockGated\" ticks=\"1\" ratio=\"0.000000\"/>\n",
            "  </component>\n",
            "  <totals dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "</mcpat_power>\n",
        ),
        u64::MAX,
    );
    let output =
        run_power_import_fixture("mcpat-residency-overflow", "mcpat.xml", "mcpat-xml", &xml);

    assert_power_import_failure(
        output,
        "failed to process power analysis artifact: invalid McPat power analysis artifact: component residency ticks overflow\n",
    );
}

#[test]
fn rem6_power_import_rejects_dsent_residency_tick_overflow() {
    let csv = format!(
        concat!(
            "record_type,tick,target,state,temperature_c,dynamic_watts,static_watts,total_watts,residency_state,residency_ticks,residency_ratio\n",
            "component,42,gpu.fabric,On,44.500000,2.250000,0.500000,2.750000,On,{},1.000000\n",
            "component,42,gpu.fabric,On,44.500000,2.250000,0.500000,2.750000,ClockGated,1,0.000000\n",
            "total,42,__total__,All,,2.250000,0.500000,2.750000,,42,1.000000\n",
        ),
        u64::MAX,
    );
    let output =
        run_power_import_fixture("dsent-residency-overflow", "dsent.csv", "dsent-csv", &csv);

    assert_power_import_failure(
        output,
        "failed to process power analysis artifact: invalid Dsent power analysis artifact: component gpu.fabric residency ticks overflow\n",
    );
}

#[test]
fn rem6_power_import_summarizes_dsent_csv() {
    let temp_dir = unique_power_import_temp_dir("dsent");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join("dsent.csv");
    std::fs::write(
        &input,
        concat!(
            "record_type,tick,target,state,temperature_c,dynamic_watts,static_watts,total_watts,residency_state,residency_ticks,residency_ratio\n",
            "component,64,gpu.fabric,On,44.500000,2.250000,0.500000,2.750000,On,64,1.000000\n",
            "total,64,__total__,All,,2.250000,0.500000,2.750000,,64,1.000000\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "power-import",
            "--format",
            "dsent-csv",
            "--input",
            input.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json.pointer("/schema").and_then(Value::as_str),
        Some("rem6.power-import.v1")
    );
    assert_eq!(
        json.pointer("/format").and_then(Value::as_str),
        Some("dsent-csv")
    );
    assert_eq!(json.pointer("/tick").and_then(Value::as_u64), Some(64));
    assert_eq!(
        json.pointer("/records/0/target").and_then(Value::as_str),
        Some("gpu.fabric")
    );
    assert_eq!(
        json.pointer("/records/0/dynamic_watts")
            .and_then(Value::as_f64),
        Some(2.25)
    );
    assert_eq!(
        json.pointer("/totals/static_watts").and_then(Value::as_f64),
        Some(0.5)
    );
}

#[test]
fn rem6_power_import_ingests_dsent_tuple_report() {
    let temp_dir = unique_power_import_temp_dir("dsent-report");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join("dsent-report.txt");
    std::fs::write(
        &input,
        concat!(
            "router0 Power:  (('Total:', 0), ('    Dynamic power: ', 0.500000), ('    Leakage power: ', 0.030000), ('    Buffer:           ', 1.250000))\n",
            "link0.nls0 Power:  (('Link:', 0), ('    Dynamic power: ', 0.125000), ('    Leakage power: ', 0.015000))\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "power-import",
            "--format",
            "dsent-report",
            "--input",
            input.to_str().unwrap(),
            "--tick",
            "64",
        ])
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json.pointer("/format").and_then(Value::as_str),
        Some("dsent-report")
    );
    assert_eq!(json.pointer("/tick").and_then(Value::as_u64), Some(64));
    assert_eq!(
        json.pointer("/record_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        json.pointer("/records/0/target").and_then(Value::as_str),
        Some("link0.nls0")
    );
    assert_eq!(
        json.pointer("/records/1/static_watts")
            .and_then(Value::as_f64),
        Some(0.03)
    );
    assert_eq!(
        json.pointer("/totals/total_watts").and_then(Value::as_f64),
        Some(0.67)
    );
}

#[test]
fn rem6_power_import_writes_summary_output_artifact() {
    let temp_dir = unique_power_import_temp_dir("output");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join("mcpat.xml");
    let artifact = temp_dir.join("summary/power-import.json");
    std::fs::write(
        &input,
        concat!(
            "<mcpat_power tick=\"7\">\n",
            "  <component id=\"memory.dram\" name=\"memory.dram\" state=\"On\">\n",
            "    <power dynamic_watts=\"1.500000\" leakage_watts=\"0.250000\" total_watts=\"1.750000\"/>\n",
            "    <thermal temperature_c=\"39.000000\"/>\n",
            "    <residency state=\"On\" ticks=\"7\" ratio=\"1.000000\"/>\n",
            "  </component>\n",
            "  <totals dynamic_watts=\"1.500000\" leakage_watts=\"0.250000\" total_watts=\"1.750000\"/>\n",
            "</mcpat_power>\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "power-import",
            "--format",
            "mcpat-xml",
            "--input",
            input.to_str().unwrap(),
            "--output",
            artifact.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        format!(
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\"}}\n",
            artifact.display()
        )
    );
    let json: Value = serde_json::from_str(&std::fs::read_to_string(&artifact).unwrap()).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert_eq!(
        json.pointer("/records/0/target").and_then(Value::as_str),
        Some("memory.dram")
    );
    assert_eq!(
        json.pointer("/totals/total_watts").and_then(Value::as_f64),
        Some(1.75)
    );
}

#[test]
fn rem6_power_import_ingests_mcpat_text_report() {
    let temp_dir = unique_power_import_temp_dir("mcpat-report");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join("mcpat-report.txt");
    std::fs::write(
        &input,
        concat!(
            "Processor:\n",
            "  Area = 12.000 mm^2\n",
            "  Peak Dynamic Power = 9.000 W\n",
            "  Subthreshold Leakage Power = 0.145 W\n",
            "  Gate Leakage Power = 0.030 W\n",
            "  Runtime Dynamic Power = 1.000 W\n",
            "  Runtime Dynamic Energy = 0.001 J\n",
            "  Total Runtime Energy = 0.001175 J\n",
            "\n",
            "    core0:\n",
            "      Area = 4.000 mm^2\n",
            "      Peak Dynamic Power = 5.000 W\n",
            "      Subthreshold Leakage Power = 0.125 W\n",
            "      Gate Leakage Power = 0.025 W\n",
            "      Runtime Dynamic Power = 0.750 W\n",
            "      Runtime Dynamic Energy = 0.00075 J\n",
            "      Total Runtime Energy = 0.00090 J\n",
            "\n",
            "    noc0:\n",
            "      Area = 1.000 mm^2\n",
            "      Peak Dynamic Power = 1.500 W\n",
            "      Subthreshold Leakage Power = 0.020 W\n",
            "      Gate Leakage Power = 0.005 W\n",
            "      Runtime Dynamic Power = 0.250 W\n",
            "      Runtime Dynamic Energy = 0.00025 J\n",
            "      Total Runtime Energy = 0.000275 J\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "power-import",
            "--format",
            "mcpat-report",
            "--input",
            input.to_str().unwrap(),
            "--tick",
            "128",
        ])
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        json.pointer("/format").and_then(Value::as_str),
        Some("mcpat-report")
    );
    assert_eq!(json.pointer("/tick").and_then(Value::as_u64), Some(128));
    assert_eq!(
        json.pointer("/record_count").and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        json.pointer("/records/0/target").and_then(Value::as_str),
        Some("Processor.core0")
    );
    assert_eq!(
        json.pointer("/records/0/static_watts")
            .and_then(Value::as_f64),
        Some(0.15)
    );
    assert_eq!(
        json.pointer("/totals/dynamic_watts")
            .and_then(Value::as_f64),
        Some(1.0)
    );
}

#[test]
fn rem6_power_import_rejects_tick_for_embedded_tick_formats() {
    let temp_dir = unique_power_import_temp_dir("tick-reject");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let input = temp_dir.join("mcpat.xml");
    std::fs::write(
        &input,
        concat!(
            "<mcpat_power tick=\"42\">\n",
            "  <component id=\"system.cpu0\" name=\"system.cpu0\" state=\"On\">\n",
            "    <power dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "    <thermal temperature_c=\"41.250000\"/>\n",
            "    <residency state=\"On\" ticks=\"42\" ratio=\"1.000000\"/>\n",
            "  </component>\n",
            "  <totals dynamic_watts=\"3.500000\" leakage_watts=\"1.250000\" total_watts=\"4.750000\"/>\n",
            "</mcpat_power>\n",
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "power-import",
            "--format",
            "mcpat-xml",
            "--input",
            input.to_str().unwrap(),
            "--tick",
            "9",
        ])
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("--tick only applies to report import formats"));
}
