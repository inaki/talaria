# Parity tracks vs `hermes --tui`

Sequencing for remaining gaps. Status table: [PARITY.md](./PARITY.md).

**Baseline:** v1 host live against official `tui_gateway` (commit after first dump). Binary stays `hermes-rust`.

## Track A — Transcript fidelity (this slice)

| Item | Exit |
|------|------|
| Thinking / reasoning as a transcript block | `thinking.delta` appends a live dim block; first `message.delta` seals it |
| Expandable tool cards | Collapsed = one summary line; **Ctrl+O** toggles the last card (args + result) |
| Clipboard image | **Ctrl+V** → gateway `clipboard.paste` (Hermes reads the OS clipboard) |

## Track B — Agent trees

| Item | Exit |
|------|------|
| Spawn-tree picker | `/trees` → `spawn_tree.list`; Enter loads via `spawn_tree.load` if the dump shows a path |
| Subagent steer | later (`subagent.steer`) |

## Track C — History edit

| Item | Exit |
|------|------|
| Rewind / regenerate | Only with `confirm_truncate` + `truncate_before_row_id` from resume `row_id`. Not ordinal-only. |

## Track D — Cutover (not now)

WebSocket attach, shipping as `hermes`, Homebrew.

Update **PARITY.md** when a track lands. Do not add `MIGRATION.md`.
