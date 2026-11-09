// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Sound Effects Library with MyInstants meme sound integration.

use serde::{Deserialize, Serialize};

/// A sound effect item from the meme library / MyInstants.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemeSound {
    /// Identifier (e.g. "vine-boom" or "sound-12345").
    pub id: String,
    /// Human-readable title of the sound effect.
    pub name: String,
    /// Absolute or direct URL to the mp3 audio file.
    pub mp3_url: String,
    /// Estimated or probed duration in seconds (defaults to 1.5).
    #[serde(default = "default_duration")]
    pub duration: f64,
    /// Hex color code for the sound button / card visual tag.
    #[serde(default = "default_color")]
    pub color: String,
}

fn default_duration() -> f64 {
    1.5
}

fn default_color() -> String {
    "#00E5FF".to_string()
}

/// Persistent state for user meme sound favorites and history.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemeLibraryState {
    /// Sound IDs marked as favorites.
    #[serde(default)]
    pub favorites: Vec<String>,
    /// Sound IDs recently previewed or inserted, newest first (capped at 50).
    #[serde(default)]
    pub history: Vec<String>,
}

impl MemeLibraryState {
    /// Constructs a clean state with empty favorites and history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if a sound is marked as favorite.
    pub fn is_favorite(&self, id: &str) -> bool {
        self.favorites.iter().any(|fav| fav == id)
    }

    /// Toggles favorite status of a sound ID. Returns the new favorite state.
    pub fn toggle_favorite(&mut self, id: &str) -> bool {
        if let Some(pos) = self.favorites.iter().position(|fav| fav == id) {
            self.favorites.remove(pos);
            false
        } else {
            self.favorites.push(id.to_string());
            true
        }
    }

    /// Records a sound as recently used (inserted into timeline or previewed).
    pub fn record_used(&mut self, id: &str) {
        self.history.retain(|h| h != id);
        self.history.insert(0, id.to_string());
        if self.history.len() > 50 {
            self.history.truncate(50);
        }
    }
}

/// Builds the search query URL for MyInstants.
pub fn myinstants_search_url(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("meme") {
        "https://www.myinstants.com/en/search/?name=meme".to_string()
    } else {
        let mut encoded = String::new();
        for b in trimmed.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    encoded.push(b as char);
                }
                b' ' => encoded.push('+'),
                _ => encoded.push_str(&format!("%{:02X}", b)),
            }
        }
        format!("https://www.myinstants.com/en/search/?name={encoded}")
    }
}

/// Parses HTML response from MyInstants search and extracts sound items.
pub fn parse_myinstants_html(html: &str) -> Vec<MemeSound> {
    let mut sounds = Vec::new();

    let mut cursor = 0;
    while let Some(btn_idx) = html[cursor..].find("<button class=\"small-button\"") {
        let abs_btn = cursor + btn_idx;
        let end_tag = match html[abs_btn..].find('>') {
            Some(pos) => abs_btn + pos,
            None => break,
        };
        let tag_slice = &html[abs_btn..=end_tag];

        if let Some(play_idx) = tag_slice.find("play(") {
            let args_slice = &tag_slice[play_idx + 5..];
            if let Some(close_paren) = args_slice.find(')') {
                let raw_args = &args_slice[..close_paren];
                let parts: Vec<&str> = raw_args
                    .split(',')
                    .map(|s| s.trim().trim_matches('\'').trim_matches('"'))
                    .collect();

                if let Some(&first) = parts.first() {
                    if !first.is_empty() {
                        let mp3_path = first;
                        let mp3_url = if mp3_path.starts_with("http://")
                            || mp3_path.starts_with("https://")
                        {
                            mp3_path.to_string()
                        } else if mp3_path.starts_with('/') {
                            format!("https://www.myinstants.com{mp3_path}")
                        } else {
                            format!("https://www.myinstants.com/{mp3_path}")
                        };

                        let sound_id = if parts.len() >= 3 && !parts[2].is_empty() {
                            parts[2].to_string()
                        } else {
                            mp3_path
                                .rsplit('/')
                                .next()
                                .unwrap_or("sound")
                                .trim_end_matches(".mp3")
                                .to_string()
                        };

                        let title = if let Some(title_idx) = tag_slice.find("title=\"") {
                            let rest = &tag_slice[title_idx + 7..];
                            if let Some(quote_idx) = rest.find('"') {
                                let raw_title = &rest[..quote_idx];
                                clean_sound_title(raw_title)
                            } else {
                                sound_id.clone()
                            }
                        } else {
                            sound_id.clone()
                        };

                        let lookback_start = abs_btn.saturating_sub(200);
                        let lookback = &html[lookback_start..abs_btn];
                        let color = extract_color_from_html(lookback);

                        sounds.push(MemeSound {
                            id: sound_id,
                            name: title,
                            mp3_url,
                            duration: 1.5,
                            color,
                        });
                    }
                }
            }
        }
        cursor = end_tag + 1;
    }

    sounds
}

fn clean_sound_title(raw: &str) -> String {
    let mut s = raw.trim();
    if let Some(stripped) = s.strip_prefix("Play ") {
        s = stripped;
    }
    if let Some(stripped) = s.strip_suffix(" sound sound") {
        s = stripped;
    } else if let Some(stripped) = s.strip_suffix(" sound") {
        s = stripped;
    }
    s.trim().to_string()
}

fn extract_color_from_html(slice: &str) -> String {
    if let Some(idx) = slice.find("background-color:") {
        let rest = &slice[idx + 17..];
        if let Some(end) = rest.find(';') {
            let col = rest[..end].trim();
            if col.starts_with('#') && (col.len() == 4 || col.len() == 7) {
                return col.to_string();
            }
        }
    }
    "#00E5FF".to_string()
}

/// Returns high-quality curated built-in meme sound effects for offline and instant editing.
pub fn builtin_meme_catalogue() -> Vec<MemeSound> {
    vec![
        MemeSound {
            id: "vine-boom".to_string(),
            name: "Vine Boom".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/vine-boom.mp3".to_string(),
            duration: 1.5,
            color: "#FF1744".to_string(),
        },
        MemeSound {
            id: "bruh".to_string(),
            name: "Bruh".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/bruh-sound-effect-1.mp3".to_string(),
            duration: 1.2,
            color: "#2979FF".to_string(),
        },
        MemeSound {
            id: "among-us".to_string(),
            name: "Among Us Role Reveal".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/among-us-role-reveal-sound.mp3"
                .to_string(),
            duration: 2.1,
            color: "#D50000".to_string(),
        },
        MemeSound {
            id: "sad-violin".to_string(),
            name: "Sad Violin".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/sad-violin.mp3".to_string(),
            duration: 4.0,
            color: "#651FFF".to_string(),
        },
        MemeSound {
            id: "metal-pipe".to_string(),
            name: "Metal Pipe Clang".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/metal-pipe-clang.mp3".to_string(),
            duration: 1.8,
            color: "#78909C".to_string(),
        },
        MemeSound {
            id: "emotional-damage".to_string(),
            name: "Emotional Damage".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/emotional-damage-meme.mp3"
                .to_string(),
            duration: 1.4,
            color: "#FF9100".to_string(),
        },
        MemeSound {
            id: "airhorn".to_string(),
            name: "MLG Airhorn".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/mlg-airhorn.mp3".to_string(),
            duration: 2.3,
            color: "#00E676".to_string(),
        },
        MemeSound {
            id: "windows-xp-error".to_string(),
            name: "Windows XP Error".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/windows-xp-error.mp3".to_string(),
            duration: 1.0,
            color: "#2979FF".to_string(),
        },
        MemeSound {
            id: "anime-wow".to_string(),
            name: "Anime Wow".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/anime-wow-sound-effect.mp3"
                .to_string(),
            duration: 1.5,
            color: "#F50057".to_string(),
        },
        MemeSound {
            id: "run-meme".to_string(),
            name: "Run Meme".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/run-meme-sound.mp3".to_string(),
            duration: 3.5,
            color: "#FFD600".to_string(),
        },
        MemeSound {
            id: "robert-weide".to_string(),
            name: "Directed by Robert B. Weide".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/directed-by-robert-b_4.mp3"
                .to_string(),
            duration: 5.0,
            color: "#00B0FF".to_string(),
        },
        MemeSound {
            id: "minecraft-oof".to_string(),
            name: "OOF Minecraft".to_string(),
            mp3_url: "https://www.myinstants.com/media/sounds/oof_5.mp3".to_string(),
            duration: 0.8,
            color: "#8D6E63".to_string(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_myinstants_search_url_formatting() {
        assert_eq!(
            myinstants_search_url(""),
            "https://www.myinstants.com/en/search/?name=meme"
        );
        assert_eq!(
            myinstants_search_url("meme"),
            "https://www.myinstants.com/en/search/?name=meme"
        );
        assert_eq!(
            myinstants_search_url("vine boom"),
            "https://www.myinstants.com/en/search/?name=vine+boom"
        );
        assert_eq!(
            myinstants_search_url("among us!"),
            "https://www.myinstants.com/en/search/?name=among+us%21"
        );
    }

    #[test]
    fn test_parse_myinstants_html() {
        let sample = r#"
            <div class="circle small-button-background" style="background-color:#FF0000;"></div>
            <button class="small-button" onclick="play('/media/sounds/vine-boom.mp3', 'loader-1', 'vine-boom-123')" title="Play Vine Boom sound" type="button"></button>
            <div class="circle small-button-background" style="background-color:#00FF00;"></div>
            <button class="small-button" onclick="play('/media/sounds/bruh.mp3', 'loader-2', 'bruh-456')" title="Play Bruh sound sound" type="button"></button>
        "#;
        let sounds = parse_myinstants_html(sample);
        assert_eq!(sounds.len(), 2);
        assert_eq!(sounds[0].id, "vine-boom-123");
        assert_eq!(sounds[0].name, "Vine Boom");
        assert_eq!(
            sounds[0].mp3_url,
            "https://www.myinstants.com/media/sounds/vine-boom.mp3"
        );
        assert_eq!(sounds[0].color, "#FF0000");

        assert_eq!(sounds[1].id, "bruh-456");
        assert_eq!(sounds[1].name, "Bruh");
        assert_eq!(
            sounds[1].mp3_url,
            "https://www.myinstants.com/media/sounds/bruh.mp3"
        );
        assert_eq!(sounds[1].color, "#00FF00");
    }

    #[test]
    fn test_meme_library_state_favorites_and_history() {
        let mut state = MemeLibraryState::new();
        assert!(!state.is_favorite("vine-boom"));

        assert!(state.toggle_favorite("vine-boom"));
        assert!(state.is_favorite("vine-boom"));
        assert_eq!(state.favorites, vec!["vine-boom"]);

        assert!(!state.toggle_favorite("vine-boom"));
        assert!(!state.is_favorite("vine-boom"));

        state.record_used("vine-boom");
        state.record_used("bruh");
        assert_eq!(state.history, vec!["bruh", "vine-boom"]);
        state.record_used("vine-boom");
        assert_eq!(state.history, vec!["vine-boom", "bruh"]);
    }

    #[test]
    fn test_builtin_meme_catalogue_coverage() {
        let catalogue = builtin_meme_catalogue();
        assert!(catalogue.len() >= 12);
        assert!(catalogue.iter().any(|s| s.id == "vine-boom"));
        assert!(catalogue.iter().any(|s| s.id == "bruh"));
        assert!(catalogue.iter().any(|s| s.id == "among-us"));
        assert!(catalogue.iter().any(|s| s.id == "metal-pipe"));
    }
}
