use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfEndian, BootElfMetadata, BootElfOperatingSystem,
};

use super::{hash_str, hash_u64};

pub(super) fn hash_elf_metadata(hash: &mut u64, metadata: Option<BootElfMetadata>) {
    match metadata {
        Some(metadata) => {
            hash_u64(hash, 1);
            hash_elf_class(hash, metadata.class());
            hash_elf_endian(hash, metadata.endian());
            hash_u64(hash, u64::from(metadata.machine()));
            hash_u64(hash, u64::from(metadata.os_abi()));
            hash_u64(hash, u64::from(metadata.flags()));
            hash_elf_architecture(hash, metadata.architecture());
            hash_elf_operating_system(hash, metadata.operating_system());
            if metadata.has_tls() {
                hash_str(hash, "elf.tls");
            }
            let symbols = [
                metadata.symbol_count(),
                metadata.function_symbol_count(),
                metadata.object_symbol_count(),
            ];
            if symbols.iter().any(|value| *value != 0) {
                hash_str(hash, "elf.symbols");
                symbols.iter().for_each(|value| hash_u64(hash, *value));
            }
            let dynamic = metadata.dynamic_table();
            if dynamic.segment_count() != 0 {
                let address = dynamic.virtual_address().map_or(u64::MAX, |a| a.get());
                hash_str(hash, "elf.dynamic");
                for value in [
                    dynamic.segment_count(),
                    dynamic.file_offset().unwrap_or(u64::MAX),
                    address,
                    u64::from(dynamic.entry_size()),
                    dynamic.entry_count(),
                    dynamic.needed_count(),
                ] {
                    hash_u64(hash, value);
                }
            }
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_elf_class(hash: &mut u64, class: BootElfClass) {
    let value = match class {
        BootElfClass::Class32 => 1,
        BootElfClass::Class64 => 2,
    };
    hash_u64(hash, value);
}

fn hash_elf_endian(hash: &mut u64, endian: BootElfEndian) {
    let value = match endian {
        BootElfEndian::Little => 1,
        BootElfEndian::Big => 2,
    };
    hash_u64(hash, value);
}

fn hash_elf_architecture(hash: &mut u64, architecture: BootElfArchitecture) {
    match architecture {
        BootElfArchitecture::Sparc32 => hash_u64(hash, 1),
        BootElfArchitecture::Sparc64 => hash_u64(hash, 2),
        BootElfArchitecture::Mips => hash_u64(hash, 3),
        BootElfArchitecture::I386 => hash_u64(hash, 4),
        BootElfArchitecture::X8664 => hash_u64(hash, 5),
        BootElfArchitecture::Arm => hash_u64(hash, 6),
        BootElfArchitecture::Thumb => hash_u64(hash, 7),
        BootElfArchitecture::Arm64 => hash_u64(hash, 8),
        BootElfArchitecture::Riscv32 => hash_u64(hash, 9),
        BootElfArchitecture::Riscv64 => hash_u64(hash, 10),
        BootElfArchitecture::Power => hash_u64(hash, 11),
        BootElfArchitecture::Power64 => hash_u64(hash, 12),
        BootElfArchitecture::Unknown { machine, class } => {
            hash_u64(hash, 13);
            hash_u64(hash, u64::from(machine));
            hash_elf_class(hash, class);
        }
    }
}

fn hash_elf_operating_system(hash: &mut u64, operating_system: BootElfOperatingSystem) {
    match operating_system {
        BootElfOperatingSystem::Linux => hash_u64(hash, 1),
        BootElfOperatingSystem::Solaris => hash_u64(hash, 2),
        BootElfOperatingSystem::Tru64 => hash_u64(hash, 3),
        BootElfOperatingSystem::LinuxArmOabi => hash_u64(hash, 4),
        BootElfOperatingSystem::LinuxPower64AbiV1 => hash_u64(hash, 5),
        BootElfOperatingSystem::LinuxPower64AbiV2 => hash_u64(hash, 6),
        BootElfOperatingSystem::FreeBsd => hash_u64(hash, 7),
        BootElfOperatingSystem::Unknown { os_abi } => {
            hash_u64(hash, 8);
            hash_u64(hash, u64::from(os_abi));
        }
    }
}
