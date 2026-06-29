use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfEndian, BootElfMetadata, BootElfOperatingSystem,
    BootImage,
};
use rem6_memory::Address;
use rem6_workload::{WorkloadBootImage, WorkloadId, WorkloadManifest};

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_u16_be(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn write_u32_be(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn write_u64_be(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
}

fn elf64_image(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0; 0x104];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 1);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, 0x100);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(&mut bytes, 96, 4);
    write_u64(&mut bytes, 104, 4);
    write_u64(&mut bytes, 112, 0x1000);
    bytes[0x100..0x104].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn elf64_image_with_interpreter(machine: u16, interpreter: &str) -> Vec<u8> {
    let mut bytes = vec![0; 0x204];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, 0x200);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(&mut bytes, 96, 4);
    write_u64(&mut bytes, 104, 4);
    write_u64(&mut bytes, 112, 0x1000);

    let mut interpreter_bytes = interpreter.as_bytes().to_vec();
    interpreter_bytes.push(0);
    write_u32(&mut bytes, 120, 3);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, 0x180);
    write_u64(&mut bytes, 136, 0);
    write_u64(&mut bytes, 144, 0);
    write_u64(&mut bytes, 152, interpreter_bytes.len() as u64);
    write_u64(&mut bytes, 160, interpreter_bytes.len() as u64);
    write_u64(&mut bytes, 168, 1);

    bytes[0x180..0x180 + interpreter_bytes.len()].copy_from_slice(&interpreter_bytes);
    bytes[0x200..0x204].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn elf64_image_with_tbss(machine: u16) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    let names = b"\0.tbss\0.shstrtab\0";
    let shstr_offset = bytes.len();
    bytes.extend_from_slice(names);

    let section_table_offset = bytes.len();
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 3);
    write_u16(&mut bytes, 62, 2);
    bytes.resize(section_table_offset + 3 * 64, 0);

    write_u32(&mut bytes, section_table_offset + 64, 1);
    write_u32(&mut bytes, section_table_offset + 68, 8);
    write_u64(&mut bytes, section_table_offset + 96, 16);

    write_u32(&mut bytes, section_table_offset + 128, 7);
    write_u32(&mut bytes, section_table_offset + 132, 3);
    write_u64(&mut bytes, section_table_offset + 152, shstr_offset as u64);
    write_u64(&mut bytes, section_table_offset + 160, names.len() as u64);
    bytes
}

fn elf64_image_with_symbols(machine: u16) -> Vec<u8> {
    let mut bytes = elf64_image(machine);
    let symbol_names = b"\0entry_func\0data_obj\0";
    let symbol_names_offset = bytes.len();
    bytes.extend_from_slice(symbol_names);

    let symbol_table_offset = bytes.len();
    bytes.resize(bytes.len() + 3 * 24, 0);
    let function_base = symbol_table_offset + 24;
    write_u32(&mut bytes, function_base, 1);
    bytes[function_base + 4] = 0x12;
    write_u16(&mut bytes, function_base + 6, 1);
    write_u64(&mut bytes, function_base + 8, 0x8004);
    write_u64(&mut bytes, function_base + 16, 4);
    let object_base = symbol_table_offset + 48;
    write_u32(&mut bytes, object_base, 12);
    bytes[object_base + 4] = 0x11;
    write_u16(&mut bytes, object_base + 6, 1);
    write_u64(&mut bytes, object_base + 8, 0x9000);
    write_u64(&mut bytes, object_base + 16, 8);

    let section_names = b"\0.symtab\0.strtab\0.shstrtab\0";
    let section_names_offset = bytes.len();
    bytes.extend_from_slice(section_names);

    let section_table_offset = bytes.len();
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 4);
    write_u16(&mut bytes, 62, 3);
    bytes.resize(section_table_offset + 4 * 64, 0);

    let symtab_base = section_table_offset + 64;
    write_u32(&mut bytes, symtab_base, 1);
    write_u32(&mut bytes, symtab_base + 4, 2);
    write_u64(&mut bytes, symtab_base + 24, symbol_table_offset as u64);
    write_u64(&mut bytes, symtab_base + 32, 3 * 24);
    write_u32(&mut bytes, symtab_base + 40, 2);
    write_u64(&mut bytes, symtab_base + 56, 24);

    let strtab_base = section_table_offset + 128;
    write_u32(&mut bytes, strtab_base, 9);
    write_u32(&mut bytes, strtab_base + 4, 3);
    write_u64(&mut bytes, strtab_base + 24, symbol_names_offset as u64);
    write_u64(&mut bytes, strtab_base + 32, symbol_names.len() as u64);

    let shstrtab_base = section_table_offset + 192;
    write_u32(&mut bytes, shstrtab_base, 17);
    write_u32(&mut bytes, shstrtab_base + 4, 3);
    write_u64(&mut bytes, shstrtab_base + 24, section_names_offset as u64);
    write_u64(&mut bytes, shstrtab_base + 32, section_names.len() as u64);
    bytes
}

fn elf64_image_with_dynamic_table(machine: u16, needed_count: usize) -> Vec<u8> {
    assert!(needed_count <= 2);
    let strtab = b"\0lib0.so\0lib1.so\0";
    let strtab_offset = 0x260usize;
    let mut bytes = vec![0; strtab_offset + strtab.len()];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, machine);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, 0x8004);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 2);

    write_u32(&mut bytes, 64, 1);
    write_u32(&mut bytes, 68, 5);
    write_u64(&mut bytes, 72, 0x200);
    write_u64(&mut bytes, 80, 0x8000);
    write_u64(&mut bytes, 88, 0x8000);
    write_u64(
        &mut bytes,
        96,
        (strtab_offset + strtab.len() - 0x200) as u64,
    );
    write_u64(
        &mut bytes,
        104,
        (strtab_offset + strtab.len() - 0x200) as u64,
    );
    write_u64(&mut bytes, 112, 0x1000);

    let dynamic_offset = 0x180usize;
    let dynamic_size = (needed_count + 3) * 16;
    write_u32(&mut bytes, 120, 2);
    write_u32(&mut bytes, 124, 4);
    write_u64(&mut bytes, 128, dynamic_offset as u64);
    write_u64(&mut bytes, 136, 0x8180);
    write_u64(&mut bytes, 144, 0x8180);
    write_u64(&mut bytes, 152, dynamic_size as u64);
    write_u64(&mut bytes, 160, dynamic_size as u64);
    write_u64(&mut bytes, 168, 8);

    let needed_offsets = [1u64, 9u64];
    for index in 0..needed_count {
        let entry = dynamic_offset + index * 16;
        write_u64(&mut bytes, entry, 1);
        write_u64(&mut bytes, entry + 8, needed_offsets[index]);
    }
    let strtab_entry = dynamic_offset + needed_count * 16;
    write_u64(&mut bytes, strtab_entry, 5);
    write_u64(&mut bytes, strtab_entry + 8, 0x8060);
    write_u64(&mut bytes, strtab_entry + 16, 10);
    write_u64(&mut bytes, strtab_entry + 24, strtab.len() as u64);
    let null_entry = strtab_entry + 32;
    write_u64(&mut bytes, null_entry, 0);
    write_u64(&mut bytes, null_entry + 8, 0);
    bytes[0x200..0x204].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes[strtab_offset..strtab_offset + strtab.len()].copy_from_slice(strtab);
    bytes
}

fn elf64_image_with_dynamic_libraries(machine: u16, libraries: &[&str]) -> Vec<u8> {
    elf64_image_with_dynamic_strings(
        machine,
        libraries,
        "librem6.so",
        "/opt/rem6/lib",
        "$ORIGIN/lib",
    )
}

fn elf64_image_with_dynamic_strings(
    machine: u16,
    libraries: &[&str],
    soname: &str,
    rpath: &str,
    runpath: &str,
) -> Vec<u8> {
    let mut bytes = elf64_image_with_dynamic_table(machine, libraries.len());
    let mut strtab = vec![0];
    let mut offsets = Vec::new();
    for library in libraries {
        offsets.push(strtab.len() as u64);
        strtab.extend_from_slice(library.as_bytes());
        strtab.push(0);
    }
    let soname_offset = strtab.len() as u64;
    strtab.extend_from_slice(soname.as_bytes());
    strtab.push(0);
    let rpath_offset = strtab.len() as u64;
    strtab.extend_from_slice(rpath.as_bytes());
    strtab.push(0);
    let runpath_offset = strtab.len() as u64;
    strtab.extend_from_slice(runpath.as_bytes());
    strtab.push(0);
    let strtab_offset = 0x260usize;
    bytes.resize(strtab_offset + strtab.len(), 0);
    let file_size = (strtab_offset + strtab.len() - 0x200) as u64;
    write_u64(&mut bytes, 96, file_size);
    write_u64(&mut bytes, 104, file_size);
    for (index, offset) in offsets.iter().enumerate() {
        write_u64(&mut bytes, 0x180 + index * 16 + 8, *offset);
    }
    let soname_entry = 0x180 + libraries.len() * 16;
    write_u64(&mut bytes, soname_entry, 14);
    write_u64(&mut bytes, soname_entry + 8, soname_offset);
    write_u64(&mut bytes, soname_entry + 16, 15);
    write_u64(&mut bytes, soname_entry + 24, rpath_offset);
    write_u64(&mut bytes, soname_entry + 32, 29);
    write_u64(&mut bytes, soname_entry + 40, runpath_offset);
    let strtab_entry = soname_entry + 48;
    write_u64(&mut bytes, strtab_entry, 5);
    write_u64(&mut bytes, strtab_entry + 8, 0x8060);
    write_u64(&mut bytes, strtab_entry + 16, 10);
    write_u64(&mut bytes, strtab_entry + 24, strtab.len() as u64);
    write_u64(&mut bytes, strtab_entry + 32, 0);
    write_u64(&mut bytes, strtab_entry + 40, 0);
    write_u64(&mut bytes, 152, ((libraries.len() + 6) * 16) as u64);
    write_u64(&mut bytes, 160, ((libraries.len() + 6) * 16) as u64);
    bytes[strtab_offset..strtab_offset + strtab.len()].copy_from_slice(&strtab);
    bytes
}

fn elf64_be_image(machine: u16) -> Vec<u8> {
    let mut bytes = vec![0; 0x104];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 2;
    bytes[6] = 1;
    write_u16_be(&mut bytes, 16, 2);
    write_u16_be(&mut bytes, 18, machine);
    write_u32_be(&mut bytes, 20, 1);
    write_u64_be(&mut bytes, 24, 0x8004);
    write_u64_be(&mut bytes, 32, 64);
    write_u16_be(&mut bytes, 52, 64);
    write_u16_be(&mut bytes, 54, 56);
    write_u16_be(&mut bytes, 56, 1);

    write_u32_be(&mut bytes, 64, 1);
    write_u32_be(&mut bytes, 68, 5);
    write_u64_be(&mut bytes, 72, 0x100);
    write_u64_be(&mut bytes, 80, 0x8000);
    write_u64_be(&mut bytes, 88, 0x8000);
    write_u64_be(&mut bytes, 96, 4);
    write_u64_be(&mut bytes, 104, 4);
    write_u64_be(&mut bytes, 112, 0x1000);
    bytes[0x100..0x104].copy_from_slice(&[0x13, 0x05, 0x00, 0x00]);
    bytes
}

fn id(value: &str) -> WorkloadId {
    WorkloadId::new(value).unwrap()
}

fn boot_image_with_metadata(metadata: BootElfMetadata) -> BootImage {
    BootImage::new(Address::new(0x9000))
        .add_segment(Address::new(0x9000), vec![0x13, 0, 0, 0])
        .unwrap()
        .with_elf_metadata(metadata)
}

#[test]
fn workload_boot_image_preserves_elf_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let metadata = image.elf_metadata().unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert_eq!(workload_image.elf_metadata(), Some(metadata));
    assert_eq!(
        workload_image
            .to_boot_image()
            .unwrap()
            .elf_metadata()
            .unwrap()
            .architecture(),
        BootElfArchitecture::Riscv64,
    );
}

#[test]
fn workload_boot_image_preserves_elf_interpreter_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-linux-riscv64-lp64d.so.1",
    ))
    .unwrap();
    let interpreter = image.elf_interpreter().unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert_eq!(workload_image.elf_interpreter(), Some(interpreter));
    assert_eq!(
        workload_image
            .to_boot_image()
            .unwrap()
            .elf_interpreter()
            .unwrap()
            .path(),
        "/lib/ld-linux-riscv64-lp64d.so.1",
    );
}

#[test]
fn workload_boot_image_preserves_elf_tls_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_tbss(243)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);

    assert!(workload_image.elf_metadata().unwrap().has_tls());
    let round_trip_image = workload_image.to_boot_image().unwrap();
    assert!(round_trip_image.elf_metadata().unwrap().has_tls());
}

#[test]
fn workload_boot_image_preserves_elf_symbol_summary_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_symbols(243)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();

    assert_eq!(round_trip_metadata.symbol_count(), 2);
    assert_eq!(round_trip_metadata.function_symbol_count(), 1);
    assert_eq!(round_trip_metadata.object_symbol_count(), 1);
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_table_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_table(243, 2)).unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.segment_count(), 1);
    assert_eq!(dynamic.file_offset(), Some(0x180));
    assert_eq!(dynamic.virtual_address().unwrap().get(), 0x8180);
    assert_eq!(dynamic.entry_size(), 16);
    assert_eq!(dynamic.entry_count(), 5);
    assert_eq!(dynamic.needed_count(), 2);
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_needed_names_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_libraries(
        243,
        &["libc.so.6", "libm.so.6"],
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.needed_count(), 2);
    assert_eq!(
        dynamic.needed_libraries(),
        &["libc.so.6".to_string(), "libm.so.6".to_string()],
    );
}

#[test]
fn workload_boot_image_preserves_elf_dynamic_string_metadata_round_trip() {
    let image = BootImage::from_elf64_le(&elf64_image_with_dynamic_libraries(
        243,
        &["libc.so.6", "libm.so.6"],
    ))
    .unwrap();

    let workload_image = WorkloadBootImage::from_boot_image(&image);
    let round_trip_metadata = workload_image
        .to_boot_image()
        .unwrap()
        .elf_metadata()
        .unwrap();
    let dynamic = round_trip_metadata.dynamic_table();

    assert_eq!(dynamic.soname(), Some("librem6.so"));
    assert_eq!(dynamic.rpath(), &["/opt/rem6/lib".to_string()]);
    assert_eq!(dynamic.runpath(), &["$ORIGIN/lib".to_string()]);
}

#[test]
fn workload_manifest_identity_includes_elf_metadata() {
    let riscv = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let x86 = BootImage::from_elf64_le(&elf64_image(62)).unwrap();

    assert_eq!(riscv.entry(), x86.entry());
    assert_eq!(riscv.segments(), x86.segments());
    assert_eq!(riscv.elf_metadata().unwrap().class(), BootElfClass::Class64,);
    assert_ne!(riscv.elf_metadata(), x86.elf_metadata());

    let riscv_manifest = WorkloadManifest::builder(id("same"), riscv)
        .build()
        .unwrap();
    let x86_manifest = WorkloadManifest::builder(id("same"), x86).build().unwrap();

    assert_ne!(riscv_manifest.identity(), x86_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_tls_metadata() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let tls = BootImage::from_elf64_le(&elf64_image_with_tbss(243)).unwrap();

    assert_eq!(plain.entry(), tls.entry());
    assert_eq!(plain.segments(), tls.segments());
    assert!(!plain.elf_metadata().unwrap().has_tls());
    assert!(tls.elf_metadata().unwrap().has_tls());

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let tls_manifest = WorkloadManifest::builder(id("same"), tls).build().unwrap();

    assert_ne!(plain_manifest.identity(), tls_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_symbol_summary() {
    let plain = BootImage::from_elf64_le(&elf64_image(243)).unwrap();
    let symbols = BootImage::from_elf64_le(&elf64_image_with_symbols(243)).unwrap();

    assert_eq!(plain.entry(), symbols.entry());
    assert_eq!(plain.segments(), symbols.segments());
    assert_eq!(plain.elf_metadata().unwrap().symbol_count(), 0);
    assert_eq!(symbols.elf_metadata().unwrap().symbol_count(), 2);

    let plain_manifest = WorkloadManifest::builder(id("same"), plain)
        .build()
        .unwrap();
    let symbol_manifest = WorkloadManifest::builder(id("same"), symbols)
        .build()
        .unwrap();

    assert_ne!(plain_manifest.identity(), symbol_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_table_summary() {
    let one_needed = BootImage::from_elf64_le(&elf64_image_with_dynamic_table(243, 1)).unwrap();
    let two_needed = BootImage::from_elf64_le(&elf64_image_with_dynamic_table(243, 2)).unwrap();

    assert_eq!(one_needed.entry(), two_needed.entry());
    assert_eq!(one_needed.segments(), two_needed.segments());
    assert_eq!(
        one_needed
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .needed_count(),
        1,
    );
    assert_eq!(
        two_needed
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .needed_count(),
        2,
    );

    let one_manifest = WorkloadManifest::builder(id("same"), one_needed)
        .build()
        .unwrap();
    let two_manifest = WorkloadManifest::builder(id("same"), two_needed)
        .build()
        .unwrap();

    assert_ne!(one_manifest.identity(), two_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_dynamic_string_metadata() {
    let baseline_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_strings(
        243,
        &["libc.so.6", "libm.so.6"],
        "librem6.so",
        "/opt/rem6/lib",
        "$ORIGIN/lib",
    ))
    .unwrap();
    let soname_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_strings(
        243,
        &["libc.so.6", "libm.so.6"],
        "libalt6.so",
        "/opt/rem6/lib",
        "$ORIGIN/lib",
    ))
    .unwrap();
    let rpath_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_strings(
        243,
        &["libc.so.6", "libm.so.6"],
        "librem6.so",
        "/tmp/rem6/lib",
        "$ORIGIN/lib",
    ))
    .unwrap();
    let runpath_source = BootImage::from_elf64_le(&elf64_image_with_dynamic_strings(
        243,
        &["libc.so.6", "libm.so.6"],
        "librem6.so",
        "/opt/rem6/lib",
        "$ORIGIN/alt",
    ))
    .unwrap();

    assert_eq!(
        baseline_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .needed_libraries(),
        soname_source
            .elf_metadata()
            .unwrap()
            .dynamic_table()
            .needed_libraries(),
    );
    let baseline = boot_image_with_metadata(baseline_source.elf_metadata().unwrap());
    let soname = boot_image_with_metadata(soname_source.elf_metadata().unwrap());
    let rpath = boot_image_with_metadata(rpath_source.elf_metadata().unwrap());
    let runpath = boot_image_with_metadata(runpath_source.elf_metadata().unwrap());

    assert_eq!(baseline.entry(), soname.entry());
    assert_eq!(baseline.segments(), soname.segments());
    assert_eq!(baseline.segments(), rpath.segments());
    assert_eq!(baseline.segments(), runpath.segments());

    let baseline_manifest = WorkloadManifest::builder(id("same"), baseline)
        .build()
        .unwrap();
    let soname_manifest = WorkloadManifest::builder(id("same"), soname)
        .build()
        .unwrap();
    let rpath_manifest = WorkloadManifest::builder(id("same"), rpath)
        .build()
        .unwrap();
    let runpath_manifest = WorkloadManifest::builder(id("same"), runpath)
        .build()
        .unwrap();

    assert_ne!(baseline_manifest.identity(), soname_manifest.identity());
    assert_ne!(baseline_manifest.identity(), rpath_manifest.identity());
    assert_ne!(baseline_manifest.identity(), runpath_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_interpreter_metadata() {
    let musl = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-musl-riscv64.so.1",
    ))
    .unwrap();
    let glibc = BootImage::from_elf64_le(&elf64_image_with_interpreter(
        243,
        "/lib/ld-linux-riscv64-lp64d.so.1",
    ))
    .unwrap();

    assert_eq!(musl.entry(), glibc.entry());
    assert_eq!(musl.segments(), glibc.segments());
    assert_eq!(musl.elf_metadata(), glibc.elf_metadata());
    assert_ne!(musl.elf_interpreter(), glibc.elf_interpreter());

    let musl_manifest = WorkloadManifest::builder(id("same"), musl).build().unwrap();
    let glibc_manifest = WorkloadManifest::builder(id("same"), glibc)
        .build()
        .unwrap();

    assert_ne!(musl_manifest.identity(), glibc_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_endian_metadata() {
    let little = BootImage::from_elf(&elf64_image(2)).unwrap();
    let big = BootImage::from_elf(&elf64_be_image(2)).unwrap();

    assert_eq!(little.entry(), big.entry());
    assert_eq!(little.segments(), big.segments());
    assert_eq!(
        little.elf_metadata().unwrap().endian(),
        BootElfEndian::Little
    );
    assert_eq!(big.elf_metadata().unwrap().endian(), BootElfEndian::Big);
    assert_ne!(little.elf_metadata(), big.elf_metadata());

    let little_manifest = WorkloadManifest::builder(id("same"), little)
        .build()
        .unwrap();
    let big_manifest = WorkloadManifest::builder(id("same"), big).build().unwrap();

    assert_ne!(little_manifest.identity(), big_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_power64_endian_default_abi() {
    let little = BootImage::from_elf(&elf64_image(21)).unwrap();
    let big = BootImage::from_elf(&elf64_be_image(21)).unwrap();

    assert_eq!(little.entry(), big.entry());
    assert_eq!(little.segments(), big.segments());
    assert_eq!(
        little.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV2,
    );
    assert_eq!(
        big.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::LinuxPower64AbiV1,
    );

    let little_manifest = WorkloadManifest::builder(id("same"), little)
        .build()
        .unwrap();
    let big_manifest = WorkloadManifest::builder(id("same"), big).build().unwrap();

    assert_ne!(little_manifest.identity(), big_manifest.identity());
}

#[test]
fn workload_manifest_identity_includes_elf_operating_system_metadata() {
    let mut linux_bytes = elf64_image(243);
    linux_bytes[7] = 3;
    let mut freebsd_bytes = elf64_image(243);
    freebsd_bytes[7] = 9;

    let linux = BootImage::from_elf64_le(&linux_bytes).unwrap();
    let freebsd = BootImage::from_elf64_le(&freebsd_bytes).unwrap();

    assert_eq!(linux.entry(), freebsd.entry());
    assert_eq!(linux.segments(), freebsd.segments());
    assert_eq!(
        linux.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::Linux,
    );
    assert_eq!(
        freebsd.elf_metadata().unwrap().operating_system(),
        BootElfOperatingSystem::FreeBsd,
    );

    let linux_manifest = WorkloadManifest::builder(id("same"), linux)
        .build()
        .unwrap();
    let freebsd_manifest = WorkloadManifest::builder(id("same"), freebsd)
        .build()
        .unwrap();

    assert_ne!(linux_manifest.identity(), freebsd_manifest.identity());
}
