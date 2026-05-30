use rem6_boot::{BootElfArchitecture, BootElfClass, BootElfEndian, BootElfOperatingSystem};

pub(crate) fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

pub(crate) fn elf_class_name(class: BootElfClass) -> &'static str {
    match class {
        BootElfClass::Class32 => "ELF32",
        BootElfClass::Class64 => "ELF64",
    }
}

pub(crate) fn elf_endian_name(endian: BootElfEndian) -> &'static str {
    match endian {
        BootElfEndian::Little => "little",
        BootElfEndian::Big => "big",
    }
}

pub(crate) fn elf_architecture_name(architecture: BootElfArchitecture) -> &'static str {
    match architecture {
        BootElfArchitecture::Sparc32 => "sparc32",
        BootElfArchitecture::Sparc64 => "sparc64",
        BootElfArchitecture::Mips => "mips",
        BootElfArchitecture::I386 => "i386",
        BootElfArchitecture::X8664 => "x86_64",
        BootElfArchitecture::Arm => "arm",
        BootElfArchitecture::Thumb => "thumb",
        BootElfArchitecture::Arm64 => "arm64",
        BootElfArchitecture::Riscv32 => "riscv32",
        BootElfArchitecture::Riscv64 => "riscv64",
        BootElfArchitecture::Power => "power",
        BootElfArchitecture::Power64 => "power64",
        BootElfArchitecture::Unknown { .. } => "unknown",
    }
}

pub(crate) fn elf_os_name(os: BootElfOperatingSystem) -> String {
    match os {
        BootElfOperatingSystem::Linux => "linux".to_string(),
        BootElfOperatingSystem::Solaris => "solaris".to_string(),
        BootElfOperatingSystem::Tru64 => "tru64".to_string(),
        BootElfOperatingSystem::LinuxArmOabi => "linux-arm-oabi".to_string(),
        BootElfOperatingSystem::LinuxPower64AbiV1 => "linux-power64-abi-v1".to_string(),
        BootElfOperatingSystem::LinuxPower64AbiV2 => "linux-power64-abi-v2".to_string(),
        BootElfOperatingSystem::FreeBsd => "freebsd".to_string(),
        BootElfOperatingSystem::Unknown { os_abi } => format!("unknown:{os_abi}"),
    }
}

pub(crate) fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped
}
