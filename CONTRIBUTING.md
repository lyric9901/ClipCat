# Contributing to ClipCat

Thanks for your interest in ClipCat. ClipCat is a free, local-first video editor that runs entirely on the user's machine, built in native Rust and Slint.

## Code of Conduct

Please review and adhere to the [Code of Conduct](CODE_OF_CONDUCT.md) in all community spaces and issue interactions.

## How to Help

1. **Test with Real Footage:** Download a release build or run from source, and edit real video projects. Real-world workflows identify issues faster than isolated synthetic runs.
2. **Detailed Bug Reports:** When filing an issue on [GitHub Issues](https://github.com/lyric9901/ClipCat/issues), include your operating system, CPU/GPU, reproduction steps, sample media if possible, and observed vs expected behavior.
3. **Focused Pull Requests:** For large architectural changes, open an issue first to align on technical strategy and constraints.

## Development Setup

### System Prerequisites
- **Rust Toolchain:** Stable Rust (1.93 or later) via [rustup.rs](https://rustup.rs).
- **FFmpeg 7+ Development Libraries:** Headers and libraries for `libavcodec`, `libavformat`, `libavutil`, `libswscale`, `libswresample`.
- **C/C++ Compiler & CMake:** GCC/Clang on Linux/macOS or Visual Studio 2022 C++ tools on Windows.

### Build and Test

```bash
# Clone the repository
git clone https://github.com/lyric9901/ClipCat.git
cd ClipCat

# Run unit and integration tests across core workspace packages
cargo test --manifest-path src/Cargo.toml -p concat-project -p concat-core -p concat-effects -p concat-text

# Run formatting checks
cargo fmt --manifest-path src/Cargo.toml --all -- --check

# Run Clippy lints
cargo clippy --manifest-path src/Cargo.toml --workspace -- -D warnings
```

## Workspace Structure

| Package / Directory | Purpose |
|---|---|
| `src/crates/concat-core` | Low-level frame buffers, timecode math, scopes, audio DSP (LUFS/pan/ducking), AI silence/beat detection |
| `src/crates/concat-project` | Project schema, tracks, clips, transitions, undo/redo, asset database, workspace presets |
| `src/crates/concat-effects` | GPU shader passes, GLSL/WGSL pipelines, color adjustment chains |
| `src/crates/concat-text` | Font parsing, shaping, layout, bounding plates, per-character typography animation |
| `src/crates/concat-render` | Multi-layer frame compositing, blend modes, scopes visualization |
| `src/crates/concat-export` | Video encoding pipeline, audio multiplexing, format export |
| `src/crates/concat` | Desktop application binary, Slint UI markup, native windowing, audio device routing |

## Design Guidelines

- **Offline-First:** Core editing, trimming, playback, audio mixing, color correction, and export must never require an internet connection.
- **Unslop Standard:** Maintain a clean, intentional interface. No artificial neon glows, emoji badges in core UI chrome, or excessive card nesting.
- **Target Budget:** Keep peak memory bounded and responsive on entry-level hardware (Intel Core i3 10th Gen, 8 GB RAM).
