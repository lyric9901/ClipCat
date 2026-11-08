// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah

//! Core vocabulary of the Concat engine.
//!
//! This crate holds the types that every other crate needs to agree on, and
//! nothing else. It has no dependencies and does no IO.
//!
//! - [`time`] - exact rational timestamps. Every time value in Concat is a
//!   [`time::Rational`] number of seconds. Never `f64`.
//! - [`arena`] - generational arenas and `Copy` handles, the way Concat models
//!   graphs.
//! - [`frame`] - a decoded RGBA8 image buffer.
//! - [`timeline`] - projects, tracks and clips.
//!
//! If you are about to add a dependency to this crate, the thing you are adding
//! probably belongs in `concat-media` or `concat-render` instead.

pub mod ai_creator;
pub mod animate;
pub mod arena;
pub mod audio;
pub mod frame;
pub mod mask;
pub mod retime;
pub mod scopes;
pub mod scrub;
pub mod shader;
pub mod time;
pub mod timeline;

pub use ai_creator::{BeatDetector, SilenceDetectConfig, SilenceDetector};
pub use audio::{
    DuckingEnvelope, KWeightingFilter, MasterBus, PanLaw, TrackMixerChannel, db_to_linear,
    linear_to_db,
};
pub use frame::Frame;
pub use retime::SpeedCurve;
pub use scopes::{
    ColorBarTarget, HistogramData, RgbParadeData, VectorscopeData, VideoScopesAnalyzer,
    WaveformData,
};
pub use scrub::{ScrubDirection, ScrubTrajectory};
pub use shader::{Lut, RevealMap, ShaderPass, Stage, TransitionPass};
pub use time::{FrameRate, Rational, ShuttleSpeed, TimeRange};
pub use timeline::{Blend, Clip, ClipId, Project, Timeline, Track, TrackId, TrackKind};
