# Concat — Dependency-Aware Implementation Sequence (Stages 1–16)

> **Document Version**: 1.0.0  
> **Status**: Approved Execution Roadmap  
> **Target Environment**: Windows 10/11, Intel Core i3 10th Gen, 8 GB DDR4 RAM  
> **Licensing**: AGPL-3.0-or-later / 100% Free / Local-First / Zero Cloud  

---

## 1. Milestone Dependency Graph

```
Milestone A: Architecture, Baseline Audit, Quality Gates (328 Tests Passing)
     │
     ▼
Milestone B: Stages 1–4 (Core Editing, 3-Point & Bins, Audio Scrubbing, Playback Caching)
     │   ├── Stage 1: Precision Editorial Core (Rolling, Slip, Slide, Shuttling) [COMPLETED]
     │   ├── Stage 2: 3-Point Source Editing & Hierarchical Asset Bins [IN PROGRESS: Bins pending]
     │   ├── Stage 3: Professional Audio Workstation & Scrubbing [Scrub pipeline complete; Mixer in Stg 13]
     │   └── Stage 4: Playback Engine & Memory-Bounded Frame Cache [COMPLETED: ReaderPool 512MB]
     ▼
Milestone C: Stages 5–7 (Editorial Structures, Zero-Copy GPU Decode, Export System)
     │   ├── Stage 5: Advanced Editorial Structures (Compound / Nested Clips)
     │   ├── Stage 6: Zero-Copy Hardware Pipeline (D3D11 / WGPU Interop)
     │   └── Stage 7: Professional Export System (In/Out Range Slicing, Presets, Queue) [COMPLETED]
     ▼
Milestone D: Stages 8–10 (UX / Keymaps, 8 GB RAM Reliability, Quality Verification)
     │   ├── Stage 8: Professional UX & Command Palette [COMPLETED]
     │   ├── Stage 9: 8 GB RAM Performance & Memory Clamping [COMPLETED]
     │   └── Stage 10: Static Verification, Clippy & Quality Gates [COMPLETED]
     ▼
Milestone E: Stages 11–13 (Creative Asset Ecosystem, GPU Scopes, DAW Mixer)
     │   ├── Stage 11: Creative Ecosystem (11.1 Shaders, 11.2 Transitions, 11.3 Titles, 11.6 Font Manager, 11.7 Packs)
     │   ├── Stage 12: Professional Color Suite & Real-Time GPU Scopes (Waveform, Vectorscope, Parade, Histogram)
     │   └── Stage 13: Professional Audio Mixer (Track Faders, Pan, LUFS Metering, Ducking)
     ▼
Milestone F: Stages 14–16 (Multicam, Interchange, Local AI Automation)
         ├── Stage 14: Advanced Editorial Structures (Multicam Editing, Angle Grid, Sync)
         ├── Stage 15: Professional Interchange (FCPXML 1.10+, CMX 3600 EDL, Subtitle SRT/VTT)
         └── Stage 16: Local Creator AI (Silence Detection/Removal, Beat Detection, Auto-Reframe)
```

---

## 2. Dependency Invariants & Technical Sequence

1. **Stage 1 (Precision Editorial)**: Fundamental basis for clip boundary manipulations. Complete and verified with 328 automated tests.
2. **Stage 2 (3-Point Editing & Hierarchical Bins)**:
   - Backend commands `InsertEdit`, `OverwriteEdit`, `SetMediaMarks`, and `ClearMediaMarks` verified.
   - Remaining work: Bin folder management (`BinFolder`, `CreateBinFolder`, `MoveMediaToBin`, `DeleteBinFolder`) and Media Pane folder tree UI.
3. **Stage 11.6 (External Font Manager)**: Prerequisite for Stage 11.3 (Motion Titles) and complex typography templates. Pure Rust implementation using `ttf-parser`, `fontdb`, and `tiny-skia`.
4. **Stage 11.7 (Local Asset Pack Manager)**: Foundation for distributing offline effects, titles, transitions, and presets without network calls.
5. **Stage 12 (Real-Time GPU Scopes)**: Builds directly on WGPU compute shaders to analyze preview frames without CPU readback.
6. **Stage 13 (Professional Audio Mixer & LUFS)**: Builds upon the CPAL audio engine and real-time scrub pipeline; adds track-level gain/pan DSP and ITU-R BS.1770-4 K-weighting.
7. **Stage 15 (Interchange - FCPXML & EDL)**: Serializes Concat timeline data models to industry standard XML/EDL formats.
8. **Stage 16 (Local Creator AI & Silence Detection)**: Uses audio RMS/peak windows for silence stripping, FFT spectral flux for beat snapping, and ONNX Runtime for auto-reframing.
