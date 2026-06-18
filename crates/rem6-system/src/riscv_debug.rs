use rem6_cpu::{CpuId, RiscvCluster, RiscvCore};
use rem6_debug::{
    GdbRemoteCommand, GdbRemoteError, GdbRemoteFeature, GdbRemoteFeatureValue, GdbRemoteFrame,
    GdbRemotePacket, GdbRemoteRegisterBytes, GdbRemoteSession, GdbRemoteThreadId,
    GdbRemoteThreadOperation, GdbRemoteTrapKind, GdbRemoteTrapOperation, GdbRemoteTrapRequest,
    DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES,
};
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvCounterCsr, RiscvCounterSnapshot, RiscvFloatCsr,
    RiscvGdbTargetDescription, RiscvGdbXlen, RiscvHartState, RiscvInterruptCsr,
    RiscvMachineIdentityCsr, RiscvMachineIsaCsr, RiscvMachineTrapCsr, RiscvStatusCsr,
    RiscvSupervisorTrapCsr, RiscvTranslationCsr, RiscvVectorFixedPointCsr, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, MemoryError, MemoryRequest, MemoryRequestId,
    PartitionedMemoryStore, TranslationPageMap, TranslationPageMappingScope,
    TranslationPagePermissions,
};
use std::error::Error;
use std::fmt::{self, Write as _};

const RISCV_GDB_INTEGER_REGISTER_COUNT: u8 = 32;
const RISCV_GDB_PC_REGISTER: u64 = 32;
const RISCV_GDB_FLOAT_REGISTER_BASE: u64 = 33;
const RISCV_GDB_FLOAT_REGISTER_COUNT: u8 = 32;
const RISCV_GDB_FLOAT_CSR_REGISTER_BASE: u64 = 66;
const RISCV_GDB_FLOAT_CSR_REGISTER_COUNT: u8 = 3;
const RISCV_GDB_FLOAT_PLACEHOLDER_REGISTER: u64 = 69;
const RISCV_GDB_RV32_CSR_REGISTER_BASE: u64 = 70;
const RISCV_GDB_RV64_CSR_REGISTER_BASE: u64 = 70;
const RISCV_GDB_CSR_REGISTER_COUNT: u8 = 20;
const RISCV_GDB_VECTOR_REGISTER_BASE: u64 = 90;
const RISCV_GDB_VECTOR_REGISTER_COUNT: u8 = 32;
const RISCV_GDB_SUPERVISOR_INTERRUPT_ENABLE_REGISTER: u64 = 122;
const RISCV_GDB_SUPERVISOR_INTERRUPT_PENDING_REGISTER: u64 = 123;
const RISCV_GDB_COUNTER_CYCLE_REGISTER: u64 = 124;
const RISCV_GDB_COUNTER_INSTRET_REGISTER: u64 = 125;
const RISCV_GDB_COUNTER_TIME_REGISTER: u64 = 126;
const RISCV_GDB_MACHINE_HART_ID_REGISTER: u64 = 127;
const RISCV_GDB_MACHINE_VENDOR_ID_REGISTER: u64 = 128;
const RISCV_GDB_MACHINE_ARCHITECTURE_ID_REGISTER: u64 = 129;
const RISCV_GDB_MACHINE_IMPLEMENTATION_ID_REGISTER: u64 = 130;
const RISCV_GDB_MACHINE_ISA_REGISTER: u64 = 131;
const RISCV_GDB_SPARSE_CSR_REGISTER_COUNT: usize = 10;
const RISCV_GDB_MEMORY_AGENT: AgentId = AgentId::new(u32::MAX - 1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGdbCsrRegister {
    Status(RiscvStatusCsr),
    Interrupt(RiscvInterruptCsr),
    MachineTrap(RiscvMachineTrapCsr),
    SupervisorTrap(RiscvSupervisorTrapCsr),
    Translation(RiscvTranslationCsr),
    VectorFixedPoint(RiscvVectorFixedPointCsr),
    Counter(RiscvCounterCsr),
    MachineIdentity(RiscvMachineIdentityCsr),
    MachineIsa(RiscvMachineIsaCsr),
}

pub fn riscv_gdb_remote_session(xlen: RiscvGdbXlen) -> GdbRemoteSession {
    let mut session = GdbRemoteSession::new(vec![
        GdbRemoteFeature::new(
            b"PacketSize".to_vec(),
            GdbRemoteFeatureValue::Value(
                format!("{DEFAULT_GDB_REMOTE_MAX_PAYLOAD_BYTES:x}").into_bytes(),
            ),
        ),
        GdbRemoteFeature::new(
            b"qXfer:features:read".to_vec(),
            GdbRemoteFeatureValue::Supported,
        ),
        GdbRemoteFeature::new(b"vContSupported".to_vec(), GdbRemoteFeatureValue::Supported),
    ]);

    for document in RiscvGdbTargetDescription::new(xlen).into_documents() {
        let (annex, content) = document.into_parts();
        let registered = session.set_xfer_feature(annex.as_bytes().to_vec(), content);
        debug_assert!(registered);
    }

    session
}

pub fn riscv_gdb_remote_session_with_page_table_dump(
    xlen: RiscvGdbXlen,
    page_table_dump: Vec<u8>,
) -> GdbRemoteSession {
    let mut session = riscv_gdb_remote_session(xlen);
    session.set_page_table_dump(page_table_dump);
    session
}

pub fn riscv_gdb_remote_session_from_translation_map(
    xlen: RiscvGdbXlen,
    map: &TranslationPageMap,
) -> GdbRemoteSession {
    riscv_gdb_remote_session_with_page_table_dump(
        xlen,
        riscv_gdb_page_table_dump_from_translation_map(map),
    )
}

pub fn riscv_gdb_page_table_dump_from_translation_map(map: &TranslationPageMap) -> Vec<u8> {
    let mut dump = String::new();
    writeln!(dump, "page_size={:#x}", map.page_size().bytes())
        .expect("page table dump writes into string");
    for mapping in map.mappings() {
        writeln!(
            dump,
            "vaddr={:#x} paddr={:#x} pages={} flags={} scope={}",
            mapping.virtual_start().get(),
            mapping.physical_start().get(),
            mapping.page_count(),
            riscv_gdb_page_permission_flags(mapping.permissions()),
            riscv_gdb_page_mapping_scope(mapping.scope()),
        )
        .expect("page table dump writes into string");
    }
    dump.into_bytes()
}

pub fn riscv_gdb_remote_session_from_hart(
    xlen: RiscvGdbXlen,
    hart: &RiscvHartState,
) -> GdbRemoteSession {
    let mut session = riscv_gdb_remote_session(xlen);
    sync_riscv_gdb_remote_session_from_hart(xlen, &mut session, hart);
    session
}

pub fn riscv_gdb_remote_session_from_core(
    xlen: RiscvGdbXlen,
    core: &RiscvCore,
) -> GdbRemoteSession {
    riscv_gdb_remote_session_from_hart(xlen, &riscv_gdb_hart_snapshot_from_core(core))
}

pub fn riscv_gdb_remote_session_from_cluster(
    xlen: RiscvGdbXlen,
    cluster: &RiscvCluster,
) -> Option<GdbRemoteSession> {
    let first_cpu = cluster.core_ids().into_iter().next()?;
    let first_core = cluster.core(first_cpu).ok()?;
    let mut session = riscv_gdb_remote_session_from_core(xlen, &first_core);
    sync_riscv_gdb_remote_threads_from_cluster(&mut session, cluster);
    Some(session)
}

pub fn sync_riscv_gdb_remote_threads_from_cluster(
    session: &mut GdbRemoteSession,
    cluster: &RiscvCluster,
) -> bool {
    session.set_thread_ids(
        cluster
            .core_ids()
            .into_iter()
            .map(riscv_gdb_remote_thread_id)
            .collect(),
    )
}

pub fn riscv_gdb_remote_thread_id(cpu: CpuId) -> u64 {
    u64::from(cpu.get()) + 1
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvGdbRegisterWriteError {
    InvalidRegisterBytes {
        number: u64,
        expected: usize,
        actual: usize,
    },
    InvalidRegisterSetBytes {
        expected: usize,
        actual: usize,
    },
    UnsupportedRegister {
        number: u64,
    },
}

impl fmt::Display for RiscvGdbRegisterWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegisterBytes {
                number,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V GDB register {number} write has {actual} byte(s), expected {expected}"
            ),
            Self::InvalidRegisterSetBytes { expected, actual } => write!(
                formatter,
                "RISC-V GDB all-register write has {actual} byte(s), expected {expected}"
            ),
            Self::UnsupportedRegister { number } => {
                write!(formatter, "RISC-V GDB register {number} is unsupported")
            }
        }
    }
}

impl Error for RiscvGdbRegisterWriteError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvGdbRemotePacketError {
    Protocol(GdbRemoteError),
    RegisterWrite(RiscvGdbRegisterWriteError),
}

impl fmt::Display for RiscvGdbRemotePacketError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Protocol(error) => write!(formatter, "{error}"),
            Self::RegisterWrite(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvGdbRemotePacketError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::RegisterWrite(error) => Some(error),
        }
    }
}

impl From<GdbRemoteError> for RiscvGdbRemotePacketError {
    fn from(error: GdbRemoteError) -> Self {
        Self::Protocol(error)
    }
}

impl From<RiscvGdbRegisterWriteError> for RiscvGdbRemotePacketError {
    fn from(error: RiscvGdbRegisterWriteError) -> Self {
        Self::RegisterWrite(error)
    }
}

pub fn apply_riscv_gdb_remote_register_write(
    xlen: RiscvGdbXlen,
    hart: &mut RiscvHartState,
    command: &GdbRemoteCommand,
) -> Result<bool, RiscvGdbRegisterWriteError> {
    let applies = validate_riscv_gdb_remote_register_write(xlen, command)?;
    match command {
        GdbRemoteCommand::WriteRegister { number, bytes } => {
            apply_single_register_write(xlen, hart, *number, bytes)?;
        }
        GdbRemoteCommand::WriteRegisters { bytes } => {
            apply_all_register_write(xlen, hart, bytes)?;
        }
        _ => {}
    }
    Ok(applies)
}

pub fn handle_riscv_gdb_remote_packet(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    hart: &mut RiscvHartState,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    if session.is_disconnected() {
        return Ok(session.handle_packet(packet)?);
    }

    let command = GdbRemoteCommand::parse(packet);
    if reads_riscv_gdb_remote_registers(&command) {
        sync_riscv_gdb_remote_session_from_hart(xlen, session, hart);
    }

    let mut updated_hart = hart.clone();
    let applies_register_write =
        apply_riscv_gdb_remote_register_write(xlen, &mut updated_hart, &command)?;
    let frames = session.handle_packet(packet)?;
    if applies_register_write {
        *hart = updated_hart;
        sync_riscv_gdb_remote_session_from_hart(xlen, session, hart);
    }
    Ok(frames)
}

pub fn apply_riscv_gdb_remote_core_register_write(
    xlen: RiscvGdbXlen,
    core: &RiscvCore,
    command: &GdbRemoteCommand,
) -> Result<bool, RiscvGdbRegisterWriteError> {
    let applies = validate_riscv_gdb_remote_register_write(xlen, command)?;
    match command {
        GdbRemoteCommand::WriteRegister { number, bytes } => {
            write_core_register_bytes(xlen, core, *number, bytes);
        }
        GdbRemoteCommand::WriteRegisters { bytes } => {
            for_each_register_bytes(xlen, bytes, |number, bytes| {
                write_core_register_bytes(xlen, core, number, bytes);
            });
        }
        _ => {}
    }
    Ok(applies)
}

pub fn handle_riscv_gdb_remote_core_packet(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    core: &RiscvCore,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    if session.is_disconnected() {
        return Ok(session.handle_packet(packet)?);
    }

    let command = GdbRemoteCommand::parse(packet);
    if reads_riscv_gdb_remote_registers(&command) {
        sync_riscv_gdb_remote_session_from_core(xlen, session, core);
    }

    let applies_register_write = validate_riscv_gdb_remote_register_write(xlen, &command)?;
    let frames = session.handle_packet(packet)?;
    if applies_register_write {
        apply_riscv_gdb_remote_core_register_write(xlen, core, &command)?;
        sync_riscv_gdb_remote_session_from_core(xlen, session, core);
    }
    Ok(frames)
}

pub fn handle_riscv_gdb_remote_cluster_packet(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    cluster: &RiscvCluster,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    if session.is_disconnected() {
        return Ok(session.handle_packet(packet)?);
    }

    sync_riscv_gdb_remote_threads_from_cluster(session, cluster);

    let command = GdbRemoteCommand::parse(packet);
    let selected_current_thread = if let GdbRemoteCommand::SetThread { operation, thread } = command
    {
        match riscv_gdb_remote_thread_selection(operation, thread, cluster) {
            Ok(thread_id) => thread_id,
            Err(payload) => return gdb_remote_error_response_with_payload(session, payload),
        }
    } else {
        None
    };

    if reads_riscv_gdb_remote_registers(&command) {
        let Some(core) = riscv_gdb_remote_selected_core(session, cluster) else {
            return gdb_remote_error_response(session);
        };
        sync_riscv_gdb_remote_session_from_core(xlen, session, &core);
    }

    let applies_register_write = validate_riscv_gdb_remote_register_write(xlen, &command)?;
    let selected_core = if applies_register_write {
        let Some(core) = riscv_gdb_remote_selected_core(session, cluster) else {
            return gdb_remote_error_response(session);
        };
        Some(core)
    } else {
        None
    };

    let frames = session.handle_packet(packet)?;
    if let Some(thread_id) = selected_current_thread {
        debug_assert!(session.set_current_thread_id(thread_id));
    }
    if let Some(core) = selected_core {
        apply_riscv_gdb_remote_core_register_write(xlen, &core, &command)?;
        sync_riscv_gdb_remote_session_from_core(xlen, session, &core);
    }
    Ok(frames)
}

pub fn handle_riscv_gdb_remote_memory_packet(
    session: &mut GdbRemoteSession,
    memory: &mut PartitionedMemoryStore,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    if session.is_disconnected() {
        return Ok(session.handle_packet(packet)?);
    }

    let command = GdbRemoteCommand::parse(packet);
    match &command {
        GdbRemoteCommand::ReadMemory { address, length } => {
            let Some(max_hex_len) = length.checked_mul(2) else {
                return gdb_remote_error_response(session);
            };
            if max_hex_len > session.response_config().max_payload_bytes() {
                return gdb_remote_error_response(session);
            }

            let bytes = match read_partitioned_memory_bytes(memory, *address, *length) {
                Ok(bytes) => bytes,
                Err(_) => return gdb_remote_error_response(session),
            };
            session.set_memory_bytes(*address, bytes);
            Ok(session.handle_packet(packet)?)
        }
        GdbRemoteCommand::WriteMemory { address, bytes } => {
            let mut updated_memory = memory.clone();
            if write_partitioned_memory_bytes(&mut updated_memory, *address, bytes).is_err() {
                return gdb_remote_error_response(session);
            }

            let frames = session.handle_packet(packet)?;
            *memory = updated_memory;
            Ok(frames)
        }
        GdbRemoteCommand::Trap { request }
            if request.point().kind() == GdbRemoteTrapKind::SoftwareBreakpoint =>
        {
            handle_riscv_gdb_remote_software_breakpoint(session, memory, packet, *request)
        }
        _ => Ok(session.handle_packet(packet)?),
    }
}

fn handle_riscv_gdb_remote_software_breakpoint(
    session: &mut GdbRemoteSession,
    memory: &mut PartitionedMemoryStore,
    packet: &GdbRemotePacket,
    request: GdbRemoteTrapRequest,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    let point = request.point();
    let Some(breakpoint) = riscv_gdb_software_breakpoint_bytes(point.size()) else {
        return gdb_remote_error_response(session);
    };

    match request.operation() {
        GdbRemoteTrapOperation::Insert => {
            if session.trap_patch(point).is_some() {
                return Ok(session.handle_packet(packet)?);
            }

            let Ok(original) =
                read_partitioned_memory_bytes(memory, point.address(), breakpoint.len())
            else {
                return gdb_remote_error_response(session);
            };
            let mut updated_memory = memory.clone();
            if write_partitioned_memory_bytes(&mut updated_memory, point.address(), &breakpoint)
                .is_err()
            {
                return gdb_remote_error_response(session);
            }

            let frames = session.handle_packet(packet)?;
            session.record_trap_patch(point, original);
            *memory = updated_memory;
            Ok(frames)
        }
        GdbRemoteTrapOperation::Remove => {
            let Some(original) = session.trap_patch(point).map(<[u8]>::to_vec) else {
                return Ok(session.handle_packet(packet)?);
            };
            let mut updated_memory = memory.clone();
            if write_partitioned_memory_bytes(&mut updated_memory, point.address(), &original)
                .is_err()
            {
                return gdb_remote_error_response(session);
            }

            let frames = session.handle_packet(packet)?;
            session.remove_trap_patch(point);
            *memory = updated_memory;
            Ok(frames)
        }
    }
}

fn riscv_gdb_software_breakpoint_bytes(size: u64) -> Option<Vec<u8>> {
    match size {
        2 => Some(0x9002_u16.to_le_bytes().to_vec()),
        4 => Some(0x0010_0073_u32.to_le_bytes().to_vec()),
        _ => None,
    }
}

pub fn handle_riscv_gdb_remote_system_packet(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    cluster: &RiscvCluster,
    memory: &mut PartitionedMemoryStore,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    let command = GdbRemoteCommand::parse(packet);
    match &command {
        GdbRemoteCommand::ReadMemory { .. } | GdbRemoteCommand::WriteMemory { .. } => {
            handle_riscv_gdb_remote_memory_packet(session, memory, packet)
        }
        GdbRemoteCommand::Trap { request } if is_riscv_gdb_software_breakpoint(*request) => {
            handle_riscv_gdb_remote_memory_packet(session, memory, packet)
        }
        _ => handle_riscv_gdb_remote_cluster_packet(xlen, session, cluster, packet),
    }
}

fn is_riscv_gdb_software_breakpoint(request: GdbRemoteTrapRequest) -> bool {
    request.point().kind() == GdbRemoteTrapKind::SoftwareBreakpoint
}

pub fn handle_riscv_gdb_remote_system_packet_with_data_translation(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    cluster: &RiscvCluster,
    memory: &mut PartitionedMemoryStore,
    page_map: &TranslationPageMap,
    packet: &GdbRemotePacket,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    if matches!(
        GdbRemoteCommand::parse(packet),
        GdbRemoteCommand::DumpPageTable
    ) {
        session.set_page_table_dump(riscv_gdb_page_table_dump_from_translation_map(page_map));
    }
    handle_riscv_gdb_remote_system_packet(xlen, session, cluster, memory, packet)
}

fn sync_riscv_gdb_remote_session_from_hart(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    hart: &RiscvHartState,
) {
    session.set_register_bytes(GdbRemoteRegisterBytes::new(riscv_gdb_register_bytes(
        xlen, hart,
    )));
    for (number, bytes) in riscv_gdb_single_register_bytes(xlen, hart) {
        session.set_register_value(number, GdbRemoteRegisterBytes::new(bytes));
    }
}

fn sync_riscv_gdb_remote_session_from_core(
    xlen: RiscvGdbXlen,
    session: &mut GdbRemoteSession,
    core: &RiscvCore,
) {
    sync_riscv_gdb_remote_session_from_hart(
        xlen,
        session,
        &riscv_gdb_hart_snapshot_from_core(core),
    );
}

fn riscv_gdb_remote_selected_core(
    session: &GdbRemoteSession,
    cluster: &RiscvCluster,
) -> Option<RiscvCore> {
    let thread_id = match session.general_thread() {
        GdbRemoteThreadId::Id(thread_id) => thread_id,
        GdbRemoteThreadId::All | GdbRemoteThreadId::Any => session.current_thread_id(),
    };
    let cpu = riscv_gdb_remote_cpu_id(thread_id)?;
    cluster.core(cpu).ok()
}

fn riscv_gdb_remote_cpu_id(thread_id: u64) -> Option<CpuId> {
    let cpu = thread_id.checked_sub(1)?;
    Some(CpuId::new(u32::try_from(cpu).ok()?))
}

fn riscv_gdb_page_permission_flags(permissions: TranslationPagePermissions) -> &'static str {
    match (
        permissions.read(),
        permissions.write(),
        permissions.execute(),
    ) {
        (false, false, false) => "---",
        (false, false, true) => "--x",
        (false, true, false) => "-w-",
        (false, true, true) => "-wx",
        (true, false, false) => "r--",
        (true, false, true) => "r-x",
        (true, true, false) => "rw-",
        (true, true, true) => "rwx",
    }
}

fn riscv_gdb_page_mapping_scope(scope: TranslationPageMappingScope) -> &'static str {
    match scope {
        TranslationPageMappingScope::Global => "global",
        TranslationPageMappingScope::NonGlobal => "non-global",
    }
}

fn riscv_gdb_remote_thread_selection(
    operation: GdbRemoteThreadOperation,
    thread: GdbRemoteThreadId,
    cluster: &RiscvCluster,
) -> Result<Option<u64>, &'static [u8]> {
    match operation {
        GdbRemoteThreadOperation::Continue => match thread {
            GdbRemoteThreadId::All if cluster.core_count() > 0 => Ok(None),
            GdbRemoteThreadId::All => Err(b"E04"),
            GdbRemoteThreadId::Any | GdbRemoteThreadId::Id(_) => Err(b"E02"),
        },
        GdbRemoteThreadOperation::General => match thread {
            GdbRemoteThreadId::All => Err(b"E03"),
            GdbRemoteThreadId::Any if cluster.core_count() > 0 => Ok(None),
            GdbRemoteThreadId::Any => Err(b"E04"),
            GdbRemoteThreadId::Id(thread_id) => {
                if riscv_gdb_remote_thread_exists(thread_id, cluster) {
                    Ok(Some(thread_id))
                } else {
                    Err(b"E04")
                }
            }
        },
    }
}

fn riscv_gdb_remote_thread_exists(thread_id: u64, cluster: &RiscvCluster) -> bool {
    riscv_gdb_remote_cpu_id(thread_id)
        .and_then(|cpu| cluster.core(cpu).ok())
        .is_some()
}

const fn reads_riscv_gdb_remote_registers(command: &GdbRemoteCommand) -> bool {
    matches!(
        command,
        GdbRemoteCommand::ReadRegisters | GdbRemoteCommand::ReadRegister { .. }
    )
}

fn read_partitioned_memory_bytes(
    memory: &PartitionedMemoryStore,
    address: u64,
    length: usize,
) -> Result<Vec<u8>, MemoryError> {
    if length == 0 {
        return Ok(Vec::new());
    }

    let start = Address::new(address);
    memory.validate_access_range(start, AccessSize::new(length as u64)?)?;

    let mut bytes = Vec::with_capacity(length);
    while bytes.len() < length {
        let current = Address::new(address + bytes.len() as u64);
        let decode = memory.decode_detail(current)?;
        let layout = memory.partition_layout(decode.target())?;
        let line = layout.line_address(current);
        let line_offset = layout.line_offset(current) as usize;
        let chunk_len = (layout.bytes() as usize - line_offset).min(length - bytes.len());
        let line_data = memory.line_data(decode.target(), line)?;
        bytes.extend_from_slice(&line_data[line_offset..line_offset + chunk_len]);
    }
    Ok(bytes)
}

fn write_partitioned_memory_bytes(
    memory: &mut PartitionedMemoryStore,
    address: u64,
    bytes: &[u8],
) -> Result<(), MemoryError> {
    if bytes.is_empty() {
        return Ok(());
    }

    let start = Address::new(address);
    memory.validate_access_range(start, AccessSize::new(bytes.len() as u64)?)?;

    let mut offset = 0;
    while offset < bytes.len() {
        let current = Address::new(address + offset as u64);
        let decode = memory.decode_detail(current)?;
        let layout = memory.partition_layout(decode.target())?;
        let line_offset = layout.line_offset(current) as usize;
        let chunk_len = (layout.bytes() as usize - line_offset).min(bytes.len() - offset);
        let size = AccessSize::new(chunk_len as u64)?;
        let request = MemoryRequest::write(
            MemoryRequestId::new(RISCV_GDB_MEMORY_AGENT, offset as u64),
            current,
            size,
            bytes[offset..offset + chunk_len].to_vec(),
            ByteMask::full(size)?,
            layout,
        )?;
        memory.respond(&request)?;
        offset += chunk_len;
    }
    Ok(())
}

fn gdb_remote_error_response(
    session: &mut GdbRemoteSession,
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    gdb_remote_error_response_with_payload(session, b"E01")
}

fn gdb_remote_error_response_with_payload(
    session: &mut GdbRemoteSession,
    payload: &[u8],
) -> Result<Vec<GdbRemoteFrame>, RiscvGdbRemotePacketError> {
    Ok(session.respond_with_payload(payload.to_vec())?)
}

fn riscv_gdb_register_bytes(xlen: RiscvGdbXlen, hart: &RiscvHartState) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(register_set_byte_len(xlen));
    for number in riscv_gdb_register_numbers(xlen) {
        bytes.extend_from_slice(&read_hart_register_bytes(xlen, hart, number));
    }
    bytes
}

fn riscv_gdb_single_register_bytes(
    xlen: RiscvGdbXlen,
    hart: &RiscvHartState,
) -> Vec<(u64, Vec<u8>)> {
    let mut registers = Vec::with_capacity(register_count(xlen));
    for number in riscv_gdb_register_numbers(xlen) {
        registers.push((number, read_hart_register_bytes(xlen, hart, number)));
    }
    registers
}

fn riscv_gdb_hart_snapshot_from_core(core: &RiscvCore) -> RiscvHartState {
    let mut hart = RiscvHartState::with_hart_id(core.pc().get(), u64::from(core.id().get()));
    for register in 0..RISCV_GDB_INTEGER_REGISTER_COUNT {
        let register = riscv_register(u64::from(register));
        hart.write(register, core.read_register(register));
    }
    for register in 0..RISCV_GDB_FLOAT_REGISTER_COUNT {
        let register = FloatRegister::new(register).unwrap();
        hart.write_float(register, core.read_float_register(register));
    }
    for register in 0..RISCV_GDB_VECTOR_REGISTER_COUNT {
        let register = VectorRegister::new(register).unwrap();
        hart.write_vector(register, core.read_vector_register(register));
    }
    hart.set_float_status(core.float_status());
    hart.restore_counter_snapshot(&core.counter_snapshot());
    hart.set_status(core.status());
    hart.set_machine_exception_delegation(core.machine_trap_csr(RiscvMachineTrapCsr::Medeleg));
    hart.set_machine_interrupt_delegation(core.machine_trap_csr(RiscvMachineTrapCsr::Mideleg));
    hart.set_machine_interrupt_enable(core.machine_interrupt_enable());
    hart.set_machine_trap_vector(core.machine_trap_csr(RiscvMachineTrapCsr::Mtvec));
    hart.set_machine_scratch(core.machine_trap_csr(RiscvMachineTrapCsr::Mscratch));
    hart.set_machine_exception_pc(core.machine_trap_csr(RiscvMachineTrapCsr::Mepc));
    hart.set_machine_trap_cause(core.machine_trap_csr(RiscvMachineTrapCsr::Mcause));
    hart.set_machine_trap_value(core.machine_trap_csr(RiscvMachineTrapCsr::Mtval));
    hart.set_machine_interrupt_pending(core.machine_interrupt_pending());
    hart.set_supervisor_trap_vector(core.supervisor_trap_vector());
    hart.set_supervisor_scratch(core.supervisor_scratch());
    hart.set_supervisor_exception_pc(core.supervisor_exception_pc());
    hart.set_supervisor_trap_cause(core.supervisor_trap_cause());
    hart.set_supervisor_trap_value(core.supervisor_trap_value());
    hart.set_translation_satp(core.translation_satp());
    hart.set_vector_fixed_point(core.vector_fixed_point());
    hart
}

fn validate_riscv_gdb_remote_register_write(
    xlen: RiscvGdbXlen,
    command: &GdbRemoteCommand,
) -> Result<bool, RiscvGdbRegisterWriteError> {
    match command {
        GdbRemoteCommand::WriteRegister { number, bytes } => {
            validate_single_register_write(xlen, *number, bytes)?;
            Ok(true)
        }
        GdbRemoteCommand::WriteRegisters { bytes } => {
            validate_all_register_write(xlen, bytes)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn apply_single_register_write(
    xlen: RiscvGdbXlen,
    hart: &mut RiscvHartState,
    number: u64,
    bytes: &[u8],
) -> Result<(), RiscvGdbRegisterWriteError> {
    validate_single_register_write(xlen, number, bytes)?;
    write_register_bytes(xlen, hart, number, bytes);
    Ok(())
}

fn apply_all_register_write(
    xlen: RiscvGdbXlen,
    hart: &mut RiscvHartState,
    bytes: &[u8],
) -> Result<(), RiscvGdbRegisterWriteError> {
    validate_all_register_write(xlen, bytes)?;
    for_each_register_bytes(xlen, bytes, |number, bytes| {
        write_register_bytes(xlen, hart, number, bytes);
    });
    Ok(())
}

fn validate_single_register_write(
    xlen: RiscvGdbXlen,
    number: u64,
    bytes: &[u8],
) -> Result<(), RiscvGdbRegisterWriteError> {
    if !riscv_gdb_register_number_is_supported(xlen, number) {
        return Err(RiscvGdbRegisterWriteError::UnsupportedRegister { number });
    }

    let expected = register_byte_len(xlen, number);
    if bytes.len() != expected {
        return Err(RiscvGdbRegisterWriteError::InvalidRegisterBytes {
            number,
            expected,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn validate_all_register_write(
    xlen: RiscvGdbXlen,
    bytes: &[u8],
) -> Result<(), RiscvGdbRegisterWriteError> {
    let expected = register_set_byte_len(xlen);
    if bytes.len() != expected {
        return Err(RiscvGdbRegisterWriteError::InvalidRegisterSetBytes {
            expected,
            actual: bytes.len(),
        });
    }
    Ok(())
}

fn write_register_bytes(xlen: RiscvGdbXlen, hart: &mut RiscvHartState, number: u64, bytes: &[u8]) {
    if is_riscv_gdb_vector_register(number) {
        let mut vector = [0; RISCV_VECTOR_REGISTER_BYTES];
        vector.copy_from_slice(bytes);
        hart.write_vector(riscv_vector_register(number), vector);
    } else {
        write_register_value(xlen, hart, number, decode_register_value(bytes));
    }
}

fn write_core_register_bytes(xlen: RiscvGdbXlen, core: &RiscvCore, number: u64, bytes: &[u8]) {
    if is_riscv_gdb_vector_register(number) {
        let mut vector = [0; RISCV_VECTOR_REGISTER_BYTES];
        vector.copy_from_slice(bytes);
        core.write_vector_register(riscv_vector_register(number), vector);
    } else {
        write_core_register_value(xlen, core, number, decode_register_value(bytes));
    }
}

fn write_register_value(xlen: RiscvGdbXlen, hart: &mut RiscvHartState, number: u64, value: u64) {
    let value = normalize_register_value(xlen, number, value);

    if number == RISCV_GDB_PC_REGISTER {
        hart.set_pc(value);
    } else if is_riscv_gdb_float_register(number) {
        hart.write_float(riscv_float_register(number), value);
    } else if is_riscv_gdb_float_csr_register(number) {
        write_hart_float_csr_register_value(hart, number, value);
    } else if is_riscv_gdb_csr_register(xlen, number) {
        write_hart_csr_register_value(xlen, hart, number, value);
    } else if !is_riscv_gdb_float_placeholder_register(number) {
        hart.write(riscv_register(number), value);
    }
}

fn write_core_register_value(xlen: RiscvGdbXlen, core: &RiscvCore, number: u64, value: u64) {
    let value = normalize_register_value(xlen, number, value);

    if number == RISCV_GDB_PC_REGISTER {
        core.redirect_pc(Address::new(value));
    } else if is_riscv_gdb_float_register(number) {
        core.write_float_register(riscv_float_register(number), value);
    } else if is_riscv_gdb_float_csr_register(number) {
        write_core_float_csr_register_value(core, number, value);
    } else if is_riscv_gdb_csr_register(xlen, number) {
        write_core_csr_register_value(xlen, core, number, value);
    } else if !is_riscv_gdb_float_placeholder_register(number) {
        core.write_register(riscv_register(number), value);
    }
}

fn read_hart_register_value(xlen: RiscvGdbXlen, hart: &RiscvHartState, number: u64) -> u64 {
    if number == RISCV_GDB_PC_REGISTER {
        hart.pc()
    } else if is_riscv_gdb_float_register(number) {
        hart.read_float(riscv_float_register(number))
    } else if is_riscv_gdb_float_csr_register(number) {
        read_hart_float_csr_register_value(hart, number)
    } else if is_riscv_gdb_float_placeholder_register(number) {
        0
    } else if is_riscv_gdb_csr_register(xlen, number) {
        read_hart_csr_register_value(xlen, hart, number)
    } else {
        hart.read(riscv_register(number))
    }
}

fn read_hart_register_bytes(xlen: RiscvGdbXlen, hart: &RiscvHartState, number: u64) -> Vec<u8> {
    if is_riscv_gdb_vector_register(number) {
        hart.read_vector(riscv_vector_register(number)).to_vec()
    } else {
        encode_register_value(xlen, number, read_hart_register_value(xlen, hart, number))
    }
}

fn for_each_register_bytes(xlen: RiscvGdbXlen, bytes: &[u8], mut visit: impl FnMut(u64, &[u8])) {
    let mut start = 0;
    for number in riscv_gdb_register_numbers(xlen) {
        let register_byte_len = register_byte_len(xlen, number);
        let end = start + register_byte_len;
        visit(number, &bytes[start..end]);
        start = end;
    }
}

fn normalize_register_value(xlen: RiscvGdbXlen, number: u64, value: u64) -> u64 {
    if is_riscv_gdb_float_register(number) {
        value
    } else if is_riscv_gdb_float_csr_register(number)
        || is_riscv_gdb_float_placeholder_register(number)
    {
        value & u64::from(u32::MAX)
    } else {
        match xlen {
            RiscvGdbXlen::Rv32 => value & u64::from(u32::MAX),
            RiscvGdbXlen::Rv64 => value,
        }
    }
}

fn riscv_register(number: u64) -> Register {
    Register::new(number as u8).unwrap()
}

fn riscv_float_register(number: u64) -> FloatRegister {
    FloatRegister::new((number - RISCV_GDB_FLOAT_REGISTER_BASE) as u8).unwrap()
}

fn riscv_vector_register(number: u64) -> VectorRegister {
    VectorRegister::new((number - RISCV_GDB_VECTOR_REGISTER_BASE) as u8).unwrap()
}

fn riscv_float_csr_register(number: u64) -> RiscvFloatCsr {
    match number - RISCV_GDB_FLOAT_CSR_REGISTER_BASE {
        0 => RiscvFloatCsr::Fflags,
        1 => RiscvFloatCsr::Frm,
        2 => RiscvFloatCsr::Fcsr,
        _ => unreachable!("validated RISC-V GDB float CSR register"),
    }
}

fn read_hart_float_csr_register_value(hart: &RiscvHartState, number: u64) -> u64 {
    riscv_float_csr_register(number).read(hart.float_status())
}

fn write_hart_float_csr_register_value(hart: &mut RiscvHartState, number: u64, value: u64) {
    let csr = riscv_float_csr_register(number);
    hart.set_float_status(csr.write(hart.float_status(), value));
}

fn write_core_float_csr_register_value(core: &RiscvCore, number: u64, value: u64) {
    let csr = riscv_float_csr_register(number);
    core.set_float_status(csr.write(core.float_status(), value));
}

fn read_hart_csr_register_value(xlen: RiscvGdbXlen, hart: &RiscvHartState, number: u64) -> u64 {
    match riscv_gdb_csr_register(xlen, number) {
        RiscvGdbCsrRegister::Status(csr) => csr.read(hart.status()),
        RiscvGdbCsrRegister::Interrupt(csr) => read_hart_interrupt_csr(hart, csr),
        RiscvGdbCsrRegister::MachineTrap(csr) => read_hart_machine_trap_csr(hart, csr),
        RiscvGdbCsrRegister::SupervisorTrap(csr) => read_hart_supervisor_trap_csr(hart, csr),
        RiscvGdbCsrRegister::Translation(csr) => read_hart_translation_csr(hart, csr),
        RiscvGdbCsrRegister::VectorFixedPoint(csr) => csr.read(hart.vector_fixed_point()),
        RiscvGdbCsrRegister::Counter(csr) => read_hart_counter_csr(hart, csr),
        RiscvGdbCsrRegister::MachineIdentity(csr) => csr.read(hart.hart_id()),
        RiscvGdbCsrRegister::MachineIsa(csr) => csr.read_for_xlen_bits(riscv_gdb_xlen_bits(xlen)),
    }
}

fn write_hart_csr_register_value(
    xlen: RiscvGdbXlen,
    hart: &mut RiscvHartState,
    number: u64,
    value: u64,
) {
    match riscv_gdb_csr_register(xlen, number) {
        RiscvGdbCsrRegister::Status(csr) => {
            hart.set_status(csr.write(hart.status(), value));
        }
        RiscvGdbCsrRegister::Interrupt(csr) => {
            write_hart_interrupt_csr(hart, csr, value);
        }
        RiscvGdbCsrRegister::MachineTrap(csr) => {
            write_hart_machine_trap_csr(hart, csr, value);
        }
        RiscvGdbCsrRegister::SupervisorTrap(csr) => {
            write_hart_supervisor_trap_csr(hart, csr, value);
        }
        RiscvGdbCsrRegister::Translation(csr) => {
            write_hart_translation_csr(hart, csr, value);
        }
        RiscvGdbCsrRegister::VectorFixedPoint(csr) => {
            hart.set_vector_fixed_point(csr.write(hart.vector_fixed_point(), value));
        }
        RiscvGdbCsrRegister::Counter(csr) => {
            write_hart_counter_csr(hart, csr, value);
        }
        RiscvGdbCsrRegister::MachineIdentity(_) => {}
        RiscvGdbCsrRegister::MachineIsa(_) => {}
    }
}

fn write_core_csr_register_value(xlen: RiscvGdbXlen, core: &RiscvCore, number: u64, value: u64) {
    match riscv_gdb_csr_register(xlen, number) {
        RiscvGdbCsrRegister::Status(csr) => {
            core.set_status(csr.write(core.status(), value));
        }
        RiscvGdbCsrRegister::Interrupt(csr) => {
            write_core_interrupt_csr(core, csr, value);
        }
        RiscvGdbCsrRegister::MachineTrap(csr) => {
            core.set_machine_trap_csr(csr, value);
        }
        RiscvGdbCsrRegister::SupervisorTrap(csr) => {
            write_core_supervisor_trap_csr(core, csr, value);
        }
        RiscvGdbCsrRegister::Translation(csr) => {
            write_core_translation_csr(core, csr, value);
        }
        RiscvGdbCsrRegister::VectorFixedPoint(csr) => {
            core.set_vector_fixed_point(csr.write(core.vector_fixed_point(), value));
        }
        RiscvGdbCsrRegister::Counter(csr) => {
            write_core_counter_csr(core, csr, value);
        }
        RiscvGdbCsrRegister::MachineIdentity(_) => {}
        RiscvGdbCsrRegister::MachineIsa(_) => {}
    }
}

fn riscv_gdb_csr_register(xlen: RiscvGdbXlen, number: u64) -> RiscvGdbCsrRegister {
    if number == RISCV_GDB_SUPERVISOR_INTERRUPT_ENABLE_REGISTER {
        return RiscvGdbCsrRegister::Interrupt(RiscvInterruptCsr::SupervisorInterruptEnable);
    }
    if number == RISCV_GDB_SUPERVISOR_INTERRUPT_PENDING_REGISTER {
        return RiscvGdbCsrRegister::Interrupt(RiscvInterruptCsr::SupervisorInterruptPending);
    }
    if number == RISCV_GDB_COUNTER_CYCLE_REGISTER {
        return RiscvGdbCsrRegister::Counter(RiscvCounterCsr::Cycle);
    }
    if number == RISCV_GDB_COUNTER_INSTRET_REGISTER {
        return RiscvGdbCsrRegister::Counter(RiscvCounterCsr::Instret);
    }
    if number == RISCV_GDB_COUNTER_TIME_REGISTER {
        return RiscvGdbCsrRegister::Counter(RiscvCounterCsr::Time);
    }
    if number == RISCV_GDB_MACHINE_HART_ID_REGISTER {
        return RiscvGdbCsrRegister::MachineIdentity(RiscvMachineIdentityCsr::HartId);
    }
    if number == RISCV_GDB_MACHINE_VENDOR_ID_REGISTER {
        return RiscvGdbCsrRegister::MachineIdentity(RiscvMachineIdentityCsr::VendorId);
    }
    if number == RISCV_GDB_MACHINE_ARCHITECTURE_ID_REGISTER {
        return RiscvGdbCsrRegister::MachineIdentity(RiscvMachineIdentityCsr::ArchitectureId);
    }
    if number == RISCV_GDB_MACHINE_IMPLEMENTATION_ID_REGISTER {
        return RiscvGdbCsrRegister::MachineIdentity(RiscvMachineIdentityCsr::ImplementationId);
    }
    if number == RISCV_GDB_MACHINE_ISA_REGISTER {
        return RiscvGdbCsrRegister::MachineIsa(RiscvMachineIsaCsr::Misa);
    }

    match number - riscv_gdb_csr_register_base(xlen) {
        0 => RiscvGdbCsrRegister::Status(RiscvStatusCsr::Sstatus),
        1 => RiscvGdbCsrRegister::SupervisorTrap(RiscvSupervisorTrapCsr::Stvec),
        2 => RiscvGdbCsrRegister::SupervisorTrap(RiscvSupervisorTrapCsr::Sscratch),
        3 => RiscvGdbCsrRegister::SupervisorTrap(RiscvSupervisorTrapCsr::Sepc),
        4 => RiscvGdbCsrRegister::SupervisorTrap(RiscvSupervisorTrapCsr::Scause),
        5 => RiscvGdbCsrRegister::SupervisorTrap(RiscvSupervisorTrapCsr::Stval),
        6 => RiscvGdbCsrRegister::Translation(RiscvTranslationCsr::Satp),
        7 => RiscvGdbCsrRegister::Status(RiscvStatusCsr::Mstatus),
        8 => RiscvGdbCsrRegister::MachineTrap(RiscvMachineTrapCsr::Medeleg),
        9 => RiscvGdbCsrRegister::MachineTrap(RiscvMachineTrapCsr::Mideleg),
        10 => RiscvGdbCsrRegister::Interrupt(RiscvInterruptCsr::MachineInterruptEnable),
        11 => RiscvGdbCsrRegister::MachineTrap(RiscvMachineTrapCsr::Mtvec),
        12 => RiscvGdbCsrRegister::MachineTrap(RiscvMachineTrapCsr::Mscratch),
        13 => RiscvGdbCsrRegister::MachineTrap(RiscvMachineTrapCsr::Mepc),
        14 => RiscvGdbCsrRegister::MachineTrap(RiscvMachineTrapCsr::Mcause),
        15 => RiscvGdbCsrRegister::MachineTrap(RiscvMachineTrapCsr::Mtval),
        16 => RiscvGdbCsrRegister::Interrupt(RiscvInterruptCsr::MachineInterruptPending),
        17 => RiscvGdbCsrRegister::VectorFixedPoint(RiscvVectorFixedPointCsr::Vxsat),
        18 => RiscvGdbCsrRegister::VectorFixedPoint(RiscvVectorFixedPointCsr::Vxrm),
        19 => RiscvGdbCsrRegister::VectorFixedPoint(RiscvVectorFixedPointCsr::Vcsr),
        _ => unreachable!("validated RISC-V GDB CSR register"),
    }
}

fn read_hart_counter_csr(hart: &RiscvHartState, csr: RiscvCounterCsr) -> u64 {
    let snapshot = hart.counter_snapshot();
    match csr {
        RiscvCounterCsr::Cycle | RiscvCounterCsr::Time => snapshot.cycle(),
        RiscvCounterCsr::Instret => snapshot.instret(),
    }
}

fn write_hart_counter_csr(hart: &mut RiscvHartState, csr: RiscvCounterCsr, value: u64) {
    let snapshot = counter_snapshot_with_value(hart.counter_snapshot(), csr, value);
    hart.restore_counter_snapshot(&snapshot);
}

fn write_core_counter_csr(core: &RiscvCore, csr: RiscvCounterCsr, value: u64) {
    let snapshot = counter_snapshot_with_value(core.counter_snapshot(), csr, value);
    core.restore_counter_snapshot(&snapshot);
}

fn counter_snapshot_with_value(
    snapshot: RiscvCounterSnapshot,
    csr: RiscvCounterCsr,
    value: u64,
) -> RiscvCounterSnapshot {
    match csr {
        RiscvCounterCsr::Cycle => RiscvCounterSnapshot::new(value, snapshot.instret()),
        RiscvCounterCsr::Instret => RiscvCounterSnapshot::new(snapshot.cycle(), value),
        RiscvCounterCsr::Time => RiscvCounterSnapshot::new(value, snapshot.instret()),
    }
}

fn read_hart_interrupt_csr(hart: &RiscvHartState, csr: RiscvInterruptCsr) -> u64 {
    match csr {
        RiscvInterruptCsr::MachineInterruptEnable => hart.machine_interrupt_enable(),
        RiscvInterruptCsr::MachineInterruptPending => hart.machine_interrupt_pending(),
        RiscvInterruptCsr::SupervisorInterruptEnable => {
            hart.machine_interrupt_enable() & hart.machine_interrupt_delegation()
        }
        RiscvInterruptCsr::SupervisorInterruptPending => {
            hart.machine_interrupt_pending() & hart.machine_interrupt_delegation()
        }
    }
}

fn write_hart_interrupt_csr(hart: &mut RiscvHartState, csr: RiscvInterruptCsr, value: u64) {
    match csr {
        RiscvInterruptCsr::MachineInterruptEnable => hart.set_machine_interrupt_enable(value),
        RiscvInterruptCsr::MachineInterruptPending => hart.set_machine_interrupt_pending(value),
        RiscvInterruptCsr::SupervisorInterruptEnable => {
            let mask = hart.machine_interrupt_delegation();
            let enable = (hart.machine_interrupt_enable() & !mask) | (value & mask);
            hart.set_machine_interrupt_enable(enable);
        }
        RiscvInterruptCsr::SupervisorInterruptPending => {
            let mask = hart.machine_interrupt_delegation();
            let pending = (hart.machine_interrupt_pending() & !mask) | (value & mask);
            hart.set_machine_interrupt_pending(pending);
        }
    }
}

fn write_core_interrupt_csr(core: &RiscvCore, csr: RiscvInterruptCsr, value: u64) {
    match csr {
        RiscvInterruptCsr::MachineInterruptEnable => core.set_machine_interrupt_enable(value),
        RiscvInterruptCsr::MachineInterruptPending => core.set_machine_interrupt_pending(value),
        RiscvInterruptCsr::SupervisorInterruptEnable => {
            let mask = core.machine_trap_csr(RiscvMachineTrapCsr::Mideleg);
            let enable = (core.machine_interrupt_enable() & !mask) | (value & mask);
            core.set_machine_interrupt_enable(enable);
        }
        RiscvInterruptCsr::SupervisorInterruptPending => {
            let mask = core.machine_trap_csr(RiscvMachineTrapCsr::Mideleg);
            let pending = (core.machine_interrupt_pending() & !mask) | (value & mask);
            core.set_machine_interrupt_pending(pending);
        }
    }
}

fn read_hart_machine_trap_csr(hart: &RiscvHartState, csr: RiscvMachineTrapCsr) -> u64 {
    match csr {
        RiscvMachineTrapCsr::Medeleg => hart.machine_exception_delegation(),
        RiscvMachineTrapCsr::Mideleg => hart.machine_interrupt_delegation(),
        RiscvMachineTrapCsr::Mtvec => hart.machine_trap_vector(),
        RiscvMachineTrapCsr::Mscratch => hart.machine_scratch(),
        RiscvMachineTrapCsr::Mepc => hart.machine_exception_pc(),
        RiscvMachineTrapCsr::Mcause => hart.machine_trap_cause(),
        RiscvMachineTrapCsr::Mtval => hart.machine_trap_value(),
    }
}

fn write_hart_machine_trap_csr(hart: &mut RiscvHartState, csr: RiscvMachineTrapCsr, value: u64) {
    match csr {
        RiscvMachineTrapCsr::Medeleg => hart.set_machine_exception_delegation(value),
        RiscvMachineTrapCsr::Mideleg => hart.set_machine_interrupt_delegation(value),
        RiscvMachineTrapCsr::Mtvec => hart.set_machine_trap_vector(value),
        RiscvMachineTrapCsr::Mscratch => hart.set_machine_scratch(value),
        RiscvMachineTrapCsr::Mepc => hart.set_machine_exception_pc(value),
        RiscvMachineTrapCsr::Mcause => hart.set_machine_trap_cause(value),
        RiscvMachineTrapCsr::Mtval => hart.set_machine_trap_value(value),
    }
}

fn read_hart_supervisor_trap_csr(hart: &RiscvHartState, csr: RiscvSupervisorTrapCsr) -> u64 {
    match csr {
        RiscvSupervisorTrapCsr::Stvec => hart.supervisor_trap_vector(),
        RiscvSupervisorTrapCsr::Sscratch => hart.supervisor_scratch(),
        RiscvSupervisorTrapCsr::Sepc => hart.supervisor_exception_pc(),
        RiscvSupervisorTrapCsr::Scause => hart.supervisor_trap_cause(),
        RiscvSupervisorTrapCsr::Stval => hart.supervisor_trap_value(),
    }
}

fn write_hart_supervisor_trap_csr(
    hart: &mut RiscvHartState,
    csr: RiscvSupervisorTrapCsr,
    value: u64,
) {
    match csr {
        RiscvSupervisorTrapCsr::Stvec => hart.set_supervisor_trap_vector(value),
        RiscvSupervisorTrapCsr::Sscratch => hart.set_supervisor_scratch(value),
        RiscvSupervisorTrapCsr::Sepc => hart.set_supervisor_exception_pc(value),
        RiscvSupervisorTrapCsr::Scause => hart.set_supervisor_trap_cause(value),
        RiscvSupervisorTrapCsr::Stval => hart.set_supervisor_trap_value(value),
    }
}

fn write_core_supervisor_trap_csr(core: &RiscvCore, csr: RiscvSupervisorTrapCsr, value: u64) {
    match csr {
        RiscvSupervisorTrapCsr::Stvec => core.set_supervisor_trap_vector(value),
        RiscvSupervisorTrapCsr::Sscratch => core.set_supervisor_scratch(value),
        RiscvSupervisorTrapCsr::Sepc => core.set_supervisor_exception_pc(value),
        RiscvSupervisorTrapCsr::Scause => core.set_supervisor_trap_cause(value),
        RiscvSupervisorTrapCsr::Stval => core.set_supervisor_trap_value(value),
    }
}

fn read_hart_translation_csr(hart: &RiscvHartState, csr: RiscvTranslationCsr) -> u64 {
    match csr {
        RiscvTranslationCsr::Satp => hart.translation_satp(),
    }
}

fn write_hart_translation_csr(hart: &mut RiscvHartState, csr: RiscvTranslationCsr, value: u64) {
    match csr {
        RiscvTranslationCsr::Satp => hart.set_translation_satp(value),
    }
}

fn write_core_translation_csr(core: &RiscvCore, csr: RiscvTranslationCsr, value: u64) {
    match csr {
        RiscvTranslationCsr::Satp => core.set_translation_satp(value),
    }
}

fn riscv_gdb_register_numbers(xlen: RiscvGdbXlen) -> impl Iterator<Item = u64> {
    let float_count = u64::from(RISCV_GDB_FLOAT_REGISTER_COUNT);
    let float_csr_count = u64::from(RISCV_GDB_FLOAT_CSR_REGISTER_COUNT);
    let csr_count = u64::from(RISCV_GDB_CSR_REGISTER_COUNT);
    (0..RISCV_GDB_INTEGER_REGISTER_COUNT)
        .map(u64::from)
        .chain(std::iter::once(RISCV_GDB_PC_REGISTER))
        .chain(RISCV_GDB_FLOAT_REGISTER_BASE..RISCV_GDB_FLOAT_REGISTER_BASE + float_count)
        .chain(
            RISCV_GDB_FLOAT_CSR_REGISTER_BASE..RISCV_GDB_FLOAT_CSR_REGISTER_BASE + float_csr_count,
        )
        .chain(std::iter::once(RISCV_GDB_FLOAT_PLACEHOLDER_REGISTER))
        .chain(riscv_gdb_csr_register_base(xlen)..riscv_gdb_csr_register_base(xlen) + csr_count)
        .chain(
            RISCV_GDB_VECTOR_REGISTER_BASE
                ..RISCV_GDB_VECTOR_REGISTER_BASE + u64::from(RISCV_GDB_VECTOR_REGISTER_COUNT),
        )
        .chain([
            RISCV_GDB_SUPERVISOR_INTERRUPT_ENABLE_REGISTER,
            RISCV_GDB_SUPERVISOR_INTERRUPT_PENDING_REGISTER,
            RISCV_GDB_COUNTER_CYCLE_REGISTER,
            RISCV_GDB_COUNTER_INSTRET_REGISTER,
            RISCV_GDB_COUNTER_TIME_REGISTER,
            RISCV_GDB_MACHINE_HART_ID_REGISTER,
            RISCV_GDB_MACHINE_VENDOR_ID_REGISTER,
            RISCV_GDB_MACHINE_ARCHITECTURE_ID_REGISTER,
            RISCV_GDB_MACHINE_IMPLEMENTATION_ID_REGISTER,
            RISCV_GDB_MACHINE_ISA_REGISTER,
        ])
}

const fn register_count(_xlen: RiscvGdbXlen) -> usize {
    RISCV_GDB_INTEGER_REGISTER_COUNT as usize
        + 1
        + RISCV_GDB_FLOAT_REGISTER_COUNT as usize
        + RISCV_GDB_FLOAT_CSR_REGISTER_COUNT as usize
        + 1
        + RISCV_GDB_CSR_REGISTER_COUNT as usize
        + RISCV_GDB_VECTOR_REGISTER_COUNT as usize
        + RISCV_GDB_SPARSE_CSR_REGISTER_COUNT
}

fn register_set_byte_len(xlen: RiscvGdbXlen) -> usize {
    riscv_gdb_register_numbers(xlen)
        .map(|number| register_byte_len(xlen, number))
        .sum()
}

const fn riscv_gdb_register_number_is_supported(xlen: RiscvGdbXlen, number: u64) -> bool {
    number <= RISCV_GDB_PC_REGISTER
        || is_riscv_gdb_float_register(number)
        || is_riscv_gdb_float_csr_register(number)
        || is_riscv_gdb_float_placeholder_register(number)
        || is_riscv_gdb_csr_register(xlen, number)
        || is_riscv_gdb_vector_register(number)
}

const fn is_riscv_gdb_float_register(number: u64) -> bool {
    number >= RISCV_GDB_FLOAT_REGISTER_BASE
        && number < RISCV_GDB_FLOAT_REGISTER_BASE + RISCV_GDB_FLOAT_REGISTER_COUNT as u64
}

const fn is_riscv_gdb_float_csr_register(number: u64) -> bool {
    number >= RISCV_GDB_FLOAT_CSR_REGISTER_BASE
        && number < RISCV_GDB_FLOAT_CSR_REGISTER_BASE + RISCV_GDB_FLOAT_CSR_REGISTER_COUNT as u64
}

const fn is_riscv_gdb_float_placeholder_register(number: u64) -> bool {
    number == RISCV_GDB_FLOAT_PLACEHOLDER_REGISTER
}

const fn is_riscv_gdb_vector_register(number: u64) -> bool {
    number >= RISCV_GDB_VECTOR_REGISTER_BASE
        && number < RISCV_GDB_VECTOR_REGISTER_BASE + RISCV_GDB_VECTOR_REGISTER_COUNT as u64
}

const fn riscv_gdb_csr_register_base(xlen: RiscvGdbXlen) -> u64 {
    match xlen {
        RiscvGdbXlen::Rv32 => RISCV_GDB_RV32_CSR_REGISTER_BASE,
        RiscvGdbXlen::Rv64 => RISCV_GDB_RV64_CSR_REGISTER_BASE,
    }
}

const fn is_riscv_gdb_csr_register(xlen: RiscvGdbXlen, number: u64) -> bool {
    (number >= riscv_gdb_csr_register_base(xlen)
        && number < riscv_gdb_csr_register_base(xlen) + RISCV_GDB_CSR_REGISTER_COUNT as u64)
        || number == RISCV_GDB_SUPERVISOR_INTERRUPT_ENABLE_REGISTER
        || number == RISCV_GDB_SUPERVISOR_INTERRUPT_PENDING_REGISTER
        || number == RISCV_GDB_COUNTER_CYCLE_REGISTER
        || number == RISCV_GDB_COUNTER_INSTRET_REGISTER
        || number == RISCV_GDB_COUNTER_TIME_REGISTER
        || number == RISCV_GDB_MACHINE_HART_ID_REGISTER
        || number == RISCV_GDB_MACHINE_VENDOR_ID_REGISTER
        || number == RISCV_GDB_MACHINE_ARCHITECTURE_ID_REGISTER
        || number == RISCV_GDB_MACHINE_IMPLEMENTATION_ID_REGISTER
        || number == RISCV_GDB_MACHINE_ISA_REGISTER
}

fn encode_register_value(xlen: RiscvGdbXlen, number: u64, value: u64) -> Vec<u8> {
    if register_byte_len(xlen, number) == 4 {
        (value as u32).to_le_bytes().to_vec()
    } else {
        value.to_le_bytes().to_vec()
    }
}

fn decode_register_value(bytes: &[u8]) -> u64 {
    let mut raw = [0; 8];
    raw[..bytes.len()].copy_from_slice(bytes);
    u64::from_le_bytes(raw)
}

const fn byte_len(xlen: RiscvGdbXlen) -> usize {
    match xlen {
        RiscvGdbXlen::Rv32 => 4,
        RiscvGdbXlen::Rv64 => 8,
    }
}

const fn riscv_gdb_xlen_bits(xlen: RiscvGdbXlen) -> u8 {
    match xlen {
        RiscvGdbXlen::Rv32 => 32,
        RiscvGdbXlen::Rv64 => 64,
    }
}

const fn register_byte_len(xlen: RiscvGdbXlen, number: u64) -> usize {
    if is_riscv_gdb_float_register(number) {
        8
    } else if is_riscv_gdb_vector_register(number) {
        RISCV_VECTOR_REGISTER_BYTES
    } else if is_riscv_gdb_float_csr_register(number)
        || is_riscv_gdb_float_placeholder_register(number)
    {
        4
    } else {
        byte_len(xlen)
    }
}
