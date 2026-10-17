use assert_cmd::Command;
use predicates::str::contains;

fn sshrek() -> Command {
    Command::cargo_bin("sshrek").expect("the sshrek binary is built")
}

#[test]
fn help_mentions_the_name_and_the_dash_shortcut() {
    sshrek()
        .arg("--help")
        .assert()
        .success()
        .stdout(contains("sshrek"))
        .stdout(contains("HOST"));
}
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Env {
    dir: TempDir,
    config: PathBuf,
}

fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn env() -> Env {
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("ssh_config");
    std::fs::copy("tests/fixtures/ssh_config_basic", &config).unwrap();
    Env { dir, config }
}

impl Env {
    fn cmd(&self) -> Command {
        let mut c = sshrek();
        c.env("SSHREK_SSH_CONFIG", &self.config)
            .env("SSHREK_STATE_DIR", self.dir.path().join("state"))
            .env(
                "SSHREK_SSH_BIN",
                script(self.dir.path(), "fake-ssh", "exit 0"),
            );
        c
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("state").join("sessions.jsonl"))
            .unwrap_or_default()
    }
}

#[test]
fn connecting_by_name_records_a_session() {
    let e = env();
    e.cmd().arg("potato").assert().success();

    let log = e.log();
    assert!(log.contains(r#""event":"start""#), "{log}");
    assert!(log.contains(r#""host":"potato""#), "{log}");
    assert!(log.contains(r#""event":"end""#), "{log}");
}

#[test]
fn exit_code_of_ssh_is_propagated() {
    let e = env();
    e.cmd()
        .env(
            "SSHREK_SSH_BIN",
            script(e.dir.path(), "failing-ssh", "exit 42"),
        )
        .arg("potato")
        .assert()
        .code(42);
}

#[test]
fn dash_connects_to_the_last_host() {
    let e = env();
    e.cmd().arg("rpi5").assert().success();
    let probe = script(e.dir.path(), "echo-ssh", "echo \"ssh:$1\"");

    e.cmd()
        .env("SSHREK_SSH_BIN", probe)
        .arg("-")
        .assert()
        .success()
        .stdout(contains("ssh:rpi5"));
}

#[test]
fn dash_without_history_explains_itself() {
    let e = env();
    e.cmd()
        .arg("-")
        .assert()
        .code(1)
        .stderr(contains("no history yet"));
}

#[test]
fn picker_selection_leads_to_a_connection() {
    let e = env();
    let picker = script(
        e.dir.path(),
        "pick-rpi5",
        "cat > /dev/null; echo 'rpi5  never  0×  pi@192.168.1.5'",
    );
    let probe = script(e.dir.path(), "echo-ssh", "echo \"ssh:$1\"");

    e.cmd()
        .env("SSHREK_PICKER", picker)
        .env("SSHREK_SSH_BIN", probe)
        .assert()
        .success()
        .stdout(contains("ssh:rpi5"));
}

#[test]
fn cancelled_picker_exits_130_silently_without_touching_the_log() {
    let e = env();
    let picker = script(e.dir.path(), "cancel", "cat > /dev/null; exit 130");

    e.cmd()
        .env("SSHREK_PICKER", picker)
        .assert()
        .code(130)
        .stdout(predicates::str::is_empty());

    assert_eq!(e.log(), "", "a cancelled pick writes no history");
}

#[test]
fn extra_args_after_double_dash_reach_ssh() {
    let e = env();
    let out = e.dir.path().join("argv");
    let probe = script(
        e.dir.path(),
        "argv-ssh",
        &format!("printf '%s\\n' \"$@\" > {}", out.display()),
    );

    e.cmd()
        .env("SSHREK_SSH_BIN", probe)
        .args(["potato", "--", "-L", "8080:localhost:80"])
        .assert()
        .success();

    let argv = std::fs::read_to_string(&out).unwrap();
    assert_eq!(
        argv.lines().collect::<Vec<_>>(),
        vec!["potato", "-L", "8080:localhost:80"]
    );
}

#[test]
fn ls_lists_configured_hosts_with_a_header() {
    let e = env();
    e.cmd()
        .arg("ls")
        .assert()
        .success()
        .stdout(contains("HOST"))
        .stdout(contains("potato"))
        .stdout(contains("never"));
}

#[test]
fn ls_porcelain_is_tab_separated_and_sortable() {
    let e = env();
    e.cmd().arg("rpi5").assert().success();

    let out = e
        .cmd()
        .args(["ls", "--porcelain", "--sort=recent"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let first = text.lines().next().unwrap();

    assert!(first.contains('\t'), "{first}");
    assert_eq!(
        first.split('\t').next(),
        Some("rpi5"),
        "the freshest host comes first"
    );
    assert!(!text.contains("HOST"), "porcelain carries no header");
}

#[test]
fn picker_format_is_aligned_and_starts_with_the_alias() {
    let e = env();
    let out = e
        .cmd()
        .args(["ls", "--porcelain", "--picker", "--sort=config"])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let first = text.lines().next().unwrap();

    assert!(
        !first.contains('\t'),
        "picker rows are padded with spaces: {first}"
    );
    assert_eq!(first.split_whitespace().next(), Some("usa"), "{first}");
}

#[test]
fn ls_on_empty_history_still_works() {
    let e = env();
    e.cmd().arg("ls").assert().success();
}

#[test]
fn preview_prints_a_card_for_one_host() {
    let e = env();
    e.cmd()
        .args(["preview", "rpi5"])
        .assert()
        .success()
        .stdout(contains("rpi5"))
        .stdout(contains("pi@192.168.1.5:2222"));
}

// Review Focus 3
#[test]
fn recent_and_frequent_flags_conflict() {
    let e = env();
    e.cmd()
        .args(["ls", "-r", "-f"])
        .assert()
        .failure()
        .stderr(contains("cannot be used with"));
}

// Review Focus 5
#[test]
fn broken_state_dir_warns_but_still_connects() {
    let e = env();
    let busy = e.dir.path().join("busy");
    std::fs::write(&busy, "i am a file, not a directory").unwrap();

    e.cmd()
        .env("SSHREK_STATE_DIR", &busy)
        .arg("potato")
        .assert()
        .success()
        .stderr(contains("history not written"));
}

#[test]
fn completions_are_generated_for_zsh() {
    let e = env();
    e.cmd()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(contains("#compdef sshrek"));
}

#[test]
fn host_completion_offers_hosts_not_files() {
    let e = env();
    let out = e.cmd().args(["completions", "zsh"]).output().unwrap();
    let script = String::from_utf8(out.stdout).unwrap();

    let host_lines: Vec<&str> = script
        .lines()
        .filter(|l| l.contains(":host") && !l.contains("SSH_ARGS"))
        .collect();
    assert!(
        !host_lines.is_empty(),
        "the host argument is present in the script"
    );
    for line in host_lines {
        assert!(
            !line.contains("_default") && !line.contains("_files"),
            "host must not be completed with file names: {line}"
        );
    }
}

#[test]
fn unreadable_log_is_reported_not_mistaken_for_an_empty_history() {
    let e = env();
    let state = e.dir.path().join("state");
    std::fs::create_dir_all(&state).unwrap();
    // A directory in place of the file: the log exists but cannot be read.
    std::fs::create_dir(state.join("sessions.jsonl")).unwrap();

    e.cmd()
        .arg("-")
        .assert()
        .code(1)
        .stderr(contains("could not read the history"));
}

#[test]
fn piping_a_long_listing_into_head_does_not_panic() {
    let e = env();
    // The output is deliberately larger than the pipe buffer so head closes it mid-sentence.
    let many: String = (0..5000).map(|i| format!("Host h{i}\n")).collect();
    let big = e.dir.path().join("big_config");
    std::fs::write(&big, many).unwrap();

    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "{} ls --porcelain | head -1",
            assert_cmd::cargo::cargo_bin("sshrek").display()
        ))
        .env("SSHREK_SSH_CONFIG", &big)
        .env("SSHREK_STATE_DIR", e.dir.path().join("state"))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("panicked"),
        "a panic instead of a quiet exit: {stderr}"
    );
    assert!(!stderr.contains("Broken pipe"), "{stderr}");
}
