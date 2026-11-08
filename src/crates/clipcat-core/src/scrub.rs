// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Audio scrub timing, trajectory calculation and envelope shaping.
//!
//! Provides the mathematical core for responsive, low-latency audio scrubbing
//! synchronized with rational timeline time. Works across forward, backward,
//! and stationary (frame-stepping) playback gestures with zero allocations and no IO.

/// Scrub playback direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrubDirection {
    /// Scrubbing towards higher timeline timestamps.
    Forward,
    /// Scrubbing towards lower timeline timestamps (reverse).
    Backward,
    /// Scrubbing in place or stepping a single frame.
    Stationary,
}

/// Calculated parameters for a single bounded audio scrub snippet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrubTrajectory {
    /// Direction of scrub motion.
    pub direction: ScrubDirection,
    /// Starting timeline position in seconds.
    pub start_seconds: f64,
    /// Target timeline position in seconds.
    pub target_seconds: f64,
    /// Timeline seconds advanced per audio output frame (can be negative for reverse).
    pub step_per_frame: f64,
    /// Total audio frames to synthesize for this snippet.
    pub total_frames: usize,
    /// Frame count for the attack and release anti-pop de-clicking ramps.
    pub fade_frames: usize,
    /// Effective playback speed multiplier.
    pub speed: f64,
}

impl ScrubTrajectory {
    /// Computes scrub trajectory from `from` to `to` timeline positions.
    ///
    /// - `from`: previous playhead position in seconds.
    /// - `to`: new target playhead position in seconds.
    /// - `sample_rate`: output audio device rate in Hz (e.g. 48000).
    pub fn compute(from: f64, to: f64, sample_rate: u32) -> Self {
        let sr = f64::from(sample_rate.max(8_000));
        let from_clamped = from.max(0.0);
        let to_clamped = to.max(0.0);
        let delta = to_clamped - from_clamped;
        let abs_delta = delta.abs();

        if abs_delta < 1e-5 {
            // Stationary frame step or click seek: synthesize a short 35ms forward preview
            let snippet_sec = 0.035;
            let total_frames = (snippet_sec * sr).round() as usize;
            let step = 1.0 / sr;
            let fade_frames = (total_frames / 4).clamp(32, 192);
            Self {
                direction: ScrubDirection::Stationary,
                start_seconds: to_clamped,
                target_seconds: to_clamped + snippet_sec,
                step_per_frame: step,
                total_frames: total_frames.max(64),
                fade_frames,
                speed: 1.0,
            }
        } else {
            let direction = if delta > 0.0 {
                ScrubDirection::Forward
            } else {
                ScrubDirection::Backward
            };
            let dir_sign = if delta > 0.0 { 1.0 } else { -1.0 };

            // Bound timeline window per gesture tick to at most 80ms to prevent audio backlog
            let max_span = 0.080;
            let effective_span = abs_delta.min(max_span);

            // Origin position: when span was truncated due to rapid movement, anchor to the destination
            let start_seconds = if dir_sign > 0.0 {
                (to_clamped - effective_span).max(0.0)
            } else {
                to_clamped + effective_span
            };

            // Adaptive speed: faster scrubbing speeds up the playback snippet (0.6x to 2.5x)
            let speed = (effective_span / 0.040).clamp(0.6, 2.5);
            let snippet_dur = (effective_span / speed).clamp(0.025, 0.075);
            let total_frames = (snippet_dur * sr).round() as usize;
            let step_per_frame = (dir_sign * speed) / sr;
            let fade_frames = (total_frames / 5).clamp(32, 240);

            Self {
                direction,
                start_seconds,
                target_seconds: to_clamped,
                step_per_frame,
                total_frames: total_frames.max(64),
                fade_frames,
                speed,
            }
        }
    }

    /// Calculates anti-pop envelope amplitude in `[0.0, 1.0]` for frame index `frame_idx`.
    #[inline]
    pub fn envelope(&self, frame_idx: usize) -> f32 {
        if self.fade_frames == 0 || self.total_frames == 0 {
            return 1.0;
        }
        if frame_idx < self.fade_frames {
            // Smooth linear attack
            frame_idx as f32 / self.fade_frames as f32
        } else if frame_idx + self.fade_frames >= self.total_frames {
            // Smooth linear release
            let remaining = self.total_frames.saturating_sub(frame_idx);
            (remaining as f32 / self.fade_frames as f32).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stationary_scrub_trajectory() {
        let traj = ScrubTrajectory::compute(2.0, 2.0, 48_000);
        assert_eq!(traj.direction, ScrubDirection::Stationary);
        assert!((traj.start_seconds - 2.0).abs() < 1e-6);
        assert!(traj.step_per_frame > 0.0);
        assert_eq!(traj.speed, 1.0);
        assert!(traj.total_frames >= 1000 && traj.total_frames <= 2000);
        assert_eq!(traj.envelope(0), 0.0);
        assert_eq!(traj.envelope(traj.fade_frames), 1.0);
        assert_eq!(traj.envelope(traj.total_frames), 0.0);
    }

    #[test]
    fn test_forward_scrub_trajectory() {
        let traj = ScrubTrajectory::compute(1.0, 1.04, 48_000);
        assert_eq!(traj.direction, ScrubDirection::Forward);
        assert!((traj.start_seconds - 1.0).abs() < 1e-6);
        assert_eq!(traj.target_seconds, 1.04);
        assert!(traj.step_per_frame > 0.0);
        assert!(traj.speed >= 0.6 && traj.speed <= 2.5);
    }

    #[test]
    fn test_backward_scrub_trajectory() {
        let traj = ScrubTrajectory::compute(3.0, 2.96, 48_000);
        assert_eq!(traj.direction, ScrubDirection::Backward);
        assert!((traj.start_seconds - 3.0).abs() < 1e-6);
        assert_eq!(traj.target_seconds, 2.96);
        assert!(traj.step_per_frame < 0.0);
    }

    #[test]
    fn test_rapid_scrub_bounded_memory() {
        // Jump 10 seconds ahead in a single tick
        let traj = ScrubTrajectory::compute(0.0, 10.0, 48_000);
        assert_eq!(traj.direction, ScrubDirection::Forward);
        // Effective span capped at 80ms (0.080s)
        assert!((traj.start_seconds - 9.92).abs() < 1e-4);
        assert_eq!(traj.target_seconds, 10.0);
        // Total output frames bounded between 25ms and 75ms (<= 3600 frames at 48k)
        assert!(traj.total_frames <= 3600);
        assert!(traj.total_frames >= 1200);
    }

    #[test]
    fn test_envelope_anti_pop() {
        let traj = ScrubTrajectory::compute(1.0, 1.05, 48_000);
        assert_eq!(traj.envelope(0), 0.0);
        let mid = traj.total_frames / 2;
        assert_eq!(traj.envelope(mid), 1.0);
        assert_eq!(traj.envelope(traj.total_frames), 0.0);
    }
}
