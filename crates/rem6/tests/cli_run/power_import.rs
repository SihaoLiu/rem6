use std::path::PathBuf;
use std::process::Command;
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
