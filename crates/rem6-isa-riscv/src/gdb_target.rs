#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvGdbXlen {
    Rv32,
    Rv64,
}

impl RiscvGdbXlen {
    pub const fn bits(self) -> u32 {
        match self {
            Self::Rv32 => 32,
            Self::Rv64 => 64,
        }
    }

    const fn csr_register_base(self) -> u8 {
        match self {
            Self::Rv32 | Self::Rv64 => 70,
        }
    }

    const fn cpu_annex(self) -> &'static str {
        match self {
            Self::Rv32 => "riscv-32bit-cpu.xml",
            Self::Rv64 => "riscv-64bit-cpu.xml",
        }
    }

    const fn csr_annex(self) -> Option<&'static str> {
        match self {
            Self::Rv32 => Some("riscv-32bit-csr.xml"),
            Self::Rv64 => Some("riscv-64bit-csr.xml"),
        }
    }

    const fn fpu_annex(self) -> Option<&'static str> {
        match self {
            Self::Rv32 => Some("riscv-32bit-fpu.xml"),
            Self::Rv64 => Some("riscv-64bit-fpu.xml"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvGdbTargetDocument {
    annex: &'static str,
    content: Vec<u8>,
}

impl RiscvGdbTargetDocument {
    pub fn new(annex: &'static str, content: Vec<u8>) -> Self {
        Self { annex, content }
    }

    pub const fn annex(&self) -> &'static str {
        self.annex
    }

    pub fn content(&self) -> &[u8] {
        &self.content
    }

    pub fn into_parts(self) -> (&'static str, Vec<u8>) {
        (self.annex, self.content)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvGdbTargetDescription {
    xlen: RiscvGdbXlen,
    documents: Vec<RiscvGdbTargetDocument>,
}

impl RiscvGdbTargetDescription {
    pub fn new(xlen: RiscvGdbXlen) -> Self {
        let mut documents = vec![target_document(xlen), cpu_document(xlen)];
        if let Some(document) = fpu_document(xlen) {
            documents.push(document);
        }
        if let Some(document) = csr_document(xlen) {
            documents.push(document);
        }

        Self { xlen, documents }
    }

    pub const fn xlen(&self) -> RiscvGdbXlen {
        self.xlen
    }

    pub fn documents(&self) -> &[RiscvGdbTargetDocument] {
        &self.documents
    }

    pub fn document(&self, annex: &str) -> Option<&RiscvGdbTargetDocument> {
        self.documents
            .iter()
            .find(|document| document.annex() == annex)
    }

    pub fn into_documents(self) -> Vec<RiscvGdbTargetDocument> {
        self.documents
    }
}

fn target_document(xlen: RiscvGdbXlen) -> RiscvGdbTargetDocument {
    let mut content = format!(
        concat!(
            "<?xml version=\"1.0\"?>\n",
            "<!DOCTYPE target SYSTEM \"gdb-target.dtd\">\n",
            "<target>\n",
            "  <architecture>riscv</architecture>\n",
            "  <xi:include href=\"{}\"/>\n",
        ),
        xlen.cpu_annex(),
    );
    if let Some(annex) = xlen.fpu_annex() {
        content.push_str(&format!("  <xi:include href=\"{annex}\"/>\n"));
    }
    if let Some(annex) = xlen.csr_annex() {
        content.push_str(&format!("  <xi:include href=\"{annex}\"/>\n"));
    }
    content.push_str("</target>\n");

    RiscvGdbTargetDocument::new("target.xml", content.into_bytes())
}

fn cpu_document(xlen: RiscvGdbXlen) -> RiscvGdbTargetDocument {
    let mut content = concat!(
        "<?xml version=\"1.0\"?>\n",
        "<!DOCTYPE feature SYSTEM \"gdb-target.dtd\">\n",
        "<feature name=\"org.gnu.gdb.riscv.cpu\">\n",
    )
    .to_string();
    for register in CPU_REGISTERS {
        content.push_str(&format!(
            "  <reg name=\"{}\" bitsize=\"{}\" type=\"{}\"",
            register.name,
            xlen.bits(),
            register.ty,
        ));
        if let Some(regnum) = register.regnum {
            content.push_str(&format!(" regnum=\"{regnum}\""));
        }
        content.push_str("/>\n");
    }
    content.push_str("</feature>\n");

    RiscvGdbTargetDocument::new(xlen.cpu_annex(), content.into_bytes())
}

fn fpu_document(xlen: RiscvGdbXlen) -> Option<RiscvGdbTargetDocument> {
    let annex = xlen.fpu_annex()?;
    let mut content = concat!(
        "<?xml version=\"1.0\"?>\n",
        "<!DOCTYPE feature SYSTEM \"gdb-target.dtd\">\n",
        "<feature name=\"org.gnu.gdb.riscv.fpu\">\n",
        "  <union id=\"riscv_double\">\n",
        "    <field name=\"float\" type=\"ieee_single\"/>\n",
        "    <field name=\"double\" type=\"ieee_double\"/>\n",
        "  </union>\n",
    )
    .to_string();
    for (index, register) in RV64D_FLOAT_REGISTERS.iter().enumerate() {
        content.push_str(&format!(
            "  <reg name=\"{}\" bitsize=\"64\" type=\"riscv_double\"",
            register
        ));
        if index == 0 {
            content.push_str(" regnum=\"33\"");
        }
        content.push_str("/>\n");
    }
    for register in RV64D_FLOAT_CSR_REGISTERS {
        content.push_str(&format!(
            "  <reg name=\"{}\" bitsize=\"32\" type=\"int\" regnum=\"{}\"/>\n",
            register.name, register.regnum,
        ));
    }
    content.push_str("</feature>\n");

    Some(RiscvGdbTargetDocument::new(annex, content.into_bytes()))
}

fn csr_document(xlen: RiscvGdbXlen) -> Option<RiscvGdbTargetDocument> {
    let annex = xlen.csr_annex()?;
    let mut content = concat!(
        "<?xml version=\"1.0\"?>\n",
        "<!DOCTYPE feature SYSTEM \"gdb-target.dtd\">\n",
        "<feature name=\"org.gnu.gdb.riscv.csr\">\n",
    )
    .to_string();
    for (index, register) in RV64_CSR_REGISTERS.iter().enumerate() {
        content.push_str(&format!(
            "  <reg name=\"{}\" bitsize=\"{}\"",
            register,
            xlen.bits(),
        ));
        if index == 0 {
            content.push_str(&format!(" regnum=\"{}\"", xlen.csr_register_base()));
        }
        content.push_str("/>\n");
    }
    content.push_str("</feature>\n");

    Some(RiscvGdbTargetDocument::new(annex, content.into_bytes()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CpuRegister {
    name: &'static str,
    ty: &'static str,
    regnum: Option<u8>,
}

impl CpuRegister {
    const fn new(name: &'static str, ty: &'static str, regnum: Option<u8>) -> Self {
        Self { name, ty, regnum }
    }
}

const CPU_REGISTERS: &[CpuRegister] = &[
    CpuRegister::new("zero", "int", Some(0)),
    CpuRegister::new("ra", "code_ptr", None),
    CpuRegister::new("sp", "data_ptr", None),
    CpuRegister::new("gp", "data_ptr", None),
    CpuRegister::new("tp", "data_ptr", None),
    CpuRegister::new("t0", "int", None),
    CpuRegister::new("t1", "int", None),
    CpuRegister::new("t2", "int", None),
    CpuRegister::new("fp", "data_ptr", None),
    CpuRegister::new("s1", "int", None),
    CpuRegister::new("a0", "int", None),
    CpuRegister::new("a1", "int", None),
    CpuRegister::new("a2", "int", None),
    CpuRegister::new("a3", "int", None),
    CpuRegister::new("a4", "int", None),
    CpuRegister::new("a5", "int", None),
    CpuRegister::new("a6", "int", None),
    CpuRegister::new("a7", "int", None),
    CpuRegister::new("s2", "int", None),
    CpuRegister::new("s3", "int", None),
    CpuRegister::new("s4", "int", None),
    CpuRegister::new("s5", "int", None),
    CpuRegister::new("s6", "int", None),
    CpuRegister::new("s7", "int", None),
    CpuRegister::new("s8", "int", None),
    CpuRegister::new("s9", "int", None),
    CpuRegister::new("s10", "int", None),
    CpuRegister::new("s11", "int", None),
    CpuRegister::new("t3", "int", None),
    CpuRegister::new("t4", "int", None),
    CpuRegister::new("t5", "int", None),
    CpuRegister::new("t6", "int", None),
    CpuRegister::new("pc", "code_ptr", None),
];

const RV64D_FLOAT_REGISTERS: &[&str] = &[
    "ft0", "ft1", "ft2", "ft3", "ft4", "ft5", "ft6", "ft7", "fs0", "fs1", "fa0", "fa1", "fa2",
    "fa3", "fa4", "fa5", "fa6", "fa7", "fs2", "fs3", "fs4", "fs5", "fs6", "fs7", "fs8", "fs9",
    "fs10", "fs11", "ft8", "ft9", "ft10", "ft11",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FloatCsrRegister {
    name: &'static str,
    regnum: u8,
}

impl FloatCsrRegister {
    const fn new(name: &'static str, regnum: u8) -> Self {
        Self { name, regnum }
    }
}

const RV64D_FLOAT_CSR_REGISTERS: &[FloatCsrRegister] = &[
    FloatCsrRegister::new("fflags", 66),
    FloatCsrRegister::new("frm", 67),
    FloatCsrRegister::new("fcsr", 68),
    FloatCsrRegister::new("placeholder", 69),
];

const RV64_CSR_REGISTERS: &[&str] = &[
    "sstatus", "stvec", "sscratch", "sepc", "scause", "stval", "satp", "mstatus", "medeleg",
    "mideleg", "mie", "mtvec", "mscratch", "mepc", "mcause", "mtval", "mip", "vxsat", "vxrm",
    "vcsr",
];
