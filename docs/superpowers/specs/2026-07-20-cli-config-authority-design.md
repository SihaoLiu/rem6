# CLI Config Authority Design

## Context

Top-level CLI configuration mechanics are currently reimplemented across the
`run`, `gups`, `trace-replay`, `gpu-run`, `accelerator-run`, `multi-run`, and
`resource-acquire` commands.

- five scanner implementations search arguments for `--config`;
- five loaders repeat the same file-read and TOML-parse error mapping;
- six modules define the same `required_value` helper;
- seven private file-config types repeat relative-path resolution; and
- `config/file_scan.rs` owns the three core-command scanner wrappers while
  auxiliary commands retain independent loops.

The duplication is behaviorally risky because prescanning has a subtle rule:
a value-taking flag whose value is literally `--config` must suppress that
token as a config selector. The `run`, `gups`, and `trace-replay` scanners use
wildcard long-option behavior so new value-taking flags are automatically
protected. GPU, accelerator, multi-run, and resource-acquire use explicit
value and boolean vocabularies. Flattening those modes would change existing
error and precedence behavior.

`config.rs` is also at 1,698 lines under a strict `< 1,700` source-policy
ratchet. Moving shared mechanics out creates headroom without relocating
command-specific parsing.

## Ledger Boundary

This is a production cleanup of existing CLI configuration behavior. It adds
no simulator capability, compatibility counter, execution mode, or external
integration evidence. `docs/architecture/gem5-to-rem6-migration.md` remains
unchanged and exactly 1,200 lines.

## Approaches

### Retain local helpers and add cross-command tests

Additional tests could detect scanner drift, but every command would still
own its own loop, read/parse mapping, value helper, and path resolver. Adding a
new command or flag would continue to require copying mechanics.

### Put shared helpers inside `config.rs`

The core config module already owns run, GUPS, and trace-replay types, but GPU,
accelerator, multi-run, resource-acquire, and power-import are sibling modules.
Making `config.rs` their utility facade would mix command-neutral mechanics
into a file already at its line cap and force sibling commands through a
domain-specific owner.

### Add a top-level `cli_config.rs` authority

This is the selected design. A private top-level module owns command-neutral
config prescanning, TOML loading, required-value errors, and relative-path
resolution. It also owns the seven command prescan profiles so scanner
vocabulary and unknown-flag policy are auditable in one place. Command modules
retain typed root-table unwrapping, `config_dir` assignment, validation,
override precedence, and list-replacement semantics.

## Prescan Authority

`cli_config.rs` defines a private `ConfigPrescanProfile` with:

- known value-taking flags;
- known boolean flags; and
- unknown-long-option behavior.

The unknown behavior has two modes:

- wildcard mode consumes the following token for any unclassified `--flag`
  when one exists; and
- explicit mode advances only past the unknown flag itself.

The shared scanner preserves these rules:

- recognized `--config` requires a following value;
- the last recognized config occurrence wins;
- known value flags always consume the following token, including the literal
  string `--config`;
- known boolean flags consume no following token;
- wildcard `run`, `gups`, and `trace-replay` profiles preserve broad protection
  for newly added value-taking long options;
- explicit GPU, accelerator, multi-run, and resource-acquire profiles preserve
  their current one-token handling for unknown flags; and
- positional tokens are ignored by the prescan and remain available to the
  full parser.

The module exposes command-named wrappers matching the current call sites:

- `run_file_config_from_args`;
- `gups_file_config_from_args`;
- `trace_replay_file_config_from_args`;
- `gpu_run_file_config_from_args`;
- `accelerator_run_file_config_from_args`;
- `multi_run_file_config_from_args`; and
- `resource_acquire_file_config_from_args`.

The wrappers keep the generic profile and scanner private. Delete
`config/file_scan.rs` and the four auxiliary scanner loops.

## Shared Mechanics

`read_toml_config<T: DeserializeOwned>` is the only generic config file reader.
It preserves the existing error variants and payloads:

- read failures return `Rem6CliError::ReadConfig` with the original path and
  operating-system error string; and
- TOML failures return `Rem6CliError::ParseConfig` with the same path and TOML
  parser string.

Typed command loaders remain local. Each calls `read_toml_config`, unwraps its
private root table, applies the current default, and assigns
`config_dir = path.parent()` exactly as before. `power-import` file reading is
excluded because it intentionally maps failures through the distinct
`PowerAnalysis` error surface.

`required_value` becomes the only helper that maps an absent next token to
`Rem6CliError::MissingFlagValue`. `config/parse.rs` re-exports it within the
config subtree; GPU, accelerator, multi-run, resource-acquire, and
power-import import it directly. Delete all local definitions.

`resolve_config_path` becomes the only relative-path resolver. Existing private
`resolve_path` methods remain on their file-config types but delegate to the
shared helper. Absolute paths remain unchanged; relative paths join
`config_dir` when present and otherwise remain relative.

## Command Boundaries

This cleanup does not centralize command semantics.

- private TOML structs and root-table names remain with their commands;
- CLI values continue to override file values;
- list replacement versus append behavior remains unchanged;
- validation and error wording after prescan remain unchanged;
- multi-run child command synthesis remains unchanged;
- resource-acquire archive, URI, and member-path handling remains unchanged;
- power-import retains its distinct file-read error mapping; and
- no helper becomes part of the public `rem6` Rust API.

## Compatibility Boundary

The refactor preserves:

- all command names, flags, defaults, and required fields;
- last-`--config` precedence;
- missing config-value errors;
- literal `--config` values for value-taking flags;
- wildcard versus explicit unknown-flag prescan behavior;
- TOML read and parse error variants, paths, and messages;
- relative and absolute config path resolution;
- CLI-over-file precedence and list semantics;
- artifact paths, JSON schemas, stats output, and child command behavior; and
- source-policy limits for `lib.rs`, `config.rs`, and all source files.

## Source Policy

A focused source-policy module enforces the final authority shape:

- `lib.rs` declares `mod cli_config;`;
- `cli_config.rs` owns all seven command prescan wrappers plus the shared
  scanner, loader, value helper, and resolver;
- `config/file_scan.rs` is deleted;
- no production file outside `cli_config.rs` defines
  `config_path_from_args`, `required_value`, or `resolve_config_path`;
- config consumers import the shared authority and no longer construct
  `ReadConfig` or `ParseConfig` locally;
- `config/parse.rs` re-exports the shared value helper;
- command-local `resolve_path` methods delegate to the shared resolver;
- `cli_config.rs` stays within a focused line budget; and
- `config.rs` remains below 1,700 lines while `lib.rs` and the source-policy
  driver remain under their existing caps.

The source-policy row is observed failing before implementation.

## Test Matrix

Focused `cli_config` unit tests cover:

- last-recognized-config precedence;
- missing config values;
- literal `--config` suppression behind known value flags;
- boolean flags followed immediately by a real config selector;
- wildcard unknown-long-option consumption;
- explicit unknown-flag non-consumption;
- all seven command wrappers' representative value/boolean profiles;
- required-value success and failure;
- relative, absent-base, and absolute path resolution; and
- generic TOML read success, read failure, and parse failure.

Representative real CLI rows cover:

- run relative binary resolution and wildcard value suppression;
- trace-replay relative trace resolution and fabric/resource value suppression;
- GPU TOML loading plus output/power/NoMali literal-config suppression;
- accelerator TOML loading and artifact paths;
- multi-run config orchestration and child command config synthesis;
- resource-acquire relative local artifacts; and
- malformed or missing config errors.

The negative/suppression rows are required because a broad successful config
load alone does not prove the prescan skipped a literal `--config` value.

## Files

- `crates/rem6/src/cli_config.rs`: add the shared authority and focused unit
  tests.
- `crates/rem6/src/lib.rs`: declare the private module.
- `crates/rem6/src/config/file_scan.rs`: delete.
- `crates/rem6/src/config.rs` and `crates/rem6/src/config/trace_replay.rs`:
  import command wrappers, generic loading, and path resolution.
- `crates/rem6/src/config/parse.rs`: re-export the shared value helper.
- `crates/rem6/src/gpu_cli.rs`, `accelerator_cli.rs`, `multi_run_cli.rs`, and
  `resource_acquire_config.rs`: delete local scanners/load mechanics/value
  helpers and delegate path resolution.
- `crates/rem6/src/power_import_cli.rs`: consume only the shared value helper.
- `crates/rem6/tests/source_policy.rs` and
  `crates/rem6/tests/source_policy/cli_config_authority.rs`: enforce ownership.
- existing focused CLI test files: strengthen only gaps needed to prove
  command-profile suppression and compatibility.

## Verification

Verification includes observed RED/GREEN source policy, focused `cli_config`
unit tests, command parser and CLI config/suppression rows, all top-level rem6
targets, formatting, diff and protected-path checks, exact 1,200-line ledger
verification, the full workspace, and an independent high-intensity read-only
review before push.
