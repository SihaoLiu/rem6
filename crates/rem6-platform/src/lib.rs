use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

use rem6_interrupt::{
    InterruptController, InterruptControllerMmioDevice, InterruptError, InterruptLineChannel,
    InterruptLineId, InterruptLinePort, InterruptRoute, InterruptSourceId, InterruptTargetId,
};
use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{AccessSize, Address, AddressRange, MemoryError};
use rem6_mmio::{MmioBus, MmioError, MmioRoute};
use rem6_timer::{
    ClintHartConfig, ClintId, ClintMmioDevice, ClintResetPolicy, ProgrammableTimer, TimerError,
    TimerId, TimerMmioDevice,
};
use rem6_topology::{Endpoint, Topology, TopologyError};
use rem6_uart::{UartId, UartMmioDevice};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformTimerConfig {
    pub id: TimerId,
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub interrupt_line: InterruptLineId,
    pub interrupt_target: InterruptTargetId,
    pub interrupt_source: InterruptSourceId,
    pub interrupt_latency: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformUartConfig {
    pub id: UartId,
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub interrupt_line: InterruptLineId,
    pub interrupt_target: InterruptTargetId,
    pub interrupt_source: InterruptSourceId,
    pub interrupt_latency: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformClintHartConfig {
    pub hart: u32,
    pub target_partition: PartitionId,
    pub interrupt_target: InterruptTargetId,
    pub software_interrupt_line: InterruptLineId,
    pub software_interrupt_source: InterruptSourceId,
    pub timer_interrupt_line: InterruptLineId,
    pub timer_interrupt_source: InterruptSourceId,
    pub interrupt_latency: Tick,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformClintConfig {
    pub id: ClintId,
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub reset_policy: ClintResetPolicy,
    pub harts: Vec<PlatformClintHartConfig>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlatformInterruptControllerConfig {
    pub base: Address,
    pub size: AccessSize,
    pub route: MmioRoute,
    pub target: InterruptTargetId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformRiscvDeviceTreeConfig {
    timebase_frequency: u32,
    cpu_isa: String,
    cpu_mmu_type: String,
    uart_clock_frequency: u32,
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
        })
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
    }
}

fn write_be32(output: &mut Vec<u8>, value: u32) {
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
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformDeviceTreePropertyValue {
    Empty,
    Strings(Vec<String>),
    Words(Vec<u32>),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PlatformDeviceTreeInventory {
    interrupt_controllers: Vec<PlatformInterruptControllerConfig>,
    clints: Vec<PlatformClintConfig>,
    timers: Vec<PlatformTimerConfig>,
    uarts: Vec<PlatformUartConfig>,
}

impl PlatformDeviceTreeInventory {
    fn new(
        interrupt_controllers: Vec<PlatformInterruptControllerConfig>,
        clints: Vec<PlatformClintConfig>,
        timers: Vec<PlatformTimerConfig>,
        uarts: Vec<PlatformUartConfig>,
    ) -> Self {
        Self {
            interrupt_controllers,
            clints,
            timers,
            uarts,
        }
    }

    fn riscv_device_tree(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
    ) -> Result<PlatformDeviceTree, PlatformError> {
        let hart_phandles = self.hart_phandles();
        let controller_phandles = self.interrupt_controller_phandles(hart_phandles.len() as u32);
        let cpus = self.cpus_node(config, &hart_phandles);
        let soc = self.soc_node(config, &hart_phandles, &controller_phandles)?;
        let root = PlatformDeviceTreeNode::new("/")
            .with_child(cpus)
            .with_child(soc);
        Ok(PlatformDeviceTree::new(root))
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
            if let std::collections::btree_map::Entry::Vacant(entry) =
                phandles.entry(controller.target)
            {
                entry.insert(next_phandle);
                next_phandle += 1;
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
            soc = soc.with_child(self.interrupt_controller_node(controller, phandle));
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
    ) -> PlatformDeviceTreeNode {
        PlatformDeviceTreeNode::new(device_node_name("interrupt-controller", controller.base))
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
                [self.max_external_interrupt_source()],
            ))
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

    fn max_external_interrupt_source(&self) -> u32 {
        self.timers
            .iter()
            .map(|timer| timer.interrupt_source.get())
            .chain(self.uarts.iter().map(|uart| uart.interrupt_source.get()))
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformTopologyRoute {
    source: Endpoint,
    target: Endpoint,
}

impl PlatformTopologyRoute {
    pub fn new(source: Endpoint, target: Endpoint) -> Self {
        Self { source, target }
    }

    pub const fn source(&self) -> &Endpoint {
        &self.source
    }

    pub const fn target(&self) -> &Endpoint {
        &self.target
    }

    pub fn resolve(&self, topology: &Topology) -> Result<MmioRoute, PlatformTopologyError> {
        let source_partition = endpoint_partition(topology, &self.source)?;
        let target_partition = endpoint_partition(topology, &self.target)?;
        let path = topology
            .find_endpoint_path(&self.source, &self.target)
            .ok_or_else(|| PlatformTopologyError::MissingPath {
                source: self.source.clone(),
                target: self.target.clone(),
            })?;

        MmioRoute::new(
            source_partition,
            target_partition,
            path.request_latency(),
            path.response_latency(),
        )
        .map_err(PlatformTopologyError::Mmio)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PlatformBuilder {
    partition_count: u32,
    interrupt_controllers: Vec<PlatformInterruptControllerConfig>,
    clints: Vec<PlatformClintConfig>,
    timers: Vec<PlatformTimerConfig>,
    uarts: Vec<PlatformUartConfig>,
}

impl PlatformBuilder {
    pub const fn new(partition_count: u32) -> Self {
        Self {
            partition_count,
            interrupt_controllers: Vec::new(),
            clints: Vec::new(),
            timers: Vec::new(),
            uarts: Vec::new(),
        }
    }

    pub fn from_topology(topology: &Topology) -> Self {
        Self::new(topology.partition_count())
    }

    pub fn add_interrupt_controller(mut self, config: PlatformInterruptControllerConfig) -> Self {
        self.interrupt_controllers.push(config);
        self
    }

    pub fn add_timer(mut self, config: PlatformTimerConfig) -> Self {
        self.timers.push(config);
        self
    }

    pub fn add_clint(mut self, config: PlatformClintConfig) -> Self {
        self.clints.push(config);
        self
    }

    pub fn add_uart(mut self, config: PlatformUartConfig) -> Self {
        self.uarts.push(config);
        self
    }

    pub fn build(self) -> Result<Platform, PlatformError> {
        if self.partition_count == 0 {
            return Err(PlatformError::NoPartitions);
        }

        let device_tree_inventory = PlatformDeviceTreeInventory::new(
            self.interrupt_controllers.clone(),
            self.clints.clone(),
            self.timers.clone(),
            self.uarts.clone(),
        );
        let controller = Arc::new(Mutex::new(InterruptController::new()));
        let mut bus = MmioBus::new();
        let mut clints = BTreeMap::new();
        let mut timers = BTreeMap::new();
        let mut uarts = BTreeMap::new();

        for config in self.interrupt_controllers {
            validate_route(self.partition_count, config.route)?;
            let device = InterruptControllerMmioDevice::new(
                Arc::clone(&controller),
                config.base,
                config.target,
                config.route.source_partition(),
            );
            bus.insert_device(region(config.base, config.size)?, config.route, device)
                .map_err(PlatformError::Mmio)?;
        }

        for config in self.timers {
            validate_route(self.partition_count, config.route)?;
            let port = register_interrupt(
                &controller,
                config.route.source_partition(),
                config.interrupt_line,
                config.interrupt_target,
                config.interrupt_latency,
            )?;
            let timer = ProgrammableTimer::new(
                config.id,
                config.route.target_partition(),
                config.interrupt_source,
                port,
            );
            let device = TimerMmioDevice::new(timer.clone(), config.base);
            bus.insert_device(region(config.base, config.size)?, config.route, device)
                .map_err(PlatformError::Mmio)?;
            timers.insert(config.id, timer);
        }

        for config in self.clints {
            validate_route(self.partition_count, config.route)?;
            let mut harts = Vec::with_capacity(config.harts.len());
            for hart in config.harts {
                validate_partition(self.partition_count, hart.target_partition)?;
                let software_port = register_interrupt(
                    &controller,
                    hart.target_partition,
                    hart.software_interrupt_line,
                    hart.interrupt_target,
                    hart.interrupt_latency,
                )?;
                let timer_port = register_interrupt(
                    &controller,
                    hart.target_partition,
                    hart.timer_interrupt_line,
                    hart.interrupt_target,
                    hart.interrupt_latency,
                )?;
                harts.push(ClintHartConfig::new(
                    hart.hart,
                    software_port,
                    hart.software_interrupt_source,
                    timer_port,
                    hart.timer_interrupt_source,
                ));
            }
            let device =
                ClintMmioDevice::with_reset_policy(config.base, harts, config.reset_policy)
                    .map_err(PlatformError::Timer)?;
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            clints.insert(config.id, device);
        }

        for config in self.uarts {
            validate_route(self.partition_count, config.route)?;
            let port = register_interrupt(
                &controller,
                config.route.source_partition(),
                config.interrupt_line,
                config.interrupt_target,
                config.interrupt_latency,
            )?;
            let device = UartMmioDevice::with_interrupt(
                config.id,
                config.base,
                config.interrupt_source,
                port,
            );
            bus.insert_device(
                region(config.base, config.size)?,
                config.route,
                device.clone(),
            )
            .map_err(PlatformError::Mmio)?;
            uarts.insert(config.id, device);
        }

        Ok(Platform {
            partition_count: self.partition_count,
            interrupt_controller: controller,
            mmio_bus: bus,
            clints,
            timers,
            uarts,
            device_tree_inventory,
        })
    }
}

#[derive(Clone)]
pub struct Platform {
    partition_count: u32,
    interrupt_controller: Arc<Mutex<InterruptController>>,
    mmio_bus: MmioBus,
    clints: BTreeMap<ClintId, ClintMmioDevice>,
    timers: BTreeMap<TimerId, ProgrammableTimer>,
    uarts: BTreeMap<UartId, UartMmioDevice>,
    device_tree_inventory: PlatformDeviceTreeInventory,
}

impl Platform {
    pub const fn partition_count(&self) -> u32 {
        self.partition_count
    }

    pub fn interrupt_controller(&self) -> Arc<Mutex<InterruptController>> {
        Arc::clone(&self.interrupt_controller)
    }

    pub const fn mmio_bus(&self) -> &MmioBus {
        &self.mmio_bus
    }

    pub fn clint(&self, id: ClintId) -> Option<&ClintMmioDevice> {
        self.clints.get(&id)
    }

    pub fn clints(&self) -> impl Iterator<Item = (ClintId, &ClintMmioDevice)> {
        self.clints.iter().map(|(id, device)| (*id, device))
    }

    pub fn timer(&self, id: TimerId) -> Option<&ProgrammableTimer> {
        self.timers.get(&id)
    }

    pub fn timers(&self) -> impl Iterator<Item = (TimerId, &ProgrammableTimer)> {
        self.timers.iter().map(|(id, timer)| (*id, timer))
    }

    pub fn uart(&self, id: UartId) -> Option<&UartMmioDevice> {
        self.uarts.get(&id)
    }

    pub fn uarts(&self) -> impl Iterator<Item = (UartId, &UartMmioDevice)> {
        self.uarts.iter().map(|(id, device)| (*id, device))
    }

    pub fn riscv_device_tree(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
    ) -> Result<PlatformDeviceTree, PlatformError> {
        self.device_tree_inventory.riscv_device_tree(config)
    }
}

fn register_interrupt(
    controller: &Arc<Mutex<InterruptController>>,
    target_partition: PartitionId,
    line: InterruptLineId,
    target: InterruptTargetId,
    latency: Tick,
) -> Result<InterruptLinePort, PlatformError> {
    let route = InterruptRoute::new(line, target, target_partition);
    controller
        .lock()
        .expect("platform interrupt controller lock")
        .register_route(route)
        .map_err(PlatformError::Interrupt)?;
    let channel = InterruptLineChannel::new(route, latency).map_err(PlatformError::Interrupt)?;
    Ok(InterruptLinePort::new(channel, Arc::clone(controller)))
}

fn region(base: Address, size: AccessSize) -> Result<AddressRange, PlatformError> {
    AddressRange::new(base, size).map_err(PlatformError::Memory)
}

fn validate_route(partition_count: u32, route: MmioRoute) -> Result<(), PlatformError> {
    validate_partition(partition_count, route.source_partition())?;
    validate_partition(partition_count, route.target_partition())
}

fn validate_partition(partitions: u32, partition: PartitionId) -> Result<(), PlatformError> {
    if partition.index() >= partitions {
        return Err(PlatformError::UnknownPartition {
            partition,
            partitions,
        });
    }

    Ok(())
}

fn endpoint_partition(
    topology: &Topology,
    endpoint: &Endpoint,
) -> Result<PartitionId, PlatformTopologyError> {
    let component = topology.component(endpoint.component()).ok_or_else(|| {
        PlatformTopologyError::Topology(TopologyError::UnknownComponent {
            component: endpoint.component().clone(),
        })
    })?;
    component.port_direction(endpoint.port()).ok_or_else(|| {
        PlatformTopologyError::Topology(TopologyError::UnknownPort {
            component: endpoint.component().clone(),
            port: endpoint.port().clone(),
        })
    })?;

    Ok(component.partition())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformTopologyError {
    MissingPath { source: Endpoint, target: Endpoint },
    Topology(TopologyError),
    Mmio(MmioError),
}

impl fmt::Display for PlatformTopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPath { source, target } => write!(
                formatter,
                "topology path from {}.{} to {}.{} is not declared",
                source.component().as_str(),
                source.port().as_str(),
                target.component().as_str(),
                target.port().as_str()
            ),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::Mmio(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PlatformTopologyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Topology(error) => Some(error),
            Self::Mmio(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlatformError {
    NoPartitions,
    InvalidDeviceTreeConfig {
        field: &'static str,
    },
    DeviceTreeMissingInterruptController {
        device: String,
    },
    UnknownPartition {
        partition: PartitionId,
        partitions: u32,
    },
    Memory(MemoryError),
    Mmio(MmioError),
    Interrupt(InterruptError),
    Timer(TimerError),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPartitions => write!(formatter, "platform requires at least one partition"),
            Self::InvalidDeviceTreeConfig { field } => {
                write!(formatter, "invalid RISC-V device tree config field {field}")
            }
            Self::DeviceTreeMissingInterruptController { device } => write!(
                formatter,
                "RISC-V device tree node {device} has no interrupt controller"
            ),
            Self::UnknownPartition {
                partition,
                partitions,
            } => write!(
                formatter,
                "partition {} is outside platform partition count {partitions}",
                partition.index()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Mmio(error) => write!(formatter, "{error}"),
            Self::Interrupt(error) => write!(formatter, "{error}"),
            Self::Timer(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for PlatformError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Mmio(error) => Some(error),
            Self::Interrupt(error) => Some(error),
            Self::Timer(error) => Some(error),
            _ => None,
        }
    }
}
