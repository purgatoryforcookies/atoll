//! Prices: what a request's tokens cost, at Anthropic's list rates or the ones set on the island.

use std::collections::HashMap;
use crate::proxy::Usage;

/// Dollars per million tokens: input, output, cache read. A cache write is 1.25× input for five
/// minutes, 2× for an hour. List prices as of 2026-11, by model id; a model not listed here waits
/// for a price set on the island. Each family's newest leads, as a new one's starting point.
const RATES: [(&str, [f64; 3]); 18] = [
    ("fable-5-2", [8.0, 40.0, 0.20]),
    ("fable-5-1", [10.0, 50.0, 0.25]),
    ("fable-5", [10.0, 50.0, 1.0]),
    ("mythos-5-2", [8.0, 40.0, 0.20]),
    ("mythos-5-1", [10.0, 50.0, 0.25]),
    ("mythos-5", [10.0, 50.0, 1.0]),
    ("opus-5-5", [4.0, 20.0, 0.20]),
    ("opus-5", [5.0, 25.0, 0.50]),
    ("opus-4-6", [5.0, 25.0, 0.50]),
    ("opus-4-5", [5.0, 25.0, 0.50]),
    ("opus-4-1", [5.0, 25.0, 0.50]),
    ("opus-4", [5.0, 25.0, 0.50]),
    ("sonnet-5-5", [2.0, 10.0, 0.20]),
    ("sonnet-5", [2.0, 10.0, 0.20]),
    ("sonnet-4-6", [3.0, 15.0, 0.30]),
    ("sonnet-4-5", [3.0, 15.0, 0.30]),
    ("sonnet-4", [3.0, 15.0, 0.30]),
    ("haiku-5-5", [0.10, 0.50, 0.01]),
];
/// Where a model of a family the list doesn't know starts on the island: Opus 5.5's rates.
const SEED: [f64; 3] = [4.0, 20.0, 0.20];

/// A model's id without its provider, date or context window: `claude-opus-5-5[1m]` and
/// `us.anthropic.claude-opus-4-5-20251101-v1:0` read `opus-5-5` and `opus-4-5`.
pub fn id(model: &str) -> &str {
    let m = model.rsplit_once("claude-").map_or(model, |(_, m)| m);
    let m = m.split(['[', '@']).next().unwrap_or(m);
    let b = m.as_bytes();
    let date = m.match_indices('-').find(|(i, _)| b.len() >= i + 9 && b[i + 1..i + 9].iter().all(u8::is_ascii_digit));
    &m[..date.map_or(m.len(), |(i, _)| i)]
}

/// Rates set on the island, by model id, over the list's; kept in `path` between launches.
#[derive(Default)]
pub struct Prices {
    pub set: HashMap<String, [f64; 3]>,
    path: Option<std::path::PathBuf>,
}

impl Prices {
    /// The rates saved last time; none, and nowhere to save, without a home folder.
    pub fn load() -> Prices {
        let path = std::env::var_os("HOME").map(|h| std::path::Path::new(&h).join("Library/Application Support/Atoll/prices.json"));
        let text = path.as_ref().and_then(|p| std::fs::read_to_string(p).ok()).unwrap_or_default();
        let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let set = v.as_object().into_iter().flatten().filter_map(|(id, r)| {
            let r: Vec<f64> = r.as_array()?.iter().filter_map(serde_json::Value::as_f64).collect();
            Some((id.clone(), <[f64; 3]>::try_from(r).ok()?))
        }).collect();
        Prices { set, path }
    }

    /// Saves the rates set for next time.
    pub fn save(&self) {
        let Some(path) = &self.path else { return };
        let _ = std::fs::create_dir_all(path.parent().unwrap_or(path));
        if let Err(e) = std::fs::write(path, serde_json::to_string_pretty(&self.set).unwrap_or_default()) { eprintln!("atoll: can't save prices: {e}") }
    }

    /// A model's rates for a prompt this long, or None while nobody has priced it.
    pub fn rates(&self, model: &str, prompt: u64) -> Option<[f64; 3]> {
        let id = id(model);
        if let Some(r) = self.set.get(id) { return Some(*r) }
        // Haiku 5.5 charges more once the prompt passes 100K tokens.
        if id == "haiku-5-5" && prompt > 100_000 { return Some([0.50, 2.50, 0.05]) }
        RATES.iter().find(|(k, _)| *k == id).map(|r| r.1)
    }

    /// Dollars one request's usage cost, or None while its model has no price.
    pub fn price(&self, model: &str, u: &Usage) -> Option<f64> {
        let prompt = u.input + u.cache_write + u.cache_read;
        let [i, o, r] = self.rates(model, prompt)?;
        let m = |n: u64, rate: f64| n as f64 * rate / 1e6;
        let hour = u.cache_write_1h.min(u.cache_write);
        Some(m(u.input, i) + m(u.cache_write - hour, i * 1.25) + m(hour, i * 2.0) + m(u.cache_read, r) + m(u.output, o))
    }
}

/// Where the island starts a model nobody has priced: its family's newest list price.
pub fn seed(id: &str) -> [f64; 3] {
    let family = |id: &str| id.split('-').next().unwrap_or("").to_string();
    RATES.iter().find(|(k, _)| family(k) == family(id)).map_or(SEED, |r| r.1)
}

/// A quarter up or down from `v`, landing on the quarter-dollar grid; never below zero.
pub fn step(v: f64, up: bool) -> f64 {
    let q = v * 4.0;
    let n = if up { (q + 1e-6).floor() + 1.0 } else { (q - 1e-6).ceil() - 1.0 };
    (n / 4.0).max(0.0)
}

pub fn usd(x: f64) -> String {
    match x.abs() {
        a if a < 1.0 => format!("${x:.3}"),
        a if a < 100.0 => format!("${x:.2}"),
        _ => format!("${x:.0}"),
    }
}

/// A rate as its stepper says it: `$0.20`, `$4.00`, `$37.50`, `$40`.
pub fn rate(x: f64) -> String {
    if x >= 10.0 && x.fract() == 0.0 { format!("${x:.0}") } else { format!("${x:.2}") }
}
