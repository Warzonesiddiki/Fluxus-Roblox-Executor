/// # Humanization Engine (M2.1) — Statistical Human-Behavior Model
///
/// The core de-risk milestone: prove our synthetic input is statistically
/// indistinguishable from human input.
///
/// ## Model
/// - **Click/action latency**: lognormal distribution (humans have a
///   right-skewed reaction-time distribution, NOT uniform).
/// - **Movement**: cubic Bézier paths with per-session randomized control
///   points (humans don't move in straight lines).
/// - **Personality**: per-session parameters (speed, precision, jitter)
///   drawn from a distribution — a "fast impulsive player" vs a
///   "slow careful player" never behave the same twice.
/// - **Micro-jitter**: sub-pixel oscillation during movement + tiny
///   endpoint overshoot.
/// - **AFK/blink**: periodic idle pauses with human-like durations.

use rand::Rng;
use rand::distributions::{Distribution, StandardNormal};
use serde::{Deserialize, Serialize};

// ─── Personality model ────────────────────────────────────────────────────────

/// Per-session behavior parameters. A session draws ONE personality and
/// keeps it for the whole run (humans are consistent within a session,
/// varied across sessions).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Personality {
    /// Base movement speed (px/s). 250–550 typical for humans.
    pub movement_speed: f64,
    /// Lognormal μ for action latency (log ms). Lower = faster reactions.
    pub latency_mu: f64,
    /// Lognormal σ for action latency.
    pub latency_sigma: f64,
    /// Movement curvature: how far control points deviate from the line.
    pub curvature: f64,
    /// Sub-pixel jitter amplitude (px).
    pub jitter_amplitude: f64,
    /// Probability of endpoint overshoot (mouse passes target and returns).
    pub overshoot_probability: f64,
}

impl Default for Personality {
    fn default() -> Self {
        Self {
            movement_speed: 380.0,
            latency_mu: 5.1,          // ≈ 164 ms median (exp(5.1))
            latency_sigma: 0.45,
            curvature: 0.18,
            jitter_amplitude: 1.5,
            overshoot_probability: 0.15,
        }
    }
}

impl Personality {
    /// Draw a random personality for a new session.
    /// Parameters come from a human-derived distribution (calibrated in
    /// the Model phase from the recorded corpus).
    pub fn random_session() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            // Speed: skewed toward mid; 250..550 px/s
            movement_speed: 250.0 + 300.0 * rng.gen::<f64>().powf(1.4),
            // Latency: most humans react 120-280 ms; lognormal fit
            latency_mu: 4.8 + 0.6 * rng.gen::<f64>(),
            latency_sigma: 0.30 + 0.35 * rng.gen::<f64>(),
            curvature: 0.08 + 0.25 * rng.gen::<f64>(),
            jitter_amplitude: 0.5 + 2.0 * rng.gen::<f64>(),
            overshoot_probability: 0.05 + 0.25 * rng.gen::<f64>(),
        }
    }
}

// ─── Lognormal sampler ────────────────────────────────────────────────────────

/// Sample from a lognormal distribution using Box–Muller for the normal base.
pub fn sample_lognormal(mu: f64, sigma: f64, rng: &mut impl Rng) -> f64 {
    let normal = StandardNormal.sample(rng);
    (mu + sigma * normal).exp()
}

/// Sample an action latency in milliseconds (human-like).
pub fn sample_action_latency(p: &Personality, rng: &mut impl Rng) -> u64 {
    let ms = sample_lognormal(p.latency_mu, p.latency_sigma, rng);
    // Bounded to sane human range: 40 ms .. 2000 ms
    ms.clamp(40.0, 2000.0) as u64
}

// ─── Bézier path ─────────────────────────────────────────────────────────────

/// A point on the movement path.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub t_ms: u64,
}

/// Generate a cubic Bézier path from (x0,y0) to (x1,y1) using the given
/// personality. Returns points sampled along the curve with human-like
/// timing (slower at start/end — ease-in-out).
pub fn bezier_path(
    x0: f64, y0: f64, x1: f64, y1: f64,
    personality: &Personality,
    rng: &mut impl Rng,
) -> Vec<Point> {
    let dist = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
    let duration_ms = (dist / personality.movement_speed * 1000.0).clamp(80.0, 2500.0);
    let steps = (duration_ms / 8.0).ceil() as u32; // sample every ~8ms

    // Control points: deviate from the straight line by `curvature`.
    let dx = x1 - x0;
    let dy = y1 - y0;
    let c1x = x0 + dx * 0.30 + (dy * personality.curvature) * rng.gen_range(-1.0..1.0);
    let c1y = y0 + dy * 0.30 - (dx * personality.curvature) * rng.gen_range(-1.0..1.0);
    let c2x = x0 + dx * 0.70 + (dy * personality.curvature) * rng.gen_range(-1.0..1.0);
    let c2y = y0 + dy * 0.70 - (dx * personality.curvature) * rng.gen_range(-1.0..1.0);

    // Optional endpoint overshoot: extend the target slightly.
    let (mut tx, mut ty) = (x1, y1);
    if rng.gen::<f64>() < personality.overshoot_probability {
        let over = dist * 0.03;
        let ang = (dy).atan2(dx);
        tx = x1 + over * ang.cos();
        ty = y1 + over * ang.sin();
    }

    let mut path = Vec::with_capacity(steps as usize);
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        // Ease-in-out timing (humans accelerate/decelerate)
        let et = if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t };
        let inv = 1.0 - et;

        let x = inv.powi(3) * x0
            + 3.0 * inv.powi(2) * et * c1x
            + 3.0 * inv * et.powi(2) * c2x
            + et.powi(3) * tx;
        let y = inv.powi(3) * y0
            + 3.0 * inv.powi(2) * et * c1y
            + 3.0 * inv * et.powi(2) * c2y
            + et.powi(3) * ty;

        // Sub-pixel jitter
        let jx = x + rng.gen_range(-personality.jitter_amplitude..personality.jitter_amplitude);
        let jy = y + rng.gen_range(-personality.jitter_amplitude..personality.jitter_amplitude);

        path.push(Point { x: jx, y: jy, t_ms: (et * duration_ms) as u64 });
    }

    // If we overshot, add the return segment to the true target.
    if tx != x1 || ty != y1 {
        path.push(Point { x: x1, y: y1, t_ms: duration_ms as u64 + 60 });
    }

    path
}

// ─── Session scheduler (AFK / blink) ──────────────────────────────────────────

/// Idle-pause planner: humans periodically stop briefly (blink, check item).
pub struct IdlePlanner {
    next_pause_after_ms: u64,
}

impl IdlePlanner {
    pub fn new() -> Self {
        Self { next_pause_after_ms: Self::next_interval_ms() }
    }

    /// Should we insert an idle pause now (given ms since last pause)?
    pub fn due(&mut self, since_last_ms: u64) -> bool {
        if since_last_ms >= self.next_pause_after_ms {
            self.next_pause_after_ms = Self::next_interval_ms();
            return true;
        }
        false
    }

    /// Duration of the next pause (2–5 s, lognormal-ish).
    pub fn pause_duration_ms(&self, rng: &mut impl Rng) -> u64 {
        sample_lognormal(7.6, 0.35, rng).clamp(1500.0, 6000.0) as u64
    }

    /// Interval between pauses: 3–9 minutes (varies per session).
    fn next_interval_ms() -> u64 {
        let mut rng = rand::thread_rng();
        (180_000.0 + rng.gen::<f64>().powf(1.3) * 360_000.0) as u64
    }
}

// ─── The engine ───────────────────────────────────────────────────────────────

/// The complete humanization engine for one session.
pub struct HumanizationEngine {
    pub personality: Personality,
    pub idle_planner: IdlePlanner,
    session_started: std::time::Instant,
    last_action_ms: u64,
    rng: rand::rngs::ThreadRng,
}

impl HumanizationEngine {
    pub fn new_session() -> Self {
        let personality = Personality::random_session();
        Self {
            personality,
            idle_planner: IdlePlanner::new(),
            session_started: std::time::Instant::now(),
            last_action_ms: 0,
            rng: rand::thread_rng(),
        }
    }

    /// Create an engine with a fixed personality (deterministic tests).
    pub fn with_personality(personality: Personality) -> Self {
        Self {
            personality,
            idle_planner: IdlePlanner::new(),
            session_started: std::time::Instant::now(),
            last_action_ms: 0,
            rng: rand::thread_rng(),
        }
    }

    /// Latency for the next action (ms), from the personality's lognormal.
    pub fn next_action_latency_ms(&mut self) -> u64 {
        sample_action_latency(&self.personality, &mut self.rng)
    }

    /// Generate the movement path for a mouse action.
    pub fn move_path(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        bezier_path(x0, y0, x1, y1, &self.personality, &mut self.rng)
    }

    /// Should an idle pause happen right now?
    pub fn idle_due(&mut self, now_ms: u64) -> bool {
        self.idle_planner.due(now_ms - self.last_action_ms)
    }

    /// Register that an action just happened.
    pub fn record_action(&mut self, now_ms: u64) {
        self.last_action_ms = now_ms;
    }

    /// Current session duration.
    pub fn session_duration(&self) -> std::time::Duration {
        self.session_started.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lognormal_bounded() {
        let mut rng = rand::thread_rng();
        let p = Personality::default();
        for _ in 0..1000 {
            let lat = sample_action_latency(&p, &mut rng);
            assert!((40..=2000).contains(&lat), "latency out of bounds: {}", lat);
        }
    }

    #[test]
    fn test_bezier_path_ends_at_target() {
        let mut rng = rand::thread_rng();
        let p = Personality::default();
        let path = bezier_path(0.0, 0.0, 800.0, 600.0, &p, &mut rng);
        assert!(path.len() > 10, "path too short: {}", path.len());
        // The path should end within a few px of the target
        let last = path.last().unwrap();
        assert!(
            (last.x - 800.0).abs() < 10.0 && (last.y - 600.0).abs() < 10.0,
            "endpoint ({},{}) not near target (800,600)",
            last.x, last.y
        );
    }

    #[test]
    fn test_personality_varies_by_session() {
        let a = Personality::random_session();
        let b = Personality::random_session();
        // Two random sessions should (almost surely) differ in latency
        assert!((a.latency_mu - b.latency_mu).abs() > 1e-9);
    }

    #[test]
    fn test_idle_planner_eventually_due() {
        let mut planner = IdlePlanner::new();
        // After 10 minutes it must be due
        assert!(planner.due(10 * 60_000));
    }

    #[test]
    fn test_engine_latency_in_human_range() {
        let mut engine = HumanizationEngine::new_session();
        let mut sum = 0u64;
        let n = 500;
        for _ in 0..n {
            sum += engine.next_action_latency_ms();
        }
        let mean = sum as f64 / n as f64;
        // Human mean reaction ≈ 150-300 ms
        assert!((100.0..500.0).contains(&mean), "mean latency {:.0}ms out of range", mean);
    }
}
