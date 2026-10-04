use std::collections::HashMap;

use chrono::{DateTime, TimeDelta, Utc};

use crate::config::HostEntry;
use crate::history::HostStats;

/// Кэп ширины колонки: длинное значение расширит свою строку,
/// но не растянет таблицу целиком.
const MAX_ALIAS_WIDTH: usize = 24;
const MAX_LAST_WIDTH: usize = 16;
const MAX_COUNT_WIDTH: usize = 8;

pub fn fmt_relative(then: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - then).num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = secs / 60;
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = mins / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = hours / 24;
    if days < 7 {
        return if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{days} days ago")
        };
    }
    let weeks = days / 7;
    if days < 30 {
        return if weeks == 1 {
            "1 week ago".to_string()
        } else {
            format!("{weeks} weeks ago")
        };
    }
    let months = days / 30;
    if months < 12 {
        return format!("{months} months ago");
    }
    let years = days / 365;
    if years <= 1 {
        "1 year ago".to_string()
    } else {
        format!("{years} years ago")
    }
}

pub fn fmt_duration(d: TimeDelta) -> String {
    let s = d.num_seconds();
    if s <= 0 {
        return "0s".to_string();
    }
    let (h, m) = (s / 3600, (s % 3600) / 60);
    if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m")
    } else {
        format!("{s}s")
    }
}

pub fn target_of(e: &HostEntry) -> String {
    let host = e.hostname.clone().unwrap_or_else(|| e.alias.clone());
    let mut t = match &e.user {
        Some(u) => format!("{u}@{host}"),
        None => host,
    };
    if let Some(p) = e.port {
        if p != 22 {
            t.push_str(&format!(":{p}"));
        }
    }
    t
}

struct Cells {
    alias: String,
    last: String,
    count: String,
    total: String,
    target: String,
}

fn cells_for(
    hosts: &[HostEntry],
    stats: &HashMap<String, HostStats>,
    now: DateTime<Utc>,
) -> Vec<Cells> {
    hosts
        .iter()
        .map(|h| {
            let st = stats.get(&h.alias);
            Cells {
                alias: h.alias.clone(),
                last: st
                    .and_then(|s| s.last_connected)
                    .map_or_else(|| "never".to_string(), |t| fmt_relative(t, now)),
                count: format!("{}×", st.map_or(0, |s| s.connections)),
                total: st.map_or_else(|| "0s".to_string(), |s| fmt_duration(s.total_duration)),
                target: target_of(h),
            }
        })
        .collect()
}

fn width(values: impl Iterator<Item = usize>, cap: usize) -> usize {
    values.max().unwrap_or(0).min(cap)
}

pub fn picker_rows(
    hosts: &[HostEntry],
    stats: &HashMap<String, HostStats>,
    now: DateTime<Utc>,
) -> Vec<String> {
    let cells = cells_for(hosts, stats, now);
    let wa = width(
        cells.iter().map(|c| c.alias.chars().count()),
        MAX_ALIAS_WIDTH,
    );
    let wl = width(cells.iter().map(|c| c.last.chars().count()), MAX_LAST_WIDTH);
    let wc = width(
        cells.iter().map(|c| c.count.chars().count()),
        MAX_COUNT_WIDTH,
    );
    cells
        .iter()
        .map(|c| {
            format!(
                "{:<wa$}  {:<wl$}  {:>wc$}  {}",
                c.alias,
                c.last,
                c.count,
                c.target,
                wa = wa,
                wl = wl,
                wc = wc
            )
        })
        .collect()
}

/// Тот же набор данных, что в таблице, но TSV без выравнивания и заголовка.
pub fn porcelain_rows(
    hosts: &[HostEntry],
    stats: &HashMap<String, HostStats>,
    orphans: &[&HostStats],
    now: DateTime<Utc>,
) -> Vec<String> {
    let mut cells = cells_for(hosts, stats, now);
    cells.extend(orphans.iter().map(|s| orphan_cells(s, now)));
    cells
        .iter()
        .map(|c| {
            format!(
                "{}\t{}\t{}\t{}\t{}",
                c.alias, c.last, c.count, c.total, c.target
            )
        })
        .collect()
}

/// Хост, живущий только в истории: подключиться к нему нельзя, но знать о нём
/// полезно — и человеку в таблице, и скрипту в porcelain.
fn orphan_cells(s: &HostStats, now: DateTime<Utc>) -> Cells {
    Cells {
        alias: s.host.clone(),
        last: s
            .last_connected
            .map_or_else(|| "never".to_string(), |t| fmt_relative(t, now)),
        count: format!("{}×", s.connections),
        total: fmt_duration(s.total_duration),
        target: "(нет в config)".to_string(),
    }
}

pub fn ls_table(
    hosts: &[HostEntry],
    stats: &HashMap<String, HostStats>,
    orphans: &[&HostStats],
    now: DateTime<Utc>,
) -> String {
    let mut cells = cells_for(hosts, stats, now);
    cells.extend(orphans.iter().map(|s| orphan_cells(s, now)));

    let header = Cells {
        alias: "HOST".into(),
        last: "LAST".into(),
        count: "COUNT".into(),
        total: "TOTAL".into(),
        target: "TARGET".into(),
    };
    let all: Vec<&Cells> = std::iter::once(&header).chain(cells.iter()).collect();
    let wa = width(all.iter().map(|c| c.alias.chars().count()), MAX_ALIAS_WIDTH);
    let wl = width(all.iter().map(|c| c.last.chars().count()), MAX_LAST_WIDTH);
    let wc = width(all.iter().map(|c| c.count.chars().count()), MAX_COUNT_WIDTH);
    let wt = width(all.iter().map(|c| c.total.chars().count()), 12);

    all.iter()
        .map(|c| {
            format!(
                "{:<wa$}  {:<wl$}  {:>wc$}  {:>wt$}  {}",
                c.alias,
                c.last,
                c.count,
                c.total,
                c.target,
                wa = wa,
                wl = wl,
                wc = wc,
                wt = wt
            )
            .trim_end()
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn preview(
    alias: &str,
    entry: Option<&HostEntry>,
    stats: Option<&HostStats>,
    now: DateTime<Utc>,
) -> String {
    let target = entry.map_or_else(|| "(нет в config)".to_string(), target_of);
    let last = stats.and_then(|s| s.last_connected).map_or_else(
        || "never".to_string(),
        |t| {
            let rel = fmt_relative(t, now);
            match stats.and_then(|s| s.last_duration) {
                Some(d) => format!("{rel} ({})", fmt_duration(d)),
                None => rel,
            }
        },
    );
    let connections = stats.map_or(0, |s| s.connections);
    let total = stats.map_or_else(|| "0s".to_string(), |s| fmt_duration(s.total_duration));
    let failures = stats.map_or(0, |s| s.failures);
    let unfinished = stats.map_or(0, |s| s.unfinished);

    let mut out = format!(
        "{alias}\n  target       {target}\n  connections  {connections}\n  last         {last}\n  total        {total}\n"
    );
    if failures > 0 {
        out.push_str(&format!("  failures     {failures}\n"));
    }
    if unfinished > 0 {
        out.push_str(&format!("  unfinished   {unfinished}\n"));
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(secs: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(secs, 0).unwrap()
    }

    fn host(alias: &str, order: usize) -> HostEntry {
        HostEntry {
            alias: alias.to_string(),
            hostname: None,
            user: None,
            port: None,
            order,
        }
    }

    fn stat(host: &str, connections: u32, last: Option<i64>, total_secs: i64) -> HostStats {
        HostStats {
            host: host.to_string(),
            connections,
            last_connected: last.map(at),
            total_duration: TimeDelta::seconds(total_secs),
            last_duration: Some(TimeDelta::seconds(total_secs)),
            failures: 0,
            unfinished: 0,
        }
    }

    fn table(v: Vec<HostStats>) -> HashMap<String, HostStats> {
        v.into_iter().map(|s| (s.host.clone(), s)).collect()
    }

    #[test]
    fn relative_time_boundaries() {
        let now = at(10_000_000);
        assert_eq!(fmt_relative(now, now), "just now");
        assert_eq!(fmt_relative(now - TimeDelta::seconds(59), now), "just now");
        assert_eq!(fmt_relative(now - TimeDelta::seconds(60), now), "1m ago");
        assert_eq!(fmt_relative(now - TimeDelta::minutes(59), now), "59m ago");
        assert_eq!(fmt_relative(now - TimeDelta::minutes(60), now), "1h ago");
        assert_eq!(fmt_relative(now - TimeDelta::hours(23), now), "23h ago");
        assert_eq!(fmt_relative(now - TimeDelta::hours(24), now), "1 day ago");
        assert_eq!(fmt_relative(now - TimeDelta::days(6), now), "6 days ago");
        assert_eq!(fmt_relative(now - TimeDelta::days(7), now), "1 week ago");
        assert_eq!(fmt_relative(now - TimeDelta::days(14), now), "2 weeks ago");
        assert_eq!(fmt_relative(now - TimeDelta::days(60), now), "2 months ago");
        assert_eq!(fmt_relative(now - TimeDelta::days(400), now), "1 year ago");
    }

    #[test]
    fn clock_skew_does_not_produce_negative_time() {
        let now = at(1_000);
        assert_eq!(fmt_relative(at(2_000), now), "just now");
    }

    #[test]
    fn duration_formatting() {
        assert_eq!(fmt_duration(TimeDelta::zero()), "0s");
        assert_eq!(fmt_duration(TimeDelta::seconds(45)), "45s");
        assert_eq!(fmt_duration(TimeDelta::seconds(90)), "1m");
        assert_eq!(fmt_duration(TimeDelta::seconds(3_600 * 3 + 720)), "3h 12m");
    }

    #[test]
    fn target_prefers_config_values_and_hides_default_port() {
        let mut h = host("potato", 0);
        assert_eq!(target_of(&h), "potato");
        h.hostname = Some("10.0.0.5".into());
        assert_eq!(target_of(&h), "10.0.0.5");
        h.user = Some("root".into());
        assert_eq!(target_of(&h), "root@10.0.0.5");
        h.port = Some(22);
        assert_eq!(target_of(&h), "root@10.0.0.5");
        h.port = Some(2222);
        assert_eq!(target_of(&h), "root@10.0.0.5:2222");
    }

    #[test]
    fn picker_rows_start_with_the_alias_and_show_never_for_fresh_hosts() {
        let hosts = vec![host("potato", 0), host("france", 1)];
        let stats = table(vec![stat("potato", 42, Some(9_999_000), 600)]);
        let rows = picker_rows(&hosts, &stats, at(10_000_000));

        assert_eq!(rows[0].split_whitespace().next(), Some("potato"));
        assert!(rows[0].contains("42×"), "{}", rows[0]);
        assert_eq!(rows[1].split_whitespace().next(), Some("france"));
        assert!(rows[1].contains("never"), "{}", rows[1]);
    }

    #[test]
    fn picker_rows_are_column_aligned() {
        let hosts = vec![host("a", 0), host("long-host-name", 1)];
        let rows = picker_rows(&hosts, &HashMap::new(), at(0));
        let second_column_start = |row: &str| row.find("never").unwrap();
        assert_eq!(
            second_column_start(&rows[0]),
            second_column_start(&rows[1]),
            "колонки выровнены:\n{}\n{}",
            rows[0],
            rows[1]
        );
    }

    // Review Focus 5 (длинные значения не должны ломать первое поле)
    #[test]
    fn overlong_alias_still_leaves_the_alias_as_field_one() {
        let hosts = vec![host(&"x".repeat(60), 0), host("short", 1)];
        let rows = picker_rows(&hosts, &HashMap::new(), at(0));
        assert_eq!(
            rows[0].split_whitespace().next(),
            Some("x".repeat(60).as_str())
        );
        assert_eq!(rows[1].split_whitespace().next(), Some("short"));
    }

    #[test]
    fn porcelain_rows_are_tab_separated_without_padding() {
        let hosts = vec![host("potato", 0)];
        let rows = porcelain_rows(&hosts, &HashMap::new(), &[], at(0));
        assert_eq!(rows[0], "potato\tnever\t0×\t0s\tpotato");
    }

    #[test]
    fn porcelain_carries_the_same_data_as_the_table() {
        let hosts = vec![host("potato", 0)];
        let stats = table(vec![stat("potato", 2, Some(9_999_000), 600)]);
        let gone = stat("retired", 5, Some(9_000_000), 60);
        let rows = porcelain_rows(&hosts, &stats, &[&gone], at(10_000_000));

        assert_eq!(
            rows.len(),
            2,
            "хост из истории, выпавший из config, тоже в выводе"
        );
        let fields: Vec<&str> = rows[0].split('\t').collect();
        assert_eq!(
            fields.len(),
            5,
            "HOST, LAST, COUNT, TOTAL, TARGET: {:?}",
            fields
        );
        assert_eq!(fields[3], "10m", "длительность доступна машинно");
        assert!(rows[1].starts_with("retired\t"), "{}", rows[1]);
        assert!(rows[1].contains("нет в config"), "{}", rows[1]);
    }

    #[test]
    fn ls_table_has_a_header_and_marks_hosts_missing_from_config() {
        let hosts = vec![host("potato", 0)];
        let stats = table(vec![stat("potato", 2, Some(9_999_000), 600)]);
        let gone = stat("retired", 5, Some(9_000_000), 60);
        let out = ls_table(&hosts, &stats, &[&gone], at(10_000_000));

        assert!(out.starts_with("HOST"), "{out}");
        assert!(out.contains("potato"), "{out}");
        assert!(out.contains("retired"), "{out}");
        assert!(out.contains("нет в config"), "{out}");
    }

    #[test]
    fn preview_covers_known_and_unknown_hosts() {
        let h = HostEntry {
            alias: "potato".into(),
            hostname: Some("10.0.0.5".into()),
            user: Some("root".into()),
            port: None,
            order: 0,
        };
        let s = stat("potato", 42, Some(9_999_000), 3_600);
        let out = preview("potato", Some(&h), Some(&s), at(10_000_000));
        assert!(out.contains("root@10.0.0.5"), "{out}");
        assert!(out.contains("42"), "{out}");
        assert!(out.contains("1h 0m"), "{out}");

        let empty = preview("ghost", None, None, at(0));
        assert!(empty.contains("ghost"), "{empty}");
        assert!(empty.contains("never"), "{empty}");
    }
}
