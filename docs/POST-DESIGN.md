# Post-design checks

What to verify **after** the chrome redesign (identity, empty states, tokens,
document, sheets) lands. Not a feature backlog and **not status SSOT** —
coverage stays in [PARITY.md](./PARITY.md). The plan is [REDESIGN.md](./REDESIGN.md).
Gallery: `cargo run --example gallery`.

Last updated: 2026-08-26.

Use this as a walkthrough, not a sprint board. Fail a row → fix before calling
the redesign done.

## Pre-design (already wired)

Do not treat these as open gaps. They landed before identity so the walkthrough
has something real to click.

| Job | Where |
|-----|--------|
| Resume on launch | `--continue` / `--resume` / `TALARIA_RESUME` |
| Ctrl+V text then image | Composer, then `clipboard.paste` |
| Enter steers; Ctrl+Enter queues | Flush on `message.complete` |
| `/new` vs `/clear` | Forge session vs wipe view |
| `spawn_tree.save` | Trees sheet, Ctrl+S |
| Sessions / trees / theme / custom / rewind / agents | **Sheet** |
| Model / skills / plugins / MCP | **Sheet** chrome around the existing pickers |
| Help / usage / tool result | **Inspector** |
| Approval / sudo / secret / quit / bang | **Modal** (`Overlay`) |
| Git branch on the meter | `session.info` `branch` / `git_branch` |
| Visual contract | `cargo run --example gallery` |

---

## How to run the walkthrough

1. `cargo test`
2. `cargo run --example gallery` — every sample, then `t` through skins
3. `cargo run -- --dev --mock=streaming` — identity + document
4. `cargo run -- --dev --mock=tools` — chips + inspector
5. `cargo run -- --dev --mock=approval` — danger modal
6. Live: `talaria` (fresh), `talaria --continue` (returning)

Check **80×24** and **~100×32**. Light and dark (gallery `t`).

---

## 1. Identity

| Check | Pass |
|-------|------|
| Wordmark is **TALARIA** only — no `- CLIENT`, no `- Agent` | |
| Tagline is “Native TUI host for Hermes Agent” | |
| No “Welcome to Hermes Agent!” | |
| Caduceus is a small **agent** mark, not the hero | |
| Splash does not say “Nous Research” in the hero | |
| NOTICE.md names still hold (unofficial, binary `talaria`) | |
| Default skin is the **site pair** (parchment / bronze-ink dark), not GitHub Dark | |
| GitHub + Hermes skins still selectable via `/skin` / palette | |
| Assistant card author is `⚕ Hermes`; **panel tokens are Talaria** | |
| Gallery **Wordmark** sample matches the live splash | |

---

## 2. Empty states

| Check | Pass |
|-------|------|
| **First run:** short wordmark, one sentence, model, `Ctrl+K` / `/` / `!`. No tool dump | |
| **Returning:** last three sessions, composer focused. `1–3` / Enter / click resume | |
| `--continue` / `--resume` / `TALARIA_RESUME` still resume | |
| `/new` forges a session; `/clear` only wipes the view | |
| Empty splash in gallery matches first-run, not the old FIGlet wall | |

---

## 3. Document

| Check | Pass |
|-------|------|
| No instructional lines in the transcript (“click or Ctrl+O…”) | |
| Roles: you / ⚕ Hermes / thinking / tool chip / `$` / system | |
| Tool click / Ctrl+O opens **inspector**, full body, `c` copies | |
| Silent 8/12/40 caps are gone from the inspector (`+N lines · ↓`) | |
| Enter while live **steers**; Ctrl+Enter **queues** | |
| `/focus` hides thinking/tools without deleting them (when it lands) | |
| Streaming does not pop layout when markdown appears | |

---

## 4. Objects (one chrome language)

Danger is **modal**. Management is **sheet**. Peek is **inspector**. Find is **palette**. Slash stays `/`.

| Surface | Must be | Must not be |
|---------|---------|-------------|
| Approval, sudo, secret, quit, rewind **confirm**, bang confirm | Modal | Sheet |
| Sessions, trees, model, skills, plugins, MCP, agents, rewind **picker**, theme | Sheet | Centered `paint_modal` |
| Tool result, thinking, usage, help | Inspector | Timeline rewrite |
| Host + catalog | Palette `Ctrl+K` | Extra overlay |

| Check | Pass |
|-------|------|
| Sheet docks right when wide; tall overlay when ~80 cols | |
| Sheet stays open while tokens stream | |
| Sheet has a search/filter | |
| Modal dims or clearly blocks; Esc cancels | |
| Palette: host actions first; type to mix Hermes catalog | |
| `/` slash menu still works | |
| Gallery has a sample for **each** object above | |

---

## 5. Meter and prefs

| Check | Pass |
|-------|------|
| Idle: model + context % (and git **branch** when `session.info` has it) | |
| Live: spinner + elapsed | |
| Title / cwd on the right, dropped first when narrow | |
| `/custom` toggles chips (at least meter + hints) | |
| YOLO / compress / bg counts only if a dump pinned `status.update` | |

---

## 6. Host jobs still work (regression)

Redesign must not break the pre-design wiring.

| Check | Pass |
|-------|------|
| Resume on launch | |
| Ctrl+V text then image | |
| Ctrl+Enter queue flushes on `message.complete` | |
| `spawn_tree.save` (Ctrl+S on trees sheet) | |
| `/new` vs `/clear` | |
| Approvals / clarify / sudo / secret | |
| Rewind still `{confirm_truncate, row_id}` only on confirm | |
| Ordinary submit is still `{session_id, text}` | |
| Dual-run lock / `~/.talaria` isolation | |
| Mock `--dev --mock=*` still demoable | |

---

## 7. Gallery contract

`cargo run --example gallery` is the visual contract. If a chrome piece is
not a sample, it is not done.

| Check | Pass |
|-------|------|
| Every Sample title unique | |
| `t` cycles all named skins without punching holes in the canvas | |
| Enter “live” works for composer, slash, palette, sheets, inspectors | |
| New sheet/inspector kinds get a Sample in the same PR | |

---

## 8. Out of redesign (do not fail the walkthrough)

Leave these. They are not post-design bugs.

- WebSocket / dashboard attach
- Voice record + TTS
- In-transcript PNG
- Shipping as `hermes`
- Pixel Ink `/agents` tree, kaomoji, LaTeX (unless it already landed)
- Parsing `~/.hermes` as Talaria chrome

---

## Sign-off

Redesign is done when:

1. This file’s sections 1–7 are checked on 80-col and ~100-col, light and dark.
2. [PARITY.md](./PARITY.md) still describes **jobs**, not Ink widgets.
3. `cargo test` is green.
4. Gallery and `--dev --mock=streaming` look like the same product as the site.

Then Phase G polish (theme-by-value, `ARCHITECTURE.md`) can follow.
