use std::{fs, process::Command};

use super::*;

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
