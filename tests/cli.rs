use assert_cmd::Command;
use predicates::str::contains;

fn sshrek() -> Command {
    Command::cargo_bin("sshrek").expect("бинарник sshrek собран")
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
