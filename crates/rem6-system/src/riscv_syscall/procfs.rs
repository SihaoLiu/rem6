use std::fmt::Write as _;

use super::{guest_fd_argument, RiscvMmapRegion, RiscvSyscallState, RISCV_LINUX_ENOTDIR};

const RISCV_LINUX_PROT_READ: u64 = 0x1;
const RISCV_LINUX_PROT_WRITE: u64 = 0x2;
const RISCV_LINUX_PROT_EXEC: u64 = 0x4;
impl RiscvSyscallState {
    pub(super) fn virtual_proc_file_contents_for_path(
        &self,
        path: &[u8],
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        let path = virtual_proc_path(self.current_directory(), path)?;
        (path == b"proc/self/maps").then(|| (path, self.proc_self_maps_bytes()))
    }

    pub(super) fn virtual_proc_link_target_result_for_path(
        &self,
        path: &[u8],
    ) -> Result<Option<Vec<u8>>, u64> {
        let resolved = match virtual_proc_path_result(self.current_directory(), path) {
            Some(resolved) => resolved,
            None => return Ok(None),
        };
        let Some(fd_text) = resolved.path.strip_prefix(b"proc/self/fd/") else {
            return Ok(None);
        };
        let Some(fd) = parse_proc_fd_component(fd_text).and_then(guest_fd_argument) else {
            return Ok(None);
        };
        let Ok(description) = self
            .guest_fds
            .description_for_fd(fd)
            .map(|description| description.id())
        else {
            return Ok(None);
        };
        let Some(target) = self
            .guest_file_description_paths
            .get(&description)
            .or_else(|| self.guest_directory_paths.get(&description))
        else {
            return Ok(None);
        };
        if resolved.crossed_proc_fd_link {
            return Err(RISCV_LINUX_ENOTDIR);
        }
        Ok(Some(target.clone()))
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

struct VirtualProcPath {
    path: Vec<u8>,
    crossed_proc_fd_link: bool,
}

fn virtual_proc_path(current_directory: &[u8], path: &[u8]) -> Option<Vec<u8>> {
    let resolved = virtual_proc_path_result(current_directory, path)?;
    (!resolved.crossed_proc_fd_link).then_some(resolved.path)
}

fn virtual_proc_path_result(current_directory: &[u8], path: &[u8]) -> Option<VirtualProcPath> {
    let mut components = if path.starts_with(b"/") {
        Vec::new()
    } else {
        virtual_proc_path_components(current_directory)?
    };
    let mut crossed_proc_fd_link = false;
    for component in path.split(|byte| *byte == b'/') {
        if is_proc_self_fd_component(&components) {
            crossed_proc_fd_link = true;
        }
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
    Some(VirtualProcPath {
        path: join_virtual_proc_path_components(&components),
        crossed_proc_fd_link,
    })
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

fn is_proc_self_fd_component(components: &[Vec<u8>]) -> bool {
    matches!(components, [proc, current, fd, number]
        if proc.as_slice() == b"proc"
            && current.as_slice() == b"self"
            && fd.as_slice() == b"fd"
            && is_virtual_proc_fd_component(number))
}

fn is_virtual_proc_path_prefix(components: &[Vec<u8>]) -> bool {
    match components {
        [] => true,
        [proc] => proc.as_slice() == b"proc",
        [proc, current] => proc.as_slice() == b"proc" && current.as_slice() == b"self",
        [proc, current, leaf] => {
            proc.as_slice() == b"proc"
                && current.as_slice() == b"self"
                && (leaf.as_slice() == b"maps" || leaf.as_slice() == b"fd")
        }
        [proc, current, fd, number] => {
            proc.as_slice() == b"proc"
                && current.as_slice() == b"self"
                && fd.as_slice() == b"fd"
                && is_virtual_proc_fd_component(number)
        }
        _ => false,
    }
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

fn parse_proc_fd_component(component: &[u8]) -> Option<u64> {
    if !is_canonical_proc_fd_component(component) {
        return None;
    }
    let mut value = 0_u64;
    for digit in component {
        value = value
            .checked_mul(10)?
            .checked_add(u64::from(digit.checked_sub(b'0')?))?;
    }
    Some(value)
}

fn is_virtual_proc_fd_component(component: &[u8]) -> bool {
    !component.is_empty() && component.iter().all(u8::is_ascii_digit)
}

fn is_canonical_proc_fd_component(component: &[u8]) -> bool {
    is_virtual_proc_fd_component(component)
        && (component.len() == 1 || component.first().copied() != Some(b'0'))
}
