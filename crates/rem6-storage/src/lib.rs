use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};

pub const STORAGE_SECTOR_BYTES: u64 = 512;
const STORAGE_IMAGE_CHUNK: &str = "storage-image";
const STORAGE_RAW_KIND: u8 = 1;
const STORAGE_COW_KIND: u8 = 2;
const U64_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StorageSectorId(u64);

impl StorageSectorId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageError {
    InvalidImageSize {
        bytes: u64,
    },
    CapacityOverflow {
        sectors: u64,
    },
    RequestAddressOverflow {
        sector: StorageSectorId,
        sectors: u64,
    },
    OutOfRange {
        sector: StorageSectorId,
        sectors: u64,
        capacity_sectors: u64,
    },
    ReadOnly,
    SnapshotCapacityMismatch {
        snapshot_sectors: u64,
        image_sectors: u64,
    },
    SnapshotKindMismatch,
    FileOperationFailed {
        path: PathBuf,
        operation: StorageFileOperation,
        message: String,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageFileOperation {
    Open,
    Metadata,
    Seek,
    Read,
    Write,
    Flush,
}

impl Display for StorageFileOperation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let operation = match self {
            Self::Open => "open",
            Self::Metadata => "metadata",
            Self::Seek => "seek",
            Self::Read => "read",
            Self::Write => "write",
            Self::Flush => "flush",
        };
        formatter.write_str(operation)
    }
}

impl Display for StorageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidImageSize { bytes } => {
                write!(
                    formatter,
                    "storage image size {bytes} is not a nonzero sector multiple"
                )
            }
            Self::CapacityOverflow { sectors } => {
                write!(formatter, "storage capacity {sectors} sectors overflows")
            }
            Self::RequestAddressOverflow { sector, sectors } => write!(
                formatter,
                "storage request at sector {} for {sectors} sectors overflows",
                sector.get()
            ),
            Self::OutOfRange {
                sector,
                sectors,
                capacity_sectors,
            } => write!(
                formatter,
                "storage request at sector {} for {sectors} sectors exceeds capacity {capacity_sectors}",
                sector.get()
            ),
            Self::ReadOnly => write!(formatter, "storage image is read-only"),
            Self::SnapshotCapacityMismatch {
                snapshot_sectors,
                image_sectors,
            } => write!(
                formatter,
                "storage snapshot has {snapshot_sectors} sectors but image has {image_sectors}"
            ),
            Self::SnapshotKindMismatch => {
                write!(formatter, "storage snapshot kind does not match image kind")
            }
            Self::FileOperationFailed {
                path,
                operation,
                message,
            } => write!(
                formatter,
                "storage file {} failed for {}: {message}",
                path.display(),
                operation
            ),
        }
    }
}

impl Error for StorageError {}

pub trait StorageImageLayer: Send + Sync + std::fmt::Debug {
    fn capacity_sectors(&self) -> u64;
    fn read_sector(&self, sector: StorageSectorId) -> Result<[u8; 512], StorageError>;
    fn write_sector(&self, sector: StorageSectorId, data: [u8; 512]) -> Result<(), StorageError>;
    fn flush(&self) -> Result<(), StorageError>;
}

#[derive(Clone, Debug)]
pub struct RawStorageImage {
    state: Arc<Mutex<RawStorageImageState>>,
}

impl RawStorageImage {
    pub fn zeroed(capacity_sectors: u64) -> Result<Self, StorageError> {
        let bytes = checked_capacity_bytes(capacity_sectors)?;
        Self::from_state(vec![0; bytes], false)
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, StorageError> {
        Self::from_state(bytes, false)
    }

    pub fn from_read_only_bytes(bytes: Vec<u8>) -> Result<Self, StorageError> {
        Self::from_state(bytes, true)
    }

    pub fn capacity_sectors(&self) -> u64 {
        let state = self.state.lock().expect("raw storage image lock");
        state.capacity_sectors()
    }

    pub fn flush_count(&self) -> u64 {
        let state = self.state.lock().expect("raw storage image lock");
        state.flush_count
    }

    pub fn read_sector(&self, sector: StorageSectorId) -> Result<[u8; 512], StorageError> {
        let data = self.read(sector, 1)?;
        Ok(data.as_slice().try_into().unwrap())
    }

    pub fn read(&self, sector: StorageSectorId, sectors: u64) -> Result<Vec<u8>, StorageError> {
        let state = self.state.lock().expect("raw storage image lock");
        let range = state.byte_range(sector, sectors)?;
        Ok(state.bytes[range].to_vec())
    }

    pub fn write_sector(
        &self,
        sector: StorageSectorId,
        data: [u8; 512],
    ) -> Result<(), StorageError> {
        self.write(sector, &data)
    }

    pub fn write(&self, sector: StorageSectorId, data: &[u8]) -> Result<(), StorageError> {
        validate_data_sectors(data)?;
        let mut state = self.state.lock().expect("raw storage image lock");
        if state.read_only {
            return Err(StorageError::ReadOnly);
        }
        let range = state.byte_range(sector, data.len() as u64 / STORAGE_SECTOR_BYTES)?;
        state.bytes[range].copy_from_slice(data);
        Ok(())
    }

    pub fn flush(&self) -> Result<(), StorageError> {
        let mut state = self.state.lock().expect("raw storage image lock");
        state.flush_count += 1;
        Ok(())
    }

    pub fn snapshot(&self) -> RawStorageSnapshot {
        let state = self.state.lock().expect("raw storage image lock");
        RawStorageSnapshot {
            capacity_sectors: state.capacity_sectors(),
            bytes: state.bytes.clone(),
            read_only: state.read_only,
            flush_count: state.flush_count,
        }
    }

    pub fn restore(&self, snapshot: &RawStorageSnapshot) -> Result<(), StorageError> {
        let mut state = self.state.lock().expect("raw storage image lock");
        let image_sectors = state.capacity_sectors();
        if snapshot.capacity_sectors != image_sectors {
            return Err(StorageError::SnapshotCapacityMismatch {
                snapshot_sectors: snapshot.capacity_sectors,
                image_sectors,
            });
        }
        state.bytes = snapshot.bytes.clone();
        state.read_only = snapshot.read_only;
        state.flush_count = snapshot.flush_count;
        Ok(())
    }

    fn from_state(bytes: Vec<u8>, read_only: bool) -> Result<Self, StorageError> {
        validate_image_bytes(bytes.len() as u64)?;
        Ok(Self {
            state: Arc::new(Mutex::new(RawStorageImageState {
                bytes,
                read_only,
                flush_count: 0,
            })),
        })
    }
}

impl StorageImageLayer for RawStorageImage {
    fn capacity_sectors(&self) -> u64 {
        self.capacity_sectors()
    }

    fn read_sector(&self, sector: StorageSectorId) -> Result<[u8; 512], StorageError> {
        self.read_sector(sector)
    }

    fn write_sector(&self, sector: StorageSectorId, data: [u8; 512]) -> Result<(), StorageError> {
        self.write_sector(sector, data)
    }

    fn flush(&self) -> Result<(), StorageError> {
        self.flush()
    }
}

#[derive(Debug)]
pub struct FileStorageImage {
    path: PathBuf,
    state: Arc<Mutex<FileStorageImageState>>,
}

impl Clone for FileStorageImage {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

impl FileStorageImage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::open_with_mode(path.as_ref(), false)
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::open_with_mode(path.as_ref(), true)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn capacity_sectors(&self) -> u64 {
        let state = self.state.lock().expect("file storage image lock");
        state.capacity_sectors
    }

    pub fn flush_count(&self) -> u64 {
        let state = self.state.lock().expect("file storage image lock");
        state.flush_count
    }

    pub fn read_sector(&self, sector: StorageSectorId) -> Result<[u8; 512], StorageError> {
        let mut state = self.state.lock().expect("file storage image lock");
        validate_range(sector, 1, state.capacity_sectors)?;
        let offset = sector_offset(sector, 1)?;
        state
            .file
            .seek(SeekFrom::Start(offset))
            .map_err(|error| file_operation_error(&self.path, StorageFileOperation::Seek, error))?;
        let mut data = [0; 512];
        state
            .file
            .read_exact(&mut data)
            .map_err(|error| file_operation_error(&self.path, StorageFileOperation::Read, error))?;
        Ok(data)
    }

    pub fn write_sector(
        &self,
        sector: StorageSectorId,
        data: [u8; 512],
    ) -> Result<(), StorageError> {
        let mut state = self.state.lock().expect("file storage image lock");
        if state.read_only {
            return Err(StorageError::ReadOnly);
        }
        validate_range(sector, 1, state.capacity_sectors)?;
        let offset = sector_offset(sector, 1)?;
        state
            .file
            .seek(SeekFrom::Start(offset))
            .map_err(|error| file_operation_error(&self.path, StorageFileOperation::Seek, error))?;
        state
            .file
            .write_all(&data)
            .map_err(|error| file_operation_error(&self.path, StorageFileOperation::Write, error))
    }

    pub fn flush(&self) -> Result<(), StorageError> {
        let mut state = self.state.lock().expect("file storage image lock");
        state.file.sync_all().map_err(|error| {
            file_operation_error(&self.path, StorageFileOperation::Flush, error)
        })?;
        state.flush_count += 1;
        Ok(())
    }

    fn open_with_mode(path: &Path, read_only: bool) -> Result<Self, StorageError> {
        let path = path.to_path_buf();
        let file = OpenOptions::new()
            .read(true)
            .write(!read_only)
            .open(&path)
            .map_err(|error| file_operation_error(&path, StorageFileOperation::Open, error))?;
        let bytes = file
            .metadata()
            .map_err(|error| file_operation_error(&path, StorageFileOperation::Metadata, error))?
            .len();
        validate_image_bytes(bytes)?;
        Ok(Self {
            path,
            state: Arc::new(Mutex::new(FileStorageImageState {
                file,
                capacity_sectors: bytes / STORAGE_SECTOR_BYTES,
                read_only,
                flush_count: 0,
            })),
        })
    }
}

impl StorageImageLayer for FileStorageImage {
    fn capacity_sectors(&self) -> u64 {
        self.capacity_sectors()
    }

    fn read_sector(&self, sector: StorageSectorId) -> Result<[u8; 512], StorageError> {
        self.read_sector(sector)
    }

    fn write_sector(&self, sector: StorageSectorId, data: [u8; 512]) -> Result<(), StorageError> {
        self.write_sector(sector, data)
    }

    fn flush(&self) -> Result<(), StorageError> {
        self.flush()
    }
}

#[derive(Debug)]
struct FileStorageImageState {
    file: File,
    capacity_sectors: u64,
    read_only: bool,
    flush_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RawStorageImageState {
    bytes: Vec<u8>,
    read_only: bool,
    flush_count: u64,
}

impl RawStorageImageState {
    fn capacity_sectors(&self) -> u64 {
        self.bytes.len() as u64 / STORAGE_SECTOR_BYTES
    }

    fn byte_range(
        &self,
        sector: StorageSectorId,
        sectors: u64,
    ) -> Result<Range<usize>, StorageError> {
        let start = sector
            .get()
            .checked_mul(STORAGE_SECTOR_BYTES)
            .ok_or(StorageError::RequestAddressOverflow { sector, sectors })?;
        let bytes = sectors
            .checked_mul(STORAGE_SECTOR_BYTES)
            .ok_or(StorageError::RequestAddressOverflow { sector, sectors })?;
        let end = start
            .checked_add(bytes)
            .ok_or(StorageError::RequestAddressOverflow { sector, sectors })?;
        if end > self.bytes.len() as u64 {
            return Err(StorageError::OutOfRange {
                sector,
                sectors,
                capacity_sectors: self.capacity_sectors(),
            });
        }
        Ok(start as usize..end as usize)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawStorageSnapshot {
    capacity_sectors: u64,
    bytes: Vec<u8>,
    read_only: bool,
    flush_count: u64,
}

impl RawStorageSnapshot {
    pub const fn capacity_sectors(&self) -> u64 {
        self.capacity_sectors
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn read_only(&self) -> bool {
        self.read_only
    }

    pub const fn flush_count(&self) -> u64 {
        self.flush_count
    }
}

#[derive(Clone, Debug)]
pub struct CowStorageImage {
    child: Arc<dyn StorageImageLayer>,
    state: Arc<Mutex<CowStorageImageState>>,
}

impl CowStorageImage {
    pub fn new(child: Arc<dyn StorageImageLayer>) -> Self {
        Self {
            child,
            state: Arc::new(Mutex::new(CowStorageImageState {
                dirty_sectors: BTreeMap::new(),
                flush_count: 0,
            })),
        }
    }

    pub fn with_raw_child(child: RawStorageImage) -> Self {
        Self::new(Arc::new(child))
    }

    pub fn capacity_sectors(&self) -> u64 {
        self.child.capacity_sectors()
    }

    pub fn flush_count(&self) -> u64 {
        let state = self.state.lock().expect("cow storage image lock");
        state.flush_count
    }

    pub fn dirty_sectors(&self) -> Vec<StorageSectorId> {
        let state = self.state.lock().expect("cow storage image lock");
        state.dirty_sectors.keys().copied().collect()
    }

    pub fn read_sector(&self, sector: StorageSectorId) -> Result<[u8; 512], StorageError> {
        self.validate_range(sector, 1)?;
        if let Some(data) = self
            .state
            .lock()
            .expect("cow storage image lock")
            .dirty_sectors
            .get(&sector)
            .copied()
        {
            return Ok(data);
        }
        self.child.read_sector(sector)
    }

    pub fn read(&self, sector: StorageSectorId, sectors: u64) -> Result<Vec<u8>, StorageError> {
        self.validate_range(sector, sectors)?;
        let mut data = Vec::with_capacity((sectors * STORAGE_SECTOR_BYTES) as usize);
        for offset in 0..sectors {
            data.extend(self.read_sector(StorageSectorId::new(sector.get() + offset))?);
        }
        Ok(data)
    }

    pub fn write_sector(
        &self,
        sector: StorageSectorId,
        data: [u8; 512],
    ) -> Result<(), StorageError> {
        self.validate_range(sector, 1)?;
        self.state
            .lock()
            .expect("cow storage image lock")
            .dirty_sectors
            .insert(sector, data);
        Ok(())
    }

    pub fn write(&self, sector: StorageSectorId, data: &[u8]) -> Result<(), StorageError> {
        validate_data_sectors(data)?;
        let sectors = data.len() as u64 / STORAGE_SECTOR_BYTES;
        self.validate_range(sector, sectors)?;
        for (offset, chunk) in data.chunks_exact(STORAGE_SECTOR_BYTES as usize).enumerate() {
            self.write_sector(
                StorageSectorId::new(sector.get() + offset as u64),
                chunk.try_into().unwrap(),
            )?;
        }
        Ok(())
    }

    pub fn flush(&self) -> Result<(), StorageError> {
        let mut state = self.state.lock().expect("cow storage image lock");
        state.flush_count += 1;
        Ok(())
    }

    pub fn writeback(&self) -> Result<(), StorageError> {
        let dirty_sectors = {
            let state = self.state.lock().expect("cow storage image lock");
            state
                .dirty_sectors
                .iter()
                .map(|(sector, data)| (*sector, *data))
                .collect::<Vec<_>>()
        };
        for (sector, data) in dirty_sectors {
            self.child.write_sector(sector, data)?;
        }
        Ok(())
    }

    pub fn snapshot(&self) -> CowStorageSnapshot {
        let state = self.state.lock().expect("cow storage image lock");
        CowStorageSnapshot {
            capacity_sectors: self.capacity_sectors(),
            dirty_sectors: state
                .dirty_sectors
                .iter()
                .map(|(sector, data)| (*sector, *data))
                .collect(),
            flush_count: state.flush_count,
        }
    }

    pub fn restore(&self, snapshot: &CowStorageSnapshot) -> Result<(), StorageError> {
        let image_sectors = self.capacity_sectors();
        if snapshot.capacity_sectors != image_sectors {
            return Err(StorageError::SnapshotCapacityMismatch {
                snapshot_sectors: snapshot.capacity_sectors,
                image_sectors,
            });
        }
        for (sector, _) in &snapshot.dirty_sectors {
            validate_range(*sector, 1, image_sectors)?;
        }
        let mut state = self.state.lock().expect("cow storage image lock");
        state.dirty_sectors = snapshot.dirty_sectors.iter().copied().collect();
        state.flush_count = snapshot.flush_count;
        Ok(())
    }

    fn validate_range(&self, sector: StorageSectorId, sectors: u64) -> Result<(), StorageError> {
        validate_range(sector, sectors, self.capacity_sectors())
    }
}

impl StorageImageLayer for CowStorageImage {
    fn capacity_sectors(&self) -> u64 {
        self.capacity_sectors()
    }

    fn read_sector(&self, sector: StorageSectorId) -> Result<[u8; 512], StorageError> {
        self.read_sector(sector)
    }

    fn write_sector(&self, sector: StorageSectorId, data: [u8; 512]) -> Result<(), StorageError> {
        self.write_sector(sector, data)
    }

    fn flush(&self) -> Result<(), StorageError> {
        self.flush()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CowStorageImageState {
    dirty_sectors: BTreeMap<StorageSectorId, [u8; 512]>,
    flush_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CowStorageSnapshot {
    capacity_sectors: u64,
    dirty_sectors: Vec<(StorageSectorId, [u8; 512])>,
    flush_count: u64,
}

impl CowStorageSnapshot {
    pub const fn capacity_sectors(&self) -> u64 {
        self.capacity_sectors
    }

    pub fn dirty_sectors(&self) -> &[(StorageSectorId, [u8; 512])] {
        &self.dirty_sectors
    }

    pub const fn flush_count(&self) -> u64 {
        self.flush_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageImageCheckpointSnapshot {
    Raw(RawStorageSnapshot),
    Cow(CowStorageSnapshot),
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
        let snapshot = self.target.snapshot();
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
}

impl StorageImageCheckpointTarget {
    fn snapshot(&self) -> StorageImageCheckpointSnapshot {
        match self {
            Self::Raw(image) => StorageImageCheckpointSnapshot::Raw(image.snapshot()),
            Self::Cow(image) => StorageImageCheckpointSnapshot::Cow(image.snapshot()),
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
    validate_decoded_raw_shape(reader, capacity_sectors, bytes.len() as u64)?;
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

fn validate_decoded_raw_shape(
    reader: &StorageCheckpointReader<'_>,
    capacity_sectors: u64,
    bytes: u64,
) -> Result<(), StorageCheckpointError> {
    validate_image_bytes(bytes).map_err(|error| reader.invalid(error.to_string()))?;
    let expected = capacity_sectors
        .checked_mul(STORAGE_SECTOR_BYTES)
        .ok_or_else(|| reader.invalid(format!("raw capacity {capacity_sectors} overflows")))?;
    if bytes != expected {
        return Err(reader.invalid(format!(
            "raw byte count {bytes} does not match capacity {capacity_sectors}"
        )));
    }
    Ok(())
}

fn validate_raw_storage_snapshot(
    image: &RawStorageImage,
    snapshot: &RawStorageSnapshot,
) -> Result<(), StorageError> {
    let image_sectors = image.capacity_sectors();
    if snapshot.capacity_sectors != image_sectors {
        return Err(StorageError::SnapshotCapacityMismatch {
            snapshot_sectors: snapshot.capacity_sectors,
            image_sectors,
        });
    }
    validate_image_bytes(snapshot.bytes.len() as u64)?;
    let expected = snapshot
        .capacity_sectors
        .checked_mul(STORAGE_SECTOR_BYTES)
        .ok_or(StorageError::CapacityOverflow {
            sectors: snapshot.capacity_sectors,
        })?;
    if snapshot.bytes.len() as u64 != expected {
        return Err(StorageError::InvalidImageSize {
            bytes: snapshot.bytes.len() as u64,
        });
    }
    Ok(())
}

fn validate_cow_storage_snapshot(
    image: &CowStorageImage,
    snapshot: &CowStorageSnapshot,
) -> Result<(), StorageError> {
    let image_sectors = image.capacity_sectors();
    if snapshot.capacity_sectors != image_sectors {
        return Err(StorageError::SnapshotCapacityMismatch {
            snapshot_sectors: snapshot.capacity_sectors,
            image_sectors,
        });
    }
    for (sector, _) in &snapshot.dirty_sectors {
        validate_range(*sector, 1, image_sectors)?;
    }
    Ok(())
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

fn validate_image_bytes(bytes: u64) -> Result<(), StorageError> {
    if bytes == 0 || !bytes.is_multiple_of(STORAGE_SECTOR_BYTES) {
        return Err(StorageError::InvalidImageSize { bytes });
    }
    Ok(())
}

fn validate_data_sectors(data: &[u8]) -> Result<(), StorageError> {
    validate_image_bytes(data.len() as u64)
}

fn sector_offset(sector: StorageSectorId, sectors: u64) -> Result<u64, StorageError> {
    sector
        .get()
        .checked_mul(STORAGE_SECTOR_BYTES)
        .ok_or(StorageError::RequestAddressOverflow { sector, sectors })
}

fn file_operation_error(
    path: &Path,
    operation: StorageFileOperation,
    error: io::Error,
) -> StorageError {
    StorageError::FileOperationFailed {
        path: path.to_path_buf(),
        operation,
        message: error.to_string(),
    }
}

fn checked_capacity_bytes(sectors: u64) -> Result<usize, StorageError> {
    let bytes = sectors
        .checked_mul(STORAGE_SECTOR_BYTES)
        .ok_or(StorageError::CapacityOverflow { sectors })?;
    validate_image_bytes(bytes)?;
    usize::try_from(bytes).map_err(|_| StorageError::CapacityOverflow { sectors })
}

fn validate_range(
    sector: StorageSectorId,
    sectors: u64,
    capacity_sectors: u64,
) -> Result<(), StorageError> {
    let end = sector
        .get()
        .checked_add(sectors)
        .ok_or(StorageError::RequestAddressOverflow { sector, sectors })?;
    if end > capacity_sectors {
        return Err(StorageError::OutOfRange {
            sector,
            sectors,
            capacity_sectors,
        });
    }
    Ok(())
}
