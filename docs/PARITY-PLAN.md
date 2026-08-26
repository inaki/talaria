# Parity tracks vs `hermes --tui`

Sequencing for remaining gaps. Status table: [PARITY.md](./PARITY.md).

**New work follows [REDESIGN.md](./REDESIGN.md)** (Talaria objects, not Ink
screens). This file is the historical leftover-track list vs `hermes --tui`.

**Baseline:** v1 host live against official `tui_gateway` (commit after first dump). Binary is `talaria`.

## Track A — Transcript fidelity (landed)

| Item | Exit |
|------|------|
| Thinking / reasoning as a transcript block | `thinking.delta` appends a live dim block; first `message.delta` seals it |
| Expandable tool cards | Collapsed = one summary line; **Ctrl+O** toggles the last card (args + result) |
| Clipboard image | **Ctrl+V** → gateway `clipboard.paste` (Hermes reads the OS clipboard) |

## Track B — Agent trees (landed; subagent steer later)

| Item | Exit |
|------|------|
| Spawn-tree picker | `/trees` → `spawn_tree.list`; Enter loads via `spawn_tree.load` if the dump shows a path |
| Subagent steer | **Landed.** `/agents` **s** → `subagent.steer` |

## Track C — History edit (this slice)

| Item | Exit |
|------|------|
| Rewind / regenerate | **Landed.** `/rewind` picker from `session.history` `row_id`. Submit is `confirm_truncate` + `truncate_before_row_id` (and `confirm_empty_truncate` when cutting the first user turn). Rebind cached ids from `survivor_user_row_ids`. Never ordinal-only. Ordinary composer submit stays `{session_id, text}`. |

## Track D — Cutover (not now)

WebSocket attach, shipping as `hermes`. Still out — see [REDESIGN.md](./REDESIGN.md) non-goals. Homebrew already ships.

Update **PARITY.md** when a *capability* lands. Do not add `MIGRATION.md`.
Do not add Ink widget rows to PARITY because Ink grew a new overlay.
