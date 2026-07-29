use super::*;

const M5_EXIT: u32 = 0x21;
const M5_CHECKPOINT: u32 = 0x43;

#[test]
fn rem6_run_rejects_fabric_qos_checkpoint_and_restore() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);

    for (case, action) in [
        ("checkpoint", ["--host-checkpoint", "8:cp"]),
        ("restore", ["--host-restore-checkpoint", "8:cp"]),
        (
            "scheduled-mode-switch",
            ["--host-switch-cpu-mode", "8:cpu0:timing"],
        ),
        ("guest-mode-switch", ["--m5-switch-cpu-mode", "timing"]),
    ] {
        let path = temp_binary(&format!("fabric-qos-{case}"), &elf);
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
                "cache-fabric-dram",
                "--fabric-qos-queue-policy",
                "least-recently-granted",
                action[0],
                action[1],
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "{case} should be rejected");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("fabric QoS checkpoint restore is not supported"),
            "stderr: {stderr}"
        );
    }
}

#[test]
fn rem6_run_rejects_guest_checkpoint_with_fabric_qos() {
    let program = riscv64_program(&[m5op(M5_CHECKPOINT), m5op(M5_EXIT)]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("fabric-qos-guest-checkpoint", &elf);
    let artifact = temp_output("fabric-qos-guest-checkpoint");
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "cache-fabric-dram",
            "--fabric-qos-queue-policy",
            "least-recently-granted",
            "--output",
            artifact.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!artifact.exists());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("fabric QoS checkpoint restore is not supported"),
        "stderr: {stderr}"
    );
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
}
