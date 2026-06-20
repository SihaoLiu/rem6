use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use flate2::{write::DeflateEncoder, Compression};

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
fn rem6_resource_acquire_loads_config_manifest_from_host_file_locator() {
    let workspace = temp_workspace("resource-acquire-host-file-config");
    let host_dir = workspace.join("host");
    fs::create_dir(&host_dir).unwrap();
    fs::write(host_dir.join("kernel.bin"), [0x13, 0x00, 0x00, 0x00]).unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-host-file-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:host-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "host-file"
acquisition_locator = "host/kernel.bin"
artifact_digest = "sha256:host-kernel"
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
    assert!(stdout.contains("\"workload_id\":\"resource-host-file-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"host-file\""));
    assert!(stdout.contains("\"acquisition_locator\":\"host/kernel.bin\""));
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
fn rem6_resource_acquire_loads_config_manifest_from_generated_zero_fill() {
    let workspace = temp_workspace("resource-acquire-generated-config");
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-generated-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:generated-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "generated"
acquisition_locator = "zero-fill:kernel"
artifact_digest = "sha256:generated-kernel"
artifact_size = 4

[[resource_acquire.resources]]
id = "initrd"
kind = "initrd"
digest = "sha256:generated-initrd"
locator = "resources/initrd.img"
required = true
acquisition_kind = "generated"
acquisition_locator = "zero-fill:initrd"
artifact_digest = "sha256:generated-initrd"
artifact_size = 2
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
    assert!(stdout.contains("\"workload_id\":\"resource-generated-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"resource\":\"initrd\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"size_bytes\":2"));
    assert!(stdout.contains("\"acquisition_kind\":\"generated\""));
    assert!(stdout.contains("\"acquisition_locator\":\"zero-fill:kernel\""));
    assert!(stdout.contains("\"acquisition_locator\":\"zero-fill:initrd\""));
    assert_stat(
        &stdout,
        "sim.resource_acquire.acquired_resources",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.resource_acquire.acquired_bytes",
        "Byte",
        6,
        "monotonic",
    );
}

#[test]
fn rem6_resource_acquire_rejects_generated_resource_without_artifact_size() {
    let workspace = temp_workspace("resource-acquire-generated-missing-size");
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-generated-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:generated-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "generated"
acquisition_locator = "zero-fill"
artifact_digest = "sha256:generated-kernel"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing required flag resource_acquire.resources.artifact_size"));
}

#[test]
fn rem6_resource_acquire_rejects_generated_resource_with_artifact_path() {
    let workspace = temp_workspace("resource-acquire-generated-artifact-conflict");
    fs::write(workspace.join("kernel.bin"), [0x13, 0x00, 0x00, 0x00]).unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-generated-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:generated-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "generated"
acquisition_locator = "zero-fill:kernel"
artifact = "kernel.bin"
artifact_digest = "sha256:generated-kernel"
artifact_size = 4
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("generated resource kernel must not declare artifact"));
}

#[test]
fn rem6_resource_acquire_rejects_unknown_generated_locator() {
    let workspace = temp_workspace("resource-acquire-generated-unknown-locator");
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-generated-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:generated-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "generated"
acquisition_locator = "pattern:kernel"
artifact_digest = "sha256:generated-kernel"
artifact_size = 4
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("generated resource kernel supports only zero-fill"));
}

#[test]
fn rem6_resource_acquire_loads_config_manifest_from_tar_archive_locator() {
    let workspace = temp_workspace("resource-acquire-archive-tar-config");
    let archive_dir = workspace.join("archives");
    fs::create_dir(&archive_dir).unwrap();
    fs::write(
        archive_dir.join("kernel.tar"),
        tar_archive_with_entry("kernel.bin", &[0x13, 0x00, 0x00, 0x00]),
    )
    .unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-archive-tar-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:archive-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "archive-tar"
acquisition_locator = "archives/kernel.tar#kernel.bin"
artifact_digest = "sha256:archive-kernel"
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
    assert!(stdout.contains("\"workload_id\":\"resource-archive-tar-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"archive-tar\""));
    assert!(stdout.contains("\"acquisition_locator\":\"archives/kernel.tar#kernel.bin\""));
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
fn rem6_resource_acquire_loads_config_manifest_from_gzip_tar_archive_locator() {
    let workspace = temp_workspace("resource-acquire-archive-tar-gz-config");
    let archive_dir = workspace.join("archives");
    fs::create_dir(&archive_dir).unwrap();
    fs::write(
        archive_dir.join("kernel.tar.gz"),
        gzip_stored(tar_archive_with_entry(
            "kernel.bin",
            &[0x13, 0x00, 0x00, 0x00],
        )),
    )
    .unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-archive-tar-gz-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:archive-gzip-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "archive-tar"
acquisition_locator = "archives/kernel.tar.gz#kernel.bin"
artifact_digest = "sha256:archive-gzip-kernel"
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
    assert!(stdout.contains("\"workload_id\":\"resource-archive-tar-gz-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"archive-tar\""));
    assert!(stdout.contains("\"acquisition_locator\":\"archives/kernel.tar.gz#kernel.bin\""));
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
fn rem6_resource_acquire_loads_config_manifest_from_zip_archive_locator() {
    let workspace = temp_workspace("resource-acquire-archive-zip-config");
    let archive_dir = workspace.join("archives");
    fs::create_dir(&archive_dir).unwrap();
    fs::write(
        archive_dir.join("kernel.zip"),
        zip_archive_with_entry("payload/kernel.bin", &[0x13, 0x00, 0x00, 0x00]),
    )
    .unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-archive-zip-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:archive-zip-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "archive-zip"
acquisition_locator = "archives/kernel.zip#payload/kernel.bin"
artifact_digest = "sha256:archive-zip-kernel"
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
    assert!(stdout.contains("\"workload_id\":\"resource-archive-zip-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"archive-zip\""));
    assert!(stdout.contains("\"acquisition_locator\":\"archives/kernel.zip#payload/kernel.bin\""));
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
fn rem6_resource_acquire_loads_config_manifest_from_deflated_zip_archive_locator() {
    let workspace = temp_workspace("resource-acquire-archive-zip-deflated-config");
    let archive_dir = workspace.join("archives");
    fs::create_dir(&archive_dir).unwrap();
    fs::write(
        archive_dir.join("kernel.zip"),
        zip_deflated_archive_with_entry("payload/kernel.bin", &[0x13, 0x00, 0x00, 0x00]),
    )
    .unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-archive-zip-deflated-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:archive-zip-deflated-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "archive-zip"
acquisition_locator = "archives/kernel.zip#payload/kernel.bin"
artifact_digest = "sha256:archive-zip-deflated-kernel"
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
    assert!(stdout.contains("\"workload_id\":\"resource-archive-zip-deflated-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"archive-zip\""));
    assert!(stdout.contains("\"acquisition_locator\":\"archives/kernel.zip#payload/kernel.bin\""));
}

#[test]
fn rem6_resource_acquire_reports_missing_zip_archive_member() {
    let workspace = temp_workspace("resource-acquire-archive-zip-missing-member");
    let archive_dir = workspace.join("archives");
    fs::create_dir(&archive_dir).unwrap();
    fs::write(
        archive_dir.join("kernel.zip"),
        zip_archive_with_entry("payload/kernel.bin", &[0x13, 0x00, 0x00, 0x00]),
    )
    .unwrap();
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-archive-zip-missing-member-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:archive-zip-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "archive-zip"
acquisition_locator = "archives/kernel.zip#payload/missing.bin"
artifact_digest = "sha256:archive-zip-kernel"
artifact_size = 4
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("zip member payload/missing.bin was not found"));
}

#[test]
fn rem6_resource_acquire_loads_config_manifest_from_remote_uri_locator() {
    let workspace = temp_workspace("resource-acquire-remote-uri-config");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/kernel.bin", listener.local_addr().unwrap());
    let server = serve_http_resource_once(listener, "/kernel.bin", [0x13, 0x00, 0x00, 0x00]);
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        format!(
            r#"[resource_acquire]
workload_id = "resource-remote-uri-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "{url}"
artifact_digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
artifact_size = 4
"#,
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"workload_id\":\"resource-remote-uri-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"remote-uri\""));
    assert!(stdout.contains(&format!("\"acquisition_locator\":\"{url}\"")));
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
fn rem6_resource_acquire_rejects_remote_uri_without_content_sha256() {
    let workspace = temp_workspace("resource-acquire-remote-uri-without-content-sha256");
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-remote-uri-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:remote-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "http://127.0.0.1:1/kernel.bin"
artifact_digest = "sha256:remote-kernel"
artifact_size = 4
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("remote-uri resource kernel requires artifact_digest sha256:"));
    assert!(stderr.contains("64 lowercase hex"));
}

#[test]
fn rem6_resource_acquire_rejects_remote_uri_without_explicit_artifact_digest() {
    let workspace = temp_workspace("resource-acquire-remote-uri-missing-artifact-digest");
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-remote-uri-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:3333333333333333333333333333333333333333333333333333333333333333"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "http://127.0.0.1:1/kernel.bin"
artifact_size = 4
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("remote-uri resource kernel requires explicit artifact_digest"));
}

#[test]
fn rem6_resource_acquire_rejects_remote_uri_uppercase_artifact_digest() {
    let workspace = temp_workspace("resource-acquire-remote-uri-uppercase-digest");
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
workload_id = "resource-remote-uri-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:3333333333333333333333333333333333333333333333333333333333333333"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "http://127.0.0.1:1/kernel.bin"
artifact_digest = "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
artifact_size = 4
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("remote-uri resource kernel requires artifact_digest sha256:"));
    assert!(stderr.contains("lowercase"));
}

#[test]
fn rem6_resource_acquire_loads_chunked_remote_uri_resource() {
    let workspace = temp_workspace("resource-acquire-remote-uri-chunked");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/kernel.bin", listener.local_addr().unwrap());
    let server =
        serve_chunked_http_resource_once(listener, "/kernel.bin", [0x13, 0x00, 0x00, 0x00]);
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        format!(
            r#"[resource_acquire]
workload_id = "resource-remote-uri-chunked-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "{url}"
artifact_digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
artifact_size = 4
"#,
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"workload_id\":\"resource-remote-uri-chunked-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"remote-uri\""));
    assert!(stdout.contains(&format!("\"acquisition_locator\":\"{url}\"")));
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
fn rem6_resource_acquire_follows_remote_uri_redirect() {
    let workspace = temp_workspace("resource-acquire-remote-uri-redirect");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let url = format!("{base}/redirect.bin");
    let server = serve_redirect_http_resource_once(
        listener,
        "/redirect.bin",
        "/kernel.bin",
        "/kernel.bin",
        [0x13, 0x00, 0x00, 0x00],
    );
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        format!(
            r#"[resource_acquire]
workload_id = "resource-remote-uri-redirect-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "{url}"
artifact_digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
artifact_size = 4
"#,
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"workload_id\":\"resource-remote-uri-redirect-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"remote-uri\""));
    assert!(stdout.contains(&format!("\"acquisition_locator\":\"{url}\"")));
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
fn rem6_resource_acquire_follows_relative_remote_uri_redirect() {
    let workspace = temp_workspace("resource-acquire-remote-uri-relative-redirect");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let url = format!("{base}/images/redirect.bin");
    let server = serve_redirect_http_resource_once(
        listener,
        "/images/redirect.bin",
        "kernel.bin",
        "/images/kernel.bin",
        [0x13, 0x00, 0x00, 0x00],
    );
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        format!(
            r#"[resource_acquire]
workload_id = "resource-remote-uri-relative-redirect-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "{url}"
artifact_digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
artifact_size = 4
"#,
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"workload_id\":\"resource-remote-uri-relative-redirect-cli\""));
    assert!(stdout.contains("\"resource\":\"kernel\""));
    assert!(stdout.contains("\"size_bytes\":4"));
    assert!(stdout.contains("\"acquisition_kind\":\"remote-uri\""));
    assert!(stdout.contains(&format!("\"acquisition_locator\":\"{url}\"")));
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
fn rem6_resource_acquire_rejects_remote_uri_redirect_loop() {
    let workspace = temp_workspace("resource-acquire-remote-uri-redirect-loop");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/loop.bin", listener.local_addr().unwrap());
    let server = serve_redirect_loop_http_resource(listener, "/loop.bin", &url, 6);
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        format!(
            r#"[resource_acquire]
workload_id = "resource-remote-uri-redirect-loop-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "{url}"
artifact_digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
artifact_size = 4
"#,
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("HTTP resource exceeded 5 redirects"));
}

#[test]
fn rem6_resource_acquire_rejects_remote_uri_redirect_to_unsupported_scheme() {
    let workspace = temp_workspace("resource-acquire-remote-uri-redirect-unsupported");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/redirect.bin", listener.local_addr().unwrap());
    let server =
        serve_redirect_loop_http_resource(listener, "/redirect.bin", "file:/tmp/kernel.bin", 1);
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        format!(
            r#"[resource_acquire]
workload_id = "resource-remote-uri-redirect-unsupported-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "{url}"
artifact_digest = "sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765"
artifact_size = 4
"#,
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("HTTP redirect Location file:/tmp/kernel.bin")
            && stderr.contains("is not an http:// URL or absolute path")
    );
}

#[test]
fn rem6_resource_acquire_rejects_remote_uri_content_digest_mismatch() {
    let workspace = temp_workspace("resource-acquire-remote-uri-digest-mismatch");
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/kernel.bin", listener.local_addr().unwrap());
    let server = serve_http_resource_once(listener, "/kernel.bin", [0x13, 0x00, 0x00, 0x00]);
    let config = workspace.join("resource-acquire.toml");
    fs::write(
        &config,
        format!(
            r#"[resource_acquire]
workload_id = "resource-remote-uri-cli"
boot_entry = 32768
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:remote-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "remote-uri"
acquisition_locator = "{url}"
artifact_digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
artifact_size = 4
"#,
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    server.join().unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("remote-uri resource kernel content digest"));
    assert!(
        stderr.contains("sha256:eba09f2f48f209cfa2dfbf19fc678d755d05559671eceda0164f3e080cb49765")
    );
    assert!(
        stderr.contains("sha256:0000000000000000000000000000000000000000000000000000000000000000")
    );
}

#[test]
fn rem6_resource_acquire_loads_suite_manifests_and_local_artifacts() {
    let workspace = temp_workspace("resource-acquire-suite-config");
    let first_artifact = workspace.join("kernel-a.bin");
    let second_artifact = workspace.join("kernel-b.bin");
    fs::write(&first_artifact, [0x13, 0x00, 0x00, 0x00]).unwrap();
    fs::write(&second_artifact, [0x93, 0x00, 0x00, 0x00]).unwrap();
    let config = workspace.join("resource-acquire-suite.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
suite_id = "suite-cli"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "work-a"
boot_entry = 32768

[[resource_acquire.manifests.resources]]
id = "kernel-a"
kind = "kernel"
digest = "sha256:kernel-a"
locator = "resources/kernel-a.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-a"
artifact = "kernel-a.bin"
artifact_digest = "sha256:kernel-a"
artifact_size = 4

[[resource_acquire.manifests]]
workload_id = "work-b"
boot_entry = 36864

[[resource_acquire.manifests.resources]]
id = "kernel-b"
kind = "kernel"
digest = "sha256:kernel-b"
locator = "resources/kernel-b.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-b"
artifact = "kernel-b.bin"
artifact_digest = "sha256:kernel-b"
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
    assert!(stdout.contains("\"mode\":\"suite\""));
    assert!(stdout.contains("\"suite_id\":\"suite-cli\""));
    assert!(stdout.contains("\"suite_manifests\":2"));
    assert!(stdout.contains("\"suite_required_resources\":2"));
    assert!(stdout.contains("\"suite_acquired_resources\":2"));
    assert!(stdout.contains("\"workload_id\":\"work-a\""));
    assert!(stdout.contains("\"workload_id\":\"work-b\""));
    assert!(stdout.contains("\"resource\":\"kernel-a\""));
    assert!(stdout.contains("\"resource\":\"kernel-b\""));
    assert!(stdout.contains("\"acquisition_locator\":\"catalog://kernel-a\""));
    assert!(stdout.contains("\"acquisition_locator\":\"catalog://kernel-b\""));
    assert_stat(
        &stdout,
        "sim.resource_acquire.required_resources",
        "Count",
        2,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.resource_acquire.acquired_resources",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.resource_acquire.acquired_bytes",
        "Byte",
        8,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.resource_acquire.suite_manifests",
        "Count",
        2,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.resource_acquire.suite_required_resources",
        "Count",
        2,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.resource_acquire.suite_acquired_resources",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_resource_acquire_rejects_suite_id_without_manifests() {
    let workspace = temp_workspace("resource-acquire-suite-missing-manifests");
    let config = workspace.join("resource-acquire-suite.toml");
    fs::write(
        &config,
        r#"[resource_acquire]
suite_id = "suite-cli"
stats_format = "json"
"#,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["resource-acquire", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("missing required flag resource_acquire.manifests"));
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

fn tar_archive_with_entry(name: &str, data: &[u8]) -> Vec<u8> {
    let mut header = [0_u8; 512];
    header[..name.len()].copy_from_slice(name.as_bytes());
    write_tar_octal(&mut header[100..108], 0o644);
    write_tar_octal(&mut header[108..116], 0);
    write_tar_octal(&mut header[116..124], 0);
    write_tar_octal(&mut header[124..136], data.len() as u64);
    write_tar_octal(&mut header[136..148], 0);
    header[148..156].fill(b' ');
    header[156] = b'0';
    header[257..263].copy_from_slice(b"ustar\0");
    header[263..265].copy_from_slice(b"00");
    let checksum = header.iter().map(|byte| u64::from(*byte)).sum::<u64>();
    write_tar_checksum(&mut header[148..156], checksum);

    let mut archive = Vec::from(header);
    archive.extend_from_slice(data);
    let padding = (512 - (data.len() % 512)) % 512;
    archive.resize(archive.len() + padding, 0);
    archive.resize(archive.len() + 1024, 0);
    archive
}

fn write_tar_octal(field: &mut [u8], value: u64) {
    field.fill(0);
    let encoded = format!("{:0width$o}", value, width = field.len() - 1);
    field[..encoded.len()].copy_from_slice(encoded.as_bytes());
}

fn write_tar_checksum(field: &mut [u8], value: u64) {
    field.fill(b' ');
    let encoded = format!("{:06o}", value);
    field[..encoded.len()].copy_from_slice(encoded.as_bytes());
    field[6] = 0;
}

fn gzip_stored(data: Vec<u8>) -> Vec<u8> {
    assert!(data.len() <= u16::MAX as usize);
    let len = data.len() as u16;
    let mut gzip = Vec::new();
    gzip.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0, 0, 0, 0, 0, 0xff]);
    gzip.push(0x01);
    gzip.extend_from_slice(&len.to_le_bytes());
    gzip.extend_from_slice(&(!len).to_le_bytes());
    gzip.extend_from_slice(&data);
    gzip.extend_from_slice(&crc32(&data).to_le_bytes());
    gzip.extend_from_slice(&(data.len() as u32).to_le_bytes());
    gzip
}

fn zip_archive_with_entry(name: &str, data: &[u8]) -> Vec<u8> {
    zip_archive_with_compressed_entry(name, 0, data, data)
}

fn zip_deflated_archive_with_entry(name: &str, data: &[u8]) -> Vec<u8> {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data).unwrap();
    let compressed = encoder.finish().unwrap();
    zip_archive_with_compressed_entry(name, 8, data, &compressed)
}

fn zip_archive_with_compressed_entry(
    name: &str,
    compression: u16,
    data: &[u8],
    compressed: &[u8],
) -> Vec<u8> {
    let crc = crc32(data);
    let local_header_offset = 0_u32;
    let mut archive = Vec::new();
    push_u32_le(&mut archive, 0x0403_4b50);
    push_u16_le(&mut archive, 20);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, compression);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, 0);
    push_u32_le(&mut archive, crc);
    push_u32_le(&mut archive, compressed.len() as u32);
    push_u32_le(&mut archive, data.len() as u32);
    push_u16_le(&mut archive, name.len() as u16);
    push_u16_le(&mut archive, 0);
    archive.extend_from_slice(name.as_bytes());
    archive.extend_from_slice(compressed);

    let central_directory_offset = archive.len() as u32;
    push_u32_le(&mut archive, 0x0201_4b50);
    push_u16_le(&mut archive, 20);
    push_u16_le(&mut archive, 20);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, compression);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, 0);
    push_u32_le(&mut archive, crc);
    push_u32_le(&mut archive, compressed.len() as u32);
    push_u32_le(&mut archive, data.len() as u32);
    push_u16_le(&mut archive, name.len() as u16);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, 0);
    push_u32_le(&mut archive, 0);
    push_u32_le(&mut archive, local_header_offset);
    archive.extend_from_slice(name.as_bytes());

    let central_directory_size = archive.len() as u32 - central_directory_offset;
    push_u32_le(&mut archive, 0x0605_4b50);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, 0);
    push_u16_le(&mut archive, 1);
    push_u16_le(&mut archive, 1);
    push_u32_le(&mut archive, central_directory_size);
    push_u32_le(&mut archive, central_directory_offset);
    push_u16_le(&mut archive, 0);
    archive
}

fn push_u16_le(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32_le(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn serve_http_resource_once(
    listener: TcpListener,
    path: &'static str,
    body: [u8; 4],
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0_u8; 1024];
                    let bytes = stream.read(&mut request).unwrap();
                    let request = String::from_utf8_lossy(&request[..bytes]);
                    assert!(request.starts_with(&format!("GET {path} HTTP/1.1\r\n")));
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(response.as_bytes()).unwrap();
                    stream.write_all(&body).unwrap();
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("failed to accept remote resource request: {error}"),
            }
        }
    })
}

fn serve_chunked_http_resource_once(
    listener: TcpListener,
    path: &'static str,
    body: [u8; 4],
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0_u8; 1024];
                    let bytes = stream.read(&mut request).unwrap();
                    let request = String::from_utf8_lossy(&request[..bytes]);
                    assert!(request.starts_with(&format!("GET {path} HTTP/1.1\r\n")));
                    stream
                        .write_all(
                            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n",
                        )
                        .unwrap();
                    stream.write_all(b"2\r\n").unwrap();
                    stream.write_all(&body[..2]).unwrap();
                    stream.write_all(b"\r\n2\r\n").unwrap();
                    stream.write_all(&body[2..]).unwrap();
                    stream.write_all(b"\r\n0\r\n\r\n").unwrap();
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("failed to accept chunked remote resource request: {error}"),
            }
        }
    })
}

fn serve_redirect_http_resource_once(
    listener: TcpListener,
    redirect_path: &'static str,
    location: &str,
    target_path: &'static str,
    body: [u8; 4],
) -> thread::JoinHandle<()> {
    let location = location.to_string();
    thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut saw_redirect = false;
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0_u8; 1024];
                    let bytes = stream.read(&mut request).unwrap();
                    let request = String::from_utf8_lossy(&request[..bytes]);
                    if !saw_redirect {
                        assert!(request.starts_with(&format!("GET {redirect_path} HTTP/1.1\r\n")));
                        let response = format!(
                            "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        stream.write_all(response.as_bytes()).unwrap();
                        saw_redirect = true;
                    } else {
                        assert!(request.starts_with(&format!("GET {target_path} HTTP/1.1\r\n")));
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        stream.write_all(response.as_bytes()).unwrap();
                        stream.write_all(&body).unwrap();
                        return;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    panic!("failed to accept redirected remote resource request: {error}")
                }
            }
        }
    })
}

fn serve_redirect_loop_http_resource(
    listener: TcpListener,
    redirect_path: &'static str,
    location: &str,
    redirects: usize,
) -> thread::JoinHandle<()> {
    let location = location.to_string();
    thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut accepted = 0usize;
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut request = [0_u8; 1024];
                    let bytes = stream.read(&mut request).unwrap();
                    let request = String::from_utf8_lossy(&request[..bytes]);
                    assert!(request.starts_with(&format!("GET {redirect_path} HTTP/1.1\r\n")));
                    let response = format!(
                        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    );
                    stream.write_all(response.as_bytes()).unwrap();
                    accepted += 1;
                    if accepted == redirects {
                        return;
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return;
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("failed to accept redirect loop resource request: {error}"),
            }
        }
    })
}
