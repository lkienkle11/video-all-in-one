# Post-release fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Tóm tắt (tiếng Việt):** Kế hoạch sửa lỗi sau phát hành, chia ba pha, mỗi pha một bản phát hành. Pha 1: cài và mở được trên máy sạch (đóng gói FFmpeg, bộ mã hóa OpenH264, không mở qua cmd, icon, kích thước cửa sổ, DMG universal, bản dev). Pha 2: dùng như người bình thường (hộp thoại hệ điều hành, lọc và validate video, toast, giao diện co giãn). Pha 3: âm thanh (tái hiện trước, sửa sau khi có nguyên nhân). Mỗi pha chỉ được coi là xong khi tester xác nhận trên máy sạch.

**Goal:** A customer installs the app on a clean macOS, Windows or Ubuntu machine and uses it with nothing else installed; testers receive a development build from every release.

**Architecture:** Fixes stay inside the existing crates: `crates/platform` (tool lookup, native dialogs), `crates/core` (source validation, planner encoder), `apps/desktop` (window options, toast, dialogs wiring), `xtask` and `.github/workflows` (packaging, bundled tools, dev build, universal DMG). No new crate. FFmpeg and FFprobe are built from the pinned source already used for libmpv and shipped inside each package.

**Tech Stack:** Rust 1.99.1 (pinned in `release.yml`), GPUI + GPUI Kit 0.11.0, Tokio, `rfd` (native dialogs, new dependency), FFmpeg 7.1.5 (LGPL) with OpenH264, NSIS, `lipo`, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-27-post-release-fixes-design.md`

## Global Constraints

- Stack is fixed: Rust, GPUI, GPUI Kit, no WebView. Self-written updater stays.
- Only LGPL builds of FFmpeg and libmpv are bundled; the packager refuses GPL or non-free (`xtask/src/package/license.rs`).
- GPUI never runs FFmpeg in an event handler. Output goes to a temp file and is renamed after FFprobe verification. Never overwrite or delete a user's source video.
- The development build never calls the update endpoint. Testers install it by hand.
- Customers never receive a development build. Production `latest.json` lists only production artifacts.
- Only video files are accepted; the filter is in the operating system file dialog and the file is validated again before use.
- Precise cut encodes H.264 with OpenH264 (`libopenh264`), never `libx264`. Windows may use `h264_mf` only under the fallback chain of Task 1.5.
- Toasts: top right; four types (Info, Success, Warning, Error); info and success close by themselves; warning and error stay until closed; the title names what failed; a batch with many failures is one toast.
- Failed exports show a plain-language reason; raw technical text only behind a "Copy details" button on the queue row.
- The save folder is checked when chosen or typed, when the window is focused again, when Export is pressed and right before a queued job starts; a missing folder is created again with an Info toast; no file-system watcher.
- No programmer terms in user-visible text (banned-terms test, Phase 2b).
- Window: the minimum size is enforced.
- URL download, notarization, paid signing, log files and report channels are out of scope.
- Tags, merges, pushes and publishing need the user's permission each time. Version numbers are chosen with the user at release time.
- Commits follow `CLAUDE.md`: `type(scope): description`, body with Scope, What did, Reviewed, Tested; no AI attribution or co-author trailer.
- Work happens on a branch the user allows (never `main`). Each phase is its own branch and its own release.
- Every user-visible string exists in English and Vietnamese (`crates/core/src/i18n/`, checked by its tests).

## Review Focus

Inputs and conditions the spec implies but no obvious task covers; each has a test in the owning task.

1. An audio-only file renamed `movie.mp4` must be rejected, not queued (Task 2.1).
2. A file deleted or moved between choosing it and pressing Export must give a clear toast, not a crash (Task 2.3).
3. A source path with spaces and non-ASCII letters (for example `Phim gia đình 2026.mp4`) must work with the bundled FFmpeg on all three systems (Task 1.4).
4. A batch of many failing jobs must not bury the window in toasts (Task 2.4).
5. The development and production builds installed side by side must keep separate data, names and update behavior (Task 1.8).
6. A folder that passes the app's write test but is blocked for `ffmpeg.exe` (Windows Controlled folder access) must end in a plain-language failure, not a raw error (Tasks 2.5, 2.7).
7. Jobs stored by v0.1.0 and v0.2.0 have no failure kind; they must still load and show a sensible row (Task 2.5).

---

## Handoff to `.context/` (after every phase)

After a phase's release has succeeded and before the testers start, the agent writes one file `.context/YYYY-MM-DD-phase-<name>-handoff.md` so that another agent in another session can continue. `.context/` is ignored by git (`.gitignore`), so the file is never committed. The file is a curated summary written from the real state of the repository, not from memory and not a chat transcript. It has these headings, all required:

1. **Phase** — name, branch, version released, release links (production and dev), date.
2. **Status** — what is done, in progress, blocked; results of `cargo fmt --check`, `cargo clippy`, `cargo test --workspace`, `cargo xtask check-structure`, `cargo xtask lang-share`, with the real output summary.
3. **State** — `git status`, `git log` since the previous phase, tags, branch, workflow runs and their results, open pull requests.
4. **Work done** — per task of the phase: what changed, files, tests, deviations (`Ruling:` entries).
5. **Context** — the user's decisions and why, the author's calls (marked), unverified risks, what was learned about the machine or the tools.
6. **Handoff** — next steps in order, the tester checklist to run (path), pending permissions to ask the user (merge, tag, push, publish), open questions.
7. **Project rules** — the workflow steps and gates, commit rules, tag rule, the files to read first (`CLAUDE.md`, the spec, this plan, the skill selection file).

---

## Phase 1: installs and opens on a clean machine

Branch: `fix/phase-1-clean-install` (create only after the user allows it).

### Task 1.1: Windows runs without a console and carries its icon

**Files:**
- Modify: `apps/desktop/src/main.rs` (first line `#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]` is the builder's call between this and a feature-based switch; a development installer must also open without a console)
- Create: `apps/desktop/build.rs` (embeds `assets/icons/generated/<channel>/icon.ico` into the Windows exe; builder picks `winresource` or equivalent; channel from `CARGO_FEATURE_PRODUCTION`)
- Create: `xtask/src/package/pe.rs` (`pub(crate) fn pe_subsystem(bytes: &[u8]) -> Result<u16>`)
- Modify: `xtask/src/package.rs` (declare `mod pe`), `.github/workflows/release.yml` (Windows smoke step)
- Test: `xtask/src/package/pe.rs` (`#[cfg(test)]`)

**Interfaces:**
- Produces: `pe_subsystem(bytes) -> Result<u16>` returning the PE optional-header subsystem (2 = GUI, 3 = console).

- [ ] **Step 1: Write the failing tests** in `pe.rs`: `gui_pe_header_returns_2`, `console_pe_header_returns_3`, `non_pe_bytes_are_an_error`, using a minimal synthetic PE header built in the test.
- [ ] **Step 2: Run** `cargo test -p xtask pe_` — expected FAIL (function missing).
- [ ] **Step 3: Implement** `pe_subsystem` (read `e_lfanew`, check `PE\0\0`, read the 2-byte subsystem at optional-header offset 68).
- [ ] **Step 4: Run** the same command — expected PASS.
- [ ] **Step 5: Add** a `cargo xtask check-pe --file <exe> --gui` subcommand that fails unless the subsystem is 2; call it from the Windows smoke step of `release.yml` on the built `target/release/vaio-desktop.exe`.
- [ ] **Step 6: Add** the windows subsystem attribute and `build.rs` icon embedding. Verify: `cargo build -p vaio-desktop` on macOS still passes; the Windows run of the workflow passes the `check-pe` step.
- [ ] **Step 7: Commit** `fix(desktop): open Windows without a console and embed the app icon`.

### Task 1.2: Window options, app id and minimum size

**Files:**
- Create: `apps/desktop/src/window_options.rs` (`pub fn main_window_options(channel: Channel, title: SharedString) -> WindowOptions`)
- Modify: `apps/desktop/src/main.rs` (use it), `apps/desktop/src/lib.rs` (declare the module)
- Test: `apps/desktop/tests/window_options.rs`

**Interfaces:**
- Produces: `MIN_WINDOW_WIDTH_PX`, `MIN_WINDOW_HEIGHT_PX: f32` and `main_window_options`. Values are the builder's call; start at 960 x 600 and confirm that Task 2.5 fits.

- [ ] **Step 1: Write the failing tests:** `options_set_a_minimum_size`, `options_use_the_channel_identifier_as_app_id` (dev and production differ), `title_is_passed_through`.
- [ ] **Step 2: Run** `cargo test -p vaio-desktop --test window_options` — expected FAIL.
- [ ] **Step 3: Implement** with `window_min_size` and `app_id` from `Channel::identifier()`. Set `icon` only if the pinned GPUI supports it on the target (X11 per `docs/RELEASING.md`); otherwise leave it and say so in the commit body. A maximum size is added only if the pinned GPUI supports it without platform code; otherwise it is dropped and the user is told in the phase report.
- [ ] **Step 4: Run** the test — expected PASS.
- [ ] **Step 5: Commit** `feat(desktop): set the app id and a minimum window size`.

### Task 1.3: Bundled-first tool lookup and the damaged-install message

**Files:**
- Modify: `crates/platform/src/binary_resolver.rs`
- Modify: `apps/desktop/src/main.rs` (pass the bundled directory), `apps/desktop/src/ui/missing_tools.rs`, `crates/core/src/i18n/mod.rs`, `en.rs`, `vi.rs`, `i18n/tests.rs`
- Test: `crates/platform/src/binary_resolver_tests.rs`, `apps/desktop/tests/missing_tools.rs`

**Interfaces:**
- Produces: `pub fn bundled_tool_dir(exe: &Path, os: &str) -> Option<PathBuf>` (macOS `<exe>/../../Resources/bin`, Windows `<exe dir>/bin`, Ubuntu `<exe>/../../lib/video-all-in-one/bin`; returned only if the folder exists) and `pub fn resolve_in_with_bundled(tool, bundled: Option<&Path>, configured: Option<&Path>, path_var: Option<&OsStr>) -> Result<PathBuf, ResolveError>` (order: bundled, configured, PATH). Adds `Text::MissingToolDamaged` ("The installation is incomplete. Reinstall Video All-in-One.").

- [ ] **Step 1: Write the failing tests:** `bundled_wins_over_a_different_tool_on_path`, `configured_folder_is_used_when_nothing_is_bundled`, `path_is_the_last_resort`, `bundled_dir_is_none_when_the_folder_is_absent`, `bundled_dir_per_os_layout`; in `missing_tools.rs`: `bundle_present_but_tool_missing_shows_the_reinstall_message` and `no_bundle_keeps_the_existing_folder_dialog` (source runs).
- [ ] **Step 2: Run** `cargo test -p vaio-platform binary_resolver` and `cargo test -p vaio-desktop --test missing_tools` — expected FAIL.
- [ ] **Step 3: Implement** both functions and the new message; the install command text (`brew`, `winget`, `apt`) is removed from every packaged path and kept only for source runs.
- [ ] **Step 4: Run** the full suite `cargo test --workspace` — expected PASS.
- [ ] **Step 5: Commit** `feat(platform): prefer the bundled FFmpeg and FFprobe`.

### Task 1.4: Build FFmpeg and FFprobe with OpenH264 (LGPL), Mac and Ubuntu

**Files:**
- Modify: `xtask/src/libmpv/recipe.rs` (`ffmpeg_configure`: `--enable-ffmpeg --enable-ffprobe --disable-ffplay`, `--enable-libopenh264`, keep `--disable-gpl --disable-nonfree`)
- Modify: `xtask/src/libmpv/sources.rs` (new pinned row for OpenH264: https URL and SHA-256; builder checks the licence text and records the source URL), `steps.rs`, `layout.rs`, `pack.rs`, `manifest.rs`; raise `RECIPE_VERSION` and `LIBMPV_RECIPE` in the three workflow files together (a test checks they agree)
- Test: `xtask/src/libmpv/` unit tests next to the recipe

**Interfaces:**
- Produces: the libmpv folder gains `bin/ffmpeg` and `bin/ffprobe`; `libopenh264` is a shared library in `lib`.

- [ ] **Step 1: Write the failing tests:** `ffmpeg_configure_enables_openh264_and_never_gpl` (asserts `--enable-libopenh264` present, `--enable-gpl`, `--enable-libx264`, `--enable-nonfree` absent), `ffmpeg_programs_are_built`, `openh264_source_is_pinned_with_a_sha256`, `recipe_version_matches_workflows`.
- [ ] **Step 2: Run** `cargo test -p xtask libmpv` — expected FAIL.
- [ ] **Step 3: Implement** the recipe and source row. Legal note for the user: a H.264 encoder may carry patent licensing duties; the builder records the source and licence in `LICENSES` and lists the question in the phase report instead of deciding it.
- [ ] **Step 4: Run** `cargo xtask libmpv build --target macos --out <dir>` locally, then `cargo xtask libmpv pack`. Verify with `<dir>/bin/ffmpeg -hide_banner -encoders | grep openh264` and that `ffprobe` runs on a file named `Phim gia đình 2026.mp4` (Review Focus 3).
- [ ] **Step 5: Commit** `feat(xtask): build FFmpeg programs with OpenH264 in the libmpv recipe`.

### Task 1.5: Windows FFmpeg and FFprobe (LGPL, OpenH264)

**Files:**
- Modify: `.github/workflows/build-libmpv.yml` and `xtask/src/libmpv/` or add a Windows job; `.github/workflows/release.yml` and `ci.yml` (download step with a pinned SHA-256, like `MPV_WIN_URL`)

The goal is that a Windows customer installs nothing, so this task has an ordered fallback chain. Each step is time-boxed:

1. Build LGPL FFmpeg with OpenH264 on `windows-2025` (MSYS2 or MSVC) from the same pinned source as Mac and Ubuntu.
2. A pinned LGPL prebuilt `ffmpeg.exe` and `ffprobe.exe` (the LGPL variant of a public FFmpeg build project; verify the source, licence text, SHA-256 and that `-encoders` lists `h264_mf`). Precise cut on Windows then uses `h264_mf` (Task 1.6). Testers check `h264_mf` on a clean Windows machine; a Windows "N" edition without the Media Feature Pack may not have it.
3. Stop and ask the user. The remaining options (GPL FFmpeg, or no Precise cut on Windows) change the licence position or a feature and are never taken silently.

- [ ] **Step 1: Spike** along the chain and write the result in `docs/spikes/2026-11-windows-ffmpeg.md` (option reached, licence evidence, build time). Stop at step 3 of the chain and report before any other Windows work.
- [ ] **Step 2: Write the failing test** in `xtask/src/package/license.rs` tests: `windows_package_without_ffmpeg_is_refused` and `gpl_ffmpeg_exe_is_refused`.
- [ ] **Step 3: Implement** the download or build step with a pinned SHA-256 that fails clearly while unpinned.
- [ ] **Step 4: Verify** `ffmpeg.exe -encoders` lists `libopenh264` on the runner and `-version` shows no `--enable-gpl`.
- [ ] **Step 5: Commit** `ci(release): provide an LGPL FFmpeg for Windows`.

### Task 1.6: Precise cut picks an H.264 encoder that the FFmpeg really has

Decision of the user (2026-11-27): the encoder is chosen at run time from what the FFmpeg in use offers, in this order: `libopenh264`, `libx264`, `h264_mf` (Windows), `h264_videotoolbox` (macOS). Our packages carry an LGPL FFmpeg with OpenH264 and no x264 (Task 1.7 refuses a package whose FFmpeg lists `libx264`), so a release never uses `libx264`; a developer's own FFmpeg may. This replaces the earlier fixed switch to `libopenh264` and makes the `windows_uses_media_foundation` flag unnecessary, because Windows uses `h264_mf` when no better encoder exists.

**Files:**
- Modify: `crates/core/src/domain/export.rs` (`H264Encoder`, `pick_h264_encoder`), `crates/core/src/services/media_planner.rs` (`ExportRequest.precise_encoder`, arguments per encoder, `video_bitrate_bps`), `crates/media/src/ffmpeg.rs` (list the encoders of an FFmpeg), `crates/media/src/export_runner.rs` (probe, pick, pass into the request, plain failure when none), `apps/desktop/src/commands/export.rs` (the validation-only request)
- Modify (compiler-flagged propagation of the new field, nothing else): `crates/core/src/services/media_planner_fixtures.rs`, `media_planner_tests.rs`, `media_planner_guard_tests.rs`, `media_planner_windows_tests.rs`, `crates/media/src/ffmpeg_tests.rs`
- Test: `crates/core/src/domain/export.rs`, `crates/core/src/services/media_planner_tests.rs`, `crates/media/src/ffmpeg_tests.rs`, `crates/media/tests/export_runner.rs`

**Interfaces:**
- Produces: `pub enum H264Encoder { OpenH264, X264, MediaFoundation, VideoToolbox }` with `fn ffmpeg_name(&self) -> &'static str`; `pub fn pick_h264_encoder(available: &[String], os: &str) -> Option<H264Encoder>` (order above; `h264_mf` only when `os == "windows"`, `h264_videotoolbox` only when `os == "macos"`); `pub fn video_bitrate_bps(width: u32, height: u32) -> u32` (`width * height * 3`, clamped to 1_500_000..=20_000_000; the builder may change the factor and says so); `ExportRequest { precise_encoder: H264Encoder, .. }`; in `crates/media/src/ffmpeg.rs`: `pub fn parse_h264_encoders(encoders_output: &str) -> Vec<String>` (names of H.264 encoders in `ffmpeg -hide_banner -encoders` output) and `pub fn list_h264_encoders(ffmpeg: &Path) -> Result<Vec<String>, ...>`.

- [ ] **Step 1: Write the failing tests:** `pick_prefers_openh264_over_every_other`, `pick_prefers_x264_over_platform_encoders`, `pick_uses_media_foundation_only_on_windows`, `pick_uses_videotoolbox_only_on_macos`, `pick_returns_none_when_no_h264_encoder`, `the_lgpl_bundle_list_never_picks_x264` (a list with `libopenh264` and `h264_mf` but no x264); `precise_args_for_openh264_use_a_bitrate_and_no_crf_or_preset`, `precise_args_for_x264_keep_the_existing_crf_and_preset`, `bitrate_is_clamped`; `parse_h264_encoders_reads_the_encoders_listing`; in `export_runner.rs`: `precise_export_fails_with_a_plain_reason_when_the_ffmpeg_has_no_h264_encoder`. Existing integration tests that run Precise cuts against the machine's FFmpeg set `precise_encoder` to `H264Encoder::X264` in their requests.
- [ ] **Step 2: Run** `cargo test -p vaio-core -p vaio-media` — expected FAIL (items missing).
- [ ] **Step 3: Implement** the enum, the pure picker, the argument change per encoder (keep the segment and audio logic), the encoder listing and the runner's probe; the picker result is cached per runner.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS, with no existing test weakened.
- [ ] **Step 5: Quality gate (tester):** with the bundled FFmpeg, pass criteria are a valid file, cut points within the existing tolerance, and no visible blocking on the shared 1080p sample; failures are reported to the user, who decides whether to accept the quality difference. Run once with `libopenh264` (macOS, Ubuntu) and once with `h264_mf` (Windows).
- [ ] **Step 6: Commit** `feat(core): pick the H.264 encoder from what the FFmpeg offers`.

### Task 1.7: Packagers carry the tools, Ubuntu stops recommending FFmpeg

**Files:**
- Modify: `xtask/src/package/macos.rs`, `macos_libs.rs`, `nsis.rs`, `deb.rs` (remove `Recommends: ffmpeg`; install tools under `/usr/lib/video-all-in-one/bin`), `license.rs`, `xtask/src/package/tests.rs`
- Modify: `.github/workflows/release.yml` (smoke steps run `bin/ffmpeg -version` from the package)

**Interfaces:**
- Consumes: `bundled_tool_dir` layouts from Task 1.3. Produces: packages that contain `ffmpeg`, `ffprobe`, `libopenh264`, their licence texts and source URLs.

- [ ] **Step 1: Write the failing tests:** `deb_control_has_no_ffmpeg_recommendation`, `deb_lists_the_bundled_tools`, `macos_bundle_contains_resources_bin_tools`, `nsis_script_installs_bin_tools`, `package_without_tools_is_refused`, `package_whose_ffmpeg_lists_libx264_is_refused` (runs `ffmpeg -hide_banner -encoders` of the bundled copy).
- [ ] **Step 2: Run** `cargo test -p xtask package` — expected FAIL.
- [ ] **Step 3: Implement** in each packager; macOS tools are signed ad hoc with the rest of the bundle and checked for `/opt/homebrew` and `/usr/local` references like the other binaries.
- [ ] **Step 4: Run** `cargo test -p xtask` — expected PASS; run the CI smoke steps on all three runners.
- [ ] **Step 5: Commit** `feat(xtask): bundle FFmpeg and FFprobe in every installer`.

### Task 1.8: The development build and the dev release

**Files:**
- Modify: `xtask/src/package.rs` (`--channel production|dev`), `macos.rs`, `nsis.rs`, `deb.rs` (name, identifier, icon from `Channel`), `xtask/src/release/latest_json.rs`
- Modify: `.github/workflows/release.yml` (build both channels from the same commit; publish step creates a second pre-release)
- Modify: `docs/RELEASING.md`
- Test: `xtask/src/package/tests.rs`, `xtask/src/release/` tests

**Interfaces:**
- Produces: artifact names `video-all-in-one-<version>-dev-<platform_key>.<ext>`. `latest-json` must ignore every `-dev-` file.

**Ruling (approved by the user):** the dev pre-release is tagged `dev-vX.Y.Z`, not `vX.Y.Z-dev`, because `release.yml` runs on every pushed tag `v*` and the latter would start the workflow again.

- [ ] **Step 1: Write the failing tests:** `dev_artifact_names_carry_the_dev_suffix`, `latest_json_ignores_dev_artifacts`, `latest_json_fails_when_a_production_artifact_is_missing_even_if_dev_exists`, `dev_package_uses_the_dev_identifier_name_and_icon`, `dev_and_production_data_folders_differ` (in `crates/platform/src/channel.rs` tests; Review Focus 5), `dev_tag_does_not_match_the_release_trigger` (asserts `dev-v1.2.3` is not matched by the pattern `v*`).
- [ ] **Step 2: Run** `cargo test -p xtask` and `cargo test -p vaio-platform channel` — expected FAIL.
- [ ] **Step 3: Implement.** In `release.yml` the build job builds production (`--features production`) and dev (no feature) per OS; the publish job signs and publishes production as today, then creates `dev-vX.Y.Z` as a pre-release containing only dev artifacts (`gh release create ... --prerelease`, never `--latest`). Dev artifacts are never signed with the update key.
- [ ] **Step 4: Verify** with a manual `workflow_dispatch` run (builds, publishes nothing) that both sets of artifacts exist; verify after a real release that `releases/latest/download/latest.json` still resolves to the production release.
- [ ] **Step 5: Commit** `ci(release): build and publish a development build with every release`.

### Task 1.9: One universal macOS DMG

**Files:**
- Modify: `xtask/src/package/macos.rs`, `macos_libs.rs`, `xtask/src/package.rs` (`--universal` combining two thin app bundles with `lipo -create`), `.github/workflows/release.yml` (a combine job on macOS after both thin builds), `docs/RELEASING.md`
- Test: `xtask/src/package/tests.rs`

**Interfaces:**
- Produces: `video-all-in-one-<version>-macos-universal.dmg`. The two per-architecture zips stay unchanged (update artifacts, platform keys `macos-aarch64` and `macos-x86_64`).

Start with a spike: every Mach-O file in the two thin bundles (main binary, libmpv and its dylibs, `ffmpeg`, `ffprobe`) must merge with `lipo`. If it cannot be done in a reasonable scope, stop and report; the fallback the user may choose is two clearly named DMGs.

- [ ] **Step 1: Write the failing tests:** `universal_dmg_name`, `universal_merge_lists_every_macho_file`, `merge_refuses_files_present_in_only_one_arch`.
- [ ] **Step 2: Run** `cargo test -p xtask package` — expected FAIL.
- [ ] **Step 3: Implement** the merge and DMG step; the CI smoke step runs `lipo -archs` on every Mach-O in the result and expects `x86_64 arm64`.
- [ ] **Step 4: Verify** the DMG starts on the Apple silicon runner and on the Intel Mac (tester).
- [ ] **Step 5: Commit** `feat(xtask): publish one universal macOS DMG`.

### Task 1.10: Documentation and the tester checklist

**Files:**
- Modify: `docs/RELEASING.md` (bundled tools, dev release, universal DMG, remove the stale "ffmpeg is recommended" text), `CHANGELOG.md` (only at release time with the user)
- Create: `docs/testing/clean-machine-checklist.md`

- [ ] **Step 1: Write** the checklist from the spec's Verification section: install the dev build, follow the README steps for the system's warning, add a video, confirm the file filter and folder dialog (Phase 2), trigger an unwritable-folder error and see the toast (Phase 2), resize the window, listen to the preview, export in Fast and Precise, check the icon, uninstall. Add the Phase 1 cases: machine with another FFmpeg on `PATH` (bundled copy used) and machine with none; on Windows, Precise cut works (`h264_mf` or OpenH264, as built). Add the Phase 2 cases: the "set from preview" buttons; a save folder that cannot be written to at each check point (chosen, window focused again, Export, queued job); a folder deleted after it was chosen (recreated, Info toast); on Windows a folder protected by Controlled folder access; a batch with several failures (one toast, details in the queue); the queue shows the video name and "Copy details". Add the Phase 2b case: read every screen and report any wording that is unclear. Name the phase's handoff file (`.context/…-handoff.md`) at the top of the checklist so the tester knows which build and state is under test.
- [ ] **Step 2: Verify** `cargo xtask check-structure` passes with the new `docs/testing` folder.
- [ ] **Step 3: Commit** `docs: describe bundled tools, the dev release and the clean-machine checklist`.

### Task 1.13: Windows update removes the files of the old version and waits for the app

Added 2026-11-27 after reading the updater: the silent NSIS install only overwrites; files the new version no longer ships (for example a DLL whose name carries a version number) stay forever, and the installer does not wait for the running app to release its executable.

**Files:**
- Modify: `xtask/src/package/nsis.rs`, `xtask/src/package/tests.rs`, `crates/platform/src/update/install_windows.rs`, `crates/platform/src/update/install_tests.rs`

**Interfaces:**
- Produces: the generated installer script (a) waits up to 60 seconds until `$INSTDIR\<exe>` can be opened for writing and fails with a clear message if it cannot; (b) before copying, reads `$INSTDIR\install-manifest.txt` (if present) and deletes exactly the files it lists, never a folder tree, and only names that stay inside `$INSTDIR` (no `..`, no drive or absolute path); (c) after copying, writes the new `install-manifest.txt` with every installed file; the uninstaller also deletes the manifest. `install_windows.rs` keeps starting the installer with `/S`.

- [ ] **Step 1: Write the failing tests:** `script_waits_for_the_executable_to_be_writable`, `script_deletes_exactly_the_files_listed_in_the_previous_manifest`, `script_never_deletes_a_folder_tree_during_an_upgrade`, `manifest_entries_with_dotdot_or_absolute_paths_are_not_deleted`, `script_writes_a_manifest_of_every_installed_file`, `uninstaller_deletes_the_manifest`.
- [ ] **Step 2: Run** `cargo test -p xtask nsis` — expected FAIL.
- [ ] **Step 3: Implement** in the generated NSIS script (builder's call: how the manifest is read and validated inside NSIS; the plain-NSIS wait loop uses `FileOpen` in append mode). The destructive step follows the `security-and-hardening` rule for derived paths: resolved under the install folder, never above it.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS. The real behaviour is checked by the testers on Windows (upgrade from the previous release with the app open).
- [ ] **Step 5: Commit** `fix(xtask): remove old files and wait for the app when the Windows installer upgrades`.

### Task 1.14: Ubuntu update installs new system dependencies

Added 2026-11-27: the update runs `dpkg -i`, which does not install the packages named in `Depends:`; a new dependency makes the update fail half-way.

**Files:**
- Modify: `crates/platform/src/update/steps/deb.rs`, `crates/platform/src/update/install_linux.rs`, `crates/platform/src/update/install_tests.rs`

**Interfaces:**
- Produces: `pub fn apt_command(copy: &Path) -> Command` running `/usr/bin/apt-get install -y --no-install-recommends <absolute path of the private copy>` as an argument list (never through a shell), used when `/usr/bin/apt-get` exists; otherwise the existing `dpkg_command` runs and a failed run still reports the unmet dependency in plain words. The SHA-256 re-check of the private copy before the root step stays unchanged.

- [ ] **Step 1: Write the failing tests:** `apt_runs_with_an_argument_list_and_no_shell`, `apt_command_names_the_private_copy_by_absolute_path`, `apt_is_used_when_it_exists_and_dpkg_otherwise`, `mismatching_bytes_are_still_refused_before_any_installer_runs`.
- [ ] **Step 2: Run** `cargo test -p vaio-platform update` — expected FAIL.
- [ ] **Step 3: Implement** the command and the choice. Note for the report: apt may warn that the file is read by root outside its sandbox; that is expected.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS. Testers check an update that adds a dependency (remove it first) on Ubuntu 22.04.
- [ ] **Step 5: Commit** `fix(platform): install the new dependencies of an Ubuntu update`.

### Task 1.15: A clear message when the app data cannot be opened

Added 2026-11-27: `main.rs` prints the error and exits silently when the database cannot be opened. With the Windows console gone (Task 1.1) the customer sees nothing.

**Files:**
- Create: `apps/desktop/src/startup_error.rs`, `apps/desktop/tests/startup_error.rs`
- Modify: `apps/desktop/src/main.rs`, `apps/desktop/src/lib.rs`, `crates/core/src/i18n/mod.rs`, `en.rs`, `vi.rs`, `tests.rs`

**Interfaces:**
- Produces: `pub enum StartupProblem { NewerData, UnknownDatabase, Other }`, `pub fn problem_of(error: &StorageError) -> StartupProblem`, and a small window (`StartupErrorView`) that shows a plain-language title, one or two sentences, the data folder path and a Close button, in the customer's language (English when none is saved). Texts (both languages, no technical terms): newer data ("Your data was created by a newer version of Video All-in-One. Install the newest version to open it."), unknown database ("The data file in the data folder is not from Video All-in-One. Nothing was changed."), other ("Video All-in-One could not open its data. A backup copy was kept in the data folder."). `main.rs` opens this window instead of returning silently.

- [ ] **Step 1: Write the failing tests:** `schema_too_new_is_newer_data`, `foreign_database_is_unknown_database`, `other_errors_are_other`, `every_startup_text_exists_in_both_languages` (i18n), `the_window_shows_the_text_and_the_data_folder`, `the_window_closes_the_app_when_close_is_pressed`.
- [ ] **Step 2: Run** `cargo test -p vaio-desktop --test startup_error` and `cargo test -p vaio-core i18n` — expected FAIL.
- [ ] **Step 3: Implement.** The view follows `gpui-kit` and `gpui-kit-design-guides`; the language comes from the settings file when it can be read, English otherwise.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS.
- [ ] **Step 5: Commit** `feat(desktop): show a message when the app data cannot be opened`.

### Task 1.16: Upgrade cases in the tester checklist

Added 2026-11-27.

**Files:**
- Modify: `docs/testing/clean-machine-checklist.md`, `docs/RELEASING.md`

- [ ] **Step 1: Add** to each system's table, marked Phase 1: update in the app from the previous released version to the new production build (macOS, Windows, Ubuntu), with data and settings kept; Windows: update while the app is open and check that no old DLL stays in `<install folder>\bin` after a build that renames one; Ubuntu: update when the new package adds a dependency that is not installed (remove it first); open an older version over newer data and see the message of Task 1.15; update with little free disk space and see a clear failure that leaves the old version working.
- [ ] **Step 2: Add** a short "Upgrade checks" section to `docs/RELEASING.md` that says these checks are part of every release.
- [ ] **Step 3: Verify** `cargo xtask check-structure` passes.
- [ ] **Step 4: Commit** `docs: add the upgrade cases to the tester checklist`.

### Task 1.17: The update dialog says when work is still running

Added 2026-11-27: the confirmation ("Update now?") does not mention running exports; confirming quits the app and stops them.

**Files:**
- Modify: `apps/desktop/src/update_wiring.rs`, `apps/desktop/src/ui/update_dialog.rs`, `apps/desktop/src/state.rs`, `crates/core/src/i18n/mod.rs`, `en.rs`, `vi.rs`, `tests.rs`
- Test: `apps/desktop/tests/update_dialog.rs`, `apps/desktop/tests/updater.rs`

**Interfaces:**
- Produces: `AppState::running_job_count(&self) -> usize` (jobs that are Queued or Running); `pub enum UpdateChoice { AfterJobs, Now, Later }`. With no running work the dialog is unchanged. With running work it says how many exports are running ("N exports are still running. Updating now stops them.") and offers three buttons: update after they finish, update now (stops them), later. "After they finish" waits until the count is 0 and then starts the same install flow as "Update now".

- [ ] **Step 1: Write the failing tests:** `dialog_without_running_work_is_unchanged`, `dialog_with_running_work_names_the_count`, `after_jobs_waits_until_no_job_is_running_then_installs`, `now_installs_at_once_even_with_running_work`, `later_installs_nothing`, `every_new_text_exists_in_both_languages`.
- [ ] **Step 2: Run** `cargo test -p vaio-desktop --test update_dialog` and `cargo test -p vaio-core i18n` — expected FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS.
- [ ] **Step 5: Commit** `feat(desktop): tell the customer about running exports before an update`.

### Task 1.18: Windows update keeps the old files until the new ones are in place

Added 2026-11-27 (the "middle" level the user chose): macOS already restores the old app when a swap fails; Windows overwrites in place with no way back and no message. The fuller design (keep the old version on every system until the new one has started once) is left for a later phase.

**Files:**
- Modify: `xtask/src/package/nsis.rs`, `xtask/src/package/tests.rs`, `crates/platform/src/update/install_windows.rs`, `crates/platform/src/update/install_tests.rs`

**Interfaces:**
- Consumes: the `install-manifest.txt` of Task 1.13. Produces, in the generated installer script: before the first file is written, every file listed in the old manifest is copied to `$INSTDIR\.previous` (same relative names); if any copy or any `File` step fails, the files from `.previous` are copied back over the install folder, the new files that were added are removed, and a text file `update-error.txt` is written in the app's data folder (the same name and place the macOS updater uses, so the app already shows "The last update did not finish") saying what failed and where `.previous` is; on success the files listed in `.previous` are deleted one by one and the empty folder removed (never a folder tree). A fresh install (no old manifest) skips the backup.

- [ ] **Step 1: Write the failing tests:** `script_copies_the_old_manifest_files_to_previous_before_writing`, `script_restores_the_previous_files_when_a_step_fails`, `script_writes_update_error_txt_in_the_data_folder_on_failure`, `script_deletes_previous_file_by_file_on_success`, `script_never_removes_a_folder_tree_for_previous`, `fresh_install_skips_the_backup`, `data_folder_in_the_script_matches_the_apps_data_folder_for_both_channels`.
- [ ] **Step 2: Run** `cargo test -p xtask nsis` — expected FAIL.
- [ ] **Step 3: Implement** in the NSIS script, after Task 1.13 (same file). The builder reads `crates/platform/src/paths.rs` and `apps/desktop/src/app.rs` (`data_base`) so the script writes to the folder the app reads.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS. The real behaviour (a forced failure in the middle of an upgrade, app restored) is checked by the Windows testers.
- [ ] **Step 5: Commit** `feat(xtask): let the Windows installer undo a failed upgrade`.

### Task 1.11: Phase 1 gate

- [ ] **Step 1:** Run `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo xtask check-structure`, `cargo xtask lang-share --min 85`; read the output.
- [ ] **Step 2:** Manual dispatch of the release workflow; download the dev packages.
- [ ] **Step 3 (testers):** Run the checklist on the four clean machines. The phase is not done until each passes or the gap is written down and the user accepts it.
- [ ] **Step 4:** Propose the version number and size rating to the user (release tag rule); wait for the answer.

### Task 1.12: Handoff for Phase 1

**Files:**
- Create: `.context/YYYY-MM-DD-phase-1-handoff.md` (not committed)

- [ ] **Step 1:** After the Phase 1 release succeeded, read the real state (`git status`, `git log`, `gh release view` for both releases, the workflow run result) and write the file with every heading of "Handoff to `.context/`".
- [ ] **Step 2: Verify** that each of the seven headings is present: `grep -c '^## ' <file>` returns at least 7, and `git status --short` does not list the file.
- [ ] **Step 3:** Tell the user the path; the testers start after this.

---

## Phase 2: usable like a normal app

Branch: `fix/phase-2-normal-app`, from the merged Phase 1.

### Task 2.1: Source validation in core

**Files:**
- Create: `crates/core/src/domain/source_check.rs`; modify `crates/core/src/domain/mod.rs`
- Modify: `crates/core/src/i18n/mod.rs`, `en.rs`, `vi.rs`, `i18n/tests.rs`
- Test: `crates/core/src/domain/source_check.rs`

**Interfaces:**
- Produces: `pub const VIDEO_EXTENSIONS: &[&str]` (builder's call: mp4, mov, mkv, avi, webm, m4v and similar), `pub enum SourceError { NotFound, NotAFile, Unreadable, NotVideoExtension, NoVideoStream, ProbeFailed }`, `pub fn check_extension(path: &Path) -> Result<(), SourceError>`, `pub fn check_media(info: &MediaInfo) -> Result<(), SourceError>`, `pub fn source_error_text(error: &SourceError) -> Text`.

- [ ] **Step 1: Write the failing tests:** `docx_and_png_are_rejected_by_extension`, `extension_check_ignores_case`, `audio_only_media_info_is_rejected` (Review Focus 1: `MediaInfo` with audio streams and `video` empty), `media_with_a_video_stream_is_accepted`, `every_source_error_has_both_language_texts`.
- [ ] **Step 2: Run** `cargo test -p vaio-core source_check` — expected FAIL.
- [ ] **Step 3: Implement** the types and functions; existence and readability are checked in `crates/media/src/analysis.rs` (`analyze_source`) and return `NotFound`, `NotAFile` or `Unreadable` before calling FFprobe.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS.
- [ ] **Step 5: Commit** `feat(core): validate that a source is a readable video`.

### Task 2.2: Native file and folder dialogs

**Files:**
- Modify: `Cargo.toml` (workspace dependency `rfd`), `crates/platform/Cargo.toml`
- Create: `crates/platform/src/file_dialog.rs`; modify `crates/platform/src/lib.rs`
- Test: `crates/platform/src/file_dialog.rs` (pure parts only)

**Interfaces:**
- Consumes: `VIDEO_EXTENSIONS` (Task 2.1). Produces: `pub async fn pick_video_files(start_dir: Option<&Path>) -> Option<Vec<PathBuf>>` and `pub async fn pick_folder(start_dir: Option<&Path>) -> Option<PathBuf>`, plus the pure helper `pub fn existing_start_dir(candidate: Option<&Path>, fallback: Option<&Path>) -> Option<PathBuf>`.

- [ ] **Step 1: Write the failing tests:** `start_dir_uses_the_saved_path_when_it_exists`, `start_dir_falls_back_when_the_saved_path_is_gone`, `filter_name_and_extensions_match_the_core_list`.
- [ ] **Step 2: Run** `cargo test -p vaio-platform file_dialog` — expected FAIL.
- [ ] **Step 3: Implement** with `rfd::AsyncFileDialog` (`add_filter`, `set_directory`). The builder checks how `rfd` must be driven next to GPUI on macOS (main thread) and records the result.
- [ ] **Step 4: Run** the tests — expected PASS; manually open both dialogs on each OS (tester checklist).
- [ ] **Step 5: Commit** `feat(platform): native video file and folder dialogs`.

### Task 2.3: Use the dialogs and validate before and at export

**Files:**
- Modify: `apps/desktop/src/ui/workspace/mod.rs` (`prompt_for_source` calls `pick_video_files`; `add_source_path` runs `check_extension` first and `check_media` after FFprobe), `apps/desktop/src/ui/workspace/export_panel.rs` (folder button), `apps/desktop/src/commands/export.rs` (`ExportCommandError::SourceInvalid(SourceError)` and a re-check in `submit_export`), `apps/desktop/src/commands/batch.rs`
- Test: `apps/desktop/tests/workspace.rs`, `apps/desktop/tests/cutting.rs`, `apps/desktop/tests/batch.rs`

- [ ] **Step 1: Write the failing tests:** `adding_a_text_file_is_rejected_before_probing`, `adding_a_renamed_audio_file_is_rejected` (Review Focus 1), `export_fails_with_a_source_error_when_the_file_was_deleted` (Review Focus 2), `batch_skips_a_missing_source_and_reports_it`, `folder_button_starts_at_the_saved_folder`.
- [ ] **Step 2: Run** `cargo test -p vaio-desktop` — expected FAIL.
- [ ] **Step 3: Implement.** The old `SourceError` modal in `workspace/mod.rs` is removed; errors are reported through Task 2.4.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS.
- [ ] **Step 5: Commit** `feat(desktop): choose files and folders with the system dialogs and validate sources`.

### Task 2.4: Toasts

**Files:**
- Create: `apps/desktop/src/ui/toast.rs` (`pub enum ToastLevel { Info, Success, Warning, Error }`, `pub fn notify(window: &mut Window, cx: &mut App, level: ToastLevel, message: impl Into<SharedString>)` built on `push_notification` from `gpui_kit::component::notification`; info and success auto-close, warning and error do not)
- Modify: `apps/desktop/src/ui/mod.rs`, `export_panel.rs` (remove the inline `Status::Failed`, `BatchFailed` text above the Export button), `workspace/mod.rs` (toast for job completion and failure from `AppState` events), `crates/core/src/i18n/*`
- Test: `apps/desktop/tests/toast.rs`

- [ ] **Step 1: Write the failing tests:** `toasts_appear_at_the_top_right`, `info_and_success_close_by_themselves`, `warning_and_error_do_not_close_by_themselves`, `each_level_has_its_own_icon`, `toast_title_names_the_video` (for example title `Cannot export “Family.mp4”`), `export_failure_shows_a_toast_and_no_inline_error`, `unwritable_folder_shows_an_error_toast`, `fifty_failed_jobs_are_summarized_in_one_toast` (Review Focus 4; one toast such as "5 videos could not be exported", details in the queue; the grouping rule is the builder's call but must cap visible toasts). The pinned library notification supports a placement, an autohide flag and four types; the builder checks that grouping needs custom code.
- [ ] **Step 2: Run** `cargo test -p vaio-desktop --test toast` — expected FAIL.
- [ ] **Step 3: Implement** the helper and replace the inline status.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS.
- [ ] **Step 5: Commit** `feat(desktop): report results and errors as toasts`.

### Task 2.5: Failure kinds for exports

**Files:**
- Modify: `crates/core/src/domain/job.rs` (a failed job keeps an optional `FailureKind`), the job-store port and `RunFailure` in `crates/core/src/ports/` (the builder finds the exact files), `crates/media/src/export_runner.rs` (classify errors), `crates/storage/src/` (the database migration and the job store), `crates/core/src/i18n/*`
- Test: `crates/core/src/domain/job.rs`, `crates/storage/src/database_tests.rs`, `crates/storage/src/job_store_tests.rs`, `crates/media/tests/export_runner.rs`

**Interfaces:**
- Produces: `pub enum FailureKind { NoPermission, DiskFull, SourceMissing, FolderMissing, Unknown }`, `RunFailure::new(kind: FailureKind, detail: String)`, `Job::failure_kind(&self) -> Option<FailureKind>`, `pub fn classify_io_error(error: &std::io::Error) -> FailureKind`, and `pub fn failure_text(kind: Option<FailureKind>) -> Text` (a plain reason; `None` and `Unknown` give a generic reason).

- [ ] **Step 1: Write the failing tests:** `permission_denied_io_error_is_no_permission`, `disk_full_io_error_is_disk_full`, `missing_source_is_source_missing`, `unrecognized_error_is_unknown`, `failed_job_round_trips_its_kind_through_storage`, `database_with_jobs_from_v0_1_0_opens_after_the_migration` (Review Focus 7), `old_failed_job_loads_with_no_kind_and_a_generic_reason`, `every_failure_kind_has_plain_text_in_both_languages`.
- [ ] **Step 2: Run** `cargo test -p vaio-core -p vaio-storage -p vaio-media` — expected FAIL.
- [ ] **Step 3: Implement** the kind and a safe migration (a nullable column; never rewrite or drop existing rows; follow the existing migration pattern in `crates/storage`). The raw error text stays stored as `detail` for "Copy details".
- [ ] **Step 4: Run** the full suite — expected PASS.
- [ ] **Step 5: Commit** `feat(core): give failed exports a kind and a plain-language reason`.

### Task 2.6: A queue that explains itself

**Files:**
- Modify: `apps/desktop/src/ui/workspace/job_queue_panel.rs`, `apps/desktop/src/state.rs` (a row carries the video name), `crates/core/src/i18n/*`
- Test: `apps/desktop/tests/workspace.rs`, `apps/desktop/tests/batch.rs`

**Interfaces:**
- Consumes: `failure_text`, `Job::failure_kind` (Task 2.5). Produces: queue rows showing the video name, the state, the progress, the plain reason, and a "Copy details" button on failed rows that puts the raw text on the clipboard.

- [ ] **Step 1: Write the failing tests:** `row_shows_the_video_name_not_only_a_number`, `failed_row_shows_the_plain_reason_not_the_raw_error`, `copy_details_puts_the_raw_error_on_the_clipboard`, `old_failed_job_shows_a_generic_reason`, `completed_row_says_completed`.
- [ ] **Step 2: Run** `cargo test -p vaio-desktop` — expected FAIL.
- [ ] **Step 3: Implement.** The video name comes from the job payload's source path file name.
- [ ] **Step 4: Run** the full suite — expected PASS.
- [ ] **Step 5: Commit** `feat(desktop): show the video name and a plain reason in the queue`.

### Task 2.7: Check the save folder before the customer exports

**Files:**
- Create: `folder_check.rs` in the crate that `cargo xtask check-structure` allows for it (`crates/platform` or `crates/media`; the builder decides and says why)
- Modify: `apps/desktop/src/ui/workspace/export_panel.rs` (checks on change after a short pause, on window focus, on Export), `apps/desktop/src/commands/export.rs` (replace the silent `create_dir_all`), `crates/media/src/export_runner.rs` (check right before a job starts), `crates/core/src/i18n/*`
- Test: the new file, `apps/desktop/tests/cutting.rs`, `apps/desktop/tests/batch.rs`, `crates/media/tests/export_runner.rs`

**Interfaces:**
- Produces: `pub enum FolderCheck { Ready, Created, NotAbsolute, IsAFile, NotWritable, CannotCreate(String) }` and `pub fn check_output_folder(path: &Path) -> FolderCheck`. A missing folder is created; an existing folder is tested by creating and deleting a temporary file that leaves nothing behind.

- [ ] **Step 1: Write the failing tests:** `missing_folder_is_created_and_reported_as_created`, `writable_folder_is_ready_and_leaves_no_probe_file`, `read_only_folder_is_not_writable` (Unix; Windows is covered by the tester checklist), `path_that_is_a_file_is_rejected`, `relative_path_is_rejected`, `folder_that_cannot_be_created_is_cannot_create`; in the app: `typing_an_unwritable_folder_shows_a_message_under_the_field`, `focusing_the_window_again_rechecks_the_folder`, `export_with_an_unwritable_folder_queues_nothing_and_shows_an_error_toast`, `recreated_folder_shows_an_info_toast`, `default_folder_on_a_read_only_source_drive_falls_back_to_the_user_videos_folder` (fallback target is the builder's call); in the runner: `queued_job_whose_folder_was_deleted_recreates_it_before_starting`, `queued_job_whose_folder_became_read_only_fails_with_no_permission` (Review Focus 6).
- [ ] **Step 2: Run** `cargo test --workspace` — expected FAIL.
- [ ] **Step 3: Implement.** No file-system watcher. The Windows case where the app can write but `ffmpeg.exe` cannot (Controlled folder access) ends as `NoPermission` through Task 2.5 and is checked by testers.
- [ ] **Step 4: Run** the full suite — expected PASS.
- [ ] **Step 5: Commit** `feat(desktop): check the save folder before, during and after choosing it`.

### Task 2.8: Set a segment time from the preview

**Files:**
- Modify: `apps/desktop/src/ui/workspace/preview.rs` (`pub fn position(&self) -> TimeMs`), `apps/desktop/src/ui/workspace/segment_editor.rs` (a button next to Start and next to End), `apps/desktop/src/ui/workspace/mod.rs` (wiring), `crates/core/src/i18n/*` (`Text::SegmentSetFromPreview` and its help text: Fast cut may differ slightly)
- Test: `apps/desktop/tests/cutting.rs`

**Interfaces:**
- Produces: `PreviewView::position(&self) -> TimeMs`; the buttons write the position into the field in the same format the field parses (existing time parser in `crates/core/src/domain/time.rs`).

- [ ] **Step 1: Write the failing tests:** `set_start_from_preview_fills_the_start_field`, `set_end_from_preview_fills_the_end_field`, `buttons_are_disabled_until_a_video_is_loaded`, `end_before_start_still_shows_the_existing_row_error`, `position_is_written_in_a_format_the_field_accepts`.
- [ ] **Step 2: Run** `cargo test -p vaio-desktop --test cutting` — expected FAIL.
- [ ] **Step 3: Implement** the accessor and the two buttons.
- [ ] **Step 4: Run** the test — expected PASS.
- [ ] **Step 5: Commit** `feat(desktop): set a segment's start or end from the preview position`.

### Task 2.9: Layout that survives a small window

**Files:**
- Modify: `apps/desktop/src/ui/workspace/mod.rs` and the panel files (`source_list.rs`, `inspector.rs`, `export_panel.rs`, `job_queue_panel.rs`); panel minimum sizes must add up to at most the window minimum from Task 1.2
- Test: `apps/desktop/tests/workspace.rs`

- [ ] **Step 1: Write the failing tests:** `workspace_renders_at_the_minimum_window_size` (renders at `MIN_WINDOW_WIDTH_PX x MIN_WINDOW_HEIGHT_PX` and asserts the Export button, the segment inputs and the two "set from preview" buttons are inside the window bounds), `panel_minimums_fit_the_window_minimum`.
- [ ] **Step 2: Run** `cargo test -p vaio-desktop --test workspace` — expected FAIL.
- [ ] **Step 3: Implement** shrinking and overflow rules from `gpui-kit-design-guides` (panels collapse before controls overlap).
- [ ] **Step 4: Run** the tests — expected PASS.
- [ ] **Step 5: Commit** `fix(desktop): keep the workspace usable at the minimum window size`.

### Task 2.10: Phase 2 gate

- [ ] **Step 1:** Run the full verification commands of Task 1.11; read the output.
- [ ] **Step 2 (testers):** Run the full checklist on the dev build on the four clean machines.
- [ ] **Step 3:** Propose the version number to the user; wait for the answer.

### Task 2.11: Handoff for Phase 2

**Files:**
- Create: `.context/YYYY-MM-DD-phase-2-handoff.md` (not committed)

- [ ] **Step 1:** After the Phase 2 release succeeded, read the real state and write the file with every heading of "Handoff to `.context/`".
- [ ] **Step 2: Verify** the seven headings are present and the file is not listed by `git status --short`.
- [ ] **Step 3:** Tell the user the path; the testers start after this.

---

## Phase 2b: plain-language wording (separate release)

Branch: `fix/phase-2b-plain-wording`, from the merged Phase 2.

### Task 2b.1: The before/after table

**Files:**
- Create: `docs/superpowers/specs/2026-11-27-copy-review-table.md`

- [ ] **Step 1:** List every `Text` key with its current English and Vietnamese text (from `crates/core/src/i18n/en.rs` and `vi.rs`) and a proposed plain-language replacement for each text that uses a programmer term (for example "absolute path", "Container", "keyframe", "re-encodes", "job #", "control character", codec names, "Sources", "Inspector"). Mark which proposals are guesses.
- [ ] **Step 2: STOP.** Show the table to the user. Nothing is applied before the user approves it.

### Task 2b.2: Apply the wording and ban the terms

**Files:**
- Modify: `crates/core/src/i18n/en.rs`, `vi.rs`, `crates/core/src/i18n/tests.rs`

**Interfaces:**
- Produces: a `BANNED_TERMS_EN` and a `BANNED_TERMS_VI` list in the i18n tests (the approved table decides the entries).

- [ ] **Step 1: Write the failing tests:** `no_english_text_contains_a_banned_term`, `no_vietnamese_text_contains_a_banned_term`, and update the existing key-count test for any added or removed key.
- [ ] **Step 2: Run** `cargo test -p vaio-core i18n` — expected FAIL on the current wording.
- [ ] **Step 3: Apply** the approved table to `en.rs` and `vi.rs`; Vietnamese reviewed for natural wording, not literal translation.
- [ ] **Step 4: Run** `cargo test --workspace` — expected PASS.
- [ ] **Step 5: Commit** `fix(i18n): use plain language in every user-visible text`.

### Task 2b.3: Phase 2b gate

- [ ] **Step 1:** Run the full verification commands of Task 1.11; read the output.
- [ ] **Step 2 (testers):** Read the new wording on the dev build on the clean machines and report anything unclear.
- [ ] **Step 3:** Propose the version number to the user; wait for the answer.

### Task 2b.4: Handoff for Phase 2b

**Files:**
- Create: `.context/YYYY-MM-DD-phase-2b-handoff.md` (not committed)

- [ ] **Step 1:** After the Phase 2b release succeeded, read the real state and write the file with every heading of "Handoff to `.context/`".
- [ ] **Step 2: Verify** the seven headings are present and the file is not listed by `git status --short`.
- [ ] **Step 3:** Tell the user the path; the testers start after this.

---

## Phase 3: sound

Branch: `fix/phase-3-sound`. Because the cause is unknown, this phase has a decision point: after Task 3.1 the plan is amended with the fix tasks and the user approves the amendment.

### Task 3.1: Reproduce on the production package and find the cause

**Files:**
- Create: `xtask/src/libmpv/audio_check.rs` and subcommand `cargo xtask check-libmpv-audio --dir <libmpv folder> --fixture <file>`
- Create: `docs/spikes/2026-11-sound-diagnosis.md`
- Test: `xtask/src/libmpv/audio_check.rs`

**Interfaces:**
- Produces: a check that loads a fixture with an audio track through the packaged libmpv and reports `audio-codec-name`, `audio-params` and `current-ao`; it fails when no audio decoder or no audio output initializes.

- [ ] **Step 1: Write the failing tests:** `report_without_an_audio_codec_is_a_failure`, `report_with_codec_params_and_ao_passes`.
- [ ] **Step 2: Run** `cargo test -p xtask audio_check` — expected FAIL.
- [ ] **Step 3: Implement** the check. Run it against the production libmpv folder of each OS (testers run it on Windows and Ubuntu); compare with a Homebrew libmpv on the developer Mac, the build that was heard to have sound.
- [ ] **Step 4:** Write the findings (which component is missing or different: FFmpeg audio decoders, swresample, the audio output, or the app's use of libmpv) into `docs/spikes/2026-11-sound-diagnosis.md`.
- [ ] **Step 5: STOP.** Report the cause to the user. Amend this plan with fix tasks (a failing test first for each) and wait for approval. If the cause is simple, Phase 3 may ship before Phase 2.
- [ ] **Step 6: Commit** `test(xtask): check that the packaged libmpv decodes and outputs audio`.

### Task 3.2: Fix and guard (written after Task 3.1)

Not specified yet because the cause is not known. The amendment must include: the fix, a packaging check that runs `check-libmpv-audio` in CI on every OS, and the testers' listening check on the four clean machines.

### Task 3.3: Handoff for Phase 3

**Files:**
- Create: `.context/YYYY-MM-DD-phase-3-handoff.md` (not committed)

- [ ] **Step 1:** After the Phase 3 release succeeded, read the real state and write the file with every heading of "Handoff to `.context/`", including the root cause found in Task 3.1.
- [ ] **Step 2: Verify** the seven headings are present and the file is not listed by `git status --short`.
- [ ] **Step 3:** Tell the user the path; the testers start after this.

---

## Execution notes

- The cargo path may need the prefix used in the previous plan (`PATH=/opt/homebrew/opt/rustup/bin:$PATH`).
- Branches and worktrees are created only with the user's permission (project rule).
- Every deviation from this plan is recorded as a `Ruling:` in the ledger.
- Execution method chosen by the user: subagent-driven (`superpowers:subagent-driven-development`).
- Every phase ends with a handoff file in `.context/` (section "Handoff to `.context/`"), written by the orchestrating agent itself after the release succeeded and before the testers start. Testing happens in a different chat session, so the file is what that session reads first.
