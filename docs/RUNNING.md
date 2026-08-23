# Running hermes-rust

## Offline (no Hermes)

```bash
cargo test
cargo run -- --dev --mock=streaming
cargo run -- --dev --mock=tools
cargo run -- --dev --mock=approval
cargo run -- --dev --mock=subagent
cargo run -- --dev --mock=error
```

## Live gateway

Needs a Python that can `import tui_gateway`. After the official installer that is
`~/.hermes/hermes-agent/venv/bin/python` (discovery finds it even before `hermes`
is on PATH). The `hermes` command itself is usually `~/.local/bin/hermes` once
the installer finishes.

**Quit `hermes --tui` first.** Two gateways on one `HERMES_HOME` is unsupported.

```bash
# optional: HERMES_PYTHON=/path/to/python
cargo run --example dump_gateway          # no model
cargo run --example dump_gateway -- --prompt "Say hi."   # calls your model
cargo run                                 # TUI
```

Logs: `~/.hermes-rust/logs/hermes-rust.log` (always on, 0600). Never writes `~/.hermes`.

Themes: `github` (default), `gold` (bronze/navy), `hermes` (brand blue). `/theme` in the TUI, or:

```bash
cargo run -- --theme gold
HERMES_RUST_THEME=hermes cargo run -- --dev --mock=streaming
```

Choice is saved to `~/.hermes-rust/theme`.

`/rewind` regenerates from a past user turn. It only offers turns that have a durable `row_id` (from `session.history` / resume). Confirming sends `confirm_truncate` + `truncate_before_row_id` — ordinary Enter never truncates.
