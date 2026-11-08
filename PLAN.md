# Concat Professional Editor Expansion — Implementation Plan

## Objective
Evolve Concat into a complete, professional, offline-first video editor inspired by Premiere Pro, DaVinci Resolve, CapCut, Alight Motion, and Filmora on an Intel Core i3 10th Gen, 8 GB RAM target machine.

## Baseline
- Regression Baseline Commit: `098cb15` (460 passed + 2 doctests).
- Current Branch: `feat/professional-editor-expansion`.
- Active Tested Count: 480 unit tests + 2 doctests (0 failed).

---

## Project ClipCat: Complete Branding & Professional UI Redesign
- [x] Inspect brand assets in `C:\Users\shaba\Downloads\favicon_io` and deploy icon suite.
- [x] Generate mipmapped icons (16, 32, 64, 128, 256, 512, .ico).
- [x] Update user-facing application branding (title, window, top bar mark, splash screen, installer, locales).
- [x] Add `WorkspacePreset::Custom` to workspace presets and Slint UI tab switcher.
- [x] Maintain backwards compatibility for `.concat` projects and internal crate namespaces.
- [x] Verify unslop adherence (dark palette `#212123`, quiet chrome, refined typography).
- [x] Full regression test verification (480 unit tests + 2 doctests).

---

## Wave 1: Unified Creative Asset Library & Offline Pack System (Current Focus)

### Milestone 1.1: Local Asset Database & Indexing
- [x] Create 16-category `AssetCategory` and `AssetItem` metadata structure.
- [x] Implement in-memory & JSON-backed `AssetDatabase` with category counts, favorites, and search.
- [x] Implement persistent disk storage manager in `concat-project` (`save_to_file` / `load_from_file`).
- [x] Unit tests for database serialization, search querying, and favorite toggling.

### Milestone 1.2: Modular Provider Engine & Graceful Offline Fallback
- [x] Define `AssetProvider` trait (`search`, `fetch_metadata`, `download`).
- [x] Implement `LocalAssetProvider` for installed & bundled files.
- [x] Implement `MyInstantsAssetProvider` with guaranteed offline fallback to bundled meme sound effects.
- [x] Add network connectivity detector (`NetworkStatus::is_offline()`) to immediately bypass network waits and serve cached assets without latency.
- [x] Unit tests verifying offline fallback and zero network stalls.

### Milestone 1.3: Pack File Format (.concatpack) & Safe Unpacker
- [x] Define `.concatpack` manifest specification (`AssetPackManifest`).
- [x] Implement path traversal sanitization (`sanitize_relative_path`, rejecting `..`, absolute paths, symlinks).
- [x] Implement folder pack installer `AssetPackInstaller::install_from_folder` with manifest validation and payload registration.
- [x] Automated security tests checking path traversal rejection and manifest parsing.

### Milestone 1.4: Slint Creative Library UI Integration
- [x] Add `in property <bool> offline-mode: false;` to `global Library` in `clip-inspector.slint`.
- [x] Add real-time Amber "Offline" status badge to `ShelfSearch` in `media-pane.slint`.
- [x] Bind `Library.offline_mode` to `NetworkStatus::is_offline()` in `src/crates/concat/src/lib.rs`.

### Milestone 1.5: Milestone Verification & Acceptance Gate
- [x] Automated test suite: `cargo test -p concat-project -p concat-core` (276 passed + 2 doctests).
- [x] Automated tests in `concat-effects` and `concat-text` (115 passed).
- [x] Total passing tests: 391 unit tests + 2 doctests across active crates.

---

## Wave 2: Dual Monitor & Workspace UI Polish (Next Focus)
- [ ] Connect side-by-side Source Monitor and Program Monitor in Editing workspace preset.
- [ ] Connect `Editor.performance_info` real-time frame/memory stats to `sysinfo.rs`.
- [ ] Verify workspace preset switching (`Editing`, `Color`, `Audio`, `MotionGraphics`, `Effects`).
