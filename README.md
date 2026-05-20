# rem6: Rust Emulator 6

A Rust-reimplementation of gem5

## Workspace

rem6 is organized as one Rust workspace with one crate per simulator subsystem.
The initial subsystem crate is `crates/rem6-kernel`, which owns deterministic
simulation time and event scheduling primitives.

Run the current verification suite with:

```bash
cargo test --workspace
```

Code under `temp/` is scratch/reference material only. It is not part of the
workspace and must not be used as a build dependency.
