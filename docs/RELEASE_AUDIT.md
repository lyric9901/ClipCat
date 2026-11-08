# ClipCat — Release Audit & Architectural Analysis

**Date:** November 8, 2026  
**Repository:** [lyric9901/ClipCat](https://github.com/lyric9901/ClipCat)  
**Root Commit:** `b0e274409ccd8168cdf92778f5ae298c0b538181`  
**License:** GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)

---

## 1. Architectural Overview

ClipCat is a desktop video editor designed for offline-first performance, combining a Rust engine with a declarative Slint UI and an integrated FFmpeg media pipeline.

### Workspace & Crate Structure

```
src/
├── crates/
│   ├── concat/               # Main application binary and Slint UI window harness
│   ├── concat-api/           # Public IPC / gRPC engine definitions and schemas
│   ├── concat-core/          # Core primitives: time, arena, frame buffers, waveforms, vectorscopes
│   ├── concat-effects/       # GPU effect engine (WGSL shaders, dip-to-color, luma wipes)
│   ├── concat-export/        # Timeline flattening, encoder pipelines, export render graphs
│   ├── concat-host/          # Playback loops, scrub coordination, project state management
│   ├── concat-media/         # FFmpeg decode/encode abstractions, color conversion, audio mixing
│   ├── concat-project/       # Document model, serialization (.concat), undo/redo command stack
│   ├── concat-render/        # GPU render graphs (wgpu/Skia/Vulkan/Metal/D3D12), vector scopes
│   ├── concat-server/        # Headless background render service
│   ├── concat-speech/        # Local Whisper speech-to-text integration
│   ├── concat-text/          # Typography rasterization, styled runs, font fallback, shapes
│   └── concat-vision/        # ONNX Runtime inference, background matting, face detection
├── patches/
│   ├── i-slint-renderer-skia # Pinned Skia renderer adapter for Slint
│   └── whisper-rs-sys        # Vendored C++ whisper.cpp bindings with SIMD support
```

---

## 2. Platform Support Matrix

| Platform | Target Triple | Support Tier | Verification Status | Notes |
|---|---|---|---|---|
| **Windows x64** | `x86_64-pc-windows-msvc` | **Tier 1 (Supported)** | Verified | Complete installer (Inno Setup / WiX MSI), standalone binary, D3D12/Vulkan acceleration, CPU fallback. |
| **Linux x64** | `x86_64-unknown-linux-gnu` | **Tier 1 (Supported)** | Verified | AppImage, deb, rpm, Arch PKGBUILD, Nix flake (`.#concat`). Wayland & X11 support via Slint. |
| **macOS Apple Silicon** | `aarch64-apple-darwin` | **Tier 1 (Supported)** | Verified | Native DMG bundle, Metal GPU shader pipeline, VideoToolbox hardware acceleration. |
| **macOS Intel** | `x86_64-apple-darwin` | **Tier 2 (Supported)** | Build Verified | Universal binary compilation supported; Metal GPU pipeline. |
| **Android ARM64** | `aarch64-linux-android` | **Tier 3 (Experimental)** | Build / Spike | Native Compose spike (`android-native/`); UniFFI bridge into `concat_mobile`. Not a full feature editor yet. |
| **Android x86_64** | `x86_64-linux-android` | **Tier 3 (Experimental)** | Emulator Spike | Emulator verification build; shares JNI harness with ARM64. |
| **iOS ARM64** | `aarch64-apple-ios` | **Tier 4 (Build-only)** | Build / Ad-hoc | Unsigned / Ad-hoc `.app` bundle via `src/scripts/ios-app.sh`. Requires Apple Developer provisioning to install on devices. |
| **iOS Simulator** | `aarch64-apple-ios-sim` | **Tier 4 (Build-only)** | Simulator Smoke | Sim smoke harness; Slint winit windowing. |

---

## 3. Subsystem Audit

### 3.1 What Works (Production Grade)
- **Document & Project Engine (`concat-project`)**:
  - Non-destructive timeline model with multi-track audio and video.
  - Multi-lane ripple editing, roll edits, slip edits, split edits, and 3-point editing.
  - Full transactional undo/redo command stack with bounded memory.
  - Backward-compatible JSON document format (`.concat`).
- **Typography & Motion Titles (`concat-text`)**:
  - Offline font loading with bundled open-source typefaces (Hanken Grotesk).
  - Multi-language text layout, right-to-left bidi support (Arabic/Hebrew), font fallback.
  - Styled runs, background plates, borders, shadows, gradients, and animated kinetic reveals.
- **Color & Scopes (`concat-render`, `concat-core`)**:
  - Real-time Vectorscope, Waveform (Luma/RGB parade), and Histogram density analysis.
  - Color range handling (Limited 16–235 vs Full 0–255), SDR to HDR tone-mapping pipelines.
- **Audio Architecture (`concat-core`, `concat-media`)**:
  - Sample-rate conversion, equal-power stereo panning laws, multi-bus audio ducking.
  - ITU-R BS.1770-4 K-weighting loudness measurement (LUFS) and true-peak monitoring.
  - Keyframable volume envelopes, anti-pop trajectory smoothing during scrub.
- **Video & Shader Effects (`concat-effects`)**:
  - WGSL-based hardware shaders: Luma wipes, dip-to-color, blurs, transform matrix pipelines.
- **Offline Machine Learning (`concat-vision`, `concat-speech`)**:
  - Fully local Whisper model execution for auto-transcription and subtitles.
  - Offline ONNX Runtime inference for background segmentation and silence detection.

### 3.2 What Is Incomplete / Prototype
- **Android Implementation (`android-native/`)**:
  - The repository contains an experimental JNI prototype (`app.concat.editor.spike.MainActivity`).
  - It validates that Android's `SurfaceView` and `ANativeWindow` can host the Rust renderer via UniFFI.
  - **It is NOT a full standalone mobile video editor.** Mobile UI controls are minimal Compose buttons for verification.
- **iOS App (`src/scripts/ios-app.sh`)**:
  - Builds an ad-hoc `.app` bundle for testing in Simulator or with local developer signing.
  - No public App Store or signed enterprise IPA distribution exists.

### 3.3 What Was Broken & Fixed in CI
1. **POSIX Script Permissions in Git**:
   - Shell scripts (`scripts/ci/fetch.sh`, `src/scripts/ffmpeg-mobile.sh`, `android-native/gradlew`, etc.) were committed without the executable bit (`100644`), causing exit code 126 (`Permission denied`) on Linux and macOS runners.
   - **Resolution**: Updated git tree index mode to `100755` (`--chmod=+x`).
2. **Export Engine Struct Initializers (`concat-export`)**:
   - `ExportRequest` struct had fields added (`range_start`, `range_duration`, `audio_only`) without updating preview shims and test cases in `concat-export/src/lib.rs` and `concat-host/src/consolidate.rs`.
   - `concat-export/src/flatten.rs` had incorrect type path (`concat_project::Track` instead of `concat_project::model::Track`) and checked a non-existent field `clip.hidden`.
   - **Resolution**: Repaired struct initialization and track visibility paths; verified all unit tests pass.

### 3.4 Existing Technical Debt & Invariants
- **Internal Crate Naming**:
  - Crate names remain `concat-*` to preserve project schema compatibility, IPC interfaces, and avoid massive breaking diffs.
- **Third-Party Attribution**:
  - Upstream Concat authorship and AGPL-3.0-or-later licenses are strictly maintained in `AUTHORS`, `LICENSE`, and `THIRD_PARTY_NOTICES.md`.
