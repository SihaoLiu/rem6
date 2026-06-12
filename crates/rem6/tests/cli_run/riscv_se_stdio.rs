use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::support::*;

#[test]
fn rem6_run_riscv_se_runs_static_newlib_fgets_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE stdin smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE stdin smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-fgets");
    let source = workspace.join("stdin.c");
    let binary = workspace.join("stdin");
    let input = workspace.join("stdin.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
int main(void) {
    char buffer[32];
    if (fgets(buffer, sizeof(buffer), stdin) == NULL) {
        return 71;
    }
    printf("stdin:%s", buffer);
    return buffer[0] == 'r' ? 23 : 24;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"rem6 stdin\nignored\n").unwrap();

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

    let qemu_output = Command::new(&qemu)
        .arg(&binary)
        .stdin(Stdio::from(fs::File::open(&input).unwrap()))
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(23),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"stdin:rem6 stdin\n");

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
            "--riscv-se-stdin",
            input.to_str().unwrap(),
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
    assert!(stdout.contains("\"stop_code\":23"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"stdin:rem6 stdin\\n\""));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 23, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_fopen_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static newlib RISC-V SE file smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-fopen");
    let source = workspace.join("file.c");
    let binary = workspace.join("file");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
int main(void) {
    FILE *file = fopen("guest.txt", "rb");
    if (file == NULL) {
        return 71;
    }

    char buffer[32];
    size_t count = fread(buffer, 1, 18, file);
    if (count != 18) {
        return 72;
    }
    buffer[count] = '\0';
    printf("file:%s", buffer);
    return buffer[0] == 'f' ? 31 : 32;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\nignored\n").unwrap();

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
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":31"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"file:file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 31, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_stat_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static newlib RISC-V SE stat smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-stat");
    let source = workspace.join("stat.c");
    let binary = workspace.join("stat");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <sys/stat.h>

int main(void) {
    struct stat st;
    if (stat("guest.txt", &st) != 0) {
        return 71;
    }
    printf("stat:%ld:%lo\n", (long)st.st_size, (unsigned long)(st.st_mode & 0777777));
    return st.st_size == 18 ? 33 : 34;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\n").unwrap();

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
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":33"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"stat:18:100444\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 33, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_access_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE access smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-access");
    let source = workspace.join("access.c");
    let binary = workspace.join("access");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <errno.h>
#include <stdio.h>
#include <unistd.h>

extern int _access(const char *path, int mode);

int main(void) {
    errno = 0;
    int present = _access("guest.txt", R_OK);
    int present_errno = errno;
    errno = 0;
    int missing = _access("missing.txt", F_OK);
    int missing_errno = errno;
    printf("access:%d:%d:%d:%d\n", present, present_errno, missing, missing_errno);
    return present == 0 &&
           present_errno == 0 &&
           missing == -1 &&
           missing_errno == ENOENT ? 51 : 52;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\n").unwrap();

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
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":51"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"access:0:0:-1:2\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 51, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_faccessat_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE faccessat smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static newlib RISC-V SE faccessat smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-faccessat");
    let source = workspace.join("faccessat.c");
    let binary = workspace.join("faccessat");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <errno.h>
#include <stdio.h>
#include <unistd.h>

extern int _faccessat(int dirfd, const char *path, int mode, int flags);

int main(void) {
    errno = 0;
    int present = _faccessat(-100, "guest.txt", R_OK, 0);
    int present_errno = errno;
    errno = 0;
    int missing = _faccessat(-100, "missing.txt", F_OK, 0);
    int missing_errno = errno;
    printf("faccessat:%d:%d:%d:%d\n", present, present_errno, missing, missing_errno);
    return present == 0 &&
           present_errno == 0 &&
           missing == -1 &&
           missing_errno == ENOENT ? 53 : 54;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\n").unwrap();

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

    let qemu_output = Command::new(&qemu)
        .current_dir(&workspace)
        .arg(&binary)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(53),
        "qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"faccessat:0:0:-1:2\n");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":53"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"faccessat:0:0:-1:2\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 53, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_unlink_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!(
            "skipping static newlib RISC-V SE unlink smoke: riscv64-unknown-elf-gcc not found"
        );
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-unlink");
    let source = workspace.join("unlink.c");
    let binary = workspace.join("unlink");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

int main(void) {
    int removed = unlink("guest.txt");
    struct stat st;
    int after = stat("guest.txt", &st);
    printf("unlink:%d:%d\n", removed, after);
    return removed == 0 && after != 0 ? 47 : 48;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\n").unwrap();

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
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":47"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"unlink:0:-1\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 47, "constant");
}

#[test]
fn rem6_run_riscv_se_runs_static_newlib_link_on_registered_guest_file() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static newlib RISC-V SE link smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-newlib-link");
    let source = workspace.join("link.c");
    let binary = workspace.join("link");
    let input = workspace.join("guest.txt");
    fs::write(
        &source,
        r#"#include <stdio.h>
#include <sys/stat.h>
#include <unistd.h>

int main(void) {
    int made = link("guest.txt", "alias.txt");
    struct stat original;
    struct stat alias;
    int original_stat = stat("guest.txt", &original);
    int alias_stat = stat("alias.txt", &alias);
    printf("link:%d:%d:%d:%ld:%ld:%ld:%ld\n",
           made,
           original_stat,
           alias_stat,
           (long)original.st_size,
           (long)alias.st_size,
           (long)original.st_nlink,
           (long)alias.st_nlink);
    return made == 0 &&
           original_stat == 0 &&
           alias_stat == 0 &&
           alias.st_size == 18 &&
           original.st_ino == alias.st_ino &&
           original.st_nlink == 2 &&
           alias.st_nlink == 2 ? 49 : 50;
}
"#,
    )
    .unwrap();
    fs::write(&input, b"file-backed input\n").unwrap();

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
            "300000",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":49"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"link:0:0:0:18:18:2:2\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 49, "constant");
}

#[test]
fn rem6_run_riscv_se_opens_registered_guest_file_from_host_bytes() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-registered-guest-file", &elf);
    let input = temp_binary(
        "riscv-se-registered-guest-file-input",
        b"file-backed input\nignored\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "240",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
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
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"riscv_guest_writes\":[{\"fd\":1"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
    assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop_code", "Count", 18, "constant");
}

#[test]
fn rem6_run_riscv_se_toml_guest_file_resolves_from_config_directory() {
    let program = registered_guest_file_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-relative-guest-file");
    let binary = workspace.join("guest.elf");
    let input = workspace.join("input.txt");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&input, b"file-backed input\nignored\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 240\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_files = [\"guest.txt=input.txt\"]\n",
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
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":18"));
    assert!(stdout.contains("\"text\":\"file-backed input\\n\""));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

#[test]
fn rem6_run_riscv_se_reports_missing_stdin_file() {
    let program = riscv64_program(&[
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-missing-stdin", &elf);
    let stdin = temp_output("riscv-se-missing-stdin-input");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-stdin",
            stdin.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!(
        "failed to read RISC-V SE stdin {}",
        stdin.display()
    )));
}

#[test]
fn rem6_run_riscv_se_reports_missing_guest_file() {
    let program = riscv64_program(&[
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("riscv-se-missing-guest-file", &elf);
    let input = temp_output("riscv-se-missing-guest-file-input");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--riscv-se",
            "--riscv-se-file",
            &format!("guest.txt={}", input.display()),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(&format!(
        "failed to read RISC-V SE file guest.txt from {}",
        input.display()
    )));
}

#[test]
fn rem6_run_riscv_se_toml_stdin_path_resolves_from_config_directory() {
    let program = riscv64_program(&[
        i_type(0, 0, 0x0, 10, 0x13),  // addi a0, x0, 0
        i_type(-8, 2, 0x0, 11, 0x13), // addi a1, sp, -8
        i_type(1, 0, 0x0, 12, 0x13),  // addi a2, x0, 1
        i_type(63, 0, 0x0, 17, 0x13), // addi a7, x0, 63
        0x0000_0073,                  // ecall
        i_type(0, 11, 0x4, 10, 0x03), // lbu a0, 0(a1)
        i_type(93, 0, 0x0, 17, 0x13), // addi a7, x0, 93
        0x0000_0073,                  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let workspace = temp_workspace("riscv-se-toml-relative-stdin");
    let binary = workspace.join("guest.elf");
    let stdin = workspace.join("stdin.txt");
    let config = workspace.join("run.toml");
    fs::write(&binary, elf).unwrap();
    fs::write(&stdin, b"relative stdin\n").unwrap();
    fs::write(
        &config,
        "[run]\nisa = \"riscv\"\nbinary = \"guest.elf\"\nmax_tick = 120\nstats_format = \"json\"\nexecute = true\nriscv_se = true\nriscv_se_stdin = \"stdin.txt\"\n",
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
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":114"));
    assert!(!stdout.contains("riscv_unknown_syscalls\":[{"));
}

fn find_riscv_tool(name: &str) -> Option<PathBuf> {
    find_tool_on_path(name).or_else(|| {
        let module_candidate =
            Path::new("/mnt/nas0/software/riscv/riscv64-elf-ubuntu-24.04-gcc/bin").join(name);
        module_candidate.is_file().then_some(module_candidate)
    })
}

fn find_tool_on_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    })
}

fn registered_guest_file_program() -> Vec<u8> {
    const PATH_OFFSET: usize = 0x80;
    const BUFFER_OFFSET: usize = 0xa0;

    let mut program = riscv64_program(&[
        i_type(-100, 0, 0x0, 10, 0x13), // addi a0, x0, AT_FDCWD
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(PATH_OFFSET as i32 - 4, 11, 0x0, 11, 0x13), // addi a1, a1, path
        i_type(0, 0, 0x0, 12, 0x13),    // addi a2, x0, O_RDONLY
        i_type(0, 0, 0x0, 13, 0x13),    // addi a3, x0, 0
        i_type(56, 0, 0x0, 17, 0x13),   // addi a7, x0, 56
        0x0000_0073,                    // ecall
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(BUFFER_OFFSET as i32 - 28, 11, 0x0, 11, 0x13), // addi a1, a1, buffer
        i_type(18, 0, 0x0, 12, 0x13),   // addi a2, x0, 18
        i_type(63, 0, 0x0, 17, 0x13),   // addi a7, x0, 63
        0x0000_0073,                    // ecall
        i_type(1, 0, 0x0, 10, 0x13),    // addi a0, x0, 1
        u_type(0, 11, 0x17),            // auipc a1, 0
        i_type(BUFFER_OFFSET as i32 - 52, 11, 0x0, 11, 0x13), // addi a1, a1, buffer
        i_type(18, 0, 0x0, 12, 0x13),   // addi a2, x0, 18
        i_type(64, 0, 0x0, 17, 0x13),   // addi a7, x0, 64
        0x0000_0073,                    // ecall
        i_type(18, 0, 0x0, 10, 0x13),   // addi a0, x0, 18
        i_type(93, 0, 0x0, 17, 0x13),   // addi a7, x0, 93
        0x0000_0073,                    // ecall
    ]);
    program.resize(PATH_OFFSET, 0);
    program.extend_from_slice(b"guest.txt\0");
    program.resize(BUFFER_OFFSET + 32, 0);
    program
}
