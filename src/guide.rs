//! The card's setup guide, open at launch until Claude Code's settings send it through Atoll:
//! open the settings, copy the lines to paste in, done; and what that costs while Atoll is closed.

use rune::{Color, El, LineCap, Shape, TextStyle, Tx, el};
use crate::{INK, LABEL, Geom, Label, State, rounded, screen};

pub struct Guide {
    pub port: u16,
    /// The lines are on the clipboard; the button says so.
    pub copied: bool,
    /// When Open was last pressed, in the island's seconds.
    pub opened: Option<f32>,
}

/// How long Open spins and takes no press: the editor can take a moment to come up.
const OPENING: f32 = 2.0;

impl Guide {
    pub fn new(port: u16) -> Guide { Guide { port, copied: false, opened: None } }

    /// Whether Open is still spinning at `t`.
    pub fn opening(&self, t: f32) -> bool { self.opened.is_some_and(|at| t - at < OPENING) }
}

/// Claude Code's settings for every project.
pub fn settings() -> Option<std::path::PathBuf> {
    Some(std::path::Path::new(&std::env::var_os("HOME")?).join(".claude/settings.json"))
}

/// Whether these settings already send Claude Code to Atoll's port.
// shortcut: reads only the user settings file; `ANTHROPIC_BASE_URL` set in the shell or a project
// still shows the guide once per launch, until a request arrives or Done.
pub fn configured(settings: &str, port: u16) -> bool {
    let v: serde_json::Value = serde_json::from_str(settings).unwrap_or_default();
    v["env"]["ANTHROPIC_BASE_URL"].as_str().is_some_and(|u| u.trim_end_matches('/').ends_with(&format!(":{port}")))
}

/// What to paste into the settings: the relay as the API, and the hook that says a permission prompt is up.
pub fn snippet(port: u16) -> String {
    let hook = format!("curl -s -m 1 -X POST http://127.0.0.1:{port}/atoll -d @- >/dev/null");
    serde_json::to_string_pretty(&serde_json::json!({
        "env": { "ANTHROPIC_BASE_URL": format!("http://127.0.0.1:{port}") },
        "hooks": { "Notification": [{ "matcher": "permission_prompt", "hooks": [{ "type": "command", "command": hook }] }] }
    })).unwrap()
}

const CAPTION: TextStyle = TextStyle::new(10.5, 500.0);
/// Label size, a step heavier, as a system notification sets its title over its body.
const TITLE: TextStyle = TextStyle::new(10.75, 650.0);
const PAD: f32 = 9.0;
/// Each step's top, down from the title: steps close together, the notes set apart below.
const STEPS: [f32; 3] = [22.0, 45.0, 68.0];
const NOTES: f32 = 96.0;

/// The steps and their buttons as the card's text, and the buttons' fills on `shape`.
pub fn labels(guide: &Guide, g: &Geom, t: f32, alpha: f32, mut shape: Shape, labels: &mut Vec<Label>) -> Shape {
    let (x0, x1, y0) = (g.left() + 18.0, g.right() - 18.0, g.top + g.row + 8.0);
    let label = |text: &str, x: f32, y: f32, a: f32, style| Label { text: text.into(), x, y, room: x1 - x, alpha: alpha * a, color: INK, right: false, head: false, style };
    labels.push(label("Connect Claude Code", x0, y0, 1.0, TITLE));
    let steps = ["Open Claude Code's settings", "Copy Atoll's lines into them", "Start a new Claude Code session"];
    let (x, w) = column(g);
    let th = rune::text::measure("Ag", &[], CAPTION, None).1;
    for (i, ((step, y), button)) in steps.into_iter().zip(STEPS).zip(words(guide)).enumerate() {
        labels.push(label(&format!("{}", i + 1), x0, y0 + y, 0.6, LABEL));
        labels.push(Label { room: x - 8.0 - (x0 + 14.0), ..label(step, x0 + 14.0, y0 + y, 0.9, LABEL) });
        // The tool badge's capsule, its words centred so "Copy" turning "Copied" doesn't shift it.
        shape = shape.fill(rounded(x + w * 0.5, y0 + y - 3.0, w * 0.5, th + 6.0, (th + 6.0) * 0.5), Color { a: 0.12 * alpha, ..INK });
        if i == 0 && guide.opening(t) {
            shape = spinner(shape, [x + w * 0.5, y0 + y - 3.0 + (th + 6.0) * 0.5], t, alpha);
            continue;
        }
        let tw = rune::text::measure(button, &[], CAPTION, None).0;
        labels.push(label(button, x + (w - tw) * 0.5, y0 + y, 0.9, CAPTION));
    }
    labels.push(label("Merge them with anything already in the file.", x0, y0 + NOTES, 0.55, CAPTION));
    labels.push(label("Claude Code can't reach the API while Atoll is closed.", x0, y0 + NOTES + 13.0, 0.55, CAPTION));
    shape
}

/// The system's activity indicator: eight round spokes, the lit one stepping clockwise twelve
/// times a second and the ones behind it fading.
fn spinner(mut shape: Shape, [cx, cy]: [f32; 2], t: f32, alpha: f32) -> Shape {
    let lit = (t * 12.0) as usize;
    for i in 0..8 {
        let a = std::f32::consts::TAU * i as f32 / 8.0;
        let (sin, cos) = a.sin_cos();
        let fade = 1.0 - ((lit + 8 - i % 8) % 8) as f32 / 8.0;
        shape = shape.line([cx + sin * 2.5, cy - cos * 2.5], [cx + sin * 5.0, cy - cos * 5.0], 1.5, LineCap::Round, Color { a: (0.15 + 0.75 * fade) * alpha, ..INK });
    }
    shape
}

/// Each step's button words.
fn words(guide: &Guide) -> [&'static str; 3] { ["Open", if guide.copied { "Copied" } else { "Copy" }, "Done"] }

/// The buttons' shared left edge and width, right-aligned to the card's edge: one width, the
/// widest word's, so they stack as a column.
fn column(g: &Geom) -> (f32, f32) {
    let w = ["Open", "Copied", "Done"].map(|t| rune::text::measure(t, &[], CAPTION, None).0).into_iter().fold(0.0, f32::max) + 2.0 * PAD;
    (g.right() - 18.0 - w, w)
}

/// What takes each button's press, over the island's own press.
/// While Open spins, its press goes to the island's beneath, as the rest of the card's does.
pub fn presses(guide: &Guide, g: &Geom, t: f32) -> Vec<El> {
    let (k, y0, port) = (g.k, g.top + g.row + 8.0, guide.port);
    let th = rune::text::measure("Ag", &[], CAPTION, None).1;
    let (x, w) = column(g);
    let hit = |i: usize| el().absolute(x * k, (y0 + STEPS[i] - 3.0) * k).size(w * k, (th + 6.0) * k);
    let open = (!guide.opening(t)).then(|| hit(0).on_press_tx(|s: &mut State, tx: &mut Tx| {
        screen::open(&settings().unwrap_or_default());
        if let Some(g) = &mut s.guide { g.opened = Some(s.t) }
        tx.wake_after(std::time::Duration::ZERO)
    }));
    open.into_iter().chain([
        hit(1).on_press(move |s: &mut State| {
            screen::copy(&snippet(port));
            if let Some(g) = &mut s.guide { g.copied = true }
        }),
        hit(2).on_press_tx(|s: &mut State, tx: &mut Tx| { (s.guide, s.expanded) = (None, false); tx.wake_after(std::time::Duration::ZERO) }),
    ]).collect()
}
