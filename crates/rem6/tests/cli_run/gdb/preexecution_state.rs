use super::*;

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
    assert_eq!(
        send_gdb_packet(&mut stream, b"P84=0c00000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P85=d000000000000000"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p84"),
        gdb_response(b"0c00000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p85"),
        gdb_response(b"d000000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p86"),
        gdb_response(b"1000000000000000")
    );
    let vector_description = send_gdb_packet(
        &mut stream,
        b"qXfer:features:read:riscv-64bit-vector.xml:0,a0",
    );
    assert!(
        String::from_utf8_lossy(&vector_description).contains("org.gnu.gdb.riscv.vector"),
        "missing vector feature in {}",
        String::from_utf8_lossy(&vector_description)
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"P5a=00112233445566778899aabbccddeeff"),
        gdb_response(b"OK")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p5a"),
        gdb_response(b"00112233445566778899aabbccddeeff")
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
        send_gdb_packet(&mut stream, b"p5a"),
        gdb_response(b"404142434445464748494a4b4c4d4e4f")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p84"),
        gdb_response(b"0900000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p85"),
        gdb_response(b"c800000000000000")
    );
    assert_eq!(
        send_gdb_packet(&mut stream, b"p87"),
        gdb_response(b"3300000000000000")
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
fn rem6_run_gdb_listen_rejects_short_all_register_write_without_disconnect() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-short-register-set", program, 40);

    assert_eq!(send_gdb_packet(&mut stream, b"?"), gdb_response(b"S05"));
    assert_eq!(
        send_gdb_packet(&mut stream, b"G00000000"),
        gdb_response(b"E01")
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
}

fn rv64_all_register_write_packet(x5: u64, pc: u64) -> Vec<u8> {
    const RV64_INTEGER_AND_PC_REGISTERS: usize = 33;
    const RV64_FLOAT_REGISTERS: usize = 32;
    const RV64_FLOAT_CSR_AND_PLACEHOLDER_REGISTERS: usize = 4;
    const RV64_CSR_REGISTERS: usize = 20;
    const RV64_CSR_EXTENSION_REGISTERS: usize = 25;
    const RV64_VECTOR_REGISTERS: usize = 32;
    const RV64_VECTOR_REGISTER_BYTES: usize = 16;
    const RV64_SPARSE_CSR_REGISTERS_BEFORE_VECTOR_CONFIG: usize = 10;
    const RV64_REGISTER_BYTES: usize = (RV64_INTEGER_AND_PC_REGISTERS
        + RV64_FLOAT_REGISTERS
        + RV64_CSR_REGISTERS
        + RV64_CSR_EXTENSION_REGISTERS)
        * 8
        + RV64_FLOAT_CSR_AND_PLACEHOLDER_REGISTERS * 4
        + RV64_VECTOR_REGISTERS * RV64_VECTOR_REGISTER_BYTES;
    const X5_OFFSET: usize = 5 * 8;
    const PC_OFFSET: usize = 32 * 8;
    const VECTOR_BASE_OFFSET: usize = RV64_INTEGER_AND_PC_REGISTERS * 8
        + RV64_FLOAT_REGISTERS * 8
        + RV64_FLOAT_CSR_AND_PLACEHOLDER_REGISTERS * 4
        + RV64_CSR_REGISTERS * 8;
    const VECTOR_CONFIG_BASE_OFFSET: usize = VECTOR_BASE_OFFSET
        + RV64_VECTOR_REGISTERS * RV64_VECTOR_REGISTER_BYTES
        + RV64_SPARSE_CSR_REGISTERS_BEFORE_VECTOR_CONFIG * 8;
    const SENVCFG_OFFSET: usize = VECTOR_CONFIG_BASE_OFFSET + 3 * 8;

    let mut registers = vec![0; RV64_REGISTER_BYTES];
    registers[X5_OFFSET..X5_OFFSET + 8].copy_from_slice(&x5.to_le_bytes());
    registers[PC_OFFSET..PC_OFFSET + 8].copy_from_slice(&pc.to_le_bytes());
    registers[VECTOR_CONFIG_BASE_OFFSET..VECTOR_CONFIG_BASE_OFFSET + 8]
        .copy_from_slice(&9_u64.to_le_bytes());
    registers[VECTOR_CONFIG_BASE_OFFSET + 8..VECTOR_CONFIG_BASE_OFFSET + 16]
        .copy_from_slice(&0xc8_u64.to_le_bytes());
    registers[SENVCFG_OFFSET..SENVCFG_OFFSET + 8].copy_from_slice(&0x33_u64.to_le_bytes());
    for (index, byte) in registers[VECTOR_BASE_OFFSET..VECTOR_BASE_OFFSET + 16]
        .iter_mut()
        .enumerate()
    {
        *byte = 0x40 + index as u8;
    }

    let mut payload = Vec::with_capacity(1 + registers.len() * 2);
    payload.push(b'G');
    for byte in registers {
        payload.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    payload
}
