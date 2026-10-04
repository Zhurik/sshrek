use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};

use super::event::{parse_line, LineOutcome, Record};

pub struct Store {
    path: PathBuf,
}

#[derive(Debug, Default)]
pub struct ReadResult {
    pub records: Vec<Record>,
    pub malformed: usize,
    pub unknown_version: usize,
    pub error: Option<String>,
}

impl Store {
    pub fn new(path: PathBuf) -> Self {
        Store { path }
    }

    /// Append-only: одна запись — одна строка, файл никогда не перезаписывается.
    pub fn append(&self, rec: &Record) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("не удалось создать {}", parent.display()))?;
        }
        let mut line = serde_json::to_string(rec).context("сериализация события")?;
        // Перевод строки уходит тем же write: O_APPEND атомарен для одного вызова,
        // но не для пары — иначе параллельные запуски склеивают строки.
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("не удалось открыть {}", self.path.display()))?;
        file.write_all(line.as_bytes())
            .with_context(|| format!("не удалось записать в {}", self.path.display()))
    }

    /// Никогда не возвращает Err: по спеке история не причина падения.
    pub fn read_all(&self) -> ReadResult {
        let mut out = ReadResult::default();
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return out,
            Err(e) => {
                out.error = Some(format!("{}: {e}", self.path.display()));
                return out;
            }
        };
        for line in text.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match parse_line(line) {
                LineOutcome::Parsed(rec) => out.records.push(rec),
                LineOutcome::UnknownVersion => out.unknown_version += 1,
                LineOutcome::Malformed => out.malformed += 1,
            }
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn rec(id: &str, host: &str, secs: i64) -> Record {
        Record::start(
            id.to_string(),
            host,
            Utc.timestamp_opt(secs, 0).unwrap(),
            &[],
        )
    }

    #[test]
    fn append_creates_missing_directories_and_adds_one_line_per_record() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("deep").join("sessions.jsonl");
        let store = Store::new(log.clone());

        store.append(&rec("a", "potato", 1)).unwrap();
        store.append(&rec("b", "rpi5", 2)).unwrap();

        let text = std::fs::read_to_string(&log).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(
            text.ends_with('\n'),
            "каждая запись завершается переводом строки"
        );
    }

    #[test]
    fn read_all_skips_malformed_and_future_versions_and_counts_them() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("sessions.jsonl");
        let good = serde_json::to_string(&rec("a", "potato", 1)).unwrap();
        std::fs::write(
            &log,
            format!("{good}\nне json\n{{\"v\":42,\"event\":\"start\"}}\n\n"),
        )
        .unwrap();

        let out = Store::new(log).read_all();
        assert_eq!(out.records.len(), 1);
        assert_eq!(out.malformed, 1);
        assert_eq!(out.unknown_version, 1);
        assert!(out.error.is_none());
    }

    #[test]
    fn missing_log_reads_as_empty_without_error() {
        let out = Store::new("/nope/not/here/sessions.jsonl".into()).read_all();
        assert!(out.records.is_empty());
        assert!(out.error.is_none(), "отсутствие истории — не ошибка");
    }

    #[test]
    fn concurrent_appends_never_interleave_within_a_line() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("sessions.jsonl");

        // O_APPEND делает атомарным каждый write, но не пару write'ов:
        // запись строки и перевода строки по отдельности позволяет соседнему
        // процессу вклиниться между ними.
        std::thread::scope(|scope| {
            for t in 0..8 {
                let log = log.clone();
                scope.spawn(move || {
                    let store = Store::new(log);
                    for i in 0..100 {
                        store
                            .append(&rec(&format!("{t}-{i}"), "potato", 1))
                            .unwrap();
                    }
                });
            }
        });

        let out = Store::new(log).read_all();
        assert_eq!(out.malformed, 0, "битых строк быть не должно");
        assert_eq!(out.records.len(), 800, "ни одна запись не потеряна");
    }

    // Review Focus 5
    #[test]
    fn append_fails_loudly_when_state_dir_is_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let busy = dir.path().join("state");
        std::fs::write(&busy, "я файл, а не каталог").unwrap();
        let store = Store::new(busy.join("sessions.jsonl"));

        let err = store.append(&rec("a", "potato", 1)).unwrap_err();
        assert!(
            err.to_string().contains("sessions.jsonl") || err.to_string().contains("state"),
            "ошибка называет путь: {err}"
        );
    }

    #[test]
    fn read_all_reports_unreadable_log_as_error() {
        let dir = tempfile::tempdir().unwrap();
        let as_dir = dir.path().join("sessions.jsonl");
        std::fs::create_dir(&as_dir).unwrap();

        let out = Store::new(as_dir).read_all();
        assert!(out.records.is_empty());
        assert!(
            out.error.is_some(),
            "каталог вместо файла — это ошибка чтения"
        );
    }
}
