use std::fs;
use std::process::Command;

use crate::support::*;

#[test]
fn rem6_run_rejects_riscv_se_file_and_output_path_conflict() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let binary = temp_binary("riscv-se-file-output-conflict", &elf);
    let output_path = temp_output("riscv-se-file-output-conflict");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", output_path.display()),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert!(!output_path.exists());
}

#[test]
fn rem6_run_rejects_riscv_se_file_and_output_path_alias_conflict() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let binary = temp_binary("riscv-se-file-output-alias-conflict", &elf);
    let output_path = temp_output("riscv-se-file-output-alias-conflict");
    let alias_directory = output_path.parent().unwrap().join("alias-parent");
    fs::create_dir_all(&alias_directory).unwrap();
    let output_alias = alias_directory
        .join("..")
        .join(output_path.file_name().unwrap());

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", output_alias.display()),
            "--output",
            output_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert!(!output_path.exists());
}

#[test]
fn rem6_run_rejects_duplicate_path_backed_riscv_se_files() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let binary = temp_binary("riscv-se-file-duplicate-host-path", &elf);
    let host_file = temp_output("riscv-se-file-duplicate-host-path");
    fs::write(&host_file, b"seed\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--riscv-se",
            "--riscv-se-file",
            &format!("first.txt={}", host_file.display()),
            "--riscv-se-file",
            &format!("second.txt={}", host_file.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("run output artifacts must use different paths"));
    assert_eq!(fs::read(&host_file).unwrap(), b"seed\n");
}

#[test]
fn rem6_run_rejects_duplicate_riscv_se_guest_paths() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let binary = temp_binary("riscv-se-file-duplicate-guest-path", &elf);
    let first = temp_output("riscv-se-file-duplicate-guest-path-first");
    let second = temp_output("riscv-se-file-duplicate-guest-path-second");
    fs::write(&first, b"first\n").unwrap();
    fs::write(&second, b"second\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", first.display()),
            "--riscv-se-file",
            &format!("guest.txt={}", second.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("RISC-V SE file guest paths must be unique"));
    assert_eq!(fs::read(&first).unwrap(), b"first\n");
    assert_eq!(fs::read(&second).unwrap(), b"second\n");
}

#[test]
fn rem6_run_riscv_se_writes_registered_path_backed_file_to_host() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE path writeback smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-path-writeback");
    let source = workspace.join("path_writeback.c");
    let binary = workspace.join("path_writeback");
    let host_file = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>

int main(void) {
    FILE *file = fopen("guest.txt", "w+");
    if (file == NULL) {
        return 71;
    }
    if (fputs("host writeback\n", file) < 0) {
        fclose(file);
        return 72;
    }
    if (fflush(file) != 0) {
        fclose(file);
        return 73;
    }
    fclose(file);
    return 74;
}
"#,
    )
    .unwrap();
    fs::write(&host_file, b"seed\n").unwrap();

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

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "400000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", host_file.display()),
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
    assert!(stdout.contains("\"stop_code\":74"));
    assert_eq!(fs::read(&host_file).unwrap(), b"host writeback\n");
}

#[test]
fn rem6_run_writes_renamed_path_backed_riscv_se_file_to_host() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE renamed-file writeback smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-file-renamed-writeback");
    let source = workspace.join("renamed_writeback.c");
    let binary = workspace.join("renamed_writeback");
    let host_file = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>

int rename(const char *, const char *);

int main(void) {
    if (rename("guest.txt", "renamed.txt") != 0) {
        return 71;
    }
    FILE *file = fopen("renamed.txt", "w");
    if (file == NULL) {
        return 72;
    }
    if (fputs("renamed writeback\n", file) < 0) {
        fclose(file);
        return 73;
    }
    fclose(file);
    return 76;
}
"#,
    )
    .unwrap();
    fs::write(&host_file, b"seed\n").unwrap();

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

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "400000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", host_file.display()),
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
    assert!(stdout.contains("\"stop_code\":76"));
    assert_eq!(fs::read(&host_file).unwrap(), b"renamed writeback\n");
}
