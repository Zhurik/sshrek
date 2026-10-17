use anyhow::{bail, Result};
use clap_complete::Shell;

pub const DEFAULT_ALIAS: &str = "cssh";

/// A name that is safe to paste into a shell as a command word. The snippet is
/// meant to be `eval`-ed, so a name with spaces or metacharacters would turn
/// into something other than an alias definition.
pub fn check_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let head_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    let tail_ok = chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if head_ok && tail_ok {
        return Ok(());
    }
    bail!("'{name}' is not a usable command name: start with a letter or '_', then letters, digits, '_', '-' or '.'")
}

/// The alias definition for one shell. The completion script, generated under
/// the same name, is appended by the caller.
///
/// bash and zsh get a function rather than an `alias`: bash expands no aliases
/// at all in a non-interactive shell, and both expand them when a command is
/// parsed, not when it runs — so an alias defined by this snippet would be
/// invisible to the rest of the same script. A function has neither limit, and
/// `complete`/`compdef` bind to it just the same. fish's own `alias` already
/// defines a function, so there it is both idiomatic and equivalent.
pub fn definition(name: &str, shell: Shell, self_exe: &str) -> Result<String> {
    check_name(name)?;
    let exe = shell_words::quote(self_exe);
    Ok(match shell {
        Shell::Bash | Shell::Zsh => format!("{name}() {{ {exe} \"$@\"; }}\n"),
        Shell::Fish => format!("alias {name} {exe}\n"),
        other => bail!(
            "no alias syntax known for {other}; register completions under the name instead: sshrek completions <shell> --name {name}"
        ),
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bash_and_zsh_get_a_function_that_forwards_every_argument() {
        for shell in [Shell::Bash, Shell::Zsh] {
            let out = definition("cssh", shell, "/usr/local/bin/sshrek").unwrap();
            assert_eq!(out, "cssh() { /usr/local/bin/sshrek \"$@\"; }\n", "{shell}");
        }
    }

    #[test]
    fn fish_gets_the_spaced_form() {
        let out = definition("cssh", Shell::Fish, "/usr/local/bin/sshrek").unwrap();
        assert_eq!(out, "alias cssh /usr/local/bin/sshrek\n");
    }

    #[test]
    fn a_path_with_spaces_is_quoted() {
        let out = definition("cssh", Shell::Zsh, "/opt/my tools/sshrek").unwrap();
        assert_eq!(out, "cssh() { '/opt/my tools/sshrek' \"$@\"; }\n");
    }

    #[test]
    fn shells_without_a_known_alias_syntax_point_at_completions() {
        let err = definition("cssh", Shell::Elvish, "/bin/sshrek")
            .unwrap_err()
            .to_string();
        assert!(err.contains("elvish"), "{err}");
        assert!(err.contains("--name cssh"), "the way out is named: {err}");
    }

    #[test]
    fn names_that_would_not_survive_eval_are_refused() {
        for bad in ["", "two words", "rm -rf /", "a;b", "$(x)", "1cssh", "-f"] {
            assert!(check_name(bad).is_err(), "accepted {bad:?}");
        }
        for good in ["cssh", "s", "_s", "ssh.old", "my-ssh", "c2"] {
            assert!(check_name(good).is_ok(), "rejected {good:?}");
        }
    }

    #[test]
    fn the_default_alias_is_a_usable_name() {
        assert!(check_name(DEFAULT_ALIAS).is_ok());
    }
}
