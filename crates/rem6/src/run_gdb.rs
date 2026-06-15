use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};

use rem6_cpu::RiscvCluster;
use rem6_debug::{
    parse_gdb_remote_frame, GdbRemoteCommand, GdbRemoteError, GdbRemoteFrame, GdbRemotePacket,
    GdbRemoteSession,
};
use rem6_isa_riscv::RiscvGdbXlen;
use rem6_memory::PartitionedMemoryStore;
use rem6_system::{handle_riscv_gdb_remote_system_packet, riscv_gdb_remote_session_from_cluster};

use crate::runtime_memory::CliMemoryRuntime;
use crate::{execute_error, Rem6CliError, Rem6RunConfig, RequestedIsa};

pub(super) fn validate_run_gdb_listen_config(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if !config.execute() {
        return Err(execute_error("--gdb-listen requires --execute"));
    }
    if config.isa() != RequestedIsa::Riscv {
        return Err(execute_error("--gdb-listen requires --isa riscv"));
    }
    if config.cores() != 1 {
        return Err(execute_error(format!(
            "--gdb-listen requires --cores 1, got {}",
            config.cores()
        )));
    }
    if config.dram_memory() {
        return Err(execute_error(
            "--gdb-listen does not yet support --dram-memory",
        ));
    }
    if config.data_cache_protocol().is_some() || config.instruction_cache_protocol().is_some() {
        return Err(execute_error(
            "--gdb-listen does not yet support cache protocol runtime options",
        ));
    }
    let _ = parse_loopback_gdb_listen_addr(
        config
            .gdb_listen()
            .expect("GDB listen config was checked before validation"),
    )?;
    Ok(())
}

pub(super) fn serve_riscv_gdb_once(
    listen: &str,
    cluster: &RiscvCluster,
    memory: &CliMemoryRuntime,
) -> Result<(), Rem6CliError> {
    let listen = parse_loopback_gdb_listen_addr(listen)?;
    let Some(mut session) = riscv_gdb_remote_session_from_cluster(RiscvGdbXlen::Rv64, cluster)
    else {
        return Err(execute_error(
            "RISC-V GDB listener requires at least one hart",
        ));
    };
    let listener = TcpListener::bind(listen)
        .map_err(|error| execute_error(format!("failed to bind GDB listener {listen}: {error}")))?;
    let (mut stream, _) = listener.accept().map_err(|error| {
        execute_error(format!(
            "failed to accept GDB connection on {listen}: {error}"
        ))
    })?;
    let mut pending = Vec::new();
    let mut buffer = [0; 1024];

    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| execute_error(format!("failed to read GDB packet: {error}")))?;
        if read == 0 {
            return Ok(());
        }
        pending.extend_from_slice(&buffer[..read]);
        let consumed = memory
            .with_store_mut(|store| {
                process_gdb_bytes(&mut session, cluster, store, &pending, &mut stream)
            })
            .ok_or_else(|| execute_error("--gdb-listen requires store-backed memory"))??;
        pending.drain(..consumed);
        if session.is_disconnected() {
            return Ok(());
        }
    }
}

fn process_gdb_bytes(
    session: &mut GdbRemoteSession,
    cluster: &RiscvCluster,
    memory: &mut PartitionedMemoryStore,
    pending: &[u8],
    stream: &mut impl Write,
) -> Result<usize, Rem6CliError> {
    let mut consumed = 0;
    while consumed < pending.len() {
        let parsed = match parse_gdb_remote_frame(&pending[consumed..]) {
            Ok(Some(parsed)) => parsed,
            Ok(None)
            | Err(GdbRemoteError::MissingChecksumSeparator | GdbRemoteError::ShortChecksum) => {
                break;
            }
            Err(error) => return Err(execute_error(format!("invalid GDB packet: {error}"))),
        };
        consumed += parsed.consumed_bytes();
        let frames = match parsed.frame() {
            GdbRemoteFrame::Packet(packet) => {
                if rejects_preexecution_gdb_command(packet) {
                    session
                        .respond_with_payload(b"E22".to_vec())
                        .map_err(|error| {
                            execute_error(format!("failed to reject GDB packet: {error}"))
                        })?
                } else {
                    handle_riscv_gdb_remote_system_packet(
                        RiscvGdbXlen::Rv64,
                        session,
                        cluster,
                        memory,
                        packet,
                    )
                    .map_err(|error| {
                        execute_error(format!("failed to handle GDB packet: {error}"))
                    })?
                }
            }
            frame => session
                .handle_frame(frame)
                .map_err(|error| execute_error(format!("failed to handle GDB frame: {error}")))?,
        };
        write_gdb_frames(stream, &frames)?;
    }
    Ok(consumed)
}

fn parse_loopback_gdb_listen_addr(listen: &str) -> Result<SocketAddr, Rem6CliError> {
    let address = listen.parse::<SocketAddr>().map_err(|_| {
        execute_error(
            "--gdb-listen requires an explicit loopback address of the form 127.0.0.1:port or [::1]:port",
        )
    })?;
    if !address.ip().is_loopback() {
        return Err(execute_error(
            "--gdb-listen requires an explicit loopback address",
        ));
    }
    Ok(address)
}

fn rejects_preexecution_gdb_command(packet: &GdbRemotePacket) -> bool {
    matches!(
        GdbRemoteCommand::parse(packet),
        GdbRemoteCommand::Resume { .. }
            | GdbRemoteCommand::ResumeActions { .. }
            | GdbRemoteCommand::Trap { .. }
            | GdbRemoteCommand::WriteMemory { .. }
            | GdbRemoteCommand::WriteRegister { .. }
            | GdbRemoteCommand::WriteRegisters { .. }
    )
}

fn write_gdb_frames(
    stream: &mut impl Write,
    frames: &[GdbRemoteFrame],
) -> Result<(), Rem6CliError> {
    for frame in frames {
        match frame {
            GdbRemoteFrame::Ack => stream
                .write_all(b"+")
                .map_err(|error| execute_error(format!("failed to write GDB ack: {error}")))?,
            GdbRemoteFrame::NegativeAck => stream
                .write_all(b"-")
                .map_err(|error| execute_error(format!("failed to write GDB nack: {error}")))?,
            GdbRemoteFrame::Interrupt => stream.write_all(&[0x03]).map_err(|error| {
                execute_error(format!("failed to write GDB interrupt: {error}"))
            })?,
            GdbRemoteFrame::Packet(packet) => stream
                .write_all(&packet.encode_frame())
                .map_err(|error| execute_error(format!("failed to write GDB packet: {error}")))?,
            GdbRemoteFrame::Notification(notification) => {
                let mut frame = Vec::with_capacity(notification.data().len() + 4);
                frame.push(b'%');
                frame.extend_from_slice(notification.data());
                frame.push(b'#');
                frame.extend_from_slice(format!("{:02x}", notification.checksum()).as_bytes());
                stream.write_all(&frame).map_err(|error| {
                    execute_error(format!("failed to write GDB notification: {error}"))
                })?;
            }
        }
    }
    stream
        .flush()
        .map_err(|error| execute_error(format!("failed to flush GDB stream: {error}")))
}
