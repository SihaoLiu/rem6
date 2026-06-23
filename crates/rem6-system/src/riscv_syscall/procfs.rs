use std::fmt::Write as _;

use super::{
    guest_fd_argument, RiscvMmapRegion, RiscvSyscallState, RISCV_LINUX_EINVAL, RISCV_LINUX_ENOTDIR,
};

const RISCV_LINUX_PROT_READ: u64 = 0x1;
const RISCV_LINUX_PROT_WRITE: u64 = 0x2;
const RISCV_LINUX_PROT_EXEC: u64 = 0x4;
impl RiscvSyscallState {
    pub(super) fn virtual_proc_file_contents_for_path(
        &self,
        path: &[u8],
    ) -> Option<(Vec<u8>, Vec<u8>)> {
        let path = virtual_proc_path(self.current_directory(), path)?;
        match path.as_slice() {
            b"proc/self/comm" => Some((path, self.proc_self_comm_bytes())),
            b"proc/self/maps" => Some((path, self.proc_self_maps_bytes())),
            b"proc/self/status" => Some((path, self.proc_self_status_bytes())),
            _ => None,
        }
    }

    pub(super) fn virtual_proc_link_target_result_for_path(
        &self,
        path: &[u8],
    ) -> Result<Option<Vec<u8>>, u64> {
        let resolved = match virtual_proc_path_result(self.current_directory(), path) {
            Some(resolved) => resolved,
            None => return Ok(None),
        };
        if resolved.crossed_proc_cwd_link {
            return Err(RISCV_LINUX_EINVAL);
        }
        if resolved.path == b"proc/self/cwd" {
            return Ok(Some(self.current_directory().to_vec()));
        }
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
            .cloned()
            .or_else(|| self.guest_directory_paths.get(&description).cloned())
            .or_else(|| self.guest_pipe_proc_fd_link_target(description))
        else {
            return Ok(None);
        };
        if resolved.crossed_proc_fd_link {
            return Err(RISCV_LINUX_ENOTDIR);
        }
        Ok(Some(target))
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

    fn proc_self_comm_bytes(&self) -> Vec<u8> {
        let mut output = self.process_name_text();
        output.push(b'\n');
        output
    }

    fn proc_self_status_bytes(&self) -> Vec<u8> {
        let identity = self.identity();
        let mut output = Vec::new();
        output.extend_from_slice(b"Name:\t");
        output.extend_from_slice(&self.process_name_text());
        output.push(b'\n');
        push_proc_status_octal_line(&mut output, b"Umask", u64::from(self.file_creation_mask()));
        output.extend_from_slice(b"State:\tR (running)\n");
        push_proc_status_decimal_line(&mut output, b"Tgid", identity.thread_group_id());
        push_proc_status_decimal_line(&mut output, b"Ngid", 0);
        push_proc_status_decimal_line(&mut output, b"Pid", identity.thread_id());
        push_proc_status_decimal_line(&mut output, b"PPid", identity.parent_process_id());
        push_proc_status_decimal_line(&mut output, b"TracerPid", 0);
        push_proc_status_id_line(
            &mut output,
            b"Uid",
            identity.user_id(),
            identity.effective_user_id(),
            identity.saved_user_id(),
            identity.file_system_user_id(),
        );
        push_proc_status_id_line(
            &mut output,
            b"Gid",
            identity.group_id(),
            identity.effective_group_id(),
            identity.saved_group_id(),
            identity.file_system_group_id(),
        );
        output.extend_from_slice(b"Groups:\t\n");
        push_proc_status_decimal_line(&mut output, b"NStgid", identity.thread_group_id());
        push_proc_status_decimal_line(&mut output, b"NSpid", identity.thread_id());
        push_proc_status_decimal_line(
            &mut output,
            b"NSpgid",
            u64::from(self.guest_wait.current_process_group().get()),
        );
        push_proc_status_decimal_line(&mut output, b"NSsid", self.session_id());
        push_proc_status_decimal_line(&mut output, b"Threads", 1);
        push_proc_status_decimal_line(&mut output, b"NoNewPrivs", u64::from(self.no_new_privs()));
        output
    }

    fn process_name_text(&self) -> Vec<u8> {
        let name = self.process_name();
        let end = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        name[..end].to_vec()
    }
}

fn push_proc_status_decimal_line(output: &mut Vec<u8>, label: &[u8], value: u64) {
    output.extend_from_slice(label);
    output.extend_from_slice(b":\t");
    output.extend_from_slice(value.to_string().as_bytes());
    output.push(b'\n');
}

fn push_proc_status_octal_line(output: &mut Vec<u8>, label: &[u8], value: u64) {
    output.extend_from_slice(label);
    output.extend_from_slice(b":\t");
    output.extend_from_slice(format!("{value:04o}").as_bytes());
    output.push(b'\n');
}

fn push_proc_status_id_line(
    output: &mut Vec<u8>,
    label: &[u8],
    real: u64,
    effective: u64,
    saved: u64,
    file_system: u64,
) {
    output.extend_from_slice(label);
    output.extend_from_slice(b":\t");
    output.extend_from_slice(real.to_string().as_bytes());
    output.push(b'\t');
    output.extend_from_slice(effective.to_string().as_bytes());
    output.push(b'\t');
    output.extend_from_slice(saved.to_string().as_bytes());
    output.push(b'\t');
    output.extend_from_slice(file_system.to_string().as_bytes());
    output.push(b'\n');
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
    crossed_proc_cwd_link: bool,
    crossed_proc_fd_link: bool,
}

fn virtual_proc_path(current_directory: &[u8], path: &[u8]) -> Option<Vec<u8>> {
    let resolved = virtual_proc_path_result(current_directory, path)?;
    (!resolved.crossed_proc_cwd_link && !resolved.crossed_proc_fd_link).then_some(resolved.path)
}

fn virtual_proc_path_result(current_directory: &[u8], path: &[u8]) -> Option<VirtualProcPath> {
    let mut components = if path.starts_with(b"/") {
        Vec::new()
    } else {
        virtual_proc_path_components(current_directory)?
    };
    let mut crossed_proc_cwd_link = false;
    let mut crossed_proc_fd_link = false;
    for component in path.split(|byte| *byte == b'/') {
        if is_proc_self_cwd_component(&components) {
            crossed_proc_cwd_link = true;
        }
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
        crossed_proc_cwd_link,
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

fn is_proc_self_cwd_component(components: &[Vec<u8>]) -> bool {
    matches!(components, [proc, current, cwd]
        if proc.as_slice() == b"proc"
            && current.as_slice() == b"self"
            && cwd.as_slice() == b"cwd")
}

fn is_virtual_proc_path_prefix(components: &[Vec<u8>]) -> bool {
    match components {
        [] => true,
        [proc] => proc.as_slice() == b"proc",
        [proc, current] => proc.as_slice() == b"proc" && current.as_slice() == b"self",
        [proc, current, leaf] => {
            proc.as_slice() == b"proc"
                && current.as_slice() == b"self"
                && (leaf.as_slice() == b"maps"
                    || leaf.as_slice() == b"comm"
                    || leaf.as_slice() == b"status"
                    || leaf.as_slice() == b"fd"
                    || leaf.as_slice() == b"cwd")
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
