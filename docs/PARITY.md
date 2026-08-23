# Parity vs `hermes --tui`

**Status SSOT for what this host covers.** Architecture discussion stays in [PLAN.md](./PLAN.md). Never create `MIGRATION.md`.

Last updated: 2026-08-22.

**Live dump (2026-08-22):** `cargo run --example dump_gateway` against
`~/.hermes/hermes-agent/venv/bin/python` — `gateway.ready` + `session.create`
(`session_id`, `stored_session_id`, `info.lazy: true`, `info.model`) + shutdown.
`hermes` may not be on PATH until the installer finishes linking `~/.local/bin/hermes`.

| Surface | hermes-rust | Notes |
|---------|-------------|-------|
| Spawn `tui_gateway` over stdio JSON-RPC | Yes | `-u` + `PYTHONUNBUFFERED=1`; import-probe discovery |
| Streaming chat | Yes | `message.delta` / `complete` |
| Markdown in completed assistant turns | Yes | pulldown-cmark; streaming stays plain text |
| Theme | Partial | Default dark palette; `gateway.ready` `payload.skin` overlay when hex colors are present |
| Composer history | Yes | `~/.hermes-rust/history` (0600, last 200), not `~/.hermes` |
| Thinking spinner | Partial | `thinking.delta` / `reasoning.delta` → status + spinner |
| Tools | Partial | Card with args/preview/result; secrets redacted; no verbose accordion |
| Approvals / clarify / sudo / secret | Yes | Overlays; expire only sudo/secret by `request_id` |
| Slash catalog | Yes | `commands.catalog` + `command.dispatch` |
| Saved / live sessions | Yes | `session.list`+`resume` vs `active_list`+`activate` |
| Session branch | Yes | `/branch` → `session.branch` |
| Steer mid-turn | Yes | Enter while streaming → `session.steer` |
| Subagents | Partial | Timeline + `/agents` overlay; interrupt selected child |
| Image attach | Partial | Paste a filesystem path (`image.attach`); no clipboard bytes |
| Rewind / edit | No | Safety rule in PLAN: ordinary submit never truncates |
| Spawn-tree dashboard | No | |
| WebSocket attach / sidecar | No | Stdio only |
| Packaging as `hermes` | No | Binary is `hermes-rust` |

How to run: [RUNNING.md](./RUNNING.md).
