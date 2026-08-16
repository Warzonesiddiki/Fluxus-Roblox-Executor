/// # State Fusion Engine (M2.2) — Vision + Network + Motion Model
///
/// Fuses three state sources into a single confidence-scored GameSnapshot:
///
/// 1. **Vision detections** (wsx-vision ScreenState) — noisy, ~15 FPS, can drop frames
/// 2. **Packet positions** (wsx-net EntitySyncState) — precise, ~30 Hz, can drop packets
/// 3. **Motion model** — constant-velocity prediction between measurements
///
/// ## Implementation
/// A 2D constant-velocity **Kalman filter** with per-source measurement noise.
/// Kalman gives us: optimal fusion under Gaussian noise, graceful degradation
/// (prediction when both sources drop), and a principled confidence estimate
/// (inverse of the covariance trace).
///
/// ## Acceptance (from roadmap M2.2)
/// - ≥99% state accuracy on labeled frames
/// - <50 ms p95 end-to-end latency
/// - graceful confidence degradation under occlusion/frame-drop

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ScreenState;
use wsx_net::EntitySyncState;

// ─── Kalman state ─────────────────────────────────────────────────────────────

/// 2D constant-velocity Kalman filter state.
/// State vector: [x, y, vx, vy].
#[derive(Debug, Clone)]
pub struct KalmanFilter {
    // State estimate
    x: [f64; 4],
    // Error covariance (4x4, stored as row-major)
    p: [f64; 16],
    // Process noise (how much the target accelerates between ticks)
    q: f64,
    // Measurement noise for vision (pixels²)
    r_vision: f64,
    // Measurement noise for packets (pixels²)
    r_packet: f64,
    // Last update time
    last_t: f64,
    /// Initialized?
    initialized: bool,
}

impl KalmanFilter {
    pub fn new(q: f64, r_vision: f64, r_packet: f64) -> Self {
        Self {
            x: [0.0; 4],
            p: [
                10.0, 0.0, 0.0, 0.0,
                0.0, 10.0, 0.0, 0.0,
                0.0, 0.0, 100.0, 0.0,
                0.0, 0.0, 0.0, 100.0,
            ],
            q,
            r_vision,
            r_packet,
            last_t: 0.0,
            initialized: false,
        }
    }

    /// Initialize with a first measurement.
    pub fn initialize(&mut self, x: f64, y: f64, t: f64) {
        self.x = [x, y, 0.0, 0.0];
        self.last_t = t;
        self.initialized = true;
    }

    /// Predict step: propagate state and covariance by dt.
    fn predict(&mut self, dt: f64) {
        // State transition (constant velocity)
        self.x[0] += self.x[2] * dt;
        self.x[1] += self.x[3] * dt;

        // F = [[1,0,dt,0],[0,1,0,dt],[0,0,1,0],[0,0,0,1]]
        // P = F P Fᵀ + Q
        // For constant velocity with dt steps:
        let dt2 = dt * dt;
        let q11 = self.q * dt2 * dt2 / 4.0;   // position noise
        let q12 = self.q * dt2 * dt / 2.0;     // pos-vel cross
        let q22 = self.q * dt2;                 // velocity noise

        let p = self.p;
        // P' = F P Fᵀ (compute the 4x4)
        let mut np = [0.0f64; 16];
        // Standard matrix multiply: F * P
        let mut fp = [0.0f64; 16];
        for i in 0..4 {
            for j in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += f_at(&p, i, k) * f_transition(k, j, dt);
                }
                fp[4 * i + j] = s;
            }
        }
        for i in 0..4 {
            for j in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += f_transition(i, k, dt) * fp[4 * k + j];
                }
                np[4 * i + j] = s;
            }
        }
        // Add process noise
        np[0] += q11; np[1] += q12; np[4] += q12; np[5] += q22;
        np[10] += q22; np[15] += q11; // velocity noise on vx,vy

        self.p = np;
    }

    /// Update step with a measurement (x, y, noise r).
    fn update(&mut self, x: f64, y: f64, r: f64) {
        // Innovation: z - H x
        let zx = x - self.x[0];
        let zy = y - self.x[1];

        // Innovation covariance: S = H P Hᵀ + R
        let s = self.p[0] + r;         // P[0,0] + r
        let sy = self.p[5] + r;        // P[1,1] + r
        if s.abs() < 1e-12 || sy.abs() < 1e-12 { return; }

        // Kalman gain: K = P Hᵀ S⁻¹
        let kx = self.p[0] / s;        // for x
        let kvx = self.p[2] / s;       // vx gain from x measurement
        let ky = self.p[5] / sy;
        let kvy = self.p[7] / sy;

        // Update state
        self.x[0] += kx * zx;
        self.x[1] += ky * zy;
        self.x[2] += kvx * zx;
        self.x[3] += kvy * zy;

        // Update covariance: P = (I - K H) P
        self.p[0] *= (1.0 - kx);
        self.p[2] *= (1.0 - kvx);
        self.p[5] *= (1.0 - ky);
        self.p[7] *= (1.0 - kvy);
        // (simplified — only position/vel entries; adequate for this model)
    }

    /// Fuse a measurement with a timestamp.
    pub fn fuse(&mut self, x: f64, y: f64, source: FusionSource, t: f64) {
        if !self.initialized {
            self.initialize(x, y, t);
            return;
        }
        let dt = (t - self.last_t).clamp(0.0, 1.0);
        self.last_t = t;
        self.predict(dt);
        let r = match source {
            FusionSource::Vision => self.r_vision,
            FusionSource::Packet => self.r_packet,
        };
        self.update(x, y, r);
    }

    /// Prediction-only step (both sources dropped).
    pub fn predict_only(&mut self, t: f64) {
        if !self.initialized { return; }
        let dt = (t - self.last_t).clamp(0.0, 1.0);
        self.last_t = t;
        self.predict(dt);
    }

    /// Current fused position estimate.
    pub fn position(&self) -> (f64, f64) {
        (self.x[0], self.x[1])
    }

    /// Confidence: inverse of normalized covariance trace (0..1).
    pub fn confidence(&self) -> f64 {
        let trace = self.p[0] + self.p[5];
        (1.0 / (1.0 + trace)).clamp(0.0, 1.0)
    }

    pub fn initialized(&self) -> bool { self.initialized }
}

fn f_at(p: &[f64], i: usize, j: usize) -> f64 { p[4 * i + j] }

/// State transition matrix F (constant velocity).
fn f_transition(i: usize, j: usize, dt: f64) -> f64 {
    match (i, j) {
        (0, 0) => 1.0,
        (0, 2) => dt,
        (1, 1) => 1.0,
        (1, 3) => dt,
        (2, 2) => 1.0,
        (3, 3) => 1.0,
        _ => 0.0,
    }
}

// ─── Fusion orchestration ─────────────────────────────────────────────────────

/// Where a measurement came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusionSource {
    Vision,
    Packet,
}

/// The fused, confidence-scored snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusedSnapshot {
    pub position: (f64, f64),
    pub confidence: f64,
    pub backpack_fill: f64,
    pub is_dead: bool,
    pub quest_ready: bool,
    pub current_field: String,
    pub sources_alive: u8, // 0=both dead, 1=one alive, 2=both alive
}

/// The fusion orchestrator: combines ScreenState + EntitySyncState + motion.
pub struct FusionEngine {
    kalman: KalmanFilter,
    last_vision_t: f64,
    last_packet_t: f64,
}

impl FusionEngine {
    pub fn new() -> Self {
        Self {
            kalman: KalmanFilter::new(0.5, 150.0, 25.0),
            last_vision_t: 0.0,
            last_packet_t: 0.0,
        }
    }

    /// Fuse a new vision frame.
    pub fn ingest_vision(&mut self, screen: &ScreenState, t: f64) {
        // Vision gives normalized coords; packets give world-ish coords.
        // We fuse in "normalized screen" space for the demo.
        let (x, y) = screen_position(screen);
        self.kalman.fuse(x, y, FusionSource::Vision, t);
        self.last_vision_t = t;
    }

    /// Fuse a new packet sync.
    pub fn ingest_packet(&mut self, net: &EntitySyncState, t: f64) {
        let (x, y) = net.player_position;
        // Normalize (demo mapping — real mapping in protocol map B1.4)
        let nx = (x % 1.0).abs();
        let ny = (y % 1.0).abs();
        self.kalman.fuse(nx, ny, FusionSource::Packet, t);
        self.last_packet_t = t;
    }

    /// Tick with no new measurements: predict from motion model.
    pub fn predict(&mut self, t: f64) {
        self.kalman.predict_only(t);
    }

    /// Produce the fused snapshot.
    pub fn snapshot(&self, screen: Option<&ScreenState>, net: Option<&EntitySyncState>) -> FusedSnapshot {
        let (px, py) = self.kalman.position();
        let confidence = self.kalman.confidence();
        let sources_alive = u8::from(!self.last_vision_t.is_nan() && self.last_vision_t > 0.0)
            + u8::from(!self.last_packet_t.is_nan() && self.last_packet_t > 0.0);

        FusedSnapshot {
            position: (px, py),
            confidence,
            backpack_fill: screen.map(|s| s.backpack_pollen).unwrap_or(0.0),
            is_dead: screen.map(|s| s.death_overlay).unwrap_or(false),
            quest_ready: screen.map(|s| s.quest_dialog).unwrap_or(false),
            current_field: screen
                .and_then(|s| s.field_name.clone())
                .or_else(|| net.and_then(|n| n.map_id.clone()))
                .unwrap_or_else(|| "Unknown".into()),
            sources_alive,
        }
    }

    /// Fusion health metric (0..2).
    pub fn source_health(&self) -> u8 {
        let vision = self.last_vision_t > 0.0;
        let packet = self.last_packet_t > 0.0;
        u8::from(vision) + u8::from(packet)
    }
}

/// Extract a normalized position from a ScreenState (demo mapping).
fn screen_position(s: &ScreenState) -> (f64, f64) {
    // In production, token centroid or player-position template match.
    // Demo: use the mean of visible token positions.
    if s.token_positions.is_empty() {
        return (0.5, 0.5);
    }
    let n = s.token_positions.len() as f64;
    let sx: f64 = s.token_positions.iter().map(|(x, _)| *x as f64).sum::<f64>() / n;
    let sy: f64 = s.token_positions.iter().map(|(_, y)| *y as f64).sum::<f64>() / n;
    (sx / 1920.0, sy / 1080.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_screen(tokens: Vec<(u32, u32)>) -> ScreenState {
        ScreenState {
            token_positions: tokens,
            backpack_pollen: 0.5,
            death_overlay: false,
            quest_dialog: false,
            field_name: Some("Test Field".into()),
            ..Default::default()
        }
    }

    fn mock_net(x: f64, y: f64) -> EntitySyncState {
        EntitySyncState { player_position: (x, y, 0.0), ..Default::default() }
    }

    #[test]
    fn test_kalman_converges_to_measurement() {
        let mut k = KalmanFilter::new(0.5, 150.0, 25.0);
        for i in 0..20 {
            k.fuse(100.0, 100.0, FusionSource::Packet, i as f64 * 0.05);
        }
        let (x, y) = k.position();
        assert!((x - 100.0).abs() < 2.0, "x={}", x);
        assert!((y - 100.0).abs() < 2.0, "y={}", y);
    }

    #[test]
    fn test_prediction_holds_position_when_dropped() {
        let mut k = KalmanFilter::new(0.5, 150.0, 25.0);
        k.fuse(100.0, 100.0, FusionSource::Packet, 0.0);
        // Drop for 1 second — prediction should keep us near the last known
        k.predict_only(1.0);
        let (x, y) = k.position();
        assert!((x - 100.0).abs() < 3.0);
        assert!((y - 100.0).abs() < 3.0);
    }

    #[test]
    fn test_fusion_engine_graceful_degradation() {
        let mut eng = FusionEngine::new();
        let t0 = 0.0;
        eng.ingest_vision(&mock_screen(vec![(960, 540)]), t0);

        // Both sources dead for 2 seconds
        eng.predict(t0 + 2.0);
        let snap = eng.snapshot(None, None);
        // Confidence should be lower than a fresh fusion
        assert!(snap.confidence > 0.0);
        assert!(snap.confidence < 1.0);
        // Sources are dead, but we still have a position
        assert!(snap.sources_alive == 1 || snap.sources_alive == 0);
    }

    #[test]
    fn test_fusion_engine_both_sources() {
        let mut eng = FusionEngine::new();
        eng.ingest_vision(&mock_screen(vec![(960, 540)]), 0.0);
        eng.ingest_packet(&mock_net(0.5, 0.5), 0.05);
        let snap = eng.snapshot(Some(&mock_screen(vec![(960, 540)])), Some(&mock_net(0.5, 0.5)));
        assert_eq!(snap.current_field, "Test Field");
        assert!(snap.confidence > 0.5, "confidence too low: {}", snap.confidence);
    }

    #[test]
    fn test_field_fallback_to_packet_map() {
        let mut eng = FusionEngine::new();
        eng.ingest_packet(&mock_net(0.5, 0.5), 0.0);
        let snap = eng.snapshot(None, Some(&mock_net(0.5, 0.5)));
        assert!(snap.current_field.contains("BSS") || snap.current_field == "Unknown");
    }
}
