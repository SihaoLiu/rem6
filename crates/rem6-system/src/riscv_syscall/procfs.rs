use std::fmt::Write as _;

use super::{RiscvMmapRegion, RiscvSyscallState};

const RISCV_LINUX_PROT_READ: u64 = 0x1;
const RISCV_LINUX_PROT_WRITE: u64 = 0x2;
const RISCV_LINUX_PROT_EXEC: u64 = 0x4;
const RISCV_PROC_SELF_MAPS_COMPONENTS: [&[u8]; 3] = [b"proc", b"self", b"maps"];

impl RiscvSyscallState {
    pub(super) fn virtual_proc_file_contents_for_path(
        &self,
        path: &[u8],
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        let path = virtual_proc_path(self.current_directory(), path)?;
        (path == b"proc/self/maps").then(|| (path, self.proc_self_maps_bytes()))
    }

    fn proc_self_maps_bytes(&self) -> Vec<u8> {
        let mut output = String::new();
        if self.program_break() > self.initial_program_break() {
            push_proc_maps_line(
                &mut output,
                self.initial_program_break(),
                self.program_break(),
                "rw-p",
                0,
                "[heap]",
            );
        }
        for region in self.mmap_regions() {
            push_mmap_region_line(&mut output, region);
        }
        output.into_bytes()
    }
}

fn push_mmap_region_line(output: &mut String, region: &RiscvMmapRegion) {
    let Some(end) = region.start().checked_add(region.length()) else {
        return;
    };
    let label = if region.fd() == u64::MAX {
        "[anon]"
    } else {
        "[file]"
    };
    push_proc_maps_line(
        output,
        region.start(),
        end,
        proc_maps_permissions(region.protection()),
        region.offset(),
        label,
    );
}

fn proc_maps_permissions(protection: u64) -> &'static str {
    match (
        protection & RISCV_LINUX_PROT_READ != 0,
        protection & RISCV_LINUX_PROT_WRITE != 0,
        protection & RISCV_LINUX_PROT_EXEC != 0,
    ) {
        (false, false, false) => "---p",
        (false, false, true) => "--xp",
        (false, true, false) => "-w-p",
        (false, true, true) => "-wxp",
        (true, false, false) => "r--p",
        (true, false, true) => "r-xp",
        (true, true, false) => "rw-p",
        (true, true, true) => "rwxp",
    }
}

fn push_proc_maps_line(
    output: &mut String,
    start: u64,
    end: u64,
    permissions: &str,
    offset: u64,
    label: &str,
) {
    writeln!(
        output,
        "{start:016x}-{end:016x} {permissions} {offset:08x} 00:00 0 {label}"
    )
    .expect("writing to proc maps string cannot fail");
}

fn virtual_proc_path(current_directory: &[u8], path: &[u8]) -> Option<Vec<u8>> {
    let mut components = if path.starts_with(b"/") {
        Vec::new()
    } else {
        virtual_proc_path_components(current_directory)?
    };
    for component in path.split(|byte| *byte == b'/') {
        match component {
            b"" | b"." => {}
            b".." => {
                components.pop();
            }
            _ => {
                components.push(component.to_vec());
                if !is_virtual_proc_path_prefix(&components) {
                    return None;
                }
            }
        }
    }
    Some(join_virtual_proc_path_components(&components))
}

fn virtual_proc_path_components(path: &[u8]) -> Option<Vec<Vec<u8>>> {
    let mut components = Vec::new();
    for component in path
        .strip_prefix(b"/")
        .unwrap_or(path)
        .split(|byte| *byte == b'/')
    {
        match component {
            b"" | b"." => {}
            b".." => {
                components.pop();
            }
            _ => {
                components.push(component.to_vec());
                if !is_virtual_proc_path_prefix(&components) {
                    return None;
                }
            }
        }
    }
    Some(components)
}

fn is_virtual_proc_path_prefix(components: &[Vec<u8>]) -> bool {
    components.len() <= RISCV_PROC_SELF_MAPS_COMPONENTS.len()
        && components
            .iter()
            .zip(RISCV_PROC_SELF_MAPS_COMPONENTS)
            .all(|(component, expected)| component.as_slice() == expected)
}

fn join_virtual_proc_path_components(components: &[Vec<u8>]) -> Vec<u8> {
    let mut path = Vec::new();
    for (index, component) in components.iter().enumerate() {
        if index != 0 {
            path.push(b'/');
        }
        path.extend_from_slice(component);
    }
    path
}
