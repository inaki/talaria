# Talaria redesign — better host, not faster clone

Implementation plan for Talaria chrome, information architecture, and
capability coverage. **Not status SSOT** — that stays [PARITY.md](./PARITY.md).
Architecture history stays in [PLAN.md](./PLAN.md). Designer brief: [handout.html](./handout.html). Live chrome:
`cargo run --example gallery`. Scratch discussion:
`journal/design-audit.md`, `journal/better-host.md` (gitignored).

**Never create `MIGRATION.md`.** This is a greenfield host redesign, not a
port of Ink TSX.

Last updated: 2026-08-26.

---

## North star

**Capability parity with the agent. Interaction parity with a good editor.
Visual parity with the Talaria site — not with Ink.**

Talaria is a *host*. Hermes Agent is the *speaker*. We take every Hermes TUI
**job** that the current crate + `tui_gateway` can support, and we put it in
a Talaria object (document, sheet, inspector, modal, palette, meter chip).
We do **not** clone Ink screens.

```
Talaria is the frame.
Hermes is the speaker.
The transcript is a document.
Management is sheets.
Danger is modals.
Slash is power.
The palette is discovery.
```

---

## Goals

1. First paint teaches the user what Talaria is (a native host), not that
   this is another Hermes TUI.
2. Cover every Hermes TUI *capability* that fits the current architecture
   (stdio `tui_gateway`, no audio stack, no second transport).
3. Put those capabilities in a design that is more usable and more
   beautiful than Ink: quieter chrome, searchable sheets, a command
   palette, a document transcript, a site-aligned visual system.
4. Add Talaria-own features that Ink cannot match without becoming a
   different product (palette, non-blocking sheets, inspectors for full
   tool output, returning workspace, chip-level meter, host prefs).
5. Keep the host/agent split, protocol caution, Esc/Ctrl+C map, dual-run
   contract, and `~/.talaria` isolation.

## Non-goals

- Pixel-perfect Ink. `docs/PLAN.md` already rejected this.
- Growing `Overlay` by one variant per Hermes slash command.
- WebSocket attach / dashboard Chat tab (`HERMES_TUI_GATEWAY_URL` is
  internal to Hermes).
- Shipping the binary as `hermes`. See [NOTICE.md](../NOTICE.md).
- Voice record + TTS, in-transcript image rendering (sixel/Kitty), billing
  checkout, parsing `~/.hermes` as a parallel config schema.
- Treating [PARITY.md](./PARITY.md) Yes/Partial/No as a UI sprint board.

---

## Design objects

Keep **one** `Screen` (chat). Change the kinds of UI on it.

| Object | When | Examples |
|--------|------|----------|
| **Document** | The transcript. No instructional text. | you / ⚕ Hermes / thinking / tool chip / `$` shell / system |
| **Modal** | Blocking; dim the rest. Esc cancels. | approval, sudo, secret, quit, rewind confirm, bang confirm |
| **Sheet** | Management; search; may stay open while a turn streams | sessions, model, skills, plugins, MCP, agents, trees |
| **Inspector** | Peek at one thing; copy/full text; not a rewrite of the timeline | tool result, thinking, usage/context, help |
| **Palette** | Fuzzy find of *actions* (`Ctrl+K`) | New session, Resume, Rewind, Model, Focus, Queue, Edit in editor |
| **Slash** | Hermes catalog (`/`) | `/compress`, `/yolo`, `/personality`, skills, anything we do not host |
| **Meter** | Progressive chips, not a status novel | idle: model + context %; live: spinner + elapsed; optional: title, branch, YOLO, compress, cwd |
| **Prefs** | Host comfort, saved under `~/.talaria/` | `/custom` chips, `/focus`, `/mouse`, `/indicator`, default skin |

---

## Capability map (Ink job → Talaria object)

Absorb the **job**. Reject the **screen**. Slash-only means `command.dispatch`
already runs it; we do not build a shrine. Out means not this redesign.

### Absorb (must land; current crate already has the plumbing)

| Hermes TUI job | Talaria object | Notes |
|----------------|----------------|-------|
| Resume last / by id (`-c`, `-r`, `HERMES_TUI_RESUME`) | CLI + returning empty state | `session.list` + `session.resume` already exist |
| New live session | Sessions **sheet** + palette | `session.create` / `Ctrl+N` exist; add `+new` draft |
| Close live session | Sessions sheet | `Ctrl+D` exists |
| Follow-up while busy | Palette / `Ctrl+Enter` **queue** | Enter stays **steer**. Two verbs |
| Ctrl+V text then image | Composer then `clipboard.paste` | Today Ctrl+V always hits the gateway |
| Path image attach | Composer paste → `image.attach` | Already |
| Spawn-tree save | Trees **sheet** action | `spawn_tree.list` / `.load` exist; add `.save` |
| Git branch, session title | Meter chips | Already on `session.info`; `apply_session_info` ignores `branch` |
| `skin.changed` | Apply Talaria tokens | Today `Unhandled` |
| `status.update` | Meter chips / toast | Dump-pin YOLO, compress count, bg tasks |
| `background.complete` | Toast + optional inspector | Today dropped |

### Redesign (same job, new object)

| Hermes TUI job | Talaria object |
|----------------|----------------|
| Collapsible splash inventories | First-run copy is short. Tools/skills/MCP are a **sheet** (`/tools`), not home |
| `/details` accordion | `/focus` + collapsed chips + inspector. No chevrons in the document |
| `/agents` tree, kill/steer | Agents **sheet** (search, interrupt, steer). Rollups only if `delegation.status` has them |
| `/usage` + `/context` glyph grid | One **inspector**: existing bar + category breakdown if the payload has it |
| `/sessions` Ctrl+X orchestrator | Sessions **sheet** (saved / live tabs we already have) |
| `$EDITOR` / `/prompt` | Palette “Edit in editor” (suspend alt-screen) |
| `/reload` | Palette → `reload.env` / `reload.mcp` |
| `/title` | Palette / sheet; `session.title` if we dump it |
| `/compress` | Slash + meter chip after `status.update` |
| `/yolo` | Slash + meter chip. No fake badge without a dump |
| `/background` | Slash + `background.complete` toast |
| `/queue` | Native queue verb (above), plus slash |
| `/steer` | Enter while live (keep) |
| Light-terminal detect | Auto-pick **Talaria** light/dark pair, not `HERMES_TUI_THEME` |
| LaTeX | `markdown.rs`, always-on, quiet |
| Paste-collapse | `TextComposer` |
| `/mouse` presets | `/custom` prefs (we already always capture mouse) |
| `/indicator` | `/custom` spinner set. Geometric default; optional playfulness is *Talaria*, not Hermes kaomoji |
| `/terminal-setup` | Palette action writing editor keybinding JSON |
| `/journey`, `/personality`, `/moa`, `/voice` toggle, `/review`, `/goal`, … | **Slash** until a structured payload exists. Palette can search the catalog name |
| Custom Hermes skins | Agent skins, secondary. `apply_skin` / `skin.changed` may tint; they do not become the brand default |

### Out of this redesign

| Item | Why |
|------|-----|
| WebSocket / dashboard attach | New transport; official docs mark it internal |
| Binary named `hermes` | [NOTICE.md](../NOTICE.md) |
| Voice record + TTS | No audio stack |
| Billing step-up UI | Dump + payment overlay is a different product |
| Inline PNG in the transcript | Attach is enough; sixel/Kitty is a renderer project |
| Full Hermes `display.skin` YAML | We do not parse `~/.hermes` as chrome |
| Concurrent two-gateway on one `HERMES_HOME` | Still unsupported |

Update [PARITY.md](./PARITY.md) when an absorb/redesign job lands as *capability*.
Do not add Ink widget names as rows.

---

## Transcript system

One typographic language. Chrome stays in chrome.

| Role | Mark | Treatment |
|------|------|-----------|
| User | quiet prefix (`you`) | No card, or a light right-weighted block |
| Hermes | `⚕` | Card; fill only while streaming. Author is Hermes; **panel tokens are Talaria** |
| Thinking | dim | Collapsible; last N + inspector “show all” |
| Tool | chip | One line. Click / Ctrl+O opens **inspector** (full args/result, copy). No “click or Ctrl+O” line in the document |
| Shell | `$` | Same chip language as tools |
| System | dim, no card | Notices, errors, steer/queue acks |

Truncation is an affordance (`+36 lines · Enter`), never silent 8/12/40 cuts
in the inspector. The timeline may still summarize.

---

## Empty state

**First run** (no saved sessions, or `--dev`): short **TALARIA** wordmark, one
sentence (“Native TUI host for Hermes Agent”), model, hint toward `/` and
`Ctrl+K`. No tool dump. No caduceus hero. No “Welcome to Hermes Agent!”

**Returning:** last three sessions (title + preview), current model +
context, composer focused. Resume is Enter on a row or `--continue`.

ANSI Shadow `TALARIA - CLIENT` is retired. A large wordmark may live on
`/about`. Caduceus is a small *agent* mark, not Talaria’s logo
([NOTICE.md](../NOTICE.md)).

---

## Visual system

Default skin matches the site: parchment light **or** a bronze/ink dark
twin of the same tokens. GitHub Dark and Hermes builtins (`ares`,
`poseidon`, `charizard`, …) remain options, clearly secondary.

Role map (small — not 40 tokens):

- `canvas` / `chrome` / `card` / `chip` / `modal`
- `text` / `text-dim` / `text-invert`
- `accent` (Talaria) / `agent` (Hermes) / `user` / `tool` / `danger`
- `focus` / `selection` / `border-subtle` / `border-input`

Pass `Theme` into render (or hold it on `App`). Stop using a process-global
`RwLock` as the long-term API (`theme::BACKGROUND()` may remain as a
compat shim until call sites move).

---

## Talaria-own features (why we are better, not faster)

These are the superiority bets. They are in the plan, not a later wishlist.

1. **Command palette (`Ctrl+K`)** — searches host actions *and* the gateway
   catalog. Slash stays for people who already know `/rewind`. Discovery is
   the palette.
2. **Sheets that survive a turn** — manage model/skills/MCP while Hermes
   streams. Ink’s modal steals the whole screen.
3. **Inspectors for full output** — tool args/result, thinking, usage.
   Copy. No 12-line cap. Timeline stays a summary.
4. **Returning workspace** — last threads on empty state + `--continue`.
   Ink’s splash is a brochure; ours is a door.
5. **Two live verbs** — Enter steers; `Ctrl+Enter` queues. Documented, not
   overloaded.
6. **Chip-level meter** — `/custom` toggles model, bar, clocks, cwd, title,
   branch, YOLO, compress, bg-count independently.
7. **Document transcript** — no help strings, one type system, drag-copy
   (already a strength) stays first-class.
8. **Host prefs** — mouse, spinner, focus, light/dark auto, key hints — in
   `~/.talaria/`, not `~/.hermes`.
9. **Site-aligned beauty** — one default look from landing page to TUI.
10. **Mock as a product** — `--dev --mock=` remains a demoable host without
    an API key (already true; keep it as we change chrome).

---

## Engineering constraints (so the UI can land)

- Split `Chat` state: `Transcript`, `Composer`, `Chrome` (meter/hints),
  `Sheet` / `Modal` / `Inspector`.
- Stop growing `ScreenAction` as a flat enum of every hub RPC; group
  `SessionNav`, `ModelHub`, `Safety`, `Palette`.
- Keep `SessionEvent` / `SessionCommand` as the UI↔session contract.
- Do not invent gateway events. Dump-pin `status.update` before YOLO /
  compress / bg badges.
- Ordinary `prompt.submit` stays `{session_id, text}` only. Queue is a
  *later* submit, not leftover rewind params.
- After Phase C (objects), add a short `ARCHITECTURE.md` (current names,
  overlay taxonomy, theme rules). Do not let `PLAN.md` (still says
  `hermes-rust`) remain the only map.

---

## Key decisions

1. **Ink is the protocol/capability catalog, not the visual spec.**
   Rationale: a clone is strictly worse than official Ink; a better host
   can win on usability.
2. **Enter steers; queue is a second verb.** Rationale: we already shipped
   steer; matching Ink’s Enter=queue would silently change a live verb.
3. **`/clear` wipes the view; `/new` creates a session.** Rationale:
   transcript-as-document. Palette labels this explicitly.
4. **Default skin is Talaria (site pair), not GitHub Dark.** Rationale:
   identity. Hermes skins stay selectable.
5. **Caduceus is an agent mark, never the hero.** Rationale: NOTICE.md.
6. **One Screen; three hosted surfaces (modal / sheet / inspector) +
   palette.** Rationale: `Overlay` as a 20-variant enum does not scale.
7. **Stdio `tui_gateway` remains the only transport in this plan.**
   Rationale: PLAN.md; WS is dashboard-internal.
8. **No audio, no in-buffer images, no `hermes` binary name.** Rationale:
   different products; identity.

---

## Phased implementation

Each phase is independently reviewable and demoable (`--dev --mock=streaming`
must still look like Talaria). Do not wait for a dump to start Phase A–C.
Do wait for a dump before YOLO/compress/bg **badges**.

```
A identity  →  B workspace  →  C document  →  D objects
     →  E palette  →  F absorb remaining jobs  →  G superiority polish
```

D and E can overlap after C. F is where leftover Hermes jobs land *into*
the new objects. G is Talaria-own beauty and speed.

### Phase A — Identity (no protocol)

**Exit:** first paint agrees with NOTICE.md and the site.

- Wordmark **TALARIA** (drop `TALARIA - CLIENT`).
- Empty-state copy: “Native TUI host for Hermes Agent” / “Talaria is ready.”
  Never “Welcome to Hermes Agent!”
- Demote caduceus; small agent mark at most.
- Assistant card: `⚕ Hermes` as author; panel uses Talaria tokens.
- Default skin = site pair (`warm-lightmode` or a new `talaria` dark).
  GitHub Dark remains a `/skin`.
- `/about` may keep a large wordmark.

**Files:** `src/ui/widgets/banner.rs`, `src/ui/screens/chat/render.rs`,
`src/theme.rs`, tests in those modules.

### Phase B — Returning workspace

**Exit:** a second launch feels like a session manager, not a splash.

- CLI: `--continue` / `--resume <id-or-title>` (and `TALARIA_RESUME`).
  Startup path: `Resume` instead of `Create` when asked.
- Empty state: first-run vs returning (last three from `session.list`).
- `/clear` = local wipe; `/new` / palette “New session” = `session.create`.
- Meter: `branch` + title from `session.info`; `/custom` starts chip
  toggles (even if only model/bar/clocks at first).

**Files:** `src/cli.rs`, `src/app/run.rs`, `src/session/live.rs` (already
has Resume), `src/ui/screens/chat/{mod,render,status,input}.rs`.

### Phase C — Transcript is a document

**Exit:** no instructional lines in the timeline; tool expand is chrome.

- Remove “click or Ctrl+O …” lines.
- Role table above (user / Hermes / thinking / tool / shell / system).
- Timeline summary caps stay; inspector (even if still a modal in C)
  shows full text + copy.
- `/focus` hides thinking/tools; nothing is discarded.
- Optional: reduce the stream→markdown pop (lite markdown while
  streaming, or hold card chrome stable).

**Files:** `src/ui/screens/chat/render.rs`, `overlay.rs`, `input.rs`,
`src/ui/widgets/markdown.rs`.

### Phase D — Modal / sheet / inspector

**Exit:** hubs are sheets with a search field; safety is still a modal;
one thing can be inspected without rewriting the transcript.

- Introduce types (even if they share a render helper at first):
  `Modal`, `Sheet`, `Inspector`.
- Move sessions, model, skills, plugins, MCP, agents, trees to **sheets**.
- Move usage, help, tool result, thinking to **inspectors**.
- Keep approval/sudo/secret/quit/rewind-confirm/bang as **modals**.
- Split `Chat` state along those lines. Group `ScreenAction`.
- Sheet may remain open across `MessageDelta` (non-blocking).

**Files:** `src/ui/screens/chat/{mod,overlay,hubs,model_picker}.rs`,
`src/ui/screens/mod.rs`, `src/app/dispatch.rs`.

**Then:** write `docs/ARCHITECTURE.md` (short).

### Phase E — Palette

**Exit:** a new user can find Rewind, Resume, Model, Focus without
knowing slash names.

- `Ctrl+K` / `Ctrl+P` fuzzy list: host actions + `commands.catalog`.
- Idle key hints shrink (`/` and `Ctrl+K` and context verbs only).
- Palette “Edit in editor”, “Queue”, “New session”, “Resume”, “Focus”.

**Files:** new `src/ui/widgets/palette.rs` (or similar), `input.rs`,
`key_hints.rs`.

### Phase F — Absorb remaining Hermes jobs into those objects

**Exit:** [PARITY.md](./PARITY.md) Partial/No rows that this plan does not
mark Out are either Yes or explicitly slash-only.

- Ctrl+V text-then-image; paste-collapse.
- `spawn_tree.save` on the trees sheet.
- Map `skin.changed`, `status.update`, `background.complete` (dump-pin
  fields before badges).
- LaTeX in markdown; light-terminal detect → Talaria pair.
- Palette `/reload` → `reload.env` / `reload.mcp`.
- Usage inspector: context breakdown if `session.usage` has categories.
- Agents sheet: extra columns only from real `delegation.status` fields.
- `/mouse`, `/indicator` as `/custom` prefs; `/terminal-setup` as a
  palette action.
- Mid-turn **queue** (`Ctrl+Enter`) flushed on `MessageComplete`.

No new Overlay variants for `/journey`, `/context`, `/personality`.

### Phase G — Superiority polish

**Exit:** the product is obviously not Ink: quieter, searchable, site-twin,
document-first.

- Theme passed into render; role tokens used everywhere.
- Inspector copy/full/follow; drop silent caps.
- Sheet + stream split feels native (resize-aware).
- Meter chip menu complete.
- Mock scenarios restyled so demos sell Talaria, not Hermes.
- `PARITY.md` language: capability coverage, not widget clone.

---

## PR plan

Incremental, independently reviewable. Each PR should be demoable. Prefer
`--dev --mock=streaming` screenshots / mock runs in review.

### PR 1 — Identity copy and wordmark

- **Title:** `ui: Talaria wordmark and empty-state copy`
- **Files:** `banner.rs`, `big_text.rs`, `render.rs` splash, tests
- **Depends on:** none
- **Description:** `TALARIA` wordmark; no “Welcome to Hermes Agent!”; no
  `TALARIA - CLIENT`. Caduceus no longer hero. Tagline stays unofficial.

### PR 2 — Default skin = site pair

- **Title:** `theme: Talaria default aligned with the site`
- **Files:** `theme.rs`, `cli.rs` help text, `RUNNING.md` if it names github
  as default
- **Depends on:** none (can parallel PR 1)
- **Description:** default id `talaria` or `warm-lightmode` + dark twin.
  GitHub remains selectable. Gateway skin overlay does not override a
  named Talaria theme (already true for non-github).

### PR 3 — Assistant card tokens

- **Title:** `ui: Hermes author inside Talaria panel tokens`
- **Files:** `render.rs` `assistant_card`
- **Depends on:** PR 2
- **Description:** keep `⚕ Hermes`; borders/fill from Talaria `card` /
  `agent` tokens, not Ink gold-by-default.

### PR 4 — Resume on launch

- **Title:** `feat: --continue / --resume and returning empty state`
- **Files:** `cli.rs`, `app/run.rs`, `dispatch.rs`, chat empty state,
  `session.list` at startup
- **Depends on:** PR 1 (empty-state layout)
- **Description:** CLI flags + `TALARIA_RESUME`. Returning empty state
  lists last sessions. First-run stays short.

### PR 5 — `/clear` vs `/new`; meter branch/title

- **Title:** `ux: document wipe vs new session; meter chips`
- **Files:** `input.rs`, `status.rs`, `mod.rs` `apply_session_info`,
  `prefs.rs`
- **Depends on:** PR 4
- **Description:** `/clear` local; `/new` creates. Show `branch` + title.
  `/custom` begins chip toggles.

### PR 6 — Document transcript

- **Title:** `ui: remove timeline help chrome; role system`
- **Files:** `render.rs`, key hints
- **Depends on:** PR 3
- **Description:** no expand-help lines; user/Hermes/tool/shell/system
  treatment from the role table. Caps stay until PR 8 inspector.

### PR 7 — Tool inspector (full result)

- **Title:** `ui: tool click opens inspector instead of rewriting the card`
- **Files:** `overlay.rs` or new inspector, `input.rs`, `render.rs`
- **Depends on:** PR 6
- **Description:** Ctrl+O / click → inspector with full args/result +
  copy. Timeline chip stays one line. Thinking “show all” can share this.

### PR 8 — `/focus` and paste-collapse

- **Title:** `ux: focus view and composer paste-collapse`
- **Files:** `prefs.rs`, `render.rs`, `text_composer.rs`
- **Depends on:** PR 6
- **Description:** `/focus` hides thinking/tools. Long paste collapses in
  the composer with an expand hint *in the composer*, not the transcript.

### PR 9 — Sheet/modal/inspector split

- **Title:** `refactor: overlay taxonomy (modal, sheet, inspector)`
- **Files:** `ui/screens/chat/overlay.rs` (split module tree), `mod.rs`
  state split, `screens/mod.rs` `ScreenAction` groups
- **Depends on:** PR 7
- **Description:** behavioral change allowed: hubs gain a search field;
  sheets do not block `MessageDelta`. Safety modals unchanged. No new
  product surfaces.

### PR 10 — Command palette

- **Title:** `feat: Ctrl+K palette for host actions and catalog`
- **Files:** new widget, `input.rs`, `key_hints.rs`
- **Depends on:** PR 9 (so palette opens a sheet/inspector, not a 21st overlay kind)
- **Description:** fuzzy list. Does not remove `/`.

### PR 11 — Queue verb + Ctrl+V order + spawn_tree.save

- **Title:** `feat: follow-up queue, smarter paste, spawn-tree save`
- **Files:** `input.rs`, `live.rs`, `clipboard.rs` / paste helpers,
  trees sheet
- **Depends on:** PR 10 (palette “Queue”), PR 9 (trees sheet)
- **Description:** `Ctrl+Enter` queues; Enter still steers. Ctrl+V tries
  text then `clipboard.paste`. Trees sheet can save.

### PR 12 — Wire ignored events + dump-pin badges

- **Title:** `feat: map status.update, skin.changed, background.complete`
- **Files:** `protocol.rs`, `session/parse.rs`, meter, toasts
- **Depends on:** PR 5 (meter chips), a **live dump** of those events
- **Description:** no invented fields. If the dump lacks YOLO/compress/bg,
  ship mapping + toast only.

### PR 13 — Markdown math, light detect, editor, reload

- **Title:** `ux: LaTeX, auto light/dark, $EDITOR, reload`
- **Files:** `markdown.rs`, theme startup, `app/run.rs` suspend, palette
  actions, `live.rs` `reload.env` / `reload.mcp`
- **Depends on:** PR 10, PR 2
- **Description:** host-side. Editor suspends the alt-screen.

### PR 14 — Prefs + usage inspector + agents sheet columns

- **Title:** `ux: /custom mouse/spinner; usage inspector; agents sheet`
- **Files:** `prefs.rs`, usage inspector, `hubs.rs` / agents sheet,
  `parse.rs` only if dump shows extra delegation fields
- **Depends on:** PR 9, PR 12
- **Description:** `/mouse` `/indicator` are prefs. `/context` is the
  usage inspector, not a mosaic unless it earns space.

### PR 15 — Theme injection + mock restyle + ARCHITECTURE.md

- **Title:** `chore: theme-by-value, mock chrome, ARCHITECTURE.md`
- **Files:** `theme.rs` call sites, mock chat fixtures, `docs/ARCHITECTURE.md`,
  this file’s status note
- **Depends on:** PR 9, PR 2
- **Description:** drop global palette as the *API*. Demos look like
  Talaria. Short architecture pointer so `PLAN.md` is not the map.

---

## How to use these docs

| Question | Doc |
|----------|-----|
| Does Talaria *cover* this agent job? | [PARITY.md](./PARITY.md) |
| How do we sequence Ink leftover *tracks*? | [PARITY-PLAN.md](./PARITY-PLAN.md) (historical; new work follows **this** file) |
| How do we redesign chrome and absorb jobs? | **This file** |
| What should designers draw? | [DESIGN-HANDOUT.md](./DESIGN-HANDOUT.md) · [handout.html](./handout.html) |
| Why host, not clone? | `journal/better-host.md`, `journal/design-audit.md` |
| How to run | [RUNNING.md](./RUNNING.md) |

When a PR in this plan lands, update **PARITY.md** if capability coverage
changed, and tick the PR row here. Do not add a second status SSOT.
