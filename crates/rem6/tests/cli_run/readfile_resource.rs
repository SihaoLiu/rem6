use std::process::Command;

use crate::support::*;

#[test]
fn rem6_run_rejects_readfile_resource_without_resource_config() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x0000_0073]);
    let path = temp_binary("readfile-resource-without-resource-config", &elf);

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
            "--readfile",
            "0x10000000:0x100:resource:boot-readfile",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("readfile resource boot-readfile requires --resource-config"));
}

#[test]
fn rem6_run_rejects_missing_readfile_resource() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x0000_0073]);
    let workspace = temp_workspace("run-readfile-resource-missing");
    std::fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    let resource_config = workspace.join("resource-acquire.toml");
    std::fs::write(
        &resource_config,
        r#"[resource_acquire]
workload_id = "missing-readfile-resource"
boot_entry = 2147483648

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:missing-readfile-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:missing-readfile-kernel"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nmax_tick = 40\nexecute = true\nreadfiles = [\"0x10000000:0x100:resource:boot-readfile\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("readfile resource boot-readfile was not acquired"));
    assert!(stderr.contains("resource-acquire.toml"));
}

#[test]
fn rem6_run_rejects_non_input_readfile_resource() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x0000_0073]);
    let workspace = temp_workspace("run-readfile-resource-wrong-kind");
    std::fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    let resource_config = workspace.join("resource-acquire.toml");
    std::fs::write(
        &resource_config,
        r#"[resource_acquire]
workload_id = "wrong-kind-readfile-resource"
boot_entry = 2147483648

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:wrong-kind-readfile-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:wrong-kind-readfile-kernel"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nmax_tick = 40\nexecute = true\nreadfiles = [\"0x10000000:0x100:resource:kernel\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("readfile resource kernel"));
    assert!(stderr.contains("has kind kernel; expected input"));
}

#[test]
fn rem6_run_rejects_ambiguous_suite_readfile_resource() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x0000_0073]);
    let workspace = temp_workspace("run-suite-readfile-resource-ambiguous");
    std::fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    std::fs::write(workspace.join("input-a.bin"), [0xaa]).unwrap();
    std::fs::write(workspace.join("input-b.bin"), [0xbb]).unwrap();
    let resource_config = workspace.join("resource-acquire-suite.toml");
    std::fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "ambiguous-readfile-suite"

[[resource_acquire.manifests]]
workload_id = "boot-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:ambiguous-readfile-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:ambiguous-readfile-kernel"

[[resource_acquire.manifests.resources]]
id = "boot-readfile"
kind = "input"
digest = "sha256:ambiguous-readfile-input-a"
locator = "resources/input-a.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://input-a"
artifact = "input-a.bin"
artifact_digest = "sha256:ambiguous-readfile-input-a"

[[resource_acquire.manifests]]
workload_id = "side-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "boot-readfile"
kind = "input"
digest = "sha256:ambiguous-readfile-input-b"
locator = "resources/input-b.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://input-b"
artifact = "input-b.bin"
artifact_digest = "sha256:ambiguous-readfile-input-b"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nmax_tick = 40\nexecute = true\nreadfiles = [\"0x10000000:0x100:resource:boot-readfile\"]\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("readfile resource boot-readfile is ambiguous"));
    assert!(stderr.contains("resource-acquire-suite.toml"));
}

#[test]
fn rem6_run_loads_qualified_suite_readfile_resource() {
    let program = riscv64_program(&[i_type(0, 10, 0x3, 5, 0x03), 0x0010_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("run-suite-readfile-resource-qualified");
    std::fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    std::fs::write(
        workspace.join("input-a.bin"),
        [0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11],
    )
    .unwrap();
    std::fs::write(
        workspace.join("input-b.bin"),
        [0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21],
    )
    .unwrap();
    let resource_config = workspace.join("resource-acquire-suite.toml");
    std::fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "qualified-readfile-suite"

[[resource_acquire.manifests]]
workload_id = "boot-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:qualified-readfile-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:qualified-readfile-kernel"

[[resource_acquire.manifests.resources]]
id = "boot-readfile"
kind = "input"
digest = "sha256:qualified-readfile-input-a"
locator = "resources/input-a.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://input-a"
artifact = "input-a.bin"
artifact_digest = "sha256:qualified-readfile-input-a"

[[resource_acquire.manifests]]
workload_id = "side-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "boot-readfile"
kind = "input"
digest = "sha256:qualified-readfile-input-b"
locator = "resources/input-b.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://input-b"
artifact = "input-b.bin"
artifact_digest = "sha256:qualified-readfile-input-b"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    std::fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nmax_tick = 80\nexecute = true\nstats_format = \"json\"\nriscv_boot_a0 = 268435456\nreadfiles = [\"0x10000000:0x100:suite-resource:boot-workload/boot-readfile\"]\n",
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
    assert!(stdout.contains("\"registers\":{\"x5\":\"0x1112131415161718\",\"x10\":\"0x10000000\"}"));
    assert!(
        stdout.contains(
            "\"readfiles\":[{\"base\":\"0x10000000\",\"size\":256,\"bytes\":8,\"path\":\"suite-resource:boot-workload/boot-readfile\"}]"
        )
    );
    assert_stat(&stdout, "sim.readfiles", "Count", 1, "constant");
    assert_stat(&stdout, "sim.readfile0.bytes", "Byte", 8, "constant");
}
