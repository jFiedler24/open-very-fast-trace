# OVFT for Zed

Language-server integration for [open-very-fast-trace](https://github.com/jFiedler24/open-very-fast-trace).

Provides, for Markdown specs and Rust sources:

- diagnostics for uncovered items, orphaned / outdated coverage links
- hover on requirement IDs (title, coverage status, links)
- go to definition (tag → requirement, requirement → covering items)

## Setup

1. `cargo install cargo-ovft`
2. Install this extension (`zed: install dev extension` → select the `ovft-zed` directory).

Override the binary in Zed settings if needed:

```json
{ "lsp": { "ovft": { "binary": { "path": "/path/to/cargo-ovft", "arguments": ["ovft", "--lsp"] } } } }
```

Not available in Zed (no extension API): HTML report panel, tree view, tag decorations.
Use `cargo ovft ovft` for reports.

## Testing

- Protocol level (no editor needed): `cargo build -p cargo-ovft && python3 cargo-ovft/tests/lsp_smoke.py target/debug/cargo-ovft`
- Extension build: `cd ovft-zed && cargo build --release --target wasm32-wasip2` (needs `rustup target add wasm32-wasip2`)
- In Zed: `cargo install --path cargo-ovft`, run `zed: install dev extension` and pick `ovft-zed`, then open a project with an OVFT spec. Check `zed: open log` for "OVFT Language Server".
