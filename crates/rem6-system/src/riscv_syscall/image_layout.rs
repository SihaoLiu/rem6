use std::fmt;

use rem6_boot::BootImage;

use super::RISCV_PAGE_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSyscallImageLayoutError {
    UnrepresentableProgramBreak {
        loaded_segment_end: u64,
        page_bytes: u64,
    },
}

impl fmt::Display for RiscvSyscallImageLayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnrepresentableProgramBreak {
                loaded_segment_end,
                page_bytes,
            } => write!(
                formatter,
                "loaded image end {loaded_segment_end:#x} cannot be rounded up to {page_bytes:#x}"
            ),
        }
    }
}

impl std::error::Error for RiscvSyscallImageLayoutError {}

pub(super) fn riscv_program_break_for_boot_image(
    image: &BootImage,
) -> Result<u64, RiscvSyscallImageLayoutError> {
    let end = image.loaded_segment_end().get();
    let mask = RISCV_PAGE_BYTES - 1;
    end.checked_add(mask).map(|value| value & !mask).ok_or(
        RiscvSyscallImageLayoutError::UnrepresentableProgramBreak {
            loaded_segment_end: end,
            page_bytes: RISCV_PAGE_BYTES,
        },
    )
}
