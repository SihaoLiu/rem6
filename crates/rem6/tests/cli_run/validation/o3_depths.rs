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
