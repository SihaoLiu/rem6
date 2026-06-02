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

    const fn cpu_annex(self) -> &'static str {
        match self {
            Self::Rv32 => "riscv-32bit-cpu.xml",
            Self::Rv64 => "riscv-64bit-cpu.xml",
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
        Self {
            xlen,
            documents: vec![target_document(xlen), cpu_document(xlen)],
        }
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
    RiscvGdbTargetDocument::new(
        "target.xml",
        format!(
            concat!(
                "<?xml version=\"1.0\"?>\n",
                "<!DOCTYPE target SYSTEM \"gdb-target.dtd\">\n",
                "<target>\n",
                "  <architecture>riscv</architecture>\n",
                "  <xi:include href=\"{}\"/>\n",
                "</target>\n",
            ),
            xlen.cpu_annex(),
        )
        .into_bytes(),
    )
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
