# Notice

**Talaria Client** is an independent, unofficial native Rust TUI for [Hermes Agent](https://github.com/NousResearch/hermes-agent).

It is **not** a product of [Nous Research](https://nousresearch.com), and it is **not** affiliated with, endorsed by, or sponsored by Nous Research.

## What this project is

Talaria is a native [ratatui](https://ratatui.rs) client. It speaks the documented stdio JSON-RPC `tui_gateway` protocol, then renders chat, tools, and slash commands. The Python Hermes Agent process still owns models, keys, tools, memory, skills, and `~/.hermes`.

This repository does **not** vendor, fork, or redistribute Hermes Agent source.

## Names

| Name | Whose |
|---|---|
| **Talaria** | This project. In classical mythology, the talaria are the winged sandals of Hermes — a public-domain story, not a Nous mark. |
| **Hermes Agent**, `hermes`, `hermes --tui`, `HERMES-AGENT` | Products and marks of Nous Research. Used here only to describe what Talaria hosts. |
| Binary `talaria` | This project. Never ships as `hermes`. |

Do not present Talaria as the official Hermes TUI. The splash wordmark is **TALARIA**. The caduceus is Hermes Agent’s mark, shown because this host talks to that agent — not as Talaria’s own logo. Do not use the tagline “Messenger of the Digital Gods”.

## Protocol

Live mode uses Hermes Agent’s public `tui_gateway` integration:

https://hermes-agent.nousresearch.com/docs/developer-guide/programmatic-integration

## License

Talaria is MIT-licensed (see `LICENSE`). Hermes Agent is separately MIT-licensed by Nous Research; using it still means accepting *their* terms.
