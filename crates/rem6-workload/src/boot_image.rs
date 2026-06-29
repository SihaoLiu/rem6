use rem6_boot::{BootElfInterpreter, BootElfMetadata, BootImage};
use rem6_memory::{Address, AddressRange};

use crate::WorkloadError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadBootImage {
    entry: Address,
    elf_metadata: Option<BootElfMetadata>,
    elf_interpreter: Option<BootElfInterpreter>,
    segments: Vec<WorkloadBootSegment>,
}

impl WorkloadBootImage {
    pub fn from_boot_image(image: &BootImage) -> Self {
        Self {
            entry: image.entry(),
            elf_metadata: image.elf_metadata(),
            elf_interpreter: image.elf_interpreter().cloned(),
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

    pub const fn elf_metadata(&self) -> Option<BootElfMetadata> {
        self.elf_metadata
    }

    pub fn elf_interpreter(&self) -> Option<&BootElfInterpreter> {
        self.elf_interpreter.as_ref()
    }

    pub fn segments(&self) -> &[WorkloadBootSegment] {
        &self.segments
    }

    pub fn to_boot_image(&self) -> Result<BootImage, WorkloadError> {
        let mut image = BootImage::new(self.entry);
        if let Some(metadata) = self.elf_metadata {
            image = image.with_elf_metadata(metadata);
        }
        if let Some(interpreter) = &self.elf_interpreter {
            image = image.with_elf_interpreter(interpreter.clone());
        }
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
