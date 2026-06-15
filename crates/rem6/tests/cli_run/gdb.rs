use std::{
    env,
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use crate::support::*;

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

    let mut stream = match connect_with_retry(listen, Duration::from_secs(3)) {
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
    assert_eq!(send_gdb_packet(&mut stream, b"c"), b"+$E22#a9");
    assert_eq!(send_gdb_packet(&mut stream, b"Z1,80000000,4"), b"+$E22#a9");
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
fn rem6_run_gdb_listen_applies_preexecution_state_changes() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gdb-listen-state-change", &elf);
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

    let mut stream = match connect_with_retry(listen, Duration::from_secs(3)) {
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
        send_gdb_packet(&mut stream, b"M80000000,4:13000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m80000000,4"),
        gdb_response(b"13000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P5=2a00000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"2a00000000000000")
    );
    let all_register_write = rv64_all_register_write_packet(0x2b, 0x8000_0000);
    assert_eq!(
        send_gdb_packet(&mut stream, &all_register_write),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5"),
        gdb_response(b"2b00000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"Z0,80000004,4"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"m80000004,4"),
        gdb_response(b"73001000")
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
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"trap_pc\":\"0x80000004\""));
    assert!(stdout.contains("\"x5\":\"0x2b\""));
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

fn unused_loopback_addr() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    address
}

fn connect_with_retry(address: SocketAddr, timeout: Duration) -> std::io::Result<TcpStream> {
    let deadline = Instant::now() + timeout;
    let mut last_error = None;
    while Instant::now() < deadline {
        match TcpStream::connect_timeout(&address, Duration::from_millis(100)) {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::TimedOut, "GDB listener did not accept")
    }))
}

fn send_gdb_packet(stream: &mut TcpStream, payload: &[u8]) -> Vec<u8> {
    stream.write_all(&gdb_packet(payload)).unwrap();
    read_gdb_response(stream)
}

fn gdb_response(payload: &[u8]) -> Vec<u8> {
    let mut response = Vec::with_capacity(payload.len() + 5);
    response.push(b'+');
    response.extend_from_slice(&gdb_packet(payload));
    response
}

fn gdb_packet(payload: &[u8]) -> Vec<u8> {
    let checksum = payload
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte));
    let mut packet = Vec::with_capacity(payload.len() + 4);
    packet.push(b'$');
    packet.extend_from_slice(payload);
    packet.push(b'#');
    packet.extend_from_slice(format!("{checksum:02x}").as_bytes());
    packet
}

fn rv64_all_register_write_packet(x5: u64, pc: u64) -> Vec<u8> {
    const RV64_REGISTER_BYTES: usize = 572;
    const X5_OFFSET: usize = 5 * 8;
    const PC_OFFSET: usize = 32 * 8;

    let mut registers = vec![0; RV64_REGISTER_BYTES];
    registers[X5_OFFSET..X5_OFFSET + 8].copy_from_slice(&x5.to_le_bytes());
    registers[PC_OFFSET..PC_OFFSET + 8].copy_from_slice(&pc.to_le_bytes());

    let mut payload = Vec::with_capacity(1 + registers.len() * 2);
    payload.push(b'G');
    for byte in registers {
        payload.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    payload
}

fn read_gdb_response(stream: &mut TcpStream) -> Vec<u8> {
    let mut response = Vec::new();
    let mut byte = [0; 1];
    while response.len() < 256 {
        stream.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
        if response.len() >= 4
            && response[response.len() - 3] == b'#'
            && response[response.len() - 2].is_ascii_hexdigit()
            && response[response.len() - 1].is_ascii_hexdigit()
        {
            return response;
        }
    }
    panic!(
        "GDB response is too long: {}",
        String::from_utf8_lossy(&response)
    );
}

fn wait_with_output_timeout(mut child: std::process::Child, timeout: Duration) -> Output {
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let stdout_reader = thread::spawn(move || read_pipe_to_end(&mut stdout));
    let stderr_reader = thread::spawn(move || read_pipe_to_end(&mut stderr));
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let status = child.wait().unwrap();
            let stdout = stdout_reader.join().unwrap();
            let stderr = stderr_reader.join().unwrap();
            panic!(
                "rem6 run did not exit within {:?}; status: {:?}; stdout: {}; stderr: {}",
                timeout,
                status,
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }
        thread::sleep(Duration::from_millis(25));
    };
    Output {
        status,
        stdout: stdout_reader.join().unwrap(),
        stderr: stderr_reader.join().unwrap(),
    }
}

fn read_pipe_to_end(reader: &mut impl Read) -> Vec<u8> {
    let mut output = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return output,
            Ok(bytes) => output.extend_from_slice(&buffer[..bytes]),
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("failed to read child pipe: {error}"),
        }
    }
}
