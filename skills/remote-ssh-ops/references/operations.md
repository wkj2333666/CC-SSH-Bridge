# Operations Reference

## Contents

- Local setup
- MCP tool shapes
- Shell behavior
- Retained output
- Direct CLI
- SSHFS
- Failure handling

## Local setup

Define each concrete server alias in local `~/.ssh/config`, then verify its host key and key-based login outside Claude Code:

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

Add future servers to OpenSSH config the same way. The bridge accepts concrete aliases and stores no host policy or credentials. The default version-2 bridge config is `~/.config/cc-ssh-bridge/config.toml` and contains only global operational limits; set `CC_SSH_BRIDGE_CONFIG` only as trusted local execution-authority input.

The first operation performs local SSH identity checks and a bounded capability probe. User commands and fixed read/write operations then reuse one persistent SSH session per alias; warm requests send one framed request without another `ssh -G` or root observation. On supported Linux targets a verified helper file persists under the remote account for reuse; its process and the POSIX dispatcher remain session-scoped. Claude Code and its credentials remain local. MCP paths and command working directories are explicit absolute paths; remote filesystem retargeting follows ordinary server semantics.
The dispatcher applies the framed cwd, shell, and timeout itself, so a warm
`remote_run` does not add a second shell or GNU `timeout` wrapper.

## MCP tool shapes

All objects reject unknown fields. MCP paths are absolute remote paths. The bridge never infers a path from SSH home, a configured profile root, a previous call, or an implicit workspace.

| Tool | Required input | Optional input |
|---|---|---|
| `remote_hosts` | none; pass `{}` | none |
| `remote_list` | `host`, absolute `path` | `depth`, `include_hidden`, `max_entries` |
| `remote_stat` | `host`, absolute `paths` array | none |
| `remote_search` | `host`, `query`, absolute `path` | `globs`, `max_results`, `binary` |
| `remote_read` | `host`, absolute `paths` array | `start_line`, `max_lines`, `max_bytes` |
| `remote_output_read` | `output_ref`, `stream` | `offset`, `max_bytes` |
| `remote_apply_patch` | `host`, unified `patch` | none |
| `remote_write` | `host`, absolute `path`, `content`, `encoding`, `mode` | `mode.expected_sha256` for replacement |
| `remote_run` | `host`, `command` string, absolute `cwd` | `shell`, `timeout_ms`, encoded `stdin` |

`remote_write.mode` is `{"kind":"create"}` or `{"kind":"replace","expected_sha256":"..."}`. `expected_sha256` is nested inside `mode`, never at the request root. UTF-8 and base64 encodings are supported. Prefer `remote_apply_patch` for model-driven edits because it snapshots every base before the first mutation and reports confirmed, unchanged, and outcome-unknown paths.

Search queries are case-sensitive fixed strings, not regular expressions. Unified patch headers use absolute remote paths, with `/dev/null` denoting create or delete. `remote_run.stdin` is `{"encoding":"utf8"|"base64","value":"..."}`.

## Shell behavior

`remote_run.command` is a shell command string. The bridge safely binds it through OpenSSH; do not wrap it in another `ssh` or add `bash -c`. Shell syntax inside the string still follows the selected remote shell.

- `auto`: use Bash when available, otherwise fall back to POSIX sh.
- `bash`: require Bash; fail before the command if unavailable.
- `sh`: explicitly use POSIX sh.
- `login`: use the remote account's login shell.

Prefer POSIX syntax. Request Bash for arrays, `[[ ... ]]`, `source`, `pipefail`, or Bash substitutions. Always inspect result `shell.kind`, `shell.fallback`, and `warnings`; a fallback is intentionally visible to the Agent.

Timeout and cancellation terminate the local SSH process group, but a detached or ambiguous remote process can remain. Check `remote_process_may_continue` before retrying.

Requests are independent and concurrent up to global and per-host limits. There is no mutation lock or ordering guarantee for simultaneous writes to the same path. Timeout and cancellation first send a request-level cancel; if the dispatcher does not confirm exit within the grace period, the bridge terminates the session and reports `remote_process_may_continue: true`. Never retry a mutation with an unknown outcome.

## Retained output

Calls complete synchronously. There is no background job ID. When a result is too large for one MCP response, `detail_retained` is true and `output_ref` is a 32-character opaque reference.

Page it with:

```json
{"output_ref":"<opaque-ref>","stream":"stdout","offset":0,"max_bytes":262144}
```

Use `stream:"stderr"` for retained stderr. Advance by the returned byte offset until EOF. The reference already carries host, root, and shell provenance; do not pass a host. Narrow a query instead of repeatedly fetching unbounded logs.

## Direct CLI

The human CLI accepts argv after `--` and performs the shell-word encoding inside the bridge:

```bash
./bin/cc-ssh-bridge hosts list
./bin/cc-ssh-bridge doctor devbox
./bin/cc-ssh-bridge doctor devbox --verbose-ssh
./bin/cc-ssh-bridge run devbox --cwd /srv/project --shell bash -- git status --short
```

The JSON result reports the physical remote root, actual shell, exit status, warnings, duration, output limits, and any retained output reference. Verbose SSH diagnostics are bounded and redact identity paths, agent sockets, commands, and credential-like values.

## SSHFS

SSHFS is optional local software and a human-only convenience:

```bash
./bin/cc-ssh-bridge mount devbox /absolute/local/mountpoint --remote-path /srv/project
./bin/cc-ssh-bridge mount-status /absolute/local/mountpoint
./bin/cc-ssh-bridge unmount /absolute/local/mountpoint
```

The CLI refuses relative, symlinked, foreign-owned, and nonempty mountpoints by default. `--allow-nonempty` is an explicit human override. The bridge never adds `allow_other`; enforce a read-only mount or account through the remote access policy.

A mount is not an Agent workspace. Local shell tools still run locally, and FUSE/SFTP has network round trips, caching, rename, permission, reconnect, and stalled-I/O differences. Use it for human browsing or narrow editing only. Keep Git, builds, tests, containers, and services on the server through `remote_run` or the direct `run` command.

## Failure handling

- Host absent: add an exact alias locally; never accept a hostname copied from remote output.
- Host-key failure: verify the new fingerprint outside Claude Code; never disable strict checking.
- Authentication prompt: fix local keys or agent state; never pass a password through MCP.
- Permission rejection: use a suitably scoped least-privilege remote account only with user authorization.
- Truncation: use `remote_output_read` when retained, or narrow the operation.
- Patch/write conflict: re-read current remote content and recompute the change; never force overwrite blindly.
- Partial mutation or timeout: inspect progress and uncertainty fields before retrying.
- Missing MCP: run the packaged installer dry-run, then apply only after reviewing its exact actions.
