use super::*;

const M5_EXIT: u32 = 0x21;
const M5_CHECKPOINT: u32 = 0x43;

#[test]
fn rem6_run_rejects_host_checkpoint_flags_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let checkpoint_path = temp_binary("host-checkpoint-without-execute", &elf);
    let restore_path = temp_binary("host-checkpoint-restore-without-execute", &elf);

    for (path, flag, message) in [
        (
            checkpoint_path.as_path(),
            "--host-checkpoint",
            "--host-checkpoint requires --execute",
        ),
        (
            restore_path.as_path(),
            "--host-restore-checkpoint",
            "--host-restore-checkpoint requires --execute",
        ),
    ] {
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
                flag,
                "8:cp",
            ])
            .output()
            .unwrap();

        assert!(!output.status.success(), "{flag} should require execution");
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "stderr: {stderr}");
    }
}

#[test]
fn rem6_run_rejects_guest_checkpoint_for_unsupported_cache_state() {
    let program = riscv64_program(&[m5op(M5_CHECKPOINT), m5op(M5_EXIT)]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);

    for (case, cache_args, message) in [(
        "mesi",
        ["--data-cache-protocol", "mesi"],
        "host checkpoint actions require MSI cache protocols",
    )] {
        let binary = temp_binary(&format!("guest-checkpoint-cache-{case}"), &elf);
        let artifact = temp_output(&format!("guest-checkpoint-cache-{case}"));
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
                "--output",
                artifact.to_str().unwrap(),
                cache_args[0],
                cache_args[1],
            ])
            .output()
            .unwrap();

        assert_eq!(output.status.code(), Some(2), "{case} should be rejected");
        assert!(output.stdout.is_empty());
        assert!(!artifact.exists(), "{case} must not write an artifact");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "stderr: {stderr}");
    }
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
}
