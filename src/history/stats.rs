use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};

use super::event::{Event, Record};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostStats {
    pub host: String,
    pub connections: u32,
    pub last_connected: Option<DateTime<Utc>>,
    pub total_duration: TimeDelta,
    pub last_duration: Option<TimeDelta>,
    pub failures: u32,
    pub unfinished: u32,
}

impl HostStats {
    fn new(host: &str) -> Self {
        HostStats {
            host: host.to_string(),
            connections: 0,
            last_connected: None,
            total_duration: TimeDelta::zero(),
            last_duration: None,
            failures: 0,
            unfinished: 0,
        }
    }
}

pub fn aggregate(records: &[Record]) -> HashMap<String, HostStats> {
    let mut out: HashMap<String, HostStats> = HashMap::new();
    // id -> (host, start moment); closed sessions are removed, what remains is unfinished.
    let mut open: HashMap<&str, (&str, DateTime<Utc>)> = HashMap::new();

    for rec in records {
        match &rec.event {
            Event::Start { id, host, at, .. } => {
                let s = out
                    .entry(host.clone())
                    .or_insert_with(|| HostStats::new(host));
                s.connections += 1;
                if s.last_connected.is_none_or(|prev| *at > prev) {
                    s.last_connected = Some(*at);
                }
                open.insert(id.as_str(), (host.as_str(), *at));
            }
            Event::End {
                id,
                at,
                exit,
                error,
                ..
            } => {
                let Some((host, started)) = open.remove(id.as_str()) else {
                    continue;
                };
                let s = out
                    .entry(host.to_string())
                    .or_insert_with(|| HostStats::new(host));
                // The clock may have jumped backwards: a negative duration counts as zero.
                let raw = *at - started;
                let d = if raw < TimeDelta::zero() {
                    TimeDelta::zero()
                } else {
                    raw
                };
                s.total_duration += d;
                s.last_duration = Some(d);
                if error.is_some() || exit.is_some_and(|c| c != 0) {
                    s.failures += 1;
                }
            }
        }
    }

    for (host, _started) in open.into_values() {
        if let Some(s) = out.get_mut(host) {
            s.unfinished += 1;
        }
    }
    out
}

pub fn last_host(records: &[Record]) -> Option<&str> {
    records.iter().rev().find_map(|r| match &r.event {
        Event::Start { host, .. } => Some(host.as_str()),
        Event::End { .. } => None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    fn start(id: &str, host: &str, secs: i64) -> Record {
        Record::start(id.to_string(), host, at(secs), &[])
    }

    fn end(id: &str, secs: i64, exit: Option<i32>) -> Record {
        Record::end(id.to_string(), at(secs), exit, None)
    }

    #[test]
    fn pairs_start_and_end_into_a_duration() {
        let recs = vec![start("a", "potato", 1_000), end("a", 1_600, Some(0))];
        let stats = aggregate(&recs);
        let s = &stats["potato"];
        assert_eq!(s.connections, 1);
        assert_eq!(s.total_duration.num_seconds(), 600);
        assert_eq!(s.last_duration.map(|d| d.num_seconds()), Some(600));
        assert_eq!(s.last_connected, Some(at(1_000)));
        assert_eq!(s.failures, 0);
        assert_eq!(s.unfinished, 0);
    }

    #[test]
    fn sums_durations_across_sessions() {
        let recs = vec![
            start("a", "potato", 0),
            end("a", 60, Some(0)),
            start("b", "potato", 100),
            end("b", 160, Some(0)),
        ];
        let s = &aggregate(&recs)["potato"];
        assert_eq!(s.connections, 2);
        assert_eq!(s.total_duration.num_seconds(), 120);
        assert_eq!(s.last_connected, Some(at(100)));
    }

    #[test]
    fn start_without_end_counts_as_unfinished_connection() {
        let recs = vec![start("a", "rpi5", 10)];
        let s = &aggregate(&recs)["rpi5"];
        assert_eq!(s.connections, 1);
        assert_eq!(s.unfinished, 1);
        assert_eq!(s.total_duration.num_seconds(), 0);
        assert_eq!(s.last_duration, None);
    }

    #[test]
    fn end_without_known_start_is_ignored() {
        let recs = vec![end("orphan", 10, Some(0))];
        assert!(aggregate(&recs).is_empty());
    }

    #[test]
    fn nonzero_exit_and_spawn_error_count_as_failures() {
        let recs = vec![
            start("a", "france", 0),
            end("a", 5, Some(255)),
            start("b", "france", 10),
            Record::end("b".into(), at(10), None, Some("no such file".into())),
        ];
        let s = &aggregate(&recs)["france"];
        assert_eq!(s.failures, 2);
        assert_eq!(s.connections, 2);
    }

    // Review Focus 1
    #[test]
    fn end_before_start_yields_zero_duration_without_panicking() {
        let recs = vec![start("a", "potato", 1_000), end("a", 400, Some(0))];
        let s = &aggregate(&recs)["potato"];
        assert_eq!(s.total_duration.num_seconds(), 0);
        assert_eq!(s.last_duration.map(|d| d.num_seconds()), Some(0));
    }

    #[test]
    fn last_host_is_the_most_recent_start_in_file_order() {
        let recs = vec![
            start("a", "potato", 0),
            end("a", 10, Some(0)),
            start("b", "rpi5", 20),
            end("b", 30, Some(0)),
        ];
        assert_eq!(last_host(&recs), Some("rpi5"));
        assert_eq!(last_host(&[]), None);
    }

    #[test]
    fn last_host_ignores_end_only_tail() {
        let recs = vec![start("a", "potato", 0), end("a", 10, Some(0))];
        assert_eq!(last_host(&recs), Some("potato"));
    }
}
