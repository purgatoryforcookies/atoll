//! The scripted stream `--scene demo` plays.

use crate::State;
use crate::proxy::{Ev, Kind, Usage};

/// A turn on a nine-second loop: think, read a file, write, edit one, write again, then wait.
/// A second session works through the middle of it.
pub fn demo(s: &mut State, dt: f32) {
    let Some(t0) = s.demo else { return };
    let t1 = t0 + dt;
    s.demo = Some(t1);
    let (a, b) = (t0 % 9.0, t1 % 9.0);
    let start = |s: &mut State, id| {
        let session = if id == 2 { "b" } else { "a" };
        s.apply(Ev::Start { id, model: "claude-opus-5-5".into(), window: 200_000, session: session.into() });
    };
    let stop = |s: &mut State, id, reason: &str| {
        let usage = Usage { input: 6, cache_read: 118_000 + id * 4_000, output: 1_600, ..Default::default() };
        s.apply(Ev::Usage { id, usage });
        s.apply(Ev::Stop { id, reason: reason.into() });
        s.apply(Ev::End { id, status: 200 });
    };
    if b < a || t0 == 0.0 { start(s, 0); s.apply(Ev::Effort { id: 0, effort: "high".into() }) }
    if a < 2.9 && b >= 2.9 { s.apply(Ev::Diff { id: 0, file: "main.rs".into(), added: 38, removed: 6 }) }
    if a < 3.0 && b >= 3.0 { stop(s, 0, "tool_use") }
    if a < 0.5 && b >= 0.5 {
        start(s, 2);
        s.apply(Ev::Todo { id: 2, step: Some((3, 7, "Fixing the session parser".into())) });
    }
    if a < 4.2 && b >= 4.2 { start(s, 1) }
    if a < 5.0 && b >= 5.0 { stop(s, 2, "end_turn") }
    if a < 5.3 && b >= 5.3 { s.apply(Ev::Diff { id: 1, file: "card.rs".into(), added: 4, removed: 1 }) }
    if a < 5.6 && b >= 5.6 { stop(s, 1, "end_turn") }
    let (id, kind) = match b {
        b if b < 1.4 => (0, Kind::Thinking),
        b if b < 1.9 => (0, Kind::Tool("Read".into())),
        b if b < 2.4 => (0, Kind::Text),
        b if b < 3.0 => (0, Kind::Tool("Edit".into())),
        b if (4.2..5.6).contains(&b) => (1, Kind::Text),
        _ => return,
    };
    if s.kind.as_ref() != Some(&kind) {
        s.apply(Ev::Block { id, kind: kind.clone() });
        if let Kind::Tool(n) = &kind {
            let file = if n == "Read" { "card.rs" } else { "main.rs" };
            s.apply(Ev::Delta { id, kind: kind.clone(), text: format!(r#"{{"file_path":"src/{file}"}}"#) });
            s.apply(Ev::Arg { id, arg: file.into() });
        }
    }
    // Bursty, like tokens arriving in uneven chunks.
    const WORDS: [&str; 12] = ["The ", "island ", "resizes ", "to ", "fit ", "its ", "label, ", "then ", "springs ", "back ", "when ", "done. "];
    if (t1 * 30.0) as u32 != (t0 * 30.0) as u32 && (t1 * 7.3).sin() > -0.3 { s.apply(Ev::Delta { id, kind, text: WORDS[(t1 * 30.0) as usize % WORDS.len()].into() }) }
}
