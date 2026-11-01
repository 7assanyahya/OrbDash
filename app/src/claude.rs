//! Claude Code usage from the local session logs (~/.claude/projects/**/*.jsonl).
//! Same method as the old Python bridge: dedupe streamed messages, then group
//! them into 5-hour blocks that start on the hour of their first message.

use chrono::{DateTime, Local, TimeZone, Timelike, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const BLOCK_SECS: i64 = 5 * 3600;

#[derive(Clone, Default)]
pub struct ClaudeStats {
    pub block_tokens: u64,
    /// Start of the current 5h block, if one is active
    pub block_start: Option<DateTime<Utc>>,
    pub today_tokens: u64,
    pub today_msgs: u32,
    pub log_files: usize,
}

impl ClaudeStats {
    pub fn block_elapsed_pct(&self) -> u8 {
        match self.block_start {
            Some(start) => {
                let secs = (Utc::now() - start).num_seconds().clamp(0, BLOCK_SECS);
                (secs * 100 / BLOCK_SECS) as u8
            }
            None => 0,
        }
    }

    /// "3h12m", or "IDLE" when no block is running
    pub fn time_left(&self) -> String {
        match self.block_start {
            Some(start) => {
                let mins = ((start.timestamp() + BLOCK_SECS - Utc::now().timestamp()) / 60).max(0);
                format!("{}h{:02}m", mins / 60, mins % 60)
            }
            None => "IDLE".into(),
        }
    }
}

type Key = (String, String);

#[derive(Default)]
pub struct ClaudeScanner {
    cache: HashMap<PathBuf, (SystemTime, Vec<(Key, DateTime<Utc>, u64)>)>,
}

impl ClaudeScanner {
    pub fn log_root() -> Option<PathBuf> {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(|h| PathBuf::from(h).join(".claude").join("projects"))
    }

    pub fn scan(&mut self) -> ClaudeStats {
        let mut files = Vec::new();
        if let Some(root) = Self::log_root() {
            collect_jsonl(&root, &mut files);
        }

        // Re-parse only files whose mtime changed
        let mut entries: HashMap<Key, (DateTime<Utc>, u64)> = HashMap::new();
        for f in &files {
            let Ok(mtime) = std::fs::metadata(f).and_then(|m| m.modified()) else { continue };
            let stale = self.cache.get(f).is_none_or(|(t, _)| *t != mtime);
            if stale {
                self.cache.insert(f.clone(), (mtime, parse_file(f)));
            }
            // Streaming writes the same message several times; keep one per id
            for (key, t, tokens) in &self.cache[f].1 {
                entries.insert(key.clone(), (*t, *tokens));
            }
        }
        self.cache.retain(|k, _| files.contains(k));

        let mut list: Vec<(DateTime<Utc>, u64)> = entries.into_values().collect();
        list.sort();
        summarize(&list, files.len())
    }
}

fn collect_jsonl(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_jsonl(&p, out);
        } else if p.extension().is_some_and(|x| x == "jsonl") {
            out.push(p);
        }
    }
}

fn parse_file(path: &Path) -> Vec<(Key, DateTime<Utc>, u64)> {
    let Ok(text) = std::fs::read(path) else { return Vec::new() };
    let text = String::from_utf8_lossy(&text);
    let mut rows = Vec::new();

    for line in text.lines() {
        if !line.contains("\"usage\"") {
            continue;
        }
        let Ok(d) = serde_json::from_str::<serde_json::Value>(line) else { continue };
        if d["type"] != "assistant" {
            continue;
        }
        let u = &d["message"]["usage"];
        let Some(ts) = d["timestamp"].as_str() else { continue };
        let Ok(t) = DateTime::parse_from_rfc3339(ts) else { continue };
        if !u.is_object() {
            continue;
        }
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        let tokens = n("input_tokens")
            + n("output_tokens")
            + n("cache_creation_input_tokens")
            + n("cache_read_input_tokens");
        let key = (
            d["message"]["id"].as_str().unwrap_or("").to_string(),
            d["requestId"].as_str().unwrap_or("").to_string(),
        );
        rows.push((key, t.with_timezone(&Utc), tokens));
    }
    rows
}

fn summarize(list: &[(DateTime<Utc>, u64)], log_files: usize) -> ClaudeStats {
    let now = Utc::now();
    let mut start: Option<DateTime<Utc>> = None;
    let mut last = now;
    let mut block_tokens = 0;

    for &(t, tok) in list {
        let new_block = match start {
            None => true,
            Some(s) => (t - s).num_seconds() >= BLOCK_SECS || (t - last).num_seconds() >= BLOCK_SECS,
        };
        if new_block {
            start = Some(t.with_minute(0).and_then(|t| t.with_second(0)).and_then(|t| t.with_nanosecond(0)).unwrap_or(t));
            block_tokens = 0;
        }
        block_tokens += tok;
        last = t;
    }

    let active = start.is_some_and(|s| {
        (now - s).num_seconds() < BLOCK_SECS && (now - last).num_seconds() < BLOCK_SECS
    });

    let midnight = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    let midnight = Local.from_local_datetime(&midnight).earliest().map(|d| d.with_timezone(&Utc));
    let today: Vec<u64> = list
        .iter()
        .filter(|(t, _)| midnight.is_some_and(|m| *t >= m))
        .map(|(_, tok)| *tok)
        .collect();

    ClaudeStats {
        block_tokens: if active { block_tokens } else { 0 },
        block_start: if active { start } else { None },
        today_tokens: today.iter().sum(),
        today_msgs: today.len() as u32,
        log_files,
    }
}
