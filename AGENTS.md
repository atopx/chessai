# AGENTS.md — chessai

## Project

Xiangqi (Chinese Chess) AI engine. Single Rust crate, no workspace.

## Build & test

```sh
cargo build --release        # fat LTO, single codegen unit, panic=abort
cargo test                   # 65 unit tests + 1 doc-test, no special setup
cargo clippy                 # passes clean
```

## Formatting

Project uses nightly-only rustfmt options (`fn_single_line`, `imports_granularity = Item`, `group_imports = StdExternalCrate`). `cargo fmt --check` on stable will report false diffs. Format with nightly:

```sh
cargo +nightly fmt
```

## Key conventions

- **Edition 2024** (not 2021).
- All modules declared `pub(crate)`; public API is exactly what `lib.rs` re-exports.
- Only external dependency: `thiserror = "2"`.
- `assets/BOOK.DAT` is embedded at compile time — do not delete it.

## Architecture

- `engine.rs` — public facade (`Engine`, `EngineBuilder`), owns position + TT + book
- `position.rs` — board state, make/undo, incremental Zobrist + material + PSQ
- `search.rs` — alpha-beta, PVS, NMP, LMR, QS, Lazy SMP
- `magic.rs` — pre-computed attack tables for rook/cannon (rank + file lookup)
- `attacks.rs` — knight, advisor, bishop, king, pawn attack tables
- `movegen.rs` / `picker.rs` — pseudo-legal generation + staged move picking (TT → captures → killers → countermove → history)
- `see.rs` — static exchange evaluation
- `eval.rs` — material + PST incremental eval
- `tt.rs` — lockless transposition table (Zobrist key + 32-bit lock XOR verification)

## Examples

- `cargo run --release --example undo_demo` — make/undo round-trip sanity check
- `cargo run --release --example vs_pikafish` — plays against Pikafish via UCI. Requires `pikafish/pikafish` and `pikafish/pikafish.nnue` (gitignored, not in repo)
