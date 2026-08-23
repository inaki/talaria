<img width="800" height="536" alt="CleanShot 2026-08-23 at 13 12 13" src="https://github.com/user-attachments/assets/a763fadb-53be-4eb5-b2f7-5f98a5e00f3b" />

# Talaria Agent

**Unofficial TUI host for [Hermes Agent](https://github.com/NousResearch/hermes-agent).** Native [ratatui](https://ratatui.rs), not a Nous Research product.

Talaria is a *host*, not a fork, and **not a Nous Research product**. It speaks the same stdio JSON-RPC 2.0 protocol as `hermes --tui`, then renders chat, tools, approvals, and slash commands in a single Rust binary. Python still owns the agent: tools, memory, skills, models, and `~/.hermes`.

See **[NOTICE.md](NOTICE.md)** for the relationship to Hermes Agent and Nous Research.

```
talaria (this repo)              official Hermes Agent
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
[![license](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![site](https://img.shields.io/badge/site-inaki.github.io%2Ftalaria-daa520)](https://inaki.github.io/talaria/)

Binary name is **`talaria`**, never `hermes` — so you can keep the official CLI on PATH. Page: **https://inaki.github.io/talaria/**

---

## Why

`hermes --tui` is TypeScript + Ink on top of Python `tui_gateway`. That split is already right: the UI is a client, the agent is a child. Talaria is the same kind of client, in Rust:

- One native binary for the *UI* (Python still required for the agent)
- Immediate-mode redraw for streaming tokens, tool cards, and modals
- Offline `--mock=` scripts so the TUI is demo-able without an API key

What we **do not** do: reimplement Hermes, vendor `hermes-agent`, write `~/.hermes`, ship as a drop-in `hermes` command, or present this as the official TUI.

---

## Quick start

**1. Hermes Agent** (if you do not already have `hermes --tui` working):

```bash
curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash
# then: hermes setup   # Nous Portal is the easy path
```

**2. This TUI** — both tools share `~/.hermes` (models, keys, sessions). This binary only writes `~/.talaria/`. Latest: **[v0.1.1](https://github.com/inaki/talaria/releases/tag/v0.1.1)**.

```bash
# Homebrew (macOS; builds from source)
brew install inaki/talaria/talaria

# curl (GitHub Releases: macOS Apple silicon/Intel, Linux x86_64/arm64)
curl -fsSL https://raw.githubusercontent.com/inaki/talaria/main/install.sh | bash
# pin:  curl … | bash -s -- v0.1.1
```

Then `talaria`. Discovery finds `~/.hermes/hermes-agent/venv/bin/python`. Override with `HERMES_PYTHON`.

**Quit the other TUI first.** `talaria` refuses to start if `hermes --tui` or another `tui_gateway` is already running (they race the same SQLite). Override with `--force` or `TALARIA_ALLOW_CONCURRENT=1`.

From source:

```bash
git clone git@github.com:inaki/talaria.git
cd talaria
cargo run -- --theme default
```

### Offline (no Python)

```bash
cargo test
cargo run -- --dev --mock=streaming --theme default
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
| **Splash** | ANSI Shadow `TALARIA - AGENT` wordmark, Braille caduceus, live tools/skills from `session.info` |
| **Chat** | Streaming deltas, markdown on completed turns, thinking blocks |
| **Tools** | Collapsed cards; **Ctrl+O** expands args/result (secrets redacted) |
| **Approvals** | Approval / clarify / sudo / secret overlays |
| **Slash** | Catalog from the gateway; drop-up palette on `/` |
| **Sessions** | `/sessions` saved vs live; `/branch`; `/rewind` (row_id + `confirm_truncate`) |
| **Steer** | Enter while a turn is running sends `session.steer` |
| **Skins** | `github` plus Hermes builtins (`default`, `ares`, `mono`, `slate`, `daylight`, `warm-lightmode`, `poseidon`, `sisyphus`, `charizard`) — `/skin` |

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

Slash: `/help` `/sessions` `/resume` `/rewind` `/branch` `/agents` `/usage` `/custom` `/trees` `/model` `/skills` `/plugins` `/mcp` `/skin` `/clear` `/quit`.

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

Library-first: `talaria` is the crate, `src/main.rs` is clap + `run_tui`. `publish = false` (crates.io already has an unrelated `talaria` crate).

On disk this process only writes **`~/.talaria/`** (logs, composer history, saved theme). A leftover `~/.hermes-rust/` is renamed once. `~/.hermes` belongs to Python.

---

## Docs

| | |
|---|---|
| **[Site](https://inaki.github.io/talaria/)** | GitHub Pages landing |
| **[NOTICE.md](NOTICE.md)** | Unofficial status, names, credits |
| **[PARITY.md](docs/PARITY.md)** | What we cover vs `hermes --tui` |
| [RUNNING.md](docs/RUNNING.md) | Mock vs live, themes, dumps |
| [docs/README.md](docs/README.md) | Index |

Official Hermes: [hermes-agent](https://github.com/NousResearch/hermes-agent) · [docs](https://hermes-agent.nousresearch.com/docs/) · [programmatic integration](https://hermes-agent.nousresearch.com/docs/developer-guide/programmatic-integration) (`tui_gateway`).

---

Talaria is independent of [Hermes Agent](https://github.com/NousResearch/hermes-agent) by [Nous Research](https://nousresearch.com). It hosts that gateway; it is not the official TUI.
