use std::fs;
use std::path::Path;

use super::{line_count, rust_function_definition_names};

const MAX_CLI_CONFIG_LINES: usize = 500;
const MAX_CONFIG_ROOT_LINES: usize = 1700;

#[test]
fn core_cli_config_mechanics_have_one_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let config_path = crate_dir.join("src/config.rs");
    let config = fs::read_to_string(&config_path).unwrap();
    let parse_path = crate_dir.join("src/config/parse.rs");
    let parse = fs::read_to_string(&parse_path).unwrap();
    let cli_config_path = crate_dir.join("src/cli_config.rs");

    assert!(
        lib.lines().any(|line| line.trim() == "mod cli_config;"),
        "src/lib.rs must declare the shared CLI config authority"
    );
    assert!(
        cli_config_path.is_file(),
        "core CLI config mechanics belong in src/cli_config.rs"
    );
    assert!(
        !crate_dir.join("src/config/file_scan.rs").exists(),
        "src/config/file_scan.rs must not remain as a second config-scan authority"
    );
    assert!(
        line_count(&cli_config_path) <= MAX_CLI_CONFIG_LINES,
        "src/cli_config.rs must stay focused"
    );
    assert!(
        line_count(&config_path) < MAX_CONFIG_ROOT_LINES,
        "src/config.rs must stay below {MAX_CONFIG_ROOT_LINES} lines"
    );

    let cli_config = fs::read_to_string(&cli_config_path).unwrap();
    let cli_config_functions = rust_function_definition_names(&cli_config);
    for function in [
        "config_path_from_args",
        "run_file_config_from_args",
        "gups_file_config_from_args",
        "trace_replay_file_config_from_args",
        "read_toml_config",
        "required_value",
        "resolve_config_path",
    ] {
        assert!(
            cli_config_functions.contains(function),
            "src/cli_config.rs must own `{function}`"
        );
    }

    let config_functions = rust_function_definition_names(&config);
    assert!(
        !config_functions.contains("resolve_config_path"),
        "src/config.rs must delegate path resolution to src/cli_config.rs"
    );
    for local_error_construction in ["Rem6CliError::ReadConfig", "Rem6CliError::ParseConfig"] {
        assert!(
            !config.contains(local_error_construction),
            "src/config.rs must not locally construct `{local_error_construction}`"
        );
    }

    let parse_functions = rust_function_definition_names(&parse);
    assert!(
        !parse_functions.contains("required_value"),
        "src/config/parse.rs must not define its own required_value helper"
    );
    assert!(
        parse.contains("pub(super) use crate::cli_config::required_value;"),
        "src/config/parse.rs must re-export crate::cli_config::required_value"
    );
}
