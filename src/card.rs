//! What the expanded island says: the current tool's argument, and the turn in four numbers.

/// A whole tool call's input as one short line: a file's name, a command, a pattern.
pub fn summary(input: &serde_json::Value) -> String {
    const KEYS: [&str; 10] = ["file_path", "notebook_path", "path", "command", "pattern", "query", "url", "skill", "description", "prompt"];
    for key in KEYS {
        if let Some(s) = input[key].as_str() {
            let s = if key.ends_with("path") { s.trim_end_matches('/').rsplit('/').next().unwrap_or(s) } else { s };
            return s.lines().next().unwrap_or("").trim().to_string();
        }
    }
    String::new()
}

/// The file one of Claude Code's edit tools touches, by name, and the lines it adds and removes. A replacement counts
/// only what differs once the lines it keeps at either end are set aside.
// shortcut: counts the edit Claude asked for, so one you deny still counts, and a Write over an
// existing file counts all its lines as added; reading tool results in the next request would fix both.
pub fn diff(tool: &str, input: &serde_json::Value) -> Option<(String, u64, u64)> {
    let file = input["file_path"].as_str()?.rsplit('/').next()?.to_string();
    let pair = |e: &serde_json::Value| changed(e["old_string"].as_str().unwrap_or(""), e["new_string"].as_str().unwrap_or(""));
    let (added, removed) = match tool {
        "Edit" => pair(input),
        "MultiEdit" => input["edits"].as_array()?.iter().map(pair).fold((0, 0), |(a, r), (x, y)| (a + x, r + y)),
        "Write" => (input["content"].as_str()?.lines().count() as u64, 0),
        _ => return None,
    };
    Some((file, added, removed))
}

fn changed(old: &str, new: &str) -> (u64, u64) {
    let (o, n): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
    let head = o.iter().zip(&n).take_while(|(a, b)| a == b).count();
    let tail = o[head..].iter().rev().zip(n[head..].iter().rev()).take_while(|(a, b)| a == b).count();
    ((n.len() - head - tail) as u64, (o.len() - head - tail) as u64)
}

/// The todo list's step in progress, counting from 1, how many steps there are, and what the
/// step says it's doing. None when no step is in progress.
pub fn todo(input: &serde_json::Value) -> Option<(usize, usize, String)> {
    let todos = input["todos"].as_array()?;
    let done = todos.iter().filter(|t| t["status"] == "completed").count();
    let now = todos.iter().find(|t| t["status"] == "in_progress")?;
    Some((done + 1, todos.len(), now["activeForm"].as_str().or(now["content"].as_str())?.to_string()))
}

/// Seconds as a clock reads them at a glance: `12s`, `4m 05s`, `1h 02m`.
pub fn duration(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    match s {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m {:02}s", s / 60, s % 60),
        s => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
    }
}

/// Reasoning effort as the footer says it: `high` reads `High effort`; a budget stays as is.
pub fn effort(e: &str) -> String {
    match e {
        "low" | "medium" | "high" | "max" => format!("{}{} effort", e[..1].to_uppercase(), &e[1..]),
        "adaptive" => "Adaptive thinking".into(),
        e => e.into(),
    }
}

/// A token count in three or four characters: `840`, `3.2k`, `1.4M`.
pub fn tokens(n: u64) -> String {
    match n {
        n if n >= 1_000_000 => format!("{:.1}M", n as f64 / 1e6),
        n if n >= 1_000 => format!("{:.1}k", n as f64 / 1e3),
        n => n.to_string(),
    }
}

/// The model's family and version, as people say it: `claude-opus-5-5` reads `Opus 5.5`.
pub fn model_name(id: &str) -> String {
    let id = id.split('[').next().unwrap_or(id).trim_start_matches("claude-");
    let mut parts = id.split('-').filter(|p| !p.is_empty() && p.len() < 8);
    let Some(family) = parts.next() else { return String::new() };
    let mut name: String = family.chars().take(1).flat_map(char::to_uppercase).chain(family.chars().skip(1)).collect();
    let version: Vec<&str> = parts.take_while(|p| p.chars().all(|c| c.is_ascii_digit())).collect();
    if !version.is_empty() { name = format!("{name} {}", version.join(".")) }
    name
}
