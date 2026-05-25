use rem6_boot::BootImage;
use rem6_memory::{Address, AddressRange};

use crate::WorkloadError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadBootImage {
    entry: Address,
    segments: Vec<WorkloadBootSegment>,
}

impl WorkloadBootImage {
    pub fn from_boot_image(image: &BootImage) -> Self {
        Self {
            entry: image.entry(),
            segments: image
                .segments()
                .iter()
                .map(|segment| WorkloadBootSegment::new(segment.range(), segment.data().to_vec()))
                .collect(),
        }
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }

    pub fn segments(&self) -> &[WorkloadBootSegment] {
        &self.segments
    }

    pub fn to_boot_image(&self) -> Result<BootImage, WorkloadError> {
        let mut image = BootImage::new(self.entry);
        for segment in &self.segments {
            image = image
                .add_segment(segment.range().start(), segment.data().to_vec())
                .map_err(WorkloadError::Boot)?;
        }
        Ok(image)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadBootSegment {
    range: AddressRange,
    data: Vec<u8>,
}

impl WorkloadBootSegment {
    pub const fn new(range: AddressRange, data: Vec<u8>) -> Self {
        Self { range, data }
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}
