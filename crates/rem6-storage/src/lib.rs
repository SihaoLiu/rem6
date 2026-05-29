use std::collections::BTreeMap;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

mod checkpoint;

pub use checkpoint::{
    StorageCheckpointError, StorageImageCheckpointBank, StorageImageCheckpointPort,
    StorageImageCheckpointRecord, StorageImageCheckpointSnapshot,
};

pub const STORAGE_SECTOR_BYTES: u64 = 512;

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

    pub fn snapshot(&self) -> Result<FileStorageSnapshot, StorageError> {
        let mut state = self.state.lock().expect("file storage image lock");
        let byte_count = checked_capacity_bytes(state.capacity_sectors)?;
        state
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|error| file_operation_error(&self.path, StorageFileOperation::Seek, error))?;
        let mut bytes = vec![0; byte_count];
        state
            .file
            .read_exact(&mut bytes)
            .map_err(|error| file_operation_error(&self.path, StorageFileOperation::Read, error))?;
        Ok(FileStorageSnapshot {
            capacity_sectors: state.capacity_sectors,
            bytes,
            read_only: state.read_only,
            flush_count: state.flush_count,
        })
    }

    pub fn restore(&self, snapshot: &FileStorageSnapshot) -> Result<(), StorageError> {
        let mut state = self.state.lock().expect("file storage image lock");
        validate_storage_bytes(snapshot.capacity_sectors, snapshot.bytes.len() as u64)?;
        if snapshot.capacity_sectors != state.capacity_sectors {
            return Err(StorageError::SnapshotCapacityMismatch {
                snapshot_sectors: snapshot.capacity_sectors,
                image_sectors: state.capacity_sectors,
            });
        }
        if state.read_only {
            return Err(StorageError::ReadOnly);
        }
        state
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|error| file_operation_error(&self.path, StorageFileOperation::Seek, error))?;
        state.file.write_all(&snapshot.bytes).map_err(|error| {
            file_operation_error(&self.path, StorageFileOperation::Write, error)
        })?;
        state.read_only = snapshot.read_only;
        state.flush_count = snapshot.flush_count;
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
pub struct FileStorageSnapshot {
    pub(crate) capacity_sectors: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) read_only: bool,
    pub(crate) flush_count: u64,
}

impl FileStorageSnapshot {
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
    pub(crate) capacity_sectors: u64,
    pub(crate) bytes: Vec<u8>,
    pub(crate) read_only: bool,
    pub(crate) flush_count: u64,
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
    pub(crate) capacity_sectors: u64,
    pub(crate) dirty_sectors: Vec<(StorageSectorId, [u8; 512])>,
    pub(crate) flush_count: u64,
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

fn validate_image_bytes(bytes: u64) -> Result<(), StorageError> {
    if bytes == 0 || !bytes.is_multiple_of(STORAGE_SECTOR_BYTES) {
        return Err(StorageError::InvalidImageSize { bytes });
    }
    Ok(())
}

pub(crate) fn validate_storage_bytes(sectors: u64, bytes: u64) -> Result<(), StorageError> {
    validate_image_bytes(bytes)?;
    let expected = sectors
        .checked_mul(STORAGE_SECTOR_BYTES)
        .ok_or(StorageError::CapacityOverflow { sectors })?;
    if bytes != expected {
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
