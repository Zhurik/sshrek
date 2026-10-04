use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MAX_INCLUDE_DEPTH: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEntry {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub order: usize,
}

pub fn parse_file(path: &Path) -> Vec<HostEntry> {
    let Ok(src) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut p = Parser::default();
    p.visited
        .insert(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
    let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    p.feed(&src, &base, 0);
    p.entries
}

#[derive(Default)]
struct Parser {
    entries: Vec<HostEntry>,
    seen: HashSet<String>,
    visited: HashSet<PathBuf>,
    order: usize,
}

impl Parser {
    fn feed(&mut self, src: &str, base: &Path, depth: usize) {
        // Индексы entries, объявленных текущим блоком Host: опции применяются ко всем.
        let mut block: Vec<usize> = Vec::new();
        // Внутри блока Match: его условия вычисляются при подключении,
        // статического списка хостов из них не получить.
        let mut skipping = false;

        for raw in src.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((kw, rest)) = split_keyword(line) else {
                continue;
            };
            match kw.as_str() {
                "host" => {
                    skipping = false;
                    block.clear();
                    for tok in rest.split_whitespace() {
                        let alias = unquote(tok);
                        if alias.is_empty() || is_pattern(&alias) {
                            continue;
                        }
                        if !self.seen.insert(alias.clone()) {
                            continue;
                        }
                        self.entries.push(HostEntry {
                            alias,
                            hostname: None,
                            user: None,
                            port: None,
                            order: self.order,
                        });
                        self.order += 1;
                        block.push(self.entries.len() - 1);
                    }
                }
                "match" => {
                    skipping = true;
                    block.clear();
                }
                "include" if !skipping => {
                    if depth >= MAX_INCLUDE_DEPTH {
                        continue;
                    }
                    let patterns: Vec<String> = rest.split_whitespace().map(unquote).collect();
                    for pattern in patterns {
                        self.include(&pattern, base, depth);
                    }
                }
                "hostname" | "user" | "port" if !skipping => {
                    let val = unquote(rest.trim());
                    for &i in &block {
                        let e = &mut self.entries[i];
                        match kw.as_str() {
                            "hostname" => {
                                if e.hostname.is_none() {
                                    e.hostname = Some(val.clone());
                                }
                            }
                            "user" => {
                                if e.user.is_none() {
                                    e.user = Some(val.clone());
                                }
                            }
                            _ => {
                                if e.port.is_none() {
                                    e.port = val.parse().ok();
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn include(&mut self, pattern: &str, base: &Path, depth: usize) {
        let expanded = expand_path(pattern, base);
        let Ok(paths) = glob::glob(&expanded.to_string_lossy()) else {
            return;
        };
        let mut found: Vec<PathBuf> = paths.flatten().collect();
        found.sort();
        for path in found {
            let canon = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !self.visited.insert(canon) {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else {
                continue;
            };
            let next_base = path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| base.to_path_buf());
            self.feed(&src, &next_base, depth + 1);
        }
    }
}

fn split_keyword(line: &str) -> Option<(String, String)> {
    let mut end = line.len();
    for (i, c) in line.char_indices() {
        if c.is_whitespace() || c == '=' {
            end = i;
            break;
        }
    }
    if end == 0 {
        return None;
    }
    let kw = line[..end].to_ascii_lowercase();
    let rest = line[end..]
        .trim_start_matches(|c: char| c.is_whitespace() || c == '=')
        .to_string();
    Some((kw, rest))
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t[1..t.len() - 1].to_string()
    } else {
        t.to_string()
    }
}

fn is_pattern(alias: &str) -> bool {
    alias.contains('*') || alias.contains('?') || alias.starts_with('!')
}

fn expand_path(p: &str, base: &Path) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    let path = Path::new(p);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    /// Разбор строки без Include: тестовый хелпер, в рабочем коде не нужен.
    fn parse_str(src: &str) -> Vec<HostEntry> {
        let mut p = Parser::default();
        p.feed(src, Path::new("."), 0);
        p.entries
    }

    #[test]
    fn collects_aliases_in_file_order() {
        let hosts = parse_str("Host usa\nHost server\n");
        assert_eq!(
            hosts.iter().map(|h| h.alias.as_str()).collect::<Vec<_>>(),
            vec!["usa", "server"]
        );
        assert_eq!(hosts[0].order, 0);
        assert_eq!(hosts[1].order, 1);
    }

    #[test]
    fn several_aliases_on_one_line_share_the_block_options() {
        let hosts = parse_str("Host foo bar\n  HostName 10.0.0.1\n  User root\n");
        assert_eq!(hosts.len(), 2);
        for h in &hosts {
            assert_eq!(h.hostname.as_deref(), Some("10.0.0.1"));
            assert_eq!(h.user.as_deref(), Some("root"));
        }
    }

    #[test]
    fn keywords_are_case_insensitive_and_accept_equals() {
        let hosts = parse_str("HOST potato\n  hostname=10.0.0.5\n  PORT = 2222\n");
        assert_eq!(hosts[0].hostname.as_deref(), Some("10.0.0.5"));
        assert_eq!(hosts[0].port, Some(2222));
    }

    #[test]
    fn patterns_and_negations_are_dropped() {
        let hosts = parse_str("Host *\n  User nobody\nHost rpi?\nHost !skip\nHost rpi5\n");
        assert_eq!(
            hosts.iter().map(|h| h.alias.as_str()).collect::<Vec<_>>(),
            vec!["rpi5"]
        );
        assert_eq!(
            hosts[0].user, None,
            "User из блока Host * не должен протечь"
        );
    }

    #[test]
    fn match_blocks_are_skipped_entirely() {
        let hosts = parse_str("Match host nope\n  HostName leak\nHost real\n  HostName ok\n");
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].alias, "real");
        assert_eq!(hosts[0].hostname.as_deref(), Some("ok"));
    }

    #[test]
    fn duplicate_alias_keeps_the_first_declaration() {
        let hosts = parse_str("Host dup\n  HostName first\nHost dup\n  HostName second\n");
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].hostname.as_deref(), Some("first"));
    }

    #[test]
    fn comments_and_quoted_values_are_handled() {
        let hosts = parse_str("# comment\nHost \"quoted\"\n  HostName \"10.0.0.9\"\n");
        assert_eq!(hosts[0].alias, "quoted");
        assert_eq!(hosts[0].hostname.as_deref(), Some("10.0.0.9"));
    }

    // Review Focus 4
    #[test]
    fn crlf_and_missing_trailing_newline_do_not_corrupt_the_last_host() {
        let hosts = parse_str("Host alpha\r\n  HostName 10.0.0.1\r\nHost omega");
        assert_eq!(
            hosts.iter().map(|h| h.alias.as_str()).collect::<Vec<_>>(),
            vec!["alpha", "omega"]
        );
        assert_eq!(hosts[0].hostname.as_deref(), Some("10.0.0.1"));
    }

    #[test]
    fn missing_file_yields_no_hosts() {
        assert!(parse_file(Path::new("/nope/definitely/not/here")).is_empty());
    }

    #[test]
    fn include_is_expanded_relative_to_the_including_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("extra"),
            "Host included\n  HostName 10.0.0.7\n",
        )
        .unwrap();
        let main = dir.path().join("config");
        std::fs::write(&main, "Host top\nInclude extra\n").unwrap();

        let hosts = parse_file(&main);
        assert_eq!(
            hosts.iter().map(|h| h.alias.as_str()).collect::<Vec<_>>(),
            vec!["top", "included"]
        );
        assert_eq!(hosts[1].hostname.as_deref(), Some("10.0.0.7"));
    }

    #[test]
    fn include_cycle_terminates() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, "Host a\nInclude b\n").unwrap();
        std::fs::write(&b, "Host b\nInclude a\n").unwrap();

        let hosts = parse_file(&a);
        assert_eq!(
            hosts.iter().map(|h| h.alias.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
    }

    #[test]
    fn include_accepts_globs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("one.conf"), "Host one\n").unwrap();
        std::fs::write(dir.path().join("two.conf"), "Host two\n").unwrap();
        let main = dir.path().join("config");
        std::fs::write(&main, "Include *.conf\n").unwrap();

        let mut aliases: Vec<String> = parse_file(&main).into_iter().map(|h| h.alias).collect();
        aliases.sort();
        assert_eq!(aliases, vec!["one", "two"]);
    }
}
