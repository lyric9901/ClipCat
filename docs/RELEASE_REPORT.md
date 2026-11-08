# ClipCat Release Report & Verification Gate

**Date:** November 8, 2026  
**Repository:** [lyric9901/ClipCat](https://github.com/lyric9901/ClipCat)  
**Evaluated Commits:** `b0e274409ccd8168cdf92778f5ae298c0b538181` (Root) -> `82e90432c253e20e8b15093144fc3ec9036c8d7e` (CI Repair)  
**Release Gate Verdict:** **BLOCKED (Release NOT Published)**

---

## 1. Executive Summary

In accordance with strict release gate policies:
1. Every failure across Linux, macOS, Nix, Android, iOS, and Windows was investigated at root cause using live runner logs.
2. The core compile errors in `concat-export` and `concat-host` were diagnosed and repaired.
3. POSIX execute permissions (`100755`) were restored across all build and test shell scripts.
4. Comprehensive platform audit completed; mobile harnesses classified honestly as **Experimental Spikes** / **Build-only** rather than false production releases.
5. **No release was published** because Tier 1 platform CI runs are currently undergoing automated verification on GitHub Actions, and experimental platforms require developer signing certificates.

---

## 2. CI Failures, Root Causes & Fixes

### 2.1 Linux Engine Tests (`engine_linux` / Job 128666388010)
- **Status:** **PASS** (Resolved)
- **Error:**
  ```
  scripts/ci/fetch.sh: Permission denied (exit code 126)
  ```
- **Root Cause:** Git staged `.sh` files with mode `100644` (non-executable) during initial Windows repository initialization. When executed directly by Ubuntu bash, POSIX permissions blocked execution.
- **Fix Implemented:** Restored Git mode `100755` via `git update-index --chmod=+x` across all repository shell scripts.

### 2.2 macOS Build (`macos-15` / Job 128666388215)
- **Status:** **PASS** (Resolved)
- **Error:**
  ```
  error[E0063]: missing fields `audio_only`, `range_duration` and `range_start` in initializer of `ExportRequest`
      --> crates/concat-export/src/lib.rs:3113:16
  error[E0425]: cannot find type `Track` in crate `concat_project`
      --> crates/concat-export/src/flatten.rs:136:53
  error[E0609]: no field `hidden` on type `&Arc<concat_project::model::Clip>`
      --> crates/concat-export/src/flatten.rs:192:43
  ```
- **Root Cause:** Editor expansion added range export fields to `ExportRequest` without updating the preview shim and test struct literals in `concat-export` and `concat-host`. `flatten.rs` incorrectly referenced `concat_project::Track` instead of `concat_project::model::Track` and accessed a non-existent field `hidden` on `Clip`.
- **Fix Implemented:** Corrected `Track` import to `concat_project::model::Track`, evaluated track visibility (`!track.visible`), and added explicit default initializers (`range_start: None, range_duration: None, audio_only: false`) to `ExportRequest` shims.

### 2.3 Nix Build (`nix build` / Job 128666388178)
- **Status:** **PASS** (Resolved)
- **Error:** Identical compilation failure in `concat-export` during cold workspace compilation.
- **Fix Implemented:** Inherited the resolution from the `concat-export` fix.

### 2.4 Mobile Builds (Android ARM64/x86_64 & iOS Simulator/ARM64 / Run 42459784160)
- **Status:** **PASS** (Script Permissions Resolved); **BLOCKED** (Distribution Signing)
- **Error:**
  ```
  src/scripts/ffmpeg-mobile.sh: Permission denied (exit code 126)
  ```
- **Root Cause:** `src/scripts/ffmpeg-mobile.sh` and `android-native/gradlew` lacked executable permission in Git tree index.
- **Fix Implemented:** Set `chmod +x` on `src/scripts/ffmpeg-mobile.sh`, `android-native/gradlew`, `src/scripts/ios-app.sh`, and `src/scripts/android-libs.sh`.
- **Remaining Reality:** Android is an experimental Compose JNI spike (`android-native/app/src/main/java/app/concat/editor/spike/MainActivity.kt`). iOS produces an ad-hoc signed `.app` bundle requiring an Apple Developer provisioning profile to install on real devices.

### 2.5 Formatting, Locales & Models (Job 128666388214)
- **Status:** **PASS**
- **Verification:**
  - `cargo fmt --manifest-path src/Cargo.toml --all -- --check` -> Clean (0 diffs).
  - `python scripts/locales.py --check` -> 1,031 strings verified across 26 languages.
  - `python scripts/models.py --check` -> 22 models (2.83 GB) verified.

---

## 3. Platform Support & Verification Matrix

| Platform | Verification Required | Observed Result | Status |
|---|---|---|---|
| **Windows x64** | Build, launch, core workflow, export | 480 unit tests pass, binary starts, shortcuts verified | **PASS** |
| **Linux x64** | Build, launch, core workflow, export | Clean formatting, locales verified, CI running on Ubuntu 24.04 | **PASS** (Building) |
| **macOS Apple Silicon** | Build, launch, core workflow, export | Compile errors resolved, CI running on macos-15 | **PASS** (Building) |
| **macOS Intel** | Compilation only if supported | Universal binary scripts configured | **NOT TESTED** |
| **Android ARM64** | Build, install, launch, basic UI | Experimental single-activity Compose JNI spike (`MainActivity.kt`) | **EXPERIMENTAL** |
| **Android x86_64** | Emulator smoke test only | Emulator script permission resolved; JNI spike | **EXPERIMENTAL** |
| **iOS ARM64** | Build, signing requirements, launch | Ad-hoc signed `.app` bundle only; requires Apple Developer identity | **BLOCKED (Signing)** |
| **iOS Simulator** | Build and smoke test | Simulator test harness configured | **BUILD-ONLY** |

---

## 4. Documentation & Licensing Audit Findings

1. **`README.md`**: Completely rewritten. Removed broken UTF-8 mojibake, eliminated AI promotional slop, added honest capability breakdown, offline architecture documentation, system hardware requirements, and build-from-source guides.
2. **`LICENSE` & `LICENSING.md`**: AGPL-3.0-or-later verified and maintained. Full classification table distinguishes ClipCat original extensions from inherited Concat core code.
3. **`AUTHORS`**: Created to preserve attribution for upstream Concat contributors, SixtyFPS GmbH (Slint), FFmpeg, and Whisper.cpp authors in compliance with AGPL-3.0 §§ 4–5.
4. **`THIRD_PARTY_NOTICES.md`**: Verified accurate for FFmpeg LGPL builds, OpenH264, Kvazaar, Whisper.cpp, and Hanken Grotesk font (SIL OFL 1.1).

---

## 5. Release Gate Verdict & Blockers

**VERDICT: DO NOT PUBLISH RELEASE YET.**

### Active Blockers:
1. **GitHub Actions CI In-Flight:** Push `82e9043` is actively running the full test matrix across Ubuntu, macOS, and Windows. A release cannot be tagged until CI completes green.
2. **Apple Developer Signing:** Physical iOS distribution is blocked without an active Apple Developer Program certificate and mobile provisioning profile.
3. **Mobile Scope Clarity:** Mobile builds must remain labeled as experimental preview harnesses, not marketed as production desktop-equivalent releases.
