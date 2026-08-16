/// # Humanized Input Engine
///
/// Simulates mouse and keyboard input at the **hardware / HID level**
/// to bypass behavioural anti-cheat detection.
///
/// ## Anti-detection features
/// - **Bezier-curve mouse paths**: No straight-line movements. Curves with
///   pseudo-random control points produce human-like acceleration/deceleration.
/// - **Micro-delays**: Every action has a random latency between the specified
///   bounds, mimicking human reaction time variance.
/// - **Jitter**: Tiny sub-pixel oscillations added to every mouse move.

pub mod humanizer;
pub mod stats;
pub mod recorder;

use std::ops::Range;
use std::thread;
use std::time::{Duration, Instant};

use log::{debug, trace};
use rand::Rng;
use thiserror::Error;

// ─── Error handling ────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum InputError {
    #[error("Failed to move mouse: {0}")]
    MouseMoveFailed(String),
    #[error("Failed to press key '{0}'")]
    KeyPressFailed(String),
    #[error("Failed to click: {0}")]
    ClickFailed(String),
    #[error("Input driver not initialized")]
    NotInitialized,
}

// ─── Humanized Input Engine ───────────────────────────────────────────────────

/// The main input driver. Wraps OS input APIs with the humanization engine.
///
/// Every action latency is drawn from the session personality's lognormal
/// model; every mouse move follows a personality-driven Bezier curve.
pub struct HumanizedInput {
    /// The session's humanization engine (personality + idle planner).
    pub humanizer: humanizer::HumanizationEngine,
    /// In production, an `enigo::Enigo` instance lives here.
    _inner: (),
}

impl HumanizedInput {
    /// Create a new input driver with a fresh random session personality.
    pub fn new() -> Result<Self, InputError> {
        Ok(Self {
            humanizer: humanizer::HumanizationEngine::new_session(),
            _inner: (),
        })
    }

    /// Create an input driver with a specific personality (deterministic tests).
    pub fn with_personality(p: humanizer::Personality) -> Result<Self, InputError> {
        Ok(Self {
            humanizer: humanizer::HumanizationEngine::with_personality(p),
            _inner: (),
        })
    }

    /// The latency (ms) for the next action, from the session personality.
    pub fn next_action_latency_ms(&mut self) -> u64 {
        self.humanizer.next_action_latency_ms()
    }

    // ─── Mouse ──────────────────────────────────────────────────────────────

    /// Move the cursor to (x, y) along a **Bezier curve** with a randomized
    /// human-like duration.
    ///
    /// We compute 3 control points: the start, two intermediate "wobble"
    /// points offset by pseudo-random amounts, and the target. The cursor
    /// traverses the curve in `dur_range` milliseconds.
    ///
    /// This eliminates the straight-line "bot teleport" signature.
    pub fn move_mouse_bezier(&mut self, target_x: i32, target_y: i32, dur_range: Range<u64>) {
        // Step 1: Get current cursor position (simulated).
        let (start_x, start_y) = (960, 540); // center-of-screen fallback
        let _current = (start_x, start_y);

        // Step 2: Generate two random control points that deviate from the
        // direct line, creating a natural arc.
        let mut rng = rand::thread_rng();
        let mid1_x = start_x + (target_x - start_x) / 3 + rng.gen_range(-100..100);
        let mid1_y = start_y + (target_y - start_y) / 3 + rng.gen_range(-100..100);
        let mid2_x = start_x + 2 * (target_x - start_x) / 3 + rng.gen_range(-100..100);
        let mid2_y = start_y + 2 * (target_y - start_y) / 3 + rng.gen_range(-100..100);

        // Step 3: Sample points along the cubic Bezier.
        let steps = rng.gen_range(20..40u32);
        let step_dur = rng.gen_range(dur_range.start..dur_range.end) as f64 / steps as f64;

        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let t_inv = 1.0 - t;

            let x = (t_inv.powi(3) * start_x as f64
                + 3.0 * t_inv.powi(2) * t * mid1_x as f64
                + 3.0 * t_inv * t.powi(2) * mid2_x as f64
                + t.powi(3) * target_x as f64) as i32;

            let y = (t_inv.powi(3) * start_y as f64
                + 3.0 * t_inv.powi(2) * t * mid1_y as f64
                + 3.0 * t_inv * t.powi(2) * mid2_y as f64
                + t.powi(3) * target_y as f64) as i32;

            // Add micro-jitter (sub-pixel noise)
            let jitter_x = rng.gen_range(-2..=2);
            let jitter_y = rng.gen_range(-2..=2);

            // In production: enigo.move_mouse(x + jitter_x, y + jitter_y)
            trace!("move_mouse({}, {})", x + jitter_x, y + jitter_y);

            // Wait for step_dur (with ±20% variance)
            let wait_ms = step_dur * (0.8 + rng.gen::<f64>() * 0.4);
            thread::sleep(Duration::from_millis(wait_ms as u64).max(Duration::from_millis(1)));
        }

        debug!("move_mouse_bezier: ({}, {}) → ({}, {}) in {:?} range",
               start_x, start_y, target_x, target_y, dur_range);
    }

    /// Perform a mouse click at the current cursor position.
    /// Latency comes from the session personality (lognormal), and the
    /// press/release split is randomized around it.
    pub fn click(&mut self) {
        // In production: enigo.button_down(MouseButton::Left) + sleep + up
        let latency = self.next_action_latency_ms();
        let down_ms = latency / 2 + rand::thread_rng().gen_range(0..10);
        let up_ms = latency - down_ms + rand::thread_rng().gen_range(0..10);

        thread::sleep(Duration::from_millis(down_ms));
        thread::sleep(Duration::from_millis(up_ms));

        trace!("click() — down {}ms, up {}ms (personality latency {}ms)", down_ms, up_ms, latency);
    }

    // ─── Keyboard ──────────────────────────────────────────────────────────

    /// Simulate a key press (down + delay + up) with personality latency.
    pub fn press_key(&mut self, key: &str) {
        let latency = self.next_action_latency_ms();
        let hold_ms = latency / 2 + rand::thread_rng().gen_range(0..15);
        let after_ms = latency - hold_ms + rand::thread_rng().gen_range(0..15);

        // In production: enigo.key_down(Key::Layout(key)) + sleep + key_up
        trace!("key_press('{}') — hold {}ms, wait {}ms", key, hold_ms, after_ms);

        thread::sleep(Duration::from_millis(hold_ms));
        thread::sleep(Duration::from_millis(after_ms));
    }

    // ─── Timing ───────────────────────────────────────────────────────────────

    /// Wait for a fixed duration (with ±15% jitter).
    pub fn wait_ms(&mut self, base_ms: u64) {
        let jitter = (base_ms as f64 * 0.15 * rand::thread_rng().gen::<f64>()) as u64;
        let actual = base_ms + jitter;
        thread::sleep(Duration::from_millis(actual));
    }
}

// ─── Tsts ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialization() {
        let input = HumanizedInput::new();
        assert!(input.is_ok());
    }

    #[test]
    fn test_bezier_curve_does_not_crash() {
        let mut input = HumanizedInput::new().unwrap();
        // This should complete without panicking
        input.move_mouse_bezier(500, 400, 100..300);
        input.move_mouse_bezier(1400, 700, 100..300);
    }

    #[test]
    fn test_key_press_does_not_panic() {
        let mut input = HumanizedInput::new().unwrap();
        input.press_key("e");
        input.press_key("r");
    }

    #[test]
    fn test_click_does_not_panic() {
        let mut input = HumanizedInput::new().unwrap();
        input.click();
    }

    #[test]
    fn test_wait_ms_at_least_half() {
        let mut input = HumanizedInput::new().unwrap();
        let start = Instant::now();
        input.wait_ms(100);
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(50));
    }
}
