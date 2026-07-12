use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::{Arc, Mutex, MutexGuard};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};

use crate::{
    validate_storage_bytes, CowStorageImage, CowStorageSnapshot, FileStorageImage,
    FileStorageSnapshot, IdeBmiSnapshot, IdeChannelId, IdeChannelSnapshot, IdeController,
    IdeControllerError, IdeControllerSnapshot, IdeDeviceId, IdeDiskSnapshot,
    IdeDiskTransferSnapshot, IdeDmaDirection, IdePendingCommandSnapshot, IdeTaskFile,
    RawStorageImage, RawStorageSnapshot, StorageError, StorageSectorId, STORAGE_SECTOR_BYTES,
};

const STORAGE_IMAGE_CHUNK: &str = "storage-image";
const IDE_CONTROLLER_CHUNK: &str = "ide-controller";
const STORAGE_RAW_KIND: u8 = 1;
const STORAGE_COW_KIND: u8 = 2;
const STORAGE_FILE_KIND: u8 = 3;
const IDE_CONTROLLER_VERSION: u8 = 2;
const IDE_TRANSFER_NONE: u8 = 0;
const IDE_TRANSFER_INPUT: u8 = 1;
const IDE_TRANSFER_OUTPUT: u8 = 2;
const IDE_TRANSFER_DMA: u8 = 3;
const IDE_PENDING_COMMAND_NONE: u8 = 0;
const IDE_PENDING_COMMAND_MEDIA: u8 = 1;
const IDE_DMA_TO_GUEST: u8 = 1;
const IDE_DMA_FROM_GUEST: u8 = 2;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageImageCheckpointSnapshot {
    Raw(RawStorageSnapshot),
    Cow(CowStorageSnapshot),
    File(FileStorageSnapshot),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageImageCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: StorageImageCheckpointSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdeControllerCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: IdeControllerSnapshot,
}

impl IdeControllerCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: IdeControllerSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &IdeControllerSnapshot {
        &self.snapshot
    }
}

impl StorageImageCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: StorageImageCheckpointSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &StorageImageCheckpointSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct StorageImageCheckpointPort {
    component: CheckpointComponentId,
    target: StorageImageCheckpointTarget,
}

#[derive(Clone, Debug)]
pub struct IdeControllerCheckpointPort {
    component: CheckpointComponentId,
    controller: Arc<Mutex<IdeController>>,
}

impl IdeControllerCheckpointPort {
    pub fn new(component: CheckpointComponentId, controller: Arc<Mutex<IdeController>>) -> Self {
        Self {
            component,
            controller,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<IdeControllerCheckpointRecord, StorageCheckpointError> {
        let snapshot = self.lock_controller()?.snapshot();
        registry
            .write_chunk(
                &self.component,
                IDE_CONTROLLER_CHUNK,
                encode_ide_controller(&snapshot),
            )
            .map_err(StorageCheckpointError::Checkpoint)?;
        Ok(IdeControllerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<IdeControllerCheckpointRecord, StorageCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<IdeControllerCheckpointRecord, StorageCheckpointError> {
        let payload = registry
            .chunk(&self.component, IDE_CONTROLLER_CHUNK)
            .ok_or_else(|| StorageCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: IDE_CONTROLLER_CHUNK.to_string(),
            })?;
        let snapshot = decode_ide_controller(&self.component, payload)?;
        Ok(IdeControllerCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &IdeControllerSnapshot,
    ) -> Result<(), StorageCheckpointError> {
        let mut cloned = self.lock_controller()?.clone();
        cloned
            .restore(snapshot)
            .map_err(|error| StorageCheckpointError::Ide {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &IdeControllerSnapshot,
    ) -> Result<(), StorageCheckpointError> {
        self.lock_controller()?
            .restore(snapshot)
            .map_err(|error| StorageCheckpointError::Ide {
                component: self.component.clone(),
                error,
            })
    }

    fn lock_controller(&self) -> Result<MutexGuard<'_, IdeController>, StorageCheckpointError> {
        self.controller
            .lock()
            .map_err(|_| StorageCheckpointError::LockPoisoned {
                component: self.component.clone(),
            })
    }
}

impl StorageImageCheckpointPort {
    pub fn raw(component: CheckpointComponentId, image: RawStorageImage) -> Self {
        Self {
            component,
            target: StorageImageCheckpointTarget::Raw(image),
        }
    }

    pub fn cow(component: CheckpointComponentId, image: CowStorageImage) -> Self {
        Self {
            component,
            target: StorageImageCheckpointTarget::Cow(image),
        }
    }

    pub fn file(component: CheckpointComponentId, image: FileStorageImage) -> Self {
        Self {
            component,
            target: StorageImageCheckpointTarget::File(image),
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    fn aliases_target(&self, other: &Self) -> bool {
        self.target.aliases(&other.target)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<StorageImageCheckpointRecord, StorageCheckpointError> {
        let snapshot = self
            .target
            .snapshot()
            .map_err(|error| StorageCheckpointError::Storage {
                component: self.component.clone(),
                error,
            })?;
        registry
            .write_chunk(
                &self.component,
                STORAGE_IMAGE_CHUNK,
                encode_storage_image(&snapshot),
            )
            .map_err(StorageCheckpointError::Checkpoint)?;
        Ok(StorageImageCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<StorageImageCheckpointRecord, StorageCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<StorageImageCheckpointRecord, StorageCheckpointError> {
        let payload = registry
            .chunk(&self.component, STORAGE_IMAGE_CHUNK)
            .ok_or_else(|| StorageCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: STORAGE_IMAGE_CHUNK.to_string(),
            })?;
        let snapshot = decode_storage_image(&self.component, payload)?;
        Ok(StorageImageCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &StorageImageCheckpointSnapshot,
    ) -> Result<(), StorageCheckpointError> {
        self.target
            .validate_snapshot(snapshot)
            .map_err(|error| StorageCheckpointError::Storage {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &StorageImageCheckpointSnapshot,
    ) -> Result<(), StorageCheckpointError> {
        self.target
            .restore_snapshot(snapshot)
            .map_err(|error| StorageCheckpointError::Storage {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug)]
enum StorageImageCheckpointTarget {
    Raw(RawStorageImage),
    Cow(CowStorageImage),
    File(FileStorageImage),
}

impl StorageImageCheckpointTarget {
    fn aliases(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Raw(left), Self::Raw(right)) => Arc::ptr_eq(&left.state, &right.state),
            (Self::Cow(left), Self::Cow(right)) => Arc::ptr_eq(&left.state, &right.state),
            (Self::File(left), Self::File(right)) => left.shares_backing_with(right),
            _ => false,
        }
    }

    fn snapshot(&self) -> Result<StorageImageCheckpointSnapshot, StorageError> {
        match self {
            Self::Raw(image) => Ok(StorageImageCheckpointSnapshot::Raw(image.snapshot())),
            Self::Cow(image) => Ok(StorageImageCheckpointSnapshot::Cow(image.snapshot())),
            Self::File(image) => image.snapshot().map(StorageImageCheckpointSnapshot::File),
        }
    }

    fn validate_snapshot(
        &self,
        snapshot: &StorageImageCheckpointSnapshot,
    ) -> Result<(), StorageError> {
        match (self, snapshot) {
            (Self::Raw(image), StorageImageCheckpointSnapshot::Raw(snapshot)) => {
                validate_raw_storage_snapshot(image, snapshot)
            }
            (Self::Cow(image), StorageImageCheckpointSnapshot::Cow(snapshot)) => {
                validate_cow_storage_snapshot(image, snapshot)
            }
            (Self::File(image), StorageImageCheckpointSnapshot::File(snapshot)) => {
                validate_file_storage_snapshot(image, snapshot)
            }
            _ => Err(StorageError::SnapshotKindMismatch),
        }
    }

    fn restore_snapshot(
        &self,
        snapshot: &StorageImageCheckpointSnapshot,
    ) -> Result<(), StorageError> {
        match (self, snapshot) {
            (Self::Raw(image), StorageImageCheckpointSnapshot::Raw(snapshot)) => {
                image.restore(snapshot)
            }
            (Self::Cow(image), StorageImageCheckpointSnapshot::Cow(snapshot)) => {
                image.restore(snapshot)
            }
            (Self::File(image), StorageImageCheckpointSnapshot::File(snapshot)) => {
                image.restore(snapshot)
            }
            _ => Err(StorageError::SnapshotKindMismatch),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StorageImageCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, StorageImageCheckpointPort>,
}

#[derive(Clone, Debug, Default)]
pub struct IdeControllerCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, IdeControllerCheckpointPort>,
}

impl IdeControllerCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = IdeControllerCheckpointPort>,
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

    pub fn insert_port(
        &mut self,
        port: IdeControllerCheckpointPort,
    ) -> Result<(), CheckpointError> {
        let component = port.component().clone();
        if self.ports.contains_key(&component) {
            return Err(CheckpointError::DuplicateComponent { component });
        }
        self.ports.insert(component, port);
        Ok(())
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
    ) -> Result<Vec<IdeControllerCheckpointRecord>, StorageCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<IdeControllerCheckpointRecord>, StorageCheckpointError> {
        self.validate_restore_from(registry)?;
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            decoded.push((port, record));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_snapshot(record.snapshot())?;
            restored.push(record);
        }
        Ok(restored)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), StorageCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
        }
        Ok(())
    }
}

impl StorageImageCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = StorageImageCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component)
                || by_component
                    .values()
                    .any(|existing: &StorageImageCheckpointPort| existing.aliases_target(&port))
            {
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

    pub fn insert_port(&mut self, port: StorageImageCheckpointPort) -> Result<(), CheckpointError> {
        let component = port.component().clone();
        if self.ports.contains_key(&component)
            || self
                .ports
                .values()
                .any(|existing| existing.aliases_target(&port))
        {
            return Err(CheckpointError::DuplicateComponent { component });
        }
        self.ports.insert(component, port);
        Ok(())
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        let mut registered = Vec::new();
        for port in self.ports.values() {
            match port.register(registry) {
                Ok(()) => registered.push(port.component().clone()),
                Err(error) => {
                    for component in registered {
                        let removed = registry.remove_component(&component);
                        debug_assert!(removed);
                    }
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<StorageImageCheckpointRecord>, StorageCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<StorageImageCheckpointRecord>, StorageCheckpointError> {
        self.validate_restore_from(registry)?;
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            decoded.push((port, record));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_snapshot(record.snapshot())?;
            restored.push(record);
        }
        Ok(restored)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), StorageCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
    Ide {
        component: CheckpointComponentId,
        error: IdeControllerError,
    },
    LockPoisoned {
        component: CheckpointComponentId,
    },
    Storage {
        component: CheckpointComponentId,
        error: StorageError,
    },
}

impl StorageCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Ide { component, .. }
            | Self::LockPoisoned { component }
            | Self::Storage { component, .. } => Some(component),
            Self::Checkpoint(_) => None,
        }
    }
}

impl Display for StorageCheckpointError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "storage checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "storage checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::Ide { component, error } => write!(
                formatter,
                "storage checkpoint component {} IDE restore failed: {error}",
                component.as_str()
            ),
            Self::LockPoisoned { component } => write!(
                formatter,
                "storage checkpoint component {} IDE controller lock is poisoned",
                component.as_str()
            ),
            Self::Storage { component, error } => write!(
                formatter,
                "storage checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for StorageCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Ide { error, .. } => Some(error),
            Self::Storage { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } | Self::LockPoisoned { .. } => {
                None
            }
        }
    }
}

fn encode_storage_image(snapshot: &StorageImageCheckpointSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    match snapshot {
        StorageImageCheckpointSnapshot::Raw(snapshot) => {
            payload.push(STORAGE_RAW_KIND);
            payload.extend(snapshot.capacity_sectors().to_le_bytes());
            payload.extend(snapshot.flush_count().to_le_bytes());
            payload.push(u8::from(snapshot.read_only()));
            payload.extend((snapshot.bytes().len() as u64).to_le_bytes());
            payload.extend(snapshot.bytes());
        }
        StorageImageCheckpointSnapshot::Cow(snapshot) => {
            payload.push(STORAGE_COW_KIND);
            payload.extend(snapshot.capacity_sectors().to_le_bytes());
            payload.extend(snapshot.flush_count().to_le_bytes());
            payload.extend((snapshot.dirty_sectors().len() as u64).to_le_bytes());
            for (sector, data) in snapshot.dirty_sectors() {
                payload.extend(sector.get().to_le_bytes());
                payload.extend(data);
            }
        }
        StorageImageCheckpointSnapshot::File(snapshot) => {
            payload.push(STORAGE_FILE_KIND);
            payload.extend(snapshot.capacity_sectors().to_le_bytes());
            payload.extend(snapshot.flush_count().to_le_bytes());
            payload.push(u8::from(snapshot.read_only()));
            payload.extend((snapshot.bytes().len() as u64).to_le_bytes());
            payload.extend(snapshot.bytes());
        }
    }
    payload
}

fn decode_storage_image(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<StorageImageCheckpointSnapshot, StorageCheckpointError> {
    let mut reader = StorageCheckpointReader::new(component, payload);
    let kind = reader.read_u8("image kind")?;
    let snapshot = match kind {
        STORAGE_RAW_KIND => StorageImageCheckpointSnapshot::Raw(decode_raw_storage(&mut reader)?),
        STORAGE_COW_KIND => StorageImageCheckpointSnapshot::Cow(decode_cow_storage(&mut reader)?),
        STORAGE_FILE_KIND => {
            StorageImageCheckpointSnapshot::File(decode_file_storage(&mut reader)?)
        }
        _ => {
            return Err(reader.invalid(format!("unknown storage image kind {kind}")));
        }
    };
    reader.finish()?;
    Ok(snapshot)
}

fn encode_ide_controller(snapshot: &IdeControllerSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(IDE_CONTROLLER_VERSION);
    for channel in snapshot.channels() {
        encode_ide_channel(&mut payload, channel);
    }
    payload
}

fn encode_ide_channel(payload: &mut Vec<u8>, snapshot: &IdeChannelSnapshot) {
    payload.push(encode_channel_id(snapshot.id()));
    payload.push(encode_device_id(snapshot.selected_device()));
    payload.push(u8::from(snapshot.pending_interrupt()));
    let bmi = snapshot.bmi();
    payload.push(bmi.command());
    payload.push(bmi.status());
    payload.extend(bmi.prd_table().to_le_bytes());
    encode_optional_ide_disk(payload, snapshot.device0());
    encode_optional_ide_disk(payload, snapshot.device1());
}

fn encode_optional_ide_disk(payload: &mut Vec<u8>, snapshot: Option<&IdeDiskSnapshot>) {
    match snapshot {
        Some(snapshot) => {
            payload.push(1);
            encode_ide_disk(payload, snapshot);
        }
        None => payload.push(0),
    }
}

fn encode_ide_disk(payload: &mut Vec<u8>, snapshot: &IdeDiskSnapshot) {
    payload.push(encode_device_id(snapshot.device_id()));
    let task_file = snapshot.task_file();
    payload.push(task_file.error());
    payload.push(task_file.sector_count());
    payload.push(task_file.sector_number());
    payload.push(task_file.cylinder_low());
    payload.push(task_file.cylinder_high());
    payload.push(task_file.drive());
    payload.push(task_file.command());
    payload.push(snapshot.status());
    payload.push(snapshot.control());
    payload.push(u8::from(snapshot.pending_interrupt()));
    encode_ide_transfer(payload, snapshot.transfer());
    encode_ide_pending_command(payload, snapshot.pending_command());
}

fn encode_ide_transfer(payload: &mut Vec<u8>, transfer: Option<&IdeDiskTransferSnapshot>) {
    match transfer {
        None => payload.push(IDE_TRANSFER_NONE),
        Some(IdeDiskTransferSnapshot::Input {
            start_sector,
            cursor,
            payload: transfer_payload,
        }) => {
            payload.push(IDE_TRANSFER_INPUT);
            payload.extend(start_sector.to_le_bytes());
            payload.extend((*cursor as u64).to_le_bytes());
            payload.extend((transfer_payload.len() as u64).to_le_bytes());
            payload.extend(transfer_payload);
        }
        Some(IdeDiskTransferSnapshot::Output {
            start_sector,
            cursor,
            payload: transfer_payload,
        }) => {
            payload.push(IDE_TRANSFER_OUTPUT);
            payload.extend(start_sector.to_le_bytes());
            payload.extend((*cursor as u64).to_le_bytes());
            payload.extend((transfer_payload.len() as u64).to_le_bytes());
            payload.extend(transfer_payload);
        }
        Some(IdeDiskTransferSnapshot::Dma {
            direction,
            start_sector,
            sectors,
        }) => {
            payload.push(IDE_TRANSFER_DMA);
            payload.push(encode_dma_direction(*direction));
            payload.extend(start_sector.to_le_bytes());
            payload.extend(sectors.to_le_bytes());
        }
    }
}

fn encode_ide_pending_command(payload: &mut Vec<u8>, pending: Option<&IdePendingCommandSnapshot>) {
    match pending {
        None => payload.push(IDE_PENDING_COMMAND_NONE),
        Some(pending) => {
            payload.push(IDE_PENDING_COMMAND_MEDIA);
            payload.push(pending.command());
            payload.extend(pending.start_sector().to_le_bytes());
            payload.extend(pending.sectors().to_le_bytes());
        }
    }
}

fn decode_ide_controller(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<IdeControllerSnapshot, StorageCheckpointError> {
    let mut reader = StorageCheckpointReader::new(component, payload);
    let version = reader.read_u8("IDE controller version")?;
    if version != IDE_CONTROLLER_VERSION {
        return Err(reader.invalid(format!("unknown IDE controller version {version}")));
    }
    let primary = decode_ide_channel(&mut reader)?;
    let secondary = decode_ide_channel(&mut reader)?;
    reader.finish()?;
    Ok(IdeControllerSnapshot::from_channels([primary, secondary]))
}

fn decode_ide_channel(
    reader: &mut StorageCheckpointReader<'_>,
) -> Result<IdeChannelSnapshot, StorageCheckpointError> {
    let channel_id = reader.read_u8("IDE channel id")?;
    let id = decode_channel_id(reader, channel_id)?;
    let selected_device_id = reader.read_u8("IDE selected device")?;
    let selected_device = decode_device_id(reader, selected_device_id)?;
    let pending_interrupt = reader.read_bool("IDE channel pending interrupt")?;
    let command = reader.read_u8("IDE BMI command")?;
    let status = reader.read_u8("IDE BMI status")?;
    let prd_table = reader.read_u32("IDE BMI PRD table")?;
    let device0 = decode_optional_ide_disk(reader)?;
    let device1 = decode_optional_ide_disk(reader)?;
    Ok(IdeChannelSnapshot {
        id,
        selected_device,
        pending_interrupt,
        bmi: IdeBmiSnapshot {
            command,
            status,
            prd_table,
        },
        device0,
        device1,
    })
}

fn decode_optional_ide_disk(
    reader: &mut StorageCheckpointReader<'_>,
) -> Result<Option<IdeDiskSnapshot>, StorageCheckpointError> {
    match reader.read_u8("IDE disk presence")? {
        0 => Ok(None),
        1 => decode_ide_disk(reader).map(Some),
        value => Err(reader.invalid(format!("IDE disk presence has invalid value {value}"))),
    }
}

fn decode_ide_disk(
    reader: &mut StorageCheckpointReader<'_>,
) -> Result<IdeDiskSnapshot, StorageCheckpointError> {
    let disk_device_id = reader.read_u8("IDE disk device id")?;
    let device_id = decode_device_id(reader, disk_device_id)?;
    let task_file = IdeTaskFile::from_registers(
        reader.read_u8("IDE task error")?,
        reader.read_u8("IDE task sector count")?,
        reader.read_u8("IDE task sector number")?,
        reader.read_u8("IDE task cylinder low")?,
        reader.read_u8("IDE task cylinder high")?,
        reader.read_u8("IDE task drive")?,
        reader.read_u8("IDE task command")?,
    );
    let status = reader.read_u8("IDE disk status")?;
    let control = reader.read_u8("IDE disk control")?;
    let pending_interrupt = reader.read_bool("IDE disk pending interrupt")?;
    let transfer = decode_ide_transfer(reader)?;
    let pending_command = decode_ide_pending_command(reader)?;
    Ok(IdeDiskSnapshot::from_parts(
        device_id,
        task_file,
        status,
        control,
        pending_interrupt,
        transfer,
        pending_command,
    ))
}

fn decode_ide_transfer(
    reader: &mut StorageCheckpointReader<'_>,
) -> Result<Option<IdeDiskTransferSnapshot>, StorageCheckpointError> {
    let tag = reader.read_u8("IDE transfer tag")?;
    match tag {
        IDE_TRANSFER_NONE => Ok(None),
        IDE_TRANSFER_INPUT | IDE_TRANSFER_OUTPUT => {
            let start_sector = reader.read_u64("IDE transfer start sector")?;
            let cursor = reader.read_count("IDE transfer cursor")?;
            let byte_count = reader.read_count("IDE transfer byte count")?;
            let payload = reader
                .read_bytes("IDE transfer payload", byte_count)?
                .to_vec();
            if tag == IDE_TRANSFER_INPUT {
                Ok(Some(IdeDiskTransferSnapshot::Input {
                    start_sector,
                    cursor,
                    payload,
                }))
            } else {
                Ok(Some(IdeDiskTransferSnapshot::Output {
                    start_sector,
                    cursor,
                    payload,
                }))
            }
        }
        IDE_TRANSFER_DMA => {
            let dma_direction = reader.read_u8("IDE DMA direction")?;
            let direction = decode_dma_direction(reader, dma_direction)?;
            let start_sector = reader.read_u64("IDE DMA start sector")?;
            let sectors = reader.read_u64("IDE DMA sectors")?;
            Ok(Some(IdeDiskTransferSnapshot::Dma {
                direction,
                start_sector,
                sectors,
            }))
        }
        _ => Err(reader.invalid(format!("unknown IDE transfer tag {tag}"))),
    }
}

fn decode_ide_pending_command(
    reader: &mut StorageCheckpointReader<'_>,
) -> Result<Option<IdePendingCommandSnapshot>, StorageCheckpointError> {
    match reader.read_u8("IDE pending command tag")? {
        IDE_PENDING_COMMAND_NONE => Ok(None),
        IDE_PENDING_COMMAND_MEDIA => {
            let command = reader.read_u8("IDE pending command")?;
            let start_sector = reader.read_u64("IDE pending command start sector")?;
            let sectors = reader.read_u64("IDE pending command sectors")?;
            Ok(Some(IdePendingCommandSnapshot::new(
                command,
                start_sector,
                sectors,
            )))
        }
        tag => Err(reader.invalid(format!("unknown IDE pending command tag {tag}"))),
    }
}

fn decode_raw_storage(
    reader: &mut StorageCheckpointReader<'_>,
) -> Result<RawStorageSnapshot, StorageCheckpointError> {
    let capacity_sectors = reader.read_u64("raw capacity")?;
    let flush_count = reader.read_u64("raw flush count")?;
    let read_only = reader.read_bool("raw read-only flag")?;
    let byte_count = reader.read_count("raw byte count")?;
    let bytes = reader.read_bytes("raw bytes", byte_count)?.to_vec();
    validate_decoded_image_shape(reader, "raw", capacity_sectors, bytes.len() as u64)?;
    Ok(RawStorageSnapshot {
        capacity_sectors,
        bytes,
        read_only,
        flush_count,
    })
}

fn decode_cow_storage(
    reader: &mut StorageCheckpointReader<'_>,
) -> Result<CowStorageSnapshot, StorageCheckpointError> {
    let capacity_sectors = reader.read_u64("cow capacity")?;
    let flush_count = reader.read_u64("cow flush count")?;
    let dirty_count = reader.read_count("cow dirty count")?;
    let mut dirty_by_sector = BTreeMap::new();
    for _ in 0..dirty_count {
        let sector = StorageSectorId::new(reader.read_u64("cow dirty sector")?);
        let data = reader
            .read_bytes("cow dirty sector data", STORAGE_SECTOR_BYTES as usize)?
            .try_into()
            .unwrap();
        if dirty_by_sector.insert(sector, data).is_some() {
            return Err(reader.invalid(format!("duplicate dirty sector {}", sector.get())));
        }
    }
    Ok(CowStorageSnapshot {
        capacity_sectors,
        dirty_sectors: dirty_by_sector.into_iter().collect(),
        flush_count,
    })
}

fn decode_file_storage(
    reader: &mut StorageCheckpointReader<'_>,
) -> Result<FileStorageSnapshot, StorageCheckpointError> {
    let capacity_sectors = reader.read_u64("file capacity")?;
    let flush_count = reader.read_u64("file flush count")?;
    let read_only = reader.read_bool("file read-only flag")?;
    let byte_count = reader.read_count("file byte count")?;
    let bytes = reader.read_bytes("file bytes", byte_count)?.to_vec();
    validate_decoded_image_shape(reader, "file", capacity_sectors, bytes.len() as u64)?;
    Ok(FileStorageSnapshot {
        capacity_sectors,
        bytes,
        read_only,
        flush_count,
    })
}

fn validate_decoded_image_shape(
    reader: &StorageCheckpointReader<'_>,
    image_kind: &str,
    capacity_sectors: u64,
    bytes: u64,
) -> Result<(), StorageCheckpointError> {
    validate_storage_bytes(capacity_sectors, bytes)
        .map_err(|error| reader.invalid(format!("{image_kind} {error}")))?;
    Ok(())
}

fn validate_raw_storage_snapshot(
    image: &RawStorageImage,
    snapshot: &RawStorageSnapshot,
) -> Result<(), StorageError> {
    let image_sectors = image.capacity_sectors();
    if snapshot.capacity_sectors() != image_sectors {
        return Err(StorageError::SnapshotCapacityMismatch {
            snapshot_sectors: snapshot.capacity_sectors(),
            image_sectors,
        });
    }
    validate_storage_bytes(snapshot.capacity_sectors(), snapshot.bytes().len() as u64)
}

fn validate_cow_storage_snapshot(
    image: &CowStorageImage,
    snapshot: &CowStorageSnapshot,
) -> Result<(), StorageError> {
    let image_sectors = image.capacity_sectors();
    if snapshot.capacity_sectors() != image_sectors {
        return Err(StorageError::SnapshotCapacityMismatch {
            snapshot_sectors: snapshot.capacity_sectors(),
            image_sectors,
        });
    }
    for (sector, _) in snapshot.dirty_sectors() {
        crate::validate_range(*sector, 1, image_sectors)?;
    }
    Ok(())
}

fn validate_file_storage_snapshot(
    image: &FileStorageImage,
    snapshot: &FileStorageSnapshot,
) -> Result<(), StorageError> {
    validate_storage_bytes(snapshot.capacity_sectors(), snapshot.bytes().len() as u64)?;
    let image_sectors = image.capacity_sectors();
    if snapshot.capacity_sectors() != image_sectors {
        return Err(StorageError::SnapshotCapacityMismatch {
            snapshot_sectors: snapshot.capacity_sectors(),
            image_sectors,
        });
    }
    if !image.can_restore_snapshot() {
        return Err(StorageError::ReadOnly);
    }
    Ok(())
}

fn encode_channel_id(channel: IdeChannelId) -> u8 {
    match channel {
        IdeChannelId::Primary => 0,
        IdeChannelId::Secondary => 1,
    }
}

fn decode_channel_id(
    reader: &StorageCheckpointReader<'_>,
    value: u8,
) -> Result<IdeChannelId, StorageCheckpointError> {
    match value {
        0 => Ok(IdeChannelId::Primary),
        1 => Ok(IdeChannelId::Secondary),
        _ => Err(reader.invalid(format!("unknown IDE channel id {value}"))),
    }
}

fn encode_device_id(device: IdeDeviceId) -> u8 {
    match device {
        IdeDeviceId::Device0 => 0,
        IdeDeviceId::Device1 => 1,
    }
}

fn decode_device_id(
    reader: &StorageCheckpointReader<'_>,
    value: u8,
) -> Result<IdeDeviceId, StorageCheckpointError> {
    match value {
        0 => Ok(IdeDeviceId::Device0),
        1 => Ok(IdeDeviceId::Device1),
        _ => Err(reader.invalid(format!("unknown IDE device id {value}"))),
    }
}

fn encode_dma_direction(direction: IdeDmaDirection) -> u8 {
    match direction {
        IdeDmaDirection::ToGuest => IDE_DMA_TO_GUEST,
        IdeDmaDirection::FromGuest => IDE_DMA_FROM_GUEST,
    }
}

fn decode_dma_direction(
    reader: &StorageCheckpointReader<'_>,
    value: u8,
) -> Result<IdeDmaDirection, StorageCheckpointError> {
    match value {
        IDE_DMA_TO_GUEST => Ok(IdeDmaDirection::ToGuest),
        IDE_DMA_FROM_GUEST => Ok(IdeDmaDirection::FromGuest),
        _ => Err(reader.invalid(format!("unknown IDE DMA direction {value}"))),
    }
}

struct StorageCheckpointReader<'a> {
    component: &'a CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> StorageCheckpointReader<'a> {
    const fn new(component: &'a CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn read_bool(&mut self, name: &str) -> Result<bool, StorageCheckpointError> {
        match self.read_u8(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} has invalid bool value {value}"))),
        }
    }

    fn read_u8(&mut self, name: &str) -> Result<u8, StorageCheckpointError> {
        Ok(self.read_bytes(name, 1)?[0])
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, StorageCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, StorageCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_count(&mut self, name: &str) -> Result<usize, StorageCheckpointError> {
        let value = self.read_u64(name)?;
        usize::try_from(value).map_err(|_| self.invalid(format!("{name} {value} overflows usize")))
    }

    fn read_bytes(&mut self, name: &str, size: usize) -> Result<&'a [u8], StorageCheckpointError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or_else(|| self.invalid(format!("{name} length overflows")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!(
                "{name} needs {size} bytes at offset {} but payload has {}",
                self.offset,
                self.payload.len()
            )));
        }
        let start = self.offset;
        self.offset = end;
        Ok(&self.payload[start..end])
    }

    fn finish(&self) -> Result<(), StorageCheckpointError> {
        if self.offset == self.payload.len() {
            Ok(())
        } else {
            Err(self.invalid(format!(
                "trailing {} bytes",
                self.payload.len() - self.offset
            )))
        }
    }

    fn invalid(&self, reason: String) -> StorageCheckpointError {
        StorageCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
