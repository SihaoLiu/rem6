use std::ops::Range;
use std::sync::{Arc, Mutex};

use rem6_kernel::{ParallelSchedulerContext, SchedulerContext, Tick};
use rem6_memory::ByteMask;
use rem6_storage::{StorageError, StorageImageLayer, StorageSectorId};

use crate::{
    modern_feature_pages, VirtioError, VirtioPciCommonConfigDevice, VirtioPciDeviceConfigDevice,
    VirtioPciDeviceConfigSpec, VirtioPciNotifyDevice, VirtioQueueIndex, VirtioQueueNotifySpec,
    VirtioQueueSpec,
};

pub const VIRTIO_BLOCK_DEVICE_ID: u16 = 2;
pub const VIRTIO_BLOCK_SECTOR_SIZE: u64 = 512;
pub const VIRTIO_BLOCK_REQUEST_QUEUE_INDEX: u16 = 0;
pub const VIRTIO_BLOCK_DEFAULT_QUEUE_SIZE: u16 = 128;

pub const VIRTIO_BLOCK_F_SIZE_MAX: u64 = 1 << 1;
pub const VIRTIO_BLOCK_F_SEG_MAX: u64 = 1 << 2;
pub const VIRTIO_BLOCK_F_GEOMETRY: u64 = 1 << 4;
pub const VIRTIO_BLOCK_F_RO: u64 = 1 << 5;
pub const VIRTIO_BLOCK_F_BLK_SIZE: u64 = 1 << 6;
pub const VIRTIO_BLOCK_F_FLUSH: u64 = 1 << 9;
pub const VIRTIO_BLOCK_F_TOPOLOGY: u64 = 1 << 10;
pub const VIRTIO_BLOCK_F_CONFIG_WCE: u64 = 1 << 11;
pub const VIRTIO_BLOCK_F_MQ: u64 = 1 << 12;
pub const VIRTIO_BLOCK_F_DISCARD: u64 = 1 << 13;
pub const VIRTIO_BLOCK_F_WRITE_ZEROES: u64 = 1 << 14;
pub const VIRTIO_BLOCK_F_SECURE_ERASE: u64 = 1 << 16;

pub const VIRTIO_BLOCK_T_IN: u32 = 0;
pub const VIRTIO_BLOCK_T_OUT: u32 = 1;
pub const VIRTIO_BLOCK_T_FLUSH: u32 = 4;
pub const VIRTIO_BLOCK_T_GET_ID: u32 = 8;
pub const VIRTIO_BLOCK_T_GET_LIFETIME: u32 = 10;
pub const VIRTIO_BLOCK_T_DISCARD: u32 = 11;
pub const VIRTIO_BLOCK_T_WRITE_ZEROES: u32 = 13;
pub const VIRTIO_BLOCK_T_SECURE_ERASE: u32 = 14;

pub const VIRTIO_BLOCK_S_OK: u8 = 0;
pub const VIRTIO_BLOCK_S_IOERR: u8 = 1;
pub const VIRTIO_BLOCK_S_UNSUPP: u8 = 2;

pub const VIRTIO_BLOCK_CONFIG_CAPACITY_OFFSET: u64 = 0;
pub const VIRTIO_BLOCK_CONFIG_SIZE_MAX_OFFSET: u64 = 8;
pub const VIRTIO_BLOCK_CONFIG_SEG_MAX_OFFSET: u64 = 12;
pub const VIRTIO_BLOCK_CONFIG_CYLINDERS_OFFSET: u64 = 16;
pub const VIRTIO_BLOCK_CONFIG_HEADS_OFFSET: u64 = 18;
pub const VIRTIO_BLOCK_CONFIG_SECTORS_OFFSET: u64 = 19;
pub const VIRTIO_BLOCK_CONFIG_BLK_SIZE_OFFSET: u64 = 20;
pub const VIRTIO_BLOCK_CONFIG_PHYSICAL_BLOCK_EXP_OFFSET: u64 = 24;
pub const VIRTIO_BLOCK_CONFIG_ALIGNMENT_OFFSET_OFFSET: u64 = 25;
pub const VIRTIO_BLOCK_CONFIG_MIN_IO_SIZE_OFFSET: u64 = 26;
pub const VIRTIO_BLOCK_CONFIG_OPT_IO_SIZE_OFFSET: u64 = 28;
pub const VIRTIO_BLOCK_CONFIG_WRITEBACK_OFFSET: u64 = 32;
pub const VIRTIO_BLOCK_CONFIG_UNUSED0_OFFSET: u64 = 33;
pub const VIRTIO_BLOCK_CONFIG_NUM_QUEUES_OFFSET: u64 = 34;
pub const VIRTIO_BLOCK_CONFIG_MAX_DISCARD_SECTORS_OFFSET: u64 = 36;
pub const VIRTIO_BLOCK_CONFIG_MAX_DISCARD_SEG_OFFSET: u64 = 40;
pub const VIRTIO_BLOCK_CONFIG_DISCARD_ALIGNMENT_OFFSET: u64 = 44;
pub const VIRTIO_BLOCK_CONFIG_MAX_WRITE_ZEROES_SECTORS_OFFSET: u64 = 48;
pub const VIRTIO_BLOCK_CONFIG_MAX_WRITE_ZEROES_SEG_OFFSET: u64 = 52;
pub const VIRTIO_BLOCK_CONFIG_WRITE_ZEROES_MAY_UNMAP_OFFSET: u64 = 56;
pub const VIRTIO_BLOCK_CONFIG_UNUSED1_OFFSET: u64 = 57;
pub const VIRTIO_BLOCK_CONFIG_MAX_SECURE_ERASE_SECTORS_OFFSET: u64 = 60;
pub const VIRTIO_BLOCK_CONFIG_MAX_SECURE_ERASE_SEG_OFFSET: u64 = 64;
pub const VIRTIO_BLOCK_CONFIG_SECURE_ERASE_ALIGNMENT_OFFSET: u64 = 68;
pub const VIRTIO_BLOCK_CONFIG_SIZE: u64 = 72;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioBlockCacheMode {
    WriteThrough,
    WriteBack,
}

impl VirtioBlockCacheMode {
    const fn as_config_byte(self) -> u8 {
        match self {
            Self::WriteThrough => 0,
            Self::WriteBack => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioBlockGeometry {
    cylinders: u16,
    heads: u8,
    sectors: u8,
}

impl VirtioBlockGeometry {
    pub fn new(cylinders: u16, heads: u8, sectors: u8) -> Result<Self, VirtioError> {
        if cylinders == 0 || heads == 0 || sectors == 0 {
            return Err(VirtioError::InvalidBlockGeometry {
                cylinders,
                heads,
                sectors,
            });
        }
        Ok(Self {
            cylinders,
            heads,
            sectors,
        })
    }

    pub const fn cylinders(self) -> u16 {
        self.cylinders
    }

    pub const fn heads(self) -> u8 {
        self.heads
    }

    pub const fn sectors(self) -> u8 {
        self.sectors
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioBlockTopology {
    physical_block_exp: u8,
    alignment_offset: u8,
    min_io_size: u16,
    opt_io_size: u32,
}

impl VirtioBlockTopology {
    pub fn new(
        physical_block_exp: u8,
        alignment_offset: u8,
        min_io_size: u16,
        opt_io_size: u32,
    ) -> Result<Self, VirtioError> {
        if physical_block_exp >= 8 {
            return Err(VirtioError::InvalidBlockTopology {
                physical_block_exp,
                alignment_offset,
                min_io_size,
                opt_io_size,
            });
        }
        let physical_blocks = 1_u16 << physical_block_exp;
        if u16::from(alignment_offset) >= physical_blocks || min_io_size == 0 || opt_io_size == 0 {
            return Err(VirtioError::InvalidBlockTopology {
                physical_block_exp,
                alignment_offset,
                min_io_size,
                opt_io_size,
            });
        }
        Ok(Self {
            physical_block_exp,
            alignment_offset,
            min_io_size,
            opt_io_size,
        })
    }

    pub const fn physical_block_exp(self) -> u8 {
        self.physical_block_exp
    }

    pub const fn alignment_offset(self) -> u8 {
        self.alignment_offset
    }

    pub const fn min_io_size(self) -> u16 {
        self.min_io_size
    }

    pub const fn opt_io_size(self) -> u32 {
        self.opt_io_size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioBlockDiscardLimits {
    max_sectors: u32,
    max_segments: u32,
    sector_alignment: u32,
}

impl VirtioBlockDiscardLimits {
    pub fn new(
        max_sectors: u32,
        max_segments: u32,
        sector_alignment: u32,
    ) -> Result<Self, VirtioError> {
        if max_sectors == 0 || max_segments == 0 {
            return Err(VirtioError::InvalidBlockDiscardLimits {
                max_sectors,
                max_segments,
                sector_alignment,
            });
        }
        Ok(Self {
            max_sectors,
            max_segments,
            sector_alignment,
        })
    }

    pub const fn max_sectors(self) -> u32 {
        self.max_sectors
    }

    pub const fn max_segments(self) -> u32 {
        self.max_segments
    }

    pub const fn sector_alignment(self) -> u32 {
        self.sector_alignment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioBlockWriteZeroesLimits {
    max_sectors: u32,
    max_segments: u32,
    may_unmap: bool,
}

impl VirtioBlockWriteZeroesLimits {
    pub fn new(max_sectors: u32, max_segments: u32, may_unmap: bool) -> Result<Self, VirtioError> {
        if max_sectors == 0 || max_segments == 0 {
            return Err(VirtioError::InvalidBlockWriteZeroesLimits {
                max_sectors,
                max_segments,
            });
        }
        Ok(Self {
            max_sectors,
            max_segments,
            may_unmap,
        })
    }

    pub const fn max_sectors(self) -> u32 {
        self.max_sectors
    }

    pub const fn max_segments(self) -> u32 {
        self.max_segments
    }

    pub const fn may_unmap(self) -> bool {
        self.may_unmap
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioBlockSecureEraseLimits {
    max_sectors: u32,
    max_segments: u32,
    sector_alignment: u32,
}

impl VirtioBlockSecureEraseLimits {
    pub fn new(
        max_sectors: u32,
        max_segments: u32,
        sector_alignment: u32,
    ) -> Result<Self, VirtioError> {
        if max_sectors == 0 || max_segments == 0 {
            return Err(VirtioError::InvalidBlockSecureEraseLimits {
                max_sectors,
                max_segments,
                sector_alignment,
            });
        }
        Ok(Self {
            max_sectors,
            max_segments,
            sector_alignment,
        })
    }

    pub const fn max_sectors(self) -> u32 {
        self.max_sectors
    }

    pub const fn max_segments(self) -> u32 {
        self.max_segments
    }

    pub const fn sector_alignment(self) -> u32 {
        self.sector_alignment
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioBlockConfigSpec {
    capacity_sectors: u64,
    size_max: Option<u32>,
    seg_max: Option<u32>,
    geometry: Option<VirtioBlockGeometry>,
    read_only: bool,
    block_size: Option<u32>,
    flush: bool,
    topology: Option<VirtioBlockTopology>,
    writeback: Option<VirtioBlockCacheMode>,
    queues: Option<u16>,
    discard: Option<VirtioBlockDiscardLimits>,
    write_zeroes: Option<VirtioBlockWriteZeroesLimits>,
    secure_erase: Option<VirtioBlockSecureEraseLimits>,
}

impl VirtioBlockConfigSpec {
    pub const fn new(capacity_sectors: u64) -> Self {
        Self {
            capacity_sectors,
            size_max: None,
            seg_max: None,
            geometry: None,
            read_only: false,
            block_size: None,
            flush: false,
            topology: None,
            writeback: None,
            queues: None,
            discard: None,
            write_zeroes: None,
            secure_erase: None,
        }
    }

    pub const fn capacity_sectors(&self) -> u64 {
        self.capacity_sectors
    }

    pub fn with_size_max(mut self, size_max: u32) -> Self {
        self.size_max = Some(size_max);
        self
    }

    pub fn with_seg_max(mut self, seg_max: u32) -> Self {
        self.seg_max = Some(seg_max);
        self
    }

    pub fn with_geometry(mut self, geometry: VirtioBlockGeometry) -> Self {
        self.geometry = Some(geometry);
        self
    }

    pub const fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub fn with_block_size(mut self, block_size: u32) -> Self {
        self.block_size = Some(block_size);
        self
    }

    pub const fn with_flush(mut self, flush: bool) -> Self {
        self.flush = flush;
        self
    }

    pub fn with_topology(mut self, topology: VirtioBlockTopology) -> Self {
        self.topology = Some(topology);
        self
    }

    pub fn with_writeback(mut self, writeback: VirtioBlockCacheMode) -> Self {
        self.writeback = Some(writeback);
        self
    }

    pub fn with_queues(mut self, queues: u16) -> Self {
        self.queues = Some(queues);
        self
    }

    pub fn with_discard(mut self, discard: VirtioBlockDiscardLimits) -> Self {
        self.discard = Some(discard);
        self
    }

    pub fn with_write_zeroes(mut self, write_zeroes: VirtioBlockWriteZeroesLimits) -> Self {
        self.write_zeroes = Some(write_zeroes);
        self
    }

    pub fn with_secure_erase(mut self, secure_erase: VirtioBlockSecureEraseLimits) -> Self {
        self.secure_erase = Some(secure_erase);
        self
    }

    pub fn feature_bits(&self) -> u64 {
        let mut features = crate::VIRTIO_F_VERSION_1;
        if self.size_max.is_some() {
            features |= VIRTIO_BLOCK_F_SIZE_MAX;
        }
        if self.seg_max.is_some() {
            features |= VIRTIO_BLOCK_F_SEG_MAX;
        }
        if self.geometry.is_some() {
            features |= VIRTIO_BLOCK_F_GEOMETRY;
        }
        if self.read_only {
            features |= VIRTIO_BLOCK_F_RO;
        }
        if self.block_size.is_some() {
            features |= VIRTIO_BLOCK_F_BLK_SIZE;
        }
        if self.flush {
            features |= VIRTIO_BLOCK_F_FLUSH;
        }
        if self.topology.is_some() {
            features |= VIRTIO_BLOCK_F_TOPOLOGY;
        }
        if self.writeback.is_some() {
            features |= VIRTIO_BLOCK_F_CONFIG_WCE;
        }
        if self.queues.is_some() {
            features |= VIRTIO_BLOCK_F_MQ;
        }
        if self.discard.is_some() {
            features |= VIRTIO_BLOCK_F_DISCARD;
        }
        if self.write_zeroes.is_some() {
            features |= VIRTIO_BLOCK_F_WRITE_ZEROES;
        }
        if self.secure_erase.is_some() {
            features |= VIRTIO_BLOCK_F_SECURE_ERASE;
        }
        features
    }

    pub fn feature_pages(&self) -> Vec<(u32, u32)> {
        let features = self.feature_bits();
        let mut pages = Vec::new();
        for page in 0..2 {
            let bits = ((features >> (page * 32)) & u64::from(u32::MAX)) as u32;
            if bits != 0 {
                pages.push((page as u32, bits));
            }
        }
        modern_feature_pages(pages)
    }

    pub fn queue_count(&self) -> Result<u16, VirtioError> {
        self.validate()?;
        Ok(self.queues.unwrap_or(1))
    }

    pub fn queue_specs(&self) -> Result<Vec<VirtioQueueSpec>, VirtioError> {
        let queue_count = self.queue_count()?;
        Ok((0..queue_count)
            .map(|index| VirtioQueueSpec::available(VIRTIO_BLOCK_DEFAULT_QUEUE_SIZE, index))
            .collect())
    }

    pub fn notify_specs(&self) -> Result<Vec<VirtioQueueNotifySpec>, VirtioError> {
        let queue_count = self.queue_count()?;
        Ok((0..queue_count)
            .map(|index| {
                VirtioQueueNotifySpec::new(
                    VirtioQueueIndex::new(index).expect("block queue index"),
                    index,
                )
            })
            .collect())
    }

    pub fn device_config_spec(&self) -> Result<VirtioPciDeviceConfigSpec, VirtioError> {
        self.validate()?;
        let mut bytes = vec![0; VIRTIO_BLOCK_CONFIG_SIZE as usize];
        write_u64(
            &mut bytes,
            VIRTIO_BLOCK_CONFIG_CAPACITY_OFFSET,
            self.capacity_sectors,
        );
        if let Some(size_max) = self.size_max {
            write_u32(&mut bytes, VIRTIO_BLOCK_CONFIG_SIZE_MAX_OFFSET, size_max);
        }
        if let Some(seg_max) = self.seg_max {
            write_u32(&mut bytes, VIRTIO_BLOCK_CONFIG_SEG_MAX_OFFSET, seg_max);
        }
        if let Some(geometry) = self.geometry {
            write_u16(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_CYLINDERS_OFFSET,
                geometry.cylinders(),
            );
            bytes[VIRTIO_BLOCK_CONFIG_HEADS_OFFSET as usize] = geometry.heads();
            bytes[VIRTIO_BLOCK_CONFIG_SECTORS_OFFSET as usize] = geometry.sectors();
        }
        if let Some(block_size) = self.block_size {
            write_u32(&mut bytes, VIRTIO_BLOCK_CONFIG_BLK_SIZE_OFFSET, block_size);
        }
        if let Some(topology) = self.topology {
            bytes[VIRTIO_BLOCK_CONFIG_PHYSICAL_BLOCK_EXP_OFFSET as usize] =
                topology.physical_block_exp();
            bytes[VIRTIO_BLOCK_CONFIG_ALIGNMENT_OFFSET_OFFSET as usize] =
                topology.alignment_offset();
            write_u16(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_MIN_IO_SIZE_OFFSET,
                topology.min_io_size(),
            );
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_OPT_IO_SIZE_OFFSET,
                topology.opt_io_size(),
            );
        }
        if let Some(writeback) = self.writeback {
            bytes[VIRTIO_BLOCK_CONFIG_WRITEBACK_OFFSET as usize] = writeback.as_config_byte();
        }
        if let Some(queues) = self.queues {
            write_u16(&mut bytes, VIRTIO_BLOCK_CONFIG_NUM_QUEUES_OFFSET, queues);
        }
        if let Some(discard) = self.discard {
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_MAX_DISCARD_SECTORS_OFFSET,
                discard.max_sectors(),
            );
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_MAX_DISCARD_SEG_OFFSET,
                discard.max_segments(),
            );
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_DISCARD_ALIGNMENT_OFFSET,
                discard.sector_alignment(),
            );
        }
        if let Some(write_zeroes) = self.write_zeroes {
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_MAX_WRITE_ZEROES_SECTORS_OFFSET,
                write_zeroes.max_sectors(),
            );
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_MAX_WRITE_ZEROES_SEG_OFFSET,
                write_zeroes.max_segments(),
            );
            bytes[VIRTIO_BLOCK_CONFIG_WRITE_ZEROES_MAY_UNMAP_OFFSET as usize] =
                u8::from(write_zeroes.may_unmap());
        }
        if let Some(secure_erase) = self.secure_erase {
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_MAX_SECURE_ERASE_SECTORS_OFFSET,
                secure_erase.max_sectors(),
            );
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_MAX_SECURE_ERASE_SEG_OFFSET,
                secure_erase.max_segments(),
            );
            write_u32(
                &mut bytes,
                VIRTIO_BLOCK_CONFIG_SECURE_ERASE_ALIGNMENT_OFFSET,
                secure_erase.sector_alignment(),
            );
        }

        let mut writable = vec![false; VIRTIO_BLOCK_CONFIG_SIZE as usize];
        if self.writeback.is_some() {
            writable[VIRTIO_BLOCK_CONFIG_WRITEBACK_OFFSET as usize] = true;
        }
        VirtioPciDeviceConfigSpec::new(
            bytes,
            ByteMask::from_bits(writable).expect("nonempty block config write mask"),
        )
    }

    pub fn build_device_config(&self) -> Result<VirtioPciDeviceConfigDevice, VirtioError> {
        self.device_config_spec()
            .map(VirtioPciDeviceConfigDevice::new)
    }

    pub fn build_common_config(&self) -> Result<VirtioPciCommonConfigDevice, VirtioError> {
        VirtioPciCommonConfigDevice::new(self.feature_pages(), self.queue_specs()?)
    }

    pub fn build_notify_device(
        &self,
        notify_off_multiplier: u32,
    ) -> Result<VirtioPciNotifyDevice, VirtioError> {
        VirtioPciNotifyDevice::new(notify_off_multiplier, self.notify_specs()?)
    }

    fn validate(&self) -> Result<(), VirtioError> {
        if self.capacity_sectors == 0 {
            return Err(VirtioError::InvalidBlockCapacity);
        }
        if matches!(self.size_max, Some(0)) {
            return Err(VirtioError::InvalidBlockSizeMax { size_max: 0 });
        }
        if matches!(self.seg_max, Some(0)) {
            return Err(VirtioError::InvalidBlockSegMax { seg_max: 0 });
        }
        if let Some(block_size) = self.block_size {
            if block_size < 512 || !block_size.is_power_of_two() {
                return Err(VirtioError::InvalidBlockSize { block_size });
            }
        }
        if self.writeback.is_some() && !self.flush {
            return Err(VirtioError::BlockWritebackRequiresFlush);
        }
        if let Some(queues) = self.queues {
            if queues == 0 {
                return Err(VirtioError::InvalidBlockQueueCount { queues });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VirtioBlockRequestId(u64);

impl VirtioBlockRequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VirtioBlockStatus {
    Ok,
    IoErr,
    Unsupported,
}

impl VirtioBlockStatus {
    pub const fn as_byte(self) -> u8 {
        match self {
            Self::Ok => VIRTIO_BLOCK_S_OK,
            Self::IoErr => VIRTIO_BLOCK_S_IOERR,
            Self::Unsupported => VIRTIO_BLOCK_S_UNSUPP,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtioBlockRequestKind {
    Read { bytes: u64 },
    Write { data: Vec<u8> },
    Flush,
    GetId,
    Unsupported { raw_type: u32, data: Vec<u8> },
}

impl VirtioBlockRequestKind {
    pub const fn raw_type(&self) -> u32 {
        match self {
            Self::Read { .. } => VIRTIO_BLOCK_T_IN,
            Self::Write { .. } => VIRTIO_BLOCK_T_OUT,
            Self::Flush => VIRTIO_BLOCK_T_FLUSH,
            Self::GetId => VIRTIO_BLOCK_T_GET_ID,
            Self::Unsupported { raw_type, .. } => *raw_type,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioBlockRequest {
    id: VirtioBlockRequestId,
    queue: VirtioQueueIndex,
    sector: u64,
    kind: VirtioBlockRequestKind,
}

impl VirtioBlockRequest {
    pub fn read(
        id: VirtioBlockRequestId,
        queue: VirtioQueueIndex,
        sector: u64,
        bytes: u64,
    ) -> Result<Self, VirtioError> {
        validate_block_payload_length(VIRTIO_BLOCK_T_IN, bytes)?;
        Ok(Self {
            id,
            queue,
            sector,
            kind: VirtioBlockRequestKind::Read { bytes },
        })
    }

    pub fn write(
        id: VirtioBlockRequestId,
        queue: VirtioQueueIndex,
        sector: u64,
        data: Vec<u8>,
    ) -> Result<Self, VirtioError> {
        validate_block_payload_length(VIRTIO_BLOCK_T_OUT, data.len() as u64)?;
        Ok(Self {
            id,
            queue,
            sector,
            kind: VirtioBlockRequestKind::Write { data },
        })
    }

    pub fn flush(id: VirtioBlockRequestId, queue: VirtioQueueIndex) -> Result<Self, VirtioError> {
        Ok(Self {
            id,
            queue,
            sector: 0,
            kind: VirtioBlockRequestKind::Flush,
        })
    }

    pub fn get_id(id: VirtioBlockRequestId, queue: VirtioQueueIndex) -> Self {
        Self {
            id,
            queue,
            sector: 0,
            kind: VirtioBlockRequestKind::GetId,
        }
    }

    pub fn unsupported(
        id: VirtioBlockRequestId,
        queue: VirtioQueueIndex,
        raw_type: u32,
        data: Vec<u8>,
    ) -> Result<Self, VirtioError> {
        Ok(Self {
            id,
            queue,
            sector: 0,
            kind: VirtioBlockRequestKind::Unsupported { raw_type, data },
        })
    }

    pub const fn id(&self) -> VirtioBlockRequestId {
        self.id
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn sector(&self) -> u64 {
        self.sector
    }

    pub const fn kind(&self) -> &VirtioBlockRequestKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioBlockCompletion {
    request: VirtioBlockRequestId,
    queue: VirtioQueueIndex,
    tick: Tick,
    status: VirtioBlockStatus,
    data: Option<Vec<u8>>,
}

impl VirtioBlockCompletion {
    fn new(
        request: VirtioBlockRequestId,
        queue: VirtioQueueIndex,
        tick: Tick,
        status: VirtioBlockStatus,
        data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            request,
            queue,
            tick,
            status,
            data,
        }
    }

    pub const fn request(&self) -> VirtioBlockRequestId {
        self.request
    }

    pub const fn queue(&self) -> VirtioQueueIndex {
        self.queue
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn status(&self) -> VirtioBlockStatus {
        self.status
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn status_byte(&self) -> u8 {
        self.status.as_byte()
    }
}

#[derive(Clone, Debug)]
pub struct VirtioBlockMemoryBackend {
    state: Arc<Mutex<VirtioBlockMemoryBackendState>>,
}

impl VirtioBlockMemoryBackend {
    pub fn zeroed(capacity_sectors: u64) -> Result<Self, VirtioError> {
        let bytes = capacity_sectors
            .checked_mul(VIRTIO_BLOCK_SECTOR_SIZE)
            .ok_or(VirtioError::BlockRequestAddressOverflow {
                sector: capacity_sectors,
                bytes: VIRTIO_BLOCK_SECTOR_SIZE,
            })?;
        Ok(Self {
            state: Arc::new(Mutex::new(VirtioBlockMemoryBackendState {
                bytes: zeroed_block_backend_bytes(bytes)?,
                flush_count: 0,
            })),
        })
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, VirtioError> {
        if bytes.is_empty() || !(bytes.len() as u64).is_multiple_of(VIRTIO_BLOCK_SECTOR_SIZE) {
            return Err(VirtioError::InvalidBlockBackendSize {
                bytes: bytes.len() as u64,
            });
        }
        Ok(Self {
            state: Arc::new(Mutex::new(VirtioBlockMemoryBackendState {
                bytes,
                flush_count: 0,
            })),
        })
    }

    pub fn capacity_sectors(&self) -> u64 {
        let state = self.state.lock().expect("virtio block backend lock");
        state.bytes.len() as u64 / VIRTIO_BLOCK_SECTOR_SIZE
    }

    pub fn flush_count(&self) -> u64 {
        self.state
            .lock()
            .expect("virtio block backend lock")
            .flush_count
    }

    pub fn read_sector(&self, sector: u64) -> Result<Vec<u8>, VirtioError> {
        self.read(sector, VIRTIO_BLOCK_SECTOR_SIZE)
    }

    fn read(&self, sector: u64, bytes: u64) -> Result<Vec<u8>, VirtioError> {
        let state = self.state.lock().expect("virtio block backend lock");
        let range = state.byte_range(sector, bytes)?;
        Ok(state.bytes[range].to_vec())
    }

    fn write(&self, sector: u64, data: &[u8]) -> Result<(), VirtioError> {
        let mut state = self.state.lock().expect("virtio block backend lock");
        let range = state.byte_range(sector, data.len() as u64)?;
        state.bytes[range].copy_from_slice(data);
        Ok(())
    }

    fn flush(&self) {
        self.state
            .lock()
            .expect("virtio block backend lock")
            .flush_count += 1;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VirtioBlockMemoryBackendState {
    bytes: Vec<u8>,
    flush_count: u64,
}

impl VirtioBlockMemoryBackendState {
    fn byte_range(&self, sector: u64, bytes: u64) -> Result<Range<usize>, VirtioError> {
        let start = sector
            .checked_mul(VIRTIO_BLOCK_SECTOR_SIZE)
            .ok_or(VirtioError::BlockRequestAddressOverflow { sector, bytes })?;
        let end = start
            .checked_add(bytes)
            .ok_or(VirtioError::BlockRequestAddressOverflow { sector, bytes })?;
        if end > self.bytes.len() as u64 {
            return Err(VirtioError::BlockRequestOutOfRange {
                sector,
                bytes,
                capacity_sectors: self.bytes.len() as u64 / VIRTIO_BLOCK_SECTOR_SIZE,
            });
        }
        Ok(start as usize..end as usize)
    }
}

#[derive(Clone, Debug)]
enum VirtioBlockBackend {
    Memory(VirtioBlockMemoryBackend),
    Storage(Arc<dyn StorageImageLayer>),
}

impl VirtioBlockBackend {
    fn capacity_sectors(&self) -> u64 {
        match self {
            Self::Memory(backend) => backend.capacity_sectors(),
            Self::Storage(backend) => backend.capacity_sectors(),
        }
    }

    fn read(&self, sector: u64, bytes: u64) -> Result<Vec<u8>, VirtioError> {
        match self {
            Self::Memory(backend) => backend.read(sector, bytes),
            Self::Storage(backend) => read_storage_backend(backend.as_ref(), sector, bytes),
        }
    }

    fn write(&self, sector: u64, data: &[u8]) -> Result<(), VirtioError> {
        match self {
            Self::Memory(backend) => backend.write(sector, data),
            Self::Storage(backend) => write_storage_backend(backend.as_ref(), sector, data),
        }
    }

    fn flush(&self) -> Result<(), VirtioError> {
        match self {
            Self::Memory(backend) => {
                backend.flush();
                Ok(())
            }
            Self::Storage(backend) => backend.flush().map_err(storage_error_to_virtio),
        }
    }
}

#[derive(Clone, Debug)]
pub struct VirtioBlockDevice {
    backend: VirtioBlockBackend,
    state: Arc<Mutex<VirtioBlockDeviceState>>,
}

impl VirtioBlockDevice {
    pub fn new(
        config: VirtioBlockConfigSpec,
        backend: VirtioBlockMemoryBackend,
    ) -> Result<Self, VirtioError> {
        Self::new_with_backend(config, VirtioBlockBackend::Memory(backend))
    }

    pub fn new_with_storage(
        config: VirtioBlockConfigSpec,
        backend: Arc<dyn StorageImageLayer>,
    ) -> Result<Self, VirtioError> {
        Self::new_with_backend(config, VirtioBlockBackend::Storage(backend))
    }

    fn new_with_backend(
        config: VirtioBlockConfigSpec,
        backend: VirtioBlockBackend,
    ) -> Result<Self, VirtioError> {
        config.validate()?;
        let backend_sectors = backend.capacity_sectors();
        if config.capacity_sectors() != backend_sectors {
            return Err(VirtioError::BlockBackendCapacityMismatch {
                config_sectors: config.capacity_sectors(),
                backend_sectors,
            });
        }
        Ok(Self {
            backend,
            state: Arc::new(Mutex::new(VirtioBlockDeviceState {
                config,
                device_id: [0; 20],
                completions: Vec::new(),
            })),
        })
    }

    pub fn with_device_id(self, device_id: &str) -> Result<Self, VirtioError> {
        let bytes = device_id.as_bytes();
        if bytes.len() > 20 {
            return Err(VirtioError::InvalidBlockDeviceId { bytes: bytes.len() });
        }
        let mut id = [0; 20];
        id[..bytes.len()].copy_from_slice(bytes);
        self.state
            .lock()
            .expect("virtio block device lock")
            .device_id = id;
        Ok(self)
    }

    pub fn config_spec(&self) -> VirtioBlockConfigSpec {
        self.state
            .lock()
            .expect("virtio block device lock")
            .config
            .clone()
    }

    pub fn feature_pages(&self) -> Vec<(u32, u32)> {
        self.config_spec().feature_pages()
    }

    pub fn queue_count(&self) -> Result<u16, VirtioError> {
        self.config_spec().queue_count()
    }

    pub fn queue_specs(&self) -> Result<Vec<VirtioQueueSpec>, VirtioError> {
        self.config_spec().queue_specs()
    }

    pub fn notify_specs(&self) -> Result<Vec<VirtioQueueNotifySpec>, VirtioError> {
        self.config_spec().notify_specs()
    }

    pub fn build_device_config(&self) -> Result<VirtioPciDeviceConfigDevice, VirtioError> {
        self.config_spec().build_device_config()
    }

    pub fn build_common_config(&self) -> Result<VirtioPciCommonConfigDevice, VirtioError> {
        self.config_spec().build_common_config()
    }

    pub fn build_notify_device(
        &self,
        notify_off_multiplier: u32,
    ) -> Result<VirtioPciNotifyDevice, VirtioError> {
        self.config_spec()
            .build_notify_device(notify_off_multiplier)
    }

    pub fn execute(
        &self,
        context: &mut SchedulerContext<'_>,
        request: VirtioBlockRequest,
    ) -> Result<VirtioBlockCompletion, VirtioError> {
        self.execute_at(context.now(), request)
    }

    pub fn execute_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        request: VirtioBlockRequest,
    ) -> Result<VirtioBlockCompletion, VirtioError> {
        self.execute_at(context.now(), request)
    }

    pub fn execute_at(
        &self,
        tick: Tick,
        request: VirtioBlockRequest,
    ) -> Result<VirtioBlockCompletion, VirtioError> {
        let (status, data) = self.execute_request(&request);
        let completion =
            VirtioBlockCompletion::new(request.id(), request.queue(), tick, status, data);
        self.state
            .lock()
            .expect("virtio block device lock")
            .completions
            .push(completion.clone());
        Ok(completion)
    }

    pub fn completions(&self) -> Vec<VirtioBlockCompletion> {
        self.state
            .lock()
            .expect("virtio block device lock")
            .completions
            .clone()
    }

    fn execute_request(
        &self,
        request: &VirtioBlockRequest,
    ) -> (VirtioBlockStatus, Option<Vec<u8>>) {
        match request.kind() {
            VirtioBlockRequestKind::Read { bytes } => {
                match self.backend.read(request.sector(), *bytes) {
                    Ok(data) => (VirtioBlockStatus::Ok, Some(data)),
                    Err(_) => (VirtioBlockStatus::IoErr, None),
                }
            }
            VirtioBlockRequestKind::Write { data } => {
                let read_only = self
                    .state
                    .lock()
                    .expect("virtio block device lock")
                    .config
                    .read_only;
                if read_only {
                    return (VirtioBlockStatus::IoErr, None);
                }
                match self.backend.write(request.sector(), data) {
                    Ok(()) => (VirtioBlockStatus::Ok, None),
                    Err(_) => (VirtioBlockStatus::IoErr, None),
                }
            }
            VirtioBlockRequestKind::Flush => match self.backend.flush() {
                Ok(()) => (VirtioBlockStatus::Ok, None),
                Err(_) => (VirtioBlockStatus::IoErr, None),
            },
            VirtioBlockRequestKind::GetId => {
                let id = self
                    .state
                    .lock()
                    .expect("virtio block device lock")
                    .device_id
                    .to_vec();
                (VirtioBlockStatus::Ok, Some(id))
            }
            VirtioBlockRequestKind::Unsupported { .. } => (VirtioBlockStatus::Unsupported, None),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VirtioBlockDeviceState {
    config: VirtioBlockConfigSpec,
    device_id: [u8; 20],
    completions: Vec<VirtioBlockCompletion>,
}

fn read_storage_backend(
    backend: &dyn StorageImageLayer,
    sector: u64,
    bytes: u64,
) -> Result<Vec<u8>, VirtioError> {
    let sectors = validate_storage_backend_range(
        sector,
        bytes,
        backend.capacity_sectors(),
        VIRTIO_BLOCK_T_IN,
    )?;
    let mut data = block_request_data_buffer(sector, bytes)?;
    for offset in 0..sectors {
        let sector = sector
            .checked_add(offset)
            .ok_or(VirtioError::BlockRequestAddressOverflow { sector, bytes })?;
        data.extend(
            backend
                .read_sector(StorageSectorId::new(sector))
                .map_err(storage_error_to_virtio)?,
        );
    }
    Ok(data)
}

fn write_storage_backend(
    backend: &dyn StorageImageLayer,
    sector: u64,
    data: &[u8],
) -> Result<(), VirtioError> {
    let bytes = data.len() as u64;
    let sectors = validate_storage_backend_range(
        sector,
        bytes,
        backend.capacity_sectors(),
        VIRTIO_BLOCK_T_OUT,
    )?;
    for offset in 0..sectors {
        let sector = sector
            .checked_add(offset)
            .ok_or(VirtioError::BlockRequestAddressOverflow { sector, bytes })?;
        let start = (offset * VIRTIO_BLOCK_SECTOR_SIZE) as usize;
        let end = start + VIRTIO_BLOCK_SECTOR_SIZE as usize;
        backend
            .write_sector(
                StorageSectorId::new(sector),
                data[start..end].try_into().unwrap(),
            )
            .map_err(storage_error_to_virtio)?;
    }
    Ok(())
}

fn validate_storage_backend_range(
    sector: u64,
    bytes: u64,
    capacity_sectors: u64,
    raw_type: u32,
) -> Result<u64, VirtioError> {
    if bytes == 0 || !bytes.is_multiple_of(VIRTIO_BLOCK_SECTOR_SIZE) {
        return Err(VirtioError::InvalidBlockRequestDataLength { raw_type, bytes });
    }
    let sectors = bytes / VIRTIO_BLOCK_SECTOR_SIZE;
    let end = sector
        .checked_add(sectors)
        .ok_or(VirtioError::BlockRequestAddressOverflow { sector, bytes })?;
    if end > capacity_sectors {
        return Err(VirtioError::BlockRequestOutOfRange {
            sector,
            bytes,
            capacity_sectors,
        });
    }
    Ok(sectors)
}

fn zeroed_block_backend_bytes(bytes: u64) -> Result<Vec<u8>, VirtioError> {
    if bytes > isize::MAX as u64 {
        return Err(VirtioError::BlockBackendTooLarge { bytes });
    }
    let byte_count =
        usize::try_from(bytes).map_err(|_| VirtioError::BlockBackendTooLarge { bytes })?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(byte_count)
        .map_err(|_| VirtioError::BlockBackendTooLarge { bytes })?;
    buffer.resize(byte_count, 0);
    Ok(buffer)
}

fn block_request_data_buffer(sector: u64, bytes: u64) -> Result<Vec<u8>, VirtioError> {
    if bytes > isize::MAX as u64 {
        return Err(VirtioError::BlockRequestTooLarge { sector, bytes });
    }
    let byte_count =
        usize::try_from(bytes).map_err(|_| VirtioError::BlockRequestTooLarge { sector, bytes })?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(byte_count)
        .map_err(|_| VirtioError::BlockRequestTooLarge { sector, bytes })?;
    Ok(buffer)
}

fn storage_error_to_virtio(error: StorageError) -> VirtioError {
    match error {
        StorageError::InvalidImageSize { bytes } => VirtioError::InvalidBlockBackendSize { bytes },
        StorageError::RequestAddressOverflow { sector, sectors } => {
            VirtioError::BlockRequestAddressOverflow {
                sector: sector.get(),
                bytes: sectors.saturating_mul(VIRTIO_BLOCK_SECTOR_SIZE),
            }
        }
        StorageError::OutOfRange {
            sector,
            sectors,
            capacity_sectors,
        } => VirtioError::BlockRequestOutOfRange {
            sector: sector.get(),
            bytes: sectors.saturating_mul(VIRTIO_BLOCK_SECTOR_SIZE),
            capacity_sectors,
        },
        error => VirtioError::BlockStorageBackend {
            message: error.to_string(),
        },
    }
}

fn validate_block_payload_length(raw_type: u32, bytes: u64) -> Result<(), VirtioError> {
    if bytes == 0 || !bytes.is_multiple_of(VIRTIO_BLOCK_SECTOR_SIZE) {
        return Err(VirtioError::InvalidBlockRequestDataLength { raw_type, bytes });
    }
    Ok(())
}

fn write_u16(bytes: &mut [u8], offset: u64, value: u16) {
    bytes[offset as usize..offset as usize + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: u64, value: u32) {
    bytes[offset as usize..offset as usize + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: u64, value: u64) {
    bytes[offset as usize..offset as usize + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_backend_allocation_helper_maps_allocator_refusal() {
        let bytes = isize::MAX as u64;

        assert!(matches!(
            zeroed_block_backend_bytes(bytes),
            Err(VirtioError::BlockBackendTooLarge { bytes: actual }) if actual == bytes
        ));
    }

    #[test]
    fn block_request_allocation_helper_maps_allocator_refusal() {
        let bytes = isize::MAX as u64;

        assert!(matches!(
            block_request_data_buffer(7, bytes),
            Err(VirtioError::BlockRequestTooLarge {
                sector: 7,
                bytes: actual,
            }) if actual == bytes
        ));
    }
}
