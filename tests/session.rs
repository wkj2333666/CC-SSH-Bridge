#![deny(unsafe_code)]

mod support;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

use cc_ssh_bridge::ErrorCode;
use cc_ssh_bridge::capability::ShellRequest;
use cc_ssh_bridge::output::OutputStore;
use cc_ssh_bridge::ssh::{RunRequest, RuntimePaths, SshRunner};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

use support::config_with_host;

fn session_runner(base: &TempDir, log: &std::path::Path) -> Arc<SshRunner> {
    let runtime = RuntimePaths::ensure_from_base(base.path()).unwrap();
    let store = Arc::new(OutputStore::new(&runtime).unwrap());
    let environment = BTreeMap::from([
        (
            OsString::from("FAKE_SSH_MODE"),
            OsString::from("local-fixed"),
        ),
        (OsString::from("FAKE_SSH_ROOT"), OsString::from("/")),
        (OsString::from("FAKE_SSH_SHELL"), OsString::from("sh")),
        (OsString::from("FAKE_SSH_LOG"), log.as_os_str().to_owned()),
    ]);
    Arc::new(
        SshRunner::with_executable(
            Arc::new(config_with_host("dev", "/tmp")),
            runtime,
            store,
            support::fake_ssh_path(),
            environment,
        )
        .unwrap(),
    )
}

fn persistent_session_runner(
    base: &TempDir,
    log: &std::path::Path,
    install_log: &std::path::Path,
    bytes_log: &std::path::Path,
    machine_arch: &str,
    persistent_fail: bool,
) -> Arc<SshRunner> {
    let runtime = RuntimePaths::ensure_from_base(base.path()).unwrap();
    let store = Arc::new(OutputStore::new(&runtime).unwrap());
    let remote_home = base.path().join("remote-home");
    fs::create_dir_all(&remote_home).unwrap();
    let environment = BTreeMap::from([
        (
            OsString::from("FAKE_SSH_MODE"),
            OsString::from("local-fixed"),
        ),
        (OsString::from("FAKE_SSH_ROOT"), OsString::from("/")),
        (OsString::from("FAKE_SSH_SHELL"), OsString::from("sh")),
        (
            OsString::from("FAKE_SSH_KERNEL_NAME"),
            OsString::from("Linux"),
        ),
        (
            OsString::from("FAKE_SSH_MACHINE_ARCH"),
            OsString::from(machine_arch),
        ),
        (OsString::from("HOME"), remote_home.into_os_string()),
        (OsString::from("FAKE_SSH_LOG"), log.as_os_str().to_owned()),
        (
            OsString::from("FAKE_SSH_INSTALL_LOG"),
            install_log.as_os_str().to_owned(),
        ),
        (
            OsString::from("FAKE_SSH_HELPER_BYTES_LOG"),
            bytes_log.as_os_str().to_owned(),
        ),
        (
            OsString::from("FAKE_SSH_PERSISTENT_FAIL"),
            OsString::from(if persistent_fail { "1" } else { "0" }),
        ),
    ]);
    Arc::new(
        SshRunner::with_executable(
            Arc::new(config_with_host("dev", "/tmp")),
            runtime,
            store,
            support::fake_ssh_path(),
            environment,
        )
        .unwrap(),
    )
}

fn request(command: &str) -> RunRequest {
    RunRequest {
        host: "dev".to_owned(),
        command: command.to_owned(),
        cwd: "/tmp".to_owned(),
        shell: ShellRequest::Sh,
        stdin: None,
        timeout: Duration::from_secs(5),
    }
}

#[tokio::test]
async fn one_host_reuses_one_persistent_ssh_dispatcher() {
    let base = TempDir::new().unwrap();
    let log = base.path().join("ssh.log");
    let runner = session_runner(&base, &log);
    let first = runner
        .execute(request("printf first"), CancellationToken::new())
        .await
        .unwrap();
    let second = runner
        .execute(request("printf second"), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(first.status, 0);
    assert_eq!(second.status, 0);
    assert_eq!(String::from_utf8_lossy(&first.output.stdout.head), "first");
    assert_eq!(
        String::from_utf8_lossy(&second.output.stdout.head),
        "second"
    );
    let log = fs::read_to_string(log).unwrap();
    assert_eq!(log.lines().filter(|line| *line == "S").count(), 1, "{log}");
}

#[tokio::test]
async fn independent_session_requests_complete_concurrently() {
    let base = TempDir::new().unwrap();
    let log = base.path().join("ssh.log");
    let runner = session_runner(&base, &log);
    let slow = {
        let runner = Arc::clone(&runner);
        tokio::spawn(async move {
            runner
                .execute(request("sleep 0.2; printf slow"), CancellationToken::new())
                .await
                .unwrap()
        })
    };
    let fast = {
        let runner = Arc::clone(&runner);
        tokio::spawn(async move {
            runner
                .execute(request("printf fast"), CancellationToken::new())
                .await
                .unwrap()
        })
    };
    let fast = fast.await.unwrap();
    let slow = slow.await.unwrap();
    assert_eq!(String::from_utf8_lossy(&fast.output.stdout.head), "fast");
    assert_eq!(String::from_utf8_lossy(&slow.output.stdout.head), "slow");
}

#[tokio::test]
async fn session_preserves_large_binary_stdin_across_pipe_short_reads() {
    let base = TempDir::new().unwrap();
    let log = base.path().join("ssh.log");
    let runner = session_runner(&base, &log);
    let stdin = vec![0xA5; 512 * 1024 + 123];
    let mut request = request("wc -c");
    request.stdin = Some(stdin.clone());
    let result = runner
        .execute(request, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.status, 0);
    assert_eq!(
        String::from_utf8_lossy(&result.output.stdout.head),
        format!("{}\n", stdin.len())
    );
}

fn install_test_helper() -> Option<(&'static str, &'static str, std::path::PathBuf)> {
    let helper_source = std::env::var("CARGO_BIN_EXE_cc-ssh-bridge-helper")
        .or_else(|_| std::env::var("CARGO_BIN_EXE_cc_ssh_bridge_helper"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("target/debug/cc-ssh-bridge-helper")
        });
    if !helper_source.is_file() {
        eprintln!("helper integration binary is not available; skipping");
        return None;
    }
    let (machine_arch, target) = match std::env::consts::ARCH {
        "x86_64" => ("x86_64", "x86_64-unknown-linux-musl"),
        "aarch64" => ("aarch64", "aarch64-unknown-linux-musl"),
        "arm" => ("armv7l", "armv7-unknown-linux-musleabihf"),
        _ => {
            eprintln!("unsupported test architecture; skipping");
            return None;
        }
    };
    let test_binary_parent = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let helper_directory = test_binary_parent.join("remote-helpers");
    fs::create_dir_all(&helper_directory).unwrap();
    let helper_path = helper_directory.join(target);
    fs::copy(&helper_source, &helper_path).unwrap();
    fs::set_permissions(&helper_path, fs::Permissions::from_mode(0o700)).unwrap();
    Some((machine_arch, target, helper_path))
}

fn remove_test_helper(helper_path: &std::path::Path) {
    let helper_directory = helper_path.parent().unwrap();
    let _ = fs::remove_file(helper_path);
    let _ = fs::remove_dir(helper_directory);
}

#[tokio::test]
async fn persistent_helper_installs_once_and_reuses_after_bridge_restart() {
    let Some((machine_arch, target, helper_path)) = install_test_helper() else {
        return;
    };

    let base = TempDir::new().unwrap();
    let log = base.path().join("ssh.log");
    let install_log = base.path().join("install.log");
    let bytes_log = base.path().join("bytes.log");
    let runner =
        persistent_session_runner(&base, &log, &install_log, &bytes_log, machine_arch, false);
    let mut first_request = request("printf helper-first");
    first_request.timeout = Duration::from_secs(30);
    let mut concurrent_request = request("printf helper-concurrent");
    concurrent_request.timeout = Duration::from_secs(30);
    let (first, concurrent) = tokio::join!(
        runner.execute(first_request, CancellationToken::new()),
        runner.execute(concurrent_request, CancellationToken::new()),
    );
    let first = first.unwrap();
    let concurrent = concurrent.unwrap();
    assert_eq!(
        first.helper_mode,
        cc_ssh_bridge::ssh::HelperMode::Persistent
    );
    assert_eq!(
        String::from_utf8_lossy(&first.output.stdout.head),
        "helper-first"
    );
    assert_eq!(
        concurrent.helper_mode,
        cc_ssh_bridge::ssh::HelperMode::Persistent
    );
    assert_eq!(
        String::from_utf8_lossy(&concurrent.output.stdout.head),
        "helper-concurrent"
    );

    let mut timed_request = request("sleep 30");
    timed_request.timeout = Duration::from_millis(80);
    let timed_out = runner
        .execute(timed_request, CancellationToken::new())
        .await
        .expect_err("the persistent helper must report its watchdog timeout");
    assert_eq!(timed_out.code, ErrorCode::CommandTimeout);
    assert_eq!(
        timed_out.details.remote_process_may_continue,
        Some(false),
        "{timed_out:?}"
    );

    let after_timeout = runner
        .execute(
            request("printf persistent-after-timeout"),
            CancellationToken::new(),
        )
        .await
        .expect("a timed-out request must not poison the shared session");
    assert_eq!(
        String::from_utf8_lossy(&after_timeout.output.stdout.head),
        "persistent-after-timeout"
    );
    assert_eq!(
        after_timeout.helper_mode,
        cc_ssh_bridge::ssh::HelperMode::Persistent
    );
    let install_events = fs::read_to_string(&install_log).unwrap();
    let install_events = install_events.lines().collect::<Vec<_>>();
    assert_eq!(install_events.len(), 2, "{install_events:?}");
    assert_eq!(
        install_events
            .iter()
            .filter(|event| **event == "NEED")
            .count(),
        1,
        "{install_events:?}"
    );
    assert_eq!(
        install_events
            .iter()
            .filter(|event| **event == "HIT")
            .count(),
        1,
        "{install_events:?}"
    );
    let uploaded = fs::read_to_string(&bytes_log)
        .unwrap()
        .lines()
        .map(|line| line.parse::<u64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(uploaded.len(), 2, "{uploaded:?}");
    assert_eq!(uploaded.iter().filter(|bytes| **bytes == 0).count(), 1);
    assert_eq!(uploaded.iter().filter(|bytes| **bytes > 0).count(), 1);
    drop(runner);

    let restart_log = base.path().join("restart-ssh.log");
    let restart_install_log = base.path().join("restart-install.log");
    let restart_bytes_log = base.path().join("restart-bytes.log");
    let restarted = persistent_session_runner(
        &base,
        &restart_log,
        &restart_install_log,
        &restart_bytes_log,
        machine_arch,
        false,
    );
    let mut second_request = request("printf helper-second");
    second_request.timeout = Duration::from_secs(30);
    let second = restarted
        .execute(second_request, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(
        second.helper_mode,
        cc_ssh_bridge::ssh::HelperMode::Persistent
    );
    assert_eq!(
        String::from_utf8_lossy(&second.output.stdout.head),
        "helper-second"
    );
    assert_eq!(fs::read_to_string(restart_install_log).unwrap(), "HIT\n");
    assert_eq!(fs::read_to_string(restart_bytes_log).unwrap(), "0\n");
    let installed = base
        .path()
        .join("remote-home/.local/share/cc-ssh-bridge/helpers")
        .join(env!("CARGO_PKG_VERSION"))
        .join(target)
        .join("helper");
    assert_eq!(
        fs::metadata(installed).unwrap().permissions().mode() & 0o777,
        0o700
    );
    drop(restarted);
    remove_test_helper(&helper_path);
}

#[tokio::test]
async fn persistent_startup_failure_falls_back_to_temporary_helper() {
    let Some((machine_arch, _target, helper_path)) = install_test_helper() else {
        return;
    };
    let base = TempDir::new().unwrap();
    let log = base.path().join("ssh.log");
    let install_log = base.path().join("install.log");
    let bytes_log = base.path().join("bytes.log");
    let runner =
        persistent_session_runner(&base, &log, &install_log, &bytes_log, machine_arch, true);
    let mut run = request("printf temporary-fallback");
    run.timeout = Duration::from_secs(30);
    let result = runner.execute(run, CancellationToken::new()).await.unwrap();
    assert_eq!(
        result.helper_mode,
        cc_ssh_bridge::ssh::HelperMode::Temporary
    );
    assert_eq!(
        String::from_utf8_lossy(&result.output.stdout.head),
        "temporary-fallback"
    );
    let log_text = fs::read_to_string(log).unwrap();
    assert_eq!(
        log_text.lines().filter(|line| *line == "S").count(),
        2,
        "{log_text}"
    );
    drop(runner);
    remove_test_helper(&helper_path);
}

#[tokio::test]
async fn unsupported_helper_architecture_uses_shell_mode() {
    let base = TempDir::new().unwrap();
    let log = base.path().join("ssh.log");
    let install_log = base.path().join("install.log");
    let bytes_log = base.path().join("bytes.log");
    let runner = persistent_session_runner(&base, &log, &install_log, &bytes_log, "mips64", false);
    let result = runner
        .execute(request("printf shell-only"), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(result.helper_mode, cc_ssh_bridge::ssh::HelperMode::Shell);
    assert_eq!(
        String::from_utf8_lossy(&result.output.stdout.head),
        "shell-only"
    );
    let log_text = fs::read_to_string(log).unwrap();
    assert_eq!(
        log_text.lines().filter(|line| *line == "S").count(),
        1,
        "{log_text}"
    );
}
