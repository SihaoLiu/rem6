use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};

use rem6_cpu::{CpuId, RiscvCluster, RiscvCoreDriveAction};
use rem6_debug::{
    parse_gdb_remote_frame, GdbRemoteCommand, GdbRemoteControlState, GdbRemoteError,
    GdbRemoteFrame, GdbRemotePacket, GdbRemoteResumeKind, GdbRemoteSession, GdbRemoteTrapKind,
};
use rem6_isa_riscv::RiscvGdbXlen;
use rem6_kernel::PartitionedScheduler;
use rem6_memory::PartitionedMemoryStore;
use rem6_system::{
    handle_riscv_gdb_remote_system_packet, riscv_gdb_remote_session_from_cluster, GuestEventId,
    RiscvSystemRun, RiscvSystemRunDriver,
};
use rem6_transport::{MemoryTrace, MemoryTransport};

use crate::data_cache_runtime::CliDataCacheRuntime;
use crate::runtime_memory::CliMemoryRuntime;
use crate::{cli_data_memory_response, execute_error, Rem6CliError, Rem6RunConfig, RequestedIsa};

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

#[allow(clippy::too_many_arguments)]
pub(super) fn serve_riscv_gdb_with_single_step(
    listen: &str,
    cluster: &RiscvCluster,
    memory: &CliMemoryRuntime,
    driver: &RiscvSystemRunDriver,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    instruction_cache: Option<CliDataCacheRuntime>,
    data_cache: Option<CliDataCacheRuntime>,
    fetch_trace: MemoryTrace,
    data_trace: MemoryTrace,
    tick_limit: u64,
    instruction_budget: Option<u64>,
) -> Result<RiscvGdbServeOutcome, Rem6CliError> {
    let gdb_memory = memory.clone();
    serve_riscv_gdb_once(listen, cluster, memory, instruction_budget, || {
        driver
            .drive_until_host_stop_or_instruction_limit_parallel(
                cluster,
                scheduler,
                transport,
                fetch_trace.clone(),
                data_trace.clone(),
                |_cpu: CpuId| {
                    let memory = gdb_memory.clone();
                    let instruction_cache = instruction_cache.clone();
                    move |delivery, _context| {
                        cli_data_memory_response(instruction_cache.as_ref(), &memory, &delivery)
                    }
                },
                |_cpu: CpuId| {
                    let memory = gdb_memory.clone();
                    let data_cache = data_cache.clone();
                    move |delivery, _context| {
                        cli_data_memory_response(data_cache.as_ref(), &memory, &delivery)
                    }
                },
                tick_limit,
                1,
                |cpu| GuestEventId::new(u64::from(cpu.get())),
            )
            .map(|run| {
                let retired_by_cpu = riscv_run_retired_instructions_by_cpu(&run);
                let retired = retired_by_cpu.values().sum::<u64>();
                if retired == 1 {
                    RiscvGdbSingleStepOutcome::InstructionRetired { retired_by_cpu }
                } else {
                    RiscvGdbSingleStepOutcome::NoInstructionRetired
                }
            })
            .map_err(execute_error)
    })
}

pub(super) fn serve_riscv_gdb_once<S>(
    listen: &str,
    cluster: &RiscvCluster,
    memory: &CliMemoryRuntime,
    instruction_budget: Option<u64>,
    mut single_step: S,
) -> Result<RiscvGdbServeOutcome, Rem6CliError>
where
    S: FnMut() -> Result<RiscvGdbSingleStepOutcome, Rem6CliError>,
{
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
    let mut should_read = true;
    let mut outcome = RiscvGdbServeOutcome::default();

    loop {
        if should_read {
            let read = stream
                .read(&mut buffer)
                .map_err(|error| execute_error(format!("failed to read GDB packet: {error}")))?;
            if read == 0 {
                return Ok(outcome);
            }
            pending.extend_from_slice(&buffer[..read]);
        }
        let mut control = RiscvGdbRunControl::None;
        let consumed = memory
            .with_store_mut(|store| {
                process_gdb_bytes(
                    &mut session,
                    cluster,
                    store,
                    &pending,
                    &mut stream,
                    &mut control,
                )
            })
            .ok_or_else(|| execute_error("--gdb-listen requires store-backed memory"))??;
        pending.drain(..consumed);
        if control == RiscvGdbRunControl::SingleStep {
            session.set_stop_reply(rem6_debug::GdbRemoteStopReply::signal(0x05));
            let step = if instruction_budget
                .is_some_and(|budget| outcome.retired_instruction_count() >= budget)
            {
                RiscvGdbSingleStepOutcome::NoInstructionRetired
            } else {
                single_step()?
            };
            let frames = session
                .async_response_with_payload(b"S05".to_vec())
                .map_err(|error| {
                    execute_error(format!("failed to build GDB step response: {error}"))
                })?;
            write_gdb_frames(&mut stream, &frames)?;
            if let RiscvGdbSingleStepOutcome::InstructionRetired { retired_by_cpu } = step {
                outcome.record_retired_by_cpu(retired_by_cpu);
            }
        }
        if session.is_disconnected() {
            return Ok(outcome);
        }
        should_read = pending.is_empty() || consumed == 0;
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvGdbServeOutcome {
    retired_by_cpu: BTreeMap<CpuId, u64>,
}

impl RiscvGdbServeOutcome {
    pub(super) fn retired_by_cpu(&self) -> &BTreeMap<CpuId, u64> {
        &self.retired_by_cpu
    }

    pub(super) fn retired_instruction_count(&self) -> u64 {
        self.retired_by_cpu.values().sum()
    }

    fn record_retired_by_cpu(&mut self, retired_by_cpu: BTreeMap<CpuId, u64>) {
        for (cpu, retired) in retired_by_cpu {
            *self.retired_by_cpu.entry(cpu).or_insert(0) += retired;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGdbSingleStepOutcome {
    InstructionRetired {
        retired_by_cpu: BTreeMap<CpuId, u64>,
    },
    NoInstructionRetired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGdbRunControl {
    None,
    SingleStep,
}

fn process_gdb_bytes(
    session: &mut GdbRemoteSession,
    cluster: &RiscvCluster,
    memory: &mut PartitionedMemoryStore,
    pending: &[u8],
    stream: &mut impl Write,
    control: &mut RiscvGdbRunControl,
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
        if matches!(
            session.control_state(),
            GdbRemoteControlState::SingleInstruction { .. }
        ) {
            *control = RiscvGdbRunControl::SingleStep;
            break;
        }
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
    match GdbRemoteCommand::parse(packet) {
        GdbRemoteCommand::Resume {
            kind,
            signal,
            address,
        } => {
            kind != GdbRemoteResumeKind::SingleInstruction || signal.is_some() || address.is_some()
        }
        GdbRemoteCommand::ResumeActions { .. } => true,
        GdbRemoteCommand::Trap { request } => {
            request.point().kind() != GdbRemoteTrapKind::SoftwareBreakpoint
        }
        _ => false,
    }
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

fn riscv_run_retired_instructions_by_cpu(run: &RiscvSystemRun) -> BTreeMap<CpuId, u64> {
    let mut retired_by_cpu = BTreeMap::new();
    for event in run.turns().iter().flat_map(|turn| turn.core_events()) {
        let RiscvCoreDriveAction::InstructionExecuted(execution) = event.action() else {
            continue;
        };
        if execution.counts_as_retired_instruction() {
            *retired_by_cpu.entry(event.cpu()).or_insert(0) += 1;
        }
    }
    retired_by_cpu
}
