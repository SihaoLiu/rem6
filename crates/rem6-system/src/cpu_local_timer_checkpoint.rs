use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_timer::{
    CpuLocalTimerBankSnapshot, CpuLocalTimerControl, CpuLocalTimerCounterSnapshot,
    CpuLocalTimerCounterSnapshotFields, CpuLocalTimerCpuSnapshot, CpuLocalTimerError,
    CpuLocalTimerMmioDevice, CpuLocalWatchdogControl, CpuLocalWatchdogSnapshot,
    CpuLocalWatchdogSnapshotFields,
};

const CPU_LOCAL_TIMER_CHUNK: &str = "cpu-local-timer";
const U8_BYTES: usize = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const TIMER_COUNTER_RECORD_BYTES: usize =
    U32_BYTES * 2 + U64_BYTES + U32_BYTES + U8_BYTES * 2 + U64_BYTES * 2;
const WATCHDOG_BASE_RECORD_BYTES: usize =
    U32_BYTES * 2 + U64_BYTES + U32_BYTES + U8_BYTES * 3 + U32_BYTES + U64_BYTES * 3;
const CPU_RECORD_MIN_BYTES: usize = TIMER_COUNTER_RECORD_BYTES + WATCHDOG_BASE_RECORD_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuLocalTimerCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: CpuLocalTimerBankSnapshot,
}

impl CpuLocalTimerCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: CpuLocalTimerBankSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &CpuLocalTimerBankSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct CpuLocalTimerCheckpointPort {
    component: CheckpointComponentId,
    device: CpuLocalTimerMmioDevice,
}

impl CpuLocalTimerCheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: CpuLocalTimerMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> CpuLocalTimerMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<CpuLocalTimerCheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(
            &self.component,
            CPU_LOCAL_TIMER_CHUNK,
            encode_cpu_local_timer(&snapshot),
        )?;
        Ok(CpuLocalTimerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<CpuLocalTimerCheckpointRecord, CpuLocalTimerCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<CpuLocalTimerCheckpointRecord, CpuLocalTimerCheckpointError> {
        let payload = registry
            .chunk(&self.component, CPU_LOCAL_TIMER_CHUNK)
            .ok_or_else(|| CpuLocalTimerCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: CPU_LOCAL_TIMER_CHUNK.to_string(),
            })?;
        let snapshot = decode_cpu_local_timer(&self.component, payload)?;
        self.validate_snapshot(&snapshot)?;
        Ok(CpuLocalTimerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &CpuLocalTimerBankSnapshot,
    ) -> Result<(), CpuLocalTimerCheckpointError> {
        self.device.validate_snapshot(snapshot).map_err(|error| {
            CpuLocalTimerCheckpointError::CpuLocalTimer {
                component: self.component.clone(),
                error,
            }
        })
    }

    fn restore_snapshot(
        &self,
        snapshot: &CpuLocalTimerBankSnapshot,
    ) -> Result<(), CpuLocalTimerCheckpointError> {
        self.device
            .restore(snapshot)
            .map_err(|error| CpuLocalTimerCheckpointError::CpuLocalTimer {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct CpuLocalTimerCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, CpuLocalTimerCheckpointPort>,
}

impl CpuLocalTimerCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = CpuLocalTimerCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }
        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<CpuLocalTimerCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<CpuLocalTimerCheckpointRecord>, CpuLocalTimerCheckpointError> {
        self.validate_restore_from(registry)?;
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.restore_snapshot(record.snapshot())?;
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), CpuLocalTimerCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuLocalTimerCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    CpuLocalTimer {
        component: CheckpointComponentId,
        error: CpuLocalTimerError,
    },
}

impl CpuLocalTimerCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::CpuLocalTimer { component, .. } => component,
        }
    }
}

impl fmt::Display for CpuLocalTimerCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "CPU local timer checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "CPU local timer checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::CpuLocalTimer { component, error } => write!(
                formatter,
                "CPU local timer checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for CpuLocalTimerCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CpuLocalTimer { error, .. } => Some(error),
            _ => None,
        }
    }
}

fn encode_cpu_local_timer(snapshot: &CpuLocalTimerBankSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, snapshot.cpus().len() as u64);
    for cpu in snapshot.cpus() {
        let timer = cpu.timer();
        write_u32(&mut payload, timer.load_value());
        write_u32(&mut payload, timer.base_value());
        write_u64(&mut payload, timer.last_updated_tick());
        write_u32(&mut payload, timer.control().bits());
        write_bool(&mut payload, timer.raw_interrupt());
        write_bool(&mut payload, timer.pending_interrupt());
        write_u64(&mut payload, timer.clock_tick());
        write_u64(&mut payload, timer.generation());

        let watchdog = cpu.watchdog();
        write_u32(&mut payload, watchdog.load_value());
        write_u32(&mut payload, watchdog.base_value());
        write_u64(&mut payload, watchdog.last_updated_tick());
        write_u32(&mut payload, watchdog.control().bits());
        write_bool(&mut payload, watchdog.raw_interrupt());
        write_bool(&mut payload, watchdog.pending_interrupt());
        write_bool(&mut payload, watchdog.raw_reset());
        write_u32(&mut payload, watchdog.disable_register());
        write_u64(&mut payload, watchdog.clock_tick());
        write_u64(&mut payload, watchdog.generation());
        write_u64(&mut payload, watchdog.reset_assertions().len() as u64);
        for tick in watchdog.reset_assertions() {
            write_u64(&mut payload, *tick);
        }
    }
    payload
}

fn decode_cpu_local_timer(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<CpuLocalTimerBankSnapshot, CpuLocalTimerCheckpointError> {
    let mut cursor = PayloadCursor::new(component, payload);
    let cpu_count = cursor.read_bounded_count("CPU local timer CPU count", CPU_RECORD_MIN_BYTES)?;
    let mut cpus = Vec::with_capacity(cpu_count);
    for _ in 0..cpu_count {
        let timer = CpuLocalTimerCounterSnapshot::from_fields(CpuLocalTimerCounterSnapshotFields {
            load_value: cursor.read_u32("timer load value")?,
            base_value: cursor.read_u32("timer base value")?,
            last_updated_tick: cursor.read_u64("timer last updated tick")?,
            control: CpuLocalTimerControl::new(cursor.read_u32("timer control")?),
            raw_interrupt: cursor.read_bool("timer raw interrupt")?,
            pending_interrupt: cursor.read_bool("timer pending interrupt")?,
            clock_tick: cursor.read_u64("timer clock tick")?,
            generation: cursor.read_u64("timer generation")?,
        });

        let watchdog_load = cursor.read_u32("watchdog load value")?;
        let watchdog_base = cursor.read_u32("watchdog base value")?;
        let watchdog_tick = cursor.read_u64("watchdog last updated tick")?;
        let watchdog_control = CpuLocalWatchdogControl::new(cursor.read_u32("watchdog control")?);
        let watchdog_raw = cursor.read_bool("watchdog raw interrupt")?;
        let watchdog_pending = cursor.read_bool("watchdog pending interrupt")?;
        let watchdog_reset = cursor.read_bool("watchdog raw reset")?;
        let watchdog_disable = cursor.read_u32("watchdog disable register")?;
        let watchdog_clock = cursor.read_u64("watchdog clock tick")?;
        let watchdog_generation = cursor.read_u64("watchdog generation")?;
        let reset_count = cursor.read_bounded_count("watchdog reset assertion count", U64_BYTES)?;
        let mut reset_assertions = Vec::with_capacity(reset_count);
        for _ in 0..reset_count {
            reset_assertions.push(cursor.read_u64("watchdog reset assertion tick")?);
        }
        let watchdog = CpuLocalWatchdogSnapshot::from_fields(CpuLocalWatchdogSnapshotFields {
            load_value: watchdog_load,
            base_value: watchdog_base,
            last_updated_tick: watchdog_tick,
            control: watchdog_control,
            raw_interrupt: watchdog_raw,
            pending_interrupt: watchdog_pending,
            raw_reset: watchdog_reset,
            disable_register: watchdog_disable,
            clock_tick: watchdog_clock,
            generation: watchdog_generation,
            reset_assertions,
        });
        cpus.push(CpuLocalTimerCpuSnapshot::new(timer, watchdog));
    }
    cursor.finish()?;
    Ok(CpuLocalTimerBankSnapshot::new(cpus))
}

fn write_bool(payload: &mut Vec<u8>, value: bool) {
    payload.push(u8::from(value));
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    component: &'a CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> PayloadCursor<'a> {
    const fn new(component: &'a CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn read_bool(&mut self, name: &str) -> Result<bool, CpuLocalTimerCheckpointError> {
        match self.read_u8(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} has invalid bool byte {value}"))),
        }
    }

    fn read_u8(&mut self, name: &str) -> Result<u8, CpuLocalTimerCheckpointError> {
        let bytes = self.read_bytes(name, U8_BYTES)?;
        Ok(bytes[0])
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, CpuLocalTimerCheckpointError> {
        let bytes: [u8; U32_BYTES] = self
            .read_bytes(name, U32_BYTES)?
            .try_into()
            .expect("cursor returned exact u32 bytes");
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, CpuLocalTimerCheckpointError> {
        let bytes: [u8; U64_BYTES] = self
            .read_bytes(name, U64_BYTES)?
            .try_into()
            .expect("cursor returned exact u64 bytes");
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_bounded_count(
        &mut self,
        name: &str,
        min_record_bytes: usize,
    ) -> Result<usize, CpuLocalTimerCheckpointError> {
        let count_value = self.read_u64(name)?;
        let count = usize::try_from(count_value)
            .map_err(|_| self.invalid(format!("{name} does not fit host usize")))?;
        let remaining = self.payload.len().saturating_sub(self.offset);
        let max_records = remaining / min_record_bytes;
        if count > max_records {
            return Err(self.invalid(format!(
                "{name} {count_value} exceeds remaining payload capacity {max_records} records"
            )));
        }
        Ok(count)
    }

    fn read_bytes(
        &mut self,
        name: &str,
        size: usize,
    ) -> Result<&'a [u8], CpuLocalTimerCheckpointError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or_else(|| self.invalid(format!("{name} size overflows host usize")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!(
                "{name} needs {size} bytes at offset {} but payload has {}",
                self.offset,
                self.payload.len()
            )));
        }
        let bytes = &self.payload[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), CpuLocalTimerCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "trailing {} bytes after CPU local timer checkpoint payload",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> CpuLocalTimerCheckpointError {
        CpuLocalTimerCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
