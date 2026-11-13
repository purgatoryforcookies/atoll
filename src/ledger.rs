//! Prices: what a request's tokens cost at Anthropic's list rates.

use crate::proxy::Usage;

/// Dollars per million tokens: input, output, cache read. A cache write is 1.25× input for five
/// minutes, 2× for an hour. List prices as of 2026-11; first match wins, so specific ids lead.
const RATES: [(&str, [f64; 3]); 16] = [
    ("fable-5-2", [8.0, 40.0, 0.20]),
    ("mythos-5-2", [8.0, 40.0, 0.20]),
    ("fable-5-1", [10.0, 50.0, 0.25]),
    ("mythos-5-1", [10.0, 50.0, 0.25]),
    ("fable-5", [10.0, 50.0, 1.0]),
    ("mythos-5", [10.0, 50.0, 1.0]),
    ("opus-5-5", [4.0, 20.0, 0.20]),
    ("opus-5", [5.0, 25.0, 0.50]),
    ("opus-4", [5.0, 25.0, 0.50]),
    ("sonnet-5-5", [2.0, 10.0, 0.20]),
    ("sonnet-5", [2.0, 10.0, 0.20]),
    ("sonnet-4", [3.0, 15.0, 0.30]),
    ("haiku-5-5", [0.10, 0.50, 0.01]),
    ("haiku-4", [1.0, 5.0, 0.10]),
    ("haiku", [1.0, 5.0, 0.10]),
    ("sonnet", [3.0, 15.0, 0.30]),
];
/// An unknown model is priced as Opus 5.5.
const FALLBACK: [f64; 3] = [4.0, 20.0, 0.20];

fn rates(model: &str, prompt: u64) -> [f64; 3] {
    // Haiku 5.5 charges more once the prompt passes 100K tokens.
    if model.contains("haiku-5-5") && prompt > 100_000 { return [0.50, 2.50, 0.05] }
    RATES.iter().find(|(id, _)| model.contains(id)).map_or(FALLBACK, |r| r.1)
}

/// Dollars one request's usage cost.
pub fn price(model: &str, u: &Usage) -> f64 {
    let prompt = u.input + u.cache_write + u.cache_read;
    let [i, o, r] = rates(model, prompt);
    let m = |n: u64, rate: f64| n as f64 * rate / 1e6;
    let hour = u.cache_write_1h.min(u.cache_write);
    m(u.input, i) + m(u.cache_write - hour, i * 1.25) + m(hour, i * 2.0) + m(u.cache_read, r) + m(u.output, o)
}

pub fn usd(x: f64) -> String {
    match x.abs() {
        a if a < 1.0 => format!("${x:.3}"),
        a if a < 100.0 => format!("${x:.2}"),
        _ => format!("${x:.0}"),
    }
}
