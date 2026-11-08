<p align="center">
  <img src="assets/clipcat_logo.png" alt="ClipCat" width="120" />
</p>

<h1 align="center">ClipCat</h1>
<p align="center"><strong>Open-source, local-first desktop video editor built in Rust and Slint.</strong></p>

<p align="center">
  <a href="https://github.com/lyric9901/ClipCat/releases"><img src="https://img.shields.io/github/v/release/lyric9901/ClipCat?style=flat&logo=github&label=Release" alt="Latest Release" /></a>
  <a href="https://github.com/lyric9901/ClipCat/actions/workflows/ci.yml"><img src="https://img.shields.io/github/actions/workflow/status/lyric9901/ClipCat/ci.yml?style=flat&logo=githubactions&label=CI" alt="Build Status" /></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-1.93%2B-orange?style=flat&logo=rust" alt="Rust 1.93+" /></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/License-AGPL--3.0--or--later-blue?style=flat" alt="License: AGPL-3.0-or-later" /></a>
  <img src="https://img.shields.io/badge/Platforms-Windows%20%7C%20Linux%20%7C%20macOS-informational?style=flat" alt="Supported Platforms" />
</p>

<div align="center">
  <img src="assets/editor.png" alt="ClipCat Desktop Workspace" width="100%" />
</div>

---

## 1. Overview

**ClipCat** is an open-source, local-first non-linear video editor (NLE) built in native Rust with declarative UI by Slint. It combines GPU compositing, multitrack audio/video editing, and integrated broadcast scopes into a responsive desktop application.

Editing, rendering, timeline playback, and audio processing operate completely offline without subscriptions, accounts, telemetry, or exported watermarks.

---

## 2. Platform Support Matrix

| Platform | Tier | Status | Artifacts | Notes |
|---|---|---|---|---|
| **Windows x64** | Tier 1 | **Supported** | `.exe`, Inno Setup installer, WiX MSI | D3D12/Vulkan GPU acceleration with CPU Skia fallback. Tested on Windows 10/11. |
| **Linux x64** | Tier 1 | **Supported** | AppImage, `.deb`, `.rpm`, Arch PKGBUILD, Nix flake | Wayland & X11 compositing via Slint. Requires standard FFmpeg & VA-API libraries. |
| **macOS Apple Silicon** | Tier 1 | **Supported** | `.dmg` | Native `aarch64` Metal GPU pipeline and VideoToolbox hardware acceleration. |
| **macOS Intel** | Tier 2 | **Supported** | Universal binary / `.dmg` | Metal GPU pipeline on macOS 12+. |
| **Android ARM64 / x86_64** | Tier 3 | **Experimental (Spike)** | APK (test harness) | JNI/UniFFI bridge test (`android-native/`). Validates native window surface; not a full editor UI. |
| **iOS ARM64 / Simulator** | Tier 4 | **Build-only** | `.app` (ad-hoc signed) | Smoke testing harness. Requires local Apple Developer signing for physical hardware deployment. |

---

## 3. Features Available Today

### Multitrack Timeline & Dual Monitors
- **Source & Program Monitors:** Dedicated preview panes with 3-point editing (`Mark In`, `Mark Out`), frame-accurate scrubbing, and `Insert` / `Overwrite` commands.
- **Multitrack Editing:** Layer multiple video, audio, title, and adjustment tracks with per-track locking, soloing, muting, and magnetic ripple trimming.
- **Precision Trimming:** Razor cut, ripple delete, roll edits, slip, and slide.
- **Speed Ramping:** Smooth cubic Bézier speed curves and time remapping with audio pitch preservation.

### Broadcast Video Scopes & Color Tools
- **Hardware-Calculated Scopes:**
  - **Luma Waveform:** 0 to 100 IRE reference lines for luminance monitoring.
  - **RGB Parade:** Side-by-side color channel traces to detect color casts.
  - **Vectorscope:** Polar chrominance display with 75% SMPTE color targets and 123° skin-tone reference axis.
  - **RGB + Luma Histogram:** Real-time distribution curves with black-crush and white-clip warnings.
- **Color Correction:** 3-way color balance wheels, exposure, contrast, saturation, and 3D LUT importing (.cube).

### Audio Architecture & Studio Mixer
- **Channel Strips:** Per-track faders, pan law controls (-3 dB constant acoustic power sum), and stereo peak meters.
- **Loudness Standards:** ITU-R BS.1770-4 K-weighting loudness measurement with an integrated -14 LUFS streaming target.
- **Audio Processing:** Automated speech-based ducking, high-pass rumble filters, and voice denoise.

### Kinetic Typography & Titles
- **Local Font Rasterization:** Bundled instances of Hanken Grotesk with full RTL and bidirectional script support (Arabic, Hebrew, Persian, Urdu).
- **Styling Controls:** Vector stroke outlines, soft drop shadows, background padding plates, and letter spacing.
- **Text Animation:** Staggered letter-by-letter reveal, bounce, and kinetic typography.

### Local Machine Learning
- **Offline Speech-to-Text:** Local Whisper models generate synchronized subtitle tracks directly on your hardware.
- **Silence & Beat Detection:** Automated waveform silence analysis for fast jump-cut assembly.

---

## 4. Hardware & Performance Expectations

ClipCat is engineered to respect modest hardware budgets:
- **Minimum:** 64-bit Intel Core i3 (or AMD equivalent), 8 GB RAM, integrated graphics (Intel HD 520+ / Iris Xe / AMD Radeon Vega).
- **Recommended:** 4-core / 8-thread modern CPU, 16 GB RAM, dedicated GPU with Vulkan 1.2, Metal, or Direct3D 12 support.
- **Storage:** Solid-state drive (SSD) recommended for real-time 4K scrubbing cache.

---

## 5. Installation & Downloads

Official desktop builds are published under [Releases](https://github.com/lyric9901/ClipCat/releases):

- **Windows:** Download `ClipCat-Setup-x64.exe` or standalone portable archive.
- **Linux:** Download `ClipCat-x86_64.AppImage`, make it executable (`chmod +x ClipCat-*.AppImage`), and run it.
- **macOS:** Download `ClipCat-macOS-arm64.dmg`, open the disk image, and drag ClipCat to `/Applications`.

---

## 6. Building from Source

### Prerequisites

1. **Rust Toolchain:** Version 1.93.0 or later:
   ```bash
   rustup default stable
   ```
2. **FFmpeg Development Libraries:** FFmpeg 6.0+ libraries (`libavcodec`, `libavformat`, `libavutil`, `libswscale`, `libswresample`).
   - **Ubuntu/Debian:** `sudo apt-get install libavcodec-dev libavformat-dev libavutil-dev libswscale-dev libswresample-dev`
   - **macOS:** `brew install ffmpeg pkg-config`
   - **Windows:** Set `FFMPEG_DIR` to an unpacked dev build containing `include/` and `lib/`.

### Build & Run

```bash
# Clone the repository
git clone https://github.com/lyric9901/ClipCat.git
cd ClipCat

# Run all core verification tests
cargo test --manifest-path src/Cargo.toml -p concat-project -p concat-core -p concat-effects -p concat-text

# Launch the desktop editor
cargo run --manifest-path src/Cargo.toml -p concat
```

### Nix Users

Build and test reproducibly with the included flake:

```bash
nix build .#concat
./result/bin/concat
```

---

## 7. Architecture & Repository Structure

- `src/crates/concat`: Primary application executable and Slint UI window harness.
- `src/crates/concat-core`: Primitives: time fractions, memory arenas, pixel buffers, vector scopes.
- `src/crates/concat-project`: Project serialization (`.concat`), transactional undo/redo engine.
- `src/crates/concat-media`: FFmpeg decode/encode bindings, audio resampling, format converters.
- `src/crates/concat-render`: GPU composition pipeline using wgpu, Metal, Vulkan, and Direct3D 12.
- `src/crates/concat-effects`: Shader effects (luma wipe, dip-to-color, blur, crop).
- `src/crates/concat-text`: OpenType text shaping, RTL bidi layout, and kinetic text kinematics.
- `src/crates/concat-speech`: Offline Whisper speech recognition bindings.
- `src/crates/concat-vision`: ONNX Runtime integration for neural background segmentation.

---

## 8. Offline-First Architecture

ClipCat never makes network requests to perform editing operations. All codecs, fonts, models, and transitions reside on your local storage:
- Projects are saved as human-readable `.concat` JSON files alongside media references.
- AI transcription models download only when explicitly requested by the user and are stored locally in user data caches.
- No network connection is ever required to edit, preview, scrub, or export.

---

## 9. Roadmap

- [x] Initial fresh repository release & independent identity.
- [x] Multitrack timeline with 3-point editing and magnetic ripple trimming.
- [x] Real-time broadcast scopes (Waveform, RGB Parade, Vectorscope, Histogram).
- [x] Offline Whisper speech-to-text auto-subtitling.
- [ ] Motion tracking and planar mask stabilization.
- [ ] Keyframe curve editor with spatial Bézier tangent handles.
- [ ] Native audio VST3 plugin hosting support.
- [ ] Mature mobile companion apps (advancing beyond experimental spikes).

---

## 10. Contributing & Bug Reports

Contributions and bug reports are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before submitting pull requests.

To report a defect or request a feature, please file an issue using the [GitHub Issue Tracker](https://github.com/lyric9901/ClipCat/issues).

---

## 11. Licensing & Attribution

ClipCat is distributed as free, open-source software under the terms of the **GNU Affero General Public License v3.0 or later** ([AGPL-3.0-or-later](LICENSE)).

- **Source Code:** Released under GNU AGPLv3. See [LICENSE](LICENSE) and [LICENSING.md](LICENSING.md).
- **Upstream Authors:** ClipCat is an independent derivative work inheriting core engine capabilities from the Concat project. Upstream author copyright notices are strictly preserved in [AUTHORS](AUTHORS) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
- **Brand Assets:** The ClipCat name and Blue Cat Clapperboard logo are copyright © 2026 lyric9901.
- **Typefaces:** Bundled fonts (Hanken Grotesk) are licensed under the [SIL Open Font License 1.1](http://scripts.sil.org/OFL).
