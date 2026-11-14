//! The card's prices: each model Claude Code has used, its three rates as steppers, and Done.
//! A model the list doesn't know stays unpriced, its tokens held back from the costs, until its
//! price is set here; then what it already spent is counted.

use rune::{Color, Drag, El, Phase, Shape, TextStyle, Tx, el};
use crate::{INK, LABEL, Geom, Label, State, card, ledger, proxy::Usage, rounded};

/// The card's width open on the prices: wider than the stats', for three steppers a row.
pub const W: f32 = 400.0;
/// The most models it lists, newest first.
pub const ROWS: usize = 4;
const CAPTION: TextStyle = TextStyle::new(10.5, 500.0);
/// Label size, a step heavier, as the guide sets its title.
const TITLE: TextStyle = TextStyle::new(10.75, 650.0);
/// A stepper's width, the gap between two, and its words' inset from its ends.
const SW: f32 = 80.0;
const GAP: f32 = 6.0;
const PAD: f32 = 8.0;
/// The first row's top below the title, and the step from one row to the next.
const FIRST: f32 = 46.0;
const ROW: f32 = 26.0;
/// What the card lists before Claude Code has used any model.
const DEFAULTS: [&str; 3] = ["fable-5-2", "opus-5-5", "sonnet-5-5"];
const NOTE: &str = "Cache writes cost 1.25× input for 5 minutes, 2× for an hour.";

/// A stepper held down: which model and rate, which way, and when it was pressed and steps next,
/// in the island's seconds.
pub struct Held {
    pub id: String,
    pub col: usize,
    pub up: bool,
    pub since: f32,
    pub next: f32,
}

/// How long a held stepper waits before it repeats, and how often it repeats the longer it's
/// held: ten quarters a second, then twenty-five after 1.2s, then sixty after 3s.
const DELAY: f32 = 0.4;
fn every(held: f32) -> f32 { if held > 3.0 { 1.0 / 60.0 } else if held > 1.2 { 0.04 } else { 0.1 } }

/// A finished request on a model nobody has priced: its usage, and the turn it belongs to.
pub struct Unpriced {
    pub model: String,
    pub session: String,
    pub turn: Option<f64>,
    pub usage: Usage,
}

/// How far the card reaches below the top row for `n` models: title, headings, rows, footer.
pub fn height(n: usize) -> f32 { 90.0 + ROW * n as f32 }

impl State {
    /// A finished request on the totals. A model with no price adds its tokens but holds its
    /// usage back until it has one.
    pub(crate) fn charge(&mut self, id: u64) {
        let Some((model, usage)) = self.requests.remove(&id) else { return };
        let cost = self.prices.price(&model, &usage);
        let turn = self.chat(id).map(|c| {
            (c.turn_out, c.turn_cost) = (c.turn_out + usage.output, c.turn_cost + cost.unwrap_or(0.0));
            (c.id.clone(), c.turn_at)
        });
        self.total_out += usage.output;
        match (cost, turn) {
            (Some(d), _) => self.total_cost += d,
            (None, turn) => {
                let (session, turn) = turn.unwrap_or_default();
                self.unpriced.push(Unpriced { model, session, turn, usage });
            }
        }
    }

    /// Prices a model, and counts what it spent while it had none: on the total, and on its
    /// turn if that turn is still the one on show.
    pub(crate) fn set_price(&mut self, id: &str, rates: [f64; 3]) {
        self.prices.set.insert(id.into(), rates);
        let (now, rest): (Vec<Unpriced>, Vec<Unpriced>) = std::mem::take(&mut self.unpriced).into_iter().partition(|u| ledger::id(&u.model) == id);
        self.unpriced = rest;
        for u in now {
            let d = self.prices.price(&u.model, &u.usage).unwrap_or(0.0);
            self.total_cost += d;
            if let Some(c) = self.chats.iter_mut().find(|c| c.id == u.session && c.turn_at == u.turn) { c.turn_cost += d }
        }
    }

    /// One of a model's rates a step up or down; a model nobody has priced is set from where it started.
    fn step_price(&mut self, id: &str, col: usize, up: bool) {
        let mut r = self.prices.rates(id, 0).unwrap_or_else(|| ledger::seed(id));
        r[col] = ledger::step(r[col], up);
        self.set_price(id, r);
    }

    /// A held stepper's repeats due by now; true while one is held. It lets go once the pointer
    /// leaves the island, whose window then lets the release through to what's under it.
    pub(crate) fn hold(&mut self) -> bool {
        if !self.hover || !self.pricing { self.release() }
        let Some(h) = &self.stepper else { return false };
        let (id, col, up, since, mut next) = (h.id.clone(), h.col, h.up, h.since, h.next);
        while next <= self.t {
            self.step_price(&id, col, up);
            next += every(next - since);
        }
        if let Some(h) = &mut self.stepper { h.next = next }
        true
    }

    /// Lets go of a held stepper, and saves where it stopped.
    fn release(&mut self) {
        if self.stepper.take().is_some() { self.prices.save() }
    }

    /// Whether the turns in focus spent tokens on a model nobody has priced.
    pub(crate) fn turn_unpriced(&self) -> bool {
        let focus = self.focus();
        self.unpriced.iter().any(|u| focus.iter().any(|c| c.id == u.session && c.turn_at == u.turn))
    }
}

/// A cost as the card says it: the dollars known, and whether tokens still wait on a price.
pub fn cost(d: f64, waiting: bool) -> String {
    match (d > 0.0, waiting) {
        (true, false) => ledger::usd(d),
        (false, false) => String::new(),
        (false, true) => "Unpriced".into(),
        (true, true) => format!("{} + unpriced", ledger::usd(d)),
    }
}

/// The models the card lists: any waiting on a price, then those the sessions use, newest first.
pub fn models(s: &State) -> Vec<String> {
    let mut ids: Vec<String> = Vec::new();
    for m in s.unpriced.iter().rev().map(|u| &u.model).chain(s.chats.iter().rev().map(|c| &c.model)) {
        let id = ledger::id(m);
        if !id.is_empty() && !ids.iter().any(|i| i == id) { ids.push(id.into()) }
    }
    if ids.is_empty() { ids = DEFAULTS.map(String::from).to_vec() }
    ids.truncate(ROWS);
    ids
}

/// The three steppers' left edges, right-aligned to the card's edge.
fn columns(g: &Geom) -> [f32; 3] {
    let x = g.right() - 18.0 - 3.0 * SW - 2.0 * GAP;
    [x, x + SW + GAP, x + 2.0 * (SW + GAP)]
}

fn wc(t: &str) -> f32 { rune::text::measure(t, &[], CAPTION, None).0 }

/// Where a model's Set button and Done sit: their left edge and width.
fn set_button(g: &Geom) -> (f32, f32) { let w = wc("Set") + 2.0 * PAD; (columns(g)[0] - 10.0 - w, w) }
fn done_button(g: &Geom) -> (f32, f32) { let w = wc("Done") + 2.0 * PAD; (g.right() - 18.0 - w, w) }

/// Each row's top, and the footer's.
fn row_y(g: &Geom, i: usize) -> f32 { g.top + g.row + 8.0 + FIRST + ROW * i as f32 }

/// The title, the steppers' headings, a row per model and the footer as the card's text, and
/// the steppers' and buttons' capsules on `shape`.
pub fn labels(s: &State, g: &Geom, alpha: f32, mut shape: Shape, labels: &mut Vec<Label>) -> Shape {
    let (x0, x1, y0) = (g.left() + 18.0, g.right() - 18.0, g.top + g.row + 8.0);
    let label = |text: &str, x: f32, y: f32, a: f32, style| Label { text: text.into(), x, y, room: 0.0, alpha: alpha * a, color: INK, right: false, head: false, style };
    let th = rune::text::measure("Ag", &[], CAPTION, None).1;
    // The tool badge's capsule, as the guide's buttons are.
    let capsule = |shape: Shape, x: f32, y: f32, w: f32| shape.fill(rounded(x + w * 0.5, y - 3.0, w * 0.5, th + 6.0, (th + 6.0) * 0.5), Color { a: 0.12 * alpha, ..INK });
    labels.push(label("Prices", x0, y0, 1.0, TITLE));
    labels.push(Label { right: true, ..label("Dollars per million tokens", x1, y0, 0.55, CAPTION) });
    let cols = columns(g);
    for (head, x) in ["Input", "Output", "Cache read"].into_iter().zip(cols) {
        labels.push(label(head, x + (SW - wc(head)) * 0.5, y0 + 24.0, 0.6, CAPTION));
    }
    let models = models(s);
    for (i, id) in models.iter().enumerate() {
        let y = row_y(g, i);
        let set = s.prices.rates(id, 0);
        let mut room = cols[0] - 10.0 - x0;
        if set.is_none() {
            let (x, w) = set_button(g);
            shape = capsule(shape, x, y, w);
            labels.push(label("Set", x + PAD, y, 0.9, CAPTION));
            room = x - 6.0 - x0;
        }
        labels.push(Label { room, ..label(&card::model_name(id), x0, y, 0.9, LABEL) });
        // A model nobody has priced shows where it would start, dimmed until it's set.
        for (x, r) in cols.into_iter().zip(set.unwrap_or_else(|| ledger::seed(id))) {
            shape = capsule(shape, x, y, SW);
            labels.push(label("−", x + PAD, y, 0.55, CAPTION));
            labels.push(Label { right: true, ..label("+", x + SW - PAD, y, 0.55, CAPTION) });
            let v = ledger::rate(r);
            labels.push(label(&v, x + (SW - wc(&v)) * 0.5, y, if set.is_some() { 0.9 } else { 0.55 }, CAPTION));
        }
    }
    let y = row_y(g, models.len()) + 6.0;
    let (x, w) = done_button(g);
    shape = capsule(shape, x, y, w);
    labels.push(label("Done", x + PAD, y, 0.9, CAPTION));
    labels.push(Label { room: x - 10.0 - x0, ..label(NOTE, x0, y, 0.55, CAPTION) });
    shape
}

/// What takes each stepper's and button's press: the left half of a stepper steps down, the
/// right half up, a quarter at once and then faster while held. The rest of the card takes presses too, so a near miss doesn't fold it.
pub fn presses(s: &State, g: &Geom) -> Vec<El> {
    let k = g.k;
    let th = rune::text::measure("Ag", &[], CAPTION, None).1;
    let hit = |x: f32, y: f32, w: f32| el().absolute(x * k, (y - 3.0) * k).size(w * k, (th + 6.0) * k);
    let body = g.top + g.row;
    let mut out = vec![el().absolute(g.left() * k, body * k).size(g.half * 2.0 * k, (g.h - g.row) * k).on_press(|_: &mut State| {})];
    let models = models(s);
    for (i, id) in models.iter().enumerate() {
        let y = row_y(g, i);
        for (col, x) in columns(g).into_iter().enumerate() {
            for (up, dx) in [(false, 0.0), (true, SW * 0.5)] {
                let id = id.clone();
                out.push(hit(x + dx, y, SW * 0.5).on_drag(move |s: &mut State, d: &mut Drag, tx: &mut Tx| match d.phase {
                    Phase::Down => {
                        s.step_price(&id, col, up);
                        s.stepper = Some(Held { id: id.clone(), col, up, since: s.t, next: s.t + DELAY });
                        tx.wake_after(std::time::Duration::ZERO)
                    }
                    Phase::Move => {}
                    Phase::Up => s.release(),
                }));
            }
        }
        if s.prices.rates(id, 0).is_none() {
            let ((x, w), id) = (set_button(g), id.clone());
            out.push(hit(x, y, w).on_press(move |s: &mut State| { s.set_price(&id, ledger::seed(&id)); s.prices.save() }));
        }
    }
    let (x, w) = done_button(g);
    out.push(hit(x, row_y(g, models.len()) + 6.0, w).on_press_tx(|s: &mut State, tx: &mut Tx| {
        (s.pricing, s.expanded) = (false, false);
        tx.wake_after(std::time::Duration::ZERO)
    }));
    out
}
