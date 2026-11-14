//! Atoll: Claude's traffic as a Dynamic Island on a MacBook's notch. A localhost relay forwards
//! every API call untouched; while an answer streams, the notch grows two
//! wings, a dot on the left in the colour of what Claude is doing and a waveform on the right
//! that moves with each piece of the stream, and beside the dot what Claude is on: thinking,
//! writing, or the tool it called. The island resizes to fit, like a Dynamic Island. When the
//! turn ends it folds into a compact island saying it's your turn until the next request.
//!
//! Point at the island and it swells a little. Click it while Claude works and it opens into
//! a card: the tool's argument, how long the turn has run, the context used, the output and
//! what the turn cost; another click folds it back. With several Claude Code sessions, a row
//! of small pills along its bottom, one each, lights the one the numbers are for:
//! the last to speak, or every session mid-turn at once, their numbers mixed.
//!
//! Beside the waveform the turn counts the lines its edits add and remove, `+42 −7`, counting
//! up as each lands. While Claude thinks the waveform turns into a slow swell sweeping across
//! it. Turned away for a rate limit, the island says when it lifts and its ring drains to it.
//!
//! ```sh
//! cargo run -p atoll --release
//! ANTHROPIC_BASE_URL=http://127.0.0.1:8788 claude
//! ```
//!
//! It sits on the screen with the menu bar, and moves when displays come and go. On a screen
//! without a notch it floats as a small pill that grows the same way; `ATOLL_PILL=1` floats it
//! on any screen. `ATOLL_PORT` changes the port, `ATOLL_UPSTREAM` the API it relays to.
//! `-- --scene demo` plays a scripted stream, `-- --scene pill` plays it on a screen without
//! a notch, `-- --scene card` with the card open, `-- --scene limit` waits out a rate limit, and `-- --scene guide` opens the setup guide, and
//! `-- --scene prices` the prices, with a model nobody has priced.

mod card;
mod chat;
mod demo;
mod expanded;
mod guide;
mod ledger;
mod prices;
mod proxy;
mod screen;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use chat::{BUSY, CHATS, Chat};
use proxy::{Ev, Kind, Usage};
use rune::{App, AppSpec, Color, Cx, El, LineCap, MacWindowOptions, RunOptions, Shape, Spring, TextStyle, Tx, Waker, WindowOptions, WindowSpec, canvas, el, oklch, text};
use screen::{CARD_H, PILLS, PILL_H, PILL_TOP, STRETCH, Screen, Shared};

/// The widest a wing reaches past the notch, in points; labels past it are cut with "…".
const WING: f32 = 132.0;
/// Where the label starts inside the left wing, past the dot.
const LABEL_X: f32 = 32.0;
/// Medium, not bold, and a touch small, as the island sets its own text.
const LABEL: TextStyle = TextStyle::new(10.75, 500.0);
/// Off-white, so the label sits in the black rather than shouting from it.
const INK: Color = Color { r: 0.86, g: 0.86, b: 0.88, a: 1.0 };
const YOUR_TURN: &str = "Your turn";
/// The concave flare where the island meets the screen's top edge, like the notch's own.
const EAR: f32 = 6.0;
const BARS: usize = 5;
/// Seconds the wings stay open after the stream goes quiet.
const LINGER: f32 = 1.2;
/// Seconds the dot pulses when a turn ends, then rests.
const PULSE: f32 = 2.4;
/// Seconds a shorter label must stay before the working island narrows to it.
const HOLD: f32 = 1.5;
/// Seconds of waiting before "Your turn" goes and only the dot stays.
const DIM: f32 = 300.0;
/// How far each wing swells under the pointer, so the island reads as something to click.
const HOVER: f32 = 5.0;
/// The diff tally's right edge, in from the wing's end, just inside the waveform, and the gap
/// between its two counts.
const DIFF_X: f32 = 48.0;
const DIFF_GAP: f32 = 5.0;
/// Lines added and removed: a green and a red muted to tint the grey rather than shout.
const ADDED: Color = Color { r: 0.56, g: 0.80, b: 0.62, a: 1.0 };
const REMOVED: Color = Color { r: 0.90, g: 0.55, b: 0.52, a: 1.0 };
/// A rate limit's amber, and needing you.
const AMBER: Color = Color { r: 0.98, g: 0.74, b: 0.36, a: 1.0 };
/// The expanded card's width and corner radius.
const CARD_W: f32 = 320.0;
const CARD_R: f32 = 22.0;

#[derive(Default)]
struct State {
    inbox: Arc<Mutex<Vec<Ev>>>,
    screen: Screen,
    /// The screen the window was last sized and placed for.
    placed: Screen,
    shared: Arc<Shared>,
    live: HashSet<u64>,
    /// Claude Code's own side requests (titles, command checks), which run on Haiku; ignored.
    side: HashSet<u64>,
    /// The session of each live request: every session shows on the island, and they're counted.
    sessions: HashMap<u64, String>,
    /// Each session heard from lately, in the order they first spoke, and the one that spoke last.
    chats: Vec<Chat>,
    last: String,
    kind: Option<Kind>,
    /// The turn ended on something other than a tool call: Claude waits for you.
    waiting: bool,
    /// The answer stopped to call a tool, which Claude Code is now running.
    running: bool,
    /// Claude Code is asking you to allow that tool.
    needs: bool,
    /// Seconds since the wait began, for the dot's two pulses.
    waited: f32,
    /// 0 to 1: the waiting content against the working content, cross-faded.
    wait_k: f32,
    label: String,
    old_label: String,
    /// 0 to 1: the old label fading out as the new one fades in.
    swap: f32,
    /// Characters streamed, decaying over a third of a second: the stream's loudness.
    flow: f32,
    quiet: f32,
    /// The wings' spring: how far each reaches past the notch, in points.
    wing: f32,
    wing_v: f32,
    /// The widest the working island has needed lately, and seconds since it needed less.
    held: f32,
    narrower: f32,
    /// The pointer is over the island, and the card is open: each with its spring.
    hover: bool,
    expanded: bool,
    card: f32,
    card_v: f32,
    /// The bars' heights, gliding toward the stream's loudness plus each bar's kick, which a
    /// piece of the stream nudges and which ebbs away; and the loudness, eased.
    bars: [f32; BARS],
    kicks: [f32; BARS],
    next_bar: usize,
    level: f32,
    /// Only a running tool's low wave moves, which reads the same at a quarter of the frames.
    wave: bool,
    failed: f32,
    /// The ring's eased share of the context window.
    ring: f32,
    /// The lines added and removed on show, counting up to what the sessions in focus changed.
    shown: [f32; 2],
    /// A rate limit: when it lifts and how long it was, on poll's clock.
    limit: Option<(f64, f64)>,
    /// 0 to 1: the waveform's thinking sweep against its stream-driven bars.
    think: f32,
    /// Each request's model and usage until it ends.
    requests: HashMap<u64, (String, Usage)>,
    /// Dollars of every finished request since Atoll opened.
    total_cost: f64,
    /// Rates set on the island, and finished requests on a model nobody has priced yet.
    prices: ledger::Prices,
    unpriced: Vec<prices::Unpriced>,
    /// The card is open on the prices, and the stepper held down on it.
    pricing: bool,
    stepper: Option<prices::Held>,
    /// Output tokens of every finished request since Atoll opened.
    total_out: u64,
    t: f32,
    last_t: f64,
    demo: Option<f32>,
    /// The setup guide, which the card shows instead until Claude Code is sent through Atoll.
    guide: Option<guide::Guide>,
}

/// Where the island is this frame, in layout points: the window's size and zoom, the top
/// row's top and height, and the whole body's half-width, height and corner radius.
struct Geom {
    w: f32,
    wh: f32,
    k: f32,
    top: f32,
    row: f32,
    half: f32,
    h: f32,
    r: f32,
}

impl Geom {
    fn left(&self) -> f32 { self.w * 0.5 - self.half }
    fn right(&self) -> f32 { self.w * 0.5 + self.half }
    fn contains(&self, [x, y]: [f32; 2]) -> bool {
        (self.left() - 2.0..=self.right() + 2.0).contains(&x) && (self.top - 2.0..=self.top + self.h + 2.0).contains(&y)
    }
}

impl State {
    fn apply(&mut self, ev: Ev) {
        let id = match &ev {
            Ev::Start { id, .. } | Ev::Block { id, .. } | Ev::Delta { id, .. } | Ev::Arg { id, .. } | Ev::Usage { id, .. } | Ev::Stop { id, .. } | Ev::End { id, .. } | Ev::Effort { id, .. } | Ev::Diff { id, .. } | Ev::Limit { id, .. } | Ev::Todo { id, .. } => Some(*id),
            _ => None,
        };
        if id.is_some_and(|id| self.side.contains(&id)) {
            if let Ev::End { id, .. } = ev { self.side.remove(&id); }
            return
        }
        // A stopped request's last chunk can land after its end; it says nothing now.
        if !matches!(ev, Ev::Start { .. }) && id.is_some_and(|id| !self.live.contains(&id)) { return }
        let now = self.last_t;
        // The card follows whichever session spoke last.
        if let Some(c) = id.and_then(|id| self.sessions.get(&id)).and_then(|sid| self.chats.iter_mut().find(|c| c.id == *sid)) {
            c.seen = now;
            if self.last != c.id { self.last = c.id.clone() }
        }
        match ev {
            // shortcut: Haiku marks a side request; a session whose main model is Haiku shows nothing.
            // A request with no model, such as a check on `/api/hello`, reads as its path: not a turn either.
            Ev::Start { id, model, .. } if model.contains("haiku") || model.starts_with('/') => { self.side.insert(id); }
            Ev::Start { id, model, window, session } => {
                let live: HashSet<&String> = self.sessions.values().collect();
                self.chats.retain(|c| now - c.seen < chat::IDLE || live.contains(&c.id));
                if !self.chats.iter().any(|c| c.id == session) {
                    if self.chats.len() >= CHATS && let Some(i) = (0..self.chats.len()).min_by(|&a, &b| self.chats[a].seen.total_cmp(&self.chats[b].seen)) { self.chats.remove(i); }
                    self.chats.push(Chat { id: session.clone(), ..Chat::default() });
                }
                let c = self.chats.iter_mut().find(|c| c.id == session).unwrap();
                c.begin(now);
                (c.model, c.window, c.seen) = (model.clone(), window, now);
                self.requests.insert(id, (model, Usage::default()));
                self.last = session.clone();
                self.sessions.insert(id, session);
                self.live.insert(id);
                self.guide = None;
                self.quiet = 0.0;
                (self.waiting, self.running, self.needs) = (false, false, false);
            }
            Ev::Block { id, kind } => {
                if let Kind::Tool(n) | Kind::Skill(n) = &kind {
                    let (name, t) = (n.rsplit("__").next().unwrap_or(n).to_string(), self.t);
                    if let Some(c) = self.chat(id) {
                        if c.trail.len() >= chat::TRAIL { c.trail.remove(0); }
                        c.trail.push((name, String::new(), t));
                    }
                }
                self.kind = Some(kind);
            }
            Ev::Stop { id, reason } => match reason.as_str() {
                "tool_use" => self.running = true,
                "pause_turn" => {}
                _ => {
                    (self.waiting, self.waited) = (true, 0.0);
                    if let Some(c) = self.chat(id) { c.turn_end = Some(now) }
                }
            },
            Ev::Arg { id, arg } => if let Some((_, a, _)) = self.chat(id).and_then(|c| c.trail.last_mut()) { *a = arg.chars().take(60).collect() },
            Ev::Delta { text, .. } => {
                self.limit = None;
                let n = text.chars().count() as f32;
                self.flow += n;
                self.quiet = 0.0;
                // Each piece of the stream nudges the next bar, so the waveform walks with it;
                // a flood of text saturates gently rather than slamming the bars.
                let k = &mut self.kicks[self.next_bar];
                *k = (*k + 0.06 + n / 300.0).min(0.45);
                self.next_bar = (self.next_bar + 2) % BARS;
            }
            Ev::Usage { id, usage } => {
                if let Some(c) = self.chat(id) { c.context = usage.input + usage.cache_read + usage.cache_write }
                if let Some(r) = self.requests.get_mut(&id) { r.1 = usage }
            }
            Ev::End { id, status } => {
                self.live.remove(&id);
                self.charge(id);
                self.sessions.remove(&id);
                if status != 0 && !(200..300).contains(&status) { self.failed = 1.0 }
            }
            Ev::Effort { id, effort } => if let Some(c) = self.chat(id) { c.effort = effort },
            Ev::Todo { id, step } => if let Some(c) = self.chat(id) { c.todo = step },
            Ev::Diff { id, file, added, removed } => if let Some(c) = self.chat(id) {
                (c.added, c.removed) = (c.added + added, c.removed + removed);
                if !c.files.contains(&file) { c.files.push(file) }
            },
            Ev::Limit { secs, .. } => if secs > 0.0 { self.limit = Some((now + secs, secs)) },
            Ev::Needs { .. } => self.needs = true,
            Ev::Error(_) => self.failed = 1.0,
        }
    }

    /// The session a live request belongs to.
    fn chat(&mut self, id: u64) -> Option<&mut Chat> {
        let sid = self.sessions.get(&id)?;
        self.chats.iter_mut().find(|c| c.id == *sid)
    }

    /// The session that spoke last.
    fn lead(&self) -> Option<&Chat> { self.chats.iter().find(|c| c.id == self.last) }

    /// The sessions the island speaks for: every one mid-turn when several are, else the last
    /// to speak. Their pills light up, and the card mixes their numbers.
    fn focus(&self) -> Vec<&Chat> {
        let busy: Vec<&Chat> = self.chats.iter()
            .filter(|c| self.sessions.values().any(|s| *s == c.id) || (c.turn_end.is_none() && self.last_t - c.seen < BUSY))
            .collect();
        if busy.len() > 1 { busy } else { self.lead().into_iter().collect() }
    }

    /// The fullest share of a context window among the sessions in focus.
    fn filled(&self) -> f32 { self.focus().iter().map(|c| c.filled()).fold(0.0, f32::max) }

    /// Output tokens and dollars of the turns in focus, together.
    fn turn(&self) -> (u64, f64) {
        self.focus().iter().fold((0, 0.0), |(o, d), c| (o + c.turn_out, d + c.turn_cost))
    }

    fn turn_tokens(&self) -> String {
        let out = self.turn().0;
        if out == 0 { String::new() } else { card::tokens(out) }
    }

    /// Lines the sessions in focus added and removed this turn, together.
    fn tally(&self) -> [u64; 2] {
        self.focus().iter().fold([0, 0], |[a, r], c| [a + c.added, r + c.removed])
    }

    /// The counting tally as the wing shows it: `+42` and `−7`, each empty while it's zero.
    fn diff(&self) -> [String; 2] {
        let [a, r] = self.shown.map(|v| v.round() as u64);
        [if a > 0 { format!("+{a}") } else { String::new() }, if r > 0 { format!("−{r}") } else { String::new() }]
    }

    /// Seconds until a rate limit lifts, while one holds.
    fn limited(&self) -> Option<f64> { self.limit.map(|(until, _)| until - self.last_t).filter(|r| *r > 0.0) }

    fn limit_words(&self) -> String { format!("Back in {}", card::duration(self.limited().unwrap_or(0.0).ceil())) }

    /// Waiting on you or on a rate limit, with nothing in flight.
    fn resting(&self) -> bool { (self.waiting || self.limit.is_some()) && !self.working() }

    /// Bar `i`'s height, 0 to 1: kicked by the stream, or while Claude thinks, a soft swell
    /// sweeping to and fro across the bars, eased in and out of.
    fn bar(&self, i: usize) -> f32 {
        let at = 2.0 + 2.6 * (self.t * 1.7).sin();
        let swell = 0.16 + 0.6 * (-(i as f32 - at).powi(2) / 1.6).exp();
        let k = self.think * self.think * (3.0 - 2.0 * self.think);
        self.bars[i] * (1.0 - k) + swell * k
    }

    /// Sessions beyond the first with a request in flight.
    fn others(&self) -> usize { self.sessions.values().collect::<HashSet<_>>().len().saturating_sub(1) }

    // An interrupted tool call never hears back, so a run counts only BUSY seconds; a prompt for you stays.
    fn working(&self) -> bool { !self.live.is_empty() || (self.running && (self.needs || self.quiet < BUSY as f32)) || (self.quiet < LINGER && !self.waiting) }

    /// What the label beside the dot says while Claude works.
    fn what(&self) -> String {
        if self.needs { return "Needs you".into() }
        match &self.kind {
            Some(Kind::Thinking) => "Thinking".into(),
            Some(Kind::Text) => "Writing".into(),
            // `mcp__server__tool` reads as its tool.
            Some(Kind::Tool(n) | Kind::Skill(n)) => n.rsplit("__").next().unwrap_or(n).into(),
            None => String::new(),
        }
    }

    /// How far the wings reach for what they show: the label on the left, the waveform or
    /// the turn's count on the right, the island staying centred on the notch.
    fn reach(&self) -> f32 {
        let w = |s: &str| if s.is_empty() { 0.0 } else { rune::text::measure(s, &[], LABEL, None).0 };
        // Counts are measured as zeros, so the island grows with a digit, not with each tick.
        let w0 = |s: &str| w(&s.replace(|c: char| c.is_ascii_digit(), "0"));
        let [a, r] = self.diff();
        let diff = w0(&a) + w0(&r) + if a.is_empty() || r.is_empty() { 0.0 } else { DIFF_GAP };
        // Never narrower than "Thinking", so only a longer tool name grows it.
        let r = if self.working() {
                let n = self.others();
                let others = if n > 0 { w(&format!("+{n}")) + 8.0 } else { 0.0 };
                let right = if diff + others > 0.0 { DIFF_X + diff + others + 12.0 } else { 0.0 };
                (LABEL_X + 12.0 + w(&self.label).max(w("Thinking"))).max(right)
            }
            else if self.limited().is_some() { LABEL_X + w0(&self.limit_words()) + 12.0 }
            else if self.waiting && self.waited < DIM {
                let n = if diff > 0.0 { diff } else { w(&self.turn_tokens()) };
                // Each side holds its own: the words on the left, the count on the right.
                (LABEL_X + w(YOUR_TURN) + 12.0).max(if n == 0.0 { 0.0 } else { 16.0 + n + 12.0 })
            }
            else if self.waiting { 34.0 }
            else { 0.0 };
        r.min(WING)
    }

    fn geom(&self) -> Geom {
        let [nw, nh] = self.screen.core();
        let ((w, wh), k) = (self.screen.layout(), self.screen.zoom());
        let flat = self.screen.notch.is_none();
        // Closing, the card's spring swings a little past shut: the island squashes just under
        // its resting size and springs back, as it bounces past open on the way out.
        let card = self.card.max(-0.2);
        let narrow = nw * 0.5 + self.wing.max(0.0);
        let prices = self.pricing && self.guide.is_none();
        let half = narrow + (narrow.max(if prices { prices::W } else { CARD_W } * 0.5) - narrow) * card;
        // Without a notch the pill floats below the top edge and stretches a little while it
        // resizes fast, as the iPhone's island does.
        let (top, row) = if flat { (PILL_TOP, PILL_H + (self.wing_v.abs() / 500.0).min(1.0) * STRETCH) } else { (0.0, nh) };
        let rest = if flat { row * 0.5 } else { (nh * 0.32).min(10.0) };
        let card_h = if prices { prices::height(prices::models(self).len()) }
            else { CARD_H + if self.guide.is_some() || expanded::pilled(self).len() > 1 { PILLS } else { 0.0 } };
        Geom { w, wh, k, top, row, half, h: row + card_h * card, r: rest + (CARD_R - rest) * card.clamp(0.0, 1.0) }
    }

    /// A press on the island opens the card, or folds it back, on the stats again.
    fn press(&mut self) { self.expanded = !self.expanded; self.pricing &= self.expanded }

    /// One frame of motion; true while anything still moves.
    fn step(&mut self, dt: f32) -> bool {
        self.t += dt;
        self.quiet += dt;
        self.waited += dt;
        self.flow *= (-dt / 0.33).exp();
        self.failed *= (-dt * 1.5).exp();
        // Once work stops the label lifts away, before the island narrows and would cut it.
        let what = if self.working() { self.what() } else { String::new() };
        if what != self.label {
            self.old_label = std::mem::replace(&mut self.label, what);
            self.swap = 0.0;
        }
        self.swap = (self.swap + dt / 0.25).min(1.0);
        // A limit that has run out hands the turn back to you, with the wait's pulse.
        if self.limit.is_some() && self.limited().is_none() { (self.limit, self.waiting, self.waited) = (None, true, 0.0) }
        let wait = if self.resting() { 1.0 } else { 0.0 };
        self.wait_k += (wait - self.wait_k) * (1.0 - (-dt * 12.0).exp());
        if (wait - self.wait_k).abs() < 1e-3 { self.wait_k = wait }
        // Under a limit the ring drains with the time left, set once a second rather than eased,
        // so a long limit costs a frame a second.
        let fill = match (self.limited(), self.limit) { (Some(rem), Some((_, span))) => (rem / span) as f32, _ => self.filled() };
        self.ring += (fill - self.ring) * (1.0 - (-dt * 6.0).exp());
        if (fill - self.ring).abs() < 1e-3 || self.limited().is_some() { self.ring = fill }
        // The tally counts up to a new edit; a new turn's zero lands at once.
        let tally = self.tally().map(|v| v as f32);
        for (v, to) in self.shown.iter_mut().zip(tally) {
            *v = if to < *v || (to - *v).abs() < 0.5 { to } else { *v + (to - *v) * (1.0 - (-dt * 6.0).exp()) };
        }
        let think = if self.working() && !self.needs && self.kind == Some(Kind::Thinking) { 1.0 } else { 0.0 };
        self.think += (think - self.think) * (1.0 - (-dt * 4.0).exp());
        if (think - self.think).abs() < 1e-3 { self.think = think }
        // Grow at once for a longer label, but give width back only once a shorter one has
        // stayed a moment, so a run of quick tools doesn't pump the island.
        let need = self.reach();
        self.narrower = if need >= self.held { 0.0 } else { self.narrower + dt };
        if need >= self.held || self.narrower > HOLD || (!self.working() && self.swap == 1.0) { self.held = need }
        let target = self.held + if self.hover && !self.expanded { HOVER } else { 0.0 };
        // Slightly underdamped springs, so the island settles with its small overshoot.
        let (k, c) = (220.0, 2.0 * 220f32.sqrt() * 0.72);
        self.wing_v += (k * (target - self.wing) - c * self.wing_v) * dt;
        self.wing += self.wing_v * dt;
        let open = if self.expanded { 1.0 } else { 0.0 };
        // A touch looser closing than opening, so the fold's bounce reads as clearly as the open's.
        let cc = if open == 0.0 { 2.0 * 220f32.sqrt() * 0.58 } else { c };
        self.card_v += (k * (open - self.card) - cc * self.card_v) * dt;
        self.card += self.card_v * dt;
        // While a tool runs nothing streams, so the bars idle in a low wave; asked to allow it,
        // they fall still.
        let level = (1.0 - (-self.flow / 30.0).exp()).max(if self.running && !self.needs { 0.7 } else { 0.0 });
        self.level += (level - self.level) * (1.0 - (-dt * 3.0).exp());
        for (i, (b, k)) in self.bars.iter_mut().zip(&mut self.kicks).enumerate() {
            let sway = 0.5 + 0.5 * (self.t * (3.4 + i as f32 * 1.1) + i as f32 * 2.1).sin();
            *k *= (-dt * 3.0).exp();
            let to = (self.level * (0.18 + 0.42 * sway) + *k).min(1.0);
            *b += (to - *b) * (1.0 - (-dt * 10.0).exp());
        }
        let still = (target - self.wing).abs() < 1e-2 && self.wing_v.abs() < 1e-2;
        if still { self.wing = target; self.wing_v = 0.0 }
        let shut = (open - self.card).abs() < 1e-3 && self.card_v.abs() < 1e-3;
        if shut { self.card = open; self.card_v = 0.0 }
        if still && target == 0.0 && open == 0.0 { self.kind = None; self.label.clear() }
        // Waiting is still once its pulses end, so an unanswered turn draws nothing.
        let calm = still && shut && self.quiet > LINGER && (!self.waiting || self.waited > PULSE) && self.swap == 1.0 && self.wait_k == wait && self.ring == fill && self.shown == tally && self.think == think && self.failed <= 0.01;
        // Asked to allow a tool, the bars fall still and the island holds "Needs you" without drawing.
        let asked = self.needs && self.level < 1e-3 && self.bars.iter().all(|b| *b < 1e-3);
        self.wave = calm && self.live.is_empty() && self.flow < 1.0;
        !(calm && (!self.working() || asked))
    }
}

/// A rounded rectangle `half` wide either side of `cx`: the pill, and the card it opens into.
pub fn rounded(cx: f32, top: f32, half: f32, h: f32, r: f32) -> Vec<[f32; 2]> {
    let r = r.min(h * 0.5).min(half);
    let mut p = Vec::new();
    for (c, from) in [([cx - half + r, top + h - r], 90.0), ([cx - half + r, top + r], 180.0), ([cx + half - r, top + r], 270.0), ([cx + half - r, top + h - r], 0.0)] {
        for i in 0..=8 {
            let a = (from + 90.0 * i as f32 / 8.0f32).to_radians();
            p.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        }
    }
    p
}

/// The island's outline in a `w`-wide window: flat along the top, flaring into the screen edge
/// at both ends, rounded underneath by `r`. `half` is half its body's width, `h` its height.
pub fn island(w: f32, half: f32, h: f32, r: f32) -> Vec<[f32; 2]> {
    let (cx, r) = (w * 0.5, r.min(h * 0.5));
    let mut p = Vec::new();
    let mut arc = |c: [f32; 2], r: f32, from: f32, to: f32| {
        for i in 0..=8 {
            let a = (from + (to - from) * i as f32 / 8.0).to_radians();
            p.push([c[0] + r * a.cos(), c[1] + r * a.sin()]);
        }
    };
    arc([cx - half - EAR, EAR], EAR, 270.0, 360.0);
    arc([cx - half + r, h - r], r, 180.0, 90.0);
    arc([cx + half - r, h - r], r, 90.0, 0.0);
    arc([cx + half + EAR, EAR], EAR, 180.0, 270.0);
    p
}

fn tint(s: &State) -> Color {
    if s.needs { return oklch(0.80, 0.15, 70.0) }
    match &s.kind {
        Some(Kind::Thinking) => oklch(0.78, 0.09, 290.0),
        Some(Kind::Tool(_) | Kind::Skill(_)) => oklch(0.72, 0.15, 250.0),
        _ => oklch(0.97, 0.0, 0.0),
    }
}

/// A line of text on the island, in layout points: `x` is its left edge, or its right edge
/// when `right`; `y` its top; `room` the width it is cut to with "…", if any.
struct Label {
    text: String,
    x: f32,
    y: f32,
    room: f32,
    alpha: f32,
    color: Color,
    right: bool,
    /// Cut from the start rather than the end, keeping the newest words.
    head: bool,
    style: TextStyle,
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let g = s.geom();
    let [nw, _] = s.screen.core();
    let flat = s.screen.notch.is_none();
    let body = if flat { rounded(g.w * 0.5, g.top, g.half, g.h, g.r) } else { island(g.w, g.half, g.h, g.r) };
    let mut shape = Shape::new().fill(body, oklch(0.0, 0.0, 0.0));
    let mut labels = Vec::new();
    let mut dots = Vec::new();
    let mut rings = Vec::new();
    let fade = |c: Color, k: f32| Color { a: c.a * k, ..c };
    let (left, right) = (g.left(), g.right());
    let cy = g.top + g.row * 0.5;
    // Centred on the capitals' ink rather than the line box, which carries room for descenders
    // and leading, as controls centre their titles.
    let lh = rune::text::measure("Ag", &[], LABEL, None).1;
    let caps = rune::text::ink("H", LABEL).map_or(lh * 0.5, |i| i.bounds[1] + i.bounds[3] * 0.5);
    let row_y = cy - caps;
    // The wings' content fades in only once they have room for it.
    // The card's closing squash dips below the wings' resting width; it shouldn't fade them.
    let a = (((g.half - nw * 0.5).max(s.wing) - 24.0) / 20.0).clamp(0.0, 1.0);
    if a > 0.0 {
        let (work, wait) = (a * (1.0 - s.wait_k), a * s.wait_k);
        let color = if s.failed > 0.05 { oklch(0.68, 0.19, 25.0) } else { tint(s) };
        // Under a rate limit the wait is amber, its words don't dim and the ring drains.
        let limited = s.limited().is_some();
        let green = if limited { AMBER } else { oklch(0.80, 0.17, 150.0) };
        let words = if limited { 1.0 } else { ((DIM - s.waited) / 0.3).clamp(0.0, 1.0) };
        let dot = color.mix(green, s.wait_k);
        let breathe = 3.2 + 0.8 * (s.t * 2.4).sin() * (1.0 - (-s.flow / 30.0).exp());
        // Two pulses as the turn ends, then the dot rests.
        let pulse = if s.waited < PULSE { (s.waited / PULSE * std::f32::consts::TAU * 2.0).sin().abs() * (1.0 - s.waited / PULSE) } else { 0.0 };
        let r = breathe * (1.0 - s.wait_k) + (3.6 + 1.2 * pulse) * s.wait_k;
        // With a context to show, the ring stands in for the dot's resting glow; the pulse still ripples past it.
        let context = s.filled() > 0.0 || limited;
        let glow = if context { 0.0 } else { 0.18 };
        // The dot, its glow and its ring share one centre on a whole device pixel, so as the
        // island slides they step cleanly rather than smearing across pixels and dimming.
        let px = g.k * s.screen.scale.max(1.0) as f32;
        let at = [(left + 18.0) * px, cy * px].map(|v| v.round() / px);
        dots.push((at, r + 3.0 + 3.0 * pulse * s.wait_k, fade(dot, (glow + 0.2 * pulse * s.wait_k) * a)));
        dots.push((at, r, fade(dot, a)));
        if context {
            // Context used, clockwise from the top, warming to amber in the last fifth.
            let ring = if limited { AMBER } else { INK.mix(oklch(0.80, 0.15, 70.0), ((s.ring - 0.8) / 0.15).clamp(0.0, 1.0)) };
            rings.push((at, 1.0, fade(INK, 0.24 * a)));
            if s.ring > 0.005 { rings.push((at, s.ring, fade(ring, 0.9 * a))) }
        }
        if work > 0.0 {
            for i in 0..BARS {
                let b = s.bar(i);
                let x = right - 20.0 - (BARS - 1 - i) as f32 * 5.0;
                let len = 1.5 + b * (g.row.min(32.0) * 0.5 - 4.0);
                shape = shape.line([x, cy - len * 0.5], [x, cy + len * 0.5], 3.0, LineCap::Round, fade(color, work * (0.55 + 0.45 * b)));
            }
            // The turn's lines added and removed just inside the waveform, and other sessions at
            // work counted inside that.
            let x = diff_labels(s, right - DIFF_X, row_y, work, &mut labels);
            let n = s.others();
            if n > 0 { labels.push(Label { text: format!("+{n}"), x: x - if x < right - DIFF_X { 8.0 } else { 0.0 }, y: row_y, room: 0.0, alpha: work * 0.5, color: INK, right: true, head: false, style: LABEL }) }
            let room = (g.half - nw * 0.5 - LABEL_X - 8.0).max(0.0);
            // The new label rises into place as the old one lifts away, as the island swaps content.
            for (t, k, dy) in [(&s.old_label, 1.0 - s.swap, -4.0 * s.swap), (&s.label, s.swap, 4.0 * (1.0 - s.swap))] {
                if !t.is_empty() && k > 0.01 {
                    labels.push(Label { text: t.clone(), x: left + LABEL_X, y: row_y + dy, room, alpha: work * k, color: INK, right: false, head: false, style: LABEL });
                }
            }
        }
        if wait * words > 0.01 {
            // Laid out as the working island is: the words beside the dot, where the label was,
            // and the turn's output on the right, in grey, where the waveform was. The green
            // stays on the dot.
            let n = s.turn_tokens();
            let text = if limited { s.limit_words() } else { YOUR_TURN.into() };
            labels.push(Label { text, x: left + LABEL_X, y: row_y, room: 0.0, alpha: wait * words, color: INK, right: false, head: false, style: LABEL });
            // The lines the turn changed stand in for its figure, which the open card says instead.
            if limited {} else if s.tally() != [0, 0] { diff_labels(s, right - 16.0, row_y, wait * words, &mut labels); }
            else if !n.is_empty() { labels.push(Label { text: n, x: right - 16.0, y: row_y, room: 0.0, alpha: wait * words * 0.5 * (1.0 - s.card).clamp(0.0, 1.0), color: INK, right: true, head: false, style: LABEL }) }
        }
    }
    // The card's content fades in over the second half of its opening.
    let ca = ((s.card - 0.5) / 0.5).clamp(0.0, 1.0);
    if ca > 0.0 {
        shape = match &s.guide {
            Some(guide) => guide::labels(guide, &g, s.t, ca, shape, &mut labels),
            None if s.pricing => prices::labels(s, &g, ca, shape, &mut labels),
            None => expanded::card_labels(s, &g, ca, shape, &mut labels),
        };
    }
    let k = g.k;
    let mut root = el().w_full().h_full().child(canvas(shape.view_box(g.w, g.wh)).size(g.w * k, g.wh * k));
    // The dot, its glow and the context ring are round boxes rather than polygons, so their edges
    // stay smooth at any zoom, and unsnapped, so they share one exact centre.
    let disc = |[x, y]: [f32; 2], r: f32| el().absolute((x - r) * k, (y - r) * k).size(2.0 * r * k, 2.0 * r * k).radius(r * k).pixel_snap(false);
    for (p, r, c) in dots {
        if c.a > 0.004 { root = root.child(disc(p, r).fill(c)) }
    }
    // The ring's centre line has a 7.5 radius; a share under one is a single dash from the top, clockwise.
    let (rr, rw) = (7.5, 2.0);
    for (p, share, c) in rings {
        let mut ring = disc(p, rr + rw * 0.5).border(rw * k, c);
        if share < 0.999 {
            let len = std::f32::consts::TAU * rr * k;
            ring = ring.border_dash(share * len, (1.0 - share) * len);
        }
        root = root.child(ring);
    }
    if ca > 0.0 && s.guide.is_none() && !s.pricing { for p in expanded::pills(s, &g, ca) { root = root.child(p) } }
    for l in labels {
        let x = if l.right { l.x - rune::text::measure(&l.text, &[], l.style, None).0 } else { l.x };
        let style = TextStyle { size: l.style.size * k, tracking: l.style.tracking * k, ..l.style };
        let mut t = text(l.text).font(style).color(Color { a: l.color.a * l.alpha, ..l.color }).absolute(x * k, l.y * k);
        if l.room > 0.0 { t = t.w(l.room * k); t = if l.head { t.truncate_head() } else { t.truncate() } }
        root = root.child(t);
    }
    // The island takes presses only while the pointer is over it; the window lets the rest through.
    let hit = el().absolute(g.left() * k, g.top * k).size(g.half * 2.0 * k, g.h * k)
        // The springs move in `poll`, so a press asks for the next frame rather than waiting for a wake.
        .on_press_tx(|s: &mut State, tx: &mut Tx| { s.press(); tx.wake_after(std::time::Duration::ZERO) });
    root = root.child(hit);
    // The guide's buttons sit over the island's press, once they've faded in.
    if let Some(guide) = &s.guide && ca > 0.5 { for p in guide::presses(guide, &g, s.t) { root = root.child(p) } }
    else if s.pricing && ca > 0.5 { for p in prices::presses(s, &g) { root = root.child(p) } }
    root.motion(Spring::NONE)
}

/// The counting tally, `+42 −7`, right-aligned to `x`; returns where its left edge lands.
fn diff_labels(s: &State, right: f32, y: f32, alpha: f32, labels: &mut Vec<Label>) -> f32 {
    let mut x = right;
    for (text, color) in s.diff().into_iter().zip([ADDED, REMOVED]).rev() {
        if text.is_empty() { continue }
        let w = rune::text::measure(&text, &[], LABEL, None).0;
        labels.push(Label { text, x, y, room: 0.0, alpha: alpha * 0.85, color, right: true, head: false, style: LABEL });
        x -= w + DIFF_GAP;
    }
    if x < right { x + DIFF_GAP } else { right }
}

/// The relay's port: `ATOLL_PORT`, or 8788.
fn port() -> u16 { std::env::var("ATOLL_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8788) }

fn setup(s: &mut State, waker: Waker) {
    let _ = s.shared.waker.set(waker.clone());
    if rune::is_headless() || s.demo.is_some() { return }
    s.prices = ledger::Prices::load();
    let port = port();
    let upstream = std::env::var("ATOLL_UPSTREAM").unwrap_or_else(|_| "https://api.anthropic.com".into());
    let inbox = s.inbox.clone();
    let sink: proxy::Sink = Arc::new(move |ev| { inbox.lock().unwrap().push(ev); waker() });
    match proxy::serve(port, upstream.trim_end_matches('/').to_string(), sink) {
        Ok(()) => {
            eprintln!("atoll: ANTHROPIC_BASE_URL=http://127.0.0.1:{port} claude");
            // Claude Code isn't sent through Atoll yet: open the card on the guide to setting it up.
            let set = guide::settings().and_then(|p| std::fs::read_to_string(p).ok()).is_some_and(|t| guide::configured(&t, port));
            if !set { (s.guide, s.expanded) = (Some(guide::Guide::new(port)), true) }
        }
        Err(e) => { eprintln!("atoll: can't listen on {port}: {e}"); s.failed = 1.0 }
    }
}

fn poll(s: &mut State, t: f64, tx: &mut Tx) -> bool {
    let gap = (t - s.last_t).max(0.0) as f32;
    // The first frame after a pause steps as one frame, so motion starting from rest doesn't
    // skip its opening; the wait still counts the whole gap.
    let dt = if gap > 0.05 { 1.0 / 120.0 } else { gap };
    if s.last_t > 0.0 { s.waited += gap - dt }
    s.last_t = t;
    let evs = std::mem::take(&mut *s.inbox.lock().unwrap());
    let mut got = !evs.is_empty();
    for ev in evs { s.apply(ev) }
    if let Some(screen) = s.shared.screen.lock().unwrap().take() { s.screen = screen; got = true }
    // Chosen from the menu, the guide opens even when the settings already send Claude Code here.
    if s.shared.guide.swap(false, std::sync::atomic::Ordering::Relaxed) {
        (s.guide, s.expanded, got) = (Some(guide::Guide::new(port())), true, true);
    }
    if s.shared.prices.swap(false, std::sync::atomic::Ordering::Relaxed) { (s.pricing, s.expanded, got) = (true, true, true) }
    // A new screen, or a scene that switched to the pill: size and place the window for it.
    if s.screen != s.placed && let Some(w) = s.shared.window.get() {
        w.resize_instant(s.screen.window());
        w.set_position(s.screen.position());
        (s.placed, *s.shared.placed.lock().unwrap()) = (s.screen, s.screen);
    }
    // Over the island the window takes clicks; anywhere else they pass to what is under it.
    let g = s.geom();
    let hover = s.shared.pointer.lock().unwrap().is_some_and(|[x, y]| g.contains([x / g.k, y / g.k]));
    if hover != s.hover {
        s.hover = hover;
        got = true;
        if let Some(w) = s.shared.window.get() { let _ = w.set_click_through(!hover); }
    }
    demo::demo(s, dt);
    // Open's spinner turns for its two seconds, then the island rests again. Asked before the
    // step, so the step that ends it still draws the frame that takes it away.
    let spinning = s.guide.as_ref().is_some_and(|g| g.opening(s.t));
    let holding = s.hold();
    let moving = s.step(dt) || s.demo.is_some() || spinning || holding;
    tx.instant();
    // Folded, it stops asking for frames; the relay's and the pointer's wakers resume it.
    if moving { tx.wake_after(std::time::Duration::from_millis(if s.wave { 33 } else { 0 })) }
    // A rate limit's countdown draws once a second, or once a minute while it reads in hours.
    else if let Some(rem) = s.limited() {
        let tick = if rem > 3600.0 { 60.0 } else { 1.0 };
        tx.wake_after(std::time::Duration::from_secs_f64(rem % tick + 0.02));
        return true
    }
    // Waiting draws nothing, so one frame is booked for when "Your turn" should go.
    else if s.waiting && s.waited < DIM { tx.wake_after(std::time::Duration::from_secs_f32(DIM - s.waited)) }
    moving || got
}

fn main() {
    let spec = AppSpec::<State> {
        scenes: &[
            ("demo", |s| s.demo = Some(0.0)),
            ("pill", |s| { s.screen.notch = None; s.demo = Some(0.0) }),
            ("card", |s| { s.expanded = true; s.demo = Some(0.0) }),
            ("limit", |s| s.limit = Some((95.0, 300.0))),
            ("guide", |s| { s.guide = Some(guide::Guide::new(8788)); s.expanded = true }),
            ("prices", |s| {
                for (id, model) in [("a", "claude-opus-5-5"), ("b", "claude-opus-5-7")] { s.chats.push(Chat { id: id.into(), model: model.into(), ..Chat::default() }) }
                s.unpriced.push(prices::Unpriced { model: "claude-opus-5-7".into(), session: "b".into(), turn: None, usage: Usage::default() });
                (s.pricing, s.expanded) = (true, true);
            }),
        ],
        setup: Some(setup),
        poll: Some(poll),
        ..AppSpec::new("Atoll", (320.0, 40.0), State::default, view)
    };
    rune::run_windows_with(RunOptions { accessory: true }, move |_| {
        let screen = screen::menu_screen();
        let size = screen.window();
        let px = |v: f32| (v as f64 * screen.scale).round() as u32;
        let options = WindowOptions {
            transparent: true, borderless: true, click_through: true, always_on_top: true,
            resizable: false, min_size: Some((0.0, 0.0)), physical_size: Some((px(size.0), px(size.1))),
            position: Some(screen.position()), ..Default::default()
        };
        // Above the menu bar (24), on every Space and beside full-screen apps, out of Exposé and
        // window cycling. A panel that takes a click without making this app active, so the
        // terminal keeps its place in front.
        let native = MacWindowOptions::default().shadow(false).level(25).collection_behavior(1 | 16 | 64 | 256).nonactivating_panel(true);
        let shared = Arc::new(Shared::default());
        *shared.placed.lock().unwrap() = screen;
        let app = App::new(spec).init({ let shared = shared.clone(); move || State { screen, placed: screen, shared: shared.clone(), ..State::default() } });
        let mut watching = false;
        // AppKit moves a new window out from under the menu bar before its level is raised;
        // placed again once it sits above the menu bar, it stays flush with the screen's top.
        vec![WindowSpec::from_app("atoll", app).configured_options(options.macos(native))
            .on_ready(move |w| {
                w.set_position(screen.position());
                if std::mem::replace(&mut watching, true) { return }
                let _ = shared.window.set(w.clone());
                screen::watch_pointer(shared.clone());
                screen::menu_bar_item(shared.clone());
                let shared = shared.clone();
                // A display plugged in or out, or the menu bar moved: follow the menu bar's screen;
                // `poll` resizes and places the window for it.
                rune::on_displays_changed(move || {
                    *shared.screen.lock().unwrap() = Some(screen::menu_screen());
                    shared.wake();
                });
            })]
    });
}
