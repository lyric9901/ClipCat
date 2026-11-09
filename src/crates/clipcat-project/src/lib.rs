// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

#![allow(clippy::collapsible_if, clippy::collapsible_else_if)]

//! The edit itself, owned by the engine.
//!
//! The model ([`model`]), every operation as a serialisable
//! [`commands::Command`], the tolerant document reader and compatible writer
//! ([`doc`]), and the undo stack ([`editor::Editor`]) - all headless, all
//! testable without a window.
//!
//! Two rules hold throughout:
//!
//! 1. **The document format is frozen by the documents that exist.** Saved
//!    projects load forever, tolerance rules included, because users' work
//!    does not migrate on our schedule. Format changes go through
//!    `DOCUMENT_VERSION`.
//! 2. **f64 seconds and String ids are the document's terms**, kept until a
//!    version 2 decides otherwise on purpose.
//!
//! Deliberately a separate crate rather than part of `concat-core`: the
//! document model needs serde, and concat-core's zero-dependency rule is worth
//! more than the adjacency.

pub mod assets;
pub mod capcut;
pub mod commands;
pub mod doc;
pub mod editor;
pub mod meme_sounds;
pub mod model;
pub mod placement;
pub mod presets;
pub mod runs;
pub mod speed;
pub mod srt;
pub mod workspace;

pub use commands::{
    Command, CommandError, Outcome, PlayheadSide, consolidation_clips, delete_to_playhead,
    why_not_merge,
};
pub use doc::{DOCUMENT_VERSION, DocumentSettings, document_version, from_document, to_document};
pub use editor::{Editor, RetiredFields};
pub use model::Project;

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn typography_preserves_source_and_round_trips() {
        use crate::model::{TextCase, TextStyle};
        let original = "«È UNA PROVA!» POI, UN’ALTRA. già qui? sì!";
        let mut style = TextStyle {
            content: original.into(),
            text_case: TextCase::Sentence,
            underline: true,
            strikethrough: true,
            ..Default::default()
        };
        assert_eq!(
            style.display_content(),
            "«È una prova!» Poi, un’altra. Già qui? Sì!"
        );
        assert_eq!(style.content, original);
        let saved = serde_json::to_value(&style).unwrap();
        let restored: TextStyle = serde_json::from_value(saved).unwrap();
        assert_eq!(restored, style);
        style.text_case = TextCase::Lower;
        assert_eq!(style.display_content(), original.to_lowercase());
        style.text_case = TextCase::Upper;
        assert_eq!(style.display_content(), original.to_uppercase());
        style.text_case = TextCase::Original;
        assert_eq!(style.display_content(), original);
    }

    /// Words styled over a range, and cleared, as undoable steps from the
    /// wire; the title's own look is left alone, and the runs survive a
    /// save and a reopen.
    #[test]
    fn a_range_of_a_titles_words_is_styled_undone_and_saved() {
        use crate::commands::Command;
        use crate::editor::Editor;
        use crate::model::{RunStyle, TextStyle};
        let mut editor = Editor::new();
        let id = editor
            .apply(Command::AddTextClip {
                track_id: None,
                above: true,
                start: 0.0,
                style: Some(TextStyle {
                    content: "make this red".into(),
                    ..TextStyle::default()
                }),
                duration: Some(3.0),
                offset_y: None,
            })
            .unwrap()
            .created_id
            .unwrap();
        let command: Command = serde_json::from_value(json!({
            "op": "styleTextRange", "clipId": id, "start": 10, "end": 13,
            "patch": { "color": "#ff0000", "fontWeight": 800 }
        }))
        .expect("the wire shape parses");
        assert!(editor.apply(command).unwrap().applied);
        let text = |editor: &Editor| {
            editor
                .project()
                .active()
                .clip(&id)
                .unwrap()
                .text
                .clone()
                .unwrap()
        };
        let styled = text(&editor);
        assert_eq!(styled.color, "#ffffff", "the title's own look is untouched");
        assert_eq!(styled.runs.len(), 1);
        assert_eq!((styled.runs[0].start, styled.runs[0].end), (10, 13));
        assert_eq!(styled.runs[0].style.font_weight, Some(800.0));

        let document = editor.to_document(&settings());
        let restored = Editor::from_document(&document).expect("loads");
        assert_eq!(restored.project(), editor.project());

        assert!(editor.undo());
        assert!(text(&editor).runs.is_empty());
        assert!(editor.redo());
        assert!(
            editor
                .apply(Command::ClearTextStyles {
                    clip_id: id.clone(),
                    start: Some(10),
                    end: Some(11),
                })
                .unwrap()
                .applied
        );
        assert_eq!(
            (text(&editor).runs[0].start, text(&editor).runs[0].end),
            (11, 13)
        );
        assert!(
            editor
                .apply(Command::ClearTextStyles {
                    clip_id: id.clone(),
                    start: None,
                    end: None,
                })
                .unwrap()
                .applied
        );
        assert!(text(&editor).runs.is_empty());
        // Nothing to do is nothing done, and not a step to undo.
        assert!(
            !editor
                .apply(Command::StyleTextRange {
                    clip_id: id,
                    start: 4,
                    end: 4,
                    patch: RunStyle {
                        italic: Some(true),
                        ..RunStyle::default()
                    },
                })
                .unwrap()
                .applied
        );
    }

    /// A timeline's canvas colour is written as `#rrggbb` (`#rrggbbaa` when
    /// it is see-through) and read back;
    /// black is not written at all, so a document without one is the
    /// document it always was, and junk reads as black (#456).
    #[test]
    fn a_canvas_colour_is_written_only_when_it_is_not_black() {
        use crate::model::VideoSettings;
        let black = serde_json::to_value(VideoSettings::default()).unwrap();
        assert!(black.get("canvas").is_none(), "{black}");
        let green = VideoSettings {
            canvas: [0, 177, 64, 255],
            ..VideoSettings::default()
        };
        let written = serde_json::to_value(green).unwrap();
        assert_eq!(written["canvas"], "#00b140");
        let read: VideoSettings = serde_json::from_value(written).unwrap();
        assert_eq!(read, green);
        let junk: VideoSettings = serde_json::from_value(json!({
            "width": 1920, "height": 1080, "rateNum": 30, "rateDen": 1, "canvas": "teal"
        }))
        .unwrap();
        assert_eq!(junk.canvas, [0, 0, 0, 255]);
        // A see-through canvas carries its alpha, and an opaque one written
        // with an `ff` alpha reads as the same colour.
        let clear = VideoSettings {
            canvas: [0, 0, 0, 0],
            ..VideoSettings::default()
        };
        let written = serde_json::to_value(clear).unwrap();
        assert_eq!(written["canvas"], "#00000000");
        let read: VideoSettings = serde_json::from_value(written).unwrap();
        assert_eq!(read.canvas, [0, 0, 0, 0]);
        let opaque: VideoSettings = serde_json::from_value(json!({
            "width": 1920, "height": 1080, "rateNum": 30, "rateDen": 1, "canvas": "#00b140ff"
        }))
        .unwrap();
        assert_eq!(opaque.canvas, [0, 177, 64, 255]);
    }

    #[test]
    fn old_styles_keep_their_original_appearance() {
        use crate::model::{TextCase, TextStyle};
        let style: TextStyle = serde_json::from_value(json!({
            "content": "Keep MY Case", "italic": true, "fontWeight": 500
        }))
        .unwrap();
        assert_eq!(style.text_case, TextCase::Original);
        assert_eq!(style.display_content(), "Keep MY Case");
        assert!(!style.underline && !style.strikethrough);
        assert!(style.italic);
        assert_eq!(style.font_weight, 500.0);
    }

    use crate::commands::{
        ClipMove, ClipPatch, Command, NewMedia, PlayheadSide, TrackFlag, TrimEdge,
    };
    use crate::doc::DocumentSettings;
    use crate::editor::Editor;
    use crate::model::{
        AudioTrack, ClipKind, MediaItem, MediaKind, MediaOrigin, Project, TextStyle,
    };

    fn media(path: &str, duration: f64, has_audio: bool) -> Command {
        Command::AddMedia {
            item: crate::commands::NewMedia {
                path: path.to_owned(),
                name: path.rsplit('/').next().unwrap_or(path).to_owned(),
                duration: Some(duration),
                kind: MediaKind::Video,
                width: Some(1920),
                height: Some(1080),
                frame_rate: Some(30.0),
                frame_rate_fraction: Some("30/1".to_owned()),
                video_codec: Some("h264".to_owned()),
                audio_codec: has_audio.then(|| "aac".to_owned()),
                has_audio,
                audio_tracks: Vec::new(),
                origin: None,
                color_space: Default::default(),
            },
        }
    }

    /// A recording that kept its desktop sound and microphone apart: two
    /// audio streams, the second named.
    fn two_track_media(path: &str) -> Command {
        let Command::AddMedia { mut item } = media(path, 10.0, true) else {
            unreachable!()
        };
        item.audio_tracks = vec![
            AudioTrack {
                index: 1,
                codec: "aac".to_owned(),
                channels: 2,
                sample_rate: 48_000,
                title: String::new(),
                language: String::new(),
            },
            AudioTrack {
                index: 2,
                codec: "aac".to_owned(),
                channels: 1,
                sample_rate: 48_000,
                title: "Mic/Aux".to_owned(),
                language: String::new(),
            },
        ];
        Command::AddMedia { item }
    }

    fn settings() -> DocumentSettings {
        DocumentSettings {
            name: "Test".to_owned(),
            width: 1920,
            height: 1080,
            rate_num: 30,
            rate_den: 1,
            canvas: [0, 0, 0, 255],
        }
    }

    /// Over JSON a null crop takes the crop off, as the patch documents:
    /// without `double_option` serde read null as "leave it", so no caller
    /// on the far side of the API could ever remove a crop.
    #[test]
    fn a_null_crop_on_the_wire_takes_the_crop_off() {
        let (mut editor, _, clip_id) = fixture();
        let patch = |crop: serde_json::Value| -> Command {
            serde_json::from_value(json!({
                "op": "updateClip",
                "clipId": clip_id,
                "patch": { "crop": crop }
            }))
            .expect("parses")
        };
        editor
            .apply(patch(
                json!({ "left": 0.2, "top": 0.0, "right": 0.0, "bottom": 0.1 }),
            ))
            .expect("crops");
        assert!(editor.project().active().clips[0].crop.is_some());
        let outcome = editor.apply(patch(json!(null))).expect("uncrops");
        assert!(outcome.applied, "null is take it off, not leave it");
        assert!(editor.project().active().clips[0].crop.is_none());

        let untouched: Command = serde_json::from_value(json!({
            "op": "updateClip",
            "clipId": clip_id,
            "patch": { "opacity": 0.5 }
        }))
        .expect("parses");
        match untouched {
            Command::UpdateClip { patch, .. } => assert_eq!(patch.crop, None),
            _ => panic!("not an update"),
        }
    }

    /// A timeline turns HLG with its first HDR clip, in the same undo step
    /// as the clip; one set back to SDR with HDR clips on it stays SDR; and
    /// a file recorded in HDR that is only in the bin changes nothing.
    #[test]
    fn a_timeline_turns_hdr_with_its_first_hdr_clip() {
        use crate::model::ColorSpace;
        let (mut editor, _, _) = fixture();
        assert!(editor.project().active().video.color_space.is_sdr());
        let Command::AddMedia { mut item } = media("/iphone.mov", 10.0, true) else {
            unreachable!()
        };
        item.color_space = ColorSpace::Hlg;
        let hdr = editor
            .apply(Command::AddMedia { item })
            .expect("adds")
            .created_id
            .expect("id");
        assert!(
            editor.project().active().video.color_space.is_sdr(),
            "in the bin only"
        );
        let track_id = editor.project().active().tracks[0].id.clone();
        let add = |start: f64| Command::AddClip {
            media_id: hdr.clone(),
            track_id: track_id.clone(),
            start,
            ripple: false,
        };
        editor.apply(add(20.0)).expect("adds");
        assert_eq!(editor.project().active().video.color_space, ColorSpace::Hlg);
        assert!(editor.undo());
        assert!(
            editor.project().active().video.color_space.is_sdr()
                && editor.project().active().clips.len() == 1,
            "one step takes both back"
        );
        assert!(editor.redo());
        let timeline_id = editor.project().active_timeline_id.clone();
        let mut video = editor.project().active().video;
        video.color_space = ColorSpace::Sdr;
        editor
            .apply(Command::SetTimelineVideo { timeline_id, video })
            .expect("sets");
        editor.apply(add(40.0)).expect("adds");
        assert!(
            editor.project().active().video.color_space.is_sdr(),
            "set back to SDR, it stays"
        );
    }

    /// A hand-edited rate whose terms would overflow the frame arithmetic
    /// opens at the fallback rather than panicking an export later.
    #[test]
    fn a_rate_no_video_has_opens_at_the_fallback_and_is_refused() {
        use crate::model::VideoSettings;
        let edited = VideoSettings {
            rate_num: i64::MAX,
            rate_den: 7,
            ..VideoSettings::default()
        };
        assert!(!edited.is_sane());
        let opened = edited.or(VideoSettings::default());
        assert_eq!((opened.rate_num, opened.rate_den), (30, 1));
        assert!(opened.is_sane());
        // A side past what one texture holds is refused the same way, and
        // opens at the fallback's size rather than panicking the GPU.
        let vast = VideoSettings {
            width: 10_000,
            height: 1080,
            ..VideoSettings::default()
        };
        assert!(!vast.is_sane());
        let opened = vast.or(VideoSettings::default());
        assert_eq!(
            (opened.width, opened.height),
            (VideoSettings::default().width, 1080)
        );
        assert!(opened.is_sane());
        let edge = VideoSettings {
            width: clipcat_core::frame::MAX_SIDE,
            height: clipcat_core::frame::MAX_SIDE,
            ..VideoSettings::default()
        };
        assert!(edge.is_sane());
        let ntsc = VideoSettings {
            rate_num: 30000,
            rate_den: 1001,
            ..VideoSettings::default()
        };
        assert_eq!(ntsc.or(VideoSettings::default()), ntsc);
    }

    /// A timeline's colour is left out of the document for SDR, kept
    /// through a save and an open for HDR, and read as SDR from a document
    /// naming one this build does not know; a file's likewise.
    #[test]
    fn a_colour_space_round_trips() {
        use crate::model::{ColorSpace, VideoSettings};
        let sdr = serde_json::to_value(VideoSettings::default()).expect("writes");
        assert!(sdr.get("colorSpace").is_none(), "SDR documents unchanged");
        let hlg = VideoSettings {
            color_space: ColorSpace::Hlg,
            ..VideoSettings::default()
        };
        let written = serde_json::to_value(hlg).expect("writes");
        assert_eq!(written["colorSpace"], "hlg");
        assert_eq!(
            serde_json::from_value::<VideoSettings>(written).expect("reads"),
            hlg
        );
        let unknown: VideoSettings = serde_json::from_value(json!({
            "width": 1920, "height": 1080, "rateNum": 30, "rateDen": 1,
            "colorSpace": "dolbyVision"
        }))
        .expect("reads");
        assert!(unknown.color_space.is_sdr());

        let (mut editor, _, _) = fixture();
        let timeline_id = editor.project().active_timeline_id.clone();
        editor
            .apply(Command::SetTimelineVideo {
                timeline_id,
                video: VideoSettings {
                    color_space: ColorSpace::Pq,
                    ..editor.project().active().video
                },
            })
            .expect("sets");
        let Command::AddMedia { mut item } = media("/hdr10.mkv", 5.0, false) else {
            unreachable!()
        };
        item.color_space = ColorSpace::Pq;
        editor.apply(Command::AddMedia { item }).expect("adds");
        let document = crate::to_document(&settings(), editor.project());
        let loaded = Editor::from_document(&document).expect("loads");
        assert_eq!(loaded.project().active().video.color_space, ColorSpace::Pq);
        assert_eq!(loaded.project().media[1].color_space, ColorSpace::Pq);
        assert!(loaded.project().media[0].color_space.is_sdr());
    }

    /// A document from before backgrounds opens with none, on the timeline
    /// and on every clip, and a project that never chooses one writes the
    /// document it always wrote. One that does choose one reads it back,
    /// and a kind from a newer build opens as none rather than failing the
    /// load.
    #[test]
    fn a_background_is_absent_until_chosen_and_round_trips() {
        use crate::model::Background;
        let (mut editor, _, clip_id) = fixture();
        let document = editor.to_document(&settings());
        let timeline = &document["timelines"][0];
        assert!(timeline.get("background").is_none(), "{timeline}");
        assert!(timeline["clips"][0].get("background").is_none());
        let loaded = Editor::from_document(&document).expect("loads");
        assert!(loaded.project().active().background.is_none());
        assert_eq!(loaded.project().active().clips[0].background, None);

        let timeline_id = editor.project().active_timeline_id.clone();
        editor
            .apply(Command::SetTimelineBackground {
                timeline_id: timeline_id.clone(),
                background: Background::Blur { amount: 3.0 },
            })
            .expect("sets");
        assert_eq!(
            editor.project().active().background,
            Background::Blur { amount: 1.0 },
            "held to 0..=1"
        );
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    background: Some(Some(Background::Color {
                        color: "#FF8000".to_owned(),
                    })),
                    ..ClipPatch::default()
                },
            })
            .expect("sets");
        let mut document = editor.to_document(&settings());
        assert_eq!(
            document["timelines"][0]["background"],
            json!({ "kind": "blur", "amount": 1.0 })
        );
        assert_eq!(
            document["timelines"][0]["clips"][0]["background"],
            json!({ "kind": "color", "color": "#ff8000" })
        );
        let loaded = Editor::from_document(&document).expect("loads");
        assert_eq!(loaded.project(), editor.project());

        document["timelines"][0]["background"] = json!({ "kind": "hologram", "depth": 4 });
        document["timelines"][0]["clips"][0]["background"] = json!({ "kind": "hologram" });
        let loaded = Editor::from_document(&document).expect("loads");
        assert!(loaded.project().active().background.is_none());
        assert_eq!(loaded.project().active().clips[0].background, None);

        // Null on the wire is "follow the timeline's" again.
        let follow: Command = serde_json::from_value(json!({
            "op": "updateClip",
            "clipId": clip_id,
            "patch": { "background": null }
        }))
        .expect("parses");
        assert!(editor.apply(follow).expect("applies").applied);
        assert_eq!(editor.project().active().clips[0].background, None);
    }

    /// Only a picture on the lowest lane that holds pictures has a
    /// background: its own over the timeline's, black meaning black. A
    /// title, a picture on a lane above, and a lane of music underneath
    /// change nothing.
    #[test]
    fn the_background_belongs_to_the_lowest_picture_lane() {
        use crate::model::Background;
        let mut editor = Editor::new();
        let video = editor
            .apply(media("/a.mp4", 10.0, true))
            .expect("adds")
            .created_id
            .expect("id");
        let Command::AddMedia { mut item } = media("/song.mp3", 10.0, true) else {
            unreachable!()
        };
        item.kind = MediaKind::Audio;
        item.width = None;
        item.height = None;
        let song = editor
            .apply(Command::AddMedia { item })
            .expect("adds")
            .created_id
            .expect("id");
        let tracks: Vec<String> = editor
            .project()
            .active()
            .tracks
            .iter()
            .map(|track| track.id.clone())
            .collect();
        let add = |editor: &mut Editor, media_id: &str, track: usize| {
            editor
                .apply(Command::AddClip {
                    media_id: media_id.to_owned(),
                    track_id: tracks[track].clone(),
                    start: 0.0,
                    ripple: false,
                })
                .expect("adds")
                .created_id
                .expect("id")
        };
        let music = add(&mut editor, &song, 0);
        let main = add(&mut editor, &video, 1);
        let overlay = add(&mut editor, &video, 2);
        let timeline_id = editor.project().active_timeline_id.clone();
        editor
            .apply(Command::SetTimelineBackground {
                timeline_id,
                background: Background::Blur { amount: 0.5 },
            })
            .expect("sets");

        let timeline = editor.project().active();
        assert_eq!(timeline.background_lane(), Some(tracks[1].as_str()));
        let of = |id: &str| timeline.background_of(timeline.clip(id).expect("a clip"));
        assert_eq!(of(&main), Some(&Background::Blur { amount: 0.5 }));
        assert_eq!(of(&overlay), None);
        assert_eq!(of(&music), None);

        editor
            .apply(Command::UpdateClip {
                clip_id: main.clone(),
                patch: ClipPatch {
                    background: Some(Some(Background::None)),
                    ..ClipPatch::default()
                },
            })
            .expect("sets");
        let timeline = editor.project().active();
        assert_eq!(
            timeline.background_of(timeline.clip(&main).expect("a clip")),
            None,
            "the clip's own black wins over the timeline's blur"
        );
    }

    /// Fit is the size every picture starts at; fill covers the frame. A
    /// wide shot in a tall frame fills at the frame's height over the
    /// picture's fitted height, and both centre and unstretch the picture
    /// as one undo step.
    #[test]
    fn fit_and_fill_size_a_picture_to_the_frame() {
        use crate::model::{Framing, VideoSettings};
        let (mut editor, _, clip_id) = fixture();
        let timeline_id = editor.project().active_timeline_id.clone();
        editor
            .apply(Command::SetTimelineVideo {
                timeline_id,
                video: VideoSettings {
                    width: 1080,
                    height: 1920,
                    ..VideoSettings::default()
                },
            })
            .expect("goes vertical");
        editor
            .apply(Command::SetClipTransform {
                clip_id: clip_id.clone(),
                scale: Some(0.5),
                offset_x: Some(0.2),
                offset_y: None,
                rotation: None,
                stretch_x: Some(1.5),
                stretch_y: None,
            })
            .expect("moves");
        editor.end_gesture();

        let frame = |framing| Command::FrameClip {
            clip_id: clip_id.clone(),
            framing,
            at: 0.0,
        };
        assert!(editor.apply(frame(Framing::Fill)).expect("fills").applied);
        let clip = &editor.project().active().clips[0];
        // 1920x1080 fitted into 1080x1920 is 1080x607.5; covering the
        // frame's 1920 rows takes 1920 / 607.5.
        assert!((clip.scale - 1920.0 / 607.5).abs() < 1e-5, "{}", clip.scale);
        assert_eq!((clip.offset_x, clip.offset_y), (0.0, 0.0));
        assert_eq!((clip.stretch_x, clip.stretch_y), (1.0, 1.0));

        editor.apply(frame(Framing::Fit)).expect("fits");
        assert_eq!(editor.project().active().clips[0].scale, 1.0);

        editor.undo();
        editor.undo();
        let clip = &editor.project().active().clips[0];
        assert_eq!((clip.scale, clip.offset_x, clip.stretch_x), (0.5, 0.2, 1.5));
    }

    /// A scale with keys keeps them: the fill lands as a key where the
    /// playhead is, and the keys either side ride on as they were.
    #[test]
    fn a_fill_on_a_keyed_scale_is_a_key() {
        use crate::model::{Framing, KeyEase, KeyProperty};
        let (mut editor, _, clip_id) = fixture();
        for (at, value) in [(0.0, 1.0), (1.0, 2.0)] {
            editor
                .apply(Command::SetClipKey {
                    clip_id: clip_id.clone(),
                    property: KeyProperty::Scale,
                    at,
                    value,
                    ease: KeyEase::LINEAR,
                })
                .expect("keys");
        }
        editor
            .apply(Command::FrameClip {
                clip_id: clip_id.clone(),
                framing: Framing::Fill,
                at: 0.5,
            })
            .expect("fills");
        let clip = &editor.project().active().clips[0];
        let keys: Vec<(f64, f64)> = clip
            .keys_on(KeyProperty::Scale)
            .map(|key| (key.at, key.value))
            .collect();
        // A 16:9 picture in a 16:9 frame already covers it.
        assert_eq!(keys, vec![(0.0, 1.0), (0.5, 1.0), (1.0, 2.0)]);
        assert_eq!(clip.scale, 1.0, "the constant under the keys is untouched");
    }

    /// A turned picture fills further and fits smaller, and a crop is what
    /// is sized: a picture cropped to a square fills a square frame at one.
    #[test]
    fn framing_follows_the_turn_and_the_crop() {
        use crate::model::{Framing, framing_scale};
        let sixteen_nine = (1920.0, 1080.0);
        let square = (1000.0, 1000.0);
        assert_eq!(
            framing_scale(Framing::Fit, sixteen_nine, square, 0.0),
            Some(1.0)
        );
        assert_eq!(
            framing_scale(Framing::Fill, sixteen_nine, square, 0.0),
            Some(1.777_778)
        );
        // On its side, the 1000x562.5 fitted picture is 562.5 wide: it
        // fits as it was and fills at 1000 / 562.5.
        assert_eq!(
            framing_scale(Framing::Fit, sixteen_nine, square, 90.0),
            Some(1.0)
        );
        assert_eq!(
            framing_scale(Framing::Fill, sixteen_nine, square, 90.0),
            Some(1.777_778)
        );
        let turned = framing_scale(Framing::Fit, square, square, 45.0).expect("a size");
        assert!((turned - 1.0 / 2f64.sqrt()).abs() < 1e-6, "{turned}");
        assert_eq!(framing_scale(Framing::Fill, (0.0, 1.0), square, 0.0), None);

        let mut clip = crate::model::Clip::blank("c", "t", ClipKind::Video, "c", 0.0, 1.0);
        clip.crop = Some(crate::model::Crop {
            left: 0.21875,
            top: 0.0,
            right: 0.21875,
            bottom: 0.0,
        });
        assert_eq!(
            clip.framing_scale(Framing::Fill, (Some(1920), Some(1080)), (1000, 1000), 0.0),
            Some(1.0)
        );
        clip.kind = ClipKind::Text;
        assert_eq!(
            clip.framing_scale(Framing::Fill, (Some(1920), Some(1080)), (1000, 1000), 0.0),
            None
        );
    }

    /// A clip made like another takes on every field but its place, and
    /// the command cannot be sent from outside.
    #[test]
    fn a_matched_clip_takes_everything_but_its_place() {
        let (mut editor, media_id, source_id) = fixture();
        let track_id = editor.project().active().tracks[0].id.clone();
        let mut like = editor
            .project()
            .active()
            .clip(&source_id)
            .expect("clip")
            .clone();
        like.mask = Some(crate::model::ClipMask::of(crate::model::MaskShape::Linear));
        like.stabilize = Some(crate::model::Stabilize::default());
        like.opacity = 0.4;
        like.transition_in = Some(crate::model::Transition {
            id: "concat.dissolve".to_owned(),
            duration: 0.5,
            params: Default::default(),
        });
        like.detached_from = Some("c99".to_owned());
        like.start = 99.0;
        let copy_id = editor
            .apply(Command::AddClip {
                media_id,
                track_id,
                start: 20.0,
                ripple: false,
            })
            .expect("adds")
            .created_id
            .expect("id");
        let applied = editor
            .apply(Command::MatchClip {
                clip_id: copy_id.clone(),
                like: Box::new(like.clone()),
            })
            .expect("matches")
            .applied;
        assert!(applied);
        let copy = editor.project().active().clip(&copy_id).expect("copy");
        assert_eq!(copy.mask, like.mask);
        assert_eq!(copy.stabilize, like.stabilize);
        assert_eq!(copy.opacity, 0.4);
        assert_eq!((copy.id.as_str(), copy.start), (copy_id.as_str(), 20.0));
        assert_eq!(copy.transition_in, None, "its neighbours' business");
        assert_eq!(copy.detached_from, None);

        let wire = serde_json::json!({ "op": "matchClip", "clipId": copy_id, "like": {} });
        assert!(serde_json::from_value::<Command>(wire).is_err());
    }

    /// Editor with one media item and one clip at [0, 10) on track one.
    fn fixture() -> (Editor, String, String) {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/a.mp4", 10.0, true))
            .expect("adds")
            .created_id
            .expect("id");
        let track_id = editor.project().active().tracks[0].id.clone();
        let clip_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id,
                start: 0.0,
                ripple: false,
            })
            .expect("adds")
            .created_id
            .expect("id");
        (editor, media_id, clip_id)
    }

    /// Clips of `media_id`, ten seconds each, at these starts on this
    /// track, in this order. The ids come back in the same order.
    fn lane(editor: &mut Editor, media_id: &str, track_id: &str, starts: &[f64]) -> Vec<String> {
        starts
            .iter()
            .map(|start| {
                editor
                    .apply(Command::AddClip {
                        media_id: media_id.to_owned(),
                        track_id: track_id.to_owned(),
                        start: *start,
                        ripple: false,
                    })
                    .expect("adds")
                    .created_id
                    .expect("id")
            })
            .collect()
    }

    /// Clips placed exactly at these starts, on top of whatever is there:
    /// the stacks an older document can hold, which no command makes now.
    fn stacked(editor: &mut Editor, media_id: &str, track_id: &str, starts: &[f64]) -> Vec<String> {
        let ids = lane(editor, media_id, track_id, starts);
        for (id, start) in ids.iter().zip(starts) {
            editor
                .project_mut()
                .active_mut()
                .clip_mut(id)
                .expect("just added")
                .start = *start;
        }
        ids
    }

    fn start_of(editor: &Editor, clip_id: &str) -> f64 {
        editor
            .project()
            .active()
            .clip(clip_id)
            .expect("the clip is still there")
            .start
    }

    fn move_to(clip_id: &str, start: f64, track_id: &str) -> Command {
        Command::MoveClips {
            moves: vec![ClipMove {
                clip_id: clip_id.to_owned(),
                start,
                track_id: track_id.to_owned(),
            }],
        }
    }

    #[test]
    fn a_move_onto_a_clip_lands_in_the_nearest_gap_beside_it() {
        let (mut editor, media_id, _first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let later = lane(&mut editor, &media_id, &video, &[20.0]);
        // Onto [0, 10) at five: just after it is five away, just before
        // it is before zero, so it lands at ten.
        editor
            .apply(move_to(&later[0], 5.0, &video))
            .expect("moves");
        assert_eq!(start_of(&editor, &later[0]), 10.0);
    }

    #[test]
    fn a_move_into_open_space_lands_where_it_is_dropped() {
        let (mut editor, media_id, _first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let later = lane(&mut editor, &media_id, &video, &[30.0]);
        editor
            .apply(move_to(&later[0], 12.0, &video))
            .expect("moves");
        assert_eq!(start_of(&editor, &later[0]), 12.0);
    }

    #[test]
    fn a_gap_too_small_for_the_clip_is_passed_over() {
        let (mut editor, media_id, _first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        // [0, 10) and [15, 25): the five-second gap cannot hold ten.
        let more = lane(&mut editor, &media_id, &video, &[15.0, 40.0]);
        editor
            .apply(move_to(&more[1], 11.0, &video))
            .expect("moves");
        assert_eq!(start_of(&editor, &more[1]), 25.0);
    }

    #[test]
    fn a_move_onto_another_lane_avoids_what_is_there() {
        let (mut editor, media_id, _first) = fixture();
        let tracks = editor.project().active().tracks.clone();
        let above = lane(&mut editor, &media_id, &tracks[1].id, &[0.0]);
        editor
            .apply(move_to(&above[0], 3.0, &tracks[0].id))
            .expect("moves");
        let clip = editor.project().active().clip(&above[0]).expect("there");
        assert_eq!(clip.track_id, tracks[0].id);
        assert_eq!(clip.start, 10.0);
    }

    #[test]
    fn a_group_keeps_its_shape_and_moves_as_one() {
        let (mut editor, media_id, _first) = fixture();
        let tracks = editor.project().active().tracks.clone();
        let a = lane(&mut editor, &media_id, &tracks[1].id, &[20.0]);
        let b = lane(&mut editor, &media_id, &tracks[2].id, &[25.0]);
        // Both down a lane and back by eighteen: `a` would land on [0, 10)
        // at two, so the pair goes on by eight, keeping five between them.
        editor
            .apply(Command::MoveClips {
                moves: vec![
                    ClipMove {
                        clip_id: a[0].clone(),
                        start: 2.0,
                        track_id: tracks[0].id.clone(),
                    },
                    ClipMove {
                        clip_id: b[0].clone(),
                        start: 7.0,
                        track_id: tracks[1].id.clone(),
                    },
                ],
            })
            .expect("moves");
        assert_eq!(start_of(&editor, &a[0]), 10.0);
        assert_eq!(start_of(&editor, &b[0]), 15.0);
    }

    #[test]
    fn a_move_whose_clips_would_cover_each_other_stays_put() {
        let (mut editor, media_id, _first) = fixture();
        let tracks = editor.project().active().tracks.clone();
        let a = lane(&mut editor, &media_id, &tracks[1].id, &[0.0]);
        let b = lane(&mut editor, &media_id, &tracks[2].id, &[0.0]);
        let outcome = editor
            .apply(Command::MoveClips {
                moves: vec![
                    ClipMove {
                        clip_id: a[0].clone(),
                        start: 40.0,
                        track_id: tracks[3].id.clone(),
                    },
                    ClipMove {
                        clip_id: b[0].clone(),
                        start: 40.0,
                        track_id: tracks[3].id.clone(),
                    },
                ],
            })
            .expect("answers");
        assert!(!outcome.applied);
        let timeline = editor.project().active();
        assert_eq!(timeline.clip(&a[0]).expect("there").track_id, tracks[1].id);
        assert_eq!(timeline.clip(&b[0]).expect("there").track_id, tracks[2].id);
    }

    #[test]
    fn a_clip_added_onto_another_takes_the_nearest_gap() {
        let (mut editor, media_id, _first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let added = lane(&mut editor, &media_id, &video, &[4.0]);
        assert_eq!(start_of(&editor, &added[0]), 10.0);
    }

    #[test]
    fn a_plain_trim_stops_at_the_next_clip() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let next = lane(&mut editor, &media_id, &video, &[12.0]);
        editor
            .apply(trim(&first, TrimEdge::End, 5.0, false))
            .expect("trims");
        let clip = editor.project().active().clip(&first).expect("there");
        assert_eq!(clip.duration, 12.0, "up to the next clip, not past it");
        assert_eq!(start_of(&editor, &next[0]), 12.0);
    }

    #[test]
    fn a_plain_head_trim_stops_at_the_clip_before() {
        let (mut editor, media_id, _first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let next = lane(&mut editor, &media_id, &video, &[12.0]);
        // Twenty seconds of source before the in-point, so the clip before
        // is what stops the head, not the start of the file.
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&next[0])
            .expect("there")
            .source_start = 20.0;
        editor
            .apply(trim(&next[0], TrimEdge::Start, -10.0, false))
            .expect("trims");
        assert_eq!(start_of(&editor, &next[0]), 10.0);
    }

    #[test]
    fn slowing_a_clip_down_moves_what_follows_it() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let next = lane(&mut editor, &media_id, &video, &[10.0]);
        editor
            .apply(Command::SetClipSpeed {
                clip_id: first,
                ripple: false,
                speed: 0.5,
            })
            .expect("retimes");
        assert_eq!(start_of(&editor, &next[0]), 20.0);
    }

    // ── ripple delete: https://github.com/jub0t/Concat/issues/106 ──

    fn unlink(editor: &mut Editor) {
        let timeline_id = editor.project().active().id.clone();
        editor
            .apply(Command::SetTimelineLinkage {
                timeline_id,
                on: false,
            })
            .expect("unlinks");
    }

    fn text_at(editor: &mut Editor, start: f64, duration: f64) -> String {
        editor
            .apply(Command::AddTextClip {
                track_id: None,
                above: true,
                start,
                style: None,
                duration: Some(duration),
                offset_y: None,
            })
            .expect("adds")
            .created_id
            .expect("id")
    }

    /// Linkage: a ripple delete on the main lane pulls the other lanes
    /// with it. What started inside the footage that went lands where the
    /// cut is, what is after it moves back by as much, and words wholly
    /// over the footage that went go with it.
    #[test]
    fn ripple_delete_on_the_main_lane_pulls_the_linked_lanes() {
        let (mut editor, media_id, first) = fixture();
        let (video, sound) = {
            let tracks = &editor.project().active().tracks;
            (tracks[0].id.clone(), tracks[1].id.clone())
        };
        let rest = lane(&mut editor, &media_id, &video, &[10.0, 20.0]);
        let other = lane(&mut editor, &media_id, &sound, &[15.0, 25.0]);
        let over_the_gone = text_at(&mut editor, 12.0, 3.0);
        let over_the_kept = text_at(&mut editor, 22.0, 3.0);
        let across = text_at(&mut editor, 8.0, 4.0);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![rest[0].clone()],
                ripple: true,
            })
            .expect("removes");
        assert_eq!(start_of(&editor, &rest[1]), 10.0, "the main lane closes up");
        assert_eq!(start_of(&editor, &other[0]), 10.0, "started in what went");
        assert_eq!(start_of(&editor, &other[1]), 15.0, "follows its footage");
        assert_eq!(
            start_of(&editor, &over_the_kept),
            12.0,
            "follows its footage"
        );
        assert_eq!(
            start_of(&editor, &across),
            8.0,
            "runs across the cut: stays"
        );
        assert!(
            editor.project().active().clip(&over_the_gone).is_none(),
            "words over footage that went go with it"
        );
        assert_eq!(start_of(&editor, &first), 0.0);
    }

    /// With linkage off a ripple delete closes its own lane and no other.
    #[test]
    fn ripple_delete_closes_the_gap_on_its_own_track_only_unlinked() {
        let (mut editor, media_id, first) = fixture();
        let (video, sound) = {
            let tracks = &editor.project().active().tracks;
            (tracks[0].id.clone(), tracks[1].id.clone())
        };
        let rest = lane(&mut editor, &media_id, &video, &[10.0, 20.0]);
        let other = lane(&mut editor, &media_id, &sound, &[15.0]);
        unlink(&mut editor);

        let outcome = editor
            .apply(Command::RemoveClips {
                clip_ids: vec![rest[0].clone()],
                ripple: true,
            })
            .expect("removes");
        assert!(outcome.applied);
        assert!(editor.project().active().clip(&rest[0]).is_none());
        assert_eq!(start_of(&editor, &first), 0.0, "what was in front stays");
        assert_eq!(
            start_of(&editor, &rest[1]),
            10.0,
            "what was behind closes up"
        );
        assert_eq!(
            start_of(&editor, &other[0]),
            15.0,
            "another lane is none of this deletion's business"
        );
    }

    #[test]
    fn a_plain_delete_still_leaves_the_hole() {
        let (mut editor, media_id, _) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let rest = lane(&mut editor, &media_id, &video, &[10.0, 20.0]);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![rest[0].clone()],
                ripple: false,
            })
            .expect("removes");
        assert_eq!(start_of(&editor, &rest[1]), 20.0);
    }

    #[test]
    fn ripple_delete_of_several_clips_moves_each_survivor_by_what_was_before_it() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let rest = lane(&mut editor, &media_id, &video, &[10.0, 20.0, 30.0]);
        // The first and the third go: the second moves by one span, the
        // fourth by two.
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![first, rest[1].clone()],
                ripple: true,
            })
            .expect("removes");
        assert_eq!(start_of(&editor, &rest[0]), 0.0);
        assert_eq!(start_of(&editor, &rest[2]), 10.0);
        assert_eq!(editor.project().active().clips.len(), 2);
    }

    #[test]
    fn ripple_delete_counts_overlapping_spans_once() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        // Two doomed clips sharing five seconds - [0, 10) and [5, 15) -
        // and a survivor at twenty. Their union is fifteen, so it lands at
        // five; their sum is twenty, which would send it to zero. Away
        // from zero on purpose: the floor there would hide the difference.
        let more = stacked(&mut editor, &media_id, &video, &[5.0, 20.0]);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![first, more[0].clone()],
                ripple: true,
            })
            .expect("removes");
        assert_eq!(start_of(&editor, &more[1]), 5.0);
    }

    #[test]
    fn ripple_delete_of_a_span_reaching_past_a_survivor_pulls_only_up_to_it() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        // A doomed clip at [10, 20) with a survivor stacked inside its span
        // at fourteen: the survivor moves by the four seconds of span in
        // front of it, to ten, not by the whole ten, which would put it at
        // four - in front of where the doomed clip began.
        let more = stacked(&mut editor, &media_id, &video, &[10.0, 14.0]);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![more[0].clone()],
                ripple: true,
            })
            .expect("removes");
        assert_eq!(start_of(&editor, &first), 0.0);
        assert_eq!(start_of(&editor, &more[1]), 10.0);
    }

    #[test]
    fn ripple_delete_leaves_a_clip_stacked_at_the_same_start_alone() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let twin = stacked(&mut editor, &media_id, &video, &[0.0]);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![first],
                ripple: true,
            })
            .expect("removes");
        assert_eq!(
            start_of(&editor, &twin[0]),
            0.0,
            "nothing was in front of it"
        );
    }

    #[test]
    fn ripple_delete_of_unknown_ids_moves_nothing_and_records_nothing() {
        let (mut editor, media_id, _) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let rest = lane(&mut editor, &media_id, &video, &[10.0]);
        let before_can_undo = editor.can_undo();
        let outcome = editor
            .apply(Command::RemoveClips {
                clip_ids: vec!["nobody".to_owned()],
                ripple: true,
            })
            .expect("tolerated");
        assert!(!outcome.applied);
        assert_eq!(start_of(&editor, &rest[0]), 10.0);
        assert_eq!(editor.can_undo(), before_can_undo);
    }

    #[test]
    fn ripple_delete_is_one_undo_step_that_puts_everything_back() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let rest = lane(&mut editor, &media_id, &video, &[10.0, 20.0]);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![first.clone(), rest[0].clone()],
                ripple: true,
            })
            .expect("removes");
        assert_eq!(start_of(&editor, &rest[1]), 0.0);
        assert!(editor.undo());
        assert_eq!(start_of(&editor, &first), 0.0);
        assert_eq!(start_of(&editor, &rest[0]), 10.0);
        assert_eq!(start_of(&editor, &rest[1]), 20.0);
        assert!(editor.redo());
        assert_eq!(start_of(&editor, &rest[1]), 0.0);
    }

    #[test]
    fn ripple_delete_of_everything_on_a_lane_is_just_a_delete() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let rest = lane(&mut editor, &media_id, &video, &[10.0]);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![first, rest[0].clone()],
                ripple: true,
            })
            .expect("removes");
        assert!(editor.project().active().clips.is_empty());
    }

    #[test]
    fn a_remove_clips_document_without_the_ripple_field_still_parses_as_a_plain_delete() {
        let command: Command = serde_json::from_value(json!({
            "op": "removeClips",
            "clipIds": ["c1"]
        }))
        .expect("the field is new; old callers do not send it");
        match command {
            Command::RemoveClips { ripple, .. } => assert!(!ripple),
            _ => panic!("not a remove"),
        }
    }

    // ── magnetic trim: https://github.com/jub0t/Concat/issues/106 ──

    fn trim(clip_id: &str, edge: TrimEdge, delta: f64, ripple: bool) -> Command {
        Command::TrimClip {
            clip_id: clip_id.to_owned(),
            edge,
            delta,
            ripple,
        }
    }

    #[test]
    fn magnetic_tail_trim_moves_the_lane_behind_it_both_ways() {
        let (mut editor, media_id, first) = fixture();
        let (video, sound) = {
            let tracks = &editor.project().active().tracks;
            (tracks[0].id.clone(), tracks[1].id.clone())
        };
        let behind = lane(&mut editor, &media_id, &video, &[10.0, 20.0]);
        let other = lane(&mut editor, &media_id, &sound, &[12.0]);

        editor
            .apply(trim(&first, TrimEdge::End, -2.0, true))
            .expect("shortens");
        assert_eq!(start_of(&editor, &behind[0]), 8.0, "closes up");
        assert_eq!(
            start_of(&editor, &behind[1]),
            18.0,
            "and the one behind that"
        );
        assert_eq!(start_of(&editor, &other[0]), 10.0, "a linked lane follows");

        editor
            .apply(trim(&first, TrimEdge::End, 3.0, true))
            .expect("lengthens");
        assert_eq!(start_of(&editor, &behind[0]), 11.0, "makes room");
        assert_eq!(start_of(&editor, &behind[1]), 21.0);
        assert_eq!(start_of(&editor, &other[0]), 13.0, "and follows back");

        unlink(&mut editor);
        editor
            .apply(trim(&first, TrimEdge::End, -2.0, true))
            .expect("shortens");
        assert_eq!(start_of(&editor, &behind[0]), 9.0);
        assert_eq!(
            start_of(&editor, &other[0]),
            13.0,
            "unlinked, another lane stays"
        );
    }

    /// Only the main lane leads: a ripple on any other lane moves that lane
    /// alone, so deleting a caption never pulls the next ones off their
    /// words, and the picture never moves for a sound.
    #[test]
    fn a_ripple_on_another_lane_leads_nothing() {
        let (mut editor, media_id, first) = fixture();
        let sound = editor.project().active().tracks[1].id.clone();
        let sounds = lane(&mut editor, &media_id, &sound, &[0.0, 10.0]);
        let words = text_at(&mut editor, 12.0, 2.0);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![sounds[0].clone()],
                ripple: true,
            })
            .expect("removes");
        assert_eq!(start_of(&editor, &sounds[1]), 0.0, "its own lane closes");
        assert_eq!(start_of(&editor, &first), 0.0);
        assert_eq!(start_of(&editor, &words), 12.0, "nothing else moves");
    }

    /// A freeze on the main lane opens it, and what is after the freeze on
    /// the other lanes moves along by the hold.
    #[test]
    fn a_freeze_on_the_main_lane_moves_the_linked_lanes_along() {
        let (mut editor, media_id, first) = fixture();
        let sound = editor.project().active().tracks[1].id.clone();
        let later = lane(&mut editor, &media_id, &sound, &[6.0]);
        let earlier = text_at(&mut editor, 1.0, 2.0);
        let still = NewMedia {
            path: "/freeze.jpg".into(),
            name: "freeze.jpg".into(),
            duration: None,
            kind: MediaKind::Image,
            width: Some(1920),
            height: Some(1080),
            frame_rate: None,
            frame_rate_fraction: None,
            video_codec: None,
            audio_codec: None,
            has_audio: false,
            audio_tracks: Vec::new(),
            origin: None,
            color_space: Default::default(),
        };
        editor
            .apply(Command::FreezeFrame {
                clip_id: first.clone(),
                time: 4.0,
                duration: Some(1.0),
                still: Some(still),
            })
            .expect("freezes");
        assert_eq!(start_of(&editor, &later[0]), 7.0, "after the hold: along");
        assert_eq!(start_of(&editor, &earlier), 1.0, "before it: stays");
    }

    /// The document leaves linkage out while it is on, as every timeline
    /// before it was written, and a timeline read without it is linked.
    #[test]
    fn linkage_is_on_unless_written_off() {
        let (mut editor, _, _) = fixture();
        let text = serde_json::to_string(editor.project().active()).expect("writes");
        assert!(!text.contains("linkage"));
        let read: crate::model::Timeline = serde_json::from_str(&text).expect("reads");
        assert!(read.linkage);
        unlink(&mut editor);
        let text = serde_json::to_string(editor.project().active()).expect("writes");
        assert!(text.contains("\"linkage\":false"));
    }

    #[test]
    fn a_plain_trim_still_leaves_the_lane_alone() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let behind = lane(&mut editor, &media_id, &video, &[10.0]);
        editor
            .apply(trim(&first, TrimEdge::End, -2.0, false))
            .expect("shortens");
        assert_eq!(start_of(&editor, &behind[0]), 10.0);
    }

    #[test]
    fn magnetic_head_trim_keeps_the_clip_where_it_was_and_closes_behind_it() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let behind = lane(&mut editor, &media_id, &video, &[10.0]);
        editor
            .apply(trim(&first, TrimEdge::Start, 2.0, true))
            .expect("trims the head");
        let clip = editor
            .project()
            .active()
            .clip(&first)
            .expect("kept")
            .clone();
        assert_eq!(clip.start, 0.0, "the clip does not move");
        assert_eq!(clip.duration, 8.0);
        assert_eq!(clip.source_start, 2.0, "the in-point does");
        assert_eq!(start_of(&editor, &behind[0]), 8.0, "and the lane closes");
    }

    #[test]
    fn a_plain_head_trim_moves_the_head_and_nothing_else() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let behind = lane(&mut editor, &media_id, &video, &[10.0]);
        editor
            .apply(trim(&first, TrimEdge::Start, 2.0, false))
            .expect("trims the head");
        assert_eq!(start_of(&editor, &first), 2.0);
        assert_eq!(start_of(&editor, &behind[0]), 10.0);
    }

    #[test]
    fn magnetic_head_trim_can_reach_back_only_as_far_as_the_source_and_moves_nothing_when_it_cannot()
     {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let behind = lane(&mut editor, &media_id, &video, &[10.0]);
        let before_can_undo = editor.can_undo();
        // In-point at zero: there is nothing before it to restore.
        let outcome = editor
            .apply(trim(&first, TrimEdge::Start, -3.0, true))
            .expect("tolerated");
        assert!(!outcome.applied);
        assert_eq!(start_of(&editor, &behind[0]), 10.0);
        assert_eq!(editor.can_undo(), before_can_undo, "a no-op is not a step");
    }

    #[test]
    fn magnetic_head_trim_restoring_source_pushes_the_lane_back() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let behind = lane(&mut editor, &media_id, &video, &[10.0]);
        editor
            .apply(trim(&first, TrimEdge::Start, 4.0, true))
            .expect("cuts four off the head");
        assert_eq!(start_of(&editor, &behind[0]), 6.0);
        // Ask for five back: only four exist, so four come back.
        editor
            .apply(trim(&first, TrimEdge::Start, -5.0, true))
            .expect("restores what there is");
        let clip = editor
            .project()
            .active()
            .clip(&first)
            .expect("kept")
            .clone();
        assert_eq!(
            clip.start, 0.0,
            "a magnetic clip at zero still does not move"
        );
        assert_eq!(clip.source_start, 0.0);
        assert_eq!(clip.duration, 10.0);
        assert_eq!(
            start_of(&editor, &behind[0]),
            10.0,
            "the lane went back by four"
        );
    }

    #[test]
    fn magnetic_trim_stops_at_the_minimum_and_the_lane_stays_touching() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let behind = lane(&mut editor, &media_id, &video, &[10.0]);
        editor
            .apply(trim(&first, TrimEdge::End, -100.0, true))
            .expect("clamps");
        let clip = editor
            .project()
            .active()
            .clip(&first)
            .expect("kept")
            .clone();
        assert!(
            clip.duration > 0.0 && clip.duration < 1.0,
            "floored at the minimum"
        );
        let gap = start_of(&editor, &behind[0]) - (clip.start + clip.duration);
        assert!(
            gap.abs() < 1e-9,
            "the lane moved by the clamped amount, not the asked one: gap {gap}"
        );
    }

    #[test]
    fn magnetic_trim_leaves_a_clip_in_front_of_the_edge_alone() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        // A clip behind the trimmed one, and one stacked before its tail
        // on the same lane: a tail trim is about what is behind the tail.
        let more = stacked(&mut editor, &media_id, &video, &[10.0, 3.0]);
        editor
            .apply(trim(&first, TrimEdge::End, -2.0, true))
            .expect("shortens");
        assert_eq!(start_of(&editor, &more[0]), 8.0);
        assert_eq!(
            start_of(&editor, &more[1]),
            3.0,
            "in front of the edge; not moved"
        );
    }

    #[test]
    fn magnetic_trim_is_one_undo_step_that_puts_the_lane_back() {
        let (mut editor, media_id, first) = fixture();
        let video = editor.project().active().tracks[0].id.clone();
        let behind = lane(&mut editor, &media_id, &video, &[10.0]);
        editor
            .apply(trim(&first, TrimEdge::Start, 2.0, true))
            .expect("trims");
        assert_eq!(start_of(&editor, &behind[0]), 8.0);
        assert!(editor.undo());
        assert_eq!(start_of(&editor, &behind[0]), 10.0);
        let clip = editor
            .project()
            .active()
            .clip(&first)
            .expect("kept")
            .clone();
        assert_eq!(
            (clip.start, clip.duration, clip.source_start),
            (0.0, 10.0, 0.0)
        );
    }

    #[test]
    fn a_trim_clip_document_without_the_ripple_field_still_parses_as_a_plain_trim() {
        let command: Command = serde_json::from_value(json!({
            "op": "trimClip",
            "clipId": "c1",
            "edge": "end",
            "delta": -1.0
        }))
        .expect("the field is new; old callers do not send it");
        match command {
            Command::TrimClip { ripple, .. } => assert!(!ripple),
            _ => panic!("not a trim"),
        }
    }

    /// A document from before the box had a height reads as no height,
    /// and a hand-edited one cannot make it negative or not a number.
    /// https://github.com/jub0t/Concat/issues/119
    #[test]
    fn a_text_style_without_a_box_height_reads_as_the_words_own() {
        let style: TextStyle =
            serde_json::from_value(json!({ "content": "Hello" })).expect("parses");
        assert_eq!(style.max_height, 0.0);
        let odd = TextStyle {
            max_height: -3.0,
            ..TextStyle::default()
        }
        .tidy();
        assert_eq!(odd.max_height, 0.0);
        let nan = TextStyle {
            max_height: f64::NAN,
            ..TextStyle::default()
        }
        .tidy();
        assert_eq!(nan.max_height, 0.0);
    }

    /// A hand-edited clip time has a ceiling as well as a floor: `1e300`
    /// loaded as written, its frame number saturated, and the export
    /// panicked computing its time (audit 2026-10-09, finding 9).
    #[test]
    fn a_clip_time_past_a_hundred_hours_is_pulled_back() {
        use crate::model::ranges::MAX_TIME;
        let (editor, _, clip_id) = fixture();
        let clip = editor
            .project()
            .active()
            .clips
            .iter()
            .find(|clip| clip.id == clip_id)
            .expect("the clip")
            .as_ref()
            .clone();
        let far = crate::model::Clip {
            start: 1e300,
            duration: f64::INFINITY,
            source_start: 1e300,
            fade_in: 1e300,
            fade_out: 1e300,
            ..clip
        }
        .tidy();
        assert_eq!(far.start, MAX_TIME);
        assert_eq!(far.duration, 1.0, "not a number falls back to a second");
        assert_eq!(far.source_start, MAX_TIME);
        assert_eq!((far.fade_in, far.fade_out), (MAX_TIME, MAX_TIME));
        let long = crate::model::Clip {
            duration: 1e300,
            ..far.clone()
        }
        .tidy();
        assert_eq!(long.duration, MAX_TIME);
    }

    #[test]
    fn a_new_project_has_one_timeline_and_four_lanes() {
        let editor = Editor::new();
        assert_eq!(editor.project().timelines.len(), 1);
        assert_eq!(editor.project().active().tracks.len(), 4);
    }

    #[test]
    fn consolidation_replaces_many_edited_pieces_and_undo_restores_everything() {
        let (mut editor, media_id, first) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let mut ids = vec![first.clone()];
        ids.extend(lane(&mut editor, &media_id, &track, &[10.0, 20.0]));
        for (n, id) in ids.iter().enumerate() {
            let clip = editor.project_mut().active_mut().clip_mut(id).unwrap();
            clip.source_start = n as f64 * 15.0;
            clip.scale = 1.5;
            clip.volume = 0.6;
            clip.speed = 1.2;
        }
        let before = editor.project().clone();
        let Command::AddMedia { mut item } = media("/project/media/merged.mp4", 30.0, true) else {
            unreachable!()
        };
        item.name = "Merged clips".into();
        let command: Command = serde_json::from_value(json!({
            "op": "consolidateClips", "clipIds": ids.iter().rev().collect::<Vec<_>>(), "item": item
        }))
        .expect("the consolidation command is available to API callers too");
        let outcome = editor.apply(command).expect("consolidates");
        assert_eq!(outcome.created_id.as_deref(), Some(first.as_str()));
        let timeline = editor.project().active();
        assert_eq!(timeline.clips.len(), 1);
        let merged = timeline.clip(&first).unwrap();
        assert_eq!(
            (merged.start, merged.duration, merged.source_start),
            (0.0, 30.0, 0.0)
        );
        assert_eq!((merged.scale, merged.volume, merged.speed), (1.0, 1.0, 1.0));
        assert_eq!(
            editor.project().media.len(),
            2,
            "original media stays in the bin"
        );
        let after = editor.project().clone();
        editor.undo();
        assert_eq!(editor.project(), &before);
        editor.redo();
        assert_eq!(editor.project(), &after);
        let document = editor.to_document(&settings());
        assert_eq!(Editor::from_document(&document).unwrap().project(), &after);
    }

    #[test]
    fn consolidation_refuses_invalid_selections_without_changing_the_edit() {
        let (mut editor, media_id, first) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let rest = lane(&mut editor, &media_id, &track, &[10.0, 20.0]);
        let Command::AddMedia { item } = media("/merged.mp4", 30.0, true) else {
            unreachable!()
        };
        for ids in [
            vec![first.clone()],
            vec![first.clone(), first.clone()],
            vec![first.clone(), "gone".into()],
            vec![first.clone(), rest[1].clone()], // Would cover an unselected clip.
        ] {
            let before = editor.project().clone();
            assert!(
                editor
                    .apply(Command::ConsolidateClips {
                        clip_ids: ids,
                        item: item.clone()
                    })
                    .is_err()
            );
            assert_eq!(editor.project(), &before);
        }
        let other = editor.project().active().tracks[1].id.clone();
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&rest[0])
            .unwrap()
            .track_id = other;
        assert!(
            crate::consolidation_clips(
                editor.project().active(),
                &[first.clone(), rest[0].clone()]
            )
            .is_err()
        );
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&rest[0])
            .unwrap()
            .track_id = track;
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&rest[0])
            .unwrap()
            .kind = ClipKind::Audio;
        assert!(
            crate::consolidation_clips(editor.project().active(), &[first, rest[0].clone()])
                .is_err()
        );
    }

    #[test]
    fn consolidation_accepts_different_sources_and_preserves_its_span_and_neighbours() {
        let (mut editor, _, first) = fixture();
        let second_media = editor
            .apply(media("/b.mp4", 10.0, false))
            .unwrap()
            .created_id
            .unwrap();
        let track = editor.project().active().tracks[0].id.clone();
        let others = lane(&mut editor, &second_media, &track, &[12.0, 30.0]);
        let neighbour = editor.project().active().clip(&others[1]).unwrap().clone();
        let Command::AddMedia { item } = media("/merged.mp4", 22.0, true) else {
            unreachable!()
        };
        editor
            .apply(Command::ConsolidateClips {
                clip_ids: vec![others[0].clone(), first.clone()],
                item,
            })
            .unwrap();
        let timeline = editor.project().active();
        assert_eq!(
            timeline.clip(&first).unwrap().duration,
            22.0,
            "timeline gaps remain gaps"
        );
        assert_eq!(timeline.clip(&others[1]).unwrap(), &neighbour);
        assert_eq!(editor.project().media.len(), 3);
    }

    #[test]
    fn consolidation_refuses_a_short_or_non_video_render_atomically() {
        let (mut editor, media_id, first) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let second = lane(&mut editor, &media_id, &track, &[10.0])[0].clone();
        let Command::AddMedia { mut item } = media("/bad.mp4", 1.0, true) else {
            unreachable!()
        };
        let before = editor.project().clone();
        assert!(
            editor
                .apply(Command::ConsolidateClips {
                    clip_ids: vec![first.clone(), second.clone()],
                    item: item.clone()
                })
                .is_err()
        );
        item.duration = Some(20.0);
        item.kind = MediaKind::Audio;
        assert!(
            editor
                .apply(Command::ConsolidateClips {
                    clip_ids: vec![first, second],
                    item
                })
                .is_err()
        );
        assert_eq!(editor.project(), &before);
    }

    #[test]
    fn consolidation_refuses_layered_content_and_detached_audio() {
        let (mut editor, media_id, first) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let second = lane(&mut editor, &media_id, &track, &[10.0])[0].clone();
        let ids = vec![first.clone(), second];
        let original = editor.project().clone();
        let upper = editor.project().active().tracks[1].id.clone();
        for id in &ids {
            editor
                .project_mut()
                .active_mut()
                .clip_mut(id)
                .unwrap()
                .track_id = upper.clone();
        }
        lane(&mut editor, &media_id, &track, &[0.0]);
        assert!(
            crate::consolidation_clips(editor.project().active(), &ids).is_err(),
            "opaque output must not cover lower content"
        );
        *editor.project_mut() = original;
        editor
            .apply(Command::DetachAudio { clip_id: first })
            .unwrap();
        assert!(
            crate::consolidation_clips(editor.project().active(), &ids).is_err(),
            "do not leave detached audio linked to a removed source"
        );
    }

    #[test]
    fn consolidation_requires_both_sides_of_edge_transitions() {
        let (mut editor, media_id, first) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let rest = lane(&mut editor, &media_id, &track, &[10.0, 20.0]);
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&rest[0])
            .unwrap()
            .transition_in = Some(crate::model::Transition {
            id: "cross-fade".into(),
            duration: 0.4,
            params: Default::default(),
        });
        assert!(
            crate::consolidation_clips(
                editor.project().active(),
                &[first.clone(), rest[0].clone()]
            )
            .is_ok(),
            "internal transitions are baked"
        );
        assert!(
            crate::consolidation_clips(editor.project().active(), &rest).is_err(),
            "incoming boundary transition needs its predecessor"
        );
        assert!(
            crate::consolidation_clips(editor.project().active(), &[first, rest[1].clone()])
                .is_err()
        );
    }

    #[test]
    fn consolidation_refuses_subframe_spans_that_would_lose_a_rendered_frame() {
        let (mut editor, media_id, first) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let second = lane(&mut editor, &media_id, &track, &[10.0])[0].clone();
        for (id, start) in [(&first, 5.01), (&second, 5.05)] {
            let clip = editor.project_mut().active_mut().clip_mut(id).unwrap();
            clip.start = start;
            clip.duration = 0.04;
        }
        assert!(crate::consolidation_clips(editor.project().active(), &[first, second]).is_err());
    }

    #[test]
    fn duplicate_media_paths_are_ignored() {
        let mut editor = Editor::new();
        editor.apply(media("/a.mp4", 10.0, true)).expect("adds");
        editor.apply(media("/a.mp4", 10.0, true)).expect("no-op");
        assert_eq!(editor.project().media.len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn a_path_spelled_another_way_is_not_imported_twice_on_windows() {
        let mut editor = Editor::new();
        editor
            .apply(media(r"C:\Videos\a.mp4", 10.0, true))
            .expect("adds");
        editor
            .apply(media("c:/videos/A.mp4", 10.0, true))
            .expect("no-op");
        assert_eq!(editor.project().media.len(), 1);
    }

    #[test]
    fn replace_clip_media_can_move_the_in_point_to_the_copy() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::Start,
                delta: 2.0,
                ripple: false,
            })
            .expect("trims");
        let trimmed = editor.project().active().clip(&clip_id).expect("clip");
        assert_eq!((trimmed.source_start, trimmed.duration), (2.0, 8.0));
        let copy = NewMedia {
            path: "/project/cache/reverse-1-2000-8000.mp4".into(),
            name: "a.mp4 (reversed)".into(),
            duration: Some(8.0),
            kind: MediaKind::Video,
            width: Some(1920),
            height: Some(1080),
            frame_rate: Some(30.0),
            frame_rate_fraction: Some("30/1".into()),
            video_codec: Some("h264".into()),
            audio_codec: None,
            has_audio: false,
            audio_tracks: Vec::new(),
            origin: None,
            color_space: Default::default(),
        };
        let outcome = editor
            .apply(Command::ReplaceClipMedia {
                clip_id: clip_id.clone(),
                item: copy,
                source_start: Some(0.0),
                enhanced: false,
                offset: 0.0,
            })
            .expect("replaces");
        assert!(outcome.applied);
        let clip = editor.project().active().clip(&clip_id).expect("clip");
        assert_eq!(clip.source_start, 0.0, "the copy starts at its own zero");
        assert_eq!(clip.duration, 8.0, "the length is kept");
    }

    #[test]
    fn replace_clip_media_points_the_clip_at_the_copy_and_keeps_the_original() {
        let (mut editor, media_id, clip_id) = fixture();
        let copy = NewMedia {
            path: "/project/cache/enhance-1-2x.mp4".into(),
            name: "a.mp4 (enhanced)".into(),
            duration: Some(10.0),
            kind: MediaKind::Video,
            width: Some(3840),
            height: Some(2160),
            frame_rate: Some(30.0),
            frame_rate_fraction: Some("30/1".into()),
            video_codec: Some("h264".into()),
            audio_codec: Some("aac".into()),
            has_audio: true,
            audio_tracks: Vec::new(),
            origin: None,
            color_space: Default::default(),
        };
        let outcome = editor
            .apply(Command::ReplaceClipMedia {
                clip_id: clip_id.clone(),
                item: copy.clone(),
                source_start: None,
                enhanced: false,
                offset: 0.0,
            })
            .expect("replaces");
        assert!(outcome.applied);
        let copy_id = outcome.created_id.expect("the copy's id");
        assert_ne!(copy_id, media_id);
        let clip = editor.project().active().clip(&clip_id).expect("clip");
        assert_eq!(clip.media_id, copy_id);
        assert_eq!(clip.source_start, 0.0, "the in-point is kept");
        assert_eq!(
            editor.project().media.len(),
            2,
            "the original stays in the bin"
        );
        assert_eq!(
            editor.project().media_by_id(&copy_id).expect("copy").width,
            Some(3840)
        );

        // The same copy again changes nothing and mints nothing.
        let again = editor
            .apply(Command::ReplaceClipMedia {
                clip_id: clip_id.clone(),
                item: copy.clone(),
                source_start: None,
                enhanced: false,
                offset: 0.0,
            })
            .expect("no-op");
        assert!(!again.applied);
        assert_eq!(editor.project().media.len(), 2);

        // A clip nobody has is a no-op that adds nothing to the bin.
        let nobody = editor
            .apply(Command::ReplaceClipMedia {
                clip_id: "c999".into(),
                item: NewMedia {
                    path: "/elsewhere.mp4".into(),
                    ..copy
                },
                source_start: None,
                enhanced: false,
                offset: 0.0,
            })
            .expect("no-op");
        assert!(!nobody.applied);
        assert_eq!(editor.project().media.len(), 2);

        // A non-finite number in the copy's facts is refused, like an import's.
        let bad = NewMedia {
            duration: Some(f64::NAN),
            path: "/nan.mp4".into(),
            ..NewMedia {
                path: String::new(),
                name: String::new(),
                duration: None,
                kind: MediaKind::Video,
                width: None,
                height: None,
                frame_rate: None,
                frame_rate_fraction: None,
                video_codec: None,
                audio_codec: None,
                has_audio: false,
                audio_tracks: Vec::new(),
                origin: None,
                color_space: Default::default(),
            }
        };
        assert!(
            editor
                .apply(Command::ReplaceClipMedia {
                    clip_id: clip_id.clone(),
                    item: bad,
                    source_start: None,
                    enhanced: false,
                    offset: 0.0,
                })
                .is_err()
        );

        // Undo puts the clip back on the original; the copy stays in the
        // bin, harmless, for redo.
        editor.undo();
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clip_id)
                .expect("clip")
                .media_id,
            media_id
        );
    }

    #[test]
    fn freeze_frame_holds_a_second_and_ripples_the_tail() {
        let (mut editor, media_id, clip_id) = fixture();
        let still = NewMedia {
            path: "/freeze.jpg".into(),
            name: "freeze.jpg".into(),
            duration: None,
            kind: MediaKind::Image,
            width: Some(1920),
            height: Some(1080),
            frame_rate: None,
            frame_rate_fraction: None,
            video_codec: None,
            audio_codec: None,
            has_audio: false,
            audio_tracks: Vec::new(),
            origin: None,
            color_space: Default::default(),
        };
        let freeze_id = editor
            .apply(Command::FreezeFrame {
                clip_id: clip_id.clone(),
                time: 4.0,
                duration: Some(1.0),
                still: Some(still),
            })
            .expect("freezes")
            .created_id
            .expect("freeze id");

        let clips = &editor.project().active().clips;
        assert_eq!(clips.len(), 3, "head + freeze + tail");
        let freeze = clips
            .iter()
            .find(|clip| clip.id == freeze_id)
            .expect("freeze");
        assert_eq!(freeze.kind, ClipKind::Image);
        assert_eq!(freeze.start, 4.0);
        assert_eq!(freeze.duration, 1.0);
        let tail = clips
            .iter()
            .find(|clip| clip.id != clip_id && clip.id != freeze_id)
            .expect("tail");
        assert_eq!(tail.start, 5.0, "tail ripples by the hold");
        assert_eq!(tail.source_start, 4.0);
        assert!(
            editor
                .project()
                .media
                .iter()
                .any(|item| item.id != media_id && item.path == "/freeze.jpg"),
            "still is imported"
        );
    }

    /// A freeze cuts the clip the way a split does, so a curved clip has to
    /// come out of it the way a split leaves one: each piece with its share
    /// of the curve, meeting at the frozen source time.
    #[test]
    fn freeze_frame_on_a_curved_clip_keeps_the_pieces_continuous() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SetClipSpeedCurve {
                clip_id: clip_id.clone(),
                ripple: false,
                curve: Some(vec![
                    crate::model::SpeedPoint {
                        at: 0.0,
                        speed: 0.5,
                        slope: None,
                    },
                    crate::model::SpeedPoint {
                        at: 1.0,
                        speed: 2.0,
                        slope: None,
                    },
                ]),
            })
            .expect("curves");
        let freeze_id = editor
            .apply(Command::FreezeFrame {
                clip_id: clip_id.clone(),
                time: 4.0,
                duration: Some(1.0),
                still: Some(NewMedia {
                    path: "/freeze.jpg".into(),
                    name: "freeze.jpg".into(),
                    duration: None,
                    kind: MediaKind::Image,
                    width: Some(1920),
                    height: Some(1080),
                    frame_rate: None,
                    frame_rate_fraction: None,
                    video_codec: None,
                    audio_codec: None,
                    has_audio: false,
                    audio_tracks: Vec::new(),
                    origin: None,
                    color_space: Default::default(),
                }),
            })
            .expect("freezes")
            .created_id
            .expect("freeze id");

        let timeline = editor.project().active();
        let head = timeline.clip(&clip_id).expect("head");
        let tail = timeline
            .clips
            .iter()
            .find(|clip| clip.id != clip_id && clip.id != freeze_id)
            .expect("tail");
        for piece in [head, tail] {
            assert!(piece.speed_curve.is_some(), "a piece lost its ramp");
        }
        assert!(
            (head.source_start + head.duration * head.speed - tail.source_start).abs() < 1e-9,
            "the tail picks up where the head ends"
        );
    }

    /// The fixture's ten-second clip under a ramp from half speed to double.
    fn ramped() -> (Editor, String) {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SetClipSpeedCurve {
                clip_id: clip_id.clone(),
                ripple: false,
                curve: Some(vec![
                    crate::model::SpeedPoint {
                        at: 0.0,
                        speed: 0.5,
                        slope: None,
                    },
                    crate::model::SpeedPoint {
                        at: 1.0,
                        speed: 2.0,
                        slope: None,
                    },
                ]),
            })
            .expect("curves");
        (editor, clip_id)
    }

    /// Source seconds `t` seconds into a placed clip, as the engine maps it.
    fn source_at(clip: &crate::model::Clip, t: f64) -> f64 {
        match clip.speed_curve.as_deref().and_then(crate::speed::curve_of) {
            Some(curve) => clip.source_start + clip.duration * curve.consumed(t / clip.duration),
            None => clip.source_start + t * clip.speed,
        }
    }

    #[test]
    fn a_split_ramp_plays_on_in_both_halves() {
        let (mut editor, clip_id) = ramped();
        let whole = editor.project().active().clip(&clip_id).unwrap().clone();
        let cut = whole.duration * 0.3;
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: cut,
            })
            .expect("splits");
        let clips = &editor.project().active().clips;
        let (head, tail) = (&clips[0], &clips[1]);
        assert!(head.speed_curve.is_some() && tail.speed_curve.is_some());
        for t in [0.0, cut / 2.0, cut] {
            assert!((source_at(head, t) - source_at(&whole, t)).abs() < 1e-9);
        }
        for t in [0.0, 1.0, tail.duration] {
            assert!((source_at(tail, t) - source_at(&whole, cut + t)).abs() < 1e-9);
        }
    }

    #[test]
    fn trimming_a_ramp_cuts_it_rather_than_squeezing_it() {
        let (mut editor, clip_id) = ramped();
        let whole = editor.project().active().clip(&clip_id).unwrap().clone();
        editor
            .apply(Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::Start,
                delta: 2.0,
                ripple: false,
            })
            .expect("trims the head");
        editor
            .apply(Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::End,
                delta: -1.0,
                ripple: false,
            })
            .expect("trims the tail");
        let trimmed = editor.project().active().clip(&clip_id).unwrap();
        assert!((trimmed.start - 2.0).abs() < 1e-9);
        // What is left plays exactly what that stretch played before.
        for t in [0.0, 1.0, trimmed.duration] {
            assert!((source_at(trimmed, t) - source_at(&whole, 2.0 + t)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_ramp_that_slows_a_clip_moves_the_lane_along() {
        let (mut editor, _, clip_id) = fixture();
        let media_id = editor
            .project()
            .active()
            .clip(&clip_id)
            .unwrap()
            .media_id
            .clone();
        let track_id = editor.project().active().tracks[0].id.clone();
        let next = editor
            .apply(Command::AddClip {
                media_id,
                track_id,
                start: 10.0,
                ripple: false,
            })
            .expect("adds")
            .created_id
            .expect("id");
        editor
            .apply(Command::SetClipSpeedCurve {
                clip_id: clip_id.clone(),
                ripple: false,
                curve: Some(vec![
                    crate::model::SpeedPoint {
                        at: 0.0,
                        speed: 1.0,
                        slope: None,
                    },
                    crate::model::SpeedPoint {
                        at: 0.5,
                        speed: 0.25,
                        slope: None,
                    },
                    crate::model::SpeedPoint {
                        at: 1.0,
                        speed: 1.0,
                        slope: None,
                    },
                ]),
            })
            .expect("curves");
        let timeline = editor.project().active();
        let ramped = timeline.clip(&clip_id).unwrap();
        let next = timeline.clip(&next).unwrap();
        assert!(ramped.duration > 10.0, "the slow middle lengthens the clip");
        assert!(
            next.start >= ramped.start + ramped.duration - 1e-9,
            "the clip after it is moved along, not covered"
        );
    }

    #[test]
    fn a_magnetic_speed_up_closes_the_lane_behind_it_and_a_plain_one_leaves_the_gap() {
        for ripple in [true, false] {
            let (mut editor, _, clip_id) = fixture();
            let media_id = editor
                .project()
                .active()
                .clip(&clip_id)
                .unwrap()
                .media_id
                .clone();
            let track_id = editor.project().active().tracks[0].id.clone();
            let mut add = |start| {
                editor
                    .apply(Command::AddClip {
                        media_id: media_id.clone(),
                        track_id: track_id.clone(),
                        start,
                        ripple: false,
                    })
                    .expect("adds")
                    .created_id
                    .expect("id")
            };
            // One clip touching it, and one past a gap of its own.
            let next = add(10.0);
            let later = add(25.0);
            let before = editor.project().clone();
            editor
                .apply(Command::SetClipSpeed {
                    clip_id: clip_id.clone(),
                    ripple,
                    speed: 2.0,
                })
                .expect("speeds up");
            editor
                .apply(Command::SetClipSpeedCurve {
                    clip_id: clip_id.clone(),
                    ripple,
                    curve: Some(vec![
                        crate::model::SpeedPoint {
                            at: 0.0,
                            speed: 4.0,
                            slope: Some(0.0),
                        },
                        crate::model::SpeedPoint {
                            at: 1.0,
                            speed: 1.0,
                            slope: Some(0.0),
                        },
                    ]),
                })
                .expect("curves");
            let timeline = editor.project().active();
            let ramped = timeline.clip(&clip_id).unwrap();
            let end = ramped.start + ramped.duration;
            let (next, later) = (
                timeline.clip(&next).unwrap().start,
                timeline.clip(&later).unwrap().start,
            );
            assert!(end < 5.0, "the clip got shorter");
            if ripple {
                assert!((next - end).abs() < 1e-9, "the next clip closes up");
                assert!(
                    (later - next - 15.0).abs() < 1e-9,
                    "a gap further down the lane is kept"
                );
            } else {
                assert_eq!((next, later), (10.0, 25.0), "a plain retime leaves the gap");
            }
            assert!(editor.undo(), "undoes");
            assert!(editor.undo(), "undoes");
            assert_eq!(editor.project(), &before, "undo puts the lane back");
        }
    }

    #[test]
    fn split_produces_source_continuous_halves_and_merge_rejoins_them() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: 4.0,
            })
            .expect("splits");

        let clips = &editor.project().active().clips;
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].duration, 4.0);
        assert_eq!(clips[1].start, 4.0);
        assert_eq!(clips[1].source_start, 4.0);

        let ids: Vec<String> = clips.iter().map(|clip| clip.id.clone()).collect();
        editor
            .apply(Command::MergeClips { clip_ids: ids })
            .expect("merges");
        let clips = &editor.project().active().clips;
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].duration, 10.0);
        assert_eq!(clips[0].id, clip_id, "the first piece keeps its identity");
    }

    #[test]
    fn a_head_trim_cannot_reach_before_the_source_begins() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::MoveClips {
                moves: vec![crate::commands::ClipMove {
                    clip_id: clip_id.clone(),
                    start: 5.0,
                    track_id: editor.project().active().clips[0].track_id.clone(),
                }],
            })
            .expect("moves");
        // Pulling the head four seconds left would want source -4: the trim
        // stops at the source's start, and the pixels do not slide.
        editor
            .apply(Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::Start,
                delta: -4.0,
                ripple: false,
            })
            .expect("trims");
        let clip = &editor.project().active().clips[0];
        assert_eq!(
            (clip.start, clip.duration, clip.source_start),
            (5.0, 10.0, 0.0)
        );
    }

    #[test]
    fn a_video_whose_detached_sound_was_deleted_can_be_unmuted() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::DetachAudio {
                clip_id: clip_id.clone(),
            })
            .expect("detaches");
        let sound: Vec<String> = editor
            .project()
            .active()
            .clips
            .iter()
            .filter(|clip| clip.id != clip_id)
            .map(|clip| clip.id.clone())
            .collect();
        assert_eq!(sound.len(), 1);
        editor
            .apply(Command::RemoveClips {
                clip_ids: sound,
                ripple: false,
            })
            .expect("removes");
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clip_id)
                .expect("video")
                .muted,
            Some(true)
        );
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    muted: Some(false),
                    ..Default::default()
                },
            })
            .expect("unmutes");
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clip_id)
                .expect("video")
                .muted,
            None,
            "unmuted is the absent value"
        );
    }

    #[test]
    fn a_command_carrying_nan_is_refused_whole() {
        let (mut editor, _, clip_id) = fixture();
        let before = editor.project().clone();
        for command in [
            Command::SetClipSpeed {
                clip_id: clip_id.clone(),
                ripple: false,
                speed: f64::NAN,
            },
            Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: f64::NAN,
            },
            Command::SetClipTransform {
                clip_id: clip_id.clone(),
                scale: None,
                offset_x: Some(f64::INFINITY),
                offset_y: None,
                rotation: None,
                stretch_x: None,
                stretch_y: None,
            },
            Command::Batch {
                commands: vec![Command::TrimClip {
                    clip_id: clip_id.clone(),
                    edge: TrimEdge::End,
                    delta: f64::NAN,
                    ripple: false,
                }],
            },
        ] {
            assert_eq!(
                editor.apply(command).expect_err("refused"),
                crate::CommandError::NotANumber
            );
        }
        assert_eq!(editor.project(), &before, "nothing changed");
    }

    #[test]
    fn nan_in_transition_or_text_patch_is_refused() {
        let (mut editor, _, clip_id) = fixture();
        let before = editor.project().clone();
        // A transition whose duration is NaN must be caught.
        let bad_transition = Command::UpdateClip {
            clip_id: clip_id.clone(),
            patch: ClipPatch {
                transition_in: Some(Some(crate::model::Transition {
                    id: "cross-fade".to_owned(),
                    duration: f64::NAN,
                    params: Default::default(),
                })),
                ..Default::default()
            },
        };
        assert_eq!(
            editor.apply(bad_transition).expect_err("refused"),
            crate::CommandError::NotANumber
        );
        // A text style whose font_size is infinite must be caught.
        let style = TextStyle {
            font_size: f64::INFINITY,
            ..Default::default()
        };
        let bad_text = Command::UpdateClip {
            clip_id: clip_id.clone(),
            patch: ClipPatch {
                text: Some(Some(style)),
                ..Default::default()
            },
        };
        assert_eq!(
            editor.apply(bad_text).expect_err("refused"),
            crate::CommandError::NotANumber
        );
        assert_eq!(editor.project(), &before, "nothing changed");
    }

    #[test]
    fn a_document_from_a_newer_build_is_refused() {
        let (editor, _, _) = fixture();
        let mut document = editor.to_document(&settings());
        document["version"] = serde_json::json!(crate::DOCUMENT_VERSION + 1);
        assert!(Editor::from_document(&document).is_none());
        assert_eq!(
            crate::document_version(&document),
            crate::DOCUMENT_VERSION + 1
        );
        assert_eq!(crate::document_version(&serde_json::json!({})), 1);
    }

    #[test]
    fn a_version_one_document_opens_and_saves_at_the_current_version() {
        let (editor, _, _) = fixture();
        let current = editor.to_document(&settings());
        assert_eq!(crate::document_version(&current), crate::DOCUMENT_VERSION);
        const {
            assert!(
                crate::DOCUMENT_VERSION >= 2,
                "0.2.8 reads version 1, and must refuse what it would lose"
            );
        }
        let mut old = current.clone();
        old["version"] = serde_json::json!(1);
        let loaded = Editor::from_document(&old).expect("a version 1 document opens");
        assert_eq!(loaded.project(), editor.project());
        assert_eq!(
            crate::document_version(&loaded.to_document(&settings())),
            crate::DOCUMENT_VERSION,
            "and is saved at the current version"
        );
    }

    #[test]
    fn a_loaded_document_holds_the_same_ranges_every_command_does() {
        let (editor, _, _) = fixture();
        let mut document = editor.to_document(&settings());
        let clip = &mut document["timelines"][0]["clips"][0];
        clip["duration"] = serde_json::json!(0.001);
        clip["scale"] = serde_json::json!(50.0);
        clip["offsetX"] = serde_json::json!(-9.0);
        clip["rotation"] = serde_json::json!(540.0);
        clip["speed"] = serde_json::json!(100.0);
        let loaded = Editor::from_document(&document).expect("loads");
        let clip = &loaded.project().active().clips[0];
        assert_eq!(clip.duration, 1.0 / 60.0);
        assert_eq!(clip.scale, 8.0);
        assert_eq!(clip.offset_x, -3.0);
        assert_eq!(clip.rotation, 180.0);
        assert_eq!(clip.speed, 16.0);
    }

    #[test]
    fn a_renamed_timeline_does_not_count_towards_the_next_number() {
        let (mut editor, _, _) = fixture();
        editor.apply(Command::AddTimeline).expect("adds");
        let second = editor.project().timelines[1].id.clone();
        editor
            .apply(Command::RenameTimeline {
                timeline_id: second,
                name: "Timeline 2 v10".to_owned(),
            })
            .expect("renames");
        editor.apply(Command::AddTimeline).expect("adds");
        assert_eq!(editor.project().timelines[2].name, "Timeline 2");
    }

    #[test]
    fn a_gesture_is_one_undo_step() {
        let (mut editor, _, clip_id) = fixture();
        let start = editor.project().active().clips[0].scale;
        for scale in [1.1, 1.2, 1.3] {
            editor
                .apply_within(
                    Some("scale"),
                    Command::SetClipTransform {
                        clip_id: clip_id.clone(),
                        scale: Some(scale),
                        offset_x: None,
                        offset_y: None,
                        rotation: None,
                        stretch_x: None,
                        stretch_y: None,
                    },
                )
                .expect("scales");
        }
        editor.end_gesture();
        editor
            .apply_within(
                Some("scale"),
                Command::SetClipTransform {
                    clip_id: clip_id.clone(),
                    scale: Some(2.0),
                    offset_x: None,
                    offset_y: None,
                    rotation: None,
                    stretch_x: None,
                    stretch_y: None,
                },
            )
            .expect("scales again");
        assert_eq!(editor.project().active().clips[0].scale, 2.0);
        // Two steps: the drag, then the second drag after the gesture ended.
        assert!(editor.undo());
        assert_eq!(editor.project().active().clips[0].scale, 1.3);
        assert!(editor.undo());
        assert_eq!(editor.project().active().clips[0].scale, start);
        assert!(editor.redo());
        assert_eq!(editor.project().active().clips[0].scale, 1.3);
    }

    #[test]
    fn switching_and_reordering_tabs_is_not_an_edit() {
        let (mut editor, _, _) = fixture();
        editor.apply(Command::AddTimeline).expect("adds");
        let (first, second) = (
            editor.project().timelines[0].id.clone(),
            editor.project().timelines[1].id.clone(),
        );
        let steps_before = {
            let mut count = 0;
            while editor.undo() {
                count += 1;
            }
            while editor.redo() {}
            count
        };
        editor
            .apply(Command::SelectTimeline {
                timeline_id: first.clone(),
            })
            .expect("selects");
        editor
            .apply(Command::MoveTimeline {
                timeline_id: second.clone(),
                index: 0,
            })
            .expect("moves");
        assert_eq!(editor.project().active_timeline_id, first);
        assert_eq!(editor.project().timelines[0].id, second);
        let mut count = 0;
        while editor.undo() {
            count += 1;
        }
        assert_eq!(
            count, steps_before,
            "neither the switch nor the move was a step"
        );
    }

    /// A ripple delete moves the clips behind the gap and copies those
    /// alone: the clip in front is still the snapshot's.
    #[test]
    fn a_ripple_copies_only_the_clips_it_moves() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: 4.0,
            })
            .expect("splits");
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![editor.project().active().clips[1].id.clone()],
                time: 7.0,
            })
            .expect("splits again");
        let head = std::sync::Arc::clone(&editor.project().active().clips[0]);
        let middle = editor.project().active().clips[1].id.clone();
        let last = std::sync::Arc::clone(&editor.project().active().clips[2]);
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![middle],
                ripple: true,
            })
            .expect("ripple deletes");
        let clips = &editor.project().active().clips;
        assert_eq!(clips.len(), 2);
        assert!(
            std::sync::Arc::ptr_eq(&head, &clips[0]),
            "the clip in front of the gap is still shared with the snapshot"
        );
        assert!(
            !std::sync::Arc::ptr_eq(&last, &clips[1]),
            "the moved clip was copied"
        );
        assert_eq!(clips[1].start, 4.0);
    }

    /// Every command's result goes through `Clip::tidy`: a key set past the
    /// field's range comes out clamped, the way the field itself would.
    #[test]
    fn a_command_leaves_a_tidy_clip_behind() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: crate::model::KeyProperty::OffsetX,
                at: 0.5,
                value: 99.0,
                ease: Default::default(),
            })
            .expect("sets a key");
        let clip = &editor.project().active().clips[0];
        let key = clip
            .keys_on(crate::model::KeyProperty::OffsetX)
            .next()
            .expect("the key");
        assert_eq!(
            key.value,
            crate::model::ranges::MAX_OFFSET,
            "clamped like the field"
        );
    }

    #[test]
    fn a_command_copies_only_what_it_writes() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: 4.0,
            })
            .expect("splits");
        editor.apply(Command::AddTimeline).expect("adds");
        editor
            .apply(Command::SelectTimeline {
                timeline_id: "TL1".to_owned(),
            })
            .expect("back to the first");
        let other = std::sync::Arc::clone(&editor.project().timelines[1]);
        let untouched = std::sync::Arc::clone(&editor.project().active().clips[1]);
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    volume: Some(0.5),
                    ..Default::default()
                },
            })
            .expect("edits the head");
        assert!(
            std::sync::Arc::ptr_eq(&other, &editor.project().timelines[1]),
            "the other timeline is still shared with the snapshot"
        );
        assert!(
            std::sync::Arc::ptr_eq(&untouched, &editor.project().active().clips[1]),
            "the clip the edit did not touch is still shared"
        );
        assert_eq!(editor.project().active().clips[0].volume, 0.5);
        assert!(editor.undo());
        assert_eq!(editor.project().active().clips[0].volume, 1.0);
    }

    #[test]
    fn fields_this_build_does_not_know_survive_a_round_trip() {
        let (editor, _, clip_id) = fixture();
        let mut document = editor.to_document(&settings());
        document["futureSetting"] = json!({ "on": true });
        document["timelines"][0]["colourSpace"] = json!("rec709");
        document["timelines"][0]["tracks"][0]["name"] = json!("Dialogue");
        document["timelines"][0]["clips"][0]["motionTrack"] = json!({ "shape": "ellipse" });
        document["media"][0]["proxy"] = json!("/proxies/a.mp4");

        let loaded = Editor::from_document(&document).expect("loads");
        assert_eq!(
            loaded
                .project()
                .active()
                .clip(&clip_id)
                .expect("clip")
                .extra["motionTrack"]["shape"],
            "ellipse"
        );
        let saved = loaded.to_document(&settings());
        assert_eq!(saved["futureSetting"]["on"], true);
        assert_eq!(saved["timelines"][0]["colourSpace"], "rec709");
        assert_eq!(saved["timelines"][0]["tracks"][0]["name"], "Dialogue");
        assert_eq!(
            saved["timelines"][0]["clips"][0]["motionTrack"]["shape"],
            "ellipse"
        );
        assert_eq!(saved["media"][0]["proxy"], "/proxies/a.mp4");
        // The flat mirror carries the clip's unknown field too: it is the
        // same clip serialised twice.
        assert_eq!(saved["clips"][0]["motionTrack"]["shape"], "ellipse");
        // And the writer's own fields are never duplicated from `extra`.
        assert_eq!(
            loaded.project().extra.len(),
            1,
            "{:?}",
            loaded.project().extra
        );
    }

    #[test]
    fn a_timeline_without_its_own_frame_takes_the_documents() {
        let document = json!({
            "name": "Old", "version": 1,
            "video": { "width": 1080, "height": 1920, "rateNum": 60, "rateDen": 1 },
            "media": [],
            "timelines": [
                { "id": "TL1", "name": "Vertical",
                  "tracks": [{ "id": "T1", "visible": true, "muted": false }], "clips": [] },
                { "id": "TL2", "name": "Half", "video": { "width": 0, "height": 540 },
                  "tracks": [{ "id": "T2", "visible": true, "muted": false }], "clips": [] }
            ],
            "activeTimelineId": "TL2"
        });
        let editor = Editor::from_document(&document).expect("loads");
        let timelines = &editor.project().timelines;
        assert_eq!(
            (timelines[0].video.width, timelines[0].video.height),
            (1080, 1920)
        );
        assert_eq!(timelines[0].video.rate_num, 60);
        // A zero width takes the document's; a stated height is kept.
        assert_eq!(
            (timelines[1].video.width, timelines[1].video.height),
            (1080, 540)
        );
        assert_eq!(editor.project().active_timeline_id, "TL2");
    }

    #[test]
    fn an_entry_the_reader_cannot_parse_is_dropped_not_the_document() {
        let document = json!({
            "name": "Mixed", "version": 1,
            "media": [
                { "id": "m1", "path": "/a.mp4", "kind": "hologram" },
                { "id": "m2", "path": "/b.mp4", "kind": "audio", "audioTracks": "nope" },
                { "path": "/no-id.mp4" }
            ],
            "tracks": [{ "id": "T1" }, { "visible": false }],
            "clips": [
                { "id": "c1", "trackId": "T1", "mediaId": "m1", "kind": "video",
                  "cutout": { "mode": "unknown" }, "keys": "garbage",
                  "transitionIn": { "id": "cross-fade" },
                  "videoEffects": [{ "id": "sepia", "keys": { "amount": [ { "at": 0.5, "value": 1.0 }, { "at": 7.0, "value": 2.0 } ] } }, "not an effect"] },
                { "id": "c2", "trackId": "T1", "mediaId": "m2", "kind": "audio", "start": "soon" }
            ]
        });
        let editor = Editor::from_document(&document).expect("loads");
        let project = editor.project();
        assert_eq!(project.media.len(), 2, "the entry without an id is dropped");
        assert_eq!(
            project.media[0].kind,
            MediaKind::Video,
            "an unknown kind is video"
        );
        assert_eq!(
            project.media[0].name, "/a.mp4",
            "a nameless entry is called by its path"
        );
        assert!(
            project.media[1].audio_tracks.is_empty(),
            "a list that is not one is empty"
        );
        let timeline = project.active();
        assert_eq!(
            timeline.tracks.len(),
            1,
            "the lane without an id is dropped"
        );
        let ids: Vec<&str> = timeline.clips.iter().map(|clip| clip.id.as_str()).collect();
        assert_eq!(ids, ["c1"], "a clip whose start is not a number is dropped");
        let clip = timeline.clip("c1").expect("c1");
        assert!(
            clip.cutout.is_none(),
            "a cutout that does not parse is no cutout"
        );
        assert!(clip.keys.is_empty());
        assert_eq!(clip.transition_in.as_ref().expect("kept").duration, 1.0);
        assert_eq!(clip.video_effects.len(), 1);
        assert_eq!(
            clip.video_effects[0].keys["amount"].len(),
            1,
            "the out-of-range key is dropped"
        );
    }

    /// A key marks an instant of the picture. Cutting the clip around it,
    /// trimming the clip under it and joining the pieces back must leave it
    /// on that instant, and a ride cut in two must play, over each piece,
    /// exactly what the whole played there.
    #[test]
    fn keys_stay_on_their_instant_through_split_trim_and_merge() {
        use crate::model::{KeyEase, KeyProperty};
        let (mut editor, _, clip_id) = fixture();
        // Opacity 0 at the head, 1 at the middle (5 s of a 10 s clip), 0.5 at the end.
        for (at, value) in [(0.0, 0.0), (0.5, 1.0), (1.0, 0.5)] {
            editor
                .apply(Command::SetClipKey {
                    clip_id: clip_id.clone(),
                    property: KeyProperty::Opacity,
                    at,
                    value,
                    ease: KeyEase::LINEAR,
                })
                .expect("keys");
        }
        let whole = editor
            .project()
            .active()
            .clip(&clip_id)
            .expect("clip")
            .clone();
        let ride_at = |seconds: f64| whole.value_at(KeyProperty::Opacity, seconds / whole.duration);

        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: 4.0,
            })
            .expect("splits");
        let timeline = editor.project().active();
        let head = timeline.clips[0].clone();
        let tail = timeline.clips[1].clone();
        // The head's ride over its four seconds is the whole's over 0-4 s;
        // the tail's over its six is the whole's over 4-10 s.
        for seconds in [0.0, 1.0, 2.5, 4.0] {
            let got = head.value_at(KeyProperty::Opacity, seconds / head.duration);
            assert!(
                (got - ride_at(seconds)).abs() < 1e-9,
                "head at {seconds}: {got}"
            );
        }
        for seconds in [4.0, 5.0, 7.0, 10.0] {
            let got = tail.value_at(KeyProperty::Opacity, (seconds - 4.0) / tail.duration);
            assert!(
                (got - ride_at(seconds)).abs() < 1e-9,
                "tail at {seconds}: {got}"
            );
        }
        assert_eq!(
            head.keys.len(),
            2,
            "the head: its own key and one on the cut"
        );
        assert_eq!(
            tail.keys.len(),
            3,
            "the tail: one on the cut and its own two"
        );

        // Trimming a second off the tail's head keeps the middle key on
        // the picture's 5 s: now one second into the tail.
        editor
            .apply(Command::TrimClip {
                clip_id: tail.id.clone(),
                edge: TrimEdge::Start,
                delta: 1.0,
                ripple: false,
            })
            .expect("trims");
        let trimmed = editor
            .project()
            .active()
            .clip(&tail.id)
            .expect("tail")
            .clone();
        let middle = trimmed
            .keys_on(KeyProperty::Opacity)
            .find(|key| (key.value - 1.0).abs() < 1e-9)
            .expect("the middle key");
        assert!(
            (middle.at * trimmed.duration - 0.0).abs() < 1e-9,
            "at {}",
            middle.at
        );
        // And back, so the pieces join again.
        editor
            .apply(Command::TrimClip {
                clip_id: tail.id.clone(),
                edge: TrimEdge::Start,
                delta: -1.0,
                ripple: false,
            })
            .expect("trims back");

        editor
            .apply(Command::MergeClips {
                clip_ids: vec![clip_id.clone(), tail.id.clone()],
            })
            .expect("merges");
        let merged = editor
            .project()
            .active()
            .clip(&clip_id)
            .expect("merged")
            .clone();
        assert_eq!(merged.duration, 10.0);
        for seconds in [0.0, 2.0, 4.0, 5.0, 8.0, 10.0] {
            let got = merged.value_at(KeyProperty::Opacity, seconds / merged.duration);
            assert!(
                (got - ride_at(seconds)).abs() < 1e-9,
                "merged at {seconds}: {got}"
            );
        }

        // Lengthening the end spreads nothing: the keys keep their seconds.
        editor
            .apply(Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::End,
                delta: 10.0,
                ripple: false,
            })
            .expect("extends");
        let longer = editor
            .project()
            .active()
            .clip(&clip_id)
            .expect("clip")
            .clone();
        let middle = longer
            .keys_on(KeyProperty::Opacity)
            .find(|key| (key.value - 1.0).abs() < 1e-9)
            .expect("the middle key");
        assert!((middle.at * longer.duration - 5.0).abs() < 1e-9);
    }

    #[test]
    fn a_split_leaves_the_fade_in_with_the_head_and_the_fade_out_with_the_tail() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    fade_in: Some(0.5),
                    fade_out: Some(0.5),
                    ..Default::default()
                },
            })
            .expect("fades");
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: 4.0,
            })
            .expect("splits");
        let timeline = editor.project().active();
        let (head, tail) = (&timeline.clips[0], &timeline.clips[1]);
        assert_eq!((head.fade_in, head.fade_out), (0.5, 0.0));
        assert_eq!((tail.fade_in, tail.fade_out), (0.0, 0.5));

        let ids: Vec<String> = timeline.clips.iter().map(|clip| clip.id.clone()).collect();
        editor
            .apply(Command::MergeClips { clip_ids: ids })
            .expect("merges");
        let clip = &editor.project().active().clips[0];
        assert_eq!((clip.fade_in, clip.fade_out), (0.5, 0.5));
    }

    #[test]
    fn effect_keys_stay_on_their_instant_through_a_split() {
        use crate::model::{AppliedFilter, KeyEase};
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    video_effects: Some(vec![AppliedFilter::new("concat.vignette")]),
                    ..Default::default()
                },
            })
            .expect("applies");
        for (at, value) in [(0.0, 10.0), (0.8, 90.0)] {
            editor
                .apply(Command::SetEffectKey {
                    clip_id: clip_id.clone(),
                    entry: 0,
                    key: "strength".to_owned(),
                    at,
                    value,
                    ease: KeyEase::LINEAR,
                })
                .expect("keys");
        }
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: 4.0,
            })
            .expect("splits");
        let timeline = editor.project().active();
        let (head, tail) = (&timeline.clips[0], &timeline.clips[1]);
        // The ride is 10 → 90 over 0-8 s: 50 at the cut.
        let at_cut_head = head.video_effects[0].value_at("strength", 1.0, 0.0);
        let at_cut_tail = tail.video_effects[0].value_at("strength", 0.0, 0.0);
        assert!((at_cut_head - 50.0).abs() < 1e-9, "{at_cut_head}");
        assert!((at_cut_tail - 50.0).abs() < 1e-9, "{at_cut_tail}");
        // The tail's second key is still on the picture's 8 s: four seconds in.
        let ninety = tail.video_effects[0]
            .keys_on("strength")
            .iter()
            .find(|key| (key.value - 90.0).abs() < 1e-9)
            .expect("the 90 key");
        assert!((ninety.at * tail.duration - 4.0).abs() < 1e-9);
    }

    #[test]
    fn rearranged_pieces_refuse_to_merge() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: 4.0,
            })
            .expect("splits");
        let tail_id = editor.project().active().clips[1].id.clone();
        let track_id = editor.project().active().tracks[0].id.clone();
        // Swap the two pieces on the timeline: adjacent, but out of order.
        editor
            .apply(Command::MoveClips {
                moves: vec![
                    ClipMove {
                        clip_id: clip_id.clone(),
                        start: 6.0,
                        track_id: track_id.clone(),
                    },
                    ClipMove {
                        clip_id: tail_id.clone(),
                        start: 0.0,
                        track_id,
                    },
                ],
            })
            .expect("moves");
        let refused = editor.apply(Command::MergeClips {
            clip_ids: vec![tail_id, clip_id],
        });
        assert!(
            refused
                .unwrap_err()
                .to_string()
                .contains("no longer in their original order")
        );
    }

    #[test]
    fn a_head_trim_moves_the_in_point_with_speed() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SetClipSpeed {
                clip_id: clip_id.clone(),
                ripple: false,
                speed: 2.0,
            })
            .expect("ok");
        // 10s of source at 2x occupies 5s of timeline.
        assert_eq!(editor.project().active().clips[0].duration, 5.0);
        editor
            .apply(Command::TrimClip {
                clip_id,
                edge: TrimEdge::Start,
                delta: 1.0,
                ripple: false,
            })
            .expect("trims");
        let clip = &editor.project().active().clips[0];
        assert_eq!(clip.start, 1.0);
        assert_eq!(clip.duration, 4.0);
        assert_eq!(
            clip.source_start, 2.0,
            "a timeline second covers two source seconds"
        );
    }

    #[test]
    fn speed_clamps_to_the_engine_range() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SetClipSpeed {
                clip_id: clip_id.clone(),
                ripple: false,
                speed: 100.0,
            })
            .expect("ok");
        assert_eq!(editor.project().active().clips[0].speed, 16.0);
        editor
            .apply(Command::SetClipSpeed {
                clip_id,
                ripple: false,
                speed: 0.0,
            })
            .expect("ok");
        assert_eq!(editor.project().active().clips[0].speed, 0.0625);
    }

    #[test]
    fn detach_and_reattach_round_trip_the_sound() {
        let (mut editor, _, clip_id) = fixture();
        let sound_id = editor
            .apply(Command::DetachAudio {
                clip_id: clip_id.clone(),
            })
            .expect("detaches")
            .created_id
            .expect("sound clip");

        let timeline = editor.project().active();
        assert_eq!(timeline.clips.len(), 2);
        let sound = timeline.clip(&sound_id).expect("exists");
        assert_eq!(sound.kind, ClipKind::Audio);
        assert_eq!(sound.detached_from.as_deref(), Some(clip_id.as_str()));
        assert_eq!(timeline.clip(&clip_id).expect("exists").muted, Some(true));

        editor
            .apply(Command::ReattachAudio { clip_id: sound_id })
            .expect("reattaches");
        let timeline = editor.project().active();
        assert_eq!(timeline.clips.len(), 1);
        assert_eq!(timeline.clip(&clip_id).expect("exists").muted, None);
    }

    /// A file with two audio tracks detaches as two sound clips, one per
    /// track, each on its own lane and named for its track; reattaching
    /// takes both back. A clip's chosen track survives the document.
    #[test]
    fn detaching_a_two_track_recording_gives_one_sound_clip_per_track() {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(two_track_media("/rec.mkv"))
            .expect("adds")
            .created_id
            .expect("id");
        assert_eq!(editor.project().media[0].audio_tracks.len(), 2);
        let track_id = editor.project().active().tracks[0].id.clone();
        let clip_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id,
                start: 0.0,
                ripple: false,
            })
            .expect("adds")
            .created_id
            .expect("id");

        editor
            .apply(Command::DetachAudio {
                clip_id: clip_id.clone(),
            })
            .expect("detaches");
        let timeline = editor.project().active();
        let sounds: Vec<&crate::model::Clip> = timeline
            .clips
            .iter()
            .map(|clip| clip.as_ref())
            .filter(|clip| clip.kind == ClipKind::Audio)
            .collect();
        assert_eq!(sounds.len(), 2);
        assert_eq!(sounds[0].audio_stream, Some(1));
        assert_eq!(sounds[1].audio_stream, Some(2));
        assert!(sounds[0].name.ends_with("Track 1"), "{}", sounds[0].name);
        assert!(sounds[1].name.ends_with("Mic/Aux"), "{}", sounds[1].name);
        assert_ne!(
            sounds[0].track_id, sounds[1].track_id,
            "each on its own lane"
        );
        assert!(
            sounds
                .iter()
                .all(|sound| sound.detached_from.as_deref() == Some(clip_id.as_str()))
        );

        // The document keeps the choice and the list.
        let saved = crate::doc::to_document(&settings(), editor.project());
        let loaded = crate::doc::from_document(&saved).expect("loads");
        assert_eq!(
            loaded.media[0].audio_tracks,
            editor.project().media[0].audio_tracks
        );
        let back: Vec<Option<u32>> = loaded
            .active()
            .clips
            .iter()
            .filter(|clip| clip.kind == ClipKind::Audio)
            .map(|clip| clip.audio_stream)
            .collect();
        assert_eq!(back, vec![Some(1), Some(2)]);
        assert_eq!(loaded.media[0].audio_track_position(Some(2)), 1);
        assert_eq!(loaded.media[0].audio_track_position(None), 0);
        assert_eq!(loaded.media[0].audio_track_position(Some(9)), 0);

        editor
            .apply(Command::ReattachAudio {
                clip_id: sounds[1].id.clone(),
            })
            .expect("reattaches");
        let timeline = editor.project().active();
        assert_eq!(timeline.clips.len(), 1);
        assert_eq!(timeline.clip(&clip_id).expect("exists").muted, None);

        // A video clip can be told which track to play, and told to forget.
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    audio_stream: Some(Some(2)),
                    ..Default::default()
                },
            })
            .expect("picks");
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clip_id)
                .unwrap()
                .audio_stream,
            Some(2)
        );
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    audio_stream: Some(None),
                    ..Default::default()
                },
            })
            .expect("forgets");
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clip_id)
                .unwrap()
                .audio_stream,
            None
        );
    }

    #[test]
    fn timelines_add_switch_and_delete_with_a_floor_of_one() {
        let mut editor = Editor::new();
        let second = editor
            .apply(Command::AddTimeline)
            .expect("adds")
            .created_id
            .expect("id");
        assert_eq!(editor.project().active_timeline_id, second);
        assert_eq!(editor.project().timelines[1].name, "Timeline 2");
        assert!(
            editor.project().timelines[1]
                .tracks
                .iter()
                .all(|track| track.id != "T1"),
            "fresh lanes must not reuse the first timeline's ids"
        );

        editor
            .apply(Command::SelectTimeline {
                timeline_id: "TL1".to_owned(),
            })
            .expect("ok");
        assert_eq!(editor.project().active_timeline_id, "TL1");

        editor
            .apply(Command::RemoveTimeline {
                timeline_id: "TL1".to_owned(),
            })
            .expect("ok");
        assert_eq!(editor.project().active_timeline_id, second);
        let last = editor.project().timelines[0].id.clone();
        let refused = editor.apply(Command::RemoveTimeline { timeline_id: last });
        assert!(refused.is_err(), "the last timeline cannot be deleted");
    }

    #[test]
    fn a_timeline_moves_to_a_new_slot_without_stealing_the_selection() {
        let mut editor = Editor::new();
        let second = editor
            .apply(Command::AddTimeline)
            .expect("adds")
            .created_id
            .expect("id");
        let third = editor
            .apply(Command::AddTimeline)
            .expect("adds")
            .created_id
            .expect("id");
        editor
            .apply(Command::SelectTimeline {
                timeline_id: "TL1".to_owned(),
            })
            .expect("ok");

        editor
            .apply(Command::MoveTimeline {
                timeline_id: third.clone(),
                index: 0,
            })
            .expect("moves");
        let order: Vec<_> = editor
            .project()
            .timelines
            .iter()
            .map(|timeline| timeline.id.clone())
            .collect();
        assert_eq!(order, vec![third.clone(), "TL1".to_owned(), second.clone()]);
        assert_eq!(
            editor.project().active_timeline_id,
            "TL1",
            "moving must not select"
        );

        // An index past the end clamps to the last slot; a same-slot move and
        // an unknown id apply nothing, so neither records history.
        editor
            .apply(Command::MoveTimeline {
                timeline_id: third.clone(),
                index: 99,
            })
            .expect("ok");
        let order: Vec<_> = editor
            .project()
            .timelines
            .iter()
            .map(|timeline| timeline.id.clone())
            .collect();
        assert_eq!(order, vec!["TL1".to_owned(), second, third]);
        let before_can_undo = editor.can_undo();
        let outcome = editor
            .apply(Command::MoveTimeline {
                timeline_id: "nope".to_owned(),
                index: 0,
            })
            .expect("no-op");
        assert!(!outcome.applied);
        assert_eq!(editor.can_undo(), before_can_undo);
    }

    #[test]
    fn undo_and_redo_walk_the_history() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id],
                time: 5.0,
            })
            .expect("splits");
        assert_eq!(editor.project().active().clips.len(), 2);

        assert!(editor.undo());
        assert_eq!(editor.project().active().clips.len(), 1);
        assert!(editor.redo());
        assert_eq!(editor.project().active().clips.len(), 2);
        assert!(editor.undo() && editor.undo() && editor.undo());
        assert!(
            editor.project().media.is_empty(),
            "all the way back to empty"
        );
    }

    #[test]
    fn a_failed_command_records_no_history() {
        let (mut editor, _, _) = fixture();
        let before_can_undo = editor.can_undo();
        let _ = editor.apply(Command::MergeClips {
            clip_ids: vec!["nope".to_owned()],
        });
        assert_eq!(editor.can_undo(), before_can_undo);
    }

    #[test]
    fn the_document_round_trips() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    volume: Some(0.5),
                    transition_in: Some(Some(crate::model::Transition {
                        id: "cross-fade".to_owned(),
                        duration: 1.5,
                        params: Default::default(),
                    })),
                    ..ClipPatch::default()
                },
            })
            .expect("updates");
        editor.apply(Command::AddTimeline).expect("adds");

        let document = editor.to_document(&settings());
        let restored = Editor::from_document(&document).expect("loads");
        assert_eq!(restored.project(), editor.project());
    }

    /// A clip's mask is an edit like any other: put on and taken off by a
    /// patch, held to its ranges, its numbers keyed, and every bit of it
    /// saved and read back as it was.
    #[test]
    fn a_mask_and_its_keys_round_trip() {
        use crate::model::{ClipMask, KeyEase, KeyProperty, MaskShape};
        let (mut editor, _, clip_id) = fixture();
        let mask = ClipMask {
            roundness: 0.4,
            feather: 0.25,
            invert: true,
            rotation: 30.0,
            // Past its range: held to it on the way in.
            width: 9.0,
            ..ClipMask::of(MaskShape::Rectangle)
        };
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    mask: Some(Some(mask)),
                    ..ClipPatch::default()
                },
            })
            .expect("masks");
        for (at, value) in [(0.0, -0.3), (1.0, 0.3)] {
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
        // A key past the feather's range is held to it, as the field is.
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::MaskFeather,
                at: 0.5,
                value: 7.0,
                ease: KeyEase::LINEAR,
            })
            .expect("keys");
        let clip = editor.project().active().clip(&clip_id).expect("clip");
        let held = clip.mask.expect("a mask");
        assert_eq!(held.width, ClipMask::MAX_SIZE);
        assert_eq!(held.shape, MaskShape::Rectangle);
        assert!((clip.value_at(KeyProperty::MaskX, 0.5)).abs() < 1e-12);
        assert!((clip.value_at(KeyProperty::MaskX, 0.75) - 0.15).abs() < 1e-12);
        assert_eq!(clip.value_at(KeyProperty::MaskFeather, 0.5), 1.0);
        // An unkeyed number is the mask's own.
        assert_eq!(clip.value_at(KeyProperty::MaskRoundness, 0.5), 0.4);

        let document = editor.to_document(&settings());
        let written = &document["timelines"][0]["clips"][0];
        assert_eq!(written["mask"]["shape"], "rectangle");
        assert_eq!(written["mask"]["invert"], true);
        assert!(
            written["keys"]
                .as_array()
                .expect("keys")
                .iter()
                .any(|key| key["property"] == "maskX")
        );
        let restored = Editor::from_document(&document).expect("loads");
        assert_eq!(restored.project(), editor.project());

        // Null takes it off and leaves the rest of the clip alone.
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    mask: Some(None),
                    ..ClipPatch::default()
                },
            })
            .expect("unmasks");
        let clip = editor.project().active().clip(&clip_id).expect("clip");
        assert!(clip.mask.is_none());
        assert!(
            !editor.to_document(&settings())["timelines"][0]["clips"][0]
                .as_object()
                .expect("a clip")
                .contains_key("mask")
        );
    }

    /// The wire spelling a caller of the API writes: camelCase numbers, a
    /// lowercase shape, anything left out a new mask's, and a shape this
    /// build has not heard of a circle rather than a failed clip.
    #[test]
    fn a_mask_reads_from_its_wire_spelling() {
        use crate::model::{ClipMask, MaskShape};
        let patch: ClipPatch =
            serde_json::from_value(json!({ "mask": { "shape": "heart", "feather": 0.5 } }))
                .expect("parses");
        let mask = patch.mask.flatten().expect("a mask");
        assert_eq!(mask.shape, MaskShape::Heart);
        assert_eq!(mask.feather, 0.5);
        assert_eq!(mask.width, ClipMask::of(MaskShape::Circle).width);
        let odd: ClipMask =
            serde_json::from_value(json!({ "shape": "blob", "x": 0.2 })).expect("parses");
        assert_eq!((odd.shape, odd.x), (MaskShape::Circle, 0.2));
        let cleared: ClipPatch = serde_json::from_value(json!({ "mask": null })).expect("parses");
        assert_eq!(cleared.mask, Some(None));
        let untouched: ClipPatch = serde_json::from_value(json!({})).expect("parses");
        assert_eq!(untouched.mask, None);
    }

    /// A document from before masks opens with none, and one written by a
    /// build whose mask this one cannot read opens without it rather than
    /// without the clip.
    #[test]
    fn a_document_without_a_mask_opens_maskless() {
        let (editor, _, clip_id) = fixture();
        let mut document = editor.to_document(&settings());
        let restored = Editor::from_document(&document).expect("loads");
        assert!(
            restored
                .project()
                .active()
                .clip(&clip_id)
                .expect("clip")
                .mask
                .is_none()
        );
        document["timelines"][0]["clips"][0]["mask"] = json!("not a mask");
        let restored = Editor::from_document(&document).expect("loads");
        assert!(
            restored
                .project()
                .active()
                .clip(&clip_id)
                .expect("clip")
                .mask
                .is_none()
        );
    }

    #[test]
    fn a_version_one_document_loads() {
        // The shape of a version-1 document as it sits on disk, optional
        // fields omitted - documents like this exist and must load forever.
        let document = json!({
            "concat": "0.1.0", "version": 1, "name": "v1",
            "video": { "width": 1920, "height": 1080, "rateNum": 30, "rateDen": 1 },
            "media": [{ "id": "m1", "path": "/a.mp4", "name": "a.mp4", "duration": 10.0,
                        "kind": "video", "width": 1920, "height": 1080, "frameRate": 30.0,
                        "frameRateFraction": "30/1", "videoCodec": "h264",
                        "audioCodec": null, "hasAudio": false }],
            "tracks": [{ "id": "T1", "name": "Track 1", "visible": true, "muted": false }],
            "clips": [{ "id": "c1", "trackId": "T1", "mediaId": "m1", "name": "a.mp4",
                        "kind": "video", "start": 0.0, "duration": 10.0, "sourceStart": 0.0,
                        "volume": 1.0, "fadeIn": 0.0, "fadeOut": 0.0, "scale": 1.0,
                        "offsetX": 0.0, "offsetY": 0.0, "rotation": 0.0, "opacity": 1.0,
                        "speed": 1.0, "preservePitch": true, "filters": [],
                        "videoEffects": [{ "id": "sepia", "params": {}, "enabled": true }] }],
            "fonts": [],
            "timelines": [{ "id": "TL1", "name": "Timeline 1",
                "tracks": [{ "id": "T1", "name": "Track 1", "visible": true, "muted": false }],
                "clips": [{ "id": "c1", "trackId": "T1", "mediaId": "m1", "name": "a.mp4",
                            "kind": "video", "start": 0.0, "duration": 10.0, "sourceStart": 0.0,
                            "volume": 1.0, "fadeIn": 0.0, "fadeOut": 0.0, "scale": 1.0,
                            "offsetX": 0.0, "offsetY": 0.0, "rotation": 0.0, "opacity": 1.0,
                            "speed": 1.0, "preservePitch": true, "filters": [],
                            "videoEffects": [{ "id": "sepia", "params": {}, "enabled": true }] }] }],
            "activeTimelineId": "TL1"
        });
        let editor = Editor::from_document(&document).expect("loads");
        let project = editor.project();
        assert_eq!(project.timelines.len(), 1);
        assert_eq!(project.active().clips[0].video_effects[0].id, "sepia");
    }

    /// Keys were shipped with four named eases before the curve editor
    /// replaced them with control points. Both spellings have to load, or a
    /// project saved a week ago opens with its animation gone straight.
    #[test]
    fn a_key_reads_its_ease_named_or_numbered() {
        let document = json!({
            "name": "Keyed", "version": 1,
            "media": [{ "id": "m1", "path": "/a.mp4", "name": "a.mp4",
                        "kind": "video", "duration": 10.0 }],
            "tracks": [{ "id": "T1", "name": "Track 1", "visible": true, "muted": false }],
            "clips": [{ "id": "c1", "trackId": "T1", "mediaId": "m1", "name": "a.mp4",
                        "kind": "video", "start": 0.0, "duration": 10.0, "sourceStart": 0.0,
                        "volume": 1.0, "fadeIn": 0.0, "fadeOut": 0.0, "scale": 1.0,
                        "offsetX": 0.0, "offsetY": 0.0, "rotation": 0.0, "opacity": 1.0,
                        "speed": 1.0, "preservePitch": true, "filters": [],
                        "keys": [
                            { "property": "opacity", "at": 0.0, "value": 0.0, "ease": "inOut" },
                            { "property": "opacity", "at": 0.5, "value": 1.0,
                              "ease": [0.1, 0.2, 0.3, 0.4] },
                            { "property": "opacity", "at": 1.0, "value": 0.5 },
                            { "property": "opacity", "at": 9.0, "value": 1.0 },
                            { "property": "nonesuch", "at": 0.2, "value": 1.0 }
                        ] }]
        });
        let editor = Editor::from_document(&document).expect("loads");
        let keys = &editor.project().active().clips[0].keys;

        // The out-of-range key and the one naming an unknown property are
        // dropped; the other three survive in order.
        assert_eq!(keys.len(), 3);
        assert!(keys.windows(2).all(|pair| pair[0].at <= pair[1].at));

        assert_eq!(keys[0].ease, crate::model::KeyEase::IN_OUT);
        assert_eq!(keys[1].ease, crate::model::KeyEase([0.1, 0.2, 0.3, 0.4]));
        // Absent is a straight line, not a refusal to load.
        assert_eq!(keys[2].ease, crate::model::KeyEase::LINEAR);
    }

    #[test]
    fn a_legacy_flat_document_loads_as_one_timeline() {
        let document = json!({
            "name": "Old", "version": 1,
            "media": [],
            "tracks": [{ "id": "T1", "name": "Track 1", "visible": true, "muted": false }],
            "clips": []
        });
        let editor = Editor::from_document(&document).expect("loads");
        assert_eq!(editor.project().timelines.len(), 1);
        assert_eq!(editor.project().timelines[0].name, "Timeline 1");
    }

    #[test]
    fn restored_ids_can_never_be_reissued() {
        let (editor, _, _) = fixture();
        let document = editor.to_document(&settings());
        let mut restored = Editor::from_document(&document).expect("loads");
        let media_id = restored
            .apply(media("/b.mp4", 5.0, false))
            .expect("adds")
            .created_id
            .expect("id");
        assert!(
            restored
                .project()
                .media
                .iter()
                .filter(|item| item.id == media_id)
                .count()
                == 1
                && editor
                    .project()
                    .media
                    .iter()
                    .all(|item| item.id != media_id),
            "the fresh id collides with nothing"
        );
    }

    #[test]
    fn commands_arrive_in_camel_case() {
        // The format the window speaks. A snake_case field here means a
        // caller silently sends values serde never sees.
        let command: Command = serde_json::from_value(json!({
            "op": "addClip", "mediaId": "m1", "trackId": "T1", "start": 2.0
        }))
        .expect("camelCase fields deserialize");
        assert!(matches!(command, Command::AddClip { .. }));

        let transform: Command = serde_json::from_value(json!({
            "op": "setClipTransform", "clipId": "c1", "offsetX": 0.5
        }))
        .expect("optional camelCase fields deserialize");
        assert!(matches!(transform, Command::SetClipTransform { .. }));

        // The template commands ride the same wire.
        let mark: Command = serde_json::from_value(json!({
            "op": "setMediaPlaceholder", "mediaId": "m1", "placeholder": true
        }))
        .expect("parses");
        assert!(matches!(
            mark,
            Command::SetMediaPlaceholder {
                placeholder: true,
                ..
            }
        ));

        // A colour range is one of two words, or null to read the tag.
        let full: Command = serde_json::from_value(json!({
            "op": "setMediaColorRange", "mediaId": "m1", "range": "full"
        }))
        .expect("parses");
        assert!(matches!(
            full,
            Command::SetMediaColorRange {
                range: Some(crate::model::ColorRange::Full),
                ..
            }
        ));
        let tagged: Command = serde_json::from_value(json!({
            "op": "setMediaColorRange", "mediaId": "m1", "range": null
        }))
        .expect("parses");
        assert!(matches!(
            tagged,
            Command::SetMediaColorRange { range: None, .. }
        ));
        assert!(
            serde_json::from_value::<Command>(json!({
                "op": "setMediaColorRange", "mediaId": "m1", "range": "wide"
            }))
            .is_err(),
            "a range is limited or full"
        );

        let batch: Command = serde_json::from_value(json!({
            "op": "batch",
            "commands": [{ "op": "fillSlot", "mediaId": "m1", "item": {
                "path": "/b.mp4", "name": "b.mp4", "duration": 3.0, "kind": "video",
                "width": null, "height": null, "frameRate": null, "frameRateFraction": null,
                "videoCodec": null, "audioCodec": null, "hasAudio": false
            }}]
        }))
        .expect("parses");
        assert!(matches!(
            &batch,
            Command::Batch { commands } if matches!(commands[0], Command::FillSlot { .. })
        ));

        // ClipPatch double-options: absent leaves alone, null clears.
        let patch: crate::commands::ClipPatch =
            serde_json::from_value(json!({ "transitionIn": null })).expect("parses");
        assert_eq!(patch.transition_in, Some(None));
        let untouched: crate::commands::ClipPatch =
            serde_json::from_value(json!({})).expect("parses");
        assert_eq!(untouched.transition_in, None);
    }

    /// A media item's colour range is set, saved, cleared, and undone
    /// like any edit; an unknown item is a no-op that records nothing.
    /// https://github.com/jub0t/Concat/issues/103
    #[test]
    fn a_medias_colour_range_is_an_undoable_edit_that_round_trips() {
        use crate::model::ColorRange;
        let (mut editor, media_id, _clip_id) = fixture();
        assert_eq!(editor.project().media[0].color_range, None);
        let before = editor.project().clone();

        let view = editor
            .apply(Command::SetMediaColorRange {
                media_id: media_id.clone(),
                range: Some(ColorRange::Full),
            })
            .expect("sets");
        assert!(view.applied);
        assert_eq!(
            editor.project().media[0].color_range,
            Some(ColorRange::Full)
        );

        // Saved as the word, read back as the range; absent stays absent,
        // so a project that never said stays byte-identical.
        let saved = editor.to_document(&settings());
        assert_eq!(saved["media"][0]["colorRange"], "full");
        let reopened = Editor::from_document(&saved).expect("loads");
        assert_eq!(
            reopened.project().media[0].color_range,
            Some(ColorRange::Full)
        );
        let untold = crate::doc::to_document(&settings(), &before);
        assert!(untold["media"][0].get("colorRange").is_none());
        // A word this build does not know reads as the tag, not as a
        // media item lost.
        let mut odd = saved.clone();
        odd["media"][0]["colorRange"] = json!("wide");
        let tolerant = Editor::from_document(&odd).expect("loads");
        assert_eq!(tolerant.project().media.len(), 1);
        assert_eq!(tolerant.project().media[0].color_range, None);

        // Cleared is back to the tag; set to what it is already, nothing.
        editor
            .apply(Command::SetMediaColorRange {
                media_id: media_id.clone(),
                range: None,
            })
            .expect("clears");
        assert_eq!(editor.project().media[0].color_range, None);
        let same = editor
            .apply(Command::SetMediaColorRange {
                media_id: media_id.clone(),
                range: None,
            })
            .expect("no-op");
        assert!(!same.applied, "a value already held changes nothing");
        let gone = editor
            .apply(Command::SetMediaColorRange {
                media_id: "m999".to_owned(),
                range: Some(ColorRange::Limited),
            })
            .expect("an unknown item is a no-op");
        assert!(!gone.applied);
        assert_eq!(gone.created_id, None);

        editor.undo();
        assert_eq!(
            editor.project().media[0].color_range,
            Some(ColorRange::Full),
            "undo of the clear brings the range back"
        );
    }

    #[test]
    fn a_slot_fills_in_place_keeping_the_template_timing() {
        let (mut editor, media_id, clip_id) = fixture();
        // Shape the clip the way a template slot looks: trimmed off the front,
        // so it has a real in-point to forget.
        editor
            .apply(Command::TrimClip {
                clip_id,
                edge: TrimEdge::Start,
                delta: 2.0,
                ripple: false,
            })
            .expect("trims");
        editor
            .apply(Command::SetMediaPlaceholder {
                media_id: media_id.clone(),
                placeholder: true,
            })
            .expect("marks");
        assert!(editor.project().media[0].placeholder);

        editor
            .apply(Command::FillSlot {
                media_id: media_id.clone(),
                item: crate::commands::NewMedia {
                    path: "/photo.jpg".to_owned(),
                    name: "photo.jpg".to_owned(),
                    duration: None,
                    kind: MediaKind::Image,
                    width: Some(4000),
                    height: Some(3000),
                    frame_rate: None,
                    frame_rate_fraction: None,
                    video_codec: None,
                    audio_codec: None,
                    has_audio: false,
                    audio_tracks: Vec::new(),
                    origin: None,
                    color_space: Default::default(),
                },
            })
            .expect("fills");

        let project = editor.project();
        let media = &project.media[0];
        assert!(!media.placeholder, "a filled slot is ordinary media again");
        assert_eq!(media.id, media_id, "the id survives, so clips keep working");
        assert_eq!(media.path, "/photo.jpg");

        let clip = &project.active().clips[0];
        assert_eq!(clip.start, 2.0, "slot position is the template's");
        assert_eq!(clip.duration, 8.0, "slot length is the template's");
        assert_eq!(
            clip.source_start, 0.0,
            "the in-point referred to the old footage"
        );
        assert_eq!(clip.kind, ClipKind::Image);
        assert_eq!(clip.name, "photo.jpg");
    }

    #[test]
    fn filling_ordinary_media_is_refused() {
        let (mut editor, media_id, _) = fixture();
        let refused = editor.apply(Command::FillSlot {
            media_id,
            item: crate::commands::NewMedia {
                path: "/b.mp4".to_owned(),
                name: "b.mp4".to_owned(),
                duration: Some(3.0),
                kind: MediaKind::Video,
                width: None,
                height: None,
                frame_rate: None,
                frame_rate_fraction: None,
                video_codec: None,
                audio_codec: None,
                has_audio: false,
                audio_tracks: Vec::new(),
                origin: None,
                color_space: Default::default(),
            },
        });
        assert!(
            refused
                .unwrap_err()
                .to_string()
                .contains("not a template slot")
        );
    }

    #[test]
    fn a_batch_is_one_undo_step() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::Batch {
                commands: vec![
                    Command::SplitClips {
                        clip_ids: vec![clip_id.clone()],
                        time: 4.0,
                    },
                    Command::SetClipSpeed {
                        clip_id,
                        ripple: false,
                        speed: 2.0,
                    },
                ],
            })
            .expect("applies");
        assert_eq!(editor.project().active().clips.len(), 2);
        assert_eq!(editor.project().active().clips[0].speed, 2.0);

        assert!(editor.undo());
        let clips = &editor.project().active().clips;
        assert_eq!(clips.len(), 1, "one undo reverses the whole batch");
        assert_eq!(clips[0].speed, 1.0);
    }

    #[test]
    fn a_failed_batch_changes_nothing() {
        let (mut editor, _, clip_id) = fixture();
        let before = editor.project().clone();
        let refused = editor.apply(Command::Batch {
            commands: vec![
                Command::SplitClips {
                    clip_ids: vec![clip_id],
                    time: 4.0,
                },
                Command::MergeClips {
                    clip_ids: vec!["nope".to_owned()],
                },
            ],
        });
        assert!(refused.is_err());
        assert_eq!(
            editor.project(),
            &before,
            "the successful half was rolled back"
        );

        assert!(editor.undo());
        assert!(
            editor.project().active().clips.is_empty(),
            "the first undo steps over the fixture's add-clip, so the batch recorded nothing"
        );
    }

    #[test]
    fn the_placeholder_flag_round_trips_and_defaults_off() {
        let (mut editor, media_id, _) = fixture();
        editor
            .apply(Command::SetMediaPlaceholder {
                media_id,
                placeholder: true,
            })
            .expect("marks");

        let document = editor.to_document(&settings());
        let restored = Editor::from_document(&document).expect("loads");
        assert!(
            restored.project().media[0].placeholder,
            "the flag survives the document"
        );

        // A document from a build that predates templates has no such field.
        let legacy = json!({
            "name": "Old", "version": 1,
            "media": [{ "id": "m1", "path": "/a.mp4", "name": "a.mp4", "kind": "video",
                        "hasAudio": false }],
            "tracks": [{ "id": "T1", "name": "Track 1", "visible": true, "muted": false }],
            "clips": []
        });
        let editor = Editor::from_document(&legacy).expect("loads");
        assert!(!editor.project().media[0].placeholder);
    }

    #[test]
    fn a_medias_origin_round_trips_and_an_unknown_one_reads_as_an_import() {
        let mut editor = Editor::new();
        let Command::AddMedia { mut item } = media("/voice.wav", 3.0, true) else {
            unreachable!()
        };
        item.kind = MediaKind::Audio;
        item.origin = Some(MediaOrigin::Speech);
        editor.apply(Command::AddMedia { item }).expect("adds");
        editor.apply(media("/a.mp4", 10.0, true)).expect("adds");

        let document = editor.to_document(&settings());
        let media = document["media"].as_array().expect("a list");
        assert_eq!(media[0]["origin"], json!("speech"));
        assert!(
            media[1].get("origin").is_none(),
            "an import says nothing, so a document without voices stays byte-identical"
        );
        let restored = Editor::from_document(&document).expect("loads");
        assert_eq!(
            restored.project().media[0].origin,
            Some(MediaOrigin::Speech)
        );
        assert_eq!(restored.project().media[1].origin, None);

        // A document from a build that predates origins, and one from a
        // build with origins this one has not heard of: both load, and
        // both files are imports here.
        let legacy = json!({
            "name": "Old", "version": 1,
            "media": [
                { "id": "m1", "path": "/a.mp4", "name": "a.mp4", "kind": "video",
                  "hasAudio": false },
                { "id": "m2", "path": "/b.wav", "name": "b.wav", "kind": "audio",
                  "hasAudio": true, "origin": "telepathy" },
                { "id": "m3", "path": "/c.wav", "name": "c.wav", "kind": "audio",
                  "hasAudio": true, "origin": 7 }
            ],
            "tracks": [{ "id": "T1", "name": "Track 1", "visible": true, "muted": false }],
            "clips": []
        });
        let editor = Editor::from_document(&legacy).expect("loads");
        assert!(
            editor
                .project()
                .media
                .iter()
                .all(|item| item.origin.is_none())
        );
    }

    #[test]
    fn a_garbage_document_is_none_not_a_panic() {
        assert!(Editor::from_document(&json!("nonsense")).is_none());
        assert!(Editor::from_document(&json!({ "tracks": [] })).is_none());
    }

    #[test]
    fn text_clips_survive_without_media_and_carry_their_words() {
        let mut editor = Editor::new();
        let clip_id = editor
            .apply(Command::AddTextClip {
                above: false,
                track_id: None,
                start: 2.0,
                style: Some(TextStyle {
                    content: "Lower third\nsecond line".to_owned(),
                    ..TextStyle::default()
                }),
                duration: Some(2.5),
                offset_y: Some(0.38),
            })
            .expect("adds")
            .created_id
            .expect("id");
        {
            let clip = editor.project().active().clip(&clip_id).expect("exists");
            assert_eq!(clip.name, "Lower third");
            assert_eq!(clip.duration, 2.5, "a stated duration lands directly");
            assert_eq!(clip.offset_y, 0.38, "and so does the placement");
        }

        // The pre-templates wire shape still parses: both fields default.
        let bare: Command = serde_json::from_value(json!({
            "op": "addTextClip", "trackId": null, "start": 1.0, "style": null
        }))
        .expect("old wire shape parses");
        assert!(matches!(
            bare,
            Command::AddTextClip {
                duration: None,
                offset_y: None,
                ..
            }
        ));

        let document = editor.to_document(&settings());
        let restored = Editor::from_document(&document).expect("loads");
        assert_eq!(restored.project(), editor.project());
    }

    #[test]
    fn a_caption_lands_above_the_highest_occupied_lane() {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/a.mp4", 10.0, true))
            .expect("adds")
            .created_id
            .expect("id");
        let video_track = editor.project().active().tracks[2].id.clone();
        editor
            .apply(Command::AddClip {
                media_id,
                track_id: video_track.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("adds");

        let caption_id = editor
            .apply(Command::AddTextClip {
                track_id: None,
                above: true,
                start: 0.0,
                style: Some(TextStyle {
                    content: "hi".to_owned(),
                    ..TextStyle::default()
                }),
                duration: Some(5.0),
                offset_y: None,
            })
            .expect("adds")
            .created_id
            .expect("id");

        let caption = editor.project().active().clip(&caption_id).expect("exists");
        let tracks = &editor.project().active().tracks;
        let caption_row = tracks
            .iter()
            .position(|t| t.id == caption.track_id)
            .expect("row");
        let video_row = tracks
            .iter()
            .position(|t| t.id == video_track)
            .expect("row");
        assert!(
            caption_row > video_row,
            "a caption must land above the video, not under it"
        );
    }

    /// An adjustment layer laid at the playhead lands over the footage,
    /// with the colour panel's package as its chain at its defaults: a
    /// layer under the picture would treat nothing (#249).
    #[test]
    fn an_adjustment_layer_lands_above_the_picture() {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/a.mp4", 10.0, true))
            .expect("adds")
            .created_id
            .expect("id");
        let video_track = editor.project().active().tracks[2].id.clone();
        editor
            .apply(Command::AddClip {
                media_id,
                track_id: video_track.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("adds");
        let layer_id = editor
            .apply(Command::AddLayerClip {
                track_id: None,
                start: 1.0,
                duration: Some(3.0),
                effect_id: "concat.adjust".into(),
                name: "Adjustment".into(),
                sound: false,
            })
            .expect("adds")
            .created_id
            .expect("id");
        let timeline = editor.project().active();
        let layer = timeline.clip(&layer_id).expect("exists");
        assert_eq!(layer.kind, ClipKind::Layer);
        assert_eq!(layer.name, "Adjustment");
        assert_eq!(layer.video_effects.len(), 1);
        assert_eq!(layer.video_effects[0].id, "concat.adjust");
        assert!(layer.video_effects[0].params.is_empty(), "at its defaults");
        let row = |id: &str| timeline.tracks.iter().position(|t| t.id == id);
        assert!(
            row(&layer.track_id) > row(&video_track),
            "the layer must sit over the footage, not under it"
        );
    }

    /// A transition placed as a clip (#621) goes over the footage like a
    /// layer, carries its package as its one link, is a second long unless
    /// told otherwise, and survives the document.
    #[test]
    fn a_transition_clip_goes_over_the_footage_and_survives_the_document() {
        let (mut editor, _, footage) = fixture();
        let video_track = editor
            .project()
            .active()
            .clip(&footage)
            .expect("the footage")
            .track_id
            .clone();
        let id = editor
            .apply(Command::AddTransitionClip {
                track_id: None,
                start: 1.5,
                duration: None,
                transition_id: "concat.push".into(),
                name: "Push".into(),
            })
            .expect("placed")
            .created_id
            .expect("id");
        let timeline = editor.project().active();
        let clip = timeline.clip(&id).expect("exists");
        assert_eq!(clip.kind, ClipKind::Transition);
        assert_eq!(clip.name, "Push");
        assert!((clip.duration - 1.0).abs() < 1e-9, "{}", clip.duration);
        assert_eq!(clip.video_effects.len(), 1);
        assert_eq!(clip.video_effects[0].id, "concat.push");
        assert!(clip.media_id.is_empty());
        let row = |id: &str| timeline.tracks.iter().position(|t| t.id == id);
        assert!(
            row(&clip.track_id) > row(&video_track),
            "a transition acts on what is beneath it"
        );

        let document = editor.to_document(&settings());
        assert_eq!(
            document["timelines"][0]["clips"]
                .as_array()
                .expect("clips")
                .iter()
                .find(|clip| clip["id"] == id.as_str())
                .expect("written")["kind"],
            "transition"
        );
        let loaded = Editor::from_document(&document).expect("loads");
        assert_eq!(loaded.project(), editor.project());

        // Never shorter than a tenth of a second, and nothing that is not a
        // number.
        let short = editor
            .apply(Command::AddTransitionClip {
                track_id: None,
                start: 4.0,
                duration: Some(0.0),
                transition_id: "concat.cross-fade".into(),
                name: String::new(),
            })
            .expect("placed")
            .created_id
            .expect("id");
        let short = editor.project().active().clip(&short).expect("exists");
        assert!(short.duration >= crate::model::ranges::MIN_TRANSITION - 1e-9);
        assert_eq!(short.name, "concat.cross-fade", "named for its package");
        assert!(
            editor
                .apply(Command::AddTransitionClip {
                    track_id: None,
                    start: f64::NAN,
                    duration: None,
                    transition_id: "concat.push".into(),
                    name: String::new(),
                })
                .is_err()
        );
    }

    /// A sound layer carries its package on the sound's chain and none on
    /// the picture's, lands over the footage as a picture layer does, is
    /// written as a kind of its own and read back as one, and goes with an
    /// undo.
    #[test]
    fn a_sound_layer_carries_an_audio_chain_and_round_trips() {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/a.mp4", 10.0, true))
            .expect("adds")
            .created_id
            .expect("id");
        let video_track = editor.project().active().tracks[2].id.clone();
        editor
            .apply(Command::AddClip {
                media_id,
                track_id: video_track.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("adds");
        let layer_id = editor
            .apply(Command::AddLayerClip {
                track_id: None,
                start: 1.0,
                duration: Some(3.0),
                effect_id: "concat.telephone".into(),
                name: "Telephone".into(),
                sound: true,
            })
            .expect("adds")
            .created_id
            .expect("id");
        let timeline = editor.project().active();
        let layer = timeline.clip(&layer_id).expect("exists");
        assert_eq!(layer.kind, ClipKind::SoundLayer);
        assert_eq!(layer.media_id, "");
        assert_eq!(layer.filters.len(), 1);
        assert_eq!(layer.filters[0].id, "concat.telephone");
        assert!(layer.video_effects.is_empty(), "nothing on the picture");
        let row = |id: &str| timeline.tracks.iter().position(|t| t.id == id);
        assert!(row(&layer.track_id) > row(&video_track));

        let document = editor.to_document(&settings());
        let written = document["timelines"][0]["clips"]
            .as_array()
            .expect("clips")
            .iter()
            .find(|clip| clip["id"] == layer_id.as_str())
            .expect("written");
        assert_eq!(written["kind"], "soundLayer");
        let reopened = Editor::from_document(&document).expect("reads");
        assert_eq!(
            reopened.project().active().clip(&layer_id),
            editor.project().active().clip(&layer_id),
            "read back as it was written"
        );

        // A copy is laid as a layer and then made like the source, the way
        // the window copies, pastes and duplicates one: kind, chain,
        // strength and ramps all come with it.
        let mut source = editor
            .project()
            .active()
            .clip(&layer_id)
            .expect("exists")
            .clone();
        source.filters[0].params.insert("drive".to_owned(), 6.0);
        source.opacity = 0.5;
        source.fade_in = 0.25;
        let copy_id = editor
            .apply(Command::AddLayerClip {
                track_id: Some(source.track_id.clone()),
                start: 4.0,
                duration: Some(source.duration),
                effect_id: source.filters[0].id.clone(),
                name: source.name.clone(),
                sound: true,
            })
            .expect("adds")
            .created_id
            .expect("id");
        editor
            .apply(Command::MatchClip {
                clip_id: copy_id.clone(),
                like: Box::new(source.clone()),
            })
            .expect("matches");
        let copy = editor.project().active().clip(&copy_id).expect("exists");
        assert_eq!(copy.kind, ClipKind::SoundLayer);
        assert_eq!(copy.filters, source.filters);
        assert_eq!((copy.opacity, copy.fade_in), (0.5, 0.25));
        assert_eq!(copy.start, 4.0);

        while editor.project().active().clip(&layer_id).is_some() {
            assert!(editor.undo());
        }
        assert!(editor.project().active().clip(&copy_id).is_none());
    }

    #[test]
    fn a_caption_on_an_empty_timeline_lands_on_the_bottom_lane() {
        let mut editor = Editor::new();
        let caption_id = editor
            .apply(Command::AddTextClip {
                track_id: None,
                above: true,
                start: 0.0,
                style: Some(TextStyle {
                    content: "hi".to_owned(),
                    ..TextStyle::default()
                }),
                duration: Some(5.0),
                offset_y: None,
            })
            .expect("adds")
            .created_id
            .expect("id");

        let caption = editor.project().active().clip(&caption_id).expect("exists");
        let tracks = &editor.project().active().tracks;
        let caption_row = tracks
            .iter()
            .position(|t| t.id == caption.track_id)
            .expect("row");
        assert_eq!(
            caption_row, 0,
            "with nothing occupied, the caption takes the bottom lane"
        );
    }

    #[test]
    fn a_plain_title_still_lands_on_the_first_free_lane_from_the_bottom() {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/a.mp4", 10.0, true))
            .expect("adds")
            .created_id
            .expect("id");
        let video_track = editor.project().active().tracks[2].id.clone();
        editor
            .apply(Command::AddClip {
                media_id,
                track_id: video_track,
                start: 0.0,
                ripple: false,
            })
            .expect("adds");

        let title_id = editor
            .apply(Command::AddTextClip {
                track_id: None,
                above: false,
                start: 0.0,
                style: Some(TextStyle {
                    content: "title".to_owned(),
                    ..TextStyle::default()
                }),
                duration: Some(5.0),
                offset_y: None,
            })
            .expect("adds")
            .created_id
            .expect("id");

        let title = editor.project().active().clip(&title_id).expect("exists");
        let tracks = &editor.project().active().tracks;
        let title_row = tracks
            .iter()
            .position(|t| t.id == title.track_id)
            .expect("row");
        assert_eq!(
            title_row, 0,
            "a plain title keeps the old bottom-first behaviour"
        );
    }

    #[test]
    fn a_caption_mints_a_new_lane_when_every_lane_above_is_taken() {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/a.mp4", 10.0, true))
            .expect("adds")
            .created_id
            .expect("id");
        let track_ids: Vec<String> = editor
            .project()
            .active()
            .tracks
            .iter()
            .map(|t| t.id.clone())
            .collect();
        for track_id in &track_ids {
            editor
                .apply(Command::AddClip {
                    media_id: media_id.clone(),
                    track_id: track_id.clone(),
                    start: 0.0,
                    ripple: false,
                })
                .expect("adds");
        }

        let caption_id = editor
            .apply(Command::AddTextClip {
                track_id: None,
                above: true,
                start: 0.0,
                style: Some(TextStyle {
                    content: "hi".to_owned(),
                    ..TextStyle::default()
                }),
                duration: Some(5.0),
                offset_y: None,
            })
            .expect("adds")
            .created_id
            .expect("id");

        let caption = editor.project().active().clip(&caption_id).expect("exists");
        let tracks = &editor.project().active().tracks;
        assert_eq!(
            tracks.len(),
            track_ids.len() + 1,
            "one lane was minted at the top for the caption"
        );
        let caption_row = tracks
            .iter()
            .position(|t| t.id == caption.track_id)
            .expect("row");
        assert_eq!(
            caption_row,
            track_ids.len(),
            "the caption sits on the newly minted top lane"
        );
    }

    #[test]
    fn a_tolerated_no_op_records_no_undo_entry() {
        // The applied flag replaced a deep state compare; if a missing-id
        // command ever reported true, undo would gain phantom steps.
        let (mut editor, _, _) = fixture();
        let outcome = editor
            .apply(Command::TrimClip {
                clip_id: "nope".to_owned(),
                edge: TrimEdge::End,
                delta: 1.0,
                ripple: false,
            })
            .expect("tolerated");
        assert!(!outcome.applied);

        assert!(editor.undo(), "history still holds the fixture's edits");
        assert_eq!(
            editor.project().active().clips.len(),
            0,
            "the first undo steps over the add-clip, not a phantom no-op"
        );
    }

    #[test]
    fn setting_a_value_already_in_place_records_no_undo_entry() {
        let (mut editor, _, clip_id) = fixture();
        // Volume is already 1.0; the old deep-compare saw no change here and
        // pushed nothing - the applied flag must agree exactly.
        let outcome = editor
            .apply(Command::UpdateClip {
                clip_id,
                patch: ClipPatch {
                    volume: Some(1.0),
                    ..ClipPatch::default()
                },
            })
            .expect("tolerated");
        assert!(!outcome.applied);
        assert!(editor.undo());
        assert!(
            editor.project().active().clips.is_empty(),
            "straight back past the add-clip"
        );
    }

    #[test]
    fn a_batch_reports_applied_only_when_a_member_changed_something() {
        let (mut editor, _, clip_id) = fixture();
        let track_id = editor.project().active().tracks[0].id.clone();
        // Visible is already true; only the speed change is real.
        let outcome = editor
            .apply(Command::Batch {
                commands: vec![
                    Command::SetTrackFlag {
                        track_id: track_id.clone(),
                        flag: TrackFlag::Visible,
                        value: true,
                    },
                    Command::SetClipSpeed {
                        clip_id,
                        ripple: false,
                        speed: 2.0,
                    },
                ],
            })
            .expect("applies");
        assert!(outcome.applied);
        assert!(editor.undo());
        assert_eq!(
            editor.project().active().clips[0].speed,
            1.0,
            "one undo undoes the batch"
        );

        // A batch of nothing-but-no-ops is itself a no-op: no undo entry.
        let outcome = editor
            .apply(Command::Batch {
                commands: vec![
                    Command::SetTrackFlag {
                        track_id,
                        flag: TrackFlag::Visible,
                        value: true,
                    },
                    Command::TrimClip {
                        clip_id: "nope".to_owned(),
                        edge: TrimEdge::End,
                        delta: 1.0,
                        ripple: false,
                    },
                ],
            })
            .expect("tolerated");
        assert!(!outcome.applied);
        assert!(editor.undo());
        assert!(
            editor.project().active().clips.is_empty(),
            "the next undo steps over the add-clip, not a phantom batch"
        );
    }

    #[test]
    fn undo_depth_evicts_the_oldest_snapshot_first() {
        // The cap used to remove(0) on a Vec; the VecDeque must keep the
        // same outward behaviour - the newest 200 steps stay undoable.
        let (mut editor, _, clip_id) = fixture();
        for step in 0..205 {
            editor
                .apply(Command::SetClipTransform {
                    clip_id: clip_id.clone(),
                    scale: None,
                    offset_x: Some(f64::from(step) / 1000.0 + 0.001),
                    offset_y: None,
                    rotation: None,
                    stretch_x: None,
                    stretch_y: None,
                })
                .expect("applies");
        }
        let mut undos = 0;
        while editor.undo() {
            undos += 1;
        }
        assert_eq!(undos, 200);
    }

    #[test]
    fn moves_to_a_vanished_track_keep_the_time_change_and_drop_the_track_change() {
        // Pinned deliberately: a drag that races a track deletion still
        // lands its horizontal half.
        let (mut editor, _, clip_id) = fixture();
        let outcome = editor
            .apply(Command::MoveClips {
                moves: vec![ClipMove {
                    clip_id: clip_id.clone(),
                    start: 3.0,
                    track_id: "gone".to_owned(),
                }],
            })
            .expect("tolerated");
        assert!(outcome.applied, "the start change is real");
        let clip = editor.project().active().clip(&clip_id).expect("exists");
        assert_eq!(clip.start, 3.0);
        assert_eq!(clip.track_id, "T1", "the vanished destination is ignored");
    }

    #[test]
    fn a_multi_move_skips_unknown_clips_and_floors_start_at_zero() {
        let (mut editor, _, clip_id) = fixture();
        let target = editor.project().active().tracks[1].id.clone();
        editor
            .apply(Command::MoveClips {
                moves: vec![
                    // The unknown clip is skipped; the rest still lands.
                    ClipMove {
                        clip_id: "nope".to_owned(),
                        start: 9.0,
                        track_id: target.clone(),
                    },
                    ClipMove {
                        clip_id: clip_id.clone(),
                        start: -2.0,
                        track_id: target.clone(),
                    },
                ],
            })
            .expect("tolerated");
        let clip = editor.project().active().clip(&clip_id).expect("exists");
        assert_eq!(
            clip.start, 0.0,
            "negative starts clamp to the timeline head"
        );
        assert_eq!(clip.track_id, target);
    }

    #[test]
    fn a_tail_trim_stretches_only_the_duration_and_stops_at_the_minimum() {
        let (mut editor, _, clip_id) = fixture();
        editor
            .apply(Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::End,
                delta: -4.0,
                ripple: false,
            })
            .expect("trims");
        let clip = editor.project().active().clip(&clip_id).expect("exists");
        assert_eq!(clip.duration, 6.0);
        assert_eq!(clip.start, 0.0, "the head does not move");
        assert_eq!(clip.source_start, 0.0, "nor the in-point");

        // Dragging far past the head floors at a sixtieth of a second
        // rather than inverting the clip.
        editor
            .apply(Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::End,
                delta: -100.0,
                ripple: false,
            })
            .expect("trims");
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clip_id)
                .expect("exists")
                .duration,
            1.0 / 60.0
        );
    }

    #[test]
    fn a_sequence_lands_end_to_end_on_one_lane_and_undoes_as_one() {
        let (mut editor, media_id, _) = fixture();
        let before = editor.project().active().clips.len();
        let first = editor
            .apply(Command::AddClipsInSequence {
                media_ids: vec![media_id.clone(), media_id.clone(), media_id.clone()],
                track_id: None,
                start: 20.0,
                ripple: false,
            })
            .expect("adds")
            .created_id
            .expect("id");
        let timeline = editor.project().active();
        let lane = timeline.clip(&first).expect("exists").track_id.clone();
        let mut run: Vec<_> = timeline
            .clips
            .iter()
            .filter(|clip| clip.start >= 20.0)
            .collect();
        run.sort_by(|a, b| a.start.total_cmp(&b.start));
        assert_eq!(run.len(), 3);
        assert!((run[0].start - 20.0).abs() < 1e-9);
        for pair in run.windows(2) {
            assert_eq!(pair[0].track_id, lane, "one lane");
            assert!(
                (pair[1].start - (pair[0].start + pair[0].duration)).abs() < 1e-9,
                "each starts where the one before ends"
            );
        }
        assert!(editor.undo(), "undoes");
        assert_eq!(
            editor.project().active().clips.len(),
            before,
            "one undo step"
        );
    }

    #[test]
    fn add_at_first_free_takes_the_lowest_empty_lane_or_opens_one() {
        let (mut editor, media_id, _) = fixture();
        // Track one is occupied for [0, 10), so the same span lands on two.
        let second = editor
            .apply(Command::AddClipAtFirstFree {
                media_id: media_id.clone(),
                start: 0.0,
            })
            .expect("adds")
            .created_id
            .expect("id");
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&second)
                .expect("exists")
                .track_id,
            editor.project().active().tracks[1].id
        );

        // Fill the remaining lanes, then ask again: rather than covering a
        // clip, the new one gets a lane of its own at the top.
        editor
            .apply(Command::AddClipAtFirstFree {
                media_id: media_id.clone(),
                start: 0.0,
            })
            .expect("adds");
        editor
            .apply(Command::AddClipAtFirstFree {
                media_id: media_id.clone(),
                start: 0.0,
            })
            .expect("adds");
        let overflow = editor
            .apply(Command::AddClipAtFirstFree {
                media_id: media_id.clone(),
                start: 0.0,
            })
            .expect("adds")
            .created_id
            .expect("id");
        let timeline = editor.project().active();
        assert_eq!(timeline.tracks.len(), 5, "a fifth lane was opened");
        assert_eq!(
            timeline.clip(&overflow).expect("exists").track_id,
            timeline.tracks[4].id,
            "a full timeline opens a lane rather than covering a clip"
        );

        // A clear span later in time finds track one free again.
        let clear = editor
            .apply(Command::AddClipAtFirstFree {
                media_id,
                start: 20.0,
            })
            .expect("adds")
            .created_id
            .expect("id");
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clear)
                .expect("exists")
                .track_id,
            editor.project().active().tracks[0].id
        );
    }

    #[test]
    fn removing_a_track_takes_its_clips_and_stops_at_the_floor_of_one() {
        let (mut editor, _, _) = fixture();
        let track_id = editor.project().active().tracks[0].id.clone();
        editor
            .apply(Command::RemoveTrack {
                track_id: track_id.clone(),
            })
            .expect("removes");
        let timeline = editor.project().active();
        assert_eq!(timeline.tracks.len(), 3);
        assert!(
            timeline.clips.is_empty(),
            "the clip on the removed track went with it"
        );

        // An unknown id is the tolerated no-op, not an error.
        let outcome = editor
            .apply(Command::RemoveTrack { track_id })
            .expect("tolerated");
        assert!(!outcome.applied);

        while editor.project().active().tracks.len() > 1 {
            let next = editor.project().active().tracks[0].id.clone();
            editor
                .apply(Command::RemoveTrack { track_id: next })
                .expect("removes");
        }
        let last = editor.project().active().tracks[0].id.clone();
        let refused = editor.apply(Command::RemoveTrack { track_id: last });
        assert_eq!(
            refused.unwrap_err().to_string(),
            "A timeline needs at least one track."
        );
    }

    #[test]
    fn removing_media_sweeps_its_clips_from_every_timeline() {
        let (mut editor, media_id, _) = fixture();
        editor.apply(Command::AddTimeline).expect("adds");
        let track_id = editor.project().active().tracks[0].id.clone();
        editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id,
                start: 0.0,
                ripple: false,
            })
            .expect("adds");

        editor
            .apply(Command::RemoveMedia {
                media_id: media_id.clone(),
            })
            .expect("removes");
        assert!(editor.project().media.is_empty());
        assert!(
            editor
                .project()
                .timelines
                .iter()
                .all(|timeline| timeline.clips.is_empty()),
            "the inactive timeline's clip is swept too, not left dangling"
        );

        // Removing what is already gone is a tolerated no-op.
        let outcome = editor
            .apply(Command::RemoveMedia { media_id })
            .expect("tolerated");
        assert!(!outcome.applied);
    }

    #[test]
    fn renames_trim_whitespace_and_refuse_to_blank_a_name() {
        let (mut editor, _, _) = fixture();
        editor
            .apply(Command::RenameTimeline {
                timeline_id: "TL1".to_owned(),
                name: "\tCut A\n".to_owned(),
            })
            .expect("renames");
        assert_eq!(editor.project().timelines[0].name, "Cut A");
        let outcome = editor
            .apply(Command::RenameTimeline {
                timeline_id: "TL1".to_owned(),
                name: String::new(),
            })
            .expect("tolerated");
        assert!(!outcome.applied);
        assert_eq!(editor.project().timelines[0].name, "Cut A");
    }

    #[test]
    fn track_flags_flip_independently_and_report_no_ops_honestly() {
        let (mut editor, _, _) = fixture();
        let track_id = editor.project().active().tracks[0].id.clone();
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_id.clone(),
                flag: TrackFlag::Visible,
                value: false,
            })
            .expect("sets");
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_id.clone(),
                flag: TrackFlag::Muted,
                value: true,
            })
            .expect("sets");
        let track = &editor.project().active().tracks[0];
        assert!(!track.visible);
        assert!(track.muted);

        // Setting the value already in place is a no-op, exactly as the old
        // deep-compare judged it.
        let outcome = editor
            .apply(Command::SetTrackFlag {
                track_id,
                flag: TrackFlag::Muted,
                value: true,
            })
            .expect("tolerated");
        assert!(!outcome.applied);
    }

    #[test]
    fn rotation_wraps_into_the_half_open_degree_range() {
        // (-180, 180]: a knob dragged through full turns must not carry the
        // turns with it, and the boundary itself belongs to +180.
        let (mut editor, _, clip_id) = fixture();
        let cases = [
            (361.0, 1.0),
            (-1.0, -1.0),
            (720.0, 0.0),
            (180.0, 180.0),
            (-180.0, 180.0),
            (540.0, 180.0),
        ];
        for (sent, landed) in cases {
            editor
                .apply(Command::SetClipTransform {
                    clip_id: clip_id.clone(),
                    scale: None,
                    offset_x: None,
                    offset_y: None,
                    rotation: Some(sent),
                    stretch_x: None,
                    stretch_y: None,
                })
                .expect("sets");
            assert_eq!(
                editor
                    .project()
                    .active()
                    .clip(&clip_id)
                    .expect("exists")
                    .rotation,
                landed,
                "{sent} degrees should land at {landed}"
            );
        }
    }

    #[test]
    fn removing_a_timeline_moves_the_active_tab_to_a_neighbour() {
        let mut editor = Editor::new();
        let second = editor
            .apply(Command::AddTimeline)
            .expect("adds")
            .created_id
            .expect("id");
        let third = editor
            .apply(Command::AddTimeline)
            .expect("adds")
            .created_id
            .expect("id");
        assert_eq!(editor.project().active_timeline_id, third);

        // An unknown id is a tolerated no-op while the floor allows it.
        let outcome = editor
            .apply(Command::RemoveTimeline {
                timeline_id: "ghost".to_owned(),
            })
            .expect("tolerated");
        assert!(!outcome.applied);

        // Removing the active last tab falls back to the previous one.
        editor
            .apply(Command::RemoveTimeline { timeline_id: third })
            .expect("removes");
        assert_eq!(editor.project().active_timeline_id, second);

        // Removing an active middle tab prefers the neighbour to its right.
        let third = editor
            .apply(Command::AddTimeline)
            .expect("adds")
            .created_id
            .expect("id");
        editor
            .apply(Command::SelectTimeline {
                timeline_id: second.clone(),
            })
            .expect("selects");
        editor
            .apply(Command::RemoveTimeline {
                timeline_id: second,
            })
            .expect("removes");
        assert_eq!(editor.project().active_timeline_id, third);

        // Removing an inactive tab leaves the selection alone.
        editor
            .apply(Command::RemoveTimeline {
                timeline_id: "TL1".to_owned(),
            })
            .expect("removes");
        assert_eq!(editor.project().active_timeline_id, third);
    }

    #[test]
    fn fonts_deduplicate_by_path_and_removal_leaves_titles_alone() {
        let mut editor = Editor::new();
        editor
            .apply(Command::AddFont {
                family: "Inter".to_owned(),
                path: "/fonts/inter.ttf".to_owned(),
            })
            .expect("adds");
        // Same path again: a re-import must not duplicate the entry.
        let outcome = editor
            .apply(Command::AddFont {
                family: "Inter Again".to_owned(),
                path: "/fonts/inter.ttf".to_owned(),
            })
            .expect("tolerated");
        assert!(!outcome.applied);
        assert_eq!(editor.project().fonts.len(), 1);

        let clip_id = editor
            .apply(Command::AddTextClip {
                above: false,
                track_id: None,
                start: 0.0,
                style: Some(TextStyle {
                    font_family: "Inter".to_owned(),
                    ..TextStyle::default()
                }),
                duration: None,
                offset_y: None,
            })
            .expect("adds")
            .created_id
            .expect("id");

        editor
            .apply(Command::RemoveFont {
                family: "Inter".to_owned(),
            })
            .expect("removes");
        assert!(editor.project().fonts.is_empty());
        let clip = editor.project().active().clip(&clip_id).expect("exists");
        assert_eq!(
            clip.text.as_ref().expect("text").font_family,
            "Inter",
            "the title keeps the family name so the face can come back"
        );

        // Removing what is already gone is a tolerated no-op.
        let outcome = editor
            .apply(Command::RemoveFont {
                family: "Inter".to_owned(),
            })
            .expect("tolerated");
        assert!(!outcome.applied);
    }

    #[test]
    fn every_command_round_trips_through_serde() {
        // One of each variant. Grow this list with the enum, or a new
        // command ships without proof its wire shape survives a round trip.
        let item = match media("/a.mp4", 10.0, true) {
            Command::AddMedia { item } => item,
            _ => unreachable!("the helper builds AddMedia"),
        };
        let commands = vec![
            Command::AddMedia { item: item.clone() },
            Command::RemoveMedia {
                media_id: "m1".to_owned(),
            },
            Command::SetMediaPlaceholder {
                media_id: "m1".to_owned(),
                placeholder: true,
            },
            Command::FillSlot {
                media_id: "m1".to_owned(),
                item,
            },
            Command::Batch {
                commands: vec![Command::AddTrack { bottom: false }],
            },
            Command::AddClip {
                media_id: "m1".to_owned(),
                track_id: "T1".to_owned(),
                start: 1.0,
                ripple: false,
            },
            Command::AddClipAtFirstFree {
                media_id: "m1".to_owned(),
                start: 2.0,
            },
            Command::AddTextClip {
                above: false,
                track_id: Some("T1".to_owned()),
                start: 0.0,
                style: Some(TextStyle::default()),
                duration: Some(2.0),
                offset_y: Some(0.4),
            },
            Command::MoveClips {
                moves: vec![ClipMove {
                    clip_id: "c1".to_owned(),
                    start: 0.0,
                    track_id: "T2".to_owned(),
                }],
            },
            Command::TrimClip {
                clip_id: "c1".to_owned(),
                edge: TrimEdge::End,
                delta: -0.5,
                ripple: false,
            },
            Command::SplitClips {
                clip_ids: vec!["c1".to_owned()],
                time: 3.0,
            },
            Command::MergeClips {
                clip_ids: vec!["c1".to_owned(), "c2".to_owned()],
            },
            Command::RemoveClips {
                clip_ids: vec!["c1".to_owned()],
                ripple: false,
            },
            Command::UpdateClip {
                clip_id: "c1".to_owned(),
                patch: ClipPatch {
                    volume: Some(0.5),
                    transition_in: Some(None),
                    ..ClipPatch::default()
                },
            },
            Command::SetClipSpeed {
                clip_id: "c1".to_owned(),
                ripple: false,
                speed: 2.0,
            },
            Command::SetClipTransform {
                clip_id: "c1".to_owned(),
                scale: Some(1.5),
                offset_x: None,
                offset_y: Some(-0.25),
                rotation: Some(90.0),
                stretch_x: None,
                stretch_y: None,
            },
            Command::DetachAudio {
                clip_id: "c1".to_owned(),
            },
            Command::ReattachAudio {
                clip_id: "c1".to_owned(),
            },
            Command::AddTrack { bottom: false },
            Command::RemoveTrack {
                track_id: "T1".to_owned(),
            },
            Command::SetTrackFlag {
                track_id: "T1".to_owned(),
                flag: TrackFlag::Muted,
                value: true,
            },
            Command::AddTimeline,
            Command::RemoveTimeline {
                timeline_id: "TL1".to_owned(),
            },
            Command::RenameTimeline {
                timeline_id: "TL1".to_owned(),
                name: "Cut A".to_owned(),
            },
            Command::SelectTimeline {
                timeline_id: "TL1".to_owned(),
            },
            Command::AddFont {
                family: "Inter".to_owned(),
                path: "/fonts/inter.ttf".to_owned(),
            },
            Command::RemoveFont {
                family: "Inter".to_owned(),
            },
        ];
        for command in commands {
            let wire = serde_json::to_value(&command).expect("serialises");
            let back: Command = serde_json::from_value(wire).expect("parses");
            assert_eq!(back, command);
        }
    }

    #[test]
    fn clips_for_vanished_tracks_or_media_are_dropped_on_load() {
        // A hand-edited or truncated file must degrade to something
        // openable: a clip with nowhere to live (or nothing to show) is
        // dropped, while a text clip - which needs no media - survives.
        let document = json!({
            "name": "Damaged", "version": 1,
            "media": [{ "id": "m1", "path": "/a.mp4", "name": "a.mp4", "kind": "video",
                        "hasAudio": false }],
            "tracks": [{ "id": "T1", "name": "Track 1", "visible": true, "muted": false }],
            "clips": [
                { "id": "c1", "trackId": "ghost", "mediaId": "m1", "kind": "video",
                  "start": 0.0, "duration": 1.0 },
                { "id": "c2", "trackId": "T1", "mediaId": "ghost", "kind": "video",
                  "start": 0.0, "duration": 1.0 },
                { "id": "c3", "trackId": "T1", "mediaId": "m1", "kind": "video",
                  "start": 0.0, "duration": 1.0 },
                { "id": "c4", "trackId": "T1", "kind": "text", "start": 0.0, "duration": 1.0 }
            ]
        });
        let editor = Editor::from_document(&document).expect("loads");
        let ids: Vec<&str> = editor
            .project()
            .active()
            .clips
            .iter()
            .map(|clip| clip.id.as_str())
            .collect();
        assert_eq!(ids, ["c3", "c4"]);
    }

    #[test]
    fn hand_edited_values_are_clamped_on_load() {
        // The reader trusts nothing: values that would render invisibly or
        // fall outside the engine's ranges are pulled back in, not obeyed.
        let document = json!({
            "name": "Edited", "version": 1,
            "media": [{ "id": "m1", "path": "/a.mp4", "name": "a.mp4", "kind": "video",
                        "hasAudio": false }],
            "tracks": [{ "id": "T1", "name": "Track 1", "visible": true, "muted": false }],
            "clips": [
                { "id": "c1", "trackId": "T1", "mediaId": "m1", "kind": "video",
                  "start": -4.0, "duration": -3.0, "sourceStart": -1.0,
                  "volume": -2.0, "opacity": 2.0, "speed": 100.0, "scale": 0.0,
                  "transitionIn": { "id": "cross-fade", "duration": 0.0 } },
                { "id": "c2", "trackId": "T1", "kind": "text", "start": 0.0, "duration": 1.0,
                  "text": { "content": "Hi", "fontSize": 0.0, "fontWeight": 9999.0,
                            "lineHeight": 0.1, "opacity": 5.0 } }
            ]
        });
        let editor = Editor::from_document(&document).expect("loads");
        let clip = &editor.project().active().clips[0];
        assert_eq!(clip.start, 0.0);
        assert_eq!(clip.duration, 1.0 / 60.0, "the floor every command holds");
        assert_eq!(clip.source_start, 0.0);
        assert_eq!(clip.volume, 0.0);
        assert_eq!(clip.opacity, 1.0);
        assert_eq!(clip.speed, 16.0);
        assert_eq!(clip.scale, 0.05);
        assert_eq!(clip.transition_in.as_ref().expect("kept").duration, 0.1);

        let text = editor.project().active().clips[1]
            .text
            .as_ref()
            .expect("text");
        assert_eq!(
            text.font_size, 0.01,
            "a zero size would render an invisible title"
        );
        assert_eq!(text.font_weight, 900.0);
        assert_eq!(
            text.line_height, 0.5,
            "lines cannot collapse onto each other"
        );
        assert_eq!(text.opacity, 1.0);
    }

    #[test]
    fn effect_keys_ride_a_knob_and_survive_the_document() {
        use crate::model::{AppliedFilter, KeyEase};
        let (mut editor, _, clip_id) = fixture();
        let mut link = AppliedFilter::new("concat.adjust");
        link.params.insert("exposure".to_owned(), 1.0);
        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    video_effects: Some(vec![link]),
                    ..ClipPatch::default()
                },
            })
            .expect("applies");
        for (at, value) in [(0.0, -1.0), (1.0, 1.0)] {
            editor
                .apply(Command::SetEffectKey {
                    clip_id: clip_id.clone(),
                    entry: 0,
                    key: "exposure".to_owned(),
                    at,
                    value,
                    ease: KeyEase::LINEAR,
                })
                .expect("keys");
        }
        let link = &editor
            .project()
            .active()
            .clip(&clip_id)
            .expect("clip")
            .video_effects[0];
        assert!(link.is_keyed("exposure"));
        assert!(!link.is_keyed("contrast"));
        // Halfway along a straight ride between -1 and 1 is 0; the constant
        // is what the knob falls back to once the keys come off.
        assert!((link.value_at("exposure", 0.5, 0.0)).abs() < 1e-9);
        assert_eq!(link.params_at(0.25).get("exposure").copied(), Some(-0.5));
        assert_eq!(link.keys_around("exposure", 0.5), (Some(0.0), Some(1.0)));
        assert!(link.key_at("exposure", 1.0).is_some());

        let document = editor.to_document(&settings());
        let restored = Editor::from_document(&document).expect("loads");
        let link = &restored
            .project()
            .active()
            .clip(&clip_id)
            .expect("clip")
            .video_effects[0];
        assert_eq!(link.keys_on("exposure").len(), 2);

        let mut editor = restored;
        editor
            .apply(Command::ClearEffectKey {
                clip_id: clip_id.clone(),
                entry: 0,
                key: "exposure".to_owned(),
                at: 1.0,
            })
            .expect("clears one");
        editor
            .apply(Command::ClearEffectKeys {
                clip_id: clip_id.clone(),
                entry: 0,
                key: "exposure".to_owned(),
            })
            .expect("clears the rest");
        let link = &editor
            .project()
            .active()
            .clip(&clip_id)
            .expect("clip")
            .video_effects[0];
        assert!(!link.is_keyed("exposure"));
        assert_eq!(link.value_at("exposure", 0.5, 0.0), 1.0);
    }

    #[test]
    fn missing_media_detects_nonexistent_paths() {
        let mut project = Project::new();

        // Dodaj media item s nepostojećom pathom
        let id = "m1".to_owned();
        project.media.push(MediaItem {
            id: id.clone(),
            path: "/does/not/exist.mp4".to_owned(),
            name: "Missing Clip".to_owned(),
            duration: Some(10.0),
            kind: MediaKind::Video,
            width: Some(1920),
            height: Some(1080),
            frame_rate: None,
            frame_rate_fraction: None,
            video_codec: None,
            audio_codec: None,
            has_audio: false,
            audio_tracks: vec![],
            placeholder: false,
            color_range: None,
            color_space: Default::default(),
            origin: None,
            source_in: None,
            source_out: None,
            folder_id: None,
            extra: Default::default(),
        });

        let missing = project.missing_media();
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0].id, "m1");
        assert_eq!(missing[0].name, "Missing Clip");
        assert_eq!(missing[0].path, "/does/not/exist.mp4");
    }

    #[test]
    fn missing_media_ignores_existing_paths() {
        let mut project = Project::new();

        // Koristi Cargo.toml koji sigurno postoji
        let existing = std::env::current_dir()
            .unwrap()
            .join("Cargo.toml")
            .to_string_lossy()
            .to_string();

        project.media.push(MediaItem {
            id: "m2".to_owned(),
            path: existing,
            name: "Existing Clip".to_owned(),
            duration: Some(5.0),
            kind: MediaKind::Video,
            width: Some(1920),
            height: Some(1080),
            frame_rate: None,
            frame_rate_fraction: None,
            video_codec: None,
            audio_codec: None,
            has_audio: false,
            audio_tracks: vec![],
            placeholder: false,
            color_range: None,
            color_space: Default::default(),
            origin: None,
            source_in: None,
            source_out: None,
            folder_id: None,
            extra: Default::default(),
        });

        let missing = project.missing_media();
        assert_eq!(missing.len(), 0);
    }

    #[test]
    fn shape_clips_survive_without_media_and_carry_their_figure() {
        use crate::model::{ClipKind, ShapeKind, ShapeStyle};
        let mut editor = Editor::new();
        let clip_id = editor
            .apply(Command::AddShapeClip {
                above: true,
                track_id: None,
                start: 1.0,
                style: Some(ShapeStyle {
                    kind: ShapeKind::Arrow,
                    fill: "#ff8800".to_owned(),
                    ..ShapeStyle::default()
                }),
                duration: Some(2.5),
                name: "Flèche".to_owned(),
            })
            .expect("adds")
            .created_id
            .expect("id");
        {
            let clip = editor.project().active().clip(&clip_id).expect("exists");
            assert_eq!(clip.kind, ClipKind::Shape);
            assert_eq!(clip.name, "Flèche", "named by the caller, in its language");
            assert_eq!(clip.duration, 2.5, "a stated duration lands directly");
            assert!(clip.media_id.is_empty(), "no file behind it");
            let shape = clip.shape.as_ref().expect("carries its figure");
            assert_eq!(shape.kind, ShapeKind::Arrow);
            assert_eq!(shape.fill, "#ff8800");
        }

        let document = editor.to_document(&settings());
        let restored = Editor::from_document(&document).expect("loads");
        assert_eq!(restored.project(), editor.project());

        // A number out of range is pulled back in, not refused: the reader
        // runs the same tidy over a hand-edited file.
        let wild = editor
            .apply(Command::AddShapeClip {
                above: false,
                track_id: None,
                start: 0.0,
                style: Some(ShapeStyle {
                    width: 40.0,
                    stroke_width: -1.0,
                    ..ShapeStyle::default()
                }),
                duration: None,
                name: String::new(),
            })
            .expect("adds")
            .created_id
            .expect("id");
        let clip = editor.project().active().clip(&wild).expect("exists");
        let shape = clip.shape.as_ref().expect("figure");
        assert_eq!((shape.width, shape.stroke_width), (4.0, 0.0));
        assert_eq!(
            clip.name, "square",
            "an empty name falls back to the figure's"
        );
    }

    /// 0.2.4's reverse flag and preset animations come off as the document
    /// is read, with the clips that had them named; a false flag or a null
    /// animation is taken off without being named.
    #[test]
    fn retired_clip_fields_are_dropped_and_named() {
        let mut editor = Editor::new();
        for (index, name) in ["Backwards", "Bouncy", "Plain"].into_iter().enumerate() {
            editor
                .apply(Command::AddLayerClip {
                    track_id: None,
                    start: index as f64 * 2.0,
                    duration: Some(1.0),
                    effect_id: "concat.warm".into(),
                    name: name.into(),
                    sound: false,
                })
                .expect("a layer");
        }
        let mut document = editor.to_document(&settings());
        let clips = document["timelines"][0]["clips"]
            .as_array_mut()
            .expect("clips");
        assert!(clips.len() >= 3, "{clips:?}");
        clips[0]["reverse"] = serde_json::json!(true);
        clips[1]["animationIn"] = serde_json::json!({"name": "bounce", "duration": 0.5});
        clips[2]["reverse"] = serde_json::json!(false);
        clips[2]["animationLoop"] = serde_json::Value::Null;
        let names: Vec<String> = clips
            .iter()
            .map(|clip| clip["name"].as_str().unwrap_or_default().to_owned())
            .collect();

        let mut reopened = Editor::from_document(&document).expect("reads");
        let retired = reopened.drop_retired_fields();
        assert_eq!(retired.reversed, vec![names[0].clone()]);
        assert_eq!(retired.animated, vec![names[1].clone()]);
        for clip in &reopened.project().active().clips {
            for key in ["reverse", "animationIn", "animationLoop"] {
                assert!(!clip.extra.contains_key(key), "{key} on {}", clip.name);
            }
        }
        assert_eq!(
            reopened.drop_retired_fields(),
            crate::RetiredFields::default()
        );
    }

    /// A lane can be added on top of the stack or under it; the command's
    /// old shape, with no `bottom`, still means the top.
    #[test]
    fn a_track_goes_on_top_or_underneath() {
        let mut editor = Editor::new();
        let before = editor.project().active().tracks.len();
        let top = editor
            .apply(Command::AddTrack { bottom: false })
            .expect("adds")
            .created_id
            .expect("an id");
        let under = editor
            .apply(Command::AddTrack { bottom: true })
            .expect("adds")
            .created_id
            .expect("an id");
        let tracks = &editor.project().active().tracks;
        assert_eq!(tracks.len(), before + 2);
        assert_eq!(tracks.last().map(|t| t.id.as_str()), Some(top.as_str()));
        assert_eq!(tracks.first().map(|t| t.id.as_str()), Some(under.as_str()));
        let old: Command = serde_json::from_str(r#"{"op":"addTrack"}"#).expect("parses");
        assert_eq!(old, Command::AddTrack { bottom: false });
        assert_eq!(
            serde_json::to_string(&Command::AddTrack { bottom: false }).expect("writes"),
            r#"{"op":"addTrack"}"#
        );
    }

    /// Enhance's copy keeps the clip's original beside it: switching it
    /// off shows the original, on shows the copy, each one undo step, and
    /// any other replacement - a reversed copy - ends the link.
    #[test]
    fn an_enhanced_clip_switches_between_copy_and_original() {
        use crate::commands::NewMedia;
        use crate::model::MediaKind;
        let item = |path: &str| NewMedia {
            path: path.into(),
            name: path.into(),
            duration: Some(10.0),
            kind: MediaKind::Video,
            width: Some(64),
            height: Some(64),
            frame_rate: Some(30.0),
            frame_rate_fraction: Some("30/1".into()),
            video_codec: None,
            audio_codec: None,
            has_audio: false,
            audio_tracks: Vec::new(),
            origin: None,
            color_space: Default::default(),
        };
        let mut editor = Editor::new();
        editor
            .apply(Command::AddMedia {
                item: item("/a.mp4"),
            })
            .expect("imports");
        let original = editor.project().media[0].id.clone();
        let track = editor.project().active().tracks[0].id.clone();
        let clip = editor
            .apply(Command::AddClip {
                media_id: original.clone(),
                track_id: track,
                start: 0.0,
                ripple: false,
            })
            .expect("places")
            .created_id
            .expect("a clip");
        let copy = editor
            .apply(Command::ReplaceClipMedia {
                clip_id: clip.clone(),
                item: item("/a-enhanced.mp4"),
                source_start: None,
                enhanced: true,
                offset: 0.0,
            })
            .expect("replaces")
            .created_id
            .expect("the copy");
        let shown = |editor: &Editor| {
            editor
                .project()
                .active()
                .clip(&clip)
                .cloned()
                .expect("there")
        };
        let link = shown(&editor).enhanced.expect("linked");
        assert_eq!(
            (link.original.as_str(), link.copy.as_str(), link.on),
            (original.as_str(), copy.as_str(), true)
        );

        editor
            .apply(Command::SetClipEnhanced {
                clip_id: clip.clone(),
                on: false,
            })
            .expect("off");
        assert_eq!(shown(&editor).media_id, original);
        assert!(!shown(&editor).enhanced.expect("kept").on);
        editor
            .apply(Command::SetClipEnhanced {
                clip_id: clip.clone(),
                on: true,
            })
            .expect("on");
        assert_eq!(shown(&editor).media_id, copy);
        editor.undo();
        assert_eq!(shown(&editor).media_id, original, "one undo step");

        // A copy of a span: it starts 4 s into the original, so the clip's
        // in-point is 4 s less in it, and switching moves it back and forth
        // with the same moment on screen.
        // The head trimmed 6 s in, so the in-point is past the copy's start.
        editor
            .apply(Command::TrimClip {
                clip_id: clip.clone(),
                edge: crate::commands::TrimEdge::Start,
                delta: 6.0,
                ripple: false,
            })
            .expect("trims");
        editor
            .apply(Command::SetClipEnhanced {
                clip_id: clip.clone(),
                on: false,
            })
            .expect("off");
        let original_start = shown(&editor).source_start;
        assert!(original_start >= 4.0, "{original_start}");
        editor
            .apply(Command::ReplaceClipMedia {
                clip_id: clip.clone(),
                item: item("/a-span.mp4"),
                source_start: Some(original_start - 4.0),
                enhanced: true,
                offset: 4.0,
            })
            .expect("replaces");
        assert_eq!(shown(&editor).source_start, original_start - 4.0);
        editor
            .apply(Command::SetClipEnhanced {
                clip_id: clip.clone(),
                on: false,
            })
            .expect("off");
        assert_eq!(shown(&editor).source_start, original_start);
        editor
            .apply(Command::SetClipEnhanced {
                clip_id: clip.clone(),
                on: true,
            })
            .expect("on");
        assert_eq!(shown(&editor).source_start, original_start - 4.0);

        editor
            .apply(Command::ReplaceClipMedia {
                clip_id: clip.clone(),
                item: item("/a-reversed.mp4"),
                source_start: None,
                enhanced: false,
                offset: 0.0,
            })
            .expect("replaces");
        assert_eq!(shown(&editor).enhanced, None);
        let off = editor
            .apply(Command::SetClipEnhanced {
                clip_id: clip.clone(),
                on: false,
            })
            .expect("no-op");
        assert!(!off.applied);
    }

    // ── Delete Left / Delete Right (Q / W) ──
    // https://github.com/jub0t/Concat/issues/400

    fn span(editor: &Editor, id: &str) -> (f64, f64, f64) {
        let clip = editor.project().active().clip(id).expect("clip");
        (clip.start, clip.duration, clip.source_start)
    }

    fn close(a: (f64, f64, f64), b: (f64, f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9 && (a.2 - b.2).abs() < 1e-9
    }

    fn cut(editor: &Editor, ids: &[String], at: f64, side: PlayheadSide, ripple: bool) -> Command {
        crate::delete_to_playhead(editor.project().active(), ids, at, side, ripple).expect("a cut")
    }

    #[test]
    fn delete_left_takes_the_clip_from_its_start_to_the_playhead() {
        let (mut editor, media_id, _) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let ids = lane(&mut editor, &media_id, &track, &[20.0, 30.0]);
        let command = cut(&editor, &ids[..1], 24.0, PlayheadSide::Left, false);
        editor.apply(command).expect("cuts");
        // The frames after the playhead stay where they were, on the
        // timeline and in the source; the hole stays on a plain timeline.
        let first = span(&editor, &ids[0]);
        assert!(close(first, (24.0, 6.0, 4.0)), "{first:?}");
        assert!(close(span(&editor, &ids[1]), (30.0, 10.0, 0.0)));
    }

    #[test]
    fn delete_right_takes_the_clip_from_the_playhead_to_its_end() {
        let (mut editor, media_id, _) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let ids = lane(&mut editor, &media_id, &track, &[20.0, 30.0]);
        let command = cut(&editor, &ids[..1], 24.0, PlayheadSide::Right, false);
        editor.apply(command).expect("cuts");
        assert!(close(span(&editor, &ids[0]), (20.0, 4.0, 0.0)));
        assert!(
            close(span(&editor, &ids[1]), (30.0, 10.0, 0.0)),
            "the gap stays"
        );
    }

    /// On a magnetic timeline the lane closes up behind the cut, as any
    /// trim there does: a Delete Left keeps the clip's start and pulls
    /// what follows in by what came off its head; a Delete Right pulls it
    /// in by what came off the tail.
    #[test]
    fn a_magnetic_delete_closes_the_lane() {
        let (mut editor, media_id, _) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let ids = lane(&mut editor, &media_id, &track, &[20.0, 30.0]);
        let left = cut(&editor, &ids[..1], 24.0, PlayheadSide::Left, true);
        editor.apply(left).expect("cuts");
        let (first, second) = (span(&editor, &ids[0]), span(&editor, &ids[1]));
        assert!(close(first, (20.0, 6.0, 4.0)), "{first:?}");
        assert!(close(second, (26.0, 10.0, 0.0)), "{second:?}");

        let right = cut(&editor, &ids[..1], 22.0, PlayheadSide::Right, true);
        editor.apply(right).expect("cuts");
        let (first, second) = (span(&editor, &ids[0]), span(&editor, &ids[1]));
        assert!(close(first, (20.0, 2.0, 4.0)), "{first:?}");
        assert!(close(second, (22.0, 10.0, 0.0)), "{second:?}");
    }

    /// Clips on several lanes are cut together, as one undo step.
    #[test]
    fn a_delete_across_lanes_is_one_undo_step() {
        let (mut editor, media_id, first) = fixture();
        let tracks = editor.project().active().tracks.clone();
        let above = lane(&mut editor, &media_id, &tracks[1].id, &[2.0]);
        let ids = vec![first.clone(), above[0].clone()];
        let command = cut(&editor, &ids, 5.0, PlayheadSide::Right, false);
        assert!(matches!(&command, Command::Batch { commands } if commands.len() == 2));
        editor.apply(command).expect("cuts");
        assert!(close(span(&editor, &first), (0.0, 5.0, 0.0)));
        assert!(close(span(&editor, &above[0]), (2.0, 3.0, 0.0)));
        assert!(editor.undo());
        assert!(
            close(span(&editor, &first), (0.0, 10.0, 0.0)),
            "both back at once"
        );
        assert!(close(span(&editor, &above[0]), (2.0, 10.0, 0.0)));
    }

    /// A playhead outside the clip, or on its very edge, has nothing to
    /// take: no command at all, rather than an undo step that did nothing.
    #[test]
    fn a_playhead_outside_the_clip_cuts_nothing() {
        let (editor, _, clip) = fixture();
        let timeline = editor.project().active();
        let ids = [clip];
        for at in [-1.0, 0.0, 10.0, 12.0] {
            for side in [PlayheadSide::Left, PlayheadSide::Right] {
                assert!(
                    crate::delete_to_playhead(timeline, &ids, at, side, true).is_none(),
                    "{at} {side:?}"
                );
            }
        }
        let gone = ["nobody".to_owned()];
        assert!(
            crate::delete_to_playhead(timeline, &gone, 5.0, PlayheadSide::Left, false).is_none()
        );
    }

    // ── Rolling Edit (Command::RollTrim) ──

    fn roll_fixture(media_dur: f64) -> (Editor, String, String, String, String) {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/roll_media.mp4", media_dur, true))
            .expect("media")
            .created_id
            .unwrap();
        let track = editor.project().active().tracks[0].id.clone();
        let c1 = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("c1")
            .created_id
            .unwrap();
        // Trim c1 down to 10.0s so c2 can start at 10.0s
        editor
            .apply(Command::TrimClip {
                clip_id: c1.clone(),
                edge: crate::commands::TrimEdge::End,
                delta: -(media_dur - 10.0),
                ripple: false,
            })
            .expect("trim c1");

        let c2 = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 10.0,
                ripple: false,
            })
            .expect("c2")
            .created_id
            .unwrap();
        // Trim c2 down to 10.0s and give it 10.0s source headroom to the left
        editor
            .apply(Command::TrimClip {
                clip_id: c2.clone(),
                edge: crate::commands::TrimEdge::End,
                delta: -(media_dur - 10.0),
                ripple: false,
            })
            .expect("trim c2");

        {
            let t = editor.project_mut().active_mut();
            let clip2 = t.clip_mut(&c2).unwrap();
            clip2.source_start = 10.0;
        }
        (editor, media_id, track, c1, c2)
    }

    #[test]
    fn roll_trim_positive_and_negative() {
        let (mut editor, _media_id, _track, c1, c2) = roll_fixture(60.0);

        // 1. Positive roll (+2.0): cut moves right from 10.0 to 12.0
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: 2.0,
            })
            .expect("roll trim positive");
        assert!(outcome.applied);

        let t = editor.project().active();
        let head = t.clip(&c1).expect("head clip");
        let tail = t.clip(&c2).expect("tail clip");

        assert!((head.start - 0.0).abs() < 1e-6);
        assert!((head.duration - 12.0).abs() < 1e-6);
        assert!((tail.start - 12.0).abs() < 1e-6);
        assert!((tail.duration - 8.0).abs() < 1e-6);
        assert!((tail.source_start - 12.0).abs() < 1e-6);

        // Invariants: combined duration preserved & outer boundaries fixed
        assert!((head.duration + tail.duration - 20.0).abs() < 1e-6);
        assert!((tail.start + tail.duration - 20.0).abs() < 1e-6);
        assert!((tail.start - (head.start + head.duration)).abs() < 1e-6);

        // 2. Negative roll (-1.5): cut moves left from 12.0 to 10.5
        let outcome2 = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: -1.5,
            })
            .expect("roll trim negative");
        assert!(outcome2.applied);

        let t = editor.project().active();
        let head = t.clip(&c1).expect("head clip");
        let tail = t.clip(&c2).expect("tail clip");

        assert!((head.start - 0.0).abs() < 1e-6);
        assert!((head.duration - 10.5).abs() < 1e-6);
        assert!((tail.start - 10.5).abs() < 1e-6);
        assert!((tail.duration - 9.5).abs() < 1e-6);
        assert!((tail.source_start - 10.5).abs() < 1e-6);

        // Combined duration preserved & outer boundaries fixed
        assert!((head.duration + tail.duration - 20.0).abs() < 1e-6);
        assert!((tail.start + tail.duration - 20.0).abs() < 1e-6);
        assert!((tail.start - (head.start + head.duration)).abs() < 1e-6);
    }

    #[test]
    fn roll_trim_minimum_duration_boundary() {
        use crate::model::ranges::MIN_CLIP_DURATION;
        let (mut editor, _media_id, _track, c1, c2) = roll_fixture(60.0);

        // Shorten both clips to 1.0s each: c1 [0..1], c2 [1..2]
        {
            let t = editor.project_mut().active_mut();
            let clip1 = t.clip_mut(&c1).unwrap();
            clip1.duration = 1.0;
            let clip2 = t.clip_mut(&c2).unwrap();
            clip2.start = 1.0;
            clip2.duration = 1.0;
            clip2.source_start = 10.0; // 10s headroom to the left
        }

        let t = editor.project().active();
        assert!((t.clip(&c1).unwrap().duration - 1.0).abs() < 1e-6);
        assert!((t.clip(&c2).unwrap().duration - 1.0).abs() < 1e-6);

        // Attempt roll of +5.0 (tail can only shrink down to MIN_CLIP_DURATION)
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: 5.0,
            })
            .expect("roll trim");
        assert!(outcome.applied);

        let t = editor.project().active();
        let head = t.clip(&c1).unwrap();
        let tail = t.clip(&c2).unwrap();

        assert!((tail.duration - MIN_CLIP_DURATION).abs() < 1e-6);
        assert!((head.duration - (2.0 - MIN_CLIP_DURATION)).abs() < 1e-6);
        assert!((head.duration + tail.duration - 2.0).abs() < 1e-6);

        // Attempt roll of -5.0 (head can only shrink down to MIN_CLIP_DURATION)
        let outcome2 = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: -5.0,
            })
            .expect("roll trim");
        assert!(outcome2.applied);

        let t = editor.project().active();
        let head = t.clip(&c1).unwrap();
        let tail = t.clip(&c2).unwrap();

        assert!((head.duration - MIN_CLIP_DURATION).abs() < 1e-6);
        assert!((tail.duration - (2.0 - MIN_CLIP_DURATION)).abs() < 1e-6);
        assert!((head.duration + tail.duration - 2.0).abs() < 1e-6);
    }

    #[test]
    fn roll_trim_source_media_boundary() {
        let mut editor = Editor::new();
        // Media with finite duration 12.0s
        let media_id = editor
            .apply(media("/source_bound.mp4", 12.0, false))
            .expect("media")
            .created_id
            .unwrap();
        let track = editor.project().active().tracks[0].id.clone();
        let c1 = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("c1")
            .created_id
            .unwrap();
        let c2 = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 12.0,
                ripple: false,
            })
            .expect("c2")
            .created_id
            .unwrap();

        // Shrink c1 to 10.0s so head has 2.0s headroom in media (from 10.0 to 12.0)
        editor
            .apply(Command::TrimClip {
                clip_id: c1.clone(),
                edge: crate::commands::TrimEdge::End,
                delta: -2.0,
                ripple: false,
            })
            .expect("trim c1");
        // Move c2 to 10.0s to meet c1
        editor
            .apply(Command::MoveClips {
                moves: vec![crate::commands::ClipMove {
                    clip_id: c2.clone(),
                    track_id: track.clone(),
                    start: 10.0,
                }],
            })
            .expect("move c2");

        // Attempt roll of +10.0: head only has 2.0s media remaining
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: 10.0,
            })
            .expect("roll");
        assert!(outcome.applied);

        let t = editor.project().active();
        let head = t.clip(&c1).unwrap();
        let tail = t.clip(&c2).unwrap();
        assert!(
            (head.duration - 12.0).abs() < 1e-6,
            "head capped at 12.0s media duration"
        );
        assert!((tail.duration - 10.0).abs() < 1e-6);

        // Now test tail boundary: tail source_start is currently 2.0s (since it moved right by 2.0s)
        assert!((tail.source_start - 2.0).abs() < 1e-6);
        // Attempt roll of -10.0: tail source_start cannot go below 0.0, so clamped to -2.0
        let outcome2 = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: -10.0,
            })
            .expect("roll");
        assert!(outcome2.applied);

        let t = editor.project().active();
        let tail = t.clip(&c2).unwrap();
        assert!(
            (tail.source_start - 0.0).abs() < 1e-6,
            "tail source_start floored at 0.0"
        );
    }

    #[test]
    fn roll_trim_non_adjacent_and_different_tracks() {
        let (mut editor, media_id, c1) = fixture();
        let tracks = editor.project().active().tracks.clone();
        let t0 = tracks[0].id.clone();
        let t1 = tracks[1].id.clone();

        // 1. Different tracks
        let c_diff = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: t1.clone(),
                start: 10.0,
                ripple: false,
            })
            .expect("add")
            .created_id
            .unwrap();
        let before = editor.project().clone();
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c_diff,
                delta: 1.0,
            })
            .expect("diff tracks");
        assert!(!outcome.applied);
        assert_eq!(editor.project(), &before);

        // 2. Gapped clips on same track: c1 [0..10], c_gap [12..22]
        let c_gap = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: t0.clone(),
                start: 12.0,
                ripple: false,
            })
            .expect("add")
            .created_id
            .unwrap();
        let before2 = editor.project().clone();
        let outcome2 = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c_gap,
                delta: 1.0,
            })
            .expect("gap");
        assert!(!outcome2.applied);
        assert_eq!(editor.project(), &before2);

        // 3. Reversed order: head starts after tail
        let ids = lane(&mut editor, &media_id, &t0, &[10.0]);
        let c2 = ids[0].clone();
        let before3 = editor.project().clone();
        let outcome3 = editor
            .apply(Command::RollTrim {
                head_clip_id: c2.clone(),
                tail_clip_id: c1.clone(),
                delta: 1.0,
            })
            .expect("reversed");
        assert!(!outcome3.applied);
        assert_eq!(editor.project(), &before3);
    }

    #[test]
    fn roll_trim_keyframe_preservation_and_rewindowing() {
        use crate::model::{KeyEase, KeyProperty};
        let (mut editor, _media_id, _track, c1, c2) = roll_fixture(60.0);

        // Keyframe on c1 at normalized 0.5 (timeline 5.0s)
        editor
            .apply(Command::SetClipKey {
                clip_id: c1.clone(),
                property: KeyProperty::Opacity,
                at: 0.5,
                value: 0.8,
                ease: KeyEase::LINEAR,
            })
            .expect("key c1");

        // Keyframe on c2 at normalized 0.5 (timeline 15.0s)
        editor
            .apply(Command::SetClipKey {
                clip_id: c2.clone(),
                property: KeyProperty::Opacity,
                at: 0.5,
                value: 0.4,
                ease: KeyEase::LINEAR,
            })
            .expect("key c2");

        // Roll by +2.0s: c1 becomes 12.0s, c2 becomes 8.0s (starts at 12.0)
        editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: 2.0,
            })
            .expect("roll");

        let t = editor.project().active();
        let head = t.clip(&c1).unwrap();
        let tail = t.clip(&c2).unwrap();

        // Head key at timeline 5.0s: normalized at in 12.0s clip is 5.0 / 12.0
        let head_keys: Vec<_> = head.keys_on(KeyProperty::Opacity).collect();
        assert_eq!(head_keys.len(), 1);
        assert!((head_keys[0].at - 5.0 / 12.0).abs() < 1e-6);
        assert_eq!(head_keys[0].value, 0.8);

        // Tail key at timeline 15.0s: in 8.0s clip starting at 12.0s, it's at (15.0 - 12.0) / 8.0 = 3.0 / 8.0
        let tail_keys: Vec<_> = tail.keys_on(KeyProperty::Opacity).collect();
        assert_eq!(tail_keys.len(), 1);
        assert!((tail_keys[0].at - 3.0 / 8.0).abs() < 1e-6);
        assert_eq!(tail_keys[0].value, 0.4);
    }

    #[test]
    fn roll_trim_undo_redo_correctness() {
        let (mut editor, _media_id, _track, c1, c2) = roll_fixture(60.0);

        let initial_project = editor.project().clone();

        editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: 2.5,
            })
            .expect("roll");

        let modified_project = editor.project().clone();
        assert_ne!(&initial_project, &modified_project);

        // Undo restores exact initial project
        assert!(editor.undo());
        assert_eq!(editor.project(), &initial_project);

        // Redo restores exact modified project
        assert!(editor.redo());
        assert_eq!(editor.project(), &modified_project);
    }

    #[test]
    fn roll_trim_linked_audio_video_synchronization() {
        let (mut editor, _media_id, _track, v1, v2) = roll_fixture(60.0);

        // Detach audio for both video clips
        let a1 = editor
            .apply(Command::DetachAudio {
                clip_id: v1.clone(),
            })
            .expect("detach v1")
            .created_id
            .unwrap();
        let a2 = editor
            .apply(Command::DetachAudio {
                clip_id: v2.clone(),
            })
            .expect("detach v2")
            .created_id
            .unwrap();

        // Roll video cut between v1 and v2 by +2.0s
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: v1.clone(),
                tail_clip_id: v2.clone(),
                delta: 2.0,
            })
            .expect("roll video");
        assert!(outcome.applied);

        let t = editor.project().active();
        let v1_clip = t.clip(&v1).unwrap();
        let v2_clip = t.clip(&v2).unwrap();
        let a1_clip = t.clip(&a1).unwrap();
        let a2_clip = t.clip(&a2).unwrap();

        // Both video and audio cuts rolled in lockstep
        assert!((v1_clip.duration - 12.0).abs() < 1e-6);
        assert!((v2_clip.start - 12.0).abs() < 1e-6);
        assert!((a1_clip.duration - 12.0).abs() < 1e-6);
        assert!((a2_clip.start - 12.0).abs() < 1e-6);
        assert!((a2_clip.source_start - 12.0).abs() < 1e-6);

        // Now roll through audio pair (a1, a2) by -1.0s
        let outcome2 = editor
            .apply(Command::RollTrim {
                head_clip_id: a1.clone(),
                tail_clip_id: a2.clone(),
                delta: -1.0,
            })
            .expect("roll audio");
        assert!(outcome2.applied);

        let t = editor.project().active();
        let v1_clip = t.clip(&v1).unwrap();
        let v2_clip = t.clip(&v2).unwrap();
        let a1_clip = t.clip(&a1).unwrap();
        let a2_clip = t.clip(&a2).unwrap();

        assert!((v1_clip.duration - 11.0).abs() < 1e-6);
        assert!((v2_clip.start - 11.0).abs() < 1e-6);
        assert!((a1_clip.duration - 11.0).abs() < 1e-6);
        assert!((a2_clip.start - 11.0).abs() < 1e-6);
        assert!((a2_clip.source_start - 11.0).abs() < 1e-6);
    }

    #[test]
    fn roll_trim_invalid_operation_leaves_project_unchanged() {
        let (mut editor, media_id, c1) = fixture();
        let track = editor.project().active().tracks[0].id.clone();
        let ids = lane(&mut editor, &media_id, &track, &[10.0]);
        let c2 = ids[0].clone();

        let initial_project = editor.project().clone();

        // 1. Unknown head ID
        let res = editor
            .apply(Command::RollTrim {
                head_clip_id: "nonexistent".to_owned(),
                tail_clip_id: c2.clone(),
                delta: 1.0,
            })
            .expect("outcome");
        assert!(!res.applied);
        assert_eq!(editor.project(), &initial_project);

        // 2. Unknown tail ID
        let res = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: "nonexistent".to_owned(),
                delta: 1.0,
            })
            .expect("outcome");
        assert!(!res.applied);
        assert_eq!(editor.project(), &initial_project);

        // 3. Same clip for head and tail
        let res = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c1.clone(),
                delta: 1.0,
            })
            .expect("outcome");
        assert!(!res.applied);
        assert_eq!(editor.project(), &initial_project);

        // 4. Zero delta
        let res = editor
            .apply(Command::RollTrim {
                head_clip_id: c1.clone(),
                tail_clip_id: c2.clone(),
                delta: 0.0,
            })
            .expect("outcome");
        assert!(!res.applied);
        assert_eq!(editor.project(), &initial_project);

        // 5. NaN delta
        let err = editor.apply(Command::RollTrim {
            head_clip_id: c1.clone(),
            tail_clip_id: c2.clone(),
            delta: f64::NAN,
        });
        assert!(matches!(
            err,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &initial_project);

        // 6. Infinity delta
        let err = editor.apply(Command::RollTrim {
            head_clip_id: c1.clone(),
            tail_clip_id: c2.clone(),
            delta: f64::INFINITY,
        });
        assert!(matches!(
            err,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &initial_project);
    }

    fn slip_fixture(media_duration: f64) -> (Editor, String, String, String) {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/slip_media.mp4", media_duration, true))
            .expect("media")
            .created_id
            .unwrap();
        let track = editor.project().active().tracks[0].id.clone();
        let clip_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 5.0,
                ripple: false,
            })
            .expect("clip")
            .created_id
            .unwrap();

        // Configure initial source_start to 5.0s, duration to 10.0s
        let t = editor.project_mut().active_mut();
        let c = t.clip_mut(&clip_id).unwrap();
        c.source_start = 5.0;
        c.duration = 10.0;

        (editor, media_id, track, clip_id)
    }

    #[test]
    fn slip_clip_positive_and_negative_deltas() {
        let (mut editor, _media_id, _track, clip_id) = slip_fixture(60.0);

        // Initial: source_start = 5.0, duration = 10.0, start = 5.0
        // Slip positive by +3.0s
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 3.0,
            })
            .expect("slip +3.0");
        assert!(outcome.applied);

        let t = editor.project().active();
        let c = t.clip(&clip_id).unwrap();
        assert_eq!(c.start, 5.0, "timeline start stays fixed");
        assert_eq!(c.duration, 10.0, "timeline duration stays fixed");
        assert_eq!(c.source_start, 8.0, "source_start shifted from 5.0 to 8.0");

        // Slip negative by -2.0s
        let outcome2 = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: -2.0,
            })
            .expect("slip -2.0");
        assert!(outcome2.applied);

        let t = editor.project().active();
        let c = t.clip(&clip_id).unwrap();
        assert_eq!(c.start, 5.0);
        assert_eq!(c.duration, 10.0);
        assert_eq!(c.source_start, 6.0, "source_start shifted from 8.0 to 6.0");
    }

    #[test]
    fn slip_clip_source_media_start_boundary() {
        let (mut editor, _media_id, _track, clip_id) = slip_fixture(60.0);

        // Set source_start to 2.0s
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&clip_id)
            .unwrap()
            .source_start = 2.0;

        // Attempt negative slip of -5.0s (only 2.0s available before media start)
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: -5.0,
            })
            .expect("slip clamped to head");
        assert!(outcome.applied);

        let t = editor.project().active();
        let c = t.clip(&clip_id).unwrap();
        assert_eq!(c.source_start, 0.0, "clamped at media start 0.0");
        assert_eq!(c.start, 5.0);
        assert_eq!(c.duration, 10.0);

        // When already at 0.0, negative slip is a no-op
        let outcome2 = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: -1.0,
            })
            .expect("slip at boundary");
        assert!(!outcome2.applied);
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clip_id)
                .unwrap()
                .source_start,
            0.0
        );
    }

    #[test]
    fn slip_clip_source_media_end_boundary() {
        // Media duration 20.0s, clip duration 10.0s, initial source_start = 5.0s
        // Available headroom at tail = 20.0 - (5.0 + 10.0) = 5.0s
        let (mut editor, _media_id, _track, clip_id) = slip_fixture(20.0);

        // Attempt positive slip of +10.0s (only 5.0s available at tail)
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 10.0,
            })
            .expect("slip clamped to tail");
        assert!(outcome.applied);

        let t = editor.project().active();
        let c = t.clip(&clip_id).unwrap();
        assert_eq!(
            c.source_start, 10.0,
            "clamped to tail boundary (source_end = 20.0)"
        );
        assert_eq!(c.start, 5.0);
        assert_eq!(c.duration, 10.0);

        // When already at tail boundary, positive slip is a no-op
        let outcome2 = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 1.0,
            })
            .expect("slip at tail boundary");
        assert!(!outcome2.applied);
        assert_eq!(
            editor
                .project()
                .active()
                .clip(&clip_id)
                .unwrap()
                .source_start,
            10.0
        );
    }

    #[test]
    fn slip_clip_linked_audio_video_synchronization() {
        let (mut editor, _media_id, _track, v_id) = slip_fixture(60.0);

        // Detach audio for video clip
        let a_id = editor
            .apply(Command::DetachAudio {
                clip_id: v_id.clone(),
            })
            .expect("detach audio")
            .created_id
            .unwrap();

        // 1. Slipped via video clip: audio must slip in lockstep
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: v_id.clone(),
                delta: 4.0,
            })
            .expect("slip video");
        assert!(outcome.applied);

        let t = editor.project().active();
        let v = t.clip(&v_id).unwrap();
        let a = t.clip(&a_id).unwrap();

        assert_eq!(v.source_start, 9.0);
        assert_eq!(
            a.source_start, 9.0,
            "detached audio slipped in lockstep with video"
        );
        assert_eq!(v.start, 5.0);
        assert_eq!(a.start, 5.0);
        assert_eq!(v.duration, 10.0);
        assert_eq!(a.duration, 10.0);

        // 2. Slipped via audio clip: video must slip in lockstep
        let outcome2 = editor
            .apply(Command::SlipClip {
                clip_id: a_id.clone(),
                delta: -2.0,
            })
            .expect("slip audio");
        assert!(outcome2.applied);

        let t = editor.project().active();
        let v = t.clip(&v_id).unwrap();
        let a = t.clip(&a_id).unwrap();

        assert_eq!(v.source_start, 7.0);
        assert_eq!(
            a.source_start, 7.0,
            "video slipped in lockstep with detached audio"
        );
    }

    #[test]
    fn slip_clip_timeline_position_and_duration_unchanged() {
        let (mut editor, media_id, track, clip_id) = slip_fixture(60.0);

        // Place an adjacent clip right after it on the same track
        let adjacent_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 15.0,
                ripple: false,
            })
            .expect("adjacent clip")
            .created_id
            .unwrap();

        let initial_start = editor.project().active().clip(&clip_id).unwrap().start;
        let initial_dur = editor.project().active().clip(&clip_id).unwrap().duration;
        let adj_start = editor.project().active().clip(&adjacent_id).unwrap().start;

        editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 3.5,
            })
            .expect("slip");

        let t = editor.project().active();
        let c = t.clip(&clip_id).unwrap();
        let adj = t.clip(&adjacent_id).unwrap();

        assert_eq!(c.start, initial_start);
        assert_eq!(c.duration, initial_dur);
        assert_eq!(
            adj.start, adj_start,
            "adjacent clip timeline position untouched"
        );
    }

    #[test]
    fn slip_clip_keyframe_preservation() {
        use crate::model::{KeyEase, KeyProperty};
        let (mut editor, _media_id, _track, clip_id) = slip_fixture(60.0);

        // Place keyframes on opacity
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::Opacity,
                at: 0.25,
                value: 0.5,
                ease: KeyEase::LINEAR,
            })
            .expect("key 1");
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::Opacity,
                at: 0.75,
                value: 0.9,
                ease: KeyEase::LINEAR,
            })
            .expect("key 2");

        // Slip clip by +4.0s
        editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 4.0,
            })
            .expect("slip");

        let t = editor.project().active();
        let c = t.clip(&clip_id).unwrap();
        let keys: Vec<_> = c.keys_on(KeyProperty::Opacity).collect();
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0].at, 0.25);
        assert_eq!(keys[0].value, 0.5);
        assert_eq!(keys[1].at, 0.75);
        assert_eq!(keys[1].value, 0.9);
    }

    #[test]
    fn slip_clip_undo_redo_correctness() {
        let (mut editor, _media_id, _track, clip_id) = slip_fixture(60.0);

        let initial_project = editor.project().clone();

        editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 3.0,
            })
            .expect("slip");

        let modified_project = editor.project().clone();
        assert_ne!(&initial_project, &modified_project);

        // Undo restores exact initial project
        assert!(editor.undo());
        assert_eq!(editor.project(), &initial_project);

        // Redo restores exact modified project
        assert!(editor.redo());
        assert_eq!(editor.project(), &modified_project);
    }

    #[test]
    fn slip_clip_invalid_operation_leaves_project_unchanged() {
        let (mut editor, _media_id, _track, clip_id) = slip_fixture(60.0);

        let initial_project = editor.project().clone();

        // 1. Unknown clip ID
        let res = editor
            .apply(Command::SlipClip {
                clip_id: "nonexistent".to_owned(),
                delta: 2.0,
            })
            .expect("outcome");
        assert!(!res.applied);
        assert_eq!(editor.project(), &initial_project);

        // 2. Clip with no media (e.g. text clip)
        let text_id = text_at(&mut editor, 0.0, 4.0);
        let project_with_text = editor.project().clone();
        let res2 = editor
            .apply(Command::SlipClip {
                clip_id: text_id,
                delta: 2.0,
            })
            .expect("outcome");
        assert!(!res2.applied);
        assert_eq!(editor.project(), &project_with_text);

        // 3. Zero delta
        let res3 = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 0.0,
            })
            .expect("outcome");
        assert!(!res3.applied);
        assert_eq!(editor.project(), &project_with_text);

        // 4. NaN delta
        let err_nan = editor.apply(Command::SlipClip {
            clip_id: clip_id.clone(),
            delta: f64::NAN,
        });
        assert!(matches!(
            err_nan,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &project_with_text);

        // 5. Infinity delta
        let err_inf = editor.apply(Command::SlipClip {
            clip_id: clip_id.clone(),
            delta: f64::INFINITY,
        });
        assert!(matches!(
            err_inf,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &project_with_text);
    }

    fn slide_fixture(media_dur: f64) -> (Editor, String, String, String, String, String) {
        let mut editor = Editor::new();
        let media_id = editor
            .apply(media("/slide_media.mp4", media_dur, true))
            .expect("media")
            .created_id
            .unwrap();
        let track = editor.project().active().tracks[0].id.clone();

        // c1: 0..10
        let c1 = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("c1")
            .created_id
            .unwrap();
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&c1)
            .unwrap()
            .duration = 10.0;

        // c2: 10..20
        let c2 = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 10.0,
                ripple: false,
            })
            .expect("c2")
            .created_id
            .unwrap();
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&c2)
            .unwrap()
            .duration = 10.0;

        // c3: 20..30 with source_start = 5.0
        let c3 = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track.clone(),
                start: 20.0,
                ripple: false,
            })
            .expect("c3")
            .created_id
            .unwrap();
        let c3_ref = editor.project_mut().active_mut().clip_mut(&c3).unwrap();
        c3_ref.duration = 10.0;
        c3_ref.source_start = 5.0;

        (editor, media_id, track, c1, c2, c3)
    }

    #[test]
    fn slide_clip_positive_and_negative_deltas() {
        let (mut editor, _media_id, _track, c1, c2, c3) = slide_fixture(60.0);

        // Slide c2 right by +2.0s
        let outcome = editor
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: 2.0,
            })
            .expect("slide +2.0");
        assert!(outcome.applied);

        let t = editor.project().active();
        let p = t.clip(&c1).unwrap();
        let target = t.clip(&c2).unwrap();
        let n = t.clip(&c3).unwrap();

        // Prev clip: duration expanded to 12.0s, start stays 0.0
        assert_eq!(p.start, 0.0);
        assert_eq!(p.duration, 12.0);

        // Target clip: start slid to 12.0s, duration stays 10.0s, source_start unchanged
        assert_eq!(target.start, 12.0);
        assert_eq!(target.duration, 10.0);
        assert_eq!(target.source_start, 0.0);

        // Next clip: start moved to 22.0s, duration shrunk to 8.0s, source_start shifted to 7.0
        assert_eq!(n.start, 22.0);
        assert_eq!(n.duration, 8.0);
        assert_eq!(n.source_start, 7.0);

        // Combined duration is conserved
        assert_eq!(p.duration + target.duration + n.duration, 30.0);

        // Now slide c2 left by -3.0s
        let outcome2 = editor
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: -3.0,
            })
            .expect("slide -3.0");
        assert!(outcome2.applied);

        let t = editor.project().active();
        let p = t.clip(&c1).unwrap();
        let target = t.clip(&c2).unwrap();
        let n = t.clip(&c3).unwrap();

        assert_eq!(p.start, 0.0);
        assert_eq!(p.duration, 9.0);
        assert_eq!(target.start, 9.0);
        assert_eq!(target.duration, 10.0);
        assert_eq!(n.start, 19.0);
        assert_eq!(n.duration, 11.0);
        assert_eq!(n.source_start, 4.0);
        assert_eq!(p.duration + target.duration + n.duration, 30.0);
    }

    #[test]
    fn slide_clip_minimum_duration_boundary() {
        use crate::model::ranges::MIN_CLIP_DURATION;
        let (mut editor, _media_id, _track, c1, c2, c3) = slide_fixture(60.0);

        // Slide right by +50.0: c3 cannot shrink below MIN_CLIP_DURATION
        let outcome = editor
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: 50.0,
            })
            .expect("slide max right");
        assert!(outcome.applied);

        let t = editor.project().active();
        let p = t.clip(&c1).unwrap();
        let target = t.clip(&c2).unwrap();
        let n = t.clip(&c3).unwrap();

        assert!((n.duration - MIN_CLIP_DURATION).abs() < 1e-6);
        assert!((p.duration - (20.0 - MIN_CLIP_DURATION)).abs() < 1e-6);
        assert_eq!(target.duration, 10.0);
        assert!((p.duration + target.duration + n.duration - 30.0).abs() < 1e-6);

        // Fresh fixture for sliding left, with ample c3 source_start
        let (mut editor2, _media_id, _track, c1, c2, c3) = slide_fixture(60.0);
        editor2
            .project_mut()
            .active_mut()
            .clip_mut(&c3)
            .unwrap()
            .source_start = 20.0;

        // Slide left by -50.0: c1 cannot shrink below MIN_CLIP_DURATION
        let outcome2 = editor2
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: -50.0,
            })
            .expect("slide max left");
        assert!(outcome2.applied);

        let t = editor2.project().active();
        let p = t.clip(&c1).unwrap();
        let target = t.clip(&c2).unwrap();
        let n = t.clip(&c3).unwrap();

        assert!((p.duration - MIN_CLIP_DURATION).abs() < 1e-6);
        assert!((n.duration - (20.0 - MIN_CLIP_DURATION)).abs() < 1e-6);
        assert_eq!(target.duration, 10.0);
        assert!((p.duration + target.duration + n.duration - 30.0).abs() < 1e-6);
    }

    #[test]
    fn slide_clip_source_media_boundary() {
        let (mut editor, _media_id, _track, c1, c2, c3) = slide_fixture(30.0);

        // Set c3 source_start to 2.0s
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&c3)
            .unwrap()
            .source_start = 2.0;

        // Slide left by -10.0s: c3 source_start cannot go below 0.0 (clamped to -2.0)
        let outcome = editor
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: -10.0,
            })
            .expect("slide clamped at c3 source start");
        assert!(outcome.applied);

        let t = editor.project().active();
        let n = t.clip(&c3).unwrap();
        let target = t.clip(&c2).unwrap();
        let p = t.clip(&c1).unwrap();

        assert_eq!(n.source_start, 0.0, "c3 source_start clamped at 0.0");
        assert_eq!(target.start, 8.0, "c2 slid by -2.0s");
        assert_eq!(p.duration, 8.0, "c1 duration shrunk by -2.0s");
        assert_eq!(n.duration, 12.0, "c3 duration expanded by 2.0s");
    }

    #[test]
    fn slide_clip_missing_or_non_adjacent_neighbours() {
        let (mut editor, _media_id, _track, c1, c2, c3) = slide_fixture(60.0);

        let initial_project = editor.project().clone();

        // 1. Sliding c1 has no preceding clip on track
        let res1 = editor
            .apply(Command::SlideClip {
                clip_id: c1.clone(),
                delta: 1.0,
            })
            .expect("slide c1");
        assert!(!res1.applied);
        assert_eq!(editor.project(), &initial_project);

        // 2. Sliding c3 has no succeeding clip on track
        let res2 = editor
            .apply(Command::SlideClip {
                clip_id: c3.clone(),
                delta: 1.0,
            })
            .expect("slide c3");
        assert!(!res2.applied);
        assert_eq!(editor.project(), &initial_project);

        // 3. Move c3 to create gap of 1.0s between c2 and c3
        editor
            .apply(Command::MoveClips {
                moves: vec![crate::commands::ClipMove {
                    clip_id: c3.clone(),
                    track_id: _track.clone(),
                    start: 21.0,
                }],
            })
            .expect("move c3");
        let project_with_gap = editor.project().clone();

        let res3 = editor
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: 1.0,
            })
            .expect("slide c2 with gap");
        assert!(!res3.applied);
        assert_eq!(editor.project(), &project_with_gap);
    }

    #[test]
    fn slide_clip_linked_audio_video_synchronization() {
        let (mut editor, _media_id, _track, v1, v2, v3) = slide_fixture(60.0);

        // Detach audio for all 3 clips
        let a1 = editor
            .apply(Command::DetachAudio {
                clip_id: v1.clone(),
            })
            .expect("detach v1")
            .created_id
            .unwrap();
        let a2 = editor
            .apply(Command::DetachAudio {
                clip_id: v2.clone(),
            })
            .expect("detach v2")
            .created_id
            .unwrap();
        let a3 = editor
            .apply(Command::DetachAudio {
                clip_id: v3.clone(),
            })
            .expect("detach v3")
            .created_id
            .unwrap();

        // Ensure a3 matches v3's source_start
        editor
            .project_mut()
            .active_mut()
            .clip_mut(&a3)
            .unwrap()
            .source_start = 5.0;

        // Slide video c2 by +2.0s
        let outcome = editor
            .apply(Command::SlideClip {
                clip_id: v2.clone(),
                delta: 2.0,
            })
            .expect("slide video");
        assert!(outcome.applied);

        let t = editor.project().active();
        let v_prev = t.clip(&v1).unwrap();
        let v_target = t.clip(&v2).unwrap();
        let v_next = t.clip(&v3).unwrap();
        let a_prev = t.clip(&a1).unwrap();
        let a_target = t.clip(&a2).unwrap();
        let a_next = t.clip(&a3).unwrap();

        // Video and Audio must match exactly
        assert_eq!(v_prev.duration, 12.0);
        assert_eq!(a_prev.duration, 12.0);

        assert_eq!(v_target.start, 12.0);
        assert_eq!(a_target.start, 12.0);
        assert_eq!(v_target.duration, 10.0);
        assert_eq!(a_target.duration, 10.0);

        assert_eq!(v_next.start, 22.0);
        assert_eq!(a_next.start, 22.0);
        assert_eq!(v_next.duration, 8.0);
        assert_eq!(a_next.duration, 8.0);
        assert_eq!(v_next.source_start, 7.0);
        assert_eq!(a_next.source_start, 7.0);
    }

    #[test]
    fn slide_clip_keyframe_preservation_and_rewindowing() {
        use crate::model::{KeyEase, KeyProperty};
        let (mut editor, _media_id, _track, c1, c2, c3) = slide_fixture(60.0);

        // Add keyframes
        editor
            .apply(Command::SetClipKey {
                clip_id: c1.clone(),
                property: KeyProperty::Opacity,
                at: 0.5,
                value: 0.7,
                ease: KeyEase::LINEAR,
            })
            .expect("key c1");
        editor
            .apply(Command::SetClipKey {
                clip_id: c2.clone(),
                property: KeyProperty::Opacity,
                at: 0.4,
                value: 0.5,
                ease: KeyEase::LINEAR,
            })
            .expect("key c2");
        editor
            .apply(Command::SetClipKey {
                clip_id: c3.clone(),
                property: KeyProperty::Opacity,
                at: 0.6,
                value: 0.9,
                ease: KeyEase::LINEAR,
            })
            .expect("key c3");

        // Slide c2 by +2.0s
        editor
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: 2.0,
            })
            .expect("slide");

        let t = editor.project().active();
        let p = t.clip(&c1).unwrap();
        let target = t.clip(&c2).unwrap();
        let n = t.clip(&c3).unwrap();

        // Target keyframe envelope is completely untouched
        let target_keys: Vec<_> = target.keys_on(KeyProperty::Opacity).collect();
        assert_eq!(target_keys.len(), 1);
        assert_eq!(target_keys[0].at, 0.4);
        assert_eq!(target_keys[0].value, 0.5);

        // Prev clip: duration was 10.0, now 12.0. Key was at 5.0s => at is 5.0 / 12.0
        let p_keys: Vec<_> = p.keys_on(KeyProperty::Opacity).collect();
        assert_eq!(p_keys.len(), 1);
        assert!((p_keys[0].at - 5.0 / 12.0).abs() < 1e-6);

        // Next clip: duration was 10.0, now 8.0. Head moved +2.0s. Key was at 6.0s (timeline 26.0s).
        // In remaining 8.0s clip (22.0..30.0s), key sits at (26.0 - 22.0) / 8.0 = 4.0 / 8.0 = 0.5
        let n_keys: Vec<_> = n.keys_on(KeyProperty::Opacity).collect();
        assert_eq!(n_keys.len(), 1);
        assert!((n_keys[0].at - 0.5).abs() < 1e-6);
    }

    #[test]
    fn slide_clip_undo_redo_correctness() {
        let (mut editor, _media_id, _track, _c1, c2, _c3) = slide_fixture(60.0);

        let initial_project = editor.project().clone();

        editor
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: 2.5,
            })
            .expect("slide");

        let modified_project = editor.project().clone();
        assert_ne!(&initial_project, &modified_project);

        // Undo restores exact initial project
        assert!(editor.undo());
        assert_eq!(editor.project(), &initial_project);

        // Redo restores exact modified project
        assert!(editor.redo());
        assert_eq!(editor.project(), &modified_project);
    }

    #[test]
    fn slide_clip_invalid_operation_leaves_project_unchanged() {
        let (mut editor, _media_id, _track, _c1, c2, _c3) = slide_fixture(60.0);

        let initial_project = editor.project().clone();

        // 1. Unknown clip ID
        let res = editor
            .apply(Command::SlideClip {
                clip_id: "nonexistent".to_owned(),
                delta: 2.0,
            })
            .expect("outcome");
        assert!(!res.applied);
        assert_eq!(editor.project(), &initial_project);

        // 2. Zero delta
        let res2 = editor
            .apply(Command::SlideClip {
                clip_id: c2.clone(),
                delta: 0.0,
            })
            .expect("outcome");
        assert!(!res2.applied);
        assert_eq!(editor.project(), &initial_project);

        // 3. NaN delta
        let err_nan = editor.apply(Command::SlideClip {
            clip_id: c2.clone(),
            delta: f64::NAN,
        });
        assert!(matches!(
            err_nan,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &initial_project);

        // 4. Infinity delta
        let err_inf = editor.apply(Command::SlideClip {
            clip_id: c2.clone(),
            delta: f64::INFINITY,
        });
        assert!(matches!(
            err_inf,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &initial_project);
    }

    #[test]
    fn add_marker_point_and_span() {
        let mut editor = Editor::new();

        // 1. Point marker at 5.0s
        let outcome1 = editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: 5.0,
                duration: None,
                name: Some("Beat 1".to_owned()),
                color: Some("yellow".to_owned()),
            })
            .expect("add marker 1");
        assert!(outcome1.applied);
        let id1 = outcome1.created_id.expect("id1");

        // 2. Span marker at 12.0s with duration 4.0s
        let outcome2 = editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: 12.0,
                duration: Some(4.0),
                name: Some("Chorus".to_owned()),
                color: Some("magenta".to_owned()),
            })
            .expect("add marker 2");
        assert!(outcome2.applied);
        let id2 = outcome2.created_id.expect("id2");

        // 3. Point marker earlier at 2.0s to test sorting
        let outcome3 = editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: 2.0,
                duration: Some(0.0),
                name: Some("Intro".to_owned()),
                color: None,
            })
            .expect("add marker 3");
        let id3 = outcome3.created_id.expect("id3");

        let tl = editor.project().active();
        assert_eq!(tl.markers.len(), 3);

        // Sorted by time
        assert_eq!(tl.markers[0].id, id3);
        assert_eq!(tl.markers[0].time, 2.0);
        assert_eq!(tl.markers[0].name, "Intro");
        assert_eq!(tl.markers[0].color, "cyan", "defaults to cyan");

        assert_eq!(tl.markers[1].id, id1);
        assert_eq!(tl.markers[1].time, 5.0);
        assert_eq!(tl.markers[1].duration, 0.0);
        assert_eq!(tl.markers[1].name, "Beat 1");
        assert_eq!(tl.markers[1].color, "yellow");

        assert_eq!(tl.markers[2].id, id2);
        assert_eq!(tl.markers[2].time, 12.0);
        assert_eq!(tl.markers[2].duration, 4.0);
        assert_eq!(tl.markers[2].name, "Chorus");
        assert_eq!(tl.markers[2].color, "magenta");
    }

    #[test]
    fn add_marker_defaults_and_clamping() {
        let mut editor = Editor::new();

        let outcome = editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: -10.0,
                duration: Some(-2.5),
                name: None,
                color: None,
            })
            .expect("add clamped marker");
        let id = outcome.created_id.expect("id");

        let marker = editor.project().active().marker(&id).expect("marker found");
        assert_eq!(marker.time, 0.0, "time floored at 0.0");
        assert_eq!(marker.duration, 0.0, "duration floored at 0.0");
        assert_eq!(marker.name, "Marker", "defaults to 'Marker'");
        assert_eq!(marker.color, "cyan", "defaults to 'cyan'");
    }

    #[test]
    fn remove_marker_success_and_noop() {
        let mut editor = Editor::new();
        let m1 = editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: 3.0,
                duration: None,
                name: None,
                color: None,
            })
            .expect("add")
            .created_id
            .unwrap();
        let m2 = editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: 6.0,
                duration: None,
                name: None,
                color: None,
            })
            .expect("add")
            .created_id
            .unwrap();

        assert_eq!(editor.project().active().markers.len(), 2);

        // Remove m1
        let outcome = editor
            .apply(Command::RemoveMarker {
                timeline_id: None,
                marker_id: m1.clone(),
            })
            .expect("remove m1");
        assert!(outcome.applied);
        assert_eq!(editor.project().active().markers.len(), 1);
        assert_eq!(editor.project().active().markers[0].id, m2);

        // Remove nonexistent marker
        let outcome2 = editor
            .apply(Command::RemoveMarker {
                timeline_id: None,
                marker_id: "nonexistent".to_owned(),
            })
            .expect("remove nonexistent");
        assert!(!outcome2.applied);
        assert_eq!(editor.project().active().markers.len(), 1);
    }

    #[test]
    fn markers_project_persistence_roundtrip() {
        let mut editor = Editor::new();

        // 1. When empty, markers field is omitted from json
        let empty_json = serde_json::to_string(editor.project()).expect("serialize empty");
        assert!(!empty_json.contains("\"markers\""));

        // 2. Add markers and serialize
        editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: 4.5,
                duration: Some(2.0),
                name: Some("Solo Section".to_owned()),
                color: Some("purple".to_owned()),
            })
            .expect("add");

        let json = serde_json::to_string(editor.project()).expect("serialize with marker");
        assert!(json.contains("\"markers\""));
        assert!(json.contains("\"Solo Section\""));
        assert!(json.contains("\"purple\""));

        // 3. Deserialize back
        let restored: crate::model::Project = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.active().markers.len(), 1);
        let m = &restored.active().markers[0];
        assert_eq!(m.time, 4.5);
        assert_eq!(m.duration, 2.0);
        assert_eq!(m.name, "Solo Section");
        assert_eq!(m.color, "purple");
    }

    #[test]
    fn markers_undo_redo_correctness() {
        let mut editor = Editor::new();
        let initial_project = editor.project().clone();

        let outcome = editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: 7.5,
                duration: Some(1.0),
                name: Some("Drop".to_owned()),
                color: Some("red".to_owned()),
            })
            .expect("add");
        let m_id = outcome.created_id.unwrap();

        // Undo restores initial state (0 markers)
        assert!(editor.undo());
        assert_eq!(editor.project(), &initial_project);
        assert_eq!(editor.project().active().markers.len(), 0);

        // Redo restores marker
        assert!(editor.redo());
        assert_eq!(editor.project().active().markers.len(), 1);
        assert_eq!(editor.project().active().markers[0].id, m_id);

        // Remove marker
        editor
            .apply(Command::RemoveMarker {
                timeline_id: None,
                marker_id: m_id.clone(),
            })
            .expect("remove");
        assert_eq!(editor.project().active().markers.len(), 0);

        // Undo restores marker
        assert!(editor.undo());
        assert_eq!(editor.project().active().markers.len(), 1);

        // Redo removes marker again
        assert!(editor.redo());
        assert_eq!(editor.project().active().markers.len(), 0);
    }

    #[test]
    fn markers_id_mint_adoption() {
        let editor = Editor::new();
        let mut p = editor.project().clone();
        p.active_mut().markers.push(crate::model::TimelineMarker {
            id: "mk88".to_owned(),
            time: 1.0,
            duration: 0.0,
            name: "Test".to_owned(),
            color: "cyan".to_owned(),
        });

        // Adopt project
        let doc = serde_json::to_value(&p).expect("doc");
        let mut new_editor = Editor::from_document(&doc).expect("editor from doc");

        // Next minted marker should be past mk88 -> mk89
        let outcome = new_editor
            .apply(Command::AddMarker {
                timeline_id: None,
                time: 5.0,
                duration: None,
                name: None,
                color: None,
            })
            .expect("add marker");
        assert_eq!(outcome.created_id.unwrap(), "mk89");
    }

    #[test]
    fn markers_specific_timeline_targeting() {
        let mut editor = Editor::new();

        // Add second timeline
        let tl2_id = editor
            .apply(Command::AddTimeline)
            .expect("tl2")
            .created_id
            .unwrap();

        // Active is currently tl2. Switch back to founding timeline TL1
        editor
            .apply(Command::SelectTimeline {
                timeline_id: "TL1".to_owned(),
            })
            .expect("select TL1");
        assert_eq!(editor.project().active_timeline_id, "TL1");

        // Add marker targeting TL2 specifically
        let outcome = editor
            .apply(Command::AddMarker {
                timeline_id: Some(tl2_id.clone()),
                time: 15.0,
                duration: Some(2.0),
                name: Some("TL2 Marker".to_owned()),
                color: Some("green".to_owned()),
            })
            .expect("add marker to tl2");
        let m_id = outcome.created_id.unwrap();

        // Active timeline TL1 has 0 markers
        assert_eq!(editor.project().active().markers.len(), 0);

        // Targeted timeline TL2 has the marker
        let tl2 = editor
            .project()
            .timelines
            .iter()
            .find(|tl| tl.id == tl2_id)
            .unwrap();
        assert_eq!(tl2.markers.len(), 1);
        assert_eq!(tl2.markers[0].id, m_id);
        assert_eq!(tl2.markers[0].name, "TL2 Marker");
    }

    #[test]
    fn markers_invalid_non_finite_rejected() {
        let mut editor = Editor::new();
        let initial_project = editor.project().clone();

        // 1. NaN time
        let err_nan = editor.apply(Command::AddMarker {
            timeline_id: None,
            time: f64::NAN,
            duration: None,
            name: None,
            color: None,
        });
        assert!(matches!(
            err_nan,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &initial_project);

        // 2. Inf time
        let err_inf = editor.apply(Command::AddMarker {
            timeline_id: None,
            time: f64::INFINITY,
            duration: None,
            name: None,
            color: None,
        });
        assert!(matches!(
            err_inf,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &initial_project);

        // 3. NaN duration
        let err_dur_nan = editor.apply(Command::AddMarker {
            timeline_id: None,
            time: 5.0,
            duration: Some(f64::NAN),
            name: None,
            color: None,
        });
        assert!(matches!(
            err_dur_nan,
            Err(crate::commands::CommandError::NotANumber)
        ));
        assert_eq!(editor.project(), &initial_project);
    }

    #[test]
    fn track_lock_prevents_edits_and_modifications() {
        let (mut editor, media_id, clip_id) = fixture();
        let track_id = editor.project().active().tracks[0].id.clone();
        let track2_id = editor
            .apply(Command::AddTrack { bottom: false })
            .expect("add track 2")
            .created_id
            .unwrap();
        let clip2_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track2_id.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("add clip on track 2")
            .created_id
            .unwrap();

        let clip_adj_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 10.0,
                ripple: false,
            })
            .expect("add adjacent clip on track 1")
            .created_id
            .unwrap();

        // Lock track 1
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_id.clone(),
                flag: TrackFlag::Locked,
                value: true,
            })
            .expect("lock track 1");
        assert!(editor.project().active().is_track_locked(&track_id));
        assert!(!editor.project().active().is_track_locked(&track2_id));

        // 1. Adding clip to locked track fails
        let add_err = editor.apply(Command::AddClip {
            media_id: media_id.clone(),
            track_id: track_id.clone(),
            start: 10.0,
            ripple: false,
        });
        assert!(matches!(
            add_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 2. Moving clip on locked track fails
        let move_err = editor.apply(Command::MoveClips {
            moves: vec![ClipMove {
                clip_id: clip_id.clone(),
                track_id: track_id.clone(),
                start: 5.0,
            }],
        });
        assert!(matches!(
            move_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 3. Moving clip from unlocked track onto locked track fails
        let move_onto_err = editor.apply(Command::MoveClips {
            moves: vec![ClipMove {
                clip_id: clip2_id.clone(),
                track_id: track_id.clone(),
                start: 15.0,
            }],
        });
        assert!(matches!(
            move_onto_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 4. Trimming clip on locked track fails
        let trim_err = editor.apply(Command::TrimClip {
            clip_id: clip_id.clone(),
            edge: TrimEdge::End,
            delta: -2.0,
            ripple: false,
        });
        assert!(matches!(
            trim_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 5. Slip edit on locked track fails
        let slip_err = editor.apply(Command::SlipClip {
            clip_id: clip_id.clone(),
            delta: 1.0,
        });
        assert!(matches!(
            slip_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 5b. Roll trim on locked track fails
        let roll_err = editor.apply(Command::RollTrim {
            head_clip_id: clip_id.clone(),
            tail_clip_id: clip_adj_id.clone(),
            delta: 1.0,
        });
        assert!(matches!(
            roll_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 6. Slide edit on locked track fails
        let slide_err = editor.apply(Command::SlideClip {
            clip_id: clip_id.clone(),
            delta: 1.0,
        });
        assert!(matches!(
            slide_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 7. Modifying properties on locked track fails
        let prop_err = editor.apply(Command::UpdateClip {
            clip_id: clip_id.clone(),
            patch: ClipPatch {
                volume: Some(0.5),
                ..Default::default()
            },
        });
        assert!(matches!(
            prop_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 8. Detaching audio on locked track fails
        let detach_err = editor.apply(Command::DetachAudio {
            clip_id: clip_id.clone(),
        });
        assert!(matches!(
            detach_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 9. Deleting locked track fails
        let remove_track_err = editor.apply(Command::RemoveTrack {
            track_id: track_id.clone(),
        });
        assert!(matches!(
            remove_track_err,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // 10. SplitClips skips clips on locked tracks without modifying them
        let orig_clip = editor.project().active().clip(&clip_id).cloned().unwrap();
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: 2.0,
            })
            .expect("split tolerates locked track by skipping");
        assert_eq!(
            editor.project().active().clip(&clip_id).unwrap(),
            &orig_clip
        );

        // 11. RemoveClips leaves clips on locked tracks untouched
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![clip_id.clone()],
                ripple: false,
            })
            .expect("remove tolerates locked track by skipping");
        assert!(editor.project().active().clip(&clip_id).is_some());

        // Unlock track 1 and verify edits succeed
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_id.clone(),
                flag: TrackFlag::Locked,
                value: false,
            })
            .expect("unlock track 1");
        assert!(!editor.project().active().is_track_locked(&track_id));

        editor
            .apply(Command::UpdateClip {
                clip_id: clip_id.clone(),
                patch: ClipPatch {
                    volume: Some(0.8),
                    ..Default::default()
                },
            })
            .expect("update clip succeeds when unlocked");
    }

    #[test]
    fn track_lock_and_solo_undo_redo_and_persistence() {
        let mut editor = Editor::new();
        let track_id = editor.project().active().tracks[0].id.clone();

        // Lock and Solo
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_id.clone(),
                flag: TrackFlag::Locked,
                value: true,
            })
            .expect("lock");
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_id.clone(),
                flag: TrackFlag::Solo,
                value: true,
            })
            .expect("solo");

        assert!(editor.project().active().tracks[0].locked);
        assert!(editor.project().active().tracks[0].solo);

        // Serialization roundtrip
        let serialized = serde_json::to_string(editor.project()).expect("serialize");
        let restored: Project = serde_json::from_str(&serialized).expect("deserialize");
        assert!(restored.active().tracks[0].locked);
        assert!(restored.active().tracks[0].solo);

        // Undo solo
        assert!(editor.undo(), "undo solo");
        assert!(editor.project().active().tracks[0].locked);
        assert!(!editor.project().active().tracks[0].solo);

        // Undo lock
        assert!(editor.undo(), "undo lock");
        assert!(!editor.project().active().tracks[0].locked);
        assert!(!editor.project().active().tracks[0].solo);

        // Redo lock
        assert!(editor.redo(), "redo lock");
        assert!(editor.project().active().tracks[0].locked);
        assert!(!editor.project().active().tracks[0].solo);

        // Redo solo
        assert!(editor.redo(), "redo solo");
        assert!(editor.project().active().tracks[0].locked);
        assert!(editor.project().active().tracks[0].solo);
    }

    #[test]
    fn roll_trim_comprehensive_validation() {
        let mut editor = Editor::new();
        let track_id = editor.project().active().tracks[0].id.clone();

        // Add 20s media
        let media_id = editor
            .apply(media("/roll_test.mp4", 20.0, true))
            .expect("add media")
            .created_id
            .expect("id");

        // Clip 1: [0, 5), source_start: 0, duration: 5
        let clip1_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("add clip 1")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: clip1_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim clip 1 to 5s");

        // Clip 2: [5, 10), source_start: 5, duration: 5
        let clip2_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 5.0,
                ripple: false,
            })
            .expect("add clip 2")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: clip2_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim clip 2 to 5s");
        editor
            .apply(Command::SlipClip {
                clip_id: clip2_id.clone(),
                delta: 5.0,
            })
            .expect("slip clip 2 source_start to 5s");

        let c1 = editor.project().active().clip(&clip1_id).unwrap().clone();
        let c2 = editor.project().active().clip(&clip2_id).unwrap().clone();
        assert!((c1.duration - 5.0).abs() < 1e-4);
        assert!((c2.start - 5.0).abs() < 1e-4);
        assert!((c2.duration - 5.0).abs() < 1e-4);
        assert!((c2.source_start - 5.0).abs() < 1e-4);

        // 1. Positive Roll (+2.0s): Clip 1 grows to 7s, Clip 2 shrinks to 3s (starts at 7s, source_start=7s)
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: clip1_id.clone(),
                tail_clip_id: clip2_id.clone(),
                delta: 2.0,
            })
            .expect("roll +2s");
        assert!(outcome.applied);

        let c1_after = editor.project().active().clip(&clip1_id).unwrap();
        let c2_after = editor.project().active().clip(&clip2_id).unwrap();
        assert!((c1_after.duration - 7.0).abs() < 1e-4);
        assert!((c2_after.start - 7.0).abs() < 1e-4);
        assert!((c2_after.duration - 3.0).abs() < 1e-4);
        assert!((c2_after.source_start - 7.0).abs() < 1e-4);
        // Overall sequence duration stays exactly 10s
        assert!((c2_after.start + c2_after.duration - 10.0).abs() < 1e-4);

        // 2. Negative Roll (-3.0s): Clip 1 shrinks to 4s, Clip 2 grows to 6s (starts at 4s, source_start=4s)
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: clip1_id.clone(),
                tail_clip_id: clip2_id.clone(),
                delta: -3.0,
            })
            .expect("roll -3s");
        assert!(outcome.applied);

        let c1_after = editor.project().active().clip(&clip1_id).unwrap();
        let c2_after = editor.project().active().clip(&clip2_id).unwrap();
        assert!((c1_after.duration - 4.0).abs() < 1e-4);
        assert!((c2_after.start - 4.0).abs() < 1e-4);
        assert!((c2_after.duration - 6.0).abs() < 1e-4);
        assert!((c2_after.source_start - 4.0).abs() < 1e-4);
        assert!((c2_after.start + c2_after.duration - 10.0).abs() < 1e-4);

        // 3. Undo / Redo cycle
        assert!(editor.undo(), "undo negative roll");
        let c1_undone = editor.project().active().clip(&clip1_id).unwrap();
        assert!((c1_undone.duration - 7.0).abs() < 1e-4);

        assert!(editor.undo(), "undo positive roll");
        let c1_orig = editor.project().active().clip(&clip1_id).unwrap();
        assert!((c1_orig.duration - 5.0).abs() < 1e-4);

        assert!(editor.redo(), "redo positive roll");
        let c1_redone = editor.project().active().clip(&clip1_id).unwrap();
        assert!((c1_redone.duration - 7.0).abs() < 1e-4);
    }

    #[test]
    fn slip_clip_comprehensive_validation() {
        let mut editor = Editor::new();
        let track_id = editor.project().active().tracks[0].id.clone();

        // 20s media
        let media_id = editor
            .apply(media("/slip_test.mp4", 20.0, true))
            .expect("add media")
            .created_id
            .expect("id");

        // Clip at [5, 10), duration 5, source_start = 5
        let clip_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 5.0,
                ripple: false,
            })
            .expect("add clip")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: clip_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim to 5s");
        editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 5.0,
            })
            .expect("set initial source_start to 5s");

        // 1. Positive slip (+3s): source_start becomes 8s, timeline start/duration unchanged
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 3.0,
            })
            .expect("slip +3s");
        assert!(outcome.applied);
        let c = editor.project().active().clip(&clip_id).unwrap();
        assert!((c.start - 5.0).abs() < 1e-4);
        assert!((c.duration - 5.0).abs() < 1e-4);
        assert!((c.source_start - 8.0).abs() < 1e-4);

        // 2. Negative slip (-6s): source_start becomes 2s
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: -6.0,
            })
            .expect("slip -6s");
        assert!(outcome.applied);
        let c = editor.project().active().clip(&clip_id).unwrap();
        assert!((c.source_start - 2.0).abs() < 1e-4);

        // 3. Lower boundary clamping: delta -10s clamps source_start to 0s
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: -10.0,
            })
            .expect("slip clamped to 0s");
        assert!(outcome.applied);
        let c = editor.project().active().clip(&clip_id).unwrap();
        assert!((c.source_start - 0.0).abs() < 1e-4);

        // 4. Upper boundary clamping: media is 20s, clip duration is 5s, max source_start is 15s
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: clip_id.clone(),
                delta: 25.0,
            })
            .expect("slip clamped to max media duration");
        assert!(outcome.applied);
        let c = editor.project().active().clip(&clip_id).unwrap();
        assert!((c.source_start - 15.0).abs() < 1e-4);

        // 5. Undo / Redo
        assert!(editor.undo());
        let c = editor.project().active().clip(&clip_id).unwrap();
        assert!((c.source_start - 0.0).abs() < 1e-4);
        assert!(editor.redo());
        let c = editor.project().active().clip(&clip_id).unwrap();
        assert!((c.source_start - 15.0).abs() < 1e-4);
    }

    #[test]
    fn slide_clip_comprehensive_validation() {
        let mut editor = Editor::new();
        let track_id = editor.project().active().tracks[0].id.clone();

        // 30s media
        let media_id = editor
            .apply(media("/slide_test.mp4", 30.0, true))
            .expect("add media")
            .created_id
            .expect("id");

        // Clip 1 (Prev): [0, 4)
        let c1_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("add c1")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: c1_id.clone(),
                edge: TrimEdge::End,
                delta: -26.0,
                ripple: false,
            })
            .expect("trim c1 to 4s");

        // Clip 2 (Target): [4, 7)
        let c2_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 4.0,
                ripple: false,
            })
            .expect("add c2")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: c2_id.clone(),
                edge: TrimEdge::End,
                delta: -27.0,
                ripple: false,
            })
            .expect("trim c2 to 3s");

        // Clip 3 (Next): [7, 12), source_start = 5.0
        let c3_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 7.0,
                ripple: false,
            })
            .expect("add c3")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: c3_id.clone(),
                edge: TrimEdge::End,
                delta: -25.0,
                ripple: false,
            })
            .expect("trim c3 to 5s");
        editor
            .apply(Command::SlipClip {
                clip_id: c3_id.clone(),
                delta: 5.0,
            })
            .expect("slip c3 to 5s");

        // 1. Slide Right (+1.5s):
        // Prev expands: 4.0 -> 5.5s
        // Target shifts: 4.0 -> 5.5s (duration stays 3.0s)
        // Next shrinks & shifts head: 7.0 -> 8.5s (duration 5.0 -> 3.5s, source_start 5.0 -> 6.5s)
        let outcome = editor
            .apply(Command::SlideClip {
                clip_id: c2_id.clone(),
                delta: 1.5,
            })
            .expect("slide +1.5s");
        assert!(outcome.applied);

        let p = editor.project().active().clip(&c1_id).unwrap();
        let t = editor.project().active().clip(&c2_id).unwrap();
        let n = editor.project().active().clip(&c3_id).unwrap();
        assert!((p.duration - 5.5).abs() < 1e-4);
        assert!((t.start - 5.5).abs() < 1e-4);
        assert!((t.duration - 3.0).abs() < 1e-4);
        assert!((n.start - 8.5).abs() < 1e-4);
        assert!((n.duration - 3.5).abs() < 1e-4);
        assert!((n.source_start - 6.5).abs() < 1e-4);

        // 2. Slide Left (-2.0s):
        // Prev shrinks: 5.5 -> 3.5s
        // Target shifts: 5.5 -> 3.5s (duration stays 3.0s)
        // Next expands: 8.5 -> 6.5s (duration 3.5 -> 5.5s, source_start 6.5 -> 4.5s)
        let outcome = editor
            .apply(Command::SlideClip {
                clip_id: c2_id.clone(),
                delta: -2.0,
            })
            .expect("slide -2.0s");
        assert!(outcome.applied);

        let p = editor.project().active().clip(&c1_id).unwrap();
        let t = editor.project().active().clip(&c2_id).unwrap();
        let n = editor.project().active().clip(&c3_id).unwrap();
        assert!((p.duration - 3.5).abs() < 1e-4);
        assert!((t.start - 3.5).abs() < 1e-4);
        assert!((t.duration - 3.0).abs() < 1e-4);
        assert!((n.start - 6.5).abs() < 1e-4);
        assert!((n.duration - 5.5).abs() < 1e-4);
        assert!((n.source_start - 4.5).abs() < 1e-4);

        // 3. Undo / Redo
        assert!(editor.undo(), "undo slide left");
        let t = editor.project().active().clip(&c2_id).unwrap();
        assert!((t.start - 5.5).abs() < 1e-4);

        assert!(editor.undo(), "undo slide right");
        let t = editor.project().active().clip(&c2_id).unwrap();
        assert!((t.start - 4.0).abs() < 1e-4);

        assert!(editor.redo(), "redo slide right");
        let t = editor.project().active().clip(&c2_id).unwrap();
        assert!((t.start - 5.5).abs() < 1e-4);
    }

    #[test]
    fn roll_slip_slide_linked_av_synchronization() {
        let mut editor = Editor::new();
        let track_v = editor.project().active().tracks[0].id.clone();

        let media_id = editor
            .apply(media("/linked_av.mp4", 20.0, true))
            .expect("add media")
            .created_id
            .expect("id");

        // Video clips: V1 [0, 5), V2 [5, 10)
        let v1_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_v.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("add v1")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: v1_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim v1");

        let v2_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_v.clone(),
                start: 5.0,
                ripple: false,
            })
            .expect("add v2")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: v2_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim v2");

        // Detach audio for V1 and V2
        let a1_id = editor
            .apply(Command::DetachAudio {
                clip_id: v1_id.clone(),
            })
            .expect("detach a1")
            .created_id
            .expect("a1 id");

        let a2_id = editor
            .apply(Command::DetachAudio {
                clip_id: v2_id.clone(),
            })
            .expect("detach a2")
            .created_id
            .expect("a2 id");

        // 1. RollTrim on (V1, V2) rolls both Video and Audio simultaneously
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: v1_id.clone(),
                tail_clip_id: v2_id.clone(),
                delta: 1.0,
            })
            .expect("roll linked av");
        assert!(outcome.applied);

        let v1 = editor.project().active().clip(&v1_id).unwrap();
        let a1 = editor.project().active().clip(&a1_id).unwrap();
        let v2 = editor.project().active().clip(&v2_id).unwrap();
        let a2 = editor.project().active().clip(&a2_id).unwrap();

        assert_eq!(v1.duration, a1.duration);
        assert_eq!(v2.start, a2.start);
        assert_eq!(v2.duration, a2.duration);
        assert!((v1.duration - 6.0).abs() < 1e-4);
        assert!((v2.duration - 4.0).abs() < 1e-4);

        // 2. SlipClip on V1 slips both V1 and linked A1
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: v1_id.clone(),
                delta: 2.0,
            })
            .expect("slip linked av");
        assert!(outcome.applied);

        let v1 = editor.project().active().clip(&v1_id).unwrap();
        let a1 = editor.project().active().clip(&a1_id).unwrap();
        assert_eq!(v1.source_start, a1.source_start);
        assert!((v1.source_start - 2.0).abs() < 1e-4);
    }

    #[test]
    fn locked_track_rejection_across_all_tools() {
        let mut editor = Editor::new();
        let track_v = editor.project().active().tracks[0].id.clone();

        let media_id = editor
            .apply(media("/locked_test.mp4", 20.0, true))
            .expect("add media")
            .created_id
            .expect("id");

        // V1 [0, 5), V2 [5, 10), V3 [10, 15)
        let v1_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_v.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("add v1")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: v1_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim v1");

        let v2_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_v.clone(),
                start: 5.0,
                ripple: false,
            })
            .expect("add v2")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: v2_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim v2");

        let v3_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_v.clone(),
                start: 10.0,
                ripple: false,
            })
            .expect("add v3")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: v3_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim v3");

        // Linked audio clip for V2
        let a2_id = editor
            .apply(Command::DetachAudio {
                clip_id: v2_id.clone(),
            })
            .expect("detach a2")
            .created_id
            .expect("a2 id");
        let a2_track = editor
            .project()
            .active()
            .clip(&a2_id)
            .unwrap()
            .track_id
            .clone();

        // 1. Lock track_v: RollTrim, SlipClip, SlideClip all fail with TrackLocked
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_v.clone(),
                flag: TrackFlag::Locked,
                value: true,
            })
            .expect("lock track_v");

        assert!(matches!(
            editor.apply(Command::RollTrim {
                head_clip_id: v1_id.clone(),
                tail_clip_id: v2_id.clone(),
                delta: 1.0,
            }),
            Err(crate::commands::CommandError::TrackLocked)
        ));

        assert!(matches!(
            editor.apply(Command::SlipClip {
                clip_id: v2_id.clone(),
                delta: 1.0,
            }),
            Err(crate::commands::CommandError::TrackLocked)
        ));

        assert!(matches!(
            editor.apply(Command::SlideClip {
                clip_id: v2_id.clone(),
                delta: 1.0,
            }),
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // Unlock track_v, but Lock a2_track: operations on V2 affecting linked A2 must fail with TrackLocked!
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_v.clone(),
                flag: TrackFlag::Locked,
                value: false,
            })
            .expect("unlock track_v");
        editor
            .apply(Command::SetTrackFlag {
                track_id: a2_track.clone(),
                flag: TrackFlag::Locked,
                value: true,
            })
            .expect("lock a2_track");

        assert!(matches!(
            editor.apply(Command::SlipClip {
                clip_id: v2_id.clone(),
                delta: 1.0,
            }),
            Err(crate::commands::CommandError::TrackLocked)
        ));
    }

    #[test]
    fn invalid_operation_rollback_guarantee() {
        let mut editor = Editor::new();
        let track_id = editor.project().active().tracks[0].id.clone();

        let media_id = editor
            .apply(media("/rollback_test.mp4", 20.0, true))
            .expect("add media")
            .created_id
            .expect("id");

        // Disjoint clips: C1 at [0, 5), C2 at [10, 15)
        let c1_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 0.0,
                ripple: false,
            })
            .expect("add c1")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: c1_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim c1");

        let c2_id = editor
            .apply(Command::AddClip {
                media_id: media_id.clone(),
                track_id: track_id.clone(),
                start: 10.0,
                ripple: false,
            })
            .expect("add c2")
            .created_id
            .expect("id");
        editor
            .apply(Command::TrimClip {
                clip_id: c2_id.clone(),
                edge: TrimEdge::End,
                delta: -15.0,
                ripple: false,
            })
            .expect("trim c2");

        let snapshot_before = serde_json::to_string(editor.project()).expect("snapshot");

        // Non-adjacent RollTrim returns applied: false and leaves state completely unchanged
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: c1_id.clone(),
                tail_clip_id: c2_id.clone(),
                delta: 1.0,
            })
            .expect("non-adjacent roll");
        assert!(!outcome.applied);

        let snapshot_after = serde_json::to_string(editor.project()).expect("snapshot");
        assert_eq!(snapshot_before, snapshot_after);

        // SlideClip with missing neighbour returns applied: false
        let outcome = editor
            .apply(Command::SlideClip {
                clip_id: c1_id.clone(),
                delta: 1.0,
            })
            .expect("slide with missing neighbour");
        assert!(!outcome.applied);
        let snapshot_after2 = serde_json::to_string(editor.project()).expect("snapshot");
        assert_eq!(snapshot_before, snapshot_after2);
    }

    #[test]
    fn performance_500_clips_timeline() {
        let mut editor = Editor::new();
        let track_id = editor.project().active().tracks[0].id.clone();

        let media_id = editor
            .apply(media("/perf_test.mp4", 1000.0, false))
            .expect("add media")
            .created_id
            .expect("id");

        // Populate 500 contiguous clips: 1.0s each
        let mut clip_ids = Vec::with_capacity(500);
        for i in 0..500 {
            let start = i as f64;
            let cid = editor
                .apply(Command::AddClip {
                    media_id: media_id.clone(),
                    track_id: track_id.clone(),
                    start,
                    ripple: false,
                })
                .expect("add clip")
                .created_id
                .expect("id");
            editor
                .apply(Command::TrimClip {
                    clip_id: cid.clone(),
                    edge: TrimEdge::End,
                    delta: -999.0,
                    ripple: false,
                })
                .expect("trim to 1s");
            clip_ids.push(cid);
        }

        assert_eq!(editor.project().active().clips.len(), 500);

        // Benchmark RollTrim on middle cut
        let t0 = std::time::Instant::now();
        let outcome = editor
            .apply(Command::RollTrim {
                head_clip_id: clip_ids[250].clone(),
                tail_clip_id: clip_ids[251].clone(),
                delta: 0.2,
            })
            .expect("roll trim on 500-clip timeline");
        let roll_duration = t0.elapsed();
        assert!(outcome.applied);
        assert!(
            roll_duration.as_millis() < 50,
            "RollTrim took {:?}",
            roll_duration
        );

        // Benchmark SlipClip on clip 250
        let t1 = std::time::Instant::now();
        let outcome = editor
            .apply(Command::SlipClip {
                clip_id: clip_ids[250].clone(),
                delta: 0.1,
            })
            .expect("slip clip on 500-clip timeline");
        let slip_duration = t1.elapsed();
        assert!(outcome.applied);
        assert!(
            slip_duration.as_millis() < 50,
            "SlipClip took {:?}",
            slip_duration
        );

        // Benchmark SlideClip on clip 250
        let t2 = std::time::Instant::now();
        let outcome = editor
            .apply(Command::SlideClip {
                clip_id: clip_ids[250].clone(),
                delta: 0.1,
            })
            .expect("slide clip on 500-clip timeline");
        let slide_duration = t2.elapsed();
        assert!(outcome.applied);
        assert!(
            slide_duration.as_millis() < 50,
            "SlideClip took {:?}",
            slide_duration
        );

        // Verify undo performance
        let t3 = std::time::Instant::now();
        assert!(editor.undo());
        let undo_duration = t3.elapsed();
        assert!(
            undo_duration.as_millis() < 50,
            "Undo took {:?}",
            undo_duration
        );
    }

    #[test]
    fn test_media_and_timeline_marks_and_undo() {
        let mut editor = Editor::default();
        let m_id = editor
            .apply(media("/test.mp4", 10.0, true))
            .unwrap()
            .created_id
            .unwrap();

        // 1. Set Media Marks
        let outcome = editor
            .apply(Command::SetMediaMarks {
                media_id: m_id.clone(),
                source_in: Some(2.0),
                source_out: Some(6.0),
            })
            .unwrap();
        assert!(outcome.applied);
        let m = editor.project().media_by_id(&m_id).unwrap();
        assert_eq!(m.source_in, Some(2.0));
        assert_eq!(m.source_out, Some(6.0));

        // 2. Clear Media Marks
        let outcome = editor
            .apply(Command::ClearMediaMarks {
                media_id: m_id.clone(),
            })
            .unwrap();
        assert!(outcome.applied);
        let m = editor.project().media_by_id(&m_id).unwrap();
        assert_eq!(m.source_in, None);
        assert_eq!(m.source_out, None);

        // Undo Clear -> Marks restored
        assert!(editor.undo());
        let m = editor.project().media_by_id(&m_id).unwrap();
        assert_eq!(m.source_in, Some(2.0));
        assert_eq!(m.source_out, Some(6.0));

        // 3. Set Timeline Marks
        let outcome = editor
            .apply(Command::SetTimelineMarks {
                timeline_id: None,
                mark_in: Some(1.0),
                mark_out: Some(5.0),
            })
            .unwrap();
        assert!(outcome.applied);
        assert_eq!(editor.project().active().mark_in, Some(1.0));
        assert_eq!(editor.project().active().mark_out, Some(5.0));

        // 4. Clear Timeline Marks
        let outcome = editor
            .apply(Command::ClearTimelineMarks { timeline_id: None })
            .unwrap();
        assert!(outcome.applied);
        assert_eq!(editor.project().active().mark_in, None);
        assert_eq!(editor.project().active().mark_out, None);

        // Undo -> restored
        assert!(editor.undo());
        assert_eq!(editor.project().active().mark_in, Some(1.0));
        assert_eq!(editor.project().active().mark_out, Some(5.0));
    }

    #[test]
    fn test_three_point_insert_edit_combinations_and_sync() {
        let mut editor = Editor::default();
        let m_id = editor
            .apply(media("/footage.mp4", 10.0, true))
            .unwrap()
            .created_id
            .unwrap();
        let track_v = editor.project().active().tracks[0].id.clone();
        let track_a = editor.project().active().tracks[1].id.clone();

        // Place a pre-existing 6-second video clip and linked audio clip at t=0
        let v1_id = editor
            .apply(Command::AddClip {
                media_id: m_id.clone(),
                track_id: track_v.clone(),
                start: 0.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();
        // Set duration to 6.0 (media is 10.0s, delta -4.0s)
        editor
            .apply(Command::TrimClip {
                clip_id: v1_id.clone(),
                edge: crate::commands::TrimEdge::End,
                delta: -4.0,
                ripple: false,
            })
            .unwrap();
        // Detach audio to get linked audio clip on track_a
        editor
            .apply(Command::DetachAudio {
                clip_id: v1_id.clone(),
            })
            .unwrap();
        // Move detached audio to track_a
        let a1_id = editor
            .project()
            .active()
            .clips
            .iter()
            .find(|c| c.detached_from.as_deref() == Some(&v1_id))
            .unwrap()
            .id
            .clone();
        editor
            .apply(Command::MoveClips {
                moves: vec![crate::commands::ClipMove {
                    clip_id: a1_id.clone(),
                    track_id: track_a.clone(),
                    start: 0.0,
                }],
            })
            .unwrap();

        // Existing clips: V1 on track_v from 0..6, A1 on track_a from 0..6 (linked)
        assert_eq!(editor.project().active().clips.len(), 2);

        // Test 1: Insert Edit at timeline_in = 2.0 with Source In=1.0, Source Out=4.0 (duration=3.0)
        // Should split straddling V1 and A1 at 2.0, ripple downstream halves from 2.0 to 5.0,
        // and insert new V and A clips at [2.0, 5.0] linked together.
        let outcome = editor
            .apply(Command::InsertEdit {
                media_id: m_id.clone(),
                source_in: Some(1.0),
                source_out: Some(4.0),
                timeline_in: Some(2.0),
                timeline_out: None,
                video_track_id: Some(track_v.clone()),
                audio_track_id: Some(track_a.clone()),
            })
            .unwrap();
        assert!(outcome.applied);

        // Check timeline state:
        // track_v should have:
        // - head V1 [0.0..2.0]
        // - inserted V [2.0..5.0] (source_start = 1.0, duration = 3.0)
        // - tail V1 [5.0..9.0]
        // track_a should have:
        // - head A1 [0.0..2.0]
        // - inserted A [2.0..5.0] (linked to inserted V)
        // - tail A1 [5.0..9.0] (linked to tail V1)
        let tl = editor.project().active();
        assert_eq!(tl.clips.len(), 6);

        let mut v_clips: Vec<_> = tl.clips.iter().filter(|c| c.track_id == track_v).collect();
        v_clips.sort_by(|a, b| a.start.total_cmp(&b.start));
        assert_eq!(v_clips.len(), 3);
        assert!((v_clips[0].start - 0.0).abs() < 1e-4);
        assert!((v_clips[0].duration - 2.0).abs() < 1e-4);
        assert!((v_clips[1].start - 2.0).abs() < 1e-4);
        assert!((v_clips[1].duration - 3.0).abs() < 1e-4);
        assert_eq!(v_clips[1].source_start, 1.0);
        assert!((v_clips[2].start - 5.0).abs() < 1e-4);
        assert!((v_clips[2].duration - 4.0).abs() < 1e-4);

        let mut a_clips: Vec<_> = tl.clips.iter().filter(|c| c.track_id == track_a).collect();
        a_clips.sort_by(|a, b| a.start.total_cmp(&b.start));
        assert_eq!(a_clips.len(), 3);
        assert!((a_clips[0].start - 0.0).abs() < 1e-4);
        assert!((a_clips[1].start - 2.0).abs() < 1e-4);
        assert!((a_clips[1].duration - 3.0).abs() < 1e-4);
        assert_eq!(a_clips[1].source_start, 1.0);
        assert_eq!(
            a_clips[1].detached_from.as_deref(),
            Some(v_clips[1].id.as_str())
        );
        assert!((a_clips[2].start - 5.0).abs() < 1e-4);
        assert_eq!(
            a_clips[2].detached_from.as_deref(),
            Some(v_clips[2].id.as_str())
        );

        // Test Locked Track Rejection
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_v.clone(),
                flag: crate::commands::TrackFlag::Locked,
                value: true,
            })
            .unwrap();
        let err = editor.apply(Command::InsertEdit {
            media_id: m_id.clone(),
            source_in: Some(0.0),
            source_out: Some(2.0),
            timeline_in: Some(1.0),
            timeline_out: None,
            video_track_id: Some(track_v.clone()),
            audio_track_id: Some(track_a.clone()),
        });
        assert_eq!(err, Err(crate::commands::CommandError::TrackLocked));

        // Unlock track
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_v.clone(),
                flag: crate::commands::TrackFlag::Locked,
                value: false,
            })
            .unwrap();

        // Undo back to before the insert
        assert!(editor.undo()); // undo unlock
        assert!(editor.undo()); // undo lock
        assert!(editor.undo()); // undo insert
        assert_eq!(editor.project().active().clips.len(), 2);
    }

    #[test]
    fn test_three_point_overwrite_edit_and_boundaries() {
        let mut editor = Editor::default();
        let m_id = editor
            .apply(media("/clip.mp4", 20.0, false))
            .unwrap()
            .created_id
            .unwrap();
        let track_v = editor.project().active().tracks[0].id.clone();
        let track_other = editor.project().active().tracks[1].id.clone();

        // Place clip on track_v from 2.0 to 12.0 (duration 10.0 from 20.0s media)
        let v1_id = editor
            .apply(Command::AddClip {
                media_id: m_id.clone(),
                track_id: track_v.clone(),
                start: 2.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();
        editor
            .apply(Command::TrimClip {
                clip_id: v1_id.clone(),
                edge: crate::commands::TrimEdge::End,
                delta: -10.0, // 20.0 -> 10.0
                ripple: false,
            })
            .unwrap();

        // Place clip on track_other from 0.0 to 15.0 to verify unaffected track never ripples
        let other_id = editor
            .apply(Command::AddClip {
                media_id: m_id.clone(),
                track_id: track_other.clone(),
                start: 0.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();
        editor
            .apply(Command::TrimClip {
                clip_id: other_id.clone(),
                edge: crate::commands::TrimEdge::End,
                delta: -5.0, // 20.0 -> 15.0
                ripple: false,
            })
            .unwrap();

        // Overwrite Edit: punch hole in middle of V1: [5.0..8.0] with source [1.0..4.0]
        let outcome = editor
            .apply(Command::OverwriteEdit {
                media_id: m_id.clone(),
                source_in: Some(1.0),
                source_out: Some(4.0),
                timeline_in: Some(5.0),
                timeline_out: None,
                video_track_id: Some(track_v.clone()),
                audio_track_id: None,
            })
            .unwrap();
        assert!(outcome.applied);

        // Track V should now have 3 clips:
        // 1. Head: [2.0..5.0] (duration 3.0)
        // 2. Overwritten middle: [5.0..8.0] (duration 3.0, source_start = 1.0)
        // 3. Tail: [8.0..12.0] (duration 4.0, source_start = 6.0)
        let mut v_clips: Vec<_> = editor
            .project()
            .active()
            .clips
            .iter()
            .filter(|c| c.track_id == track_v)
            .collect();
        v_clips.sort_by(|a, b| a.start.total_cmp(&b.start));
        assert_eq!(v_clips.len(), 3);
        assert!((v_clips[0].start - 2.0).abs() < 1e-4);
        assert!((v_clips[0].duration - 3.0).abs() < 1e-4);
        assert!((v_clips[1].start - 5.0).abs() < 1e-4);
        assert!((v_clips[1].duration - 3.0).abs() < 1e-4);
        assert_eq!(v_clips[1].source_start, 1.0);
        assert!((v_clips[2].start - 8.0).abs() < 1e-4);
        assert!((v_clips[2].duration - 4.0).abs() < 1e-4);
        assert!((v_clips[2].source_start - 6.0).abs() < 1e-4);

        let head_id = v_clips[0].id.clone();

        // Crucial: track_other must remain COMPLETELY UNTOUCHED at [0.0..15.0]
        let other_clip = editor
            .project()
            .active()
            .clips
            .iter()
            .find(|c| c.track_id == track_other)
            .unwrap();
        assert!((other_clip.start - 0.0).abs() < 1e-4);
        assert!((other_clip.duration - 15.0).abs() < 1e-4);

        // Now test Overwrite boundary trimming: overwrite [1.0..3.0]
        // This overlaps left boundary of head clip [2.0..5.0]
        editor
            .apply(Command::OverwriteEdit {
                media_id: m_id.clone(),
                source_in: Some(0.0),
                source_out: Some(2.0),
                timeline_in: Some(1.0),
                timeline_out: None,
                video_track_id: Some(track_v.clone()),
                audio_track_id: None,
            })
            .unwrap();
        // The head clip was [2.0..5.0], overwritten by [1.0..3.0], so head clip's left gets trimmed to [3.0..5.0]
        let trimmed_head = editor
            .project()
            .active()
            .clips
            .iter()
            .find(|c| c.id == head_id)
            .unwrap();
        assert!((trimmed_head.start - 3.0).abs() < 1e-4);
        assert!((trimmed_head.duration - 2.0).abs() < 1e-4);
        let trimmed_head_id = trimmed_head.id.clone();

        // Test complete enclosure removal: overwrite [2.5..5.5]
        // Trimmed head is [3.0..5.0], completely inside [2.5..5.5], so it gets removed!
        editor
            .apply(Command::OverwriteEdit {
                media_id: m_id.clone(),
                source_in: Some(0.0),
                source_out: Some(3.0),
                timeline_in: Some(2.5),
                timeline_out: None,
                video_track_id: Some(track_v.clone()),
                audio_track_id: None,
            })
            .unwrap();
        assert!(editor.project().active().clip(&trimmed_head_id).is_none());

        // Full Undo chain puts everything back exactly
        assert!(editor.undo());
        assert!(editor.undo());
        assert!(editor.undo());
        let original_v = editor.project().active().clip(&v1_id).unwrap();
        assert!((original_v.start - 2.0).abs() < 1e-4);
        assert!((original_v.duration - 10.0).abs() < 1e-4);
    }

    #[test]
    fn test_three_point_locked_track_rejection_and_track_patching() {
        let mut editor = Editor::default();
        let m_id = editor
            .apply(media("/test.mp4", 15.0, true))
            .unwrap()
            .created_id
            .unwrap();

        let track_v = editor.project().active().tracks[0].id.clone();
        let track_a = editor
            .apply(Command::AddTrack { bottom: true })
            .unwrap()
            .created_id
            .unwrap();

        // 1. Lock track_v and verify InsertEdit rejects with TrackLocked
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_v.clone(),
                flag: TrackFlag::Locked,
                value: true,
            })
            .unwrap();

        let err_insert = editor.apply(Command::InsertEdit {
            media_id: m_id.clone(),
            source_in: Some(1.0),
            source_out: Some(5.0),
            timeline_in: Some(0.0),
            timeline_out: None,
            video_track_id: Some(track_v.clone()),
            audio_track_id: Some(track_a.clone()),
        });
        assert!(matches!(
            err_insert,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // Verify OverwriteEdit also rejects with TrackLocked
        let err_overwrite = editor.apply(Command::OverwriteEdit {
            media_id: m_id.clone(),
            source_in: Some(1.0),
            source_out: Some(5.0),
            timeline_in: Some(0.0),
            timeline_out: None,
            video_track_id: Some(track_v.clone()),
            audio_track_id: Some(track_a.clone()),
        });
        assert!(matches!(
            err_overwrite,
            Err(crate::commands::CommandError::TrackLocked)
        ));

        // Unlock track_v
        editor
            .apply(Command::SetTrackFlag {
                track_id: track_v.clone(),
                flag: TrackFlag::Locked,
                value: false,
            })
            .unwrap();

        // 2. Track patching: Insert only Video (audio_track_id = None)
        editor
            .apply(Command::InsertEdit {
                media_id: m_id.clone(),
                source_in: Some(2.0),
                source_out: Some(6.0),
                timeline_in: Some(0.0),
                timeline_out: None,
                video_track_id: Some(track_v.clone()),
                audio_track_id: None,
            })
            .unwrap();

        let clips = &editor.project().active().clips;
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].kind, ClipKind::Video);
        assert_eq!(clips[0].track_id, track_v);
        assert!((clips[0].duration - 4.0).abs() < 1e-4);

        // 3. Track patching: Overwrite only Audio (video_track_id = None)
        editor
            .apply(Command::OverwriteEdit {
                media_id: m_id.clone(),
                source_in: Some(0.0),
                source_out: Some(5.0),
                timeline_in: Some(0.0),
                timeline_out: None,
                video_track_id: None,
                audio_track_id: Some(track_a.clone()),
            })
            .unwrap();

        let clips = &editor.project().active().clips;
        assert_eq!(clips.len(), 2);
        let audio_clip = clips.iter().find(|c| c.kind == ClipKind::Audio).unwrap();
        assert_eq!(audio_clip.track_id, track_a);
        assert!((audio_clip.duration - 5.0).abs() < 1e-4);

        // 4. When all tracks in the timeline are locked, fallback to None/None rejects with TrackLocked
        for track in editor.project().active().tracks.clone() {
            editor
                .apply(Command::SetTrackFlag {
                    track_id: track.id,
                    flag: TrackFlag::Locked,
                    value: true,
                })
                .unwrap();
        }
        let err_none = editor.apply(Command::InsertEdit {
            media_id: m_id.clone(),
            source_in: None,
            source_out: None,
            timeline_in: None,
            timeline_out: None,
            video_track_id: None,
            audio_track_id: None,
        });
        assert!(matches!(
            err_none,
            Err(crate::commands::CommandError::TrackLocked)
        ));
    }

    #[test]
    fn test_bin_folders_crud_move_reparent_and_undo() {
        let mut editor = Editor::new();
        let m_outcome = editor
            .apply(media("C:/media/clip1.mp4", 10.0, false))
            .unwrap();
        let m_id = m_outcome.created_id.unwrap();

        // 1. Create root bin folder
        let f1 = editor
            .apply(Command::CreateBinFolder {
                name: "B-Roll".into(),
                parent_id: None,
                color: Some("#0088ff".into()),
            })
            .unwrap();
        let f1_id = f1.created_id.expect("folder id created");
        assert_eq!(editor.project().folders.len(), 1);
        assert_eq!(editor.project().folders[0].name, "B-Roll");
        assert_eq!(editor.project().folders[0].color, "#0088ff");

        // 2. Create nested subfolder
        let f2 = editor
            .apply(Command::CreateBinFolder {
                name: "Drone".into(),
                parent_id: Some(f1_id.clone()),
                color: None,
            })
            .unwrap();
        let f2_id = f2.created_id.expect("subfolder id created");
        assert_eq!(editor.project().folders.len(), 2);
        let children = editor.project().child_folders(Some(&f1_id));
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, f2_id);

        // 3. Move media to subfolder
        editor
            .apply(Command::MoveMediaToBin {
                media_ids: vec![m_id.clone()],
                folder_id: Some(f2_id.clone()),
            })
            .unwrap();
        assert_eq!(
            editor.project().media_by_id(&m_id).unwrap().folder_id,
            Some(f2_id.clone())
        );
        let in_f2 = editor.project().media_in_folder(Some(&f2_id));
        assert_eq!(in_f2.len(), 1);
        assert_eq!(in_f2[0].id, m_id);

        // 4. Rename subfolder
        editor
            .apply(Command::RenameBinFolder {
                folder_id: f2_id.clone(),
                name: "Drone 4K".into(),
            })
            .unwrap();
        assert_eq!(
            editor.project().folder_by_id(&f2_id).unwrap().name,
            "Drone 4K"
        );

        // 5. Delete subfolder -> media reparented to parent (f1_id)
        editor
            .apply(Command::DeleteBinFolder {
                folder_id: f2_id.clone(),
            })
            .unwrap();
        assert_eq!(editor.project().folders.len(), 1);
        assert_eq!(editor.project().folder_by_id(&f2_id), None);
        assert_eq!(
            editor.project().media_by_id(&m_id).unwrap().folder_id,
            Some(f1_id.clone())
        );

        // 6. Undo deletion
        editor.undo();
        assert_eq!(editor.project().folders.len(), 2);
        assert_eq!(
            editor.project().folder_by_id(&f2_id).unwrap().name,
            "Drone 4K"
        );
        assert_eq!(
            editor.project().media_by_id(&m_id).unwrap().folder_id,
            Some(f2_id.clone())
        );

        // 7. Redo deletion
        editor.redo();
        assert_eq!(editor.project().folders.len(), 1);
        assert_eq!(
            editor.project().media_by_id(&m_id).unwrap().folder_id,
            Some(f1_id.clone())
        );
    }

    #[test]
    fn test_compound_clips_creation_nesting_and_undo() {
        let mut editor = Editor::new();
        let m1 = editor
            .apply(media("vid1.mp4", 30.0, true))
            .unwrap()
            .created_id
            .unwrap();
        let m2 = editor
            .apply(media("vid2.mp4", 30.0, true))
            .unwrap()
            .created_id
            .unwrap();

        let c1 = editor
            .apply(Command::AddClip {
                media_id: m1,
                track_id: "T1".into(),
                start: 5.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();
        // Set c1 duration to 10.0
        editor
            .apply(Command::TrimClip {
                clip_id: c1.clone(),
                edge: TrimEdge::End,
                delta: -20.0,
                ripple: false,
            })
            .unwrap();

        let c2 = editor
            .apply(Command::AddClip {
                media_id: m2,
                track_id: "T2".into(),
                start: 8.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();
        // Set c2 duration to 12.0
        editor
            .apply(Command::TrimClip {
                clip_id: c2.clone(),
                edge: TrimEdge::End,
                delta: -18.0,
                ripple: false,
            })
            .unwrap();

        assert_eq!(editor.project().active().clips.len(), 2);

        // 1. Create Compound Clip
        let compound_id = editor
            .apply(Command::CreateCompoundClip {
                clip_ids: vec![c1.clone(), c2.clone()],
                name: Some("Intro Scene".into()),
            })
            .unwrap()
            .created_id
            .unwrap();

        // 2. Verify parent timeline state
        let parent = editor.project().active();
        assert_eq!(parent.clips.len(), 1);
        let compound_clip = parent.clip(&compound_id).unwrap();
        assert_eq!(compound_clip.kind, ClipKind::Sequence);
        assert_eq!(compound_clip.name, "Intro Scene");
        assert!((compound_clip.start - 5.0).abs() < 1e-4);
        assert!((compound_clip.duration - 15.0).abs() < 1e-4); // (8.0 + 12.0) - 5.0 = 15.0

        let child_tl_id = compound_clip.media_id.clone();
        assert_eq!(editor.project().timelines.len(), 2);
        let child_tl = editor.project().timeline_by_id(&child_tl_id).unwrap();
        assert_eq!(child_tl.name, "Intro Scene");
        assert_eq!(child_tl.clips.len(), 2);

        // Verify child clips re-anchored to 0.0 at min_start (5.0)
        let child_c1 = child_tl.clips.iter().find(|c| c.track_id == "T1").unwrap();
        assert!((child_c1.start - 0.0).abs() < 1e-4);
        assert!((child_c1.duration - 10.0).abs() < 1e-4);

        let child_c2 = child_tl.clips.iter().find(|c| c.track_id == "T2").unwrap();
        assert!((child_c2.start - 3.0).abs() < 1e-4); // 8.0 - 5.0 = 3.0
        assert!((child_c2.duration - 12.0).abs() < 1e-4);

        // 3. Test independent editing of child sequence
        editor
            .apply(Command::SelectTimeline {
                timeline_id: child_tl_id.clone(),
            })
            .unwrap();
        assert_eq!(editor.project().active_timeline_id, child_tl_id);
        // Rename child timeline
        editor
            .apply(Command::RenameTimeline {
                timeline_id: child_tl_id.clone(),
                name: "Intro Scene v2".into(),
            })
            .unwrap();
        assert_eq!(
            editor.project().timeline_by_id(&child_tl_id).unwrap().name,
            "Intro Scene v2"
        );

        // Switch back to parent timeline
        editor
            .apply(Command::SelectTimeline {
                timeline_id: "TL1".into(),
            })
            .unwrap();
        assert_eq!(editor.project().active_timeline_id, "TL1");

        // 4. Test Undo creation (SelectTimeline is not recorded in undo history)
        editor.undo(); // undo rename
        editor.undo(); // undo compound creation

        let restored_parent = editor.project().active();
        assert_eq!(restored_parent.clips.len(), 2);
        assert!(restored_parent.clip(&c1).is_some());
        assert!(restored_parent.clip(&c2).is_some());
        assert_eq!(editor.project().timelines.len(), 1);

        // 5. Test Redo
        editor.redo();
        let redone_tl1 = editor.project().timeline_by_id("TL1").unwrap();
        assert_eq!(redone_tl1.clips.len(), 1);
        assert!(redone_tl1.clip(&compound_id).is_some());
        assert_eq!(editor.project().timelines.len(), 2);
    }

    #[test]
    fn test_compound_clips_decompose_and_window_trimming() {
        let mut editor = Editor::new();
        let m1 = editor
            .apply(media("vid1.mp4", 30.0, true))
            .unwrap()
            .created_id
            .unwrap();
        let m2 = editor
            .apply(media("vid2.mp4", 30.0, true))
            .unwrap()
            .created_id
            .unwrap();

        let c1 = editor
            .apply(Command::AddClip {
                media_id: m1,
                track_id: "T1".into(),
                start: 0.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();
        editor
            .apply(Command::TrimClip {
                clip_id: c1.clone(),
                edge: TrimEdge::End,
                delta: -20.0, // duration 10.0
                ripple: false,
            })
            .unwrap();

        let c2 = editor
            .apply(Command::AddClip {
                media_id: m2,
                track_id: "T2".into(),
                start: 5.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();
        editor
            .apply(Command::TrimClip {
                clip_id: c2.clone(),
                edge: TrimEdge::End,
                delta: -20.0, // duration 10.0 (ends at 15.0)
                ripple: false,
            })
            .unwrap();

        let compound_id = editor
            .apply(Command::CreateCompoundClip {
                clip_ids: vec![c1.clone(), c2.clone()],
                name: Some("Comp1".into()),
            })
            .unwrap()
            .created_id
            .unwrap();

        // Trim the compound clip: move source_start to 2.0, trim duration to 8.0 (span 2.0..10.0)
        editor
            .apply(Command::SlipClip {
                clip_id: compound_id.clone(),
                delta: 2.0,
            })
            .unwrap();
        editor
            .apply(Command::TrimClip {
                clip_id: compound_id.clone(),
                edge: TrimEdge::End,
                delta: -7.0, // 15.0 - 7.0 = 8.0
                ripple: false,
            })
            .unwrap();

        let comp = editor.project().active().clip(&compound_id).unwrap();
        assert!((comp.source_start - 2.0).abs() < 1e-4);
        assert!((comp.duration - 8.0).abs() < 1e-4);

        // Decompose the trimmed compound clip
        editor
            .apply(Command::DecomposeCompoundClip {
                clip_id: compound_id.clone(),
            })
            .unwrap();

        let decomposed = editor.project().active();
        assert_eq!(decomposed.clips.len(), 2);
        assert!(decomposed.clip(&compound_id).is_none());

        // c1 was originally 0..10. Overlap with [2.0, 10.0] is [2.0, 10.0] (duration 8.0, start_trim 2.0)
        let dec_c1 = decomposed
            .clips
            .iter()
            .find(|c| c.track_id == "T1")
            .unwrap();
        assert!((dec_c1.start - 0.0).abs() < 1e-4);
        assert!((dec_c1.duration - 8.0).abs() < 1e-4);
        assert!((dec_c1.source_start - 2.0).abs() < 1e-4);

        // c2 was originally 5..15. Overlap with [2.0, 10.0] is [5.0, 10.0] (duration 5.0, starts at 3.0 in comp)
        let dec_c2 = decomposed
            .clips
            .iter()
            .find(|c| c.track_id == "T2")
            .unwrap();
        assert!((dec_c2.start - 3.0).abs() < 1e-4); // (5.0 - 2.0) = 3.0
        assert!((dec_c2.duration - 5.0).abs() < 1e-4);
        assert!((dec_c2.source_start - 0.0).abs() < 1e-4);

        // Undo decompose
        editor.undo();
        let restored_comp = editor.project().active();
        assert_eq!(restored_comp.clips.len(), 1);
        assert!(restored_comp.clip(&compound_id).is_some());
    }

    #[test]
    fn test_compound_clips_cycle_detection_and_locked_tracks() {
        let mut editor = Editor::new();
        let m1 = editor
            .apply(media("vid1.mp4", 30.0, true))
            .unwrap()
            .created_id
            .unwrap();

        let c1 = editor
            .apply(Command::AddClip {
                media_id: m1,
                track_id: "T1".into(),
                start: 0.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();

        let compound_id = editor
            .apply(Command::CreateCompoundClip {
                clip_ids: vec![c1.clone()],
                name: Some("Level1".into()),
            })
            .unwrap()
            .created_id
            .unwrap();

        let comp = editor.project().active().clip(&compound_id).unwrap();
        let child_tl_id = comp.media_id.clone();

        // Cycle detection:
        // Inserting TL1 into child_tl_id would create a cycle because TL1 contains child_tl_id!
        assert!(editor.project().would_create_cycle(&child_tl_id, "TL1"));
        // Inserting child_tl_id into child_tl_id is self-cycle
        assert!(
            editor
                .project()
                .would_create_cycle(&child_tl_id, &child_tl_id)
        );
        // Inserting child_tl_id into TL1 is what already happened (not a new cycle from TL1 perspective)
        assert!(!editor.project().would_create_cycle("TL1", &child_tl_id));

        // Test locked track enforcement:
        // Lock T1
        editor
            .apply(Command::SetTrackFlag {
                track_id: "T1".into(),
                flag: TrackFlag::Locked,
                value: true,
            })
            .unwrap();

        // Decompose on locked track must be rejected with TrackLocked
        let err = editor
            .apply(Command::DecomposeCompoundClip {
                clip_id: compound_id.clone(),
            })
            .unwrap_err();
        assert_eq!(err, crate::commands::CommandError::TrackLocked);

        // Unlock T1
        editor
            .apply(Command::SetTrackFlag {
                track_id: "T1".into(),
                flag: TrackFlag::Locked,
                value: false,
            })
            .unwrap();

        // Now decompose succeeds
        editor
            .apply(Command::DecomposeCompoundClip {
                clip_id: compound_id,
            })
            .unwrap();
    }

    #[test]
    fn test_hold_and_custom_bezier_interpolation() {
        use crate::model::{KeyEase, KeyProperty};
        use clipcat_core::animate::Ease;

        // 1. Verify Ease::HOLD interpolation
        let hold_ease = Ease::HOLD;
        assert!(hold_ease.is_hold());
        assert_eq!(hold_ease.apply(0.0), 0.0);
        assert_eq!(hold_ease.apply(0.5), 0.0);
        assert_eq!(hold_ease.apply(0.99), 0.0);
        assert_eq!(hold_ease.apply(1.0), 1.0);

        // 2. Verify KeyEase::HOLD round-trip and preset
        assert!(KeyEase::HOLD.is_hold());
        assert_eq!(KeyEase::from_name("hold"), KeyEase::HOLD);

        // 3. Setup editor with clip
        let mut editor = Editor::new();
        let m1 = editor
            .apply(media("vid1.mp4", 20.0, true))
            .unwrap()
            .created_id
            .unwrap();
        let clip_outcome = editor
            .apply(Command::AddClip {
                track_id: "T1".into(),
                media_id: m1,
                start: 0.0,
                ripple: false,
            })
            .unwrap();
        let clip_id = clip_outcome.created_id.unwrap();

        // 4. Set key at 0.0 with value 0.1 and at 0.5 with value 0.8 (Hold ease)
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetX,
                at: 0.0,
                value: 0.1,
                ease: KeyEase::LINEAR,
            })
            .unwrap();
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetX,
                at: 0.5,
                value: 0.8,
                ease: KeyEase::HOLD,
            })
            .unwrap();

        let clip = editor.project().active().clip(&clip_id).unwrap();
        // At 0.25 (between 0.0 and 0.5), value should hold at 0.1 because ease into 0.5 is HOLD
        assert!((clip.value_at(KeyProperty::OffsetX, 0.25) - 0.1).abs() < 1e-4);
        // At 0.5 and after, value should be 0.8
        assert!((clip.value_at(KeyProperty::OffsetX, 0.5) - 0.8).abs() < 1e-4);
        assert!((clip.value_at(KeyProperty::OffsetX, 0.75) - 0.8).abs() < 1e-4);

        // 5. Test SetKeyEase command
        editor
            .apply(Command::SetKeyEase {
                clip_id: clip_id.clone(),
                property: KeyProperty::OffsetX,
                at: 0.5,
                ease: KeyEase::LINEAR,
            })
            .unwrap();
        let clip = editor.project().active().clip(&clip_id).unwrap();
        // Now linear interpolation between 0.1 and 0.8 at t=0.25 should be 0.45
        assert!((clip.value_at(KeyProperty::OffsetX, 0.25) - 0.45).abs() < 1e-4);

        // 6. Test undo and redo of SetKeyEase
        assert!(editor.undo());
        let clip = editor.project().active().clip(&clip_id).unwrap();
        assert!((clip.value_at(KeyProperty::OffsetX, 0.25) - 0.1).abs() < 1e-4);

        assert!(editor.redo());
        let clip = editor.project().active().clip(&clip_id).unwrap();
        assert!((clip.value_at(KeyProperty::OffsetX, 0.25) - 0.45).abs() < 1e-4);
    }

    #[test]
    fn test_independent_scalex_scaley_and_crop_keyframes() {
        use crate::model::{KeyEase, KeyProperty};

        let mut editor = Editor::new();
        let m1 = editor
            .apply(media("vid1.mp4", 20.0, true))
            .unwrap()
            .created_id
            .unwrap();
        let clip_outcome = editor
            .apply(Command::AddClip {
                track_id: "T1".into(),
                media_id: m1,
                start: 0.0,
                ripple: false,
            })
            .unwrap();
        let clip_id = clip_outcome.created_id.unwrap();

        // 1. Keyframe ScaleX and ScaleY independently
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::ScaleX,
                at: 0.0,
                value: 1.0,
                ease: KeyEase::LINEAR,
            })
            .unwrap();
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::ScaleX,
                at: 1.0,
                value: 2.0,
                ease: KeyEase::LINEAR,
            })
            .unwrap();

        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::ScaleY,
                at: 0.0,
                value: 0.5,
                ease: KeyEase::LINEAR,
            })
            .unwrap();
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::ScaleY,
                at: 1.0,
                value: 1.5,
                ease: KeyEase::LINEAR,
            })
            .unwrap();

        // 2. Keyframe CropLeft and CropBottom
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::CropLeft,
                at: 0.0,
                value: 0.1,
                ease: KeyEase::LINEAR,
            })
            .unwrap();
        editor
            .apply(Command::SetClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::CropLeft,
                at: 1.0,
                value: 0.4,
                ease: KeyEase::LINEAR,
            })
            .unwrap();

        let clip = editor.project().active().clip(&clip_id).unwrap();
        assert!((clip.value_at(KeyProperty::ScaleX, 0.5) - 1.5).abs() < 1e-4);
        assert!((clip.value_at(KeyProperty::ScaleY, 0.5) - 1.0).abs() < 1e-4);
        assert!((clip.value_at(KeyProperty::CropLeft, 0.5) - 0.25).abs() < 1e-4);

        // 3. Test key properties metadata and names
        assert_eq!(KeyProperty::ScaleX.name(), "scaleX");
        assert_eq!(KeyProperty::from_name("scaleX"), Some(KeyProperty::ScaleX));
        assert_eq!(KeyProperty::CropLeft.name(), "cropLeft");
        assert_eq!(
            KeyProperty::from_name("cropLeft"),
            Some(KeyProperty::CropLeft)
        );
    }

    #[test]
    fn test_motion_graphics_presets_and_curve_retiming() {
        use crate::model::KeyProperty;

        let mut editor = Editor::new();
        let m1 = editor
            .apply(media("vid1.mp4", 20.0, true))
            .unwrap()
            .created_id
            .unwrap();
        let clip_outcome = editor
            .apply(Command::AddClip {
                track_id: "T1".into(),
                media_id: m1.clone(),
                start: 0.0,
                ripple: false,
            })
            .unwrap();
        let clip_id = clip_outcome.created_id.unwrap();

        // 1. Apply 'pop-in' animation preset
        editor
            .apply(Command::ApplyAnimationPreset {
                clip_id: clip_id.clone(),
                preset: "pop-in".into(),
            })
            .unwrap();

        let clip = editor.project().active().clip(&clip_id).unwrap();
        assert!(clip.is_keyed(KeyProperty::Scale));
        assert_eq!(clip.keys_on(KeyProperty::Scale).count(), 3);
        assert!((clip.value_at(KeyProperty::Scale, 0.0) - 0.05).abs() < 1e-4);
        assert!((clip.value_at(KeyProperty::Scale, 0.20) - 1.0).abs() < 1e-4);

        // 2. Test MoveClipKey
        editor
            .apply(Command::MoveClipKey {
                clip_id: clip_id.clone(),
                property: KeyProperty::Scale,
                from: 0.20,
                to: 0.30,
            })
            .unwrap();

        let clip = editor.project().active().clip(&clip_id).unwrap();
        assert!(clip.key_at(KeyProperty::Scale, 0.30).is_some());
        assert!(clip.key_at(KeyProperty::Scale, 0.20).is_none());

        // 3. Test CopyPasteKeys to another clip
        let clip2_outcome = editor
            .apply(Command::AddClip {
                track_id: "T2".into(),
                media_id: m1,
                start: 0.0,
                ripple: false,
            })
            .unwrap();
        let clip2_id = clip2_outcome.created_id.unwrap();

        editor
            .apply(Command::CopyPasteKeys {
                source_clip_id: clip_id.clone(),
                source_property: KeyProperty::Scale,
                target_clip_id: clip2_id.clone(),
                target_property: KeyProperty::Scale,
                offset: 0.1,
            })
            .unwrap();

        let clip2 = editor.project().active().clip(&clip2_id).unwrap();
        assert!(clip2.is_keyed(KeyProperty::Scale));
        assert_eq!(clip2.keys_on(KeyProperty::Scale).count(), 3);
        // First key at 0.0 was shifted by offset 0.1 -> 0.1
        assert!(clip2.key_at(KeyProperty::Scale, 0.1).is_some());

        // 4. Test Undo / Redo
        assert!(editor.undo()); // undo CopyPasteKeys
        let clip2 = editor.project().active().clip(&clip2_id).unwrap();
        assert!(!clip2.is_keyed(KeyProperty::Scale));

        assert!(editor.redo()); // redo CopyPasteKeys
        let clip2 = editor.project().active().clip(&clip2_id).unwrap();
        assert!(clip2.is_keyed(KeyProperty::Scale));
    }

    #[test]
    fn test_parenting_hierarchy_transform_cascades_and_cycle_prevention() {
        let mut editor = Editor::new();
        let m1 = editor
            .apply(media("vid1.mp4", 20.0, true))
            .unwrap()
            .created_id
            .unwrap();

        // Create Parent clip A on T1
        let a_outcome = editor
            .apply(Command::AddClip {
                track_id: "T1".into(),
                media_id: m1.clone(),
                start: 0.0,
                ripple: false,
            })
            .unwrap();
        let a_id = a_outcome.created_id.unwrap();

        // Create Child clip B on T2
        let b_outcome = editor
            .apply(Command::AddClip {
                track_id: "T2".into(),
                media_id: m1.clone(),
                start: 0.0,
                ripple: false,
            })
            .unwrap();
        let b_id = b_outcome.created_id.unwrap();

        // Create Grandchild clip C on T3
        let t3_outcome = editor.apply(Command::AddTrack { bottom: false }).unwrap();
        let t3_id = t3_outcome.created_id.unwrap();
        let c_outcome = editor
            .apply(Command::AddClip {
                track_id: t3_id,
                media_id: m1,
                start: 0.0,
                ripple: false,
            })
            .unwrap();
        let c_id = c_outcome.created_id.unwrap();

        // Set B's parent to A
        editor
            .apply(Command::SetClipParent {
                clip_id: b_id.clone(),
                parent_clip_id: Some(a_id.clone()),
            })
            .unwrap();

        // Set C's parent to B
        editor
            .apply(Command::SetClipParent {
                clip_id: c_id.clone(),
                parent_clip_id: Some(b_id.clone()),
            })
            .unwrap();

        // 1. Verify cycle prevention:
        // A -> C would create A -> B -> C -> A
        let err = editor
            .apply(Command::SetClipParent {
                clip_id: a_id.clone(),
                parent_clip_id: Some(c_id.clone()),
            })
            .unwrap_err();
        assert_eq!(err, crate::commands::CommandError::CircularReference);

        // Self-parenting: A -> A must fail
        let err = editor
            .apply(Command::SetClipParent {
                clip_id: a_id.clone(),
                parent_clip_id: Some(a_id.clone()),
            })
            .unwrap_err();
        assert_eq!(err, crate::commands::CommandError::CircularReference);

        // 2. Transform cascade verification:
        // Configure Parent A: offset_x = 0.5, scale = 2.0
        editor
            .apply(Command::SetClipTransform {
                clip_id: a_id.clone(),
                scale: Some(2.0),
                offset_x: Some(0.5),
                offset_y: None,
                rotation: None,
                stretch_x: None,
                stretch_y: None,
            })
            .unwrap();

        // Configure Child B: local offset_x = 0.1, scale = 1.5
        editor
            .apply(Command::SetClipTransform {
                clip_id: b_id.clone(),
                scale: Some(1.5),
                offset_x: Some(0.1),
                offset_y: None,
                rotation: None,
                stretch_x: None,
                stretch_y: None,
            })
            .unwrap();

        let timeline = editor.project().active();
        let b_clip = timeline.clip(&b_id).unwrap();
        let world_tf = timeline.world_transform_at(b_clip, 2.0);

        // Child scale in world = parent.scale * child.scale = 2.0 * 1.5 = 3.0
        assert!((world_tf.scale - 3.0).abs() < 1e-4);
        // Child offset_x in world = parent.offset_x + child.offset_x * parent.scale = 0.5 + 0.1 * 2.0 = 0.7
        assert!((world_tf.offset_x - 0.7).abs() < 1e-4);

        // 3. Undo/Redo of parenting
        assert!(editor.undo()); // undo update B
        assert!(editor.undo()); // undo update A
        assert!(editor.undo()); // undo parent C->B
        let timeline = editor.project().active();
        assert_eq!(timeline.clip(&c_id).unwrap().parent_clip_id, None);

        assert!(editor.redo()); // redo parent C->B
        let timeline = editor.project().active();
        assert_eq!(
            timeline.clip(&c_id).unwrap().parent_clip_id,
            Some(b_id.clone())
        );
    }

    #[test]
    fn test_effect_stack_crud_toggle_reorder_and_duplicate() {
        let mut editor = Editor::new();
        let m = editor
            .apply(media("test.mp4", 10.0, true))
            .unwrap()
            .created_id
            .unwrap();

        let c_id = editor
            .apply(Command::AddClip {
                track_id: "T1".into(),
                media_id: m,
                start: 0.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();

        // 1. Add video effects (blur and color)
        editor
            .apply(Command::AddClipEffect {
                clip_id: c_id.clone(),
                effect_id: "concat.gaussian-blur".to_string(),
                is_audio: false,
            })
            .unwrap();
        editor
            .apply(Command::AddClipEffect {
                clip_id: c_id.clone(),
                effect_id: "concat.brightness".to_string(),
                is_audio: false,
            })
            .unwrap();

        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert_eq!(clip.video_effects.len(), 2);
        assert_eq!(clip.video_effects[0].id, "concat.gaussian-blur");
        assert_eq!(clip.video_effects[1].id, "concat.brightness");

        // 2. Set parameter on blur
        editor
            .apply(Command::SetEffectParam {
                clip_id: c_id.clone(),
                index: 0,
                is_audio: false,
                key: "radius".to_string(),
                value: 15.0,
            })
            .unwrap();
        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert_eq!(clip.video_effects[0].params.get("radius"), Some(&15.0));

        // 3. Toggle effect (bypass)
        editor
            .apply(Command::ToggleClipEffect {
                clip_id: c_id.clone(),
                index: 0,
                is_audio: false,
                enabled: false,
            })
            .unwrap();
        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert!(!clip.video_effects[0].enabled);

        // 4. Duplicate effect in place
        editor
            .apply(Command::DuplicateClipEffect {
                clip_id: c_id.clone(),
                index: 0,
                is_audio: false,
            })
            .unwrap();
        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert_eq!(clip.video_effects.len(), 3);
        assert_eq!(clip.video_effects[1].id, "concat.gaussian-blur");
        assert_eq!(clip.video_effects[1].params.get("radius"), Some(&15.0));
        assert!(!clip.video_effects[1].enabled);

        // 5. Reorder effect
        editor
            .apply(Command::ReorderClipEffect {
                clip_id: c_id.clone(),
                from: 2,
                to: 0,
                is_audio: false,
            })
            .unwrap();
        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert_eq!(clip.video_effects[0].id, "concat.brightness");

        // 6. Reset params
        editor
            .apply(Command::ResetEffectParams {
                clip_id: c_id.clone(),
                index: 1,
                is_audio: false,
            })
            .unwrap();
        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert!(clip.video_effects[1].params.is_empty());

        // 7. Remove effect
        editor
            .apply(Command::RemoveClipEffect {
                clip_id: c_id.clone(),
                index: 0,
                is_audio: false,
            })
            .unwrap();
        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert_eq!(clip.video_effects.len(), 2);

        // 8. Undo/Redo stack verification
        assert!(editor.undo()); // undo remove
        let timeline = editor.project().active();
        assert_eq!(timeline.clip(&c_id).unwrap().video_effects.len(), 3);

        assert!(editor.redo()); // redo remove
        let timeline = editor.project().active();
        assert_eq!(timeline.clip(&c_id).unwrap().video_effects.len(), 2);
    }

    #[test]
    fn test_effects_copy_paste_between_clips() {
        let mut editor = Editor::new();
        let m = editor
            .apply(media("test.mp4", 10.0, true))
            .unwrap()
            .created_id
            .unwrap();

        let c1 = editor
            .apply(Command::AddClip {
                track_id: "T1".into(),
                media_id: m.clone(),
                start: 0.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();

        let c2 = editor
            .apply(Command::AddClip {
                track_id: "T1".into(),
                media_id: m,
                start: 4.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();

        // Add 2 effects to c1
        editor
            .apply(Command::AddClipEffect {
                clip_id: c1.clone(),
                effect_id: "concat.vignette".to_string(),
                is_audio: false,
            })
            .unwrap();
        editor
            .apply(Command::SetEffectParam {
                clip_id: c1.clone(),
                index: 0,
                is_audio: false,
                key: "amount".to_string(),
                value: 0.8,
            })
            .unwrap();

        // Copy effects from c1 to c2
        editor
            .apply(Command::CopyPasteEffects {
                source_clip_id: c1,
                target_clip_id: c2.clone(),
                is_audio: false,
            })
            .unwrap();

        let timeline = editor.project().active();
        let target_clip = timeline.clip(&c2).unwrap();
        assert_eq!(target_clip.video_effects.len(), 1);
        assert_eq!(target_clip.video_effects[0].id, "concat.vignette");
        assert_eq!(
            target_clip.video_effects[0].params.get("amount"),
            Some(&0.8)
        );

        // Undo copy/paste
        assert!(editor.undo());
        let timeline = editor.project().active();
        assert!(timeline.clip(&c2).unwrap().video_effects.is_empty());
    }

    #[test]
    fn test_clip_transitions_and_audio_fade_curves() {
        use crate::CommandError;
        use crate::commands::{ClipPatch, TrackFlag};
        use crate::model::AudioFadeCurve;

        let mut editor = Editor::new();
        let m = editor
            .apply(media("test.mp4", 10.0, true))
            .unwrap()
            .created_id
            .unwrap();

        let c_id = editor
            .apply(Command::AddClip {
                track_id: "T1".into(),
                media_id: m,
                start: 0.0,
                ripple: false,
            })
            .unwrap()
            .created_id
            .unwrap();

        // 1. Set transition on clip
        editor
            .apply(Command::SetClipTransition {
                clip_id: c_id.clone(),
                transition_id: "concat.dip-color".to_string(),
                duration: Some(1.2),
                params: None,
            })
            .unwrap();

        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert!(clip.transition_in.is_some());
        let tr = clip.transition_in.as_ref().unwrap();
        assert_eq!(tr.id, "concat.dip-color");
        assert_eq!(tr.duration, 1.2);

        // 2. Audio fade curve patch
        editor
            .apply(Command::UpdateClip {
                clip_id: c_id.clone(),
                patch: ClipPatch {
                    audio_fade_curve: Some(AudioFadeCurve::EqualPower),
                    fade_in: Some(0.5),
                    fade_out: Some(0.5),
                    ..Default::default()
                },
            })
            .unwrap();
        let timeline = editor.project().active();
        let clip = timeline.clip(&c_id).unwrap();
        assert_eq!(clip.audio_fade_curve, AudioFadeCurve::EqualPower);
        assert_eq!(clip.fade_in, 0.5);

        // 3. Clear transition
        editor
            .apply(Command::ClearClipTransition {
                clip_id: c_id.clone(),
            })
            .unwrap();
        let timeline = editor.project().active();
        assert!(timeline.clip(&c_id).unwrap().transition_in.is_none());

        // 4. Undo clear transition
        assert!(editor.undo());
        let timeline = editor.project().active();
        assert!(timeline.clip(&c_id).unwrap().transition_in.is_some());

        // 5. Track lock protection
        let track_id = timeline.clip(&c_id).unwrap().track_id.clone();
        editor
            .apply(Command::SetTrackFlag {
                track_id,
                flag: TrackFlag::Locked,
                value: true,
            })
            .unwrap();

        let res = editor.apply(Command::AddClipEffect {
            clip_id: c_id.clone(),
            effect_id: "concat.vivid".to_string(),
            is_audio: false,
        });
        assert_eq!(res.unwrap_err(), CommandError::TrackLocked);
    }

    #[test]
    fn test_silence_detection_and_ripple_cut_pipeline() {
        use clipcat_core::ai_creator::{SilenceDetectConfig, SilenceDetector};

        let mut editor = Editor::new();
        let track_id = editor.project().active().tracks[0].id.clone();

        // 1. Add 10-second media
        let media_id = editor
            .apply(media("/dialogue.mp4", 10.0, true))
            .expect("add media")
            .created_id
            .expect("id");

        let clip_id = editor
            .apply(Command::AddClip {
                media_id,
                track_id,
                start: 0.0,
                ripple: false,
            })
            .expect("add clip")
            .created_id
            .expect("id");

        // 2. Synthesize audio with speech at [0..3s], silence at [3..7s], speech at [7..10s]
        let sr = 10_000u32;
        let mut samples = vec![0.0f32; 100_000]; // 10 seconds
        for i in 0..30_000 {
            samples[i] = (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / sr as f32)).sin() * 0.8;
        }
        for i in 70_000..100_000 {
            samples[i] = (2.0 * std::f32::consts::PI * 440.0 * (i as f32 / sr as f32)).sin() * 0.8;
        }

        // 3. Detect silence with 0.0s padding for exact cut boundary
        let config = SilenceDetectConfig {
            threshold_db: -30.0,
            min_silence_secs: 1.0,
            padding_secs: 0.0,
            window_secs: 0.02,
        };
        let silences = SilenceDetector::detect_silences(&samples, sr, config);
        assert_eq!(silences.len(), 1);
        let s = silences[0];
        let sil_start = s.start.as_f64();
        let sil_end = s.end().as_f64();
        assert!((sil_start - 3.0).abs() < 0.05);
        assert!((sil_end - 7.0).abs() < 0.05);

        // 4. Split clip at silence end, then silence start
        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: sil_end,
            })
            .expect("split at silence end");

        editor
            .apply(Command::SplitClips {
                clip_ids: vec![clip_id.clone()],
                time: sil_start,
            })
            .expect("split at silence start");

        // The timeline now has 3 clips: [0..3], [3..7], [7..10]
        let timeline = editor.project().active();
        assert_eq!(timeline.clips.len(), 3);
        let mid_clip = timeline
            .clips
            .iter()
            .find(|c| (c.start - sil_start).abs() < 0.1)
            .unwrap()
            .id
            .clone();

        // 5. Ripple remove the silence clip
        editor
            .apply(Command::RemoveClips {
                clip_ids: vec![mid_clip],
                ripple: true,
            })
            .expect("ripple remove silence");

        // Timeline now has 2 clips, and the second clip was pulled forward by the silence duration!
        let timeline = editor.project().active();
        assert_eq!(timeline.clips.len(), 2);
        let c1 = &timeline.clips[0];
        let c2 = &timeline.clips[1];
        assert!((c1.duration - sil_start).abs() < 0.1);
        assert!((c2.start - sil_start).abs() < 0.1);
        let expected_total = 10.0 - (sil_end - sil_start);
        assert!((c2.start + c2.duration - expected_total).abs() < 0.1);

        // 6. Undo restores the 3 clips, and undoing splits restores the original uncut 10s clip!
        assert!(editor.undo()); // undo remove
        assert_eq!(editor.project().active().clips.len(), 3);
        assert!(editor.undo()); // undo second split
        assert!(editor.undo()); // undo first split
        let timeline = editor.project().active();
        assert_eq!(timeline.clips.len(), 1);
        assert!((timeline.clips[0].duration - 10.0).abs() < 1e-4);
    }
}
