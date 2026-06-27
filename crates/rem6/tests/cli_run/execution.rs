use std::{fs, process::Command};

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_executes_riscv_elf_on_parallel_cores_and_emits_core_stats() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("parallel-exec", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"cores\":2"));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"trap\":\"environment_call\""));
    assert!(stdout.contains("\"parallel\":{\"scheduler\":{"));
    assert!(stdout.contains("\"worker_limit\":2"));
    assert!(stdout.contains("\"worker_slots\":[{\"slot\":0"));
    assert!(stdout.contains("\"worker_lanes\":[{\"lane\":0,\"partition\":0"));
    assert!(stdout.contains("{\"lane\":1,\"partition\":1"));
    assert!(stdout.contains("\"partitions\":[{\"partition\":0"));
    assert!(stdout.contains("\"transport\":{\"fetch\":{\"requests\":4"));
    assert!(stdout.contains("\"route\":0,\"source\":\"cpu0.ifetch\",\"requests\":2"));
    assert!(stdout.contains("\"route\":2,\"source\":\"cpu1.ifetch\",\"requests\":2"));
    assert!(stdout.contains("\"data\":{\"requests\":0"));
    assert!(stdout.contains("\"cpu\":0"));
    assert!(stdout.contains("\"cpu\":1"));
    assert!(stdout.contains("\"trap_pc\":\"0x80000004\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(stdout.contains("\"path\":\"sim.instructions.committed\""));
    assert!(stdout.contains("\"path\":\"sim.cpu0.instructions.committed\""));
    assert!(stdout.contains("\"path\":\"sim.cpu1.instructions.committed\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.max_workers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.dispatches\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.batches\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.total_workers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.active_partitions\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.remote_sends\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.batch.worker_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.batch.worker_capacity_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.batch.idle_worker_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.frontiers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.final_frontiers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.ready_partitions\""));
    assert!(!stdout.contains("\"path\":\"sim.parallel.scheduler.frontier0.partition\""));
    assert!(!stdout.contains("\"path\":\"sim.parallel.scheduler.frontier0.now\""));
    assert!(!stdout.contains("\"path\":\"sim.parallel.scheduler.frontier0.safe_until\""));
    assert!(!stdout.contains("\"path\":\"sim.parallel.scheduler.frontier0.pending_events\""));
    assert!(!stdout.contains("\"path\":\"sim.parallel.scheduler.final_frontier0.partition\""));
    assert!(!stdout.contains("\"path\":\"sim.parallel.scheduler.final_frontier0.now\""));
    assert!(!stdout.contains("\"path\":\"sim.parallel.scheduler.ready_partition0.partition\""));
    assert!(!stdout.contains("\"path\":\"sim.parallel.scheduler.ready_partition0.next_tick\""));
    assert!(stdout.contains("\"frontiers\":[{\"partition\":0"));
    assert!(stdout.contains("\"final_frontiers\":[{\"partition\":0"));
    assert!(stdout.contains("\"ready_partitions\":[{\"partition\":0,\"next_tick\":0}"));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.frontier.now\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.frontier.safe_until\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.frontier.pending_events\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.frontier.final_now\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker0.active_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker0.idle_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker1.active_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker1.idle_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker0.partition0.active_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker1.partition1.active_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.scheduler.dispatches\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.scheduler.workers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition1.scheduler.dispatches\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition1.scheduler.workers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition2.scheduler.remote_receives\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition3.scheduler.remote_receives\""));
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        2,
        4,
        2,
    );
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route2.source.cpu1.ifetch",
        2,
        4,
        2,
    );
    assert!(stdout.contains("\"value\":4"));
    assert!(stdout.contains("\"value\":2"));
}

#[test]
fn rem6_run_loads_execution_defaults_from_toml_config() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("toml-config-exec", &elf);
    let config = temp_config(
        "toml-config-exec",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ncores = 2\nparallel_workers = 1\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
    assert!(stdout.contains("\"binary\":\""));
    assert!(stdout.contains("\"max_tick\":40"));
    assert!(stdout.contains("\"cores\":2"));
    assert!(stdout.contains("\"worker_limit\":1"));
    assert!(stdout.contains("\"cpu\":0"));
    assert!(stdout.contains("\"cpu\":1"));
}

#[test]
fn rem6_run_cli_flags_override_toml_config_defaults() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("toml-config-override", &elf);
    let config = temp_config(
        "toml-config-override",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 20\nstats_format = \"json\"\nexecute = true\ncores = 2\nparallel_workers = 2\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--max-tick",
            "40",
            "--cores",
            "1",
            "--parallel-workers",
            "1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"max_tick\":40"));
    assert!(stdout.contains("\"cores\":1"));
    assert!(stdout.contains("\"worker_limit\":1"));
    assert!(stdout.contains("\"cpu\":0"));
    assert!(!stdout.contains("\"cpu\":1"));
}

#[test]
fn rem6_run_resolves_toml_relative_binary_from_config_directory() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("toml-config-relative");
    let binary_name = format!("kernel-{}.elf", std::process::id());
    let binary = workspace.join(&binary_name);
    fs::write(&binary, elf).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\n",
            binary_name
        ),
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
    assert!(stdout.contains("\"binary\":\""));
    assert!(stdout.contains(&binary_name));
}

#[test]
fn rem6_run_loads_kernel_binary_from_manifest_resource_config() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("run-resource-config-kernel");
    let binary_name = "kernel.elf";
    let binary = workspace.join(binary_name);
    fs::write(&binary, &elf).unwrap();
    let resource_config = workspace.join("resource-acquire.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
workload_id = "run-resource-cli"
boot_entry = 2147483648
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:run-kernel-resource"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:run-kernel-resource"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ncores = 1\n",
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
    assert!(stdout.contains("\"binary\":\"resource-config:"));
    assert!(stdout.contains("resource-acquire.toml"));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"trap\":\"environment_call\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert_stat(
        &stdout,
        "sim.instructions.committed",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_loads_preloaded_kernel_from_acquisition_locator() {
    let program = riscv64_program(&[
        0x0090_0293, // addi x5, x0, 9
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("run-preloaded-resource-config-kernel");
    fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    let resource_config = workspace.join("resource-acquire.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
workload_id = "run-preloaded-resource-cli"
boot_entry = 2147483648
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:run-preloaded-kernel-resource"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "preloaded"
acquisition_locator = "kernel.elf"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ncores = 1\n",
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
    assert!(stdout.contains("\"binary\":\"resource-config:"));
    assert!(stdout.contains("resource-acquire.toml"));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"trap\":\"environment_call\""));
    assert!(stdout.contains("\"x5\":\"0x9\""));
    assert_stat(
        &stdout,
        "sim.instructions.committed",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_loads_kernel_binary_from_suite_resource_config() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("run-suite-resource-config-kernel");
    fs::write(workspace.join("kernel.elf"), &elf).unwrap();
    fs::write(workspace.join("input.bin"), [0xaa]).unwrap();
    let resource_config = workspace.join("resource-acquire-suite.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "run-suite-cli"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "run-kernel-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel"
kind = "kernel"
digest = "sha256:run-suite-kernel"
locator = "resources/kernel.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel"
artifact = "kernel.elf"
artifact_digest = "sha256:run-suite-kernel"

[[resource_acquire.manifests]]
workload_id = "side-input-workload"
boot_entry = 4096

[[resource_acquire.manifests.resources]]
id = "input"
kind = "input"
digest = "sha256:run-suite-input"
locator = "resources/input.bin"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://input"
artifact = "input.bin"
artifact_digest = "sha256:run-suite-input"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ncores = 1\n",
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
    assert!(stdout.contains("\"binary\":\"resource-config:"));
    assert!(stdout.contains("resource-acquire-suite.toml"));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"trap\":\"environment_call\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert_stat(
        &stdout,
        "sim.instructions.committed",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_selects_kernel_binary_from_suite_resource_config() {
    let program_a = riscv64_program(&[
        0x00a0_0293, // addi x5, x0, 10
        0x0000_0073, // ecall
    ]);
    let program_b = riscv64_program(&[
        0x00b0_0293, // addi x5, x0, 11
        0x0000_0073, // ecall
    ]);
    let elf_a = riscv64_elf(0x8000_0000, 0x8000_0000, &program_a);
    let elf_b = riscv64_elf(0x8000_0000, 0x8000_0000, &program_b);
    let workspace = temp_workspace("run-suite-resource-config-selected-kernel");
    fs::write(workspace.join("kernel-a.elf"), &elf_a).unwrap();
    fs::write(workspace.join("kernel-b.elf"), &elf_b).unwrap();
    let resource_config = workspace.join("resource-acquire-suite.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
suite_id = "selected-kernel-suite"
stats_format = "json"

[[resource_acquire.manifests]]
workload_id = "kernel-a-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel-a"
kind = "kernel"
digest = "sha256:selected-kernel-a"
locator = "resources/kernel-a.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-a"
artifact = "kernel-a.elf"
artifact_digest = "sha256:selected-kernel-a"

[[resource_acquire.manifests]]
workload_id = "kernel-b-workload"
boot_entry = 2147483648

[[resource_acquire.manifests.resources]]
id = "kernel-b"
kind = "kernel"
digest = "sha256:selected-kernel-b"
locator = "resources/kernel-b.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-b"
artifact = "kernel-b.elf"
artifact_digest = "sha256:selected-kernel-b"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nkernel_resource = \"suite-resource:kernel-b-workload/kernel-b\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ncores = 1\n",
    )
    .unwrap();

    let assert_kernel_b_output = |output: std::process::Output| {
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("\"status\":\"executed_until_trap\""));
        assert!(stdout.contains("\"binary\":\"resource-config:"));
        assert!(
            stdout.contains("\"kernel_resource\":\"suite-resource:kernel-b-workload/kernel-b\"")
        );
        assert!(stdout.contains("resource-acquire-suite.toml"));
        assert!(stdout.contains("\"stop_code\":0"));
        assert!(stdout.contains("\"trap\":\"environment_call\""));
        assert!(stdout.contains("\"x5\":\"0xb\""));
        assert_stat(
            &stdout,
            "sim.instructions.committed",
            "Count",
            2,
            "monotonic",
        );
    };

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();
    assert_kernel_b_output(output);

    let cli_config = workspace.join("run-cli.toml");
    fs::write(
        &cli_config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire-suite.toml\"\nkernel_resource = \"suite-resource:kernel-a-workload/kernel-a\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ncores = 1\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            cli_config.to_str().unwrap(),
            "--kernel-resource",
            "suite-resource:kernel-b-workload/kernel-b",
        ])
        .output()
        .unwrap();
    assert_kernel_b_output(output);
}

#[test]
fn rem6_run_selects_kernel_binary_from_manifest_resource_config() {
    let program_a = riscv64_program(&[
        0x00d0_0293, // addi x5, x0, 13
        0x0000_0073, // ecall
    ]);
    let program_b = riscv64_program(&[
        0x00e0_0293, // addi x5, x0, 14
        0x0000_0073, // ecall
    ]);
    let elf_a = riscv64_elf(0x8000_0000, 0x8000_0000, &program_a);
    let elf_b = riscv64_elf(0x8000_0000, 0x8000_0000, &program_b);
    let workspace = temp_workspace("run-manifest-resource-config-selected-kernel");
    fs::write(workspace.join("kernel-a.elf"), &elf_a).unwrap();
    fs::write(workspace.join("kernel-b.elf"), &elf_b).unwrap();
    let resource_config = workspace.join("resource-acquire.toml");
    fs::write(
        &resource_config,
        r#"[resource_acquire]
workload_id = "selected-manifest-kernel"
boot_entry = 2147483648
stats_format = "json"

[[resource_acquire.resources]]
id = "kernel-a"
kind = "kernel"
digest = "sha256:selected-manifest-kernel-a"
locator = "resources/kernel-a.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-a"
artifact = "kernel-a.elf"
artifact_digest = "sha256:selected-manifest-kernel-a"

[[resource_acquire.resources]]
id = "kernel-b"
kind = "kernel"
digest = "sha256:selected-manifest-kernel-b"
locator = "resources/kernel-b.elf"
required = true
acquisition_kind = "local-file"
acquisition_locator = "catalog://kernel-b"
artifact = "kernel-b.elf"
artifact_digest = "sha256:selected-manifest-kernel-b"
"#,
    )
    .unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"resource-acquire.toml\"\nkernel_resource = \"resource:kernel-b\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ncores = 1\n",
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
    assert!(stdout.contains("\"binary\":\"resource-config:"));
    assert!(stdout.contains("\"kernel_resource\":\"resource:kernel-b\""));
    assert!(stdout.contains("resource-acquire.toml"));
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0xe\""));
    assert_stat(
        &stdout,
        "sim.instructions.committed",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_binary_flag_clears_config_kernel_resource() {
    let program = riscv64_program(&[
        0x00c0_0293, // addi x5, x0, 12
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("run-binary-overrides-kernel-resource");
    let binary = workspace.join("direct.elf");
    fs::write(&binary, &elf).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nresource_config = \"unused-resource-acquire-suite.toml\"\nkernel_resource = \"suite-resource:unused-workload/kernel\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\ncores = 1\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--binary",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"binary\":\""));
    assert!(stdout.contains("direct.elf"));
    assert!(stdout.contains("\"kernel_resource\":null"));
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0xc\""));
}

#[test]
fn rem6_run_cli_load_blob_flags_replace_toml_load_blob_defaults() {
    let program = riscv64_program(&[0x0000_0073]); // ecall
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("toml-load-blob-override", &elf);
    let workspace = temp_workspace("toml-load-blob-override");
    let default_blob = workspace.join("default.bin");
    let cli_blob0 = workspace.join("cli0.bin");
    let cli_blob1 = workspace.join("cli1.bin");
    fs::write(&default_blob, [0xaa]).unwrap();
    fs::write(&cli_blob0, [0xbb]).unwrap();
    fs::write(&cli_blob1, [0xcc]).unwrap();
    let config = workspace.join("run.toml");
    fs::write(
        &config,
        format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nstats_format = \"json\"\nload_blobs = [\"0x90000000:default.bin\"]\n",
            binary.display()
        ),
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .current_dir(std::env::temp_dir())
        .args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--load-blob",
            &format!("0x90000010:{}", cli_blob0.display()),
            "--load-blob",
            &format!("0x90000020:{}", cli_blob1.display()),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("default.bin"));
    assert!(stdout.contains("cli0.bin"));
    assert!(stdout.contains("cli1.bin"));
    assert!(stdout.contains("\"address\":\"0x90000010\""));
    assert!(stdout.contains("\"address\":\"0x90000020\""));
    assert!(!stdout.contains("\"address\":\"0x90000000\""));
}

#[test]
fn rem6_run_cli_dump_memory_flags_replace_toml_dump_memory_defaults() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
        0x0000_0013, // addi x0, x0, 0
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("toml-dump-memory-override", &elf);
    let config = temp_config(
        "toml-dump-memory-override",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nstats_format = \"json\"\nexecute = true\nmemory_dumps = [\"0x80000000:4\"]\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--dump-memory",
            "0x80000004:4",
            "--dump-memory",
            "0x80000008:4",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"address\":\"0x80000004\""));
    assert!(stdout.contains("\"address\":\"0x80000008\""));
    assert!(!stdout.contains("\"address\":\"0x80000000\""));
}

#[test]
fn rem6_run_config_scan_preserves_non_config_flag_errors() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--bogus", "--isa"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unknown flag --bogus"));
}

#[test]
fn rem6_run_reports_illegal_instruction_trap_name() {
    let program = riscv64_program(&[
        0x0000_12b7,                                    // lui x5, 0x1
        i_type(16, 5, 0x0, 5, 0x13),                    // addi x5, x5, 16
        (0x341 << 20) | (5 << 15) | (0x1 << 12) | 0x73, // csrrw x0, mepc, x5
        0x3020_0073,                                    // mret into user mode
        0x3020_0073,                                    // mret from user mode
    ]);
    let elf = riscv64_elf(0x1000, 0x1000, &program);
    let path = temp_binary("illegal-instruction-trap", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "20",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_reason\":\"host_trap\""));
    assert!(stdout.contains("\"stop_code\":2"));
    assert!(stdout.contains("\"trap\":\"illegal_instruction\""));
    assert!(stdout.contains("\"trap_pc\":\"0x1010\""));
}

#[test]
fn rem6_run_respects_explicit_parallel_worker_limit() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("parallel-worker-limit", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--parallel-workers",
            "1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert_stat(
        &stdout,
        "sim.parallel.scheduler.worker_limit",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.parallel.scheduler.max_workers",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_stops_riscv_execution_at_instruction_limit() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("instruction-limit", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "1",
            "--max-instructions",
            "1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_instruction_limit\""));
    assert!(stdout.contains("\"stop_reason\":\"instruction_limit\""));
    assert!(stdout.contains("\"instruction_limit\":1"));
    assert!(stdout.contains("\"committed_instructions\":1"));
    assert!(stdout.contains("\"pc\":\"0x80000004\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\""));
    assert!(!stdout.contains("\"stop_code\""));
    assert!(!stdout.contains("\"trap\""));
    assert_stat(&stdout, "sim.instructions.limit", "Count", 1, "constant");
    assert_stat(
        &stdout,
        "sim.stop.instruction_limit",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.instructions.committed",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_instruction_limit_is_a_hard_cap_across_parallel_cores() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("parallel-instruction-limit", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--max-instructions",
            "1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_instruction_limit\""));
    assert!(stdout.contains("\"cores\":2"));
    assert!(stdout.contains("\"committed_instructions\":1"));
    assert_stat(
        &stdout,
        "sim.instructions.committed",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.stop.instruction_limit",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.instructions.committed",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu1.instructions.committed",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_stops_riscv_execution_at_tick_limit() {
    let program = riscv64_program(&[
        b_type(0, 0, 0, 0x0), // beq x0, x0, self
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("tick-limit", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "4",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_tick_limit\""));
    assert!(stdout.contains("\"stop_reason\":\"tick_limit\""));
    assert!(stdout.contains("\"executed_ticks\":4"));
    assert!(stdout.contains("\"final_tick\":4"));
    assert!(stdout.contains("\"tick_limit\":4"));
    assert!(!stdout.contains("\"stop_code\""));
    assert!(!stdout.contains("\"trap\""));
    assert_stat(&stdout, "sim.final_tick", "Tick", 4, "monotonic");
    assert_stat(&stdout, "sim.stop.tick_limit", "Count", 1, "constant");
}

#[test]
fn rem6_run_accepts_scheduler_min_remote_delay_runtime_option() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("min-remote-delay", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "1",
            "--min-remote-delay",
            "4",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_reason\":\"host_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"min_remote_delay\":4"));
    assert!(stdout.contains("\"host_event_delay\":4"));
    assert!(stdout.contains("\"executed_ticks\":20"));
    assert!(stdout.contains("\"final_tick\":20"));
    assert_stat(
        &stdout,
        "sim.parallel.scheduler.min_remote_delay",
        "Tick",
        4,
        "constant",
    );
    assert_stat(&stdout, "sim.host.event_delay", "Tick", 4, "constant");
    assert_stat(&stdout, "sim.final_tick", "Tick", 20, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 2, 16, 8);
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        2,
        16,
        8,
    );
}

#[test]
fn rem6_run_accepts_memory_route_delay_runtime_option() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(16, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&7u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("memory-route-delay", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "1",
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
            "5",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"memory_route_delay\":5"));
    assert!(stdout.contains("\"min_remote_delay\":2"));
    assert!(stdout.contains("\"executed_ticks\":52"));
    assert!(stdout.contains("\"final_tick\":52"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert_stat(&stdout, "sim.memory.route_delay", "Tick", 5, "constant");
    assert_stat(&stdout, "sim.final_tick", "Tick", 52, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 4, 40, 10);
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        4,
        40,
        10,
    );
    assert_transport_stats(&stdout, "sim.memory.data", 1, 10, 10);
    assert_transport_stats(
        &stdout,
        "sim.memory.data.route1.source.cpu0.dmem",
        1,
        10,
        10,
    );
}

#[test]
fn rem6_run_can_execute_riscv_elf_through_dram_memory_and_emit_dram_stats() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-memory", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(stdout.contains("\"dram\":{\"active_targets\":1"));
    assert!(stdout.contains("\"accesses\":2"));
    assert!(stdout.contains("\"reads\":2"));
    assert!(stdout.contains("\"row_hits\":1"));
    assert!(stdout.contains("\"row_misses\":1"));
    assert!(stdout.contains("\"refreshes\":0"));
    assert!(stdout.contains("\"refresh_ticks\":0"));
    assert!(stdout.contains("\"total_ready_latency_ticks\":13"));
    assert!(stdout.contains("\"max_ready_latency_ticks\":8"));
    assert!(stdout.contains("\"low_power_timing\":{\"precharge_powerdown_entry_delay\":0"));
    assert!(stdout.contains("\"self_refresh_entry_delay\":0"));
    assert!(stdout.contains("\"exit_latency\":0"));
    assert!(stdout.contains("\"self_refresh_exit_latency\":0"));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let resources = json
        .pointer("/memory_resources")
        .expect("run JSON should include memory resources");
    let resource_bank_read_bytes =
        json_u64(resources, "/dram/targets/0/ports/0/banks/0/read_bytes");
    let resource_bank_total_ready_latency_ticks = json_u64(
        resources,
        "/dram/targets/0/ports/0/banks/0/total_ready_latency_ticks",
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/target"),
        json_u64(&json, "/dram/targets/0/target")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/accesses"),
        json_u64(&json, "/dram/targets/0/accesses")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/reads"),
        json_u64(&json, "/dram/targets/0/reads")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/row_misses"),
        json_u64(&json, "/dram/targets/0/row_misses")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/port"),
        json_u64(&json, "/dram/targets/0/ports/0/port")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/accesses"),
        json_u64(&json, "/dram/targets/0/ports/0/accesses")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/commands"),
        json_u64(&json, "/dram/targets/0/ports/0/commands")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/banks/0/bank"),
        json_u64(&json, "/dram/targets/0/ports/0/banks/0/bank")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/banks/0/accesses"),
        json_u64(&json, "/dram/targets/0/ports/0/banks/0/accesses")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/banks/0/row_hits"),
        json_u64(&json, "/dram/targets/0/ports/0/banks/0/row_hits")
    );
    assert_eq!(
        resource_bank_read_bytes,
        json_u64(&json, "/dram/targets/0/ports/0/banks/0/read_bytes")
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/banks/0/row_misses"),
        json_u64(&json, "/dram/targets/0/ports/0/banks/0/row_misses")
    );
    assert_eq!(
        resource_bank_total_ready_latency_ticks,
        json_u64(
            &json,
            "/dram/targets/0/ports/0/banks/0/total_ready_latency_ticks"
        )
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.active_targets",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(&stdout, "sim.memory.dram.accesses", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.memory.dram.reads", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.memory.dram.row_hits", "Count", 1, "monotonic");
    assert_stat(
        &stdout,
        "sim.memory.dram.row_misses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.refreshes",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.refresh_ticks",
        "Tick",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.total_ready_latency_ticks",
        "Tick",
        13,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.max_ready_latency_ticks",
        "Tick",
        8,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.reads",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.row_misses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.commands",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.bank0.accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.bank0.row_hits",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.bank0.row_misses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.reads",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.row_misses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.commands",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.bank0.accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.bank0.row_hits",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.bank0.read_bytes",
        "Byte",
        resource_bank_read_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.bank0.row_misses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.bank0.total_ready_latency_ticks",
        "Tick",
        resource_bank_total_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.parallel_ports",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.scheduler_banks",
        "Count",
        4,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.precharge_powerdown_entry_delay",
        "Tick",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.self_refresh_entry_delay",
        "Tick",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.exit_latency",
        "Tick",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.self_refresh_exit_latency",
        "Tick",
        0,
        "constant",
    );
}

#[test]
fn rem6_run_dram_memory_resources_expose_byte_row_hit_and_read_latency_counters() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(7, 0, 0x0, 5, 0x13),                  // addi x5, x0, 7
        s_type(0, 5, 2, 0x3),                        // sd x5, 0(x2)
        i_type(1, 5, 0x0, 5, 0x13),                  // addi x5, x5, 1
        s_type(8, 5, 2, 0x3),                        // sd x5, 8(x2)
        i_type(0, 2, 0x3, 6, 0x03),                  // ld x6, 0(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-resource-split-row-hits", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "320",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x6\":\"0x7\""));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let resources = json
        .pointer("/memory_resources")
        .expect("run JSON should include memory resources");
    let dram_reads = json_u64(&json, "/dram/reads");
    let dram_writes = json_u64(&json, "/dram/writes");
    let dram_row_hits = json_u64(&json, "/dram/row_hits");
    let dram_read_row_hits = json_u64(&json, "/dram/read_row_hits");
    let dram_write_row_hits = json_u64(&json, "/dram/write_row_hits");
    let dram_bank_reads = sum_dram_bank_field(&json, "reads");
    let dram_bank_writes = sum_dram_bank_field(&json, "writes");
    let dram_bank_refreshes = sum_dram_bank_field(&json, "refreshes");
    let dram_bank_refresh_ticks = sum_dram_bank_field(&json, "refresh_ticks");
    let dram_read_bytes = sum_dram_bank_field(&json, "read_bytes");
    let dram_write_bytes = sum_dram_bank_field(&json, "write_bytes");
    let dram_target0_read_bytes = json_u64(&json, "/dram/targets/0/read_bytes");
    let dram_target0_write_bytes = json_u64(&json, "/dram/targets/0/write_bytes");
    let dram_port0_bank_count = json
        .pointer("/dram/targets/0/ports/0/banks")
        .and_then(Value::as_array)
        .expect("DRAM port bank array")
        .len() as u64;
    let dram_port0_active_banks = json_u64(&json, "/dram/targets/0/ports/0/active_banks");
    let dram_port0_read_bytes = json_u64(&json, "/dram/targets/0/ports/0/read_bytes");
    let dram_port0_write_bytes = json_u64(&json, "/dram/targets/0/ports/0/write_bytes");
    let dram_port0_row_hits = json_u64(&json, "/dram/targets/0/ports/0/row_hits");
    let dram_port0_read_row_hits = json_u64(&json, "/dram/targets/0/ports/0/read_row_hits");
    let dram_port0_write_row_hits = json_u64(&json, "/dram/targets/0/ports/0/write_row_hits");
    let dram_port0_row_misses = json_u64(&json, "/dram/targets/0/ports/0/row_misses");
    let dram_port0_refreshes = json_u64(&json, "/dram/targets/0/ports/0/refreshes");
    let dram_port0_refresh_ticks = json_u64(&json, "/dram/targets/0/ports/0/refresh_ticks");
    let dram_port0_total_ready_latency_ticks =
        json_u64(&json, "/dram/targets/0/ports/0/total_ready_latency_ticks");
    let dram_port0_max_ready_latency_ticks =
        json_u64(&json, "/dram/targets/0/ports/0/max_ready_latency_ticks");
    let dram_bank0_reads = json_u64(&json, "/dram/targets/0/ports/0/banks/0/reads");
    let dram_bank0_writes = json_u64(&json, "/dram/targets/0/ports/0/banks/0/writes");
    let dram_read_ready_latency_ticks = stat_value(&stdout, "system.mem_ctrl.dram.totMemAccLat");

    assert!(dram_reads > 0);
    assert!(dram_writes > 0);
    assert!(dram_read_row_hits > 0);
    assert!(dram_write_row_hits <= dram_writes);
    assert_eq!(dram_bank_reads, dram_reads);
    assert_eq!(dram_bank_writes, dram_writes);
    assert_eq!(dram_bank_refreshes, json_u64(&json, "/dram/refreshes"));
    assert_eq!(
        dram_bank_refresh_ticks,
        json_u64(&json, "/dram/refresh_ticks")
    );
    assert!(dram_read_bytes > 0);
    assert!(dram_write_bytes > 0);
    assert_eq!(dram_target0_read_bytes, dram_read_bytes);
    assert_eq!(dram_target0_write_bytes, dram_write_bytes);
    assert_eq!(dram_port0_active_banks, dram_port0_bank_count);
    assert_eq!(dram_port0_read_bytes, dram_read_bytes);
    assert_eq!(dram_port0_write_bytes, dram_write_bytes);
    assert_eq!(dram_port0_row_hits, dram_row_hits);
    assert_eq!(dram_port0_read_row_hits, dram_read_row_hits);
    assert_eq!(dram_port0_write_row_hits, dram_write_row_hits);
    assert_eq!(
        dram_port0_row_hits,
        dram_port0_read_row_hits + dram_port0_write_row_hits
    );
    assert_eq!(dram_port0_row_misses, json_u64(&json, "/dram/row_misses"));
    assert_eq!(dram_port0_refreshes, json_u64(&json, "/dram/refreshes"));
    assert_eq!(
        dram_port0_refresh_ticks,
        json_u64(&json, "/dram/refresh_ticks")
    );
    assert_eq!(
        dram_port0_total_ready_latency_ticks,
        json_u64(&json, "/dram/total_ready_latency_ticks")
    );
    assert_eq!(
        dram_port0_max_ready_latency_ticks,
        json_u64(&json, "/dram/max_ready_latency_ticks")
    );
    assert!(dram_bank0_reads > 0);
    assert!(dram_bank0_writes > 0);
    assert!(dram_read_ready_latency_ticks > 0);
    assert_eq!(dram_row_hits, dram_read_row_hits + dram_write_row_hits);
    assert!(dram_read_ready_latency_ticks <= json_u64(&json, "/dram/total_ready_latency_ticks"));
    assert_eq!(json_u64(&json, "/dram/read_bytes"), dram_read_bytes);
    assert_eq!(json_u64(&json, "/dram/write_bytes"), dram_write_bytes);
    assert_eq!(
        json_u64(&json, "/dram/read_ready_latency_ticks"),
        dram_read_ready_latency_ticks
    );
    assert_eq!(
        json_u64(resources, "/dram/read_row_hits"),
        dram_read_row_hits
    );
    assert_eq!(
        json_u64(resources, "/dram/write_row_hits"),
        dram_write_row_hits
    );
    assert_eq!(json_u64(resources, "/dram/read_bytes"), dram_read_bytes);
    assert_eq!(json_u64(resources, "/dram/write_bytes"), dram_write_bytes);
    assert_eq!(
        json_u64(resources, "/dram/read_ready_latency_ticks"),
        dram_read_ready_latency_ticks
    );
    assert_eq!(sum_dram_bank_field(resources, "reads"), dram_bank_reads);
    assert_eq!(sum_dram_bank_field(resources, "writes"), dram_bank_writes);
    assert_eq!(
        sum_dram_bank_field(resources, "refreshes"),
        dram_bank_refreshes
    );
    assert_eq!(
        sum_dram_bank_field(resources, "refresh_ticks"),
        dram_bank_refresh_ticks
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/read_bytes"),
        dram_target0_read_bytes
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/write_bytes"),
        dram_target0_write_bytes
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/read_bytes"),
        dram_port0_read_bytes
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/write_bytes"),
        dram_port0_write_bytes
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/active_banks"),
        dram_port0_active_banks
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/row_hits"),
        dram_port0_row_hits
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/read_row_hits"),
        dram_port0_read_row_hits
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/write_row_hits"),
        dram_port0_write_row_hits
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/row_misses"),
        dram_port0_row_misses
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/refreshes"),
        dram_port0_refreshes
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/refresh_ticks"),
        dram_port0_refresh_ticks
    );
    assert_eq!(
        json_u64(
            resources,
            "/dram/targets/0/ports/0/total_ready_latency_ticks"
        ),
        dram_port0_total_ready_latency_ticks
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/max_ready_latency_ticks"),
        dram_port0_max_ready_latency_ticks
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/banks/0/reads"),
        dram_bank0_reads
    );
    assert_eq!(
        json_u64(resources, "/dram/targets/0/ports/0/banks/0/writes"),
        dram_bank0_writes
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.read_row_hits",
        "Count",
        dram_read_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.write_row_hits",
        "Count",
        dram_write_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.read_bytes",
        "Byte",
        dram_read_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.write_bytes",
        "Byte",
        dram_write_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.read_ready_latency_ticks",
        "Tick",
        dram_read_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.bank0.reads",
        "Count",
        dram_bank0_reads,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.bank0.reads"),
        dram_bank0_reads
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.bank0.writes",
        "Count",
        dram_bank0_writes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.bank0.writes"),
        dram_bank0_writes
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.read_bytes",
        "Byte",
        dram_target0_read_bytes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.read_bytes"),
        dram_target0_read_bytes
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.write_bytes",
        "Byte",
        dram_target0_write_bytes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.write_bytes"),
        dram_target0_write_bytes
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.active_banks",
        "Count",
        dram_port0_active_banks,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.active_banks"),
        dram_port0_active_banks
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.read_bytes",
        "Byte",
        dram_port0_read_bytes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.read_bytes"),
        dram_port0_read_bytes
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.write_bytes",
        "Byte",
        dram_port0_write_bytes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.write_bytes"),
        dram_port0_write_bytes
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.row_hits",
        "Count",
        dram_port0_row_hits,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.row_hits"),
        dram_port0_row_hits
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.read_row_hits",
        "Count",
        dram_port0_read_row_hits,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.read_row_hits"),
        dram_port0_read_row_hits
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.write_row_hits",
        "Count",
        dram_port0_write_row_hits,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.write_row_hits"),
        dram_port0_write_row_hits
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.row_misses",
        "Count",
        dram_port0_row_misses,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.row_misses"),
        dram_port0_row_misses
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.refreshes",
        "Count",
        dram_port0_refreshes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.refreshes"),
        dram_port0_refreshes
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.refresh_ticks",
        "Tick",
        dram_port0_refresh_ticks,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.dram.target0.port0.refresh_ticks"),
        dram_port0_refresh_ticks
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.total_ready_latency_ticks",
        "Tick",
        dram_port0_total_ready_latency_ticks,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.dram.target0.port0.total_ready_latency_ticks"
        ),
        dram_port0_total_ready_latency_ticks
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.target0.port0.max_ready_latency_ticks",
        "Tick",
        dram_port0_max_ready_latency_ticks,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.dram.target0.port0.max_ready_latency_ticks"
        ),
        dram_port0_max_ready_latency_ticks
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.read_row_hits",
        "Count",
        dram_read_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.write_row_hits",
        "Count",
        dram_write_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.read_bytes",
        "Byte",
        dram_read_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.write_bytes",
        "Byte",
        dram_write_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.read_ready_latency_ticks",
        "Tick",
        dram_read_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.bank0.reads",
        "Count",
        dram_bank0_reads,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.bank0.reads"
        ),
        dram_bank0_reads
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.bank0.writes",
        "Count",
        dram_bank0_writes,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.bank0.writes"
        ),
        dram_bank0_writes
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.read_bytes",
        "Byte",
        dram_target0_read_bytes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.resources.dram.target0.read_bytes"),
        dram_target0_read_bytes
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.write_bytes",
        "Byte",
        dram_target0_write_bytes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.resources.dram.target0.write_bytes"),
        dram_target0_write_bytes
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.active_banks",
        "Count",
        dram_port0_active_banks,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.active_banks"
        ),
        dram_port0_active_banks
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.read_bytes",
        "Byte",
        dram_port0_read_bytes,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.read_bytes"
        ),
        dram_port0_read_bytes
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.write_bytes",
        "Byte",
        dram_port0_write_bytes,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.write_bytes"
        ),
        dram_port0_write_bytes
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.row_hits",
        "Count",
        dram_port0_row_hits,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.resources.dram.target0.port0.row_hits"),
        dram_port0_row_hits
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.read_row_hits",
        "Count",
        dram_port0_read_row_hits,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.read_row_hits"
        ),
        dram_port0_read_row_hits
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.write_row_hits",
        "Count",
        dram_port0_write_row_hits,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.write_row_hits"
        ),
        dram_port0_write_row_hits
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.row_misses",
        "Count",
        dram_port0_row_misses,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.row_misses"
        ),
        dram_port0_row_misses
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.refreshes",
        "Count",
        dram_port0_refreshes,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.resources.dram.target0.port0.refreshes"),
        dram_port0_refreshes
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.refresh_ticks",
        "Tick",
        dram_port0_refresh_ticks,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.refresh_ticks"
        ),
        dram_port0_refresh_ticks
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.total_ready_latency_ticks",
        "Tick",
        dram_port0_total_ready_latency_ticks,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.total_ready_latency_ticks"
        ),
        dram_port0_total_ready_latency_ticks
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.target0.port0.max_ready_latency_ticks",
        "Tick",
        dram_port0_max_ready_latency_ticks,
        "monotonic",
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.memory.resources.dram.target0.port0.max_ready_latency_ticks"
        ),
        dram_port0_max_ready_latency_ticks
    );
}

#[test]
fn rem6_run_ddr_profile_refreshes_during_riscv_dram_execution() {
    let mut words = vec![i_type(0, 0, 0x0, 5, 0x13); 64];
    words.push(0x0000_0073); // ecall
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-memory-ddr-refresh", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "2000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "ddr",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"technology\":\"ddr\""));
    let refreshes = json_u64(&json, "/dram/refreshes");
    let refresh_ticks = json_u64(&json, "/dram/refresh_ticks");
    assert!(refreshes > 0);
    assert!(refresh_ticks > 0);
    assert_eq!(
        json_u64(&json, "/memory_resources/dram/refreshes"),
        refreshes
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/dram/refresh_ticks"),
        refresh_ticks
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.dram.refreshes",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.dram.refresh_ticks",
        "Tick",
        0,
        "monotonic",
    );
    assert!(stdout.contains("\"refresh_interval\":32"));
    assert!(stdout.contains("\"refresh_recovery\":5"));
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.refresh_interval",
        "Tick",
        32,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.refresh_recovery",
        "Tick",
        5,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.refreshes",
        "Count",
        refreshes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.refresh_ticks",
        "Tick",
        refresh_ticks,
        "monotonic",
    );
}

#[test]
fn rem6_run_can_select_external_memory_profile_for_dram_backed_execution() {
    struct Case {
        profile: &'static str,
        parallel_port_label: &'static str,
        topology_unit_label: &'static str,
        parallel_ports: u64,
        topology_units: u64,
        scheduler_banks: u64,
        topology_banks: u64,
        bank_group_count: u64,
        scheduler_bank_groups: u64,
        same_bank_group_burst_spacing: u64,
    }

    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-memory-profile", &elf);

    for case in [
        Case {
            profile: "hbm",
            parallel_port_label: "pseudo_channel",
            topology_unit_label: "pseudo_channel",
            parallel_ports: 4,
            topology_units: 4,
            scheduler_banks: 16,
            topology_banks: 16,
            bank_group_count: 2,
            scheduler_bank_groups: 8,
            same_bank_group_burst_spacing: 6,
        },
        Case {
            profile: "lpddr",
            parallel_port_label: "channel",
            topology_unit_label: "die",
            parallel_ports: 2,
            topology_units: 4,
            scheduler_banks: 8,
            topology_banks: 16,
            bank_group_count: 0,
            scheduler_bank_groups: 0,
            same_bank_group_burst_spacing: 0,
        },
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run",
                "--isa",
                "riscv",
                "--binary",
                path.to_str().unwrap(),
                "--max-tick",
                "80",
                "--stats-format",
                "json",
                "--execute",
                "--cores",
                "1",
                "--dram-memory",
                "--dram-memory-profile",
                case.profile,
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr for {}: {}",
            case.profile,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("\"status\":\"executed_until_trap\""));
        assert!(stdout.contains("\"x5\":\"0x7\""));
        assert!(stdout.contains(&format!("\"technology\":\"{}\"", case.profile)));
        assert!(stdout.contains(&format!(
            "\"parallel_port_label\":\"{}\"",
            case.parallel_port_label
        )));
        assert!(stdout.contains(&format!(
            "\"topology_unit_label\":\"{}\"",
            case.topology_unit_label
        )));
        assert!(stdout.contains(&format!("\"parallel_ports\":{}", case.parallel_ports)));
        assert!(stdout.contains(&format!("\"topology_units\":{}", case.topology_units)));
        assert!(stdout.contains(&format!("\"scheduler_banks\":{}", case.scheduler_banks)));
        assert!(stdout.contains(&format!("\"topology_banks\":{}", case.topology_banks)));
        assert!(stdout.contains(&format!("\"bank_group_count\":{}", case.bank_group_count)));
        assert!(stdout.contains(&format!(
            "\"same_bank_group_burst_spacing\":{}",
            case.same_bank_group_burst_spacing
        )));
        assert!(stdout.contains(&format!(
            "\"scheduler_bank_groups\":{}",
            case.scheduler_bank_groups
        )));
        assert_stat(
            &stdout,
            &format!("sim.memory.dram.profile.technology.{}", case.profile),
            "Count",
            1,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.parallel_ports",
            "Count",
            case.parallel_ports,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.topology_units",
            "Count",
            case.topology_units,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.scheduler_banks",
            "Count",
            case.scheduler_banks,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.topology_banks",
            "Count",
            case.topology_banks,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.geometry.bank_group_count",
            "Count",
            case.bank_group_count,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.timing.same_bank_group_burst_spacing",
            "Tick",
            case.same_bank_group_burst_spacing,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.scheduler_bank_groups",
            "Count",
            case.scheduler_bank_groups,
            "constant",
        );
    }
}

#[test]
fn rem6_run_lpddr_fetches_record_dram_low_power_residency() {
    let program = riscv64_program(&[
        b_type(0, 0, 0, 0x0), // beq x0, x0, self
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("lpddr-low-power", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "360",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "lpddr",
            "--memory-route-delay",
            "120",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_tick_limit\""));
    assert!(stdout.contains("\"technology\":\"lpddr\""));
    assert!(stdout.contains("\"committed_instructions\":1"));
    let precharge_powerdown_entries =
        json_u64(&json, "/dram/low_power/precharge_powerdown/entries");
    let precharge_powerdown_ticks = json_u64(&json, "/dram/low_power/precharge_powerdown/ticks");
    assert!(precharge_powerdown_entries > 0);
    assert!(precharge_powerdown_ticks > 0);
    for (json_suffix, stat_suffix, unit) in [
        (
            "active_powerdown/entries",
            "active_powerdown.entries",
            "Count",
        ),
        ("active_powerdown/ticks", "active_powerdown.ticks", "Tick"),
        (
            "precharge_powerdown/entries",
            "precharge_powerdown.entries",
            "Count",
        ),
        (
            "precharge_powerdown/ticks",
            "precharge_powerdown.ticks",
            "Tick",
        ),
        ("self_refresh/entries", "self_refresh.entries", "Count"),
        ("self_refresh/ticks", "self_refresh.ticks", "Tick"),
        ("exits", "exits", "Count"),
        ("exit_latency_ticks", "exit_latency_ticks", "Tick"),
    ] {
        let dram_value = json_u64(&json, &format!("/dram/low_power/{json_suffix}"));
        assert_eq!(
            json_u64(
                &json,
                &format!("/memory_resources/dram/low_power/{json_suffix}")
            ),
            dram_value
        );
        assert_stat(
            &stdout,
            &format!("sim.memory.resources.dram.low_power.{stat_suffix}"),
            unit,
            dram_value,
            "monotonic",
        );
        for prefix in [
            "/dram/targets/0",
            "/memory_resources/dram/targets/0",
            "/dram/targets/0/ports/0",
            "/memory_resources/dram/targets/0/ports/0",
            "/dram/targets/0/ports/0/banks/0",
            "/memory_resources/dram/targets/0/ports/0/banks/0",
        ] {
            assert_eq!(
                json_u64(&json, &format!("{prefix}/low_power/{json_suffix}")),
                dram_value
            );
        }
        for stat_prefix in [
            "sim.memory.dram.target0",
            "sim.memory.resources.dram.target0",
            "sim.memory.dram.target0.port0",
            "sim.memory.resources.dram.target0.port0",
            "sim.memory.dram.target0.port0.bank0",
            "sim.memory.resources.dram.target0.port0.bank0",
        ] {
            assert_stat(
                &stdout,
                &format!("{stat_prefix}.low_power.{stat_suffix}"),
                unit,
                dram_value,
                "monotonic",
            );
        }
    }
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fetch.requests",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(&stdout, "sim.memory.dram.accesses", "Count", 0, "monotonic");
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.technology.lpddr",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.precharge_powerdown_entry_delay",
        "Tick",
        20,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.self_refresh_entry_delay",
        "Tick",
        80,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.exit_latency",
        "Tick",
        7,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.self_refresh_exit_latency",
        "Tick",
        17,
        "constant",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.dram.low_power.precharge_powerdown.entries",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.dram.low_power.precharge_powerdown.ticks",
        "Tick",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_lpddr_accepts_custom_low_power_timing() {
    let program = riscv64_program(&[
        b_type(0, 0, 0, 0x0), // beq x0, x0, self
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("lpddr-custom-low-power", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "lpddr",
            "--dram-low-power-precharge-powerdown-entry-delay",
            "8",
            "--dram-low-power-self-refresh-entry-delay",
            "24",
            "--dram-low-power-exit-latency",
            "5",
            "--dram-low-power-self-refresh-exit-latency",
            "11",
            "--memory-route-delay",
            "72",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert!(stdout.contains("\"technology\":\"lpddr\""));
    assert_eq!(
        json_u64(
            &json,
            "/dram/profile/low_power_timing/precharge_powerdown_entry_delay"
        ),
        8
    );
    assert_eq!(
        json_u64(
            &json,
            "/dram/profile/low_power_timing/self_refresh_entry_delay"
        ),
        24
    );
    assert_eq!(
        json_u64(&json, "/dram/profile/low_power_timing/exit_latency"),
        5
    );
    assert_eq!(
        json_u64(
            &json,
            "/dram/profile/low_power_timing/self_refresh_exit_latency"
        ),
        11
    );

    let active_powerdown_entries = json_u64(&json, "/dram/low_power/active_powerdown/entries");
    let self_refresh_entries = json_u64(&json, "/dram/low_power/self_refresh/entries");
    assert!(active_powerdown_entries > 0);
    assert!(self_refresh_entries > 0);
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.precharge_powerdown_entry_delay",
        "Tick",
        8,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.self_refresh_entry_delay",
        "Tick",
        24,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.exit_latency",
        "Tick",
        5,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.self_refresh_exit_latency",
        "Tick",
        11,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.low_power.active_powerdown.entries",
        "Count",
        active_powerdown_entries,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.low_power.self_refresh.entries",
        "Count",
        self_refresh_entries,
        "monotonic",
    );
}

#[test]
fn rem6_run_accepts_custom_dram_refresh_timing() {
    let mut words = vec![i_type(0, 0, 0x0, 5, 0x13); 64];
    words.push(0x0000_0073); // ecall
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-memory-custom-refresh", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "2000",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "ddr",
            "--dram-refresh-interval",
            "17",
            "--dram-refresh-recovery",
            "4",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"technology\":\"ddr\""));
    assert_eq!(json_u64(&json, "/dram/profile/timing/refresh_interval"), 17);
    assert_eq!(json_u64(&json, "/dram/profile/timing/refresh_recovery"), 4);
    assert!(json_u64(&json, "/dram/refreshes") > 0);
    assert!(json_u64(&json, "/dram/refresh_ticks") > 0);
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.refresh_interval",
        "Tick",
        17,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.refresh_recovery",
        "Tick",
        4,
        "constant",
    );
}

#[test]
fn rem6_run_accepts_toml_jedec_profile_with_custom_dram_refresh_timing() {
    let mut words = vec![i_type(0, 0, 0x0, 5, 0x13); 64];
    words.push(0x0000_0073); // ecall
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("toml-jedec-custom-refresh-bin", &elf);
    let config = temp_config(
        "toml-jedec-custom-refresh",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 2000\nstats_format = \"json\"\nexecute = true\ncores = 1\ndram_memory = true\ndram_memory_profile = \"ddr4-2400-8gb\"\ndram_refresh_interval = 19\ndram_refresh_recovery = 6\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"technology\":\"ddr\""));
    assert_eq!(json_u64(&json, "/dram/profile/timing/refresh_interval"), 19);
    assert_eq!(json_u64(&json, "/dram/profile/timing/refresh_recovery"), 6);
    assert!(json_u64(&json, "/dram/refreshes") > 0);
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.refresh_interval",
        "Tick",
        19,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.refresh_recovery",
        "Tick",
        6,
        "constant",
    );
}

#[test]
fn rem6_run_accepts_jedec_dram_profile_presets() {
    struct Case {
        cli_profile: &'static str,
        technology: &'static str,
        refresh_interval: u64,
        refresh_recovery: u64,
    }

    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-memory-jedec-profile", &elf);

    for case in [
        Case {
            cli_profile: "ddr4-2400-8gb",
            technology: "ddr",
            refresh_interval: 9_360,
            refresh_recovery: 420,
        },
        Case {
            cli_profile: "ddr5-4800-16gb",
            technology: "ddr",
            refresh_interval: 9_360,
            refresh_recovery: 708,
        },
        Case {
            cli_profile: "hbm2-2000-2gb",
            technology: "hbm",
            refresh_interval: 3_900,
            refresh_recovery: 220,
        },
        Case {
            cli_profile: "lpddr4-3200-16gb",
            technology: "lpddr",
            refresh_interval: 6_247,
            refresh_recovery: 448,
        },
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run",
                "--isa",
                "riscv",
                "--binary",
                path.to_str().unwrap(),
                "--max-tick",
                "80",
                "--stats-format",
                "json",
                "--execute",
                "--cores",
                "1",
                "--dram-memory",
                "--dram-memory-profile",
                case.cli_profile,
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr for {}: {}",
            case.cli_profile,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("\"status\":\"executed_until_trap\""));
        assert!(stdout.contains(&format!("\"technology\":\"{}\"", case.technology)));
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.timing.refresh_interval",
            "Tick",
            case.refresh_interval,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.timing.refresh_recovery",
            "Tick",
            case.refresh_recovery,
            "constant",
        );
    }
}

#[test]
fn rem6_run_accepts_host_event_delay_runtime_option() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("host-event-delay", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "1",
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
            "5",
            "--host-event-delay",
            "7",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"host_event_delay\":7"));
    assert!(stdout.contains("\"memory_route_delay\":5"));
    assert!(stdout.contains("\"min_remote_delay\":2"));
    assert!(stdout.contains("\"executed_ticks\":27"));
    assert!(stdout.contains("\"final_tick\":27"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert_stat(&stdout, "sim.host.event_delay", "Tick", 7, "constant");
    assert_stat(&stdout, "sim.final_tick", "Tick", 27, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 2, 20, 10);
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        2,
        20,
        10,
    );
}

#[test]
fn rem6_run_cache_dram_path_emits_unified_resource_activity_stats() {
    const DATA_OFFSET: usize = 32;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("memory-resource-activity", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let resources = json
        .pointer("/memory_resources")
        .expect("run JSON should include memory resources");
    let activity = json_u64(resources, "/activity");
    let active = json_u64(resources, "/active");
    let cache_activity = json_u64(resources, "/cache/activity");
    let active_caches = json_u64(resources, "/cache/active");
    let cache_cpu_responses = json_u64(resources, "/cache/cpu_responses");
    let cache_directory_decisions = json_u64(resources, "/cache/directory_decisions");
    let cache_dram_accesses = json_u64(resources, "/cache/dram_accesses");
    let cache_bank_accepted = json_u64(resources, "/cache/bank_accepted");
    let cache_bank_immediate_hits = json_u64(resources, "/cache/bank_immediate_hits");
    let cache_bank_scheduled_misses = json_u64(resources, "/cache/bank_scheduled_misses");
    let cache_bank_coalesced_misses = json_u64(resources, "/cache/bank_coalesced_misses");
    let transport_activity = json_u64(resources, "/transport/activity");
    let active_transports = json_u64(resources, "/transport/active");
    let dram_activity = json_u64(resources, "/dram/activity");
    let active_dram = json_u64(resources, "/dram/active");
    let dram_active_targets = json_u64(resources, "/dram/active_targets");
    let dram_active_ports = json_u64(resources, "/dram/active_ports");
    let dram_active_banks = json_u64(resources, "/dram/active_banks");
    let dram_accesses = json_u64(resources, "/dram/accesses");
    let dram_reads = json_u64(resources, "/dram/reads");
    let dram_writes = json_u64(resources, "/dram/writes");
    let dram_row_hits = json_u64(resources, "/dram/row_hits");
    let dram_row_misses = json_u64(resources, "/dram/row_misses");
    let dram_commands = json_u64(resources, "/dram/commands");
    let dram_turnarounds = json_u64(resources, "/dram/turnarounds");
    let dram_total_ready_latency_ticks = json_u64(resources, "/dram/total_ready_latency_ticks");
    let dram_max_ready_latency_ticks = json_u64(resources, "/dram/max_ready_latency_ticks");

    assert_eq!(
        activity,
        cache_activity + transport_activity + dram_activity
    );
    assert_eq!(active, active_caches + active_transports + active_dram);
    assert!(
        (1..=2).contains(&active_caches),
        "active caches are bounded by instruction/data cache runtimes"
    );
    assert_eq!(active_transports, 2);
    let active_dram_scope = json_u64(&json, "/dram/active_targets")
        .max(json_u64(&json, "/dram/active_ports"))
        .max(json_u64(&json, "/dram/active_banks"));
    assert_eq!(
        active_dram, active_dram_scope,
        "DRAM active resources use the deepest active runtime scope, not summed hierarchy levels"
    );
    assert!(
        active <= 2 + 2 + active_dram_scope,
        "active resources are bounded by this run shape"
    );
    let low_power_entries = json_u64(&json, "/dram/low_power/active_powerdown/entries")
        + json_u64(&json, "/dram/low_power/precharge_powerdown/entries")
        + json_u64(&json, "/dram/low_power/self_refresh/entries");
    let dram_operation_activity = json_u64(&json, "/dram/accesses")
        .max(json_u64(&json, "/dram/reads") + json_u64(&json, "/dram/writes"))
        .max(json_u64(&json, "/dram/row_hits") + json_u64(&json, "/dram/row_misses"))
        .max(json_u64(&json, "/dram/commands"))
        .max(json_u64(&json, "/dram/refreshes"))
        .max(json_u64(&json, "/dram/turnarounds"))
        .max(low_power_entries)
        .max(json_u64(&json, "/dram/low_power/exits"));
    assert_eq!(dram_activity, dram_operation_activity);
    assert_eq!(dram_active_targets, json_u64(&json, "/dram/active_targets"));
    assert_eq!(dram_active_ports, json_u64(&json, "/dram/active_ports"));
    assert_eq!(dram_active_banks, json_u64(&json, "/dram/active_banks"));
    assert_eq!(dram_accesses, json_u64(&json, "/dram/accesses"));
    assert_eq!(dram_reads, json_u64(&json, "/dram/reads"));
    assert_eq!(dram_writes, json_u64(&json, "/dram/writes"));
    assert_eq!(dram_row_hits, json_u64(&json, "/dram/row_hits"));
    assert_eq!(dram_row_misses, json_u64(&json, "/dram/row_misses"));
    assert_eq!(dram_commands, json_u64(&json, "/dram/commands"));
    assert_eq!(dram_turnarounds, json_u64(&json, "/dram/turnarounds"));
    assert_eq!(
        dram_total_ready_latency_ticks,
        json_u64(&json, "/dram/total_ready_latency_ticks")
    );
    assert_eq!(
        dram_max_ready_latency_ticks,
        json_u64(&json, "/dram/max_ready_latency_ticks")
    );
    assert!(cache_activity > 0);
    assert!(cache_cpu_responses > 0);
    assert!(cache_directory_decisions > 0);
    assert!(cache_dram_accesses > 0);
    assert!(cache_bank_accepted > 0);
    assert!(cache_bank_scheduled_misses > 0);
    assert_eq!(
        cache_cpu_responses,
        json_u64(&json, "/simulation/instruction_cache_cpu_responses")
            + json_u64(&json, "/simulation/data_cache_cpu_responses")
    );
    assert_eq!(
        cache_directory_decisions,
        json_u64(&json, "/simulation/instruction_cache_directory_decisions")
            + json_u64(&json, "/simulation/data_cache_directory_decisions")
    );
    assert_eq!(
        cache_dram_accesses,
        json_u64(&json, "/simulation/instruction_cache_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_dram_accesses")
    );
    assert_eq!(
        cache_bank_accepted,
        json_u64(&json, "/simulation/instruction_cache_bank_accepted")
            + json_u64(&json, "/simulation/data_cache_bank_accepted")
    );
    assert_eq!(
        cache_bank_immediate_hits,
        json_u64(&json, "/simulation/instruction_cache_bank_immediate_hits")
            + json_u64(&json, "/simulation/data_cache_bank_immediate_hits")
    );
    assert_eq!(
        cache_bank_scheduled_misses,
        json_u64(&json, "/simulation/instruction_cache_bank_scheduled_misses")
            + json_u64(&json, "/simulation/data_cache_bank_scheduled_misses")
    );
    assert_eq!(
        cache_bank_coalesced_misses,
        json_u64(&json, "/simulation/instruction_cache_bank_coalesced_misses")
            + json_u64(&json, "/simulation/data_cache_bank_coalesced_misses")
    );
    assert!(transport_activity > 0);
    assert!(dram_activity > 0);

    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.activity", activity);
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.active", active);
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.cache.activity",
        cache_activity,
    );
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.cache.active", active_caches);
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.cache.cpu_responses",
        cache_cpu_responses,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.cache.directory_decisions",
        cache_directory_decisions,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.cache.dram_accesses",
        cache_dram_accesses,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.cache.bank.accepted",
        cache_bank_accepted,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.cache.bank.immediate_hits",
        cache_bank_immediate_hits,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.cache.bank.scheduled_misses",
        cache_bank_scheduled_misses,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.cache.bank.coalesced_misses",
        cache_bank_coalesced_misses,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.transport.activity",
        transport_activity,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.transport.active",
        active_transports,
    );
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.dram.activity", dram_activity);
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.dram.active", active_dram);
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.dram.active_targets",
        dram_active_targets,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.dram.active_ports",
        dram_active_ports,
    );
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.dram.active_banks",
        dram_active_banks,
    );
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.dram.accesses", dram_accesses);
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.dram.reads", dram_reads);
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.dram.writes", dram_writes);
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.dram.row_hits", dram_row_hits);
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.dram.row_misses",
        dram_row_misses,
    );
    assert_resource_stat_matches_json(&stdout, "sim.memory.resources.dram.commands", dram_commands);
    assert_resource_stat_matches_json(
        &stdout,
        "sim.memory.resources.dram.turnarounds",
        dram_turnarounds,
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.total_ready_latency_ticks",
        "Tick",
        dram_total_ready_latency_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.dram.max_ready_latency_ticks",
        "Tick",
        dram_max_ready_latency_ticks,
        "monotonic",
    );
}

#[test]
fn rem6_run_accepts_start_address_runtime_option() {
    let program = riscv64_program(&[
        i_type(3, 0, 0x0, 6, 0x13), // addi x6, x0, 3
        i_type(7, 6, 0x0, 5, 0x13), // addi x5, x6, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("start-address", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "1",
            "--start-address",
            "0x80000004",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"entry\":\"0x80000000\""));
    assert!(stdout.contains("\"start_address\":\"0x80000004\""));
    assert!(stdout.contains("\"executed_ticks\":5"));
    assert!(stdout.contains("\"final_tick\":5"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert_stat(
        &stdout,
        "sim.start_address",
        "Address",
        0x8000_0004,
        "constant",
    );
    assert_stat(&stdout, "sim.final_tick", "Tick", 5, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 2, 4, 2);
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        2,
        4,
        2,
    );
}

#[test]
fn rem6_run_accepts_riscv_boot_register_runtime_options() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-boot-registers", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--cores",
            "1",
            "--riscv-boot-a0",
            "0x123",
            "--riscv-boot-a1",
            "0X80001000",
        ])
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
        "\"riscv_boot\":{\"a0\":\"0x123\",\"a1\":\"0x80001000\",\"sbi\":false,\"se\":false}"
    ));
    assert!(stdout.contains("\"executed_ticks\":3"));
    assert!(stdout.contains("\"final_tick\":3"));
    assert!(stdout.contains("\"x10\":\"0x123\""));
    assert!(stdout.contains("\"x11\":\"0x80001000\""));
    assert_stat(&stdout, "sim.riscv.boot.a0", "Value", 0x123, "constant");
    assert_stat(
        &stdout,
        "sim.riscv.boot.a1",
        "Value",
        0x8000_1000,
        "constant",
    );
}

#[test]
fn rem6_run_riscv_se_loads_startup_stack_and_exits_through_syscall() {
    let program = riscv64_program(&[
        i_type(0, 2, 0x3, 5, 0x03),   // ld x5, 0(sp)
        i_type(8, 2, 0x3, 6, 0x03),   // ld x6, 8(sp)
        i_type(0, 6, 0x0, 7, 0x03),   // lb x7, 0(x6)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        i_type(0, 7, 0x0, 10, 0x13),  // addi a0, x7, 0
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-startup-stack", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":47"));
    assert!(
        stdout.contains("\"riscv_boot\":{\"a0\":\"0x0\",\"a1\":\"0x0\",\"sbi\":false,\"se\":true}")
    );
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains("\"x7\":\"0x2f\""));
    assert!(stdout.contains("\"x10\":\"0x2f\""));
    assert!(stdout.contains("\"data_loads\":3"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 47, "constant");
}

#[test]
fn rem6_run_riscv_se_executes_no_libc_rv64gc_line_end_fetches() {
    let mut program = 0x0001_u16.to_le_bytes().to_vec(); // c.nop
    program.extend(i_type(93, 0, 0x0, 17, 0x13).to_le_bytes()); // addi a7, x0, 93
    program.extend(i_type(0, 0, 0x0, 10, 0x13).to_le_bytes()); // addi a0, x0, 0
    program.extend(0x0000_0073_u32.to_le_bytes()); // ecall

    let entry = 0x8000_003c;
    let elf = riscv64_elf(entry, entry, &program);
    let path = temp_binary("riscv-se-no-libc-rv64gc-line-end", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(
        stdout.contains("\"riscv_boot\":{\"a0\":\"0x0\",\"a1\":\"0x0\",\"sbi\":false,\"se\":true}")
    );
    assert!(stdout.contains("\"committed_instructions\":4"));
    assert!(stdout.contains("\"fetch\":{\"requests\":5"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 0, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_printf_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static newlib RISC-V SE smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-printf");
    let source = workspace.join("hello.c");
    let binary = workspace.join("hello");
    fs::write(
        &source,
        r#"#include <stdio.h>
int main(void) {
    printf("rem6 newlib smoke\n");
    return 37;
}
"#,
    )
    .unwrap();

    let compile = Command::new(&gcc)
        .args([
            "-O1",
            "-static",
            "-march=rv64gc",
            "-mabi=lp64d",
            source.to_str().unwrap(),
            "-o",
            binary.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "gcc stderr: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let qemu_output = Command::new(&qemu).arg(&binary).output().unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(37),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"rem6 newlib smoke\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "200000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":37"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"rem6 newlib smoke\\n\""));
    assert!(stdout.contains("\"data_loads\":"));
    assert!(stdout.contains("\"data_stores\":"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 37, "constant");
}

#[test]
fn rem6_run_riscv_se_cli_arguments_and_environment_reach_startup_stack() {
    let program = riscv_se_argv_env_probe_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-cli-argv-env", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-arg",
            "A0",
            "--riscv-se-arg",
            "B1",
            "--riscv-se-env",
            "C=1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":200"));
    assert!(stdout.contains("\"x5\":\"0x2\""));
    assert!(stdout.contains("\"x7\":\"0x41\""));
    assert!(stdout.contains("\"x9\":\"0x42\""));
    assert!(stdout.contains("\"x12\":\"0x43\""));
}

#[test]
fn rem6_run_riscv_se_toml_arguments_and_environment_reach_startup_stack() {
    let program = riscv_se_argv_env_probe_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-toml-argv-env", &elf);
    let config = temp_config(
        "riscv-se-toml-argv-env",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 180\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_args = [\"A0\", \"B1\"]\nriscv_se_env = [\"C=1\"]\n",
            path.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":200"));
    assert!(stdout.contains("\"x5\":\"0x2\""));
    assert!(stdout.contains("\"x7\":\"0x41\""));
    assert!(stdout.contains("\"x9\":\"0x42\""));
    assert!(stdout.contains("\"x12\":\"0x43\""));
}

#[test]
fn rem6_run_riscv_se_provides_zeroed_stack_backing_below_startup_frame() {
    let program = riscv64_program(&[
        i_type(-16, 2, 0x0, 2, 0x13), // addi sp, sp, -16
        i_type(42, 0, 0x0, 5, 0x13),  // addi x5, x0, 42
        s_type(0, 5, 2, 0x3),         // sd x5, 0(sp)
        i_type(0, 2, 0x3, 10, 0x03),  // ld a0, 0(sp)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-stack-backing", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":42"));
    assert!(stdout.contains("\"x10\":\"0x2a\""));
}

#[test]
fn rem6_run_riscv_se_futex_wait_reads_guest_word_mismatch() {
    let program = riscv64_program(&[
        i_type(-16, 2, 0x0, 2, 0x13),    // addi sp, sp, -16
        i_type(1, 0, 0x0, 5, 0x13),      // addi x5, x0, 1
        s_type(0, 5, 2, 0x2),            // sw x5, 0(sp)
        i_type(0, 2, 0x0, 10, 0x13),     // addi a0, sp, 0
        i_type(0, 0, 0x0, 11, 0x13),     // addi a1, x0, 0
        i_type(2, 0, 0x0, 12, 0x13),     // addi a2, x0, 2
        i_type(0, 0, 0x0, 13, 0x13),     // addi a3, x0, 0
        i_type(98, 0, 0x0, 17, 0x13),    // addi a7, x0, 98
        0x0000_0073,                     // ecall
        i_type(0xff, 10, 0x7, 10, 0x13), // andi a0, a0, 255
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, 93
        0x0000_0073,                     // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-futex-wait-eagain", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":245"));
    assert!(stdout.contains("\"x10\":\"0xf5\""));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 245, "constant");
}

#[test]
fn rem6_run_riscv_se_futex_wait_bitset_reads_guest_word_mismatch() {
    let program = riscv64_program(&[
        i_type(-16, 2, 0x0, 2, 0x13),    // addi sp, sp, -16
        i_type(1, 0, 0x0, 5, 0x13),      // addi x5, x0, 1
        s_type(0, 5, 2, 0x2),            // sw x5, 0(sp)
        i_type(0, 2, 0x0, 10, 0x13),     // addi a0, sp, 0
        i_type(9, 0, 0x0, 11, 0x13),     // addi a1, x0, 9
        i_type(2, 0, 0x0, 12, 0x13),     // addi a2, x0, 2
        i_type(0, 0, 0x0, 13, 0x13),     // addi a3, x0, 0
        i_type(0, 0, 0x0, 14, 0x13),     // addi a4, x0, 0
        i_type(-1, 0, 0x0, 15, 0x13),    // addi a5, x0, -1
        i_type(98, 0, 0x0, 17, 0x13),    // addi a7, x0, 98
        0x0000_0073,                     // ecall
        i_type(0xff, 10, 0x7, 10, 0x13), // andi a0, a0, 255
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, 93
        0x0000_0073,                     // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-futex-wait-bitset-eagain", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":245"));
    assert!(stdout.contains("\"x10\":\"0xf5\""));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 245, "constant");
}

#[test]
fn rem6_run_riscv_se_futex_wait_zero_timeout_returns_etimedout() {
    let program = riscv64_program(&[
        i_type(-32, 2, 0x0, 2, 0x13),    // addi sp, sp, -32
        i_type(1, 0, 0x0, 5, 0x13),      // addi x5, x0, 1
        s_type(0, 5, 2, 0x2),            // sw x5, 0(sp)
        s_type(8, 0, 2, 0x3),            // sd x0, 8(sp)
        s_type(16, 0, 2, 0x3),           // sd x0, 16(sp)
        i_type(0, 2, 0x0, 10, 0x13),     // addi a0, sp, 0
        i_type(0, 0, 0x0, 11, 0x13),     // addi a1, x0, 0
        i_type(1, 0, 0x0, 12, 0x13),     // addi a2, x0, 1
        i_type(8, 2, 0x0, 13, 0x13),     // addi a3, sp, 8
        i_type(98, 0, 0x0, 17, 0x13),    // addi a7, x0, 98
        0x0000_0073,                     // ecall
        i_type(0xff, 10, 0x7, 10, 0x13), // andi a0, a0, 255
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, 93
        0x0000_0073,                     // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-futex-wait-zero-timeout", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":146"));
    assert!(stdout.contains("\"x10\":\"0x92\""));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 146, "constant");
}

fn riscv_se_argv_env_probe_program() -> Vec<u8> {
    riscv64_program(&[
        i_type(0, 2, 0x3, 5, 0x03),   // ld x5, 0(sp)
        i_type(8, 2, 0x3, 6, 0x03),   // ld x6, 8(sp)
        i_type(0, 6, 0x0, 7, 0x03),   // lb x7, 0(x6)
        i_type(16, 2, 0x3, 8, 0x03),  // ld x8, 16(sp)
        i_type(0, 8, 0x0, 9, 0x03),   // lb x9, 0(x8)
        i_type(32, 2, 0x3, 11, 0x03), // ld x11, 32(sp)
        i_type(0, 11, 0x0, 12, 0x03), // lb x12, 0(x11)
        0x0072_8533,                  // add a0, x5, x7
        0x0095_0533,                  // add a0, a0, x9
        0x00c5_0533,                  // add a0, a0, x12
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ])
}

#[test]
fn rem6_run_riscv_se_reports_loaded_program_headers_in_auxv() {
    let program = riscv64_program(&[
        i_type(40, 2, 0x3, 10, 0x03),  // ld a0, 40(sp)
        i_type(28, 10, 0x5, 10, 0x13), // srli a0, a0, 28
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, 93
        0x0000_0073,                   // ecall
    ]);
    let mut elf = riscv64_elf(0x8000_0080, 0x8000_0000, &program);
    let loaded_bytes = elf.len() as u64;
    write_u64_le(&mut elf, 72, 0);
    write_u64_le(&mut elf, 80, 0x9000_0000);
    write_u64_le(&mut elf, 96, loaded_bytes);
    write_u64_le(&mut elf, 104, loaded_bytes);
    let path = temp_binary("riscv-se-auxv-phdr", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":8"));
    assert!(stdout.contains("\"x10\":\"0x8\""));
}

#[test]
fn rem6_run_riscv_se_mmap_installs_zeroed_backing() {
    let program = riscv64_program(&[
        i_type(222, 0, 0x0, 17, 0x13), // addi a7, x0, 222
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        i_type(64, 0, 0x0, 11, 0x13),  // addi a1, x0, 64
        i_type(3, 0, 0x0, 12, 0x13),   // addi a2, x0, 3
        i_type(34, 0, 0x0, 13, 0x13),  // addi a3, x0, 34
        i_type(-1, 0, 0x0, 14, 0x13),  // addi a4, x0, -1
        i_type(0, 0, 0x0, 15, 0x13),   // addi a5, x0, 0
        0x0000_0073,                   // ecall
        i_type(0, 10, 0x0, 6, 0x03),   // lb x6, 0(a0)
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, 93
        i_type(0, 6, 0x0, 10, 0x13),   // addi a0, x6, 0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-mmap-zero", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--dump-memory",
            "0x4000000000000000:1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"data_loads\":1"));
    assert!(stdout.contains("{\"address\":\"0x4000000000000000\",\"bytes\":1,\"hex\":\"00\"}"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 0, "constant");
}

#[test]
fn rem6_run_riscv_se_mmap_hint_overlap_preserves_text() {
    let program = riscv64_program(&[
        u_type(0x10000, 5, 0x37),      // lui x5, 0x10
        i_type(222, 0, 0x0, 17, 0x13), // addi a7, x0, 222
        i_type(0, 5, 0x0, 10, 0x13),   // addi a0, x5, 0
        i_type(64, 0, 0x0, 11, 0x13),  // addi a1, x0, 64
        i_type(3, 0, 0x0, 12, 0x13),   // addi a2, x0, 3
        i_type(34, 0, 0x0, 13, 0x13),  // addi a3, x0, 34
        i_type(-1, 0, 0x0, 14, 0x13),  // addi a4, x0, -1
        i_type(0, 0, 0x0, 15, 0x13),   // addi a5, x0, 0
        0x0000_0073,                   // ecall
        i_type(93, 0, 0x0, 17, 0x13),  // addi a7, x0, 93
        i_type(0, 0, 0x0, 10, 0x13),   // addi a0, x0, 0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x10000, 0x10000, &program);
    let path = temp_binary("riscv-se-mmap-hint-overlap", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--dump-memory",
            "0x10000:1",
            "--dump-memory",
            "0x4000000000000000:1",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("{\"address\":\"0x10000\",\"bytes\":1,\"hex\":\"b7\"}"));
    assert!(stdout.contains("{\"address\":\"0x4000000000000000\",\"bytes\":1,\"hex\":\"00\"}"));
}

#[test]
fn rem6_run_riscv_se_handles_memory_backed_write_syscall() {
    let program = riscv64_program(&[
        i_type(8, 2, 0x3, 6, 0x03),   // ld x6, 8(sp)
        i_type(1, 0, 0x0, 10, 0x13),  // addi a0, x0, 1
        i_type(0, 6, 0x0, 11, 0x13),  // addi a1, x6, 0
        i_type(24, 0, 0x0, 12, 0x13), // addi a2, x0, 24
        i_type(64, 0, 0x0, 17, 0x13), // addi a7, x0, 64
        0x0000_0073,                  // ecall
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-write-syscall", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":24"));
    assert!(stdout.contains("\"x10\":\"0x18\""));
    assert!(stdout.contains("\"x17\":\"0x5d\""));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 24, "constant");
}

fn write_u64_le(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn rem6_run_riscv_se_write_syscall_faults_unmapped_buffer() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 10, 0x13),     // addi a0, x0, 1
        i_type(0, 0, 0x0, 11, 0x13),     // addi a1, x0, 0
        i_type(1, 0, 0x0, 12, 0x13),     // addi a2, x0, 1
        i_type(64, 0, 0x0, 17, 0x13),    // addi a7, x0, 64
        0x0000_0073,                     // ecall
        i_type(0xff, 10, 0x7, 10, 0x13), // andi a0, a0, 255
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, 93
        0x0000_0073,                     // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-write-efault", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":242"));
    assert!(stdout.contains("\"x10\":\"0xf2\""));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.host_stop", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 242, "constant");
}

#[test]
fn rem6_run_riscv_se_reports_unknown_syscalls() {
    let program = riscv64_program(&[
        u_type(0x2000, 17, 0x37),        // lui a7, 0x2
        i_type(1807, 17, 0x0, 17, 0x13), // addi a7, a7, 1807
        0x0000_0073,                     // ecall
        i_type(38, 10, 0x0, 5, 0x13),    // addi x5, a0, 38
        i_type(93, 0, 0x0, 17, 0x13),    // addi a7, x0, 93
        i_type(0, 5, 0x0, 10, 0x13),     // addi a0, x5, 0
        0x0000_0073,                     // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-se-unknown-syscall", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[{\"pc\":\"0x80000008\""));
    assert!(stdout.contains("\"number\":9999"));
    assert!(stdout.contains("\"arguments\":[\"0x0\",\"0x0\",\"0x0\",\"0x0\",\"0x0\",\"0x0\"]"));
    assert_stat(
        &stdout,
        "sim.riscv.unknown_syscalls",
        "Count",
        1,
        "monotonic",
    );
}

fn json_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing unsigned JSON value at {pointer}: {json}"))
}

fn sum_dram_bank_field(json: &Value, field: &str) -> u64 {
    json.pointer("/dram/targets")
        .and_then(Value::as_array)
        .expect("DRAM target array")
        .iter()
        .flat_map(|target| {
            target
                .get("ports")
                .and_then(Value::as_array)
                .expect("DRAM target ports")
        })
        .flat_map(|port| {
            port.get("banks")
                .and_then(Value::as_array)
                .expect("DRAM port banks")
        })
        .map(|bank| {
            bank.get(field)
                .and_then(Value::as_u64)
                .unwrap_or_else(|| panic!("DRAM bank missing unsigned field {field}: {bank}"))
        })
        .sum()
}

fn assert_resource_stat_matches_json(stdout: &str, path: &str, value: u64) {
    assert_stat(stdout, path, "Count", value, "monotonic");
    assert_eq!(stat_value(stdout, path), value);
}
