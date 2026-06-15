use std::fs;
use std::process::Command;

use crate::support::*;

#[test]
fn rem6_resource_acquire_loads_config_manifest_and_local_artifact() {
    let workspace = temp_workspace("resource-acquire-config");
    let artifact = workspace.join("kernel.bin");
    fs::write(&artifact, [0x13, 0x00, 0x00, 0x00]).unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
acquisition_tool = "local-catalog"
acquisition_revision = "rev1"
artifact = "kernel.bin"
artifact_digest = "sha256:kernel"
artifact_size = 4
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"schema\":\"rem6.cli.resource_acquire.v1\""));
    assert!(stdout.contains("\"workload_id\":\"resource-cli\""));
    assert!(stdout.contains("\"boot_entry\":\"0x8000\""));
    assert!(stdout.contains("\"required_resources\":1"));
    assert!(stdout.contains("\"acquired_resources\":1"));
    assert!(stdout.contains("\"resolved_resources\":1"));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"kind\":\"kernel\""));
    assert!(stdout.contains("\"digest\":\"sha256:kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"local-file\""));
    assert!(stdout.contains("\"acquisition_locator\":\"catalog://kernel\""));
    assert_stat(
        &stdout,
        "sim.resource_acquire.required_resources",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.resource_acquire.acquired_resources",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.resource_acquire.acquired_bytes",
        "Byte",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_resource_acquire_reports_executor_digest_mismatch() {
    let workspace = temp_workspace("resource-acquire-digest-mismatch");
    let artifact = workspace.join("kernel.bin");
    fs::write(&artifact, [0x13, 0x00, 0x00, 0x00]).unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.bin"
artifact_digest = "sha256:other"
artifact_size = 4
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr
        .contains("resource kernel artifact digest sha256:other does not match sha256:kernel"));
}
