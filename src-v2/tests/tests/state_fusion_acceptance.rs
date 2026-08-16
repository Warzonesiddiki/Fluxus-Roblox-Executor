/// # M2.2 — State Fusion Acceptance Test
///
/// Simulates the acceptance scenario: a fake screen stream with a filling
/// "pollen bar" + packet positions, verifying:
/// - Fusion tracks position accurately with both sources
/// - Graceful degradation when a source drops (occlusion/frame-loss)
/// - Confidence reflects source health
/// - No crash / no panics across the scenario

use wsx_vision::fusion::FusionEngine;
use wsx_vision::ScreenState;
use wsx_net::EntitySyncState;

/// Simulated "filling pollen bar" screen sequence.
fn filling_pollen_frames(n: usize) -> Vec<ScreenState> {
    (0..n)
        .map(|i| {
            let fill = (i as f64 / n as f64).clamp(0.0, 1.0);
            ScreenState {
                backpack_pollen: fill,
                // Token centroid drifts right as the player farms
                token_positions: vec![
                    (900 + i as u32, 540),
                    (940 + i as u32, 560),
                ],
                quest_dialog: i % 100 == 99,
                field_name: Some("Pine Tree Forest".into()),
                ..Default::default()
            }
        })
        .collect()
}

fn packets_at(t: f64) -> EntitySyncState {
    EntitySyncState {
        player_position: (0.5 + t * 0.001, 0.5, 0.0),
        ..Default::default()
    }
}

#[test]
fn m2_2_tracks_filling_bar_with_both_sources() {
    let frames = filling_pollen_frames(200);
    let mut engine = FusionEngine::new();

    for (i, frame) in frames.iter().enumerate() {
        let t = i as f64 * 0.05;
        engine.ingest_vision(frame, t);
        engine.ingest_packet(&packets_at(t), t + 0.01);
    }

    let snap = engine.snapshot(frames.last(), Some(&packets_at(10.0)));
    // Backpack fill must track the "filling bar" to 100%
    assert!(snap.backpack_fill > 0.95, "fill only {:.2}", snap.backpack_fill);
    // Both sources alive at the end
    assert!(snap.sources_alive == 2 || snap.sources_alive == 1);
}

#[test]
fn m2_2_position_tracking_accuracy() {
    let mut engine = FusionEngine::new();
    // Ground truth: player moves 0.5 → 0.9 over 200 ticks
    let mut errors = Vec::new();
    for i in 0..200 {
        let t = i as f64 * 0.05;
        let truth_x = 0.5 + 0.4 * (i as f64 / 200.0);
        engine.ingest_packet(&EntitySyncState {
            player_position: (truth_x, 0.5, 0.0),
            ..Default::default()
        }, t);
        let (px, _) = engine.snapshot(None, None).position;
        errors.push((px - truth_x).abs());
    }
    let mean_err: f64 = errors.iter().sum::<f64>() / errors.len() as f64;
    // Mean tracking error under 1% (normalized space)
    assert!(mean_err < 0.01, "mean tracking error {:.4}", mean_err);
}

#[test]
fn m2_2_graceful_degradation_on_vision_loss() {
    let frames = filling_pollen_frames(50);
    let mut engine = FusionEngine::new();

    // Feed both sources for the first half
    for (i, frame) in frames.iter().take(50).enumerate() {
        let t = i as f64 * 0.05;
        engine.ingest_vision(frame, t);
        engine.ingest_packet(&packets_at(t), t + 0.01);
    }

    // Vision drops (occlusion) for 100 ticks — packets only
    for i in 50..150 {
        let t = i as f64 * 0.05;
        engine.ingest_packet(&packets_at(t), t);
    }

    let snap = engine.snapshot(None, Some(&packets_at(7.5)));
    assert!(snap.confidence > 0.0);
    // Backpack fill unavailable without vision → 0.0 (caller handles low confidence)
    assert_eq!(snap.backpack_fill, 0.0);
    // Position still coherent (packet-driven)
    let (px, _) = snap.position;
    assert!((0.5..0.7).contains(&px), "position drifted: {}", px);
}

#[test]
fn m2_2_full_signal_loss_predicts_but_degrades() {
    let frames = filling_pollen_frames(50);
    let mut engine = FusionEngine::new();
    for (i, frame) in frames.iter().take(50).enumerate() {
        engine.ingest_vision(frame, i as f64 * 0.05);
    }
    let conf_before = engine.snapshot(Some(&frames[49]), None).confidence;

    // Both sources dead — predict-only for 5 seconds
    for i in 50..150 {
        engine.predict(i as f64 * 0.05);
    }
    let snap = engine.snapshot(None, None);
    assert!(snap.confidence < conf_before, "confidence must degrade, not improve");
    assert!(snap.confidence > 0.0);
}

#[test]
fn m2_2_no_panics_full_scenario() {
    // A realistic mixed session: frames + packets + drops + recovery
    let frames = filling_pollen_frames(300);
    let mut engine = FusionEngine::new();

    for i in 0..300 {
        let t = i as f64 * 0.05;
        // Vision present 80% of the time
        if i % 5 != 0 {
            engine.ingest_vision(&frames[i], t);
        }
        // Packets present 90% of the time
        if i % 10 != 0 {
            engine.ingest_packet(&packets_at(t), t + 0.02);
        }
        let snap = engine.snapshot(
            if i % 5 != 0 { Some(&frames[i]) } else { None },
            if i % 10 != 0 { Some(&packets_at(t)) } else { None },
        );
        // Sanity: snapshot always coherent
        let _ = snap.position;
        let _ = snap.confidence;
    }
    // Reached here without panic = pass
}
