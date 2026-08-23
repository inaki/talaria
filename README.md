# hermes-rust

Native **ratatui** host for official Hermes `tui_gateway` (stdio JSON-RPC).
This is not a reimplementation of the agent. Python still owns tools, memory, skills, and `~/.hermes`.

**Status:** [docs/PARITY.md](docs/PARITY.md). Architecture notes (historical): [docs/PLAN.md](docs/PLAN.md).

## Run

```bash
# Offline TUI (no Hermes install needed)
cargo run -- --dev --mock=streaming

# Other mock scripts
cargo run -- --dev --mock=tools
cargo run -- --dev --mock=approval
cargo run -- --dev --mock=error
cargo run -- --dev --mock=subagent

# Live gateway (needs `python -c "import tui_gateway"`)
cargo run

# Protocol dump — no model call
cargo run --example dump_gateway

# Protocol dump — calls your configured model
cargo run --example dump_gateway -- --prompt "Say hello in one short sentence."
```

Binary name is **`hermes-rust`**, never `hermes`. Sequential dual-run with `hermes --tui` is supported; two gateways at once on the same `HERMES_HOME` is not.

Logs: `~/.hermes-rust/logs/hermes-rust.log` (always on).

Keys: **Enter** send · **/** commands · **Esc** interrupt (dismisses overlays first) · **Ctrl+C** quit.

`--mock=approval` opens the approval modal (1/2/3 or Enter; Esc denies). `/sessions` or `/resume` opens the saved/live picker. `/help` keys. `/agents` subagent list. Enter during a turn **steers** instead of starting a new prompt. Completed replies render markdown.
