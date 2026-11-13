//! One Claude Code session as the island keeps it: its pill, and what the card says when it
//! speaks for it.

/// The most sessions that keep a pill; past it the one quiet longest goes.
pub const CHATS: usize = 5;
/// Seconds a session mid-turn counts as busy without a request in flight, while a tool runs.
// shortcut: an interrupted turn never ends, so it reads as busy this long; upgrade with a Stop hook.
pub const BUSY: f64 = 120.0;
/// The most tool calls the trail keeps.
pub const TRAIL: usize = 4;

#[derive(Default)]
pub struct Chat {
    pub id: String,
    pub model: String,
    pub effort: String,
    /// Tokens its last prompt filled, and the window it asked for.
    pub context: u64,
    pub window: u64,
    /// Its todo list's step in progress, of how many, and what it says; kept across turns, as
    /// the list is.
    pub todo: Option<(usize, usize, String)>,
    /// This turn's last few tool calls, oldest first: the tool, its argument once whole, and
    /// when it began (the island's clock), which its chip eases in from.
    pub trail: Vec<(String, String, f32)>,
    /// The turn so far: output tokens and dollars of its finished requests, when it began and
    /// ended (poll's clock).
    pub turn_out: u64,
    pub turn_cost: f64,
    pub turn_at: Option<f64>,
    pub turn_end: Option<f64>,
    /// Lines its edits added and removed this turn, and the files they touched.
    pub added: u64,
    pub removed: u64,
    pub files: Vec<String>,
    /// When it last streamed anything.
    pub seen: f64,
}

impl Chat {
    pub fn filled(&self) -> f32 { (self.context as f32 / self.window.max(1) as f32).min(1.0) }

    /// A request after its wait, or its first, begins a turn from nothing.
    pub fn begin(&mut self, now: f64) {
        if self.turn_end.is_some() || self.turn_at.is_none() {
            *self = Chat { id: std::mem::take(&mut self.id), model: std::mem::take(&mut self.model), effort: std::mem::take(&mut self.effort),
                context: self.context, window: self.window, todo: self.todo.take(), turn_at: Some(now), ..Chat::default() };
        }
    }
}
