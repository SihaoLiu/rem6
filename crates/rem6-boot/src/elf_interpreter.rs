use crate::error::{invalid_elf, BootElfError, BootError};
use crate::metadata::BootElfInterpreter;

pub(crate) fn read_interpreter(
    bytes: &[u8],
    segment: u16,
    file_offset: u64,
    file_size: u64,
) -> Result<BootElfInterpreter, BootError> {
    let file_range = checked_file_range(bytes, file_offset, file_size).map_err(|_| {
        invalid_elf(BootElfError::InterpreterFileRangeOutOfBounds {
            segment,
            offset: file_offset,
            size: file_size,
            image_size: bytes.len() as u64,
        })
    })?;
    let path_end = file_range
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(file_range.len());
    let path = std::str::from_utf8(&file_range[..path_end])
        .map_err(|_| invalid_elf(BootElfError::InvalidInterpreterPath { segment }))?;
    if path.is_empty() {
        return Err(invalid_elf(BootElfError::InvalidInterpreterPath {
            segment,
        }));
    }
    Ok(BootElfInterpreter::new(path, file_offset, file_size))
}

fn checked_file_range(bytes: &[u8], offset: u64, size: u64) -> Result<&[u8], ()> {
    let end = offset.checked_add(size).ok_or(())?;
    let start = usize::try_from(offset).map_err(|_| ())?;
    let end = usize::try_from(end).map_err(|_| ())?;
    bytes.get(start..end).ok_or(())
}
