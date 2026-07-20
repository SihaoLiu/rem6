use std::{fs, path::Path};

pub(crate) fn checkpoint_output_summaries_derive_hierarchy_totals_from_projections() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host_actions_path = crate_dir.join("src/host_actions.rs");
    let summary_totals_path = crate_dir.join("src/host_actions/summary_totals.rs");
    let checkpoint_json_path = crate_dir.join("src/artifact_json/checkpoint.rs");

    let host_actions = fs::read_to_string(host_actions_path).unwrap();
    let checkpoint_json = fs::read_to_string(checkpoint_json_path).unwrap();

    assert!(
        normalized(&host_actions).contains("mod summary_totals;"),
        "src/host_actions.rs must delegate derived checkpoint totals to summary_totals"
    );
    assert!(
        summary_totals_path.is_file(),
        "src/host_actions/summary_totals.rs must own derived checkpoint total accessors"
    );

    let host_action_body = struct_body(&host_actions, "Rem6HostActionSummary");
    assert_absent_fields(
        "Rem6HostActionSummary",
        host_action_body,
        &[
            "checkpoint_restored_count",
            "checkpoint_restored_component_count",
            "checkpoint_restored_chunk_count",
            "checkpoint_restored_payload_bytes",
        ],
    );

    let checkpoint_body = struct_body(&host_actions, "Rem6HostCheckpointSummary");
    assert_absent_fields(
        "Rem6HostCheckpointSummary",
        checkpoint_body,
        &["component_count", "chunk_count", "payload_bytes"],
    );

    let checkpoint_component_body =
        struct_body(&host_actions, "Rem6HostCheckpointComponentSummary");
    assert_absent_fields(
        "Rem6HostCheckpointComponentSummary",
        checkpoint_component_body,
        &["chunk_count", "payload_bytes"],
    );

    let transfer_body = struct_body(&host_actions, "Rem6ExecutionModeStateTransferSummary");
    assert_absent_fields(
        "Rem6ExecutionModeStateTransferSummary",
        transfer_body,
        &["component_count", "chunk_count", "payload_bytes"],
    );

    let summary_totals = fs::read_to_string(summary_totals_path).unwrap();
    let summary_totals = normalized(&summary_totals);
    for signature in [
        "pub(crate) const fn checkpoint_restored_count(&self) -> u64",
        "pub(crate) const fn component_count(&self) -> u64",
        "pub(crate) const fn chunk_count(&self) -> u64",
    ] {
        assert!(
            summary_totals.contains(signature),
            "summary_totals.rs must preserve const len-only accessor `{signature}`"
        );
    }
    for signature in [
        "pub(crate) fn checkpoint_restored_component_count(&self) -> u64",
        "pub(crate) fn checkpoint_restored_chunk_count(&self) -> u64",
        "pub(crate) fn checkpoint_restored_payload_bytes(&self) -> u64",
        "pub(crate) fn chunk_count(&self) -> u64",
        "pub(crate) fn payload_bytes(&self) -> u64",
    ] {
        assert!(
            summary_totals.contains(signature),
            "summary_totals.rs must expose non-const summing accessor `{signature}`"
        );
    }
    for body_anchor in [
        "self.checkpoint_restores.len() as u64",
        "self.components.len() as u64",
        "self.chunks.len() as u64",
        ".map(Rem6HostCheckpointSummary::component_count)",
        ".map(Rem6HostCheckpointSummary::chunk_count)",
        ".map(Rem6HostCheckpointSummary::payload_bytes)",
        ".map(Rem6HostCheckpointComponentSummary::chunk_count)",
        ".map(Rem6HostCheckpointComponentSummary::payload_bytes)",
        ".map(|chunk| chunk.payload_bytes)",
    ] {
        assert!(
            summary_totals.contains(body_anchor),
            "summary_totals.rs must derive totals from owned projections: missing `{body_anchor}`"
        );
    }

    assert_uses_accessors(
        impl_body(&checkpoint_json, "Rem6HostCheckpointSummary"),
        "self",
        &["component_count", "chunk_count", "payload_bytes"],
        "checkpoint JSON root",
    );
    assert_uses_accessors(
        impl_body(&checkpoint_json, "Rem6HostCheckpointComponentSummary"),
        "self",
        &["chunk_count", "payload_bytes"],
        "checkpoint component JSON",
    );
}

fn assert_absent_fields(struct_name: &str, body: &str, fields: &[&str]) {
    let body = normalized(body);
    for field in fields {
        assert!(
            !body.contains(&format!("pub(crate) {field}:")) && !body.contains(&format!("{field}:")),
            "{struct_name} must derive `{field}` from owned projections instead of storing it"
        );
    }
}

fn assert_uses_accessors(source: &str, receiver: &str, fields: &[&str], context: &str) {
    for field in fields {
        assert!(
            !source.contains(&format!("{receiver}.{field},"))
                && !source.contains(&format!("{receiver}.{field} ")),
            "{context} must not read `{receiver}.{field}` directly"
        );
        assert!(
            source.contains(&format!("{receiver}.{field}()")),
            "{context} must serialize `{field}` through its derived accessor"
        );
    }
}

fn struct_body<'a>(source: &'a str, struct_name: &str) -> &'a str {
    let needle = format!("struct {struct_name}");
    let struct_start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("missing struct `{struct_name}`"));
    let open = struct_start
        + source[struct_start..]
            .find('{')
            .unwrap_or_else(|| panic!("missing opening brace for `{struct_name}`"));
    let mut depth = 0usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("missing closing brace for `{struct_name}`");
}

fn impl_body<'a>(source: &'a str, type_name: &str) -> &'a str {
    let needle = format!("impl {type_name}");
    let impl_start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("missing impl `{type_name}`"));
    let open = impl_start
        + source[impl_start..]
            .find('{')
            .unwrap_or_else(|| panic!("missing opening brace for impl `{type_name}`"));
    let mut depth = 0usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }
    panic!("missing closing brace for impl `{type_name}`");
}

fn normalized(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}
