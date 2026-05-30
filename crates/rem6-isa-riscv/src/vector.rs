use std::error::Error;
use std::fmt;

use crate::MemoryWidth;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvVectorElements {
    width: MemoryWidth,
    elements: Vec<u64>,
}

impl RiscvVectorElements {
    pub fn new(width: MemoryWidth, elements: Vec<u64>) -> Result<Self, RiscvVectorError> {
        if let Some(value) = elements
            .iter()
            .copied()
            .find(|value| *value & !element_mask(width) != 0)
        {
            return Err(RiscvVectorError::ElementExceedsWidth { width, value });
        }

        Ok(Self { width, elements })
    }

    pub const fn width(&self) -> MemoryWidth {
        self.width
    }

    pub fn as_slice(&self) -> &[u64] {
        &self.elements
    }

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvVectorTailPolicy {
    Undisturbed,
    AgnosticAllOnes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorCompressPlan {
    vl: usize,
    tail_policy: RiscvVectorTailPolicy,
}

impl RiscvVectorCompressPlan {
    pub const fn new(vl: usize, tail_policy: RiscvVectorTailPolicy) -> Self {
        Self { vl, tail_policy }
    }

    pub const fn vl(self) -> usize {
        self.vl
    }

    pub const fn tail_policy(self) -> RiscvVectorTailPolicy {
        self.tail_policy
    }

    pub fn execute(
        self,
        destination: &RiscvVectorElements,
        source: &RiscvVectorElements,
        mask: &[bool],
    ) -> Result<RiscvVectorCompressResult, RiscvVectorError> {
        validate_compress_shape(self.vl, destination, source, mask)?;

        let mut output = destination.elements.clone();
        let mut compressed_count = 0;

        for (source_element, selected) in source.elements.iter().zip(mask.iter()).take(self.vl) {
            if *selected {
                output[compressed_count] = *source_element;
                compressed_count += 1;
            }
        }

        match self.tail_policy {
            RiscvVectorTailPolicy::Undisturbed => {}
            RiscvVectorTailPolicy::AgnosticAllOnes => {
                let ones = element_mask(source.width);
                output
                    .iter_mut()
                    .skip(compressed_count)
                    .for_each(|element| *element = ones);
            }
        }

        Ok(RiscvVectorCompressResult {
            elements: RiscvVectorElements {
                width: destination.width,
                elements: output,
            },
            compressed_count,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvVectorCompressResult {
    elements: RiscvVectorElements,
    compressed_count: usize,
}

impl RiscvVectorCompressResult {
    pub const fn elements(&self) -> &RiscvVectorElements {
        &self.elements
    }

    pub const fn compressed_count(&self) -> usize {
        self.compressed_count
    }
}

fn validate_compress_shape(
    vl: usize,
    destination: &RiscvVectorElements,
    source: &RiscvVectorElements,
    mask: &[bool],
) -> Result<(), RiscvVectorError> {
    if destination.width != source.width {
        return Err(RiscvVectorError::ElementWidthMismatch {
            destination: destination.width,
            source: source.width,
        });
    }

    if destination.len() != source.len() {
        return Err(RiscvVectorError::ElementCountMismatch {
            destination: destination.len(),
            source: source.len(),
        });
    }

    if vl > source.len() {
        return Err(RiscvVectorError::VlExceedsElementCount {
            vl,
            elements: source.len(),
        });
    }

    if vl > mask.len() {
        return Err(RiscvVectorError::VlExceedsMaskLength {
            vl,
            mask: mask.len(),
        });
    }

    Ok(())
}

const fn element_mask(width: MemoryWidth) -> u64 {
    match width {
        MemoryWidth::Byte => 0xff,
        MemoryWidth::Halfword => 0xffff,
        MemoryWidth::Word => 0xffff_ffff,
        MemoryWidth::Doubleword => u64::MAX,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvVectorError {
    ElementExceedsWidth {
        width: MemoryWidth,
        value: u64,
    },
    ElementWidthMismatch {
        destination: MemoryWidth,
        source: MemoryWidth,
    },
    ElementCountMismatch {
        destination: usize,
        source: usize,
    },
    VlExceedsElementCount {
        vl: usize,
        elements: usize,
    },
    VlExceedsMaskLength {
        vl: usize,
        mask: usize,
    },
}

impl fmt::Display for RiscvVectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ElementExceedsWidth { width, value } => {
                write!(
                    formatter,
                    "RISC-V vector element {value:#x} exceeds {width:?} width"
                )
            }
            Self::ElementWidthMismatch {
                destination,
                source,
            } => {
                write!(
                    formatter,
                    "RISC-V vector destination width {destination:?} does not match source width {source:?}"
                )
            }
            Self::ElementCountMismatch {
                destination,
                source,
            } => {
                write!(
                    formatter,
                    "RISC-V vector destination element count {destination} does not match source count {source}"
                )
            }
            Self::VlExceedsElementCount { vl, elements } => {
                write!(
                    formatter,
                    "RISC-V vector length {vl} exceeds element count {elements}"
                )
            }
            Self::VlExceedsMaskLength { vl, mask } => {
                write!(
                    formatter,
                    "RISC-V vector length {vl} exceeds mask length {mask}"
                )
            }
        }
    }
}

impl Error for RiscvVectorError {}
