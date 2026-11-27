# Post-release fixes: design

**Tóm tắt (tiếng Việt):** Sau khi v0.1.0 và v0.2.0 tới tay khách và đội tester, họ báo nhiều lỗi khiến app không dùng được như một app cho khách phổ thông: thiếu FFmpeg, Windows mở qua cmd, mất icon, không có hộp thoại chọn file/thư mục, không lọc video, không có toast, giao diện không co giãn, không có giới hạn cửa sổ, mất âm thanh, không có bản dev. Tài liệu này chốt nguyên nhân (đã đọc mã), phân loại lỗi nào sửa được bằng mã, lỗi nào cần tiền hoặc cần tái hiện, và chia việc sửa thành ba pha. Nguyên tắc chính: khách cài trên máy sạch thì dùng được ngay, không phải làm gì kỹ thuật.

Status: draft, waiting for the user's approval. Nothing in this document has been built.

## Intent and why

The app is for ordinary customers, not for developers or testers. A customer downloads an installer, installs it, opens it and processes a video, with nothing else installed and no technical step. The first releases do not meet that. The reports below come from the user and from the tester team, who tested the production installers on real machines. Sprint 1 and 2 were marked done without an install on a clean machine; `docs/RELEASING.md` itself says the Windows and Ubuntu packaging had not been run.

Success: on a clean macOS (Apple silicon and Intel), Windows 10+ and Ubuntu 22.04 machine, a customer installs the app, adds a video, cuts it, exports it, and hears the sound while previewing, without installing FFmpeg or any other tool, and every success or failure is shown as a toast.

## Goals

1. The installer carries everything the app needs, including FFmpeg and FFprobe, on all three operating systems.
2. The app opens like a normal desktop app: no console window on Windows, a correct icon, a sensible window size range.
3. File and folder choosing use the operating system's own dialogs; the file dialog shows only video files; the chosen file is validated again before it is used.
4. Success, failure and warnings are shown as toasts (info, success, warning, error), not inline above the Export button.
5. The layout stays usable when the window is made smaller.
6. Preview plays sound on production builds.
7. Every release builds both a development build and a production build from the same commit. Testers use the development build; customers receive the production build.
8. The customer sets the start and end of a segment by seeking the preview and pressing a button, not only by typing numbers.
9. A failed export tells the customer in plain words which video failed, why, and what to do; the queue shows the same, with the video's name.
10. The save folder is checked before the customer exports, so a folder that cannot be written to is found when it is chosen, not after Export.
11. Every user-visible text is plain language, in English and Vietnamese, with no programmer terms.

## Non-goals

- Downloading from a URL (F02, F03). It stays in Sprint 3 as in the design spec. The v0.1.0 changelog already says downloading is not part of it; the next changelog says so again in plain words.
- Notarization and paid code signing (needs a paid Apple Developer account and a Windows signing certificate; the user cannot pay for them now).
- Writing log files or crash reports, and any way for customers to send reports. Deferred.
- Any new editing feature (timeline, remove sections, projects, Smart Paste).

## Anti-goals

A fix that works only on a machine that has FFmpeg, brew, winget or apt. A fix that depends on the developer's machine. A "done" claim that was not proven by installing the packaged app on a clean machine of that operating system.

## Findings (read from the code; nothing was run)

| # | Report | Cause and place | Kind |
|---|--------|-----------------|------|
| 1 | Mac: app does not open after installing the DMG | App is signed ad hoc and not notarized (`docs/RELEASING.md`, First-run warnings). Two DMGs are published (`aarch64`, `x86_64`); a user on Apple silicon opened the Intel one (Rosetta warning in the user's screenshot). | Needs money for the real fix. Temporary: user guides customers (written in README by the user's later decision). Code part: one universal DMG. |
| 1b | Windows SmartScreen blocks the installer | Installer is not code-signed. | Needs money. Temporary: More info, Run anyway. |
| 1c | Ubuntu: is it blocked? | A local `.deb` installed with `apt install ./file.deb` is not signature-checked (Ubuntu security docs), so no Gatekeeper-style block. Opening it with the graphical installer was not verified. | Unverified; clean-machine test decides. |
| 2 | Missing FFmpeg dialog (Mac, Windows) | The design spec put bundling in Sprint 4 (item 4.4). `crates/platform/src/binary_resolver.rs` searches only the configured folder and `PATH`. A Finder-launched Mac app usually has no `/opt/homebrew/bin` in `PATH` (suspected, not tested). | Fixable in code. |
| 2b | Missing FFmpeg (Ubuntu) | `xtask/src/package/deb.rs` writes `Recommends: ffmpeg` only. `apt install ./file.deb` installs recommends; a graphical install or `dpkg -i` (used by the in-app updater) may not. | Fixable in code. |
| 3 | Windows opens the main window through a console | No `windows_subsystem = "windows"` in `apps/desktop/src/main.rs`. | Fixable, a few lines. |
| 4 | Production app has no icon | No resource is embedded in the Windows exe (no `build.rs`); no window icon is set in `main.rs` (GPUI has `WindowOptions.icon` and `app_id`). The macOS bundle has `AppIcon.icns`; Linux has the `hicolor` icon in the `.deb`. | Fixable. Mac and Linux need a check on a real install. |
| 5 | No folder chooser for the save path | The output folder is a text input (`export_panel.rs`). | Fixable. |
| 6 | File dialog shows every file type; no validation | `PathPromptOptions` in the pinned GPUI has no filter. `add_source_path` (`ui/workspace/mod.rs`) only runs FFprobe. | Fixable. `rfd` is already in the cargo registry. |
| 7 | Layout is not responsive | Panels use fixed `rem` sizes with no window minimum. | Fixable. |
| 8 | No toast | Errors are shown through `Status` in `export_panel.rs` and a modal in `ui/workspace/mod.rs`. `gpui-component` provides `notification.rs` and `push_notification`, unused by the app. | Fixable. |
| 9 | No URL feature | By design in Sprint 3 (F02, F03). | Not a defect; priority call stays with the user (kept in Sprint 3). |
| 10 | No minimum or maximum window size | `main.rs` uses `WindowOptions::default()`; GPUI has `window_min_size`. | Fixable. |
| 11 | No sound in preview, on every OS, on production installs | Not found by reading. The app enables real audio outside tests (`WorkspaceView::new`, `MpvPlayer::new`). The spike heard sound with Homebrew's libmpv; production uses our own LGPL libmpv build (Mac, Ubuntu) and a third-party DLL (Windows). | Cause unknown. Reproduce on a production package first. |
| 12 | No development build | `Channel::Development` exists (own name, identifier, data folder, DEV icon) but no development installer is built or published. | Fixable in the release pipeline. |
| 13 | Segment start and end can only be typed | `segment_editor.rs` has two text inputs, Add and Remove; nothing reads the preview position. The preview has Play and a seek slider (`preview.rs`). The visual timeline (F07) is a placeholder (`timeline_placeholder.rs`) and stays in Sprint 3. | Fixable: a button that fills a field from the current position. |
| 14 | Export errors are vague; the queue is hard to read | A failed job stores the raw error text (`RunFailure(error.to_string())`, English, technical). `job_queue_panel.rs` shows `#<id> · <state> · <percent> · <raw error>` with no video name. The queue does show completed and failed states. | Fixable. |
| 15 | Save folder is not checked before export | `export.rs` checks that the path is absolute and not a file, then calls `create_dir_all` (a deleted folder is silently recreated). Nothing checks that the folder can be written to; that is found when FFmpeg runs. Observed on Windows 11 on 2026-11-25 with v0.1.0. | Fixable. |
| 16 | Wording is too technical | Examples from `en.rs`: "absolute path", "Container", "keyframe", "re-encodes", "job #3", "control character", codec names in the inspector. | Fixable; a full review is Phase 2b. |

## Constraints

- Stack is fixed: Rust, GPUI, GPUI Kit, no WebView (see the design spec). Self-written updater stays.
- Only LGPL builds of FFmpeg and libmpv are bundled; the packager already refuses GPL or non-free (`docs/RELEASING.md`, Our own libmpv).
- GPUI never runs FFmpeg in an event handler; output goes to a temp file and is renamed after FFprobe verification (existing rule).
- The development build never calls the update endpoint (existing rule). Testers install development builds by hand.
- Tags, merges, pushes and publishing need the user's permission each time (project rules). Version numbers are chosen with the user at release time (release tag rule).
- Customers must never receive a development build. GitHub does not count a pre-release as the latest release, and the production app reads `releases/latest`; the builder confirms this before relying on it.

## Approaches considered

1. **One large fix release.** One release and one test round, but the unknown sound bug can hold up easy fixes and customers wait longer.
2. **Three phases, one small release each (chosen by the user's agreement).** The worst blockers reach customers first and each phase is small enough to test on clean machines.
3. **UI fixes first, packaging later.** Fast visible results, but customers still cannot install or open the app.

## Decisions by the user

- Approach 2, three phases.
- FFmpeg and FFprobe are bundled in the installers of all three operating systems (option A). Not Ubuntu only, not `Depends: ffmpeg`.
- The Gatekeeper and SmartScreen warnings stay for now; the user guides customers and the README carries the steps. Notarization is a later decision when money allows.
- The development build is for the tester team. Testers download and install it by hand; it does not self-update.
- Every release builds both the development and the production build.
- Only video files are accepted. The filter is inside the operating system's file dialog, and the file is validated again before use.
- The macOS DMG becomes one universal DMG.
- URL download stays in Sprint 3.
- Clean machines exist for all four environments (Apple silicon Mac, Intel Mac, Windows, Ubuntu 22.04) in the tester team.
- Log files and a report channel are out of scope for this round.
- Every phase ends, after its release succeeded, with a handoff file in `.context/` (ignored by git) that records the phase, status, state, work done, context and next steps, so that a tester session or a new agent can continue. Testing happens in a separate chat session.
- H.264 encoding for Precise cut uses OpenH264 inside an LGPL-only FFmpeg, not libx264 (GPL). The "LGPL only" rule stays.
- Windows FFmpeg has an ordered fallback, because the goal is that customers install nothing: (1) build LGPL FFmpeg with OpenH264 on a Windows runner; (2) a pinned LGPL prebuilt `ffmpeg.exe` whose Precise cut uses the Windows `h264_mf` encoder (source, licence and the encoder's presence must be verified first); (3) stop and ask the user, because the remaining options change the licence position (GPL FFmpeg) or drop Precise cut on Windows. A Windows "N" edition without the Media Feature Pack may lack `h264_mf`; testers check this on a clean machine.
- OpenH264 may carry H.264 patent duties that are separate from its source licence. The plan records the source and licence and lists the question in the phase report; it is the user's decision and does not block releases.
- Toasts: top right of the window; four types (Info, Success, Warning, Error), each with its own icon and colour and a title that names what failed, never only colour. Info and Success close by themselves; Warning and Error stay until closed. A batch with many failures is one toast ("5 videos could not be exported"); details are in the queue. Blocking dialogs stay only for decisions the customer must take.
- Failed exports: the toast title names the video, the description gives the reason and what to do (for example "Cannot write to the folder “D:\Phim”. Choose another folder."). The raw technical text is shown only through a "Copy details" button on the queue row.
- The queue is upgraded instead of adding a separate log screen: each row shows the video name, the state, the plain-language reason and the copy button. A failure kind (no permission, disk full, source missing, unknown) is stored with the job, so old jobs without a kind must still load.
- Save folder checks: when chosen or typed (after a short pause), when the window is focused again, when Export is pressed, and right before a queued job starts. A missing folder is created again (as today) with an Info toast; if it cannot be created the customer gets a plain error and chooses again. An existing folder is tested by creating and deleting a temporary file; permission bits alone are not trusted on Windows. No file-system watcher.
- Wording review is Phase 2b, a separate release. The before/after table is shown to the user before it is applied, and a test bans programmer terms in user-visible text.

## Phases

### Phase 1: installs and opens on a clean machine

- Bundle FFmpeg and FFprobe in the macOS, Windows and Ubuntu packages, with their licence texts and source URLs. The tool lookup order is: the bundled copy first, then the folder chosen in Settings, then `PATH`. If the bundled copy is missing, the app says the installation is damaged and asks the user to reinstall; it never tells the customer to run `brew`, `winget` or `apt`.
- Precise cut switches its encoder from `libx264` to OpenH264 (`media_planner.rs`). OpenH264 is not proven to match the current quality, so the plan includes a test of Precise cut with explicit pass criteria (valid output, correct cut points, acceptable quality) on all four clean machines, and Precise cut is not called done until it passes. The licence texts and source URLs for OpenH264 go into the package like the other third-party components.
- One universal macOS DMG. The builder checks whether libmpv and FFmpeg can be built for both architectures and merged; if that is not possible in a reasonable scope, report to the user and do not change direction silently. Update artifacts stay one zip per architecture (`macos-aarch64`, `macos-x86_64`) so installed apps keep updating.
- Windows: no console window.
- Icons: embedded in the Windows exe, set on the window, checked on macOS and Ubuntu installs.
- Window minimum size and a sensible maximum.
- Release pipeline builds the development build too (own installers named with a `dev` suffix, published as a pre-release that is never "latest") from the same commit as production.
- Ubuntu: the `.deb` carries FFmpeg, so no missing-tool dialog on any install route.
- Done when the testers install on all four clean machines, open the app by the documented steps, add a video, cut and export it, and never see a missing-FFmpeg dialog. This includes one machine that has another FFmpeg on `PATH` (the bundled copy must still be used) and one that has none.

### Phase 2: usable like a normal app

- Native file dialog for the source, video only (extension filter, list chosen by the builder: mp4, mov, mkv, avi, webm, m4v and similar). Native folder dialog for the save path, opening at the saved path on first show.
- Validation before use and before processing: the file exists, is readable, and has a real video stream (an audio-only file renamed `.mp4` is rejected). Re-validate at export time because the file may have changed.
- Toast for info, success, warning and error, using the existing `notification.rs`. Export success or failure, a batch result, and an unwritable save folder all appear as toasts. Errors no longer appear inline above the Export button.
- Responsive layout down to the window's minimum size.
- A button next to each segment's Start and End fields sets the field from the current preview position (disabled until a video is loaded). Fast cut still snaps to the nearest keyframe, so the button's help text says the cut may differ slightly. The full visual timeline stays in Sprint 3.
- Failure kinds, upgraded queue rows and the save-folder checks described in Decisions by the user.
- Done when the testers confirm each point on the development build on all four clean machines, including an unwritable save folder at each check point and, on Windows, a folder protected by Controlled folder access.

### Phase 2b: plain-language wording (separate release)

- Review every user-visible string in English and Vietnamese for programmer terms, and show the user a before/after table. Nothing is applied before the user approves the table.
- Apply it and add a test that fails when a banned term appears in user-visible text (for example "absolute path", "keyframe", "codec", "re-encode", "container", "job #").
- Done when the testers read the new wording on a clean machine and report nothing confusing.

### Phase 3: sound

- Reproduce first, on the production package: check mpv's own log for whether an audio device opens and whether an audio stream decodes, on each operating system. Hypotheses to test, none confirmed: our LGPL FFmpeg or libmpv build lacks something needed for audio; the app's use of libmpv differs from the spike.
- Fix the root cause, add a check that the packaging step can run without a speaker (for example, that the built libmpv reports an audio output and an audio decoder), and keep the tester's listening check on all four machines.
- Done when the testers hear sound on all four clean machines.

## Answers from the builder check

Answered by the project:

- Minimum macOS is 13.0 (`LSMinimumSystemVersion`, `xtask/src/package/macos.rs`). The Ubuntu floor is `libc6 >= 2.35` (22.04), already in `docs/RELEASING.md`.
- The Windows libmpv is the LGPL build of zhongfly/mpv-winbuild, pinned in `ci.yml` and `release.yml`. The Windows FFmpeg source for bundling is part of the licence check below.
- FFmpeg is already built from a pinned source archive (`xtask/src/libmpv/sources.rs`, FFmpeg 7.1.5, `--disable-gpl --disable-nonfree --disable-programs`). Bundling the `ffmpeg` and `ffprobe` programs means building them from the same pinned source with the programs enabled.

Calls made by the author (the user may change them):

- Fallback for the universal DMG: the plan stops and asks the user. Two DMGs with clear names is the fallback the user can choose then.
- Window size: the minimum size is enforced. GPUI has `window_min_size` and no matching maximum, so a maximum is added only if the pinned GPUI supports it without platform code; otherwise it is dropped and the user is told.
- Development release: same version number as production, published as a separate pre-release object tagged `dev-vX.Y.Z` by the same workflow run, never "latest". Production keeps `vX.Y.Z`. (`vX.Y.Z-dev` was rejected: it matches the `v*` trigger and would start the workflow again. The user approved `dev-vX.Y.Z`.)
- If the sound cause turns out to be simple, Phase 3 may ship before Phase 2.

Answered by the user: the encoder for Precise cut under an LGPL-only FFmpeg is OpenH264 (see Decisions by the user).

## What is left to the builder

Exact window sizes, the extension list, toast durations (suggested: info and success close by themselves after a few seconds; warning and error stay until closed), task order inside each phase, the universal-binary method, and the licence check for the bundled FFmpeg build. Mark each such choice as the builder's call in the plan.

## Verification

The tester checklist per operating system is part of the plan: install from the development build, follow the README steps for the system's warning, add a video, confirm the filter and the folder dialog, trigger an error (unwritable folder) and see the toast, resize the window, listen to the preview, export, check the icon, uninstall. A phase is not called done until it passed on all four clean machines or the missing ones are written down.

## Out of scope for this round

URL download, notarization and paid signing, a separate log screen, log files and report channels, the full visual timeline (F07), new editing features beyond the segment button.
