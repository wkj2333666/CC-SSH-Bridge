# CC SSH Bridge

Use Claude Code on this local machine to inspect, edit, and run commands on allowlisted SSH servers without installing or signing in to Claude Code on those servers.

```text
local Claude Code
    │ local stdio MCP
    ▼
native Rust bridge
    │ local OpenSSH + local keys/agent/known_hosts
    ▼
remote sshd ── files, compilers, tests, services

optional, human-only: local SSHFS mount over SFTP
```

The bridge keeps one locally owned SSH session per alias. On supported Linux architectures it reuses a versioned precompiled Rust helper from the remote account's private data directory, uploading it only when the exact length and SHA-256 are not already present. Startup can fall back to a temporary helper and then the complete POSIX dispatcher. The server receives no Claude Code binary, API key, plugin, MCP server, or background daemon.

## Why this design

| Approach | Strength | Problem for this use case | Role |
|---|---|---|---|
| Raw `ssh` | Universal and minimal | Leaves target selection, quoting, limits, shell detection, cancellation, and output handling to the Agent | Transport below the bridge |
| SSHFS | Convenient human browsing | Makes remote files look local while commands still run locally; adds FUSE/SFTP latency and reconnect semantics | Explicit optional CLI only |
| Native local MCP | Closed schemas, allowlisted hosts, bounded I/O, shared policy, visible Bash/sh fallback | Non-interactive by design | Default Agent interface |
| Official SSH Remote | Native remote project experience | Currently requires remote installation/authentication | Deliberately not used |

The bridge is Rust rather than a Bash program because strict MCP framing, bounded parsing, async concurrency, process-group cancellation, spool quotas, and transactional installation need one auditable state machine. Bash and POSIX sh remain supported as the *remote command shells*; the result always reports which shell actually ran.

SSHFS is intentionally absent from the MCP tool list. This prevents an Agent from silently treating a FUSE path as a local workspace.

## Requirements

- Local Linux host with the packaged `cc-ssh-bridge` binary.
- Local OpenSSH client at `/usr/bin/ssh`.
- Key-based or local-agent authentication and verified host keys.
- Remote `sshd`, a POSIX sh, a GNU- or BSD-compatible `stat`, and the ordinary utilities checked by `doctor`; Bash is optional. `shell=login` additionally needs an account shell that can be resolved through `getent passwd` or, when `getent` is absent, one unique readable `/etc/passwd` record.
- Optional local `sshfs` and `fusermount3` for the human mount commands.
- Rust 1.91.1 or newer only when rebuilding.

The bundled bridge binary is native to the local machine/architecture on which it was built. Rebuild and replace `bin/cc-ssh-bridge` when moving the plugin to a different local architecture. Unsupported remote architectures continue to use the POSIX dispatcher.

## Build and package locally

```bash
cargo build --release
mkdir -p bin
cp target/release/cc-ssh-bridge bin/cc-ssh-bridge
chmod 0755 bin/cc-ssh-bridge
sha256sum target/release/cc-ssh-bridge bin/cc-ssh-bridge
./bin/cc-ssh-bridge --help
```

There is no Python runtime or remote build step.

## CI and release builds

GitHub Actions runs formatting, Clippy, the full test suite, a release build,
and source-package checks for pull requests and pushes to `main`.

Release builds are created only from version tags, which must match the version
in `Cargo.toml`. The workflow publishes bridge binaries for eight common Linux
GNU/musl targets. Each archive also contains `remote-helpers/` for all six
supported remote architectures: static musl helpers for `x86_64`, `aarch64`,
and `armv7l`, plus GNU-target helpers for `riscv64`, `ppc64le`, and `s390x`.
When a GNU helper cannot run because the remote loader or libc is incompatible,
the bridge falls back during startup to the POSIX dispatcher.

Keep `remote-helpers/` beside the bridge binary. The bridge probes `uname -s`
and `uname -m`, selects the matching artifact, and installs it as mode 0700 at
`~/.local/share/cc-ssh-bridge/helpers/<bridge-version>/<target>/helper`. A
length and SHA-256 match reuses that helper without another upload. Persistent
startup failure falls back to a one-session temporary helper; unsupported hosts
or artifacts use the POSIX dispatcher. Results report the selected
`helper_mode` as `persistent`, `temporary`, or `shell`. For local development
or a custom package, set `CC_SSH_BRIDGE_HELPERS_DIR` to a private directory
containing executable files named by their Rust target triple.

## Configure hosts

Define and manually verify a concrete alias in local `~/.ssh/config`:

```sshconfig
Host devbox
  HostName devbox.example.com
  User deploy
  IdentityFile ~/.ssh/id_ed25519
  ForwardAgent no
```

```bash
ssh devbox
./bin/cc-ssh-bridge hosts list
./bin/cc-ssh-bridge doctor devbox
```

The bridge automatically discovers concrete aliases from `~/.ssh/config` and recursively follows bounded `Include` files. Pattern-only and negated aliases are not exposed, and there is no five-host ceiling. The default local config is `~/.config/cc-ssh-bridge/config.toml`; [config.example.toml](config.example.toml) documents global operational limits. It accepts exactly configuration `version = 2`. Hosts, roots, credentials, descriptions, read-only policy, and admission-control limits are not stored there.

On the first operation for an alias, the bridge resolves the local OpenSSH policy with bounded `ssh -G`, records its immutable connection identity, and probes shell/utility capabilities. The policy and capability result are cached for the lifetime of the bridge; later operations use one framed request on the already-open SSH session without another `ssh -G`, root observation, or physical-root guard. The local Unix user and that user's OpenSSH configuration remain trusted execution authority.

`doctor` reports the connection-time physical path and device/inode identity as diagnostics. MCP operations use caller-supplied absolute paths, and ordinary remote filesystem behavior—including symlink retargeting—matches a command run directly on that server. Individual writes and patches still use expected hashes, no-follow identity checks, atomic replacement, and explicit unknown-outcome reporting.

`doctor devbox --verbose-ssh` also runs a bounded local OpenSSH diagnostic and redacts identity paths, agent sockets, commands, and credential-like fields.

## Install for local Claude Code

The package contains a Claude Code plugin manifest, Skill, and local stdio MCP manifest. Claude Code supports MCP servers and Skills natively ([MCP](https://docs.anthropic.com/en/docs/claude-code/mcp)).

For a direct user installation, review the dry run first:

```bash
./bin/cc-ssh-bridge install --user
./bin/cc-ssh-bridge install --user --apply
claude mcp get ssh-bridge
```

The installer:

- accepts only this canonical Rust package layout;
- refuses an unrelated MCP entry or Skill target;
- validates trusted source ancestors and the complete Skill tree;
- migrates a secure version-1 bridge config to version 2 only after every former host alias is present in OpenSSH, with exact rollback on a later install failure;
- serializes bridge-managed install/uninstall transactions with a private user lock;
- journals mutations and compensates a partially successful Claude Code CLI call;
- stores a private content-hashed installation identity;
- is dry-run unless `--apply` is explicit.

Uninstall follows the same rule:

```bash
./bin/cc-ssh-bridge uninstall --user
./bin/cc-ssh-bridge uninstall --user --apply
```

Start a new Claude Code session after installing or updating so the Skill and MCP surface are reloaded. The user running the bridge is the local installation trust boundary: another process running as that same Unix user can bypass the bridge and edit Claude Code configuration directly.

Keep an installed bundle at a durable, versioned, private path such as `~/.local/share/cc-ssh-bridge/0.1.0`; the MCP entry and Skill symlink intentionally point back to that reviewed bundle. For an update, do not overwrite the active bundle in place: run its recorded `uninstall --user --apply`, stage the new version in a new directory, review the new dry run, then apply it. The content-hashed identity deliberately rejects an overwritten or unrelated bundle instead of guessing that it is a safe upgrade.

For a direct MCP entry, Claude Code can prompt only for tools not marked read-only:

```toml
[mcp_servers.ssh-bridge]
default_tools_approval_mode = "writes"
```

## Agent workflow

Invoke the Skill explicitly when useful:

```text
Use $remote-ssh-ops to inspect the devbox repository, patch the timeout bug, and run its focused tests.
Use $remote-ssh-ops to search devbox logs without downloading unbounded output.
```

The fifteen MCP tools are:

| Read-oriented | Mutation/command |
|---|---|
| `remote_hosts`, `remote_list`, `remote_stat`, `remote_search`, `remote_read`, `remote_output_read`, `remote_job_status`, `remote_job_logs`, `remote_job_list` | `remote_apply_patch`, `remote_write`, `remote_run`, `remote_job_start`, `remote_job_cancel`, `remote_job_delete` |

The default flow is bounded search/read → Claude Code or unified patch → remote verification. `remote_run` is synchronous; durable remote jobs use an opaque job ID. Oversized synchronous detail is retained under an opaque `output_ref` and paged with `remote_output_read`, so the Agent never needs to reconstruct transport logic.

All MCP file paths and `remote_run.cwd` are absolute remote paths. `remote_apply_patch` accepts Claude Code's native `*** Begin Patch` Add/Update/Delete envelope or standard unified diff. File paths must be absolute (or `/dev/null` in unified create/delete headers), and `*** Move to` is unsupported.

The bridge never infers a remote working directory from SSH home, a host profile, a previous call, or the current task.

Use `remote_job_start` for a long-running service, training run, download, or other durable work. Its runner and records survive the initiating MCP call, bridge disconnect, and local Claude Code restart, but are not automatically restarted after a remote reboot. Preserve the returned `job_id`; inspect status or recent jobs before retrying an interrupted start or control call.

`remote_run` accepts one command string plus `shell: bash|sh|login`; omitting `shell` means Bash. Prefer POSIX syntax and request `sh` explicitly when Bash is unavailable. A Bash request fails closed instead of silently changing command meaning. `login` resolves the account shell from NSS or `/etc/passwd`, never from `$SHELL`, and fails closed when it cannot do so safely. Always inspect the returned actual shell, warnings, exit status, truncation, and process-continuation uncertainty.

Writes and patches use a bounded per-process in-memory write-back cache.
Complete cached reads and consecutive edits are local-memory operations after
the first guarded snapshot. Dirty edits synchronize within 30 seconds, after
16 KiB of edit payload, before commands and filesystem-wide observations, or
on clean MCP shutdown. An SSH interruption or abnormal bridge exit can lose an
unsynchronized edit; a failed synchronization prevents its barrier operation
from starting.

Operational requests are multiplexed over one persistent SSH session per alias without bridge-defined host or concurrency admission limits. Each request has an independent process group and cancellation. Buffered edits and filesystem barriers coordinate same-host visibility, but otherwise concurrent calls have no general ordering guarantee. If cancellation cannot be confirmed, the result reports that the remote process may continue rather than retrying it.

## Human direct CLI

The direct CLI accepts argv and handles shell-word encoding inside the bridge:

```bash
./bin/cc-ssh-bridge hosts list
./bin/cc-ssh-bridge run devbox --cwd . --shell bash -- git status --short
```

This is convenient for a person or a diagnostic. Model-driven work should use MCP so results remain structured and approvals follow tool annotations.

## Optional SSHFS

Mount only when a person explicitly wants local browsing:

```bash
mkdir -p /absolute/local/mountpoint
./bin/cc-ssh-bridge mount devbox /absolute/local/mountpoint --remote-path .
./bin/cc-ssh-bridge mount-status /absolute/local/mountpoint
./bin/cc-ssh-bridge unmount /absolute/local/mountpoint
```

The CLI requires a real absolute current-user-owned mountpoint, refuses nonempty directories without `--allow-nonempty`, and never enables `allow_other`. Read-only behavior must come from the remote access policy. It prints that the mount is remote and not an Agent workspace.

Use SSHFS for browsing or narrow human editing. Keep builds, Git, tests, containers, and services on the server through `remote_run`. SFTP/FUSE workloads add a round trip to many metadata operations; caching, permissions, hardlinks, rename behavior, and broken-connection recovery also differ from a native filesystem. See the [SSHFS documentation](https://github.com/libfuse/sshfs).

## Security and performance

The bridge forces non-interactive authentication, strict host keys, no agent/X11/port forwarding, no local command, no TTY, bounded connection time, `ServerAliveInterval=15`, `ServerAliveCountMax=3`, and a private hashed ControlMaster socket for ordinary SSH and SSHFS. It never accepts arbitrary SSH options from MCP. Remote output remains untrusted and remote Unix permissions are the hard isolation boundary.

Read [docs/security.md](docs/security.md) for the complete trust model and flags. Read [docs/performance.md](docs/performance.md) for reproducible commands and raw measurements.

This bridge keeps identity and runtime local — no remote installation, authentication, or API keys on the server.
