# Parity vs `hermes --tui`

**Status SSOT for what this host covers.** Architecture discussion stays in [PLAN.md](./PLAN.md). Never create `MIGRATION.md`.

Last updated: 2026-08-23.

**Live dump (2026-08-22):** `cargo run --example dump_gateway` against
`~/.hermes/hermes-agent/venv/bin/python` — `gateway.ready` + `session.create`
(`session_id`, `stored_session_id`, `info.lazy: true`, `info.model`) + shutdown.
`hermes` may not be on PATH until the installer finishes linking `~/.local/bin/hermes`.

| Surface | talaria | Notes |
|---------|-------------|-------|
| Spawn `tui_gateway` over stdio JSON-RPC | Yes | `-u` + `PYTHONUNBUFFERED=1`; import-probe discovery |
| Streaming chat | Yes | `message.delta` / `complete` |
| Markdown in completed assistant turns | Yes | pulldown-cmark; streaming stays plain text |
| Skin | Yes | `/skin` picker: `github` plus Hermes builtins (`default`, `ares`, `mono`, `slate`, `daylight`, `warm-lightmode`, `poseidon`, `sisyphus`, `charizard`). Alias `/theme`. Saved to `~/.talaria/theme` |
| Opening splash | Yes | Empty transcript shows ANSI Shadow `TALARIA - CLIENT` wordmark + Braille caduceus. Panel lists **Talaria Client v…** and **Hermes Agent v…** on separate lines |
| Composer history | Yes | `~/.talaria/history` (0600, last 200), not `~/.hermes` |
| Thinking spinner | Yes | Live dim transcript block + status; sealed on first `message.delta` |
| Tools | Yes | Collapsed one-liner; **click** or **Ctrl+O** expands a card (Ctrl+O uses the selected card, else the last); secrets redacted |
| Approvals / clarify / sudo / secret | Yes | Overlays; expire only sudo/secret by `request_id` |
| Slash catalog | Yes | `commands.catalog` + `command.dispatch` |
| Model picker | Yes | Native `/model` overlay (`model.options` / `model.save_key` / `model.disconnect` / `config.set`). Bare `/model` opens the picker; `/model <id>` hot-swaps. API-key providers can be added in-place |
| Skills / plugins / MCP | Yes | Native `/skills`, `/plugins`, `/mcp` overlays (`skills.manage`, `plugins.manage`, `mcp.servers.*`). Add/remove/toggle stay in this TUI |
| Saved / live sessions | Yes | `session.list`+`resume` vs `active_list`+`activate`. **Ctrl+N** new session (`session.create`); **Ctrl+D** closes selected live (`session.close`) |
| Session branch | Yes | `/branch` → `session.branch` |
| Steer mid-turn | Yes | Enter while streaming → `session.steer` |
| Bang shell | Yes | `! pwd` / `!ls` runs locally in session cwd after a confirm overlay (shows cwd); not sent to the model. 30s timeout kills the process group. Output is secret-scrubbed |
| Subagents | Yes | Timeline + `/agents` overlay; Enter interrupts; **s** steers (`subagent.steer`) |
| Usage | Yes | Native `/usage` overlay via `session.usage` |
| Composer meter | Yes | Strip above the prompt: model, context fill, session / last-turn / idle clocks. `/custom` toggles the meter and the key-hint row (saved in `~/.talaria/custom`) |
| Copy | Yes | Drag-select transcript copies on mouse-up (OSC 52 + pbcopy / wl-copy / xclip + `~/.talaria/last-copy.txt`). `/copy` last assistant response; `/copy N` and `/copy [N] file` like Grok. `TALARIA_COPY_FILE` overrides the backup. Shift+drag still uses the terminal's native selection. |
| Image attach | Partial | Path paste + **Ctrl+V** `clipboard.paste` (gateway reads OS clipboard) |
| Rewind / edit | Yes | `/rewind` lists `session.history` user turns with `row_id`; confirm → `prompt.submit` with `confirm_truncate` + `truncate_before_row_id` (+ `confirm_empty_truncate` on the first turn). Ordinary send stays `{session_id, text}` only |
| Spawn-tree dashboard | Partial | `/trees` lists `spawn_tree.list`; Enter → `spawn_tree.load` |
| WebSocket attach / sidecar | No | Stdio only |
| Packaging as `hermes` | No | Binary is `talaria`. Unofficial host — see [NOTICE.md](../NOTICE.md) |
| Update notice | Yes | Live start checks GitHub Releases (24h cache). Splash + `/help` show `Talaria X is out · brew upgrade / curl \| bash`. No auto-upgrade. `TALARIA_NO_UPDATE_CHECK=1` skips |

How to run: [RUNNING.md](./RUNNING.md).
