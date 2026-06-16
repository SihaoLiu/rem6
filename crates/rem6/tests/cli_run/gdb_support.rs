use std::{
    fmt,
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    process::Output,
    sync::{Mutex, MutexGuard, OnceLock},
    thread,
    time::{Duration, Instant},
};

pub(crate) struct ReservedLoopbackAddr {
    address: SocketAddr,
    _guard: MutexGuard<'static, ()>,
}

impl ReservedLoopbackAddr {
    pub(crate) fn address(&self) -> SocketAddr {
        self.address
    }
}

impl fmt::Display for ReservedLoopbackAddr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.address.fmt(formatter)
    }
}

pub(crate) fn unused_loopback_addr() -> ReservedLoopbackAddr {
    static GDB_LISTEN_ADDR_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let guard = GDB_LISTEN_ADDR_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    ReservedLoopbackAddr {
        address,
        _guard: guard,
    }
}

pub(crate) fn connect_with_retry(
    address: SocketAddr,
    timeout: Duration,
) -> std::io::Result<TcpStream> {
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

pub(crate) fn send_gdb_packet(stream: &mut TcpStream, payload: &[u8]) -> Vec<u8> {
    stream.write_all(&gdb_packet(payload)).unwrap();
    read_gdb_response(stream)
}

pub(crate) fn read_gdb_ack(stream: &mut TcpStream) {
    let mut byte = [0; 1];
    stream.read_exact(&mut byte).unwrap();
    assert_eq!(byte[0], b'+');
}

pub(crate) fn send_gdb_packets(stream: &mut TcpStream, payloads: &[&[u8]]) {
    let mut packets = Vec::new();
    for payload in payloads {
        packets.extend_from_slice(&gdb_packet(payload));
    }
    stream.write_all(&packets).unwrap();
}

pub(crate) fn gdb_response(payload: &[u8]) -> Vec<u8> {
    let mut response = Vec::with_capacity(payload.len() + 5);
    response.push(b'+');
    response.extend_from_slice(&gdb_packet(payload));
    response
}

pub(crate) fn gdb_packet(payload: &[u8]) -> Vec<u8> {
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

pub(crate) fn read_gdb_response(stream: &mut TcpStream) -> Vec<u8> {
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

pub(crate) fn wait_with_output_timeout(
    mut child: std::process::Child,
    timeout: Duration,
) -> Output {
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
