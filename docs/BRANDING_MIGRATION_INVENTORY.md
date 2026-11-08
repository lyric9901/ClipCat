# ClipCat — Complete Branding Migration Reference Inventory

**Document Version:** 1.0.0  
**Date:** 2026-11-08  
**Repository:** [lyric9901/ClipCat](https://github.com/lyric9901/ClipCat)  
**Total References Audited:** 5,799 across 562 files  

---

## 1. Inventory Summary & Classification Rules

Every occurrence of `concat`, `Concat`, `CONCAT`, and `app.concat` has been scanned and classified into three operational categories:

1. **Category A: Legally / Historically Protected (Preserved Unaltered)**
   - Statutory copyright notices (`SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors`).
   - Upstream author credits in [`AUTHORS`](../AUTHORS).
   - Upstream issue tracking links in code comments (e.g. `https://github.com/jub0t/Concat/issues/320`).
   - Historical release notes and documentation provenance.
2. **Category B: Active User-Facing & System Migration Targets (Migrated to ClipCat)**
   - Application binary name: `clipcat` / `clipcat.exe`.
   - Desktop and Start Menu shortcut names and targets.
   - Operating system application identifier: `app.clipcat.editor` (with backward compatibility fallback).
   - Window title and application dialog headings.
   - Default project and media directories (`Desktop/ClipCat`, `Movies/ClipCat`).
   - Windows Inno Setup (`assets/windows/concat.iss` / `clipcat.iss`) and WiX (`concat.wxs` / `clipcat.wxs`).
   - Linux package descriptors (`assets/linux/nfpm.yaml`, `assets/arch/PKGBUILD`).
   - macOS bundle descriptor (`src/scripts/ios-app.sh`, `build-app.yml` Info.plist).
   - GitHub Actions CI/CD artifact names and executable paths (`ClipCat-*`).
3. **Category C: Architecture & Backward-Compatibility Preserved (Dual-Supported)**
   - Internal workspace member crate paths (`src/crates/concat-*`): maintained to ensure absolute zero regression in downstream dependencies.
   - Project manifests: `clipcat.json` as primary, `concat.json` and `wolfcut.json` preserved as legacy manifests.
   - Project lockfiles: `clipcat.lock` as primary, `concat.lock` supported.
   - Upstream model mirror URLs (`jub0t/Concat/releases/download/models-v1/`): preserved so neural network weights (2.83 GB) continue to download seamlessly.

---

## 2. Category Breakdown

### 2.1. Rust Source Code (`src/crates/`)
- **Total Occurrences:** 1,008 across 170 files
- **Sub-Breakdown:**
  - *SPDX Copyright Headers:* ~170 occurrences (Preserved under AGPL-3.0 compliance).
  - *Upstream Issue Links:* ~120 occurrences (e.g. `#320`, `#135`, `#223`) (Preserved as architectural context).
  - *Application Identifier (`IDENTIFIER`):* Located in `concat-host/src/dirs.rs`. Migrated from `"app.concat.editor"` to `"app.clipcat.editor"`. Former identifiers list `["app.concat.editor", "org.standalones.concat"]` maintained in `adopt()` to migrate existing user settings automatically.
  - *Model Path Search Vectors:* Located in `concat-host/src/enhance.rs`, `concat-render/src/upscale.rs`, `concat-vision/src/compact.rs`. Prepend `"app.clipcat.editor"` while retaining fallback to `"app.concat.editor"`.
  - *Project Manifest Naming:* In `concat-host/src/projects.rs`: `MANIFEST` changed to `"clipcat.json"`; `LEGACY_MANIFESTS` includes `"concat.json"` and `"wolfcut.json"`.
  - *Lock File Naming:* In `concat-host/src/lock.rs`: `FILE` set to `"clipcat.lock"`, with legacy check on `"concat.lock"`.
  - *Logging Environment:* In `concat-host/src/logs.rs`: `LEVEL_VARIABLE` set to `"CLIPCAT_LOG"` with fallback to `"CONCAT_LOG"`. Log files generated as `clipcat-{timestamp}.log`.
  - *Window Title & Fatal Dialogs:* In `concat/src/platform.rs`: `set_title("ClipCat")` and error message `"ClipCat could not start."`.
  - *Default Save Folders:* In `concat/src/panes/start.rs`: `location_on()` updated to create `Desktop/ClipCat` and `home("ClipCat")`.
  - *Temporary Export Files:* In `concat-export/src/lib.rs`: Staging filenames updated to `.{stem}.clipcat-video.{container}` and `.{stem}.clipcat-audio.{container}`, with cleanup filter matching both `.clipcat-` and `.concat-`.
  - *CLI Usage String:* In `concat/src/launch.rs`: Updated to `usage: clipcat ...`.

### 2.2. Slint UI (`src/crates/concat/ui/`)
- **Total Occurrences:** 96 across 34 files
- **Key References:**
  - `ui/app.slint`: Window title `title: "ClipCat";` and window icon `clipcat_logo_256.png` verified active.
  - `ui/dialogs/settings.slint`: Keyboard shortcut preset label `ClipCat (Default)` verified.
  - `ui/start.slint`: Project creation cards and documentation links pointing to ClipCat resources.

### 2.3. Manifests (`Cargo.toml`, `Cargo.lock`)
- **Total Occurrences:** 213 across 20 files
- **Key References:**
  - `src/crates/concat/Cargo.toml`: Added primary `[[bin]] name = "clipcat"` with secondary compatibility `[[bin]] name = "concat"`.
  - `src/.cargo/config.toml`: Updated developer alias `dev = "run --bin clipcat --profile quick"`.
  - Package metadata: `description = "ClipCat, the editor window: the Slint front end that drives the engine."`.

### 2.4. Packaging & Installers (`assets/windows/`, `assets/linux/`, `assets/arch/`)
- **Total Occurrences:** 30 across 3 files
- **Key References:**
  - `assets/windows/concat.iss` (and `clipcat.iss`):
    - AppName: `ClipCat`
    - Shortcut Filename: `{app}\clipcat.exe`
    - SetupIconFile: `..\icons\clipcat.ico`
    - UninstallDisplayIcon: `{app}\clipcat.ico`
  - `assets/windows/concat.wxs` (and `clipcat.wxs`):
    - Package Name: `ClipCat`
    - Shortcut Target: `[INSTALLFOLDER]clipcat.exe`
    - Safe Graphics Target: `[INSTALLFOLDER]clipcat.exe`
    - Product Icon: `clipcat.ico`
  - `assets/linux/nfpm.yaml`:
    - Package name: `clipcat`
    - Binary destination: `/opt/clipcat/clipcat`
    - Symlink: `/usr/bin/clipcat`
    - Desktop entry: `clipcat.desktop`
  - `assets/arch/PKGBUILD`:
    - `pkgname=clipcat`
    - Binary install: `/opt/clipcat/clipcat`

### 2.5. Workflows (`.github/workflows/`)
- **Total Occurrences:** 121 across 8 files
- **Key Workflows:**
  - `build-app.yml`:
    - Windows: Packages `clipcat.exe` and `concat.exe`, outputs `ClipCat-${VERSION}-windows-${arch}-setup.exe` and `.msi`.
    - macOS: Creates `ClipCat.app`, `ClipCat.icns`, and `ClipCat-${VERSION}-macos-${arch}.dmg`.
    - Linux: Stages `clipcat` executable, `clipcat.desktop`, `clipcat.png`, and builds `.deb`, `.rpm`, `.AppImage`, and `.pkg.tar.zst`.
    - Artifact naming: `ClipCat-windows-x86_64`, `ClipCat-macos-arm64`, `ClipCat-linux-x86_64`.
  - `ci.yml`:
    - Runs linting and test suites across all 14 workspace crates.
  - `release.yml`:
    - Collects all multi-platform assets and generates verified SHA-256 checksums under the ClipCat release identity.

### 2.6. Build & CI Scripts (`scripts/`, `src/scripts/`)
- **Total Occurrences:** 340 across 41 files
- **Key Scripts:**
  - `scripts/ci/smoke-windows.ps1`: Validates `clipcat.exe --version` and `clipcat.exe --smoke` with fallback to `concat.exe`.
  - `scripts/ci/smoke-linux.sh`: Validates `clipcat --version` and `clipcat --smoke` with fallback to `concat`.
  - `scripts/ci/smoke-macos.sh`: Validates `ClipCat.app/Contents/MacOS/clipcat` with fallback to `concat`.
  - `src/scripts/ios-app.sh`: Bundles `ClipCat.app` with `app.clipcat.editor` CFBundleIdentifier.
  - `scripts/models.py`: Preserves upstream mirror `jub0t/Concat` for 22 ONNX/Whisper model weights (2.83 GB).

### 2.7. Documentation & Licenses (`docs/`, `AUTHORS`, `LICENSE`)
- **Total Occurrences:** 2,684 across 272 files
- **Inventory Details:**
  - `AUTHORS`: Preserves complete legal attribution for Jareer Abdullah and 49 upstream contributors.
  - `LICENSE`: Preserves AGPL-3.0-or-later license text.
  - `THIRD_PARTY_NOTICES.md`: Preserves notices for Slint, FFmpeg, Whisper, ONNX Runtime, and OFL fonts.
  - Multi-language READMEs (`docs/README.*.md`): Upstream localized descriptions documenting the platform's lineage.
  - `docs/CODE_PROVENANCE.md`: Documents full legal classification matrix.
  - `docs/BRANDING_MIGRATION_REPORT.md`: Comprehensive final transition audit.
