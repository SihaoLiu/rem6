use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use rem6::Rem6GpuRunConfig;
use serde_json::Value;

use crate::support::{assert_stat, assert_transport_stats, stat_path_segment};

fn unique_gpu_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "rem6-gpu-{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
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

fn assert_gpu_fabric_lane(
    lanes: &[Value],
    link: &str,
    virtual_network: u64,
    byte_count: u64,
    flit_count: u64,
) {
    let lane = lanes
        .iter()
        .find(|lane| {
            lane.get("link").and_then(Value::as_str) == Some(link)
                && lane.get("virtual_network").and_then(Value::as_u64) == Some(virtual_network)
        })
        .expect("fabric lane activity");
    assert_eq!(lane.get("transfer_count").and_then(Value::as_u64), Some(1));
    assert_eq!(
        lane.get("byte_count").and_then(Value::as_u64),
        Some(byte_count)
    );
    assert_eq!(
        lane.get("flit_count").and_then(Value::as_u64),
        Some(flit_count)
    );
    assert!(lane
        .get("credit_delay_ticks")
        .and_then(Value::as_u64)
        .is_some());
    assert!(lane
        .get("max_credit_delay_ticks")
        .and_then(Value::as_u64)
        .is_some());
    assert!(lane.get("occupied_ticks").and_then(Value::as_u64).is_some());
    assert!(lane.get("first_tick").and_then(Value::as_u64).is_some());
    assert!(lane.get("last_tick").and_then(Value::as_u64).is_some());
}

fn assert_gpu_fabric_virtual_network_stats(
    stdout: &str,
    lanes: &[Value],
    link: &str,
    virtual_network: u64,
) {
    let lane = lanes
        .iter()
        .find(|lane| {
            lane.get("link").and_then(Value::as_str) == Some(link)
                && lane.get("virtual_network").and_then(Value::as_u64) == Some(virtual_network)
        })
        .expect("fabric virtual network activity source lane");
    let prefix = format!("sim.gpu_run.fabric.vn{virtual_network}");
    assert_stat(
        stdout,
        &format!("{prefix}.active_lanes"),
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.transfers"),
        "Count",
        lane.get("transfer_count")
            .and_then(Value::as_u64)
            .expect("fabric virtual network transfers"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.bytes"),
        "Byte",
        lane.get("byte_count")
            .and_then(Value::as_u64)
            .expect("fabric virtual network bytes"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.flits"),
        "Count",
        lane.get("flit_count")
            .and_then(Value::as_u64)
            .expect("fabric virtual network flits"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        lane.get("occupied_ticks")
            .and_then(Value::as_u64)
            .expect("fabric virtual network occupied ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        lane.get("queue_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric virtual network queue delay ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        lane.get("max_queue_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric virtual network max queue delay ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        lane.get("credit_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric virtual network credit delay ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_credit_delay_ticks"),
        "Tick",
        lane.get("max_credit_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric virtual network max credit delay ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.contended_lanes"),
        "Count",
        0,
        "monotonic",
    );
}

fn assert_gpu_fabric_lane_stats(stdout: &str, lane: &Value) {
    let link = lane
        .get("link")
        .and_then(Value::as_str)
        .expect("fabric lane link");
    let virtual_network = lane
        .get("virtual_network")
        .and_then(Value::as_u64)
        .expect("fabric lane virtual network");
    let prefix = format!(
        "sim.gpu_run.fabric.link.{}.vn{virtual_network}",
        stat_path_segment(link)
    );

    assert_stat(
        stdout,
        &format!("{prefix}.transfers"),
        "Count",
        lane.get("transfer_count")
            .and_then(Value::as_u64)
            .expect("fabric lane transfers"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.bytes"),
        "Byte",
        lane.get("byte_count")
            .and_then(Value::as_u64)
            .expect("fabric lane bytes"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.flits"),
        "Count",
        lane.get("flit_count")
            .and_then(Value::as_u64)
            .expect("fabric lane flits"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        lane.get("occupied_ticks")
            .and_then(Value::as_u64)
            .expect("fabric lane occupied ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        lane.get("queue_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric lane queue delay ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        lane.get("max_queue_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric lane max queue delay ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        lane.get("credit_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric lane credit delay ticks"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_credit_delay_ticks"),
        "Tick",
        lane.get("max_credit_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric lane max credit delay ticks"),
        "monotonic",
    );
}

fn assert_gpu_fabric_link_stats(stdout: &str, fabric: &Value, link: &str) {
    let links = fabric
        .get("link_activities")
        .and_then(Value::as_array)
        .expect("fabric link activities");
    assert_eq!(links.len(), 1);
    let link_activity = links
        .iter()
        .find(|activity| activity.get("link").and_then(Value::as_str) == Some(link))
        .expect("fabric link activity");
    assert_eq!(
        link_activity
            .get("active_virtual_networks")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        link_activity
            .get("contended_virtual_networks")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        link_activity.get("transfer_count").and_then(Value::as_u64),
        fabric.get("transfers").and_then(Value::as_u64)
    );
    assert_eq!(
        link_activity.get("byte_count").and_then(Value::as_u64),
        fabric.get("bytes").and_then(Value::as_u64)
    );
    assert_eq!(
        link_activity.get("flit_count").and_then(Value::as_u64),
        fabric.get("flits").and_then(Value::as_u64)
    );
    for field in [
        "occupied_ticks",
        "queue_delay_ticks",
        "max_queue_delay_ticks",
        "credit_delay_ticks",
        "max_credit_delay_ticks",
    ] {
        assert_eq!(
            link_activity.get(field).and_then(Value::as_u64),
            fabric.get(field).and_then(Value::as_u64),
            "fabric link field {field}"
        );
    }
    assert!(link_activity
        .get("first_tick")
        .and_then(Value::as_u64)
        .is_some());
    assert!(link_activity
        .get("last_tick")
        .and_then(Value::as_u64)
        .is_some());

    let prefix = format!("sim.gpu_run.fabric.link.{}", stat_path_segment(link));
    for (suffix, unit, field) in [
        ("transfers", "Count", "transfer_count"),
        ("bytes", "Byte", "byte_count"),
        ("flits", "Count", "flit_count"),
        ("occupied_ticks", "Tick", "occupied_ticks"),
        ("queue_delay_ticks", "Tick", "queue_delay_ticks"),
        ("max_queue_delay_ticks", "Tick", "max_queue_delay_ticks"),
        ("credit_delay_ticks", "Tick", "credit_delay_ticks"),
        ("max_credit_delay_ticks", "Tick", "max_credit_delay_ticks"),
    ] {
        assert_stat(
            stdout,
            &format!("{prefix}.{suffix}"),
            unit,
            link_activity
                .get(field)
                .and_then(Value::as_u64)
                .expect("fabric link stat value"),
            "monotonic",
        );
    }
    assert_stat(
        stdout,
        &format!("{prefix}.active_virtual_networks"),
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.contended_virtual_networks"),
        "Count",
        0,
        "monotonic",
    );
}

fn assert_gpu_fabric_hop_stats(stdout: &str, hop: &Value) {
    let link = hop
        .get("link")
        .and_then(Value::as_str)
        .expect("fabric hop link");
    let virtual_network = hop
        .get("virtual_network")
        .and_then(Value::as_u64)
        .expect("fabric hop virtual network");
    let hop_index = hop
        .get("hop_index")
        .and_then(Value::as_u64)
        .expect("fabric hop index");
    let prefix = format!(
        "sim.gpu_run.fabric.link.{}.vn{virtual_network}.hop{hop_index}",
        stat_path_segment(link)
    );

    assert_stat(
        stdout,
        &format!("{prefix}.transfers"),
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.bytes"),
        "Byte",
        hop.get("bytes")
            .and_then(Value::as_u64)
            .expect("fabric hop bytes"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.flits"),
        "Count",
        hop.get("flits")
            .and_then(Value::as_u64)
            .expect("fabric hop flits"),
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        hop.get("occupied_ticks")
            .and_then(Value::as_u64)
            .expect("fabric hop occupied ticks"),
        "monotonic",
    );
    let queue_delay_ticks = hop
        .get("queue_delay_ticks")
        .and_then(Value::as_u64)
        .expect("fabric hop queue delay ticks");
    assert_stat(
        stdout,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        hop.get("credit_delay_ticks")
            .and_then(Value::as_u64)
            .expect("fabric hop credit delay ticks"),
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_routes_coalesced_global_memory_through_cache_and_dram() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "2",
            "--global-load",
            "0x1000:4:4:4",
            "--memory-start",
            "0x1000",
            "--memory-size",
            "64",
            "--data-cache-protocol",
            "msi",
            "--dram-memory",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"workgroups\":2"));
    assert!(stdout.contains("\"workgroup_completions\":2"));
    assert!(stdout.contains("\"coalesced_memory_accesses\":2"));
    assert!(stdout.contains("\"global_memory_requests\":2"));
    assert!(stdout.contains("\"data_cache_protocol\":\"msi\""));
    assert!(stdout.contains("\"data_cache_runs\":2"));
    assert!(stdout.contains("\"data_cache_msi_runs\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":1"));
    assert!(stdout.contains("\"data_cache_bank_accepted\":2"));
    assert!(stdout.contains("\"data_cache_bank_immediate_hits\":0"));
    assert!(stdout.contains("\"data_cache_bank_scheduled_misses\":2"));
    assert!(stdout.contains("\"data_cache_bank_coalesced_misses\":0"));
    assert!(stdout.contains("\"accesses\":1"));
    assert!(stdout.contains("\"reads\":1"));
    assert_transport_stats(&stdout, "sim.gpu_run.transport", 2, 4, 2);
    assert_stat(
        &stdout,
        "sim.gpu_run.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.coalesced_memory_accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.runs",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.dram_accesses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.bank.accepted",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.bank.scheduled_misses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.memory.dram.accesses",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_data_cache_prefetcher_issues_tagged_next_line_prefetch() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--compute-units",
            "1",
            "--global-load",
            "0x4000:4:4:4",
            "--memory-start",
            "0x4000",
            "--memory-size",
            "128",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let data_cache = artifact.get("data_cache").unwrap();
    assert_eq!(
        data_cache
            .get("data_cache_prefetch_identified")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        data_cache
            .get("data_cache_prefetch_issued")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        data_cache
            .get("data_cache_prefetch_queue_enqueued")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        data_cache
            .get("data_cache_prefetch_queue_issued")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        data_cache
            .get("data_cache_prefetch_queue_dropped")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        data_cache.get("data_cache_runs").and_then(Value::as_u64),
        Some(2)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.prefetch.identified",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.prefetch.issued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.prefetch.queue.enqueued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.prefetch.queue.issued",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_loads_data_cache_prefetcher_from_toml_config() {
    let temp_dir = unique_gpu_temp_dir("prefetcher-toml");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
compute_units = 1
memory_start = 16640
memory_size = 128
data_cache_protocol = "msi"
data_cache_prefetcher = "tagged-next-line"
max_tick = 80
stats_format = "json"
global_loads = ["0x4100:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let data_cache = artifact.get("data_cache").unwrap();
    assert_eq!(
        data_cache
            .get("data_cache_prefetch_issued")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.prefetch.issued",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_rejects_data_cache_prefetcher_without_data_cache_protocol() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--global-load",
            "0x4200:4:4:4",
            "--memory-start",
            "0x4200",
            "--memory-size",
            "128",
            "--data-cache-prefetcher",
            "tagged-next-line",
            "--max-tick",
            "80",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--data-cache-prefetcher requires --data-cache-protocol"));
}

#[test]
fn rem6_gpu_run_rejects_toml_data_cache_prefetcher_without_data_cache_protocol() {
    let temp_dir = unique_gpu_temp_dir("prefetcher-toml-missing-protocol");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 17152
memory_size = 128
data_cache_prefetcher = "tagged-next-line"
max_tick = 80
global_loads = ["0x4300:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--data-cache-prefetcher requires --data-cache-protocol"));
}

#[test]
fn rem6_gpu_run_writes_power_analysis_output() {
    let temp_dir = unique_gpu_temp_dir("power-output");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let power_path = temp_dir.join("gpu-power.csv");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "2",
            "--global-load",
            "0x1800:4:4:4",
            "--memory-start",
            "0x1800",
            "--memory-size",
            "64",
            "--data-cache-protocol",
            "msi",
            "--dram-memory",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--power-format",
            "dsent-csv",
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"power_analysis\":{\"format\":\"dsent-csv\""));
    assert!(stdout.contains(&format!("\"artifact\":\"{}\"", power_path.display())));

    let power = std::fs::read_to_string(&power_path).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(power.starts_with("record_type,tick,target,state,temperature_c"));
    assert!(power.contains("gpu.compute_unit0"));
    assert!(power.contains("gpu.compute_unit1"));
    assert!(power.contains("gpu.data_cache"));
    assert!(power.contains("memory.dram"));
}

#[test]
fn rem6_gpu_run_power_output_includes_fabric_activity() {
    let temp_dir = unique_gpu_temp_dir("fabric-power-output");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let power_path = temp_dir.join("gpu-fabric-power.xml");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--compute-units",
            "1",
            "--global-load",
            "0x2e00:4:4:4",
            "--memory-start",
            "0x2e00",
            "--memory-size",
            "64",
            "--memory-route-delay",
            "4",
            "--fabric-link",
            "gpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "32",
            "--fabric-request-virtual-network",
            "7",
            "--fabric-response-virtual-network",
            "8",
            "--fabric-credit-depth",
            "2",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
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
    let power = std::fs::read_to_string(&power_path).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert_power_component_dynamic_watts_positive(&power, "gpu.fabric");
}

#[test]
fn rem6_gpu_run_writes_nomali_adapter_output() {
    let temp_dir = unique_gpu_temp_dir("nomali-output");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let nomali_path = temp_dir.join("gpu-nomali.json");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "2",
            "--global-load",
            "0x3400:4:4:4",
            "--global-store",
            "0x3420:4:4:4",
            "--memory-start",
            "0x3400",
            "--memory-size",
            "128",
            "--data-cache-protocol",
            "msi",
            "--dram-memory",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--nomali-output",
            nomali_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"nomali_adapter\":{\"schema\":\"rem6.nomali.gpu-adapter.v1\""));
    assert!(stdout.contains(&format!("\"artifact\":\"{}\"", nomali_path.display())));

    let adapter: Value =
        serde_json::from_str(&std::fs::read_to_string(&nomali_path).unwrap()).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();

    let string_fields = [
        ("/schema", "rem6.nomali.gpu-adapter.v1"),
        ("/gpu/type", "T760"),
        ("/pio/command_writes/0/name", "gpu_command"),
        ("/pio/command_writes/0/offset", "0x030"),
        ("/pio/command_writes/0/value", "0x00000002"),
        ("/pio/command_writes/0/command", "hard_reset"),
        ("/pio/command_writes/0/effect", "reset_completed_interrupt"),
        ("/pio/command_writes/1/name", "gpu_command"),
        ("/pio/command_writes/1/offset", "0x030"),
        ("/pio/command_writes/1/value", "0xdeaddead"),
        ("/pio/command_writes/1/command", "unsupported"),
        ("/pio/command_writes/1/effect", "ignored"),
        ("/pio/command_writes/2/name", "gpu_command"),
        ("/pio/command_writes/2/offset", "0x030"),
        ("/pio/command_writes/2/value", "0x00000001"),
        ("/pio/command_writes/2/command", "soft_reset"),
        ("/pio/command_writes/2/effect", "reset_completed_interrupt"),
        ("/pio/command_writes/3/name", "gpu_command"),
        ("/pio/command_writes/3/offset", "0x030"),
        ("/pio/command_writes/3/value", "0x00000004"),
        ("/pio/command_writes/3/command", "perf_counter_sample"),
        (
            "/pio/command_writes/3/effect",
            "perf_counter_sample_completed_interrupt",
        ),
        ("/pio/command_writes/4/name", "gpu_command"),
        ("/pio/command_writes/4/offset", "0x030"),
        ("/pio/command_writes/4/value", "0x00000007"),
        ("/pio/command_writes/4/command", "clean_caches"),
        (
            "/pio/command_writes/4/effect",
            "clean_caches_completed_interrupt",
        ),
        ("/pio/command_writes/5/name", "gpu_command"),
        ("/pio/command_writes/5/offset", "0x030"),
        ("/pio/command_writes/5/value", "0x00000008"),
        ("/pio/command_writes/5/command", "clean_invalidate_caches"),
        (
            "/pio/command_writes/5/effect",
            "clean_invalidate_caches_completed_interrupt",
        ),
        ("/pio/command_writes/6/name", "gpu_command"),
        ("/pio/command_writes/6/offset", "0x030"),
        ("/pio/command_writes/6/value", "0x00000000"),
        ("/pio/command_writes/6/command", "nop"),
        ("/pio/command_writes/6/effect", "no_effect"),
        ("/pio/command_writes/7/name", "gpu_command"),
        ("/pio/command_writes/7/offset", "0x030"),
        ("/pio/command_writes/7/value", "0x00000003"),
        ("/pio/command_writes/7/command", "perf_counter_clear"),
        ("/pio/command_writes/7/effect", "no_effect"),
        ("/pio/command_writes/8/name", "gpu_command"),
        ("/pio/command_writes/8/offset", "0x030"),
        ("/pio/command_writes/8/value", "0x00000005"),
        ("/pio/command_writes/8/command", "cycle_count_start"),
        ("/pio/command_writes/8/effect", "no_effect"),
        ("/pio/command_writes/9/name", "gpu_command"),
        ("/pio/command_writes/9/offset", "0x030"),
        ("/pio/command_writes/9/value", "0x00000006"),
        ("/pio/command_writes/9/command", "cycle_count_stop"),
        ("/pio/command_writes/9/effect", "no_effect"),
        ("/pio/irq_writes/0/name", "gpu_irq_clear"),
        ("/pio/irq_writes/0/offset", "0x024"),
        ("/pio/irq_writes/0/value", "0x00000100"),
        ("/pio/irq_writes/0/effect", "clear_reset_completed"),
        ("/pio/irq_writes/1/name", "gpu_irq_clear"),
        ("/pio/irq_writes/1/offset", "0x024"),
        ("/pio/irq_writes/1/value", "0x00000600"),
        ("/pio/irq_writes/1/effect", "clear_power_changed"),
        ("/pio/irq_writes/2/name", "job_irq_clear"),
        ("/pio/irq_writes/2/offset", "0x1004"),
        ("/pio/irq_writes/2/value", "0x00000001"),
        ("/pio/irq_writes/2/effect", "clear_job_slot_0"),
        ("/pio/irq_writes/3/name", "mmu_irq_clear"),
        ("/pio/irq_writes/3/offset", "0x2004"),
        ("/pio/irq_writes/3/value", "0x00010000"),
        ("/pio/irq_writes/3/effect", "clear_mmu_bus_error_as0"),
        ("/pio/irq_writes/4/name", "gpu_irq_clear"),
        ("/pio/irq_writes/4/offset", "0x024"),
        ("/pio/irq_writes/4/value", "0x00030000"),
        ("/pio/irq_writes/4/effect", "clear_command_completed"),
        ("/pio/irq_writes/5/name", "gpu_irq_clear"),
        ("/pio/irq_writes/5/offset", "0x024"),
        ("/pio/irq_writes/5/value", "0x00020000"),
        ("/pio/irq_writes/5/effect", "clear_command_completed"),
        ("/pio/irq_snapshots/0/name", "after_soft_reset_masked"),
        ("/pio/irq_snapshots/0/rawstat", "0x00000100"),
        ("/pio/irq_snapshots/0/mask", "0x00000100"),
        ("/pio/irq_snapshots/0/status", "0x00000100"),
        ("/pio/irq_snapshots/1/name", "after_irq_clear"),
        ("/pio/irq_snapshots/1/rawstat", "0x00000000"),
        ("/pio/irq_snapshots/1/mask", "0x00000100"),
        ("/pio/irq_snapshots/1/status", "0x00000000"),
        ("/pio/irq_snapshots/2/name", "after_shader_power_on"),
        ("/pio/irq_snapshots/2/rawstat", "0x00000600"),
        ("/pio/irq_snapshots/2/mask", "0x00000700"),
        ("/pio/irq_snapshots/2/status", "0x00000600"),
        ("/pio/irq_snapshots/3/name", "after_power_irq_clear"),
        ("/pio/irq_snapshots/3/rawstat", "0x00000000"),
        ("/pio/irq_snapshots/3/mask", "0x00000700"),
        ("/pio/irq_snapshots/3/status", "0x00000000"),
        ("/pio/irq_snapshots/4/name", "after_shader_power_off"),
        ("/pio/irq_snapshots/4/rawstat", "0x00000600"),
        ("/pio/irq_snapshots/4/mask", "0x00000700"),
        ("/pio/irq_snapshots/4/status", "0x00000600"),
        ("/pio/irq_snapshots/5/name", "after_perf_sample_command"),
        ("/pio/irq_snapshots/5/rawstat", "0x00010600"),
        ("/pio/irq_snapshots/5/mask", "0x00030700"),
        ("/pio/irq_snapshots/5/status", "0x00010600"),
        ("/pio/irq_snapshots/6/name", "after_clean_caches_command"),
        ("/pio/irq_snapshots/6/rawstat", "0x00030600"),
        ("/pio/irq_snapshots/6/mask", "0x00030700"),
        ("/pio/irq_snapshots/6/status", "0x00030600"),
        ("/pio/irq_snapshots/7/name", "after_command_irq_clear"),
        ("/pio/irq_snapshots/7/rawstat", "0x00000600"),
        ("/pio/irq_snapshots/7/mask", "0x00030700"),
        ("/pio/irq_snapshots/7/status", "0x00000600"),
        (
            "/pio/irq_snapshots/8/name",
            "after_clean_invalidate_caches_command",
        ),
        ("/pio/irq_snapshots/8/rawstat", "0x00020600"),
        ("/pio/irq_snapshots/8/mask", "0x00030700"),
        ("/pio/irq_snapshots/8/status", "0x00020600"),
        (
            "/pio/irq_snapshots/9/name",
            "after_clean_invalidate_irq_clear",
        ),
        ("/pio/irq_snapshots/9/rawstat", "0x00000600"),
        ("/pio/irq_snapshots/9/mask", "0x00030700"),
        ("/pio/irq_snapshots/9/status", "0x00000600"),
        ("/pio/interrupt_block_snapshots/0/block", "job"),
        (
            "/pio/interrupt_block_snapshots/0/name",
            "after_job_slot0_masked",
        ),
        ("/pio/interrupt_block_snapshots/0/rawstat_offset", "0x1000"),
        ("/pio/interrupt_block_snapshots/0/mask_offset", "0x1008"),
        ("/pio/interrupt_block_snapshots/0/status_offset", "0x100c"),
        ("/pio/interrupt_block_snapshots/0/rawstat", "0x00000001"),
        ("/pio/interrupt_block_snapshots/0/mask", "0x00000001"),
        ("/pio/interrupt_block_snapshots/0/status", "0x00000001"),
        ("/pio/interrupt_block_snapshots/1/block", "job"),
        (
            "/pio/interrupt_block_snapshots/1/name",
            "after_job_slot0_clear",
        ),
        ("/pio/interrupt_block_snapshots/1/rawstat", "0x00000000"),
        ("/pio/interrupt_block_snapshots/1/mask", "0x00000001"),
        ("/pio/interrupt_block_snapshots/1/status", "0x00000000"),
        ("/pio/interrupt_block_snapshots/2/block", "mmu"),
        (
            "/pio/interrupt_block_snapshots/2/name",
            "after_mmu_bus_error_masked",
        ),
        ("/pio/interrupt_block_snapshots/2/rawstat_offset", "0x2000"),
        ("/pio/interrupt_block_snapshots/2/mask_offset", "0x2008"),
        ("/pio/interrupt_block_snapshots/2/status_offset", "0x200c"),
        ("/pio/interrupt_block_snapshots/2/rawstat", "0x00010001"),
        ("/pio/interrupt_block_snapshots/2/mask", "0x00010000"),
        ("/pio/interrupt_block_snapshots/2/status", "0x00010000"),
        ("/pio/interrupt_block_snapshots/3/block", "mmu"),
        (
            "/pio/interrupt_block_snapshots/3/name",
            "after_mmu_bus_error_clear",
        ),
        ("/pio/interrupt_block_snapshots/3/rawstat", "0x00000001"),
        ("/pio/interrupt_block_snapshots/3/mask", "0x00010000"),
        ("/pio/interrupt_block_snapshots/3/status", "0x00000000"),
        ("/pio/power_writes/0/name", "shader_pwron_lo"),
        ("/pio/power_writes/0/offset", "0x180"),
        ("/pio/power_writes/0/value", "0x0000000f"),
        ("/pio/power_writes/0/ready_register", "shader_ready_lo"),
        ("/pio/power_writes/0/ready_offset", "0x140"),
        ("/pio/power_writes/0/ready_value", "0x0000000f"),
        ("/pio/power_writes/0/effect", "power_changed_interrupt"),
        ("/pio/power_writes/1/name", "shader_pwroff_lo"),
        ("/pio/power_writes/1/offset", "0x1c0"),
        ("/pio/power_writes/1/value", "0x00000003"),
        ("/pio/power_writes/1/ready_register", "shader_ready_lo"),
        ("/pio/power_writes/1/ready_value", "0x0000000c"),
        ("/pio/power_writes/1/effect", "power_changed_interrupt"),
        ("/pio/power_writes/2/name", "tiler_pwron_lo"),
        ("/pio/power_writes/2/offset", "0x190"),
        ("/pio/power_writes/2/value", "0x00000001"),
        ("/pio/power_writes/2/ready_register", "tiler_ready_lo"),
        ("/pio/power_writes/2/ready_offset", "0x150"),
        ("/pio/power_writes/2/ready_value", "0x00000001"),
        ("/pio/power_writes/2/effect", "power_changed_interrupt"),
        ("/pio/power_writes/3/name", "tiler_pwroff_lo"),
        ("/pio/power_writes/3/offset", "0x1d0"),
        ("/pio/power_writes/3/value", "0x00000001"),
        ("/pio/power_writes/3/ready_register", "tiler_ready_lo"),
        ("/pio/power_writes/3/ready_offset", "0x150"),
        ("/pio/power_writes/3/ready_value", "0x00000000"),
        ("/pio/power_writes/3/effect", "power_changed_interrupt"),
        ("/pio/power_writes/4/name", "l2_pwron_lo"),
        ("/pio/power_writes/4/offset", "0x1a0"),
        ("/pio/power_writes/4/value", "0x00000001"),
        ("/pio/power_writes/4/ready_register", "l2_ready_lo"),
        ("/pio/power_writes/4/ready_offset", "0x160"),
        ("/pio/power_writes/4/ready_value", "0x00000001"),
        ("/pio/power_writes/4/effect", "power_changed_interrupt"),
        ("/pio/power_writes/5/name", "l2_pwroff_lo"),
        ("/pio/power_writes/5/offset", "0x1e0"),
        ("/pio/power_writes/5/value", "0x00000001"),
        ("/pio/power_writes/5/ready_register", "l2_ready_lo"),
        ("/pio/power_writes/5/ready_offset", "0x160"),
        ("/pio/power_writes/5/ready_value", "0x00000000"),
        ("/pio/power_writes/5/effect", "power_changed_interrupt"),
        ("/pio/irq/rawstat", "0x00000600"),
        ("/pio/irq/mask", "0x00000700"),
        ("/pio/irq/status", "0x00000600"),
        ("/pio/register_reads/0/name", "gpu_id"),
        ("/pio/register_reads/0/offset", "0x000"),
        ("/pio/register_reads/0/value", "0x07500000"),
        ("/pio/register_reads/3/name", "thread_features"),
        ("/pio/register_reads/3/value", "0x0a040400"),
        ("/pio/register_reads/5/name", "shader_present_hi"),
        ("/pio/register_reads/5/value", "0x00000000"),
        ("/pio/register_faults/0/operation", "read"),
        ("/pio/register_faults/0/offset", "0x003"),
        ("/pio/register_faults/0/reason", "misaligned_offset"),
        ("/pio/register_faults/0/effect", "fault_recorded"),
        ("/pio/register_faults/1/operation", "write"),
        ("/pio/register_faults/1/offset", "0x4000"),
        ("/pio/register_faults/1/value", "0x12345678"),
        ("/pio/register_faults/1/reason", "offset_out_of_range"),
        ("/pio/register_faults/1/effect", "fault_recorded"),
    ];
    for (pointer, expected) in string_fields {
        assert_eq!(
            adapter.pointer(pointer).and_then(Value::as_str),
            Some(expected)
        );
    }
    assert_eq!(
        adapter
            .pointer("/pio/register_faults/0/value")
            .map(Value::is_null),
        Some(true)
    );
    let array_lengths = [
        ("/pio/command_writes", 10),
        ("/pio/irq_writes", 6),
        ("/pio/power_writes", 6),
        ("/pio/irq_snapshots", 10),
        ("/pio/interrupt_block_snapshots", 4),
        ("/pio/register_reads", 6),
        ("/pio/register_faults", 2),
    ];
    for (pointer, expected) in array_lengths {
        assert_eq!(
            adapter
                .pointer(pointer)
                .and_then(Value::as_array)
                .map(|array| array.len()),
            Some(expected)
        );
    }
    let numeric_fields = [
        ("/gpu/api_version", 0),
        ("/gpu/register_window_bytes", 0x4000),
        ("/pio/reset_count", 3),
        ("/pio/register_fault_count", 2),
        ("/pio/interrupt_block_snapshots/0/nomali_int", 1),
        ("/pio/interrupt_block_snapshots/1/nomali_int", 1),
        ("/pio/interrupt_block_snapshots/2/nomali_int", 2),
        ("/pio/interrupt_block_snapshots/3/nomali_int", 2),
        ("/pio/checkpoint/word_count", 4096),
        ("/interface/interrupts/job/nomali_int", 1),
        ("/execution/workgroup_completions", 2),
        ("/execution/global_memory_reads", 2),
        ("/execution/global_memory_writes", 2),
    ];
    for (pointer, expected) in numeric_fields {
        assert_eq!(
            adapter.pointer(pointer).and_then(Value::as_u64),
            Some(expected)
        );
    }
    let bool_fields = [
        ("/pio/irq/asserted", true),
        ("/pio/irq_snapshots/0/asserted", true),
        ("/pio/irq_snapshots/1/asserted", false),
        ("/pio/irq_snapshots/2/asserted", true),
        ("/pio/irq_snapshots/3/asserted", false),
        ("/pio/irq_snapshots/4/asserted", true),
        ("/pio/irq_snapshots/5/asserted", true),
        ("/pio/irq_snapshots/6/asserted", true),
        ("/pio/irq_snapshots/7/asserted", true),
        ("/pio/irq_snapshots/8/asserted", true),
        ("/pio/irq_snapshots/9/asserted", true),
        ("/pio/interrupt_block_snapshots/0/asserted", true),
        ("/pio/interrupt_block_snapshots/1/asserted", false),
        ("/pio/interrupt_block_snapshots/2/asserted", true),
        ("/pio/interrupt_block_snapshots/3/asserted", false),
    ];
    for (pointer, expected) in bool_fields {
        assert_eq!(
            adapter.pointer(pointer).and_then(Value::as_bool),
            Some(expected)
        );
    }
}

#[test]
fn rem6_gpu_run_loads_power_analysis_output_from_toml_config() {
    let temp_dir = unique_gpu_temp_dir("power-output-toml");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    let power_path = temp_dir.join("artifacts/gpu-power.xml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 2
compute_units = 2
memory_start = 8192
memory_size = 64
max_tick = 80
stats_format = "json"
dram_memory = true
data_cache_protocol = "msi"
power_format = "mcpat-xml"
power_output = "artifacts/gpu-power.xml"
global_loads = ["0x2000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"power_analysis\":{\"format\":\"mcpat-xml\""));
    assert!(stdout.contains(&format!("\"artifact\":\"{}\"", power_path.display())));

    let power = std::fs::read_to_string(&power_path).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(power.contains("<component id=\"gpu.compute_unit0\""));
    assert!(power.contains("<component id=\"gpu.compute_unit1\""));
    assert!(power.contains("<component id=\"gpu.data_cache\""));
    assert!(power.contains("<component id=\"memory.dram\""));
}

#[test]
fn rem6_gpu_run_loads_nomali_output_from_toml_config() {
    let temp_dir = unique_gpu_temp_dir("nomali-output-toml");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    let nomali_path = temp_dir.join("artifacts/gpu-nomali.json");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 2
compute_units = 2
memory_start = 16384
memory_size = 128
max_tick = 80
stats_format = "json"
dram_memory = true
data_cache_protocol = "msi"
nomali_output = "artifacts/gpu-nomali.json"
global_loads = ["0x4000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"nomali_adapter\":{\"schema\":\"rem6.nomali.gpu-adapter.v1\""));
    assert!(stdout.contains(&format!("\"artifact\":\"{}\"", nomali_path.display())));

    let adapter = std::fs::read_to_string(&nomali_path).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(adapter.contains("\"schema\":\"rem6.nomali.gpu-adapter.v1\""));
    assert!(adapter.contains("\"register_window_bytes\":16384"));
    assert!(adapter.contains("\"workgroup_completions\":2"));
}

#[test]
fn rem6_gpu_run_reports_all_output_artifact_paths_when_power_analysis_is_requested() {
    let temp_dir = unique_gpu_temp_dir("power-output-envelope");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let artifact_path = temp_dir.join("gpu.json");
    let stats_path = temp_dir.join("gpu-stats.json");
    let power_path = temp_dir.join("gpu-power.csv");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "2",
            "--global-load",
            "0x2400:4:4:4",
            "--memory-start",
            "0x2400",
            "--memory-size",
            "64",
            "--data-cache-protocol",
            "msi",
            "--dram-memory",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--output",
            artifact_path.to_str().unwrap(),
            "--stats-output",
            stats_path.to_str().unwrap(),
            "--power-format",
            "dsent-csv",
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
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout,
        format!(
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\",\"stats_artifact\":\"{}\",\"power_artifact\":\"{}\"}}\n",
            artifact_path.display(),
            stats_path.display(),
            power_path.display(),
        )
    );
    let artifact = std::fs::read_to_string(&artifact_path).unwrap();
    let stats = std::fs::read_to_string(&stats_path).unwrap();
    let power = std::fs::read_to_string(&power_path).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(artifact.contains("\"power_analysis\":{\"format\":\"dsent-csv\""));
    assert_stat(
        &stats,
        "sim.gpu_run.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
    assert!(power.contains("gpu.compute_unit0"));
    assert!(power.contains("gpu.data_cache"));
}

#[test]
fn rem6_gpu_run_reports_all_output_artifact_paths_when_power_and_nomali_are_requested() {
    let temp_dir = unique_gpu_temp_dir("power-nomali-output-envelope");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let artifact_path = temp_dir.join("gpu.json");
    let stats_path = temp_dir.join("gpu-stats.json");
    let power_path = temp_dir.join("gpu-power.csv");
    let nomali_path = temp_dir.join("gpu-nomali.json");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "2",
            "--global-load",
            "0x3800:4:4:4",
            "--memory-start",
            "0x3800",
            "--memory-size",
            "64",
            "--data-cache-protocol",
            "msi",
            "--dram-memory",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--output",
            artifact_path.to_str().unwrap(),
            "--stats-output",
            stats_path.to_str().unwrap(),
            "--power-format",
            "dsent-csv",
            "--power-output",
            power_path.to_str().unwrap(),
            "--nomali-output",
            nomali_path.to_str().unwrap(),
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
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\",\"stats_artifact\":\"{}\",\"power_artifact\":\"{}\",\"nomali_artifact\":\"{}\"}}\n",
            artifact_path.display(),
            stats_path.display(),
            power_path.display(),
            nomali_path.display(),
        )
    );
    let artifact = std::fs::read_to_string(&artifact_path).unwrap();
    let stats = std::fs::read_to_string(&stats_path).unwrap();
    let power = std::fs::read_to_string(&power_path).unwrap();
    let nomali = std::fs::read_to_string(&nomali_path).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(artifact.contains("\"power_analysis\":{\"format\":\"dsent-csv\""));
    assert!(artifact.contains("\"nomali_adapter\":{\"schema\":\"rem6.nomali.gpu-adapter.v1\""));
    assert_stat(
        &stats,
        "sim.gpu_run.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
    assert!(power.contains("gpu.compute_unit0"));
    assert!(nomali.contains("\"schema\":\"rem6.nomali.gpu-adapter.v1\""));
    assert!(nomali.contains("\"workgroup_completions\":2"));
}

#[test]
fn rem6_gpu_run_rejects_overlapping_power_output_paths() {
    let output_path = unique_gpu_temp_dir("power-output-conflict-json").join("gpu.json");
    let stats_path = unique_gpu_temp_dir("power-output-conflict-stats").join("gpu-stats.json");

    let output_conflict = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--global-load",
            "0x2800:4:4:4",
            "--memory-start",
            "0x2800",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--output",
            output_path.to_str().unwrap(),
            "--power-output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output_conflict.status.success());
    assert!(output_conflict.stdout.is_empty());
    let stderr = String::from_utf8(output_conflict.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert!(!output_path.exists());

    let stats_conflict = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--global-load",
            "0x2c00:4:4:4",
            "--memory-start",
            "0x2c00",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--stats-output",
            stats_path.to_str().unwrap(),
            "--power-output",
            stats_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!stats_conflict.status.success());
    assert!(stats_conflict.stdout.is_empty());
    let stderr = String::from_utf8(stats_conflict.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert!(!stats_path.exists());
}

#[test]
fn rem6_gpu_run_rejects_overlapping_nomali_output_paths() {
    let output_path = unique_gpu_temp_dir("nomali-output-conflict-json").join("gpu.json");
    let power_path = unique_gpu_temp_dir("nomali-output-conflict-power").join("gpu-power.json");

    let output_conflict = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--global-load",
            "0x3c00:4:4:4",
            "--memory-start",
            "0x3c00",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--output",
            output_path.to_str().unwrap(),
            "--nomali-output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output_conflict.status.success());
    assert!(output_conflict.stdout.is_empty());
    let stderr = String::from_utf8(output_conflict.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert!(!output_path.exists());

    let power_conflict = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--global-load",
            "0x4400:4:4:4",
            "--memory-start",
            "0x4400",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--power-output",
            power_path.to_str().unwrap(),
            "--nomali-output",
            power_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!power_conflict.status.success());
    assert!(power_conflict.stdout.is_empty());
    let stderr = String::from_utf8(power_conflict.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert!(!power_path.exists());
}

#[test]
fn rem6_gpu_run_routes_recorded_store_to_direct_memory_and_dumps_result() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--compute-units",
            "1",
            "--global-store",
            "0x2000:4:4:4",
            "--memory-start",
            "0x2000",
            "--memory-size",
            "16",
            "--dump-memory",
            "0x2000:16",
            "--max-tick",
            "40",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(stdout.contains("\"data_cache_protocol\":null"));
    assert!(stdout.contains("\"global_memory_requests\":1"));
    assert!(stdout.contains("\"memory_responses\":1"));
    assert!(stdout.contains("\"data_cache_runs\":0"));
    assert!(stdout.contains("\"active_targets\":0"));
    assert!(stdout.contains(
        "\"memory\":[{\"address\":\"0x2000\",\"bytes\":16,\"hex\":\"a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5\"}]"
    ));
    assert_transport_stats(&stdout, "sim.gpu_run.transport", 1, 2, 2);
    assert_stat(
        &stdout,
        "sim.gpu_run.data_cache.runs",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.memory.dram.accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(&stdout, "sim.gpu_run.memory.dumps", "Count", 1, "constant");
}

#[test]
fn rem6_gpu_run_routes_global_memory_through_configured_fabric() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--compute-units",
            "1",
            "--global-load",
            "0x2e00:4:4:4",
            "--memory-start",
            "0x2e00",
            "--memory-size",
            "64",
            "--memory-route-delay",
            "4",
            "--fabric-link",
            "gpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "32",
            "--fabric-request-virtual-network",
            "7",
            "--fabric-response-virtual-network",
            "8",
            "--fabric-credit-depth",
            "2",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let fabric = artifact.get("fabric").unwrap();
    assert_eq!(fabric.get("link").and_then(Value::as_str), Some("gpu_mem"));
    assert_eq!(
        fabric
            .get("bandwidth_bytes_per_tick")
            .and_then(Value::as_u64),
        Some(32)
    );
    assert_eq!(
        fabric
            .get("request_virtual_network")
            .and_then(Value::as_u64),
        Some(7)
    );
    assert_eq!(
        fabric
            .get("response_virtual_network")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(fabric.get("credit_depth").and_then(Value::as_u64), Some(2));
    assert_eq!(fabric.get("active_lanes").and_then(Value::as_u64), Some(2));
    assert_eq!(
        fabric
            .get("active_virtual_networks")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(fabric.get("transfers").and_then(Value::as_u64), Some(2));
    assert_eq!(fabric.get("bytes").and_then(Value::as_u64), Some(32));
    assert_eq!(fabric.get("flits").and_then(Value::as_u64), Some(2));
    let credit_delay_ticks = fabric
        .get("credit_delay_ticks")
        .and_then(Value::as_u64)
        .expect("fabric credit delay ticks");
    let max_credit_delay_ticks = fabric
        .get("max_credit_delay_ticks")
        .and_then(Value::as_u64)
        .expect("fabric max credit delay ticks");
    let lanes = fabric
        .get("lane_activities")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(lanes.len(), 2);
    assert_gpu_fabric_lane(lanes, "gpu_mem", 7, 16, 1);
    assert_gpu_fabric_lane(lanes, "gpu_mem", 8, 16, 1);
    assert_gpu_fabric_virtual_network_stats(&stdout, lanes, "gpu_mem", 7);
    assert_gpu_fabric_virtual_network_stats(&stdout, lanes, "gpu_mem", 8);
    assert_gpu_fabric_link_stats(&stdout, fabric, "gpu_mem");
    for lane in lanes {
        assert_gpu_fabric_lane_stats(&stdout, lane);
    }
    let hops = fabric
        .get("hop_activities")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(hops.len(), 2);
    assert!(hops.iter().any(|hop| {
        hop.get("link").and_then(Value::as_str) == Some("gpu_mem")
            && hop.get("virtual_network").and_then(Value::as_u64) == Some(7)
            && hop.get("flits").and_then(Value::as_u64) == Some(1)
            && hop
                .get("credit_delay_ticks")
                .and_then(Value::as_u64)
                .is_some()
    }));
    assert!(hops.iter().any(|hop| {
        hop.get("link").and_then(Value::as_str) == Some("gpu_mem")
            && hop.get("virtual_network").and_then(Value::as_u64) == Some(8)
            && hop.get("flits").and_then(Value::as_u64) == Some(1)
            && hop
                .get("credit_delay_ticks")
                .and_then(Value::as_u64)
                .is_some()
    }));
    for hop in hops {
        assert_gpu_fabric_hop_stats(&stdout, hop);
    }
    assert_stat(
        &stdout,
        "sim.gpu_run.fabric.active_virtual_networks",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.fabric.transfers",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(&stdout, "sim.gpu_run.fabric.flits", "Count", 2, "monotonic");
    assert_stat(
        &stdout,
        "sim.gpu_run.fabric.credit_delay_ticks",
        "Tick",
        credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.fabric.max_credit_delay_ticks",
        "Tick",
        max_credit_delay_ticks,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_loads_configured_fabric_from_toml_config() {
    let temp_dir = unique_gpu_temp_dir("fabric-toml");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
compute_units = 1
memory_start = 12288
memory_size = 64
memory_route_delay = 4
fabric_link = "gpu_mem_toml"
fabric_bandwidth_bytes_per_tick = 64
fabric_request_virtual_network = 5
fabric_response_virtual_network = 6
fabric_credit_depth = 3
max_tick = 80
stats_format = "json"
global_loads = ["0x3000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let fabric = artifact.get("fabric").unwrap();
    assert_eq!(
        fabric.get("link").and_then(Value::as_str),
        Some("gpu_mem_toml")
    );
    assert_eq!(
        fabric
            .get("request_virtual_network")
            .and_then(Value::as_u64),
        Some(5)
    );
    assert_eq!(
        fabric
            .get("response_virtual_network")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(fabric.get("credit_depth").and_then(Value::as_u64), Some(3));
    assert_eq!(
        fabric
            .get("active_virtual_networks")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.fabric.active_virtual_networks",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_rejects_fabric_virtual_network_without_link() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--global-load",
            "0x3400:4:4:4",
            "--memory-start",
            "0x3400",
            "--memory-size",
            "64",
            "--fabric-request-virtual-network",
            "2",
            "--max-tick",
            "80",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing required flag --fabric-link"));
}

#[test]
fn rem6_gpu_run_reports_per_compute_unit_activity() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "5",
            "--compute-units",
            "2",
            "--wave-slots-per-compute-unit",
            "1",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3000:4:4:4",
            "--memory-start",
            "0x3000",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let simulation = artifact.get("simulation").unwrap();
    let compute_units = simulation
        .get("compute_unit_activity")
        .and_then(Value::as_array)
        .unwrap();
    assert!(stdout.contains("\"workgroup_completions\":5"));
    assert_eq!(
        compute_units[0]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        compute_units[0].get("busy_cycles").and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        compute_units[1]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[1].get("busy_cycles").and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        simulation
            .get("workgroup_queue_wait_count")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        simulation
            .get("workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        simulation
            .get("max_workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        compute_units[0]
            .get("workgroup_queue_wait_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0]
            .get("workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        compute_units[0]
            .get("max_workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        compute_units[1]
            .get("workgroup_queue_wait_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        compute_units[1]
            .get("workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        compute_units[1]
            .get("max_workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.workgroup_completions",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.busy_cycles",
        "Cycle",
        12,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.first_started_at",
        "Tick",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.last_completed_at",
        "Tick",
        13,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.workgroup_queue_wait_count",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.workgroup_queue_wait_ticks",
        "Tick",
        12,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.max_workgroup_queue_wait_ticks",
        "Tick",
        8,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.busy_cycles",
        "Cycle",
        8,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.first_started_at",
        "Tick",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.last_completed_at",
        "Tick",
        9,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_queue_wait_count",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_queue_wait_ticks",
        "Tick",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.max_workgroup_queue_wait_ticks",
        "Tick",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.workgroup_queue_wait_count",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.workgroup_queue_wait_ticks",
        "Tick",
        16,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.max_workgroup_queue_wait_ticks",
        "Tick",
        8,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_reports_per_compute_unit_memory_activity() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "3",
            "--compute-units",
            "2",
            "--wave-slots-per-compute-unit",
            "1",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3200:4:4:4",
            "--global-store",
            "0x3240:4:4:4",
            "--memory-start",
            "0x3200",
            "--memory-size",
            "128",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let simulation = artifact.get("simulation").unwrap();
    let compute_units = simulation
        .get("compute_unit_activity")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        simulation
            .get("global_memory_requests")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        compute_units[0]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0].get("busy_cycles").and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        compute_units[0]
            .get("coalesced_memory_accesses")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        compute_units[0]
            .get("global_memory_reads")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0]
            .get("global_memory_writes")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[1]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        compute_units[1].get("busy_cycles").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        compute_units[1]
            .get("coalesced_memory_accesses")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[1]
            .get("global_memory_reads")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        compute_units[1]
            .get("global_memory_writes")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.coalesced_memory_accesses",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.global_memory_reads",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.global_memory_writes",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.coalesced_memory_accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.global_memory_reads",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.global_memory_writes",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_merges_overlapping_wave_slots_for_compute_unit_busy_cycles() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "1",
            "--wave-slots-per-compute-unit",
            "2",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3400:4:4:4",
            "--memory-start",
            "0x3400",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let simulation = artifact.get("simulation").unwrap();
    let compute_units = simulation
        .get("compute_unit_activity")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        compute_units[0]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0].get("busy_cycles").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        compute_units[0]
            .get("coalesced_memory_accesses")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0]
            .get("global_memory_reads")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0]
            .get("global_memory_writes")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[0]
            .get("first_started_at")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        compute_units[0]
            .get("last_completed_at")
            .and_then(Value::as_u64),
        Some(5)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.busy_cycles",
        "Cycle",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_omits_activity_window_stats_for_inactive_compute_units() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--compute-units",
            "2",
            "--wave-slots-per-compute-unit",
            "1",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3800:4:4:4",
            "--memory-start",
            "0x3800",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let simulation = artifact.get("simulation").unwrap();
    let compute_units = simulation
        .get("compute_unit_activity")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        compute_units[1]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1].get("busy_cycles").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1]
            .get("coalesced_memory_accesses")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1]
            .get("global_memory_reads")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1]
            .get("global_memory_writes")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1].get("first_started_at").unwrap(),
        &Value::Null
    );
    assert_eq!(
        compute_units[1].get("last_completed_at").unwrap(),
        &Value::Null
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_completions",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.busy_cycles",
        "Cycle",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("sim.gpu_run.compute_unit.cu1.first_started_at"));
    assert!(!stdout.contains("sim.gpu_run.compute_unit.cu1.last_completed_at"));
}

#[test]
fn rem6_gpu_run_accepts_toml_config_for_top_level_execution() {
    let temp_dir = unique_gpu_temp_dir("config");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 2
compute_units = 2
memory_start = 4096
memory_size = 64
max_tick = 80
stats_format = "json"
dram_memory = true
data_cache_protocol = "msi"
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(stdout.contains("\"status\":\"completed\""));
    assert!(stdout.contains("\"workgroups\":2"));
    assert!(stdout.contains("\"compute_units\":2"));
    assert!(stdout.contains("\"global_memory_requests\":2"));
    assert!(stdout.contains("\"data_cache_protocol\":\"msi\""));
    assert!(stdout.contains("\"data_cache_msi_runs\":2"));
    assert!(stdout.contains("\"accesses\":1"));
    assert_transport_stats(&stdout, "sim.gpu_run.transport", 2, 4, 2);
    assert_stat(
        &stdout,
        "sim.gpu_run.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_writes_toml_relative_output_files() {
    let temp_dir = unique_gpu_temp_dir("relative-output");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    let artifact_path = temp_dir.join("artifacts/gpu.json");
    let stats_path = temp_dir.join("artifacts/gpu-stats.json");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 4096
memory_size = 64
max_tick = 80
stats_format = "json"
output = "artifacts/gpu.json"
stats_output = "artifacts/gpu-stats.json"
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
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
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\",\"stats_artifact\":\"{}\"}}\n",
            artifact_path.display(),
            stats_path.display()
        )
    );
    let artifact = std::fs::read_to_string(&artifact_path).unwrap();
    let stats = std::fs::read_to_string(&stats_path).unwrap();
    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(artifact.contains("\"schema\":\"rem6.cli.gpu-run.v1\""));
    assert!(artifact.contains("\"status\":\"completed\""));
    assert!(artifact.contains("\"workgroups\":1"));
    assert_stat(
        &stats,
        "sim.gpu_run.workgroup_completions",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_merges_toml_stores_with_cli_loads() {
    let temp_dir = unique_gpu_temp_dir("mixed-access");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 8192
memory_size = 64
max_tick = 80
stats_format = "json"
global_stores = ["0x2000:4:4:4"]
memory_dumps = ["0x2000:16"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .args(["--global-load", "0x2010:4:4:4"])
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"global_memory_requests\":2"));
    assert!(stdout.contains("\"memory_responses\":2"));
    assert!(stdout.contains(
        "\"memory\":[{\"address\":\"0x2000\",\"bytes\":16,\"hex\":\"a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5\"}]"
    ));
}

#[test]
fn rem6_gpu_run_rejects_toml_output_stats_output_path_conflict() {
    let temp_dir = unique_gpu_temp_dir("output-conflict");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 4096
memory_size = 64
max_tick = 80
output = "same.json"
stats_output = "same.json"
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--output and --stats-output must use different paths"));
}

#[test]
fn rem6_gpu_run_config_scan_skips_value_that_matches_config_flag() {
    let temp_dir = unique_gpu_temp_dir("config-scan");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 4096
memory_size = 64
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let config = Rem6GpuRunConfig::parse_args(vec![
        "gpu-run".to_string(),
        "--output".to_string(),
        "--config".to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
    ])
    .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert_eq!(config.output().unwrap(), std::path::Path::new("--config"));
    assert_eq!(config.workgroups(), 1);
}

#[test]
fn rem6_gpu_run_config_scan_treats_power_output_value_as_a_value() {
    let temp_dir = unique_gpu_temp_dir("power-output-config-scan");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 4096
memory_size = 64
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let config = Rem6GpuRunConfig::parse_args(vec![
        "gpu-run".to_string(),
        "--power-output".to_string(),
        "--config".to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
    ])
    .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert_eq!(
        config.power_output().unwrap(),
        std::path::Path::new("--config")
    );
    assert_eq!(config.workgroups(), 1);
}

#[test]
fn rem6_gpu_run_config_scan_treats_nomali_output_value_as_a_value() {
    let temp_dir = unique_gpu_temp_dir("nomali-output-config-scan");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 1
memory_start = 4096
memory_size = 64
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let config = Rem6GpuRunConfig::parse_args(vec![
        "gpu-run".to_string(),
        "--nomali-output".to_string(),
        "--config".to_string(),
        "--config".to_string(),
        config_path.display().to_string(),
    ])
    .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert_eq!(
        config.nomali_output().unwrap(),
        std::path::Path::new("--config")
    );
    assert_eq!(config.workgroups(), 1);
}

#[test]
fn rem6_gpu_run_rejects_zero_workgroups_from_toml_config() {
    let temp_dir = unique_gpu_temp_dir("bad-config");
    std::fs::create_dir_all(&temp_dir).unwrap();
    let config_path = temp_dir.join("gpu.toml");
    std::fs::write(
        &config_path,
        r#"
[gpu_run]
workgroups = 0
memory_start = 4096
memory_size = 64
max_tick = 80
global_loads = ["0x1000:4:4:4"]
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["gpu-run", "--config"])
        .arg(&config_path)
        .output()
        .unwrap();

    std::fs::remove_dir_all(&temp_dir).unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--workgroups must be positive, got 0"));
}
