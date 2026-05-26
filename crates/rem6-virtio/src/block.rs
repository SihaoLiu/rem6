use rem6_memory::ByteMask;

use crate::{VirtioError, VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSpec};

pub const VIRTIO_BLOCK_DEVICE_ID: u16 = 2;

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
        let mut features = 0_u64;
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
        pages
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

fn write_u16(bytes: &mut [u8], offset: u64, value: u16) {
    bytes[offset as usize..offset as usize + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: u64, value: u32) {
    bytes[offset as usize..offset as usize + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: u64, value: u64) {
    bytes[offset as usize..offset as usize + 8].copy_from_slice(&value.to_le_bytes());
}
