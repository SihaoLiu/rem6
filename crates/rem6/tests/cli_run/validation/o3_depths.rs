use super::*;

#[test]
fn rem6_run_accepts_max_riscv_o3_scalar_memory_depth() {
    let program = riscv64_program(&[0x0000_0013; 64]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-o3-scalar-memory-depth-max", &elf);

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
            "--riscv-o3-scalar-memory-depth",
            "4",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
fn rem6_run_accepts_riscv_o3_scalar_memory_depth_from_config() {
    let program = riscv64_program(&[0x0000_0013; 64]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-o3-scalar-memory-depth-config", &elf);
    let config = temp_output("riscv-o3-scalar-memory-depth-config.toml");
    std::fs::write(&config, "[run]\nriscv_o3_scalar_memory_depth = 4\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rem6_run_validates_toml_riscv_o3_scalar_memory_depth_requirements() {
    let riscv = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let x86 = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);

    for (name, isa, elf, execute, expected) in [
        (
            "riscv-o3-scalar-memory-depth-config-without-execute",
            "riscv",
            riscv.as_slice(),
            false,
            "--riscv-o3-scalar-memory-depth requires --execute",
        ),
        (
            "riscv-o3-scalar-memory-depth-config-without-riscv",
            "x86",
            x86.as_slice(),
            true,
            "--riscv-o3-scalar-memory-depth requires --isa riscv",
        ),
    ] {
        let path = temp_binary(name, elf);
        let config = temp_output(&format!("{name}.toml"));
        std::fs::write(&config, "[run]\nriscv_o3_scalar_memory_depth = 4\n").unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
        command.args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--isa",
            isa,
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
        ]);
        if execute {
            command.arg("--execute");
        }

        let output = command.output().unwrap();

        assert!(!output.status.success(), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected), "{name}: {stderr}");
    }
}

#[test]
fn rem6_run_rejects_riscv_o3_scalar_memory_depth_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-o3-scalar-memory-depth-without-execute", &elf);

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
            "--riscv-o3-scalar-memory-depth",
            "4",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-o3-scalar-memory-depth requires --execute"));
}

#[test]
fn rem6_run_rejects_riscv_o3_scalar_memory_depth_without_riscv_isa() {
    let elf = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);
    let path = temp_binary("riscv-o3-scalar-memory-depth-without-riscv", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "x86",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-format",
            "json",
            "--riscv-o3-scalar-memory-depth",
            "4",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-o3-scalar-memory-depth requires --isa riscv"));
}

#[test]
fn rem6_run_rejects_invalid_riscv_o3_scalar_memory_depth_values() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-o3-scalar-memory-depth-invalid", &elf);

    for value in ["0", "5"] {
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
                "--riscv-o3-scalar-memory-depth",
                value,
            ])
            .output()
            .unwrap();

        assert!(!output.status.success(), "{value}");
        assert!(output.stdout.is_empty(), "{value}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("invalid RISC-V O3 scalar memory depth"),
            "{value}: {stderr}"
        );
    }
}

#[test]
fn rem6_run_rejects_invalid_riscv_o3_scalar_memory_depth_from_config() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-o3-scalar-memory-depth-invalid-config", &elf);
    let config = temp_output("riscv-o3-scalar-memory-depth-invalid-config.toml");
    std::fs::write(&config, "[run]\nriscv_o3_scalar_memory_depth = 5\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid RISC-V O3 scalar memory depth 5"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_config_scan_treats_riscv_o3_scalar_memory_depth_as_value_taking() {
    let bogus_config = temp_output("riscv-o3-scalar-memory-depth-prescan-bogus-config");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--riscv-o3-scalar-memory-depth",
            "--config",
            bogus_config.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid RISC-V O3 scalar memory depth --config"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains(&format!("failed to read config {}", bogus_config.display())));
}

#[test]
fn rem6_run_accepts_riscv_o3_window_depths_from_cli_and_toml() {
    for scenario in [
        LiveWindowDepthScenario {
            name: "riscv-o3-window-depths-cli-memory4-live8",
            config: "",
            args: &[
                "--riscv-o3-scalar-memory-depth",
                "4",
                "--riscv-o3-scalar-live-window-depth",
                "8",
            ],
        },
        LiveWindowDepthScenario {
            name: "riscv-o3-window-depths-toml-memory4-live8",
            config: "riscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 8\n",
            args: &[],
        },
    ] {
        assert_live_window_depth_run_succeeds(scenario);
    }
}

#[test]
fn rem6_run_cli_riscv_o3_scalar_live_window_depth_overrides_toml() {
    assert_live_window_depth_run_succeeds(LiveWindowDepthScenario {
        name: "riscv-o3-window-depths-cli-live8-overrides-toml-live6",
        config: "riscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 6\n",
        args: &["--riscv-o3-scalar-live-window-depth", "8"],
    });
}

#[test]
fn rem6_run_accepts_live_only_riscv_o3_scalar_live_window_depth_with_implicit_memory() {
    for scenario in [
        LiveWindowDepthScenario {
            name: "riscv-o3-window-depths-cli-live6-implicit-memory2",
            config: "",
            args: &["--riscv-o3-scalar-live-window-depth", "6"],
        },
        LiveWindowDepthScenario {
            name: "riscv-o3-window-depths-toml-live6-implicit-memory2",
            config: "riscv_o3_scalar_live_window_depth = 6\n",
            args: &[],
        },
    ] {
        assert_live_window_depth_run_succeeds(scenario);
    }
}

#[test]
fn rem6_run_validates_final_riscv_o3_window_depth_pair_after_cli_precedence() {
    assert_live_window_depth_run_succeeds(LiveWindowDepthScenario {
        name: "riscv-o3-window-depths-toml-live3-repaired-by-cli-live8",
        config: "riscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 3\n",
        args: &["--riscv-o3-scalar-live-window-depth", "8"],
    });

    assert_live_window_depth_run_fails(
        LiveWindowDepthScenario {
            name: "riscv-o3-window-depths-toml-live8-overridden-by-cli-live3",
            config: "riscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 8\n",
            args: &["--riscv-o3-scalar-live-window-depth", "3"],
        },
        "RISC-V O3 scalar live-window depth 3 is below scalar memory depth 4",
    );
}

#[test]
fn rem6_run_rejects_invalid_cli_riscv_o3_scalar_live_window_depths() {
    for (name, args, expected) in [
        (
            "riscv-o3-window-depths-cli-live0",
            vec!["--riscv-o3-scalar-live-window-depth", "0"],
            "invalid RISC-V O3 scalar live-window depth 0",
        ),
        (
            "riscv-o3-window-depths-cli-live9",
            vec!["--riscv-o3-scalar-live-window-depth", "9"],
            "invalid RISC-V O3 scalar live-window depth 9",
        ),
        (
            "riscv-o3-window-depths-cli-memory4-live3",
            vec![
                "--riscv-o3-scalar-memory-depth",
                "4",
                "--riscv-o3-scalar-live-window-depth",
                "3",
            ],
            "RISC-V O3 scalar live-window depth 3 is below scalar memory depth 4",
        ),
        (
            "riscv-o3-window-depths-cli-implicit-memory2-live1",
            vec!["--riscv-o3-scalar-live-window-depth", "1"],
            "RISC-V O3 scalar live-window depth 1 is below scalar memory depth 2",
        ),
    ] {
        assert_live_window_depth_run_fails(
            LiveWindowDepthScenario {
                name,
                config: "",
                args: &args,
            },
            expected,
        );
    }
}

#[test]
fn rem6_run_rejects_invalid_toml_riscv_o3_scalar_live_window_depths() {
    for (name, config, expected) in [
        (
            "riscv-o3-window-depths-toml-live0",
            "riscv_o3_scalar_live_window_depth = 0\n",
            "invalid RISC-V O3 scalar live-window depth 0",
        ),
        (
            "riscv-o3-window-depths-toml-live9",
            "riscv_o3_scalar_live_window_depth = 9\n",
            "invalid RISC-V O3 scalar live-window depth 9",
        ),
        (
            "riscv-o3-window-depths-toml-memory4-live3",
            "riscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 3\n",
            "RISC-V O3 scalar live-window depth 3 is below scalar memory depth 4",
        ),
    ] {
        assert_live_window_depth_run_fails(
            LiveWindowDepthScenario {
                name,
                config,
                args: &[],
            },
            expected,
        );
    }
}

#[test]
fn rem6_run_rejects_riscv_o3_scalar_live_window_depth_without_execution_and_riscv() {
    let riscv = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let x86 = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);

    for (name, isa, elf, execute, config, args, expected) in [
        (
            "riscv-o3-live-depth-cli-without-execute",
            "riscv",
            riscv.as_slice(),
            false,
            "",
            &["--riscv-o3-scalar-live-window-depth", "6"][..],
            "--riscv-o3-scalar-live-window-depth requires --execute",
        ),
        (
            "riscv-o3-live-depth-toml-without-execute",
            "riscv",
            riscv.as_slice(),
            false,
            "riscv_o3_scalar_live_window_depth = 6\n",
            &[][..],
            "--riscv-o3-scalar-live-window-depth requires --execute",
        ),
        (
            "riscv-o3-live-depth-cli-without-riscv",
            "x86",
            x86.as_slice(),
            true,
            "",
            &["--riscv-o3-scalar-live-window-depth", "6"][..],
            "--riscv-o3-scalar-live-window-depth requires --isa riscv",
        ),
        (
            "riscv-o3-live-depth-toml-without-riscv",
            "x86",
            x86.as_slice(),
            true,
            "riscv_o3_scalar_live_window_depth = 6\n",
            &[][..],
            "--riscv-o3-scalar-live-window-depth requires --isa riscv",
        ),
    ] {
        let path = temp_binary(name, elf);
        let output =
            run_live_window_depth_cli(name, config, isa, path.to_str().unwrap(), execute, args);

        assert!(!output.status.success(), "{name}");
        assert!(output.stdout.is_empty(), "{name}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(expected), "{name}: {stderr}");
    }
}

#[test]
fn rem6_run_config_scan_treats_riscv_o3_scalar_live_window_depth_as_value_taking() {
    let bogus_config = temp_output("riscv-o3-scalar-live-window-depth-prescan-bogus-config");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--riscv-o3-scalar-live-window-depth",
            "--config",
            bogus_config.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid RISC-V O3 scalar live-window depth --config"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains(&format!("failed to read config {}", bogus_config.display())));
}

struct LiveWindowDepthScenario<'a> {
    name: &'a str,
    config: &'a str,
    args: &'a [&'a str],
}

fn assert_live_window_depth_run_succeeds(scenario: LiveWindowDepthScenario<'_>) {
    let program = riscv64_program(&[0x0000_0013; 64]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(scenario.name, &elf);
    let output = run_live_window_depth_cli(
        scenario.name,
        scenario.config,
        "riscv",
        path.to_str().unwrap(),
        true,
        scenario.args,
    );

    assert!(
        output.status.success(),
        "{} stderr: {}",
        scenario.name,
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_live_window_depth_run_fails(scenario: LiveWindowDepthScenario<'_>, expected: &str) {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary(scenario.name, &elf);
    let output = run_live_window_depth_cli(
        scenario.name,
        scenario.config,
        "riscv",
        path.to_str().unwrap(),
        true,
        scenario.args,
    );

    assert!(!output.status.success(), "{}", scenario.name);
    assert!(output.stdout.is_empty(), "{}", scenario.name);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(expected), "{}: {stderr}", scenario.name);
}

fn run_live_window_depth_cli(
    name: &str,
    config_body: &str,
    isa: &str,
    binary: &str,
    execute: bool,
    extra_args: &[&str],
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        isa,
        "--binary",
        binary,
        "--max-tick",
        "40",
        "--stats-format",
        "json",
    ]);
    if !config_body.is_empty() {
        let config = temp_output(&format!("{name}.toml"));
        std::fs::write(&config, format!("[run]\n{config_body}")).unwrap();
        command.args(["--config", config.to_str().unwrap()]);
    }
    if execute {
        command.arg("--execute");
    }
    command.args(extra_args);
    command.output().unwrap()
}
