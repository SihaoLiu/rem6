use super::*;

#[test]
fn rem6_run_gdb_listen_writes_trap_csrs_before_execution() {
    let program = riscv64_program(&[
        csr_read(0x105, 5),  // csrr x5, stvec
        csr_read(0x141, 6),  // csrr x6, sepc
        csr_read(0x142, 7),  // csrr x7, scause
        csr_read(0x143, 28), // csrr x28, stval
        csr_read(0x305, 29), // csrr x29, mtvec
        csr_read(0x341, 30), // csrr x30, mepc
        csr_read(0x342, 31), // csrr x31, mcause
        csr_read(0x343, 8),  // csrr x8, mtval
        0x0000_0073,         // ecall
    ]);
    let (child, mut stream) = start_riscv_gdb_run("gdb-listen-trap-csrs", program, 80);

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
    ] {
        csr_description.push_str(&String::from_utf8_lossy(&send_gdb_packet(
            &mut stream,
            payload,
        )));
    }
    for register in [
        "stvec", "sepc", "scause", "stval", "mtvec", "mepc", "mcause", "mtval",
    ] {
        assert!(
            csr_description.contains(register),
            "missing {register} in {csr_description}"
        );
    }

    for (packet, response) in [
        (b"P47=0070665544332211".as_slice(), b"OK".as_slice()),
        (b"P49=1071665544332211".as_slice(), b"OK".as_slice()),
        (b"P4a=9900000000000000".as_slice(), b"OK".as_slice()),
        (b"P4b=3073665544332211".as_slice(), b"OK".as_slice()),
        (b"P51=0080776655443322".as_slice(), b"OK".as_slice()),
        (b"P53=1081776655443322".as_slice(), b"OK".as_slice()),
        (b"P54=aa00000000000000".as_slice(), b"OK".as_slice()),
        (b"P55=3083776655443322".as_slice(), b"OK".as_slice()),
        (b"p47".as_slice(), b"0070665544332211".as_slice()),
        (b"p49".as_slice(), b"1071665544332211".as_slice()),
        (b"p4a".as_slice(), b"9900000000000000".as_slice()),
        (b"p4b".as_slice(), b"3073665544332211".as_slice()),
        (b"p51".as_slice(), b"0080776655443322".as_slice()),
        (b"p53".as_slice(), b"1081776655443322".as_slice()),
        (b"p54".as_slice(), b"aa00000000000000".as_slice()),
        (b"p55".as_slice(), b"3083776655443322".as_slice()),
    ] {
        assert_eq!(send_gdb_packet(&mut stream, packet), gdb_response(response));
    }

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
    for expected in [
        "\"x5\":\"0x1122334455667000\"",
        "\"x6\":\"0x1122334455667110\"",
        "\"x7\":\"0x99\"",
        "\"x8\":\"0x2233445566778330\"",
        "\"x28\":\"0x1122334455667330\"",
        "\"x29\":\"0x2233445566778000\"",
        "\"x30\":\"0x2233445566778110\"",
        "\"x31\":\"0xaa\"",
    ] {
        assert!(stdout.contains(expected), "missing {expected} in {stdout}");
    }
}
