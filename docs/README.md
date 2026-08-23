# hermes-rust docs

Living documentation for the native Rust + ratatui Hermes TUI host.

## Source of truth

| Doc | Purpose |
|-----|---------|
| **[PARITY.md](./PARITY.md)** | **Status SSOT.** What we cover vs `hermes --tui`. |
| [PLAN.md](./PLAN.md) | Architecture, protocol notes, crate layout, PR history |
| [PARITY-PLAN.md](./PARITY-PLAN.md) | Remaining tracks vs Ink TUI |
| [RUNNING.md](./RUNNING.md) | How to run mock / live |

**Never create `MIGRATION.md`.** This is a greenfield host, not a port of Ink TSX.

## Follow-on (optional)

| Doc | Purpose |
|-----|---------|
| `ARCHITECTURE.md` | Principles once we want a short pointer |
| `STRUCTURE.md` | Module-by-module tour |
| `BEST_PRACTICES.md` | Conventions |

## Frozen names

Cargo package `hermes-rust`, binary `hermes-rust`, library `hermes_rust`.

## Upstream (read-only)

- Official protocol: [Programmatic Integration](https://hermes-agent.nousresearch.com/docs/developer-guide/programmatic-integration)
