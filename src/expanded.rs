//! The open card's drawing: what the session is on, the turn in four numbers, a quiet footer,
//! and a pill for each session.

use rune::{Color, El, LineCap, Shape, TextStyle, el};
use crate::{INK, LABEL, Geom, Label, State, card, chat::{self, Chat}, ledger, rounded};

/// The open card under the top row: the step of Claude's todo list, or its last few tool calls;
/// the turn's time and context, and its output tokens and those since launch with what they cost
/// under them, each a caption over its value; and a quiet footer with the model, its effort and
/// the files touched.
pub fn card_labels(s: &State, g: &Geom, alpha: f32, mut shape: Shape, labels: &mut Vec<Label>) -> Shape {
    const CAPTION: TextStyle = TextStyle::new(10.5, 500.0);
    const VALUE: TextStyle = TextStyle::new(15.0, 550.0).tracking(-0.2);
    let (x0, x1) = (g.left() + 18.0, g.right() - 18.0);
    // Clear of the notch's bottom edge, which shows because the panel's black is lighter than the notch's.
    let y0 = g.top + g.row + 8.0;
    let w = |t: &str| rune::text::measure(t, &[], LABEL, None).0;
    let label = |text: String, x: f32, room: f32, a: f32| Label { text, x, y: y0, room, alpha: alpha * a, color: INK, right: false, head: false, style: LABEL };
    let lead = s.lead();
    // The step of Claude's todo list it's on, and how far through the list, as a hairline under it.
    if let Some((step, total, now)) = lead.and_then(|c| c.todo.as_ref()) {
        let count = format!("{step}/{total}");
        let x = x0 + w(&count) + 8.0;
        labels.push(label(count, x0, 0.0, 0.6));
        labels.push(label(now.clone(), x, x1 - x, 0.9));
        let y = y0 + 20.0;
        shape = shape.line([x0, y], [x1, y], 2.0, LineCap::Round, Color { a: 0.08 * alpha, ..INK });
        // The step in progress counts as half done, so step 1 of 1 doesn't read as stalled.
        let done = (*step as f32 - 0.5) / (*total).max(1) as f32;
        shape = shape.line([x0, y], [x0 + (x1 - x0) * done, y], 2.0, LineCap::Round, Color { a: 0.5 * alpha, ..INK });
    } else if let Some(c) = lead && let Some((name, arg, at)) = c.trail.last() {
        // Without a list, the newest tool call owns the band: its tool as a badge, then its argument
        // at full width, a command or pattern in mono. The calls before it sit on the right as dim
        // names, newest first, the oldest going first when they would crowd the argument. The
        // badge and argument ease in from a little to the right.
        const PAD: f32 = 7.0;
        const GAP: f32 = 6.0;
        const MONO: TextStyle = TextStyle::new(10.0, 500.0).family(rune::Family::SystemMono);
        let wc = |t: &str| rune::text::measure(t, &[], CAPTION, None).0;
        let mut before: Vec<&str> = c.trail.iter().rev().skip(1).map(|(n, _, _)| n.as_str()).collect();
        let marks = |b: &[&str]| b.iter().map(|n| wc(n) + GAP).sum::<f32>();
        while marks(&before) > (x1 - x0) * 0.4 { before.pop(); }
        let mut x = x1 - marks(&before) + GAP;
        for n in &before {
            labels.push(Label { text: n.to_string(), x, y: y0, room: 0.0, alpha: alpha * 0.55, color: INK, right: false, head: false, style: CAPTION });
            x += wc(n) + GAP;
        }
        let k = ((s.t - at) / 0.3).clamp(0.0, 1.0);
        let e = k * k * (3.0 - 2.0 * k);
        let cx = x0 + 8.0 * (1.0 - e);
        let th = rune::text::measure("Ag", &[], CAPTION, None).1;
        let bw = wc(name) + 2.0 * PAD;
        shape = shape.fill(rounded(cx + bw * 0.5, y0 - 3.0, bw * 0.5, th + 6.0, (th + 6.0) * 0.5), Color { a: 0.12 * alpha * e, ..INK });
        labels.push(Label { text: name.clone(), x: cx + PAD, y: y0, room: 0.0, alpha: alpha * 0.9 * e, color: INK, right: false, head: false, style: CAPTION });
        if !arg.is_empty() {
            let style = if matches!(name.as_str(), "Bash" | "Grep" | "Glob") { MONO } else { LABEL };
            let ax = cx + bw + 7.0;
            let room = x1 - marks(&before) - 4.0 - ax;
            labels.push(Label { text: arg.clone(), x: ax, y: y0, room: room.max(0.0), alpha: alpha * 0.9 * e, color: INK, right: false, head: false, style });
        }
    } else {
        // Nothing to list yet: say so, rather than leave the band blank.
        let text = if s.working() { "No tool calls yet" } else { "–" };
        labels.push(label(text.into(), x0, 0.0, 0.6));
    }
    // Several sessions at once: time runs from the first of their turns to start until the last
    // ends, context reads the fullest, as the ring does, output and cost add up.
    let focus = s.focus();
    let took = focus.iter().filter_map(|c| c.turn_at).reduce(f64::min).map(|at| {
        let end = if focus.iter().any(|c| c.turn_end.is_none()) { s.last_t } else { focus.iter().filter_map(|c| c.turn_end).fold(at, f64::max) };
        card::duration(end - at)
    });
    let (out, cost) = s.turn();
    // A dash is a number not known yet; context is unknown until a request has come back.
    let filled = s.filled();
    let tokens = |n: u64| if n > 0 { card::tokens(n) } else { "–".into() };
    let usd = |d: f64| if d > 0.0 { ledger::usd(d) } else { String::new() };
    // Output tokens lead; what they cost sits under them, smaller.
    let stats = [
        ("Time", took.unwrap_or_else(|| "–".into()), String::new()),
        ("Context", if filled > 0.0 { format!("{:.0}%", filled * 100.0) } else { "–".into() }, String::new()),
        ("Output", tokens(out), usd(cost)),
        ("Total", tokens(s.total_out), usd(s.total_cost)),
    ];
    // The first starts at the left edge and the last ends at the right, with even gaps between.
    // Digits are measured as zeros, so the row holds still while the numbers count.
    let zeros = |t: &str, style| rune::text::measure(&t.replace(|c: char| c.is_ascii_digit(), "0"), &[], style, None).0;
    let widths: Vec<f32> = stats.iter().map(|(c, v, d)| zeros(c, CAPTION).max(zeros(v, VALUE)).max(zeros(d, CAPTION))).collect();
    let gap = ((x1 - x0 - widths.iter().sum::<f32>()) / (stats.len() - 1) as f32).max(0.0);
    let mut x = x0;
    for ((caption, value, cost), w) in stats.into_iter().zip(widths) {
        labels.push(Label { text: caption.into(), x, y: y0 + 30.0, room: 0.0, alpha: alpha * 0.6, color: INK, right: false, head: false, style: CAPTION });
        labels.push(Label { text: value, x, y: y0 + 44.0, room: 0.0, alpha, color: INK, right: false, head: false, style: VALUE });
        if !cost.is_empty() { labels.push(Label { text: cost, x, y: y0 + 64.0, room: 0.0, alpha: alpha * 0.55, color: INK, right: false, head: false, style: CAPTION }) }
        x += w + gap;
    }
    // Who is working on the left and the files the turn touched on the right, as the stats run
    // edge to edge. The stats add up every session in focus, which the lit pills below say; the
    // model is the lead's. Short of room, the effort goes rather than letters.
    let (model, effort) = s.lead().map_or((String::new(), String::new()), |c| (card::model_name(&c.model), card::effort(&c.effort)));
    let files = match focus.iter().map(|c| c.files.len()).sum::<usize>() { 0 => String::new(), 1 => "1 file".into(), n => format!("{n} files") };
    let both = [&model, &effort].into_iter().filter(|t| !t.is_empty()).map(String::as_str).collect::<Vec<_>>().join(" · ");
    let room = x1 - x0 - zeros(&files, CAPTION) - 16.0;
    let left = if zeros(&both, CAPTION) <= room { both } else { model };
    let foot = |text: String, x: f32, room: f32, right: bool| Label { text, x, y: y0 + 88.0, room, alpha: alpha * 0.55, color: INK, right, head: false, style: CAPTION };
    labels.push(foot(left, x0, room.max(0.0), false));
    labels.push(foot(files, x1, 0.0, true));
    shape
}

/// A pill for each session, centred along the card's bottom; the ones the card speaks for are
/// lit. Round boxes rather than polygons, so their ends stay smooth at this size.
pub fn pills(s: &State, g: &Geom, alpha: f32) -> Vec<El> {
    const W: f32 = 14.0;
    const H: f32 = 4.0;
    const GAP: f32 = 5.0;
    let row = pilled(s);
    let n = row.len();
    if n < 2 { return Vec::new() }
    let focus = s.focus();
    let (x0, y, k) = (g.w * 0.5 - (W * n as f32 + GAP * (n - 1) as f32) * 0.5, g.top + g.row + 120.0, g.k);
    row.into_iter().enumerate().map(|(i, c)| {
        let a = if focus.iter().any(|f| f.id == c.id) { 0.85 } else { 0.22 } * alpha;
        el().absolute((x0 + (W + GAP) * i as f32) * k, y * k).size(W * k, H * k).radius(H * 0.5 * k).pixel_snap(false).fill(Color { a, ..INK })
    }).collect()
}

/// The sessions that keep a pill: any with a request in flight, or that streamed within `IDLE`.
pub fn pilled(s: &State) -> Vec<&Chat> {
    s.chats.iter().filter(|c| s.last_t - c.seen < chat::IDLE || s.sessions.values().any(|id| *id == c.id)).collect()
}
