//! The open card's drawing: what the session is on, the turn in four numbers, a quiet footer,
//! and a pill for each session.

use rune::{Color, El, LineCap, Shape, TextStyle, el};
use crate::{INK, LABEL, Geom, Label, State, card, ledger, rounded};

/// The open card under the top row: the step of Claude's todo list, or its last few tool calls;
/// the turn's time, context, output and cost, each a caption over its value; and a quiet footer
/// with the model and its effort.
pub fn card_labels(s: &State, g: &Geom, alpha: f32, mut shape: Shape, labels: &mut Vec<Label>) -> Shape {
    const CAPTION: TextStyle = TextStyle::new(10.5, 500.0);
    const VALUE: TextStyle = TextStyle::new(15.0, 550.0).tracking(-0.2);
    let (x0, x1) = (g.left() + 18.0, g.right() - 18.0);
    // Clear of the notch's bottom edge, which shows against the island's not-quite-black.
    let y0 = g.top + g.row + 8.0;
    let w = |t: &str| rune::text::measure(t, &[], LABEL, None).0;
    let label = |text: String, x: f32, room: f32, a: f32| Label { text, x, y: y0, room, alpha: alpha * a, color: INK, right: false, head: false, style: LABEL };
    let lead = s.lead();
    // The step of Claude's todo list it's on, and how far through the list, as a hairline under it.
    if let Some((step, total, now)) = lead.and_then(|c| c.todo.as_ref()) {
        let count = format!("{step}/{total}");
        let x = x0 + w(&count) + 8.0;
        labels.push(label(count, x0, 0.0, 0.45));
        labels.push(label(now.clone(), x, x1 - x, 0.9));
        let y = y0 + 20.0;
        shape = shape.line([x0, y], [x1, y], 2.0, LineCap::Round, Color { a: 0.08 * alpha, ..INK });
        let done = (*step - 1) as f32 / (*total).max(1) as f32;
        if done > 0.0 { shape = shape.line([x0, y], [x0 + (x1 - x0) * done, y], 2.0, LineCap::Round, Color { a: 0.5 * alpha, ..INK }) }
    } else if let Some(c) = lead && !c.trail.is_empty() {
        // Without a list, the turn's last few tool calls as chips, oldest first and quieter; those
        // that don't fit drop off the left, and the newest eases in from a little to the right.
        const PAD: f32 = 7.0;
        const GAP: f32 = 6.0;
        const ARG: f32 = 110.0;
        let th = rune::text::measure("Ag", &[], LABEL, None).1;
        let widths: Vec<f32> = c.trail.iter().map(|(n, a, _)| w(n) + if a.is_empty() { 0.0 } else { 5.0 + w(a).min(ARG) } + 2.0 * PAD).collect();
        let (mut first, mut used) = (widths.len(), 0.0);
        while first > 0 && (first == widths.len() || used + GAP + widths[first - 1] <= x1 - x0) {
            used += widths[first - 1] + if first == widths.len() { 0.0 } else { GAP };
            first -= 1;
        }
        let mut x = x0;
        for (i, ((name, arg, at), cw)) in c.trail.iter().zip(&widths).enumerate().skip(first) {
            let k = ((s.t - at) / 0.3).clamp(0.0, 1.0);
            let e = k * k * (3.0 - 2.0 * k);
            let a = [1.0, 0.7, 0.5, 0.35][(c.trail.len() - 1 - i).min(3)] * e;
            let cx = x + 8.0 * (1.0 - e);
            shape = shape.fill(rounded(cx + cw * 0.5, y0 - 3.0, cw * 0.5, th + 6.0, (th + 6.0) * 0.5), Color { a: 0.08 * alpha * a, ..INK });
            labels.push(label(name.clone(), cx + PAD, 0.0, 0.5 * a));
            if !arg.is_empty() { labels.push(label(arg.clone(), cx + PAD + w(name) + 5.0, w(arg).min(ARG) + 1.0, 0.9 * a)) }
            x += cw + GAP;
        }
    }
    // Several sessions at once: time runs from the first of their turns to start until the last
    // ends, context reads the fullest, as the ring does, output and cost add up.
    let focus = s.focus();
    let took = focus.iter().filter_map(|c| c.turn_at).reduce(f64::min).map(|at| {
        let end = if focus.iter().any(|c| c.turn_end.is_none()) { s.last_t } else { focus.iter().filter_map(|c| c.turn_end).fold(at, f64::max) };
        card::duration(end - at)
    });
    let (out, cost) = s.turn();
    let stats = [
        ("Time", took.unwrap_or_else(|| "–".into())),
        ("Context", format!("{:.0}%", s.filled() * 100.0)),
        ("Output", if out > 0 { card::tokens(out) } else { "–".into() }),
        ("Cost", if cost > 0.0 { ledger::usd(cost) } else { "–".into() }),
    ];
    let col = (x1 - x0) / stats.len() as f32;
    for (i, (caption, value)) in stats.into_iter().enumerate() {
        let x = x0 + col * i as f32;
        labels.push(Label { text: caption.into(), x, y: y0 + 30.0, room: col - 6.0, alpha: alpha * 0.45, color: INK, right: false, head: false, style: CAPTION });
        labels.push(Label { text: value, x, y: y0 + 44.0, room: col - 6.0, alpha, color: INK, right: false, head: false, style: VALUE });
    }
    let total = if s.total_cost > 0.0 { format!("{} total", ledger::usd(s.total_cost)) } else { String::new() };
    let (model, effort) = s.lead().map_or((String::new(), String::new()), |c| (card::model_name(&c.model), card::effort(&c.effort)));
    let files = focus.iter().map(|c| c.files.len()).sum::<usize>();
    let files = match files { 0 => String::new(), 1 => "1 file".into(), n => format!("{n} files") };
    let footer: Vec<String> = [model, effort, files, total].into_iter().filter(|t| !t.is_empty()).collect();
    labels.push(Label { text: footer.join(" · "), x: x0, y: y0 + 74.0, room: x1 - x0, alpha: alpha * 0.4, color: INK, right: false, head: false, style: CAPTION });
    shape
}

/// A pill for each session, centred along the card's bottom; the ones the card speaks for are
/// lit. Round boxes rather than polygons, so their ends stay smooth at this size.
pub fn pills(s: &State, g: &Geom, alpha: f32) -> Vec<El> {
    const W: f32 = 14.0;
    const H: f32 = 4.0;
    const GAP: f32 = 5.0;
    let n = s.chats.len();
    if n < 2 { return Vec::new() }
    let focus = s.focus();
    let (x0, y, k) = (g.w * 0.5 - (W * n as f32 + GAP * (n - 1) as f32) * 0.5, g.top + g.row + 106.0, g.k);
    s.chats.iter().enumerate().map(|(i, c)| {
        let a = if focus.iter().any(|f| f.id == c.id) { 0.85 } else { 0.22 } * alpha;
        el().absolute((x0 + (W + GAP) * i as f32) * k, y * k).size(W * k, H * k).radius(H * 0.5 * k).pixel_snap(false).fill(Color { a, ..INK })
    }).collect()
}
