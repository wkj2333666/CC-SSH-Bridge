use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

const EXPECTED_TOOLS: [&str; 18] = [
    "remote_hosts",
    "remote_list",
    "remote_stat",
    "remote_search",
    "remote_read",
    "remote_output_read",
    "remote_edit_status",
    "remote_sync_edits",
    "remote_discard_edits",
    "remote_apply_patch",
    "remote_write",
    "remote_run",
    "remote_job_start",
    "remote_job_status",
    "remote_job_logs",
    "remote_job_cancel",
    "remote_job_list",
    "remote_job_delete",
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_text(relative_path: impl AsRef<Path>) -> String {
    let path = repository_root().join(relative_path);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn read_json(relative_path: impl AsRef<Path>) -> Value {
    let relative_path = relative_path.as_ref();
    let text = read_text(relative_path);
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", relative_path.display()))
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        files.push(path.to_owned());
        return;
    }

    let mut entries: Vec<_> = fs::read_dir(path)
        .unwrap_or_else(|error| panic!("failed to list {}: {error}", path.display()))
        .map(|entry| entry.expect("failed to read directory entry").path())
        .collect();
    entries.sort();

    for entry in entries {
        collect_files(&entry, files);
    }
}

fn identifier_tokens(text: &str) -> BTreeSet<&str> {
    text.split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
        .filter(|token| !token.is_empty())
        .collect()
}

fn section<'a>(document: &'a str, heading: &str) -> &'a str {
    let start = document
        .find(heading)
        .unwrap_or_else(|| panic!("missing required Skill section {heading:?}"));
    let body = &document[start + heading.len()..];
    let end = body.find("\n## ").unwrap_or(body.len());
    &body[..end]
}

#[test]
fn mcp_manifest_launches_the_packaged_rust_binary() {
    let manifest = read_json(".mcp.json");
    let servers = manifest
        .get("mcpServers")
        .and_then(Value::as_object)
        .expect(".mcp.json must contain an mcpServers object");
    assert_eq!(
        servers.len(),
        1,
        "the plugin must install exactly one MCP server"
    );

    let server = servers
        .get("ssh-bridge")
        .expect("the single MCP server must be named ssh-bridge");
    assert_eq!(server.get("command"), Some(&json!("./bin/cc-ssh-bridge")));
    assert_eq!(server.get("args"), Some(&json!(["mcp"])));
}

#[test]
fn example_config_is_v2_limits_only() {
    let example = read_text("config.example.toml");
    assert!(example.contains("version = 2"));
    for forbidden in [
        "[hosts",
        "root =",
        "description =",
        "read_only",
        "global_concurrency",
        "per_host_concurrency",
    ] {
        assert!(
            !example.contains(forbidden),
            "example retains removed field {forbidden}"
        );
    }
}

#[test]
fn release_workflow_builds_and_packages_all_common_targets() {
    let workflow = read_text(".github/workflows/release.yml");
    for main_target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "armv7-unknown-linux-gnueabihf",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "riscv64gc-unknown-linux-gnu",
        "powerpc64le-unknown-linux-gnu",
        "s390x-unknown-linux-gnu",
    ] {
        assert!(
            workflow.contains(main_target),
            "release workflow omits {main_target}"
        );
    }
    for helper_target in [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "armv7-unknown-linux-musleabihf",
        "riscv64gc-unknown-linux-gnu",
        "powerpc64le-unknown-linux-gnu",
        "s390x-unknown-linux-gnu",
    ] {
        assert!(
            workflow.contains(helper_target),
            "release workflow omits {helper_target}"
        );
    }
    assert!(workflow.contains("name: helper-${{ matrix.target }}"));
    assert!(workflow.contains("remote-helpers/$helper"));
    assert!(workflow.contains("--bin cc-ssh-bridge-helper"));
    assert!(workflow.contains("statically linked|musl"));
    assert!(workflow.contains("find release-assets -maxdepth 1 -type f"));
    for packaged_resource in [
        "$root/bin/cc-ssh-bridge",
        ".claude-plugin",
        "skills",
        "README.md",
        "LICENSE",
        "config.example.toml",
        ".mcp.json",
        "docs/security.md",
        "docs/performance.md",
    ] {
        assert!(
            workflow.contains(packaged_resource),
            "release archive omits {packaged_resource}"
        );
    }
    assert!(workflow.contains("Check out tagged source for package resources"));
    assert!(workflow.contains("(cd dist && sha256sum"));
}

#[test]
fn cross_architecture_workflow_builds_every_release_binary() {
    let workflow = read_text(".github/workflows/cross-architecture.yml");
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "armv7-unknown-linux-gnueabihf",
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "armv7-unknown-linux-musleabihf",
        "riscv64gc-unknown-linux-gnu",
        "powerpc64le-unknown-linux-gnu",
        "s390x-unknown-linux-gnu",
    ] {
        assert!(workflow.contains(target), "cross workflow omits {target}");
    }
    assert_eq!(workflow.matches("--bin cc-ssh-bridge\n").count(), 3);
    assert_eq!(
        workflow
            .matches("--bin cc-ssh-bridge --bin cc-ssh-bridge-helper")
            .count(),
        5
    );
    assert_eq!(
        workflow.matches("bins: --bin cc-ssh-bridge-helper").count(),
        1
    );
    assert!(workflow.contains("workflow_dispatch:"));
}

#[test]
fn installed_chain_has_no_python_runtime_or_legacy_module_references() {
    let root = repository_root();
    let mut files = Vec::new();
    collect_files(&root.join(".claude-plugin"), &mut files);
    files.push(root.join(".mcp.json"));
    collect_files(&root.join("skills"), &mut files);
    files.push(root.join("README.md"));
    files.sort();
    files.dedup();

    let forbidden = ["python3", "server.py", "ssh_bridge"];
    let mut violations = Vec::new();
    for path in files {
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        for needle in forbidden {
            if text.contains(needle) {
                let relative = path.strip_prefix(&root).unwrap_or(&path);
                violations.push(format!("{} references {needle:?}", relative.display()));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "installed plugin chain still references the Python/legacy runtime:\n{}",
        violations.join("\n")
    );
}

#[test]
fn skill_names_exactly_the_public_remote_tools() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md");
    let identifiers = identifier_tokens(&skill);
    let actual_remote_tools: BTreeSet<_> = identifiers
        .iter()
        .copied()
        .filter(|token| token.starts_with("remote_"))
        .collect();
    let expected_remote_tools: BTreeSet<_> = EXPECTED_TOOLS.into_iter().collect();

    assert_eq!(
        actual_remote_tools, expected_remote_tools,
        "the Skill must name exactly the public MCP tool set"
    );
}

#[test]
fn skill_names_no_legacy_ssh_tools() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md");
    let identifiers = identifier_tokens(&skill);
    let legacy_tools: Vec<_> = identifiers
        .iter()
        .copied()
        .filter(|token| token.starts_with("ssh_"))
        .collect();
    assert!(
        legacy_tools.is_empty(),
        "the Skill still names legacy ssh_ tools: {legacy_tools:?}"
    );
}

#[test]
fn skill_exposes_no_sshfs_mcp_tool() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md");
    let identifiers = identifier_tokens(&skill);
    let sshfs_mcp_tools: Vec<_> = identifiers
        .iter()
        .copied()
        .filter(|token| {
            token.starts_with("remote_")
                && (token.contains("sshfs")
                    || token.ends_with("_mount")
                    || token.ends_with("_unmount"))
        })
        .collect();
    assert!(
        sshfs_mcp_tools.is_empty(),
        "SSHFS must remain a CLI workflow, not an MCP tool: {sshfs_mcp_tools:?}"
    );
}

#[test]
fn skill_teaches_the_low_burden_default_workflow_in_order() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md");
    let workflow = section(&skill, "## Default workflow");
    let search = workflow
        .find("remote_search")
        .expect("default workflow must start from bounded remote search");
    let read = workflow
        .find("remote_read")
        .expect("default workflow must read before changing files");
    let patch = workflow
        .find("remote_apply_patch")
        .expect("default workflow must prefer remote_apply_patch");
    let run = workflow
        .find("remote_run")
        .expect("default workflow must verify with remote_run");
    assert!(search < read && read < patch && patch < run);
}

#[test]
fn skill_states_remote_shell_output_and_sshfs_boundaries() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md").to_ascii_lowercase();
    for required in [
        "every path",
        "untrusted",
        "actual shell",
        "posix",
        "bash-only",
        "fallback",
        "human-only",
        "not an agent workspace",
    ] {
        assert!(
            skill.contains(required),
            "Skill omits required boundary phrase {required:?}"
        );
    }
}

#[test]
fn skill_closes_search_stdin_and_patch_schema_ambiguities() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md").to_ascii_lowercase();
    for required in [
        "case-sensitive literal",
        "stdin is an object",
        "encoding",
        "value",
        "absolute remote path",
    ] {
        assert!(
            skill.contains(required),
            "Skill omits schema clarification {required:?}"
        );
    }
}

#[test]
fn packaged_skill_and_reference_describe_both_patch_formats() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md");
    let operations = read_text("skills/remote-ssh-ops/references/operations.md");
    let combined = format!("{skill}\n{operations}");

    for required in [
        "Claude Code",
        "*** Begin Patch",
        "standard unified diff",
        "absolute paths",
        "*** Move to",
        "unsupported",
    ] {
        assert!(
            combined.contains(required),
            "packaged patch documentation omits {required:?}"
        );
    }
}

#[test]
fn skill_and_reference_teach_the_durable_remote_job_boundary() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md").to_ascii_lowercase();
    let operations =
        read_text("skills/remote-ssh-ops/references/operations.md").to_ascii_lowercase();
    for document in [&skill, &operations] {
        for tool in [
            "remote_job_start",
            "remote_job_status",
            "remote_job_logs",
            "remote_job_cancel",
            "remote_job_list",
            "remote_job_delete",
        ] {
            assert!(document.contains(tool), "job reference omits {tool}");
        }
        for boundary in [
            "`remote_run` remains synchronous",
            "claude code task",
            "bridge disconnect",
            "survives",
            "never submit the command again blindly",
            "no automatic restart after a remote reboot",
        ] {
            assert!(
                document.contains(boundary),
                "job reference omits lifecycle boundary {boundary:?}"
            );
        }
    }
}

#[test]
fn public_docs_state_remote_job_storage_security_and_retention() {
    let docs = [
        read_text("README.md"),
        read_text("docs/security.md"),
        read_text("docs/performance.md"),
    ]
    .join("\n")
    .to_ascii_lowercase();
    for required in [
        ".local/state/cc-ssh-bridge/jobs",
        "0700",
        "0600",
        "no-follow",
        "process start",
        "64 mib",
        "seven-day",
        "lazy retention",
        "persistent binary helper",
        "no automatic restart after a remote reboot",
    ] {
        assert!(
            docs.contains(required),
            "public job documentation omits {required:?}"
        );
    }
}

#[test]
fn skill_states_buffered_edit_durability_without_burdening_the_agent() {
    let skill = read_text("skills/remote-ssh-ops/SKILL.md")
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "bounded in-memory edit cache",
        "within 30 seconds",
        "16 kib",
        "clean mcp shutdown",
        "buffered write may fail",
        "do not manage generations",
        "remote_edit_status",
        "remote_sync_edits",
        "remote_discard_edits",
        "normal editing",
    ] {
        assert!(
            skill.contains(required),
            "Skill omits buffered-edit boundary phrase {required:?}"
        );
    }
}
