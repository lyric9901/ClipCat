# Concat — Architecture Decisions & Engine Invariants

> **Document Version**: 1.0.0  
> **Status**: Living Architectural Decision Record (ADR)  

---

## ADR-001: Rational Timebase Representation
- **Decision**: All timeline timestamps and clip offsets use `concat_core::time::Rational` representing exact rational seconds ($P/Q$ with 64-bit integer numerator and denominator).
- **Rationale**: Floating-point representations (`f32`/`f64`) accumulate rounding drift over multi-hour projects (e.g. 29.97 fps NTSC or 23.976 fps drop-frame timecodes). Exact rational math guarantees sub-microsecond precision and bit-identical cut points between preview and export.

---

## ADR-002: Undo/Redo via Structural Sharing Snapshots
- **Decision**: State mutations occur via `Command` applied to `Project`. Each state transition produces a whole-state snapshot where unchanged data structures (`Arc<Timeline>`, `Arc<Vec<Clip>>`) share underlying memory.
- **Rationale**: Avoids the complexity and bugs of reversible command pairs (`do`/`undo`). Deep clones are avoided by leveraging `Arc::make_mut` copy-on-write semantics.

---

## ADR-003: Memory-Bounded Audio Scrubbing Architecture
- **Decision**: Audio scrub playback uses bounded 80ms snippets with smooth linear de-clicking micro-fades (32 to 240 samples) and monotonic generational request cancellation.
- **Rationale**: Direct real-time audio playback during scrub provides the tape-jog feedback professional editors require without starving the CPAL audio thread or allocating heap memory.

---

## ADR-004: Local Creative Assets & Font Management
- **Decision**: External font files (`.ttf`, `.otf`) and creative packs are managed entirely within project asset folders or application local data directories.
- **Rationale**: Writing to Windows system directories (`C:\Windows\Fonts`) requires elevated administrator privileges and risks OS corruption. Local project font bundling ensures zero-friction project migration across machines.

---

## ADR-005: Real-Time GPU Scopes via WGPU Compute Shaders
- **Decision**: Video scope analysis (Waveform, Vectorscope, Histogram) is performed on the GPU using WGPU compute shaders operating on 540p downsampled proxy textures.
- **Rationale**: Copying frame pixels back to CPU RAM for analysis saturates PCIe bandwidth and exhausts CPU cycles needed for media decoding and audio DSP.
