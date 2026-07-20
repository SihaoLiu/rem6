use std::fs;
use std::path::PathBuf;

#[test]
fn checkpoint_summaries_derive_hierarchy_totals_from_projections() {
    let source = fs::read_to_string(source_path()).unwrap();
    let component_body = struct_body(&source, "CheckpointComponentSummary");
    let manifest_body = struct_body(&source, "CheckpointManifestSummary");
    let compact_source = without_whitespace(&source);

    for (summary, body) in [
        ("CheckpointComponentSummary", component_body),
        ("CheckpointManifestSummary", manifest_body),
    ] {
        assert!(
            !body.contains("chunk_count:"),
            "{summary} must not cache chunk_count"
        );
        assert!(
            !body.contains("payload_bytes:"),
            "{summary} must not cache payload_bytes"
        );
    }

    assert!(
        !compact_source.contains("pubfnnew(component:CheckpointComponentId,chunk_count:"),
        "CheckpointComponentSummary must not expose the aggregate-only constructor"
    );
    assert!(
        compact_source.contains("self.chunk_summaries.len()"),
        "component chunk_count must derive from chunk_summaries.len()"
    );
    assert!(
        compact_source.contains(
            "self.chunk_summaries.iter().map(CheckpointChunkSummary::payload_bytes).sum()"
        ),
        "component payload_bytes must derive from chunk payload projections"
    );
    assert!(
        compact_source.contains(
            "self.component_summaries.iter().map(CheckpointComponentSummary::chunk_count).sum()"
        ),
        "manifest chunk_count must derive from component chunk projections"
    );
    assert!(
        compact_source.contains(
            "self.component_summaries.iter().map(CheckpointComponentSummary::payload_bytes).sum()"
        ),
        "manifest payload_bytes must derive from component payload projections"
    );
}

fn source_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs")
}

fn struct_body<'a>(source: &'a str, name: &str) -> &'a str {
    let needle = format!("pub struct {name}");
    let struct_start = source
        .find(&needle)
        .unwrap_or_else(|| panic!("missing {needle}"));
    let open = source[struct_start..]
        .find('{')
        .map(|offset| struct_start + offset)
        .unwrap_or_else(|| panic!("missing body for {needle}"));

    let mut depth = 0usize;
    for (offset, byte) in source[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open + 1..open + offset];
                }
            }
            _ => {}
        }
    }

    panic!("unterminated body for {needle}");
}

fn without_whitespace(source: &str) -> String {
    source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}
