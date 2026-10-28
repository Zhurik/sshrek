//! The order of the host list: as declared in the config, most recent first,
//! or most frequent first.

use std::cmp::Ordering;
use std::collections::HashMap;

use chrono::{DateTime, Utc};

use crate::config::HostEntry;
use crate::history::HostStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
#[value(rename_all = "lower")]
pub enum SortKey {
    /// Declaration order in ~/.ssh/config
    Config,
    /// Most recently used first
    Recent,
    /// Most frequently used first
    Frequent,
}

fn last_of(stats: &HashMap<String, HostStats>, alias: &str) -> Option<DateTime<Utc>> {
    stats.get(alias).and_then(|s| s.last_connected)
}

fn count_of(stats: &HashMap<String, HostStats>, alias: &str) -> u32 {
    stats.get(alias).map_or(0, |s| s.connections)
}

pub fn sort_hosts(hosts: &mut [HostEntry], stats: &HashMap<String, HostStats>, key: SortKey) {
    match key {
        SortKey::Config => hosts.sort_by_key(|h| h.order),
        SortKey::Recent => {
            hosts.sort_by(
                |a, b| match (last_of(stats, &a.alias), last_of(stats, &b.alias)) {
                    (Some(x), Some(y)) => y.cmp(&x).then(a.order.cmp(&b.order)),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => a.order.cmp(&b.order),
                },
            )
        }
        SortKey::Frequent => hosts.sort_by(|a, b| {
            count_of(stats, &b.alias)
                .cmp(&count_of(stats, &a.alias))
                .then_with(|| last_of(stats, &b.alias).cmp(&last_of(stats, &a.alias)))
                .then(a.order.cmp(&b.order))
        }),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeDelta, TimeZone};

    fn host(alias: &str, order: usize) -> HostEntry {
        HostEntry {
            alias: alias.to_string(),
            hostname: None,
            user: None,
            port: None,
            order,
        }
    }

    fn stat(host: &str, connections: u32, last_secs: Option<i64>) -> HostStats {
        HostStats {
            host: host.to_string(),
            connections,
            last_connected: last_secs.map(|s| Utc.timestamp_opt(s, 0).unwrap()),
            total_duration: TimeDelta::zero(),
            last_duration: None,
            failures: 0,
            unfinished: 0,
        }
    }

    fn table(entries: Vec<HostStats>) -> HashMap<String, HostStats> {
        entries.into_iter().map(|s| (s.host.clone(), s)).collect()
    }

    fn aliases(hosts: &[HostEntry]) -> Vec<&str> {
        hosts.iter().map(|h| h.alias.as_str()).collect()
    }

    #[test]
    fn config_order_follows_the_file() {
        let mut hosts = vec![host("b", 1), host("a", 0), host("c", 2)];
        sort_hosts(&mut hosts, &HashMap::new(), SortKey::Config);
        assert_eq!(aliases(&hosts), vec!["a", "b", "c"]);
    }

    #[test]
    fn recent_puts_freshest_first_and_never_visited_last() {
        let mut hosts = vec![host("a", 0), host("b", 1), host("c", 2)];
        let stats = table(vec![stat("a", 1, Some(100)), stat("c", 1, Some(500))]);
        sort_hosts(&mut hosts, &stats, SortKey::Recent);
        assert_eq!(aliases(&hosts), vec!["c", "a", "b"]);
    }

    #[test]
    fn never_visited_hosts_keep_config_order_between_themselves() {
        let mut hosts = vec![host("z", 2), host("y", 1), host("x", 0)];
        sort_hosts(&mut hosts, &HashMap::new(), SortKey::Recent);
        assert_eq!(aliases(&hosts), vec!["x", "y", "z"]);
    }

    #[test]
    fn frequent_sorts_by_count_then_recency_then_config() {
        let mut hosts = vec![host("a", 0), host("b", 1), host("c", 2), host("d", 3)];
        let stats = table(vec![
            stat("a", 2, Some(10)),
            stat("b", 5, Some(10)),
            stat("c", 2, Some(900)),
        ]);
        sort_hosts(&mut hosts, &stats, SortKey::Frequent);
        // b (5), then c and a with 2 each — c is fresher, then d with no history
        assert_eq!(aliases(&hosts), vec!["b", "c", "a", "d"]);
    }

    #[test]
    fn every_order_is_total_so_output_is_deterministic() {
        let stats = table(vec![stat("a", 3, Some(10)), stat("b", 3, Some(10))]);
        for key in [SortKey::Config, SortKey::Recent, SortKey::Frequent] {
            let mut first = vec![host("a", 0), host("b", 1)];
            let mut second = vec![host("b", 1), host("a", 0)];
            sort_hosts(&mut first, &stats, key);
            sort_hosts(&mut second, &stats, key);
            assert_eq!(
                aliases(&first),
                aliases(&second),
                "key {key:?} leaves a tie"
            );
        }
    }
}
