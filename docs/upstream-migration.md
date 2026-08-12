# Upstream Migration Roadmap

This repository ports `wkj2333666/Codex-SSH-Bridge` to Claude Code in dependency order. The port is intentionally incremental: each batch must preserve the Claude Code plugin, MCP registration, installer, naming, and release layout rather than copying Codex-specific packaging verbatim.

## Verification policy

- Do not run builds, tests, Clippy, benchmarks, release builds, or publishing locally.
- GitHub Actions is authoritative for formatting, compilation, tests, packaging, and releases.
- Local checks are limited to source comparison, focused static inspection, and `git diff --check`.
- A batch is complete only after its pushed commit reaches a terminal successful CI state.

## Comparison anchors

- Local Claude Code migration baseline: upstream 0.1.x behavior, ported on 2026-07-21.
- Latest upstream snapshot reviewed: `97ad0224904e7a8e3cfff4f88a121c7b127efc44` (`v0.8.1`, 2026-08-02).
- First post-baseline upstream contract commit: `6aa96ed05a86` (explicit shell selection, 2026-07-22).

## Ordered batches

| Batch | Upstream capability | Status |
|---|---|---|
| 1 | Public `remote_run` shell contract: Bash by default, explicit `sh`, no silent fallback | Complete — CI run 31599638885 |
| 2 | Bounded frames, POSIX dispatcher, and persistent per-host SSH sessions | Complete — CI run 31604910188 |
| 3 | Remote binary helper fast path and complete cross-architecture CI packaging | In progress — 3C remote kernel architecture probe |
| 4 | OpenSSH alias discovery, configuration v2 migration, and explicit absolute MCP paths | Pending |
| 5 | Compact MCP results, bounded session liveness, output/RSS controls, and cache policy | Pending |
| 6 | Write-back edit cache, synchronization barriers, poisoned-session recovery, and one setup deadline | Pending |
| 7 | Native helper search and cancellation-safe concurrent session leasing | Pending |
| 8 | Durable remote job lifecycle and MCP tools | Pending |
| 9 | Dual unified/Codex patch syntax adapted to Claude Code terminology | Pending |
| 10 | Full source, behavior, documentation, packaging, CI, and release parity audit against the pinned upstream snapshot | Pending |

### Batch 2 delivery gates

The persistent-session work is deliberately split at compile-safe boundaries:

1. Port the bounded `CXSB1` frame codec and its malformed/oversize regression cases.
2. Port the audited POSIX dispatcher and deterministic dispatcher protocol fixtures.
3. Add `HostSession` lifecycle, cancellation, request multiplexing, and transport-unknown outcomes.
4. Route existing high-level operations through sessions without weakening root, identity, output, or mutation guards.
5. Adapt Claude Code-facing wording and acceptance coverage, then run the full GitHub CI gate.

Each gate is pushed and made green before the next gate is implemented. Persistent-session startup must remain fail-closed; a request is never replayed after delivery becomes ambiguous.

When upstream advances, update the reviewed snapshot and append or reorder batches based on actual dependencies. Do not collapse later features into an earlier batch merely because their final files overlap.
