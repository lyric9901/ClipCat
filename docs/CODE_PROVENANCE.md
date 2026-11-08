# ClipCat — Code Provenance & Legal Classification

**Document Version:** 1.0.0  
**Date:** 2026-11-08  
**Repository:** [lyric9901/ClipCat](https://github.com/lyric9901/ClipCat)  
**Primary License:** GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)  

---

## 1. Executive Summary & Legal Principles

ClipCat is an open-source, non-linear video editor developed in Rust and Slint, originally derived from the Concat video editor (created by Jareer Abdullah and 49 upstream open-source contributors).

This document establishes the precise provenance and licensing classification of every component in the repository. In strict adherence to open-source licensing laws and the GNU Affero General Public License v3.0 (AGPL-3.0-or-later, Sections 4, 5, and 13):
1. **No False Attribution:** Renaming, branding, or refactoring inherited source code does **not** make its underlying copyright exclusively the new maintainer's. Original copyright notices in all files, manifests, and documentation remain intact.
2. **Attribution Preservation:** The original author (Jareer Abdullah) and all 49 upstream contributors are permanently preserved in the [`AUTHORS`](../AUTHORS) file and referenced in [`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md).
3. **Copyleft Continuity:** All modifications, derivative works, and additions to the core engine and editor are licensed under **AGPL-3.0-or-later**. The source code corresponding to any deployed or distributed binary remains freely accessible under the AGPL.
4. **Third-Party Dual/Multi-Licensing Integrity:** Components governed by other licenses (e.g., Slint under GPL-3.0-only, FFmpeg under LGPL-2.1-or-later, Hanken Grotesk under SIL Open Font License 1.1) maintain their specific licensing boundaries without conflict.

---

## 2. Component Classification Matrix

The repository components are classified into five distinct provenance categories:

| Category | Description | Primary Location | Applicable License | Copyright Holder(s) |
| :--- | :--- | :--- | :--- | :--- |
| **Category 1: Original ClipCat Code** | Modifications, branding assets, build repairs, CI fixes, Windows installer scripts, migration docs, and platform patches added specifically for the ClipCat project. | `.github/workflows/`, `assets/icons/clipcat*`, `docs/`, `src/crates/concat/build.rs`, `src/crates/concat-host/src/dirs.rs`, `src/crates/concat-export/` fixes | AGPL-3.0-or-later | 2026 ClipCat contributors |
| **Category 2: Inherited Upstream Code** | Core media engine, timeline data structures, audio mixing, rendering pipeline, Slint UI layout, and shaders inherited from the Concat codebase. | `src/crates/concat-*`, `src/crates/concat/src/`, `src/crates/concat/ui/` | AGPL-3.0-or-later | 2026 Jareer Abdullah and Concat contributors |
| **Category 3: Third-Party Dependencies** | External Rust crates, libraries, and C/C++ native frameworks linked dynamically or statically. | `Cargo.lock`, FFmpeg, ONNX Runtime, Whisper.cpp, Slint GUI toolkit | Various (LGPL-2.1+, MIT, Apache 2.0, GPL-3.0, OFL-1.1) | Respective upstream authors and projects |
| **Category 4: Generated Code & Assets** | Slint compiled code (`ui/app.slint` rust artifacts), bundled effect card snapshots, and localized message dictionaries. | `target/`, `OUT_DIR`, `locales/*.json`, `src/crates/concat/cards/` | AGPL-3.0-or-later | Jareer Abdullah, Concat contributors, ClipCat contributors |
| **Category 5: Unknown / Untracked** | None. All files in the repository have verified provenance. | N/A | N/A | N/A |

---

## 3. Detailed Component Breakdown

### 3.1. Original ClipCat Code (Category 1)
- **Branding Assets:**
  - `clipcat.ico`, `assets/icons/clipcat.ico`, `assets/icons/clipcat_logo_*.png` (Custom clapperboard cat branding).
- **Windows Packaging & Installation Infrastructure:**
  - `assets/windows/concat.iss` / `clipcat.iss` Inno Setup definitions configured for ClipCat executable and desktop shortcuts.
  - `assets/windows/concat.wxs` / `clipcat.wxs` WiX 4 / 5 configuration for machine-wide MSI deployment.
  - WinResource metadata integration in `src/crates/concat/build.rs` embedding product name, legal copyright, and file versioning.
- **CI / CD Pipeline Fixes & Script Permissions:**
  - Executable bit (`100755`) remediation across all shell scripts (`scripts/ci/*.sh`, `src/scripts/*.sh`, `android-native/gradlew`).
  - `.github/workflows/ci.yml`, `build-app.yml`, `release.yml` repaired for cross-platform artifact staging.
- **Rust Engine Patching & Compilation Fixes:**
  - Missing field corrections in `concat-export/src/lib.rs` (`audio_only`, `range_duration`, `range_start`).
  - `Track` visibility resolution in `concat-export/src/flatten.rs`.
  - Multi-version data adoption logic in `src/crates/concat-host/src/dirs.rs` supporting `app.clipcat.editor` alongside `app.concat.editor` and `org.standalones.concat`.
  - Dual project manifest recognition in `src/crates/concat-host/src/projects.rs` supporting `clipcat.json` and legacy `concat.json`.
- **Documentation & Legal Architecture:**
  - `README.md` (re-authored with comprehensive cross-platform matrix and technical accuracy).
  - `AUTHORS` (comprehensive attribution of all 50 historical contributors).
  - `docs/RELEASE_AUDIT.md`, `docs/RELEASE_REPORT.md`, `docs/CODE_PROVENANCE.md`, `docs/BRANDING_MIGRATION_INVENTORY.md`, `docs/BRANDING_MIGRATION_REPORT.md`.

### 3.2. Inherited Upstream Code (Category 2)
The underlying architecture remains faithful to the robust foundation created by Jareer Abdullah and Concat contributors:
- `concat-core`: Timebase, rational arithmetic, geometry, color representations, and keyframe interpolation.
- `concat-media`: Demuxing, decoding, hardware accelerated FFmpeg probing, and frame caching.
- `concat-project`: Timeline models, tracks, clips, transitions, and serialization.
- `concat-render`: Compositing engine, wgpu pipeline, shader shaders, and blitting.
- `concat-export`: Multi-track rendering, audio bus mixdown, encoder muxing.
- `concat-host`: App runtime, session state, background job queues, and hardware monitoring.
- `concat-speech`: Whisper speech-to-text integration and Chatterbox text-to-speech.
- `concat-vision`: RealESRGAN image enhancement and ONNX background cutout segmentation.
- `concat-effects`: GPU and CPU video effects, color grading, LUT application.
- `concat-text`: Slint text typography, font discovery, and subtitle formatting.
- `concat-api` & `concat-server`: Remote JSON-RPC API and MCP agent server.
- `concat` (binary & UI): Slint user interface, timeline UI, inspector panels, audio mixer.

### 3.3. Third-Party Dependencies (Category 3)
Third-party libraries are incorporated in compliance with their respective open-source licenses:
- **Slint (v1.18)**: User interface toolkit used under **GPL-3.0-only**. Combines with AGPL-3.0-or-later without conflict as permitted by GNU licensing guidelines.
- **FFmpeg (v7.x)**: Media decoding, encoding, scaling, and format probing. Linked dynamically under **LGPL-2.1-or-later**. Built without GPL-incompatible components (e.g., `--disable-gpl`, `--enable-version3`, `--disable-nonfree`).
- **ONNX Runtime (v1.20+)**: Neural network execution for machine learning models (DirectML, CoreML, NNAPI, CPU). MIT License.
- **Whisper.cpp / whisper-rs**: Transcriptions and voice recognition. MIT License.
- **wgpu (v30) / naga**: WebGPU-compliant native graphics pipeline. MIT / Apache 2.0 dual license.
- **Hanken Grotesk**: Primary application typography. SIL Open Font License 1.1.
- **Rust Standard Library & Crates.io Ecosystem**: `serde`, `tokio`, `cpal`, `image`, `hound`, etc., governed by their respective MIT/Apache-2.0 licenses.

### 3.4. Generated Assets & Models (Category 4)
- **Localizations:** 26 language translation dictionaries in `src/crates/concat/locales/*.json`.
- **Effect Preview Cards:** 20+ pre-rendered GPU thumbnails embedded in `src/crates/concat/cards/*.jpg`.
- **Model Distribution:** Neural network weights (`.onnx`, `.bin`) fetched on demand via secure SHA-256 verified download mirrors.

---

## 4. AGPL-3.0-or-later Compliance & Distribution Checklist

When distributing ClipCat binaries or running network services, the following obligations are satisfied:
1. **Source Code Availability (AGPL Section 6 & 13):**
   - The full corresponding source code is published at `https://github.com/lyric9901/ClipCat`.
   - Settings > About and the application manifest provide direct clickable links to the repository.
2. **Notice Retention (AGPL Section 4):**
   - Every source file retains its original `SPDX-FileCopyrightText` and `SPDX-License-Identifier` headers.
   - The full AGPL-3.0 license text is distributed in the root [`LICENSE`](../LICENSE) file and packaged with Windows, Linux, and macOS installers.
3. **Change Tracking (AGPL Section 5(a)):**
   - All alterations made to upstream code are prominently documented in git commit history and summarized in the migration reports.
4. **Patent Grant (AGPL Section 11):**
   - Each contributor grants a non-exclusive, worldwide, royalty-free patent license for their contributions.

---

## 5. Summary Statement

ClipCat is an independently maintained, actively developed fork of Concat. The project proudly acknowledges its lineage, honors all intellectual property and copyright notices of the original creators, and commits to advancing the software entirely under the GNU Affero General Public License.
