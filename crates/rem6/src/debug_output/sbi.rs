use crate::formatting::{bytes_to_hex, json_escape};
use crate::{
    Rem6RiscvSbiConsoleSummary, Rem6RiscvSbiHsmStatusSummary, Rem6RiscvSbiHsmSummary,
    Rem6RiscvSbiHsmWakeSummary, Rem6RiscvSbiIpiSummary, Rem6RiscvSbiResetSummary,
    Rem6RiscvSbiRfenceCompletionSummary, Rem6RiscvSbiRfenceSummary, Rem6RiscvSbiTimerSummary,
};

pub(crate) struct Rem6SbiTraceInputs<'a> {
    pub(crate) console: &'a Rem6RiscvSbiConsoleSummary,
    pub(crate) timers: &'a [Rem6RiscvSbiTimerSummary],
    pub(crate) hsm_events: &'a [Rem6RiscvSbiHsmSummary],
    pub(crate) hsm_wakes: &'a [Rem6RiscvSbiHsmWakeSummary],
    pub(crate) hsm_statuses: &'a [Rem6RiscvSbiHsmStatusSummary],
    pub(crate) ipis: &'a [Rem6RiscvSbiIpiSummary],
    pub(crate) rfences: &'a [Rem6RiscvSbiRfenceSummary],
    pub(crate) rfence_completions: &'a [Rem6RiscvSbiRfenceCompletionSummary],
    pub(crate) resets: &'a [Rem6RiscvSbiResetSummary],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6SbiTraceRecord {
    kind: &'static str,
    rank: u8,
    fields: Vec<Rem6SbiTraceField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6SbiTraceField {
    name: &'static str,
    value: Rem6SbiTraceValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Rem6SbiTraceValue {
    HexU64(u64),
    I32(i32),
    Null,
    String(String),
    U32(u32),
    U64(u64),
    U64List(Vec<u64>),
}

impl Rem6SbiTraceRecord {
    fn new(kind: &'static str, rank: u8, fields: Vec<Rem6SbiTraceField>) -> Self {
        Self { kind, rank, fields }
    }

    pub(crate) const fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) fn console_byte_count(&self) -> u64 {
        if self.kind != "console" {
            return 0;
        }
        self.fields
            .iter()
            .find(|field| field.name == "bytes")
            .and_then(Rem6SbiTraceField::as_u64)
            .unwrap_or(0)
    }

    pub(crate) fn target_count(&self) -> u64 {
        self.fields
            .iter()
            .find(|field| field.name == "targets")
            .and_then(Rem6SbiTraceField::list_len)
            .unwrap_or(0)
    }

    pub(crate) fn to_json(&self) -> String {
        let mut fields = vec![format!("\"kind\":\"{}\"", self.kind)];
        fields.extend(self.fields.iter().map(Rem6SbiTraceField::to_json));
        format!("{{{}}}", fields.join(","))
    }
}

impl Rem6SbiTraceField {
    fn to_json(&self) -> String {
        format!("\"{}\":{}", self.name, self.value.to_json())
    }

    fn as_u64(&self) -> Option<u64> {
        match self.value {
            Rem6SbiTraceValue::U64(value) => Some(value),
            Rem6SbiTraceValue::U32(value) => Some(u64::from(value)),
            _ => None,
        }
    }

    fn list_len(&self) -> Option<u64> {
        match &self.value {
            Rem6SbiTraceValue::U64List(values) => Some(values.len() as u64),
            _ => None,
        }
    }
}

impl Rem6SbiTraceValue {
    fn to_json(&self) -> String {
        match self {
            Self::HexU64(value) => format!("\"0x{value:x}\""),
            Self::I32(value) => value.to_string(),
            Self::Null => "null".to_string(),
            Self::String(value) => format!("\"{}\"", json_escape(value)),
            Self::U32(value) => value.to_string(),
            Self::U64(value) => value.to_string(),
            Self::U64List(values) => format!(
                "[{}]",
                values
                    .iter()
                    .map(u64::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

pub(crate) fn sbi_trace_records(inputs: Rem6SbiTraceInputs<'_>) -> Vec<Rem6SbiTraceRecord> {
    let mut records = Vec::new();
    if inputs.console.byte_count() > 0 || inputs.console.dbcn_byte_count() > 0 {
        records.push(console_record(inputs.console));
    }
    records.extend(inputs.timers.iter().map(timer_record));
    records.extend(inputs.hsm_events.iter().map(hsm_event_record));
    records.extend(inputs.hsm_wakes.iter().map(hsm_wake_record));
    records.extend(inputs.hsm_statuses.iter().map(hsm_status_record));
    records.extend(inputs.ipis.iter().map(ipi_record));
    records.extend(inputs.rfences.iter().map(rfence_record));
    records.extend(
        inputs
            .rfence_completions
            .iter()
            .map(rfence_completion_record),
    );
    records.extend(inputs.resets.iter().map(reset_record));
    records.sort_by_key(|record| record.rank);
    records
}

fn console_record(summary: &Rem6RiscvSbiConsoleSummary) -> Rem6SbiTraceRecord {
    let text = std::str::from_utf8(summary.bytes())
        .ok()
        .map(str::to_string)
        .map(Rem6SbiTraceValue::String)
        .unwrap_or(Rem6SbiTraceValue::Null);
    Rem6SbiTraceRecord::new(
        "console",
        10,
        vec![
            field_u64("bytes", summary.byte_count()),
            field_u64("dbcn_bytes", summary.dbcn_byte_count()),
            Rem6SbiTraceField {
                name: "text",
                value: text,
            },
            field_string("hex", &bytes_to_hex(summary.bytes())),
        ],
    )
}

fn timer_record(summary: &Rem6RiscvSbiTimerSummary) -> Rem6SbiTraceRecord {
    Rem6SbiTraceRecord::new(
        "timer",
        20,
        vec![
            field_u32("cpu", summary.cpu()),
            field_u64("deadline", summary.deadline()),
        ],
    )
}

fn hsm_event_record(summary: &Rem6RiscvSbiHsmSummary) -> Rem6SbiTraceRecord {
    let fields = if summary.is_hart_suspend() {
        vec![
            field_u32("source_cpu", summary.source_cpu()),
            field_u64("function", summary.function()),
            field_hex_u64("suspend_type", summary.arg0()),
            field_hex_u64("resume_addr", summary.arg1()),
            field_hex_u64("opaque", summary.arg2()),
        ]
    } else {
        vec![
            field_u32("source_cpu", summary.source_cpu()),
            field_u64("function", summary.function()),
            field_u64("target_hart", summary.arg0()),
            field_hex_u64("start_addr", summary.arg1()),
            field_hex_u64("opaque", summary.arg2()),
        ]
    };
    Rem6SbiTraceRecord::new("hsm_event", 30, fields)
}

fn hsm_wake_record(summary: &Rem6RiscvSbiHsmWakeSummary) -> Rem6SbiTraceRecord {
    Rem6SbiTraceRecord::new(
        "hsm_wake",
        40,
        vec![
            field_u32("source_cpu", summary.source_cpu()),
            field_u64("target_hart", summary.target_hart()),
            field_hex_u64("interrupt_bits", summary.interrupt_bits()),
        ],
    )
}

fn hsm_status_record(summary: &Rem6RiscvSbiHsmStatusSummary) -> Rem6SbiTraceRecord {
    Rem6SbiTraceRecord::new(
        "hsm_status",
        50,
        vec![
            field_u32("source_cpu", summary.source_cpu()),
            field_u64("target_hart", summary.target_hart()),
            field_u64("status", summary.status()),
            field_string("status_name", summary.status_name()),
        ],
    )
}

fn ipi_record(summary: &Rem6RiscvSbiIpiSummary) -> Rem6SbiTraceRecord {
    Rem6SbiTraceRecord::new(
        "ipi",
        60,
        vec![
            field_u32("source_cpu", summary.source_cpu()),
            field_hex_u64("hart_mask", summary.hart_mask()),
            field_hex_u64("hart_mask_base", summary.hart_mask_base()),
            field_u64("target_count", summary.target_count()),
            field_u64_list("targets", summary.targets()),
        ],
    )
}

fn rfence_record(summary: &Rem6RiscvSbiRfenceSummary) -> Rem6SbiTraceRecord {
    Rem6SbiTraceRecord::new(
        "rfence",
        70,
        vec![
            field_u32("source_cpu", summary.source_cpu()),
            field_u64("function", summary.function()),
            field_hex_u64("hart_mask", summary.hart_mask()),
            field_hex_u64("hart_mask_base", summary.hart_mask_base()),
            field_hex_u64("start_addr", summary.start_addr()),
            field_hex_u64("size", summary.size()),
            field_optional_u64("address_space", summary.address_space()),
            field_u64("target_count", summary.target_count()),
            field_u64_list("targets", summary.targets()),
        ],
    )
}

fn rfence_completion_record(summary: &Rem6RiscvSbiRfenceCompletionSummary) -> Rem6SbiTraceRecord {
    Rem6SbiTraceRecord::new(
        "rfence_completion",
        80,
        vec![
            field_u32("source_cpu", summary.source_cpu()),
            field_u64("target_hart", summary.target_hart()),
            field_u64("function", summary.function()),
            field_hex_u64("start_addr", summary.start_addr()),
            field_hex_u64("size", summary.size()),
            field_optional_u64("address_space", summary.address_space()),
            field_u64("completed_tick", summary.completed_tick()),
            field_optional_u64("flushed_entries", summary.flushed_entries()),
        ],
    )
}

fn reset_record(summary: &Rem6RiscvSbiResetSummary) -> Rem6SbiTraceRecord {
    Rem6SbiTraceRecord::new(
        "reset",
        90,
        vec![
            field_u32("cpu", summary.cpu()),
            field_u32("reset_type", summary.reset_type()),
            field_u32("reset_reason", summary.reset_reason()),
            field_i32("code", summary.code()),
        ],
    )
}

fn field_hex_u64(name: &'static str, value: u64) -> Rem6SbiTraceField {
    Rem6SbiTraceField {
        name,
        value: Rem6SbiTraceValue::HexU64(value),
    }
}

fn field_i32(name: &'static str, value: i32) -> Rem6SbiTraceField {
    Rem6SbiTraceField {
        name,
        value: Rem6SbiTraceValue::I32(value),
    }
}

fn field_optional_u64(name: &'static str, value: Option<u64>) -> Rem6SbiTraceField {
    Rem6SbiTraceField {
        name,
        value: value
            .map(Rem6SbiTraceValue::U64)
            .unwrap_or(Rem6SbiTraceValue::Null),
    }
}

fn field_string(name: &'static str, value: &str) -> Rem6SbiTraceField {
    Rem6SbiTraceField {
        name,
        value: Rem6SbiTraceValue::String(value.to_string()),
    }
}

fn field_u32(name: &'static str, value: u32) -> Rem6SbiTraceField {
    Rem6SbiTraceField {
        name,
        value: Rem6SbiTraceValue::U32(value),
    }
}

fn field_u64(name: &'static str, value: u64) -> Rem6SbiTraceField {
    Rem6SbiTraceField {
        name,
        value: Rem6SbiTraceValue::U64(value),
    }
}

fn field_u64_list(name: &'static str, values: &[u64]) -> Rem6SbiTraceField {
    Rem6SbiTraceField {
        name,
        value: Rem6SbiTraceValue::U64List(values.to_vec()),
    }
}
