/// # Dynamic Token Telemetry — CV Token Detection & Tracking
///
/// Dedicated low-latency thread that scans flower fields for high-priority
/// collectible tokens using colour-space segmentation and contour detection.
///
/// ## Pipeline
/// ```
/// Frame Grab → HSV Threshold → Contour Detection → Sort by Distance → Emit
/// ```
///
/// Runs at ~15 FPS to minimise CPU while keeping token positions fresh.

use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

use log::trace;
use serde::{Deserialize, Serialize};

/// A single token detected on the field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// Normalised screen X (0.0 – 1.0).
    pub x: f64,
    /// Normalised screen Y (0.0 – 1.0).
    pub y: f64,
    /// Token type identifier.
    pub token_type: TokenType,
    /// Estimated time until despawn (seconds, -1 if unknown).
    pub ttl_seconds: f64,
    /// Distance from the player's current position (normalised).
    pub distance: f64,
}

/// Categorical types of collectible tokens in Bee Swarm Simulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TokenType {
    /// +1 basic pollen token (white/yellow).
    BasicPollen,
    /// Red ability token (e.g., Pop Star, Gummy Star).
    Ability,
    /// Link token (connects to other players).
    Link,
    /// Sprout summon token (green).
    Sprout,
    /// Moon amulet fragment.
    MoonAmulet,
    /// Bitterberry / treat.
    Treat,
    /// Glue / goo splatter.
    Goo,
    /// Honey drop token.
    Honey,
    /// Unknown / unclassified.
    Unknown,
}

/// Configuration for the token detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenDetectorConfig {
    /// Number of tokens to track per frame (capped for performance).
    pub max_tokens_per_frame: usize,
    /// Minimum contour area to consider as a token (px²).
    pub min_token_area_px: u32,
    /// Scan interval (ms) — lower = faster but more CPU.
    pub scan_interval_ms: u64,
    /// HSV ranges for each token type.
    /// These are calibrated for the specific game palette.
    pub colour_ranges: Vec<TokenColourRange>,
}

impl Default for TokenDetectorConfig {
    fn default() -> Self {
        Self {
            max_tokens_per_frame: 50,
            min_token_area_px: 16,
            scan_interval_ms: 66, // ~15 FPS
            colour_ranges: vec![
                // Basic pollen — yellowish-white
                TokenColourRange { token_type: TokenType::BasicPollen, h_min: 20, h_max: 40, s_min: 30, s_max: 100, v_min: 150, v_max: 255 },
                // Ability tokens — reddish
                TokenColourRange { token_type: TokenType::Ability,      h_min: 0,  h_max: 10,  s_min: 80, s_max: 255, v_min: 100, v_max: 255 },
                // Link tokens — cyan
                TokenColourRange { token_type: TokenType::Link,         h_min: 80, h_max: 100, s_min: 60, s_max: 200, v_min: 100, v_max: 255 },
                // Sprout — bright green
                TokenColourRange { token_type: TokenType::Sprout,       h_min: 35, h_max: 55,  s_min: 70, s_max: 255, v_min: 100, v_max: 255 },
                // Honey — golden
                TokenColourRange { token_type: TokenType::Honey,        h_min: 15, h_max: 30,  s_min: 80, s_max: 255, v_min: 180, v_max: 255 },
            ],
        }
    }
}

/// An HSV colour range that maps to a token type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenColourRange {
    pub token_type: TokenType,
    pub h_min: u8, pub h_max: u8,
   pub s_min: u8, pub s_max: u8,
    pub v_min: u8, pub v_max: u8,
}

/// The token detector engine.
pub struct TokenDetector {
    config: TokenDetectorConfig,
    /// Tokes currently being tracked.
    pub tracked_tokens: Vec<Token>,
    last_scan: Instant,
    frame_count: u64,
}

impl TokenDetector {
    pub fn new(config: TokenDetectorConfig) -> Self {
        Self {
            config,
            tracked_tokens: Vec::new(),
            last_scan: Instant::now(),
            frame_count: 0,
        }
    }

    /// Process a new camera frame. Returns detected tokens.
    /// 
    /// # Arguments
    /// * `rgba_frame` — raw RGBA pixel buffer (width × height × 4 bytes).
    /// * `player_x`, `player_y` — player's position for distance sort.
    pub fn process_frame(
        &mut self,
        rgba_frame: &[u8],
        width: u32,
        height: u32,
        player_x: f64,
        player_y: f64,
    ) -> &[Token] {
        let now = Instant::now();
        if now.duration_since(self.last_scan) < Duration::from_millis(self.config.scan_interval_ms) {
            return &self.tracked_tokens; // rate-limited
        }
        self.last_scan = now;
        self.frame_count += 1;

        // Step 1: Convert RGBA → HSV and threshold each colour range.
        // In production: use OpenCV cvtColor + inRange for each TokenColourRange.
        let mut detections = Vec::new();
        let step = 4; // RGBA stride

        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) as usize * step;
                let (r, g, b) = (rgba_frame[idx], rgba_frame[idx + 1], rgba_frame[idx + 2]);

                // Simple RGB → HSV conversion (precise version in production).
                let (h, s, v) = Self::rgb_to_hsv(r, g, b);

                for range in &self.config.colour_ranges {
                    if h >= range.h_min && h <= range.h_max
                        && s >= range.s_min && s <= range.s_max
                        && v >= range.v_min && v <= range.v_max
                    {
                        let dist = ((player_x - x as f64).powi(2) + (player_y - y as f64).powi(2)).sqrt();
                        detections.push(Token {
                            x: x as f64 / width as f64,
                            y: y as f64 / height as f64,
                            token_type: range.token_type,
                            ttl_seconds: -1.0,
                            distance: dist,
                        });
                        break; // first match wins
                    }
                }
            }
        }

        // Step 2: Cluster close detections (mean-shift in production).
        // Step 3: Sort by distance (closest first).
        detections.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal));

        // Cap
        detections.truncate(self.config.max_tokens_per_frame);
        self.tracked_tokens = detections;

        trace!("Frame #{}: {} tokens detected", self.frame_count, self.tracked_tokens.len());
        &self.tracked_tokens
    }

    /// Simple RGB → HSV conversion (integer approximation).
    /// Accurate enough for colour-threshold detection.
    fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let r = r as f32 / 255.0;
        let g = g as f32 / 255.0;
        let b = b as f32 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;

        // Hue
        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * ((g - b) / delta % 6.0)
        } else if max == g {
            60.0 * ((b - r) / delta + 2.0) 
        } else {
            60.0 * ((r - g) / delta + 4.0)
        };

        // Saturation
        let s = if max == 0.0 { 0.0 } else { delta / max };

        // Value
        let v = max;

        (
            ((h % 360.0) / 2.0) as u8,  // OpenCV H range: 0–179
            (s * 255.0) as u8,
            (v * 255.0) as u8,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_hsv_basic() {
        // Pure red → H=0, S=255, V=255
        let (h, s, v) = TokenDetector::rgb_to_hsv(255, 0, 0);
        assert_eq!(h, 0);
        assert_eq!(s, 255);
        assert_eq!(v, 255);
    }

    #[test]
    fn test_rgb_to_hsv_white() {
        let (h, s, v) = TokenDetector::rgb_to_hsv(255, 255, 255);
        assert_eq!(h, 0);   // undefined, defaults to 0
        assert_eq!(s, 0);   // no saturation
        assert_eq!(v, 255); // full brightness
    }

    #[test]
    fn test_token_detector_creation() {
        let config = TokenDetectorConfig::default();
        let detector = TokenDetector::new(config);
        assert!(detector.tracked_tokens.is_empty());
        assert_eq!(detector.frame_count, 0);
    }

    #[test]
    fn test_process_frame_results() {
        let mut detector = TokenDetector::new(TokenDetectorConfig::default());
        // Create a dummy frame (all black = no tokens)
        let frame = vec![0u8; 640 * 480 * 4];
        let tokens = detector.process_frame(&frame, 640, 480, 320.0, 240.0);
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_token_types() {
        let types = vec![
            TokenType::BasicPollen,
            TokenType::Ability,
            TokenType::Link,
            TokenType::Sprout,
            TokenType::Honey,
        ];
        for t in &types {
            match t {
                TokenType::BasicPollen => assert_ne!(*t as u8, 0),
                _ => {}
            }
        }
    }
}
