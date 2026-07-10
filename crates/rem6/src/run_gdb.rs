use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};

use rem6_cpu::{
    CpuId, RiscvCluster, RiscvClusterTurn, RiscvDataAccessEvent, RiscvDataAccessEventKind,
};
use rem6_debug::{
    parse_gdb_remote_frame, GdbRemoteCommand, GdbRemoteControlState, GdbRemoteError,
    GdbRemoteFrame, GdbRemotePacket, GdbRemoteResumeKind, GdbRemoteResumeRequest, GdbRemoteSession,
    GdbRemoteStopReply, GdbRemoteThreadId, GdbRemoteTrapKind, GdbRemoteTrapPoint,
};
use rem6_isa_riscv::RiscvGdbXlen;
use rem6_kernel::PartitionedScheduler;
use rem6_memory::{MemoryOperation, PartitionedMemoryStore};
use rem6_system::{
    handle_riscv_gdb_remote_system_packet, riscv_gdb_remote_session_from_cluster, GuestEventId,
    RiscvGdbRemotePacketError, RiscvSystemRun, RiscvSystemRunDriver,
};
use rem6_transport::{MemoryTrace, MemoryTransport};

use crate::data_cache_runtime::{
    cli_data_memory_response, invalidate_cli_cache_hierarchies, CliCacheHierarchy,
};
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
    if !config.readfiles().is_empty() {
        return Err(execute_error(
            "--gdb-listen does not yet support --readfile",
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
pub(super) fn serve_riscv_gdb_with_run_control(
    xlen: RiscvGdbXlen,
    listen: &str,
    cluster: &RiscvCluster,
    memory: &CliMemoryRuntime,
    driver: &RiscvSystemRunDriver,
    scheduler: &mut PartitionedScheduler,
    transport: &MemoryTransport,
    instruction_cache: CliCacheHierarchy,
    data_cache: CliCacheHierarchy,
    fetch_trace: MemoryTrace,
    data_trace: MemoryTrace,
    tick_limit: u64,
    instruction_budget: Option<u64>,
) -> Result<RiscvGdbServeOutcome, Rem6CliError> {
    let gdb_memory = memory.clone();
    let gdb_instruction_cache = instruction_cache.clone();
    let gdb_data_cache = data_cache.clone();
    serve_riscv_gdb_once(
        xlen,
        listen,
        cluster,
        memory,
        gdb_instruction_cache,
        gdb_data_cache,
        instruction_budget,
        |max_instructions, debug_stop| {
            if let Some(debug_stop) = debug_stop {
                return driver
                    .drive_until_host_stop_or_instruction_limit_parallel_with_debug_stop(
                        cluster,
                        scheduler,
                        transport,
                        fetch_trace.clone(),
                        data_trace.clone(),
                        |_cpu: CpuId| {
                            let memory = gdb_memory.clone();
                            let instruction_cache = instruction_cache.clone();
                            move |delivery, _context| {
                                cli_data_memory_response(&instruction_cache, &memory, &delivery)
                            }
                        },
                        |_cpu: CpuId| {
                            let memory = gdb_memory.clone();
                            let data_cache = data_cache.clone();
                            move |delivery, _context| {
                                cli_data_memory_response(&data_cache, &memory, &delivery)
                            }
                        },
                        tick_limit,
                        max_instructions.unwrap_or(u64::MAX),
                        |cpu| GuestEventId::new(u64::from(cpu.get())),
                        debug_stop,
                    )
                    .map_err(execute_error);
            }
            match max_instructions {
                Some(max_instructions) => driver
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
                                cli_data_memory_response(&instruction_cache, &memory, &delivery)
                            }
                        },
                        |_cpu: CpuId| {
                            let memory = gdb_memory.clone();
                            let data_cache = data_cache.clone();
                            move |delivery, _context| {
                                cli_data_memory_response(&data_cache, &memory, &delivery)
                            }
                        },
                        tick_limit,
                        max_instructions,
                        |cpu| GuestEventId::new(u64::from(cpu.get())),
                    )
                    .map(|run| (run, false))
                    .map_err(execute_error),
                None => driver
                    .drive_until_host_stop_or_tick_limit_parallel(
                        cluster,
                        scheduler,
                        transport,
                        fetch_trace.clone(),
                        data_trace.clone(),
                        |_cpu: CpuId| {
                            let memory = gdb_memory.clone();
                            let instruction_cache = instruction_cache.clone();
                            move |delivery, _context| {
                                cli_data_memory_response(&instruction_cache, &memory, &delivery)
                            }
                        },
                        |_cpu: CpuId| {
                            let memory = gdb_memory.clone();
                            let data_cache = data_cache.clone();
                            move |delivery, _context| {
                                cli_data_memory_response(&data_cache, &memory, &delivery)
                            }
                        },
                        tick_limit,
                        |cpu| GuestEventId::new(u64::from(cpu.get())),
                    )
                    .map(|run| (run, false))
                    .map_err(execute_error),
            }
        },
    )
}

type RiscvGdbDebugStopPredicate<'a> = dyn FnMut(&RiscvCluster, &RiscvClusterTurn) -> bool + 'a;

pub(super) fn serve_riscv_gdb_once<D>(
    xlen: RiscvGdbXlen,
    listen: &str,
    cluster: &RiscvCluster,
    memory: &CliMemoryRuntime,
    instruction_cache: CliCacheHierarchy,
    data_cache: CliCacheHierarchy,
    instruction_budget: Option<u64>,
    mut drive: D,
) -> Result<RiscvGdbServeOutcome, Rem6CliError>
where
    D: FnMut(
        Option<u64>,
        Option<&mut RiscvGdbDebugStopPredicate<'_>>,
    ) -> Result<(RiscvSystemRun, bool), Rem6CliError>,
{
    let listen = parse_loopback_gdb_listen_addr(listen)?;
    let Some(mut session) = riscv_gdb_remote_session_from_cluster(xlen, cluster) else {
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
    let mut data_access_cursor = RiscvGdbDataAccessCursor::from_cluster(cluster);

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
                let mut processor = GdbPacketProcessor {
                    xlen,
                    session: &mut session,
                    cluster,
                    memory: store,
                    stream: &mut stream,
                    instruction_cache: &instruction_cache,
                    data_cache: &data_cache,
                };
                process_gdb_bytes(
                    &mut processor,
                    &pending,
                    &mut control,
                    !outcome.has_completed_run(),
                )
            })
            .ok_or_else(|| execute_error("--gdb-listen requires store-backed memory"))??;
        pending.drain(..consumed);
        match control {
            RiscvGdbRunControl::None => {}
            RiscvGdbRunControl::SingleStep => {
                session.set_stop_reply(GdbRemoteStopReply::signal(0x05));
                let step = if instruction_budget
                    .is_some_and(|budget| outcome.gdb_retired_instruction_count() >= budget)
                    || cluster_hits_active_gdb_hardware_breakpoint(&session, cluster)
                {
                    RiscvGdbSingleStepOutcome::NoInstructionRetired
                } else if has_active_gdb_runtime_stops(&session) {
                    data_access_cursor.sync_to_cluster(cluster);
                    let mut watchpoint_stop = None;
                    let mut debug_stop = |cluster: &RiscvCluster, _turn: &RiscvClusterTurn| {
                        if cluster_hits_active_gdb_hardware_breakpoint(&session, cluster) {
                            return true;
                        }
                        if let Some(point) = cluster_hits_active_gdb_data_watchpoint(
                            &session,
                            &mut data_access_cursor,
                            cluster,
                        ) {
                            watchpoint_stop = Some(data_watchpoint_stop_reply(point));
                            return true;
                        }
                        false
                    };
                    let (run, _stopped_at_watchpoint) = drive(Some(1), Some(&mut debug_stop))?;
                    if let Some(stop) = watchpoint_stop {
                        session.set_stop_reply(stop);
                    }
                    let retired_by_cpu = run.retired_instruction_counts_by_cpu();
                    let retired = retired_by_cpu.values().sum::<u64>();
                    if retired == 1 {
                        RiscvGdbSingleStepOutcome::InstructionRetired { retired_by_cpu }
                    } else {
                        RiscvGdbSingleStepOutcome::NoInstructionRetired
                    }
                } else {
                    let (run, _debug_stop) = drive(Some(1), None)?;
                    let retired_by_cpu = run.retired_instruction_counts_by_cpu();
                    let retired = retired_by_cpu.values().sum::<u64>();
                    if retired == 1 {
                        RiscvGdbSingleStepOutcome::InstructionRetired { retired_by_cpu }
                    } else {
                        RiscvGdbSingleStepOutcome::NoInstructionRetired
                    }
                };
                write_gdb_stop_reply(&mut session, &mut stream, "step")?;
                if let RiscvGdbSingleStepOutcome::InstructionRetired { retired_by_cpu } = step {
                    outcome.record_retired_by_cpu(retired_by_cpu);
                }
            }
            RiscvGdbRunControl::Continue => {
                session.set_stop_reply(GdbRemoteStopReply::signal(0x05));
                let remaining_instructions = instruction_budget
                    .map(|budget| budget.saturating_sub(outcome.gdb_retired_instruction_count()));
                let continue_outcome =
                    if cluster_hits_active_gdb_hardware_breakpoint(&session, cluster) {
                        RiscvGdbContinueOutcome::StoppedAtTrap {
                            retired_by_cpu: BTreeMap::new(),
                        }
                    } else if has_active_gdb_runtime_stops(&session) {
                        data_access_cursor.sync_to_cluster(cluster);
                        let mut watchpoint_stop = None;
                        let mut debug_stop = |cluster: &RiscvCluster, _turn: &RiscvClusterTurn| {
                            if cluster_hits_active_gdb_hardware_breakpoint(&session, cluster) {
                                return true;
                            }
                            if let Some(point) = cluster_hits_active_gdb_data_watchpoint(
                                &session,
                                &mut data_access_cursor,
                                cluster,
                            ) {
                                watchpoint_stop = Some(data_watchpoint_stop_reply(point));
                                return true;
                            }
                            false
                        };
                        let (run, stopped_at_watchpoint) =
                            drive(remaining_instructions, Some(&mut debug_stop))?;
                        if stopped_at_watchpoint {
                            if let Some(stop) = watchpoint_stop {
                                session.set_stop_reply(stop);
                            }
                            RiscvGdbContinueOutcome::StoppedAtTrap {
                                retired_by_cpu: run.retired_instruction_counts_by_cpu(),
                            }
                        } else {
                            RiscvGdbContinueOutcome::CompletedRun { run: Box::new(run) }
                        }
                    } else {
                        RiscvGdbContinueOutcome::CompletedRun {
                            run: Box::new(drive(remaining_instructions, None)?.0),
                        }
                    };
                write_gdb_stop_reply(&mut session, &mut stream, "continue")?;
                match continue_outcome {
                    RiscvGdbContinueOutcome::StoppedAtTrap { retired_by_cpu } => {
                        outcome.record_retired_by_cpu(retired_by_cpu);
                    }
                    RiscvGdbContinueOutcome::CompletedRun { run } => {
                        outcome.set_completed_run(*run);
                    }
                }
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
    completed_run: Option<RiscvSystemRun>,
}

impl RiscvGdbServeOutcome {
    pub(super) fn retired_by_cpu(&self) -> &BTreeMap<CpuId, u64> {
        &self.retired_by_cpu
    }

    pub(super) fn retired_instruction_count(&self) -> u64 {
        self.gdb_retired_instruction_count()
    }

    pub(super) fn take_completed_run(&mut self) -> Option<RiscvSystemRun> {
        self.completed_run.take()
    }

    fn has_completed_run(&self) -> bool {
        self.completed_run.is_some()
    }

    fn gdb_retired_instruction_count(&self) -> u64 {
        self.retired_by_cpu.values().sum::<u64>() + self.completed_run_retired_instruction_count()
    }

    fn completed_run_retired_instruction_count(&self) -> u64 {
        self.completed_run
            .as_ref()
            .map(RiscvSystemRun::retired_instruction_counts_by_cpu)
            .map(|retired_by_cpu| retired_by_cpu.values().sum())
            .unwrap_or_default()
    }

    fn record_retired_by_cpu(&mut self, retired_by_cpu: BTreeMap<CpuId, u64>) {
        for (cpu, count) in retired_by_cpu {
            *self.retired_by_cpu.entry(cpu).or_insert(0) += count;
        }
    }

    fn set_completed_run(&mut self, run: RiscvSystemRun) {
        self.completed_run = Some(run);
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
    Continue,
    SingleStep,
}

struct GdbPacketProcessor<'a, W: Write> {
    xlen: RiscvGdbXlen,
    session: &'a mut GdbRemoteSession,
    cluster: &'a RiscvCluster,
    memory: &'a mut PartitionedMemoryStore,
    stream: &'a mut W,
    instruction_cache: &'a CliCacheHierarchy,
    data_cache: &'a CliCacheHierarchy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RiscvGdbContinueOutcome {
    StoppedAtTrap {
        retired_by_cpu: BTreeMap<CpuId, u64>,
    },
    CompletedRun {
        run: Box<RiscvSystemRun>,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct RiscvGdbDataAccessCursor {
    cursors: BTreeMap<CpuId, usize>,
}

impl RiscvGdbDataAccessCursor {
    fn from_cluster(cluster: &RiscvCluster) -> Self {
        let mut cursor = Self::default();
        cursor.sync_to_cluster(cluster);
        cursor
    }

    fn sync_to_cluster(&mut self, cluster: &RiscvCluster) {
        let mut cursors = BTreeMap::new();
        for cpu in cluster.core_ids() {
            let events = cluster
                .core(cpu)
                .expect("cluster core id exists")
                .data_access_events();
            cursors.insert(cpu, events.len());
        }
        self.cursors = cursors;
    }

    fn take_new_events(&mut self, cluster: &RiscvCluster) -> Vec<(CpuId, RiscvDataAccessEvent)> {
        let mut new_events = Vec::new();
        for cpu in cluster.core_ids() {
            let events = cluster
                .core(cpu)
                .expect("cluster core id exists")
                .data_access_events();
            let cursor = self.cursors.entry(cpu).or_insert(0);
            new_events.extend(
                events
                    .iter()
                    .skip(*cursor)
                    .cloned()
                    .map(|event| (cpu, event)),
            );
            *cursor = events.len();
        }
        new_events
    }
}

fn process_gdb_bytes<W: Write>(
    processor: &mut GdbPacketProcessor<'_, W>,
    pending: &[u8],
    control: &mut RiscvGdbRunControl,
    resume_allowed: bool,
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
                if rejects_preexecution_gdb_command(processor.session, packet, resume_allowed) {
                    processor
                        .session
                        .respond_with_payload(b"E22".to_vec())
                        .map_err(|error| {
                            execute_error(format!("failed to reject GDB packet: {error}"))
                        })?
                } else {
                    match handle_riscv_gdb_remote_system_packet(
                        processor.xlen,
                        processor.session,
                        processor.cluster,
                        processor.memory,
                        packet,
                    ) {
                        Ok(frames) => frames,
                        Err(RiscvGdbRemotePacketError::RegisterWrite(_)) => processor
                            .session
                            .respond_with_payload(b"E01".to_vec())
                            .map_err(|error| {
                                execute_error(format!("failed to reject GDB packet: {error}"))
                            })?,
                        Err(error) => {
                            return Err(execute_error(format!(
                                "failed to handle GDB packet: {error}"
                            )));
                        }
                    }
                }
            }
            frame => processor
                .session
                .handle_frame(frame)
                .map_err(|error| execute_error(format!("failed to handle GDB frame: {error}")))?,
        };
        if let GdbRemoteFrame::Packet(packet) = parsed.frame() {
            if gdb_packet_may_mutate_memory(packet)
                && !invalidate_cli_cache_hierarchies(
                    processor.instruction_cache,
                    processor.data_cache,
                )
            {
                return Err(execute_error(
                    "failed to invalidate CLI caches after GDB memory mutation",
                ));
            }
        }
        write_gdb_frames(&mut processor.stream, &frames)?;
        match processor.session.control_state() {
            GdbRemoteControlState::SingleInstruction { .. } => {
                *control = RiscvGdbRunControl::SingleStep;
                break;
            }
            GdbRemoteControlState::Continue { .. } => {
                *control = RiscvGdbRunControl::Continue;
                break;
            }
            GdbRemoteControlState::Stopped
            | GdbRemoteControlState::Interrupted
            | GdbRemoteControlState::Disconnected => {}
        }
    }
    Ok(consumed)
}

fn gdb_packet_may_mutate_memory(packet: &GdbRemotePacket) -> bool {
    match GdbRemoteCommand::parse(packet) {
        GdbRemoteCommand::WriteMemory { .. } => true,
        GdbRemoteCommand::Trap { request } => {
            request.point().kind() == GdbRemoteTrapKind::SoftwareBreakpoint
        }
        _ => false,
    }
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

fn rejects_preexecution_gdb_command(
    session: &GdbRemoteSession,
    packet: &GdbRemotePacket,
    resume_allowed: bool,
) -> bool {
    match GdbRemoteCommand::parse(packet) {
        GdbRemoteCommand::Resume {
            kind,
            signal,
            address,
        } => {
            !resume_allowed
                || !supports_preexecution_resume_requests(&[GdbRemoteResumeRequest::new(
                    kind,
                    signal,
                    address,
                    session.continue_thread(),
                )])
        }
        GdbRemoteCommand::ResumeActions { requests } => {
            !resume_allowed || !supports_preexecution_resume_requests(&requests)
        }
        GdbRemoteCommand::Trap { request } => !matches!(
            request.point().kind(),
            GdbRemoteTrapKind::SoftwareBreakpoint
                | GdbRemoteTrapKind::HardwareBreakpoint
                | GdbRemoteTrapKind::WriteWatchpoint
                | GdbRemoteTrapKind::ReadWatchpoint
                | GdbRemoteTrapKind::AccessWatchpoint
        ),
        _ => false,
    }
}

fn supports_preexecution_resume_requests(requests: &[GdbRemoteResumeRequest]) -> bool {
    matches!(requests, [request] if supports_preexecution_resume_request(request))
}

fn supports_preexecution_resume_request(request: &GdbRemoteResumeRequest) -> bool {
    matches!(
        request.kind(),
        GdbRemoteResumeKind::Continue | GdbRemoteResumeKind::SingleInstruction
    ) && request.signal().is_none()
        && request.address().is_none()
        && matches!(
            request.thread(),
            GdbRemoteThreadId::All | GdbRemoteThreadId::Any
        )
}

fn has_active_gdb_runtime_stops(session: &GdbRemoteSession) -> bool {
    has_active_gdb_hardware_breakpoints(session) || has_active_gdb_data_watchpoints(session)
}

fn has_active_gdb_hardware_breakpoints(session: &GdbRemoteSession) -> bool {
    session
        .active_traps()
        .iter()
        .any(|point| point.kind() == GdbRemoteTrapKind::HardwareBreakpoint)
}

fn cluster_hits_active_gdb_hardware_breakpoint(
    session: &GdbRemoteSession,
    cluster: &RiscvCluster,
) -> bool {
    let active_traps = session.active_traps();
    cluster.core_ids().into_iter().any(|cpu| {
        let pc = cluster
            .core(cpu)
            .expect("cluster core id exists")
            .pc()
            .get();
        active_traps.iter().any(|point| {
            point.kind() == GdbRemoteTrapKind::HardwareBreakpoint
                && range_overlaps(pc, 1, point.address(), point.size())
        })
    })
}

fn has_active_gdb_data_watchpoints(session: &GdbRemoteSession) -> bool {
    session.active_traps().iter().any(|point| {
        matches!(
            point.kind(),
            GdbRemoteTrapKind::WriteWatchpoint
                | GdbRemoteTrapKind::ReadWatchpoint
                | GdbRemoteTrapKind::AccessWatchpoint
        )
    })
}

fn cluster_hits_active_gdb_data_watchpoint(
    session: &GdbRemoteSession,
    data_access_cursor: &mut RiscvGdbDataAccessCursor,
    cluster: &RiscvCluster,
) -> Option<GdbRemoteTrapPoint> {
    let active_traps = session.active_traps();
    let data_events = data_access_cursor.take_new_events(cluster);
    data_events
        .iter()
        .find_map(|(_, event)| data_event_hits_active_data_watchpoint(event, active_traps))
}

fn data_event_hits_active_data_watchpoint(
    event: &RiscvDataAccessEvent,
    active_traps: &[rem6_debug::GdbRemoteTrapPoint],
) -> Option<GdbRemoteTrapPoint> {
    if event.kind() != RiscvDataAccessEventKind::Completed {
        return None;
    }
    active_traps.iter().copied().find(|point| {
        data_watchpoint_kind_matches_access(point.kind(), event.operation())
            && range_overlaps(
                event.physical_address().get(),
                event.size().bytes(),
                point.address(),
                point.size(),
            )
    })
}

fn data_watchpoint_stop_reply(point: GdbRemoteTrapPoint) -> GdbRemoteStopReply {
    GdbRemoteStopReply::data_watchpoint(0x05, point.kind(), point.address())
        .expect("data watchpoint stop reply requires a data watchpoint trap kind")
}

fn write_gdb_stop_reply(
    session: &mut GdbRemoteSession,
    stream: &mut impl Write,
    command: &str,
) -> Result<(), Rem6CliError> {
    let frames = session
        .async_response_with_payload(session.stop_reply().encode_payload())
        .map_err(|error| {
            execute_error(format!("failed to build GDB {command} response: {error}"))
        })?;
    write_gdb_frames(stream, &frames)
}

fn data_watchpoint_kind_matches_access(
    kind: GdbRemoteTrapKind,
    operation: MemoryOperation,
) -> bool {
    match kind {
        GdbRemoteTrapKind::WriteWatchpoint => memory_operation_writes(operation),
        GdbRemoteTrapKind::ReadWatchpoint => memory_operation_reads(operation),
        GdbRemoteTrapKind::AccessWatchpoint => {
            memory_operation_writes(operation) || memory_operation_reads(operation)
        }
        GdbRemoteTrapKind::SoftwareBreakpoint | GdbRemoteTrapKind::HardwareBreakpoint => false,
    }
}

fn memory_operation_reads(operation: MemoryOperation) -> bool {
    matches!(
        operation,
        MemoryOperation::ReadShared
            | MemoryOperation::ReadUnique
            | MemoryOperation::LoadLocked
            | MemoryOperation::LockedRmwRead
            | MemoryOperation::Atomic
            | MemoryOperation::AtomicNoReturn
    )
}

fn memory_operation_writes(operation: MemoryOperation) -> bool {
    matches!(
        operation,
        MemoryOperation::Write
            | MemoryOperation::LockedRmwWrite
            | MemoryOperation::StoreConditional
            | MemoryOperation::Atomic
            | MemoryOperation::AtomicNoReturn
    )
}

fn range_overlaps(left_start: u64, left_size: u64, right_start: u64, right_size: u64) -> bool {
    let Some(left_end) = left_start.checked_add(left_size) else {
        return false;
    };
    let Some(right_end) = right_start.checked_add(right_size) else {
        return false;
    };
    left_start < right_end && right_start < left_end
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
            GdbRemoteFrame::NegativeAck => stream.write_all(b"-").map_err(|error| {
                execute_error(format!("failed to write GDB negative ack: {error}"))
            })?,
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
