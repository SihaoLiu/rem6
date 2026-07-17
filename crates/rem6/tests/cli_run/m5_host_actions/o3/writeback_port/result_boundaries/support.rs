#[derive(Clone, Copy)]
enum PmpDeniedAmoGdbControl {
    Detach,
    Continue,
}

fn pmp_denied_amo_output(
    path: &std::path::Path,
    max_tick: u64,
    extra_args: &[&str],
    control: PmpDeniedAmoGdbControl,
    label: &str,
) -> std::process::Output {
    let listen = crate::gdb_support::unused_loopback_addr();
    let listen_text = listen.to_string();
    let mut command = result_boundary_command(path, max_tick);
    command.args(["--gdb-listen", listen_text.as_str()]);
    command.args(extra_args);
    let child = command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("failed to spawn {label}: {error}"));
    let mut stream = match crate::gdb_support::connect_with_retry(
        listen.address(),
        std::time::Duration::from_secs(3),
    ) {
        Ok(stream) => stream,
        Err(error) => {
            let output = crate::gdb_support::wait_with_output_timeout(
                child,
                std::time::Duration::from_secs(1),
            );
            panic!(
                "failed to connect to {label} GDB listener: {error}; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    };
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();

    assert_eq!(
        crate::gdb_support::send_gdb_packet(&mut stream, b"?"),
        crate::gdb_support::gdb_response(b"S05")
    );
    // Locked TOR entries allow code below 0x800000c0 and make the AMO range read-only.
    for packet in [
        b"P89=3000002000000000".as_slice(),
        b"P8c=3200002000000000".as_slice(),
        b"P88=8f89000000000000".as_slice(),
    ] {
        assert_eq!(
            crate::gdb_support::send_gdb_packet(&mut stream, packet),
            crate::gdb_support::gdb_response(b"OK"),
            "{label} PMP setup rejected {}",
            String::from_utf8_lossy(packet)
        );
    }
    match control {
        PmpDeniedAmoGdbControl::Detach => assert_eq!(
            crate::gdb_support::send_gdb_packet(&mut stream, b"D"),
            crate::gdb_support::gdb_response(b"OK")
        ),
        PmpDeniedAmoGdbControl::Continue => {
            std::io::Write::write_all(&mut stream, &crate::gdb_support::gdb_packet(b"c")).unwrap();
            crate::gdb_support::read_gdb_ack(&mut stream);
        }
    }
    drop(stream);
    crate::gdb_support::wait_with_output_timeout(child, std::time::Duration::from_secs(30))
}

fn assert_denied_amo_failure_diagnostics(stderr: &str) {
    let stderr = stderr
        .strip_suffix('\n')
        .unwrap_or_else(|| panic!("denied AMO stderr must end in one newline: {stderr:?}"));
    let (display, diagnostic) = stderr
        .split_once('\n')
        .unwrap_or_else(|| panic!("denied AMO stderr must contain exactly two lines: {stderr:?}"));
    assert!(
        !diagnostic.contains('\n'),
        "extra denied AMO stderr lines: {stderr:?}"
    );
    assert_eq!(
        display,
        "failed to execute run: CPU 0 action failed: data PMP check for fetch response 4 from agent 0 failed: RISC-V PMP denied Write access at 0x800000c0 with 8 byte(s) for Machine mode at entry Some(1)"
    );
    let diagnostic: Value = serde_json::from_str(diagnostic)
        .unwrap_or_else(|error| panic!("denied AMO diagnostic JSON is invalid: {error}"));
    assert_eq!(
        diagnostic.pointer("/schema").and_then(Value::as_str),
        Some("rem6.cli.riscv_data_pmp_failure.v1")
    );
    assert_eq!(json_u64(&diagnostic, "/completed_cpu_data_events"), 0);
    assert_eq!(
        json_u64(&diagnostic, "/data_channel_request_sent_events"),
        0
    );
    assert_eq!(json_u64(&diagnostic, "/cores/0/cpu"), 0);
    for field in ["rob_entries", "lsq_entries", "writeback_reservations"] {
        assert_eq!(
            json_u64(&diagnostic, &format!("/cores/0/{field}")),
            0,
            "denied AMO diagnostic {field}: {diagnostic}"
        );
    }
    assert_eq!(
        diagnostic
            .pointer("/memory_dumps/0/address")
            .and_then(Value::as_str),
        Some("0x800000c0")
    );
    assert_eq!(json_u64(&diagnostic, "/memory_dumps/0/bytes"), 8);
    assert_eq!(
        diagnostic
            .pointer("/memory_dumps/0/hex")
            .and_then(Value::as_str),
        Some("0900000000000000")
    );
    assert!(
        diagnostic
            .pointer("/capture_errors")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "denied AMO diagnostic capture failed: {diagnostic}"
    );
}
