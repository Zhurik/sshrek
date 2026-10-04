use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

pub const DEFAULT_PICKER: &str = "fzf";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerCmd {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub enum PickOutcome {
    Selected(String),
    Cancelled,
}

/// Флаги для fzf. `fzf-tmux` незнакомые ему аргументы передаёт в fzf,
/// поэтому `SSHREK_PICKER='fzf-tmux -p'` работает с тем же набором.
pub fn default_args(self_exe: &str) -> Vec<String> {
    let me = shell_words::quote(self_exe).to_string();
    vec![
        "--no-sort".to_string(),
        "--layout=reverse".to_string(),
        "--height=40%".to_string(),
        "--prompt".to_string(),
        "ssh > ".to_string(),
        "--preview".to_string(),
        format!("{me} preview {{1}}"),
        "--preview-window".to_string(),
        "right,45%,border-left".to_string(),
        "--header".to_string(),
        "ctrl-o config · ctrl-r recent · ctrl-f frequent".to_string(),
        "--bind".to_string(),
        format!("ctrl-o:reload({me} ls --porcelain --picker --sort=config)"),
        "--bind".to_string(),
        format!("ctrl-r:reload({me} ls --porcelain --picker --sort=recent)"),
        "--bind".to_string(),
        format!("ctrl-f:reload({me} ls --porcelain --picker --sort=frequent)"),
    ]
}

fn non_empty(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

pub fn build_command(
    picker_env: Option<&str>,
    args_env: Option<&str>,
    self_exe: &str,
) -> Result<PickerCmd> {
    let spec = non_empty(picker_env).unwrap_or(DEFAULT_PICKER);
    let mut words =
        shell_words::split(spec).context("SSHREK_PICKER не разбирается как команда шелла")?;
    if words.is_empty() {
        bail!("SSHREK_PICKER пуст");
    }
    let program = words.remove(0);
    let mut args = words;
    match non_empty(args_env) {
        Some(custom) => args.extend(
            shell_words::split(custom)
                .context("SSHREK_PICKER_ARGS не разбирается как аргументы шелла")?,
        ),
        None => args.extend(default_args(self_exe)),
    }
    Ok(PickerCmd { program, args })
}

/// Есть ли у процесса управляющий терминал.
///
/// fzf без него не выходит с ошибкой, а виснет в ожидании ввода, которого
/// не будет: запуск из cron или демона иначе накапливал бы мёртвые процессы.
pub fn has_controlling_terminal() -> bool {
    std::fs::File::open("/dev/tty").is_ok()
}

pub fn ensure_terminal(present: bool) -> Result<()> {
    if present {
        return Ok(());
    }
    bail!("для выбора хоста нужен терминал — если запускаешь не в нём, укажи хост явно: sshrek <host>")
}

pub fn run(cmd: &PickerCmd, rows: &[String]) -> Result<PickOutcome> {
    let mut child = Command::new(&cmd.program)
        .args(&cmd.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "не удалось запустить пикер '{}' (задаётся через SSHREK_PICKER)",
                cmd.program
            )
        })?;

    {
        let mut stdin = child.stdin.take().expect("stdin запрошен как piped");
        // Пикер мог закрыться раньше, чем мы дописали строки: EPIPE здесь
        // не ошибка, настоящий вердикт — код выхода.
        for row in rows {
            if writeln!(stdin, "{row}").is_err() {
                break;
            }
        }
    }

    let out = child
        .wait_with_output()
        .with_context(|| format!("пикер '{}' не завершился корректно", cmd.program))?;

    match out.status.code() {
        Some(0) => {
            let text = String::from_utf8_lossy(&out.stdout);
            let alias = text
                .lines()
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");
            if alias.is_empty() {
                Ok(PickOutcome::Cancelled)
            } else {
                Ok(PickOutcome::Selected(alias.to_string()))
            }
        }
        // 130 — Esc/Ctrl-C, 1 — ничего не нашлось: для нас это одно и то же.
        Some(130) | Some(1) => Ok(PickOutcome::Cancelled),
        Some(code) => bail!(
            "пикер '{}' завершился с кодом {code}; для выбора нужен терминал — если запускаешь не в нём, укажи хост явно: sshrek <host>",
            cmd.program
        ),
        None => Ok(PickOutcome::Cancelled),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn stub(script: &str) -> (tempfile::TempDir, PickerCmd) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("stub-picker");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let cmd = PickerCmd {
            program: path.to_string_lossy().into_owned(),
            args: vec![],
        };
        (dir, cmd)
    }

    #[test]
    fn default_picker_is_fzf() {
        let cmd = build_command(None, None, "/usr/local/bin/sshrek").unwrap();
        assert_eq!(cmd.program, "fzf");
    }

    #[test]
    fn picker_env_is_split_into_program_and_args() {
        let cmd = build_command(Some("fzf-tmux -p"), None, "/bin/sshrek").unwrap();
        assert_eq!(cmd.program, "fzf-tmux");
        assert_eq!(cmd.args[0], "-p");
        assert!(cmd.args.iter().any(|a| a == "--no-sort"));
    }

    #[test]
    fn default_args_wire_preview_and_sort_binds_to_our_own_binary() {
        let args = default_args("/opt/bin/sshrek");
        let joined = args.join(" ");
        assert!(joined.contains("/opt/bin/sshrek preview {1}"), "{joined}");
        assert!(joined.contains("ctrl-r:reload"), "{joined}");
        assert!(joined.contains("--sort=frequent"), "{joined}");
        assert!(
            args.iter().any(|a| a == "--no-sort"),
            "порядок из config сохраняется"
        );
    }

    #[test]
    fn picker_args_env_replaces_our_flags_entirely() {
        let cmd = build_command(Some("fzf"), Some("--height=100%"), "/bin/sshrek").unwrap();
        assert_eq!(cmd.args, vec!["--height=100%"]);
    }

    #[test]
    fn unparsable_picker_env_is_an_error() {
        assert!(build_command(Some("fzf 'unclosed"), None, "/bin/sshrek").is_err());
        assert!(
            build_command(Some("   "), None, "/bin/sshrek").is_ok(),
            "пустое значение = дефолт"
        );
    }

    #[test]
    fn without_a_controlling_terminal_we_refuse_before_launching_the_picker() {
        let err = ensure_terminal(false).unwrap_err().to_string();
        assert!(err.contains("терминал"), "{err}");
        assert!(err.contains("sshrek <host>"), "подсказываем выход: {err}");
        assert!(ensure_terminal(true).is_ok());
    }

    #[test]
    fn selected_alias_is_the_first_field_of_the_chosen_line() {
        let (_dir, cmd) = stub("cat > /dev/null; echo 'rpi5  2h ago  11×  pi@192.168.1.5'");
        match run(&cmd, &["rpi5  2h ago  11×  pi@192.168.1.5".to_string()]).unwrap() {
            PickOutcome::Selected(alias) => assert_eq!(alias, "rpi5"),
            PickOutcome::Cancelled => panic!("ожидали выбор"),
        }
    }

    #[test]
    fn exit_130_and_empty_output_mean_cancelled() {
        let (_dir, cancelled) = stub("cat > /dev/null; exit 130");
        assert!(matches!(
            run(&cancelled, &[]).unwrap(),
            PickOutcome::Cancelled
        ));

        let (_dir2, no_match) = stub("cat > /dev/null; exit 1");
        assert!(matches!(
            run(&no_match, &[]).unwrap(),
            PickOutcome::Cancelled
        ));
    }

    // Review Focus 2
    #[test]
    fn other_nonzero_exits_explain_the_tty_requirement() {
        let (_dir, broken) = stub("cat > /dev/null; exit 2");
        let err = run(&broken, &[]).unwrap_err().to_string();
        assert!(
            err.contains("терминал"),
            "сообщение подсказывает про TTY: {err}"
        );
        assert!(err.contains("sshrek <host>"), "и предлагает выход: {err}");
    }

    #[test]
    fn missing_picker_binary_names_the_env_var() {
        let cmd = PickerCmd {
            program: "/definitely/not/a/picker".into(),
            args: vec![],
        };
        let err = run(&cmd, &[]).unwrap_err().to_string();
        assert!(err.contains("SSHREK_PICKER"), "{err}");
    }
}
