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
| 3 | Remote binary helper fast path and complete cross-architecture CI packaging | Complete — CI run 31616578171; cross-architecture run 31616578255 |
| 4 | OpenSSH alias discovery, configuration v2 migration, and explicit absolute MCP paths | Complete — 4A (CI 31617909377; cross-architecture 31617909331); 4B (CI 31620176593; cross-architecture 31620176591); 4C1 configuration v2 and discovery-only CLI (CI 31621703294; cross-architecture 31621703286); 4C2 transactional installed-config migration (CI 31622376325; cross-architecture 31622375360); 4C3 legacy host policy removal (CI 31630291845; cross-architecture 31629421059); 4C4a artificial admission queue removal (CI 31631616503; cross-architecture 31631238373); 4C4b obsolete concurrency configuration removal (CI 31632509089; cross-architecture 31632330285) |
| 5 | Compact MCP results, bounded session liveness, output/RSS controls, and cache policy | In progress — 5A explicit remote helper timeout reporting complete (CI 31633649813; cross-architecture 31633194392); 5B recoverable MCP response backpressure complete (CI 31634607936; cross-architecture 31634290790); 5C retained model-presentation contract complete (CI 31635138728; cross-architecture 31635138709); 5D bounded CI and release caches complete (CI 31636323152; cross-architecture 31635726057); 5E direct framed command dispatch, bounded search producers, and watchdog reaping complete (CI 31639965332; cross-architecture 31639074793); 5F isolated streaming output capture complete (CI 31641280885; cross-architecture 31641280921); 5G bounded session setup, per-runner transport isolation, and lazy previews complete (CI 31642742575; cross-architecture 31642742581); 5H remote admission boundary complete (CI 31643377690; cross-architecture 31643377730) |
| 6 | Write-back edit cache, synchronization barriers, poisoned-session recovery, and one setup deadline | Complete — 6A write-back cache configuration contract (CI 31644038846; cross-architecture 31644038789); 6B in-memory edit cache state machine (CI 31644506705; cross-architecture 31644506611); 6C guarded batch commit backend (CI 31645434511; cross-architecture 31645274236); 6D cached read/write wiring (CI 31647503889; cross-architecture 31647503898); 6E synchronization barriers (CI 31648022263; cross-architecture 31648022295); 6F shutdown flush (CI 31648531055; cross-architecture 31648531027); 6G1 durability contract (CI 31650833130); 6G2a opt-in profiling prerequisite (CI 31650833130; cross-architecture 31649762124); 6G2b cache latency, RSS, and required real-SSH gates (CI 31650833130); 6G3a factual synchronization errors and safe staging (CI 31650833130; cross-architecture 31650589162); 6G3b cancellation-safe barriers (CI 31652148759; cross-architecture 31651314312); poisoned-session capture recovery (CI 31652148759; cross-architecture 31651747087); single setup deadline (CI 31652528546; cross-architecture 31652528550) |
| 7 | Native helper search and cancellation-safe concurrent session leasing | Complete — 7A nested basename glob semantics complete (CI 31653184871; cross-architecture 31652940665); 7B exhaustive search beyond the bounded candidate prefix complete (CI 31653572618; cross-architecture 31653572633); 7C0a continuation-aware EXIT schema complete (CI 31654015080; cross-architecture 31654015094); 7C0b1 bounded binary-helper descendant-pipe cleanup complete (CI 31654375340; cross-architecture 31654375464); 7C0b2 POSIX dispatcher parity and 7C1a native helper search protocol complete (CI 31655452427; cross-architecture 31655452480); 7C1b bridge/session integration complete (CI 31656039373; cross-architecture 31656039369); 7C2a helper cancellation reuse and vectorized performance gates complete (CI 31656436859; cross-architecture 31656436869); 7C2b pre-PID cancellation race complete (CI 31656910094; cross-architecture 31656910095); 7D isolated concurrent session leasing complete (CI 31657229176; cross-architecture 31657229252); 7E bounded idle session retention complete (CI 31657590760; cross-architecture 31657590794) |
| 8 | Durable remote job lifecycle and MCP tools | In progress — 8A closed job protocol contract complete (CI 31657983208; cross-architecture 31657983206); 8B durable job runner complete (CI 31658353506; cross-architecture 31658353486); 8C detached helper job transport complete (CI 31658710114; cross-architecture 31658710099); 8D complete job lifecycle control complete (CI 31659031629; cross-architecture 31659031618); 8E persistent job transport complete (CI 31659366581; cross-architecture 31659366563); 8F bridge job operations in progress |
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
