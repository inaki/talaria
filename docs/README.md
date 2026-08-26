# Talaria docs

Living documentation for Talaria Client, the unofficial native Rust TUI for Hermes Agent.

## Source of truth

| Doc | Purpose |
|-----|---------|
| **[PARITY.md](./PARITY.md)** | **Status SSOT.** Agent-job coverage vs `hermes --tui`. Not a visual spec. |
| **[REDESIGN.md](./REDESIGN.md)** | Chrome / IA implementation plan: better host, not faster clone. |
| **[POST-DESIGN.md](./POST-DESIGN.md)** | Walkthrough to sign off the redesign. |
| **[DESIGN-HANDOUT.md](./DESIGN-HANDOUT.md)** | Designer brief (tokens, frames, copy). Visual: [handout.html](./handout.html). |
| [PLAN.md](./PLAN.md) | Historical architecture discussion. **Not status.** |
| [PARITY-PLAN.md](./PARITY-PLAN.md) | Historical Ink leftover tracks. New work follows **REDESIGN.md**. |
| [RUNNING.md](./RUNNING.md) | How to run mock / live / `cargo run --example gallery` |

**Never create `MIGRATION.md`.** This is a greenfield host, not a port of Ink TSX.

## Follow-on (optional)

| Doc | Purpose |
|-----|---------|
| `ARCHITECTURE.md` | Principles once we want a short pointer |
| `STRUCTURE.md` | Module-by-module tour |
| `BEST_PRACTICES.md` | Conventions |

## Names

Cargo package `talaria`, binary `talaria`, library `talaria`, `publish = false`. Chrome `~/.talaria`. Agent home remains `~/.hermes`. See [NOTICE.md](../NOTICE.md).

## Upstream (read-only)

- Official protocol: [Programmatic Integration](https://hermes-agent.nousresearch.com/docs/developer-guide/programmatic-integration)
