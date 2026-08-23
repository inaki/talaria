# hermes-rust

**A native [ratatui](https://ratatui.rs) TUI for [Hermes Agent](https://github.com/NousResearch/hermes-agent).**

This is a *host*, not a fork. It speaks the same stdio JSON-RPC 2.0 protocol as `hermes --tui`, then renders chat, tools, approvals, and slash commands in a single Rust binary. Python still owns the agent: tools, memory, skills, models, and `~/.hermes`.

```
hermes-rust (this repo)          official Hermes
┌─────────────────────┐          ┌──────────────────────┐
│  ratatui TUI        │  NDJSON  │  python -m           │
│  keys · overlays    │◄────────►│  tui_gateway.entry   │
│  composer · splash  │  JSON-RPC│  AIAgent, tools,     │
└─────────────────────┘          │  ~/.hermes           │
                                 └──────────────────────┘
```

[![Rust](https://img.shields.io/badge/rust-2021-dea584?logo=rust)](https://www.rust-lang.org/)
[![ratatui](https://img.shields.io/badge/tui-ratatui-000?logo=ratatui)](https://ratatui.rs)
[![Hermes](https://img.shields.io/badge/hosts-Hermes%20Agent-FFD700)](https://github.com/NousResearch/hermes-agent)
[![status](https://img.shields.io/badge/status-PARITY.md-4caf50)](docs/PARITY.md)

Binary name is **`hermes-rust`**, never `hermes` — so you can keep the official CLI on PATH.

---

## Why

`hermes --tui` is TypeScript + Ink on top of Python `tui_gateway`. That split is already right: the UI is a client, the agent is a child. hermes-rust is the same client, in Rust:

- One native binary for the *UI* (Python still required for the agent)
- Immediate-mode redraw for streaming tokens, tool cards, and modals
- Offline `--mock=` scripts so the TUI is demo-able without an API key

What we **do not** do: reimplement Hermes, vendor `hermes-agent`, write `~/.hermes`, or ship as a drop-in `hermes` command.

---

## Quick start

**1. Hermes Agent** (if you do not already have `hermes --tui` working):

```bash
curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash
# then: hermes setup   # Nous Portal is the easy path
```

**2. This TUI**

```bash
git clone git@github.com:inaki/hermes-rust.git
cd hermes-rust
cargo run -- --theme gold
```

Discovery finds `~/.hermes/hermes-agent/venv/bin/python` even if `hermes` is not on PATH. Override with `HERMES_PYTHON`.

**Quit `hermes --tui` first.** Two gateways on one `HERMES_HOME` race the same SQLite. Sequential use is fine.

### Offline (no Python)

```bash
cargo test
cargo run -- --dev --mock=streaming --theme gold
cargo run -- --dev --mock=tools
cargo run -- --dev --mock=approval
```

### Protocol dump

```bash
cargo run --example dump_gateway                 # spawn → ready → create → close (no model)
cargo run --example dump_gateway -- --prompt hi  # calls your configured model
```

---

## What you get

| | |
|---|---|
| **Splash** | ANSI Shadow wordmark, Braille caduceus, live tools/skills from `session.info` |
| **Chat** | Streaming deltas, markdown on completed turns, thinking blocks |
| **Tools** | Collapsed cards; **Ctrl+O** expands args/result (secrets redacted) |
| **Approvals** | Approval / clarify / sudo / secret overlays |
| **Slash** | Catalog from the gateway; drop-up palette on `/` |
| **Sessions** | `/sessions` saved vs live; `/branch`; `/rewind` (row_id + `confirm_truncate`) |
| **Steer** | Enter while a turn is running sends `session.steer` |
| **Themes** | `github` · `gold` · `hermes` — `/theme`, `--theme`, `HERMES_RUST_THEME` |

Full matrix: **[docs/PARITY.md](docs/PARITY.md)** (status SSOT).

### Keys

| Key | |
|-----|---|
| **Enter** | Send (or steer if a turn is running) |
| **Shift+Enter** / **Alt+Enter** | Newline (box grows) |
| **/** | Command palette |
| **Esc** | Interrupt · dismiss overlay |
| **Ctrl+O** | Expand last tool card |
| **Ctrl+V** | Clipboard image (`clipboard.paste`) |
| **Ctrl+C** | Quit (confirm) |

Slash: `/help` `/sessions` `/resume` `/rewind` `/branch` `/agents` `/trees` `/model` `/skills` `/plugins` `/mcp` `/theme` `/clear` `/quit`.

---

## Layout

```
src/
  gateway.rs      spawn + JSON-RPC NDJSON
  session/        Live + Mock, same SessionEvent
  ui/screens/chat splash, transcript, overlays
  ui/widgets/     composer, slash menu, banner, markdown
  app/            ratatui loop (sync) + Tokio I/O
```

Library-first: `hermes_rust` is the crate, `src/main.rs` is clap + `run_tui`.

On disk this process only writes **`~/.hermes-rust/`** (logs, composer history, saved theme). `~/.hermes` belongs to Python.

---

## Docs

| | |
|---|---|
| **[PARITY.md](docs/PARITY.md)** | What we cover vs `hermes --tui` |
| [RUNNING.md](docs/RUNNING.md) | Mock vs live, themes, dumps |
| [docs/README.md](docs/README.md) | Index |

Official Hermes: [hermes-agent](https://github.com/NousResearch/hermes-agent) · [docs](https://hermes-agent.nousresearch.com/docs/) · [programmatic integration](https://hermes-agent.nousresearch.com/docs/developer-guide/programmatic-integration) (`tui_gateway`).

---

Built next to [Hermes Agent](https://github.com/NousResearch/hermes-agent) by [Nous Research](https://nousresearch.com). This repo is an independent ratatui host for that gateway.
