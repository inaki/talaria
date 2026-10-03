# Running Talaria

## Install

```bash
brew install inaki/talaria/talaria
curl -fsSL https://raw.githubusercontent.com/inaki/talaria/main/install.sh | bash
```

Latest release: [v0.1.6](https://github.com/inaki/talaria/releases/tag/v0.1.6). Prebuilt curl binaries: macOS (Apple silicon and Intel), Linux x86_64/arm64.

The UI binary is `talaria`. Live mode uses the same `~/.hermes` as `hermes --tui`. Chrome (skin, composer history, `/custom`) is `~/.talaria/`.

If another Hermes Agent TUI is running, this process exits before the alt-screen with pids to quit. `--force` skips that check.

## Offline (no Hermes)

```bash
cargo test
cargo run -- --dev --mock=home        # first-run splash (no recents)
cargo run -- --dev --mock=streaming   # returning empty + keyword hub
```

`--mock=streaming` is a keyword hub. Type one of these in the composer and Enter:

| Keyword | UI |
|---------|-----|
| `code` | rust fence in the document |
| `pdf` / `doc` | `read_file` on a PDF, then a summary |
| `ask` | clarify modal — pick a choice |
| `type` | clarify modal — type to continue |
| `batch` | multi-question clarify (`1/3` … `3/3`), Esc cancels all |
| `tools` | tool chips |
| `error` | provider error |
| `toolerror` | failed tool |
| `approval` | danger modal |
| `agent` | subagent rollup |
| `help` | this list |

Anything else streams a short reply that lists the keywords. Forced scripts still work: `--mock=tools`, `--mock=approval`, `--mock=subagent`, `--mock=error`.

Home screens in the gallery: **Empty splash** (first run) and **Empty returning** (last sessions).

```bash
cargo run --example gallery
cargo run --example mascot          # winged sneaker PNG → Braille dots (like caduceus)
```

## Design gallery

Renders every chrome piece (tokens, document, sheet, inspector, palette, modals)
on the live theme. Dev-only; not in the `talaria` binary.

```bash
cargo run --example gallery
```

Identity samples: **Mascot** (Braille, gold/mint tokens), **Mascot (logo-art)** (true-color `▄`/`▀`), and **Effects (tachyonfx)** (home enter, mint pulse, gold fade, dissolve, tool panel expand, response left-rule).

↑/↓ move · Enter interact (composer, slash, palette, sheets, inspectors) · `t` cycle skin · `q` quit.

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
cargo run                                 # TUI (fresh session)
cargo run -- --continue                   # most recent saved session
cargo run -- --resume latest
cargo run -- --resume "my thread"         # id or title
TALARIA_RESUME=1 cargo run                # same as --continue
```

Logs: `~/.talaria/logs/talaria.log` (always on, 0600). Never writes `~/.hermes`.

Skins: `talaria` (default, bronze/ink dark) and `talaria-light` (parchment) match the site. GitHub Dark and Hermes builtins (`default`, `ares`, `mono`, `slate`, `daylight`, `warm-lightmode`, `poseidon`, `sisyphus`, `charizard`) stay selectable. `/skin` in the TUI (alias `/theme`), or:

```bash
cargo run -- --theme default
TALARIA_THEME=ares cargo run -- --dev --mock=streaming
```

Choice is saved to `~/.talaria/theme`.

`/rewind` regenerates from a past user turn. It only offers turns that have a durable `row_id` (from `session.history` / resume). Confirming sends `confirm_truncate` + `truncate_before_row_id` — ordinary Enter never truncates.
