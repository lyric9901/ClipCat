# Changelog

All notable changes to **ClipCat** are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.2.9] - 2026-11-08

### Added
- **ClipCat Professional Branding & Identity:**
  - Official multi-resolution icon suite (16px to 512px mipmaps + Windows multi-resolution `.ico`).
  - Native window branding, application title bar logo mark, splash screen, and Windows installer metadata (`concat.iss`).
  - Preserved internal crate architecture and `.concat` project file backwards compatibility.
- **6 Professional Workspaces:**
  - Built-in layouts for `Editing`, `Color`, `Audio`, `MotionGraphics`, `Effects`, and user-defined `Custom`.
  - Workspace selector tabs in the top application bar with instant layout reconfiguration.
- **Offline Creative Asset Library & Pack System:**
  - 16-category local asset database (`AssetDatabase`) with category counts, search, and favorites.
  - Safe `.concatpack` sandboxed folder installer with strict directory-traversal protection (`sanitize_relative_path`).
  - Modular `AssetProvider` architecture with offline detection and bundled meme SFX fallback.
  - Visual offline indicator badge in the media library shelf.
- **Broadcast Video Scopes:**
  - Real-time Histogram with RGB, BT.709 Luma channels, and black/white clipping counters.
  - 0–100 IRE Luma Waveform with reference graticule.
  - Side-by-side RGB Parade trace generation.
  - Polar Vectorscope with Cb/Cr chrominance density, 75% SMPTE color targets, and 123° skin-tone reference line.
- **Fairlight-Style Audio Mixer:**
  - Multi-channel fader strips with mute/solo and stereo peak level meters.
  - ITU-R BS.1770-4 K-weighting filter pair and integrated -14 LUFS loudness targeting.
  - `PanLaw::EqualPower` (-3 dB constant acoustic power sum) and linear pan options.
  - Dialogue ducking envelope generator with configurable attack and release curves.
- **AI Creator Automation:**
  - Offline RMS energy silence detector with automatic timeline ripple cut compaction.
  - Transient onset flux beat detector for rhythmic editing snapping.
- **Dual Monitor Workspace:**
  - Source Monitor and Program Monitor with 3-point editing (`Mark In`, `Mark Out`).
  - Direct timeline `Insert` and `Overwrite` commands with timecode display.

### Changed
- Refactored English localizations to reference ClipCat.
- Optimized timeline rendering and memory usage for 8 GB RAM target configurations.

### Fixed
- Addressed audio clipping during rapid playhead scrubbing with Hann anti-pop envelope smoothing.
- Resolved track locking bypass on ripple operations.
