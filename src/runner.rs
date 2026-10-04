use std::os::unix::process::CommandExt;
use std::process::Command;
use std::time::SystemTime;

use chrono::Utc;

use crate::history::{new_session_id, Record, Store};

/// На время ожидания ssh родитель перестаёт умирать от Ctrl-C: иначе
/// end-событие не запишется и сессия навсегда останется незавершённой.
///
/// Игнорируемая диспозиция переживает fork+exec, поэтому ребёнку её возвращают
/// в SIG_DFL (см. `spawn_ssh`): без этого Ctrl-C не дошёл бы и до самого ssh,
/// и `sshrek host -- -N -L …` стало бы невозможно остановить с клавиатуры.
struct SignalGuard {
    int: libc::sighandler_t,
    quit: libc::sighandler_t,
}

impl SignalGuard {
    fn ignore_interrupts() -> Self {
        unsafe {
            SignalGuard {
                int: libc::signal(libc::SIGINT, libc::SIG_IGN),
                quit: libc::signal(libc::SIGQUIT, libc::SIG_IGN),
            }
        }
    }
}

impl Drop for SignalGuard {
    fn drop(&mut self) {
        unsafe {
            libc::signal(libc::SIGINT, self.int);
            libc::signal(libc::SIGQUIT, self.quit);
        }
    }
}

fn warn_on_err(result: anyhow::Result<()>) {
    if let Err(e) = result {
        eprintln!("sshrek: история не записана: {e}");
    }
}

pub fn connect(store: &Store, ssh_bin: &str, host: &str, extra: &[String]) -> i32 {
    let id = new_session_id(SystemTime::now(), std::process::id());
    warn_on_err(store.append(&Record::start(id.clone(), host, Utc::now(), extra)));

    // Guard живёт до самого конца: Ctrl-C между wait и записью end оставил бы
    // сессию навсегда незавершённой.
    let _guard = SignalGuard::ignore_interrupts();
    let status = spawn_ssh(ssh_bin, host, extra);

    match status {
        Ok(st) => {
            let code = exit_code_of(&st);
            warn_on_err(store.append(&Record::end(id, Utc::now(), Some(code), None)));
            code
        }
        Err(e) => {
            eprintln!("sshrek: не удалось запустить '{ssh_bin}' (см. SSHREK_SSH_BIN): {e}");
            warn_on_err(store.append(&Record::end(id, Utc::now(), None, Some(e.to_string()))));
            1
        }
    }
}

fn spawn_ssh(
    ssh_bin: &str,
    host: &str,
    extra: &[String],
) -> std::io::Result<std::process::ExitStatus> {
    let mut cmd = Command::new(ssh_bin);
    cmd.arg(host).args(extra);
    // SAFETY: между fork и exec зовётся только signal(2) — он async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            libc::signal(libc::SIGINT, libc::SIG_DFL);
            libc::signal(libc::SIGQUIT, libc::SIG_DFL);
            Ok(())
        });
    }
    cmd.status()
}

fn exit_code_of(status: &std::process::ExitStatus) -> i32 {
    use std::os::unix::process::ExitStatusExt;
    status
        .code()
        .unwrap_or_else(|| 128 + status.signal().unwrap_or(0))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::event::Event;

    fn fake_ssh(dir: &std::path::Path, script: &str) -> String {
        let path = dir.join("fake-ssh");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn writes_start_and_end_and_propagates_the_exit_code() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = fake_ssh(dir.path(), "exit 7");
        let store = Store::new(dir.path().join("sessions.jsonl"));

        let code = connect(&store, &ssh, "potato", &[]);
        assert_eq!(code, 7, "код ssh прокидывается как есть");

        let recs = store.read_all().records;
        assert_eq!(recs.len(), 2);
        match (&recs[0].event, &recs[1].event) {
            (
                Event::Start { host, id: sid, .. },
                Event::End {
                    id: eid,
                    exit,
                    error,
                    ..
                },
            ) => {
                assert_eq!(host, "potato");
                assert_eq!(sid, eid, "end ссылается на тот же id");
                assert_eq!(*exit, Some(7));
                assert!(error.is_none());
            }
            other => panic!("ожидали start+end, получили {other:?}"),
        }
    }

    #[test]
    fn passes_host_and_extra_args_to_ssh_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("argv");
        let ssh = fake_ssh(
            dir.path(),
            &format!("printf '%s\\n' \"$@\" > {}", out.display()),
        );
        let store = Store::new(dir.path().join("sessions.jsonl"));

        let extra = vec!["-L".to_string(), "8080:localhost:80".to_string()];
        assert_eq!(connect(&store, &ssh, "rpi5", &extra), 0);

        let argv = std::fs::read_to_string(&out).unwrap();
        assert_eq!(
            argv.lines().collect::<Vec<_>>(),
            vec!["rpi5", "-L", "8080:localhost:80"]
        );
    }

    #[test]
    fn extra_args_are_recorded_in_the_start_event() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = fake_ssh(dir.path(), "exit 0");
        let store = Store::new(dir.path().join("sessions.jsonl"));

        connect(&store, &ssh, "potato", &["-v".to_string()]);

        match &store.read_all().records[0].event {
            Event::Start { args, .. } => assert_eq!(args, &vec!["-v".to_string()]),
            other => panic!("ожидали Start, получили {other:?}"),
        }
    }

    #[test]
    fn unlaunchable_ssh_reports_error_and_closes_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::new(dir.path().join("sessions.jsonl"));

        let code = connect(&store, "/definitely/not/ssh", "potato", &[]);
        assert_eq!(code, 1);

        let recs = store.read_all().records;
        assert_eq!(recs.len(), 2, "start уже записан, поэтому пишем и end");
        match &recs[1].event {
            Event::End { exit, error, .. } => {
                assert_eq!(*exit, None);
                assert!(error.is_some(), "причина неудачного запуска сохранена");
            }
            other => panic!("ожидали End, получили {other:?}"),
        }
    }

    #[test]
    fn ssh_child_gets_default_sigint_not_the_parents_ignore() {
        let dir = tempfile::tempdir().unwrap();
        // Скрипт шлёт SIGINT сам себе: с унаследованным от родителя SIG_IGN он
        // это переживёт и выйдет с нулём, с дефолтной диспозицией — умрёт от сигнала.
        let ssh = fake_ssh(dir.path(), "kill -INT $$\nexit 0");
        let store = Store::new(dir.path().join("sessions.jsonl"));

        let code = connect(&store, &ssh, "potato", &[]);

        assert_eq!(
            code, 130,
            "ssh должен получать SIGINT по умолчанию (128+2), а не наследовать SIG_IGN родителя"
        );
    }

    // Review Focus 5: сломанный лог не мешает подключению
    #[test]
    fn connection_succeeds_even_when_the_log_cannot_be_written() {
        let dir = tempfile::tempdir().unwrap();
        let ssh = fake_ssh(dir.path(), "exit 0");
        let busy = dir.path().join("state");
        std::fs::write(&busy, "я файл").unwrap();
        let store = Store::new(busy.join("sessions.jsonl"));

        assert_eq!(connect(&store, &ssh, "potato", &[]), 0);
    }
}
