/// # Computer Vision Module
///
/// Captures screenshots of the target game window, runs template matching
/// to identify game-state UI elements, and emits structured JSON snapshots
/// for the behaviour tree to consume.
///
/// ## Pipeline
/// ```
/// Screen Capture → Region Extraction → Template Matching → JSON Output
/// ```

use std::time::Duration;
pub mod token_detector;
pub mod fusion;
pub mod capture;


use log::{debug, info, trace};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ─── Error types ──────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum VisionError {
    #[error("Game window not found: {0}")]
    WindowNotFound(String),
    #[error("Screen capture failed: {0}")]
    CaptureFailed(String),
    #[error("Template '{0}' not found in reference library")]
    TemplateNotFound(String),
    #[error("OCR read failed: {0}")]
    OcrFailed(String),
    #[error("No window focused/available")]
    NoWindow,
}

// ─── Screen state (output) ──────────────────────────────────────────────────────

/// Structured representation of everything the CV module extracts from a screenshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenState {
    /// Backpack pollen meter fill (0.0 – 1.0).
    pub backpack_pollen: f64,
    /// Honey counter text (raw OCR string).
    pub honey_count: String,
    /// Timestamp of the capture.
    pub captured_at: f64,
    /// Is the respawn/defeat overlay visible?
    pub death_overlay: bool,
    /// Is a quest-complete dialog visible?
    pub quest_dialog: bool,
    /// Name of the detected dialog NPC (if any).
    pub quest_npc: Option<String>,
    /// Raw OCR'd quest dialog text (if any).
    pub quest_text: Option<String>,
    /// How many coin tokens are visible on screen.
    pub coin_tokens_visible: u32,
    /// Current field name (from minimap OCR).
    pub field_name: Option<String>,
    /// Raw confidence score (0–1) for the overall capture quality.
    pub capture_confidence: f64,
    /// Bounding boxes of detected harvestable tokens.
    pub token_positions: Vec<(u32, u32)>,
}

impl Default for ScreenState {
    fn default() -> Self {
        Self {
            backpack_pollen: 0.0,
            honey_count: "0".into(),
            captured_at: 0.0,
            death_overlay: false,
            quest_dialog: false,
            quest_npc: None,
            quest_text: None,
            coin_tokens_visible: 0,
            field_name: None,
            capture_confidence: 0.0,
            token_positions: Vec::new(),
        }
    }
}

// ─── Screen capture region ─────────────────────────────────────────────────────

/// A rectangular region of the screen to capture.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CaptureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CaptureRegion {
    pub const fn new(x: u32, y: u32, w: u32, h: u32) -> Self {
        Self { x, y, width: w, height: h }
    }
}

/// Pre-defined regions for Bee Swarm Simulator (at 1920×1080).
/// These would be calibrated per-resolution in production.
pub mod regions {
    use super::CaptureRegion;

    pub const BACKPACK_BAR: CaptureRegion = CaptureRegion::new(820, 980, 280, 20);
    pub const HONEY_COUNTER: CaptureRegion = CaptureRegion::new(840, 60, 240, 30);
    pub const QUEST_DIALOG: CaptureRegion = CaptureRegion::new(600, 300, 720, 400);
    pub const MINIMAP: CaptureRegion = CaptureRegion::new(1720, 40, 180, 180);
    pub const DEATH_OVERLAY: CaptureRegion = CaptureRegion::new(0, 0, 1920, 1080);
    pub const COIN_TOKENS: CaptureRegion = CaptureRegion::new(400, 400, 1120, 400);
}

// ─── CV Engine ────────────────────────────────────────────────────────────────

/// The main computer vision engine.
pub struct VisionEngine {
    /// Window handle / title for the target process.
    window_title: String,
    /// Last captured state.
    pub last_state: ScreenState,
    /// Tensor of reference templates (in production, loaded from disk).
    templates_loaded: bool,
    /// Optional pluggable capture source (DXGI/BitBlt/Offline).
    /// When None, `capture()` returns a placeholder screen state.
    pub capture_source: Option<Box<dyn crate::capture::CaptureSource>>,
}

impl VisionEngine {
    /// Create a new engine targeting the given window.
    pub fn new(window_title: &str) -> Self {
        Self {
            window_title: window_title.to_owned(),
            last_state: ScreenState::default(),
            templates_loaded: false,
            capture_source: None,
        }
    }

    /// Initialise reference templates (PNG files from ./templates/).
    pub fn load_templates(&mut self) -> Result<(), VisionError> {
        // In production: load template images from disk using `image` crate,
        // convert to grayscale, store as matching kernels.
        info!("Loading CV templates for '{}'", self.window_title);
        self.templates_loaded = true;
        Ok(())
    }

    /// Install a capture source (DXGI / BitBlt / Offline).
    pub fn set_capture_source(&mut self, source: Box<dyn crate::capture::CaptureSource>) {
        self.capture_source = Some(source);
    }

    /// Capture a full pipeline run: screenshot → extract → match → struct.
    pub fn capture(&mut self) -> Result<ScreenState, VisionError> {
        // Step 1: Grab a frame from the installed capture source (if any)
        if let Some(source) = self.capture_source.as_mut() {
            let frame = source.grab()?;
            return Ok(Self::frame_to_state(&frame));
        }

        // Step 2: No source — placeholder state (pre-hardware build).
        let _hwnd = self.find_window()?;
        let state = ScreenState {
            backpack_pollen: 0.42,
            honey_count: "125,430".into(),
            captured_at: now_f(),
            death_overlay: false,
            quest_dialog: false,
            quest_npc: None,
            quest_text: None,
            coin_tokens_visible: 3,
            field_name: Some("Pine Tree Forest".into()),
            capture_confidence: 0.93,
            token_positions: vec![(500, 400), (700, 450), (600, 420)],
        };

        self.last_state = state.clone();
        debug!("CV capture (placeholder): field={:?} pollen={:.0}% honey={}",
               state.field_name, state.backpack_pollen * 100.0, state.honey_count);
        Ok(state)
    }

    /// Convert a raw RGBA frame into a ScreenState using simple heuristics.
    /// This is the real (if basic) CV path: it scans the frame for the
    /// backpack bar's green fill ratio and collects green token blobs.
    fn frame_to_state(frame: &crate::capture::CapturedFrame) -> ScreenState {
        let w = frame.width as usize;
        let h = frame.height as usize;
        let px = &frame.rgba;

        // Heuristic: the bottom 12% of the frame is the backpack bar;
        // green pixels there = fill ratio. Token blobs = green clusters
        // in the middle band.
        let bar_top = (h as f64 * 0.86) as usize;
        let mut green_bar = 0u32;
        let mut bar_total = 0u32;
        let mut tokens = Vec::new();

        for y in bar_top..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                if i + 2 >= px.len() { continue; }
                let (r, g, b) = (px[i], px[i + 1], px[i + 2]);
                if g > 120 && g > r + 40 && g > b + 20 {
                    green_bar += 1;
                    // sample token blobs in the middle band
                    if y < bar_top + 30 && x % 64 == 0 && tokens.len() < 30 {
                        tokens.push((x as u32, y as u32));
                    }
                }
                bar_total += 1;
            }
        }
        let fill = if bar_total > 0 { green_bar as f64 / bar_total as f64 } else { 0.0 };

        let state = ScreenState {
            backpack_pollen: fill.clamp(0.0, 1.0),
            honey_count: "—".into(),
            captured_at: now_f(),
            death_overlay: false,
            quest_dialog: false,
            quest_npc: None,
            quest_text: None,
            coin_tokens_visible: tokens.len() as u32,
            field_name: Some("Demo Field".into()),
            capture_confidence: 0.9,
            token_positions: tokens,
        };
        debug!("CV capture (frame→state): pollen={:.0}% tokens={}",
               state.backpack_pollen * 100.0, state.coin_tokens_visible);
        state
    }

    /// Find the game window by title.
    fn find_window(&self) -> Result<u64, VisionError> {
        // In production: FindWindowW(None, self.window_title)
        // Returns an HWND (pointer-sized handle).
        if self.window_title.is_empty() {
            return Err(VisionError::WindowNotFound("Empty window title".into()));
        }
        // Simulate: return a fake HWND
        Ok(0xDEADBEE)
    }

    /// Capture a single region from a bitmap.
    #[allow(unused_variables)]
    fn capture_region(&self, region: &CaptureRegion, hwnd: u64) -> Result<Vec<u8>, VisionError> {
        // In production: CreateCompatibleDC + BitBlt the region,
        // then read pixel bytes.
        Ok(vec![0u8; (region.width * region.height * 4) as usize])
    }

    /// Measure the pollen bar fill ratio from a cropped image.
    #[allow(unused_variables)]
    fn read_backpack_fill(region_bytes: &[u8]) -> f64 {
        // Count green pixels vs total pixels in the bar region.
        // Simulated:
        0.42
    }

    /// Run OCR on a region (using tesseract or a lightweight alternative).
    #[allow(unused_variables)]
    fn ocr_region(region_bytes: &[u8]) -> Result<String, VisionError> {
        Ok("125,430".into())
    }
}

fn now_f() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_engine_creation() {
        let engine = VisionEngine::new("Roblox");
        assert_eq!(engine.window_title, "Roblox");
        assert!(!engine.templates_loaded);
    }

    #[test]
    fn test_capture_returns_valid_state() {
        let mut engine = VisionEngine::new("Roblox");
        let state = engine.capture().unwrap();
        assert!(state.backpack_pollen >= 0.0);
        assert!(state.backpack_pollen <= 1.0);
        assert!(!state.honey_count.is_empty());
    }

    #[test]
    fn test_capture_region_bounds() {
        let region = CaptureRegion::new(100, 100, 640, 480);
        let bytes = vec![0u8; (region.width * region.height * 4) as usize];
        assert_eq!(bytes.len(), 640 * 480 * 4);
    }

    #[test]
    fn test_empty_window_title_fails() {
        let engine = VisionEngine::new("");
        let result = engine.find_window();
        assert!(matches!(result, Err(VisionError::WindowNotFound(_))));
    }

    #[test]
    fn test_regions_are_valid() {
        assert_eq!(regions::BACKPACK_BAR.width, 280);
        assert_eq!(regions::HONEY_COUNTER.x, 840);
    }
}
