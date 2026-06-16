use std::{
    env,
    process::{Command, Stdio},
    time::Duration,
};

use crate::gdb_support::*;
use crate::support::*;

#[test]
fn rem6_run_gdb_listen_executes_through_cache_runtimes() {
    let mut program = riscv64_program(&[
        0x0000_0297, // auipc x5, 0
        0x0402_8293, // addi x5, x5, 64
        0x0052_b023, // sd x5, 0(x5)
        0x0002_b303, // ld x6, 0(x5)
        0x0000_0073, // ecall
    ]);
    program.resize(0x50, 0);
    let elf = riscv64_elf(0x1000, 0x1000, &program);
    let path = temp_binary("gdb-listen-cache-runtime", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--gdb-listen",
            &listen.to_string(),
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stream = match connect_with_retry(listen.address(), Duration::from_secs(3)) {
        Ok(stream) => stream,
        Err(error) => {
            let output = wait_with_output_timeout(child, Duration::from_secs(1));
            panic!(
                "failed to connect to GDB listener: {error}; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(send_gdb_packet(&mut stream, b"c"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"4010000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.runs",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.cpu_responses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(&stdout, "sim.data_cache.runs", "Count", 0, "monotonic");
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.cpu_responses",
        "Count",
        0,
        "monotonic",
    );
}
