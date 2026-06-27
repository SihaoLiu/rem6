use std::{
    env,
    io::Write,
    net::TcpStream,
    process::{Child, Command, Stdio},
    time::Duration,
};

use crate::gdb_support::*;
use crate::support::*;

#[path = "gdb/preexecution_state.rs"]
mod preexecution_state;
#[path = "gdb/trap_csrs.rs"]
mod trap_csrs;

fn start_riscv_gdb_run(name: &str, program: Vec<u8>, max_tick: u64) -> (Child, TcpStream) {
    let elf = riscv64_elf(0x1000, 0x1000, &program);
    let path = temp_binary(name, &elf);
    let listen = unused_loopback_addr();
    let max_tick = max_tick.to_string();
    let listen_text = listen.to_string();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            &max_tick,
            "--stats-format",
            "json",
            "--execute",
            "--gdb-listen",
            &listen_text,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let stream = match connect_with_retry(listen.address(), Duration::from_secs(3)) {
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
    (child, stream)
}

#[test]
fn rem6_run_gdb_listen_serves_loaded_riscv_state_before_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-exec", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--gdb-listen",
            &listen.to_string(),
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

    assert_eq!(send_gdb_packet(&mut stream, b"?"), b"+$S05#b8");
    assert_eq!(
        send_gdb_packet(&mut stream, b"p20"),
        b"+$0000008000000000#08"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m80000000,4"),
        b"+$93027000#95"
    );
    assert_eq!(send_gdb_packet(&mut stream, b"C05"), b"+$E22#a9");
    assert_eq!(send_gdb_packet(&mut stream, b"c80000000"), b"+$E22#a9");
    assert_eq!(send_gdb_packet(&mut stream, b"vCont;C05"), b"+$E22#a9");
    assert_eq!(send_gdb_packet(&mut stream, b"vCont;c;s"), b"+$E22#a9");
    assert_eq!(send_gdb_packet(&mut stream, b"vCont;c:1"), b"+$E22#a9");
    assert_eq!(send_gdb_packet(&mut stream, b"Hc1"), b"+$E02#a7");
    assert_eq!(send_gdb_packet(&mut stream, b"Z1,80000000,4"), b"+$OK#9a");
    assert_eq!(send_gdb_packet(&mut stream, b"z1,80000000,4"), b"+$OK#9a");
    assert_eq!(
        send_gdb_packet(&mut stream, b"m80000000,4"),
        b"+$93027000#95"
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), b"+$OK#9a");

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
}

#[test]
fn rem6_run_gdb_listen_writes_sscratch_before_execution() {
    let program = riscv64_program(&[
        0x1400_22f3, // csrr x5, sscratch
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-sscratch", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let csr_description = send_gdb_packet(
        &mut stream,
        b"qXfer:features:read:riscv-64bit-csr.xml:a0,a0",
    );
    assert!(
        String::from_utf8_lossy(&csr_description).contains("sscratch"),
        "missing sscratch in {}",
        String::from_utf8_lossy(&csr_description)
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P48=8877665544332211"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"8877665544332211")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
}

#[test]
fn rem6_run_gdb_listen_writes_satp_before_execution() {
    let program = riscv64_program(&[
        0x1800_22f3, // csrr x5, satp
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-satp", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let csr_description = send_gdb_packet(
        &mut stream,
        b"qXfer:features:read:riscv-64bit-csr.xml:100,a0",
    );
    assert!(
        String::from_utf8_lossy(&csr_description).contains("satp"),
        "missing satp in {}",
        String::from_utf8_lossy(&csr_description)
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P4c=8877665544332211"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"8877665544332211")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
}

#[test]
fn rem6_run_gdb_listen_writes_mscratch_before_execution() {
    let program = riscv64_program(&[
        0x3400_22f3, // csrr x5, mscratch
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-mscratch", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let csr_description = send_gdb_packet(
        &mut stream,
        b"qXfer:features:read:riscv-64bit-csr.xml:1c0,80",
    );
    assert!(
        String::from_utf8_lossy(&csr_description).contains("mscratch"),
        "missing mscratch in {}",
        String::from_utf8_lossy(&csr_description)
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P52=8877665544332211"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"8877665544332211")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
}

#[test]
fn rem6_run_gdb_listen_reads_mhartid_before_execution() {
    let program = riscv64_program(&[
        csr_read(0xf14, 5),          // csrr x5, mhartid
        i_type(11, 5, 0x0, 5, 0x13), // addi x5, x5, 11
        0x0000_0073,                 // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-mhartid", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:140,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:1e0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:280,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:320,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:3c0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:460,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("mhartid"),
        "missing mhartid in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7f"),
        gdb_response(b"0000000000000000")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0b00000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"status\":\"executed_until_trap\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"x5\":\"0xb\""), "stdout: {stdout}");
}

#[test]
fn rem6_run_gdb_listen_reads_misa_before_execution() {
    let program = riscv64_program(&[
        csr_read(0x301, 5), // csrr x5, misa
        0x0000_0073,        // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-misa", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:140,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:1e0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:280,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:320,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:3c0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:460,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("misa"),
        "missing misa in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p83"),
        gdb_response(b"2d11140000000080")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P83=0000000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p83"),
        gdb_response(b"2d11140000000080")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"status\":\"executed_until_trap\""),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("\"x5\":\"0x800000000014112d\""),
        "stdout: {stdout}"
    );
}

#[test]
fn rem6_run_gdb_listen_reads_machine_identity_csrs_before_execution() {
    let program = riscv64_program(&[
        csr_read(0xf11, 5),         // csrr x5, mvendorid
        csr_read(0xf12, 6),         // csrr x6, marchid
        csr_read(0xf13, 7),         // csrr x7, mimpid
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(2, 6, 0x0, 6, 0x13), // addi x6, x6, 2
        i_type(3, 7, 0x0, 7, 0x13), // addi x7, x7, 3
        0x0000_0073,                // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-machine-identity", program, 80);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:140,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:1e0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:280,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:320,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:3c0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:460,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    for register in ["mvendorid", "marchid", "mimpid"] {
        assert!(
            csr_description.contains(register),
            "missing {register} in {csr_description}"
        );
    }
    assert_eq!(
        send_gdb_packet(&mut stream, b"p80"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p81"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p82"),
        gdb_response(b"0000000000000000")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"status\":\"executed_until_trap\""),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("\"x5\":\"0x1\""), "stdout: {stdout}");
    assert!(stdout.contains("\"x6\":\"0x2\""), "stdout: {stdout}");
    assert!(stdout.contains("\"x7\":\"0x3\""), "stdout: {stdout}");
}

#[test]
fn rem6_run_gdb_listen_writes_sie_before_execution() {
    let program = riscv64_program(&[
        0x1040_22f3, // csrr x5, sie
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-sie", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"P4f=aa0a000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P7a=8808000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7a"),
        gdb_response(b"8808000000000000")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"8808000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x888\""));
}

#[test]
fn rem6_run_gdb_listen_writes_senvcfg_before_execution() {
    let program = riscv64_program(&[
        csr_read(0x10a, 5), // csrr x5, senvcfg
        0x0000_0073,        // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-senvcfg", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:140,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:1e0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:280,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:320,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:3c0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:460,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:5a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:640,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("senvcfg"),
        "missing senvcfg in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P87=3300000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p87"),
        gdb_response(b"3300000000000000")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"3300000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x33\""));
}

#[test]
fn rem6_run_gdb_listen_writes_pmp_csrs_before_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-pmp-csrs", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:140,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:1e0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:280,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:320,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:3c0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:460,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:5a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:640,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    for register in ["pmpcfg0", "pmpaddr0"] {
        assert!(
            csr_description.contains(register),
            "missing {register} in {csr_description}"
        );
    }
    assert_eq!(
        send_gdb_packet(&mut stream, b"p88"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p89"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P88=0f00000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P89=0002000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p88"),
        gdb_response(b"0f00000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p89"),
        gdb_response(b"0002000000000000")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0700000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
}

#[test]
fn rem6_run_gdb_listen_pmp_csr_write_is_consumed_by_fetch_access_check() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-pmp-fetch-deny", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"P89=0000002000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P88=8800000000000000"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("PMP") || stderr.contains("pmp"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_gdb_listen_packed_pmpcfg0_entry1_is_consumed_by_fetch_access_check() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-pmp-entry1-fetch-deny", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:5a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:640,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("pmpaddr1"),
        "missing pmpaddr1 in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P8c=0008000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P88=0088000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p88"),
        gdb_response(b"0088000000000000")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("PMP") || stderr.contains("pmp"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_gdb_listen_pmpaddr2_is_consumed_by_fetch_access_check() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-pmp-entry2-fetch-deny", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:5a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:640,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:6e0,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("pmpaddr2"),
        "missing pmpaddr2 in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P8d=0008000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P88=0000880000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p8d"),
        gdb_response(b"0008000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p88"),
        gdb_response(b"0000880000000000")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("PMP") || stderr.contains("pmp"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_gdb_listen_pmpaddr3_is_consumed_by_fetch_access_check() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-pmp-entry3-fetch-deny", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:5a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:640,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:6e0,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("pmpaddr3"),
        "missing pmpaddr3 in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P8e=0008000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P88=0000008800000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p8e"),
        gdb_response(b"0008000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p88"),
        gdb_response(b"0000008800000000")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("PMP") || stderr.contains("pmp"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_gdb_listen_pmpaddr7_is_consumed_by_fetch_access_check() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-pmp-entry7-fetch-deny", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:5a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:640,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:6e0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:780,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("pmpaddr7"),
        "missing pmpaddr7 in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P92=0008000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P88=0000000000000088"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p92"),
        gdb_response(b"0008000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p88"),
        gdb_response(b"0000000000000088")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        !output.status.success(),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("PMP") || stderr.contains("pmp"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_gdb_listen_single_steps_before_detach() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-single-step", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--gdb-listen",
            &listen.to_string(),
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
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    stream.write_all(b"-").unwrap();
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p20"),
        gdb_response(b"0400008000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0700000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(stdout.contains("\"x6\":\"0x8\""));
}

#[test]
fn rem6_run_gdb_listen_reads_counter_csrs_after_single_step() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-counter-csrs", program, 80);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:140,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:1e0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:280,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:320,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:3c0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:460,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("cycle"),
        "missing cycle in {csr_description}"
    );
    assert!(
        csr_description.contains("instret"),
        "missing instret in {csr_description}"
    );
    assert!(
        csr_description.contains("time"),
        "missing time in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7c"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7d"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7e"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7c"),
        gdb_response(b"0100000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7d"),
        gdb_response(b"0100000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7e"),
        gdb_response(b"0100000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
}

#[test]
fn rem6_run_gdb_listen_counter_csr_writes_keep_time_independent() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-counter-csr-writes", program, 80);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"P7c=4000000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7c"),
        gdb_response(b"4000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7d"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P7d=0200000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7c"),
        gdb_response(b"4000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7d"),
        gdb_response(b"0200000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7c"),
        gdb_response(b"4100000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7d"),
        gdb_response(b"0300000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
}

#[test]
fn rem6_run_gdb_listen_exposes_machine_counter_csr_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-machine-counter-csrs", program, 80);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    let mut csr_description = String::new();
    for payload in [
        b"qXfer:features:read:riscv-64bit-csr.xml:0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:140,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:1e0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:280,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:320,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:3c0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:460,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:500,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:5a0,a0".as_slice(),
        b"qXfer:features:read:riscv-64bit-csr.xml:640,a0".as_slice(),
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    assert!(
        csr_description.contains("mcycle"),
        "missing mcycle in {csr_description}"
    );
    assert!(
        csr_description.contains("minstret"),
        "missing minstret in {csr_description}"
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p8a"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P8a=0900000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7c"),
        gdb_response(b"0900000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P8b=0500000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7d"),
        gdb_response(b"0500000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p8a"),
        gdb_response(b"0a00000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p8b"),
        gdb_response(b"0600000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
}

#[test]
fn rem6_run_gdb_listen_continues_until_guest_stop() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-continue", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--gdb-listen",
            &listen.to_string(),
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
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0700000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"0800000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(stdout.contains("\"x6\":\"0x8\""));
}

#[test]
fn rem6_run_gdb_listen_vcont_continue_runs_until_guest_stop() {
    let program = riscv64_program(&[
        0x0090_0293, // addi x5, x0, 9
        0x0022_8313, // addi x6, x5, 2
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-vcont-continue", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--gdb-listen",
            &listen.to_string(),
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
    let supported = send_gdb_packet(&mut stream, b"qSupported:vContSupported+");
    assert!(String::from_utf8_lossy(&supported).contains("vContSupported+"));
    stream.write_all(&gdb_packet(b"vCont;c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0900000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"0b00000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x5\":\"0x9\""));
    assert!(stdout.contains("\"x6\":\"0xb\""));
}

#[test]
fn rem6_run_gdb_listen_hardware_breakpoint_stops_at_current_pc_before_retire() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) =
        start_riscv_gdb_run("gdb-listen-hardware-breakpoint-current", program, 120);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z1,1000,4"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p20"),
        gdb_response(b"0010000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z1,1000,4"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"c"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0700000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
}

#[test]
fn rem6_run_gdb_listen_hardware_breakpoint_stops_before_target_instruction() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0080_0313, // addi x6, x0, 8
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) =
        start_riscv_gdb_run("gdb-listen-hardware-breakpoint-next", program, 120);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z1,1004,4"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p20"),
        gdb_response(b"0410000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0700000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z1,1004,4"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"c"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"0800000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(stdout.contains("\"x6\":\"0x8\""));
}

#[test]
fn rem6_run_gdb_listen_hardware_breakpoint_wins_over_instruction_budget() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0080_0313, // addi x6, x0, 8
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x1000, 0x1000, &program);
    let path = temp_binary("gdb-listen-hardware-breakpoint-instruction-budget", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--max-instructions",
            "1",
            "--stats-format",
            "json",
            "--execute",
            "--gdb-listen",
            &listen.to_string(),
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
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z1,1004,4"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p20"),
        gdb_response(b"0410000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0700000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z1,1004,4"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_instruction_limit\""));
    assert!(stdout.contains("\"committed_instructions\":1"));
    assert!(!stdout.contains("\"x6\":\"0x8\""));
}

#[test]
fn rem6_run_gdb_listen_write_watchpoint_stops_on_riscv_store() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),    // auipc x2, 0
        0x0550_0313,           // addi x6, x0, 0x55
        s_type(64, 6, 2, 0x3), // sd x6, 64(x2)
        0x0010_0393,           // addi x7, x0, 1
        0x0000_0073,           // ecall
    ]);
    program.resize(72, 0);
    let elf = riscv64_elf(0x1000, 0x1000, &program);
    let path = temp_binary("gdb-listen-write-watchpoint", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--gdb-listen",
            &listen.to_string(),
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
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z2,1040,8"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(
        read_gdb_response(&mut stream),
        gdb_packet(b"T05watch:1040;")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m1040,8"),
        gdb_response(b"5500000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z2,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x6\":\"0x55\""));
    assert!(stdout.contains("\"x7\":\"0x1\""));
}

#[test]
fn rem6_run_gdb_listen_read_watchpoint_stops_on_riscv_load() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(64, 2, 0x3, 6, 0x03), // ld x6, 64(x2)
        0x0010_0393,                 // addi x7, x0, 1
        0x0000_0073,                 // ecall
    ]);
    program.resize(72, 0);
    program[64..72].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-read-watchpoint", program, 120);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z3,1040,8"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(
        read_gdb_response(&mut stream),
        gdb_packet(b"T05rwatch:1040;")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"8877665544332211")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z3,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x6\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x7\":\"0x1\""));
}

#[test]
fn rem6_run_gdb_listen_access_watchpoint_stops_on_riscv_store() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),    // auipc x2, 0
        0x0550_0313,           // addi x6, x0, 0x55
        s_type(64, 6, 2, 0x3), // sd x6, 64(x2)
        0x0010_0393,           // addi x7, x0, 1
        0x0000_0073,           // ecall
    ]);
    program.resize(72, 0);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-access-watchpoint", program, 120);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z4,1040,8"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(
        read_gdb_response(&mut stream),
        gdb_packet(b"T05awatch:1040;")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m1040,8"),
        gdb_response(b"5500000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z4,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x6\":\"0x55\""));
    assert!(stdout.contains("\"x7\":\"0x1\""));
}

#[test]
fn rem6_run_gdb_listen_read_watchpoint_ignores_riscv_store() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),    // auipc x2, 0
        0x0550_0313,           // addi x6, x0, 0x55
        s_type(64, 6, 2, 0x3), // sd x6, 64(x2)
        0x0010_0393,           // addi x7, x0, 1
        0x0000_0073,           // ecall
    ]);
    program.resize(72, 0);
    let (child, mut stream) =
        start_riscv_gdb_run("gdb-listen-read-watchpoint-ignore-store", program, 120);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z3,1040,8"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7"),
        gdb_response(b"0100000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m1040,8"),
        gdb_response(b"5500000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z3,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x7\":\"0x1\""));
}

#[test]
fn rem6_run_gdb_listen_write_watchpoint_ignores_riscv_load() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(64, 2, 0x3, 6, 0x03), // ld x6, 64(x2)
        0x0010_0393,                 // addi x7, x0, 1
        0x0000_0073,                 // ecall
    ]);
    program.resize(72, 0);
    program[64..72].copy_from_slice(&0x1122_3344_5566_7788_u64.to_le_bytes());
    let (child, mut stream) =
        start_riscv_gdb_run("gdb-listen-write-watchpoint-ignore-load", program, 120);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z2,1040,8"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"8877665544332211")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7"),
        gdb_response(b"0100000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z2,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x6\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x7\":\"0x1\""));
}

#[test]
fn rem6_run_gdb_listen_write_watchpoint_single_step_drains_pending_store_before_next_retire() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),    // auipc x2, 0
        0x0550_0313,           // addi x6, x0, 0x55
        s_type(64, 6, 2, 0x3), // sd x6, 64(x2)
        0x0010_0393,           // addi x7, x0, 1
        0x0000_0073,           // ecall
    ]);
    program.resize(72, 0);
    let (child, mut stream) =
        start_riscv_gdb_run("gdb-listen-watchpoint-pending-store", program, 120);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z2,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"s"),
        gdb_response(b"T05watch:1040;")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m1040,8"),
        gdb_response(b"5500000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z2,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x7\":\"0x1\""));
}

#[test]
fn rem6_run_gdb_listen_write_watchpoint_wins_over_instruction_budget_drain() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),    // auipc x2, 0
        0x0550_0313,           // addi x6, x0, 0x55
        s_type(64, 6, 2, 0x3), // sd x6, 64(x2)
        0x0010_0393,           // addi x7, x0, 1
        0x0000_0073,           // ecall
    ]);
    program.resize(72, 0);
    let elf = riscv64_elf(0x1000, 0x1000, &program);
    let path = temp_binary("gdb-listen-watchpoint-instruction-budget", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--max-instructions",
            "3",
            "--stats-format",
            "json",
            "--execute",
            "--gdb-listen",
            &listen.to_string(),
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
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z2,1040,8"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(
        read_gdb_response(&mut stream),
        gdb_packet(b"T05watch:1040;")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m1040,8"),
        gdb_response(b"5500000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"z2,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_instruction_limit\""));
    assert!(stdout.contains("\"committed_instructions\":3"));
    assert!(!stdout.contains("\"x7\":\"0x1\""));
}

#[test]
fn rem6_run_gdb_listen_write_watchpoint_ignores_completed_store_history() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),    // auipc x2, 0
        0x0550_0313,           // addi x6, x0, 0x55
        s_type(64, 6, 2, 0x3), // sd x6, 64(x2)
        0x0010_0393,           // addi x7, x0, 1
        0x0020_0413,           // addi x8, x0, 2
        0x0000_0073,           // ecall
    ]);
    program.resize(72, 0);
    let (child, mut stream) =
        start_riscv_gdb_run("gdb-listen-watchpoint-stale-store", program, 160);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p7"),
        gdb_response(b"0100000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m1040,8"),
        gdb_response(b"5500000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z2,1040,8"),
        gdb_response(b"OK")
    );

    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p8"),
        gdb_response(b"0200000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"c"), gdb_response(b"E22"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"z2,1040,8"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"x8\":\"0x2\""));
}

#[test]
fn rem6_run_gdb_listen_rejects_continue_after_completed_guest_stop() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-completed-continue", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--gdb-listen",
            &listen.to_string(),
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
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z0,80000004,4"),
        gdb_response(b"OK")
    );
    stream.write_all(&gdb_packet(b"c")).unwrap();
    read_gdb_ack(&mut stream);
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0700000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p6"),
        gdb_response(b"0000000000000000")
    );

    assert_eq!(
        send_gdb_packet(&mut stream, b"z0,80000004,4"),
        gdb_response(b"OK")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"c"), gdb_response(b"E22"));
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"trap_pc\":\"0x80000004\""));
    assert!(stdout.contains("\"committed_instructions\":2"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x8\""));
}

#[test]
fn rem6_run_gdb_listen_rejects_single_step_without_tick_budget() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-single-step-no-tick", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "0",
            "--stats-format",
            "json",
            "--execute",
            "--gdb-listen",
            &listen.to_string(),
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
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    stream.write_all(b"-").unwrap();
    assert_eq!(read_gdb_response(&mut stream), gdb_packet(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p20"),
        gdb_response(b"0000008000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"0000000000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_tick_limit\""));
    assert!(stdout.contains("\"committed_instructions\":0"));
}

#[test]
fn rem6_run_gdb_listen_single_step_counts_against_instruction_limit() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0012_8313, // addi x6, x5, 1
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-single-step-instruction-limit", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--max-instructions",
            "1",
            "--stats-format",
            "json",
            "--execute",
            "--gdb-listen",
            &listen.to_string(),
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
    assert_eq!(send_gdb_packet(&mut stream, b"s"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"p20"),
        gdb_response(b"0400008000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_instruction_limit\""));
    assert!(stdout.contains("\"instruction_limit\":1"));
    assert!(stdout.contains("\"committed_instructions\":1"));
    assert!(stdout.contains("\"pc\":\"0x80000004\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x8\""));
}

#[test]
fn rem6_run_gdb_listen_processes_buffered_packets_after_single_step() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-single-step-buffered", &elf);
    let listen = unused_loopback_addr();
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
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
            "--gdb-listen",
            &listen.to_string(),
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
    send_gdb_packets(&mut stream, &[b"s".as_slice(), b"p20".as_slice()]);
    assert_eq!(read_gdb_response(&mut stream), gdb_response(b"S05"));
    assert_eq!(
        read_gdb_response(&mut stream),
        gdb_response(b"0400008000000000")
    );
    assert_eq!(send_gdb_packet(&mut stream, b"D"), gdb_response(b"OK"));

    let output = wait_with_output_timeout(child, Duration::from_secs(5));
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rem6_run_rejects_gdb_listen_from_toml_config() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("gdb-listen-toml", &elf);
    let config = temp_config(
        "gdb-listen-toml",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\ngdb_listen = \"127.0.0.1:1\"\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("failed to parse config"));
    assert!(stderr.contains("gdb_listen"));
}

#[test]
fn rem6_run_config_prescan_treats_gdb_listen_argument_as_a_value() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("gdb-listen-prescan", &elf);
    let config = temp_config(
        "gdb-listen-prescan",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 40\nexecute = true\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--gdb-listen",
            "--config",
            "--config",
            config.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("--gdb-listen requires an explicit loopback address"),
        "stderr: {stderr}"
    );
    assert!(
        !stderr.contains("failed to read config --config"),
        "stderr: {stderr}"
    );
}

#[test]
fn rem6_run_rejects_non_loopback_gdb_listen_before_accepting_connections() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("gdb-listen-non-loopback", &elf);
    let child = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            binary.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--gdb-listen",
            "0.0.0.0:0",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let output = wait_with_output_timeout(child, Duration::from_secs(2));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--gdb-listen requires an explicit loopback address"));
}
