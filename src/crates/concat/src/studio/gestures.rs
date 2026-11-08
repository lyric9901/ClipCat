// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors
//! Gestures in flight: drags, trims and moves, previewed on the echo.

use super::*;

impl Studio {
    // ── gestures ──

    /// A press resolves the selection before anything moves, so a drag that
    /// starts on an already-selected clip carries the whole set.
    pub fn clip_pressed(&mut self, id: &str, additive: bool, edge: i32) {
        let Some(clip) = self.clip(id).cloned() else {
            return;
        };
        if self.locked(&clip.track_id) {
            return;
        }
        // Whatever the inspector still holds for the clip selected until
        // now lands before the selection moves: a commit flushed after the
        // change would look for it on the newly selected clip and lose it.
        self.flush_commit();
        self.end_audition();
        // A press on a transition's seam or handles selects the transition,
        // not the clip it rides into; a drag there still sets its length.
        if edge == 2
            && let Some(transition) = clip.transition_in.as_ref()
        {
            self.selection.clear();
            self.transition_selected = Some(id.to_owned());
            self.begin_echo();
            self.gesture = Gesture::TransitionResize {
                clip: id.to_owned(),
                original_duration: transition.duration as f32,
            };
            return;
        }
        self.transition_selected = None;
        let already = self.selection.iter().any(|held| held == id);
        if edge == -1 && self.lanes.tool == TimelineTool::Select && additive {
            self.selection = if already {
                self.selection
                    .iter()
                    .filter(|held| held.as_str() != id)
                    .cloned()
                    .collect()
            } else {
                let mut next = self.selection.clone();
                next.push(id.to_owned());
                next
            };
        } else if already {
            // Keep existing selection if already selected
        } else {
            self.selection = vec![id.to_owned()];
        }

        self.begin_echo();

        // ── 1. Rolling Edit (edge 3, 4, or edge >= 0 with Roll tool or Alt) ──
        let is_roll = edge == 3
            || edge == 4
            || (edge >= 0 && (additive || self.lanes.tool == TimelineTool::Roll));
        if is_roll {
            let track_id = clip.track_id.clone();
            let is_tail_edge = edge == 1 || edge == 4;
            let head_tail = if is_tail_edge {
                let cut = clip.start + clip.duration;
                let adj = self
                    .timeline()
                    .clips
                    .iter()
                    .find(|c| c.track_id == track_id && (c.start - cut).abs() <= 1e-4);
                adj.map(|t| (clip.clone(), t.clone()))
            } else {
                let cut = clip.start;
                let adj = self.timeline().clips.iter().find(|c| {
                    c.track_id == track_id && ((c.start + c.duration) - cut).abs() <= 1e-4
                });
                adj.map(|h| (h.clone(), clip.clone()))
            };
            if let Some((head, tail)) = head_tail {
                self.selection = vec![head.id.clone(), tail.id.clone()];
                self.gesture = Gesture::RollTrim {
                    head_clip: head.id.clone(),
                    tail_clip: tail.id.clone(),
                    head_duration: head.duration as f32,
                    tail_start: tail.start as f32,
                    tail_duration: tail.duration as f32,
                    tail_source_start: tail.source_start as f32,
                };
                self.notify(
                    &format!("Rolling Edit: {} | {}", head.name, tail.name),
                    false,
                );
                return;
            } else {
                self.notify("No adjacent clip to roll", false);
                if edge == 3 || edge == 4 {
                    return;
                }
            }
        }

        // ── 2. Standard Edge Trim (edge 0 or 1) ──
        if edge == 0 || edge == 1 {
            self.selection = vec![id.to_owned()];
            self.gesture = Gesture::Trim {
                clip: id.to_owned(),
                edge: if edge == 0 { Edge::Start } else { Edge::End },
                start: clip.start as f32,
                duration: clip.duration as f32,
                source_start: clip.source_start as f32,
            };
            return;
        }

        // ── 3. Slide Edit (edge == -2 or Slide Tool) ──
        let is_slide = edge == -2 || (edge < 0 && self.lanes.tool == TimelineTool::Slide);
        if is_slide {
            let track_id = clip.track_id.clone();
            let c_start = clip.start;
            let c_end = clip.start + clip.duration;
            let prev = self.timeline().clips.iter().find(|c| {
                c.track_id == track_id
                    && c.id != clip.id
                    && (c.start + c.duration - c_start).abs() <= 1e-4
            });
            let next = self.timeline().clips.iter().find(|c| {
                c.track_id == track_id && c.id != clip.id && (c.start - c_end).abs() <= 1e-4
            });
            if let (Some(p), Some(n)) = (prev, next) {
                self.selection = vec![id.to_owned()];
                self.gesture = Gesture::Slide {
                    clip: id.to_owned(),
                    prev_clip: p.id.clone(),
                    next_clip: n.id.clone(),
                    prev_duration: p.duration as f32,
                    target_start: clip.start as f32,
                    next_start: n.start as f32,
                    next_duration: n.duration as f32,
                    next_source_start: n.source_start as f32,
                };
                self.notify(&format!("Slide Edit: {}", clip.name), false);
                return;
            } else {
                self.notify("Slide edit requires adjacent clips before and after", true);
                return;
            }
        }

        // ── 4. Slip Edit (edge == -3 or Slip Tool or body with Alt) ──
        let is_slip = edge == -3 || (edge < 0 && self.lanes.tool == TimelineTool::Slip);
        if is_slip {
            if clip.media_id.is_empty() {
                self.notify("Cannot slip generated clip without source media", true);
                return;
            }
            self.selection = vec![id.to_owned()];
            self.gesture = Gesture::Slip {
                clip: id.to_owned(),
                original_source_start: clip.source_start as f32,
            };
            self.notify(&format!("Slip: {}", clip.name), false);
            return;
        }
        // What moves is what is selected, and nothing else. A detached
        // sound is a clip of its own: it goes where it is dragged, and its
        // picture stays, as any two clips do. It used to travel with its
        // picture, which made Detach a split in name only (#265); the link
        // between them is kept for Reattach, not for the drag.
        let moving = if self.selection.iter().any(|held| held == id) {
            self.selection.clone()
        } else {
            vec![id.to_owned()]
        };
        let origins = moving
            .iter()
            .filter_map(|clip_id| {
                let clip = self.clip(clip_id)?;
                Some(MoveOrigin {
                    clip: clip.id.clone(),
                    start: clip.start as f32,
                    row: self.row_of(&clip.track_id),
                })
            })
            .collect();
        self.gesture = Gesture::Move {
            primary: id.to_owned(),
            origins,
            lanes: self.lane_heights(),
        };
    }

    /// The pointer moved: the echo follows it.
    pub fn clip_dragged(&mut self, seconds: f32, pixels: f32) {
        let gesture = std::mem::replace(&mut self.gesture, Gesture::None);
        match &gesture {
            Gesture::Move {
                primary,
                origins,
                lanes,
            } => {
                let Some(anchor) = origins.iter().find(|origin| &origin.clip == primary) else {
                    self.gesture = gesture;
                    return;
                };
                let threshold = 8.0 * self.lanes.seconds_per_pixel;
                // Every moving clip's two edges, where the pointer would
                // put them, and the shift that lands the nearest on a target.
                let moving: Vec<&str> = origins.iter().map(|origin| origin.clip.as_str()).collect();
                let edges: Vec<f32> = origins
                    .iter()
                    .filter_map(|origin| {
                        let clip = self.clip(&origin.clip)?;
                        let start = origin.start + seconds;
                        Some([start, start + clip.duration as f32])
                    })
                    .flatten()
                    .collect();
                let shift = seconds + self.snap_shift(&edges, threshold, &moving);
                let rows = nearest_row(lanes, row_top(lanes, anchor.row) + pixels) - anchor.row;
                let count = self.timeline().tracks.len() as i32;
                let moves: Vec<ClipMove> = origins
                    .iter()
                    .filter_map(|origin| {
                        let row = (origin.row + rows).clamp(0, count - 1);
                        let track_id = self
                            .row_track(row)
                            .filter(|track| !self.locked(&track.id))
                            .map(|track| track.id.clone())
                            .or_else(|| {
                                self.clip(&origin.clip).map(|clip| clip.track_id.clone())
                            })?;
                        Some(ClipMove {
                            clip_id: origin.clip.clone(),
                            start: f64::from((origin.start + shift).max(0.0)),
                            track_id,
                        })
                    })
                    .collect();
                // The drop's own rule, asked of the timeline as it was before
                // the drag: the echo shows where the clips will land, in the
                // nearest gap rather than on top of a clip, or back where
                // they started when they would cover each other.
                let landed = self.timeline().resolve_moves(&moves).unwrap_or_else(|| {
                    origins
                        .iter()
                        .filter_map(|origin| {
                            let clip = self.clip(&origin.clip)?;
                            Some(ClipMove {
                                clip_id: clip.id.clone(),
                                start: clip.start,
                                track_id: clip.track_id.clone(),
                            })
                        })
                        .collect()
                });
                for wanted in landed {
                    if let Some(clip) = self.echo_clip_mut(&wanted.clip_id) {
                        clip.start = wanted.start;
                        clip.track_id = wanted.track_id;
                    }
                }
            }
            Gesture::Trim {
                clip,
                edge,
                start,
                duration,
                source_start,
            } => {
                let (id, edge) = (clip.clone(), *edge);
                let (start, duration, source_start) = (*start, *duration, *source_start);
                let threshold = 8.0 * self.lanes.seconds_per_pixel;
                let speed = self.clip(&id).map_or(1.0, |clip| clip.speed as f32);
                // A plain trim stops at the neighbours, as the command will;
                // a magnetic one moves them, so the lane is all its room.
                let (floor, ceiling) = if self.prefs.magnetic {
                    (0.0, f32::INFINITY)
                } else {
                    self.timeline()
                        .room_around(&id)
                        .map_or((0.0, f32::INFINITY), |(before, after)| {
                            (before as f32, after as f32)
                        })
                };
                if edge == Edge::Start {
                    // The head cannot pass the tail, and cannot pull material
                    // out of a file that has none before the in-point.
                    let wanted = self.snapped(start + seconds, threshold, &id);
                    let limit = start + duration - MIN_DURATION;
                    let at = wanted.clamp(
                        (start - source_start / speed.max(0.01))
                            .max(floor)
                            .min(limit),
                        limit,
                    );
                    let delta = at - start;
                    if let Some(clip) = self.echo_clip_mut(&id) {
                        clip.start = f64::from(at);
                        clip.duration = f64::from(duration - delta);
                        clip.source_start = f64::from(source_start + delta * speed);
                    }
                    // Trim follow: the playhead on the new first frame.
                    if self.prefs.trim_follow {
                        self.seek(at);
                    }
                } else {
                    let wanted = self.snapped(start + duration + seconds, threshold, &id);
                    let at = wanted.min(ceiling).max(start + MIN_DURATION);
                    if let Some(clip) = self.echo_clip_mut(&id) {
                        clip.duration = f64::from(at - start);
                    }
                    // Trim follow: the playhead on the new last frame, one
                    // frame inside the edge - at the edge itself the clip
                    // has already ended and the monitor would show what
                    // comes after it.
                    if self.prefs.trim_follow {
                        let frame = 1.0 / self.frame_rate().max(1.0);
                        self.seek((at - frame).max(start));
                    }
                }
            }
            Gesture::RollTrim {
                head_clip,
                tail_clip,
                head_duration,
                tail_start,
                tail_duration,
                tail_source_start,
            } => {
                let delta = seconds;
                let head_id = head_clip.clone();
                let tail_id = tail_clip.clone();

                let mut min_d = -(*head_duration - 0.0166667);
                let mut max_d = *tail_duration - 0.0166667;

                // Tail source start cannot be negative
                min_d = min_d.max(-*tail_source_start);

                // Check media duration limits if available
                if let Some(h) = self.clip(&head_id)
                    && let Some(m) = self.project().media_by_id(&h.media_id)
                    && let Some(dur) = m.duration
                    && dur.is_finite()
                    && dur > 0.0
                {
                    let speed = if h.speed > 0.0 { h.speed } else { 1.0 };
                    let rem = dur - (h.source_start + h.duration * speed);
                    if rem >= 0.0 {
                        max_d = max_d.min((rem / speed) as f32);
                    }
                }

                let actual_delta = delta.clamp(min_d, max_d);
                let head_dur = *head_duration + actual_delta;
                let tail_st = *tail_start + actual_delta;
                let tail_dur = *tail_duration - actual_delta;
                let tail_src = *tail_source_start + actual_delta;

                if let Some(h) = self.echo_clip_mut(&head_id) {
                    h.duration = f64::from(head_dur);
                }
                if let Some(t) = self.echo_clip_mut(&tail_id) {
                    t.start = f64::from(tail_st);
                    t.duration = f64::from(tail_dur);
                    t.source_start = f64::from(tail_src);
                }

                // Update linked audio/video partners in echo
                let mut linked_updates = Vec::new();
                if let Some(echo) = self.echo.as_ref() {
                    for c in &echo.active().clips {
                        if c.detached_from.as_deref() == Some(&head_id) {
                            linked_updates.push((c.id.clone(), None, Some(head_dur), None));
                        } else if c.detached_from.as_deref() == Some(&tail_id) {
                            linked_updates.push((
                                c.id.clone(),
                                Some(tail_st),
                                Some(tail_dur),
                                Some(tail_src),
                            ));
                        }
                    }
                }
                for (id, st, dur, src) in linked_updates {
                    if let Some(c) = self.echo_clip_mut(&id) {
                        if let Some(s) = st {
                            c.start = f64::from(s);
                        }
                        if let Some(d) = dur {
                            c.duration = f64::from(d);
                        }
                        if let Some(sr) = src {
                            c.source_start = f64::from(sr);
                        }
                    }
                }

                if self.prefs.trim_follow {
                    self.seek(tail_st);
                }
                self.notify(&format!("Rolling Trim: {:+.2}s", actual_delta), false);
            }
            Gesture::Slip {
                clip,
                original_source_start,
            } => {
                let id = clip.clone();
                let orig_src = *original_source_start;
                let delta = seconds;

                let mut min_d = -orig_src;
                let mut max_d = 1_000_000.0f32;

                if let Some(c) = self.clip(&id)
                    && let Some(media) = self.project().media_by_id(&c.media_id)
                    && let Some(dur) = media.duration
                    && dur.is_finite()
                    && dur > 0.0
                {
                    let speed = if c.speed > 0.0 { c.speed } else { 1.0 };
                    let consumed = c.duration * speed;
                    let remaining = (dur - consumed).max(0.0) as f32;
                    max_d = (remaining - orig_src).max(0.0);
                }

                let actual_delta = delta.clamp(min_d, max_d);
                let new_src = orig_src + actual_delta;

                if let Some(c) = self.echo_clip_mut(&id) {
                    c.source_start = f64::from(new_src);
                }

                // Update linked audio/video partner clips in echo
                let mut linked_ids = Vec::new();
                if let Some(echo) = self.echo.as_ref() {
                    for c in &echo.active().clips {
                        if c.detached_from.as_deref() == Some(&id) {
                            linked_ids.push(c.id.clone());
                        }
                    }
                }
                for lid in linked_ids {
                    if let Some(c) = self.echo_clip_mut(&lid) {
                        c.source_start = f64::from(new_src);
                    }
                }

                self.notify(&format!("Slip: {:+.2}s", actual_delta), false);
            }
            Gesture::Slide {
                clip,
                prev_clip,
                next_clip,
                prev_duration,
                target_start,
                next_start,
                next_duration,
                next_source_start,
            } => {
                let delta = seconds;
                let mut min_d = -(*prev_duration - 0.0166667);
                let mut max_d = *next_duration - 0.0166667;

                min_d = min_d.max(-*next_source_start);

                let actual_delta = delta.clamp(min_d, max_d);
                let actual_prev_dur = *prev_duration + actual_delta;
                let actual_target_start = *target_start + actual_delta;
                let actual_next_start = *next_start + actual_delta;
                let actual_next_dur = *next_duration - actual_delta;
                let actual_next_src = *next_source_start + actual_delta;

                if let Some(p) = self.echo_clip_mut(prev_clip) {
                    p.duration = f64::from(actual_prev_dur);
                }
                if let Some(t) = self.echo_clip_mut(clip) {
                    t.start = f64::from(actual_target_start);
                }
                if let Some(n) = self.echo_clip_mut(next_clip) {
                    n.start = f64::from(actual_next_start);
                    n.duration = f64::from(actual_next_dur);
                    n.source_start = f64::from(actual_next_src);
                }

                // Update linked audio/video partner clips in echo
                let mut linked_updates = Vec::new();
                if let Some(echo) = self.echo.as_ref() {
                    for c in &echo.active().clips {
                        if c.detached_from.as_deref() == Some(clip) {
                            linked_updates.push((c.id.clone(), actual_target_start));
                        }
                    }
                }
                for (lid, start) in linked_updates {
                    if let Some(c) = self.echo_clip_mut(&lid) {
                        c.start = f64::from(start);
                    }
                }

                if self.prefs.trim_follow {
                    self.seek(actual_target_start);
                }
                self.notify(&format!("Slide: {:+.2}s", actual_delta), false);
            }
            Gesture::TransitionResize {
                clip,
                original_duration,
            } => {
                let id = clip.clone();
                let original_duration = *original_duration;
                if let Some(clip_ref) = self.clip(&id).cloned() {
                    let wanted = (original_duration + seconds).max(0.0) as f64;
                    if let Some(duration) = self.transition_duration(&clip_ref, wanted) {
                        if let Some(echo_clip) = self.echo_clip_mut(&id) {
                            if let Some(t) = echo_clip.transition_in.as_mut() {
                                t.duration = duration;
                            }
                        }
                    }
                }
            }
            // A stage gesture is the monitor's; the lanes have nothing to
            // add to it.
            Gesture::None
            | Gesture::StageMove { .. }
            | Gesture::StageScale { .. }
            | Gesture::StageRotate { .. }
            | Gesture::StageStretch { .. }
            | Gesture::TextWidth { .. }
            | Gesture::TextHeight { .. }
            | Gesture::StageMask { .. }
            | Gesture::Paint { .. } => {}
        }
        self.gesture = gesture;
    }

    /// The pointer let go: the whole gesture becomes one command.
    pub fn clip_released(&mut self) {
        let gesture = std::mem::replace(&mut self.gesture, Gesture::None);
        let Some(echo) = self.echo.as_ref() else {
            return;
        };
        match gesture {
            Gesture::Move { origins, .. } => {
                let moves: Vec<ClipMove> = origins
                    .iter()
                    .filter_map(|origin| {
                        let after = echo.active().clip(&origin.clip)?;
                        Some(ClipMove {
                            clip_id: origin.clip.clone(),
                            start: after.start,
                            track_id: after.track_id.clone(),
                        })
                    })
                    .collect();
                self.echo = None;
                // A press and a release with no drag between them is a
                // click: nothing moved, and a MoveClips of the clips onto
                // their own places is still an undo step and still a pass
                // through the drop's rules, which can put a clip elsewhere.
                // https://github.com/jub0t/Concat/issues/247
                if moved_any(&moves, |id| self.clip(id)) {
                    self.apply(Command::MoveClips { moves });
                }
            }
            Gesture::Trim {
                clip,
                edge,
                start,
                duration,
                ..
            } => {
                let after = echo.active().clip(&clip).cloned();
                self.echo = None;
                let Some(after) = after else { return };
                let delta = match edge {
                    Edge::Start => after.start - f64::from(start),
                    Edge::End => after.duration - f64::from(duration),
                };
                if delta.abs() > 1e-6 {
                    let id = clip.clone();
                    self.apply(Command::TrimClip {
                        clip_id: clip,
                        edge: match edge {
                            Edge::Start => TrimEdge::Start,
                            Edge::End => TrimEdge::End,
                        },
                        delta,
                        // Magnetic: the lane closes behind the edge.
                        // https://github.com/jub0t/Concat/issues/106
                        ripple: self.prefs.magnetic,
                    });
                    // Trim follow: a magnetic head trim slides the clip back
                    // into the gap, and the playhead goes with its first
                    // frame.
                    if self.prefs.trim_follow && edge == Edge::Start {
                        if let Some(start) = self.clip(&id).map(|clip| clip.start) {
                            self.seek(start as f32);
                        }
                    }
                }
            }
            Gesture::RollTrim {
                head_clip,
                tail_clip,
                head_duration,
                ..
            } => {
                let after_head = echo.active().clip(&head_clip).cloned();
                self.echo = None;
                if let Some(after) = after_head {
                    let delta = after.duration - f64::from(head_duration);
                    if delta.abs() > 1e-6 {
                        self.apply(Command::RollTrim {
                            head_clip_id: head_clip,
                            tail_clip_id: tail_clip,
                            delta,
                        });
                        self.notify(&format!("Rolled cut: {:+.2}s", delta), false);
                    }
                }
            }
            Gesture::Slip {
                clip,
                original_source_start,
            } => {
                let after = echo.active().clip(&clip).cloned();
                self.echo = None;
                if let Some(after) = after {
                    let delta = after.source_start - f64::from(original_source_start);
                    if delta.abs() > 1e-6 {
                        self.apply(Command::SlipClip {
                            clip_id: clip,
                            delta,
                        });
                        self.notify(&format!("Slipped: {:+.2}s", delta), false);
                    }
                }
            }
            Gesture::Slide {
                clip, target_start, ..
            } => {
                let after = echo.active().clip(&clip).cloned();
                self.echo = None;
                if let Some(after) = after {
                    let delta = after.start - f64::from(target_start);
                    if delta.abs() > 1e-6 {
                        self.apply(Command::SlideClip {
                            clip_id: clip,
                            delta,
                        });
                        self.notify(&format!("Slid: {:+.2}s", delta), false);
                    }
                }
            }
            Gesture::TransitionResize {
                clip,
                original_duration,
            } => {
                let after = echo.active().clip(&clip).cloned();
                self.echo = None;
                let Some(after) = after else { return };
                let new_duration = after
                    .transition_in
                    .as_ref()
                    .map(|t| t.duration)
                    .unwrap_or(0.0);
                if (new_duration - f64::from(original_duration)).abs() > 1e-4 {
                    self.set_clip_transition_duration(&clip, new_duration);
                }
            }
            Gesture::None => {
                self.echo = None;
            }
            // Not a lane gesture: hand it back untouched, echo and all.
            other @ (Gesture::StageMove { .. }
            | Gesture::StageScale { .. }
            | Gesture::StageRotate { .. }
            | Gesture::StageStretch { .. }
            | Gesture::TextWidth { .. }
            | Gesture::TextHeight { .. }
            | Gesture::StageMask { .. }
            | Gesture::Paint { .. }) => {
                self.gesture = other;
            }
        }
    }

    /// Abort any gesture in flight or revert active editing tool to Select mode.
    /// Returns true if a gesture was discarded or a tool was reset.
    pub fn cancel_gesture(&mut self) -> bool {
        if self.echo.is_some() || !matches!(self.gesture, Gesture::None) {
            self.echo = None;
            self.gesture = Gesture::None;
            self.notify("Edit cancelled", false);
            true
        } else if self.lanes.tool != TimelineTool::Select {
            self.lanes.tool = TimelineTool::Select;
            self.notify("Selection tool active", false);
            true
        } else {
            false
        }
    }
}
