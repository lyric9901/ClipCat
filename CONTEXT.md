# Concat Professional Editor Expansion — Technical Context

## Environment & Architecture Context
- **Target OS**: Windows 10/11 x86_64
- **Target Hardware**: Intel Core i3 10th Gen, 8 GB RAM (4 GB soldered + 4 GB DDR4), Integrated Intel UHD Graphics
- **Compiler / Toolchain**: Rust 1.93.1 (`x86_64-pc-windows-gnu` MinGW active toolchain)
- **UI Framework**: Slint 1.18 with Skia / Software renderer fallback
- **Compositor**: WGPU 30 with WGSL shaders and reference compositors
- **Media Engine**: FFmpeg 8.1 LGPL shared (avcodec, avformat, avutil, swresample, swscale)
- **Audio Output**: CPAL synchronized to audio clock
- **Licensing Constraint**: AGPL-3.0-or-later, 100% free, local-first, zero cloud accounts, zero telemetry.

## New Modules & APIs Added
1. `concat_project::assets`:
   - `AssetCategory` (16 categories), `AssetItem`, `AssetDatabase`
   - `AssetProvider` trait, `LocalAssetProvider`, `MyInstantsAssetProvider`
   - `AssetPackManifest`, `sanitize_relative_path`, `validate_pack_manifest`
2. `concat_project::workspace`:
   - `WorkspacePane`, `WorkspacePreset`, `WorkspaceLayout`
3. `concat_core::scopes` & `concat_render::scopes`:
   - `HistogramData` (256 levels for R, G, B, Luma, clipping flags, SVG generator)
   - `WaveformData` (0..100 IRE luminance density grid, SVG envelope generator)
   - `RgbParadeData` (R, G, B channel column traces)
   - `VectorscopeData` (256x256 polar Cb/Cr chrominance plot, 123° skin tone line, 75% SMPTE color bar targets)
   - `VideoScopesAnalyzer` (zero-allocation runner with adaptive subsampling stride)
4. `concat_core::audio`:
   - `PanLaw::EqualPower` (-3dB constant acoustic power sum), `PanLaw::Linear`
   - `TrackMixerChannel`, `MasterBus`, `db_to_linear`, `linear_to_db`
   - `KWeightingFilter` (ITU-R BS.1770-4 biquad filter pair for LUFS)
   - `DuckingEnvelope` (smooth speech-triggered background music attenuation)
5. `concat_core::ai_creator`:
   - `SilenceDetector` (RMS sliding window silence interval detector with speech padding)
   - `BeatDetector` (transient onset spectral flux beat detector for musical cut snapping)
6. Slint UI Components:
   - `ui/inspector/scopes.slint`: `ScopesView` with mode switcher, clipping badges, real-time SVG paths.
   - `ui/workspace/audio-mixer.slint`: `AudioMixerPane` with Fairlight-style channel strips, faders, meters, and LUFS target badge.
   - `ui/title-bar.slint`: Segmented workspace presets tab bar, undo/redo buttons, and emerald performance badge.
   - `ui/workspace/config-pane.slint`: Embedded real-time video scopes inside Color/Adjust inspector panel.

## Verification Evidence
- `concat-core`: **88 unit tests + 2 doctests passed (0 failed)**
- `concat-project`: **274 unit tests passed (0 failed)**
- `concat-effects`: **74 unit tests passed (0 failed)**
- `concat-text`: **41 unit tests passed (0 failed)**
- Total passing tests: **479 unit tests + 2 doctests (0 failed)**
