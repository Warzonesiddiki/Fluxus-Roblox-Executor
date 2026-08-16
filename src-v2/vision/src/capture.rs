/// # Capture Layer (A3.1) — Screen Capture Abstraction
///
/// A `CaptureSource` trait abstracts where frames come from:
/// - **DXGI Desktop Duplication** (production): low-latency 60 FPS capture
///   of the game window.
/// - **BitBlt** (fallback): GDI capture, slower but universally compatible.
/// - **Offline** (Model phase / tests): replays labeled frames from disk
///   so the pipeline can be tested deterministically without a game running.
///
/// The trait design lets the Model phase swap sources without touching
/// downstream logic.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::VisionError;

// ─── Captured frame ───────────────────────────────────────────────────────────

/// A raw captured frame (RGBA, row-major).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// RGBA bytes: width × height × 4.
    pub rgba: Vec<u8>,
    /// Capture timestamp (ms since epoch).
    pub timestamp_ms: u64,
}

impl CapturedFrame {
    /// Create a solid-color frame (useful for tests).
    pub fn solid(width: u32, height: u32, r: u8, g: u8, b: u8) -> Self {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            rgba.extend_from_slice(&[r, g, b, 255]);
        }
        Self { width, height, rgba, timestamp_ms: now_ms() }
    }

    /// Crop a region of the frame (bounds-checked).
    pub fn crop(&self, x: u32, y: u32, w: u32, h: u32) -> Result<CapturedFrame, VisionError> {
        if x + w > self.width || y + h > self.height {
            return Err(VisionError::CaptureFailed(
                format!("crop ({x},{y},{w}x{h}) outside frame {}x{}", self.width, self.height),
            ));
        }
        let mut out = Vec::with_capacity((w * h * 4) as usize);
        for row in y..(y + h) {
            let start = ((row * self.width + x) * 4) as usize;
            let end = start + (w * 4) as usize;
            out.extend_from_slice(&self.rgba[start..end]);
        }
        Ok(CapturedFrame { width: w, height: h, rgba: out, timestamp_ms: now_ms() })
    }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ─── Capture sources ──────────────────────────────────────────────────────────

/// Abstraction over any frame producer.
pub trait CaptureSource: Send {
    /// Grab the next frame. Should return quickly (< 16 ms in production).
    fn grab(&mut self) -> Result<CapturedFrame, VisionError>;

    /// Human-readable source name (for logs/debug).
    fn name(&self) -> &str;

    /// Current capture rate (frames per second), if known.
    fn nominal_fps(&self) -> u32 { 60 }
}

/// DXGI Desktop Duplication source (production path, Windows).
///
/// In production this uses:
/// - `D3D11CreateDevice` → `IDXGIOutputDuplication::AcquireNextFrame`
/// - Converts BGRA → RGBA
/// - `ReleaseFrame` each cycle
///
/// The struct holds the necessary handles; the implementation is gated
/// behind the `windows` dependency in the Architect phase.
pub struct DxgiDuplicationSource {
    pub window_title: String,
    pub region: Option<CaptureRect>,
    frames_captured: u64,
}

/// A simple integer rectangle for capture regions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CaptureRect {
    pub x: u32, pub y: u32, pub width: u32, pub height: u32,
}

impl DxgiDuplicationSource {
    pub fn new(window_title: &str) -> Self {
        Self { window_title: window_title.to_owned(), region: None, frames_captured: 0 }
    }

    pub fn with_region(mut self, rect: CaptureRect) -> Self {
        self.region = Some(rect);
        self
    }
}

impl CaptureSource for DxgiDuplicationSource {
    fn grab(&mut self) -> Result<CapturedFrame, VisionError> {
        // A3.1 production implementation:
        //   1. FindWindowW(self.window_title) → hwnd
        //   2. GetClientRect → size
        //   3. DXGI duplication AcquireNextFrame
        //   4. Map texture, copy pixels, convert BGRA→RGBA
        //   5. ReleaseFrame
        //
        // For now, return a placeholder so downstream code can run.
        self.frames_captured += 1;
        Ok(CapturedFrame::solid(1920, 1080, 30, 30, 30))
    }

    fn name(&self) -> &str { "dxgi" }
}

/// BitBlt GDI fallback source.
pub struct BitBltSource {
    pub window_title: String,
    frames_captured: u64,
}

impl BitBltSource {
    pub fn new(window_title: &str) -> Self {
        Self { window_title: window_title.to_owned(), frames_captured: 0 }
    }
}

impl CaptureSource for BitBltSource {
    fn grab(&mut self) -> Result<CapturedFrame, VisionError> {
        // Production: GetDC(hwnd) → CreateCompatibleDC → BitBlt into a
        // DIB section → copy bytes → ReleaseDC.
        self.frames_captured += 1;
        Ok(CapturedFrame::solid(1920, 1080, 40, 40, 40))
    }

    fn name(&self) -> &str { "bitblt" }
}

/// Offline source — replays pre-recorded frames (Model phase, tests, demos).
pub struct OfflineSource {
    frames: Vec<CapturedFrame>,
    cursor: usize,
}

impl OfflineSource {
    pub fn new(frames: Vec<CapturedFrame>) -> Self {
        Self { frames, cursor: 0 }
    }

    /// Build a synthetic sequence: N frames with a slowly filling bar.
    pub fn synthetic_bar_frames(n: usize, width: u32, height: u32) -> Self {
        let frames = (0..n)
            .map(|i| {
                let fill = (i as f64 / n as f64) as f32;
                let bar_w = (width as f32 * fill) as u32;
                let mut f = CapturedFrame::solid(width, height, 20, 20, 20);
                // Draw a green bar across the bottom (backpack bar proxy)
                let bar_y = height - 30;
                for row in bar_y..(bar_y + 15) {
                    for col in 0..bar_w {
                        let idx = ((row * width + col) * 4) as usize;
                        f.rgba[idx] = 60; f.rgba[idx + 1] = 220; f.rgba[idx + 2] = 90;
                    }
                }
                f
            })
            .collect();
        Self::new(frames)
    }
}

impl CaptureSource for OfflineSource {
    fn grab(&mut self) -> Result<CapturedFrame, VisionError> {
        if self.frames.is_empty() {
            return Err(VisionError::NoWindow);
        }
        let frame = self.frames[self.cursor % self.frames.len()].clone();
        self.cursor += 1;
        Ok(frame)
    }

    fn name(&self) -> &str { "offline" }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_crop() {
        let f = CapturedFrame::solid(100, 100, 10, 200, 10);
        let crop = f.crop(10, 10, 50, 50).unwrap();
        assert_eq!(crop.width, 50);
        assert_eq!(crop.height, 50);
        assert_eq!(crop.rgba.len(), 50 * 50 * 4);
        // Pixel value check
        assert_eq!(crop.rgba[0], 10);
        assert_eq!(crop.rgba[1], 200);
    }

    #[test]
    fn test_frame_crop_out_of_bounds() {
        let f = CapturedFrame::solid(100, 100, 0, 0, 0);
        assert!(f.crop(90, 90, 50, 50).is_err());
    }

    #[test]
    fn test_offline_source_replays_and_wraps() {
        let frames = vec![
            CapturedFrame::solid(10, 10, 255, 0, 0),
            CapturedFrame::solid(10, 10, 0, 255, 0),
        ];
        let mut src = OfflineSource::new(frames);
        assert_eq!(src.grab().unwrap().rgba[0], 255);
        assert_eq!(src.grab().unwrap().rgba[0], 0);
        // Wraps around
        assert_eq!(src.grab().unwrap().rgba[0], 255);
    }

    #[test]
    fn test_synthetic_bar_frames_fill() {
        let mut src = OfflineSource::synthetic_bar_frames(10, 200, 100);
        // First frame: nearly empty bar
        let first = src.grab().unwrap();
        // Last frame: nearly full bar (grab 10 times)
        let mut last = first;
        for _ in 0..10 { last = src.grab().unwrap(); }
        // Sample the bar row center — last frame should have green at right edge
        let bar_y = 100 - 30 + 7;
        let right_edge = ((bar_y * 200 + 190) * 4) as usize;
        assert!(last.rgba[right_edge + 1] > 150, "bar not filled: {}", last.rgba[right_edge + 1]);
    }

    #[test]
    fn test_dxgi_and_bitblt_names() {
        let mut dx = DxgiDuplicationSource::new("Roblox");
        let mut bb = BitBltSource::new("Roblox");
        assert_eq!(dx.name(), "dxgi");
        assert_eq!(bb.name(), "bitblt");
        // Both can grab (placeholder path)
        assert!(dx.grab().is_ok());
        assert!(bb.grab().is_ok());
    }
}
