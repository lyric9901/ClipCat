// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Real-time video scopes analyzing frame pixel buffers.
//!
//! Provides Histogram, Waveform (Luma), RGB Parade, and Vectorscope (polar Cb/Cr)
//! calculations for color grading and signal monitoring, matching professional
//! NLE specifications (ITU-R BT.709, SMPTE 170M).
//!
//! Designed for zero heap allocations in the hot path: [`VideoScopesAnalyzer`] owns
//! pre-allocated density grids and processes frames with an adaptive subsampling stride
//! to guarantee under-1ms analysis on integrated GPUs and dual-core CPUs.

use crate::frame::Frame;

/// Number of discrete bins in histogram and vertical waveform levels.
pub const SCOPE_LEVELS: usize = 256;

/// Default horizontal columns in waveform and vectorscope resolution.
pub const SCOPE_GRID_SIZE: usize = 256;

/// Target maximum pixel sample count per frame analysis to preserve 60 FPS.
pub const DEFAULT_MAX_SAMPLES: usize = 120_000;

/// Histogram distribution across Red, Green, Blue, and BT.709 Luminance channels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistogramData {
    /// Red channel distribution from 0 to 255.
    pub red: [u32; SCOPE_LEVELS],
    /// Green channel distribution from 0 to 255.
    pub green: [u32; SCOPE_LEVELS],
    /// Blue channel distribution from 0 to 255.
    pub blue: [u32; SCOPE_LEVELS],
    /// ITU-R BT.709 Luminance distribution from 0 to 255.
    pub luma: [u32; SCOPE_LEVELS],
    /// Count of samples clipped at pure black (level 0).
    pub black_clipping: u32,
    /// Count of samples clipped at peak white (level 255).
    pub white_clipping: u32,
    /// Total sampled pixels.
    pub total_samples: u32,
}

impl Default for HistogramData {
    fn default() -> Self {
        Self {
            red: [0; SCOPE_LEVELS],
            green: [0; SCOPE_LEVELS],
            blue: [0; SCOPE_LEVELS],
            luma: [0; SCOPE_LEVELS],
            black_clipping: 0,
            white_clipping: 0,
            total_samples: 0,
        }
    }
}

impl HistogramData {
    /// Clears all bins and counters for the next analysis pass.
    pub fn reset(&mut self) {
        self.red.fill(0);
        self.green.fill(0);
        self.blue.fill(0);
        self.luma.fill(0);
        self.black_clipping = 0;
        self.white_clipping = 0;
        self.total_samples = 0;
    }

    /// Normalized luminance frequencies in [0.0, 1.0].
    pub fn normalized_luma(&self) -> [f32; SCOPE_LEVELS] {
        let max = self.luma.iter().copied().max().unwrap_or(0).max(1) as f32;
        let mut out = [0.0f32; SCOPE_LEVELS];
        for (i, &count) in self.luma.iter().enumerate() {
            out[i] = count as f32 / max;
        }
        out
    }

    /// Peak count across all color and luma channels.
    pub fn peak_count(&self) -> u32 {
        let max_r = self.red.iter().copied().max().unwrap_or(0);
        let max_g = self.green.iter().copied().max().unwrap_or(0);
        let max_b = self.blue.iter().copied().max().unwrap_or(0);
        let max_y = self.luma.iter().copied().max().unwrap_or(0);
        max_r.max(max_g).max(max_b).max(max_y)
    }

    /// Converts a 256-level channel histogram to SVG path commands in [0.0, 1.0] unit box.
    pub fn channel_to_svg_path(bins: &[u32; SCOPE_LEVELS], peak: u32) -> String {
        use std::fmt::Write;
        if peak == 0 {
            return "M 0 1 L 1 1".to_string();
        }
        let mut path = String::with_capacity(SCOPE_LEVELS * 14);
        let _ = write!(path, "M 0 1");
        let peak_f = peak as f32;
        for (i, &count) in bins.iter().enumerate() {
            let x = i as f32 / 255.0;
            let y = 1.0 - (count as f32 / peak_f).clamp(0.0, 1.0);
            let _ = write!(path, " L {x:.3} {y:.3}");
        }
        let _ = write!(path, " L 1 1");
        path
    }
}

/// 2D luminance density grid representing horizontal pixel columns vs. signal level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaveformData {
    /// Horizontal columns across the image width (typically 256).
    pub columns: usize,
    /// Vertical signal levels (256, from 0 to 255).
    pub rows: usize,
    /// Flat density buffer of size `columns * rows`.
    /// Indexed by `row * columns + col`.
    pub density: Vec<u16>,
    /// Maximum density recorded in any cell.
    pub max_density: u16,
}

impl WaveformData {
    /// Creates a new waveform grid with specified dimensions.
    pub fn new(columns: usize, rows: usize) -> Self {
        Self {
            columns,
            rows,
            density: vec![0; columns * rows],
            max_density: 0,
        }
    }

    /// Clears the waveform density grid.
    pub fn reset(&mut self) {
        self.density.fill(0);
        self.max_density = 0;
    }

    /// Records a sample at column and level.
    #[inline]
    pub fn record(&mut self, col: usize, level: u8) {
        let col = col.min(self.columns.saturating_sub(1));
        let row = level as usize;
        let idx = row * self.columns + col;
        if let Some(val) = self.density.get_mut(idx) {
            *val = val.saturating_add(1);
            if *val > self.max_density {
                self.max_density = *val;
            }
        }
    }

    /// Gets density value at specified column and level (0..=255).
    pub fn get(&self, col: usize, level: u8) -> u16 {
        let col = col.min(self.columns.saturating_sub(1));
        let row = level as usize;
        let idx = row * self.columns + col;
        self.density.get(idx).copied().unwrap_or(0)
    }

    /// Converts waveform envelope into an SVG path in [0.0, 1.0] unit box.
    pub fn to_svg_envelope(&self) -> String {
        use std::fmt::Write;
        if self.columns == 0 {
            return String::new();
        }
        let mut top_points = Vec::with_capacity(self.columns);
        let mut btm_points = Vec::with_capacity(self.columns);

        for col in 0..self.columns {
            let mut min_level = None;
            let mut max_level = None;
            for row in 0..self.rows {
                let count = self.density[row * self.columns + col];
                if count > 0 {
                    if min_level.is_none() {
                        min_level = Some(row);
                    }
                    max_level = Some(row);
                }
            }
            if let (Some(min_l), Some(max_l)) = (min_level, max_level) {
                // Invert vertical since SVG y=0 is top (100% white) and y=1 is bottom (0% black)
                let y_top = 1.0 - (max_l as f32 / 255.0);
                let y_btm = 1.0 - (min_l as f32 / 255.0);
                top_points.push((col, y_top));
                btm_points.push((col, y_btm));
            }
        }

        if top_points.is_empty() {
            return "M 0 1 L 1 1".to_string();
        }

        let mut path = String::with_capacity(self.columns * 16);
        let col_max = (self.columns - 1).max(1) as f32;
        let (first_col, first_y) = top_points[0];
        let _ = write!(path, "M {:.3} {:.3}", first_col as f32 / col_max, first_y);
        for &(col, y) in &top_points[1..] {
            let _ = write!(path, " L {:.3} {:.3}", col as f32 / col_max, y);
        }
        for &(col, y) in btm_points.iter().rev() {
            let _ = write!(path, " L {:.3} {:.3}", col as f32 / col_max, y);
        }
        let _ = write!(path, " Z");
        path
    }
}

impl Default for WaveformData {
    fn default() -> Self {
        Self::new(SCOPE_GRID_SIZE, SCOPE_LEVELS)
    }
}

/// RGB Parade containing isolated Red, Green, and Blue waveforms.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RgbParadeData {
    /// Red channel column density.
    pub red: WaveformData,
    /// Green channel column density.
    pub green: WaveformData,
    /// Blue channel column density.
    pub blue: WaveformData,
}

impl RgbParadeData {
    /// Clears red, green, and blue waveforms.
    pub fn reset(&mut self) {
        self.red.reset();
        self.green.reset();
        self.blue.reset();
    }
}

/// Target graticule box for 75% SMPTE color bars on a vectorscope.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorBarTarget {
    /// Color name identifier.
    pub name: &'static str,
    /// Cb (U) horizontal graticule center in [0..255].
    pub cb: u8,
    /// Cr (V) vertical graticule center in [0..255].
    pub cr: u8,
    /// Tolerance box half-width in units.
    pub box_radius: u8,
}

/// Polar chrominance density plot in Cb/Cr color space.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VectorscopeData {
    /// Grid width and height (typically 256x256).
    pub size: usize,
    /// 2D density buffer of size `size * size`.
    /// Indexed by `y * size + x`.
    pub density: Vec<u16>,
    /// Maximum density recorded in any cell.
    pub max_density: u16,
}

impl VectorscopeData {
    /// Creates a new vectorscope density grid.
    pub fn new(size: usize) -> Self {
        Self {
            size,
            density: vec![0; size * size],
            max_density: 0,
        }
    }

    /// Clears the vectorscope density buffer.
    pub fn reset(&mut self) {
        self.density.fill(0);
        self.max_density = 0;
    }

    /// Records a chrominance point with Cb (horizontal) and Cr (vertical).
    #[inline]
    pub fn record(&mut self, cb: u8, cr: u8) {
        let x = (cb as usize).min(self.size.saturating_sub(1));
        // Invert Y so positive Cr points upwards
        let y = (255usize.saturating_sub(cr as usize)).min(self.size.saturating_sub(1));
        let idx = y * self.size + x;
        if let Some(val) = self.density.get_mut(idx) {
            *val = val.saturating_add(1);
            if *val > self.max_density {
                self.max_density = *val;
            }
        }
    }

    /// Standard 75% color bar targets (ITU-R BT.709) for graticule calibration.
    pub fn color_bar_targets() -> [ColorBarTarget; 6] {
        [
            ColorBarTarget {
                name: "R",
                cb: 107,
                cr: 223,
                box_radius: 8,
            },
            ColorBarTarget {
                name: "Mg",
                cb: 201,
                cr: 214,
                box_radius: 8,
            },
            ColorBarTarget {
                name: "B",
                cb: 223,
                cr: 120,
                box_radius: 8,
            },
            ColorBarTarget {
                name: "Cy",
                cb: 149,
                cr: 33,
                box_radius: 8,
            },
            ColorBarTarget {
                name: "G",
                cb: 55,
                cr: 42,
                box_radius: 8,
            },
            ColorBarTarget {
                name: "Yl",
                cb: 33,
                cr: 136,
                box_radius: 8,
            },
        ]
    }

    /// Standard skin tone indicator angle in degrees (~123° in Cb/Cr plane).
    pub const fn skin_tone_angle_deg() -> f32 {
        123.0
    }

    /// Converts vectorscope density samples into SVG scatter commands in [0.0, 1.0] unit box.
    pub fn to_svg_scatter(&self, max_points: usize) -> String {
        use std::fmt::Write;
        let mut path = String::with_capacity(max_points * 16);
        let threshold = (self.max_density / 10).max(1);
        let mut count = 0;
        let sz = self.size as f32;
        for y in 0..self.size {
            for x in 0..self.size {
                let d = self.density[y * self.size + x];
                if d >= threshold {
                    let nx = x as f32 / sz;
                    let ny = y as f32 / sz;
                    let _ = write!(path, "M {nx:.3} {ny:.3} l 0.005 0 ");
                    count += 1;
                    if count >= max_points {
                        return path;
                    }
                }
            }
        }
        path
    }
}

impl Default for VectorscopeData {
    fn default() -> Self {
        Self::new(SCOPE_GRID_SIZE)
    }
}

/// Zero-allocation analyzer maintaining reusable scope buffers.
#[derive(Clone, Debug, Default)]
pub struct VideoScopesAnalyzer {
    /// Cached histogram metrics.
    pub histogram: HistogramData,
    /// Cached luma waveform density grid.
    pub waveform: WaveformData,
    /// Cached RGB parade waveform grids.
    pub parade: RgbParadeData,
    /// Cached vectorscope chrominance density grid.
    pub vectorscope: VectorscopeData,
}

impl VideoScopesAnalyzer {
    /// Creates a new analyzer with pre-allocated grids.
    pub fn new() -> Self {
        Self {
            histogram: HistogramData::default(),
            waveform: WaveformData::default(),
            parade: RgbParadeData::default(),
            vectorscope: VectorscopeData::default(),
        }
    }

    /// Resets all buffers in preparation for the next frame.
    pub fn reset(&mut self) {
        self.histogram.reset();
        self.waveform.reset();
        self.parade.reset();
        self.vectorscope.reset();
    }

    /// Analyzes an 8-bit RGBA frame and populates all scope datasets.
    ///
    /// Subsamples with an adaptive stride to keep total processed samples around
    /// `max_samples` (default: [`DEFAULT_MAX_SAMPLES`]), guaranteeing smooth 60 FPS
    /// operation on low-power hardware.
    pub fn analyze_frame(&mut self, frame: &Frame, max_samples: usize) {
        self.reset();

        let width = frame.width() as usize;
        let height = frame.height() as usize;
        if width == 0 || height == 0 {
            return;
        }

        let total_pixels = width * height;
        let target_samples = max_samples.max(1_000);
        let stride = (total_pixels / target_samples).max(1);

        let pixels = frame.pixels();
        let bytes_per_pixel = 4;

        let wf_cols = self.waveform.columns;
        let mut sampled_count = 0u32;

        for (pixel_idx, chunk) in pixels
            .chunks_exact(bytes_per_pixel)
            .step_by(stride)
            .enumerate()
        {
            let actual_idx = pixel_idx * stride;
            let x = actual_idx % width;

            let r = chunk[0];
            let g = chunk[1];
            let b = chunk[2];

            // ITU-R BT.709 integer arithmetic:
            // Y  = (54 * R + 183 * G + 19 * B) >> 8
            // Cb = ((-29 * R - 99 * G + 128 * B) >> 8) + 128
            // Cr = ((128 * R - 116 * G - 12 * B) >> 8) + 128
            let r_i32 = r as i32;
            let g_i32 = g as i32;
            let b_i32 = b as i32;

            let y_raw = (54 * r_i32 + 183 * g_i32 + 19 * b_i32) >> 8;
            let y = y_raw.clamp(0, 255) as u8;

            let cb_raw = ((-29 * r_i32 - 99 * g_i32 + 128 * b_i32) >> 8) + 128;
            let cb = cb_raw.clamp(0, 255) as u8;

            let cr_raw = ((128 * r_i32 - 116 * g_i32 - 12 * b_i32) >> 8) + 128;
            let cr = cr_raw.clamp(0, 255) as u8;

            // 1. Histogram
            self.histogram.red[r as usize] += 1;
            self.histogram.green[g as usize] += 1;
            self.histogram.blue[b as usize] += 1;
            self.histogram.luma[y as usize] += 1;

            if y == 0 {
                self.histogram.black_clipping += 1;
            } else if y == 255 {
                self.histogram.white_clipping += 1;
            }

            // 2. Waveform & Parade horizontal column mapping
            let col = (x * wf_cols) / width;
            self.waveform.record(col, y);
            self.parade.red.record(col, r);
            self.parade.green.record(col, g);
            self.parade.blue.record(col, b);

            // 3. Vectorscope
            self.vectorscope.record(cb, cr);

            sampled_count += 1;
        }

        self.histogram.total_samples = sampled_count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_black_frame_histogram_and_clipping() {
        let frame = Frame::black(64, 64);
        let mut analyzer = VideoScopesAnalyzer::new();
        analyzer.analyze_frame(&frame, 10_000);

        assert_eq!(analyzer.histogram.total_samples, 64 * 64);
        assert_eq!(analyzer.histogram.black_clipping, 64 * 64);
        assert_eq!(analyzer.histogram.white_clipping, 0);
        assert_eq!(analyzer.histogram.luma[0], 64 * 64);
    }

    #[test]
    fn test_pure_white_frame_clipping() {
        let pixels = vec![255u8; 64 * 64 * 4];
        let frame = Frame::from_rgba(64, 64, pixels).expect("frame");
        let mut analyzer = VideoScopesAnalyzer::new();
        analyzer.analyze_frame(&frame, 10_000);

        assert_eq!(analyzer.histogram.white_clipping, 64 * 64);
        assert_eq!(analyzer.histogram.black_clipping, 0);
        assert_eq!(analyzer.histogram.red[255], 64 * 64);
        assert_eq!(analyzer.histogram.luma[255], 64 * 64);
    }

    #[test]
    fn test_waveform_density_and_record() {
        let mut wf = WaveformData::new(10, 256);
        wf.record(5, 128);
        wf.record(5, 128);
        assert_eq!(wf.get(5, 128), 2);
        assert_eq!(wf.max_density, 2);
        assert_eq!(wf.get(0, 0), 0);
    }

    #[test]
    fn test_color_bar_targets_vectorscope() {
        let targets = VectorscopeData::color_bar_targets();
        assert_eq!(targets.len(), 6);
        assert_eq!(targets[0].name, "R");
        assert_eq!(targets[0].cb, 107);
        assert_eq!(targets[0].cr, 223);
        assert_eq!(VectorscopeData::skin_tone_angle_deg(), 123.0);
    }

    #[test]
    fn test_histogram_and_waveform_svg_path_generation() {
        let mut bins = [0u32; SCOPE_LEVELS];
        bins[128] = 50;
        let path = HistogramData::channel_to_svg_path(&bins, 100);
        assert!(path.starts_with("M 0 1"));
        assert!(path.contains("L 0.502 0.500"));

        let mut wf = WaveformData::new(4, 256);
        wf.record(0, 100);
        wf.record(1, 150);
        let wf_svg = wf.to_svg_envelope();
        assert!(wf_svg.starts_with("M"));
        assert!(wf_svg.ends_with("Z"));
    }
}
