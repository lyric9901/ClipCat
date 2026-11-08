// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah

//! The source preview: a file from the library seen and heard on its own,
//! before it is anywhere near the timeline (#197).
//!
//! A click on a video or a still in the bin shows it on the monitor,
//! paused at its start, with a chip that names it; Play and Space play it,
//! the monitor's timecode moves through it. A click on a sound plays it at
//! once, with a line moving across its card; a second click pauses it.
//! The timeline is not touched: the monitor and the speakers are lent a
//! scratch copy of the project holding the one clip, and given back the
//! moment the person clicks the timeline or the chip's cross.

use std::sync::Arc;

use clipcat_project::Command;
use clipcat_project::commands::{self, IdMint};
use clipcat_project::model::{MediaKind, Project};

use crate::studio::Studio;
use crate::ui::SourcePreview;
use slint::{ComponentHandle, SharedString};

/// The file lent to the monitor and the speakers.
pub struct Source {
    pub media_id: String,
    pub name: String,
    pub kind: MediaKind,
    /// The bin's row for the file, for its card to show the playing line;
    /// -1 for a sound from the Audio page.
    pub row: i32,
    /// The Audio page's card for the file, or empty for one in the bin.
    pub card: String,
    /// The scratch project flattened, as the monitor draws it.
    pub clips: Arc<Vec<clipcat_export::ExportClip>>,
    pub duration: f64,
    pub time: f64,
    pub playing: bool,
    /// Follows the speakers' clock while it plays.
    timer: slint::Timer,
}

/// The project as it is, with its active timeline holding only `media_id`
/// from zero: what the source preview draws and plays.
fn scratch(project: &Project, media_id: &str) -> Option<Project> {
    let mut scratch = project.clone();
    let timeline = scratch.active_mut();
    timeline.clips.clear();
    let track_id = timeline.tracks.first()?.id.clone();
    commands::apply(
        &mut scratch,
        &mut IdMint::default(),
        Command::AddClip {
            media_id: media_id.to_owned(),
            track_id,
            start: 0.0,
            ripple: false,
        },
    )
    .ok()?;
    Some(scratch)
}

impl Studio {
    /// A card was clicked. A sound plays, or pauses when it is the one
    /// playing; a picture goes on the monitor, paused at its start.
    pub fn preview_media(&mut self, media_id: &str, row: i32) {
        if let Some(source) = self.source.as_ref()
            && source.media_id == media_id
            && source.card.is_empty()
        {
            if source.kind == MediaKind::Audio {
                self.source_toggle();
            }
            return;
        }
        let Some(media) = self.project().media_by_id(media_id).cloned() else {
            return;
        };
        let Some(project_dir) = self
            .session
            .as_ref()
            .map(|session| std::path::PathBuf::from(session.path()))
        else {
            return;
        };
        let Some(scratch) = scratch(self.project(), media_id) else {
            return;
        };
        // The timeline stops: the speakers and the monitor are the
        // source's now.
        self.pause();
        if let Some(old) = self.source.take() {
            old.timer.stop();
        }
        let clips = Arc::new(clipcat_export::flatten::flatten_timeline_in(
            &scratch,
            None,
            Some(&project_dir),
        ));
        let duration = scratch
            .active()
            .clips
            .first()
            .map_or(0.0, |clip| clip.duration);
        self.host
            .playback
            .set_clips(project_dir, crate::studio::audio_specs(&clips));
        self.host.playback.seek(0.0);
        self.source = Some(Source {
            media_id: media_id.to_owned(),
            name: media.name.clone(),
            kind: media.kind,
            row,
            card: String::new(),
            clips,
            duration,
            time: 0.0,
            playing: false,
            timer: slint::Timer::default(),
        });
        if media.kind == MediaKind::Audio {
            self.source_toggle();
        }
        self.request_preview();
    }

    /// A card on the Audio page was clicked: its sound plays through the
    /// speakers as a sound from the bin does, without being in the bin, or
    /// pauses when it is the one playing. `card` names it for its card's
    /// playing line.
    pub fn preview_file(&mut self, card: &str, name: &str, path: &std::path::Path) {
        if let Some(source) = self.source.as_ref()
            && source.card == card
        {
            self.source_toggle();
            return;
        }
        let Some(project_dir) = self
            .session
            .as_ref()
            .map(|session| std::path::PathBuf::from(session.path()))
        else {
            return;
        };
        let summary = match clipcat_host::media::probe(&path.to_string_lossy()) {
            Ok(summary) => summary,
            Err(error) => {
                self.notify(&crate::host::probe_error(&error), true);
                return;
            }
        };
        // The scratch project with the file in its bin, under an id no
        // real item has, and the one clip cut from it.
        let mut scratch = self.project().clone();
        scratch.media.retain(|item| item.path != summary.path);
        let mut mint = IdMint::default();
        mint.adopt_project(&scratch);
        let Ok(Some(media_id)) = commands::apply(
            &mut scratch,
            &mut mint,
            Command::AddMedia {
                item: summary.to_new_media(),
            },
        )
        .map(|outcome| outcome.created_id) else {
            return;
        };
        let Some(scratch) = self::scratch(&scratch, &media_id) else {
            return;
        };
        self.pause();
        if let Some(old) = self.source.take() {
            old.timer.stop();
        }
        let clips = Arc::new(clipcat_export::flatten::flatten_timeline_in(
            &scratch,
            None,
            Some(&project_dir),
        ));
        let duration = scratch
            .active()
            .clips
            .first()
            .map_or(0.0, |clip| clip.duration);
        self.host
            .playback
            .set_clips(project_dir, crate::studio::audio_specs(&clips));
        self.host.playback.seek(0.0);
        self.source = Some(Source {
            media_id,
            name: name.to_owned(),
            kind: MediaKind::Audio,
            row: -1,
            card: card.to_owned(),
            clips,
            duration,
            time: 0.0,
            playing: false,
            timer: slint::Timer::default(),
        });
        self.source_toggle();
        self.request_preview();
    }

    /// Back to the timeline: its picture on the monitor, its sound in the
    /// speakers, the transport where the playhead was.
    pub fn close_source(&mut self) {
        let Some(source) = self.source.take() else {
            return;
        };
        source.timer.stop();
        self.host.playback.pause();
        self.sync_audio();
        self.host.playback.seek(f64::from(self.playhead));
        self.request_preview();
    }

    /// Play or pause the source, from where it stands; from its start
    /// again once it has played to the end.
    pub fn source_toggle(&mut self) {
        let Some(source) = self.source.as_mut() else {
            return;
        };
        if source.playing {
            source.playing = false;
            source.timer.stop();
            self.host.playback.pause();
            return;
        }
        if source.time >= source.duration - 1e-3 {
            source.time = 0.0;
        }
        source.playing = true;
        self.host.playback.play(source.time);
        source.timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16),
            || {
                crate::host::Shell::with(|shell, app| {
                    let mut studio = shell.studio.borrow_mut();
                    let position = studio.host.playback.position_now();
                    let Some(source) = studio.source.as_mut() else {
                        return;
                    };
                    source.time = position.min(source.duration);
                    let ended = source.time >= source.duration;
                    if ended {
                        source.playing = false;
                        source.timer.stop();
                    }
                    if ended {
                        studio.host.playback.pause();
                    }
                    studio.request_preview();
                    studio.publish_source(&app);
                });
            },
        );
        self.wake_meters();
    }

    /// The monitor's timecode moved through the source.
    pub fn source_seek(&mut self, seconds: f64) {
        let Some(source) = self.source.as_mut() else {
            return;
        };
        let old_time = source.time;
        source.time = seconds.clamp(0.0, source.duration);
        let time = source.time;
        if source.playing {
            self.host.playback.play(time);
        } else if self.prefs.scrub_audio() {
            self.host
                .playback
                .scrub(old_time, time, self.prefs.scrub_volume());
        } else {
            self.host.playback.seek(time);
        }
        self.request_preview();
    }

    /// What the window shows of the source: the chip, the card's line, the
    /// monitor's readout.
    pub fn publish_source(&self, app: &crate::ui::App) {
        let preview = app.global::<SourcePreview>();
        match self.source.as_ref() {
            Some(source) => {
                preview.set_open(true);
                preview.set_name(source.name.as_str().into());
                preview.set_row(source.row);
                preview.set_card(source.card.as_str().into());
                preview.set_time(source.time as f32);
                preview.set_duration(source.duration as f32);
                preview.set_playing(source.playing);
                preview.set_progress(if source.duration > 0.0 {
                    (source.time / source.duration) as f32
                } else {
                    0.0
                });

                if let Some(media) = self.project().media_by_id(&source.media_id) {
                    preview.set_mark_in(media.source_in.map(|v| v as f32).unwrap_or(-1.0));
                    preview.set_mark_out(media.source_out.map(|v| v as f32).unwrap_or(-1.0));
                    if let (Some(w), Some(h)) = (media.width, media.height) {
                        preview.set_resolution(format!("{}x{}", w, h).into());
                    } else {
                        preview.set_resolution("".into());
                    }
                    if let Some(fps) = media.frame_rate {
                        preview.set_frame_rate(format!("{:.2} fps", fps).into());
                    } else {
                        preview.set_frame_rate("".into());
                    }
                    if let Some(codec) = &media.video_codec {
                        preview.set_codec(codec.as_str().into());
                    } else if let Some(codec) = &media.audio_codec {
                        preview.set_codec(codec.as_str().into());
                    } else {
                        preview.set_codec("".into());
                    }
                } else {
                    preview.set_mark_in(-1.0);
                    preview.set_mark_out(-1.0);
                    preview.set_resolution("".into());
                    preview.set_frame_rate("".into());
                    preview.set_codec("".into());
                }
            }
            None => {
                preview.set_open(false);
                preview.set_row(-1);
                preview.set_card(SharedString::new());
                preview.set_playing(false);
                preview.set_mark_in(-1.0);
                preview.set_mark_out(-1.0);
                preview.set_resolution("".into());
                preview.set_frame_rate("".into());
                preview.set_codec("".into());
            }
        }
    }

    /// Sets the source In point to the current source timecode.
    pub fn source_mark_in(&mut self) {
        let Some(source) = self.source.as_ref() else {
            return;
        };
        let time = source.time;
        let media_id = source.media_id.clone();
        let current_out = self
            .project()
            .media_by_id(&media_id)
            .and_then(|m| m.source_out);
        self.apply(Command::SetMediaMarks {
            media_id,
            source_in: Some(time),
            source_out: current_out,
        });
        self.notify(&format!("Source Mark In: {:.2}s", time), false);
        self.request_preview();
    }

    /// Sets the source Out point to the current source timecode.
    pub fn source_mark_out(&mut self) {
        let Some(source) = self.source.as_ref() else {
            return;
        };
        let time = source.time;
        let media_id = source.media_id.clone();
        let current_in = self
            .project()
            .media_by_id(&media_id)
            .and_then(|m| m.source_in);
        self.apply(Command::SetMediaMarks {
            media_id,
            source_in: current_in,
            source_out: Some(time),
        });
        self.notify(&format!("Source Mark Out: {:.2}s", time), false);
        self.request_preview();
    }

    /// Clears both In and Out points on the current source.
    pub fn source_clear_marks(&mut self) {
        let Some(source) = self.source.as_ref() else {
            return;
        };
        let media_id = source.media_id.clone();
        self.apply(Command::ClearMediaMarks { media_id });
        self.notify("Source Marks Cleared", false);
        self.request_preview();
    }

    /// 3-Point Insert Edit: inserts source content rippling targeted and linked tracks.
    pub fn insert_edit(&mut self) {
        let media_id = if let Some(source) = self.source.as_ref() {
            source.media_id.clone()
        } else if let Some(m) = self.project().media.first() {
            m.id.clone()
        } else {
            self.notify("No media in project bin", true);
            return;
        };

        let tl = self.timeline();
        let tl_in = tl.mark_in.unwrap_or(f64::from(self.playhead));
        let tl_out = tl.mark_out;

        let target_v = self.target_video_track.clone();
        let target_a = self.target_audio_track.clone();

        let outcome = self.apply(Command::InsertEdit {
            media_id,
            source_in: None,
            source_out: None,
            timeline_in: Some(tl_in),
            timeline_out: tl_out,
            video_track_id: target_v,
            audio_track_id: target_a,
        });

        match outcome {
            Ok(outcome) if outcome.applied => {
                self.notify("Insert edit applied", false);
                self.request_preview();
            }
            Err(e) => {
                self.notify(&e.to_string(), true);
            }
            _ => {}
        }
    }

    /// 3-Point Overwrite Edit: replaces targeted timeline range without rippling.
    pub fn overwrite_edit(&mut self) {
        let media_id = if let Some(source) = self.source.as_ref() {
            source.media_id.clone()
        } else if let Some(m) = self.project().media.first() {
            m.id.clone()
        } else {
            self.notify("No media in project bin", true);
            return;
        };

        let tl = self.timeline();
        let tl_in = tl.mark_in.unwrap_or(f64::from(self.playhead));
        let tl_out = tl.mark_out;

        let target_v = self.target_video_track.clone();
        let target_a = self.target_audio_track.clone();

        let outcome = self.apply(Command::OverwriteEdit {
            media_id,
            source_in: None,
            source_out: None,
            timeline_in: Some(tl_in),
            timeline_out: tl_out,
            video_track_id: target_v,
            audio_track_id: target_a,
        });

        match outcome {
            Ok(outcome) if outcome.applied => {
                self.notify("Overwrite edit applied", false);
                self.request_preview();
            }
            Err(e) => {
                self.notify(&e.to_string(), true);
            }
            _ => {}
        }
    }
}
