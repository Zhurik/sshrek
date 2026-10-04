use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;

pub type SessionId = String;

/// `<наносекунды в hex>-<pid в hex>`: уникально без генератора UUID и читаемо глазами.
pub fn new_session_id(now: SystemTime, pid: u32) -> SessionId {
    let nanos = now
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}-{pid:x}")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Start {
        id: SessionId,
        host: String,
        at: DateTime<Utc>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        args: Vec<String>,
    },
    End {
        id: SessionId,
        at: DateTime<Utc>,
        exit: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub v: u32,
    #[serde(flatten)]
    pub event: Event,
}

impl Record {
    pub fn start(id: SessionId, host: &str, at: DateTime<Utc>, args: &[String]) -> Self {
        Record {
            v: SCHEMA_VERSION,
            event: Event::Start {
                id,
                host: host.to_string(),
                at,
                args: args.to_vec(),
            },
        }
    }

    pub fn end(id: SessionId, at: DateTime<Utc>, exit: Option<i32>, error: Option<String>) -> Self {
        Record {
            v: SCHEMA_VERSION,
            event: Event::End {
                id,
                at,
                exit,
                error,
            },
        }
    }
}

#[derive(Debug)]
pub enum LineOutcome {
    Parsed(Record),
    UnknownVersion,
    Malformed,
}

/// Сначала смотрим только на `v`: строка будущей схемы должна пропускаться,
/// а не считаться битой.
pub fn parse_line(line: &str) -> LineOutcome {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
        return LineOutcome::Malformed;
    };
    match value.get("v").and_then(serde_json::Value::as_u64) {
        Some(v) if v == u64::from(SCHEMA_VERSION) => {}
        Some(_) => return LineOutcome::UnknownVersion,
        None => return LineOutcome::Malformed,
    }
    match serde_json::from_value::<Record>(value) {
        Ok(rec) => LineOutcome::Parsed(rec),
        Err(_) => LineOutcome::Malformed,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    #[test]
    fn start_record_serializes_with_version_first() {
        let rec = Record::start("abc".into(), "potato", at(1_700_000_000), &[]);
        let json = serde_json::to_string(&rec).unwrap();
        assert!(json.starts_with(r#"{"v":1,"event":"start""#), "{json}");
        assert!(json.contains(r#""host":"potato""#), "{json}");
        assert!(!json.contains("args"), "пустые args не пишем: {json}");
    }

    #[test]
    fn end_record_round_trips() {
        let rec = Record::end("abc".into(), at(1_700_000_060), Some(0), None);
        let json = serde_json::to_string(&rec).unwrap();
        match parse_line(&json) {
            LineOutcome::Parsed(back) => assert_eq!(back, rec),
            other => panic!("ожидали Parsed, получили {other:?}"),
        }
    }

    #[test]
    fn start_record_keeps_extra_ssh_args() {
        let args = vec!["-L".to_string(), "8080:localhost:80".to_string()];
        let rec = Record::start("abc".into(), "potato", at(1), &args);
        let json = serde_json::to_string(&rec).unwrap();
        assert!(
            json.contains(r#""args":["-L","8080:localhost:80"]"#),
            "{json}"
        );
    }

    #[test]
    fn unknown_schema_version_is_reported_not_parsed() {
        let line = r#"{"v":99,"event":"start","id":"x","host":"h","at":"2026-01-01T00:00:00Z"}"#;
        assert!(matches!(parse_line(line), LineOutcome::UnknownVersion));
    }

    #[test]
    fn truncated_and_garbage_lines_are_malformed() {
        assert!(matches!(
            parse_line(r#"{"v":1,"event":"st"#),
            LineOutcome::Malformed
        ));
        assert!(matches!(
            parse_line("не json вовсе"),
            LineOutcome::Malformed
        ));
        assert!(matches!(
            parse_line(r#"{"v":1,"event":"wat"}"#),
            LineOutcome::Malformed
        ));
    }

    #[test]
    fn session_ids_differ_for_different_moments() {
        let base = SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(123_456_789);
        let a = new_session_id(base, 42);
        let b = new_session_id(base + std::time::Duration::from_nanos(1), 42);
        assert_ne!(a, b);
        assert!(a.contains('-'), "id читаем глазами: {a}");
    }
}
