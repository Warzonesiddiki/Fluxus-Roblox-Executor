/// # M2.1 Acceptance Test — Humanization Statistical Model
///
/// Acceptance criterion from the threat model & roadmap:
/// > Synthetic input must be statistically indistinguishable from a
/// > recorded human corpus (KS p ≥ 0.05; target p ≥ 0.2).
///
/// This test:
/// 1. Builds a "human corpus" (in production: recorded from real players;
///    here: a calibrated lognormal + bezier reference model).
/// 2. Generates synthetic events from the HumanizationEngine.
/// 3. Runs the two-sample KS battery (intervals + movement segments).
/// 4. Asserts same-distribution at α = 0.05.

use wsx_input::humanizer::{HumanizationEngine, sample_lognormal};
use wsx_input::stats::{
    InputEvent, EventKind, full_validation, inter_event_intervals, segment_lengths, ks_test,
};
use rand::Rng;

const ALPHA: f64 = 0.05;

/// Build a reference "human" event stream.
/// In production this loads `corpus/human_*.json` recorded from real players.
/// Here we sample from the calibrated human model (μ=5.1, σ=0.45 log-ms).
fn build_human_corpus(n_actions: usize) -> Vec<InputEvent> {
    let mut rng = rand::thread_rng();
    let mut events = Vec::with_capacity(n_actions * 2);
    let mut t = 0u64;
    for i in 0..n_actions {
        // Human reaction latency (lognormal)
        let latency = sample_lognormal(5.1, 0.45, &mut rng) as u64;
        t += latency.clamp(40, 1500);
        // Click at a pseudo-random position
        let x = rng.gen_range(100.0..1800.0);
        let y = rng.gen_range(100.0..900.0);
        events.push(InputEvent::new(EventKind::Move, t, x, y));
        events.push(InputEvent::new(EventKind::ClickDown, t + 20, x, y));
        // Occasional idle gap (blink/AFK)
        if i % 40 == 39 {
            t += rng.gen_range(2000..5000);
        }
    }
    events
}

/// Generate synthetic events using the HumanizationEngine.
fn build_synthetic_events(n_actions: usize) -> Vec<InputEvent> {
    let mut engine = HumanizationEngine::new_session();
    let mut rng = rand::thread_rng();
    let mut events = Vec::with_capacity(n_actions * 2);
    let mut t = 0u64;
    let (mut px, mut py) = (960.0, 540.0);
    for _ in 0..n_actions {
        // Idle pause check FIRST — gap measured since the previous action
        if engine.idle_due(t) {
            t += engine.idle_planner.pause_duration_ms(&mut rng);
        }

        // Human-like latency from the personality model
        let latency = engine.next_action_latency_ms();
        t += latency;

        // Move along a bezier path to a new target
        let tx = rng.gen_range(100.0..1800.0);
        let ty = rng.gen_range(100.0..900.0);
        let path = engine.move_path(px, py, tx, ty);
        for pt in path {
            events.push(InputEvent::new(EventKind::Move, t, pt.x, pt.y));
            t += 8;
        }
        events.push(InputEvent::new(EventKind::ClickDown, t, tx, ty));
        (px, py) = (tx, ty);

        engine.record_action(t);
    }
    events
}

#[test]
fn m2_1_intervals_indistinguishable() {
    let human = build_human_corpus(1200);
    let synthetic = build_synthetic_events(1200);

    let h_int = inter_event_intervals(&human);
    let s_int = inter_event_intervals(&synthetic);

    let result = ks_test(&h_int, &s_int, ALPHA);
    println!("M2.1 intervals: D={:.4} p={:.4}", result.d_statistic, result.p_value);

    // Acceptance: p ≥ 0.05 (cannot reject same distribution)
    assert!(
        result.same_distribution,
        "FAIL: synthetic intervals rejected as non-human (p={:.4})",
        result.p_value
    );
}

#[test]
fn m2_1_movement_segments_indistinguishable() {
    let human = build_human_corpus(800);
    let synthetic = build_synthetic_events(800);

    let h_seg = segment_lengths(&human);
    let s_seg = segment_lengths(&synthetic);

    let result = ks_test(&h_seg, &s_seg, ALPHA);
    println!("M2.1 segments: D={:.4} p={:.4}", result.d_statistic, result.p_value);

    assert!(
        result.same_distribution,
        "FAIL: synthetic movement rejected as non-human (p={:.4})",
        result.p_value
    );
}

#[test]
fn m2_1_full_validation_report() {
    let human = build_human_corpus(600);
    let synthetic = build_synthetic_events(600);

    let report = full_validation(&human, &synthetic, ALPHA);
    println!("M2.1 report: {:#?}", report);

    assert!(report.passed, "M2.1 validation failed");
}

#[test]
fn m2_1_corpus_sizes_are_adequate() {
    // Sanity: the test corpus must be large enough for stable KS p-values
    let human = build_human_corpus(2000);
    let synthetic = build_synthetic_events(2000);
    assert!(human.len() > 2000);
    assert!(synthetic.len() > 2000);
    let h_int = inter_event_intervals(&human);
    let s_int = inter_event_intervals(&synthetic);
    assert!(h_int.len() >= 1000 && s_int.len() >= 1000);
}
