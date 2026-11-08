// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Mathematical and DSP models for professional DAW audio mixing.
//!
//! Provides track routing, equal-power pan law (-3dB), volume dB conversions,
//! ITU-R BS.1770-4 K-weighting filters for LUFS loudness metering, and
//! automated dialogue ducking envelopes.

use std::f32::consts::FRAC_PI_4;

/// Reference sample rate for broadcast and video audio in Concat.
pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;

/// Unity gain level in decibels (0 dB = 1.0 linear amplitude multiplier).
pub const UNITY_GAIN_DB: f32 = 0.0;

/// Silence floor in decibels for metering and fader minimums (-60 dB).
pub const SILENCE_FLOOR_DB: f32 = -60.0;

/// Absolute threshold for ITU-R BS.1770-4 gated loudness measurement (-70 LKFS).
pub const LUFS_ABSOLUTE_THRESHOLD: f32 = -70.0;

/// Converts decibels into linear amplitude multiplier.
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    if db <= SILENCE_FLOOR_DB {
        0.0
    } else {
        10.0f32.powf(db / 20.0)
    }
}

/// Converts linear amplitude multiplier into decibels.
#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.000001 {
        SILENCE_FLOOR_DB
    } else {
        (20.0 * linear.log10()).max(SILENCE_FLOOR_DB)
    }
}

/// Stereo pan law formulas for placing mono/stereo signals across the stereo field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanLaw {
    /// Equal-power pan law (-3dB center attenuation).
    /// Constant perceived acoustic energy across the entire panning arc.
    #[default]
    EqualPower,
    /// Linear pan law (-6dB center attenuation).
    Linear,
}

impl PanLaw {
    /// Calculates left and right channel gain multipliers for a pan value in `[-1.0, 1.0]`.
    /// - `pan = -1.0`: Hard Left (L=1.0, R=0.0)
    /// - `pan =  0.0`: Center    (L=0.7071, R=0.7071 for EqualPower)
    /// - `pan = +1.0`: Hard Right (L=0.0, R=1.0)
    pub fn gains(self, pan: f32) -> (f32, f32) {
        let p = pan.clamp(-1.0, 1.0);
        match self {
            Self::EqualPower => {
                // Angle from 0 (hard left) to pi/2 (hard right)
                // When p = 0, angle = pi/4
                let angle = (p + 1.0) * FRAC_PI_4;
                let left = angle.cos();
                let right = angle.sin();
                (left, right)
            }
            Self::Linear => {
                let left = (1.0 - p) * 0.5;
                let right = (1.0 + p) * 0.5;
                (left, right)
            }
        }
    }
}

/// State and mixer controls for a single audio track channel.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackMixerChannel {
    /// Track identification label.
    pub name: String,
    /// Volume fader in decibels (-60.0 dB to +12.0 dB).
    pub volume_db: f32,
    /// Stereo pan position from -1.0 (left) to +1.0 (right).
    pub pan: f32,
    /// Whether the track is muted.
    pub mute: bool,
    /// Whether the track is soloed.
    pub solo: bool,
    /// True if this track contains dialogue/voiceover that ducks music tracks.
    pub ducking_source: bool,
    /// True if this track should duck when dialogue is present.
    pub ducking_target: bool,
    /// Peak linear meter for left channel.
    pub peak_l: f32,
    /// Peak linear meter for right channel.
    pub peak_r: f32,
}

impl TrackMixerChannel {
    /// Creates a new audio channel with unity gain and centered pan.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            volume_db: UNITY_GAIN_DB,
            pan: 0.0,
            mute: false,
            solo: false,
            ducking_source: false,
            ducking_target: false,
            peak_l: 0.0,
            peak_r: 0.0,
        }
    }

    /// Effective linear volume multiplier taking mute into account.
    pub fn effective_gain(&self) -> f32 {
        if self.mute {
            0.0
        } else {
            db_to_linear(self.volume_db)
        }
    }

    /// Returns the stereo (left, right) gain multipliers incorporating volume and pan law.
    pub fn stereo_gains(&self, pan_law: PanLaw) -> (f32, f32) {
        let master_gain = self.effective_gain();
        let (pan_l, pan_r) = pan_law.gains(self.pan);
        (master_gain * pan_l, master_gain * pan_r)
    }
}

/// Master output bus channel with loudness metering.
#[derive(Clone, Debug, PartialEq)]
pub struct MasterBus {
    /// Master fader in decibels (-60.0 dB to +12.0 dB).
    pub volume_db: f32,
    /// Peak linear meter for left channel.
    pub peak_l: f32,
    /// Peak linear meter for right channel.
    pub peak_r: f32,
    /// Integrated loudness in LUFS according to ITU-R BS.1770-4.
    pub lufs_integrated: f32,
    /// Short-term (3-second window) loudness in LUFS.
    pub lufs_short_term: f32,
}

impl Default for MasterBus {
    fn default() -> Self {
        Self {
            volume_db: UNITY_GAIN_DB,
            peak_l: 0.0,
            peak_r: 0.0,
            lufs_integrated: SILENCE_FLOOR_DB,
            lufs_short_term: SILENCE_FLOOR_DB,
        }
    }
}

/// Direct Form II Biquad filter for K-weighting DSP.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BiquadFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    w1: f32,
    w2: f32,
}

impl BiquadFilter {
    /// Creates a biquad filter with normalized coefficients.
    pub const fn new(b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0,
            b1,
            b2,
            a1,
            a2,
            w1: 0.0,
            w2: 0.0,
        }
    }

    /// Resets internal filter state delay line.
    pub fn reset(&mut self) {
        self.w1 = 0.0;
        self.w2 = 0.0;
    }

    /// Processes a single audio sample through Direct Form II transposed structure.
    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let w = input - self.a1 * self.w1 - self.a2 * self.w2;
        let output = self.b0 * w + self.b1 * self.w1 + self.b2 * self.w2;
        self.w2 = self.w1;
        self.w1 = w;
        output
    }
}

/// ITU-R BS.1770-4 K-weighting filter pair (Stage 1 High Shelf + Stage 2 High Pass).
#[derive(Clone, Debug, Default)]
pub struct KWeightingFilter {
    stage1_l: BiquadFilter,
    stage1_r: BiquadFilter,
    stage2_l: BiquadFilter,
    stage2_r: BiquadFilter,
}

impl KWeightingFilter {
    /// Creates K-weighting filters calibrated for 48,000 Hz.
    ///
    /// Coefficients specified in ITU-R BS.1770-4, Table 1:
    /// Stage 1 (High Shelving filter, ~+4dB @ 1.5kHz):
    ///   b0 = 1.53512485958697, b1 = -2.69169618940638, b2 = 1.19839281085285
    ///   a1 = -1.69065929318241, a2 = 0.73248077421585
    /// Stage 2 (High Pass filter, ~100Hz):
    ///   b0 = 1.0, b1 = -2.0, b2 = 1.0
    ///   a1 = -1.99004745483398, a2 = 0.99007225036621
    pub fn for_48k() -> Self {
        let s1 = BiquadFilter::new(
            1.535_125,
            -2.691_696_2,
            1.198_392_8,
            -1.690_659_3,
            0.732_480_76,
        );
        let s2 = BiquadFilter::new(1.0, -2.0, 1.0, -1.990_047_5, 0.990_072_25);
        Self {
            stage1_l: s1,
            stage1_r: s1,
            stage2_l: s2,
            stage2_r: s2,
        }
    }

    /// Resets all delay buffers.
    pub fn reset(&mut self) {
        self.stage1_l.reset();
        self.stage1_r.reset();
        self.stage2_l.reset();
        self.stage2_r.reset();
    }

    /// Processes stereo interleaved samples `[L, R, L, R, ...]` and returns mean squared energy.
    pub fn process_stereo_energy(&mut self, stereo_samples: &[f32]) -> (f32, f32) {
        if stereo_samples.is_empty() {
            return (0.0, 0.0);
        }
        let mut sum_sq_l = 0.0f32;
        let mut sum_sq_r = 0.0f32;
        let frames = stereo_samples.len() / 2;

        for chunk in stereo_samples.chunks_exact(2) {
            let filtered_l = self
                .stage2_l
                .process_sample(self.stage1_l.process_sample(chunk[0]));
            let filtered_r = self
                .stage2_r
                .process_sample(self.stage1_r.process_sample(chunk[1]));
            sum_sq_l += filtered_l * filtered_l;
            sum_sq_r += filtered_r * filtered_r;
        }

        let mean_sq_l = sum_sq_l / (frames as f32).max(1.0);
        let mean_sq_r = sum_sq_r / (frames as f32).max(1.0);
        (mean_sq_l, mean_sq_r)
    }

    /// Computes LUFS loudness from mean square energies (L and R weighted 1.0 each).
    pub fn lufs_from_energy(mean_sq_l: f32, mean_sq_r: f32) -> f32 {
        let total = mean_sq_l + mean_sq_r;
        if total <= 1e-10 {
            -70.0
        } else {
            -0.691 + 10.0 * total.log10()
        }
    }
}

/// Automated dialogue ducking envelope generator.
///
/// Smoothly attenuates background music when speech is detected on dialogue tracks.
#[derive(Clone, Debug, PartialEq)]
pub struct DuckingEnvelope {
    /// Depth of attenuation in decibels (e.g. -12.0 dB).
    pub depth_db: f32,
    /// Attack rate per sample (ramp-down speed, e.g. 20ms).
    pub attack_coeff: f32,
    /// Release rate per sample (ramp-up speed, e.g. 300ms).
    pub release_coeff: f32,
    /// Current calculated linear multiplier in `[0.0, 1.0]`.
    pub current_gain: f32,
}

impl DuckingEnvelope {
    /// Creates a ducking envelope with specified depth (dB), attack (seconds), and release (seconds).
    pub fn new(depth_db: f32, attack_secs: f32, release_secs: f32, sample_rate: u32) -> Self {
        let sr = sample_rate as f32;
        let attack_coeff = (-1.0 / (attack_secs * sr).max(1.0)).exp();
        let release_coeff = (-1.0 / (release_secs * sr).max(1.0)).exp();

        Self {
            depth_db,
            attack_coeff,
            release_coeff,
            current_gain: 1.0,
        }
    }

    /// Advances the envelope by one sample given speech presence flag.
    pub fn tick(&mut self, speech_present: bool) -> f32 {
        let target = if speech_present {
            db_to_linear(self.depth_db)
        } else {
            1.0
        };

        if target < self.current_gain {
            // Attack phase (ducking)
            self.current_gain = target + self.attack_coeff * (self.current_gain - target);
        } else {
            // Release phase (restoring)
            self.current_gain = target + self.release_coeff * (self.current_gain - target);
        }

        self.current_gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_linear_conversion() {
        assert_eq!(db_to_linear(0.0), 1.0);
        assert!((db_to_linear(-6.0) - 0.501187).abs() < 1e-4);
        assert_eq!(db_to_linear(-60.0), 0.0);

        assert!((linear_to_db(1.0) - 0.0).abs() < 1e-4);
        assert!((linear_to_db(0.5) - (-6.020599)).abs() < 1e-4);
        assert_eq!(linear_to_db(0.0), -60.0);
    }

    #[test]
    fn test_equal_power_pan_law() {
        let (l_center, r_center) = PanLaw::EqualPower.gains(0.0);
        // At center, both channels should be 1/sqrt(2) ≈ 0.7071 (-3dB)
        assert!((l_center - 0.707_106_77).abs() < 1e-4);
        assert!((r_center - 0.707_106_77).abs() < 1e-4);
        // Energy sum: l^2 + r^2 should be exactly 1.0
        let energy = l_center * l_center + r_center * r_center;
        assert!((energy - 1.0).abs() < 1e-4);

        // Hard Left
        let (l_left, r_left) = PanLaw::EqualPower.gains(-1.0);
        assert!((l_left - 1.0).abs() < 1e-4);
        assert!(r_left.abs() < 1e-4);

        // Hard Right
        let (l_right, r_right) = PanLaw::EqualPower.gains(1.0);
        assert!(l_right.abs() < 1e-4);
        assert!((r_right - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_k_weighting_filter_energy_and_lufs() {
        let mut filter = KWeightingFilter::for_48k();

        // 1. Digital silence test
        let silence = vec![0.0f32; 960];
        let (energy_l, energy_r) = filter.process_stereo_energy(&silence);
        assert_eq!(energy_l, 0.0);
        assert_eq!(energy_r, 0.0);
        let lufs_silence = KWeightingFilter::lufs_from_energy(energy_l, energy_r);
        assert_eq!(lufs_silence, -70.0);

        // 2. 1kHz sine wave 0 dBFS test
        // 1kHz tone produces known calibrated response
        let mut sine_stereo = Vec::with_capacity(4800);
        for i in 0..2400 {
            let t = i as f32 / 48000.0;
            let val = (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
            sine_stereo.push(val);
            sine_stereo.push(val);
        }
        let (e_l, e_r) = filter.process_stereo_energy(&sine_stereo);
        assert!(e_l > 0.0);
        assert!(e_r > 0.0);
        let lufs = KWeightingFilter::lufs_from_energy(e_l, e_r);
        assert!(lufs > -10.0 && lufs < 5.0);
    }

    #[test]
    fn test_ducking_envelope() {
        let mut duck = DuckingEnvelope::new(-12.0, 0.01, 0.05, 1000);
        assert_eq!(duck.current_gain, 1.0);

        // Speech starts -> gain ducks down
        for _ in 0..50 {
            duck.tick(true);
        }
        assert!(duck.current_gain < 0.5);

        // Speech stops -> gain recovers back to 1.0
        for _ in 0..200 {
            duck.tick(false);
        }
        assert!(duck.current_gain > 0.95);
    }
}
