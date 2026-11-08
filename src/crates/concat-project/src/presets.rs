// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Motion graphics and animation presets for timeline clips.
//!
//! Presets generate real `ClipKey` entries on clips, allowing users to
//! visually inspect, customize, retime, or refine them with curve editors.

use crate::model::{Clip, KeyEase, KeyProperty};

/// Metadata description of an animation preset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetInfo {
    /// Unique machine identifier for this preset.
    pub id: &'static str,
    /// Human-readable display name.
    pub name: &'static str,
    /// Category grouping (Entrance, Exit, Motion).
    pub category: &'static str,
    /// Detailed description of the motion style.
    pub description: &'static str,
}

/// Built-in motion graphics presets.
pub const PRESETS: &[PresetInfo] = &[
    PresetInfo {
        id: "fade-in",
        name: "Fade In",
        category: "Entrance",
        description: "Smooth fade in from transparent to fully opaque",
    },
    PresetInfo {
        id: "fade-out",
        name: "Fade Out",
        category: "Exit",
        description: "Smooth fade out from opaque to transparent",
    },
    PresetInfo {
        id: "pop-in",
        name: "Pop In",
        category: "Entrance",
        description: "Scale pop with overshoot settling into place",
    },
    PresetInfo {
        id: "slide-in-left",
        name: "Slide In Left",
        category: "Entrance",
        description: "Slide in smoothly from the left edge of frame",
    },
    PresetInfo {
        id: "slide-in-right",
        name: "Slide In Right",
        category: "Entrance",
        description: "Slide in smoothly from the right edge of frame",
    },
    PresetInfo {
        id: "slide-in-top",
        name: "Slide In Top",
        category: "Entrance",
        description: "Slide in smoothly from the top edge of frame",
    },
    PresetInfo {
        id: "slide-in-bottom",
        name: "Slide In Bottom",
        category: "Entrance",
        description: "Slide in smoothly from the bottom edge of frame",
    },
    PresetInfo {
        id: "bounce",
        name: "Bounce",
        category: "Motion",
        description: "Decaying vertical bounce settling on position",
    },
    PresetInfo {
        id: "shake",
        name: "Shake",
        category: "Motion",
        description: "Rapid energetic horizontal camera shake",
    },
    PresetInfo {
        id: "spin-in",
        name: "Spin In",
        category: "Entrance",
        description: "Fast 360-degree rotation and scale zoom entrance",
    },
    PresetInfo {
        id: "zoom-in",
        name: "Zoom In",
        category: "Entrance",
        description: "Smooth zoom into standard framing",
    },
    PresetInfo {
        id: "zoom-out",
        name: "Zoom Out",
        category: "Exit",
        description: "Smooth zoom out to background",
    },
];

/// Applies an animation preset to a clip by setting real keyframes.
/// Returns true if the preset was recognized and applied.
pub fn apply_preset(clip: &mut Clip, preset: &str) -> bool {
    match preset.to_lowercase().as_str() {
        "fade-in" => {
            clip.set_key(KeyProperty::Opacity, 0.0, 0.0, KeyEase::OUT);
            clip.set_key(KeyProperty::Opacity, 0.15, 1.0, KeyEase::LINEAR);
            true
        }
        "fade-out" => {
            clip.set_key(KeyProperty::Opacity, 0.85, 1.0, KeyEase::IN);
            clip.set_key(KeyProperty::Opacity, 1.0, 0.0, KeyEase::LINEAR);
            true
        }
        "pop-in" => {
            clip.set_key(KeyProperty::Scale, 0.0, 0.05, KeyEase::OUT);
            clip.set_key(KeyProperty::Scale, 0.12, 1.15, KeyEase::IN_OUT);
            clip.set_key(KeyProperty::Scale, 0.20, 1.0, KeyEase::LINEAR);
            true
        }
        "slide-in-left" => {
            clip.set_key(KeyProperty::OffsetX, 0.0, -1.0, KeyEase::OUT);
            clip.set_key(KeyProperty::OffsetX, 0.20, 0.0, KeyEase::LINEAR);
            true
        }
        "slide-in-right" => {
            clip.set_key(KeyProperty::OffsetX, 0.0, 1.0, KeyEase::OUT);
            clip.set_key(KeyProperty::OffsetX, 0.20, 0.0, KeyEase::LINEAR);
            true
        }
        "slide-in-top" => {
            clip.set_key(KeyProperty::OffsetY, 0.0, -1.0, KeyEase::OUT);
            clip.set_key(KeyProperty::OffsetY, 0.20, 0.0, KeyEase::LINEAR);
            true
        }
        "slide-in-bottom" => {
            clip.set_key(KeyProperty::OffsetY, 0.0, 1.0, KeyEase::OUT);
            clip.set_key(KeyProperty::OffsetY, 0.20, 0.0, KeyEase::LINEAR);
            true
        }
        "bounce" => {
            clip.set_key(KeyProperty::OffsetY, 0.0, 0.0, KeyEase::OUT);
            clip.set_key(KeyProperty::OffsetY, 0.08, -0.20, KeyEase::IN);
            clip.set_key(KeyProperty::OffsetY, 0.16, 0.0, KeyEase::OUT);
            clip.set_key(KeyProperty::OffsetY, 0.22, -0.08, KeyEase::IN);
            clip.set_key(KeyProperty::OffsetY, 0.28, 0.0, KeyEase::LINEAR);
            true
        }
        "shake" => {
            clip.set_key(KeyProperty::OffsetX, 0.0, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetX, 0.04, -0.05, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetX, 0.08, 0.05, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetX, 0.12, -0.03, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetX, 0.16, 0.03, KeyEase::LINEAR);
            clip.set_key(KeyProperty::OffsetX, 0.20, 0.0, KeyEase::LINEAR);
            true
        }
        "spin-in" => {
            clip.set_key(KeyProperty::Rotation, 0.0, -360.0, KeyEase::OUT);
            clip.set_key(KeyProperty::Rotation, 0.25, 0.0, KeyEase::LINEAR);
            clip.set_key(KeyProperty::Scale, 0.0, 0.0, KeyEase::OUT);
            clip.set_key(KeyProperty::Scale, 0.25, 1.0, KeyEase::LINEAR);
            true
        }
        "zoom-in" => {
            clip.set_key(KeyProperty::Scale, 0.0, 0.5, KeyEase::OUT);
            clip.set_key(KeyProperty::Scale, 0.20, 1.0, KeyEase::LINEAR);
            true
        }
        "zoom-out" => {
            clip.set_key(KeyProperty::Scale, 0.80, 1.0, KeyEase::IN);
            clip.set_key(KeyProperty::Scale, 1.0, 0.5, KeyEase::LINEAR);
            true
        }
        _ => false,
    }
}
