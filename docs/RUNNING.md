# Running Talaria

## Install

```bash
brew install inaki/talaria/talaria
curl -fsSL https://raw.githubusercontent.com/inaki/talaria/main/install.sh | bash
```

Latest release: [v0.1.4](https://github.com/inaki/talaria/releases/tag/v0.1.4). Prebuilt curl binaries: macOS (Apple silicon and Intel), Linux x86_64/arm64.

The UI binary is `talaria`. Live mode uses the same `~/.hermes` as `hermes --tui`. Chrome (skin, composer history, `/custom`) is `~/.talaria/`.

If another Hermes Agent TUI is running, this process exits before the alt-screen with pids to quit. `--force` skips that check.

## Offline (no Hermes)

```bash
cargo test
cargo run -- --dev --mock=streaming
cargo run -- --dev --mock=tools
cargo run -- --dev --mock=approval
cargo run -- --dev --mock=subagent
cargo run -- --dev --mock=error
```

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
cargo run                                 # TUI
```

Logs: `~/.talaria/logs/talaria.log` (always on, 0600). Never writes `~/.hermes`.

Skins: `github` (this host's default) plus Hermes builtins (`default`, `ares`, `mono`, `slate`, `daylight`, `warm-lightmode`, `poseidon`, `sisyphus`, `charizard`). `/skin` in the TUI (alias `/theme`), or:

```bash
cargo run -- --theme default
TALARIA_THEME=ares cargo run -- --dev --mock=streaming
```

Choice is saved to `~/.talaria/theme`.

`/rewind` regenerates from a past user turn. It only offers turns that have a durable `row_id` (from `session.history` / resume). Confirming sends `confirm_truncate` + `truncate_before_row_id` — ordinary Enter never truncates.
