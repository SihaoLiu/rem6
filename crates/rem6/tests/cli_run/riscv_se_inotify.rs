use std::{fs, process::Command};

use crate::support::{find_riscv_tool, temp_workspace};

#[test]
fn rem6_run_riscv_se_runs_static_raw_inotify_against_qemu() {
    let Some(gcc) = find_riscv_tool("riscv64-unknown-elf-gcc") else {
        eprintln!("skipping static RISC-V SE raw inotify smoke: riscv64-unknown-elf-gcc not found");
        return;
    };
    let Some(qemu) = find_riscv_tool("qemu-riscv64") else {
        eprintln!("skipping static RISC-V SE raw inotify smoke: qemu-riscv64 not found");
        return;
    };
    let workspace = temp_workspace("riscv-se-raw-inotify");
    let source = workspace.join("raw-inotify.c");
    let binary = workspace.join("raw-inotify");
    fs::write(
        &source,
        r#"#define AT_FDCWD -100
#define IN_CREATE 0x00000100
#define IN_IGNORED 0x00008000
#define IN_NONBLOCK 0x800
#define O_WRONLY 1
#define O_CREAT 0100
#define POLLIN 0x0001

struct pollfd {
    int fd;
    short events;
    short revents;
};

struct timespec {
    long tv_sec;
    long tv_nsec;
};

struct inotify_event_buffer {
    int wd;
    unsigned int mask;
    unsigned int cookie;
    unsigned int len;
    char name[64];
};

static long linux_syscall1(long number, long arg0) {
    register long a0 asm("a0") = arg0;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a7) : "memory");
    return a0;
}

static long linux_syscall2(long number, long arg0, long arg1) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall3(long number, long arg0, long arg1, long arg2) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a7) : "memory");
    return a0;
}

static long linux_syscall4(long number, long arg0, long arg1, long arg2, long arg3) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3),
                  "r"(a7) : "memory");
    return a0;
}

static long linux_syscall5(long number, long arg0, long arg1, long arg2,
                           long arg3, long arg4) {
    register long a0 asm("a0") = arg0;
    register long a1 asm("a1") = arg1;
    register long a2 asm("a2") = arg2;
    register long a3 asm("a3") = arg3;
    register long a4 asm("a4") = arg4;
    register long a7 asm("a7") = number;
    asm volatile ("ecall" : "+r"(a0) : "r"(a1), "r"(a2), "r"(a3),
                  "r"(a4), "r"(a7) : "memory");
    return a0;
}

static void write_stdout(const char *text, long length) {
    linux_syscall3(64, 1, (long)text, length);
}

static int starts_with_new_txt(const char *name) {
    return name[0] == 'n' && name[1] == 'e' && name[2] == 'w' &&
           name[3] == '.' && name[4] == 't' && name[5] == 'x' &&
           name[6] == 't' && name[7] == 0;
}

int main(void) {
    static const char watched[] = "watched";
    static const char created[] = "watched/new.txt";
    struct inotify_event_buffer event;
    struct timespec zero_timeout = {0, 0};

    long mkdir_status = linux_syscall3(34, AT_FDCWD, (long)watched, 0777);
    long fd = linux_syscall1(26, IN_NONBLOCK);
    long wd = fd >= 0 ? linux_syscall3(27, fd, (long)watched, IN_CREATE) : -99;
    struct pollfd poll_fd = {(int)fd, POLLIN, 0};
    long initial_read = fd >= 0 ? linux_syscall3(63, fd, (long)&event, sizeof(event)) : -99;
    long initial_poll = fd >= 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                                (long)&zero_timeout, 0, 0) : -99;
    long created_fd = linux_syscall4(56, AT_FDCWD, (long)created,
                                    O_WRONLY | O_CREAT, 0644);
    long close_created = created_fd >= 0 ? linux_syscall1(57, created_fd) : -99;
    poll_fd.revents = 0;
    long ready_poll = fd >= 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                              (long)&zero_timeout, 0, 0) : -99;
    short ready_revents = poll_fd.revents;
    long read_status = fd >= 0 ? linux_syscall3(63, fd, (long)&event, sizeof(event)) : -99;
    int create_event_ok = read_status >= 24 &&
                          event.wd == wd &&
                          event.mask == IN_CREATE &&
                          event.cookie == 0 &&
                          event.len >= 8 &&
                          starts_with_new_txt(event.name);
    poll_fd.revents = 0;
    long drained_poll = fd >= 0 ? linux_syscall5(73, (long)&poll_fd, 1,
                                                (long)&zero_timeout, 0, 0) : -99;
    long remove_status = fd >= 0 ? linux_syscall2(28, fd, wd) : -99;
    long ignored_read = fd >= 0 ? linux_syscall3(63, fd, (long)&event, sizeof(event)) : -99;
    int ignored_ok = ignored_read == 16 &&
                     event.wd == wd &&
                     event.mask == IN_IGNORED &&
                     event.cookie == 0 &&
                     event.len == 0;
    long write_status = fd >= 0 ? linux_syscall3(64, fd, (long)&event, 8) : -99;
    long close_status = fd >= 0 ? linux_syscall1(57, fd) : -99;
    long read_after_close = fd >= 0 ? linux_syscall3(63, fd, (long)&event, sizeof(event)) : -99;

    int ok = mkdir_status == 0 &&
             fd >= 0 &&
             wd == 1 &&
             initial_read == -11 &&
             initial_poll == 0 &&
             created_fd >= 0 &&
             close_created == 0 &&
             ready_poll == 1 &&
             (ready_revents & POLLIN) != 0 &&
             create_event_ok &&
             drained_poll == 0 &&
             remove_status == 0 &&
             ignored_ok &&
             write_status == -9 &&
             close_status == 0 &&
             read_after_close == -9;

    if (ok) {
        write_stdout("raw-inotify:ok\n", sizeof("raw-inotify:ok\n") - 1);
        linux_syscall1(93, 76);
    }
    write_stdout("raw-inotify:fail\n", sizeof("raw-inotify:fail\n") - 1);
    linux_syscall1(93, 91);
    return 0;
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

    let qemu_output = Command::new(&qemu)
        .arg(&binary)
        .current_dir(&workspace)
        .output()
        .unwrap();
    assert_eq!(
        qemu_output.status.code(),
        Some(76),
        "qemu stdout: {}; qemu stderr: {}",
        String::from_utf8_lossy(&qemu_output.stdout),
        String::from_utf8_lossy(&qemu_output.stderr)
    );
    assert_eq!(qemu_output.stdout, b"raw-inotify:ok\n");

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
        ])
        .current_dir(&workspace)
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
    assert!(stdout.contains("\"text\":\"raw-inotify:ok\\n\""));
    assert!(stdout.contains("\"riscv_unknown_syscalls\":[]"));
}
