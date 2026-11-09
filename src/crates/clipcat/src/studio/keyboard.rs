// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors
//! The keyboard, and the menu's verbs.

use super::*;

impl Studio {
    // ── the keyboard, and the menu's verbs ──

    /// A chord from the window's key table; see `Editor.shortcut`. The view
    /// chords - zoom, the playhead's ends, the sheets - go through the app
    /// menu's own handler in lib.rs, so a key and the row that advertises
    /// it are one thing.
    pub fn shortcut(&mut self, action: &str) {
        log::info!("shortcut: {action}");
        match action {
            // The selected clips when there are any, as the tray's button
            // and the menu split: the chord cut every track under the
            // playhead whatever was selected, titles and music with it.
            "split" => {
                let at = self.playhead;
                self.split_at(at, true);
            }
            "split-selected" => {
                let at = self.playhead;
                self.split_at(at, true);
            }
            // B: the razor, on and off.
            "split-tool" => {
                self.lanes.tool = if self.lanes.tool == TimelineTool::Split {
                    TimelineTool::Select
                } else {
                    TimelineTool::Split
                };
            }
            "roll-tool" => {
                self.lanes.tool = if self.lanes.tool == TimelineTool::Roll {
                    TimelineTool::Select
                } else {
                    TimelineTool::Roll
                };
                self.notify(
                    if self.lanes.tool == TimelineTool::Roll {
                        "Rolling Edit tool active"
                    } else {
                        "Selection tool active"
                    },
                    false,
                );
            }
            "slip-tool" => {
                self.lanes.tool = if self.lanes.tool == TimelineTool::Slip {
                    TimelineTool::Select
                } else {
                    TimelineTool::Slip
                };
                self.notify(
                    if self.lanes.tool == TimelineTool::Slip {
                        "Slip Edit tool active"
                    } else {
                        "Selection tool active"
                    },
                    false,
                );
            }
            "slide-tool" => {
                self.lanes.tool = if self.lanes.tool == TimelineTool::Slide {
                    TimelineTool::Select
                } else {
                    TimelineTool::Slide
                };
                self.notify(
                    if self.lanes.tool == TimelineTool::Slide {
                        "Slide Edit tool active"
                    } else {
                        "Selection tool active"
                    },
                    false,
                );
            }
            "select-tool" => {
                self.lanes.tool = TimelineTool::Select;
                self.notify("Selection tool active", false);
            }
            // M: Add marker at playhead.
            "add-marker" => {
                let at = f64::from(self.playhead);
                self.apply(Command::AddMarker {
                    timeline_id: None,
                    time: at,
                    duration: None,
                    name: Some("Marker".to_owned()),
                    color: Some("cyan".to_owned()),
                });
                self.notify(&format!("Marker added at {:.2}s", at), false);
            }
            // Q / W: the clip loses what is left, or right,
            // of the playhead.
            "delete-left" => self.delete_to_playhead(PlayheadSide::Left),
            "delete-right" => self.delete_to_playhead(PlayheadSide::Right),
            "select-all" => self.select_all(),
            // The phone's way out of the clip tools: nothing selected, the
            // way a press on an empty lane leaves it.
            "deselect" => {
                self.flush_commit();
                self.end_audition();
                self.selection.clear();
                self.transition_selected = None;
            }
            // ⇧⌫, from the key table; plain ⌫ comes in as its own callback.
            "ripple-delete" => self.ripple_delete_selected(),
            // Copy takes the whole selection; see `copy_clips`.
            "copy" => self.copy_clips(None),
            "duplicate" | "mute" => {
                if let Some(id) = self.sole_selection() {
                    self.clip_action(&id, action);
                }
            }
            // The tray's picture verbs. Mirror is the flip the H key does,
            // without the setting that gates the key: a button on the tray
            // is not a key someone might press by accident.
            "mirror" => self.toggle_flip_h(),
            "rotate" => self.rotate_selected(),
            "reverse" => {
                if let Some(id) = self.sole_selection() {
                    self.reverse_clip(&id);
                }
            }
            "freeze" => {
                self.menu_target = None;
                self.freeze_at_playhead();
            }
            // After the selected clip, on its lane, as the menu does; with
            // nothing selected, at the playhead on the rows copied from.
            "paste" => {
                let after = self.sole_selection();
                self.paste_clips(after.as_deref());
            }
            "shuttle-forward" => self.shuttle_forward(),
            "shuttle-reverse" => self.shuttle_reverse(),
            "shuttle-stop" => self.shuttle_stop(),
            "command-palette" => {
                self.notify("Command Palette active", false);
            }
            "mark-in" => {
                if let Some(source) = self.source.as_ref() {
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
                } else {
                    let time = f64::from(self.playhead);
                    let current_out = self.timeline().mark_out;
                    self.apply(Command::SetTimelineMarks {
                        timeline_id: None,
                        mark_in: Some(time),
                        mark_out: current_out,
                    });
                    self.notify(&format!("Timeline Mark In: {:.2}s", time), false);
                    self.request_preview();
                }
            }
            "mark-out" => {
                if let Some(source) = self.source.as_ref() {
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
                } else {
                    let time = f64::from(self.playhead);
                    let current_in = self.timeline().mark_in;
                    self.apply(Command::SetTimelineMarks {
                        timeline_id: None,
                        mark_in: current_in,
                        mark_out: Some(time),
                    });
                    self.notify(&format!("Timeline Mark Out: {:.2}s", time), false);
                    self.request_preview();
                }
            }
            "clear-marks" => {
                if let Some(source) = self.source.as_ref() {
                    let media_id = source.media_id.clone();
                    self.apply(Command::ClearMediaMarks { media_id });
                    self.notify("Source Marks Cleared", false);
                    self.request_preview();
                } else {
                    self.apply(Command::ClearTimelineMarks { timeline_id: None });
                    self.notify("Timeline Marks Cleared", false);
                    self.request_preview();
                }
            }
            "insert-edit" => self.insert_edit(),
            "overwrite-edit" => self.overwrite_edit(),
            "toggle-scrub-audio" => {
                let next = !self.prefs.scrub_audio();
                self.prefs.scrub_audio = Some(next);
                self.prefs.save(&self.host.dirs);
                let msg = if next {
                    "Audio scrubbing on"
                } else {
                    "Audio scrubbing off"
                };
                self.notify(msg, false);
            }
            _ => {}
        }
    }

    /// One of the clip menu's verbs on clip `id`. The menu's rows and the
    /// keyboard's chords both land here, so a shortcut and the row that
    /// advertises it cannot disagree.
    /// Takes a video clip's sound off onto a lane of its own, and selects
    /// what came off: the lane it lands on may be a new one, out of view,
    /// and a detach that changed nothing anyone could see read as one that
    /// did nothing. One that cannot happen says why instead of nothing.
    /// https://github.com/jub0t/Concat/issues/310
    fn detach_audio(&mut self, clip: &Clip) {
        let has_audio = self
            .project()
            .media_by_id(&clip.media_id)
            .is_some_and(|media| media.has_audio);
        let detached = self
            .timeline()
            .clips
            .iter()
            .any(|other| other.detached_from.as_deref() == Some(clip.id.as_str()));
        let detach = Command::DetachAudio {
            clip_id: clip.id.clone(),
        };
        let command = match detach_plan(clip, has_audio, detached) {
            DetachPlan::NoSound => {
                self.notify(&t("studio.detachNoSound"), false);
                return;
            }
            DetachPlan::Already => {
                self.notify(&t("studio.detachAlready"), false);
                return;
            }
            DetachPlan::Detach => detach,
            // Its sound was taken off once and that clip deleted, which
            // left the picture silent with nothing to reattach. Unmuted
            // and taken off again, as one step: the sound comes back as a
            // clip, which is what was asked for.
            DetachPlan::Again => Command::Batch {
                commands: vec![
                    Command::UpdateClip {
                        clip_id: clip.id.clone(),
                        patch: ClipPatch {
                            muted: Some(false),
                            ..Default::default()
                        },
                    },
                    detach,
                ],
            },
        };
        let token = self.toast.token;
        self.apply(command);
        let sounds: Vec<String> = self
            .timeline()
            .clips
            .iter()
            .filter(|other| other.detached_from.as_deref() == Some(clip.id.as_str()))
            .map(|other| other.id.clone())
            .collect();
        if sounds.is_empty() {
            // A failure has said so already.
            if self.toast.token == token {
                self.notify(&t("studio.detachNoSound"), false);
            }
            return;
        }
        self.transition_selected = None;
        self.selection = sounds;
    }

    pub fn clip_action(&mut self, id: &str, action: &str) {
        log::info!("clip menu: {action} on {id}");
        let Some(clip) = self.clip(id).cloned() else {
            return;
        };
        match action {
            "copy" => self.copy_clips(Some(id)),
            "duplicate" => self.duplicate_selected(),
            // Pasted after the clip that was right-clicked, on its lane.
            "paste" => self.paste_clips(Some(id)),
            "split" => {
                let at = self.playhead;
                self.flush_commit();
                self.selection = vec![id.to_owned()];
                self.split_at(at, true);
            }
            "freeze" => self.freeze_at_playhead(),
            // The clip right-clicked, or the selection when it is one of
            // it, as Delete takes it.
            "delete-left" | "delete-right" => {
                let ids =
                    if self.selection.len() > 1 && self.selection.iter().any(|held| held == id) {
                        self.selection.clone()
                    } else {
                        vec![id.to_owned()]
                    };
                let side = if action == "delete-left" {
                    PlayheadSide::Left
                } else {
                    PlayheadSide::Right
                };
                self.delete_clips_to_playhead(&ids, side);
            }
            "merge" => self.merge(),
            // On the clip that was right-clicked, which becomes the
            // selection, as Enhance's row makes it.
            "fit" | "fill" => {
                self.flush_commit();
                self.selection = vec![id.to_owned()];
                self.transition_selected = None;
                self.frame_selected(action == "fill");
            }
            // The menu opens the clip's Enhance in the inspector, where its
            // switch starts it and later turns it off and on again.
            "enhance" => {
                self.flush_commit();
                self.selection = vec![id.to_owned()];
                self.transition_selected = None;
                self.inspector_jump = (self.inspector_jump.0 + 1, "Video", "Enhance");
            }
            "export-audio" => self.export_clip_audio(id),
            // One of the two ways, or back to the recording when that way
            // is the one already on; on the selection when the clip is in
            // it, else on the clip alone.
            "voice-keep" | "voice-remove" => {
                self.flush_commit();
                if !self.selection.iter().any(|held| held == id) {
                    self.selection = vec![id.to_owned()];
                    self.transition_selected = None;
                }
                let wanted = clipcat_project::model::VoiceMix {
                    // The kind the clip had; vocals apart from music else.
                    kind: clip.voice.unwrap_or_default().kind,
                    ..if action == "voice-keep" {
                        clipcat_project::model::VoiceMix::KEEP_VOICE
                    } else {
                        clipcat_project::model::VoiceMix::REMOVE_VOICE
                    }
                };
                let on = clip.voice.is_some_and(|mix| {
                    (mix.voice == 0.0) == (wanted.voice == 0.0)
                        && (mix.background == 0.0) == (wanted.background == 0.0)
                });
                self.set_voice((!on).then_some(wanted));
            }
            "detach" => self.detach_audio(&clip),
            "reattach" => {
                self.apply(Command::ReattachAudio {
                    clip_id: id.to_owned(),
                });
            }
            "mute" => {
                let volume = if clip.volume <= 0.0 { 1.0 } else { 0.0 };
                self.apply(Command::UpdateClip {
                    clip_id: id.to_owned(),
                    patch: ClipPatch {
                        volume: Some(volume),
                        ..Default::default()
                    },
                });
            }
            "lock" => self.toggle_lock(&clip.track_id),
            "roll-trim" => {
                self.lanes.tool = TimelineTool::Roll;
                self.notify("Rolling Edit tool active", false);
            }
            "slip" => {
                self.lanes.tool = TimelineTool::Slip;
                self.notify("Slip Edit tool active", false);
            }
            "slide" => {
                self.lanes.tool = TimelineTool::Slide;
                self.notify("Slide Edit tool active", false);
            }
            "create-compound" => {
                let ids = if self.selection.contains(&id.to_owned()) {
                    self.selection.clone()
                } else {
                    vec![id.to_owned()]
                };
                self.apply(Command::CreateCompoundClip {
                    clip_ids: ids,
                    name: None,
                });
            }
            "decompose-compound" => {
                self.apply(Command::DecomposeCompoundClip {
                    clip_id: id.to_owned(),
                });
            }
            "open-sequence" => {
                if clip.kind == model::ClipKind::Sequence {
                    self.selection.clear();
                    self.apply(Command::SelectTimeline {
                        timeline_id: clip.media_id.clone(),
                    });
                }
            }
            preset if preset.starts_with("preset-") => {
                if let Some(name) = preset.strip_prefix("preset-") {
                    self.apply(Command::ApplyAnimationPreset {
                        clip_id: id.to_owned(),
                        preset: name.to_owned(),
                    });
                }
            }
            // A clip that is part of the selection takes the selection with
            // it: Delete on one of five selected clips means the five.
            "delete" | "ripple-delete" => {
                let whole =
                    self.selection.len() > 1 && self.selection.iter().any(|held| held == id);
                let ripple = action == "ripple-delete"
                    || if whole {
                        self.closes_gap(&self.selection)
                    } else {
                        self.closes_gap(&[id.to_owned()])
                    };
                if whole {
                    self.remove_selected(ripple);
                } else {
                    self.apply(Command::RemoveClips {
                        clip_ids: vec![id.to_owned()],
                        ripple,
                    });
                }
                self.menu_target = None;
            }
            _ => {}
        }
    }

    /// Every clip on an unlocked lane.
    pub fn select_all(&mut self) {
        self.flush_commit();
        self.selection = self
            .timeline()
            .clips
            .iter()
            .filter(|clip| !self.locked(&clip.track_id))
            .map(|clip| clip.id.clone())
            .collect();
    }

    /// Tab `from` dropped at `slot`, a position counted over the strip as
    /// it stands; the command counts with the tab already removed.
    pub fn move_timeline(&mut self, from: i32, slot: i32) {
        let Some(from) = usize::try_from(from).ok() else {
            return;
        };
        let Some(timeline) = self.project().timelines.get(from) else {
            return;
        };
        let id = timeline.id.clone();
        let slot = slot.max(0) as usize;
        let index = if from < slot { slot - 1 } else { slot };
        if index == from {
            return;
        }
        self.apply(Command::MoveTimeline {
            timeline_id: id,
            index,
        });
    }
}
