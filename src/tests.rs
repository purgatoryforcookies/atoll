use super::*;
use crate::screen::*;

#[test]
fn island_meets_the_top_edge_and_stays_inside_its_window() {
    let (w, half, h) = (330.0, 120.0, 32.0);
    let p = island(w, half, h, 10.0);
    assert_eq!(p.first().map(|p| p[1].round()), Some(0.0));
    assert_eq!(p.last().map(|p| p[1].round()), Some(0.0));
    assert!(p.iter().all(|p| (-0.01..=w + 0.01).contains(&p[0]) && (-0.01..=h + 0.01).contains(&p[1])));
    // Symmetric about the centre.
    for (a, b) in p.iter().zip(p.iter().rev()) { assert!((a[0] + b[0] - w).abs() < 0.01 && (a[1] - b[1]).abs() < 0.01) }
}

#[test]
fn wings_open_while_streaming_and_fold_away_after() {
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: String::new(), window: 200_000, session: String::new() });
    for _ in 0..60 {
        s.apply(Ev::Delta { id: 1, kind: Kind::Text, text: "hey".into() });
        s.step(1.0 / 60.0);
    }
    assert!(s.wing > 50.0 && s.bars.iter().any(|b| *b > 0.1));
    s.apply(Ev::End { id: 1, status: 200 });
    let mut frames = 0;
    while s.step(1.0 / 60.0) { frames += 1; assert!(frames < 600, "never settled") }
    assert_eq!(s.wing, 0.0);
}

#[test]
fn a_finished_turn_waits_for_you_and_a_tool_call_does_not() {
    let mut s = State::default();
    let settle = |s: &mut State| { let mut f = 0; while s.step(1.0 / 60.0) { f += 1; assert!(f < 600, "never settled") } };
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Block { id: 1, kind: Kind::Tool("mcp__graft__graft_find_code".into()) });
    s.apply(Ev::Stop { id: 1, reason: "tool_use".into() });
    s.apply(Ev::End { id: 1, status: 200 });
    // A side request on Haiku ending its own turn says nothing about Claude's.
    s.apply(Ev::Start { id: 2, model: "claude-haiku".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Stop { id: 2, reason: "end_turn".into() });
    s.apply(Ev::End { id: 2, status: 200 });
    for _ in 0..120 { s.step(1.0 / 60.0); }
    assert!(s.running && !s.waiting && s.label == "graft_find_code" && s.wing > 56.0);
    s.apply(Ev::Start { id: 3, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Stop { id: 3, reason: "end_turn".into() });
    s.apply(Ev::End { id: 3, status: 200 });
    settle(&mut s);
    // Compact, still, and drawing nothing until the next request.
    assert!(s.waiting && s.wait_k == 1.0 && s.wing > 0.0 && s.wing < 120.0);
    // After five minutes only the dot stays.
    let full = s.wing;
    s.waited = DIM;
    settle(&mut s);
    assert!(s.wing < full && s.wing > 24.0);
    s.apply(Ev::Start { id: 4, model: "claude-opus".into(), window: 200_000, session: String::new() });
    assert!(!s.waiting && s.step(1.0 / 60.0));
}

#[test]
fn a_tool_call_that_never_hears_back_stops_drawing() {
    let tool = |s: &mut State| {
        s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
        s.apply(Ev::Block { id: 1, kind: Kind::Tool("Bash".into()) });
        s.apply(Ev::Stop { id: 1, reason: "tool_use".into() });
        s.apply(Ev::End { id: 1, status: 200 });
    };
    // Interrupted while the tool runs: the low wave draws at a quarter rate, then stops.
    let mut s = State::default();
    tool(&mut s);
    for _ in 0..180 { s.step(1.0 / 60.0); }
    assert!(s.wave && s.step(1.0 / 60.0));
    let mut frames = 0;
    while s.step(1.0 / 60.0) { frames += 1; assert!(frames < 60 * 130, "still drawing") }
    // Asked to allow it and left unanswered: "Needs you" stays, but nothing is drawn.
    let mut s = State::default();
    tool(&mut s);
    s.apply(Ev::Needs { session: String::new() });
    let mut frames = 0;
    while s.step(1.0 / 60.0) { frames += 1; assert!(frames < 600, "still drawing") }
    assert_eq!(s.label, "Needs you");
}

#[test]
fn a_turn_counts_its_output_and_the_ring_reads_the_context() {
    let mut s = State::default();
    let usage = |output, cache_read| proxy::Usage { input: 10, cache_read, output, ..Default::default() };
    for (id, reason) in [(1, "tool_use"), (2, "end_turn")] {
        s.apply(Ev::Start { id, model: "claude-opus".into(), window: 200_000, session: String::new() });
        s.apply(Ev::Usage { id, usage: usage(1_200, 99_990) });
        s.apply(Ev::Stop { id, reason: reason.into() });
        s.apply(Ev::End { id, status: 200 });
    }
    assert_eq!(s.turn_tokens(), "2.4k");
    assert!((s.filled() - 0.5).abs() < 1e-3);
    // Opus 5.5: 10 input at $4/M, 99,990 cache reads at $0.20/M, 1,200 output at $20/M, twice.
    let one = 10.0 * 4e-6 + 99_990.0 * 0.2e-6 + 1_200.0 * 20e-6;
    assert!((s.turn().1 - 2.0 * one).abs() < 1e-9 && s.total_cost == s.turn().1);
    // The next turn counts from zero, but the total keeps counting; a 1M request reads its ring against 1M.
    s.apply(Ev::Start { id: 3, model: "claude-opus".into(), window: 1_000_000, session: String::new() });
    s.apply(Ev::Usage { id: 3, usage: usage(0, 300_000) });
    assert_eq!(s.turn_tokens(), "");
    assert!((s.filled() - 0.3).abs() < 1e-3 && s.total_cost == 2.0 * one && s.total_out == 2_400);
}

#[test]
fn a_request_names_its_window() {
    let beta = [("anthropic-beta".to_string(), "context-1m-2025-08-07".to_string())];
    assert_eq!(proxy::window("claude-opus-5-5", &[]), 200_000);
    assert_eq!(proxy::window("claude-sonnet-4-5[1m]", &[]), 1_000_000);
    assert_eq!(proxy::window("claude-opus-5-5", &beta), 1_000_000);
}

#[test]
fn a_shorter_label_waits_before_the_island_narrows() {
    let mut s = State::default();
    let run = |s: &mut State, secs: f32| for _ in 0..(secs * 60.0) as usize { s.step(1.0 / 60.0); };
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Block { id: 1, kind: Kind::Tool("graft_find_code".into()) });
    run(&mut s, 1.0);
    let wide = s.wing;
    s.apply(Ev::Block { id: 1, kind: Kind::Tool("Read".into()) });
    run(&mut s, 1.0);
    assert!((s.wing - wide).abs() < 0.5, "narrowed too soon");
    run(&mut s, 1.5);
    assert!(s.wing < wide - 10.0, "never narrowed");
}

#[test]
fn the_label_goes_before_the_island_narrows_past_it() {
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Block { id: 1, kind: Kind::Thinking });
    for _ in 0..60 { s.step(1.0 / 60.0); }
    let wide = s.wing;
    // Stopped mid-think: the request ends with no stop reason.
    s.apply(Ev::End { id: 1, status: 0 });
    let mut frames = 0;
    while s.step(1.0 / 60.0) {
        frames += 1;
        assert!(frames < 600, "never settled");
        if s.wing < wide - 1.0 { assert!(s.label.is_empty() && s.swap == 1.0, "label cut as the island narrowed") }
    }
    assert_eq!(s.wing, 0.0);
}

#[test]
fn a_chunk_landing_after_a_stopped_request_ends_is_ignored() {
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Block { id: 1, kind: Kind::Tool("Bash".into()) });
    s.apply(Ev::End { id: 1, status: 0 });
    s.apply(Ev::Stop { id: 1, reason: "tool_use".into() });
    s.apply(Ev::Delta { id: 1, kind: Kind::Text, text: "late".into() });
    let mut frames = 0;
    while s.step(1.0 / 60.0) { frames += 1; assert!(frames < 600, "stuck working") }
    assert!(!s.running && s.wing == 0.0);
}

#[test]
fn without_a_notch_a_pill_floats_centred_under_the_top_edge() {
    // A 32" 4K monitor at 2560 points: 0.277 mm a point, so the pill zooms to 7.5 mm tall.
    let big = Screen { width: 2560.0, height: 1440.0, scale: 1.5, notch: None, mm: 708.0 / 2560.0, inset: 0.0 };
    assert!((big.zoom() * PILL_LINE * big.mm - PILL_MM).abs() < 0.01);
    assert!(big.window().1 * big.mm > PILL_MM);
    let flat = Screen { width: 1920.0, height: 1080.0, scale: 2.0, notch: None, mm: 0.0, inset: 0.0 };
    let (w, h) = flat.layout();
    assert_eq!(flat.position(), (((1920.0 - w * flat.zoom()) as f64).round() as i32, 0));
    let p = rounded(w * 0.5, PILL_TOP, PILL_CORE * 0.5, PILL_H, PILL_H * 0.5);
    assert!(p.iter().all(|p| p[1] >= PILL_TOP - 0.01 && p[1] <= PILL_TOP + PILL_H + 0.01 && p[1] < h));
    for (a, b) in p.iter().zip(p.iter().rev()) { assert!((a[0] + b[0] - w).abs() < 0.01, "not symmetric") }
}

#[test]
fn a_pill_on_a_notched_screen_stays_small_and_hangs_below_the_notch() {
    let laptop = Screen { width: 1512.0, height: 982.0, scale: 2.0, notch: None, mm: 0.2, inset: 33.0 };
    assert_eq!(laptop.zoom(), 1.0);
    assert_eq!(laptop.position().1, 66);
}

#[test]
fn the_card_reads_tools_models_and_apps_the_way_people_say_them() {
    assert_eq!(card::summary(&serde_json::json!({ "file_path": "/a/b/main.rs", "limit": 40 })), "main.rs");
    assert_eq!(card::summary(&serde_json::json!({ "command": "cargo test\nmore", "description": "Run" })), "cargo test");
    assert_eq!(card::model_name("claude-opus-5-5"), "Opus 5.5");
    assert_eq!(card::effort("high"), "High effort");
    assert_eq!(card::effort("32k thinking"), "32k thinking");
    assert_eq!(card::model_name("claude-sonnet-4-5-20250929[1m]"), "Sonnet 4.5");
    assert_eq!(card::duration(65.0), "1m 05s");
}

#[test]
fn a_press_opens_the_card_while_working_and_the_pointer_leaving_does_not_close_an_untouched_card() {
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: "claude-opus-5-5".into(), window: 200_000, session: String::new() });
    s.press();
    assert!(s.expanded);
    for _ in 0..120 { s.step(1.0 / 60.0); }
    let g = s.geom();
    assert!(g.h > g.row + CARD_H * 0.9 && g.half * 2.0 >= CARD_W - 0.5);
    s.press();
    assert!(!s.expanded);
}

#[test]
fn closing_the_card_bounces_just_past_shut_and_settles() {
    let mut s = State { expanded: true, card: 1.0, ..State::default() };
    s.press();
    let mut least = f32::MAX;
    for _ in 0..240 { s.step(1.0 / 60.0); least = least.min(s.geom().h - s.geom().row) }
    assert!(least < -2.0, "no bounce past shut: {least}");
    assert_eq!(s.card, 0.0);
}

#[test]
fn every_session_shows_on_the_island_and_any_permission_prompt_needs_you() {
    let mut s = State::default();
    let start = |s: &mut State, id, session: &str| s.apply(Ev::Start { id, model: "claude-opus".into(), window: 200_000, session: session.into() });
    start(&mut s, 1, "a");
    s.apply(Ev::Block { id: 1, kind: Kind::Tool("Read".into()) });
    // Session b works alongside a: it shows, and a is counted on the wing.
    start(&mut s, 2, "b");
    s.apply(Ev::Block { id: 2, kind: Kind::Thinking });
    for _ in 0..30 { s.step(1.0 / 60.0); }
    assert!(s.others() == 1 && s.label == "Thinking");
    s.apply(Ev::Needs { session: "a".into() });
    for _ in 0..30 { s.step(1.0 / 60.0); }
    assert!(s.needs && s.label == "Needs you");
    s.apply(Ev::End { id: 1, status: 200 });
    assert_eq!(s.others(), 0);
    let req = serde_json::json!({ "metadata": { "user_id": "user_abc_account_def_session_ghi" } });
    assert_eq!(proxy::session(&req, &[]), "ghi");
    assert_eq!(proxy::session(&serde_json::json!({}), &[]), "");
    let header = [("x-claude-code-session-id".to_string(), "jkl".to_string())];
    assert_eq!(proxy::session(&req, &header), "jkl");
}

#[test]
fn the_card_speaks_for_the_last_session_or_mixes_those_working_at_once() {
    let mut s = State::default();
    let usage = |output, cache_read| proxy::Usage { output, cache_read, ..Default::default() };
    let run = |s: &mut State, id, session: &str, u: proxy::Usage, reason: Option<&str>| {
        s.apply(Ev::Start { id, model: "claude-opus".into(), window: 200_000, session: session.into() });
        s.apply(Ev::Usage { id, usage: u });
        if let Some(r) = reason { s.apply(Ev::Stop { id, reason: r.into() }); s.apply(Ev::End { id, status: 200 }) }
    };
    // a finishes a turn alone: the card is a's.
    run(&mut s, 1, "a", usage(1_000, 40_000), Some("end_turn"));
    assert!(s.turn_tokens() == "1.0k" && (s.filled() - 0.2).abs() < 1e-3);
    // b alone: b's own numbers, a's pill dark.
    s.last_t = 10.0;
    run(&mut s, 2, "b", usage(500, 120_000), Some("end_turn"));
    assert!(s.turn_tokens() == "500" && (s.filled() - 0.6).abs() < 1e-3 && s.focus().len() == 1);
    // Both mid-turn: both lit, output summed, the ring at the fuller one.
    run(&mut s, 3, "a", usage(200, 40_000), None);
    run(&mut s, 4, "b", usage(300, 120_000), None);
    assert_eq!(s.focus().len(), 2);
    s.apply(Ev::End { id: 3, status: 200 });
    s.apply(Ev::End { id: 4, status: 200 });
    assert!(s.turn_tokens() == "500" && (s.filled() - 0.6).abs() < 1e-3);
    // b's turn ends; a, still mid-turn and heard from last, has the card alone.
    s.apply(Ev::Start { id: 5, model: "claude-opus".into(), window: 200_000, session: "b".into() });
    s.apply(Ev::Stop { id: 5, reason: "end_turn".into() });
    s.apply(Ev::End { id: 5, status: 200 });
    run(&mut s, 6, "a", usage(100, 40_000), Some("tool_use"));
    assert!(s.focus().len() == 1 && s.last == "a" && s.turn_tokens() == "300");
}

#[test]
fn an_edit_counts_only_the_lines_it_changes_and_the_tally_counts_up_to_them() {
    let edit = serde_json::json!({ "file_path": "/a/main.rs", "old_string": "a\nb\nc\nd", "new_string": "a\nB\nB2\nd" });
    assert_eq!(card::diff("Edit", &edit), Some(("main.rs".into(), 2, 2)));
    let multi = serde_json::json!({ "file_path": "/a/x.rs", "edits": [{ "old_string": "a", "new_string": "b" }, { "old_string": "", "new_string": "c\nd" }] });
    assert_eq!(card::diff("MultiEdit", &multi), Some(("x.rs".into(), 3, 1)));
    assert_eq!(card::diff("Write", &serde_json::json!({ "file_path": "n.rs", "content": "1\n2\n3\n" })), Some(("n.rs".into(), 3, 0)));
    assert_eq!(card::diff("Read", &edit), None);
    // Streamed: the parser gathers an Edit's input and counts it once whole.
    let evs = std::sync::Mutex::new(Vec::new());
    let mut p = proxy::Parser::default();
    p.id = 7;
    let sse = [
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","name":"Edit","input":{}}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"file_path\":\"/a/main.rs\",\"old_str"}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"ing\":\"a\",\"new_string\":\"a\\nb\"}"}}"#,
        r#"{"type":"content_block_stop","index":1}"#,
    ].map(|l| format!("data: {l}\n")).concat();
    p.feed(sse.as_bytes(), &|e| evs.lock().unwrap().push(e));
    assert!(evs.lock().unwrap().contains(&Ev::Diff { id: 7, file: "main.rs".into(), added: 1, removed: 0 }));
    // The tally counts up rather than jumping, and a new turn starts from zero at once.
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Diff { id: 1, file: "main.rs".into(), added: 40, removed: 6 });
    s.step(1.0 / 60.0);
    assert!(s.shown[0] > 0.0 && s.shown[0] < 40.0);
    for _ in 0..120 { s.step(1.0 / 60.0); }
    assert_eq!(s.diff(), ["+40".to_string(), "−6".to_string()]);
    s.apply(Ev::Stop { id: 1, reason: "end_turn".into() });
    s.apply(Ev::End { id: 1, status: 200 });
    s.apply(Ev::Start { id: 2, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.step(1.0 / 60.0);
    assert_eq!(s.shown, [0.0, 0.0]);
}

#[test]
fn a_rate_limit_counts_down_then_hands_the_turn_back() {
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Limit { id: 1, secs: 90.0 });
    s.apply(Ev::End { id: 1, status: 429 });
    for _ in 0..120 { s.step(1.0 / 60.0); }
    assert!(s.resting() && s.limit_words() == "Back in 1m 30s" && s.ring == 1.0);
    s.last_t = 60.0;
    s.step(1.0 / 60.0);
    assert!((s.ring - 1.0 / 3.0).abs() < 1e-3 && s.limit_words() == "Back in 30s");
    s.last_t = 91.0;
    s.step(1.0 / 60.0);
    assert!(s.limit.is_none() && s.waiting && s.waited < PULSE);
}

#[test]
fn thinking_eases_the_waveform_into_a_sweep_and_back() {
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Block { id: 1, kind: Kind::Thinking });
    s.step(1.0 / 60.0);
    assert!(s.think > 0.0 && s.think < 0.2, "eases in, not at once");
    for _ in 0..120 { s.step(1.0 / 60.0); }
    assert_eq!(s.think, 1.0);
    // The swell moves: the tallest bar changes over a second.
    let top = |s: &State| (0..BARS).max_by(|&a, &b| s.bar(a).total_cmp(&s.bar(b))).unwrap();
    let first = top(&s);
    let moved = (0..60).any(|_| { s.step(1.0 / 60.0); top(&s) != first });
    assert!(moved);
    s.apply(Ev::Block { id: 1, kind: Kind::Text });
    for _ in 0..120 { s.step(1.0 / 60.0); }
    assert_eq!(s.think, 0.0);
}


#[test]
fn a_flood_of_text_moves_the_waveform_calmly() {
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: String::new() });
    s.apply(Ev::Block { id: 1, kind: Kind::Text });
    // Two seconds of a fast answer: a few words every frame at 120 Hz.
    let (mut prev, mut jump, mut sum) = ([0.0; BARS], 0.0f32, 0.0f32);
    for f in 0..240 {
        s.apply(Ev::Delta { id: 1, kind: Kind::Text, text: "some words here ".into() });
        s.step(1.0 / 120.0);
        let now: [f32; BARS] = std::array::from_fn(|i| s.bar(i));
        if f > 60 { for i in 0..BARS { jump = jump.max((now[i] - prev[i]).abs()); sum += (now[i] - prev[i]).abs() } }
        prev = now;
    }
    // Glides rather than jumps: under 3% of the bar a frame, and about a height a second at most.
    let travel = sum / BARS as f32 / (179.0 / 120.0);
    assert!(jump < 0.03 && travel < 1.5, "jump {jump:.3}, travel {travel:.2}/s");
    assert!(s.bars.iter().any(|b| *b > 0.2), "still moves with the stream");
}

#[test]
fn the_card_line_reads_the_todo_step_or_else_the_last_few_tools() {
    let todos = serde_json::json!({ "todos": [
        { "content": "Read", "status": "completed", "activeForm": "Reading" },
        { "content": "Fix the parser", "status": "in_progress", "activeForm": "Fixing the parser" },
        { "content": "Test", "status": "pending", "activeForm": "Testing" },
    ] });
    assert_eq!(card::todo(&todos), Some((2, 3, "Fixing the parser".into())));
    assert_eq!(card::todo(&serde_json::json!({ "todos": [{ "content": "x", "status": "completed" }] })), None);
    let mut s = State::default();
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: "a".into() });
    for (tool, input) in [("Read", r#"{"file_path":"/x/card.rs"}"#), ("Bash", r#"{"command":"cargo test"}"#), ("Grep", r#"{"pattern":"fn main"}"#), ("Edit", r#"{"file_path":"/x/main.rs"}"#), ("Read", r#"{"file_path":"/x/tests.rs"}"#)] {
        s.apply(Ev::Block { id: 1, kind: Kind::Tool(tool.into()) });
        s.apply(Ev::Arg { id: 1, arg: card::summary(&serde_json::from_str(input).unwrap()) });
    }
    // The last four, oldest first, each with its argument; the list is kept across turns, the trail isn't.
    let trail: Vec<(String, String)> = s.lead().unwrap().trail.iter().map(|(n, a, _)| (n.clone(), a.clone())).collect();
    assert_eq!(trail, [("Bash", "cargo test"), ("Grep", "fn main"), ("Edit", "main.rs"), ("Read", "tests.rs")].map(|(n, a)| (n.to_string(), a.to_string())));
    s.apply(Ev::Todo { id: 1, step: Some((2, 3, "Fixing the parser".into())) });
    s.apply(Ev::Stop { id: 1, reason: "end_turn".into() });
    s.apply(Ev::End { id: 1, status: 200 });
    s.apply(Ev::Start { id: 2, model: "claude-opus".into(), window: 200_000, session: "a".into() });
    assert!(s.lead().unwrap().trail.is_empty() && s.lead().unwrap().todo.is_some());
}


/// Feeds `sse` lines to `p` and plays what it reads into `s`.
fn play(s: &mut State, p: &mut proxy::Parser, sse: &[&str]) {
    let evs = std::sync::Mutex::new(Vec::new());
    p.feed(sse.iter().map(|l| format!("data: {l}\n")).collect::<String>().as_bytes(), &|e| evs.lock().unwrap().push(e));
    for e in evs.into_inner().unwrap() { s.apply(e) }
}

#[test]
fn each_session_keeps_its_own_tool_arguments_while_both_stream() {
    let mut s = State::default();
    let (mut a, mut b) = (proxy::Parser::default(), proxy::Parser::default());
    (a.id, b.id) = (1, 2);
    s.apply(Ev::Start { id: 1, model: "claude-opus".into(), window: 200_000, session: "a".into() });
    s.apply(Ev::Start { id: 2, model: "claude-opus".into(), window: 200_000, session: "b".into() });
    play(&mut s, &mut a, &[
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_01","name":"Bash","input":{}}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"command\": \"cargo"}}"#,
    ]);
    play(&mut s, &mut b, &[
        r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_02","name":"Read","input":{}}}"#,
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"file_path\": \"/x/card.rs\"}"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
        r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_03","name":"Skill","input":{}}}"#,
        r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"skill\": \"pdf\"}"}}"#,
        r#"{"type":"content_block_stop","index":1}"#,
    ]);
    play(&mut s, &mut a, &[
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":" test\"}"}}"#,
        r#"{"type":"content_block_stop","index":0}"#,
    ]);
    let trail = |id: &str| s.chats.iter().find(|c| c.id == id).unwrap().trail.iter().map(|(n, a, _)| format!("{n} {a}")).collect::<Vec<_>>();
    assert_eq!(trail("a"), ["Bash cargo test"]);
    assert_eq!(trail("b"), ["Read card.rs", "pdf pdf"]);
}

#[test]
fn usage_read_across_the_stream_prices_the_request() {
    let sse = concat!(
        "event: message_start\n",
        r#"data: {"type":"message_start","message":{"id":"msg_01","type":"message","role":"assistant","model":"claude-opus-5-5","content":[],"stop_reason":null,"stop_sequence":null,"usage":{"input_tokens":3,"cache_creation_input_tokens":2048,"cache_read_input_tokens":41210,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":2048},"output_tokens":1,"service_tier":"standard"}}}"#, "\n\n",
        "event: ping\ndata: {\"type\": \"ping\"}\n\n",
        "event: message_delta\n",
        r#"data: {"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":512}}"#, "\n\n",
        "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
    );
    let evs = std::sync::Mutex::new(Vec::new());
    let mut p = proxy::Parser::default();
    // Network reads split lines anywhere, here mid-way through the first event's JSON.
    let (x, y) = sse.split_at(sse.find("cache_read").unwrap());
    for part in [x, y] { p.feed(part.as_bytes(), &|e| evs.lock().unwrap().push(e)) }
    let evs = evs.into_inner().unwrap();
    let Some(Ev::Usage { usage, .. }) = evs.iter().rfind(|e| matches!(e, Ev::Usage { .. })) else { panic!("no usage in {evs:?}") };
    assert_eq!(*usage, proxy::Usage { input: 3, output: 512, cache_read: 41_210, cache_write: 2048, cache_write_1h: 2048 });
    assert!(evs.contains(&Ev::Stop { id: 0, reason: "end_turn".into() }));
    // 3 × $4/M + 2048 × $8/M (an hour's cache write) + 41,210 × $0.20/M + 512 × $20/M.
    assert!((ledger::price("claude-opus-5-5", usage) - 0.034878).abs() < 1e-9);
}

/// A fake API that sends `answer` to one request, then holds the connection open for `hold`;
/// and the relay in front of it, with what it reports.
fn relay_to(answer: &'static str, hold: std::time::Duration) -> (u16, std::sync::mpsc::Receiver<Ev>) {
    use std::io::{Read, Write};
    let api = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let upstream = format!("http://{}", api.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut c, _) = api.accept().unwrap();
        // Read the whole request, which `ask` ends with its `{}` body: closing on unread bytes
        // resets the connection, and the relay would see the answer fail.
        let mut got = Vec::new();
        while !got.ends_with(b"{}") {
            let mut buf = [0u8; 4096];
            let n = c.read(&mut buf).unwrap();
            if n == 0 { break }
            got.extend_from_slice(&buf[..n]);
        }
        c.write_all(answer.as_bytes()).unwrap();
        std::thread::sleep(hold);
    });
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let (tx, rx) = std::sync::mpsc::channel();
    proxy::serve(port, upstream, Arc::new(move |e| { let _ = tx.send(e); })).unwrap();
    (port, rx)
}

fn ask(port: u16) -> std::net::TcpStream {
    use std::io::Write;
    let mut c = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    c.write_all(b"POST /v1/messages HTTP/1.1\r\nhost: x\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n{}").unwrap();
    c
}

#[test]
fn the_relay_hands_back_a_redirect_and_every_header_once() {
    use std::io::Read;
    let (port, rx) = relay_to("HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:9/elsewhere\r\nset-cookie: a=1\r\nset-cookie: b=2\r\ncontent-length: 0\r\n\r\n", Default::default());
    let mut got = String::new();
    ask(port).read_to_string(&mut got).unwrap();
    assert!(got.starts_with("HTTP/1.1 302"), "{got}");
    for h in ["location: http://127.0.0.1:9/elsewhere", "set-cookie: a=1", "set-cookie: b=2"] { assert_eq!(got.matches(h).count(), 1, "{h} in {got}") }
    // The relay keeps its sender for good, so wait a while rather than for the channel to close.
    assert!(std::iter::from_fn(|| rx.recv_timeout(std::time::Duration::from_secs(2)).ok()).any(|e| e == Ev::End { id: 0, status: 302 }));
}

#[test]
fn stopping_claude_mid_think_ends_the_request_at_once() {
    use std::io::Read;
    let start = r#"{"type":"message_start","message":{"usage":{"input_tokens":3}}}"#;
    let answer = format!("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\nevent: message_start\ndata: {start}\n\n").leak();
    // The API goes quiet, as it does through a long think.
    let (port, rx) = relay_to(answer, std::time::Duration::from_secs(10));
    let mut c = ask(port);
    let mut got = Vec::new();
    while !String::from_utf8_lossy(&got).contains("message_start") {
        let mut buf = [0u8; 1024];
        let n = c.read(&mut buf).unwrap();
        got.extend_from_slice(&buf[..n]);
    }
    drop(c);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match rx.recv_timeout(end.saturating_duration_since(std::time::Instant::now())) {
            Ok(Ev::End { status, .. }) => break assert_eq!(status, 0),
            Ok(Ev::Error(e)) => panic!("a stop is no error: {e}"),
            Ok(_) => {}
            Err(_) => panic!("the request was still live 2s after the client left"),
        }
    }
}

#[test]
fn a_session_quiet_fifteen_minutes_leaves_the_pill_row() {
    let mut s = State::default();
    for (id, session) in [(1, "a"), (2, "b")] {
        s.apply(Ev::Start { id, model: String::new(), window: 200_000, session: session.into() });
        s.apply(Ev::End { id, status: 200 });
    }
    s.last_t = chat::IDLE - 1.0;
    assert_eq!(expanded::pilled(&s).len(), 2);
    s.last_t = chat::IDLE + 1.0;
    s.apply(Ev::Start { id: 3, model: String::new(), window: 200_000, session: "b".into() });
    // "a" went quiet; "b" just spoke.
    assert_eq!(expanded::pilled(&s).iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["b"]);
}

#[test]
fn the_guide_shows_until_settings_send_claude_code_to_atolls_port_and_a_request_closes_it() {
    let pasted = crate::guide::snippet(8788);
    assert!(crate::guide::configured(&pasted, 8788));
    assert!(!crate::guide::configured(&pasted, 9000));
    assert!(crate::guide::configured(r#"{ "model": "opus", "env": { "ANTHROPIC_BASE_URL": "http://localhost:8788/" } }"#, 8788));
    assert!(!crate::guide::configured(r#"{ "env": { "ANTHROPIC_BASE_URL": "https://gateway.example.com" } }"#, 8788));
    assert!(!crate::guide::configured("not json", 8788));
    assert!(pasted.contains("http://127.0.0.1:8788/atoll") && pasted.contains("permission_prompt"));
    let mut g = crate::guide::Guide::new(8788);
    assert!(!g.opening(0.0));
    g.opened = Some(3.0);
    assert!(g.opening(4.9) && !g.opening(5.0));
    let mut s = State { guide: Some(crate::guide::Guide::new(8788)), expanded: true, ..State::default() };
    s.apply(Ev::Start { id: 1, model: "claude-opus-5-5".into(), window: 200_000, session: String::new() });
    assert!(s.guide.is_none());
}
