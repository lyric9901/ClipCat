// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Placing, moving and cutting clips: the edits that change what is on the timeline and when.
//!
//! One arm per command, exactly as [`super::apply`] routes them here;
//! everything these arms share lives in the parent module.

use std::collections::HashMap;

use super::*;

/// Applies one of this module's commands. Any other is a routing error.
pub(super) fn apply(
    project: &mut Project,
    mint: &mut IdMint,
    command: Command,
) -> Result<Outcome, CommandError> {
    match command {
        Command::AddClip {
            media_id,
            track_id,
            start,
            ripple,
        } => {
            let media = project
                .media_by_id(&media_id)
                .ok_or(CommandError::MediaGone)?
                .clone();
            let timeline = project.active_mut();
            if timeline.track(&track_id).is_none() {
                return Err(CommandError::TrackGone);
            }
            if timeline.is_track_locked(&track_id) {
                return Err(CommandError::TrackLocked);
            }
            // Magnetic, the lane makes room; otherwise the clip takes the
            // nearest gap it fits, never the top of another clip.
            let start = if ripple {
                ripple_room_for(timeline, &track_id, start, &media)
            } else {
                timeline.nearest_free_start(&track_id, start, clip_length(&media), &[])
            };
            let id = mint.next("c");
            timeline
                .clips
                .push(Arc::new(default_clip(id.clone(), track_id, &media, start)));
            Ok(Outcome {
                created_id: Some(id),
                applied: true,
            })
        }

        Command::AddClipAtFirstFree { media_id, start } => {
            let media = project
                .media_by_id(&media_id)
                .ok_or(CommandError::MediaGone)?
                .clone();
            let duration = clip_length(&media);
            let timeline = project.active_mut();
            let track_id = first_free_track(timeline, start, duration)
                .unwrap_or_else(|| fresh_track(timeline, mint));
            let id = mint.next("c");
            timeline
                .clips
                .push(Arc::new(default_clip(id.clone(), track_id, &media, start)));
            Ok(Outcome {
                created_id: Some(id),
                applied: true,
            })
        }

        Command::AddClipsInSequence {
            media_ids,
            track_id,
            start,
            ripple,
        } => {
            let items = media_ids
                .iter()
                .map(|id| {
                    project
                        .media_by_id(id)
                        .cloned()
                        .ok_or(CommandError::MediaGone)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if items.is_empty() {
                return Ok(Outcome {
                    created_id: None,
                    applied: false,
                });
            }
            let start = start.max(0.0);
            let total: f64 = items.iter().map(clip_length).sum();
            let timeline = project.active_mut();
            let track_id = match track_id {
                Some(id) if timeline.track(&id).is_none() => return Err(CommandError::TrackGone),
                Some(id) if timeline.is_track_locked(&id) => return Err(CommandError::TrackLocked),
                Some(id) => id,
                None => first_free_track(timeline, start, total)
                    .unwrap_or_else(|| fresh_track(timeline, mint)),
            };
            let mut at = start;
            let mut first = None;
            for media in &items {
                let place = if ripple {
                    ripple_room_for(timeline, &track_id, at, media)
                } else {
                    timeline.nearest_free_start(&track_id, at, clip_length(media), &[])
                };
                let id = mint.next("c");
                timeline.clips.push(Arc::new(default_clip(
                    id.clone(),
                    track_id.clone(),
                    media,
                    place,
                )));
                first.get_or_insert(id);
                at = place + clip_length(media);
            }
            Ok(Outcome {
                created_id: first,
                applied: true,
            })
        }

        Command::AddTextClip {
            track_id,
            start,
            style,
            duration,
            offset_y,
            above,
        } => {
            let style = style.unwrap_or_default();
            let duration = duration
                .unwrap_or(DEFAULT_TEXT_DURATION)
                .max(MIN_CLIP_DURATION);
            let timeline = project.active_mut();
            let track_id = match track_id {
                Some(id) if timeline.track(&id).is_none() => return Err(CommandError::TrackGone),
                Some(id) if timeline.is_track_locked(&id) => return Err(CommandError::TrackLocked),
                Some(id) => id,
                // Every lane above the video is taken: a new one at the top
                // for the words to land on.
                None if above => first_free_track_above(timeline, start, duration)
                    .unwrap_or_else(|| fresh_track(timeline, mint)),
                None => first_free_track(timeline, start, duration)
                    .unwrap_or_else(|| fresh_track(timeline, mint)),
            };
            let start = timeline.nearest_free_start(&track_id, start, duration, &[]);
            let id = mint.next("c");
            let mut clip = Clip::blank(
                id.clone(),
                track_id,
                ClipKind::Text,
                first_line(&style.content),
                start,
                duration,
            );
            clip.offset_y = offset_y.unwrap_or(0.0).clamp(-MAX_OFFSET, MAX_OFFSET);
            clip.text = Some(style);
            timeline.clips.push(Arc::new(clip));
            Ok(Outcome {
                created_id: Some(id),
                applied: true,
            })
        }

        Command::AddShapeClip {
            track_id,
            above,
            start,
            style,
            duration,
            name,
        } => {
            let style = style.unwrap_or_default().tidy();
            let duration = duration
                .unwrap_or(DEFAULT_TEXT_DURATION)
                .max(MIN_CLIP_DURATION);
            let timeline = project.active_mut();
            let track_id = match track_id {
                Some(id) if timeline.track(&id).is_none() => return Err(CommandError::TrackGone),
                Some(id) if timeline.is_track_locked(&id) => return Err(CommandError::TrackLocked),
                Some(id) => id,
                None if above => first_free_track_above(timeline, start, duration)
                    .unwrap_or_else(|| fresh_track(timeline, mint)),
                None => first_free_track(timeline, start, duration)
                    .unwrap_or_else(|| fresh_track(timeline, mint)),
            };
            let start = timeline.nearest_free_start(&track_id, start, duration, &[]);
            let id = mint.next("c");
            let name = if name.trim().is_empty() {
                style.kind.id().to_owned()
            } else {
                name
            };
            let mut clip =
                Clip::blank(id.clone(), track_id, ClipKind::Shape, name, start, duration);
            clip.shape = Some(style);
            timeline.clips.push(Arc::new(clip));
            Ok(Outcome {
                created_id: Some(id),
                applied: true,
            })
        }

        Command::AddLayerClip {
            track_id,
            start,
            duration,
            effect_id,
            name,
            sound,
        } => {
            let duration = duration
                .unwrap_or(DEFAULT_LAYER_DURATION)
                .max(MIN_CLIP_DURATION);
            let timeline = project.active_mut();
            let track_id = match track_id {
                Some(id) if timeline.track(&id).is_none() => return Err(CommandError::TrackGone),
                Some(id) if timeline.is_track_locked(&id) => return Err(CommandError::TrackLocked),
                Some(id) => id,
                // Over the picture, as a title is: a layer treats what is
                // beneath it, and the bottom-first walk put one under the
                // footage whenever a lower lane was free, where it treated
                // nothing. A sound layer treats every lane alike, and goes
                // over the picture too, where it reads as a treatment and
                // not as one more sound.
                None => first_free_track_above(timeline, start, duration)
                    .unwrap_or_else(|| fresh_track(timeline, mint)),
            };
            let start = timeline.nearest_free_start(&track_id, start, duration, &[]);
            let id = mint.next("c");
            let name = if name.trim().is_empty() {
                effect_id.clone()
            } else {
                name
            };
            let kind = if sound {
                ClipKind::SoundLayer
            } else {
                ClipKind::Layer
            };
            let mut clip = Clip::blank(id.clone(), track_id, kind, name, start, duration);
            if sound {
                clip.filters = vec![AppliedFilter::new(effect_id)];
            } else {
                clip.video_effects = vec![AppliedFilter::new(effect_id)];
            }
            timeline.clips.push(Arc::new(clip));
            Ok(Outcome {
                created_id: Some(id),
                applied: true,
            })
        }

        Command::AddTransitionClip {
            track_id,
            start,
            duration,
            transition_id,
            name,
        } => {
            let duration = duration
                .unwrap_or(DEFAULT_TRANSITION_DURATION)
                .max(crate::model::ranges::MIN_TRANSITION);
            let timeline = project.active_mut();
            let track_id = match track_id {
                Some(id) if timeline.track(&id).is_none() => return Err(CommandError::TrackGone),
                Some(id) if timeline.is_track_locked(&id) => return Err(CommandError::TrackLocked),
                Some(id) => id,
                // Over the picture, as a layer is: a transition acts on what
                // is beneath it, and one under the footage would act on
                // nothing.
                None => first_free_track_above(timeline, start, duration)
                    .unwrap_or_else(|| fresh_track(timeline, mint)),
            };
            let start = timeline.nearest_free_start(&track_id, start, duration, &[]);
            let id = mint.next("c");
            let name = if name.trim().is_empty() {
                transition_id.clone()
            } else {
                name
            };
            let mut clip = Clip::blank(
                id.clone(),
                track_id,
                ClipKind::Transition,
                name,
                start,
                duration,
            );
            clip.video_effects = vec![AppliedFilter::new(transition_id)];
            timeline.clips.push(Arc::new(clip));
            Ok(Outcome {
                created_id: Some(id),
                applied: true,
            })
        }

        Command::MoveClips { moves } => {
            let timeline = project.active_mut();
            for wanted in &moves {
                if let Some(clip) = timeline.clip(&wanted.clip_id) {
                    if timeline.is_track_locked(&clip.track_id) {
                        return Err(CommandError::TrackLocked);
                    }
                    if timeline.track(&wanted.track_id).is_some()
                        && timeline.is_track_locked(&wanted.track_id)
                    {
                        return Err(CommandError::TrackLocked);
                    }
                }
            }
            // Onto another clip is never where a move lands: the group goes
            // to the nearest gap that holds it, and a move whose own clips
            // would cover each other stays where it was.
            let Some(moves) = timeline.resolve_moves(&moves) else {
                return Ok(Outcome::default());
            };
            let mut applied = false;
            for wanted in moves {
                if let Some(clip) = timeline.clip_mut(&wanted.clip_id) {
                    applied |= assign(&mut clip.start, wanted.start);
                    applied |= assign(&mut clip.track_id, wanted.track_id);
                }
            }
            Ok(Outcome {
                created_id: None,
                applied,
            })
        }

        Command::TrimClip {
            clip_id,
            edge,
            delta,
            ripple,
        } => {
            let timeline = project.active_mut();
            let main = timeline.background_lane().map(str::to_owned);
            let Some(clip) = timeline.clip(&clip_id) else {
                return Ok(Outcome::default());
            };
            if timeline.is_track_locked(&clip.track_id) {
                return Err(CommandError::TrackLocked);
            }
            let track_id = clip.track_id.clone();
            let anchor = clip.start;
            let old_end = clip.start + clip.duration;
            // A plain trim stops at the neighbours; a magnetic one moves
            // them instead, so it has the lane's whole length.
            let (floor, ceiling) = if ripple {
                (0.0, f64::INFINITY)
            } else {
                let (before, after) = timeline
                    .room_around(&clip_id)
                    .unwrap_or((0.0, f64::INFINITY));
                (before, after)
            };
            let Some(clip) = timeline.clip_mut(&clip_id) else {
                return Ok(Outcome::default());
            };
            // What the trim did, and what the lane behind it does about
            // it when it is magnetic: `by` is how far the later clips
            // move, `behind` where "later" begins.
            let (applied, by, behind) = match edge {
                TrimEdge::End => {
                    let duration = (clip.duration + delta)
                        .min(ceiling - clip.start)
                        .max(MIN_CLIP_DURATION);
                    let old = clip.duration;
                    let applied = assign(&mut clip.duration, duration);
                    if applied {
                        clip.rewindow_keys(old, 0.0, duration);
                        crate::speed::rewindow_clip(clip, old, 0.0, duration);
                    }
                    (applied, duration - old, old_end - JOIN_EPSILON)
                }
                TrimEdge::Start => {
                    // Dragging the head moves the in-point too, so the pixels
                    // under the remaining part of the clip do not slide. The
                    // head cannot reach before the source begins.
                    // Under a curve the head extends at the speed it starts at.
                    let lead = clip
                        .speed_curve
                        .as_deref()
                        .and_then(crate::speed::curve_of)
                        .map_or(clip.speed, |curve| curve.speed_at(0.0));
                    let shift = delta
                        .min(clip.duration - MIN_CLIP_DURATION)
                        .max(-clip.source_start / lead);
                    // A magnetic head trim never moves the clip, so the
                    // timeline's own start is no limit to it; a plain one
                    // stops at zero.
                    let start = if ripple {
                        clip.start + shift
                    } else {
                        (clip.start + shift).max(floor)
                    };
                    let moved = start - clip.start;
                    let duration = clip.duration - moved;
                    let old = clip.duration;
                    // The source the head took with it: a constant rate's
                    // share, or under a curve the curve's own. Worked on a
                    // copy, so a trim that changes nothing changes nothing.
                    let mut retimed = Clip::clone(clip);
                    let consumed = crate::speed::rewindow_clip(&mut retimed, old, moved, old)
                        .unwrap_or(moved * clip.speed);
                    let source_start = (clip.source_start + consumed).max(0.0);
                    // Bitwise so no assignment is short-circuited away.
                    let applied = assign(&mut clip.start, start)
                        | assign(&mut clip.duration, duration)
                        | assign(&mut clip.source_start, source_start);
                    if applied {
                        clip.rewindow_keys(old, moved, old);
                        clip.speed = retimed.speed;
                        clip.speed_curve = retimed.speed_curve;
                    }
                    if ripple {
                        // The in-point moved; the clip stays put, and the
                        // lane behind it closes by what came off the head.
                        clip.start = anchor;
                    }
                    (applied, -moved, anchor + JOIN_EPSILON)
                }
            };
            if ripple && applied && by != 0.0 {
                for other in timeline.clips_where(|other| {
                    other.id != clip_id && other.track_id == track_id && other.start >= behind
                }) {
                    other.start = (other.start + by).max(0.0);
                }
                // In the lane's old time the edit is at the tail's old end
                // or the clip's start, whichever edge moved: what came off
                // went from there, what was restored went in there.
                let at = match edge {
                    TrimEdge::End => old_end,
                    TrimEdge::Start => anchor,
                };
                let ripple = if by < 0.0 {
                    let from = match edge {
                        TrimEdge::End => old_end + by,
                        TrimEdge::Start => anchor,
                    };
                    Ripple::Removed(vec![(from, from - by)])
                } else {
                    Ripple::Inserted { at, length: by }
                };
                follow_main(timeline, main.as_deref(), &track_id, ripple);
            }
            Ok(Outcome {
                created_id: None,
                applied,
            })
        }

        Command::RollTrim {
            head_clip_id,
            tail_clip_id,
            delta,
        } => {
            use concat_core::time::Rational;

            let timeline = project.active();
            let Some(head) = timeline.clip(&head_clip_id) else {
                return Ok(Outcome::default());
            };
            let Some(tail) = timeline.clip(&tail_clip_id) else {
                return Ok(Outcome::default());
            };

            // Both clips must be on the same track, distinct, and adjacent within tolerance
            if head.track_id != tail.track_id || head.id == tail.id {
                return Ok(Outcome::default());
            }

            if timeline.is_track_locked(&head.track_id) {
                return Err(CommandError::TrackLocked);
            }

            let head_end = head.start + head.duration;
            if (tail.start - head_end).abs() > JOIN_EPSILON || tail.start < head.start {
                return Ok(Outcome::default());
            }

            let Some(delta_rat) = Rational::approximate(delta) else {
                return Ok(Outcome::default());
            };
            if delta_rat.is_zero() {
                return Ok(Outcome::default());
            }

            // Collect all pairs to roll (including linked video/audio partners)
            let mut pairs = vec![(head_clip_id.clone(), tail_clip_id.clone())];

            // 1. Linked audio partners if head & tail are video
            for ha in timeline
                .clips
                .iter()
                .filter(|c| c.detached_from.as_deref() == Some(head.id.as_str()))
            {
                for ta in timeline
                    .clips
                    .iter()
                    .filter(|c| c.detached_from.as_deref() == Some(tail.id.as_str()))
                {
                    if ha.track_id == ta.track_id
                        && (ta.start - (ha.start + ha.duration)).abs() <= JOIN_EPSILON
                    {
                        let pair = (ha.id.clone(), ta.id.clone());
                        if !pairs.contains(&pair) {
                            pairs.push(pair);
                        }
                    }
                }
            }

            // 2. Linked video partners if head & tail are detached audio
            if let (Some(head_vid_id), Some(tail_vid_id)) =
                (&head.detached_from, &tail.detached_from)
                && let (Some(hv), Some(tv)) =
                    (timeline.clip(head_vid_id), timeline.clip(tail_vid_id))
                && hv.track_id == tv.track_id
                && (tv.start - (hv.start + hv.duration)).abs() <= JOIN_EPSILON
            {
                let pair = (head_vid_id.clone(), tail_vid_id.clone());
                if !pairs.contains(&pair) {
                    pairs.push(pair);
                }
                for ha in timeline
                    .clips
                    .iter()
                    .filter(|c| c.detached_from.as_deref() == Some(head_vid_id.as_str()))
                {
                    for ta in timeline
                        .clips
                        .iter()
                        .filter(|c| c.detached_from.as_deref() == Some(tail_vid_id.as_str()))
                    {
                        if ha.track_id == ta.track_id
                            && (ta.start - (ha.start + ha.duration)).abs() <= JOIN_EPSILON
                        {
                            let pair = (ha.id.clone(), ta.id.clone());
                            if !pairs.contains(&pair) {
                                pairs.push(pair);
                            }
                        }
                    }
                }
            }

            // Calculate min_delta and max_delta across all participating pairs
            let min_clip_dur_rat = Rational::new(1, 60);
            let mut min_delta = Rational::new(-1_000_000, 1);
            let mut max_delta = Rational::new(1_000_000, 1);

            for (h_id, t_id) in &pairs {
                let Some(h) = timeline.clip(h_id) else {
                    continue;
                };
                let Some(t) = timeline.clip(t_id) else {
                    continue;
                };

                if timeline.is_track_locked(&h.track_id) || timeline.is_track_locked(&t.track_id) {
                    return Err(CommandError::TrackLocked);
                }

                let h_dur_rat = Rational::approximate(h.duration).unwrap_or(min_clip_dur_rat);
                let t_dur_rat = Rational::approximate(t.duration).unwrap_or(min_clip_dur_rat);

                // Head cannot shrink below MIN_CLIP_DURATION
                min_delta = min_delta.max(min_clip_dur_rat - h_dur_rat);

                // Tail cannot shrink below MIN_CLIP_DURATION
                max_delta = max_delta.min(t_dur_rat - min_clip_dur_rat);

                // Tail source start cannot be negative
                let t_lead = t
                    .speed_curve
                    .as_deref()
                    .and_then(crate::speed::curve_of)
                    .map_or(t.speed, |curve| curve.speed_at(0.0));
                let t_lead_rat = Rational::approximate(t_lead).unwrap_or(Rational::ONE);
                let t_src_start_rat =
                    Rational::approximate(t.source_start).unwrap_or(Rational::ZERO);
                if !t_lead_rat.is_zero() {
                    min_delta = min_delta.max(-(t_src_start_rat / t_lead_rat));
                }

                // Head source end cannot exceed media duration if known
                if let Some(media) = project.media_by_id(&h.media_id)
                    && let Some(media_duration) = media.duration
                    && media_duration.is_finite()
                    && media_duration > 0.0
                {
                    let h_tail_speed = h
                        .speed_curve
                        .as_deref()
                        .and_then(crate::speed::curve_of)
                        .map_or(h.speed, |curve| curve.speed_at(1.0));
                    let h_tail_speed_rat =
                        Rational::approximate(h_tail_speed).unwrap_or(Rational::ONE);

                    let h_source_consumed = if let Some(curve) =
                        h.speed_curve.as_deref().and_then(crate::speed::curve_of)
                    {
                        h.duration * curve.mean()
                    } else {
                        h.duration * h.speed
                    };
                    let h_source_end = h.source_start + h_source_consumed;
                    if media_duration > h_source_end {
                        let remaining = media_duration - h_source_end;
                        let rem_rat = Rational::approximate(remaining).unwrap_or(Rational::ZERO);
                        if !h_tail_speed_rat.is_zero() {
                            max_delta = max_delta.min(rem_rat / h_tail_speed_rat);
                        }
                    } else {
                        max_delta = max_delta.min(Rational::ZERO);
                    }
                }
            }

            if min_delta > max_delta {
                return Ok(Outcome::default());
            }

            let clamped_delta_rat = delta_rat.clamp_to(min_delta, max_delta);
            if clamped_delta_rat.is_zero() {
                return Ok(Outcome::default());
            }

            let clamped_delta = clamped_delta_rat.as_f64();

            // Mutate project timeline atomically
            let timeline_mut = project.active_mut();
            let mut applied = false;

            for (h_id, t_id) in pairs {
                let Some(h) = timeline_mut.clip_mut(&h_id) else {
                    continue;
                };
                let old_head_dur = h.duration;
                let old_head_dur_rat =
                    Rational::approximate(old_head_dur).unwrap_or(Rational::ZERO);
                let new_head_dur = (old_head_dur_rat + clamped_delta_rat).as_f64();

                let h_applied = assign(&mut h.duration, new_head_dur);
                if h_applied {
                    h.rewindow_keys(old_head_dur, 0.0, new_head_dur);
                    crate::speed::rewindow_clip(h, old_head_dur, 0.0, new_head_dur);
                    applied = true;
                }

                let Some(t) = timeline_mut.clip_mut(&t_id) else {
                    continue;
                };
                let old_tail_dur = t.duration;
                let old_tail_start = t.start;
                let old_tail_dur_rat =
                    Rational::approximate(old_tail_dur).unwrap_or(Rational::ZERO);
                let old_tail_start_rat =
                    Rational::approximate(old_tail_start).unwrap_or(Rational::ZERO);

                let new_tail_start = (old_tail_start_rat + clamped_delta_rat).as_f64();
                let new_tail_dur = (old_tail_dur_rat - clamped_delta_rat).as_f64();

                let mut retimed = Clip::clone(t);
                let consumed = crate::speed::rewindow_clip(
                    &mut retimed,
                    old_tail_dur,
                    clamped_delta,
                    old_tail_dur,
                )
                .unwrap_or(clamped_delta * t.speed);
                let new_source_start = (t.source_start + consumed).max(0.0);

                let t_applied = assign(&mut t.start, new_tail_start)
                    | assign(&mut t.duration, new_tail_dur)
                    | assign(&mut t.source_start, new_source_start);

                if t_applied {
                    t.rewindow_keys(old_tail_dur, clamped_delta, old_tail_dur);
                    t.speed = retimed.speed;
                    t.speed_curve = retimed.speed_curve;
                    if let Some(transition) = t.transition_in.as_mut()
                        && transition.duration > new_tail_dur
                    {
                        transition.duration = new_tail_dur;
                    }
                    applied = true;
                }
            }

            Ok(Outcome {
                created_id: None,
                applied,
            })
        }

        Command::SlipClip { clip_id, delta } => {
            use concat_core::time::Rational;

            let timeline = project.active();
            let Some(clip) = timeline.clip(&clip_id) else {
                return Ok(Outcome::default());
            };
            if timeline.is_track_locked(&clip.track_id) {
                return Err(CommandError::TrackLocked);
            }

            // Clips with no media (e.g. text/shape) cannot be slipped
            if clip.media_id.is_empty() {
                return Ok(Outcome::default());
            }

            let Some(delta_rat) = Rational::approximate(delta) else {
                return Ok(Outcome::default());
            };
            if delta_rat.is_zero() {
                return Ok(Outcome::default());
            }

            // Collect target clip and linked AV partners (e.g. detached audio or parent video)
            let mut slip_ids = vec![clip_id.clone()];

            // If this clip is video, find any detached audio clips linked to it
            for c in &timeline.clips {
                if c.detached_from.as_deref() == Some(clip.id.as_str()) && !slip_ids.contains(&c.id)
                {
                    slip_ids.push(c.id.clone());
                }
            }

            // If this clip is detached audio, find the parent video clip and any sibling audio clips
            if let Some(parent_video_id) = &clip.detached_from {
                if !slip_ids.contains(parent_video_id) && timeline.clip(parent_video_id).is_some() {
                    slip_ids.push(parent_video_id.clone());
                }
                for c in &timeline.clips {
                    if c.detached_from.as_deref() == Some(parent_video_id.as_str())
                        && !slip_ids.contains(&c.id)
                    {
                        slip_ids.push(c.id.clone());
                    }
                }
            }

            // Compute global min_delta and max_delta across all participating clips
            let mut min_delta = Rational::new(-1_000_000, 1);
            let mut max_delta = Rational::new(1_000_000, 1);

            for id in &slip_ids {
                let Some(c) = timeline.clip(id) else {
                    continue;
                };

                if timeline.is_track_locked(&c.track_id) {
                    return Err(CommandError::TrackLocked);
                }

                // Lower bound: source_start cannot go below 0
                let src_start_rat = Rational::approximate(c.source_start).unwrap_or(Rational::ZERO);
                min_delta = min_delta.max(-src_start_rat);

                // Upper bound: source_end cannot exceed media duration if known
                if let Some(media) = project.media_by_id(&c.media_id)
                    && let Some(media_duration) = media.duration
                    && media_duration.is_finite()
                    && media_duration > 0.0
                {
                    let source_consumed = if let Some(curve) =
                        c.speed_curve.as_deref().and_then(crate::speed::curve_of)
                    {
                        c.duration * curve.mean()
                    } else {
                        c.duration * c.speed
                    };
                    let consumed_rat =
                        Rational::approximate(source_consumed).unwrap_or(Rational::ZERO);
                    let media_dur_rat =
                        Rational::approximate(media_duration).unwrap_or(Rational::ZERO);
                    let remaining = media_dur_rat - (src_start_rat + consumed_rat);
                    max_delta = max_delta.min(remaining);
                }
            }

            if min_delta > max_delta {
                return Ok(Outcome::default());
            }

            let clamped_delta_rat = delta_rat.clamp_to(min_delta, max_delta);
            if clamped_delta_rat.is_zero() {
                return Ok(Outcome::default());
            }

            // Mutate project timeline atomically
            let timeline_mut = project.active_mut();
            let mut applied = false;

            for id in slip_ids {
                let Some(c) = timeline_mut.clip_mut(&id) else {
                    continue;
                };
                let old_src = c.source_start;
                let old_src_rat = Rational::approximate(old_src).unwrap_or(Rational::ZERO);
                let new_src = (old_src_rat + clamped_delta_rat).as_f64().max(0.0);
                applied |= assign(&mut c.source_start, new_src);
            }

            Ok(Outcome {
                created_id: None,
                applied,
            })
        }

        Command::SlideClip { clip_id, delta } => {
            use concat_core::time::Rational;

            let timeline = project.active();
            let Some(target) = timeline.clip(&clip_id) else {
                return Ok(Outcome::default());
            };
            if timeline.is_track_locked(&target.track_id) {
                return Err(CommandError::TrackLocked);
            }

            let Some(delta_rat) = Rational::approximate(delta) else {
                return Ok(Outcome::default());
            };
            if delta_rat.is_zero() {
                return Ok(Outcome::default());
            }

            // Find preceding and succeeding adjacent clips on the same track
            let track_id = &target.track_id;
            let target_start = target.start;
            let target_end = target.start + target.duration;

            let prev_clip = timeline.clips.iter().find(|c| {
                c.track_id == *track_id
                    && c.id != target.id
                    && (c.start + c.duration - target_start).abs() <= JOIN_EPSILON
            });
            let next_clip = timeline.clips.iter().find(|c| {
                c.track_id == *track_id
                    && c.id != target.id
                    && (c.start - target_end).abs() <= JOIN_EPSILON
            });

            // Slide requires both neighbours to be present and adjacent
            let (Some(prev), Some(next)) = (prev_clip, next_clip) else {
                return Ok(Outcome::default());
            };

            // Collect participating triplets (prev, target, next), including linked AV partners
            let mut triplets = vec![(prev.id.clone(), target.id.clone(), next.id.clone())];

            // 1. Linked audio partners if target is video
            for target_a in timeline
                .clips
                .iter()
                .filter(|c| c.detached_from.as_deref() == Some(target.id.as_str()))
            {
                let a_track = &target_a.track_id;
                let a_start = target_a.start;
                let a_end = target_a.start + target_a.duration;

                let prev_a = timeline.clips.iter().find(|c| {
                    c.track_id == *a_track
                        && c.id != target_a.id
                        && (c.start + c.duration - a_start).abs() <= JOIN_EPSILON
                });
                let next_a = timeline.clips.iter().find(|c| {
                    c.track_id == *a_track
                        && c.id != target_a.id
                        && (c.start - a_end).abs() <= JOIN_EPSILON
                });

                if let (Some(pa), Some(na)) = (prev_a, next_a) {
                    let triplet = (pa.id.clone(), target_a.id.clone(), na.id.clone());
                    if !triplets.contains(&triplet) {
                        triplets.push(triplet);
                    }
                }
            }

            // 2. Linked video partners if target is detached audio
            if let Some(target_video_id) = &target.detached_from
                && let Some(tv) = timeline.clip(target_video_id)
            {
                let v_track = &tv.track_id;
                let v_start = tv.start;
                let v_end = tv.start + tv.duration;

                let prev_v = timeline.clips.iter().find(|c| {
                    c.track_id == *v_track
                        && c.id != tv.id
                        && (c.start + c.duration - v_start).abs() <= JOIN_EPSILON
                });
                let next_v = timeline.clips.iter().find(|c| {
                    c.track_id == *v_track
                        && c.id != tv.id
                        && (c.start - v_end).abs() <= JOIN_EPSILON
                });

                if let (Some(pv), Some(nv)) = (prev_v, next_v) {
                    let triplet = (pv.id.clone(), tv.id.clone(), nv.id.clone());
                    if !triplets.contains(&triplet) {
                        triplets.push(triplet);
                    }
                }
            }

            // Compute global min_delta and max_delta across all triplets
            let min_clip_dur_rat = Rational::new(1, 60);
            let mut min_delta = Rational::new(-1_000_000, 1);
            let mut max_delta = Rational::new(1_000_000, 1);

            for (p_id, t_id, n_id) in &triplets {
                let (Some(p), Some(t), Some(n)) = (
                    timeline.clip(p_id),
                    timeline.clip(t_id),
                    timeline.clip(n_id),
                ) else {
                    continue;
                };

                if timeline.is_track_locked(&p.track_id)
                    || timeline.is_track_locked(&t.track_id)
                    || timeline.is_track_locked(&n.track_id)
                {
                    return Err(CommandError::TrackLocked);
                }

                let p_dur_rat = Rational::approximate(p.duration).unwrap_or(min_clip_dur_rat);
                let n_dur_rat = Rational::approximate(n.duration).unwrap_or(min_clip_dur_rat);

                // Preceding clip cannot shrink below MIN_CLIP_DURATION
                min_delta = min_delta.max(min_clip_dur_rat - p_dur_rat);

                // Succeeding clip cannot shrink below MIN_CLIP_DURATION
                max_delta = max_delta.min(n_dur_rat - min_clip_dur_rat);

                // Succeeding clip source_start cannot go below 0 (when expanding left)
                let n_lead = n
                    .speed_curve
                    .as_deref()
                    .and_then(crate::speed::curve_of)
                    .map_or(n.speed, |curve| curve.speed_at(0.0));
                let n_lead_rat = Rational::approximate(n_lead).unwrap_or(Rational::ONE);
                let n_src_start_rat =
                    Rational::approximate(n.source_start).unwrap_or(Rational::ZERO);
                if !n_lead_rat.is_zero() {
                    min_delta = min_delta.max(-(n_src_start_rat / n_lead_rat));
                }

                // Preceding clip source_end cannot exceed media duration if known
                if let Some(media) = project.media_by_id(&p.media_id)
                    && let Some(media_duration) = media.duration
                    && media_duration.is_finite()
                    && media_duration > 0.0
                {
                    let p_tail_speed = p
                        .speed_curve
                        .as_deref()
                        .and_then(crate::speed::curve_of)
                        .map_or(p.speed, |curve| curve.speed_at(1.0));
                    let p_tail_speed_rat =
                        Rational::approximate(p_tail_speed).unwrap_or(Rational::ONE);

                    let p_source_consumed = if let Some(curve) =
                        p.speed_curve.as_deref().and_then(crate::speed::curve_of)
                    {
                        p.duration * curve.mean()
                    } else {
                        p.duration * p.speed
                    };
                    let p_source_end = p.source_start + p_source_consumed;
                    if media_duration > p_source_end {
                        let remaining = media_duration - p_source_end;
                        let rem_rat = Rational::approximate(remaining).unwrap_or(Rational::ZERO);
                        if !p_tail_speed_rat.is_zero() {
                            max_delta = max_delta.min(rem_rat / p_tail_speed_rat);
                        }
                    } else {
                        max_delta = max_delta.min(Rational::ZERO);
                    }
                }
            }

            if min_delta > max_delta {
                return Ok(Outcome::default());
            }

            let clamped_delta_rat = delta_rat.clamp_to(min_delta, max_delta);
            if clamped_delta_rat.is_zero() {
                return Ok(Outcome::default());
            }

            let clamped_delta = clamped_delta_rat.as_f64();

            // Mutate project timeline atomically
            let timeline_mut = project.active_mut();
            let mut applied = false;

            for (p_id, t_id, n_id) in triplets {
                // 1. Mutate prev clip (duration expands/contracts)
                let Some(p) = timeline_mut.clip_mut(&p_id) else {
                    continue;
                };
                let old_p_dur = p.duration;
                let old_p_dur_rat = Rational::approximate(old_p_dur).unwrap_or(Rational::ZERO);
                let new_p_dur = (old_p_dur_rat + clamped_delta_rat).as_f64();

                let p_applied = assign(&mut p.duration, new_p_dur);
                if p_applied {
                    p.rewindow_keys(old_p_dur, 0.0, new_p_dur);
                    crate::speed::rewindow_clip(p, old_p_dur, 0.0, new_p_dur);
                    applied = true;
                }

                // 2. Mutate target clip (start slides, duration & source unchanged)
                let Some(t) = timeline_mut.clip_mut(&t_id) else {
                    continue;
                };
                let old_t_start = t.start;
                let old_t_start_rat = Rational::approximate(old_t_start).unwrap_or(Rational::ZERO);
                let new_t_start = (old_t_start_rat + clamped_delta_rat).as_f64();
                applied |= assign(&mut t.start, new_t_start);

                // 3. Mutate next clip (start and duration adjust, source_start shifts)
                let Some(n) = timeline_mut.clip_mut(&n_id) else {
                    continue;
                };
                let old_n_dur = n.duration;
                let old_n_start = n.start;
                let old_n_dur_rat = Rational::approximate(old_n_dur).unwrap_or(Rational::ZERO);
                let old_n_start_rat = Rational::approximate(old_n_start).unwrap_or(Rational::ZERO);

                let new_n_start = (old_n_start_rat + clamped_delta_rat).as_f64();
                let new_n_dur = (old_n_dur_rat - clamped_delta_rat).as_f64();

                let mut retimed = Clip::clone(n);
                let consumed =
                    crate::speed::rewindow_clip(&mut retimed, old_n_dur, clamped_delta, old_n_dur)
                        .unwrap_or(clamped_delta * n.speed);
                let new_n_src = (n.source_start + consumed).max(0.0);

                let n_applied = assign(&mut n.start, new_n_start)
                    | assign(&mut n.duration, new_n_dur)
                    | assign(&mut n.source_start, new_n_src);

                if n_applied {
                    n.rewindow_keys(old_n_dur, clamped_delta, old_n_dur);
                    n.speed = retimed.speed;
                    n.speed_curve = retimed.speed_curve;
                    if let Some(transition) = n.transition_in.as_mut()
                        && transition.duration > new_n_dur
                    {
                        transition.duration = new_n_dur;
                    }
                    applied = true;
                }
            }

            Ok(Outcome {
                created_id: None,
                applied,
            })
        }

        Command::InsertEdit {
            media_id,
            source_in,
            source_out,
            timeline_in,
            timeline_out,
            video_track_id,
            audio_track_id,
        } => {
            let media = project
                .media_by_id(&media_id)
                .ok_or(CommandError::MediaGone)?
                .clone();
            let timeline = project.active();
            if timeline.tracks.is_empty() {
                return Err(CommandError::NoTracks);
            }

            // Determine target tracks
            let (target_video_track, target_audio_track) = match (video_track_id, audio_track_id) {
                (Some(v), Some(a)) => (Some(v), Some(a)),
                (Some(v), None) => (Some(v), None),
                (None, Some(a)) => (None, Some(a)),
                (None, None) => {
                    let unlocked: Vec<String> = timeline
                        .tracks
                        .iter()
                        .filter(|t| !t.locked)
                        .map(|t| t.id.clone())
                        .collect();
                    if unlocked.is_empty() {
                        return Err(CommandError::TrackLocked);
                    }
                    let v_track = Some(unlocked[0].clone());
                    let a_track = if (media.has_audio || media.kind == MediaKind::Audio)
                        && unlocked.len() > 1
                    {
                        Some(unlocked[1].clone())
                    } else if media.has_audio || media.kind == MediaKind::Audio {
                        Some(unlocked[0].clone())
                    } else {
                        None
                    };
                    (v_track, a_track)
                }
            };

            // Validate targeted tracks
            if let Some(ref vid) = target_video_track {
                if timeline.track(vid).is_none() {
                    return Err(CommandError::TrackGone);
                }
                if timeline.is_track_locked(vid) {
                    return Err(CommandError::TrackLocked);
                }
            }
            if let Some(ref aid) = target_audio_track {
                if timeline.track(aid).is_none() {
                    return Err(CommandError::TrackGone);
                }
                if timeline.is_track_locked(aid) {
                    return Err(CommandError::TrackLocked);
                }
            }

            let (s_in, _s_out, tl_in, tl_out) = resolve_three_point(
                &media,
                timeline,
                source_in,
                source_out,
                timeline_in,
                timeline_out,
            );
            let duration = tl_out - tl_in;
            if duration < MIN_CLIP_DURATION {
                return Ok(Outcome::default());
            }

            let should_place_video = media.kind != MediaKind::Audio && target_video_track.is_some();
            let should_place_audio = if media.kind == MediaKind::Audio {
                target_audio_track.is_some()
                    || (target_video_track.is_some() && !should_place_video)
            } else {
                media.has_audio && target_audio_track.is_some()
            };
            let effective_audio_track =
                if media.kind == MediaKind::Audio && target_audio_track.is_none() {
                    target_video_track.clone()
                } else {
                    target_audio_track.clone()
                };

            if !should_place_video && !should_place_audio {
                return Ok(Outcome::default());
            }

            // Identify all tracks that must ripple
            let mut ripple_tracks = HashSet::new();
            if let Some(ref vid) = target_video_track {
                if should_place_video {
                    ripple_tracks.insert(vid.clone());
                }
            }
            if let Some(ref aid) = effective_audio_track {
                if should_place_audio {
                    ripple_tracks.insert(aid.clone());
                }
            }

            // Expand to include linked tracks
            let mut expanded = true;
            while expanded {
                expanded = false;
                for c in &timeline.clips {
                    if ripple_tracks.contains(&c.track_id) {
                        if let Some(ref parent_id) = c.detached_from {
                            if let Some(parent) = timeline.clip(parent_id) {
                                if !timeline.is_track_locked(&parent.track_id)
                                    && ripple_tracks.insert(parent.track_id.clone())
                                {
                                    expanded = true;
                                }
                            }
                        }
                        for other in &timeline.clips {
                            if other.detached_from.as_deref() == Some(&c.id) {
                                if !timeline.is_track_locked(&other.track_id)
                                    && ripple_tracks.insert(other.track_id.clone())
                                {
                                    expanded = true;
                                }
                            }
                        }
                    }
                }
            }

            let timeline_mut = project.active_mut();

            // Linkage follow main: if active timeline has linkage enabled and background lane ripples
            if timeline_mut.linkage {
                if let Some(main) = timeline_mut.background_lane() {
                    if ripple_tracks.contains(main) {
                        for track in &timeline_mut.tracks {
                            if !track.locked {
                                ripple_tracks.insert(track.id.clone());
                            }
                        }
                    }
                }
            }

            // Split straddling clips at tl_in on ripple_tracks
            let straddling: Vec<String> = timeline_mut
                .clips
                .iter()
                .filter(|c| {
                    ripple_tracks.contains(&c.track_id)
                        && c.start < tl_in - JOIN_EPSILON
                        && c.start + c.duration > tl_in + JOIN_EPSILON
                })
                .map(|c| c.id.clone())
                .collect();

            let mut tail_map: HashMap<String, String> = HashMap::new();

            for clip_id in straddling {
                let Some(idx) = timeline_mut.clips.iter().position(|c| c.id == clip_id) else {
                    continue;
                };
                let clip = &timeline_mut.clips[idx];
                let offset = tl_in - clip.start;
                if offset <= MIN_CLIP_DURATION || offset >= clip.duration - MIN_CLIP_DURATION {
                    continue;
                }
                let (head_source, mut tail_source) =
                    split_source(clip.source_start, clip.speed, offset);
                let whole = clip.duration;
                let mut tail = clip.as_ref().clone();
                if let Some(consumed) = crate::speed::rewindow_clip(&mut tail, whole, offset, whole)
                {
                    tail_source = clip.source_start + consumed;
                }
                let tail_id = mint.next("c");
                tail.id = tail_id.clone();
                tail.start = clip.start + offset;
                tail.duration = clip.duration - offset;
                tail.source_start = tail_source;
                tail.transition_in = None;
                tail.fade_in = 0.0;
                tail.rewindow_keys(whole, offset, whole);

                tail_map.insert(clip_id.clone(), tail_id);

                let head = timeline_mut.clip_at_mut(idx);
                crate::speed::rewindow_clip(head, whole, 0.0, offset);
                head.duration = offset;
                head.source_start = head_source;
                head.fade_out = 0.0;
                head.rewindow_keys(whole, 0.0, offset);

                timeline_mut.clips.insert(idx + 1, Arc::new(tail));
            }

            // Re-link split tail clips to preserve AV sync
            for clip in timeline_mut.clips.iter_mut() {
                if let Some(tail_parent_id) =
                    clip.detached_from.as_ref().and_then(|p| tail_map.get(p))
                {
                    if tail_map.values().any(|tid| tid == &clip.id) {
                        Arc::make_mut(clip).detached_from = Some(tail_parent_id.clone());
                    }
                }
            }

            // Ripple downstream clips
            for clip in timeline_mut.clips_where(|c| {
                ripple_tracks.contains(&c.track_id) && c.start >= tl_in - JOIN_EPSILON
            }) {
                clip.start += duration;
            }

            // Insert new clip(s)
            let mut created_id = None;
            let mut video_clip_id = None;

            if should_place_video {
                let vid = mint.next("c");
                let trk = target_video_track.unwrap();
                let mut v_clip = Clip::blank(
                    vid.clone(),
                    trk,
                    if media.kind == MediaKind::Image {
                        ClipKind::Image
                    } else {
                        ClipKind::Video
                    },
                    media.name.clone(),
                    tl_in,
                    duration,
                );
                v_clip.media_id = media.id.clone();
                v_clip.source_start = s_in;
                timeline_mut.clips.push(Arc::new(v_clip));
                video_clip_id = Some(vid.clone());
                created_id = Some(vid);
            }

            if should_place_audio {
                let aid = mint.next("c");
                let trk = effective_audio_track.unwrap();
                let mut a_clip = Clip::blank(
                    aid.clone(),
                    trk,
                    ClipKind::Audio,
                    media.name.clone(),
                    tl_in,
                    duration,
                );
                a_clip.media_id = media.id.clone();
                a_clip.source_start = s_in;
                if let Some(vid) = video_clip_id {
                    a_clip.detached_from = Some(vid);
                }
                timeline_mut.clips.push(Arc::new(a_clip));
                if created_id.is_none() {
                    created_id = Some(aid);
                }
            }

            Ok(Outcome {
                created_id,
                applied: true,
            })
        }

        Command::OverwriteEdit {
            media_id,
            source_in,
            source_out,
            timeline_in,
            timeline_out,
            video_track_id,
            audio_track_id,
        } => {
            let media = project
                .media_by_id(&media_id)
                .ok_or(CommandError::MediaGone)?
                .clone();
            let timeline = project.active();
            if timeline.tracks.is_empty() {
                return Err(CommandError::NoTracks);
            }

            // Determine target tracks
            let (target_video_track, target_audio_track) = match (video_track_id, audio_track_id) {
                (Some(v), Some(a)) => (Some(v), Some(a)),
                (Some(v), None) => (Some(v), None),
                (None, Some(a)) => (None, Some(a)),
                (None, None) => {
                    let unlocked: Vec<String> = timeline
                        .tracks
                        .iter()
                        .filter(|t| !t.locked)
                        .map(|t| t.id.clone())
                        .collect();
                    if unlocked.is_empty() {
                        return Err(CommandError::TrackLocked);
                    }
                    let v_track = Some(unlocked[0].clone());
                    let a_track = if (media.has_audio || media.kind == MediaKind::Audio)
                        && unlocked.len() > 1
                    {
                        Some(unlocked[1].clone())
                    } else if media.has_audio || media.kind == MediaKind::Audio {
                        Some(unlocked[0].clone())
                    } else {
                        None
                    };
                    (v_track, a_track)
                }
            };

            // Validate targeted tracks
            if let Some(ref vid) = target_video_track {
                if timeline.track(vid).is_none() {
                    return Err(CommandError::TrackGone);
                }
                if timeline.is_track_locked(vid) {
                    return Err(CommandError::TrackLocked);
                }
            }
            if let Some(ref aid) = target_audio_track {
                if timeline.track(aid).is_none() {
                    return Err(CommandError::TrackGone);
                }
                if timeline.is_track_locked(aid) {
                    return Err(CommandError::TrackLocked);
                }
            }

            let (s_in, _s_out, tl_in, tl_out) = resolve_three_point(
                &media,
                timeline,
                source_in,
                source_out,
                timeline_in,
                timeline_out,
            );
            let duration = tl_out - tl_in;
            if duration < MIN_CLIP_DURATION {
                return Ok(Outcome::default());
            }

            let should_place_video = media.kind != MediaKind::Audio && target_video_track.is_some();
            let should_place_audio = if media.kind == MediaKind::Audio {
                target_audio_track.is_some()
                    || (target_video_track.is_some() && !should_place_video)
            } else {
                media.has_audio && target_audio_track.is_some()
            };
            let effective_audio_track =
                if media.kind == MediaKind::Audio && target_audio_track.is_none() {
                    target_video_track.clone()
                } else {
                    target_audio_track.clone()
                };

            if !should_place_video && !should_place_audio {
                return Ok(Outcome::default());
            }

            let mut target_tracks = HashSet::new();
            if let Some(ref vid) = target_video_track {
                if should_place_video {
                    target_tracks.insert(vid.clone());
                }
            }
            if let Some(ref aid) = effective_audio_track {
                if should_place_audio {
                    target_tracks.insert(aid.clone());
                }
            }

            let ow_start = tl_in;
            let ow_end = tl_in + duration;
            let timeline_mut = project.active_mut();

            // 1. Remove fully enclosed clips on target_tracks
            timeline_mut.clips.retain(|c| {
                if !target_tracks.contains(&c.track_id) {
                    return true;
                }
                let is_enclosed = c.start >= ow_start - JOIN_EPSILON
                    && c.start + c.duration <= ow_end + JOIN_EPSILON;
                !is_enclosed
            });

            // 2. Trim or split partially overlapping clips on target_tracks
            let mut new_tails = Vec::new();
            for clip in timeline_mut.clips.iter_mut() {
                if !target_tracks.contains(&clip.track_id) {
                    continue;
                }
                let c_start = clip.start;
                let c_end = clip.start + clip.duration;

                // Outside range?
                if c_end <= ow_start + JOIN_EPSILON || c_start >= ow_end - JOIN_EPSILON {
                    continue;
                }

                // Hole punch (straddles both ends)
                if c_start < ow_start - JOIN_EPSILON && c_end > ow_end + JOIN_EPSILON {
                    let whole = clip.duration;
                    let head_dur = ow_start - c_start;
                    let tail_dur = c_end - ow_end;
                    let tail_offset = ow_end - c_start;

                    let mut retimed_tail = clip.as_ref().clone();
                    let consumed =
                        crate::speed::rewindow_clip(&mut retimed_tail, whole, tail_offset, whole)
                            .unwrap_or(tail_offset * clip.speed);
                    let mut tail = clip.as_ref().clone();
                    tail.id = mint.next("c");
                    tail.start = ow_end;
                    tail.duration = tail_dur;
                    tail.source_start = (clip.source_start + consumed).max(0.0);
                    tail.speed = retimed_tail.speed;
                    tail.speed_curve = retimed_tail.speed_curve;
                    tail.transition_in = None;
                    tail.fade_in = 0.0;
                    tail.rewindow_keys(whole, tail_offset, whole);
                    new_tails.push(Arc::new(tail));

                    let head = Arc::make_mut(clip);
                    crate::speed::rewindow_clip(head, whole, 0.0, head_dur);
                    head.duration = head_dur;
                    head.fade_out = 0.0;
                    head.rewindow_keys(whole, 0.0, head_dur);
                } else if c_start < ow_start - JOIN_EPSILON && c_end > ow_start + JOIN_EPSILON {
                    // Overlaps left boundary: truncate right end of clip
                    let whole = clip.duration;
                    let new_dur = ow_start - c_start;
                    let head = Arc::make_mut(clip);
                    crate::speed::rewindow_clip(head, whole, 0.0, new_dur);
                    head.duration = new_dur;
                    head.fade_out = 0.0;
                    head.rewindow_keys(whole, 0.0, new_dur);
                } else if c_start >= ow_start - JOIN_EPSILON
                    && c_start < ow_end - JOIN_EPSILON
                    && c_end > ow_end + JOIN_EPSILON
                {
                    // Overlaps right boundary: truncate left end of clip
                    let whole = clip.duration;
                    let cut_offset = ow_end - c_start;
                    let new_dur = c_end - ow_end;
                    let mut retimed = clip.as_ref().clone();
                    let consumed =
                        crate::speed::rewindow_clip(&mut retimed, whole, cut_offset, whole)
                            .unwrap_or(cut_offset * clip.speed);
                    let new_src = (clip.source_start + consumed).max(0.0);

                    let tail = Arc::make_mut(clip);
                    tail.start = ow_end;
                    tail.duration = new_dur;
                    tail.source_start = new_src;
                    tail.speed = retimed.speed;
                    tail.speed_curve = retimed.speed_curve;
                    tail.transition_in = None;
                    tail.fade_in = 0.0;
                    tail.rewindow_keys(whole, cut_offset, whole);
                }
            }

            timeline_mut.clips.extend(new_tails);

            // 3. Insert new overwrite clip(s)
            let mut created_id = None;
            let mut video_clip_id = None;

            if should_place_video {
                let vid = mint.next("c");
                let trk = target_video_track.unwrap();
                let mut v_clip = Clip::blank(
                    vid.clone(),
                    trk,
                    if media.kind == MediaKind::Image {
                        ClipKind::Image
                    } else {
                        ClipKind::Video
                    },
                    media.name.clone(),
                    ow_start,
                    duration,
                );
                v_clip.media_id = media.id.clone();
                v_clip.source_start = s_in;
                timeline_mut.clips.push(Arc::new(v_clip));
                video_clip_id = Some(vid.clone());
                created_id = Some(vid);
            }

            if should_place_audio {
                let aid = mint.next("c");
                let trk = effective_audio_track.unwrap();
                let mut a_clip = Clip::blank(
                    aid.clone(),
                    trk,
                    ClipKind::Audio,
                    media.name.clone(),
                    ow_start,
                    duration,
                );
                a_clip.media_id = media.id.clone();
                a_clip.source_start = s_in;
                if let Some(vid) = video_clip_id {
                    a_clip.detached_from = Some(vid);
                }
                timeline_mut.clips.push(Arc::new(a_clip));
                if created_id.is_none() {
                    created_id = Some(aid);
                }
            }

            Ok(Outcome {
                created_id,
                applied: true,
            })
        }

        Command::SplitClips { clip_ids, time } => {
            let timeline = project.active_mut();
            let mut created = None;
            for clip_id in clip_ids {
                let Some(index) = timeline.clips.iter().position(|clip| clip.id == clip_id) else {
                    continue;
                };
                let clip: &Clip = &timeline.clips[index];
                if timeline.is_track_locked(&clip.track_id) {
                    continue;
                }
                let offset = time - clip.start;
                if offset <= MIN_CLIP_DURATION || offset >= clip.duration - MIN_CLIP_DURATION {
                    continue;
                }
                let (head_source, mut tail_source) =
                    split_source(clip.source_start, clip.speed, offset);
                let whole = clip.duration;
                let mut tail = clip.clone();
                // Under a curve each half keeps its share of it, and the
                // tail starts where the curve had got to in the source.
                if let Some(consumed) = crate::speed::rewindow_clip(&mut tail, whole, offset, whole)
                {
                    tail_source = clip.source_start + consumed;
                }
                tail.id = mint.next("c");
                tail.start = clip.start + offset;
                tail.duration = clip.duration - offset;
                tail.source_start = tail_source;
                // The transition belongs to the cut at the original clip's
                // start, which the head keeps; the way in belongs to the
                // head and the way out to the tail, so neither piece plays
                // an entrance or an exit the whole did not have at the cut.
                tail.transition_in = None;
                tail.fade_in = 0.0;
                tail.rewindow_keys(whole, offset, whole);
                created = Some(tail.id.clone());
                let head = timeline.clip_at_mut(index);
                crate::speed::rewindow_clip(head, whole, 0.0, offset);
                head.duration = offset;
                head.source_start = head_source;
                head.fade_out = 0.0;
                head.rewindow_keys(whole, 0.0, offset);
                timeline.clips.insert(index + 1, Arc::new(tail));
            }
            // A split always mints the tail, so "minted anything" and
            // "changed anything" are the same fact here.
            let applied = created.is_some();
            Ok(Outcome {
                created_id: created,
                applied,
            })
        }

        Command::ReplaceClipMedia {
            clip_id,
            item,
            source_start,
            enhanced,
            offset,
        } => {
            if project.active().clip(&clip_id).is_none() {
                return Ok(Outcome::default());
            }
            // The copy's bin entry, or the one already there for its path:
            // two clips of one file enhanced in turn share one copy.
            let media_id = match project.media.iter().find(|media| media.path == item.path) {
                Some(existing) => existing.id.clone(),
                None => {
                    let id = mint.next("m");
                    project.media.push(MediaItem {
                        id: id.clone(),
                        path: item.path,
                        name: item.name,
                        duration: item.duration,
                        kind: item.kind,
                        width: item.width,
                        height: item.height,
                        frame_rate: item.frame_rate,
                        frame_rate_fraction: item.frame_rate_fraction,
                        video_codec: item.video_codec,
                        audio_codec: item.audio_codec,
                        has_audio: item.has_audio,
                        audio_tracks: item.audio_tracks,
                        origin: item.origin,
                        placeholder: false,
                        color_range: None,
                        color_space: item.color_space,
                        source_in: None,
                        source_out: None,
                        folder_id: None,
                        extra: Default::default(),
                    });
                    id
                }
            };
            let timeline = project.active_mut();
            let Some(index) = timeline.clips.iter().position(|clip| clip.id == clip_id) else {
                return Ok(Outcome::default());
            };
            let clip = timeline.clip_at_mut(index);
            let link = if enhanced {
                // The original is the one Enhance started from, kept through
                // a second run on a clip already showing a copy.
                let original = clip
                    .enhanced
                    .as_ref()
                    .map(|link| link.original.clone())
                    .unwrap_or_else(|| clip.media_id.clone());
                Some(Enhanced {
                    original,
                    copy: media_id.clone(),
                    on: true,
                    offset,
                })
            } else {
                None
            };
            // Bitwise so no assignment is short-circuited away.
            let applied = assign(&mut clip.media_id, media_id.clone())
                | source_start.is_some_and(|start| assign(&mut clip.source_start, start.max(0.0)))
                | assign(&mut clip.enhanced, link);
            if !applied {
                return Ok(Outcome::default());
            }
            Ok(Outcome {
                created_id: Some(media_id),
                applied: true,
            })
        }

        Command::SetClipEnhanced { clip_id, on } => {
            let timeline = project.active_mut();
            let Some(index) = timeline.clips.iter().position(|clip| clip.id == clip_id) else {
                return Ok(Outcome::default());
            };
            let Some(link) = timeline.clips[index].enhanced.clone() else {
                return Ok(Outcome::default());
            };
            let clip = timeline.clip_at_mut(index);
            let media = if on {
                link.copy.clone()
            } else {
                link.original.clone()
            };
            // The in-point moves with the media, from one's seconds to the
            // other's, so the frame on screen is the same frame.
            let start = match (link.on, on) {
                (false, true) => (clip.source_start - link.offset).max(0.0),
                (true, false) => clip.source_start + link.offset,
                _ => clip.source_start,
            };
            let applied = assign(&mut clip.media_id, media)
                | assign(&mut clip.source_start, start)
                | assign(&mut clip.enhanced, Some(Enhanced { on, ..link }));
            Ok(Outcome {
                created_id: None,
                applied,
            })
        }

        Command::FreezeFrame {
            clip_id,
            time,
            duration,
            still,
        } => {
            let hold = duration
                .filter(|value| *value > 0.0)
                .unwrap_or(DEFAULT_FREEZE_DURATION)
                .max(MIN_CLIP_DURATION);

            let (kind, media_id, track_id, start, clip_duration, speed, source_start, picture) = {
                let timeline = project.active();
                let Some(clip) = timeline.clip(&clip_id) else {
                    return Ok(Outcome::default());
                };
                if clip.kind != ClipKind::Video && clip.kind != ClipKind::Image {
                    return Ok(Outcome::default());
                }
                let offset = time - clip.start;
                if offset <= MIN_CLIP_DURATION || offset >= clip.duration - MIN_CLIP_DURATION {
                    return Ok(Outcome::default());
                }
                (
                    clip.kind,
                    clip.media_id.clone(),
                    clip.track_id.clone(),
                    clip.start,
                    clip.duration,
                    clip.speed,
                    clip.source_start,
                    clip.clone(),
                )
            };

            let freeze_media_id = if kind == ClipKind::Image && still.is_none() {
                media_id
            } else {
                let Some(item) = still else {
                    return Ok(Outcome::default());
                };
                if let Some(existing) = project.media.iter().find(|media| media.path == item.path) {
                    existing.id.clone()
                } else {
                    let id = mint.next("m");
                    project.media.push(MediaItem {
                        id: id.clone(),
                        path: item.path,
                        name: item.name,
                        duration: item.duration,
                        kind: MediaKind::Image,
                        width: item.width,
                        height: item.height,
                        frame_rate: item.frame_rate,
                        frame_rate_fraction: item.frame_rate_fraction,
                        video_codec: item.video_codec,
                        audio_codec: None,
                        has_audio: false,
                        audio_tracks: Vec::new(),
                        origin: item.origin,
                        placeholder: false,
                        color_range: None,
                        color_space: item.color_space,
                        source_in: None,
                        source_out: None,
                        folder_id: None,
                        extra: Default::default(),
                    });
                    id
                }
            };

            let timeline = project.active_mut();
            let main = timeline.background_lane().map(str::to_owned);
            let Some(index) = timeline.clips.iter().position(|clip| clip.id == clip_id) else {
                return Ok(Outcome::default());
            };
            // The cut is a split's, so it leaves the pieces as a split does:
            // under a curve each keeps its share of it.
            let offset = time - start;
            let (head_source, mut tail_source) = split_source(source_start, speed, offset);
            let mut tail = Clip::clone(&timeline.clips[index]);
            if let Some(consumed) =
                crate::speed::rewindow_clip(&mut tail, clip_duration, offset, clip_duration)
            {
                tail_source = source_start + consumed;
            }
            tail.id = mint.next("c");
            tail.start = time;
            tail.duration = clip_duration - offset;
            tail.source_start = tail_source;
            tail.transition_in = None;
            tail.fade_in = 0.0;
            tail.rewindow_keys(clip_duration, offset, clip_duration);
            let head = timeline.clip_at_mut(index);
            crate::speed::rewindow_clip(head, clip_duration, 0.0, offset);
            head.duration = offset;
            head.source_start = head_source;
            head.fade_out = 0.0;
            head.rewindow_keys(clip_duration, 0.0, offset);
            timeline.clips.insert(index + 1, Arc::new(tail));

            // Ripple every later placement on this track (including the new
            // tail) so the freeze does not sit on top of the remainder.
            for clip in timeline.clips_where(|clip| clip.track_id == track_id && clip.start >= time)
            {
                clip.start += hold;
            }
            follow_main(
                timeline,
                main.as_deref(),
                &track_id,
                Ripple::Inserted {
                    at: time,
                    length: hold,
                },
            );

            // The still is the source clip turned into a picture: cloning it
            // first carries every look field - transform, effects, crop,
            // flips, whatever the model grows - and then the hold's own
            // facts overwrite the moving ones.
            let freeze_id = mint.next("c");
            let mut frozen = picture;
            frozen.id = freeze_id.clone();
            frozen.track_id = track_id;
            frozen.kind = ClipKind::Image;
            frozen.media_id = freeze_media_id;
            frozen.start = time;
            frozen.duration = hold;
            frozen.source_start = 0.0;
            frozen.speed = 1.0;
            frozen.speed_curve = None;
            frozen.volume = 1.0;
            frozen.fade_in = 0.0;
            frozen.fade_out = 0.0;
            frozen.filters = Vec::new();
            frozen.muted = None;
            frozen.detached_from = None;
            frozen.transition_in = None;
            frozen.text = None;
            timeline.clips.push(Arc::new(frozen));

            Ok(Outcome {
                created_id: Some(freeze_id),
                applied: true,
            })
        }

        Command::MergeClips { clip_ids } => {
            let timeline = project.active_mut();
            for id in &clip_ids {
                if timeline
                    .clip(id)
                    .is_some_and(|clip| timeline.is_track_locked(&clip.track_id))
                {
                    return Err(CommandError::TrackLocked);
                }
            }
            if let Some(reason) = why_not_merge(timeline, &clip_ids) {
                return Err(CommandError::CannotMerge { reason });
            }
            let mut ordered: Vec<Clip> = clip_ids
                .iter()
                .filter_map(|id| timeline.clip(id).cloned())
                .collect();
            ordered.sort_by(|left, right| left.start.total_cmp(&right.start));
            let first = ordered.first().expect("validated above").clone();
            let last = ordered.last().expect("validated above");
            let merged_duration = last.start + last.duration - first.start;

            // A set, not a Vec: the retain below tests every clip on the
            // timeline against it.
            let doomed: HashSet<String> =
                ordered.iter().skip(1).map(|clip| clip.id.clone()).collect();
            timeline.clips.retain(|clip| !doomed.contains(&clip.id));
            let survivor = timeline
                .clip_mut(&first.id)
                .expect("the first piece survives the retain");
            survivor.duration = merged_duration;
            // Every piece's keys land where they were on the picture; the
            // way out is the last piece's, as the way in is the first's.
            survivor.rewindow_keys(first.duration, 0.0, merged_duration);
            for piece in ordered.iter().skip(1) {
                survivor.absorb_keys(piece, piece.start - first.start);
            }
            survivor.fade_out = last.fade_out;
            // A validated merge always absorbs at least one piece.
            Ok(Outcome {
                created_id: Some(first.id),
                applied: true,
            })
        }

        Command::ConsolidateClips { clip_ids, item } => {
            let clips = consolidation_clips(project.active(), &clip_ids)?;
            let first = &clips[0];
            let end = clips
                .iter()
                .map(|clip| clip.start + clip.duration)
                .fold(0.0, f64::max);
            let duration = end - first.start;
            if item.kind != MediaKind::Video
                || item.path.is_empty()
                || item.duration.is_none_or(|length| length + 1e-6 < duration)
            {
                return Err(CommandError::CannotMerge {
                    reason: "The merged video does not cover the selected clips.".into(),
                });
            }
            let path = item.path.clone();
            super::media::apply(project, mint, Command::AddMedia { item })?;
            let media = project
                .media
                .iter()
                .find(|media| super::media::same_path(&media.path, &path))
                .expect("imported above");
            let mut merged =
                default_clip(first.id.clone(), first.track_id.clone(), media, first.start);
            merged.duration = duration;
            let timeline = project.active_mut();
            let selected: HashSet<_> = clip_ids.iter().collect();
            timeline.clips.retain(|clip| !selected.contains(&clip.id));
            timeline.clips.push(Arc::new(merged));
            Ok(Outcome {
                created_id: Some(first.id.clone()),
                applied: true,
            })
        }

        Command::CreateCompoundClip { clip_ids, name } => {
            if clip_ids.is_empty() {
                return Err(CommandError::CannotMerge {
                    reason: "Select one or more clips to create a compound clip.".into(),
                });
            }
            let active_tl_id = project.active_timeline_id.clone();
            let timeline = project.active();
            let mut selected_clips = Vec::with_capacity(clip_ids.len());
            for id in &clip_ids {
                let Some(clip) = timeline.clip(id) else {
                    return Err(CommandError::CannotMerge {
                        reason: "Selected clip no longer exists.".into(),
                    });
                };
                if timeline.is_track_locked(&clip.track_id) {
                    return Err(CommandError::TrackLocked);
                }
                selected_clips.push(clip.clone());
            }

            let min_start = selected_clips
                .iter()
                .map(|c| c.start)
                .fold(f64::INFINITY, f64::min);
            let max_end = selected_clips
                .iter()
                .map(|c| c.start + c.duration)
                .fold(0.0, f64::max);
            let duration = (max_end - min_start).max(MIN_CLIP_DURATION);

            selected_clips.sort_by(|a, b| a.start.total_cmp(&b.start));
            let primary_track_id = selected_clips[0].track_id.clone();

            let child_tl_id = mint.next("tl");
            let child_name = name.unwrap_or_else(|| format!("Compound Clip {}", child_tl_id));

            if project.would_create_cycle(&active_tl_id, &child_tl_id) {
                return Err(CommandError::CircularReference);
            }

            let mut child_timeline = Timeline {
                id: child_tl_id.clone(),
                name: child_name.clone(),
                video: timeline.video,
                background: Background::None,
                tracks: timeline.tracks.clone(),
                clips: Vec::with_capacity(selected_clips.len()),
                linkage: timeline.linkage,
                markers: Vec::new(),
                mark_in: None,
                mark_out: None,
                extra: serde_json::Map::new(),
            };

            for clip in &selected_clips {
                let mut child_clip = clip.clone();
                child_clip.start = (clip.start - min_start).max(0.0);
                child_timeline.clips.push(Arc::new(child_clip));
            }

            let compound_clip_id = mint.next("c");
            let mut compound_clip = Clip::blank(
                compound_clip_id.clone(),
                primary_track_id,
                ClipKind::Sequence,
                child_name,
                min_start,
                duration,
            );
            compound_clip.media_id = child_tl_id.clone();

            let selected_ids: HashSet<&str> = clip_ids.iter().map(String::as_str).collect();
            let active = project.active_mut();
            active
                .clips
                .retain(|c| !selected_ids.contains(c.id.as_str()));
            active.clips.push(Arc::new(compound_clip));

            project.timelines.push(Arc::new(child_timeline));

            Ok(Outcome {
                created_id: Some(compound_clip_id),
                applied: true,
            })
        }

        Command::DecomposeCompoundClip { clip_id } => {
            let active = project.active();
            let Some(compound_clip) = active.clip(&clip_id).cloned() else {
                return Err(CommandError::CannotMerge {
                    reason: "Clip no longer exists.".into(),
                });
            };
            if compound_clip.kind != ClipKind::Sequence {
                return Err(CommandError::CannotMerge {
                    reason: "Clip is not a compound clip.".into(),
                });
            }
            if active.is_track_locked(&compound_clip.track_id) {
                return Err(CommandError::TrackLocked);
            }
            let Some(child_timeline) = project.timeline_by_id(&compound_clip.media_id).cloned()
            else {
                return Err(CommandError::CannotMerge {
                    reason: "Nested sequence no longer exists.".into(),
                });
            };

            let window_start = compound_clip.source_start;
            let window_duration = compound_clip.duration * compound_clip.speed;
            let window_end = window_start + window_duration;

            let mut extracted_clips = Vec::new();
            for child in &child_timeline.clips {
                let child_end = child.start + child.duration;
                if child_end <= window_start || child.start >= window_end {
                    continue;
                }
                let overlap_start = child.start.max(window_start);
                let overlap_end = child_end.min(window_end);
                let overlap_dur = (overlap_end - overlap_start).max(MIN_CLIP_DURATION);
                let start_trim = overlap_start - child.start;

                let mut restored = (**child).clone();
                restored.id = mint.next("c");
                restored.source_start += start_trim * child.speed;
                restored.start =
                    compound_clip.start + (overlap_start - window_start) / compound_clip.speed;
                restored.duration = overlap_dur / compound_clip.speed;
                restored.speed *= compound_clip.speed;
                restored.opacity *= compound_clip.opacity;
                if compound_clip.volume != 1.0 {
                    restored.volume *= compound_clip.volume;
                }
                if compound_clip.muted == Some(true) {
                    restored.muted = Some(true);
                }
                if active.track(&restored.track_id).is_none() {
                    restored.track_id = compound_clip.track_id.clone();
                }
                extracted_clips.push(Arc::new(restored));
            }

            let active_mut = project.active_mut();
            active_mut.clips.retain(|c| c.id != clip_id);
            active_mut.clips.extend(extracted_clips);

            Ok(Outcome {
                created_id: None,
                applied: true,
            })
        }

        Command::RemoveClips { clip_ids, ripple } => {
            let timeline = project.active_mut();
            let doomed: HashSet<&str> = clip_ids
                .iter()
                .filter(|id| {
                    timeline
                        .clip(id)
                        .map(|c| !timeline.is_track_locked(&c.track_id))
                        .unwrap_or(false)
                })
                .map(String::as_str)
                .collect();
            // The spans going, per track, taken before they are gone: the
            // ripple closes exactly these.
            let removed: Vec<(String, f64, f64)> = timeline
                .clips
                .iter()
                .filter(|clip| doomed.contains(clip.id.as_str()))
                .map(|clip| {
                    (
                        clip.track_id.clone(),
                        clip.start,
                        clip.start + clip.duration,
                    )
                })
                .collect();
            // The main lane before it loses anything: deleting its last
            // picture is still an edit of it.
            let main = timeline.background_lane().map(str::to_owned);
            let clip_count = timeline.clips.len();
            timeline
                .clips
                .retain(|clip| !doomed.contains(clip.id.as_str()));
            let applied = timeline.clips.len() != clip_count;
            if ripple && applied {
                close_gaps(timeline, &removed);
                if let Some(main) = main.as_deref() {
                    let spans = removed
                        .iter()
                        .filter(|(track, _, _)| track == main)
                        .map(|(_, start, end)| (*start, *end))
                        .collect();
                    follow_main(timeline, Some(main), main, Ripple::Removed(spans));
                }
            }
            Ok(Outcome {
                created_id: None,
                applied,
            })
        }

        _ => unreachable!("commands::apply routes only this module's commands here"),
    }
}

/// How far apart two clips may sit and still count as touching, in seconds.
const JOIN_EPSILON: f64 = 1e-6;

/// The ripple of a delete: pulls every clip that sits after a removed
/// span left by the length of the removed spans before it, on that clip's
/// own track. Spans that overlap count once - a clip behind two doomed
/// clips that shared five seconds moves by their union, not their sum, or
/// it would land in front of what was in front of it. A span reaching past
/// the clip's start counts only up to it, and nothing is pulled before
/// zero.
/// https://github.com/jub0t/Concat/issues/106
fn close_gaps(timeline: &mut Timeline, removed: &[(String, f64, f64)]) {
    let locked_tracks: HashSet<String> = timeline
        .tracks
        .iter()
        .filter(|t| t.locked)
        .map(|t| t.id.clone())
        .collect();
    let behind_a_span = |clip: &Clip| {
        !locked_tracks.contains(&clip.track_id)
            && removed
                .iter()
                .any(|(track, start, _)| *track == clip.track_id && *start < clip.start)
    };
    for clip in timeline.clips_where(behind_a_span) {
        let spans: Vec<(f64, f64)> = removed
            .iter()
            .filter(|(track, _, _)| *track == clip.track_id)
            .map(|(_, start, end)| (*start, *end))
            .collect();
        clip.start = (clip.start - removed_before(&spans, clip.start)).max(0.0);
    }
}

/// How much of `spans` lies before `at`, counting what they share once: a
/// span reaching past `at` counts only up to it.
fn removed_before(spans: &[(f64, f64)], at: f64) -> f64 {
    let mut before: Vec<(f64, f64)> = spans
        .iter()
        .filter(|(start, _)| *start < at)
        .map(|&(start, end)| (start, end.min(at)))
        .collect();
    if before.is_empty() {
        return 0.0;
    }
    before.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut gap = 0.0;
    let (mut from, mut to) = before[0];
    for (start, end) in before.into_iter().skip(1) {
        if start <= to {
            to = to.max(end);
        } else {
            gap += to - from;
            from = start;
            to = end;
        }
    }
    gap + (to - from)
}

/// What an edit did to the main lane's time, counted in the lane as it was
/// before: spans taken out of it, or a length put in at a time.
pub(crate) enum Ripple {
    Removed(Vec<(f64, f64)>),
    Inserted { at: f64, length: f64 },
}

/// Linkage: the rest of the timeline follows a ripple on its main lane, so
/// what was placed against the footage stays on the frames it was placed
/// against. A clip on another lane that starts at or after an insertion
/// moves along by its length, and one after a removal moves back by what
/// was taken out before it - landing where the cut is when it started
/// inside what went. A clip that runs across the edit stays: a music bed
/// is not cut up or dragged. Words wholly inside footage that went - a
/// caption, a title - go with it, as they would have been spoken over
/// nothing. `main` is the timeline's main lane as it stood before the
/// edit; a ripple on any other lane, or with linkage off, moves only its
/// own lane, as it always did.
pub(crate) fn follow_main(timeline: &mut Timeline, main: Option<&str>, lane: &str, ripple: Ripple) {
    if !timeline.linkage || main != Some(lane) {
        return;
    }
    let locked_tracks: HashSet<String> = timeline
        .tracks
        .iter()
        .filter(|t| t.locked)
        .map(|t| t.id.clone())
        .collect();
    match ripple {
        Ripple::Inserted { at, length } => {
            if length <= 0.0 {
                return;
            }
            for clip in timeline.clips_where(|clip| {
                clip.track_id != lane
                    && !locked_tracks.contains(&clip.track_id)
                    && clip.start >= at - JOIN_EPSILON
            }) {
                clip.start += length;
            }
        }
        Ripple::Removed(spans) => {
            let spans: Vec<(f64, f64)> = spans.into_iter().filter(|(s, e)| e > s).collect();
            if spans.is_empty() {
                return;
            }
            let spoken_over_nothing = |clip: &Clip| {
                clip.track_id != lane
                    && !locked_tracks.contains(&clip.track_id)
                    && clip.kind == ClipKind::Text
                    && spans.iter().any(|&(start, end)| {
                        clip.start >= start - JOIN_EPSILON
                            && clip.start + clip.duration <= end + JOIN_EPSILON
                    })
            };
            timeline.clips.retain(|clip| !spoken_over_nothing(clip));
            for clip in timeline.clips_where(|clip| {
                clip.track_id != lane
                    && !locked_tracks.contains(&clip.track_id)
                    && spans.iter().any(|&(start, _)| start < clip.start)
            }) {
                clip.start = (clip.start - removed_before(&spans, clip.start)).max(0.0);
            }
        }
    }
}

fn default_clip(id: String, track_id: String, media: &MediaItem, start: f64) -> Clip {
    let kind = match media.kind {
        MediaKind::Video => ClipKind::Video,
        MediaKind::Audio => ClipKind::Audio,
        MediaKind::Image => ClipKind::Image,
    };
    let mut clip = Clip::blank(
        id,
        track_id,
        kind,
        media.name.clone(),
        start,
        clip_length(media),
    );
    clip.media_id = media.id.clone();
    clip
}

/// Makes room on `track_id` for a new clip at `start`: a drop onto the
/// middle of a clip lands at that clip's end instead, and every clip at
/// or after the place it lands moves right by the new clip's length, so
/// the new clip slots in and nothing is covered (#129). Returns where the
/// new clip lands. A drop with room to spare changes nothing.
fn ripple_room_for(timeline: &mut Timeline, track_id: &str, start: f64, media: &MediaItem) -> f64 {
    let duration = clip_length(media);
    // Dropped onto a clip: after it, rather than over it or through it.
    let start = timeline
        .clips
        .iter()
        .filter(|clip| {
            clip.track_id == track_id && clip.start < start && start < clip.start + clip.duration
        })
        .map(|clip| clip.start + clip.duration)
        .fold(start, f64::max);
    let end = start + duration;
    let overlaps = timeline.clips.iter().any(|clip| {
        clip.track_id == track_id && clip.start < end && start < clip.start + clip.duration
    });
    if !overlaps {
        return start;
    }
    let main = timeline.background_lane().map(str::to_owned);
    for clip in timeline.clips_where(|clip| clip.track_id == track_id && clip.start >= start) {
        clip.start += duration;
    }
    follow_main(
        timeline,
        main.as_deref(),
        track_id,
        Ripple::Inserted {
            at: start,
            length: duration,
        },
    );
    start
}

/// How long a new clip of `media` is: a still's default hold, or the
/// file's own length.
fn clip_length(media: &MediaItem) -> f64 {
    match media.kind {
        MediaKind::Image => DEFAULT_IMAGE_DURATION,
        _ => media.duration.unwrap_or(UNKNOWN_DURATION),
    }
}

/// Resolves standard 3-point editing combinations into effective (s_in, s_out, tl_in, tl_out).
fn resolve_three_point(
    media: &MediaItem,
    timeline: &Timeline,
    source_in: Option<f64>,
    source_out: Option<f64>,
    timeline_in: Option<f64>,
    timeline_out: Option<f64>,
) -> (f64, f64, f64, f64) {
    let media_dur = clip_length(media);
    let min_dur = MIN_CLIP_DURATION;

    let s_in_cand = source_in.or(media.source_in);
    let s_out_cand = source_out.or(media.source_out);
    let tl_in_cand = timeline_in.or(timeline.mark_in);
    let tl_out_cand = timeline_out.or(timeline.mark_out);

    if let (Some(si), Some(so)) = (s_in_cand, s_out_cand) {
        let si = si.clamp(0.0, media_dur);
        let so = so.clamp(si, media_dur);
        let d = (so - si).max(min_dur);
        let so = (si + d).min(media_dur);
        let d = so - si;
        if let Some(to) = tl_out_cand
            && tl_in_cand.is_none()
        {
            let to = to.max(d);
            let ti = (to - d).max(0.0);
            (si, so, ti, to)
        } else {
            let ti = tl_in_cand.unwrap_or(0.0).max(0.0);
            let to = ti + d;
            (si, so, ti, to)
        }
    } else if let (Some(ti), Some(to)) = (tl_in_cand, tl_out_cand) {
        let ti = ti.max(0.0);
        let to = to.max(ti + min_dur);
        let d = to - ti;
        if let Some(so) = s_out_cand
            && s_in_cand.is_none()
        {
            let so = so.clamp(0.0, media_dur);
            let si = (so - d).max(0.0);
            (si, so, ti, to)
        } else {
            let si = s_in_cand.unwrap_or(0.0).clamp(0.0, media_dur);
            let so = (si + d).min(media_dur);
            (si, so, ti, to)
        }
    } else if let Some(si) = s_in_cand {
        let si = si.clamp(0.0, media_dur);
        let d = (media_dur - si).max(min_dur);
        let so = si + d;
        let ti = tl_in_cand.unwrap_or(0.0).max(0.0);
        let to = ti + d;
        (si, so, ti, to)
    } else if let Some(so) = s_out_cand {
        let so = so.clamp(0.0, media_dur);
        let d = so.max(min_dur);
        let si = (so - d).max(0.0);
        let ti = tl_in_cand.unwrap_or(0.0).max(0.0);
        let to = ti + d;
        (si, so, ti, to)
    } else {
        let si = 0.0;
        let so = media_dur;
        let d = (so - si).max(min_dur);
        let ti = tl_in_cand.unwrap_or(0.0).max(0.0);
        let to = ti + d;
        (si, so, ti, to)
    }
}

/// The lowest track with nothing occupying `[start, start + duration)`.
/// `None` when every lane is taken there: the caller opens a new one
/// rather than covering a clip.
fn first_free_track(timeline: &Timeline, start: f64, duration: f64) -> Option<String> {
    timeline
        .tracks
        .iter()
        .filter(|track| !track.locked)
        .find(|track| timeline.is_free(&track.id, start, duration, &[]))
        .map(|track| track.id.clone())
}

/// A new, empty lane at the top, for a clip with nowhere else to go.
fn fresh_track(timeline: &mut Timeline, mint: &mut IdMint) -> String {
    let id = mint.next("t");
    timeline.tracks.push(Track {
        id: id.clone(),
        visible: true,
        muted: false,
        locked: false,
        solo: false,
        extra: Default::default(),
    });
    id
}

/// First free lane *above* the highest one occupied over `[start, start +
/// duration)`. `None` when every lane above is taken, which is the caller's
/// cue to mint a new one at the top.
///
/// Captions and titles go through this so they sit over the video, not
/// under it. The plain `first_free_track` still walks from the bottom,
/// which is what a sound or a picture wants.
fn first_free_track_above(timeline: &Timeline, start: f64, duration: f64) -> Option<String> {
    let end = start + duration;
    let occupied = |track_id: &str| {
        timeline.clips.iter().any(|clip| {
            clip.track_id == track_id && clip.start < end && start < clip.start + clip.duration
        })
    };
    let floor = timeline
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| occupied(&track.id))
        .map(|(row, _)| row + 1)
        .max()
        .unwrap_or(0);
    timeline
        .tracks
        .iter()
        .skip(floor)
        .filter(|track| !track.locked)
        .find(|track| !occupied(&track.id))
        .map(|track| track.id.clone())
}

/// Which side of the playhead [`delete_to_playhead`] takes away.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlayheadSide {
    /// From the clip's start up to the playhead: Delete Left (Q).
    Left,
    /// From the playhead to the clip's end: Delete Right (W).
    Right,
}

/// Delete Left / Delete Right: each named clip that `at` runs through loses
/// what lies on `side` of it, as trims of the edge on that side brought to
/// `at` - so a head cut keeps the frames after the playhead where they
/// were in the source. More than one clip is one [`Command::Batch`], one
/// undo step. `ripple` closes each lane behind the cut, as a trim on the
/// magnetic timeline does.
///
/// A clip the instant misses, or grazes within [`MIN_CLIP_DURATION`] of an
/// end, is skipped, the way [`Command::SplitClips`] skips it: there is
/// nothing to take. So is a second clip on a lane that already has one:
/// a magnetic cut moves what comes after it, and the second clip's cut
/// was measured before that move. None when nothing is left to cut.
pub fn delete_to_playhead(
    timeline: &Timeline,
    clip_ids: &[String],
    at: f64,
    side: PlayheadSide,
    ripple: bool,
) -> Option<Command> {
    let mut lanes = HashSet::new();
    let mut trims: Vec<Command> = clip_ids
        .iter()
        .filter_map(|id| timeline.clip(id))
        .filter(|clip| {
            clip.start + MIN_CLIP_DURATION < at
                && at < clip.start + clip.duration - MIN_CLIP_DURATION
        })
        .filter(|clip| lanes.insert(clip.track_id.clone()))
        .map(|clip| {
            let (edge, delta) = match side {
                PlayheadSide::Left => (TrimEdge::Start, at - clip.start),
                PlayheadSide::Right => (TrimEdge::End, at - (clip.start + clip.duration)),
            };
            Command::TrimClip {
                clip_id: clip.id.clone(),
                edge,
                delta,
                ripple,
            }
        })
        .collect();
    match trims.len() {
        0 => None,
        1 => trims.pop(),
        _ => Some(Command::Batch { commands: trims }),
    }
}

/// Where each piece of a clip cut at `offset` begins in the source: the
/// head keeps its in-point and the tail starts `offset × speed` later, so
/// the two pieces together show exactly what the whole did.
fn split_source(source_start: f64, speed: f64, offset: f64) -> (f64, f64) {
    (source_start, source_start + offset * speed)
}

/// Why these clips cannot be merged, or None if they can. A sentence, because
/// a disabled button that will not say why is worse than no button.
pub fn why_not_merge(timeline: &Timeline, clip_ids: &[String]) -> Option<String> {
    if clip_ids.len() < 2 {
        return Some("Select two or more clips to merge.".to_owned());
    }
    let clips: Vec<&Clip> = clip_ids.iter().filter_map(|id| timeline.clip(id)).collect();
    if clips.len() < 2 {
        return Some("Select two or more clips to merge.".to_owned());
    }
    if clips.iter().any(|clip| clip.track_id != clips[0].track_id) {
        return Some("Merged clips must be on the same track.".to_owned());
    }
    if clips.iter().any(|clip| clip.media_id != clips[0].media_id) {
        return Some("Merged clips must come from the same file.".to_owned());
    }
    if clips.iter().any(|clip| clip.speed != clips[0].speed) {
        return Some("Merged clips must play at the same speed.".to_owned());
    }
    if clips.iter().any(|clip| clip.kind != clips[0].kind) {
        return Some("Merged clips must be the same kind.".to_owned());
    }
    if clips.iter().any(|clip| clip.speed_curve.is_some()) {
        return Some("A clip with a speed curve cannot be merged.".to_owned());
    }
    if clips
        .iter()
        .any(|clip| clip.audio_stream != clips[0].audio_stream)
    {
        return Some("Merged clips must play the same audio track.".to_owned());
    }

    let mut ordered = clips.clone();
    ordered.sort_by(|left, right| left.start.total_cmp(&right.start));
    for pair in ordered.windows(2) {
        let (previous, current) = (pair[0], pair[1]);
        if (current.start - (previous.start + previous.duration)).abs() > JOIN_EPSILON {
            return Some("Merged clips must touch, with no gap or overlap.".to_owned());
        }
        // The next piece starts where the last one's source ended.
        let continuous =
            current.source_start - (previous.source_start + previous.duration * previous.speed);
        if continuous.abs() > JOIN_EPSILON {
            return Some("These pieces are no longer in their original order.".to_owned());
        }
    }
    None
}

/// Validates a selection for rendering into one clip, in timeline order.
/// Gaps remain in the render. Unselected clips in its span are refused so
/// the replacement cannot cover them. Source order and file need not match.
pub fn consolidation_clips(
    timeline: &Timeline,
    clip_ids: &[String],
) -> Result<Vec<Clip>, CommandError> {
    let refuse = |reason: &str| CommandError::CannotMerge {
        reason: reason.into(),
    };
    let selected: HashSet<_> = clip_ids.iter().collect();
    if selected.len() < 2 || selected.len() != clip_ids.len() {
        return Err(refuse("Select two or more distinct video clips to merge."));
    }
    let mut clips: Vec<Clip> = clip_ids
        .iter()
        .map(|id| {
            timeline
                .clip(id)
                .cloned()
                .ok_or_else(|| refuse("A selected clip no longer exists."))
        })
        .collect::<Result<_, _>>()?;
    clips.sort_by(|a, b| a.start.total_cmp(&b.start));
    let first = &clips[0];
    if clips
        .iter()
        .any(|clip| clip.kind != ClipKind::Video || clip.track_id != first.track_id)
    {
        return Err(refuse("Select video clips on the same track to merge."));
    }
    if timeline.track(&first.track_id).is_none() {
        return Err(CommandError::TrackGone);
    }
    if clips
        .windows(2)
        .any(|pair| pair[1].start < pair[0].start + pair[0].duration - 1e-6)
    {
        return Err(refuse("Overlapping clips cannot be merged."));
    }
    if timeline.clips.iter().any(|clip| {
        clip.detached_from
            .as_ref()
            .is_some_and(|id| selected.contains(id))
    }) {
        return Err(refuse(
            "Reattach detached audio before merging its video clips.",
        ));
    }
    let end = clips
        .iter()
        .map(|clip| clip.start + clip.duration)
        .fold(0.0, f64::max);
    if timeline.clips.iter().any(|clip| {
        clip.track_id == first.track_id
            && !selected.contains(&clip.id)
            && clip.start < end - 1e-6
            && clip.start + clip.duration > first.start + 1e-6
    }) {
        return Err(refuse("Select every clip inside the span to merge."));
    }
    // Rounding several subframe spans is not always equivalent to rounding
    // their union. Refuse a span that one ordinary clip could truncate, or
    // whose pieces overlap after quantisation, instead of losing a frame.
    let fps = timeline.video.rate();
    let frame = |seconds: f64| (seconds * fps).round();
    let rendered_end = clips
        .iter()
        .map(|clip| frame(clip.start) + frame(clip.duration))
        .fold(0.0, f64::max);
    if rendered_end != frame(first.start) + frame(end - first.start)
        || clips
            .windows(2)
            .any(|pair| frame(pair[1].start) < frame(pair[0].start) + frame(pair[0].duration))
    {
        return Err(refuse(
            "Align clip boundaries to the timeline frame grid before merging.",
        ));
    }
    // MP4 bakes against black and cannot carry the selection's alpha.
    // Refuse a bake that would cover an existing lower-track composite.
    let lane = timeline
        .tracks
        .iter()
        .position(|track| track.id == first.track_id)
        .expect("checked above");
    if timeline.clips.iter().any(|clip| {
        clip.kind != ClipKind::Audio
            && clip.start < end
            && clip.start + clip.duration > first.start
            && timeline
                .tracks
                .iter()
                .take(lane)
                .any(|track| track.id == clip.track_id && track.visible)
    }) {
        return Err(refuse(
            "Clips above other visible content cannot be merged into an opaque video.",
        ));
    }
    // An edge transition needs footage outside the selected span. Internal
    // transitions are rendered normally; never silently bake only half one.
    if first.transition_in.is_some()
        || timeline.clips.iter().any(|clip| {
            clip.track_id == first.track_id
                && !selected.contains(&clip.id)
                && (clip.start - end).abs() < 1e-6
                && clip.transition_in.is_some()
        })
    {
        return Err(refuse("Include both sides of a transition in the merge."));
    }
    Ok(clips)
}
