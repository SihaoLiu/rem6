use rem6_isa_riscv::{RiscvGdbTargetDescription, RiscvGdbTargetDocument, RiscvGdbXlen};

#[test]
fn riscv_gdb_target_description_reports_rv64_cpu_documents() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv64);

    assert_eq!(description.xlen(), RiscvGdbXlen::Rv64);
    assert_eq!(
        annex_names(description.documents()),
        vec![
            "target.xml",
            "riscv-64bit-cpu.xml",
            "riscv-64bit-fpu.xml",
            "riscv-64bit-csr.xml",
            "riscv-64bit-vector.xml",
        ],
    );
    assert_eq!(
        description.document("target.xml").unwrap().content(),
        concat!(
            "<?xml version=\"1.0\"?>\n",
            "<!DOCTYPE target SYSTEM \"gdb-target.dtd\">\n",
            "<target>\n",
            "  <architecture>riscv</architecture>\n",
            "  <xi:include href=\"riscv-64bit-cpu.xml\"/>\n",
            "  <xi:include href=\"riscv-64bit-fpu.xml\"/>\n",
            "  <xi:include href=\"riscv-64bit-csr.xml\"/>\n",
            "  <xi:include href=\"riscv-64bit-vector.xml\"/>\n",
            "</target>\n",
        )
        .as_bytes(),
    );
    assert!(description.document("riscv-64bit-fpu.xml").is_some());
    assert!(description.document("riscv-64bit-vector.xml").is_some());
}

#[test]
fn riscv_gdb_target_description_reports_rv64_csr_document() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv64);

    let target = text(description.document("target.xml").unwrap());
    assert!(target.contains("<xi:include href=\"riscv-64bit-csr.xml\"/>"));

    let csr = text(description.document("riscv-64bit-csr.xml").unwrap());
    assert_eq!(
        csr,
        concat!(
            "<?xml version=\"1.0\"?>\n",
            "<!DOCTYPE feature SYSTEM \"gdb-target.dtd\">\n",
            "<feature name=\"org.gnu.gdb.riscv.csr\">\n",
            "  <reg name=\"sstatus\" bitsize=\"64\" regnum=\"70\"/>\n",
            "  <reg name=\"stvec\" bitsize=\"64\"/>\n",
            "  <reg name=\"sscratch\" bitsize=\"64\"/>\n",
            "  <reg name=\"sepc\" bitsize=\"64\"/>\n",
            "  <reg name=\"scause\" bitsize=\"64\"/>\n",
            "  <reg name=\"stval\" bitsize=\"64\"/>\n",
            "  <reg name=\"satp\" bitsize=\"64\"/>\n",
            "  <reg name=\"mstatus\" bitsize=\"64\"/>\n",
            "  <reg name=\"medeleg\" bitsize=\"64\"/>\n",
            "  <reg name=\"mideleg\" bitsize=\"64\"/>\n",
            "  <reg name=\"mie\" bitsize=\"64\"/>\n",
            "  <reg name=\"mtvec\" bitsize=\"64\"/>\n",
            "  <reg name=\"mscratch\" bitsize=\"64\"/>\n",
            "  <reg name=\"mepc\" bitsize=\"64\"/>\n",
            "  <reg name=\"mcause\" bitsize=\"64\"/>\n",
            "  <reg name=\"mtval\" bitsize=\"64\"/>\n",
            "  <reg name=\"mip\" bitsize=\"64\"/>\n",
            "  <reg name=\"vxsat\" bitsize=\"64\"/>\n",
            "  <reg name=\"vxrm\" bitsize=\"64\"/>\n",
            "  <reg name=\"vcsr\" bitsize=\"64\"/>\n",
            "  <reg name=\"sie\" bitsize=\"64\" regnum=\"122\"/>\n",
            "  <reg name=\"sip\" bitsize=\"64\" regnum=\"123\"/>\n",
            "  <reg name=\"cycle\" bitsize=\"64\" regnum=\"124\"/>\n",
            "  <reg name=\"instret\" bitsize=\"64\" regnum=\"125\"/>\n",
            "  <reg name=\"time\" bitsize=\"64\" regnum=\"126\"/>\n",
            "  <reg name=\"mhartid\" bitsize=\"64\" regnum=\"127\"/>\n",
            "  <reg name=\"mvendorid\" bitsize=\"64\" regnum=\"128\"/>\n",
            "  <reg name=\"marchid\" bitsize=\"64\" regnum=\"129\"/>\n",
            "  <reg name=\"mimpid\" bitsize=\"64\" regnum=\"130\"/>\n",
            "  <reg name=\"misa\" bitsize=\"64\" regnum=\"131\"/>\n",
            "  <reg name=\"vl\" bitsize=\"64\" regnum=\"132\"/>\n",
            "  <reg name=\"vtype\" bitsize=\"64\" regnum=\"133\"/>\n",
            "  <reg name=\"vlenb\" bitsize=\"64\" regnum=\"134\"/>\n",
            "  <reg name=\"senvcfg\" bitsize=\"64\" regnum=\"135\"/>\n",
            "  <reg name=\"pmpcfg0\" bitsize=\"64\" regnum=\"136\"/>\n",
            "  <reg name=\"pmpaddr0\" bitsize=\"64\" regnum=\"137\"/>\n",
            "  <reg name=\"mcycle\" bitsize=\"64\" regnum=\"138\"/>\n",
            "  <reg name=\"minstret\" bitsize=\"64\" regnum=\"139\"/>\n",
            "  <reg name=\"pmpaddr1\" bitsize=\"64\" regnum=\"140\"/>\n",
            "</feature>\n",
        ),
    );
    assert_eq!(
        register_names(csr),
        vec![
            "sstatus",
            "stvec",
            "sscratch",
            "sepc",
            "scause",
            "stval",
            "satp",
            "mstatus",
            "medeleg",
            "mideleg",
            "mie",
            "mtvec",
            "mscratch",
            "mepc",
            "mcause",
            "mtval",
            "mip",
            "vxsat",
            "vxrm",
            "vcsr",
            "sie",
            "sip",
            "cycle",
            "instret",
            "time",
            "mhartid",
            "mvendorid",
            "marchid",
            "mimpid",
            "misa",
            "vl",
            "vtype",
            "vlenb",
            "senvcfg",
            "pmpcfg0",
            "pmpaddr0",
            "mcycle",
            "minstret",
            "pmpaddr1",
        ],
    );
    assert_eq!(csr.matches("bitsize=\"64\"").count(), 39);
}

#[test]
fn riscv_gdb_target_description_reports_rv64d_fpu_document() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv64);

    let target = text(description.document("target.xml").unwrap());
    assert!(target.contains("<xi:include href=\"riscv-64bit-fpu.xml\"/>"));

    let fpu = text(description.document("riscv-64bit-fpu.xml").unwrap());
    assert!(fpu.contains("<feature name=\"org.gnu.gdb.riscv.fpu\">"));
    assert!(fpu.contains("<union id=\"riscv_double\">"));
    assert!(fpu.contains("<field name=\"float\" type=\"ieee_single\"/>"));
    assert!(fpu.contains("<field name=\"double\" type=\"ieee_double\"/>"));
    assert!(fpu.contains("<reg name=\"ft0\" bitsize=\"64\" type=\"riscv_double\" regnum=\"33\"/>"));
    assert!(fpu.contains("<reg name=\"ft11\" bitsize=\"64\" type=\"riscv_double\"/>"));
    assert!(fpu.contains("<reg name=\"fflags\" bitsize=\"32\" type=\"int\" regnum=\"66\"/>"));
    assert!(fpu.contains("<reg name=\"frm\" bitsize=\"32\" type=\"int\" regnum=\"67\"/>"));
    assert!(fpu.contains("<reg name=\"fcsr\" bitsize=\"32\" type=\"int\" regnum=\"68\"/>"));
    assert!(fpu.contains("<reg name=\"placeholder\" bitsize=\"32\" type=\"int\" regnum=\"69\"/>"));
    assert_eq!(
        register_names(fpu),
        vec![
            "ft0",
            "ft1",
            "ft2",
            "ft3",
            "ft4",
            "ft5",
            "ft6",
            "ft7",
            "fs0",
            "fs1",
            "fa0",
            "fa1",
            "fa2",
            "fa3",
            "fa4",
            "fa5",
            "fa6",
            "fa7",
            "fs2",
            "fs3",
            "fs4",
            "fs5",
            "fs6",
            "fs7",
            "fs8",
            "fs9",
            "fs10",
            "fs11",
            "ft8",
            "ft9",
            "ft10",
            "ft11",
            "fflags",
            "frm",
            "fcsr",
            "placeholder",
        ],
    );
    assert_eq!(fpu.matches("bitsize=\"64\"").count(), 32);
    assert_eq!(fpu.matches("bitsize=\"32\"").count(), 4);
}

#[test]
fn riscv_gdb_target_description_reports_rv64_vector_document() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv64);

    let target = text(description.document("target.xml").unwrap());
    assert!(target.contains("<xi:include href=\"riscv-64bit-vector.xml\"/>"));

    let vector = text(description.document("riscv-64bit-vector.xml").unwrap());
    assert!(vector.contains("<feature name=\"org.gnu.gdb.riscv.vector\">"));
    assert!(vector.contains("<reg name=\"v0\" bitsize=\"128\" type=\"uint128\" regnum=\"90\"/>"));
    assert!(vector.contains("<reg name=\"v31\" bitsize=\"128\" type=\"uint128\"/>"));
    assert_eq!(
        register_names(vector),
        vec![
            "v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13",
            "v14", "v15", "v16", "v17", "v18", "v19", "v20", "v21", "v22", "v23", "v24", "v25",
            "v26", "v27", "v28", "v29", "v30", "v31",
        ],
    );
    assert_eq!(vector.matches("bitsize=\"128\"").count(), 32);
}

#[test]
fn riscv_gdb_target_description_reports_rv32_cpu_documents() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv32);

    assert_eq!(description.xlen(), RiscvGdbXlen::Rv32);
    assert_eq!(
        annex_names(description.documents()),
        vec![
            "target.xml",
            "riscv-32bit-cpu.xml",
            "riscv-32bit-fpu.xml",
            "riscv-32bit-csr.xml",
            "riscv-32bit-vector.xml",
        ],
    );

    let target = text(description.document("target.xml").unwrap());
    assert!(target.contains("<architecture>riscv</architecture>"));
    assert!(target.contains("<xi:include href=\"riscv-32bit-cpu.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-32bit-fpu.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-32bit-csr.xml\"/>"));
    assert!(target.contains("<xi:include href=\"riscv-32bit-vector.xml\"/>"));
    assert!(!target.contains("riscv-64bit-cpu.xml"));
    assert!(description.document("riscv-32bit-fpu.xml").is_some());
    assert!(description.document("riscv-32bit-vector.xml").is_some());
}

#[test]
fn riscv_gdb_target_description_reports_rv32d_fpu_document() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv32);

    let fpu = text(description.document("riscv-32bit-fpu.xml").unwrap());
    assert!(fpu.contains("<feature name=\"org.gnu.gdb.riscv.fpu\">"));
    assert!(fpu.contains("<union id=\"riscv_double\">"));
    assert!(fpu.contains("<field name=\"float\" type=\"ieee_single\"/>"));
    assert!(fpu.contains("<field name=\"double\" type=\"ieee_double\"/>"));
    assert!(fpu.contains("<reg name=\"ft0\" bitsize=\"64\" type=\"riscv_double\" regnum=\"33\"/>"));
    assert!(fpu.contains("<reg name=\"ft11\" bitsize=\"64\" type=\"riscv_double\"/>"));
    assert!(fpu.contains("<reg name=\"fflags\" bitsize=\"32\" type=\"int\" regnum=\"66\"/>"));
    assert!(fpu.contains("<reg name=\"frm\" bitsize=\"32\" type=\"int\" regnum=\"67\"/>"));
    assert!(fpu.contains("<reg name=\"fcsr\" bitsize=\"32\" type=\"int\" regnum=\"68\"/>"));
    assert!(fpu.contains("<reg name=\"placeholder\" bitsize=\"32\" type=\"int\" regnum=\"69\"/>"));
    assert_eq!(
        register_names(fpu),
        vec![
            "ft0",
            "ft1",
            "ft2",
            "ft3",
            "ft4",
            "ft5",
            "ft6",
            "ft7",
            "fs0",
            "fs1",
            "fa0",
            "fa1",
            "fa2",
            "fa3",
            "fa4",
            "fa5",
            "fa6",
            "fa7",
            "fs2",
            "fs3",
            "fs4",
            "fs5",
            "fs6",
            "fs7",
            "fs8",
            "fs9",
            "fs10",
            "fs11",
            "ft8",
            "ft9",
            "ft10",
            "ft11",
            "fflags",
            "frm",
            "fcsr",
            "placeholder",
        ],
    );
    assert_eq!(fpu.matches("bitsize=\"64\"").count(), 32);
    assert_eq!(fpu.matches("bitsize=\"32\"").count(), 4);
}

#[test]
fn riscv_gdb_target_description_reports_rv32_vector_document() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv32);

    let vector = text(description.document("riscv-32bit-vector.xml").unwrap());
    assert!(vector.contains("<feature name=\"org.gnu.gdb.riscv.vector\">"));
    assert!(vector.contains("<reg name=\"v0\" bitsize=\"128\" type=\"uint128\" regnum=\"90\"/>"));
    assert!(vector.contains("<reg name=\"v31\" bitsize=\"128\" type=\"uint128\"/>"));
    assert_eq!(
        register_names(vector),
        vec![
            "v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8", "v9", "v10", "v11", "v12", "v13",
            "v14", "v15", "v16", "v17", "v18", "v19", "v20", "v21", "v22", "v23", "v24", "v25",
            "v26", "v27", "v28", "v29", "v30", "v31",
        ],
    );
    assert_eq!(vector.matches("bitsize=\"128\"").count(), 32);
}

#[test]
fn riscv_gdb_target_description_reports_rv32_csr_document() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv32);

    let csr = text(description.document("riscv-32bit-csr.xml").unwrap());
    assert_eq!(
        csr,
        concat!(
            "<?xml version=\"1.0\"?>\n",
            "<!DOCTYPE feature SYSTEM \"gdb-target.dtd\">\n",
            "<feature name=\"org.gnu.gdb.riscv.csr\">\n",
            "  <reg name=\"sstatus\" bitsize=\"32\" regnum=\"70\"/>\n",
            "  <reg name=\"stvec\" bitsize=\"32\"/>\n",
            "  <reg name=\"sscratch\" bitsize=\"32\"/>\n",
            "  <reg name=\"sepc\" bitsize=\"32\"/>\n",
            "  <reg name=\"scause\" bitsize=\"32\"/>\n",
            "  <reg name=\"stval\" bitsize=\"32\"/>\n",
            "  <reg name=\"satp\" bitsize=\"32\"/>\n",
            "  <reg name=\"mstatus\" bitsize=\"32\"/>\n",
            "  <reg name=\"medeleg\" bitsize=\"32\"/>\n",
            "  <reg name=\"mideleg\" bitsize=\"32\"/>\n",
            "  <reg name=\"mie\" bitsize=\"32\"/>\n",
            "  <reg name=\"mtvec\" bitsize=\"32\"/>\n",
            "  <reg name=\"mscratch\" bitsize=\"32\"/>\n",
            "  <reg name=\"mepc\" bitsize=\"32\"/>\n",
            "  <reg name=\"mcause\" bitsize=\"32\"/>\n",
            "  <reg name=\"mtval\" bitsize=\"32\"/>\n",
            "  <reg name=\"mip\" bitsize=\"32\"/>\n",
            "  <reg name=\"vxsat\" bitsize=\"32\"/>\n",
            "  <reg name=\"vxrm\" bitsize=\"32\"/>\n",
            "  <reg name=\"vcsr\" bitsize=\"32\"/>\n",
            "  <reg name=\"sie\" bitsize=\"32\" regnum=\"122\"/>\n",
            "  <reg name=\"sip\" bitsize=\"32\" regnum=\"123\"/>\n",
            "  <reg name=\"cycle\" bitsize=\"32\" regnum=\"124\"/>\n",
            "  <reg name=\"instret\" bitsize=\"32\" regnum=\"125\"/>\n",
            "  <reg name=\"time\" bitsize=\"32\" regnum=\"126\"/>\n",
            "  <reg name=\"mhartid\" bitsize=\"32\" regnum=\"127\"/>\n",
            "  <reg name=\"mvendorid\" bitsize=\"32\" regnum=\"128\"/>\n",
            "  <reg name=\"marchid\" bitsize=\"32\" regnum=\"129\"/>\n",
            "  <reg name=\"mimpid\" bitsize=\"32\" regnum=\"130\"/>\n",
            "  <reg name=\"misa\" bitsize=\"32\" regnum=\"131\"/>\n",
            "  <reg name=\"vl\" bitsize=\"32\" regnum=\"132\"/>\n",
            "  <reg name=\"vtype\" bitsize=\"32\" regnum=\"133\"/>\n",
            "  <reg name=\"vlenb\" bitsize=\"32\" regnum=\"134\"/>\n",
            "  <reg name=\"senvcfg\" bitsize=\"32\" regnum=\"135\"/>\n",
            "  <reg name=\"pmpcfg0\" bitsize=\"32\" regnum=\"136\"/>\n",
            "  <reg name=\"pmpaddr0\" bitsize=\"32\" regnum=\"137\"/>\n",
            "  <reg name=\"mcycle\" bitsize=\"32\" regnum=\"138\"/>\n",
            "  <reg name=\"minstret\" bitsize=\"32\" regnum=\"139\"/>\n",
            "  <reg name=\"pmpaddr1\" bitsize=\"32\" regnum=\"140\"/>\n",
            "</feature>\n",
        ),
    );
    assert_eq!(
        register_names(csr),
        vec![
            "sstatus",
            "stvec",
            "sscratch",
            "sepc",
            "scause",
            "stval",
            "satp",
            "mstatus",
            "medeleg",
            "mideleg",
            "mie",
            "mtvec",
            "mscratch",
            "mepc",
            "mcause",
            "mtval",
            "mip",
            "vxsat",
            "vxrm",
            "vcsr",
            "sie",
            "sip",
            "cycle",
            "instret",
            "time",
            "mhartid",
            "mvendorid",
            "marchid",
            "mimpid",
            "misa",
            "vl",
            "vtype",
            "vlenb",
            "senvcfg",
            "pmpcfg0",
            "pmpaddr0",
            "mcycle",
            "minstret",
            "pmpaddr1",
        ],
    );
    assert_eq!(csr.matches("bitsize=\"32\"").count(), 39);
}

#[test]
fn riscv_gdb_cpu_document_uses_stable_abi_register_order() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv64);
    let cpu = text(description.document("riscv-64bit-cpu.xml").unwrap());

    let names = register_names(cpu);
    assert_eq!(
        names,
        vec![
            "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "fp", "s1", "a0", "a1", "a2", "a3",
            "a4", "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11",
            "t3", "t4", "t5", "t6", "pc",
        ],
    );
    assert_eq!(cpu.matches("bitsize=\"64\"").count(), 33);
    assert!(cpu.contains("<reg name=\"zero\" bitsize=\"64\" type=\"int\" regnum=\"0\"/>"));
    assert!(cpu.contains("<reg name=\"ra\" bitsize=\"64\" type=\"code_ptr\"/>"));
    assert!(cpu.contains("<reg name=\"sp\" bitsize=\"64\" type=\"data_ptr\"/>"));
    assert!(cpu.contains("<reg name=\"pc\" bitsize=\"64\" type=\"code_ptr\"/>"));
}

#[test]
fn riscv_gdb_cpu_document_scales_register_bits_by_xlen() {
    let rv32 = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv32);
    let rv64 = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv64);

    let rv32_cpu = text(rv32.document("riscv-32bit-cpu.xml").unwrap());
    let rv64_cpu = text(rv64.document("riscv-64bit-cpu.xml").unwrap());

    assert_eq!(rv32_cpu.matches("bitsize=\"32\"").count(), 33);
    assert_eq!(rv64_cpu.matches("bitsize=\"64\"").count(), 33);
    assert!(!rv32_cpu.contains("bitsize=\"64\""));
    assert!(!rv64_cpu.contains("bitsize=\"32\""));
}

#[test]
fn riscv_gdb_target_documents_keep_unique_annexes_for_debug_registry() {
    let description = RiscvGdbTargetDescription::new(RiscvGdbXlen::Rv64);
    let mut annexes = annex_names(description.documents());
    annexes.sort_unstable();
    annexes.dedup();

    assert_eq!(annexes.len(), description.documents().len());
    for document in description.documents() {
        assert!(!document.annex().is_empty());
        assert!(!document.content().is_empty());
    }
}

fn annex_names(documents: &[RiscvGdbTargetDocument]) -> Vec<&str> {
    documents
        .iter()
        .map(RiscvGdbTargetDocument::annex)
        .collect()
}

fn text(document: &RiscvGdbTargetDocument) -> &str {
    std::str::from_utf8(document.content()).unwrap()
}

fn register_names(cpu: &str) -> Vec<&str> {
    cpu.lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("<reg name=\"")?;
            let end = rest.find('"')?;
            Some(&rest[..end])
        })
        .collect()
}
