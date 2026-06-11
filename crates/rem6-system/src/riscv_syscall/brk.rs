use super::{RiscvGuestMemoryMapResult, RiscvGuestMemoryWriter, RiscvSyscallState};
use crate::riscv_syscall::RISCV_PAGE_BYTES;

pub(super) fn syscall_brk(
    requested: u64,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    if requested == 0 {
        return state.program_break();
    }

    let old_break = state.program_break();
    if requested <= old_break {
        state.set_program_break(requested);
        return state.program_break();
    }

    if let Some(backing_end) = install_heap_backing(
        state.program_break_backing_end(),
        requested,
        guest_memory_writer,
    ) {
        state.set_program_break_backing_end(backing_end);
        state.set_program_break(requested);
    }
    state.program_break()
}

fn install_heap_backing(
    old_backing_end: u64,
    requested: u64,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let Some(guest_memory_writer) = guest_memory_writer else {
        return align_up_to_page(requested);
    };
    let map_start = align_up_to_page(old_backing_end)?;
    let map_end = align_up_to_page(requested)?;
    if map_end <= map_start {
        return Some(old_backing_end.max(map_end));
    }
    let length = map_end - map_start;

    match guest_memory_writer.map_region(map_start, length, false) {
        RiscvGuestMemoryMapResult::Mapped => {}
        RiscvGuestMemoryMapResult::Overlap | RiscvGuestMemoryMapResult::Failed => return None,
    }

    write_zeroed_heap(map_start, length, guest_memory_writer).then_some(map_end)
}

fn write_zeroed_heap(
    start: u64,
    length: u64,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> bool {
    let zero_page = [0; RISCV_PAGE_BYTES as usize];
    let mut cursor = start;
    let mut remaining = length;
    while remaining > 0 {
        let bytes = remaining.min(RISCV_PAGE_BYTES);
        if !guest_memory_writer.write(cursor, &zero_page[..bytes as usize]) {
            return false;
        }
        let Some(next) = cursor.checked_add(bytes) else {
            return false;
        };
        cursor = next;
        remaining -= bytes;
    }
    true
}

fn align_up_to_page(value: u64) -> Option<u64> {
    value
        .checked_add(RISCV_PAGE_BYTES - 1)
        .map(|rounded| rounded & !(RISCV_PAGE_BYTES - 1))
}
