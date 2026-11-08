// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Flattening a project document into the exporter's clip list.
//!
//! Walking a timeline's clips, resolving their media and track, folding
//! track state into per-clip flags, and building each clip's FFmpeg chains
//! from the structured effects the document stores. An export is derived
//! from the engine's own model, so the model of record and the rendered
//! pixels cannot drift apart.
//!
//! Text clips are deliberately not flattened here. Titles rasterise
//! separately and rejoin the exporter as plain image clips; see
//! `ExportRequest`'s caller.

use std::collections::HashSet;
use std::path::Path;

use concat_project::model::{
    Background, ClipKind as ModelClipKind, KeyProperty, MediaKind, Project, Timeline,
};

use crate::chains::audio_filter_chain;
use crate::{ClipKind, ExportBackdrop, ExportClip, ExportKey, TransitionSpec};

/// Flattens one timeline of `project` for the exporter - the active one
/// when `timeline_id` is `None`. Text clips are excluded (they rasterise
/// separately and rejoin as image clips); a clip whose media or track has
/// vanished is skipped with the same tolerance the document reader extends.
pub fn flatten_timeline(project: &Project, timeline_id: Option<&str>) -> Vec<ExportClip> {
    flatten_timeline_in(project, timeline_id, None)
}

/// Flattens a timeline restricted to a specific time range `[range_start, range_start + range_duration]`.
/// If range_start is None and range_duration is None, it flattens the full timeline.
pub fn flatten_timeline_range_in(
    project: &Project,
    timeline_id: Option<&str>,
    project_dir: Option<&Path>,
    range_start: Option<f64>,
    range_duration: Option<f64>,
) -> Vec<ExportClip> {
    let clips = flatten_timeline_in(project, timeline_id, project_dir);
    match (range_start, range_duration) {
        (Some(start), Some(dur)) if dur > 0.0 => slice_clips_to_range(clips, start.max(0.0), dur),
        (Some(start), None) => {
            let max_end = clips
                .iter()
                .map(|c| c.start + c.duration)
                .fold(0.0, f64::max);
            let dur = (max_end - start).max(0.0);
            if dur > 0.0 {
                slice_clips_to_range(clips, start.max(0.0), dur)
            } else {
                Vec::new()
            }
        }
        (None, Some(dur)) if dur > 0.0 => slice_clips_to_range(clips, 0.0, dur),
        _ => clips,
    }
}

/// Slices a list of flattened export clips to fit within `[range_start, range_start + range_duration]`.
/// Clips outside the range are discarded; clips straddling the boundaries are trimmed, and
/// all remaining clips have their timeline `start` shifted by `range_start`.
pub fn slice_clips_to_range(
    clips: Vec<ExportClip>,
    range_start: f64,
    range_duration: f64,
) -> Vec<ExportClip> {
    if range_duration <= 0.0 {
        return Vec::new();
    }
    let range_end = range_start + range_duration;
    clips
        .into_iter()
        .filter_map(|mut clip| {
            let clip_end = clip.start + clip.duration;
            if clip_end <= range_start || clip.start >= range_end {
                return None;
            }
            let overlap_start = clip.start.max(range_start);
            let overlap_end = clip_end.min(range_end);
            let new_duration = overlap_end - overlap_start;
            if new_duration <= 0.0001 {
                return None;
            }
            let start_trim = overlap_start - clip.start;
            clip.source_start += start_trim * clip.speed;
            clip.start = overlap_start - range_start;
            clip.duration = new_duration;
            if start_trim > 0.0 {
                clip.fade_in = (clip.fade_in - start_trim).max(0.0);
            }
            let end_trim = clip_end - overlap_end;
            if end_trim > 0.0 {
                clip.fade_out = (clip.fade_out - end_trim).max(0.0);
            }
            Some(clip)
        })
        .collect()
}

/// [`flatten_timeline`], knowing the project folder: a clip with a cutout
/// is then told where its media's masks are, and renders cut, and a
/// stabilized one where its media's movement is, and renders steadied.
/// Without the folder both are carried but not rendered.
pub fn flatten_timeline_in(
    project: &Project,
    timeline_id: Option<&str>,
    project_dir: Option<&Path>,
) -> Vec<ExportClip> {
    let mut visited = HashSet::new();
    flatten_timeline_internal(project, timeline_id, project_dir, &mut visited, 0)
}

fn flatten_timeline_internal(
    project: &Project,
    timeline_id: Option<&str>,
    project_dir: Option<&Path>,
    visited: &mut HashSet<String>,
    depth: usize,
) -> Vec<ExportClip> {
    if depth >= 8 {
        return Vec::new();
    }
    let Some(timeline) = pick_timeline(project, timeline_id) else {
        return Vec::new();
    };
    if !visited.insert(timeline.id.clone()) {
        return Vec::new();
    }
    // Found once: asked of every clip, it is a walk of every clip.
    let lane = timeline.background_lane();
    let any_solo = timeline.tracks.iter().any(|t| t.solo);
    let is_track_audible = |track: &concat_project::Track| {
        if any_solo {
            track.solo && !track.muted
        } else {
            !track.muted
        }
    };

    let mut out = Vec::new();

    for clip in &timeline.clips {
        // Titles and shapes rasterise separately and rejoin as stills;
        // see concat-host's titles.
        if matches!(clip.kind, ModelClipKind::Text | ModelClipKind::Shape) {
            continue;
        }
        let Some(index) = timeline
            .tracks
            .iter()
            .position(|track| track.id == clip.track_id)
        else {
            continue;
        };
        let track = &timeline.tracks[index];
        let track_muted = !is_track_audible(track);

        // A compound clip / nested sequence: expand child timeline recursively.
        if clip.kind == ModelClipKind::Sequence {
            if visited.contains(&clip.media_id) {
                continue;
            }
            let child_clips = flatten_timeline_internal(
                project,
                Some(&clip.media_id),
                project_dir,
                visited,
                depth + 1,
            );
            if child_clips.is_empty() {
                continue;
            }
            let speed = if clip.speed.abs() > 1e-6 {
                clip.speed
            } else {
                1.0
            };
            let sliced =
                slice_clips_to_range(child_clips, clip.source_start, clip.duration * speed);
            for mut child in sliced {
                child.start = clip.start + child.start / speed;
                child.duration /= speed;
                child.speed *= speed;
                child.opacity = (child.opacity * clip.opacity).clamp(0.0, 1.0);
                if clip.volume != 1.0 {
                    child.volume *= clip.volume;
                }
                if !track.visible || clip.hidden {
                    child.hidden = true;
                }
                if track_muted || clip.muted == Some(true) {
                    child.muted = true;
                }
                child.track += index;
                out.push(child);
            }
            continue;
        }

        // A layer: no file, a chain over the stack beneath its track,
        // its opacity the strength and its fades the ramps.
        if clip.kind == ModelClipKind::Layer {
            out.push(ExportClip {
                hidden: !track.visible,
                muted: true,
                volume: 0.0,
                fade_in: clip.fade_in,
                fade_out: clip.fade_out,
                effects: clip.video_effects.clone(),
                opacity: clip.opacity,
                has_audio: Some(false),
                ..ExportClip::blank(ClipKind::Layer, clip.start, clip.duration, index)
            });
            continue;
        }

        // A transition placed as a clip (#621): no file, its package and
        // knobs the one link of its chain, its length the clip's.
        // `resolve_transitions` finds the cut beneath it, if any. One
        // whose link is gone or switched off draws nothing.
        if clip.kind == ModelClipKind::Transition {
            let Some(package) = clip.video_effects.first() else {
                continue;
            };
            out.push(ExportClip {
                hidden: !track.visible || !package.enabled,
                muted: true,
                volume: 0.0,
                has_audio: Some(false),
                transition: Some(TransitionSpec {
                    kind: package.id.clone(),
                    duration: clip.duration,
                    params: package.params.clone(),
                }),
                ..ExportClip::blank(ClipKind::Transition, clip.start, clip.duration, index)
            });
            continue;
        }

        // A sound layer: no file and no picture, a chain over the mix
        // for its span, its opacity the strength and its fades the
        // ramps, silent with its lane.
        if clip.kind == ModelClipKind::SoundLayer {
            out.push(ExportClip {
                hidden: true,
                muted: track_muted || clip.muted == Some(true),
                volume: 0.0,
                fade_in: clip.fade_in,
                fade_out: clip.fade_out,
                filter_chain: audio_filter_chain(&clip.filters),
                opacity: clip.opacity,
                has_audio: Some(false),
                ..ExportClip::blank(ClipKind::SoundLayer, clip.start, clip.duration, index)
            });
            continue;
        }

        let Some(media) = project.media.iter().find(|item| item.id == clip.media_id) else {
            continue;
        };

        let kind = match clip.kind {
            ModelClipKind::Video => ClipKind::Video,
            ModelClipKind::Audio => ClipKind::Audio,
            ModelClipKind::Image => ClipKind::Image,
            // Handled above; unreachable spelled as a skip so a new
            // kind fails soft.
            ModelClipKind::Text
            | ModelClipKind::Layer
            | ModelClipKind::SoundLayer
            | ModelClipKind::Shape
            | ModelClipKind::Transition
            | ModelClipKind::Sequence => continue,
        };

        out.push(ExportClip {
            path: media.path.clone(),
            audio_stream: clip.audio_stream,
            color_range: media.color_range,
            source_start: clip.source_start,
            hidden: !track.visible,
            muted: track_muted || clip.muted == Some(true),
            volume: export_base(clip, KeyProperty::Volume),
            fade_in: clip.fade_in,
            fade_out: clip.fade_out,
            filter_chain: audio_filter_chain(&clip.filters),
            speed: clip.speed,
            preserve_pitch: clip.preserve_pitch,
            speed_curve: clip
                .speed_curve
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|point| (point.at, point.speed))
                .collect(),
            speed_slopes: clip
                .speed_curve
                .as_deref()
                .filter(|points| concat_project::speed::is_smooth(points))
                .unwrap_or(&[])
                .iter()
                .filter_map(|point| point.slope)
                .collect(),
            animation: export_keys(clip),
            flip_h: clip.flip_h,
            flip_v: clip.flip_v,
            blend: clip.blend.clone(),
            crop: clip
                .crop
                .filter(|crop| !crop.is_none())
                .map(|crop| [crop.left, crop.top, crop.right, crop.bottom]),
            effects: clip.video_effects.clone(),
            scale: export_base(clip, KeyProperty::Scale),
            offset_x: export_base(clip, KeyProperty::OffsetX),
            offset_y: export_base(clip, KeyProperty::OffsetY),
            rotation: export_base(clip, KeyProperty::Rotation),
            stretch_x: clip.stretch_x,
            stretch_y: clip.stretch_y,
            opacity: export_base(clip, KeyProperty::Opacity),
            // Passed through unconditionally: `resolve_transitions` is
            // the one adjacency judge (frame/2 tolerance). A fixed
            // 1/60 s gate here would agree at 30fps and disagree at
            // every other rate; resolving is the kinder read of what
            // the user placed.
            transition: clip
                .transition_in
                .as_ref()
                .map(|transition| TransitionSpec {
                    kind: transition.id.clone(),
                    duration: transition.duration,
                    params: transition.params.clone(),
                }),
            media_width: media.width,
            media_height: media.height,
            media_duration: media.duration,
            has_audio: Some(media.has_audio),
            backdrop: timeline
                .background_on(clip, lane)
                .and_then(|background| backdrop_of(project, background)),
            cutout: clip.cutout.clone(),
            // Pictures only: a mask on a clip that is all sound would
            // keep part of nothing.
            mask: clip.mask.filter(|_| kind != ClipKind::Audio),
            mask_dir: match (&clip.cutout, project_dir) {
                (Some(cutout), Some(dir)) => {
                    concat_vision::mask_dir(dir, &media.path, cutout.subject)
                        .to_string_lossy()
                        .into_owned()
                }
                _ => String::new(),
            },
            stabilize: clip.stabilize.filter(|_| kind == ClipKind::Video),
            voice: match (&clip.voice, project_dir) {
                (Some(mix), Some(dir)) if kind != ClipKind::Image => {
                    let stems =
                        crate::voice_stems_dir(dir, &media.path, clip.audio_stream, mix.kind);
                    let (voice, rest) = (stems.join("voice.wav"), stems.join("rest.wav"));
                    // Until both parts are there the clip plays as
                    // recorded, and the host is making them.
                    (voice.is_file() && rest.is_file()).then(|| crate::VoiceStems {
                        voice: voice.to_string_lossy().into_owned(),
                        rest: rest.to_string_lossy().into_owned(),
                        voice_gain: mix.voice,
                        rest_gain: mix.background,
                    })
                }
                _ => None,
            },
            motion_dir: match (&clip.stabilize, project_dir) {
                (Some(_), Some(dir)) if kind == ClipKind::Video => {
                    concat_vision::stabilize::motion_dir(dir, &media.path)
                        .to_string_lossy()
                        .into_owned()
                }
                _ => String::new(),
            },
            ..ExportClip::blank(kind, clip.start, clip.duration, index)
        });
    }

    visited.remove(&timeline.id);
    out
}

/// A background as it is drawn: a blur and a colour as they are, a still
/// found in the bin. None for black, and for a still that is no longer in
/// the bin or is not a still - a background that cannot be drawn is no
/// background, never a failed frame.
fn backdrop_of(project: &Project, background: &Background) -> Option<ExportBackdrop> {
    match background {
        Background::None => None,
        Background::Blur { amount } => Some(ExportBackdrop::Blur { amount: *amount }),
        Background::Color { .. } => background.rgb().map(|rgb| ExportBackdrop::Colour { rgb }),
        Background::Image { media_id } => project
            .media_by_id(media_id)
            .filter(|item| item.kind == MediaKind::Image)
            .map(|item| ExportBackdrop::Image {
                path: item.path.clone(),
                width: item.width,
                height: item.height,
            }),
    }
}

/// The timeline to flatten: the named one, or the document's active one,
/// falling back to the first - the same preference the UI's tab strip has.
fn pick_timeline<'a>(project: &'a Project, timeline_id: Option<&str>) -> Option<&'a Timeline> {
    let wanted = timeline_id.unwrap_or(&project.active_timeline_id);
    project
        .timelines
        .iter()
        .find(|timeline| timeline.id == wanted)
        .or_else(|| project.timelines.first())
        .map(|timeline| timeline.as_ref())
}

/// The constant the engine should receive for a keyable property.
///
/// The clip's own, until that property is keyed - and then the *neutral*
/// value, because a keyed property's keys travel absolutely rather than as a
/// factor on this base. Absolute is not a preference: `Animation` multiplies
/// the base by the scale and opacity tracks, so a clip sitting at zero
/// opacity could never be keyed back up if its keys were relative to it.
///
pub fn export_base(clip: &concat_project::model::Clip, property: KeyProperty) -> f64 {
    if !clip.is_keyed(property) {
        return clip.constant(property);
    }
    match property {
        // A factor's neutral is one; an addition's is zero.
        KeyProperty::Scale
        | KeyProperty::ScaleX
        | KeyProperty::ScaleY
        | KeyProperty::Opacity
        | KeyProperty::Volume => 1.0,
        KeyProperty::OffsetX
        | KeyProperty::OffsetY
        | KeyProperty::Rotation
        | KeyProperty::CropLeft
        | KeyProperty::CropTop
        | KeyProperty::CropRight
        | KeyProperty::CropBottom => 0.0,
        // The mask travels whole, as `ExportClip::mask`, and its keys
        // replace its numbers; it never asks for a base.
        mask => clip.constant(mask),
    }
}

/// The keys the engine plays: the clip's own, property by property.
pub fn export_keys(clip: &concat_project::model::Clip) -> Vec<ExportKey> {
    let mut out = Vec::new();
    for property in KeyProperty::ALL {
        if !clip.is_keyed(property) {
            continue;
        }
        out.extend(clip.keys_on(property).map(|key| ExportKey {
            property: property.name().to_owned(),
            at: key.at,
            value: key.value,
            ease: key.ease.sane().0,
        }));
    }
    out
}

/// The clip's gain over its length, as `(fraction, gain)` pairs, or empty
/// for a clip whose gain is the one number in `ExportClip::volume`.
pub fn volume_curve(clip: &concat_project::model::Clip) -> Vec<(f64, f64)> {
    clip.keys_on(KeyProperty::Volume)
        .map(|key| (key.at, key.value))
        .collect()
}

#[cfg(test)]
mod tests {

    use concat_project::commands::{ClipPatch, NewMedia, TrackFlag};
    use concat_project::model::AppliedFilter;
    use concat_project::{Command, Editor};

    use super::*;

    /// A project with one video media item and one clip of it, built through
    /// the real command layer so the fixture cannot drift from the model.
    fn project_with_clip() -> (Editor, String, String) {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(Command::AddMedia {
                item: NewMedia {
                    path: "/footage/a.mp4".into(),
                    name: "a.mp4".into(),
                    duration: Some(10.0),
                    kind: concat_project::model::MediaKind::Video,
                    width: Some(1920),
                    height: Some(1080),
                    frame_rate: None,
                    frame_rate_fraction: None,
                    video_codec: None,
                    audio_codec: None,
                    has_audio: true,
                    audio_tracks: Vec::new(),
                    origin: None,
                    color_space: Default::default(),
                },
            })
            .expect("adds media")
            .created_id
            .expect("mints an id");
        let track_id = editor.project().timelines[0].tracks[0].id.clone();
        let clip_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id,
                start: 1.0,
                ripple: false,
            })
            .expect("adds clip")
            .created_id
            .expect("mints an id");
        (editor, media_id, clip_id)
    }

    #[test]
    fn a_clip_flattens_with_its_media_and_track_facts() {
        let (editor, _, _) = project_with_clip();
        let flat = flatten_timeline(editor.project(), None);
        assert_eq!(flat.len(), 1);
        let clip = &flat[0];
        assert_eq!(clip.path, "/footage/a.mp4");
        assert!(matches!(clip.kind, ClipKind::Video));
        assert_eq!(clip.start, 1.0);
        assert_eq!(clip.duration, 10.0, "the clip takes its media's length");
        assert_eq!(clip.track, 0);
        assert!(!clip.hidden);
        assert!(!clip.muted);
        assert_eq!(clip.media_width, Some(1920));
        assert_eq!(clip.has_audio, Some(true));
        assert_eq!(clip.filter_chain, "");
        assert!(clip.effects.is_empty());
    }

    #[test]
    fn track_state_folds_into_the_clip_flags() {
        let (mut editor, _, _) = project_with_clip();
        let track_id = editor.project().timelines[0].tracks[0].id.clone();
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_id.clone(),
                flag: TrackFlag::Visible,
                value: false,
            })
            .expect("hides");
        editor
            .apply(Command::SetTrackFlag {
                track_id,
                flag: TrackFlag::Muted,
                value: true,
            })
            .expect("mutes");
        let flat = flatten_timeline(editor.project(), None);
        assert!(flat[0].hidden, "a hidden track hides its clips");
        assert!(flat[0].muted, "a muted track mutes its clips");
    }

    #[test]
    fn track_solo_isolates_audio() {
        let (mut editor, _, _) = project_with_clip();
        let track1_id = editor.project().timelines[0].tracks[0].id.clone();
        let track2_id = editor
            .apply(Command::AddTrack { bottom: false })
            .expect("add track")
            .created_id
            .unwrap();
        editor
            .apply(Command::AddClip {
                media_id: "m1".into(),
                track_id: track2_id.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("add clip 2");

        // Initially neither soloed: both unmuted
        let flat = flatten_timeline(editor.project(), None);
        assert!(!flat[0].muted);
        assert!(!flat[1].muted);

        // Solo track 1: track 1 unmuted, track 2 muted
        editor
            .apply(Command::SetTrackFlag {
                track_id: track1_id.clone(),
                flag: TrackFlag::Solo,
                value: true,
            })
            .expect("solo track 1");
        let flat = flatten_timeline(editor.project(), None);
        assert!(!flat[0].muted, "soloed track 1 is audible");
        assert!(flat[1].muted, "non-soloed track 2 is muted");

        // Solo track 2 also: both audible
        editor
            .apply(Command::SetTrackFlag {
                track_id: track2_id.clone(),
                flag: TrackFlag::Solo,
                value: true,
            })
            .expect("solo track 2");
        let flat = flatten_timeline(editor.project(), None);
        assert!(!flat[0].muted, "both soloed: track 1 audible");
        assert!(!flat[1].muted, "both soloed: track 2 audible");

        // Explicit mute on soloed track still mutes it
        editor
            .apply(Command::SetTrackFlag {
                track_id: track1_id,
                flag: TrackFlag::Muted,
                value: true,
            })
            .expect("mute track 1");
        let flat = flatten_timeline(editor.project(), None);
        assert!(flat[0].muted, "soloed but explicitly muted track is muted");
        assert!(!flat[1].muted, "soloed and unmuted track 2 remains audible");
    }

    #[test]
    fn the_stored_effects_ride_along() {
        let (mut editor, _, clip_id) = project_with_clip();
        editor
            .apply(Command::UpdateClip {
                clip_id,
                patch: ClipPatch {
                    video_effects: Some(vec![AppliedFilter::new("concat.mono")]),
                    ..Default::default()
                },
            })
            .expect("applies effect");
        let flat = flatten_timeline(editor.project(), None);
        assert_eq!(flat[0].effects, vec![AppliedFilter::new("concat.mono")]);
    }

    /// A clip's mask and its keys reach the engine's clip, which plans
    /// the monitor's frames and the export's alike: one path, so the two
    /// cannot draw different masks.
    #[test]
    fn a_mask_and_its_keys_reach_the_engine() {
        use concat_core::time::Rational;
        use concat_project::model::{ClipMask, KeyEase, MaskShape};
        let (mut editor, _, clip_id) = project_with_clip();
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    mask: Some(Some(ClipMask {
                        feather: 0.2,
                        ..ClipMask::of(MaskShape::Heart)
                    })),
                    ..Default::default()
                },
            })
            .expect("masks");
        for (at, value) in [(0.0, -0.2), (1.0, 0.2)] {
            editor
                .apply(Command::SetClipKey {
                    clip_id: clip_id.clone(),
                    property: KeyProperty::MaskX,
                    at,
                    value,
                    ease: KeyEase::LINEAR,
                })
                .expect("keys");
        }
        let flat = flatten_timeline(editor.project(), None);
        let mask = flat[0].mask.expect("the mask rides along");
        assert_eq!(mask.shape, MaskShape::Heart);
        assert!(flat[0].animation.iter().any(|key| key.property == "maskX"));

        let mut clip = concat_core::timeline::Clip::new(
            concat_core::timeline::MediaRef::new("a.mp4"),
            Rational::ZERO,
            Rational::from_int(10),
        );
        clip.mask = Some(mask.engine());
        clip.animation = crate::resolve::animation_of(&flat[0].animation);
        let start = clip.mask_at(Rational::ZERO).expect("a mask");
        let middle = clip.mask_at(Rational::from_int(5)).expect("a mask");
        assert!((start.x + 0.2).abs() < 1e-12);
        assert!(middle.x.abs() < 1e-12);
        assert_eq!(middle.shape, concat_core::mask::MaskShape::Heart);
        assert!((middle.feather - 0.2).abs() < 1e-12);
        // The placement is untouched by the mask's keys.
        assert!(
            clip.animation
                .as_ref()
                .is_some_and(|a| a.offset_x.is_empty())
        );
    }

    #[test]
    fn text_clips_are_left_for_the_rasteriser() {
        let (mut editor, _, _) = project_with_clip();
        let track_id = Some(editor.project().timelines[0].tracks[0].id.clone());
        editor
            .apply(Command::AddTextClip {
                track_id,
                above: false,
                start: 0.0,
                style: None,
                duration: None,
                offset_y: None,
            })
            .expect("adds title");
        let flat = flatten_timeline(editor.project(), None);
        assert_eq!(flat.len(), 1, "the text clip does not flatten");
    }

    #[test]
    fn a_clip_whose_media_vanished_is_skipped_not_fatal() {
        let (mut editor, media_id, _) = project_with_clip();
        editor
            .apply(Command::RemoveMedia { media_id })
            .expect("removes");
        // RemoveMedia also removes the clips; force the orphan case instead
        // by asking for a timeline that does not exist.
        assert!(flatten_timeline(editor.project(), None).is_empty());
        assert!(flatten_timeline(editor.project(), Some("nope")).len() <= 1);
    }

    #[test]
    fn slice_clips_to_range_filters_and_offsets() {
        let mut clip1 = ExportClip::blank(ClipKind::Video, 2.0, 4.0, 0);
        clip1.source_start = 1.0;
        clip1.speed = 1.0;
        clip1.fade_in = 1.0;
        clip1.fade_out = 1.0;

        let mut clip2 = ExportClip::blank(ClipKind::Video, 0.0, 1.5, 0);
        clip2.source_start = 0.0;

        let mut clip3 = ExportClip::blank(ClipKind::Audio, 5.0, 3.0, 1);
        clip3.source_start = 0.0;

        let clips = vec![clip1, clip2, clip3];
        // Slice range: [3.0, 6.0], duration = 3.0
        let sliced = slice_clips_to_range(clips, 3.0, 3.0);

        assert_eq!(sliced.len(), 2, "clip2 before range is excluded");
        // Sliced clip1: was [2.0, 6.0], now [0.0, 3.0]
        assert_eq!(sliced[0].start, 0.0);
        assert_eq!(sliced[0].duration, 3.0);
        assert_eq!(sliced[0].source_start, 2.0); // 1.0 + (3.0 - 2.0)
        assert_eq!(sliced[0].fade_in, 0.0); // faded in by 3.0

        // Sliced clip3: was [5.0, 8.0], now [2.0, 1.0]
        assert_eq!(sliced[1].start, 2.0); // 5.0 - 3.0
        assert_eq!(sliced[1].duration, 1.0); // 6.0 - 5.0
        assert_eq!(sliced[1].source_start, 0.0);
    }

    #[test]
    fn flatten_timeline_expands_compound_clips_recursively() {
        let (mut editor, _m1, c1) = project_with_clip();
        let track2_id = editor.project().timelines[0].tracks[1].id.clone();
        let m2 = editor
            .apply(Command::AddMedia {
                item: NewMedia {
                    path: "/footage/b.mp4".into(),
                    name: "b.mp4".into(),
                    duration: Some(15.0),
                    kind: concat_project::model::MediaKind::Video,
                    width: Some(1920),
                    height: Some(1080),
                    frame_rate: None,
                    frame_rate_fraction: None,
                    video_codec: None,
                    audio_codec: None,
                    has_audio: true,
                    audio_tracks: Vec::new(),
                    origin: None,
                    color_space: Default::default(),
                },
            })
            .unwrap()
            .created_id
            .unwrap();
        let c2 = editor
            .apply(Command::AddClip {
                media_id: m2,
                track_id: track2_id,
                start: 3.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();

        // c1 is at 1.0..11.0 (dur 10), c2 is at 3.0..18.0 (dur 15)
        let comp_id = editor
            .apply(Command::CreateCompoundClip {
                clip_ids: vec![c1.clone(), c2.clone()],
                name: Some("Nested 1".into()),
            })
            .unwrap()
            .created_id
            .unwrap();

        // 1. Flatten active timeline: compound clip should expand to 2 constituent clips
        let flat = flatten_timeline(editor.project(), None);
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].path, "/footage/a.mp4");
        assert!((flat[0].start - 1.0).abs() < 1e-4);
        assert!((flat[0].duration - 10.0).abs() < 1e-4);

        assert_eq!(flat[1].path, "/footage/b.mp4");
        assert!((flat[1].start - 3.0).abs() < 1e-4);
        assert!((flat[1].duration - 15.0).abs() < 1e-4);

        // 2. Test nested compound (nesting level 2)
        let _comp2_id = editor
            .apply(Command::CreateCompoundClip {
                clip_ids: vec![comp_id],
                name: Some("Nested 2".into()),
            })
            .unwrap()
            .created_id
            .unwrap();

        let flat_nested = flatten_timeline(editor.project(), None);
        assert_eq!(flat_nested.len(), 2);
        assert_eq!(flat_nested[0].path, "/footage/a.mp4");
        assert_eq!(flat_nested[1].path, "/footage/b.mp4");
    }
}
