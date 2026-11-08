// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Workspace Architecture: Premiere Pro & DaVinci Resolve-inspired workspace
//! presets, dock tree models, pane kinds, and layout serialization.

use serde::{Deserialize, Serialize};

/// The distinct panels that can occupy any seat in the Concat editing workspace.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspacePane {
    /// Project media pool, bin folders, and asset browsers.
    Media,
    /// Program Monitor / Source Monitor video preview canvas.
    Preview,
    /// Properties inspector (transforms, effects, color controls, audio knobs).
    Inspector,
    /// Multitrack video and audio timeline with ruler and track headers.
    Timeline,
    /// Visual keyframe dopesheet and bezier speed/value graph editor.
    Keyframes,
    /// Fairlight-style multi-track audio mixer console with faders and meters.
    AudioMixer,
    /// Real-time video scopes (Waveform, Vectorscope, Histogram, RGB Parade).
    Scopes,
    /// Dedicated 16-category Creative Asset Library.
    CreativeLibrary,
}

impl WorkspacePane {
    /// Static string identifier of the pane.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Media => "media",
            Self::Preview => "preview",
            Self::Inspector => "inspector",
            Self::Timeline => "timeline",
            Self::Keyframes => "keyframes",
            Self::AudioMixer => "audio-mixer",
            Self::Scopes => "scopes",
            Self::CreativeLibrary => "creative-library",
        }
    }

    /// English display name for pane headers and dropdown selectors.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Media => "Project Media",
            Self::Preview => "Program Monitor",
            Self::Inspector => "Inspector",
            Self::Timeline => "Timeline",
            Self::Keyframes => "Keyframes & Curves",
            Self::AudioMixer => "Audio Mixer",
            Self::Scopes => "Video Scopes",
            Self::CreativeLibrary => "Creative Library",
        }
    }

    /// Parses pane from string identifier.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "media" | "bin" | "project" => Some(Self::Media),
            "preview" | "monitor" | "program" => Some(Self::Preview),
            "inspector" | "properties" | "details" => Some(Self::Inspector),
            "timeline" | "tracks" | "lanes" => Some(Self::Timeline),
            "keyframes" | "curves" | "graph" => Some(Self::Keyframes),
            "audio-mixer" | "mixer" | "fairlight" => Some(Self::AudioMixer),
            "scopes" | "lumetri-scopes" => Some(Self::Scopes),
            "creative-library" | "library" | "assets" => Some(Self::CreativeLibrary),
            _ => None,
        }
    }
}

/// The 5 professional workspace presets inspired by Premiere Pro and DaVinci Resolve.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspacePreset {
    /// Premiere Pro-style editing: Media & Bins left, Program monitor center, Inspector right, Timeline bottom.
    Editing,
    /// DaVinci Resolve-style color grading: Preview top-left, Real-time Scopes top-right, Inspector right, Timeline bottom.
    Color,
    /// Fairlight DAW-style audio: Media & Preview top, Multi-track Timeline & Audio Mixer bottom.
    Audio,
    /// After Effects & Alight Motion-style motion graphics: Preview & Inspector top, Keyframes dopesheet & graph editor bottom.
    MotionGraphics,
    /// Creative library & effects laboratory: 16-category Creative Library left, Preview center, Effect controls right, Timeline bottom.
    Effects,
    /// User-customized workspace layout.
    Custom,
}

impl WorkspacePreset {
    /// All 6 professional workspace presets.
    pub const ALL: [Self; 6] = [
        Self::Editing,
        Self::Color,
        Self::Audio,
        Self::MotionGraphics,
        Self::Effects,
        Self::Custom,
    ];

    /// String identifier of the workspace preset.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Editing => "editing",
            Self::Color => "color",
            Self::Audio => "audio",
            Self::MotionGraphics => "motion-graphics",
            Self::Effects => "effects",
            Self::Custom => "custom",
        }
    }

    /// English display name for top bar workspace selector.
    pub const fn display_name(&self) -> &'static str {
        match self {
            Self::Editing => "Editing",
            Self::Color => "Color",
            Self::Audio => "Audio",
            Self::MotionGraphics => "Motion Graphics",
            Self::Effects => "Effects",
            Self::Custom => "Custom",
        }
    }

    /// Parses workspace preset from string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "editing" | "edit" => Some(Self::Editing),
            "color" | "grading" => Some(Self::Color),
            "audio" | "fairlight" | "sound" => Some(Self::Audio),
            "motion-graphics" | "motion" | "motiongraphics" | "animation" => {
                Some(Self::MotionGraphics)
            }
            "effects" | "vfx" | "creative" => Some(Self::Effects),
            "custom" | "user" => Some(Self::Custom),
            _ => None,
        }
    }

    /// Builds the layout tree for this workspace preset.
    pub fn layout(&self) -> WorkspaceLayout {
        match self {
            Self::Custom => Self::Editing.layout(),
            Self::Editing => WorkspaceLayout::Split {
                columns: true,
                ratio: 0.78,
                first: Box::new(WorkspaceLayout::Split {
                    columns: false,
                    ratio: 0.60,
                    first: Box::new(WorkspaceLayout::Split {
                        columns: true,
                        ratio: 0.30,
                        first: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Media)),
                        second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Preview)),
                    }),
                    second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Timeline)),
                }),
                second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Inspector)),
            },
            Self::Color => WorkspaceLayout::Split {
                columns: false,
                ratio: 0.65,
                first: Box::new(WorkspaceLayout::Split {
                    columns: true,
                    ratio: 0.45,
                    first: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Preview)),
                    second: Box::new(WorkspaceLayout::Split {
                        columns: true,
                        ratio: 0.50,
                        first: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Scopes)),
                        second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Inspector)),
                    }),
                }),
                second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Timeline)),
            },
            Self::Audio => WorkspaceLayout::Split {
                columns: false,
                ratio: 0.40,
                first: Box::new(WorkspaceLayout::Split {
                    columns: true,
                    ratio: 0.35,
                    first: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Media)),
                    second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Preview)),
                }),
                second: Box::new(WorkspaceLayout::Split {
                    columns: true,
                    ratio: 0.60,
                    first: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Timeline)),
                    second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::AudioMixer)),
                }),
            },
            Self::MotionGraphics => WorkspaceLayout::Split {
                columns: false,
                ratio: 0.50,
                first: Box::new(WorkspaceLayout::Split {
                    columns: true,
                    ratio: 0.65,
                    first: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Preview)),
                    second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Inspector)),
                }),
                second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Keyframes)),
            },
            Self::Effects => WorkspaceLayout::Split {
                columns: true,
                ratio: 0.35,
                first: Box::new(WorkspaceLayout::Leaf(WorkspacePane::CreativeLibrary)),
                second: Box::new(WorkspaceLayout::Split {
                    columns: false,
                    ratio: 0.65,
                    first: Box::new(WorkspaceLayout::Split {
                        columns: true,
                        ratio: 0.60,
                        first: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Preview)),
                        second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Inspector)),
                    }),
                    second: Box::new(WorkspaceLayout::Leaf(WorkspacePane::Timeline)),
                }),
            },
        }
    }
}

/// Recursive workspace dock tree arrangement.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceLayout {
    /// A single leaf pane filling its allocated bounding box.
    Leaf(WorkspacePane),
    /// Two layout nodes partitioned by a draggable divider.
    Split {
        /// True if split vertically (left and right columns); false if split horizontally (top and bottom rows).
        columns: bool,
        /// Fraction of usable space occupied by the first node (clamped to 0.05..=0.95).
        ratio: f32,
        /// First node in order (left or top).
        first: Box<WorkspaceLayout>,
        /// Second node in order (right or bottom).
        second: Box<WorkspaceLayout>,
    },
}

impl WorkspaceLayout {
    /// Checks if a specific pane kind exists anywhere in this layout tree.
    pub fn contains_pane(&self, pane: WorkspacePane) -> bool {
        match self {
            Self::Leaf(held) => *held == pane,
            Self::Split { first, second, .. } => {
                first.contains_pane(pane) || second.contains_pane(pane)
            }
        }
    }

    /// Counts total number of leaf panes in the tree.
    pub fn count_leaves(&self) -> usize {
        match self {
            Self::Leaf(_) => 1,
            Self::Split { first, second, .. } => first.count_leaves() + second.count_leaves(),
        }
    }

    /// Normalizes all split ratios to guaranteed usable bounds (0.05..=0.95).
    pub fn sanitize(&mut self) {
        if let Self::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            if ratio.is_nan() || *ratio < 0.05 {
                *ratio = 0.05;
            } else if *ratio > 0.95 {
                *ratio = 0.95;
            }
            first.sanitize();
            second.sanitize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_panes_parsing() {
        for pane in [
            WorkspacePane::Media,
            WorkspacePane::Preview,
            WorkspacePane::Inspector,
            WorkspacePane::Timeline,
            WorkspacePane::Keyframes,
            WorkspacePane::AudioMixer,
            WorkspacePane::Scopes,
            WorkspacePane::CreativeLibrary,
        ] {
            assert_eq!(WorkspacePane::parse(pane.as_str()), Some(pane));
            assert!(!pane.display_name().is_empty());
        }
        assert_eq!(
            WorkspacePane::parse("mixer"),
            Some(WorkspacePane::AudioMixer)
        );
        assert_eq!(
            WorkspacePane::parse("curves"),
            Some(WorkspacePane::Keyframes)
        );
    }

    #[test]
    fn test_all_workspace_presets_layout_sanity() {
        for preset in WorkspacePreset::ALL {
            assert_eq!(WorkspacePreset::parse(preset.as_str()), Some(preset));
            let mut layout = preset.layout();
            layout.sanitize();

            let leaves = layout.count_leaves();
            assert!(
                leaves >= 3,
                "Workspace {preset:?} must have at least 3 panes"
            );

            // Key invariants per workspace
            match preset {
                WorkspacePreset::Editing => {
                    assert!(layout.contains_pane(WorkspacePane::Timeline));
                    assert!(layout.contains_pane(WorkspacePane::Preview));
                    assert!(layout.contains_pane(WorkspacePane::Media));
                }
                WorkspacePreset::Color => {
                    assert!(layout.contains_pane(WorkspacePane::Scopes));
                    assert!(layout.contains_pane(WorkspacePane::Preview));
                }
                WorkspacePreset::Audio => {
                    assert!(layout.contains_pane(WorkspacePane::AudioMixer));
                    assert!(layout.contains_pane(WorkspacePane::Timeline));
                }
                WorkspacePreset::MotionGraphics => {
                    assert!(layout.contains_pane(WorkspacePane::Keyframes));
                    assert!(layout.contains_pane(WorkspacePane::Preview));
                }
                WorkspacePreset::Effects => {
                    assert!(layout.contains_pane(WorkspacePane::CreativeLibrary));
                    assert!(layout.contains_pane(WorkspacePane::Preview));
                }
                WorkspacePreset::Custom => {
                    assert!(layout.contains_pane(WorkspacePane::Timeline));
                    assert!(layout.contains_pane(WorkspacePane::Preview));
                }
            }

            // JSON roundtrip
            let json = serde_json::to_string(&layout).unwrap();
            let parsed: WorkspaceLayout = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, layout);
        }
    }
}
