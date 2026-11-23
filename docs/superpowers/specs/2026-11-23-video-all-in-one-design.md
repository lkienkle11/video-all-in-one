# Video All-in-One - product and four-sprint design

Date: 2026-11-23. Status: draft, waiting for the user's approval.

**Tóm tắt (tiếng Việt):** Video All-in-One là app desktop Rust-native (GPUI, không WebView) để tải, cắt, bỏ đoạn, ghép và xuất video. Sprint 1 làm khung phát hành và tự cập nhật trước, để mọi bản sau đều tới tay người dùng. Sprint 2 làm nền media, hàng đợi job bền vững và cắt video local. Sprint 3 làm tải online, bỏ đoạn và timeline trực quan. Sprint 4 làm project, Smart Paste, Batch và đóng gói v1.0.0.

## Intent and why

The author cuts local videos with FFmpeg, downloads public videos with yt-dlp, and downloads only chosen segments of long online videos, all by hand on the command line. The user wants one desktop app that does all of it with fewer steps. Source notes: three ChatGPT exports in `~/Downloads` (idea and market, web vs desktop and the 10 requirements, skills and stack).

The app is a video tool for download, cutting, removing sections, merging and exporting. It is not a video editor like CapCut or Premiere.

## The 10 requirements

| ID | Name | What it does |
|----|------|--------------|
| F01 | Local Video Cutting | Cut one or more segments from a local video |
| F02 | Online Video Downloading | Download a whole video from a supported public URL (yt-dlp) |
| F03 | Online Multi-Segment Downloading | Download only chosen segments of an online video |
| F04 | Remove Video Sections | Remove unwanted ranges, keep and join the rest |
| F05 | Project Save & Resume | Save a project and continue after reopening |
| F06 | Smart Paste | Paste URLs with timestamps and `full`; the app builds sources and jobs (plain parser, no AI) |
| F07 | Visual Clip Selector | Pick and adjust segments on a timeline with thumbnails, playhead, handles, preview |
| F08 | Multi-Segment Export | Export segments separately or merged into one video |
| S01 | Batch Processing | Run many sources and tasks in one go ("Process All") |
| S02 | Persistent Job Queue | Jobs with progress, cancel, retry; state survives restart |

F06, F07, F08 live inside one workspace, not on separate screens. Project Save & Resume is not job recovery: reopening a project does not resume an encode from the exact frame.

## Goals

- All 10 requirements working on Windows 10+, macOS 13+, Ubuntu 22.04+.
- One workspace: sources, preview, inspector, timeline, job queue.
- The app updates itself from GitHub Releases. Every release reaches installed machines.
- Local-first: no server, no account, video is processed on the user's machine.
- Fast Cut (stream copy, snaps to keyframes) and Precise Cut (re-encode) as two explicit modes. Export is verified with FFprobe before a job is reported done.

## Non-goals

- No video editing features beyond cut, remove, merge, export.
- No AI features (find by description, example-to-clips, transcript search), browser extension, study library, converter, audio extractor, compressor in sprints 1-4.
- No web version. No Apple Developer ID or Windows signing certificate (Gatekeeper and SmartScreen warnings on first install are accepted by the user).
- No beta update channel. No Linux AppImage, `.rpm`, mobile.

## Anti-goals (a failure even if it works)

- A forged or tampered update is installed.
- The private signing key is committed, logged or printed.
- A dev build touches the production database or the update endpoint.
- Installing an update loses the user's database or projects.
- The app silently overwrites or deletes a user's source video.
- A UI event handler runs FFmpeg or yt-dlp and freezes the window.
- User input reaches a shell string.

## Decisions made by the user

- Stack is final: Rust + GPUI + GPUI Kit, Tokio, SQLite (rusqlite), FFmpeg/FFprobe, yt-dlp. No WebView, no Tauri, no React, no C++. There is no fallback framework.
- The auto-updater is written in Rust by us (no plugin).
- The repository is public on GitHub, so the app reads Releases without a token.
- No code-signing certificate will be bought.
- Release and update come first (sprint 1). Dev and production builds are separate.
- Four sprints, each described separately; sprints 1 and 2 are built first.
- Branch for this work: `feat/init-video-all-in-one`.
- The app UI is bilingual, English and Vietnamese. On first launch a modal asks which language to use (English or Vietnamese) and shows the whole UI in that language; the choice can be changed later in Settings. No other languages in sprints 1-4.
- Modal rule for every modal and dialog in the app (language picker, update dialog, confirmations, errors): the content behind it is dimmed by a backdrop, and the content behind it cannot be scrolled or clicked while the modal is open. Focus stays inside the modal. Dialogs and sheets from GPUI Kit are used so the rule is applied in one place; a test checks that the background does not scroll.
- The repository has an About (description and topics) and a `README.md`. Both are written in English only, with no Vietnamese. The `CHANGELOG.md` stays bilingual (English, then Vietnamese) because it is the text shown in the update dialog.
- At least 85% of the code is Rust, measured by line count with `tokei` over source files (documentation, generated and vendored files excluded). CI fails if Rust drops below 85%. Release and helper scripts are therefore written in Rust (a `cargo xtask` crate), not shell, Node or Python, except where the platform needs a tiny file (for example a GitHub Actions YAML file or an installer manifest). [Agent's call: the measuring tool and the xtask approach.]

## Constraints and verified facts

- GPUI is pre-1.0. GPUI Kit pins one GPUI snapshot; player crates may pin another, so they may not compile together. Source: ChatGPT export 2 (not yet verified by us; verify in the sprint 1 spike).
- GPUI has no built-in video player. Options: `alhazen-gpui`, `gpui-video-player` (GStreamer, needs plugins in the installer), or FFmpeg decode drawn with GPUI. [Unverified.]
- `yt-dlp --download-sections` can be throttled on long YouTube videos and failed with some FFmpeg 8.x builds. Partial download always needs a full-download fallback. Source: yt-dlp issue tracker, cited in export 3.
- Stream-copy cuts snap to keyframes; never promise frame accuracy from them.
- Some yt-dlp extractors need a JavaScript runtime, so "pure Rust" still ships external executables.
- FFmpeg and yt-dlp licences must be checked before bundling. [To research in sprint 4; do not assume.]
- Downloading is limited by platform terms (YouTube terms). The product positions itself for videos the user owns or may process. The legal wording of the first-run notice is the user's call.
- Reference for release and update: `~/CustomFiles/aws_clf_c02_cards/easy_mindmap_knowledge` (Tauri app; its design uses the Tauri updater plugin, which we cannot use).
- Skills in the repo: `gpui-kit`, `gpui-kit-design-guides`, `gpui-video-timeline`, `media-pipeline-engineering`, `durable-local-jobs`, `rust`, `rust-async-patterns`, `sqlite`, `ffmpeg`, `security-and-hardening`, `structure-codebase`, plus agents `desktop-app-engineer`, `gpui-ui-engineer`, `devops-automator`.

## Approaches considered

### Order of work

1. **Release and update first (chosen by the user).** Costs: sprint 1 ships almost no video feature. Gain: every later release reaches installed machines; the riskiest OS-specific part is proven first.
2. Features first, update last. Rejected: builds installed before the updater existed can never update themselves.
3. Half and half. Rejected by the user.

### Updater

1. **Self-written Rust updater (chosen by the user).** Read `latest.json`, compare versions, download, verify a signature, install, restart. Costs: per-OS install code, key management.
2. Only open the Releases page. Rejected: no install-now.

### Preview in GPUI (decided in the sprint 1 spike, not now)

The user's rule: real playback (moving picture and sound) is required; a still-frame preview is not acceptable as the end state. The player may use a native library from another language (C or C++) if it is light, as long as it is linked as a library and our own code stays at least 85% Rust.

A. `alhazen-gpui`; B. `gpui-video-player` (GStreamer; heavy to ship, plugins needed); C. FFmpeg libraries (`libav*` through Rust bindings) decoding frames, an audio output crate for sound, our own sync, frames drawn by GPUI; D. `libmpv` (C library) through Rust bindings, using its software or OpenGL render API to hand frames to GPUI, with mpv doing decoding, audio and sync. [D and C are the agent's additions. Whether GPUI can show a stream of frames fast enough on each OS, and how heavy each option is in the installer, are unverified: the spike measures both.] All options sit behind a `VideoPlayerAdapter` port so the choice can change without touching core logic. A native child window drawn over GPUI is the last resort because it breaks modals and layout.

## Architecture

Modular monolith with ports and adapters, one Cargo workspace.

```text
video-all-in-one/
  apps/desktop/        GPUI views, components, timeline, UI state (no business rules)
  crates/core/         domain, services, ports (no GPUI dependency)
  crates/media/        FFmpeg, FFprobe, yt-dlp, process runner, thumbnails, cache, verification, player adapter
  crates/storage/      SQLite, migrations, repositories
  crates/platform/     file dialogs, OS paths, binary resolver, updater (OS install steps)
  docs/  scripts/  .github/workflows/
```

Rules: UI only observes and sends commands; the core decides job state. Long work runs on Tokio, never in a UI handler. Subprocess arguments are passed as argument lists, never as a shell string. Output is written to a temporary file and renamed only after FFprobe verification.

Create modules per sprint; do not create the full tree of empty files at once.

## Build channel and release (sprint 1)

- One switch: environment variable `APP_CHANNEL=production`, set only by the release workflow. Default is development. [Builder's call: Cargo feature, build-time constant, or both. The plan decides and tests it.]
- Development build: name "Video All-in-One (Dev)", own identifier and data folder, icon with a "DEV" band, no updater, never contacts the update endpoint.
- Production build: official name and data folder, updater active.
- `scripts/release/set-version X.Y.Z` updates every version file in one step and fails if they disagree. The release fails if the tag does not equal the version or is not higher than the latest release.
- Release workflow on tag `vX.Y.Z`: test, build macOS, Windows, Ubuntu with `APP_CHANNEL=production`, sign updates, publish one Release with installers and `latest.json` (version, notes, per-platform URL, size, signature).
- Signing: a key pair made once on the user's machine; the public key is compiled into the app; the private key lives only in GitHub Secrets and the user's backup. If the key is lost, installed apps can never update again.
- Release notes: `CHANGELOG.md` in English and Vietnamese per version; the update dialog shows them.

## Self-written updater (sprint 1)

1. Check `latest.json` on app start (production only, silent on failure) and when opening Settings.
2. If the version is higher: show size and notes; the user presses Download, then Install (confirmation dialog), as in the reference app.
3. Download with progress; verify the signature with the compiled-in public key before touching anything. [Builder's call: Ed25519 or minisign format; the plan chooses.]
4. Install and restart: macOS replace the app bundle, Windows run the installer, Ubuntu install the `.deb` with an administrator prompt. [Per-OS details are unverified; the sprint 1 spike must prove each one.]
5. User data is kept in a separate data folder; the updater never touches it.
6. The updater sits behind an interface (check, download, verify, install) so tests use a fake.

## The four sprints

Each sprint ends with a tagged release that earlier installs receive by themselves (from v0.2.0 on).

### Sprint 1 - Foundation, release and auto-update (v0.1.0, then v0.1.1 to prove the update)

| # | Work |
|---|------|
| 1.1 | Cargo workspace (`core`, `media`, `storage`, `platform`, `apps/desktop`); empty GPUI window builds on three OS |
| 1.2 | Lock the GPUI and GPUI Kit versions |
| 1.3 | Spike (time-boxed): play and seek one video behind `VideoPlayerAdapter`; a scrolling strip of hundreds of thumbnails; FFmpeg running in the background without freezing the UI |
| 1.4 | Dev/prod channel split |
| 1.5 | `set-version` script and tag checks |
| 1.6 | Signing key pair and GitHub Secrets |
| 1.7 | Release workflow and `latest.json` |
| 1.8 | Updater module and per-OS install steps |
| 1.9 | Settings screen: version, update states (checking, up to date, available, downloading with progress, ready, error with retry), confirmation dialog |
| 1.10 | Bilingual CHANGELOG shown in the dialog; `docs/RELEASING.md` |
| 1.11 | CI on pull requests for three OS, plus a check that Rust is at least 85% of the code |
| 1.12 | Localization layer (English and Vietnamese string tables, no hard-coded UI text), first-launch language picker as a modal, language setting in Settings; shared modal behaviour (dimmed backdrop, background not scrollable); the picker, Settings, update dialog and every sprint 1 screen are translated |
| 1.13 | English `README.md` (what the app is, features, screenshots later, install, build from source, update behaviour, licence, platform terms notice) |
| 1.14 | GitHub About, in English, applied with the user's permission when the repository is pushed: description, topics, optional website (see below) |

Done when: v0.1.0 is installed from the published installer; tag v0.1.1 is pushed; the app finds it, downloads, verifies, installs and restarts. The dev build never calls the update endpoint. The spike result (chosen player option) is written down.

### Sprint 2 - Media base, durable job queue, local cutting (v0.2.0)

| # | Work |
|---|------|
| 2.1 | Binary resolver (bundled copy, then PATH) with a clear error when missing |
| 2.2 | Process runner: spawn, progress parsing, cancel (including child processes on Windows), no orphan processes |
| 2.3 | SQLite: WAL, first migration, tables for sources and jobs |
| 2.4 | Persistent Job Queue (S02): states and valid transitions, persist before run, reconcile interrupted jobs on start, retry, cancel, separate limits for download and encode |
| 2.5 | Source analysis with FFprobe (duration, streams, codecs) |
| 2.6 | Workspace shell: toolbar, sources, preview placeholder, inspector, timeline placeholder, job queue panel |
| 2.7 | Local Video Cutting (F01): typed start/end, several segments per source; interval normalization (sort, clamp, merge overlap and adjacent) |
| 2.8 | Multi-Segment Export (F08): separate files or merged; Fast Cut and Precise Cut; export verification with FFprobe |
| 2.9 | Basic Batch (S01): several jobs queued together |

Done when: three segments of a local video export merged into one correct file; the app is closed mid-job and reopened, and the job resumes or fails with a clear message; v0.1.x installs update to v0.2.0 by themselves.

### Sprint 3 - Online sources, removing sections, visual timeline (v0.3.0)

| # | Work |
|---|------|
| 3.1 | Online Video Downloading (F02): URL metadata, quality choice, full download with progress |
| 3.2 | Online Multi-Segment Downloading (F03): partial download when the source supports it, always with a full-download fallback; do not download the same source twice in one batch |
| 3.3 | Remove Video Sections (F04): reuse the same segment model for keep and remove |
| 3.4 | Real video preview using the player chosen in the sprint 1 spike |
| 3.5 | Visual Clip Selector (F07): thumbnails, playhead, zoom, seek, drag handles, Keep/Remove mode, Boundary Preview |
| 3.6 | Preview for online sources: decide in the plan between a low-quality proxy download and no preview until downloaded |
| 3.7 | Network errors: retry, resume where the source allows, clear message for unsupported URLs |

Done when: a pasted URL gives two segments chosen on the timeline and exactly those two files; a middle section of a local video is removed and the rest joined.

### Sprint 4 - Projects, Smart Paste, full Batch, packaging (v1.0.0)

| # | Work |
|---|------|
| 4.1 | Project Save & Resume (F05): sources, segments, export settings; save at any step; references files, does not copy video |
| 4.2 | Smart Paste (F06): URLs with timestamps and `full`, grouped per URL, user reviews the parsed jobs before running |
| 4.3 | Batch Processing (S01) complete: many sources, per-source settings, Process All |
| 4.4 | Bundle FFmpeg and yt-dlp in installers after a licence check; keep yt-dlp updatable on its own |
| 4.5 | Cache management (never delete an entry a running job uses) and output naming templates |
| 4.6 | Manual full-flow test on a clean machine for each OS |
| 4.7 | First-run notice about platform terms and the Gatekeeper/SmartScreen warning; user guide |

Done when: from a clean install on each OS, several URLs with timestamps are pasted, processed as a batch, the project is saved, reopened and continued.

## Testing

- Interval normalization and job state transitions: property tests (`proptest`).
- Media: deterministic fixtures (single and multiple segments, overlapping and adjacent ranges, removal at start, middle and end, keyframe and non-keyframe boundaries, audio/video mismatch, corrupt source), checked with FFprobe and an FFmpeg decode pass.
- Jobs: normal completion, failure, cancel, shutdown mid-job, crash and restart, duplicate submit, missing source, low disk space.
- UI: GPUI UI integration tests (`#[gpui_kit::test]`) for the workspace flows.
- Updater: fake updater in unit tests; the real install path is verified by hand per OS in sprint 1.
- Every task follows test-first.

## Repository About and README (English only)

Proposed About text (the agent's draft; the user may change it):

- Description: `Rust-native desktop app to download, cut, remove sections from, merge and export videos. Built with GPUI, FFmpeg and yt-dlp.`
- Topics: `rust`, `gpui`, `desktop-app`, `video`, `ffmpeg`, `yt-dlp`, `video-cutter`, `video-downloader`, `cross-platform`.
- Website: left empty until a release page exists.

The README starts with the same one-line description, then: features (the 10 requirements in plain words), supported platforms, install (download from Releases, first-run Gatekeeper/SmartScreen note), updates (how the app updates itself), build from source, project layout, contributing, licence, and a notice that the user is responsible for using the app only on videos they own or may process. Setting the GitHub About and pushing the README are outward-facing steps and wait for the user's permission.

## Calls made by the agent (after the builder check; the user may change any of them)

- **Update install fallback.** If in-app install cannot work on one OS, the app downloads and verifies the installer, then opens it (or reveals it in the file manager) and tells the user to finish by hand. The sprint 1 done-criterion requires the full in-app flow on macOS; Windows and Ubuntu may use the fallback if the spike shows the full flow is not possible, and the gap is written in `RELEASING.md`.
- **Spike criteria and time box.** One local 1080p H.264, 30 fps file about 10 minutes long. Pass means: real playback with sound (not stills) and seek without crash on all three OS, installer size growth and idle CPU noted for each candidate, seek shows the new frame in under 500 ms, a strip of at least 500 thumbnails scrolls without visible stutter, and a running FFmpeg job leaves the UI responsive. Time box: 3 working days; at the end the result and the chosen option are written down.
- **Database safety.** Before a migration runs, the app copies the database file next to it. If the database has a newer schema than the app knows, the app refuses to open it and says so (no silent downgrade). Added to task 2.3.
- **Binaries before bundling.** In v0.2.0 and v0.3.0 FFmpeg, FFprobe and yt-dlp are found on `PATH` (or a configured folder), and the app shows a clear message with what to install when one is missing. Bundling stays in sprint 4 together with the licence check.
- **Manifest format.** `latest.json` carries a `schema` number from the first release, and the updater ignores a manifest with a schema it does not know (it never crashes on it). The format is frozen early because installed builds can never be fixed. No forced or minimum-version update in sprints 1-4.
- **Job resume policy.** Downloads resume where yt-dlp and the source allow (partial file kept). Encodes and cuts are marked failed after an interruption and can be retried from the start; their partial temp files are deleted on retry. A job is never half-published.
- **Concurrency limits.** Defaults: 2 downloads and 1 encode at a time. Not configurable in sprint 2; a settings option may come later.

## Open questions and what is left to the builder

- Choice of preview player (sprint 1 spike result).
- Signature format for updates (Ed25519 or minisign).
- Channel switch mechanism (Cargo feature or build constant).
- How the app finds a newer yt-dlp and whether it is replaced in place.
- Whether online preview uses a proxy download (sprint 3 plan).
- Licence terms for bundling FFmpeg and yt-dlp (sprint 4 research).

## Risks

| Risk | Effect | Handling |
|------|--------|----------|
| GPUI video playback does not work well | Visual Clip Selector blocked | Spike in sprint 1 behind an adapter with four candidates, including a linked C library (libmpv or FFmpeg libs); no framework fallback exists; if no candidate passes, stop and decide with the user |
| GPUI breaking change (pre-1.0) | Build breaks | Pin versions; upgrade only on purpose |
| Installing updates fails on one OS | Update flow broken | Prove on all three OS in sprint 1 |
| Lost signing key | No future updates | Backup in a password manager; documented in RELEASING.md |
| yt-dlp breaks when sites change | Downloads fail | Updatable yt-dlp; clear error text |
| Platform terms | Legal exposure | First-run notice; user decides final wording |
