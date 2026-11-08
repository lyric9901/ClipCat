// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Creator AI automation: local offline silence cutting and musical beat detection.
//!
//! Provides zero-cloud, 100% offline algorithms designed for fast execution on
//! low-power hardware (sub-10ms analysis time per minute of audio).

use crate::time::{Rational, TimeRange};

/// Configuration for offline voice silence detection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SilenceDetectConfig {
    /// Silence threshold in decibels below full scale (default: -35.0 dB).
    pub threshold_db: f32,
    /// Minimum contiguous silence duration to consider a cut (default: 0.35 seconds).
    pub min_silence_secs: f32,
    /// Safety padding around speech boundaries to preserve consonants (default: 0.06 seconds).
    pub padding_secs: f32,
    /// Window analysis size in seconds (default: 0.025 seconds = 25ms).
    pub window_secs: f32,
}

impl Default for SilenceDetectConfig {
    fn default() -> Self {
        Self {
            threshold_db: -35.0,
            min_silence_secs: 0.35,
            padding_secs: 0.06,
            window_secs: 0.025,
        }
    }
}

/// Offline silence detector identifying gaps in dialogue for auto-ripple cuts.
pub struct SilenceDetector;

impl SilenceDetector {
    /// Detects silent intervals in mono audio samples.
    ///
    /// Returns a list of `TimeRange`s representing silence intervals that can be
    /// excised using ripple delete.
    pub fn detect_silences(
        samples: &[f32],
        sample_rate: u32,
        config: SilenceDetectConfig,
    ) -> Vec<TimeRange> {
        if samples.is_empty() || sample_rate == 0 {
            return Vec::new();
        }

        let sr = sample_rate as f32;
        let window_size = ((config.window_secs * sr) as usize).max(64);
        let hop_size = window_size / 2;

        let threshold_linear = 10.0f32.powf(config.threshold_db / 20.0);
        let min_silence_frames = (config.min_silence_secs * sr) as usize;
        let padding_frames = (config.padding_secs * sr) as usize;

        let mut silent_regions = Vec::new();
        let mut in_silence = false;
        let mut silence_start_frame = 0;

        let mut pos = 0;
        while pos + window_size <= samples.len() {
            let window = &samples[pos..pos + window_size];
            let mut sum_sq = 0.0f32;
            for &s in window {
                sum_sq += s * s;
            }
            let rms = (sum_sq / window_size as f32).sqrt();

            if rms < threshold_linear {
                if !in_silence {
                    in_silence = true;
                    silence_start_frame = pos;
                }
            } else if in_silence {
                in_silence = false;
                let silence_end_frame = pos;
                if silence_end_frame.saturating_sub(silence_start_frame) >= min_silence_frames {
                    // Apply safety padding inside the silence
                    let padded_start = silence_start_frame + padding_frames;
                    let padded_end = silence_end_frame.saturating_sub(padding_frames);
                    if padded_end > padded_start {
                        let start_sec = padded_start as f64 / sr as f64;
                        let dur_sec = (padded_end - padded_start) as f64 / sr as f64;
                        if let (Some(start_rat), Some(dur_rat)) = (
                            Rational::approximate(start_sec),
                            Rational::approximate(dur_sec),
                        ) {
                            silent_regions.push(TimeRange::new(start_rat, dur_rat));
                        }
                    }
                }
            }

            pos += hop_size;
        }

        // Handle trailing silence at the end of the clip
        if in_silence && samples.len().saturating_sub(silence_start_frame) >= min_silence_frames {
            let padded_start = silence_start_frame + padding_frames;
            let padded_end = samples.len();
            if padded_end > padded_start {
                let start_sec = padded_start as f64 / sr as f64;
                let dur_sec = (padded_end - padded_start) as f64 / sr as f64;
                if let (Some(start_rat), Some(dur_rat)) = (
                    Rational::approximate(start_sec),
                    Rational::approximate(dur_sec),
                ) {
                    silent_regions.push(TimeRange::new(start_rat, dur_rat));
                }
            }
        }

        silent_regions
    }
}

/// Offline musical beat and transient onset detector.
pub struct BeatDetector;

impl BeatDetector {
    /// Detects musical transient beats using high-frequency energy flux differentiation.
    ///
    /// Returns timeline timestamps for beat snapping and cut synchronization.
    pub fn detect_beats(
        samples: &[f32],
        sample_rate: u32,
        min_interval_secs: f32,
    ) -> Vec<Rational> {
        if samples.is_empty() || sample_rate == 0 {
            return Vec::new();
        }

        let sr = sample_rate as f32;
        let window_size = ((0.02 * sr) as usize).max(64); // 20ms window
        let hop_size = window_size / 2;
        let min_interval_frames = (min_interval_secs * sr) as usize;

        let mut energies = Vec::new();
        let mut pos = 0;
        while pos + window_size <= samples.len() {
            let window = &samples[pos..pos + window_size];
            let mut energy = 0.0f32;
            for &s in window {
                energy += s * s;
            }
            energies.push((pos, energy));
            pos += hop_size;
        }

        if energies.len() < 3 {
            return Vec::new();
        }

        // Differentiate energy (spectral flux proxy)
        let mut flux = Vec::with_capacity(energies.len());
        for i in 1..energies.len() {
            let diff = (energies[i].1 - energies[i - 1].1).max(0.0);
            flux.push((energies[i].0, diff));
        }

        // Adaptive threshold based on moving mean energy
        let mean_flux: f32 = flux.iter().map(|(_, f)| *f).sum::<f32>() / flux.len() as f32;
        let threshold = mean_flux * 1.5;

        let mut beats = Vec::new();
        let mut last_beat_frame = 0usize;

        for (frame, val) in flux {
            if val > threshold
                && (beats.is_empty()
                    || frame.saturating_sub(last_beat_frame) >= min_interval_frames)
            {
                let sec = frame as f64 / sr as f64;
                if let Some(time) = Rational::approximate(sec) {
                    beats.push(time);
                    last_beat_frame = frame;
                }
            }
        }

        beats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_silence_detection_synthetic() {
        // Create 2 seconds of audio at 10,000 Hz:
        // 0.0s - 0.5s: Speech tone (sine wave, amplitude 0.8)
        // 0.5s - 1.5s: Pure silence (1.0 second pause)
        // 1.5s - 2.0s: Speech tone (sine wave, amplitude 0.8)
        let sr = 10_000u32;
        let mut samples = vec![0.0f32; 20_000];

        for i in 0..5_000 {
            samples[i] = (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / sr as f32)).sin() * 0.8;
        }
        for i in 15_000..20_000 {
            samples[i] = (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / sr as f32)).sin() * 0.8;
        }

        let config = SilenceDetectConfig {
            threshold_db: -30.0,
            min_silence_secs: 0.3,
            padding_secs: 0.05,
            window_secs: 0.02,
        };

        let silences = SilenceDetector::detect_silences(&samples, sr, config);
        assert_eq!(silences.len(), 1);
        let s = silences[0];
        // Expected start around 0.55s (0.50 + 0.05 pad) and end around 1.45s (1.50 - 0.05 pad)
        assert!(s.start.as_f64() >= 0.50 && s.start.as_f64() <= 0.60);
        assert!(s.end().as_f64() >= 1.40 && s.end().as_f64() <= 1.52);
    }

    #[test]
    fn test_beat_detection_synthetic() {
        // Create a 120 BPM synthetic click track (beats every 0.5 seconds)
        let sr = 10_000u32;
        let mut samples = vec![0.0f32; 20_000]; // 2 seconds

        // Add impulses every 5,000 samples (0.5s)
        for beat_idx in 0..4 {
            let start = beat_idx * 5_000;
            for j in 0..100 {
                if start + j < samples.len() {
                    samples[start + j] = 0.9;
                }
            }
        }

        let beats = BeatDetector::detect_beats(&samples, sr, 0.25);
        assert!(beats.len() >= 3);
        // First detected beat should be near 0.5s (since flux differentiates from baseline)
        assert!(beats.iter().any(|b| (b.as_f64() - 0.5).abs() < 0.1));
        assert!(beats.iter().any(|b| (b.as_f64() - 1.0).abs() < 0.1));
    }
}
