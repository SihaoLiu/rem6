use std::collections::BTreeMap;
use std::fmt::Write as _;

use rem6_interrupt::InterruptTargetId;
use rem6_memory::{AccessSize, Address, AddressRange};

use crate::{
    PlatformClintConfig, PlatformError, PlatformInterruptControllerConfig, PlatformPl031RtcConfig,
    PlatformRtcConfig, PlatformTimerConfig, PlatformUartConfig,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformRiscvDeviceTreeConfig {
    timebase_frequency: u32,
    cpu_isa: String,
    cpu_mmu_type: String,
    uart_clock_frequency: u32,
    bootargs: Option<String>,
    initrd: Option<AddressRange>,
}

impl PlatformRiscvDeviceTreeConfig {
    pub fn new(
        timebase_frequency: u32,
        cpu_isa: impl Into<String>,
        cpu_mmu_type: impl Into<String>,
        uart_clock_frequency: u32,
    ) -> Result<Self, PlatformError> {
        if timebase_frequency == 0 {
            return Err(PlatformError::InvalidDeviceTreeConfig {
                field: "timebase_frequency",
            });
        }
        let cpu_isa = cpu_isa.into();
        if cpu_isa.is_empty() {
            return Err(PlatformError::InvalidDeviceTreeConfig { field: "cpu_isa" });
        }
        let cpu_mmu_type = cpu_mmu_type.into();
        if cpu_mmu_type.is_empty() {
            return Err(PlatformError::InvalidDeviceTreeConfig {
                field: "cpu_mmu_type",
            });
        }
        if uart_clock_frequency == 0 {
            return Err(PlatformError::InvalidDeviceTreeConfig {
                field: "uart_clock_frequency",
            });
        }

        Ok(Self {
            timebase_frequency,
            cpu_isa,
            cpu_mmu_type,
            uart_clock_frequency,
            bootargs: None,
            initrd: None,
        })
    }

    pub fn with_bootargs(mut self, bootargs: impl Into<String>) -> Self {
        self.bootargs = Some(bootargs.into());
        self
    }

    pub fn with_initrd(mut self, start: Address, size: AccessSize) -> Result<Self, PlatformError> {
        self.initrd = Some(AddressRange::new(start, size).map_err(PlatformError::Memory)?);
        Ok(self)
    }

    pub const fn timebase_frequency(&self) -> u32 {
        self.timebase_frequency
    }

    pub fn cpu_isa(&self) -> &str {
        &self.cpu_isa
    }

    pub fn cpu_mmu_type(&self) -> &str {
        &self.cpu_mmu_type
    }

    pub const fn uart_clock_frequency(&self) -> u32 {
        self.uart_clock_frequency
    }

    pub fn bootargs(&self) -> Option<&str> {
        self.bootargs.as_deref()
    }

    pub const fn initrd(&self) -> Option<AddressRange> {
        self.initrd
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformDeviceTree {
    root: PlatformDeviceTreeNode,
}

impl PlatformDeviceTree {
    pub fn new(root: PlatformDeviceTreeNode) -> Self {
        Self { root }
    }

    pub const fn root(&self) -> &PlatformDeviceTreeNode {
        &self.root
    }

    pub fn to_dts(&self) -> String {
        let mut output = String::new();
        self.root.write_dts(0, &mut output);
        output
    }

    pub fn to_dtb(&self) -> Vec<u8> {
        let mut writer = PlatformDeviceTreeBlobWriter::new();
        writer.write_node(&self.root);
        writer.finish()
    }
}

const FDT_MAGIC: u32 = 0xd00d_feed;
const FDT_VERSION: u32 = 17;
const FDT_LAST_COMP_VERSION: u32 = 16;
const FDT_HEADER_BYTES: usize = 40;
const FDT_RESERVE_TERMINATOR_BYTES: usize = 16;
const FDT_BEGIN_NODE: u32 = 1;
const FDT_END_NODE: u32 = 2;
const FDT_PROP: u32 = 3;
const FDT_END: u32 = 9;

#[derive(Clone, Debug, Default)]
struct PlatformDeviceTreeBlobWriter {
    structure: Vec<u8>,
    strings: Vec<u8>,
}

impl PlatformDeviceTreeBlobWriter {
    fn new() -> Self {
        Self::default()
    }

    fn write_node(&mut self, node: &PlatformDeviceTreeNode) {
        write_be32(&mut self.structure, FDT_BEGIN_NODE);
        if node.name() == "/" {
            write_be32(&mut self.structure, 0);
        } else {
            self.structure.extend_from_slice(node.name().as_bytes());
            self.structure.push(0);
            pad_to_4(&mut self.structure);
        }

        for property in node.properties() {
            self.write_property(property);
        }
        for child in node.children() {
            self.write_node(child);
        }

        write_be32(&mut self.structure, FDT_END_NODE);
    }

    fn write_property(&mut self, property: &PlatformDeviceTreeProperty) {
        let value = encode_property_value(property.value());
        let name_offset = self.string_offset(property.name());
        write_be32(&mut self.structure, FDT_PROP);
        write_be32(&mut self.structure, value.len() as u32);
        write_be32(&mut self.structure, name_offset);
        self.structure.extend_from_slice(&value);
        pad_to_4(&mut self.structure);
    }

    fn string_offset(&mut self, name: &str) -> u32 {
        let mut cursor = 0usize;
        while cursor < self.strings.len() {
            let end = self.strings[cursor..]
                .iter()
                .position(|byte| *byte == 0)
                .map(|relative| cursor + relative)
                .expect("device-tree string table entries are nul-terminated");
            if &self.strings[cursor..end] == name.as_bytes() {
                return cursor as u32;
            }
            cursor = end + 1;
        }

        let offset = self.strings.len() as u32;
        self.strings.extend_from_slice(name.as_bytes());
        self.strings.push(0);
        offset
    }

    fn finish(mut self) -> Vec<u8> {
        write_be32(&mut self.structure, FDT_END);

        let reserve_offset = FDT_HEADER_BYTES;
        let structure_offset = reserve_offset + FDT_RESERVE_TERMINATOR_BYTES;
        let strings_offset = structure_offset + self.structure.len();
        let total_size = strings_offset + self.strings.len();

        let mut blob = Vec::with_capacity(total_size);
        write_be32(&mut blob, FDT_MAGIC);
        write_be32(&mut blob, total_size as u32);
        write_be32(&mut blob, structure_offset as u32);
        write_be32(&mut blob, strings_offset as u32);
        write_be32(&mut blob, reserve_offset as u32);
        write_be32(&mut blob, FDT_VERSION);
        write_be32(&mut blob, FDT_LAST_COMP_VERSION);
        write_be32(&mut blob, 0);
        write_be32(&mut blob, self.strings.len() as u32);
        write_be32(&mut blob, self.structure.len() as u32);
        blob.extend_from_slice(&[0; FDT_RESERVE_TERMINATOR_BYTES]);
        blob.extend_from_slice(&self.structure);
        blob.extend_from_slice(&self.strings);
        blob
    }
}

fn encode_property_value(value: &PlatformDeviceTreePropertyValue) -> Vec<u8> {
    match value {
        PlatformDeviceTreePropertyValue::Empty => Vec::new(),
        PlatformDeviceTreePropertyValue::Strings(values) => {
            let mut encoded = Vec::new();
            for value in values {
                encoded.extend_from_slice(value.as_bytes());
                encoded.push(0);
            }
            encoded
        }
        PlatformDeviceTreePropertyValue::Words(values) => {
            let mut encoded = Vec::with_capacity(values.len() * 4);
            for value in values {
                write_be32(&mut encoded, *value);
            }
            encoded
        }
        PlatformDeviceTreePropertyValue::DoubleWords(values) => {
            let mut encoded = Vec::with_capacity(values.len() * 8);
            for value in values {
                write_be64(&mut encoded, *value);
            }
            encoded
        }
    }
}

fn write_be32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn write_be64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_be_bytes());
}

fn pad_to_4(output: &mut Vec<u8>) {
    while !output.len().is_multiple_of(4) {
        output.push(0);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformDeviceTreeNode {
    name: String,
    properties: Vec<PlatformDeviceTreeProperty>,
    children: Vec<PlatformDeviceTreeNode>,
}

impl PlatformDeviceTreeNode {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn with_property(mut self, property: PlatformDeviceTreeProperty) -> Self {
        self.properties.push(property);
        self
    }

    pub fn with_child(mut self, child: PlatformDeviceTreeNode) -> Self {
        self.children.push(child);
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn properties(&self) -> &[PlatformDeviceTreeProperty] {
        &self.properties
    }

    pub fn children(&self) -> &[PlatformDeviceTreeNode] {
        &self.children
    }

    pub fn property(&self, name: &str) -> Option<&PlatformDeviceTreeProperty> {
        self.properties
            .iter()
            .find(|property| property.name() == name)
    }

    pub fn child(&self, name: &str) -> Option<&PlatformDeviceTreeNode> {
        self.children.iter().find(|child| child.name() == name)
    }

    fn write_dts(&self, indent: usize, output: &mut String) {
        write_indent(output, indent);
        let _ = writeln!(output, "{} {{", self.name);
        for property in &self.properties {
            property.write_dts(indent + 1, output);
        }
        for child in &self.children {
            child.write_dts(indent + 1, output);
        }
        write_indent(output, indent);
        let _ = writeln!(output, "}};");
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformDeviceTreeProperty {
    name: String,
    value: PlatformDeviceTreePropertyValue,
}

impl PlatformDeviceTreeProperty {
    pub fn empty(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: PlatformDeviceTreePropertyValue::Empty,
        }
    }

    pub fn string_list<I, S>(name: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            name: name.into(),
            value: PlatformDeviceTreePropertyValue::Strings(
                values.into_iter().map(Into::into).collect(),
            ),
        }
    }

    pub fn word_list<I>(name: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = u32>,
    {
        Self {
            name: name.into(),
            value: PlatformDeviceTreePropertyValue::Words(values.into_iter().collect()),
        }
    }

    pub fn double_word_list<I>(name: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = u64>,
    {
        Self {
            name: name.into(),
            value: PlatformDeviceTreePropertyValue::DoubleWords(values.into_iter().collect()),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn value(&self) -> &PlatformDeviceTreePropertyValue {
        &self.value
    }

    pub fn strings(&self) -> Option<&[String]> {
        match &self.value {
            PlatformDeviceTreePropertyValue::Strings(values) => Some(values),
            _ => None,
        }
    }

    pub fn words(&self) -> Option<&[u32]> {
        match &self.value {
            PlatformDeviceTreePropertyValue::Words(values) => Some(values),
            _ => None,
        }
    }

    pub fn double_words(&self) -> Option<&[u64]> {
        match &self.value {
            PlatformDeviceTreePropertyValue::DoubleWords(values) => Some(values),
            _ => None,
        }
    }

    fn write_dts(&self, indent: usize, output: &mut String) {
        write_indent(output, indent);
        let _ = write!(output, "{}", self.name);
        match &self.value {
            PlatformDeviceTreePropertyValue::Empty => {
                let _ = writeln!(output, ";");
            }
            PlatformDeviceTreePropertyValue::Strings(values) => {
                let _ = write!(output, " = ");
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        let _ = write!(output, ", ");
                    }
                    write_dts_string(output, value);
                }
                let _ = writeln!(output, ";");
            }
            PlatformDeviceTreePropertyValue::Words(values) => {
                let _ = write!(output, " = <");
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        let _ = write!(output, " ");
                    }
                    let _ = write!(output, "0x{value:x}");
                }
                let _ = writeln!(output, ">;");
            }
            PlatformDeviceTreePropertyValue::DoubleWords(values) => {
                let _ = write!(output, " = <");
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        let _ = write!(output, " ");
                    }
                    let high = (value >> 32) as u32;
                    let low = *value as u32;
                    let _ = write!(output, "0x{high:x} 0x{low:x}");
                }
                let _ = writeln!(output, ">;");
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformDeviceTreePropertyValue {
    Empty,
    Strings(Vec<String>),
    Words(Vec<u32>),
    DoubleWords(Vec<u64>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PlatformDeviceTreeInventory {
    memory_ranges: Vec<AddressRange>,
    interrupt_controllers: Vec<PlatformInterruptControllerConfig>,
    clints: Vec<PlatformClintConfig>,
    timers: Vec<PlatformTimerConfig>,
    uarts: Vec<PlatformUartConfig>,
    rtcs: Vec<PlatformRtcConfig>,
    pl031_rtcs: Vec<PlatformPl031RtcConfig>,
}

impl PlatformDeviceTreeInventory {
    pub(crate) fn new(
        memory_ranges: Vec<AddressRange>,
        interrupt_controllers: Vec<PlatformInterruptControllerConfig>,
        clints: Vec<PlatformClintConfig>,
        timers: Vec<PlatformTimerConfig>,
        uarts: Vec<PlatformUartConfig>,
        rtcs: Vec<PlatformRtcConfig>,
        pl031_rtcs: Vec<PlatformPl031RtcConfig>,
    ) -> Self {
        Self {
            memory_ranges,
            interrupt_controllers,
            clints,
            timers,
            uarts,
            rtcs,
            pl031_rtcs,
        }
    }

    pub(crate) fn riscv_device_tree(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
    ) -> Result<PlatformDeviceTree, PlatformError> {
        let hart_phandles = self.hart_phandles();
        let controller_phandles = self.interrupt_controller_phandles(hart_phandles.len() as u32);
        let cpus = self.cpus_node(config, &hart_phandles);
        let soc = self.soc_node(config, &hart_phandles, &controller_phandles)?;
        let mut root = PlatformDeviceTreeNode::new("/");
        if !self.memory_ranges.is_empty() {
            root = root
                .with_property(PlatformDeviceTreeProperty::word_list("#address-cells", [2]))
                .with_property(PlatformDeviceTreeProperty::word_list("#size-cells", [2]));
        }
        for range in &self.memory_ranges {
            root = root.with_child(Self::memory_node(*range));
        }
        root = root.with_child(cpus).with_child(soc);
        if let Some(chosen) = self.chosen_node(config) {
            root = root.with_child(chosen);
        }
        Ok(PlatformDeviceTree::new(root))
    }

    fn memory_node(range: AddressRange) -> PlatformDeviceTreeNode {
        PlatformDeviceTreeNode::new(device_node_name("memory", range.start()))
            .with_property(PlatformDeviceTreeProperty::string_list(
                "device_type",
                ["memory"],
            ))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "reg",
                address_size_cells(range.start(), range.size()),
            ))
    }

    fn chosen_node(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
    ) -> Option<PlatformDeviceTreeNode> {
        let mut chosen = PlatformDeviceTreeNode::new("chosen");
        if let Some(bootargs) = config.bootargs() {
            chosen = chosen.with_property(PlatformDeviceTreeProperty::string_list(
                "bootargs",
                [bootargs.to_string()],
            ));
        }
        if let Some(initrd) = config.initrd() {
            chosen = chosen
                .with_property(PlatformDeviceTreeProperty::double_word_list(
                    "linux,initrd-start",
                    [initrd.start().get()],
                ))
                .with_property(PlatformDeviceTreeProperty::double_word_list(
                    "linux,initrd-end",
                    [initrd.end().get()],
                ));
        }

        (!chosen.properties().is_empty()).then_some(chosen)
    }

    fn hart_phandles(&self) -> BTreeMap<u32, u32> {
        let mut harts = BTreeMap::new();
        for clint in &self.clints {
            for hart in &clint.harts {
                harts.insert(hart.hart, 0);
            }
        }

        for (index, phandle) in harts.values_mut().enumerate() {
            *phandle = index as u32 + 1;
        }
        harts
    }

    fn interrupt_controller_phandles(
        &self,
        hart_phandle_count: u32,
    ) -> BTreeMap<InterruptTargetId, u32> {
        let mut phandles = BTreeMap::new();
        let mut next_phandle = hart_phandle_count + 1;
        for controller in &self.interrupt_controllers {
            let phandle = next_phandle;
            next_phandle += 1;
            phandles.entry(controller.target).or_insert(phandle);
            for context in &controller.contexts {
                phandles.entry(context.target).or_insert(phandle);
            }
        }
        phandles
    }

    fn cpus_node(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
        hart_phandles: &BTreeMap<u32, u32>,
    ) -> PlatformDeviceTreeNode {
        let mut cpus = PlatformDeviceTreeNode::new("cpus").with_property(
            PlatformDeviceTreeProperty::word_list(
                "timebase-frequency",
                [config.timebase_frequency()],
            ),
        );

        for (hart, phandle) in hart_phandles {
            let interrupt_controller = PlatformDeviceTreeNode::new("interrupt-controller")
                .with_property(PlatformDeviceTreeProperty::word_list(
                    "#interrupt-cells",
                    [1],
                ))
                .with_property(PlatformDeviceTreeProperty::empty("interrupt-controller"))
                .with_property(PlatformDeviceTreeProperty::string_list(
                    "compatible",
                    ["riscv,cpu-intc"],
                ))
                .with_property(PlatformDeviceTreeProperty::word_list("phandle", [*phandle]));
            let cpu = PlatformDeviceTreeNode::new(format!("cpu@{hart:x}"))
                .with_property(PlatformDeviceTreeProperty::string_list(
                    "device_type",
                    ["cpu"],
                ))
                .with_property(PlatformDeviceTreeProperty::word_list("reg", [*hart]))
                .with_property(PlatformDeviceTreeProperty::string_list("status", ["okay"]))
                .with_property(PlatformDeviceTreeProperty::string_list(
                    "riscv,isa",
                    [config.cpu_isa().to_string()],
                ))
                .with_property(PlatformDeviceTreeProperty::string_list(
                    "mmu-type",
                    [config.cpu_mmu_type().to_string()],
                ))
                .with_property(PlatformDeviceTreeProperty::string_list(
                    "compatible",
                    ["riscv"],
                ))
                .with_child(interrupt_controller);
            cpus = cpus.with_child(cpu);
        }

        cpus
    }

    fn soc_node(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
        hart_phandles: &BTreeMap<u32, u32>,
        controller_phandles: &BTreeMap<InterruptTargetId, u32>,
    ) -> Result<PlatformDeviceTreeNode, PlatformError> {
        let mut soc = PlatformDeviceTreeNode::new("soc")
            .with_property(PlatformDeviceTreeProperty::word_list("#address-cells", [2]))
            .with_property(PlatformDeviceTreeProperty::word_list("#size-cells", [2]))
            .with_property(PlatformDeviceTreeProperty::empty("ranges"))
            .with_property(PlatformDeviceTreeProperty::string_list(
                "compatible",
                ["simple-bus"],
            ));

        for controller in &self.interrupt_controllers {
            let phandle = controller_phandles
                .get(&controller.target)
                .copied()
                .expect("validated interrupt-controller phandle");
            soc = soc.with_child(self.interrupt_controller_node(
                controller,
                phandle,
                hart_phandles,
            )?);
        }
        for clint in &self.clints {
            soc = soc.with_child(self.clint_node(clint, hart_phandles));
        }
        for uart in &self.uarts {
            let device = device_node_name("uart", uart.base);
            let Some(interrupt_parent) = controller_phandles.get(&uart.interrupt_target) else {
                return Err(PlatformError::DeviceTreeMissingInterruptController { device });
            };
            soc = soc.with_child(self.uart_node(uart, config, *interrupt_parent));
        }

        Ok(soc)
    }

    fn interrupt_controller_node(
        &self,
        controller: &PlatformInterruptControllerConfig,
        phandle: u32,
        hart_phandles: &BTreeMap<u32, u32>,
    ) -> Result<PlatformDeviceTreeNode, PlatformError> {
        let device = device_node_name("interrupt-controller", controller.base);
        let mut node = PlatformDeviceTreeNode::new(device.clone())
            .with_property(PlatformDeviceTreeProperty::word_list(
                "#interrupt-cells",
                [1],
            ))
            .with_property(PlatformDeviceTreeProperty::empty("interrupt-controller"))
            .with_property(PlatformDeviceTreeProperty::string_list(
                "compatible",
                ["riscv,plic0"],
            ))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "reg",
                address_size_cells(controller.base, controller.size),
            ))
            .with_property(PlatformDeviceTreeProperty::word_list("phandle", [phandle]))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "riscv,ndev",
                [self.max_external_interrupt_source(controller)],
            ));
        if !controller.contexts.is_empty() {
            let mut interrupts_extended = Vec::with_capacity(controller.contexts.len() * 2);
            for context in &controller.contexts {
                let hart_phandle = hart_phandles.get(&context.hart).copied().ok_or(
                    PlatformError::DeviceTreeMissingHart {
                        device: device.clone(),
                        hart: context.hart,
                    },
                )?;
                interrupts_extended.extend([hart_phandle, context.interrupt]);
            }
            node = node.with_property(PlatformDeviceTreeProperty::word_list(
                "interrupts-extended",
                interrupts_extended,
            ));
        }

        Ok(node)
    }

    fn clint_node(
        &self,
        clint: &PlatformClintConfig,
        hart_phandles: &BTreeMap<u32, u32>,
    ) -> PlatformDeviceTreeNode {
        let mut interrupts_extended = Vec::with_capacity(clint.harts.len() * 4);
        for hart in &clint.harts {
            let phandle = hart_phandles
                .get(&hart.hart)
                .copied()
                .expect("validated CLINT hart phandle");
            interrupts_extended.extend([phandle, 0x3, phandle, 0x7]);
        }

        PlatformDeviceTreeNode::new(device_node_name("clint", clint.base))
            .with_property(PlatformDeviceTreeProperty::string_list(
                "compatible",
                ["riscv,clint0"],
            ))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "reg",
                address_size_cells(clint.base, clint.size),
            ))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "interrupts-extended",
                interrupts_extended,
            ))
    }

    fn uart_node(
        &self,
        uart: &PlatformUartConfig,
        config: &PlatformRiscvDeviceTreeConfig,
        interrupt_parent: u32,
    ) -> PlatformDeviceTreeNode {
        PlatformDeviceTreeNode::new(device_node_name("uart", uart.base))
            .with_property(PlatformDeviceTreeProperty::string_list(
                "compatible",
                ["ns8250", "ns16550a"],
            ))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "reg",
                address_size_cells(uart.base, uart.size),
            ))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "interrupts",
                [uart.interrupt_source.get()],
            ))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "clock-frequency",
                [config.uart_clock_frequency()],
            ))
            .with_property(PlatformDeviceTreeProperty::word_list(
                "interrupt-parent",
                [interrupt_parent],
            ))
    }

    pub(crate) fn max_external_interrupt_source(
        &self,
        controller: &PlatformInterruptControllerConfig,
    ) -> u32 {
        self.timers
            .iter()
            .map(|timer| timer.interrupt_source.get())
            .chain(self.uarts.iter().map(|uart| uart.interrupt_source.get()))
            .chain(self.rtcs.iter().filter_map(|rtc| {
                rtc.periodic_interrupt
                    .map(|interrupt| interrupt.source.get())
            }))
            .chain(
                self.pl031_rtcs
                    .iter()
                    .filter_map(|rtc| rtc.interrupt.map(|interrupt| interrupt.source.get())),
            )
            .chain([controller.source_count])
            .max()
            .unwrap_or_default()
    }
}

fn device_node_name(kind: &str, base: Address) -> String {
    format!("{kind}@{:x}", base.get())
}

fn address_size_cells(base: Address, size: AccessSize) -> [u32; 4] {
    [
        (base.get() >> 32) as u32,
        base.get() as u32,
        (size.bytes() >> 32) as u32,
        size.bytes() as u32,
    ]
}

fn write_indent(output: &mut String, indent: usize) {
    for _ in 0..indent {
        let _ = write!(output, "    ");
    }
}

fn write_dts_string(output: &mut String, value: &str) {
    let _ = write!(output, "\"");
    for character in value.chars() {
        match character {
            '\\' => {
                let _ = write!(output, "\\\\");
            }
            '"' => {
                let _ = write!(output, "\\\"");
            }
            _ => {
                let _ = write!(output, "{character}");
            }
        }
    }
    let _ = write!(output, "\"");
}
