# Talaria docs

Living documentation for Talaria Client, the unofficial native Rust TUI for Hermes Agent.

## Source of truth

| Doc | Purpose |
|-----|---------|
| **[PARITY.md](./PARITY.md)** | **Status SSOT.** What we cover vs `hermes --tui`. |
| [PLAN.md](./PLAN.md) | Historical architecture discussion. **Not status.** |
| [PARITY-PLAN.md](./PARITY-PLAN.md) | Remaining tracks vs Ink TUI |
| [RUNNING.md](./RUNNING.md) | How to run mock / live |

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
