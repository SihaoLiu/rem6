use std::fs;
use std::process::Command;

use crate::support::*;

#[test]
fn rem6_run_rejects_load_blob_resource_without_resource_config() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x0000_0073]);
    let path = temp_binary("load-blob-resource-without-resource-config", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-format",
            "json",
            "--load-blob",
            "0x80001000:resource:initrd",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("load blob resource initrd requires --resource-config"));
}

#[test]
fn rem6_run_loads_slash_named_load_blob_resource() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("run-load-blob-resource-slash-id");
    fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    fs::write(workspace.join("initrd-v1.bin"), [0xde, 0xad, 0xbe, 0xef]).unwrap();
    let resource_config = workspace.join("resource-acquire.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
workload_id = "slash-load-blob-resource"
boot_entry = 2147483648

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:slash-load-blob-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:slash-load-blob-kernel"

[[resource_acquire.resources]]
id = "initrd/v1"
kind = "initrd"
digest = "sha256:slash-load-blob-initrd"
locator = "resources/initrd-v1.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://initrd-v1"
artifact = "initrd-v1.bin"
artifact_digest = "sha256:slash-load-blob-initrd"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nmax_tick = 40\nexecute = true\nstats_format = \"json\"\nload_blobs = [\"0x80001000:resource:initrd/v1\"]\nmemory_dumps = [\"0x80001000:4\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains(
        "\"load_blobs\":[{\"address\":\"0x80001000\",\"bytes\":4,\"path\":\"resource:initrd/v1\"}]"
    ));
    assert!(stdout.contains("\"address\":\"0x80001000\""));
    assert!(stdout.contains("\"hex\":\"deadbeef\""));
    assert_stat(&stdout, "sim.load_blobs", "Count", 1, "constant");
}

#[test]
fn rem6_run_rejects_ambiguous_suite_load_blob_resource() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x0000_0073]);
    let workspace = temp_workspace("run-suite-load-blob-resource-ambiguous");
    fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    fs::write(workspace.join("initrd-a.bin"), [0xaa]).unwrap();
    fs::write(workspace.join("initrd-b.bin"), [0xbb]).unwrap();
    let resource_config = workspace.join("resource-acquire-suite.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "ambiguous-load-blob-suite"

[[resource_acquire.manifests]]
workload_id = "boot-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:ambiguous-load-blob-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:ambiguous-load-blob-kernel"

[[resource_acquire.manifests.resources]]
id = "initrd"
kind = "initrd"
digest = "sha256:ambiguous-load-blob-initrd-a"
locator = "resources/initrd-a.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://initrd-a"
artifact = "initrd-a.bin"
artifact_digest = "sha256:ambiguous-load-blob-initrd-a"

[[resource_acquire.manifests]]
workload_id = "side-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "initrd"
kind = "initrd"
digest = "sha256:ambiguous-load-blob-initrd-b"
locator = "resources/initrd-b.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://initrd-b"
artifact = "initrd-b.bin"
artifact_digest = "sha256:ambiguous-load-blob-initrd-b"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nmax_tick = 40\nstats_format = \"json\"\nload_blobs = [\"0x80001000:resource:initrd\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("load blob resource initrd is ambiguous"));
    assert!(stderr.contains("resource-acquire-suite.toml"));
}

#[test]
fn rem6_run_loads_qualified_suite_load_blob_resource() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("run-suite-load-blob-resource-qualified");
    fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    fs::write(workspace.join("initrd-a.bin"), [0xde, 0xad, 0xbe, 0xef]).unwrap();
    fs::write(workspace.join("initrd-b.bin"), [0xba, 0xad, 0xf0, 0x0d]).unwrap();
    let resource_config = workspace.join("resource-acquire-suite.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "qualified-load-blob-suite"

[[resource_acquire.manifests]]
workload_id = "boot-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:qualified-load-blob-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:qualified-load-blob-kernel"

[[resource_acquire.manifests.resources]]
id = "initrd"
kind = "initrd"
digest = "sha256:qualified-load-blob-initrd-a"
locator = "resources/initrd-a.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://initrd-a"
artifact = "initrd-a.bin"
artifact_digest = "sha256:qualified-load-blob-initrd-a"

[[resource_acquire.manifests]]
workload_id = "side-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "initrd"
kind = "initrd"
digest = "sha256:qualified-load-blob-initrd-b"
locator = "resources/initrd-b.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://initrd-b"
artifact = "initrd-b.bin"
artifact_digest = "sha256:qualified-load-blob-initrd-b"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nmax_tick = 40\nexecute = true\nstats_format = \"json\"\nload_blobs = [\"0x80001000:suite-resource:boot-workload/initrd\"]\nmemory_dumps = [\"0x80001000:4\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains(
        "\"load_blobs\":[{\"address\":\"0x80001000\",\"bytes\":4,\"path\":\"suite-resource:boot-workload/initrd\"}]"
    ));
    assert!(stdout.contains("\"address\":\"0x80001000\""));
    assert!(stdout.contains("\"hex\":\"deadbeef\""));
    assert_stat(&stdout, "sim.load_blobs", "Count", 1, "constant");
    assert_stat(&stdout, "sim.load_blob_bytes", "Byte", 4, "constant");
}

#[test]
fn rem6_run_loads_initrd_resource_blob_from_suite_config() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("run-suite-load-blob-resource");
    fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    fs::write(workspace.join("initrd.bin"), [0xde, 0xad, 0xbe, 0xef]).unwrap();
    let resource_config = workspace.join("resource-acquire-suite.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "load-blob-suite"

[[resource_acquire.manifests]]
workload_id = "boot-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:load-blob-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:load-blob-kernel"

[[resource_acquire.manifests.resources]]
id = "initrd"
kind = "initrd"
digest = "sha256:load-blob-initrd"
locator = "resources/initrd.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://initrd"
artifact = "initrd.bin"
artifact_digest = "sha256:load-blob-initrd"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nmax_tick = 40\nexecute = true\nstats_format = \"json\"\nload_blobs = [\"0x80001000:resource:initrd\"]\nmemory_dumps = [\"0x80001000:4\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains(
        "\"load_blobs\":[{\"address\":\"0x80001000\",\"bytes\":4,\"path\":\"resource:initrd\"}]"
    ));
    assert!(stdout.contains("\"address\":\"0x80001000\""));
    assert!(stdout.contains("\"hex\":\"deadbeef\""));
    assert_stat(&stdout, "sim.load_blobs", "Count", 1, "constant");
    assert_stat(&stdout, "sim.load_blob_bytes", "Byte", 4, "constant");
    assert_stat(&stdout, "sim.load_blob0.bytes", "Byte", 4, "constant");
}
