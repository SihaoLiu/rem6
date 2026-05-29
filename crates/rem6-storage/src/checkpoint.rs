use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};

use crate::{
    validate_storage_bytes, CowStorageImage, CowStorageSnapshot, FileStorageImage,
    FileStorageSnapshot, RawStorageImage, RawStorageSnapshot, StorageError, StorageSectorId,
    STORAGE_SECTOR_BYTES,
};

const STORAGE_IMAGE_CHUNK: &str = "storage-image";
const STORAGE_RAW_KIND: u8 = 1;
const STORAGE_COW_KIND: u8 = 2;
const STORAGE_FILE_KIND: u8 = 3;
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

impl StorageImageCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = StorageImageCheckpointPort>,
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

    pub fn insert_port(&mut self, port: StorageImageCheckpointPort) -> Result<(), CheckpointError> {
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
            Self::Storage { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
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
    let image_sectors = image.capacity_sectors();
    if snapshot.capacity_sectors() != image_sectors {
        return Err(StorageError::SnapshotCapacityMismatch {
            snapshot_sectors: snapshot.capacity_sectors(),
            image_sectors,
        });
    }
    validate_storage_bytes(snapshot.capacity_sectors(), snapshot.bytes().len() as u64)
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
